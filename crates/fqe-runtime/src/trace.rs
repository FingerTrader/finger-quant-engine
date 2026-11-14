//! Bounded observations of existing computation. No IO, provider calls, or semantic state.
use crate::{
    events::ExecutionError,
    temporal::{WindowNotice, WindowState},
};
use fqe_core::types::{Evaluation, TypedValue};
use fqe_ir::{
    compiler::SourceMap, expressions::ExpressionObservation, model::ContentId,
    workflow::InstanceValues,
};
use std::{
    collections::BTreeMap,
    fmt::{self, Write},
};
pub const MAX_TRACE_ENTRIES: usize = 4096;
pub const MAX_TRACE_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceLimits {
    pub entries: usize,
    pub bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceContext {
    pub run: String,
    pub execution_id: ContentId,
    pub sequence: Option<u64>,
    pub timestamp_ns: Option<u64>,
    pub node: Vec<String>,
    pub source: SourceMap,
    /// Logical evaluator subpath relative to the enclosing source definition.
    pub expression: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceData {
    Delivery {
        stream: String,
        dataset_id: ContentId,
        source_sequence: u64,
        alignment: BTreeMap<String, (u64, u64, u64)>,
    },
    PureCall {
        provider: String,
        call_sequence: u64,
        outputs: BTreeMap<String, Evaluation<TypedValue>>,
    },
    Crossing {
        previous: Option<(TypedValue, TypedValue)>,
        current: Option<(TypedValue, TypedValue)>,
        output: Evaluation<TypedValue>,
    },
    Values(BTreeMap<String, Evaluation<TypedValue>>),
    State {
        before: InstanceValues,
        after: InstanceValues,
    },
    Windows {
        before: BTreeMap<String, WindowState>,
        after: BTreeMap<String, WindowState>,
        notices: Vec<WindowNotice>,
    },
    Command {
        index: u64,
        output_sequence: Option<u64>,
        outcome: &'static str,
    },
    Error(ExecutionError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceEntry {
    pub context: TraceContext,
    /// Computed candidate observations are not committed state. Consult the final command marker.
    pub phase: &'static str,
    pub data: TraceData,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceRecord {
    pub command_index: Option<u64>,
    pub entry: TraceEntry,
}
/// Captured-prefix storage. Exhaustion permanently seals this collector; computation continues.
#[derive(Debug)]
pub struct TraceBuffer {
    limits: Option<TraceLimits>,
    entries: Vec<TraceRecord>,
    command_index: Option<u64>,
    bytes: usize,
    dropped: u64,
    sealed: bool,
}
impl TraceBuffer {
    pub fn off() -> Self {
        Self {
            limits: None,
            entries: vec![],
            command_index: None,
            bytes: 0,
            dropped: 0,
            sealed: false,
        }
    }
    pub fn new(limits: TraceLimits) -> Result<Self, ExecutionError> {
        if limits.entries == 0
            || limits.entries > MAX_TRACE_ENTRIES
            || limits.bytes == 0
            || limits.bytes > MAX_TRACE_BYTES
        {
            return Err(ExecutionError {
                code: "trace_limit",
                path: "$.trace".into(),
                detail: "entries1..4096 and bytes1..4194304 required".into(),
            });
        }
        Ok(Self {
            limits: Some(limits),
            ..Self::off()
        })
    }
    pub fn entries(&self) -> &[TraceRecord] {
        &self.entries
    }
    /// Storage charge is this same-engine DTO's Debug UTF-8 length, not a portable wire size.
    pub fn used_bytes(&self) -> usize {
        self.bytes
    }
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
    pub fn truncated(&self) -> bool {
        self.sealed
    }
    pub fn limits(&self) -> Option<TraceLimits> {
        self.limits
    }
    pub(crate) fn expression(
        &mut self,
        context: impl FnOnce() -> TraceContext,
        observation: ExpressionObservation<'_>,
    ) {
        self.push(|| {
            let mut context = context();
            let (phase, data) = match observation {
                ExpressionObservation::Call {
                    path,
                    provider,
                    call_sequence,
                    outputs,
                } => {
                    context
                        .node
                        .extend(["call".into(), call_sequence.to_string()]);
                    context.expression = Some(path.into());
                    (
                        "pure-call-candidate",
                        TraceData::PureCall {
                            provider: provider.into(),
                            call_sequence,
                            outputs: outputs.clone(),
                        },
                    )
                }
                ExpressionObservation::Crossing {
                    path,
                    id,
                    previous,
                    current,
                    output,
                } => {
                    context.node.extend(["crossing".into(), id.into()]);
                    context.expression = Some(path.into());
                    (
                        "crossing-candidate",
                        TraceData::Crossing {
                            previous: previous.cloned(),
                            current: current.cloned(),
                            output: output.clone(),
                        },
                    )
                }
            };
            TraceEntry {
                context,
                phase,
                data,
            }
        });
    }
    pub(crate) fn command(&mut self, index: Option<u64>) {
        self.command_index = index;
    }
    pub(crate) fn push(&mut self, make: impl FnOnce() -> TraceEntry) {
        let Some(limits) = self.limits else { return };
        if self.sealed || self.entries.len() >= limits.entries {
            self.sealed = true;
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        let entry = TraceRecord {
            command_index: self.command_index,
            entry: make(),
        };
        let mut count = Size {
            bytes: 0,
            limit: limits.bytes - self.bytes,
        };
        if write!(&mut count, "{entry:?}").is_err() {
            self.sealed = true;
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.bytes += count.bytes;
        self.entries.push(entry);
    }
}
struct Size {
    bytes: usize,
    limit: usize,
}
impl Write for Size {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if text.len() > self.limit.saturating_sub(self.bytes) {
            return Err(fmt::Error);
        }
        self.bytes += text.len();
        Ok(())
    }
}
pub(crate) fn context(
    run: &str,
    execution_id: ContentId,
    sequence: Option<u64>,
    timestamp_ns: Option<u64>,
    node: Vec<String>,
    definition: &str,
    call_sites: Vec<String>,
) -> TraceContext {
    TraceContext {
        run: run.into(),
        execution_id,
        sequence,
        timestamp_ns,
        node,
        expression: None,
        source: SourceMap {
            definition: definition.into(),
            call_sites,
        },
    }
}
