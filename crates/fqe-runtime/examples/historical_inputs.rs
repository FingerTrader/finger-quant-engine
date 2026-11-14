//! Offline public synthetic history -> pinned inputs -> existing strategy evaluation.
use fqe_core::{
    InstrumentId,
    types::{NumericUnit, ScalarType, TypeDescriptor, ValueShape},
};
use fqe_ir::{
    event_plan::LoadedEventPlan,
    history::{HistoricalDataset, HistorySpec, HistoryStrategy},
    indicators,
    registry::RegistryBuilder,
};
use fqe_runtime::{events::EventRuntime, history::PreparedHistoricalRun};
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = Arc::new(HistorySpec::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/history/cash-long-spec.json"
    ))?);
    let dataset = Arc::new(HistoricalDataset::from_csv(
        spec,
        include_bytes!("../../fqe-ir/tests/fixtures/history/cash-long.csv"),
        None,
    )?);
    let ty = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 2,
            unit: NumericUnit::Price(InstrumentId::new("DEMO")?),
        },
    )?;
    let mut builder = RegistryBuilder::new();
    indicators::register(&mut builder, "reference", ty)?;
    let plan = Arc::new(
        LoadedEventPlan::from_bytes(include_bytes!(
            "../../fqe-ir/tests/fixtures/history/cash-long-events.json"
        ))?
        .compile(&builder.freeze()?, vec![])?,
    );
    let selection = dataset.select(
        dataset.bars()[1].input().open_ns,
        dataset.bars()[3].input().close_ns,
        1,
    )?;
    let prepared = PreparedHistoricalRun::new(
        selection,
        &HistoryStrategy::from(plan.as_ref()),
        "history_example",
        0,
    )?;
    let mut runtime = EventRuntime::new(plan, "history_example", dataset.content_id())?;
    let mut cursor = prepared.cursor();
    let mut warmup = 0;
    let mut signals = 0;
    for bar in dataset.bars() {
        assert!(cursor.next_until(bar.input().close_ns - 1)?.is_none());
        let event = cursor
            .next_until(bar.input().close_ns)?
            .expect("closed bar now available");
        warmup += usize::from(event.warmup);
        signals += runtime
            .step(event.bar.input().clone().into())?
            .signals
            .len();
    }
    runtime.finish()?;
    assert!(cursor.finished());
    println!(
        "dataset={} selection={}",
        dataset.content_id(),
        prepared.selection_id()
    );
    println!(
        "history rows={} warmup={} signals={} financial_execution=not-selected",
        cursor.emitted(),
        warmup,
        signals
    );
    println!("{}", std::str::from_utf8(prepared.manifest())?);
    Ok(())
}
