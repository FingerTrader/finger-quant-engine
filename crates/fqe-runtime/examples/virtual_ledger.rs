//! Public synthetic accounting comparison. Trades are explicit postings, not simulated fills.
use fqe_core::{finance::parse_decimal, types::ScalarValue};
use fqe_ir::history::{HistoricalDataset, HistorySpec};
use fqe_runtime::ledger::*;
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = Arc::new(HistorySpec::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/history/cash-long-spec.json"
    ))?);
    let config = AccountConfig {
        account: "ledger_example".into(),
        instrument: spec.instrument().clone(),
        profile: LedgerProfile::CashLong,
        initial_cash: parse_decimal("1000.00", 2)?,
    };
    let dataset = HistoricalDataset::from_csv(
        spec,
        include_bytes!("../../fqe-ir/tests/fixtures/history/cash-long.csv"),
        None,
    )?;
    let mut ledger = VirtualLedger::new(config.clone())?;
    for (id, side, bar) in [("buy", TradeSide::Buy, 1), ("sell", TradeSide::Sell, 3)] {
        let input = dataset.bars()[bar].input();
        let ScalarValue::Decimal(price) = input.open.value() else {
            unreachable!("validated price")
        };
        ledger.apply(LedgerTransaction {
            id: id.into(),
            timestamp_ns: input.open_ns,
            action: LedgerAction::Trade {
                side,
                price: *price,
                quantity: parse_decimal("2.000", 3)?,
                fee: parse_decimal("0.50", 2)?,
                release: None,
            },
        })?;
    }
    let restored = VirtualLedger::replay(config, ledger.journal())?;
    assert_eq!(restored.journal(), ledger.journal());
    let mark = parse_decimal("132.00", 2)?;
    let valuation = ledger.value_at(mark)?;
    let view = ledger.view();
    assert_eq!(view.cash, parse_decimal("1019.00", 2)?);
    assert_eq!(valuation.total_profit, parse_decimal("19.00", 2)?);
    println!(
        "profile={} journal_entries={} replay=matched",
        ledger.config().profile.version(),
        ledger.journal().len()
    );
    println!(
        "cash={} quantity={} fees={} total_profit={}",
        fqe_core::finance::decimal_text(view.cash),
        fqe_core::finance::decimal_text(view.quantity),
        fqe_core::finance::decimal_text(view.fees),
        fqe_core::finance::decimal_text(valuation.total_profit)
    );
    Ok(())
}
