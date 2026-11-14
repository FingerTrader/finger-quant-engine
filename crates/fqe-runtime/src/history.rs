//! Historical visibility cursor and frozen prepared input manifest; no ledger or simulated fills.
use crate::events::ExecutionError;
use fqe_core::bars::ClosedBar;
use fqe_ir::{
    history::{HistorySelection, HistoryStrategy},
    model::ContentId,
};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoricalEvent {
    pub bar: ClosedBar,
    pub warmup: bool,
    pub gap_before: bool,
}
pub struct HistoricalCursor {
    selection: HistorySelection,
    next: usize,
    frontier: Option<u64>,
}
impl HistoricalCursor {
    pub fn new(selection: HistorySelection) -> Self {
        Self {
            selection,
            next: 0,
            frontier: None,
        }
    }
    pub fn emitted(&self) -> usize {
        self.next
    }
    pub fn frontier_ns(&self) -> Option<u64> {
        self.frontier
    }
    pub fn finished(&self) -> bool {
        self.next == self.selection.bars().len()
    }
    /// The caller supplies an inclusive availability frontier; no later closed bar is returned.
    pub fn next_until(&mut self, frontier: u64) -> Result<Option<HistoricalEvent>, ExecutionError> {
        if self.frontier.is_some_and(|t| frontier < t) {
            return Err(ExecutionError {
                code: "history_frontier",
                path: "$.frontier_ns".into(),
                detail: "availability frontier cannot move backwards".into(),
            });
        }
        self.frontier = Some(frontier);
        let Some(bar) = self.selection.bars().get(self.next) else {
            return Ok(None);
        };
        if bar.input().close_ns > frontier {
            return Ok(None);
        }
        let event = HistoricalEvent {
            bar: bar.clone(),
            warmup: self.next < self.selection.warmup_bars(),
            gap_before: self.next > 0
                && self.selection.bars()[self.next - 1].input().close_ns != bar.input().open_ns,
        };
        self.next += 1;
        Ok(Some(event))
    }
}
pub struct PreparedHistoricalRun {
    selection: HistorySelection,
    manifest: Arc<[u8]>,
}
impl PreparedHistoricalRun {
    pub fn new(
        selection: HistorySelection,
        strategy: &HistoryStrategy,
        run: &str,
        seed: u64,
    ) -> Result<Self, ExecutionError> {
        let manifest = fqe_ir::history::run_input_manifest(
            &selection,
            strategy,
            run,
            crate::source_identity::engine_id(),
            seed,
        )?;
        Ok(Self {
            selection,
            manifest: manifest.into(),
        })
    }
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }
    pub fn selection_id(&self) -> ContentId {
        self.selection.content_id()
    }
    pub fn cursor(&self) -> HistoricalCursor {
        HistoricalCursor::new(self.selection.clone())
    }
}
