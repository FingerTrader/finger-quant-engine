use fqe_core::{
    InstrumentId, types::NumericUnit, types::ScalarType, types::TypeDescriptor, types::ValueShape,
};
use fqe_ir::{
    event_plan::LoadedEventPlan,
    history::{HistoricalDataset, HistorySpec, HistoryStrategy},
    indicators,
    registry::RegistryBuilder,
};
use fqe_runtime::{events::EventRuntime, history::PreparedHistoricalRun};
use std::sync::Arc;
const SPEC: &[u8] = include_bytes!("../../fqe-ir/tests/fixtures/history/cash-long-spec.json");
const CSV: &[u8] = include_bytes!("../../fqe-ir/tests/fixtures/history/cash-long.csv");
const PLAN: &[u8] = include_bytes!("../../fqe-ir/tests/fixtures/history/cash-long-events.json");
fn dataset() -> Arc<HistoricalDataset> {
    Arc::new(
        HistoricalDataset::from_csv(Arc::new(HistorySpec::from_bytes(SPEC).unwrap()), CSV, None)
            .unwrap(),
    )
}
fn plan() -> fqe_ir::event_plan::CompiledEventPlan {
    let ty = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 2,
            unit: NumericUnit::Price(InstrumentId::new("DEMO").unwrap()),
        },
    )
    .unwrap();
    let mut b = RegistryBuilder::new();
    indicators::register(&mut b, "reference", ty).unwrap();
    LoadedEventPlan::from_bytes(PLAN)
        .unwrap()
        .compile(&b.freeze().unwrap(), vec![])
        .unwrap()
}
#[test]
fn inclusive_frontier_blocks_future_data_and_rejection_does_not_advance_cursor() {
    let data = dataset();
    let bars = data.bars();
    let selection = data
        .select(bars[1].input().open_ns, bars[3].input().close_ns, 1)
        .unwrap();
    let p = plan();
    let prepared =
        PreparedHistoricalRun::new(selection, &HistoryStrategy::from(&p), "cash_run", 7).unwrap();
    let mut cursor = prepared.cursor();
    let t = bars[0].input().close_ns;
    assert!(cursor.next_until(t - 1).unwrap().is_none());
    assert_eq!(cursor.emitted(), 0);
    let first = cursor.next_until(t).unwrap().unwrap();
    assert!(first.warmup);
    assert!(!first.gap_before);
    assert_eq!(first.bar, bars[0]);
    assert!(cursor.next_until(t).unwrap().is_none());
    assert!(cursor.next_until(t - 1).is_err());
    assert_eq!(cursor.emitted(), 1);
    assert_eq!(cursor.frontier_ns(), Some(t));
    let mut runtime = EventRuntime::new(Arc::new(p), "cash_run", data.content_id()).unwrap();
    let mut observations = vec![runtime.step(first.bar.input().clone().into()).unwrap()];
    for expected in &bars[1..] {
        let event = cursor
            .next_until(expected.input().close_ns)
            .unwrap()
            .unwrap();
        assert!(!event.warmup);
        assert!(!event.gap_before);
        observations.push(runtime.step(event.bar.input().clone().into()).unwrap());
    }
    assert!(cursor.finished());
    assert!(cursor.next_until(u64::MAX).unwrap().is_none());
    assert_eq!(
        observations
            .iter()
            .map(|o| o.source_sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    runtime.finish().unwrap();
    assert_eq!(prepared.cursor().emitted(), 0);
}
#[test]
fn frozen_manifest_pins_compiled_bindings_data_units_seed_and_warmup_without_claiming_fills() {
    let data = dataset();
    let bars = data.bars();
    let p = plan();
    let strategy = HistoryStrategy::from(&p);
    let select = || {
        data.select(bars[1].input().open_ns, bars[3].input().close_ns, 1)
            .unwrap()
    };
    let a = PreparedHistoricalRun::new(select(), &strategy, "run", 7).unwrap();
    let b = PreparedHistoricalRun::new(select(), &strategy, "run", 7).unwrap();
    assert_eq!(a.manifest(), b.manifest());
    assert_ne!(
        a.manifest(),
        PreparedHistoricalRun::new(select(), &strategy, "run", 8)
            .unwrap()
            .manifest()
    );
    let text = std::str::from_utf8(a.manifest()).unwrap();
    for expected in [
        data.content_id().to_string(),
        p.content_id().to_string(),
        a.selection_id().to_string(),
        "\"warmup_bars\":\"1\"".into(),
        "\"money_step\":\"0.01\"".into(),
        "\"financial_execution\":\"not-selected\"".into(),
    ] {
        assert!(text.contains(&expected), "{expected}");
    }
    assert!(PreparedHistoricalRun::new(select(), &strategy, "invalid run", 7).is_err());
    let bad = std::str::from_utf8(PLAN)
        .unwrap()
        .replace("\"volume_scale\": 3", "\"volume_scale\": 0");
    let mut b = RegistryBuilder::new();
    let ty = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 2,
            unit: NumericUnit::Price(InstrumentId::new("DEMO").unwrap()),
        },
    )
    .unwrap();
    indicators::register(&mut b, "reference", ty).unwrap();
    let bad = LoadedEventPlan::from_bytes(bad.as_bytes())
        .unwrap()
        .compile(&b.freeze().unwrap(), vec![])
        .unwrap();
    assert!(PreparedHistoricalRun::new(select(), &HistoryStrategy::from(&bad), "run", 7).is_err());
}
#[test]
fn explicit_session_gap_can_be_inspected_but_cannot_enter_continuous_runtime() {
    let spec=std::str::from_utf8(SPEC).unwrap().replace("\"close\": \"2024-01-02T00:04:00Z\"","\"close\": \"2024-01-02T00:02:00Z\"},{\"open\":\"2024-01-02T00:03:00Z\",\"close\":\"2024-01-02T00:04:00Z\"");
    let csv = std::str::from_utf8(CSV)
        .unwrap()
        .lines()
        .enumerate()
        .filter(|(i, _)| *i != 3)
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let data = Arc::new(
        HistoricalDataset::from_csv(
            Arc::new(HistorySpec::from_bytes(spec.as_bytes()).unwrap()),
            csv.as_bytes(),
            None,
        )
        .unwrap(),
    );
    let selection = data
        .select(
            data.bars()[0].input().open_ns,
            data.bars()[2].input().close_ns,
            0,
        )
        .unwrap();
    assert!(
        PreparedHistoricalRun::new(selection.clone(), &HistoryStrategy::from(&plan()), "run", 0)
            .is_err()
    );
    let mut cursor = fqe_runtime::history::HistoricalCursor::new(selection);
    assert!(!cursor.next_until(u64::MAX).unwrap().unwrap().gap_before);
    assert!(!cursor.next_until(u64::MAX).unwrap().unwrap().gap_before);
    assert!(cursor.next_until(u64::MAX).unwrap().unwrap().gap_before);
}
