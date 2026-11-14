#[path = "support/historical.rs"]
mod support;
use fqe_core::finance::{decimal_text, parse_decimal};
use fqe_ir::history::HistoryStrategy;
use fqe_runtime::{
    events::EventRuntime,
    history::PreparedHistoricalRun,
    ledger::{AccountConfig, LedgerProfile},
    simulator::*,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = support::dataset();
    let plan = support::plan();
    let source = SignalSource {
        run: "simulated_example".into(),
        execution_id: plan.content_id(),
    };
    let selection = dataset.select(
        dataset.bars()[0].input().open_ns,
        dataset.bars().last().unwrap().input().close_ns,
        0,
    )?;
    let prepared = PreparedHistoricalRun::new(
        selection,
        &HistoryStrategy::from(plan.as_ref()),
        &source.run,
        0,
    )?;
    let config = SimulationConfig {
        limits: SimulationLimits::default(),
        account: AccountConfig {
            account: "example".into(),
            instrument: dataset.spec().instrument().clone(),
            profile: LedgerProfile::CashLong,
            initial_cash: parse_decimal("100.00", 2)?,
        },
        timeframe_ns: dataset.spec().duration_ns(),
        participation_bps: 10000,
        fee_bps: 0,
        minimum_fee: parse_decimal("0.50", 2)?,
        slippage_bps: 0,
        signal_source: Some(source.clone()),
    };
    let mut broker = SimulationBroker::new(config.clone())?;
    let mut runtime = EventRuntime::new(plan, &source.run, dataset.content_id())?;
    let mut cursor = prepared.cursor();
    let mut signals = 0;
    let policy = LongBuyPolicy {
        quantity: parse_decimal("2.000", 3)?,
        cash_cap: parse_decimal("20.00", 2)?,
    };
    while let Some(event) = cursor.next_until(u64::MAX)? {
        broker.apply(SimulationCommand::Bar(Box::new(event.bar.clone())))?;
        let observation = runtime.step(event.bar.input().clone().into())?;
        for index in 0..observation.signals.len() {
            let intent = broker.intent_from_long(
                &policy,
                &observation,
                index,
                &format!("signal.{}.{}", observation.source_sequence, index),
            )?;
            broker.apply(SimulationCommand::Submit(intent))?;
            signals += 1;
        }
    }
    runtime.finish()?;
    broker.apply(SimulationCommand::Finish)?;
    let restored = SimulationBroker::replay(config, broker.receipts())?;
    assert_eq!(restored.ledger().journal(), broker.ledger().journal());
    assert_eq!(restored.fills(), broker.fills());
    assert_eq!(signals, 1);
    assert_eq!(broker.fills().len(), 1);
    assert_eq!(broker.fills()[0].source_bar, 6);
    let v = broker.ledger().view();
    assert_eq!(v.cash, parse_decimal("93.50", 2)?);
    assert_eq!(v.quantity, parse_decimal("2.000", 3)?);
    println!(
        "model={} signals={} orders={} fills={} replay=matched",
        MODEL,
        signals,
        broker.orders().len(),
        broker.fills().len()
    );
    println!(
        "fill source_bar={} timestamp_ns={} price={}",
        broker.fills()[0].source_bar,
        broker.fills()[0].timestamp_ns,
        decimal_text(broker.fills()[0].price)
    );
    println!(
        "cash={} quantity={} fees={} reserved_cash={}",
        decimal_text(v.cash),
        decimal_text(v.quantity),
        decimal_text(v.fees),
        decimal_text(v.reserved_cash)
    );
    println!(
        "dataset={} selection={}",
        dataset.content_id(),
        prepared.selection_id()
    );
    Ok(())
}
