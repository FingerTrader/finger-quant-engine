//! Exact bounded optimization provenance and separate training/validation results.
use crate::{events::ExecutionError, optimization::*};
use fqe_core::finance::decimal_text;
use fqe_ir::{
    financial_exchange::{self as json, JsonValue as V, object as o, string as s},
    history::HistorySelection,
};
fn selection(v: &HistorySelection) -> V {
    o([
        ("selection_id", s(v.content_id())),
        ("first_open_ns", s(v.bars()[0].input().open_ns)),
        (
            "last_close_ns",
            s(v.bars()
                .last()
                .expect("nonempty selection")
                .input()
                .close_ns),
        ),
        ("performance_start_ns", s(v.performance_start_ns())),
        ("warmup_bars", s(v.warmup_bars())),
        ("source_bars", s(v.bars().len())),
    ])
}
pub(crate) fn manifest(p: &PreparedOptimization) -> Result<Vec<u8>, ExecutionError> {
    let r = &p.request;
    let c = &r.simulation;
    let data = r.training.dataset();
    let a = &c.account;
    let value = o([
        ("format", s("fqe.optimization-inputs")),
        ("schema_version", V::from(1)),
        ("engine_id", s(crate::source_identity::engine_id())),
        ("engine_version", s(env!("CARGO_PKG_VERSION"))),
        ("run", s(&r.run)),
        ("seed", s(r.seed)),
        (
            "strategy",
            o([
                ("artifact_id", s(r.artifact.content_id())),
                ("source_id", s(r.artifact.source_id())),
                ("registry_id", s(r.registry.content_id())),
                (
                    "kind",
                    s(match r.artifact {
                        SearchArtifact::Event(_) => "event",
                        SearchArtifact::Workflow(_) => "workflow",
                    }),
                ),
            ]),
        ),
        (
            "history",
            o([
                ("dataset_id", s(data.content_id())),
                ("csv_source_id", s(data.source_id())),
                ("bars_id", s(data.bars_id())),
                ("spec_id", s(data.spec().content_id())),
                ("spec_source_id", s(data.spec().source_id())),
                ("spec", json::decode_input(data.spec().canonical_bytes())?),
            ]),
        ),
        ("training", selection(&r.training)),
        ("validation", selection(&r.validation)),
        (
            "configuration",
            o([
                ("account", s(&a.account)),
                ("profile", s(a.profile.version())),
                ("initial_cash", s(decimal_text(a.initial_cash))),
                (
                    "model",
                    s(if c.margin.is_some() {
                        crate::margin::MARGIN_MODEL
                    } else {
                        crate::simulator::MODEL
                    }),
                ),
                ("timeframe_ns", s(c.timeframe_ns)),
                ("participation_bps", s(c.participation_bps)),
                ("fee_bps", s(c.fee_bps)),
                ("minimum_fee", s(decimal_text(c.minimum_fee))),
                ("slippage_bps", s(c.slippage_bps)),
                (
                    "margin",
                    c.margin.as_ref().map_or(V::Null, |m| {
                        o([
                            ("max_leverage_bps", s(m.max_leverage_bps)),
                            ("maintenance_bps", s(m.maintenance_bps)),
                        ])
                    }),
                ),
                (
                    "limits",
                    o([
                        ("orders", s(c.limits.orders)),
                        ("commands", s(c.limits.commands)),
                        ("fills", s(c.limits.fills)),
                    ]),
                ),
                (
                    "signal_source",
                    s("derived-from-trial-run-and-execution-id"),
                ),
            ]),
        ),
        (
            "policies",
            V::Array(
                r.policies
                    .iter()
                    .map(crate::backtest_encoding::policy)
                    .collect(),
            ),
        ),
        (
            "grid",
            V::Array(
                r.grid
                    .iter()
                    .map(|d| {
                        o([
                            ("key", s(&d.key)),
                            (
                                "values",
                                V::Array(d.values.iter().map(json::typed_value).collect()),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "trial_parameters",
            V::Array(p.parameters.iter().map(parameters_json).collect()),
        ),
        ("schedule", V::Array(p.order.iter().map(s).collect())),
        (
            "budget",
            o([
                ("max_trials", s(r.budget.max_trials)),
                ("max_total_source_bars", s(r.budget.max_total_source_bars)),
                ("max_report_bytes", s(r.budget.max_report_bytes)),
            ]),
        ),
        (
            "control",
            match r.control {
                SearchControl::Complete => o([("kind", s("complete"))]),
                SearchControl::CancelAfterTrials(n) => {
                    o([("kind", s("cancel_after_trials")), ("trials", s(n))])
                }
            },
        ),
        ("scheduler", s("sequential-splitmix64-fisher-yates/1")),
        (
            "trial_seed",
            s("request-seed-xor-canonical-zero-based-trial-index/1"),
        ),
        (
            "ranking",
            s("completed-training-net-profit-desc;max-drawdown-asc;parameter-content-id-asc/1"),
        ),
        (
            "validation_policy",
            s("fresh-ledger-later-performance-window-selected-training-winner-only/1"),
        ),
    ]);
    let bytes = json::encode(&value, 262144)?;
    json::decode_input(&bytes)?;
    Ok(bytes)
}
fn outcome(v: &TrialOutcome) -> Result<V, ExecutionError> {
    Ok(match v {
        TrialOutcome::NotRun => o([("status", s("not_run"))]),
        TrialOutcome::Failed(e) => o([
            ("status", s("failed")),
            (
                "error",
                o([
                    ("code", s(e.code)),
                    ("path", s(&e.path)),
                    ("detail", s(&e.detail)),
                ]),
            ),
        ]),
        TrialOutcome::Completed(r) | TrialOutcome::Cancelled(r) => o([
            (
                "status",
                s(if matches!(v, TrialOutcome::Completed(_)) {
                    "completed"
                } else {
                    "cancelled"
                }),
            ),
            ("report_id", s(r.content_id())),
            ("report", crate::backtest_encoding::result_value(r)?),
        ]),
    })
}
pub(crate) fn result(r: &OptimizationResult, limit: usize) -> Result<Vec<u8>, ExecutionError> {
    let trials = r
        .trials
        .iter()
        .map(|t| {
            Ok(o([
                ("id", s(t.id)),
                ("parameters", parameters_json(&t.parameters)),
                ("training", outcome(&t.outcome)?),
            ]))
        })
        .collect::<Result<Vec<_>, ExecutionError>>()?;
    let value = o([
        ("format", s("fqe.optimization-result")),
        ("schema_version", V::from(1)),
        ("manifest_id", s(r.manifest_id)),
        ("manifest", json::decode_input(&r.manifest)?),
        (
            "terminal",
            s(match r.terminal {
                SearchTerminal::Completed => "completed",
                SearchTerminal::Cancelled => "cancelled",
                SearchTerminal::NoSuccessfulTrials => "no_successful_trials",
            }),
        ),
        ("completed_trials", s(r.completed_trials)),
        ("trials", V::Array(trials)),
        ("ranking", V::Array(r.ranking.iter().map(s).collect())),
        ("selected", r.selected.map_or(V::Null, s)),
        ("validation", outcome(&r.validation)?),
        (
            "warnings",
            V::Array(vec![
                s(
                    "experimental finite local grid; training winner is not a recommendation or a guarantee of out-of-sample performance",
                ),
                s(
                    "validation is reported separately, never used in ranking; past-only validation warmup may overlap training history",
                ),
            ]),
        ),
    ]);
    Ok(json::encode(&value, limit)?)
}
