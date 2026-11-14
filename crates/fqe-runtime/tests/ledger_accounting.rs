use fqe_core::{finance::parse_decimal, numeric::Decimal};
use fqe_ir::history::{HistoricalDataset, HistorySpec};
use fqe_runtime::ledger::*;
use std::sync::Arc;
fn d(s: &str, scale: u8) -> Decimal {
    parse_decimal(s, scale).unwrap()
}
fn m(s: &str) -> Decimal {
    d(s, 2)
}
fn q(s: &str) -> Decimal {
    d(s, 3)
}
fn config(cash: &str, profile: LedgerProfile) -> AccountConfig {
    let spec = HistorySpec::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/history/cash-long-spec.json"
    ))
    .unwrap();
    AccountConfig {
        account: "virtual.account".into(),
        instrument: spec.instrument().clone(),
        profile,
        initial_cash: m(cash),
    }
}
fn transaction(id: &str, action: LedgerAction) -> LedgerTransaction {
    LedgerTransaction {
        id: id.into(),
        timestamp_ns: 1,
        action,
    }
}
fn trade(side: TradeSide, price: &str, qty: &str, fee: &str) -> LedgerAction {
    LedgerAction::Trade {
        side,
        price: m(price),
        quantity: q(qty),
        fee: m(fee),
        release: None,
    }
}
fn checked_rejection(ledger: &mut VirtualLedger, tx: LedgerTransaction) {
    let before = ledger.view();
    let journal = ledger.journal().to_vec();
    let holds = ledger.reservations().clone();
    assert!(ledger.apply(tx).is_err());
    assert_eq!(ledger.view(), before);
    assert_eq!(ledger.journal(), journal);
    assert_eq!(ledger.reservations(), &holds);
}
fn reconcile(ledger: &VirtualLedger) {
    let mut cash = 0;
    let mut cost = 0;
    let mut capital = 0;
    let mut profit = 0;
    let mut fees = 0;
    let mut quantity = 0;
    for entry in ledger.journal() {
        assert_eq!(
            entry
                .money
                .iter()
                .map(|p| p.delta.coefficient())
                .sum::<i128>(),
            0
        );
        for p in &entry.money {
            match p.account {
                MoneyAccount::Cash => cash += p.delta.coefficient(),
                MoneyAccount::InventoryCost => cost += p.delta.coefficient(),
                MoneyAccount::ExternalCapital => capital -= p.delta.coefficient(),
                MoneyAccount::RealizedProfit => profit -= p.delta.coefficient(),
                MoneyAccount::FeeExpense => fees += p.delta.coefficient(),
            }
        }
        if let Some(p) = &entry.quantity {
            assert_eq!(p.inventory.coefficient() + p.counterparty.coefficient(), 0);
            quantity += p.inventory.coefficient();
        }
    }
    let v = ledger.view();
    assert_eq!(
        (cash, cost, capital, profit, fees, quantity),
        (
            v.cash.coefficient(),
            v.cost.coefficient(),
            v.external_capital.coefficient(),
            v.realized_gross.coefficient(),
            v.fees.coefficient(),
            v.quantity.coefficient()
        )
    );
    assert_eq!(cash + cost, capital + profit - fees);
}
#[test]
fn independent_fraction_pack_matches_each_long_short_partial_and_rounding_transaction() {
    let mut ledger = None;
    let mut current = "";
    let mut count = 0;
    for line in include_str!("fixtures/p3/ledger.expected.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = line.split('\t').collect();
        assert_eq!(c.len(), 18);
        if c[0] != current {
            current = c[0];
            ledger = Some(
                VirtualLedger::new(config(
                    c[1],
                    if current == "short" {
                        LedgerProfile::ProceedsRestrictedShort
                    } else {
                        LedgerProfile::CashLong
                    },
                ))
                .unwrap(),
            );
        }
        let ledger = ledger.as_mut().unwrap();
        let side = if c[3] == "buy" {
            TradeSide::Buy
        } else {
            TradeSide::Sell
        };
        ledger
            .apply(transaction(
                &format!("trade{}", c[2]),
                trade(side, c[4], c[5], c[6]),
            ))
            .unwrap();
        let v = ledger.view();
        if current != "short" {
            // Legacy oracle columns are exact rational strings; compare cash independently.
            let fraction = |s: &str| {
                let (n, den) = s.split_once('/').unwrap_or((s, "1"));
                (n.parse::<i128>().unwrap(), den.parse::<i128>().unwrap())
            };
            let (n, den) = fraction(c[15]);
            assert_eq!(v.cash.coefficient() * den, n * 100);
            if v.quantity.coefficient() == 0 {
                assert_eq!(c[16], "0");
                let (n, den) = fraction(c[17]);
                assert_eq!(
                    (v.realized_gross.coefficient() - v.fees.coefficient()) * den,
                    n * 100
                );
            }
        }
        assert_eq!(
            (
                v.cash,
                v.quantity,
                v.cost,
                v.realized_gross,
                v.fees,
                v.restricted_short_proceeds,
                v.available_cash,
                v.cash_rounding_total
            ),
            (
                m(c[7]),
                q(c[8]),
                m(c[9]),
                m(c[10]),
                m(c[11]),
                m(c[12]),
                m(c[13]),
                d(c[14], 18)
            ),
            "{current}/{}",
            c[2]
        );
        reconcile(ledger);
        let restored = VirtualLedger::replay(ledger.config().clone(), ledger.journal()).unwrap();
        assert_eq!(restored.view(), v);
        assert_eq!(restored.journal(), ledger.journal());
        count += 1;
    }
    assert_eq!(count, 16);
}
#[test]
fn public_cash_long_history_comparison_has_explicit_fee_cost_difference_and_same_flat_net() {
    let spec = Arc::new(
        HistorySpec::from_bytes(include_bytes!(
            "../../fqe-ir/tests/fixtures/history/cash-long-spec.json"
        ))
        .unwrap(),
    );
    let data = HistoricalDataset::from_csv(
        spec,
        include_bytes!("../../fqe-ir/tests/fixtures/history/cash-long.csv"),
        None,
    )
    .unwrap();
    let mut ledger = VirtualLedger::new(config("1000.00", LedgerProfile::CashLong)).unwrap();
    let price = |i: usize| match data.bars()[i].input().open.value() {
        fqe_core::types::ScalarValue::Decimal(d) => *d,
        _ => panic!("decimal price"),
    };
    ledger
        .apply(transaction(
            "buy",
            LedgerAction::Trade {
                side: TradeSide::Buy,
                price: price(1),
                quantity: q("2.000"),
                fee: m("0.50"),
                release: None,
            },
        ))
        .unwrap();
    assert_eq!(ledger.view().cash, m("759.50"));
    assert_eq!(ledger.view().cost, m("240.00")); // Legacy fee-capitalized cost is240.50.
    let value = ledger.value_at(m("124.00")).unwrap();
    assert_eq!(value.equity, m("1007.50"));
    assert_eq!(value.unrealized, m("8.00"));
    assert_eq!(value.total_profit, m("7.50"));
    ledger
        .apply(transaction(
            "sell",
            LedgerAction::Trade {
                side: TradeSide::Sell,
                price: price(3),
                quantity: q("2.000"),
                fee: m("0.50"),
                release: None,
            },
        ))
        .unwrap();
    let v = ledger.view();
    assert_eq!(v.cash, m("1019.00"));
    assert_eq!(v.quantity, q("0.000"));
    assert_eq!(v.realized_gross, m("20.00"));
    assert_eq!(v.fees, m("1.00"));
    assert_eq!(
        ledger.value_at(m("130.00")).unwrap().total_profit,
        m("19.00")
    );
    reconcile(&ledger);
}
#[test]
fn reservations_partially_consume_only_named_hold_and_failures_do_not_release_funds() {
    let mut ledger = VirtualLedger::new(config("1000.00", LedgerProfile::CashLong)).unwrap();
    for (name, cash) in [("a", "500.00"), ("b", "300.00")] {
        ledger
            .apply(transaction(
                &format!("reserve{name}"),
                LedgerAction::Reserve {
                    reservation: name.into(),
                    amount: Reservation {
                        cash: m(cash),
                        quantity: q("0.000"),
                    },
                },
            ))
            .unwrap();
    }
    assert_eq!(ledger.view().available_cash, m("200.00"));
    ledger
        .apply(transaction(
            "partial",
            LedgerAction::Trade {
                side: TradeSide::Buy,
                price: m("120.00"),
                quantity: q("2.000"),
                fee: m("0.50"),
                release: Some(ReservationRelease {
                    reservation: "a".into(),
                    cash: m("240.50"),
                    quantity: q("0.000"),
                }),
            },
        ))
        .unwrap();
    assert_eq!(ledger.reservations()["a"].cash, m("259.50"));
    assert_eq!(ledger.reservations()["b"].cash, m("300.00"));
    assert_eq!(ledger.view().available_cash, m("200.00"));
    checked_rejection(
        &mut ledger,
        transaction(
            "overspend",
            trade(TradeSide::Buy, "100.00", "3.000", "0.00"),
        ),
    );
    checked_rejection(
        &mut ledger,
        transaction(
            "overrelease",
            LedgerAction::Trade {
                side: TradeSide::Buy,
                price: m("1.00"),
                quantity: q("1.000"),
                fee: m("0.00"),
                release: Some(ReservationRelease {
                    reservation: "a".into(),
                    cash: m("260.00"),
                    quantity: q("0.000"),
                }),
            },
        ),
    );
    ledger
        .apply(transaction(
            "release_a",
            LedgerAction::Release {
                reservation: "a".into(),
            },
        ))
        .unwrap();
    ledger
        .apply(transaction(
            "sell_hold",
            LedgerAction::Reserve {
                reservation: "sale".into(),
                amount: Reservation {
                    cash: m("0.00"),
                    quantity: q("2.000"),
                },
            },
        ))
        .unwrap();
    checked_rejection(
        &mut ledger,
        transaction(
            "use_other_hold",
            trade(TradeSide::Sell, "130.00", "1.000", "0.00"),
        ),
    );
    ledger
        .apply(transaction(
            "sell_partial",
            LedgerAction::Trade {
                side: TradeSide::Sell,
                price: m("130.00"),
                quantity: q("1.000"),
                fee: m("0.50"),
                release: Some(ReservationRelease {
                    reservation: "sale".into(),
                    cash: m("0.00"),
                    quantity: q("1.000"),
                }),
            },
        ))
        .unwrap();
    assert_eq!(ledger.view().quantity, q("1.000"));
    assert_eq!(ledger.reservations()["sale"].quantity, q("1.000"));
    assert_eq!(ledger.view().available_long_quantity, q("0.000"));
    checked_rejection(
        &mut ledger,
        transaction(
            "reuse_a",
            LedgerAction::Reserve {
                reservation: "a".into(),
                amount: Reservation {
                    cash: m("1.00"),
                    quantity: q("0.000"),
                },
            },
        ),
    );
    reconcile(&ledger);
}
#[test]
fn duplicate_time_units_insufficient_funds_cross_zero_and_wrong_profile_are_atomic() {
    let mut ledger = VirtualLedger::new(config("100.00", LedgerProfile::CashLong)).unwrap();
    for (name, action) in [
        ("short", trade(TradeSide::Sell, "10.00", "1.000", "0.00")),
        (
            "overdraft",
            trade(TradeSide::Buy, "100.00", "2.000", "0.00"),
        ),
        ("fee", trade(TradeSide::Buy, "10.00", "1.000", "-0.01")),
        ("zero", trade(TradeSide::Buy, "0.00", "1.000", "0.00")),
        ("zeroqty", trade(TradeSide::Buy, "10.00", "0.000", "0.00")),
        (
            "unknown",
            LedgerAction::Release {
                reservation: "missing".into(),
            },
        ),
    ] {
        checked_rejection(&mut ledger, transaction(name, action));
    }
    ledger
        .apply(transaction(
            "overdraft",
            trade(TradeSide::Buy, "10.00", "2.000", "0.00"),
        ))
        .unwrap(); // Failed ID remains retryable.
    checked_rejection(
        &mut ledger,
        transaction("overdraft", trade(TradeSide::Buy, "10.00", "1.000", "0.00")),
    );
    checked_rejection(
        &mut ledger,
        LedgerTransaction {
            id: "backward".into(),
            timestamp_ns: 0,
            action: trade(TradeSide::Sell, "11.00", "1.000", "0.00"),
        },
    );
    checked_rejection(
        &mut ledger,
        transaction("flip", trade(TradeSide::Sell, "11.00", "3.000", "0.00")),
    );
    checked_rejection(
        &mut ledger,
        transaction(
            "wrong_scale",
            LedgerAction::Trade {
                side: TradeSide::Buy,
                price: d("10.0", 1),
                quantity: q("1.000"),
                fee: m("0.00"),
                release: None,
            },
        ),
    );
    let mut cfg = config("100.00", LedgerProfile::CashLong);
    let mut draft = cfg.instrument.declaration().clone();
    draft.price_step = m("0.05");
    cfg.instrument = fqe_core::finance::FinancialInstrument::new(draft).unwrap();
    let mut stepped = VirtualLedger::new(cfg).unwrap();
    checked_rejection(
        &mut stepped,
        transaction("tick", trade(TradeSide::Buy, "10.01", "1.000", "0.00")),
    );
}
#[test]
fn proceeds_remain_restricted_and_cover_risk_is_rejected_without_fabricating_margin() {
    let mut ledger =
        VirtualLedger::new(config("1000.00", LedgerProfile::ProceedsRestrictedShort)).unwrap();
    ledger
        .apply(transaction(
            "short",
            trade(TradeSide::Sell, "100.00", "3.000", "1.00"),
        ))
        .unwrap();
    assert_eq!(ledger.view().cash, m("1299.00"));
    assert_eq!(ledger.view().restricted_short_proceeds, m("300.00"));
    assert_eq!(ledger.view().available_cash, m("999.00"));
    checked_rejection(
        &mut ledger,
        transaction(
            "withdraw_proceeds",
            LedgerAction::Funding {
                direction: FundingDirection::Withdraw,
                amount: m("1000.00"),
            },
        ),
    );
    checked_rejection(
        &mut ledger,
        transaction(
            "insolvent_cover",
            trade(TradeSide::Buy, "500.00", "3.000", "0.00"),
        ),
    );
    assert_eq!(ledger.value_at(m("500.00")).unwrap().equity, m("-201.00")); // Reporting loss is not automatic liquidation.
    ledger
        .apply(transaction(
            "cover",
            trade(TradeSide::Buy, "90.00", "3.000", "1.00"),
        ))
        .unwrap();
    assert_eq!(ledger.view().cash, m("1028.00"));
    assert_eq!(ledger.view().restricted_short_proceeds, m("0.00"));
    reconcile(&ledger);
}
#[test]
fn replay_recomputes_every_field_and_binds_opening_account_profile_instrument_engine() {
    let cfg = config("1000.00", LedgerProfile::CashLong);
    let mut ledger = VirtualLedger::new(cfg.clone()).unwrap();
    ledger
        .apply(transaction(
            "buy",
            trade(TradeSide::Buy, "120.00", "2.000", "0.50"),
        ))
        .unwrap();
    let source = ledger.journal().to_vec();
    for kind in 0..8 {
        let mut altered = source.clone();
        match kind {
            0 => altered[0].opening.as_mut().unwrap().config.account = "other".into(),
            1 => {
                altered[0].opening.as_mut().unwrap().engine_id =
                    fqe_ir::model::ContentId::from_bytes([0; 32])
            }
            2 => altered[1].sequence += 1,
            3 => altered[1].after.cash = m("759.51"),
            4 => altered[1].money[0].delta = m("-240.49"),
            5 => altered[1].quantity.as_mut().unwrap().inventory = q("3.000"),
            6 => altered[1].trade.as_mut().unwrap().cash_rounding = d("0.01", 18),
            _ => altered[1].transaction.id = "opening".into(),
        };
        assert!(
            VirtualLedger::replay(cfg.clone(), &altered).is_err(),
            "{kind}"
        );
    }
    let mut wrong = cfg.clone();
    wrong.profile = LedgerProfile::ProceedsRestrictedShort;
    assert!(VirtualLedger::replay(wrong, &source).is_err());
    assert!(VirtualLedger::replay(cfg.clone(), &[]).is_err());
    assert_eq!(ledger.journal(), source);
    let mut restored = VirtualLedger::replay(cfg, &source).unwrap();
    for l in [&mut ledger, &mut restored] {
        l.apply(transaction(
            "sell",
            trade(TradeSide::Sell, "130.00", "2.000", "0.50"),
        ))
        .unwrap();
    }
    assert_eq!(ledger.journal(), restored.journal());
}
#[test]
fn funding_and_reserved_capacity_are_bounded_and_do_not_change_realized_profit() {
    let mut ledger = VirtualLedger::new(config("1000.00", LedgerProfile::CashLong)).unwrap();
    ledger
        .apply(transaction(
            "deposit",
            LedgerAction::Funding {
                direction: FundingDirection::Deposit,
                amount: m("100.00"),
            },
        ))
        .unwrap();
    ledger
        .apply(transaction(
            "withdraw",
            LedgerAction::Funding {
                direction: FundingDirection::Withdraw,
                amount: m("50.00"),
            },
        ))
        .unwrap();
    assert_eq!(ledger.view().external_capital, m("1050.00"));
    assert_eq!(ledger.value_at(m("1.00")).unwrap().total_profit, m("0.00"));
    for i in 0..MAX_RESERVATIONS {
        ledger
            .apply(transaction(
                &format!("reserve{i}"),
                LedgerAction::Reserve {
                    reservation: format!("hold{i}"),
                    amount: Reservation {
                        cash: m("0.01"),
                        quantity: q("0.000"),
                    },
                },
            ))
            .unwrap();
    }
    checked_rejection(
        &mut ledger,
        transaction(
            "too_many",
            LedgerAction::Reserve {
                reservation: "extra".into(),
                amount: Reservation {
                    cash: m("0.01"),
                    quantity: q("0.000"),
                },
            },
        ),
    );
    let mut ledger = VirtualLedger::new(config("0.00", LedgerProfile::CashLong)).unwrap();
    for i in 1..MAX_JOURNAL_ENTRIES {
        ledger
            .apply(transaction(
                &format!("deposit{i}"),
                LedgerAction::Funding {
                    direction: FundingDirection::Deposit,
                    amount: m("0.01"),
                },
            ))
            .unwrap();
    }
    checked_rejection(
        &mut ledger,
        transaction(
            "full",
            LedgerAction::Funding {
                direction: FundingDirection::Deposit,
                amount: m("0.01"),
            },
        ),
    );
    assert_eq!(ledger.journal().len(), MAX_JOURNAL_ENTRIES);
}
#[test]
fn partial_lots_can_be_below_order_minimum_without_ignoring_tick_or_lot_steps() {
    let mut cfg = config("1.00", LedgerProfile::CashLong);
    let mut draft = cfg.instrument.declaration().clone();
    draft.minimum_quantity = q("0.010");
    cfg.instrument = fqe_core::finance::FinancialInstrument::new(draft).unwrap();
    assert!(cfg.instrument.quantity(q("0.001"), false).is_err());
    assert!(cfg.instrument.quantity_lots(q("0.001"), false).is_ok());
    let mut ledger = VirtualLedger::new(cfg).unwrap();
    ledger
        .apply(transaction(
            "partial",
            trade(TradeSide::Buy, "5.00", "0.002", "0.00"),
        ))
        .unwrap();
    assert_eq!(ledger.view().quantity, q("0.002"));
    assert_eq!(ledger.view().cost, m("0.01"));
}
#[test]
fn account_quantity_and_notional_bounds_fail_atomically_and_money_steps_are_actual_multiples() {
    let mut ledger =
        VirtualLedger::new(config("1000000000000000000.00", LedgerProfile::CashLong)).unwrap();
    checked_rejection(
        &mut ledger,
        transaction(
            "overflow_cash",
            LedgerAction::Funding {
                direction: FundingDirection::Deposit,
                amount: m("0.01"),
            },
        ),
    );
    checked_rejection(
        &mut ledger,
        transaction(
            "over_limit_flow",
            trade(TradeSide::Buy, "1000000000000.00", "1000000.000", "0.01"),
        ),
    );
    ledger
        .apply(transaction(
            "max_quantity",
            trade(TradeSide::Buy, "0.01", "1000000.000", "0.00"),
        ))
        .unwrap();
    checked_rejection(
        &mut ledger,
        transaction(
            "too_many_lots",
            trade(TradeSide::Buy, "0.01", "0.001", "0.00"),
        ),
    );
    let mut cfg = config("10.00", LedgerProfile::CashLong);
    let mut draft = cfg.instrument.declaration().clone();
    draft.money_step = m("0.05");
    cfg.instrument = fqe_core::finance::FinancialInstrument::new(draft).unwrap();
    let mut stepped = VirtualLedger::new(cfg).unwrap();
    stepped
        .apply(transaction(
            "step",
            trade(TradeSide::Buy, "1.02", "1.000", "0.05"),
        ))
        .unwrap();
    assert_eq!(stepped.view().cash, m("8.95"));
    assert_eq!(stepped.view().cost, m("1.00"));
    assert_eq!(stepped.view().cash_rounding_total, d("0.02", 18));
    checked_rejection(
        &mut stepped,
        transaction("fee_step", trade(TradeSide::Sell, "1.02", "1.000", "0.01")),
    );
    reconcile(&stepped);
}
#[test]
fn every_journal_split_continues_identically_through_funding_hold_partial_release_and_trades() {
    let cfg = config("1000.00", LedgerProfile::CashLong);
    let mut ledger = VirtualLedger::new(cfg.clone()).unwrap();
    let actions = vec![
        LedgerAction::Reserve {
            reservation: "buy_hold".into(),
            amount: Reservation {
                cash: m("500.00"),
                quantity: q("0.000"),
            },
        },
        LedgerAction::Trade {
            side: TradeSide::Buy,
            price: m("120.00"),
            quantity: q("2.000"),
            fee: m("0.50"),
            release: Some(ReservationRelease {
                reservation: "buy_hold".into(),
                cash: m("240.50"),
                quantity: q("0.000"),
            }),
        },
        LedgerAction::Release {
            reservation: "buy_hold".into(),
        },
        LedgerAction::Funding {
            direction: FundingDirection::Deposit,
            amount: m("50.00"),
        },
        LedgerAction::Reserve {
            reservation: "sale_hold".into(),
            amount: Reservation {
                cash: m("0.00"),
                quantity: q("2.000"),
            },
        },
        LedgerAction::Trade {
            side: TradeSide::Sell,
            price: m("130.00"),
            quantity: q("1.000"),
            fee: m("0.25"),
            release: Some(ReservationRelease {
                reservation: "sale_hold".into(),
                cash: m("0.00"),
                quantity: q("1.000"),
            }),
        },
        LedgerAction::Trade {
            side: TradeSide::Sell,
            price: m("125.00"),
            quantity: q("1.000"),
            fee: m("0.25"),
            release: Some(ReservationRelease {
                reservation: "sale_hold".into(),
                cash: m("0.00"),
                quantity: q("1.000"),
            }),
        },
        LedgerAction::Funding {
            direction: FundingDirection::Withdraw,
            amount: m("50.00"),
        },
    ];
    for (i, action) in actions.into_iter().enumerate() {
        ledger
            .apply(transaction(&format!("event{i}"), action))
            .unwrap();
    }
    for split in 1..=ledger.journal().len() {
        let mut restored = VirtualLedger::replay(cfg.clone(), &ledger.journal()[..split]).unwrap();
        for entry in &ledger.journal()[split..] {
            restored.apply(entry.transaction.clone()).unwrap();
        }
        assert_eq!(restored.journal(), ledger.journal());
        assert_eq!(restored.view(), ledger.view());
        reconcile(&restored);
    }
}
