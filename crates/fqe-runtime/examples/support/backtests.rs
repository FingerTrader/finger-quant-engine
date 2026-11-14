use fqe_core::{
    InstrumentId,
    finance::parse_decimal,
    types::{NumericUnit, ScalarType, TypeDescriptor, ValueShape},
};
use fqe_ir::{
    event_plan::LoadedEventPlan,
    history::{HistoricalDataset, HistorySelection, HistorySpec},
    indicators,
    registry::RegistryBuilder,
};
use fqe_runtime::{
    backtest::*,
    ledger::{AccountConfig, LedgerProfile},
    simulator::{SimulationConfig, SimulationLimits},
};
use std::sync::Arc;
pub fn dataset() -> Arc<HistoricalDataset> {
    Arc::new(
        HistoricalDataset::from_csv(
            Arc::new(
                HistorySpec::from_bytes(include_bytes!(
                    "../../tests/fixtures/p3/backtest-history-spec.json"
                ))
                .unwrap(),
            ),
            include_bytes!("../../tests/fixtures/p3/backtest-history.csv"),
            None,
        )
        .unwrap(),
    )
}
pub fn selection() -> HistorySelection {
    let data = dataset();
    data.select(
        data.bars()[0].input().open_ns,
        data.bars().last().unwrap().input().close_ns,
        0,
    )
    .unwrap()
}
pub fn registry() -> fqe_ir::registry::Registry {
    let ty = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 2,
            unit: NumericUnit::Price(InstrumentId::new("DEMO").unwrap()),
        },
    )
    .unwrap();
    let mut r = RegistryBuilder::new();
    indicators::register(&mut r, "reference", ty).unwrap();
    r.freeze().unwrap()
}
pub fn plan() -> StrategyPlan {
    StrategyPlan::Event(Arc::new(
        LoadedEventPlan::from_bytes(include_bytes!(
            "../../tests/fixtures/p3/backtest-events.json"
        ))
        .unwrap()
        .compile(&registry(), vec![])
        .unwrap(),
    ))
}
pub fn config() -> SimulationConfig {
    SimulationConfig {
        margin: None,
        limits: SimulationLimits::default(),
        account: AccountConfig {
            account: "research.account".into(),
            instrument: dataset().spec().instrument().clone(),
            profile: LedgerProfile::CashLong,
            initial_cash: parse_decimal("100.00", 2).unwrap(),
        },
        timeframe_ns: 60_000_000_000,
        participation_bps: 10000,
        fee_bps: 0,
        minimum_fee: parse_decimal("0.50", 2).unwrap(),
        slippage_bps: 0,
        signal_source: None,
    }
}
pub fn policies() -> Vec<PolicyBinding> {
    vec![
        PolicyBinding {
            id: "entry".into(),
            source: PolicySource::LongSignal,
            action: PolicyAction::BuyIfFlat {
                quantity: parse_decimal("2.000", 3).unwrap(),
                cash_cap: parse_decimal("20.00", 2).unwrap(),
            },
        },
        PolicyBinding {
            id: "exit".into(),
            source: PolicySource::EventBoolean {
                name: "exit".into(),
            },
            action: PolicyAction::CloseLong,
        },
    ]
}
pub fn prepared() -> PreparedBacktest {
    PreparedBacktest::new(
        selection(),
        plan(),
        policies(),
        config(),
        "research.example",
        42,
        RunControl::Complete,
    )
    .unwrap()
}
