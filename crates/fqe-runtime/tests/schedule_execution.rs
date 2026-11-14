use fqe_core::{
    Event, InstrumentId, MarketTick, PriceTicks,
    bars::BarInput,
    numeric::Decimal,
    types::{NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{catalog, model::ContentId, schedule::ScheduleDefinition, workflow::LoadedWorkflow};
use fqe_runtime::{
    events::{ExecutionError, InputEvent},
    schedule::{MarketScheduler, ScheduledEvent},
    workflow::WorkflowRuntime,
};
use std::sync::Arc;
const CONFIG: &str = include_str!("../../fqe-ir/tests/fixtures/schedule.json");
fn scheduler(src: &str) -> MarketScheduler {
    MarketScheduler::new(
        Arc::new(ScheduleDefinition::from_bytes(src.as_bytes()).unwrap()),
        "alignment",
    )
    .unwrap()
}
fn tick(sequence: u64, time: u64, price: u64) -> InputEvent {
    Event::new(
        sequence,
        time,
        MarketTick {
            instrument: InstrumentId::new("DEMO").unwrap(),
            price: PriceTicks::new(price).unwrap(),
        },
    )
    .unwrap()
    .into()
}
fn bar(seq: u64, instrument: &str, tf: u64, open: u64, value: i128) -> InputEvent {
    let instrument = InstrumentId::new(instrument).unwrap();
    let typed = |unit, n| {
        TypedValue::new(
            TypeDescriptor::new(
                1,
                ValueShape::Scalar,
                ScalarType::Decimal { scale: 0, unit },
            )
            .unwrap(),
            ScalarValue::Decimal(Decimal::new(n, 0).unwrap()),
        )
        .unwrap()
    };
    let price = typed(NumericUnit::Price(instrument.clone()), value);
    let volume = typed(NumericUnit::Quantity(instrument.clone()), 1);
    BarInput {
        sequence: seq,
        timestamp_ns: open + tf,
        instrument,
        timeframe_ns: tf,
        open_ns: open,
        close_ns: open + tf,
        closed: true,
        open: price.clone(),
        high: price.clone(),
        low: price.clone(),
        close: price,
        volume,
    }
    .into()
}
fn seal_all(r: &mut MarketScheduler) {
    for id in ["a.fast", "a.slow", "b.fast", "demo.ticks"] {
        r.seal(id).unwrap();
    }
}
fn drain(r: &mut MarketScheduler) -> Vec<ScheduledEvent> {
    let mut out = vec![];
    while let Some(e) = r.next_ready().unwrap() {
        out.push(e);
    }
    out
}
fn input() -> Vec<(&'static str, InputEvent)> {
    vec![
        ("a.fast", bar(5, "AAA", 10, 0, 10)),
        ("a.fast", bar(6, "AAA", 10, 10, 11)),
        ("a.fast", bar(7, "AAA", 10, 20, 12)),
        ("a.slow", bar(80, "AAA", 30, 0, 99)),
        ("b.fast", bar(1, "BBB", 10, 0, 20)),
        ("b.fast", bar(2, "BBB", 10, 10, 21)),
        ("b.fast", bar(3, "BBB", 10, 20, 22)),
        ("demo.ticks", tick(100, 10, 90)),
        ("demo.ticks", tick(200, 30, 110)),
    ]
}
#[test]
fn interleaved_arrivals_equal_time_ties_and_global_source_provenance() {
    let events = input();
    let mut a = scheduler(CONFIG);
    let mut b = scheduler(CONFIG);
    for (id, e) in &events {
        a.ingest(id, e.clone()).unwrap();
    }
    for i in [7, 4, 0, 3, 1, 5, 8, 2, 6] {
        let (id, e) = &events[i];
        b.ingest(id, e.clone()).unwrap();
    }
    seal_all(&mut a);
    seal_all(&mut b);
    let a = drain(&mut a);
    let b = drain(&mut b);
    assert_eq!(a, b);
    let actual = a
        .iter()
        .map(|e| {
            format!(
                "{}\t{}\t{}\t{}",
                e.logical_sequence, e.timestamp_ns, e.stream, e.source_sequence
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let golden = include_str!("fixtures/p2/alignment.expected.tsv")
        .lines()
        .skip(2)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(actual, golden);
    for e in a {
        assert_eq!(
            e.schedule_id,
            ScheduleDefinition::from_bytes(CONFIG.as_bytes())
                .unwrap()
                .content_id()
        );
        assert_eq!(
            e.dataset_id,
            if e.stream == "demo.ticks" {
                ContentId::from_bytes([34; 32])
            } else {
                ContentId::from_bytes([17; 32])
            }
        );
        match e.event {
            InputEvent::Tick(t) => assert_eq!(t.sequence(), e.logical_sequence),
            InputEvent::Bar(b) => assert_eq!(b.sequence, e.logical_sequence),
        }
    }
}
#[test]
fn inclusive_watermarks_stall_until_all_streams_are_available_and_block_late_input() {
    let mut r = scheduler(CONFIG);
    r.ingest("a.fast", bar(1, "AAA", 10, 0, 10)).unwrap();
    assert!(r.next_ready().unwrap().is_none());
    for id in ["a.fast", "b.fast", "demo.ticks"] {
        r.advance_watermark(id, 10).unwrap();
    }
    r.advance_watermark("a.slow", 9).unwrap();
    assert_eq!(r.watermark_ns(), Some(9));
    assert!(r.next_ready().unwrap().is_none());
    r.advance_watermark("a.slow", 10).unwrap();
    assert_eq!(r.next_ready().unwrap().unwrap().timestamp_ns, 10);
    let state = r.state().clone();
    let cursors = r.cursors().clone();
    assert_eq!(
        r.ingest("b.fast", bar(1, "BBB", 10, 0, 10))
            .unwrap_err()
            .code,
        "schedule_late"
    );
    assert_eq!(r.state(), &state);
    assert_eq!(r.cursors(), &cursors);
    assert!(r.advance_watermark("a.slow", 9).is_err());
    assert_eq!(r.watermark_ns(), Some(10));
    assert!(r.ingest("unknown", tick(1, 20, 1)).is_err());
    r.seal("a.slow").unwrap();
    assert!(r.seal("a.slow").is_err());
    assert!(r.advance_watermark("a.slow", 30).is_err());
    assert!(r.ingest("a.slow", bar(1, "AAA", 30, 0, 99)).is_err());
}
#[test]
fn as_of_view_hides_queued_future_and_unpublished_same_time_higher_frame() {
    let mut r = scheduler(CONFIG);
    for (id, e) in input() {
        r.ingest(id, e).unwrap();
    }
    seal_all(&mut r);
    let mut seen = vec![];
    while r
        .dispatch(|event, view| {
            for value in view.streams().values() {
                assert!(value.timestamp_ns <= event.timestamp_ns);
                assert!(value.logical_sequence <= event.logical_sequence);
            }
            if event.timestamp_ns < 30 || event.stream == "a.fast" && event.timestamp_ns == 30 {
                assert!(view.latest("a.slow").is_none());
            }
            if event.timestamp_ns == 30 && event.stream == "b.fast" {
                assert_eq!(view.latest("a.slow").unwrap().timestamp_ns, 30);
                assert_eq!(view.age_ns("a.slow"), Some(0));
            }
            if event.stream == "a.fast" && event.timestamp_ns == 20 {
                assert_eq!(view.latest("b.fast").unwrap().timestamp_ns, 10);
                assert_eq!(view.age_ns("b.fast"), Some(10));
            }
            seen.push(event.logical_sequence);
            Ok(())
        })
        .unwrap()
        .is_some()
    {}
    assert_eq!(seen.len(), 9);
    assert!(r.is_finished());
}
#[test]
fn malformed_unclosed_off_grid_duplicate_gap_and_order_are_atomic() {
    let mut r = scheduler(CONFIG);
    r.ingest("a.fast", bar(1, "AAA", 10, 0, 10)).unwrap();
    let state = r.state().clone();
    let cursors = r.cursors().clone();
    let mut unclosed = bar(2, "AAA", 10, 10, 11);
    if let InputEvent::Bar(b) = &mut unclosed {
        b.closed = false;
    }
    let mut wrong_scale = bar(2, "AAA", 10, 10, 11);
    if let InputEvent::Bar(b) = &mut wrong_scale {
        b.volume = TypedValue::new(
            TypeDescriptor::new(
                1,
                ValueShape::Scalar,
                ScalarType::Decimal {
                    scale: 1,
                    unit: NumericUnit::Quantity(b.instrument.clone()),
                },
            )
            .unwrap(),
            ScalarValue::Decimal(Decimal::new(1, 1).unwrap()),
        )
        .unwrap();
    }
    for event in [
        bar(1, "AAA", 10, 0, 10),
        bar(2, "AAA", 10, 20, 11),
        bar(2, "AAA", 10, 1, 11),
        bar(2, "AAA", 30, 0, 11),
        bar(2, "BBB", 10, 10, 11),
        unclosed,
        wrong_scale,
        tick(2, 20, 11),
    ] {
        assert!(r.ingest("a.fast", event).is_err());
        assert_eq!(r.state(), &state);
        assert_eq!(r.cursors(), &cursors);
    }
    r.ingest("a.fast", bar(2, "AAA", 10, 10, 11)).unwrap();
    assert_eq!(r.state().accepted_events, 2);
    r.ingest("demo.ticks", tick(3, 10, 90)).unwrap();
    assert!(r.ingest("demo.ticks", tick(4, 9, 90)).is_err());
    assert!(r.ingest("demo.ticks", tick(2, 11, 90)).is_err());
    r.ingest("demo.ticks", tick(4, 10, 90)).unwrap();
}
#[test]
fn gap_report_is_explicit_never_fills_missing_bars_or_higher_timeframe() {
    let mut r = scheduler(include_str!("fixtures/p2/schedule-gaps.json"));
    r.ingest("a.fast", bar(1, "AAA", 10, 0, 10)).unwrap();
    r.ingest("a.fast", bar(3, "AAA", 10, 30, 13)).unwrap();
    r.seal("a.fast").unwrap();
    let events = drain(&mut r);
    assert_eq!(events.len(), 2);
    assert!(events[0].gap.is_none());
    let gap = events[1].gap.as_ref().unwrap();
    assert_eq!((gap.previous_close_ns, gap.next_open_ns), (10, 30));
    assert_eq!(events[1].timestamp_ns, 40);
}
#[test]
fn backpressure_has_no_cursor_advance_and_retry_after_publication_is_exact() {
    let mut r = scheduler(include_str!("fixtures/p2/schedule-tick-bounded.json"));
    r.ingest("demo.ticks", tick(1, 10, 90)).unwrap();
    let cursors = r.cursors().clone();
    assert_eq!(
        r.ingest("demo.ticks", tick(2, 20, 110)).unwrap_err().code,
        "schedule_backpressure"
    );
    assert_eq!(r.cursors(), &cursors);
    assert_eq!(r.state().accepted_events, 1);
    r.advance_watermark("demo.ticks", 10).unwrap();
    r.next_ready().unwrap().unwrap();
    r.ingest("demo.ticks", tick(2, 20, 110)).unwrap();
    r.seal("demo.ticks").unwrap();
    let next = r.next_ready().unwrap().unwrap();
    assert_eq!((next.source_sequence, next.logical_sequence), (2, 2));
    assert!(r.is_finished());
}
#[test]
fn failed_dispatch_does_not_publish_alignment_or_consume_then_transactional_workflow_runs() {
    let mut r = scheduler(CONFIG);
    for (id, e) in input() {
        r.ingest(id, e).unwrap();
    }
    seal_all(&mut r);
    let state = r.state().clone();
    let alignment = r.alignment().clone();
    assert_eq!(
        r.dispatch::<()>(|_, _| Err(ExecutionError {
            code: "consumer_rejected",
            path: "test".into(),
            detail: "injected".into()
        }))
        .unwrap_err()
        .code,
        "consumer_rejected"
    );
    assert_eq!(r.state(), &state);
    assert_eq!(r.alignment(), &alignment);
    let loaded = LoadedWorkflow::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/workflow-counter.json"
    ))
    .unwrap();
    let registry = catalog::reference(loaded.event_plan().trigger()).unwrap();
    let p = Arc::new(loaded.compile(&registry, vec![]).unwrap());
    let mut workflow =
        WorkflowRuntime::new(p, "alignment", ContentId::from_bytes([34; 32])).unwrap();
    let mut signals = vec![];
    while r
        .dispatch(|e, _| {
            if e.stream == "demo.ticks" {
                let o = workflow.step(e.event.clone())?;
                signals.extend(o.signals);
            }
            Ok(())
        })
        .unwrap()
        .is_some()
    {}
    assert_eq!(workflow.state().processed_events, 2);
    assert_eq!(workflow.state().last_sequence, Some(9));
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].record.source_sequence, 9);
    assert_eq!(r.state().published_events, 9);
    workflow.finish().unwrap();
}
#[test]
fn tick_before_bar_tie_and_full_width_boundary_are_exact() {
    let config = CONFIG.replace("\"instrument\": \"DEMO\"", "\"instrument\": \"AAA\"");
    let mut r = scheduler(&config);
    r.ingest("a.fast", bar(1, "AAA", 10, 0, 10)).unwrap();
    let e = Event::new(
        99,
        10,
        MarketTick {
            instrument: InstrumentId::new("AAA").unwrap(),
            price: PriceTicks::new(90).unwrap(),
        },
    )
    .unwrap();
    r.ingest("demo.ticks", e.into()).unwrap();
    seal_all(&mut r);
    assert_eq!(r.next_ready().unwrap().unwrap().stream, "demo.ticks");
    assert_eq!(r.next_ready().unwrap().unwrap().stream, "a.fast");
    let mut r = scheduler(include_str!("fixtures/p2/schedule-tick-bounded.json"));
    r.ingest("demo.ticks", tick(u64::MAX, u64::MAX, u64::MAX))
        .unwrap();
    r.advance_watermark("demo.ticks", u64::MAX).unwrap();
    let event = r.next_ready().unwrap().unwrap();
    assert_eq!(event.source_sequence, u64::MAX);
    assert_eq!(event.timestamp_ns, u64::MAX);
    assert_eq!(event.logical_sequence, 1);
    assert!(
        MarketScheduler::new(
            Arc::new(ScheduleDefinition::from_bytes(CONFIG.as_bytes()).unwrap()),
            "bad id"
        )
        .is_err()
    );
}
