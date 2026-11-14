//! Experimental desktop boundary. Native engine owns all financial state and values.
mod codec;
pub mod ffi;
pub mod framing;
use fqe_core::finance::parse_decimal;
use fqe_ir::{
    catalog,
    event_plan::LoadedEventPlan,
    exchange::ParameterBindings,
    financial_exchange::{self as json, JsonValue as Value, object, string},
    history::{HistoricalDataset, HistorySpec},
    model::ModelError,
    registry::Registry,
    workflow::LoadedWorkflow,
};
use fqe_runtime::{
    backtest::*,
    ledger::{AccountConfig, LedgerProfile},
    margin::MarginRules,
    simulator::{SimulationConfig, SimulationLimits},
};
use std::sync::{Arc, atomic::AtomicBool};
pub const MAX_REQUEST: usize = 262144;
/// Whole response envelope, including native report, is bounded by 4 MiB.
pub const MAX_RESPONSE: usize = 4 * 1024 * 1024;
pub const CAPABILITIES: &[&str] = &[
    "desktop.exact-json/1",
    "catalog.reference/1",
    "model.validate/1",
    "backtest.cash-long/1",
    "backtest.margin-long/1",
    "cancel.bar-boundary/1",
];
#[derive(Debug)]
pub struct Failure {
    pub code: &'static str,
    pub path: String,
}
type Result<T> = std::result::Result<T, Failure>;
fn fail(code: &'static str) -> Failure {
    Failure {
        code,
        path: "$".into(),
    }
}
impl From<ModelError> for Failure {
    fn from(e: ModelError) -> Self {
        Self {
            code: e.code,
            path: e.path,
        }
    }
}
impl From<fqe_runtime::events::ExecutionError> for Failure {
    fn from(e: fqe_runtime::events::ExecutionError) -> Self {
        Self {
            code: e.code,
            path: e.path,
        }
    }
}
fn fields(value: &Value, keys: &[&str]) -> Result<()> {
    let m = value.as_object().ok_or_else(|| fail("object_required"))?;
    if m.len() != keys.len() || keys.iter().any(|k| !m.contains_key(*k)) {
        return Err(fail("fields"));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| fail("string_required"))
}
fn unsigned(value: &Value, key: &str) -> Result<u64> {
    let t = text(value, key)?;
    let n = t.parse::<u64>().map_err(|_| fail("unsigned_string"))?;
    if n.to_string() != t {
        return Err(fail("unsigned_string"));
    }
    Ok(n)
}
fn count(value: &Value, key: &str) -> Result<u32> {
    u32::try_from(unsigned(value, key)?).map_err(|_| fail("count_range"))
}
pub struct Request {
    pub id: String,
    pub method: String,
    pub params: Value,
}
impl Request {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let v = codec::decode(bytes)?;
        fields(&v, &["format", "schema_version", "id", "method", "params"])?;
        if text(&v, "format")? != "fqe.desktop-request" || v["schema_version"].as_u64() != Some(1) {
            return Err(fail("protocol_version"));
        }
        let id = text(&v, "id")?;
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        {
            return Err(fail("request_id"));
        }
        let method = text(&v, "method")?;
        if method.len() > 64 || !v["params"].is_object() {
            return Err(fail("request_shape"));
        }
        Ok(Self {
            id: id.into(),
            method: method.into(),
            params: v["params"].clone(),
        })
    }
}
pub fn bridge_id() -> String {
    let mut bytes = Vec::new();
    for src in [
        include_bytes!("lib.rs").as_slice(),
        include_bytes!("ffi.rs").as_slice(),
        include_bytes!("framing.rs").as_slice(),
        include_bytes!("codec.rs").as_slice(),
        include_bytes!("bin/peer.rs").as_slice(),
        include_bytes!("../Cargo.toml").as_slice(),
    ] {
        bytes.extend_from_slice(&(src.len() as u64).to_be_bytes());
        bytes.extend_from_slice(src);
    }
    json::content_id(b"FQE-DESKTOP-BRIDGE/1\n", &bytes).to_string()
}
pub fn response(id: Option<&str>, outcome: Result<Value>) -> Vec<u8> {
    let (ok, body) = match outcome {
        Ok(v) => (true, v),
        Err(e) => (
            false,
            object([("code", string(e.code)), ("path", string(e.path))]),
        ),
    };
    let v = object([
        ("format", string("fqe.desktop-response")),
        ("schema_version", Value::from(1)),
        ("id", id.map_or(Value::Null, string)),
        ("ok", Value::Bool(ok)),
        ("body", body),
    ]);
    match json::encode(&v, MAX_RESPONSE) {
        Ok(bytes) => bytes,
        Err(_) => json::encode(
            &object([
                ("format", string("fqe.desktop-response")),
                ("schema_version", Value::from(1)),
                ("id", id.map_or(Value::Null, string)),
                ("ok", Value::Bool(false)),
                (
                    "body",
                    object([("code", string("response_quota")), ("path", string("$"))]),
                ),
            ]),
            1024,
        )
        .expect("fixed bounded error"),
    }
}
pub fn dispatch(bytes: &[u8], cancel: &AtomicBool) -> Vec<u8> {
    let request = match Request::parse(bytes) {
        Ok(v) => v,
        Err(e) => return response(None, Err(e)),
    };
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| execute(&request, cancel)))
            .unwrap_or_else(|_| Err(fail("engine_panic")));
    response(Some(&request.id), outcome)
}
pub fn execute(request: &Request, cancel: &AtomicBool) -> Result<Value> {
    let p = &request.params;
    match request.method.as_str() {
        "hello" => {
            fields(p, &["major", "minor", "required"])?;
            if p["major"].as_u64() != Some(1) || p["minor"].as_u64().is_none() {
                return Err(fail("incompatible_protocol"));
            }
            let required = p["required"]
                .as_array()
                .ok_or_else(|| fail("capability_shape"))?;
            if required.len() > 32
                || required
                    .iter()
                    .any(|v| !v.as_str().is_some_and(|s| CAPABILITIES.contains(&s)))
            {
                return Err(fail("unsupported_capability"));
            }
            Ok(object([
                ("major", Value::from(1)),
                ("minor", Value::from(0)),
                ("engine_package", string(env!("CARGO_PKG_VERSION"))),
                ("bridge_id", string(bridge_id())),
                (
                    "capabilities",
                    Value::Array(CAPABILITIES.iter().map(string).collect()),
                ),
                ("max_request_bytes", string(MAX_REQUEST)),
                ("max_response_bytes", string(MAX_RESPONSE)),
                ("max_bars", string(MAX_BACKTEST_BARS)),
                ("financial_authority", string("native-ledger")),
                ("maturity", string("experimental")),
            ]))
        }
        // Measurement method is an exact arbitrary bounded JSON echo, not strategy execution.
        "echo" => Ok(p.clone()),
        "model.validate" | "descriptor.list" => {
            fields(p, &["kind", "model", "parameters"])?;
            let (plan, registry) = compile(p)?;
            if request.method == "model.validate" {
                Ok(object([
                    ("execution_id", string(plan.execution_id())),
                    ("registry_id", string(registry.content_id())),
                    (
                        "parameters",
                        json::parameters(plan.event().graph().parameters()),
                    ),
                ]))
            } else {
                let descriptors = registry
                    .descriptors()
                    .map(|d| json::decode_input(d.canonical_bytes()).map_err(Failure::from))
                    .collect::<Result<Vec<_>>>()?;
                Ok(object([
                    ("registry_id", string(registry.content_id())),
                    ("descriptors", Value::Array(descriptors)),
                ]))
            }
        }
        "backtest.run" => {
            let prepared = prepare(p)?;
            let report = prepared.run_with_cancel(cancel)?;
            // Trusted native-generated report, already bounded/exact; input strict decoder has a smaller quota.
            let report_value: Value =
                serde_json::from_slice(report.json()).map_err(|_| fail("native_report"))?;
            Ok(object([
                ("manifest_id", string(report.manifest_id())),
                ("result_id", string(report.content_id())),
                ("report", report_value),
            ]))
        }
        _ => Err(fail("unsupported_method")),
    }
}
fn compile(p: &Value) -> Result<(StrategyPlan, Registry)> {
    let values = ParameterBindings::from_bytes(text(p, "parameters")?.as_bytes())?.into_values();
    match text(p, "kind")? {
        "event" => {
            let loaded = LoadedEventPlan::from_bytes(text(p, "model")?.as_bytes())?;
            let r = catalog::reference(loaded.trigger())?;
            let plan = StrategyPlan::Event(Arc::new(loaded.compile(&r, values)?));
            Ok((plan, r))
        }
        "workflow" => {
            let loaded = LoadedWorkflow::from_bytes(text(p, "model")?.as_bytes())?;
            let r = catalog::reference(loaded.event_plan().trigger())?;
            let plan = StrategyPlan::Workflow(Arc::new(loaded.compile(&r, values)?));
            Ok((plan, r))
        }
        _ => Err(fail("artifact_kind")),
    }
}
pub fn prepare(p: &Value) -> Result<PreparedBacktest> {
    fields(
        p,
        &[
            "kind",
            "model",
            "parameters",
            "history_spec",
            "csv",
            "start_ns",
            "end_ns",
            "warmup_bars",
            "run",
            "seed",
            "account",
            "policies",
        ],
    )?;
    let (strategy, _) = compile(p)?;
    let spec = Arc::new(HistorySpec::from_bytes(
        text(p, "history_spec")?.as_bytes(),
    )?);
    let dataset = Arc::new(HistoricalDataset::from_csv(
        spec.clone(),
        text(p, "csv")?.as_bytes(),
        None,
    )?);
    let selection = dataset.select(
        unsigned(p, "start_ns")?,
        unsigned(p, "end_ns")?,
        count(p, "warmup_bars")? as usize,
    )?;
    let a = &p["account"];
    fields(
        a,
        &[
            "id",
            "profile",
            "initial_cash",
            "participation_bps",
            "fee_bps",
            "minimum_fee",
            "slippage_bps",
            "risk",
        ],
    )?;
    let (profile, margin) = match text(a, "profile")? {
        "cash_long" if a["risk"].is_null() => (LedgerProfile::CashLong, None),
        "margin_long" => {
            let risk = &a["risk"];
            fields(risk, &["max_leverage_bps", "maintenance_bps"])?;
            (
                LedgerProfile::MarginLong,
                Some(MarginRules {
                    max_leverage_bps: count(risk, "max_leverage_bps")?,
                    maintenance_bps: count(risk, "maintenance_bps")?,
                }),
            )
        }
        _ => return Err(fail("financial_profile")),
    };
    let d = spec.instrument().declaration();
    let money = |v: &Value, k: &str| {
        parse_decimal(text(v, k)?, d.money_step.scale()).map_err(|_| fail("money_value"))
    };
    let quantity = |v: &Value, k: &str| {
        parse_decimal(text(v, k)?, d.quantity_step.scale()).map_err(|_| fail("quantity_value"))
    };
    let config = SimulationConfig {
        margin,
        limits: SimulationLimits::default(),
        account: AccountConfig {
            account: text(a, "id")?.into(),
            instrument: spec.instrument().clone(),
            profile,
            initial_cash: money(a, "initial_cash")?,
        },
        timeframe_ns: spec.duration_ns(),
        participation_bps: count(a, "participation_bps")?,
        fee_bps: count(a, "fee_bps")?,
        minimum_fee: money(a, "minimum_fee")?,
        slippage_bps: count(a, "slippage_bps")?,
        signal_source: None,
    };
    let list = p["policies"]
        .as_array()
        .ok_or_else(|| fail("policy_shape"))?;
    if list.len() > MAX_POLICY_BINDINGS {
        return Err(fail("policy_quota"));
    }
    let mut policies = Vec::new();
    for v in list {
        fields(v, &["id", "source", "action"])?;
        let source = &v["source"];
        let action = &v["action"];
        let source = match text(source, "kind")? {
            "long_signal" => {
                fields(source, &["kind"])?;
                PolicySource::LongSignal
            }
            "event_boolean" => {
                fields(source, &["kind", "name"])?;
                PolicySource::EventBoolean {
                    name: text(source, "name")?.into(),
                }
            }
            "workflow_boolean" => {
                fields(source, &["kind", "instance", "output"])?;
                PolicySource::WorkflowBoolean {
                    instance: text(source, "instance")?.into(),
                    output: text(source, "output")?.into(),
                }
            }
            _ => return Err(fail("policy_source")),
        };
        let action = match text(action, "kind")? {
            "buy_if_flat" => {
                fields(action, &["kind", "quantity", "cash_cap"])?;
                PolicyAction::BuyIfFlat {
                    quantity: quantity(action, "quantity")?,
                    cash_cap: money(action, "cash_cap")?,
                }
            }
            "close_long" => {
                fields(action, &["kind"])?;
                PolicyAction::CloseLong
            }
            _ => return Err(fail("policy_action")),
        };
        policies.push(PolicyBinding {
            id: text(v, "id")?.into(),
            source,
            action,
        });
    }
    Ok(PreparedBacktest::new(
        selection,
        strategy,
        policies,
        config,
        text(p, "run")?,
        unsigned(p, "seed")?,
        RunControl::Complete,
    )?)
}
