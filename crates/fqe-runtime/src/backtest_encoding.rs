//! Experimental financial report schema, independent of Rust Debug formatting and memory layout.
use crate::{backtest::*, ledger::*, margin::MarginStatus, simulator::*};
use fqe_core::{
    finance::decimal_text,
    numeric::Decimal,
    types::{Evaluation, TypedValue},
};
use fqe_ir::{
    financial_exchange::{self as json, JsonValue as V, object as o, string as s},
    model::ContentId,
};
use std::collections::BTreeMap;
fn d(v: Decimal) -> V {
    s(decimal_text(v))
}
fn opt(v: Option<Decimal>) -> V {
    v.map_or(V::Null, d)
}
fn list<T>(values: &[T], f: impl Fn(&T) -> V) -> V {
    V::Array(values.iter().map(f).collect())
}
fn side(v: TradeSide) -> V {
    s(match v {
        TradeSide::Buy => "buy",
        TradeSide::Sell => "sell",
    })
}
fn margin(v: Option<MarginStatus>) -> V {
    v.map_or(V::Null, |v| {
        s(match v {
            MarginStatus::Healthy => "healthy",
            MarginStatus::Liquidating => "liquidating",
            MarginStatus::Liquidated => "liquidated",
            MarginStatus::Insolvent => "insolvent",
            MarginStatus::LiquidationFailed => "liquidation_failed",
            MarginStatus::Stopped => "stopped",
        })
    })
}
fn source(v: &SignalSource) -> V {
    o([("run", s(&v.run)), ("execution_id", s(v.execution_id))])
}
fn policy(v: &PolicyBinding) -> V {
    let src = match &v.source {
        PolicySource::LongSignal => o([("kind", s("long_signal"))]),
        PolicySource::EventBoolean { name } => {
            o([("kind", s("event_boolean")), ("output", s(name))])
        }
        PolicySource::WorkflowBoolean { instance, output } => o([
            ("kind", s("workflow_boolean")),
            ("instance", s(instance)),
            ("output", s(output)),
        ]),
    };
    let action = match v.action {
        PolicyAction::BuyIfFlat { quantity, cash_cap } => o([
            ("kind", s("buy_if_flat")),
            ("quantity", d(quantity)),
            ("cash_cap", d(cash_cap)),
        ]),
        PolicyAction::CloseLong => o([("kind", s("close_long"))]),
    };
    o([("id", s(&v.id)), ("source", src), ("action", action)])
}
pub(crate) fn manifest(
    p: &PreparedBacktest,
    history: &[u8],
) -> Result<Vec<u8>, crate::events::ExecutionError> {
    let c = &p.config;
    let a = &c.account;
    let control = match p.control {
        RunControl::Complete => o([("kind", s("complete"))]),
        RunControl::StopAfter(n) => o([("kind", s("stop_after")), ("source_bars", s(n))]),
        RunControl::CancelAfter(n) => o([("kind", s("cancel_after")), ("source_bars", s(n))]),
    };
    let value = o([
        ("format", s("fqe.backtest-inputs")),
        ("schema_version", V::from(1)),
        ("engine_version", s(env!("CARGO_PKG_VERSION"))),
        ("engine_id", s(crate::source_identity::engine_id())),
        ("run", s(&p.run)),
        ("seed", s(p.seed)),
        ("history_inputs", json::decode_input(history)?),
        (
            "workflow_parameters",
            match &p.strategy {
                StrategyPlan::Event(_) => V::Null,
                StrategyPlan::Workflow(w) => o(w
                    .instances()
                    .iter()
                    .map(|i| (i.id.as_str(), json::parameters(&i.parameters)))),
            },
        ),
        (
            "bound_parameters",
            json::parameters(p.strategy.event().graph().parameters()),
        ),
        (
            "parameters_id",
            s(fqe_ir::checkpoint::parameters_id(
                p.strategy.event().graph().parameters(),
            )),
        ),
        (
            "account",
            o([
                ("id", s(&a.account)),
                ("profile", s(a.profile.version())),
                ("initial_cash", d(a.initial_cash)),
            ]),
        ),
        (
            "financial_execution",
            o([
                (
                    "model",
                    s(if c.margin.is_some() {
                        crate::margin::MARGIN_MODEL
                    } else {
                        MODEL
                    }),
                ),
                ("timeframe_ns", s(c.timeframe_ns)),
                ("participation_bps", s(c.participation_bps)),
                ("fee_bps", s(c.fee_bps)),
                ("minimum_fee", d(c.minimum_fee)),
                ("slippage_bps", s(c.slippage_bps)),
                (
                    "signal_source",
                    source(c.signal_source.as_ref().expect("prepared source")),
                ),
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
            ]),
        ),
        ("policies", list(&p.policies, policy)),
        ("control", control),
        ("metrics_model", s("fqe.close-equity-ratio12-half-even/1")),
        ("max_source_bars", s(MAX_BACKTEST_BARS)),
        ("max_report_bytes", s(json::MAX_FINANCIAL_REPORT_BYTES)),
        (
            "phases",
            s("incoming-open-fill;closed-strategy;ordered-policy;close-equity"),
        ),
    ]);
    let bytes = json::encode(&value, 262144)?;
    json::decode_input(&bytes)?;
    Ok(bytes)
}
fn account(v: &AccountView) -> V {
    o([
        ("cash", d(v.cash)),
        ("debt", d(v.debt)),
        ("reserved_cash", d(v.reserved_cash)),
        ("restricted_short_proceeds", d(v.restricted_short_proceeds)),
        ("available_cash", d(v.available_cash)),
        ("quantity", d(v.quantity)),
        ("reserved_quantity", d(v.reserved_quantity)),
        ("available_long_quantity", d(v.available_long_quantity)),
        ("cost", d(v.cost)),
        ("realized_gross", d(v.realized_gross)),
        ("fees", d(v.fees)),
        ("external_capital", d(v.external_capital)),
        ("cash_rounding_total", d(v.cash_rounding_total)),
    ])
}
fn origin(v: &IntentOrigin) -> V {
    match v {
        IntentOrigin::Manual => o([("kind", s("manual"))]),
        IntentOrigin::Signal(v) => o([
            ("kind", s("signal")),
            ("source", source(&v.source)),
            ("sequence", s(v.sequence)),
            ("timestamp_ns", s(v.timestamp_ns)),
            ("index", s(v.index)),
        ]),
        IntentOrigin::Policy {
            source: src,
            sequence,
            timestamp_ns,
            binding,
        } => o([
            ("kind", s("policy")),
            ("source", source(src)),
            ("sequence", s(sequence)),
            ("timestamp_ns", s(timestamp_ns)),
            ("binding", s(binding)),
        ]),
    }
}
fn intent(v: &OrderIntent) -> V {
    o([
        ("id", s(&v.id)),
        ("side", side(v.side)),
        (
            "kind",
            match v.kind {
                OrderKind::Market => o([("kind", s("market"))]),
                OrderKind::Limit(p) => o([("kind", s("limit")), ("price", d(p))]),
            },
        ),
        ("quantity", d(v.quantity)),
        ("cash_cap", opt(v.cash_cap)),
        ("origin", origin(&v.origin)),
    ])
}
fn order(v: &SimOrder) -> V {
    o([
        ("number", s(v.number)),
        ("intent", intent(&v.intent)),
        ("liquidation", V::Bool(v.liquidation)),
        ("submitted_after", s(v.submitted_after)),
        ("filled_quantity", d(v.filled_quantity)),
        (
            "status",
            s(match v.status {
                OrderStatus::Accepted => "accepted",
                OrderStatus::PartiallyFilled => "partially_filled",
                OrderStatus::Filled => "filled",
                OrderStatus::Cancelled => "cancelled",
                OrderStatus::Replaced => "replaced",
                OrderStatus::Rejected => "rejected",
            }),
        ),
        (
            "rejection",
            v.rejection.map_or(V::Null, |r| {
                s(match r {
                    Rejection::MarginLimit => "margin_limit",
                    Rejection::InsufficientCash => "insufficient_cash",
                    Rejection::InsufficientPosition => "insufficient_position",
                    Rejection::CashCap => "cash_cap",
                    Rejection::PriceOrModelRange => "price_or_model_range",
                    Rejection::AccountLimits => "account_limits",
                })
            }),
        ),
    ])
}
fn fill(v: &SimFill) -> V {
    o([
        ("number", s(v.number)),
        ("order", s(&v.order)),
        ("source_bar", s(v.source_bar)),
        ("timestamp_ns", s(v.timestamp_ns)),
        ("side", side(v.side)),
        ("price", d(v.price)),
        ("quantity", d(v.quantity)),
        ("settled_gross", d(v.settled_gross)),
        ("fee", d(v.fee)),
        ("ledger_sequence", s(v.ledger_sequence)),
    ])
}
fn amount(v: &Reservation) -> V {
    o([("cash", d(v.cash)), ("quantity", d(v.quantity))])
}
fn action(v: &LedgerAction) -> V {
    match v {
        LedgerAction::Borrow { amount } => o([("kind", s("borrow")), ("amount", d(*amount))]),
        LedgerAction::Repay { amount } => o([("kind", s("repay")), ("amount", d(*amount))]),
        LedgerAction::Funding { direction, amount } => o([
            ("kind", s("funding")),
            (
                "direction",
                s(match direction {
                    FundingDirection::Deposit => "deposit",
                    FundingDirection::Withdraw => "withdraw",
                }),
            ),
            ("amount", d(*amount)),
        ]),
        LedgerAction::Reserve {
            reservation,
            amount: a,
        } => o([
            ("kind", s("reserve")),
            ("reservation", s(reservation)),
            ("amount", amount(a)),
        ]),
        LedgerAction::Release { reservation } => {
            o([("kind", s("release")), ("reservation", s(reservation))])
        }
        LedgerAction::Trade {
            side: sd,
            price,
            quantity,
            fee,
            release,
        } => o([
            ("kind", s("trade")),
            ("side", side(*sd)),
            ("price", d(*price)),
            ("quantity", d(*quantity)),
            ("fee", d(*fee)),
            (
                "release",
                release.as_ref().map_or(V::Null, |r| {
                    o([
                        ("reservation", s(&r.reservation)),
                        ("cash", d(r.cash)),
                        ("quantity", d(r.quantity)),
                    ])
                }),
            ),
        ]),
    }
}
fn money_account(v: MoneyAccount) -> V {
    s(match v {
        MoneyAccount::LoanDebt => "loan_debt",
        MoneyAccount::Cash => "cash",
        MoneyAccount::InventoryCost => "inventory_cost",
        MoneyAccount::ExternalCapital => "external_capital",
        MoneyAccount::RealizedProfit => "realized_profit",
        MoneyAccount::FeeExpense => "fee_expense",
    })
}
fn journal(v: &JournalEntry, manifest_id: ContentId) -> V {
    o([
        ("sequence", s(v.sequence)),
        (
            "transaction",
            o([
                ("id", s(&v.transaction.id)),
                ("timestamp_ns", s(v.transaction.timestamp_ns)),
                ("action", action(&v.transaction.action)),
            ]),
        ),
        (
            "opening",
            v.opening.as_ref().map_or(V::Null, |op| {
                o([
                    ("engine_id", s(op.engine_id)),
                    ("manifest_id", s(manifest_id)),
                ])
            }),
        ),
        (
            "money",
            list(&v.money, |p| {
                o([("account", money_account(p.account)), ("delta", d(p.delta))])
            }),
        ),
        (
            "quantity",
            v.quantity.as_ref().map_or(V::Null, |q| {
                o([
                    ("inventory", d(q.inventory)),
                    ("counterparty", d(q.counterparty)),
                ])
            }),
        ),
        (
            "trade",
            v.trade.as_ref().map_or(V::Null, |t| {
                o([
                    ("exact_gross", d(t.exact_gross)),
                    ("settled_gross", d(t.settled_gross)),
                    ("allocated_basis", d(t.allocated_basis)),
                    ("cash_rounding", d(t.cash_rounding)),
                ])
            }),
        ),
        ("after", account(&v.after)),
    ])
}
fn values(v: &BTreeMap<String, Evaluation<TypedValue>>) -> V {
    o(v.iter()
        .map(|(k, v)| (k.as_str(), fqe_ir::workflow::readiness_json(v))))
}
fn signal(v: &crate::SignalRecord) -> V {
    o([
        ("sequence", s(v.source_sequence)),
        ("timestamp_ns", s(v.timestamp_ns)),
        (
            "kind",
            s(match v.signal.kind {
                fqe_core::SignalKind::Long => "long",
                fqe_core::SignalKind::Flat => "flat",
            }),
        ),
        ("instrument", s(v.signal.instrument.as_str())),
    ])
}
fn observation(v: &StrategyObservation) -> V {
    match v {
        StrategyObservation::Event(v) => o([
            ("kind", s("event")),
            ("run", s(&v.run)),
            ("execution_id", s(v.execution_id)),
            ("source_sequence", s(v.source_sequence)),
            ("timestamp_ns", s(v.timestamp_ns)),
            ("matched", V::Bool(v.matched)),
            ("warmup", V::Bool(v.warmup)),
            ("outputs", values(&v.outputs)),
            ("signals", list(&v.signals, signal)),
        ]),
        StrategyObservation::Workflow(v) => o([
            ("kind", s("workflow")),
            ("run", s(&v.run)),
            ("execution_id", s(v.execution_id)),
            ("source_sequence", s(v.source_sequence)),
            ("timestamp_ns", s(v.timestamp_ns)),
            ("matched", V::Bool(v.matched)),
            ("warmup", V::Bool(v.warmup)),
            ("event_outputs", values(&v.event_outputs)),
            (
                "outputs",
                o(v.outputs.iter().map(|(k, v)| (k.as_str(), values(v)))),
            ),
            (
                "transitions",
                o(v.transitions
                    .iter()
                    .map(|(k, v)| (k.as_str(), list(v, |x| s(x))))),
            ),
            (
                "signals",
                list(&v.signals, |v| {
                    o([
                        ("instance", s(&v.instance)),
                        ("output", s(&v.output)),
                        ("signal", signal(&v.record)),
                    ])
                }),
            ),
        ]),
    }
}
fn decision(v: &PolicyDecision) -> V {
    o([
        ("source_sequence", s(v.source_sequence)),
        ("timestamp_ns", s(v.timestamp_ns)),
        ("binding", s(&v.binding)),
        (
            "outcome",
            s(match v.outcome {
                DecisionOutcome::Warmup => "warmup",
                DecisionOutcome::NotReadyOrFalse => "not_ready_or_false",
                DecisionOutcome::PositionOrPending => "position_or_pending",
                DecisionOutcome::FlatOrPending => "flat_or_pending",
                DecisionOutcome::ResidualBelowMinimum => "residual_below_minimum",
                DecisionOutcome::Submitted => "submitted",
            }),
        ),
        ("order", v.order.as_ref().map_or(V::Null, s)),
    ])
}
fn simulation_event(v: &SimulationEvent) -> V {
    match v {
        SimulationEvent::Margin {
            status,
            source_bar,
            mark,
            equity,
            exposure,
            required,
        } => o([
            ("kind", s("margin")),
            ("status", margin(Some(*status))),
            ("source_bar", s(source_bar)),
            ("mark", d(*mark)),
            ("equity", d(*equity)),
            ("exposure", d(*exposure)),
            ("required", d(*required)),
        ]),
        SimulationEvent::Order(v) => o([("kind", s("order")), ("order", order(v))]),
        SimulationEvent::Fill(v) => o([("kind", s("fill")), ("fill", fill(v))]),
        SimulationEvent::Finished => o([("kind", s("finished"))]),
    }
}
pub(crate) fn result(r: &BacktestResult) -> Result<Vec<u8>, crate::events::ExecutionError> {
    let m = &r.metrics;
    let b = &r.broker;
    let metrics = o([
        ("initial_equity", d(m.initial_equity)),
        ("final_equity", d(m.final_equity)),
        ("net_profit", d(m.net_profit)),
        ("fees", d(m.fees)),
        ("total_return", opt(m.total_return)),
        ("max_drawdown", d(m.max_drawdown)),
        ("max_drawdown_ratio", opt(m.max_drawdown_ratio)),
    ]);
    let value = o([
        ("format", s("fqe.backtest-result")),
        ("schema_version", V::from(1)),
        ("manifest_id", s(r.manifest_id)),
        ("manifest", json::decode_input(&r.manifest)?),
        (
            "terminal",
            s(match r.terminal {
                BacktestTerminal::Completed => "completed",
                BacktestTerminal::Stopped => "stopped",
                BacktestTerminal::Cancelled => "cancelled",
            }),
        ),
        ("processed_source_bars", s(r.observations.len())),
        ("observations", list(&r.observations, observation)),
        ("decisions", list(&r.decisions, decision)),
        ("orders", list(b.orders(), order)),
        ("fills", list(b.fills(), fill)),
        (
            "journal",
            list(b.ledger().journal(), |j| journal(j, r.manifest_id)),
        ),
        ("account", account(&b.ledger().view())),
        (
            "valuation",
            o([
                ("market_value", d(r.valuation.market_value)),
                ("equity", d(r.valuation.equity)),
                ("unrealized", d(r.valuation.unrealized)),
                ("total_profit", d(r.valuation.total_profit)),
            ]),
        ),
        ("margin_status", margin(b.margin_status())),
        (
            "execution_events",
            list(b.receipts(), |v| {
                o([
                    ("command_sequence", s(v.sequence)),
                    ("margin", margin(v.margin)),
                    ("events", list(&v.events, simulation_event)),
                    ("account", account(&v.account)),
                ])
            }),
        ),
        (
            "equity_curve",
            list(&r.curve, |p| {
                o([
                    ("source_sequence", s(p.source_sequence)),
                    ("timestamp_ns", s(p.timestamp_ns)),
                    ("mark", d(p.mark)),
                    ("cash", d(p.cash)),
                    ("debt", d(p.debt)),
                    ("quantity", d(p.quantity)),
                    ("equity", d(p.equity)),
                    ("fees", d(p.fees)),
                ])
            }),
        ),
        ("metrics", metrics),
        (
            "warnings",
            V::Array(vec![
                s("experimental bounded single-instrument historical simulation; not live trading"),
                s(
                    "close-sampled drawdown omits intrabar extrema; pending orders cancelled at finish; open holdings retained",
                ),
                s(
                    "raw data only; no survivorship/corporate-action correction; no annualization/Sharpe or benchmark inference",
                ),
            ]),
        ),
    ]);
    Ok(json::encode(&value, json::MAX_FINANCIAL_REPORT_BYTES)?)
}
