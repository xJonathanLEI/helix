use std::mem::take;
use std::ops::{Deref, Range};
use std::sync::Arc;

use arc_swap::ArcSwap;
use helix_core::{Rope, RopeSlice};
use imara_diff::intern::InternedInput;
use imara_diff::Algorithm;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio::time::{timeout_at, Duration, Instant};

use crate::rope_line_cache::InternedRopeLines;
use crate::{LineDiff, LineDiffs};

#[cfg(test)]
mod test;

#[derive(Clone, Debug)]
pub struct Differ {
    channel: UnboundedSender<Event>,
    line_diffs: Arc<ArcSwap<LineDiffs>>,
}

impl Differ {
    pub fn new(diff_base: Rope, doc: Rope) -> Differ {
        Differ::new_with_handle(diff_base, doc).0
    }

    fn new_with_handle(diff_base: Rope, doc: Rope) -> (Differ, JoinHandle<()>) {
        let (sender, receiver) = unbounded_channel();
        let line_diffs: Arc<ArcSwap<LineDiffs>> = Arc::default();
        let worker = DiffWorker {
            channel: receiver,
            line_diffs: line_diffs.clone(),
            new_line_diffs: LineDiffs::default(),
        };
        let handle = tokio::spawn(worker.run(diff_base, doc));
        let differ = Differ {
            channel: sender,
            line_diffs,
        };
        (differ, handle)
    }
    pub fn get_line_diffs(&self) -> impl Deref<Target = impl Deref<Target = LineDiffs>> {
        self.line_diffs.load()
    }

    pub fn update_document(&self, doc: Rope) -> bool {
        self.channel.send(Event::UpdateDocument(doc)).is_ok()
    }

    pub fn update_diff_base(&self, diff_base: Rope) -> bool {
        self.channel.send(Event::UpdateDiffBase(diff_base)).is_ok()
    }
}

// TODO configuration
const DIFF_DEBOUNCE_TIME: u64 = 100;
const ALGORITHM: Algorithm = Algorithm::Histogram;

struct DiffWorker {
    channel: UnboundedReceiver<Event>,
    line_diffs: Arc<ArcSwap<LineDiffs>>,
    new_line_diffs: LineDiffs,
}

impl DiffWorker {
    async fn run(mut self, diff_base: Rope, doc: Rope) {
        let mut interner = InternedRopeLines::new(diff_base, doc);
        if let Some(lines) = interner.interned_lines() {
            self.perform_diff(lines);
        }
        self.apply_line_diff();
        while let Some(event) = self.channel.recv().await {
            let mut accumulator = EventAccumulator::new();
            accumulator.handle_event(event);
            accumulator
                .accumulate_debounced_events(&mut self.channel)
                .await;

            if let Some(new_base) = accumulator.diff_base {
                interner.update_diff_base(new_base, accumulator.doc)
            } else {
                interner.update_doc(accumulator.doc.unwrap())
            }

            if let Some(lines) = interner.interned_lines() {
                self.perform_diff(lines);
            }
            self.apply_line_diff();
        }
    }

    /// update the line diff (used by the gutter) by replacing it with `self.new_line_diffs`.
    /// `self.new_line_diffs` is always empty after this function runs.
    /// To improve performance this function tries to reuse the allocation of the old diff previously stored in `self.line_diffs`
    fn apply_line_diff(&mut self) {
        let diff_to_apply = take(&mut self.new_line_diffs);
        let old_line_diff = self.line_diffs.swap(Arc::new(diff_to_apply));
        if let Ok(mut cached_alloc) = Arc::try_unwrap(old_line_diff) {
            cached_alloc.clear();
            self.new_line_diffs = cached_alloc;
        }
    }

    fn perform_diff(&mut self, input: &InternedInput<RopeSlice>) {
        imara_diff::diff(ALGORITHM, input, |before: Range<u32>, after: Range<u32>| {
            if after.is_empty() {
                self.add_line_diff(after.start as usize, LineDiff::Deleted);
            } else {
                let tag = if before.is_empty() {
                    LineDiff::Added
                } else {
                    LineDiff::Modified
                };
                for line in after {
                    self.add_line_diff(line as usize, tag);
                }
            }
        })
    }

    fn add_line_diff(&mut self, line: usize, op: LineDiff) {
        self.new_line_diffs.insert(line, op);
    }
}

struct EventAccumulator {
    diff_base: Option<Rope>,
    doc: Option<Rope>,
}
impl EventAccumulator {
    fn new() -> EventAccumulator {
        EventAccumulator {
            diff_base: None,
            doc: None,
        }
    }
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::UpdateDocument(doc) => self.doc = Some(doc),
            Event::UpdateDiffBase(new_diff_base) => self.diff_base = Some(new_diff_base),
        }
    }
    async fn accumulate_debounced_events(&mut self, channel: &mut UnboundedReceiver<Event>) {
        let debounce = Duration::from_millis(DIFF_DEBOUNCE_TIME);
        let timeout = Instant::now() + debounce;
        while let Ok(Some(event)) = timeout_at(timeout, channel.recv()).await {
            self.handle_event(event)
        }
    }
}

enum Event {
    UpdateDocument(Rope),
    UpdateDiffBase(Rope),
}
