use fqe_core::{
    bars::{BarInput, ClosedBar},
    finance::parse_decimal,
    numeric::Decimal,
    types::{NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::history::HistorySpec;
use fqe_runtime::{ledger::*, simulator::*};
fn m(s: &str) -> Decimal {
    parse_decimal(s, 2).unwrap()
}
fn q(s: &str) -> Decimal {
    parse_decimal(s, 3).unwrap()
}
fn config() -> SimulationConfig {
    let spec = HistorySpec::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/history/cash-long-spec.json"
    ))
    .unwrap();
    SimulationConfig {
        margin: None,
        limits: SimulationLimits::default(),
        account: AccountConfig {
            account: "simulation".into(),
            instrument: spec.instrument().clone(),
            profile: LedgerProfile::CashLong,
            initial_cash: m("1000.00"),
        },
        timeframe_ns: 1000,
        participation_bps: 10000,
        fee_bps: 0,
        minimum_fee: m("0.00"),
        slippage_bps: 0,
        signal_source: None,
    }
}
fn bar(n: u64, open: &str, close: &str, volume: &str) -> ClosedBar {
    let instrument = config().account.instrument.declaration().instrument.clone();
    let typed = |value: Decimal, unit| {
        TypedValue::new(
            TypeDescriptor::new(
                1,
                ValueShape::Scalar,
                ScalarType::Decimal {
                    scale: value.scale(),
                    unit,
                },
            )
            .unwrap(),
            ScalarValue::Decimal(value),
        )
        .unwrap()
    };
    let pt = |p| typed(p, NumericUnit::Price(instrument.clone()));
    let open = m(open);
    let close = m(close);
    ClosedBar::new(BarInput {
        sequence: n,
        timestamp_ns: n * 1000,
        instrument: instrument.clone(),
        timeframe_ns: 1000,
        open_ns: (n - 1) * 1000,
        close_ns: n * 1000,
        closed: true,
        open: pt(open),
        high: pt(if open.coefficient() > close.coefficient() {
            open
        } else {
            close
        }),
        low: pt(if open.coefficient() < close.coefficient() {
            open
        } else {
            close
        }),
        close: pt(close),
        volume: typed(q(volume), NumericUnit::Quantity(instrument)),
    })
    .unwrap()
}
fn next(broker: &mut SimulationBroker, n: u64, open: &str, close: &str, volume: &str) {
    broker
        .apply(SimulationCommand::Bar(Box::new(bar(
            n, open, close, volume,
        ))))
        .unwrap();
}
fn buy(id: &str, quantity: &str, cap: &str) -> OrderIntent {
    OrderIntent {
        id: id.into(),
        side: TradeSide::Buy,
        kind: OrderKind::Market,
        quantity: q(quantity),
        cash_cap: Some(m(cap)),
        origin: IntentOrigin::Manual,
    }
}
fn sell(id: &str, quantity: &str) -> OrderIntent {
    OrderIntent {
        id: id.into(),
        side: TradeSide::Sell,
        kind: OrderKind::Market,
        quantity: q(quantity),
        cash_cap: None,
        origin: IntentOrigin::Manual,
    }
}
fn reject(broker: &mut SimulationBroker, command: SimulationCommand) {
    let receipts = broker.receipts().to_vec();
    let journal = broker.ledger().journal().to_vec();
    let orders = broker.orders().to_vec();
    let fills = broker.fills().to_vec();
    assert!(broker.apply(command).is_err());
    assert_eq!(broker.receipts(), receipts);
    assert_eq!(broker.ledger().journal(), journal);
    assert_eq!(broker.orders(), orders);
    assert_eq!(broker.fills(), fills);
}
#[test]
fn independent_fee_slippage_cash_position_and_flat_profit_match_hand_calculation() {
    let mut cfg = config();
    cfg.fee_bps = 25;
    cfg.minimum_fee = m("0.10");
    cfg.slippage_bps = 100;
    let mut broker = SimulationBroker::new(cfg).unwrap();
    next(&mut broker, 1, "100.00", "100.00", "2.000");
    broker
        .apply(SimulationCommand::Submit(buy("buy", "2.000", "300.00")))
        .unwrap();
    assert!(broker.fills().is_empty());
    next(&mut broker, 2, "100.00", "100.00", "2.000");
    let fill = &broker.fills()[0];
    assert_eq!(
        (
            fill.price,
            fill.quantity,
            fill.settled_gross,
            fill.fee,
            fill.source_bar,
            fill.timestamp_ns
        ),
        (m("101.00"), q("2.000"), m("202.00"), m("0.51"), 2, 1000)
    );
    assert_eq!(broker.ledger().view().cash, m("797.49"));
    assert_eq!(broker.ledger().view().cost, m("202.00"));
    assert_eq!(broker.ledger().view().reserved_cash, m("0.00"));
    broker
        .apply(SimulationCommand::Submit(sell("sell", "2.000")))
        .unwrap();
    next(&mut broker, 3, "110.00", "110.00", "2.000");
    let fill = &broker.fills()[1];
    assert_eq!((fill.price, fill.fee), (m("108.90"), m("0.54")));
    let v = broker.ledger().view();
    assert_eq!(
        (v.cash, v.quantity, v.cost, v.realized_gross, v.fees),
        (m("1014.75"), q("0.000"), m("0.00"), m("15.80"), m("1.05"))
    );
    assert_eq!(
        broker.ledger().value_at(m("110.00")).unwrap().total_profit,
        m("14.75")
    );
}
#[test]
fn future_close_and_volume_cannot_change_next_open_fill_and_same_bar_is_rejected() {
    let mut base = SimulationBroker::new(config()).unwrap();
    reject(
        &mut base,
        SimulationCommand::Submit(buy("early", "2.000", "300.00")),
    );
    next(&mut base, 1, "100.00", "100.00", "1.000");
    base.apply(SimulationCommand::Submit(buy("buy", "2.000", "300.00")))
        .unwrap();
    reject(
        &mut base,
        SimulationCommand::Bar(Box::new(bar(1, "100.00", "100.00", "1.000"))),
    );
    assert!(base.fills().is_empty());
    let mut a = base.clone();
    let mut b = base;
    next(&mut a, 2, "100.00", "50.00", "0.000");
    next(&mut b, 2, "100.00", "200.00", "99999.000");
    assert_eq!(a.fills(), b.fills());
    assert_eq!(a.ledger().view(), b.ledger().view());
    assert_eq!(a.fills()[0].quantity, q("1.000"));
    next(&mut a, 3, "100.00", "100.00", "500.000");
    next(&mut b, 3, "100.00", "100.00", "500.000");
    assert_eq!(a.fills().len(), 1);
    assert_eq!(b.fills().len(), 2);
    assert_eq!(a.order("buy").unwrap().status, OrderStatus::PartiallyFilled);
    assert_eq!(b.order("buy").unwrap().status, OrderStatus::Filled);
}
#[test]
fn previous_volume_participation_is_lot_floored_and_shared_by_acceptance_fifo() {
    let mut cfg = config();
    cfg.participation_bps = 5000;
    let mut broker = SimulationBroker::new(cfg).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "3.003");
    broker
        .apply(SimulationCommand::Submit(buy("z-first", "1.000", "20.00")))
        .unwrap();
    broker
        .apply(SimulationCommand::Submit(buy("a-second", "1.000", "20.00")))
        .unwrap();
    next(&mut broker, 2, "10.00", "10.00", "0.000");
    assert_eq!(broker.fills().len(), 2);
    assert_eq!(broker.fills()[0].order, "z-first");
    assert_eq!(broker.fills()[0].quantity, q("1.000"));
    assert_eq!(broker.fills()[1].quantity, q("0.501"));
    assert_eq!(
        broker.order("a-second").unwrap().status,
        OrderStatus::PartiallyFilled
    );
    assert_eq!(broker.ledger().view().quantity, q("1.501"));
    broker
        .apply(SimulationCommand::Cancel {
            order: "a-second".into(),
        })
        .unwrap();
    assert_eq!(broker.ledger().view().reserved_cash, m("0.00"));
    assert_eq!(
        broker.order("a-second").unwrap().status,
        OrderStatus::Cancelled
    );
}
#[test]
fn open_only_limit_waits_despite_intrabar_touch_then_improves_with_price_cap() {
    let mut cfg = config();
    cfg.slippage_bps = 200;
    let mut broker = SimulationBroker::new(cfg).unwrap();
    next(&mut broker, 1, "100.00", "100.00", "2.000");
    let mut intent = buy("limit", "1.000", "110.00");
    intent.kind = OrderKind::Limit(m("101.00"));
    broker.apply(SimulationCommand::Submit(intent)).unwrap();
    next(&mut broker, 2, "102.00", "99.00", "2.000");
    assert!(broker.fills().is_empty());
    assert_eq!(broker.order("limit").unwrap().status, OrderStatus::Accepted);
    next(&mut broker, 3, "100.00", "100.00", "2.000");
    assert_eq!(broker.fills()[0].price, m("101.00"));
    assert_eq!(broker.ledger().view().cash, m("899.00"));
    let mut sale = sell("sale", "1.000");
    sale.kind = OrderKind::Limit(m("105.00"));
    broker.apply(SimulationCommand::Submit(sale)).unwrap();
    next(&mut broker, 4, "104.00", "110.00", "2.000");
    assert_eq!(broker.fills().len(), 1);
    next(&mut broker, 5, "110.00", "110.00", "2.000");
    assert_eq!(broker.fills()[1].price, m("107.80"));
    assert_eq!(broker.ledger().view().cash, m("1006.80"));
}
#[test]
fn cancel_replace_races_are_sequential_and_rejected_replacement_preserves_original() {
    let mut broker = SimulationBroker::new(config()).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "2.000");
    broker
        .apply(SimulationCommand::Submit(buy("original", "1.000", "20.00")))
        .unwrap();
    reject(
        &mut broker,
        SimulationCommand::Replace {
            order: "original".into(),
            replacement: buy("bad", "100.000", "1001.00"),
        },
    );
    assert_eq!(
        broker.order("original").unwrap().status,
        OrderStatus::Accepted
    );
    assert!(broker.order("bad").is_none());
    broker
        .apply(SimulationCommand::Replace {
            order: "original".into(),
            replacement: buy("replacement", "2.000", "30.00"),
        })
        .unwrap();
    assert_eq!(
        broker.order("original").unwrap().status,
        OrderStatus::Replaced
    );
    broker
        .apply(SimulationCommand::Cancel {
            order: "replacement".into(),
        })
        .unwrap();
    next(&mut broker, 2, "10.00", "10.00", "2.000");
    assert!(broker.fills().is_empty());
    assert!(broker.ledger().reservations().is_empty());
    broker
        .apply(SimulationCommand::Submit(buy("filled", "1.000", "20.00")))
        .unwrap();
    next(&mut broker, 3, "10.00", "10.00", "2.000");
    reject(
        &mut broker,
        SimulationCommand::Cancel {
            order: "filled".into(),
        },
    );
    assert_eq!(broker.fills().len(), 1);
}
#[test]
fn gap_cash_cap_and_initial_funds_position_rejections_release_holds_without_fills() {
    let mut broker = SimulationBroker::new(config()).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "2.000");
    for intent in [
        buy("cap", "1.000", "9.00"),
        buy("funds", "1.000", "1001.00"),
        sell("position", "1.000"),
    ] {
        broker.apply(SimulationCommand::Submit(intent)).unwrap();
    }
    assert_eq!(
        broker.order("cap").unwrap().rejection,
        Some(Rejection::CashCap)
    );
    assert_eq!(
        broker.order("funds").unwrap().rejection,
        Some(Rejection::InsufficientCash)
    );
    assert_eq!(
        broker.order("position").unwrap().rejection,
        Some(Rejection::InsufficientPosition)
    );
    assert!(broker.ledger().reservations().is_empty());
    broker
        .apply(SimulationCommand::Submit(buy("gap", "2.000", "25.00")))
        .unwrap();
    next(&mut broker, 2, "20.00", "20.00", "2.000");
    assert_eq!(
        broker.order("gap").unwrap().rejection,
        Some(Rejection::CashCap)
    );
    assert!(broker.fills().is_empty());
    assert_eq!(broker.ledger().view().cash, m("1000.00"));
    assert!(broker.ledger().reservations().is_empty());
}
#[test]
fn partials_may_be_below_order_minimum_and_per_fill_fee_can_exhaust_a_cash_cap() {
    let mut cfg = config();
    cfg.minimum_fee = m("0.10");
    let mut draft = cfg.account.instrument.declaration().clone();
    draft.minimum_quantity = q("2.000");
    cfg.account.instrument = fqe_core::finance::FinancialInstrument::new(draft).unwrap();
    let mut broker = SimulationBroker::new(cfg).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "1.000");
    broker
        .apply(SimulationCommand::Submit(buy("buy", "2.000", "20.10")))
        .unwrap();
    next(&mut broker, 2, "10.00", "10.00", "1.000");
    assert_eq!(
        broker.order("buy").unwrap().status,
        OrderStatus::PartiallyFilled
    );
    assert_eq!(broker.fills()[0].quantity, q("1.000"));
    assert_eq!(broker.ledger().view().reserved_cash, m("10.00"));
    next(&mut broker, 3, "10.00", "10.00", "1.000");
    assert_eq!(broker.order("buy").unwrap().status, OrderStatus::Rejected);
    assert_eq!(broker.order("buy").unwrap().filled_quantity, q("1.000"));
    assert_eq!(broker.ledger().view().cash, m("989.90"));
    assert!(broker.ledger().reservations().is_empty());
}
#[test]
fn malformed_scale_tick_minimums_bar_gaps_and_terminal_commands_are_atomic() {
    let mut cfg = config();
    let mut draft = cfg.account.instrument.declaration().clone();
    draft.price_step = m("0.05");
    draft.quantity_step = q("0.005");
    draft.minimum_quantity = q("0.010");
    draft.minimum_notional = m("1.00");
    cfg.account.instrument = fqe_core::finance::FinancialInstrument::new(draft).unwrap();
    let mut broker = SimulationBroker::new(cfg).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "2.000");
    for intent in [
        buy("lot", "0.011", "10.00"),
        buy("minimum", "0.005", "10.00"),
        buy("notional", "0.010", "10.00"),
    ] {
        reject(&mut broker, SimulationCommand::Submit(intent.clone()));
    }
    let mut limit = buy("tick", "1.000", "20.00");
    limit.kind = OrderKind::Limit(m("10.01"));
    reject(&mut broker, SimulationCommand::Submit(limit));
    let mut wrong = bar(2, "10.00", "10.00", "2.000").input().clone();
    wrong.open_ns = 3000;
    wrong.close_ns = 4000;
    wrong.timestamp_ns = 4000;
    reject(
        &mut broker,
        SimulationCommand::Bar(Box::new(ClosedBar::new(wrong).unwrap())),
    );
    broker
        .apply(SimulationCommand::Submit(buy("pending", "1.000", "20.00")))
        .unwrap();
    broker.apply(SimulationCommand::Finish).unwrap();
    assert!(broker.finished());
    assert!(broker.ledger().reservations().is_empty());
    assert_eq!(
        broker.order("pending").unwrap().status,
        OrderStatus::Cancelled
    );
    reject(
        &mut broker,
        SimulationCommand::Bar(Box::new(bar(2, "10.00", "10.00", "2.000"))),
    );
    reject(&mut broker, SimulationCommand::Finish);
}
#[test]
fn complete_command_replay_at_every_split_includes_partial_cancel_replace_costs_and_finish() {
    let cfg = config();
    let mut broker = SimulationBroker::new(cfg.clone()).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "1.000");
    broker
        .apply(SimulationCommand::Submit(buy("buy", "2.000", "30.00")))
        .unwrap();
    next(&mut broker, 2, "10.00", "10.00", "1.000");
    broker
        .apply(SimulationCommand::Replace {
            order: "buy".into(),
            replacement: buy("replace", "1.000", "20.00"),
        })
        .unwrap();
    next(&mut broker, 3, "11.00", "11.00", "1.000");
    broker
        .apply(SimulationCommand::Submit(sell("sell", "2.000")))
        .unwrap();
    next(&mut broker, 4, "12.00", "12.00", "1.000");
    broker
        .apply(SimulationCommand::Cancel {
            order: "sell".into(),
        })
        .unwrap();
    broker.apply(SimulationCommand::Finish).unwrap();
    for split in 0..=broker.receipts().len() {
        let mut restored =
            SimulationBroker::replay(cfg.clone(), &broker.receipts()[..split]).unwrap();
        for r in &broker.receipts()[split..] {
            restored.apply(r.command.clone()).unwrap();
        }
        assert_eq!(restored.receipts(), broker.receipts());
        assert_eq!(restored.fills(), broker.fills());
        assert_eq!(restored.orders(), broker.orders());
        assert_eq!(restored.ledger().journal(), broker.ledger().journal());
    }
    let source = broker.receipts().to_vec();
    for kind in 0..4 {
        let mut tampered = source.clone();
        match kind {
            0 => tampered[0].opening.as_mut().unwrap().config.slippage_bps = 1,
            1 => tampered[2].account.cash = m("0.00"),
            2 => tampered[1].sequence += 1,
            _ => tampered[0].opening.as_mut().unwrap().engine_id = ContentId::from_bytes([0; 32]),
        };
        assert!(SimulationBroker::replay(cfg.clone(), &tampered).is_err());
    }
    assert_eq!(broker.receipts(), source);
}
use fqe_ir::model::ContentId;
#[path = "../examples/support/historical.rs"]
mod support;
#[test]
fn real_loaded_strategy_requires_explicit_bound_nonwarmup_intent_and_only_fills_next_bar() {
    let data = support::dataset();
    let plan = support::plan();
    let mut cfg = config();
    cfg.account.initial_cash = m("100.00");
    cfg.timeframe_ns = data.spec().duration_ns();
    cfg.minimum_fee = m("0.50");
    cfg.signal_source = Some(SignalSource {
        run: "actual_strategy".into(),
        execution_id: plan.content_id(),
    });
    let mut broker = SimulationBroker::new(cfg.clone()).unwrap();
    let mut runtime =
        fqe_runtime::events::EventRuntime::new(plan, "actual_strategy", data.content_id()).unwrap();
    let mut last = None;
    for bar in &data.bars()[..5] {
        broker
            .apply(SimulationCommand::Bar(Box::new(bar.clone())))
            .unwrap();
        last = Some(runtime.step(bar.input().clone().into()).unwrap());
    }
    let observation = last.unwrap();
    assert_eq!(observation.signals.len(), 1);
    assert!(!observation.warmup);
    let policy = LongBuyPolicy {
        quantity: q("2.000"),
        cash_cap: m("20.00"),
    };
    let intent = broker
        .intent_from_long(&policy, &observation, 0, "signal")
        .unwrap();
    assert!(broker.orders().is_empty());
    assert!(broker.fills().is_empty());
    assert!(broker.ledger().reservations().is_empty());
    for change in 0..5 {
        let mut wrong = observation.clone();
        match change {
            0 => wrong.warmup = true,
            1 => wrong.matched = false,
            2 => wrong.execution_id = ContentId::from_bytes([0; 32]),
            3 => wrong.source_sequence -= 1,
            _ => wrong.signals[0].timestamp_ns -= 1,
        };
        assert!(
            broker
                .intent_from_long(&policy, &wrong, 0, "signal")
                .is_err()
        );
    }
    assert!(
        broker
            .intent_from_long(&policy, &observation, 1, "signal")
            .is_err()
    );
    broker
        .apply(SimulationCommand::Submit(intent.clone()))
        .unwrap();
    assert!(broker.fills().is_empty());
    let mut duplicate = intent;
    duplicate.id = "another".into();
    reject(&mut broker, SimulationCommand::Submit(duplicate));
    broker
        .apply(SimulationCommand::Bar(Box::new(data.bars()[5].clone())))
        .unwrap();
    assert_eq!(broker.fills().len(), 1);
    assert_eq!(broker.fills()[0].source_bar, 6);
    assert_eq!(broker.fills()[0].price, m("3.00"));
    assert_eq!(
        broker.fills()[0].timestamp_ns,
        data.bars()[5].input().open_ns
    );
    assert_eq!(broker.ledger().view().cash, m("93.50"));
    assert_eq!(broker.ledger().view().quantity, q("2.000"));
    broker.apply(SimulationCommand::Finish).unwrap();
    let restored = SimulationBroker::replay(cfg, broker.receipts()).unwrap();
    assert_eq!(restored.ledger().journal(), broker.ledger().journal());
}
#[test]
fn late_fill_capacity_failure_rolls_back_whole_bar_and_same_bar_can_retry_after_cancel() {
    let mut cfg = config();
    cfg.limits.fills = 1;
    let mut broker = SimulationBroker::new(cfg).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "2.000");
    broker
        .apply(SimulationCommand::Submit(buy("first", "1.000", "20.00")))
        .unwrap();
    broker
        .apply(SimulationCommand::Submit(buy("second", "1.000", "20.00")))
        .unwrap();
    reject(
        &mut broker,
        SimulationCommand::Bar(Box::new(bar(2, "10.00", "10.00", "2.000"))),
    );
    assert!(broker.fills().is_empty());
    assert_eq!(broker.ledger().view().cash, m("1000.00"));
    assert_eq!(broker.ledger().view().reserved_cash, m("40.00"));
    broker
        .apply(SimulationCommand::Cancel {
            order: "second".into(),
        })
        .unwrap();
    next(&mut broker, 2, "10.00", "10.00", "2.000");
    assert_eq!(broker.fills().len(), 1);
    assert_eq!(broker.ledger().view().cash, m("990.00"));
    assert_eq!(broker.order("first").unwrap().status, OrderStatus::Filled);
}
#[test]
fn configured_limits_keep_a_finish_slot_and_order_capacity_failures_preserve_state() {
    let mut cfg = config();
    cfg.limits.commands = 3;
    let mut broker = SimulationBroker::new(cfg).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "2.000");
    broker
        .apply(SimulationCommand::Submit(buy("live", "1.000", "20.00")))
        .unwrap();
    reject(
        &mut broker,
        SimulationCommand::Cancel {
            order: "live".into(),
        },
    );
    broker.apply(SimulationCommand::Finish).unwrap();
    assert!(broker.finished());
    assert_eq!(broker.receipts().len(), 3);
    assert!(broker.ledger().reservations().is_empty());
    let mut cfg = config();
    cfg.limits.orders = 1;
    let mut broker = SimulationBroker::new(cfg).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "2.000");
    broker
        .apply(SimulationCommand::Submit(buy("one", "1.000", "20.00")))
        .unwrap();
    reject(
        &mut broker,
        SimulationCommand::Submit(buy("two", "1.000", "20.00")),
    );
    let mut cfg = config();
    cfg.limits.commands = 1;
    assert!(SimulationBroker::new(cfg).is_err());
    let mut cfg = config();
    cfg.account.profile = LedgerProfile::ProceedsRestrictedShort;
    assert!(SimulationBroker::new(cfg).is_err());
}
#[test]
fn adverse_slippage_rounds_to_actual_price_tick_and_out_of_range_gap_is_an_order_rejection() {
    let mut cfg = config();
    cfg.slippage_bps = 1;
    let mut draft = cfg.account.instrument.declaration().clone();
    draft.price_step = m("0.05");
    cfg.account.instrument = fqe_core::finance::FinancialInstrument::new(draft).unwrap();
    let mut broker = SimulationBroker::new(cfg).unwrap();
    next(&mut broker, 1, "10.00", "10.00", "1.000");
    broker
        .apply(SimulationCommand::Submit(buy("buy", "1.000", "20.00")))
        .unwrap();
    next(&mut broker, 2, "10.00", "10.00", "1.000");
    assert_eq!(broker.fills()[0].price, m("10.05"));
    broker
        .apply(SimulationCommand::Submit(sell("sell", "1.000")))
        .unwrap();
    next(&mut broker, 3, "10.00", "10.00", "1.000");
    assert_eq!(broker.fills()[1].price, m("9.95"));
    assert_eq!(broker.ledger().view().cash, m("999.90"));
    broker
        .apply(SimulationCommand::Submit(buy("gap", "1.000", "20.00")))
        .unwrap();
    next(
        &mut broker,
        4,
        "1000000000000.00",
        "1000000000000.00",
        "1.000",
    );
    assert_eq!(
        broker.order("gap").unwrap().rejection,
        Some(Rejection::PriceOrModelRange)
    );
    assert_eq!(broker.fills().len(), 2);
    assert!(broker.ledger().reservations().is_empty());
}
