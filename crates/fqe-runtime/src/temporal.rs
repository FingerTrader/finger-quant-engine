//! Host-owned temporal memory; no wall clock, language object or implicit missing-bar synthesis.
use crate::{events::ExecutionError, workflow::WorkflowObservation};
use fqe_core::types::{Evaluation, ScalarValue};
use fqe_ir::{
    temporal::{BoundTemporalRules, WindowSpan},
    workflow::Source,
};
use std::{collections::BTreeMap, sync::Arc};
type Result<T> = std::result::Result<T, ExecutionError>;
fn error(code: &'static str, path: &str, detail: &str) -> ExecutionError {
    ExecutionError {
        code,
        path: path.into(),
        detail: detail.into(),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowState {
    pub id: String,
    pub opened_sequence: u64,
    pub opened_ns: u64,
    pub observed_bars: u64,
    pub deadline_ns: Option<u64>,
    pub satisfied: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpiryCause {
    ClosedBar { logical_sequence: u64 },
    Watermark,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowNotice {
    pub window: String,
    pub opened_sequence: u64,
    pub opened_ns: u64,
    pub expired_ns: u64,
    pub satisfied: bool,
    pub cause: ExpiryCause,
}
#[derive(Clone)]
pub(crate) struct TemporalBook {
    rules: Arc<BoundTemporalRules>,
    active: BTreeMap<String, WindowState>,
}
impl TemporalBook {
    pub(crate) fn new(rules: Arc<BoundTemporalRules>) -> Self {
        Self {
            rules,
            active: BTreeMap::new(),
        }
    }
    pub(crate) fn active(&self) -> &BTreeMap<String, WindowState> {
        &self.active
    }
    pub(crate) fn next_due(&self) -> Option<(u64, String)> {
        self.active
            .values()
            .filter_map(|v| v.deadline_ns.map(|t| (t, v.id.clone())))
            .min()
    }
    pub(crate) fn advance(&mut self, event: &WorkflowObservation) -> Result<Vec<WindowNotice>> {
        let mut notices = vec![];
        if !event.matched {
            return Ok(notices);
        }
        for rule in self.rules.rules() {
            if let Some(active) = self.active.get_mut(&rule.id) {
                if active.deadline_ns.is_some_and(|d| event.timestamp_ns > d) {
                    return Err(error(
                        "temporal_frontier",
                        &rule.id,
                        "deadline must be dispatched before later market event",
                    ));
                }
                if !event.warmup && flag(event, &rule.predicate)? {
                    active.satisfied = true;
                }
                if let WindowSpan::ClosedBars(n) = rule.span {
                    active.observed_bars =
                        active.observed_bars.checked_add(1).ok_or_else(|| {
                            error("temporal_overflow", &rule.id, "bar count overflow")
                        })?;
                    if active.observed_bars == n {
                        let closed = self.active.remove(&rule.id).expect("active window");
                        notices.push(notice(
                            closed,
                            event.timestamp_ns,
                            ExpiryCause::ClosedBar {
                                logical_sequence: event.source_sequence,
                            },
                        ));
                    }
                }
                // Active re-arms are ignored, including the event that consumes a count expiry.
            } else if !event.warmup && flag(event, &rule.arm)? {
                let deadline_ns = match rule.span {
                    WindowSpan::ClosedBars(_) => None,
                    WindowSpan::EventTime(n) => {
                        Some(event.timestamp_ns.checked_add(n).ok_or_else(|| {
                            error(
                                "temporal_overflow",
                                &rule.id,
                                "event-time deadline overflow",
                            )
                        })?)
                    }
                };
                self.active.insert(
                    rule.id.clone(),
                    WindowState {
                        id: rule.id.clone(),
                        opened_sequence: event.source_sequence,
                        opened_ns: event.timestamp_ns,
                        observed_bars: 0,
                        deadline_ns,
                        satisfied: false,
                    },
                );
            }
        }
        Ok(notices)
    }
    pub(crate) fn expire(&mut self, time: u64, id: &str) -> Result<WindowNotice> {
        let state = self
            .active
            .get(id)
            .ok_or_else(|| error("temporal_timer", id, "timer absent"))?;
        if state.deadline_ns != Some(time) {
            return Err(error("temporal_timer", id, "timer deadline differs"));
        }
        let state = self.active.remove(id).expect("validated timer");
        Ok(notice(state, time, ExpiryCause::Watermark))
    }
    pub(crate) fn clear(&mut self) -> Vec<WindowState> {
        std::mem::take(&mut self.active).into_values().collect()
    }
}
fn notice(state: WindowState, time: u64, cause: ExpiryCause) -> WindowNotice {
    WindowNotice {
        window: state.id,
        opened_sequence: state.opened_sequence,
        opened_ns: state.opened_ns,
        expired_ns: time,
        satisfied: state.satisfied,
        cause,
    }
}
fn flag(event: &WorkflowObservation, source: &Source) -> Result<bool> {
    let value = match source {
        Source::Event(n) => event.event_outputs.get(n),
        Source::Instance { instance, output } => {
            event.outputs.get(instance).and_then(|v| v.get(output))
        }
    }
    .ok_or_else(|| {
        error(
            "temporal_source",
            "$.window",
            "bound workflow output absent",
        )
    })?;
    match value {
        Evaluation::Ready(v) => match v.value() {
            ScalarValue::Bool(v) => Ok(*v),
            _ => Err(error(
                "temporal_type",
                "$.window",
                "bound output is not Boolean",
            )),
        },
        Evaluation::Warmup | Evaluation::Missing => Ok(false),
        Evaluation::Error(_) => Err(error(
            "temporal_numeric",
            "$.window",
            "numeric error cannot enter temporal memory",
        )),
    }
}
