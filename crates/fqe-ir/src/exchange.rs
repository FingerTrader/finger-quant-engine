//! Experimental local file/result codecs; not a worker transport or external provider loader.
use crate::{
    event_plan::{CompiledEventPlan, Trigger},
    graph::{digest, error},
    loading::{self, Obj, integer, mapped, text, typed, typed_json, uint},
    model::{ContentId, ModelError},
    parameters::ParameterInput,
};
use fqe_core::{
    Event, InstrumentId, MarketTick, PriceTicks, Signal,
    bars::{BarInput, ClosedBar},
    types::{Evaluation, TypedValue},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, Write},
};
type Result<T> = std::result::Result<T, ModelError>;
pub const MAX_FILE_BYTES: usize = 262144;
pub const MAX_REPORT_BYTES: usize = 4 * 1024 * 1024;

/// Reuses the existing version-1 typed binding document; no implicit numeric conversion.
pub struct ParameterBindings {
    values: Vec<(String, ParameterInput)>,
    source_id: ContentId,
}
impl ParameterBindings {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut o = Obj::new(loading::decode(bytes)?, "$")?;
        if uint(o.take("version")?, "$.version")? != 1 {
            return Err(error(
                "bindings_version",
                "$.version",
                "expected binding version 1",
            ));
        }
        let values = Obj::new(o.take("values")?, "$.values")?
            .fields
            .into_iter()
            .map(|(name, v)| {
                crate::model::identifier(&name, "$.values")?;
                let value = if v.is_null() {
                    ParameterInput::Null
                } else {
                    ParameterInput::Value(Evaluation::Ready(typed(v, "$.values")?))
                };
                Ok((name, value))
            })
            .collect::<Result<Vec<_>>>()?;
        o.end()?;
        Ok(Self {
            values,
            source_id: digest(b"", bytes),
        })
    }
    pub fn source_id(&self) -> ContentId {
        self.source_id
    }
    pub fn into_values(self) -> Vec<(String, ParameterInput)> {
        self.values
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarketRecord {
    Tick(Event),
    Bar(Box<ClosedBar>),
}
pub struct MarketData {
    events: Vec<MarketRecord>,
    source_id: ContentId,
}
fn instrument(v: Value, p: &str) -> Result<InstrumentId> {
    InstrumentId::new(text(v, p)?).map_err(|_| error("data_instrument", p, "invalid instrument"))
}
pub(crate) fn record(v: Value, p: &str) -> Result<MarketRecord> {
    let mut o = Obj::new(v, p)?;
    let kind = text(o.take("kind")?, p)?;
    let sequence = integer(o.take("sequence")?, p)?;
    let timestamp_ns = integer(o.take("timestamp_ns")?, p)?;
    let instrument = instrument(o.take("instrument")?, p)?;
    let event = match kind.as_str() {
        "tick" => {
            let price = PriceTicks::new(integer(o.take("price")?, p)?)
                .map_err(|_| error("data_tick", p, "tick price must be positive"))?;
            MarketRecord::Tick(
                Event::new(sequence, timestamp_ns, MarketTick { instrument, price })
                    .map_err(|_| error("data_sequence", p, "sequence must be positive"))?,
            )
        }
        "closed_bar" => {
            let timeframe_ns = integer(o.take("timeframe_ns")?, p)?;
            let open_ns = integer(o.take("open_ns")?, p)?;
            let close_ns = integer(o.take("close_ns")?, p)?;
            let closed = o
                .take("closed")?
                .as_bool()
                .ok_or_else(|| error("data_type", p, "closed must be Boolean"))?;
            let b = BarInput {
                sequence,
                timestamp_ns,
                instrument,
                timeframe_ns,
                open_ns,
                close_ns,
                closed,
                open: typed(o.take("open")?, p)?,
                high: typed(o.take("high")?, p)?,
                low: typed(o.take("low")?, p)?,
                close: typed(o.take("close")?, p)?,
                volume: typed(o.take("volume")?, p)?,
            };
            MarketRecord::Bar(Box::new(ClosedBar::new(b).map_err(|_| {
                error("data_bar", p, "invalid closed-bar values/interval/types")
            })?))
        }
        _ => return Err(error("data_kind", p, "unsupported market record kind")),
    };
    o.end()?;
    Ok(event)
}
impl MarketData {
    /// Structural/per-record validation only; event ordering/domain checks occur in EventRuntime.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut o = Obj::new(loading::decode(bytes)?, "$")?;
        if text(o.take("format")?, "$.format")? != "fqe.market-data"
            || uint(o.take("schema_version")?, "$.schema_version")? != 1
        {
            return Err(error("data_version", "$", "expected fqe.market-data/1"));
        }
        let events = mapped(o.take("events")?, "$.events", record)?;
        o.end()?;
        Ok(Self {
            events,
            source_id: digest(b"", bytes),
        })
    }
    pub fn events(&self) -> &[MarketRecord] {
        &self.events
    }
    pub fn source_id(&self) -> ContentId {
        self.source_id
    }
}
fn trigger_json(t: &Trigger) -> Value {
    match t {
        Trigger::Tick { instrument } => json!({"kind":"tick","instrument":instrument.as_str()}),
        Trigger::ClosedBar {
            instrument,
            timeframe_ns,
            price_scale,
            volume_scale,
        } => {
            json!({"kind":"closed_bar","instrument":instrument.as_str(),"timeframe_ns":timeframe_ns.to_string(),"price_scale":price_scale,"volume_scale":volume_scale})
        }
    }
}
fn plan_json(plan: &CompiledEventPlan, parameters_source_id: Option<ContentId>) -> Value {
    json!({"artifact_id":plan.artifact_id().to_string(),"artifact_source_id":plan.source_id().to_string(),"execution_id":plan.content_id().to_string(),"graph_id":plan.graph().content_id().to_string(),"registry_id":plan.graph().registry_id().to_string(),"parameters_source_id":parameters_source_id.map(|i|i.to_string()),"trigger":trigger_json(plan.trigger()),"warmup_samples":plan.warmup_samples().to_string()})
}
struct LimitedBuffer {
    bytes: Vec<u8>,
}
impl Write for LimitedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_REPORT_BYTES - self.bytes.len() {
            return Err(io::Error::other("report exceeds limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl LimitedBuffer {
    fn new() -> Self {
        Self { bytes: vec![] }
    }
    fn raw(&mut self, b: &[u8]) -> Result<()> {
        self.write_all(b)
            .map_err(|_| error("report_limit", "$.result", "result exceeds 4MiB"))
    }
    fn json(&mut self, v: &Value) -> Result<()> {
        serde_json::to_writer(self, v)
            .map_err(|_| error("report_limit", "$.result", "result exceeds 4MiB"))
    }
}
/// Encode only a compiled validation result; no market evaluation/state factory is invoked.
pub fn validation_result(
    plan: &CompiledEventPlan,
    parameters_source_id: Option<ContentId>,
) -> Result<Vec<u8>> {
    let mut b = LimitedBuffer::new();
    b.json(&json!({"format":"fqe.validation-result","schema_version":1,"status":"valid","manifest":plan_json(plan,parameters_source_id),"inputs":plan.graph().inputs().iter().map(crate::graph::port_json).collect::<Vec<_>>(),"outputs":plan.roots().output_types().iter().map(crate::graph::port_json).collect::<Vec<_>>(),"node_count":plan.graph().nodes().len().to_string()}))?;
    Ok(b.bytes)
}
/// Public wire DTO deliberately independent of the internal runtime's Rust structs.
pub struct ReportContext<'a> {
    pub plan: &'a CompiledEventPlan,
    pub run: &'a str,
    pub dataset_id: ContentId,
    pub parameters_source_id: Option<ContentId>,
    pub engine_version: &'a str,
    pub interval_convention: &'a str,
}
pub struct ReportSignal<'a> {
    pub source_sequence: u64,
    pub timestamp_ns: u64,
    pub signal: &'a Signal,
}
pub struct ReportObservation<'a> {
    pub source_sequence: u64,
    pub timestamp_ns: u64,
    pub matched: bool,
    pub warmup: bool,
    pub outputs: &'a BTreeMap<String, Evaluation<TypedValue>>,
    pub signals: Vec<ReportSignal<'a>>,
}
pub struct ReportSummary {
    pub processed_events: u64,
    pub matched_events: u64,
    pub last_sequence: Option<u64>,
    pub last_timestamp_ns: Option<u64>,
}
/// Bounded all-or-nothing serialization; publish bytes only after finish succeeds.
pub struct ReportWriter {
    buffer: LimitedBuffer,
    run: String,
    execution_id: ContentId,
    observations: u64,
    signals: u64,
    failed: bool,
}
impl ReportWriter {
    pub fn new(context: ReportContext<'_>) -> Result<Self> {
        crate::model::identifier(context.run, "$.run")?;
        if context.engine_version.len() > 32 || context.interval_convention.len() > 128 {
            return Err(error(
                "report_context",
                "$.manifest",
                "context text exceeds limit",
            ));
        }
        let mut manifest = plan_json(context.plan, context.parameters_source_id);
        manifest["run"] = json!(context.run);
        manifest["dataset_id"] = json!(context.dataset_id.to_string());
        manifest["engine_version"] = json!(context.engine_version);
        manifest["interval_convention"] = json!(context.interval_convention);
        let mut buffer = LimitedBuffer::new();
        buffer.raw(b"{\"format\":\"fqe.run-result\",\"manifest\":")?;
        buffer.json(&manifest)?;
        buffer.raw(b",\"observations\":[")?;
        Ok(Self {
            buffer,
            run: context.run.into(),
            execution_id: context.plan.content_id(),
            observations: 0,
            signals: 0,
            failed: false,
        })
    }
    pub fn push(&mut self, o: ReportObservation<'_>) -> Result<()> {
        if self.failed {
            return Err(error("report_state", "$.result", "writer already failed"));
        }
        let result = self.push_inner(o);
        self.failed = result.is_err();
        result
    }
    fn push_inner(&mut self, o: ReportObservation<'_>) -> Result<()> {
        let outputs = o
            .outputs
            .iter()
            .map(|(n, v)| {
                let value = match v {
                    Evaluation::Ready(v) => json!({"status":"ready","value":typed_json(v)}),
                    Evaluation::Missing => json!({"status":"missing"}),
                    Evaluation::Warmup => json!({"status":"warmup"}),
                    Evaluation::Error(_) => {
                        return Err(error(
                            "report_error",
                            "$.observations.outputs",
                            "numeric error is not an accepted observation",
                        ));
                    }
                };
                Ok((n.clone(), value))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let signals=o.signals.iter().map(|s|json!({"source_sequence":s.source_sequence.to_string(),"timestamp_ns":s.timestamp_ns.to_string(),"instrument":s.signal.instrument.as_str(),"kind":s.signal.kind.to_string()})).collect::<Vec<_>>();
        if self.observations != 0 {
            self.buffer.raw(b",")?;
        }
        self.buffer.json(&json!({"run":self.run,"execution_id":self.execution_id.to_string(),"source_sequence":o.source_sequence.to_string(),"timestamp_ns":o.timestamp_ns.to_string(),"matched":o.matched,"warmup":o.warmup,"outputs":outputs,"signals":signals}))?;
        self.observations = self
            .observations
            .checked_add(1)
            .ok_or_else(|| error("report_limit", "$.result", "count overflow"))?;
        self.signals = self
            .signals
            .checked_add(o.signals.len() as u64)
            .ok_or_else(|| error("report_limit", "$.result", "count overflow"))?;
        Ok(())
    }
    pub fn finish(mut self, s: ReportSummary) -> Result<Vec<u8>> {
        if self.failed {
            return Err(error(
                "report_state",
                "$.result",
                "cannot finish a failed writer",
            ));
        }
        if s.processed_events != self.observations || s.matched_events > s.processed_events {
            return Err(error(
                "report_summary",
                "$.summary",
                "inconsistent accepted event counts",
            ));
        }
        self.buffer.raw(b"],\"schema_version\":1,\"summary\":")?;
        self.buffer.json(&json!({"status":"completed","processed_events":s.processed_events.to_string(),"matched_events":s.matched_events.to_string(),"signals":self.signals.to_string(),"last_sequence":s.last_sequence.map(|v|v.to_string()),"last_timestamp_ns":s.last_timestamp_ns.map(|v|v.to_string())}))?;
        self.buffer.raw(b"}")?;
        Ok(self.buffer.bytes)
    }
}
/// The caller supplies a fixed public message; raw input values and OS errors are not serialized.
pub fn diagnostic(stage: &str, code: &str, path: &str, message: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"format":"fqe.diagnostic","schema_version":1,"stage":stage,"code":code,"path":path.chars().take(512).collect::<String>(),"message":message})).expect("JSON diagnostic")
}
