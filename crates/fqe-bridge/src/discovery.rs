//! Model-independent reference discovery for an explicit native event domain.
use crate::{Failure, Result, fields, object, string, text, unsigned};
use fqe_core::InstrumentId;
use fqe_ir::{catalog, event_plan::Trigger, financial_exchange as json};
use serde_json::Value;

pub(crate) fn context(p: &Value) -> Result<Trigger> {
    let instrument = InstrumentId::new(text(p, "instrument")?).map_err(|_| Failure {
        code: "event_instrument",
        path: "$.trigger.instrument".into(),
    })?;
    match text(p, "kind")? {
        "tick" => {
            fields(p, &["kind", "instrument"])?;
            Ok(Trigger::Tick { instrument })
        }
        "closed_bar" => {
            fields(
                p,
                &[
                    "kind",
                    "instrument",
                    "timeframe_ns",
                    "price_scale",
                    "volume_scale",
                ],
            )?;
            // Scale fields follow the Event schema's small integer representation.
            let scale = |key: &str, max| -> Result<u8> {
                let n = p[key]
                    .as_u64()
                    .filter(|n| *n <= max)
                    .ok_or_else(|| Failure {
                        code: "event_trigger",
                        path: format!("$.trigger.{key}"),
                    })?;
                Ok(n as u8)
            };
            let timeframe_ns = unsigned(p, "timeframe_ns")?;
            if timeframe_ns == 0 {
                return Err(crate::fail("event_trigger"));
            }
            Ok(Trigger::ClosedBar {
                instrument,
                timeframe_ns,
                price_scale: scale("price_scale", 12)?,
                volume_scale: scale("volume_scale", 18)?,
            })
        }
        _ => Err(crate::fail("event_trigger")),
    }
}

pub(crate) fn describe(p: &Value) -> Result<Value> {
    fields(p, &["trigger"])?;
    let trigger = context(&p["trigger"])?;
    let registry = catalog::reference(&trigger)?;
    let descriptors = registry
        .descriptors()
        .map(|d| json::decode_input(d.canonical_bytes()).map_err(Failure::from))
        .collect::<Result<Vec<_>>>()?;
    let strings = |values: &[&str]| Value::Array(values.iter().map(string).collect());
    Ok(object([
        ("format", string("fqe.catalog-description")),
        ("schema_version", Value::from(1)),
        ("trigger", p["trigger"].clone()),
        ("registry_id", string(registry.content_id())),
        ("descriptors", Value::Array(descriptors)),
        (
            "support",
            object([
                ("artifact_kinds", strings(&["event", "workflow"])),
                ("financial_profiles", strings(&["cash_long", "margin_long"])),
                ("authoring", Value::Bool(true)),
                ("authoring_profile", string("authoring.event/1")),
                ("optimization", Value::Bool(false)),
                ("external_languages", Value::Bool(false)),
                ("temporal_scheduler", Value::Bool(false)),
                ("multimarket_financial", Value::Bool(false)),
                ("progress", string("indeterminate-batch/1")),
                (
                    "limits",
                    object([
                        ("request_bytes", string(crate::MAX_REQUEST)),
                        ("response_bytes", string(crate::MAX_RESPONSE)),
                        (
                            "financial_source_bars",
                            string(fqe_runtime::backtest::MAX_BACKTEST_BARS),
                        ),
                        ("model_bytes", string(fqe_ir::model::MAX_MODEL_BYTES)),
                        ("graph_nodes", string(fqe_ir::model::MAX_NODES)),
                        ("event_slots", string(fqe_ir::event_plan::MAX_SLOTS)),
                        (
                            "pure_module_depth",
                            string(fqe_ir::compiler::MAX_MODULE_DEPTH),
                        ),
                        ("retained_jobs", string(crate::managed::MAX_JOBS)),
                        ("operation_receipts", string(crate::managed::MAX_OPERATIONS)),
                    ]),
                ),
            ]),
        ),
    ]))
}
