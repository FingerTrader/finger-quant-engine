use fqe_core::finance::{decimal_text, parse_decimal};
use fqe_ir::history::{HistoricalDataset, HistorySpec};
use fqe_runtime::{ledger::*, margin::*, simulator::*};
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = Arc::new(HistorySpec::from_bytes(include_bytes!(
        "../tests/fixtures/p3/margin-history-spec.json"
    ))?);
    let data = HistoricalDataset::from_csv(
        Arc::clone(&spec),
        include_bytes!("../tests/fixtures/p3/margin-history.csv"),
        None,
    )?;
    let cfg = SimulationConfig {
        margin: Some(MarginRules {
            max_leverage_bps: 20000,
            maintenance_bps: 2500,
        }),
        limits: SimulationLimits::default(),
        account: AccountConfig {
            account: "margin_example".into(),
            instrument: spec.instrument().clone(),
            profile: LedgerProfile::MarginLong,
            initial_cash: parse_decimal("1000.00", 2)?,
        },
        timeframe_ns: spec.duration_ns(),
        participation_bps: 10000,
        fee_bps: 0,
        minimum_fee: parse_decimal("0.00", 2)?,
        slippage_bps: 0,
        signal_source: None,
    };
    let mut broker = SimulationBroker::new(cfg.clone())?;
    for (index, bar) in data.bars().iter().enumerate() {
        broker.apply(SimulationCommand::Bar(Box::new(bar.clone())))?;
        if index == 0 {
            broker.apply(SimulationCommand::Submit(OrderIntent {
                id: "leveraged-buy".into(),
                side: TradeSide::Buy,
                kind: OrderKind::Market,
                quantity: parse_decimal("20.000", 3)?,
                cash_cap: Some(parse_decimal("2000.00", 2)?),
                origin: IntentOrigin::Manual,
            }))?;
        }
        if index == 1 {
            assert_eq!(broker.margin_status(), Some(MarginStatus::Liquidating));
            assert_eq!(broker.fills().len(), 1);
        }
    }
    assert_eq!(broker.margin_status(), Some(MarginStatus::Liquidated));
    broker.apply(SimulationCommand::Finish)?;
    let restored = SimulationBroker::replay(cfg, broker.receipts())?;
    assert_eq!(restored.ledger().journal(), broker.ledger().journal());
    let v = broker.ledger().view();
    assert_eq!(v.cash, parse_decimal("100.00", 2)?);
    assert_eq!(v.debt, parse_decimal("0.00", 2)?);
    println!(
        "model={} status={:?} fills={} replay=matched",
        broker.model(),
        broker.margin_status().unwrap(),
        broker.fills().len()
    );
    println!(
        "cash={} debt={} quantity={} realized_gross={}",
        decimal_text(v.cash),
        decimal_text(v.debt),
        decimal_text(v.quantity),
        decimal_text(v.realized_gross)
    );
    println!("dataset={}", data.content_id());
    Ok(())
}
