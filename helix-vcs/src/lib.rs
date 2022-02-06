use std::{collections::HashMap, path::Path};

pub use differ::Differ;
pub use git::Git;

mod differ;
mod git;
mod rope_line_cache;

const MAX_DIFF_LINES: usize = u16::MAX as usize;
// cap average line length to 128 for files with MAX_DIFF_LINES
const MAX_DIFF_BYTES: usize = MAX_DIFF_LINES * 128;

// TODO: Move to helix_core once we have a generic diff mode
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LineDiff {
    Added,
    Deleted,
    Modified,
}

/// Maps line numbers to changes
pub type LineDiffs = HashMap<usize, LineDiff>;

trait DiffProvider {
    /// Returns the data that a diff should be computed against
    /// if this provider is used.
    /// The data is returned as raw byte without any decoding or encoding performed
    /// to ensure all file encodings are handled correctly.
    fn get_diff_base(&self, file: &Path) -> Option<Vec<u8>>;
}
pub struct DiffProviderRegistry {
    providers: Vec<Box<dyn DiffProvider>>,
}

impl DiffProviderRegistry {
    pub fn get_diff_base(&self, file: &Path) -> Option<Vec<u8>> {
        self.providers
            .iter()
            .find_map(|provider| provider.get_diff_base(file))
    }
}

impl Default for DiffProviderRegistry {
    fn default() -> Self {
        // currently only git is supported
        // TODO make this configurable when more providers are added
        let git: Box<dyn DiffProvider> = Box::new(Git);
        let providers = vec![git];
        DiffProviderRegistry { providers }
    }
}
