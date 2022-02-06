use helix_core::Rope;
use tokio::task::JoinHandle;

use crate::{Differ, LineDiff};

impl Differ {
    fn new_test(diff_base: &str, doc: &str) -> (Differ, JoinHandle<()>) {
        Differ::new_with_handle(Rope::from_str(diff_base), Rope::from_str(doc))
    }
    async fn into_diff(self, handle: JoinHandle<()>) -> Vec<(usize, LineDiff)> {
        let line_diffs = self.line_diffs;
        // dropping the channel terminates the task
        drop(self.channel);
        handle.await.unwrap();
        let diffs = line_diffs.load();
        let mut res: Vec<_> = diffs.iter().map(|(&line, &op)| (line, op)).collect();
        res.sort_unstable_by_key(|&(line, _)| line);
        res
    }
}

#[tokio::test]
async fn append_line() {
    let (differ, handle) = Differ::new_test("foo\n", "foo\nbar\n");
    let line_diffs = differ.into_diff(handle).await;
    assert_eq!(&line_diffs, &[(1, LineDiff::Added)])
}

#[tokio::test]
async fn prepend_line() {
    let (differ, handle) = Differ::new_test("foo\n", "bar\nfoo\n");
    let line_diffs = differ.into_diff(handle).await;
    assert_eq!(&line_diffs, &[(0, LineDiff::Added)])
}

#[tokio::test]
async fn modify() {
    let (differ, handle) = Differ::new_test("foo\nbar\n", "foo bar\nbar\n");
    let line_diffs = differ.into_diff(handle).await;
    assert_eq!(&line_diffs, &[(0, LineDiff::Modified)])
}

#[tokio::test]
async fn delete_line() {
    let (differ, handle) = Differ::new_test("foo\nfoo bar\nbar\n", "foo\nbar\n");
    let line_diffs = differ.into_diff(handle).await;
    assert_eq!(&line_diffs, &[(1, LineDiff::Deleted)])
}

#[tokio::test]
async fn delete_line_and_modify() {
    let (differ, handle) = Differ::new_test("foo\nbar\ntest\nfoo", "foo\ntest\nfoo bar");
    let line_diffs = differ.into_diff(handle).await;
    assert_eq!(
        &line_diffs,
        &[(1, LineDiff::Deleted), (2, LineDiff::Modified)]
    )
}

#[tokio::test]
async fn add_use() {
    let (differ, handle) = Differ::new_test(
        "use ropey::Rope;\nuse tokio::task::JoinHandle;\n",
        "use ropey::Rope;\nuse ropey::RopeSlice;\nuse tokio::task::JoinHandle;\n",
    );
    let line_diffs = differ.into_diff(handle).await;
    assert_eq!(&line_diffs, &[(1, LineDiff::Added)])
}

#[tokio::test]
async fn update_document() {
    let (differ, handle) = Differ::new_test("foo\nbar\ntest\nfoo", "foo\nbar\ntest\nfoo");
    differ.update_document(Rope::from_str("foo\ntest\nfoo bar"));
    let line_diffs = differ.into_diff(handle).await;
    assert_eq!(
        &line_diffs,
        &[(1, LineDiff::Deleted), (2, LineDiff::Modified)]
    )
}

#[tokio::test]
async fn update_base() {
    let (differ, handle) = Differ::new_test("foo\ntest\nfoo bar", "foo\ntest\nfoo bar");
    differ.update_diff_base(Rope::from_str("foo\nbar\ntest\nfoo"));
    let line_diffs = differ.into_diff(handle).await;
    assert_eq!(
        &line_diffs,
        &[(1, LineDiff::Deleted), (2, LineDiff::Modified)]
    )
}
