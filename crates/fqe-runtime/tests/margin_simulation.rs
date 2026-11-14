use fqe_core::{
    bars::{BarInput, ClosedBar},
    finance::parse_decimal,
    numeric::Decimal,
    types::{NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::history::HistorySpec;
use fqe_runtime::{ledger::*, margin::*, simulator::*};
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
        margin: Some(MarginRules {
            max_leverage_bps: 20000,
            maintenance_bps: 2500,
        }),
        limits: SimulationLimits::default(),
        account: AccountConfig {
            account: "margin.account".into(),
            instrument: spec.instrument().clone(),
            profile: LedgerProfile::MarginLong,
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
fn next(b: &mut SimulationBroker, n: u64, open: &str, close: &str, volume: &str) {
    b.apply(SimulationCommand::Bar(Box::new(bar(
        n, open, close, volume,
    ))))
    .unwrap();
}
fn buy(id: &str, qty: &str, cap: &str) -> OrderIntent {
    OrderIntent {
        id: id.into(),
        side: TradeSide::Buy,
        kind: OrderKind::Market,
        quantity: q(qty),
        cash_cap: Some(m(cap)),
        origin: IntentOrigin::Manual,
    }
}
fn sell(id: &str, qty: &str) -> OrderIntent {
    OrderIntent {
        id: id.into(),
        side: TradeSide::Sell,
        kind: OrderKind::Market,
        quantity: q(qty),
        cash_cap: None,
        origin: IntentOrigin::Manual,
    }
}
fn reconcile(b: &SimulationBroker) {
    let v = b.ledger().view();
    assert_eq!(
        v.cash.coefficient() + v.cost.coefficient() - v.debt.coefficient(),
        v.external_capital.coefficient() + v.realized_gross.coefficient() - v.fees.coefficient()
    );
    let mut debt = 0;
    for e in b.ledger().journal() {
        assert_eq!(
            e.money.iter().map(|p| p.delta.coefficient()).sum::<i128>(),
            0
        );
        for p in &e.money {
            if p.account == MoneyAccount::LoanDebt {
                debt -= p.delta.coefficient();
            }
        }
    }
    assert_eq!(debt, v.debt.coefficient());
}
fn reject(b: &mut SimulationBroker, command: SimulationCommand) {
    let receipts = b.receipts().to_vec();
    let journal = b.ledger().journal().to_vec();
    let status = b.margin_status();
    let orders = b.orders().to_vec();
    assert!(b.apply(command).is_err());
    assert_eq!(b.receipts(), receipts);
    assert_eq!(b.ledger().journal(), journal);
    assert_eq!(b.margin_status(), status);
    assert_eq!(b.orders(), orders);
}
#[test]
fn independent_fraction_margin_pack_matches_financing_maintenance_partial_liquidation_and_gap_debt()
{
    let mut current = "";
    let mut broker = None;
    let mut mark = m("100.00");
    let mut count = 0;
    for line in include_str!("fixtures/p3/margin.expected.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = line.split('\t').collect();
        assert_eq!(c.len(), 10);
        let case = c[0];
        let phase = c[1].parse::<u32>().unwrap();
        if case != current {
            current = case;
            let mut cfg = config();
            if case == "costs" {
                cfg.minimum_fee = m("1.00");
            }
            broker = Some(SimulationBroker::new(cfg).unwrap());
        }
        let b = broker.as_mut().unwrap();
        match phase {
            1 => {
                next(
                    b,
                    1,
                    "100.00",
                    "100.00",
                    if case == "costs" { "15.000" } else { "20.000" },
                );
                mark = m("100.00");
            }
            2 => {
                b.apply(SimulationCommand::Submit(if case == "costs" {
                    buy("buy", "15.000", "1501.00")
                } else {
                    buy("buy", "20.000", "2000.00")
                }))
                .unwrap();
            }
            3 => {
                let close = if case == "costs" { "80.00" } else { "60.00" };
                next(
                    b,
                    2,
                    "100.00",
                    close,
                    if case == "partial" { "5.000" } else { "20.000" },
                );
                mark = m(close);
            }
            4 if case == "costs" => {
                b.apply(SimulationCommand::Submit(sell("exit", "15.000")))
                    .unwrap();
            }
            4 => {
                let price = match case {
                    "full" => "55.00",
                    "gap" => "40.00",
                    _ => "75.00",
                };
                next(b, 3, price, price, "1000.000");
                mark = m(price);
            }
            5 => {
                let price = if case == "costs" { "90.00" } else { "75.00" };
                next(
                    b,
                    if case == "costs" { 3 } else { 4 },
                    price,
                    price,
                    "20.000",
                );
                mark = m(price);
            }
            _ => panic!("phase"),
        }
        let v = b.ledger().view();
        let value = b.ledger().value_at(mark).unwrap();
        let required = b
            .config()
            .margin
            .as_ref()
            .unwrap()
            .maintenance(
                value.market_value,
                b.config().account.instrument.declaration().money_step,
            )
            .unwrap();
        assert_eq!(
            (
                v.cash,
                v.debt,
                v.quantity,
                v.cost,
                v.fees,
                value.equity,
                required
            ),
            (
                m(c[2]),
                m(c[3]),
                q(c[4]),
                m(c[5]),
                m(c[6]),
                m(c[7]),
                m(c[8])
            ),
            "{case}/{phase}"
        );
        assert_eq!(format!("{:?}", b.margin_status().unwrap()), c[9]);
        reconcile(b);
        count += 1;
    }
    assert_eq!(count, 18);
}
#[test]
fn initial_pending_limits_cancel_and_failed_replace_keep_debt_and_holds_correct() {
    let mut b = SimulationBroker::new(config()).unwrap();
    next(&mut b, 1, "100.00", "100.00", "20.000");
    b.apply(SimulationCommand::Submit(buy(
        "too_big", "21.000", "2100.00",
    )))
    .unwrap();
    assert_eq!(
        b.order("too_big").unwrap().rejection,
        Some(Rejection::MarginLimit)
    );
    assert_eq!(b.ledger().view().debt, m("0.00"));
    b.apply(SimulationCommand::Submit(buy("live", "20.000", "2000.00")))
        .unwrap();
    assert_eq!(b.ledger().view().debt, m("1000.00"));
    assert_eq!(b.ledger().view().reserved_cash, m("2000.00"));
    assert_eq!(b.ledger().view().available_cash, m("0.00"));
    b.apply(SimulationCommand::Submit(buy(
        "pending_over",
        "1.000",
        "100.00",
    )))
    .unwrap();
    assert_eq!(
        b.order("pending_over").unwrap().rejection,
        Some(Rejection::MarginLimit)
    );
    reject(
        &mut b,
        SimulationCommand::Replace {
            order: "live".into(),
            replacement: buy("invalid_replace", "30.000", "3000.00"),
        },
    );
    assert_eq!(b.ledger().view().debt, m("1000.00"));
    b.apply(SimulationCommand::Cancel {
        order: "live".into(),
    })
    .unwrap();
    let v = b.ledger().view();
    assert_eq!(
        (v.cash, v.debt, v.reserved_cash),
        (m("1000.00"), m("0.00"), m("0.00"))
    );
    reconcile(&b);
}
#[test]
fn after_fee_fill_leverage_is_rechecked_and_rejection_reverses_financing_without_charging_fee() {
    let mut cfg = config();
    cfg.minimum_fee = m("0.50");
    let mut b = SimulationBroker::new(cfg).unwrap();
    next(&mut b, 1, "100.00", "100.00", "20.000");
    b.apply(SimulationCommand::Submit(buy("buy", "19.995", "2000.00")))
        .unwrap();
    assert_eq!(b.order("buy").unwrap().status, OrderStatus::Accepted);
    next(&mut b, 2, "100.00", "100.00", "20.000");
    assert!(b.fills().is_empty());
    assert_eq!(
        b.order("buy").unwrap().rejection,
        Some(Rejection::MarginLimit)
    );
    let v = b.ledger().view();
    assert_eq!(
        (v.cash, v.debt, v.quantity, v.fees),
        (m("1000.00"), m("0.00"), q("0.000"), m("0.00"))
    );
    assert!(b.ledger().reservations().is_empty());
    reconcile(&b);
}
#[test]
fn maintenance_equality_is_healthy_breach_waits_for_next_open_and_system_exit_cannot_be_cancelled()
{
    let mut cfg = config();
    cfg.margin.as_mut().unwrap().maintenance_bps = 2000;
    let mut b = SimulationBroker::new(cfg).unwrap();
    next(&mut b, 1, "100.00", "100.00", "20.000");
    b.apply(SimulationCommand::Submit(buy("buy", "20.000", "2000.00")))
        .unwrap();
    next(&mut b, 2, "100.00", "62.50", "20.000");
    assert_eq!(b.margin_status(), Some(MarginStatus::Healthy));
    assert_eq!(b.ledger().value_at(m("62.50")).unwrap().equity, m("250.00"));
    next(&mut b, 3, "62.50", "62.45", "20.000");
    assert_eq!(b.margin_status(), Some(MarginStatus::Liquidating));
    assert_eq!(b.fills().len(), 1);
    assert!(b.order("__fqe.liquidation").unwrap().liquidation);
    reject(
        &mut b,
        SimulationCommand::Cancel {
            order: "__fqe.liquidation".into(),
        },
    );
    reject(
        &mut b,
        SimulationCommand::Submit(buy("new_risk", "1.000", "100.00")),
    );
    next(&mut b, 4, "60.00", "60.00", "20.000");
    assert_eq!(b.fills()[1].source_bar, 4);
    assert_eq!(b.fills()[1].timestamp_ns, 3000);
    assert_eq!(b.margin_status(), Some(MarginStatus::Liquidated));
    assert_eq!(b.ledger().view().cash, m("200.00"));
}
#[test]
fn zero_prior_volume_stalls_liquidation_and_finish_reports_open_stopped_risk() {
    let mut b = SimulationBroker::new(config()).unwrap();
    next(&mut b, 1, "100.00", "100.00", "20.000");
    b.apply(SimulationCommand::Submit(buy("buy", "20.000", "2000.00")))
        .unwrap();
    next(&mut b, 2, "100.00", "60.00", "0.000");
    next(&mut b, 3, "55.00", "55.00", "1000.000");
    assert_eq!(b.fills().len(), 1);
    assert_eq!(b.margin_status(), Some(MarginStatus::Liquidating));
    b.apply(SimulationCommand::Finish).unwrap();
    assert_eq!(b.margin_status(), Some(MarginStatus::Stopped));
    assert_eq!(b.ledger().view().quantity, q("20.000"));
    assert_eq!(b.ledger().view().debt, m("1000.00"));
    assert!(b.ledger().reservations().is_empty());
    reconcile(&b);
}
#[test]
fn every_margin_command_split_and_tampered_rules_debt_status_replay_are_verified() {
    let cfg = config();
    let mut b = SimulationBroker::new(cfg.clone()).unwrap();
    next(&mut b, 1, "100.00", "100.00", "20.000");
    b.apply(SimulationCommand::Submit(buy("buy", "20.000", "2000.00")))
        .unwrap();
    next(&mut b, 2, "100.00", "60.00", "5.000");
    next(&mut b, 3, "75.00", "75.00", "1000.000");
    next(&mut b, 4, "75.00", "75.00", "20.000");
    b.apply(SimulationCommand::Finish).unwrap();
    for split in 0..=b.receipts().len() {
        let mut restored = SimulationBroker::replay(cfg.clone(), &b.receipts()[..split]).unwrap();
        for r in &b.receipts()[split..] {
            restored.apply(r.command.clone()).unwrap();
        }
        assert_eq!(restored.receipts(), b.receipts());
        assert_eq!(restored.ledger().journal(), b.ledger().journal());
        assert_eq!(restored.orders(), b.orders());
        assert_eq!(restored.fills(), b.fills());
    }
    for kind in 0..3 {
        let mut changed = b.receipts().to_vec();
        match kind {
            0 => {
                changed[0]
                    .opening
                    .as_mut()
                    .unwrap()
                    .config
                    .margin
                    .as_mut()
                    .unwrap()
                    .max_leverage_bps = 30000
            }
            1 => changed[1].account.debt = m("0.00"),
            _ => changed[2].margin = Some(MarginStatus::Healthy),
        };
        assert!(SimulationBroker::replay(cfg.clone(), &changed).is_err());
    }
    let restored = VirtualLedger::replay(cfg.account, b.ledger().journal()).unwrap();
    assert_eq!(restored.view(), b.ledger().view());
}
#[test]
fn margin_profile_rates_scales_and_reserved_namespace_fail_explicitly() {
    for rules in [
        MarginRules {
            max_leverage_bps: 9999,
            maintenance_bps: 2500,
        },
        MarginRules {
            max_leverage_bps: 50001,
            maintenance_bps: 1000,
        },
        MarginRules {
            max_leverage_bps: 20000,
            maintenance_bps: 5000,
        },
        MarginRules {
            max_leverage_bps: 20000,
            maintenance_bps: 0,
        },
    ] {
        let mut cfg = config();
        cfg.margin = Some(rules);
        assert!(SimulationBroker::new(cfg).is_err());
    }
    let mut cfg = config();
    cfg.margin = None;
    assert!(SimulationBroker::new(cfg).is_err());
    let mut cfg = config();
    cfg.account.profile = LedgerProfile::CashLong;
    assert!(SimulationBroker::new(cfg).is_err());
    let rules = config().margin.unwrap();
    assert!(rules.exposure_limit(m("1.00"), q("0.001")).is_err());
    assert_eq!(rules.maintenance(m("1.05"), m("0.05")).unwrap(), m("0.30"));
    assert_eq!(
        rules.exposure_limit(m("-1.00"), m("0.01")).unwrap(),
        m("0.00")
    );
    let mut b = SimulationBroker::new(config()).unwrap();
    next(&mut b, 1, "100.00", "100.00", "20.000");
    reject(
        &mut b,
        SimulationCommand::Submit(buy("__fqe.spoof", "1.000", "100.00")),
    );
}
#[test]
fn financing_is_balanced_principal_only_profile_gated_and_replay_validated() {
    let mut cfg = config().account;
    let mut book = VirtualLedger::new(cfg.clone()).unwrap();
    let tx = |id: &str, action| LedgerTransaction {
        id: id.into(),
        timestamp_ns: 1,
        action,
    };
    book.apply(tx(
        "borrow",
        LedgerAction::Borrow {
            amount: m("1000.00"),
        },
    ))
    .unwrap();
    assert_eq!(book.view().cash, m("2000.00"));
    assert_eq!(book.view().debt, m("1000.00"));
    assert_eq!(book.value_at(m("100.00")).unwrap().equity, m("1000.00"));
    let before = book.journal().to_vec();
    assert!(
        book.apply(tx(
            "overrepay",
            LedgerAction::Repay {
                amount: m("1000.01")
            }
        ))
        .is_err()
    );
    assert_eq!(book.journal(), before);
    book.apply(tx(
        "repay",
        LedgerAction::Repay {
            amount: m("1000.00"),
        },
    ))
    .unwrap();
    assert_eq!(book.view().cash, m("1000.00"));
    assert_eq!(book.view().debt, m("0.00"));
    assert_eq!(
        VirtualLedger::replay(cfg.clone(), book.journal())
            .unwrap()
            .view(),
        book.view()
    );
    cfg.profile = LedgerProfile::CashLong;
    let mut cash = VirtualLedger::new(cfg).unwrap();
    let before = cash.journal().to_vec();
    assert!(
        cash.apply(tx("borrow", LedgerAction::Borrow { amount: m("1.00") }))
            .is_err()
    );
    assert_eq!(cash.journal(), before);
}
#[test]
fn liquidation_has_a_reserved_system_order_slot_and_zero_execution_price_is_not_a_fake_close() {
    let mut cfg = config();
    cfg.limits.orders = 1;
    let mut b = SimulationBroker::new(cfg).unwrap();
    next(&mut b, 1, "100.00", "100.00", "20.000");
    b.apply(SimulationCommand::Submit(buy("buy", "20.000", "2000.00")))
        .unwrap();
    next(&mut b, 2, "100.00", "0.00", "20.000");
    assert_eq!(b.margin_status(), Some(MarginStatus::Liquidating));
    assert_eq!(b.orders().len(), 2);
    assert_eq!(b.model(), MARGIN_MODEL);
    assert_eq!(
        b.ledger().value_at(m("0.00")).unwrap().equity,
        m("-1000.00")
    );
    next(&mut b, 3, "0.00", "0.00", "20.000");
    assert_eq!(b.margin_status(), Some(MarginStatus::Insolvent));
    assert_eq!(b.ledger().view().quantity, q("20.000"));
    assert_eq!(b.ledger().view().debt, m("1000.00"));
    assert_eq!(b.fills().len(), 1);
    reconcile(&b);
}
#[test]
fn late_fill_quota_rolls_back_leveraged_bar_and_cancel_repays_before_same_bar_retry() {
    let mut cfg = config();
    cfg.limits.fills = 1;
    let mut b = SimulationBroker::new(cfg).unwrap();
    next(&mut b, 1, "100.00", "100.00", "20.000");
    b.apply(SimulationCommand::Submit(buy("first", "10.000", "1000.00")))
        .unwrap();
    b.apply(SimulationCommand::Submit(buy(
        "second", "10.000", "1000.00",
    )))
    .unwrap();
    assert_eq!(b.ledger().view().debt, m("1000.00"));
    reject(
        &mut b,
        SimulationCommand::Bar(Box::new(bar(2, "100.00", "100.00", "20.000"))),
    );
    assert!(b.fills().is_empty());
    assert_eq!(b.ledger().view().cash, m("2000.00"));
    assert_eq!(b.ledger().view().debt, m("1000.00"));
    b.apply(SimulationCommand::Cancel {
        order: "second".into(),
    })
    .unwrap();
    assert_eq!(b.ledger().view().debt, m("0.00"));
    next(&mut b, 2, "100.00", "100.00", "20.000");
    assert_eq!(b.fills().len(), 1);
    assert_eq!(b.ledger().view().quantity, q("10.000"));
    assert_eq!(b.ledger().view().debt, m("0.00"));
    reconcile(&b);
}
#[test]
fn failed_zero_price_exit_preserves_distinct_failed_or_insolvent_status_through_finish() {
    for (close, expected) in [
        ("100.00", MarginStatus::LiquidationFailed),
        ("0.00", MarginStatus::Insolvent),
    ] {
        let mut b = SimulationBroker::new(config()).unwrap();
        next(&mut b, 1, "100.00", "100.00", "20.000");
        b.apply(SimulationCommand::Submit(buy("buy", "20.000", "2000.00")))
            .unwrap();
        next(&mut b, 2, "100.00", "60.00", "20.000");
        next(&mut b, 3, "0.00", close, "20.000");
        assert_eq!(b.margin_status(), Some(expected));
        assert_eq!(b.fills().len(), 1);
        assert_eq!(b.ledger().view().quantity, q("20.000"));
        b.apply(SimulationCommand::Finish).unwrap();
        assert_eq!(b.margin_status(), Some(expected));
    }
    assert!(
        config()
            .margin
            .unwrap()
            .maintenance(m("-1.00"), m("0.01"))
            .is_err()
    );
}
