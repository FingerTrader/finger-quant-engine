//! Bounded batch report adapter; CLI and library callers share the event evaluator.
use crate::events::{EventRuntime, ExecutionError, InputEvent};
use fqe_ir::{
    event_plan::CompiledEventPlan,
    exchange::{
        MarketData, MarketRecord, ReportContext, ReportObservation, ReportSignal, ReportSummary,
        ReportWriter,
    },
    model::ContentId,
};
use std::sync::Arc;
/// Stops on the first failure; returns no partial result and never retries a rejected record.
/// Runtime failures are annotated with the zero-based source record index.
pub fn run_report(
    plan: Arc<CompiledEventPlan>,
    run: &str,
    data: &MarketData,
    parameters_source_id: Option<ContentId>,
) -> Result<Vec<u8>, ExecutionError> {
    let mut runtime = EventRuntime::new(Arc::clone(&plan), run, data.source_id())?;
    let mut writer = ReportWriter::new(ReportContext {
        plan: &plan,
        run,
        dataset_id: data.source_id(),
        parameters_source_id,
        engine_version: env!("CARGO_PKG_VERSION"),
        interval_convention: runtime.manifest().interval_convention,
    })?;
    for (i, record) in data.events().iter().enumerate() {
        let input = match record {
            MarketRecord::Tick(t) => InputEvent::from(t.clone()),
            MarketRecord::Bar(b) => InputEvent::from(b.input().clone()),
        };
        let observation = runtime.step(input).map_err(|e| ExecutionError {
            path: format!(
                "$.events[{i}]{}",
                e.path.strip_prefix('$').unwrap_or(&e.path)
            ),
            ..e
        })?;
        writer.push(ReportObservation {
            source_sequence: observation.source_sequence,
            timestamp_ns: observation.timestamp_ns,
            matched: observation.matched,
            warmup: observation.warmup,
            outputs: &observation.outputs,
            signals: observation
                .signals
                .iter()
                .map(|s| ReportSignal {
                    source_sequence: s.source_sequence,
                    timestamp_ns: s.timestamp_ns,
                    signal: &s.signal,
                })
                .collect(),
        })?;
    }
    let state = runtime.finish()?;
    Ok(writer.finish(ReportSummary {
        processed_events: state.processed_events,
        matched_events: runtime.matched_events(),
        last_sequence: state.last_sequence,
        last_timestamp_ns: state.last_timestamp_ns,
    })?)
}
