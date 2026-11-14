use fqe_core::{
    InstrumentId,
    types::{NumericUnit, ScalarType, TypeDescriptor, ValueShape},
};
use fqe_ir::{
    event_plan::{CompiledEventPlan, LoadedEventPlan},
    history::{HistoricalDataset, HistorySpec},
    indicators,
    registry::RegistryBuilder,
};
use std::sync::Arc;
pub fn dataset() -> Arc<HistoricalDataset> {
    Arc::new(
        HistoricalDataset::from_csv(
            Arc::new(
                HistorySpec::from_bytes(include_bytes!(
                    "../../tests/fixtures/p3/signal-history-spec.json"
                ))
                .unwrap(),
            ),
            include_bytes!("../../tests/fixtures/p3/signal-history.csv"),
            None,
        )
        .unwrap(),
    )
}
pub fn plan() -> Arc<CompiledEventPlan> {
    let ty = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 2,
            unit: NumericUnit::Price(InstrumentId::new("DEMO").unwrap()),
        },
    )
    .unwrap();
    let mut builder = RegistryBuilder::new();
    indicators::register(&mut builder, "reference", ty).unwrap();
    Arc::new(
        LoadedEventPlan::from_bytes(include_bytes!(
            "../../../fqe-ir/tests/fixtures/history/cash-long-events.json"
        ))
        .unwrap()
        .compile(&builder.freeze().unwrap(), vec![])
        .unwrap(),
    )
}
