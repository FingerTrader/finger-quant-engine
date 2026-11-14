#[path = "../examples/support/backtests.rs"]
mod support;
use fqe_core::{
    finance::parse_decimal as decimal,
    numeric::Decimal,
    types::{NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{
    event_plan::LoadedEventPlan,
    financial_exchange::{self as json, JsonValue as V},
    history::{HistoricalDataset, HistorySpec},
    parameters::ParameterInput,
    workflow::LoadedWorkflow,
};
use fqe_runtime::{
    backtest::*,
    ledger::LedgerProfile,
    margin::MarginRules,
    portfolio::{self, EquityPoint},
    simulator::*,
};
use std::sync::{Arc, atomic::AtomicBool};
fn m(v: &str) -> Decimal {
    decimal(v, 2).unwrap()
}
fn result(control: RunControl) -> BacktestResult {
    PreparedBacktest::new(
        support::selection(),
        support::plan(),
        support::policies(),
        support::config(),
        "test",
        42,
        control,
    )
    .unwrap()
    .run()
    .unwrap()
}
fn plan_value() -> V {
    json::decode_input(include_bytes!("fixtures/p3/backtest-events.json")).unwrap()
}
fn compile(v: &V) -> StrategyPlan {
    StrategyPlan::Event(Arc::new(
        LoadedEventPlan::from_bytes(&json::encode(v, 262144).unwrap())
            .unwrap()
            .compile(&support::registry(), vec![])
            .unwrap(),
    ))
}
fn workflow(v: &V) -> StrategyPlan {
    StrategyPlan::Workflow(Arc::new(
        LoadedWorkflow::from_bytes(&json::encode(v, 262144).unwrap())
            .unwrap()
            .compile(&support::registry(), vec![])
            .unwrap(),
    ))
}
#[test]
fn actual_strategy_to_two_fills_account_report_and_independent_hashes() {
    let r = result(RunControl::Complete);
    assert_eq!(r.metrics().final_equity, m("97.00"));
    assert_eq!(r.metrics().net_profit, m("-3.00"));
    assert_eq!(r.metrics().fees, m("1.00"));
    assert_eq!(r.metrics().max_drawdown, m("3.00"));
    assert_eq!(
        r.metrics().total_return,
        Some(decimal("-0.030000000000", 12).unwrap())
    );
    assert_eq!(
        r.curve().iter().map(|p| p.equity).collect::<Vec<_>>(),
        [
            m("100.00"),
            m("100.00"),
            m("100.00"),
            m("100.00"),
            m("100.00"),
            m("99.50"),
            m("97.00")
        ]
    );
    assert_eq!(
        r.broker()
            .fills()
            .iter()
            .map(|f| (f.source_bar, f.side, f.price, f.quantity, f.fee))
            .collect::<Vec<_>>(),
        [
            (
                6,
                fqe_runtime::ledger::TradeSide::Buy,
                m("3.00"),
                decimal("2.000", 3).unwrap(),
                m("0.50")
            ),
            (
                7,
                fqe_runtime::ledger::TradeSide::Sell,
                m("2.00"),
                decimal("2.000", 3).unwrap(),
                m("0.50")
            )
        ]
    );
    let replay =
        SimulationBroker::replay(r.broker().config().clone(), r.broker().receipts()).unwrap();
    assert_eq!(replay.ledger().journal(), r.broker().ledger().journal());
    assert_eq!(
        r.content_id(),
        json::content_id(b"FQE-BACKTEST-RESULT/1\n", r.json())
    );
    assert_eq!(
        r.manifest_id(),
        json::content_id(b"FQE-BACKTEST-INPUTS/1\n", r.manifest())
    );
    let value: V = json::decode_input(r.json()).unwrap();
    assert_eq!(value["valuation"]["unrealized"], "0.00");
    assert_eq!(
        value["manifest"]["history_inputs"]["financial_execution"],
        "not-selected"
    );
    assert_eq!(value["manifest"]["financial_execution"]["model"], MODEL);
}
#[test]
fn deterministic_reruns_and_every_native_financial_prefix() {
    let p = support::prepared();
    let a = p.run().unwrap();
    let b = p.run().unwrap();
    assert_eq!(a.json(), b.json());
    assert_eq!(a.content_id(), b.content_id());
    for i in 0..=a.broker().receipts().len() {
        let mut restored =
            SimulationBroker::replay(a.broker().config().clone(), &a.broker().receipts()[..i])
                .unwrap();
        for receipt in &a.broker().receipts()[i..] {
            assert_eq!(restored.apply(receipt.command.clone()).unwrap(), receipt);
        }
        assert_eq!(restored.ledger().journal(), a.broker().ledger().journal());
    }
}
#[test]
fn exact_metrics_match_nine_independent_fraction_cases_and_reject_bad_curve() {
    for line in include_str!("fixtures/p3/backtest-metrics.expected.tsv")
        .lines()
        .skip(1)
    {
        let c = line.split('\t').collect::<Vec<_>>();
        let initial = m(c[1]);
        let curve = if c[2].is_empty() {
            vec![]
        } else {
            c[2].split(',')
                .enumerate()
                .map(|(i, e)| EquityPoint {
                    source_sequence: i as u64 + 1,
                    timestamp_ns: i as u64,
                    mark: m("1.00"),
                    cash: m(e),
                    debt: m("0.00"),
                    quantity: decimal("0.000", 3).unwrap(),
                    equity: m(e),
                    fees: m(c[3]),
                })
                .collect()
        };
        let v = portfolio::calculate(initial, &curve, m(c[3])).unwrap();
        assert_eq!(v.final_equity, m(c[4]), "{}", c[0]);
        assert_eq!(v.net_profit, m(c[5]));
        assert_eq!(v.fees, m(c[6]));
        assert_eq!(
            v.total_return,
            if c[7] == "-" {
                None
            } else {
                Some(decimal(c[7], 12).unwrap())
            }
        );
        assert_eq!(v.max_drawdown, m(c[8]));
        assert_eq!(
            v.max_drawdown_ratio,
            if c[9] == "-" {
                None
            } else {
                Some(decimal(c[9], 12).unwrap())
            }
        );
    }
    let mut curve = result(RunControl::Complete).curve().to_vec();
    curve[1].source_sequence = curve[0].source_sequence;
    assert!(portfolio::calculate(m("100.00"), &curve, m("0.00")).is_err());
    assert!(portfolio::calculate(m("-1.00"), &[], m("0.00")).is_err());
    assert!(portfolio::calculate(m("1.00"), &[], decimal("0.000", 3).unwrap()).is_err());
}
#[test]
fn prefix_stop_cancel_empty_and_pending_release_keep_truthful_open_positions() {
    let empty = result(RunControl::CancelAfter(0));
    assert_eq!(empty.terminal(), BacktestTerminal::Cancelled);
    assert!(empty.observations().is_empty());
    assert_eq!(empty.metrics().final_equity, m("100.00"));
    let pending = result(RunControl::CancelAfter(5));
    assert_eq!(pending.broker().orders().len(), 1);
    assert!(pending.broker().fills().is_empty());
    assert_eq!(pending.broker().orders()[0].status, OrderStatus::Cancelled);
    assert!(pending.broker().ledger().reservations().is_empty());
    let stopped = result(RunControl::StopAfter(6));
    assert_eq!(stopped.terminal(), BacktestTerminal::Stopped);
    assert_eq!(stopped.broker().fills().len(), 1);
    assert_eq!(
        stopped.broker().ledger().view().quantity,
        decimal("2.000", 3).unwrap()
    );
    assert_eq!(stopped.metrics().net_profit, m("-0.50"));
    assert_eq!(stopped.valuation().unrealized, m("0.00"));
    let async_cancel = support::prepared()
        .run_with_cancel(&AtomicBool::new(true))
        .unwrap();
    assert_eq!(async_cancel.terminal(), BacktestTerminal::Cancelled);
    assert!(async_cancel.observations().is_empty());
    assert_eq!(
        result(RunControl::StopAfter(7)).terminal(),
        BacktestTerminal::Stopped
    );
}
#[test]
fn bound_parameters_provenance_and_no_trade_run() {
    let ty = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 12,
            unit: NumericUnit::Ratio,
        },
    )
    .unwrap();
    let p = LoadedEventPlan::from_bytes(include_bytes!("fixtures/p3/backtest-events.json"))
        .unwrap()
        .compile(
            &support::registry(),
            vec![(
                "threshold".into(),
                ParameterInput::Value(fqe_core::types::Evaluation::Ready(
                    TypedValue::new(
                        ty,
                        ScalarValue::Decimal(decimal("90.000000000000", 12).unwrap()),
                    )
                    .unwrap(),
                )),
            )],
        )
        .unwrap();
    let prep = PreparedBacktest::new(
        support::selection(),
        StrategyPlan::Event(Arc::new(p)),
        support::policies(),
        support::config(),
        "test",
        42,
        RunControl::Complete,
    )
    .unwrap();
    let r = prep.run().unwrap();
    assert!(r.broker().fills().is_empty());
    assert_eq!(r.metrics().final_equity, m("100.00"));
    assert_eq!(
        json::decode_input(r.manifest()).unwrap()["bound_parameters"]["threshold"]["value"],
        "90000000000000"
    );
    assert_ne!(r.manifest_id(), result(RunControl::Complete).manifest_id());
}
#[test]
fn typed_policy_preflight_and_missing_input_rejections() {
    for source in [
        PolicySource::EventBoolean {
            name: "missing".into(),
        },
        PolicySource::EventBoolean { name: "rsi".into() },
        PolicySource::WorkflowBoolean {
            instance: "policy".into(),
            output: "entry".into(),
        },
    ] {
        let mut p = support::policies();
        p[0].source = source;
        assert_eq!(
            PreparedBacktest::new(
                support::selection(),
                support::plan(),
                p,
                support::config(),
                "test",
                0,
                RunControl::Complete
            )
            .err()
            .unwrap()
            .code,
            "backtest_policy"
        );
    }
    let mut p = support::policies();
    p.push(p[0].clone());
    assert!(
        PreparedBacktest::new(
            support::selection(),
            support::plan(),
            p,
            support::config(),
            "test",
            0,
            RunControl::Complete
        )
        .is_err()
    );
    let mut c = support::config();
    c.timeframe_ns += 1;
    assert!(
        PreparedBacktest::new(
            support::selection(),
            support::plan(),
            vec![],
            c,
            "test",
            0,
            RunControl::Complete
        )
        .is_err()
    );
    assert!(
        PreparedBacktest::new(
            support::selection(),
            support::plan(),
            vec![],
            support::config(),
            "test",
            0,
            RunControl::StopAfter(8)
        )
        .is_err()
    );
    let spec = Arc::new(
        HistorySpec::from_bytes(include_bytes!("fixtures/p3/backtest-history-spec.json")).unwrap(),
    );
    assert!(HistoricalDataset::from_csv(spec, b"", None).is_err());
}
#[test]
fn selection_and_engine_warmup_never_generate_orders() {
    let data = support::dataset();
    let sel = data
        .select(
            data.bars()[5].input().open_ns,
            data.bars()[6].input().close_ns,
            5,
        )
        .unwrap();
    let r = PreparedBacktest::new(
        sel,
        support::plan(),
        support::policies(),
        support::config(),
        "test",
        0,
        RunControl::Complete,
    )
    .unwrap()
    .run()
    .unwrap();
    assert!(r.broker().fills().is_empty());
    assert_eq!(r.curve().len(), 2);
    assert!(
        r.decisions()
            .iter()
            .filter(|d| d.source_sequence <= 5)
            .all(|d| d.outcome == DecisionOutcome::Warmup)
    );
    let mut p = plan_value();
    p["warmup_samples"] = V::from("7");
    let r = PreparedBacktest::new(
        support::selection(),
        compile(&p),
        support::policies(),
        support::config(),
        "test",
        0,
        RunControl::Complete,
    )
    .unwrap()
    .run()
    .unwrap();
    assert!(r.broker().fills().is_empty());
}
#[test]
fn workflow_boolean_bindings_and_flat_signals_are_not_long_entries() {
    let mut v = json::decode_input(include_bytes!("fixtures/p3/backtest-workflow.json")).unwrap();
    let mut p = support::policies();
    p[1].source = PolicySource::WorkflowBoolean {
        instance: "policy".into(),
        output: "exit".into(),
    };
    let r = PreparedBacktest::new(
        support::selection(),
        workflow(&v),
        p.clone(),
        support::config(),
        "test",
        0,
        RunControl::Complete,
    )
    .unwrap()
    .run()
    .unwrap();
    assert_eq!(r.metrics().final_equity, m("97.00"));
    assert_eq!(
        json::decode_input(r.manifest()).unwrap()["workflow_parameters"]["policy"]["step"]["value"],
        "1"
    );
    v["signals"]
        .as_array_mut()
        .unwrap()
        .retain(|s| s["kind"] == "flat");
    let r = PreparedBacktest::new(
        support::selection(),
        workflow(&v),
        vec![p[0].clone()],
        support::config(),
        "test",
        0,
        RunControl::Complete,
    )
    .unwrap()
    .run()
    .unwrap();
    assert!(r.broker().orders().is_empty());
}
#[test]
fn late_batch_quota_failure_returns_no_report_and_does_not_poison_rerun() {
    let mut c = support::config();
    c.limits.fills = 1;
    let p = PreparedBacktest::new(
        support::selection(),
        support::plan(),
        support::policies(),
        c,
        "test",
        0,
        RunControl::Complete,
    )
    .unwrap();
    let e = p.run().err().unwrap();
    assert_eq!(e.code, "simulation_limit");
    assert_eq!(p.run().err().unwrap(), e);
    assert_eq!(support::prepared().run().unwrap().broker().fills().len(), 2);
}
#[test]
fn changed_future_prices_cannot_change_prior_orders_decisions_or_equity() {
    let data = support::dataset();
    let base = result(RunControl::Complete);
    let csv =
        String::from_utf8(include_bytes!("fixtures/p3/backtest-history.csv").to_vec()).unwrap();
    let mut rows = csv.lines().map(str::to_owned).collect::<Vec<_>>();
    rows[7] =
        "DEMO,1m,2024-01-02T00:06:00Z,2024-01-02T00:07:00Z,50.00,50.00,50.00,50.00,100.000,true"
            .into();
    let revised = Arc::new(
        HistoricalDataset::from_csv(
            Arc::new(data.spec().clone()),
            (rows.join("\n") + "\n").as_bytes(),
            None,
        )
        .unwrap(),
    );
    let selection = revised
        .select(
            revised.bars()[0].input().open_ns,
            revised.bars()[6].input().close_ns,
            0,
        )
        .unwrap();
    let r = PreparedBacktest::new(
        selection,
        support::plan(),
        support::policies(),
        support::config(),
        "test",
        42,
        RunControl::Complete,
    )
    .unwrap()
    .run()
    .unwrap();
    assert_eq!(&r.decisions()[..12], &base.decisions()[..12]);
    assert_eq!(&r.curve()[..6], &base.curve()[..6]);
    assert_eq!(r.broker().fills()[0], base.broker().fills()[0]);
    assert_ne!(r.metrics().net_profit, base.metrics().net_profit);
}
#[test]
fn margin_runner_reports_debt_in_equity_and_finishes_without_invented_close() {
    let mut c = support::config();
    c.account.profile = LedgerProfile::MarginLong;
    c.margin = Some(MarginRules {
        max_leverage_bps: 20000,
        maintenance_bps: 2500,
    });
    c.account.initial_cash = m("5.00");
    let mut p = support::policies();
    if let PolicyAction::BuyIfFlat { cash_cap, .. } = &mut p[0].action {
        *cash_cap = m("8.50");
    }
    let r = PreparedBacktest::new(
        support::selection(),
        support::plan(),
        vec![p[0].clone()],
        c,
        "test",
        0,
        RunControl::StopAfter(6),
    )
    .unwrap()
    .run()
    .unwrap();
    let a = r.broker().ledger().view();
    assert_eq!(a.debt, m("1.50"));
    assert_eq!(r.metrics().final_equity, m("4.50"));
    assert_eq!(r.metrics().net_profit, m("-0.50"));
    assert_eq!(r.valuation().market_value, m("6.00"));
    assert!(r.broker().ledger().reservations().is_empty());
}
#[test]
fn exact_json_encoder_bounds_float_depth_and_bytes() {
    assert!(json::encode(&V::from(0.1), 1024).is_err());
    assert!(json::encode(&V::from("too long"), 3).is_err());
    let mut v = V::Null;
    for _ in 0..34 {
        v = V::Array(vec![v]);
    }
    assert!(json::encode(&v, 1024).is_err());
    assert!(json::encode(&V::Null, 0).is_err());
}
#[test]
fn source_capacity_rejected_before_running_and_policy_origin_is_bound_and_deduped() {
    let mut spec =
        json::decode_input(include_bytes!("fixtures/p3/backtest-history-spec.json")).unwrap();
    spec["calendar"]["sessions"][0]["close"] = V::from("2024-01-02T04:17:00Z");
    spec["acquired_at"] = V::from("2024-01-02T04:18:00Z");
    let spec = Arc::new(HistorySpec::from_bytes(&json::encode(&spec, 262144).unwrap()).unwrap());
    let mut csv = String::from(
        "instrument_id,timeframe,open_time,close_time,open,high,low,close,volume,is_closed\n",
    );
    for n in 0..257 {
        csv.push_str(&format!("DEMO,1m,2024-01-02T{:02}:{:02}:00Z,2024-01-02T{:02}:{:02}:00Z,1.00,1.00,1.00,1.00,2.000,true\n",n/60,n%60,(n+1)/60,(n+1)%60));
    }
    let data = Arc::new(HistoricalDataset::from_csv(spec, csv.as_bytes(), None).unwrap());
    let selection = data
        .select(
            data.bars()[0].input().open_ns,
            data.bars()[256].input().close_ns,
            0,
        )
        .unwrap();
    assert_eq!(
        PreparedBacktest::new(
            selection,
            support::plan(),
            vec![],
            support::config(),
            "test",
            0,
            RunControl::Complete
        )
        .err()
        .unwrap()
        .code,
        "backtest_limit"
    );
    let r = result(RunControl::Complete);
    let mut broker = SimulationBroker::new(r.broker().config().clone()).unwrap();
    for b in support::dataset().bars().iter().take(5) {
        broker
            .apply(SimulationCommand::Bar(Box::new(b.clone())))
            .unwrap();
    }
    let intent = r.broker().orders()[0].intent.clone();
    let mut bad = intent.clone();
    if let IntentOrigin::Policy { timestamp_ns, .. } = &mut bad.origin {
        *timestamp_ns += 1;
    }
    assert_eq!(
        broker
            .apply(SimulationCommand::Submit(bad))
            .unwrap_err()
            .code,
        "simulation_signal"
    );
    broker
        .apply(SimulationCommand::Submit(intent.clone()))
        .unwrap();
    let mut duplicate = intent;
    duplicate.id = "distinct.id".into();
    assert_eq!(
        broker
            .apply(SimulationCommand::Submit(duplicate))
            .unwrap_err()
            .code,
        "simulation_duplicate"
    );
}
