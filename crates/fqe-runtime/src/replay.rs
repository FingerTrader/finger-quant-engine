//! Explicit user replay and first-observed divergence diagnostics over the pinned command profile.
use crate::{
    checkpoint::{CheckpointRuntime, CheckpointSeed},
    events::ExecutionError,
    trace::{TraceBuffer, TraceContext, TraceLimits, TraceRecord},
};
use fqe_ir::{
    checkpoint::{self, SnapshotDocument, SnapshotPins},
    model::ContentId,
};
use std::sync::Arc;
type Result<T> = std::result::Result<T, ExecutionError>;
/// A trusted diagnostic baseline, not an authenticity proof. Capture the collector used for this run.
#[derive(Clone, Debug)]
pub struct TraceReference {
    pins: SnapshotPins,
    limits: Option<TraceLimits>,
    entries: Vec<TraceRecord>,
    truncated: bool,
    commands: usize,
}
impl TraceReference {
    pub fn capture(runtime: &CheckpointRuntime, trace: &TraceBuffer) -> Result<Self> {
        let mut previous = 0;
        for record in trace.entries() {
            let index = record
                .command_index
                .ok_or_else(|| invalid("reference requires command-scoped observations"))?;
            if index < previous
                || index > previous + 1
                || index == 0
                || index > runtime.command_count() as u64
            {
                return Err(invalid("reference command index outside this history"));
            }
            previous = index;
        }
        if trace.limits().is_some()
            && !trace.truncated()
            && previous != runtime.command_count() as u64
        {
            return Err(invalid(
                "complete reference must cover the entire command prefix",
            ));
        }
        Ok(Self {
            commands: runtime.command_count(),
            pins: SnapshotDocument::from_bytes(&runtime.checkpoint()?)?.pins,
            limits: trace.limits(),
            entries: trace.entries().to_vec(),
            truncated: trace.truncated(),
        })
    }
    pub fn entries(&self) -> &[TraceRecord] {
        &self.entries
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}
fn invalid(detail: &str) -> ExecutionError {
    ExecutionError {
        code: "replay_reference",
        path: "$.trace_reference".into(),
        detail: detail.into(),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DivergenceKind {
    CommandOutcome,
    ObservedTrace,
}
#[derive(Debug)]
pub struct Divergence {
    pub kind: DivergenceKind,
    /// One-based command index, matching CommandReceipt.
    pub command_index: u64,
    pub expected_outcome: ContentId,
    pub actual_outcome: ContentId,
    pub expected_cursor: u64,
    pub actual_cursor: u64,
    /// Present only for an actual first observed trace mismatch; never a guessed hidden cause.
    pub location: Option<TraceContext>,
    pub expected: Option<TraceRecord>,
    pub actual: Option<TraceRecord>,
    pub trace: TraceBuffer,
}
pub struct ReplaySuccess {
    pub runtime: CheckpointRuntime,
    pub trace: TraceBuffer,
    pub verified_commands: usize,
    /// Complete reference comparison, as opposed to matching a captured prefix only.
    pub complete_trace_comparison: bool,
}
pub enum ReplayOutcome {
    Verified(Box<ReplaySuccess>),
    Diverged(Box<Divergence>),
}
/// Validate before factories; verify historical outcomes without publishing historical receipts.
/// A partial trace cannot prove all nodes equal: outcome verification still covers every command.
pub fn replay(
    seed: Arc<CheckpointSeed>,
    bytes: &[u8],
    limits: Option<TraceLimits>,
    reference: Option<&TraceReference>,
) -> Result<ReplayOutcome> {
    let saved = SnapshotDocument::from_bytes(bytes)?;
    let mut trace = match limits {
        Some(limits) => TraceBuffer::new(limits)?,
        None => TraceBuffer::off(),
    };
    if saved.pins != seed.pins()? {
        return Err(ExecutionError {
            code: "checkpoint_incompatible",
            path: "$.pins".into(),
            detail: "replay seed differs from the pinned checkpoint".into(),
        });
    }
    if let Some(reference) = reference
        && (reference.pins != saved.pins
            || reference.limits != limits
            || reference.commands != saved.entries.len())
    {
        return Err(invalid("reference pins/trace limits/command count differ"));
    }
    let mut runtime = CheckpointRuntime::new(seed)?;
    let mut compared = 0;
    for (i, expected) in saved.entries.iter().enumerate() {
        let actual = runtime.execute_traced(expected.command.clone(), &mut trace)?;
        let actual_outcome = checkpoint::observation_id(format!("{:?}", actual.result).as_bytes());
        let different = reference.and_then(|reference| {
            let until = trace.entries().len().min(reference.entries.len());
            let first = (compared..until).find(|j| reference.entries[*j] != trace.entries()[*j]);
            compared = until;
            first.map(|j| (reference.entries[j].clone(), trace.entries()[j].clone()))
        });
        if expected.outcome_id != actual_outcome
            || expected.output_cursor != actual.output_cursor
            || different.is_some()
        {
            let (kind, location, expected_trace, actual_trace, index) = match different {
                Some((expected, actual)) => (
                    DivergenceKind::ObservedTrace,
                    Some(actual.entry.context.clone()),
                    Some(expected),
                    Some(actual.clone()),
                    actual.command_index.unwrap_or(i as u64 + 1),
                ),
                None => (
                    DivergenceKind::CommandOutcome,
                    None,
                    None,
                    None,
                    i as u64 + 1,
                ),
            };
            return Ok(ReplayOutcome::Diverged(Box::new(Divergence {
                kind,
                command_index: index,
                expected_outcome: expected.outcome_id,
                actual_outcome,
                expected_cursor: expected.output_cursor,
                actual_cursor: actual.output_cursor,
                location,
                expected: expected_trace,
                actual: actual_trace,
                trace,
            })));
        }
    }
    if let Some(reference) = reference
        && !reference.truncated
        && !trace.truncated()
        && reference.entries.len() != trace.entries().len()
    {
        let expected_trace = reference.entries.get(compared).cloned();
        let actual_trace = trace.entries().get(compared).cloned();
        let observed = actual_trace
            .as_ref()
            .or(expected_trace.as_ref())
            .expect("different lengths");
        let index = observed.command_index.expect("command-scoped reference");
        let expected = &saved.entries[index as usize - 1];
        return Ok(ReplayOutcome::Diverged(Box::new(Divergence {
            kind: DivergenceKind::ObservedTrace,
            command_index: index,
            expected_outcome: expected.outcome_id,
            actual_outcome: expected.outcome_id,
            expected_cursor: expected.output_cursor,
            actual_cursor: expected.output_cursor,
            location: Some(observed.entry.context.clone()),
            expected: expected_trace,
            actual: actual_trace,
            trace,
        })));
    }
    let complete_trace_comparison = reference.is_some_and(|r| {
        r.limits.is_some() && !r.truncated && !trace.truncated() && r.entries == trace.entries()
    });
    Ok(ReplayOutcome::Verified(Box::new(ReplaySuccess {
        runtime,
        trace,
        verified_commands: saved.entries.len(),
        complete_trace_comparison,
    })))
}
