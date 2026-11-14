#[allow(dead_code)]
#[path = "backtests.rs"]
mod backtests;
use fqe_core::{
    finance::parse_decimal,
    types::{NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{
    event_plan::LoadedEventPlan,
    history::{HistoricalDataset, HistorySpec},
};
use fqe_runtime::{backtest::PolicyAction, optimization::*};
use std::sync::Arc;
pub fn value(v: &str) -> TypedValue {
    TypedValue::new(
        TypeDescriptor::new(
            1,
            ValueShape::Scalar,
            ScalarType::Decimal {
                scale: 12,
                unit: NumericUnit::Ratio,
            },
        )
        .unwrap(),
        ScalarValue::Decimal(parse_decimal(v, 12).unwrap()),
    )
    .unwrap()
}
pub fn dataset() -> Arc<HistoricalDataset> {
    Arc::new(
        HistoricalDataset::from_csv(
            Arc::new(
                HistorySpec::from_bytes(include_bytes!(
                    "../../tests/fixtures/p3/optimization-history-spec.json"
                ))
                .unwrap(),
            ),
            include_bytes!("../../tests/fixtures/p3/optimization-history.csv"),
            None,
        )
        .unwrap(),
    )
}
pub fn request() -> OptimizationRequest {
    let data = dataset();
    let mut policies = backtests::policies();
    if let PolicyAction::BuyIfFlat { cash_cap, .. } = &mut policies[0].action {
        *cash_cap = parse_decimal("50.00", 2).unwrap();
    }
    OptimizationRequest {
        artifact: SearchArtifact::Event(Arc::new(
            LoadedEventPlan::from_bytes(include_bytes!(
                "../../tests/fixtures/p3/backtest-events.json"
            ))
            .unwrap(),
        )),
        registry: Arc::new(backtests::registry()),
        training: data
            .select(
                data.bars()[0].input().open_ns,
                data.bars()[6].input().close_ns,
                0,
            )
            .unwrap(),
        validation: data
            .select(
                data.bars()[7].input().open_ns,
                data.bars()[13].input().close_ns,
                0,
            )
            .unwrap(),
        grid: vec![ParameterDimension {
            key: "threshold".into(),
            values: vec![value("0"), value("60"), value("90")],
        }],
        policies,
        simulation: backtests::config(),
        run: "research.search".into(),
        seed: 42,
        budget: SearchBudget::default(),
        control: SearchControl::Complete,
    }
}
