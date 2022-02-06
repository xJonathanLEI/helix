use std::path::Path;

use git::objs::tree::EntryMode;
use git::sec::trust::DefaultForLevel;
use git::{Commit, ObjectId, Repository, ThreadSafeRepository};
use git_repository as git;

use crate::DiffProvider;

#[cfg(test)]
mod test;

pub struct Git;

impl Git {
    fn open_repo(path: &Path, ceiling_dir: Option<&Path>) -> Option<ThreadSafeRepository> {
        // custom open options
        let mut git_open_opts_map = git::sec::trust::Mapping::<git::open::Options>::default();

        // don't use the global git configs (not needed)
        let config = git::permissions::Config {
            system: false,
            git: false,
            user: false,
            env: true,
            includes: true,
        };
        // change options for config permissions without touching anything else
        git_open_opts_map.reduced = git_open_opts_map.reduced.permissions(git::Permissions {
            config,
            ..git::Permissions::default_for_level(git::sec::Trust::Reduced)
        });
        git_open_opts_map.full = git_open_opts_map.full.permissions(git::Permissions {
            config,
            ..git::Permissions::default_for_level(git::sec::Trust::Full)
        });

        let mut open_options = git::discover::upwards::Options::default();
        if let Some(ceiling_dir) = ceiling_dir {
            open_options.ceiling_dirs = vec![ceiling_dir.to_owned()];
        }

        ThreadSafeRepository::discover_with_environment_overrides_opts(
            path,
            open_options,
            git_open_opts_map,
        )
        .ok()
    }
}

impl DiffProvider for Git {
    fn get_diff_base(&self, file: &Path) -> Option<Vec<u8>> {
        debug_assert!(!file.exists() || file.is_file());
        debug_assert!(file.is_absolute());

        // TODO cache repository lookup
        let repo = Git::open_repo(file.parent()?, None)?.to_thread_local();
        let head = repo.head_commit().ok()?;
        let file_oid = find_file_in_commit(&repo, &head, file)?;

        let file_object = repo.find_object(file_oid).ok()?;
        Some(file_object.detach().data)
    }
}

/// Finds the object that contains the contents of a file at a specific commit.
fn find_file_in_commit(repo: &Repository, commit: &Commit, file: &Path) -> Option<ObjectId> {
    let repo_dir = repo.work_dir()?;
    let rel_path = file.strip_prefix(repo_dir).ok()?;
    let rel_path_components = byte_path_components(rel_path)?;
    let tree = commit.tree().ok()?;
    let tree_entry = tree.lookup_path(rel_path_components).ok()??;
    match tree_entry.mode {
        // not a file, everything is new, do not show diff
        EntryMode::Tree | EntryMode::Commit | EntryMode::Link => None,
        // found a file
        EntryMode::Blob | EntryMode::BlobExecutable => Some(tree_entry.oid),
    }
}

// TODO replace with lookup_path function in git-repository 0.24 once we can update to it

/// On unix paths are always raw bytes (that are usually utf-8) that can be passed to git directly.
/// This function is infallible
#[cfg(any(unix, target_os = "wasi"))]
fn byte_path_components(path: &Path) -> Option<impl Iterator<Item = &[u8]>> {
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    #[cfg(target_os = "wasi")]
    use std::os::wasi::ffi::OsStrExt;
    let components = path
        .components()
        .map(|component| component.as_os_str().as_bytes());
    Some(components)
}

/// On other platforms (windows) `OsStr` can only be converted to bytes if it is valid utf-8 (so fallible).
/// The path components need to be checked for invalid utf-8 before they can be passed to git.
/// Therefore this implementation collects the components into a temporary vector
#[cfg(not(any(unix, target_os = "wasi")))]
fn byte_path_components(path: &Path) -> Option<impl Iterator<Item = &[u8]>> {
    let components: Option<Vec<_>> = path
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .map(|component| component.as_bytes())
        })
        .collect();
    components.map(Vec::into_iter)
}
