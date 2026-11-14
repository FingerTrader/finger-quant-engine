use fqe_core::{
    InstrumentId,
    bars::BarInput,
    numeric::Decimal,
    types::{NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{
    event_plan::LoadedEventPlan,
    indicators,
    model::ContentId,
    registry::{Registry, RegistryBuilder},
};
use fqe_runtime::events::{EventRuntime, InputEvent};
use std::sync::Arc;
pub const G002: &str = include_str!("../../../fqe-ir/tests/fixtures/g002-events.json");
pub const G007: &str = include_str!("../../../fqe-ir/tests/fixtures/g007-events.json");
pub fn instrument() -> InstrumentId {
    InstrumentId::new("DEMO").unwrap()
}
pub fn price_type() -> TypeDescriptor {
    TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 12,
            unit: NumericUnit::Price(instrument()),
        },
    )
    .unwrap()
}
pub fn price(n: i128) -> TypedValue {
    TypedValue::new(
        price_type(),
        ScalarValue::Decimal(Decimal::new(n * 1_000_000_000_000, 12).unwrap()),
    )
    .unwrap()
}
pub fn bar(sequence: u64, n: i128) -> BarInput {
    let volume = TypedValue::new(
        TypeDescriptor::new(
            1,
            ValueShape::Scalar,
            ScalarType::Decimal {
                scale: 0,
                unit: NumericUnit::Quantity(instrument()),
            },
        )
        .unwrap(),
        ScalarValue::Decimal(Decimal::new(0, 0).unwrap()),
    )
    .unwrap();
    BarInput {
        sequence,
        timestamp_ns: sequence * 1000,
        instrument: instrument(),
        timeframe_ns: 1000,
        open_ns: (sequence - 1) * 1000,
        close_ns: sequence * 1000,
        closed: true,
        open: price(n),
        high: price(n),
        low: price(n),
        close: price(n),
        volume,
    }
}
pub fn registry() -> Registry {
    let mut b = RegistryBuilder::new();
    indicators::register(&mut b, "reference", price_type()).unwrap();
    b.freeze().unwrap()
}
pub fn runtime(json: &str, run: &str) -> EventRuntime {
    let artifact = LoadedEventPlan::from_bytes(json.as_bytes()).unwrap();
    let plan = artifact.compile(&registry(), vec![]).unwrap();
    EventRuntime::new(Arc::new(plan), run, ContentId::from_bytes([7; 32])).unwrap()
}
pub fn run(json: &str, closes: &[i128]) -> Vec<fqe_runtime::events::EventObservation> {
    let mut r = runtime(json, "example");
    let observations = closes
        .iter()
        .enumerate()
        .map(|(i, n)| r.step(InputEvent::from(bar(i as u64 + 1, *n))).unwrap())
        .collect();
    r.finish().unwrap();
    observations
}
