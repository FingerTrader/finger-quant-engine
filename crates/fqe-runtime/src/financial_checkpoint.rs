//! Full logical financial continuation by explicitly permitted, bounded deterministic reconstruction.
use crate::{
    backtest::{self, BacktestResult, PreparedBacktest, RunControl, StrategyObservation},
    events::ExecutionError,
};
use fqe_ir::{
    financial_exchange::{self as json, JsonValue as V, object as o, string as s},
    model::ContentId,
};
use std::sync::Arc;
type Result<T> = std::result::Result<T, ExecutionError>;
const REPRESENTATION: &str = "bounded-financial-prefix-reconstruction/1";
fn err(code: &'static str, detail: impl ToString) -> ExecutionError {
    backtest::error(code, detail)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinancialCheckpoint {
    pub manifest_id: ContentId,
    pub engine_id: ContentId,
    pub source_bars: usize,
    pub prefix_id: ContentId,
}
impl FinancialCheckpoint {
    fn value(&self) -> V {
        o([
            ("format", s("fqe.financial-checkpoint")),
            ("schema_version", V::from(1)),
            ("representation", s(REPRESENTATION)),
            ("manifest_id", s(self.manifest_id)),
            ("engine_id", s(self.engine_id)),
            ("source_bars", s(self.source_bars)),
            ("prefix_id", s(self.prefix_id)),
        ])
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        let mut value = self.value();
        let bytes = json::encode(&value, 262144)?;
        value.as_object_mut().expect("object").insert(
            "content_id".into(),
            s(json::content_id(b"FQE-FINANCIAL-CHECKPOINT/1\n", &bytes)),
        );
        Ok(json::encode(&value, 262144)?)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut v = json::decode_input(bytes)?;
        let map = v
            .as_object_mut()
            .ok_or_else(|| err("financial_checkpoint_shape", "object required"))?;
        if map.len() != 8
            || map.get("format") != Some(&s("fqe.financial-checkpoint"))
            || map.get("schema_version") != Some(&V::from(1))
            || map.get("representation") != Some(&s(REPRESENTATION))
        {
            return Err(err(
                "financial_checkpoint_version",
                "unknown shape/version/representation",
            ));
        }
        let text = |k: &str| {
            map.get(k)
                .and_then(V::as_str)
                .ok_or_else(|| err("financial_checkpoint_shape", format!("missing string: {k}")))
        };
        let content = ContentId::parse(text("content_id")?)?;
        let manifest_id = ContentId::parse(text("manifest_id")?)?;
        let engine_id = ContentId::parse(text("engine_id")?)?;
        let prefix_id = ContentId::parse(text("prefix_id")?)?;
        let n = text("source_bars")?;
        let source_bars = n
            .parse::<usize>()
            .map_err(|_| err("financial_checkpoint_cursor", "invalid source count"))?;
        if source_bars > backtest::MAX_BACKTEST_BARS || n != source_bars.to_string() {
            return Err(err(
                "financial_checkpoint_cursor",
                "noncanonical/out-of-bound cursor",
            ));
        }
        map.remove("content_id");
        let canonical = json::encode(&v, 262144)?;
        if content != json::content_id(b"FQE-FINANCIAL-CHECKPOINT/1\n", &canonical) {
            return Err(err(
                "financial_checkpoint_integrity",
                "checkpoint checksum mismatch",
            ));
        }
        Ok(Self {
            manifest_id,
            engine_id,
            source_bars,
            prefix_id,
        })
    }
}
fn capability(p: &PreparedBacktest) -> Result<()> {
    if p.control != RunControl::Complete {
        return Err(err(
            "financial_checkpoint_control",
            "only complete-run preparation supports paused continuation",
        ));
    }
    for node in p.strategy.event().graph().nodes() {
        if !node.call.permits_checkpoint_replay() {
            return Err(ExecutionError {
                code: "financial_checkpoint_capability",
                path: node.source.definition.clone(),
                detail: "provider has no explicit deterministic reconstruction permit".into(),
            });
        }
    }
    Ok(())
}
fn prefix(previous: &BacktestResult, next: &BacktestResult) -> Result<()> {
    fn starts<T: PartialEq>(a: &[T], b: &[T]) -> bool {
        b.starts_with(a)
    }
    if !starts(previous.observations(), next.observations())
        || !starts(previous.decisions(), next.decisions())
        || !starts(previous.curve(), next.curve())
        || !starts(previous.broker().receipts(), next.broker().receipts())
        || !starts(previous.broker().fills(), next.broker().fills())
        || !starts(
            previous.broker().ledger().journal(),
            next.broker().ledger().journal(),
        )
    {
        return Err(err(
            "financial_checkpoint_divergence",
            "previously committed financial/strategy outputs changed during reconstruction",
        ));
    }
    Ok(())
}
/// Current is an explicitly Paused historical prefix. Advance returns only the newly consumed observation.
pub struct FinancialSession {
    prepared: Arc<PreparedBacktest>,
    current: BacktestResult,
    finished: bool,
}
impl FinancialSession {
    pub fn new(prepared: Arc<PreparedBacktest>) -> Result<Self> {
        capability(&prepared)?;
        let current = prepared.reconstruct_prefix(0, false)?;
        Ok(Self {
            prepared,
            current,
            finished: false,
        })
    }
    pub fn current(&self) -> &BacktestResult {
        &self.current
    }
    pub fn source_cursor(&self) -> usize {
        self.current.observations().len()
    }
    pub fn finished(&self) -> bool {
        self.finished
    }
    pub fn advance(&mut self) -> Result<&StrategyObservation> {
        if self.finished || self.source_cursor() >= self.prepared.selection.bars().len() {
            return Err(err("financial_checkpoint_lifecycle", "no next source bar"));
        }
        let candidate = self
            .prepared
            .reconstruct_prefix(self.source_cursor() + 1, false)?;
        prefix(&self.current, &candidate)?;
        self.current = candidate;
        Ok(self.current.observations().last().expect("advanced bar"))
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>> {
        if self.finished {
            return Err(err(
                "financial_checkpoint_lifecycle",
                "finished session has no paused checkpoint",
            ));
        }
        FinancialCheckpoint {
            manifest_id: self.prepared.manifest_id(),
            engine_id: crate::source_identity::engine_id(),
            source_bars: self.source_cursor(),
            prefix_id: self.current.content_id(),
        }
        .bytes()
    }
    /// Decode/compatibility/capabilities are checked before any provider factory is instantiated.
    pub fn restore(prepared: Arc<PreparedBacktest>, bytes: &[u8]) -> Result<Self> {
        let checkpoint = FinancialCheckpoint::from_bytes(bytes)?;
        if checkpoint.engine_id != crate::source_identity::engine_id()
            || checkpoint.manifest_id != prepared.manifest_id()
            || checkpoint.source_bars > prepared.selection.bars().len()
        {
            return Err(err(
                "financial_checkpoint_pins",
                "engine/input manifest/source cursor mismatch",
            ));
        }
        capability(&prepared)?;
        let current = prepared.reconstruct_prefix(checkpoint.source_bars, false)?;
        if current.content_id() != checkpoint.prefix_id {
            return Err(err(
                "financial_checkpoint_divergence",
                "complete reconstructed prefix differs",
            ));
        }
        Ok(Self {
            prepared,
            current,
            finished: false,
        })
    }
    pub fn finish(&mut self) -> Result<&BacktestResult> {
        if self.finished {
            return Err(err("financial_checkpoint_lifecycle", "already finished"));
        }
        let candidate = self
            .prepared
            .reconstruct_prefix(self.source_cursor(), true)?;
        prefix(&self.current, &candidate)?;
        self.current = candidate;
        self.finished = true;
        Ok(&self.current)
    }
}
