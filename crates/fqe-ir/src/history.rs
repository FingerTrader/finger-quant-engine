//! Versioned exact historical inputs. Bytes are supplied by the host; no download or file IO.
use crate::{
    graph::{digest, error},
    loading::{self, Obj, integer, mapped, text, uint},
    model::{ContentId, ModelError},
};
use fqe_core::{
    InstrumentId,
    finance::{self, FinancialInstrument, InstrumentDraft, PricePolicy},
    numeric::Decimal,
    types::CurrencyCode,
    utc,
};
use serde_json::Value;
use std::sync::Arc;
pub const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_HISTORY_ROWS: usize = 200_000;
pub const CSV_HEADER: &str =
    "instrument_id,timeframe,open_time,close_time,open,high,low,close,volume,is_closed";
pub(crate) type Result<T> = std::result::Result<T, ModelError>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub open_ns: u64,
    pub close_ns: u64,
}
#[derive(Clone, Debug)]
pub struct HistorySpec {
    pub(crate) instrument: FinancialInstrument,
    pub(crate) timeframe: String,
    pub(crate) duration_ns: u64,
    pub(crate) sessions: Vec<Session>,
    pub(crate) expected_rows: usize,
    pub(crate) acquired_ns: u64,
    pub(crate) performance_ns: u64,
    pub(crate) revision: u64,
    pub(crate) parent: Option<ContentId>,
    pub(crate) label: String,
    pub(crate) profile: String,
    pub(crate) source: ContentId,
    pub(crate) content: ContentId,
    pub(crate) canonical: Vec<u8>,
}
fn numeric(value: Value, scale: u8, path: &str) -> Result<Decimal> {
    finance::parse_decimal(&text(value, path)?, scale)
        .map_err(|e| error("history_decimal", path, &e.to_string()))
}
fn timestamp(value: Value, path: &str) -> Result<u64> {
    utc::parse_utc_ns(&text(value, path)?).map_err(|e| error("history_time", path, &e.to_string()))
}
fn scale(value: Value, max: u32, path: &str) -> Result<u8> {
    let n = uint(value, path)?;
    if n > max {
        return Err(error("history_scale", path, "unsupported financial scale"));
    }
    Ok(n as u8)
}
impl HistorySpec {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let value = loading::decode(bytes)?;
        let canonical = serde_json::to_vec(&value).expect("bounded history spec");
        let mut o = Obj::new(value, "$")?;
        if text(o.take("format")?, "$.format")? != "fqe.history-spec"
            || uint(o.take("schema_version")?, "$.schema_version")? != 1
        {
            return Err(error("history_version", "$", "expected fqe.history-spec/1"));
        }
        let profile = text(o.take("profile")?, "$.profile")?;
        let mut i = Obj::new(o.take("instrument")?, "$.instrument")?;
        let instrument =
            InstrumentId::new(text(i.take("id")?, "$.instrument.id")?).map_err(|_| {
                error(
                    "history_instrument",
                    "$.instrument.id",
                    "invalid instrument",
                )
            })?;
        let currency = CurrencyCode::new(text(i.take("currency")?, "$.instrument.currency")?)
            .map_err(|_| {
                error(
                    "history_currency",
                    "$.instrument.currency",
                    "invalid currency unit",
                )
            })?;
        let ps = scale(i.take("price_scale")?, 12, "$.instrument.price_scale")?;
        let qs = scale(i.take("quantity_scale")?, 6, "$.instrument.quantity_scale")?;
        let ms = scale(i.take("money_scale")?, 6, "$.instrument.money_scale")?;
        let price_step = numeric(i.take("price_step")?, ps, "$.instrument.price_step")?;
        let quantity_step = numeric(i.take("quantity_step")?, qs, "$.instrument.quantity_step")?;
        let money_step = numeric(i.take("money_step")?, ms, "$.instrument.money_step")?;
        let minimum_quantity = numeric(
            i.take("minimum_quantity")?,
            qs,
            "$.instrument.minimum_quantity",
        )?;
        let minimum_notional = numeric(
            i.take("minimum_notional")?,
            ms,
            "$.instrument.minimum_notional",
        )?;
        let prices = match text(i.take("price_policy")?, "$.instrument.price_policy")?.as_str() {
            "nonnegative" => PricePolicy::NonNegative,
            "signed" => PricePolicy::Signed,
            _ => {
                return Err(error(
                    "history_policy",
                    "$.instrument.price_policy",
                    "unsupported price policy",
                ));
            }
        };
        i.end()?;
        if !matches!(
            (profile.as_str(), prices),
            ("fqe.cash-long-history/1", PricePolicy::NonNegative)
                | ("fqe.signed-history/1", PricePolicy::Signed)
        ) {
            return Err(error(
                "history_profile",
                "$.profile",
                "profile/price policy unsupported or inconsistent",
            ));
        }
        let instrument = FinancialInstrument::new(InstrumentDraft {
            instrument,
            currency,
            price_step,
            quantity_step,
            money_step,
            minimum_quantity,
            minimum_notional,
            prices,
        })
        .map_err(|e| error("history_instrument", "$.instrument", &e.to_string()))?;
        let mut t = Obj::new(o.take("timeframe")?, "$.timeframe")?;
        let timeframe = text(t.take("id")?, "$.timeframe.id")?;
        let duration_ns = integer(t.take("duration_ns")?, "$.timeframe.duration_ns")?;
        t.end()?;
        let seconds = match timeframe.as_str() {
            "1m" => 60,
            "3m" => 180,
            "5m" => 300,
            "15m" => 900,
            "30m" => 1800,
            "1h" => 3600,
            "2h" => 7200,
            "4h" => 14400,
            "6h" => 21600,
            "12h" => 43200,
            "1d" => 86400,
            "1w" => 604800,
            _ => {
                return Err(error(
                    "history_timeframe",
                    "$.timeframe",
                    "unsupported explicit timeframe",
                ));
            }
        };
        if duration_ns != seconds * 1_000_000_000 {
            return Err(error(
                "history_timeframe",
                "$.timeframe",
                "label/duration mismatch",
            ));
        }
        let mut c = Obj::new(o.take("calendar")?, "$.calendar")?;
        if text(c.take("id")?, "$.calendar.id")? != "utc-explicit-sessions/1" {
            return Err(error(
                "history_calendar",
                "$.calendar.id",
                "only explicit Gregorian UTC sessions supported",
            ));
        }
        let sessions = mapped(c.take("sessions")?, "$.calendar.sessions", |v, p| {
            let mut s = Obj::new(v, p)?;
            let open_ns = timestamp(s.take("open")?, p)?;
            let close_ns = timestamp(s.take("close")?, p)?;
            s.end()?;
            Ok(Session { open_ns, close_ns })
        })?;
        c.end()?;
        let acquired_ns = timestamp(o.take("acquired_at")?, "$.acquired_at")?;
        let performance_ns = timestamp(o.take("performance_start")?, "$.performance_start")?;
        if text(o.take("adjustment")?, "$.adjustment")? != "raw-no-actions"
            || text(o.take("volume_unit")?, "$.volume_unit")? != "base_quantity"
        {
            return Err(error(
                "history_actions",
                "$",
                "raw-no-actions/base_quantity required; no implicit adjustments",
            ));
        }
        let mut source = Obj::new(o.take("source")?, "$.source")?;
        let label = text(source.take("label")?, "$.source.label")?;
        let revision = integer(source.take("revision")?, "$.source.revision")?;
        let parent = match source.take("parent")? {
            Value::Null => None,
            v => Some(ContentId::parse(&text(v, "$.source.parent")?).map_err(|_| {
                error("history_revision", "$.source.parent", "expected content ID")
            })?),
        };
        source.end()?;
        o.end()?;
        if label.trim().is_empty()
            || label.len() > 512
            || revision == 0
            || (revision == 1) != parent.is_none()
        {
            return Err(error(
                "history_revision",
                "$.source",
                "nonempty source and explicit monotone revision/parent required",
            ));
        }
        if sessions.is_empty() {
            return Err(error(
                "history_calendar",
                "$.calendar.sessions",
                "sessions required",
            ));
        }
        let mut expected_rows = 0_usize;
        let mut previous = 0;
        for s in &sessions {
            if s.close_ns <= s.open_ns
                || s.open_ns < previous
                || !s.open_ns.is_multiple_of(duration_ns)
                || !s.close_ns.is_multiple_of(duration_ns)
            {
                return Err(error(
                    "history_calendar",
                    "$.calendar.sessions",
                    "sorted nonoverlapping aligned positive sessions required",
                ));
            }
            let count = (s.close_ns - s.open_ns) / duration_ns;
            if count > MAX_HISTORY_ROWS as u64 || count as usize > MAX_HISTORY_ROWS - expected_rows
            {
                return Err(error(
                    "history_limit",
                    "$.calendar.sessions",
                    "more than200000 expected rows",
                ));
            }
            expected_rows += count as usize;
            previous = s.close_ns;
        }
        if acquired_ns < previous
            || !sessions.iter().any(|s| {
                performance_ns >= s.open_ns
                    && performance_ns < s.close_ns
                    && (performance_ns - s.open_ns).is_multiple_of(duration_ns)
            })
        {
            return Err(error(
                "history_range",
                "$",
                "acquisition before last close or performance start not a declared bar open",
            ));
        }
        Ok(Self {
            instrument,
            timeframe,
            duration_ns,
            sessions,
            expected_rows,
            acquired_ns,
            performance_ns,
            revision,
            parent,
            label,
            profile,
            source: digest(b"", bytes),
            content: digest(b"FQE-HISTORY-SPEC/1\n", &canonical),
            canonical,
        })
    }
    pub fn instrument(&self) -> &FinancialInstrument {
        &self.instrument
    }
    pub fn timeframe(&self) -> &str {
        &self.timeframe
    }
    pub fn duration_ns(&self) -> u64 {
        self.duration_ns
    }
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }
    pub fn expected_rows(&self) -> usize {
        self.expected_rows
    }
    pub fn acquired_ns(&self) -> u64 {
        self.acquired_ns
    }
    pub fn performance_ns(&self) -> u64 {
        self.performance_ns
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn parent_id(&self) -> Option<ContentId> {
        self.parent
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn profile(&self) -> &str {
        &self.profile
    }
    pub fn source_id(&self) -> ContentId {
        self.source
    }
    pub fn content_id(&self) -> ContentId {
        self.content
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
}
#[derive(Clone, Debug)]
pub struct HistoricalDataset {
    pub(crate) spec: Arc<HistorySpec>,
    pub(crate) bars: Arc<[fqe_core::bars::ClosedBar]>,
    pub(crate) source: ContentId,
    pub(crate) bars_id: ContentId,
    pub(crate) content: ContentId,
}
impl HistoricalDataset {
    pub fn from_csv(spec: Arc<HistorySpec>, bytes: &[u8], previous: Option<&Self>) -> Result<Self> {
        crate::history_csv::load(spec, bytes, previous)
    }
    pub fn spec(&self) -> &HistorySpec {
        &self.spec
    }
    pub fn bars(&self) -> &[fqe_core::bars::ClosedBar] {
        &self.bars
    }
    pub fn source_id(&self) -> ContentId {
        self.source
    }
    pub fn bars_id(&self) -> ContentId {
        self.bars_id
    }
    pub fn content_id(&self) -> ContentId {
        self.content
    }
    pub fn select(
        self: &Arc<Self>,
        start_ns: u64,
        end_ns: u64,
        warmup: usize,
    ) -> Result<HistorySelection> {
        if end_ns <= start_ns || warmup > 10_000 {
            return Err(error(
                "history_selection",
                "$.range",
                "positive range and warmup<=10000 required",
            ));
        }
        let first = self
            .bars
            .iter()
            .position(|b| b.input().open_ns == start_ns)
            .ok_or_else(|| error("history_selection", "$.range.start", "not a bar open"))?;
        let last = self
            .bars
            .iter()
            .position(|b| b.input().close_ns == end_ns)
            .ok_or_else(|| error("history_selection", "$.range.end", "not a bar close"))?;
        if last < first || first < warmup {
            return Err(error(
                "history_selection",
                "$.range",
                "range reversed or insufficient warmup",
            ));
        }
        let content = digest(
            b"FQE-HISTORY-SELECTION/1\n",
            &serde_json::to_vec(&(
                self.content.to_string(),
                start_ns.to_string(),
                end_ns.to_string(),
                warmup.to_string(),
            ))
            .expect("bounded selection"),
        );
        Ok(HistorySelection {
            dataset: Arc::clone(self),
            first: first - warmup,
            performance: first,
            last: last + 1,
            content,
        })
    }
}
#[derive(Clone, Debug)]
pub struct HistorySelection {
    dataset: Arc<HistoricalDataset>,
    first: usize,
    performance: usize,
    last: usize,
    content: ContentId,
}
impl HistorySelection {
    pub fn dataset(&self) -> &HistoricalDataset {
        &self.dataset
    }
    pub fn content_id(&self) -> ContentId {
        self.content
    }
    pub fn bars(&self) -> &[fqe_core::bars::ClosedBar] {
        &self.dataset.bars[self.first..self.last]
    }
    pub fn warmup_bars(&self) -> usize {
        self.performance - self.first
    }
    pub fn performance_start_ns(&self) -> u64 {
        self.dataset.bars[self.performance].input().open_ns
    }
    pub fn contiguous(&self) -> bool {
        self.bars()
            .windows(2)
            .all(|w| w[0].input().close_ns == w[1].input().open_ns)
    }
}

/// Validated compiler identity/trigger bundle; arbitrary fields cannot be injected by a client.
#[derive(Clone, Debug)]
pub struct HistoryStrategy {
    artifact: ContentId,
    source: ContentId,
    execution: ContentId,
    registry: ContentId,
    trigger: crate::event_plan::Trigger,
    kind: &'static str,
}
impl From<&crate::workflow::CompiledWorkflow> for HistoryStrategy {
    fn from(plan: &crate::workflow::CompiledWorkflow) -> Self {
        Self {
            artifact: plan.artifact_id(),
            source: plan.source_id(),
            execution: plan.content_id(),
            registry: plan.event().graph().registry_id(),
            trigger: plan.event().trigger().clone(),
            kind: "workflow/1",
        }
    }
}
impl From<&crate::event_plan::CompiledEventPlan> for HistoryStrategy {
    fn from(plan: &crate::event_plan::CompiledEventPlan) -> Self {
        Self {
            artifact: plan.artifact_id(),
            source: plan.source_id(),
            execution: plan.content_id(),
            registry: plan.graph().registry_id(),
            trigger: plan.trigger().clone(),
            kind: "event-plan/1",
        }
    }
}
/// Input-only manifest. Accounts/fills/costs are deliberately not selected by historical preparation.
pub fn run_input_manifest(
    selection: &HistorySelection,
    strategy: &HistoryStrategy,
    run: &str,
    engine: ContentId,
    seed: u64,
) -> Result<Vec<u8>> {
    crate::model::identifier(run, "$.run")?;
    let data = selection.dataset();
    let spec = data.spec();
    let d = spec.instrument.declaration();
    let expected = crate::event_plan::Trigger::ClosedBar {
        instrument: d.instrument.clone(),
        timeframe_ns: spec.duration_ns,
        price_scale: d.price_step.scale(),
        volume_scale: d.quantity_step.scale(),
    };
    if strategy.trigger != expected {
        return Err(error(
            "history_strategy_binding",
            "$.strategy",
            "compiled instrument/timeframe/price/volume scales differ",
        ));
    }
    if !selection.contiguous() {
        return Err(error(
            "history_runtime_calendar",
            "$.selection",
            "dataset is valid but current compiled event profile requires continuous bars",
        ));
    }
    let bars = selection.bars();
    let bytes=serde_json::to_vec(&serde_json::json!({
        "format":"fqe.history-run-inputs","schema_version":1,"stage":"historical-inputs-only","run":run,"engine_id":engine.to_string(),"seed":seed.to_string(),
        "strategy":{"kind":strategy.kind,"artifact_id":strategy.artifact.to_string(),"source_id":strategy.source.to_string(),"execution_id":strategy.execution.to_string(),"registry_id":strategy.registry.to_string()},
        "history":{"dataset_id":data.content_id().to_string(),"spec_id":spec.content_id().to_string(),"spec_source_id":spec.source_id().to_string(),"source_id":data.source_id().to_string(),"bars_id":data.bars_id().to_string(),"selection_id":selection.content_id().to_string(),"profile":spec.profile,"revision":spec.revision.to_string(),"parent":spec.parent.map(|p|p.to_string()),"source_label":spec.label,"acquired_ns":spec.acquired_ns.to_string(),"calendar":"utc-explicit-sessions/1","adjustment":"raw-no-actions","volume_unit":"base_quantity","timeframe":spec.timeframe,"duration_ns":spec.duration_ns.to_string()},
        "range":{"first_open_ns":bars[0].input().open_ns.to_string(),"last_close_ns":bars.last().expect("nonempty selection").input().close_ns.to_string(),"performance_start_ns":selection.performance_start_ns().to_string(),"warmup_bars":selection.warmup_bars().to_string(),"row_count":bars.len().to_string()},
        "instrument":{"id":d.instrument.as_str(),"currency":d.currency.as_str(),"price_step":finance::decimal_text(d.price_step),"quantity_step":finance::decimal_text(d.quantity_step),"money_step":finance::decimal_text(d.money_step),"minimum_quantity":finance::decimal_text(d.minimum_quantity),"minimum_notional":finance::decimal_text(d.minimum_notional),"price_policy":match d.prices {PricePolicy::NonNegative=>"nonnegative",PricePolicy::Signed=>"signed"}},
        "financial_execution":"not-selected"
    })).map_err(|_|error("history_encoding","$","bounded manifest encoding failed"))?;
    loading::decode(&bytes)?;
    Ok(bytes)
}
