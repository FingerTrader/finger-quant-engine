use fqe_core::{
    Event, InstrumentId, MarketTick, PriceTicks,
    bars::BarInput,
    numeric::Decimal,
    types::{NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{
    catalog,
    schedule::ScheduleDefinition,
    temporal::LoadedTemporalRules,
    workflow::{CompiledWorkflow, LoadedWorkflow},
};
use fqe_runtime::{
    events::InputEvent,
    instances::{DispatchOutcome, InstanceLimits, InstanceScheduler, LaneStatus},
    schedule::MarketScheduler,
    temporal::{ExpiryCause, WindowNotice},
};
use std::sync::Arc;
const BAR: &str = include_str!("../../fqe-ir/tests/fixtures/workflow-bar-counter.json");
const TICK: &str = include_str!("../../fqe-ir/tests/fixtures/workflow-counter.json");
const COUNT: &str = include_str!("../../fqe-ir/tests/fixtures/temporal-count.json");
const TIME: &str = include_str!("../../fqe-ir/tests/fixtures/temporal-time.json");
const SCHEDULE: &str = include_str!("../../fqe-ir/tests/fixtures/schedule-temporal.json");
const TICK_SCHEDULE: &str =
    include_str!("../../fqe-ir/tests/fixtures/schedule-temporal-ticks.json");
fn plan(source: &str) -> Arc<CompiledWorkflow> {
    let loaded = LoadedWorkflow::from_bytes(source.as_bytes()).unwrap();
    Arc::new(
        loaded
            .compile(
                &catalog::reference(loaded.event_plan().trigger()).unwrap(),
                vec![],
            )
            .unwrap(),
    )
}
fn host(schedule: &str, limits: InstanceLimits) -> InstanceScheduler {
    let market = MarketScheduler::new(
        Arc::new(ScheduleDefinition::from_bytes(schedule.as_bytes()).unwrap()),
        "host",
    )
    .unwrap();
    InstanceScheduler::new(market, limits).unwrap()
}
fn limits() -> InstanceLimits {
    InstanceLimits {
        instances: 4,
        pending_total: 32,
        pending_per_instance: 16,
    }
}
fn add(
    host: &mut InstanceScheduler,
    id: &str,
    stream: &str,
    p: &Arc<CompiledWorkflow>,
    rules: Option<&str>,
) {
    let rules = rules.map(|src| {
        Arc::new(
            LoadedTemporalRules::from_bytes(src.as_bytes())
                .unwrap()
                .bind(p)
                .unwrap(),
        )
    });
    host.attach(id, stream, Arc::clone(p), &format!("run.{id}"), rules)
        .unwrap();
}
fn bar(seq: u64, n: i128) -> InputEvent {
    let instrument = InstrumentId::new("DEMO").unwrap();
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
    let price = typed(NumericUnit::Price(instrument.clone()), n);
    let volume = typed(NumericUnit::Quantity(instrument.clone()), 1);
    BarInput {
        sequence: seq,
        timestamp_ns: seq * 10,
        instrument,
        timeframe_ns: 10,
        open_ns: (seq - 1) * 10,
        close_ns: seq * 10,
        closed: true,
        open: price.clone(),
        high: price.clone(),
        low: price.clone(),
        close: price,
        volume,
    }
    .into()
}
fn tick(seq: u64, time: u64, n: u64, instrument: &str) -> InputEvent {
    Event::new(
        seq,
        time,
        MarketTick {
            instrument: InstrumentId::new(instrument).unwrap(),
            price: PriceTicks::new(n).unwrap(),
        },
    )
    .unwrap()
    .into()
}
fn drain(host: &mut InstanceScheduler) -> Vec<DispatchOutcome> {
    let mut out = vec![];
    for _ in 0..1000 {
        match host.drive_one().unwrap() {
            Some(o) => out.push(o),
            None => return out,
        }
    }
    panic!("unbounded dispatch")
}
fn notices(out: &[DispatchOutcome]) -> Vec<WindowNotice> {
    out.iter()
        .flat_map(|o| match o {
            DispatchOutcome::Workflow { windows, .. } => windows.clone(),
            DispatchOutcome::Timer { notice, .. } => vec![notice.clone()],
            _ => vec![],
        })
        .collect()
}
fn count(host: &InstanceScheduler, id: &str) -> u64 {
    match host.runtime(id).unwrap().instance_values()["fast"].state["count"].value() {
        ScalarValue::U64(n) => *n,
        _ => panic!(),
    }
}
#[test]
fn count_window_includes_expiry_excludes_arm_latches_early_and_ignores_expiry_recross() {
    for (prices, satisfied) in [
        (vec![90, 110, 120, 80, 80], true),
        (vec![90, 110, 80, 80, 130], true),
        (vec![90, 110, 80, 80, 80, 130], false),
    ] {
        let mut h = host(SCHEDULE, limits());
        let p = plan(BAR);
        add(&mut h, "a", "demo.bars", &p, Some(COUNT));
        for (i, n) in prices.iter().enumerate() {
            h.ingest("demo.bars", bar(i as u64 + 1, *n)).unwrap();
        }
        h.ingest("other.ticks", tick(77, 25, 1, "OTHER")).unwrap();
        h.seal("demo.bars").unwrap();
        h.seal("other.ticks").unwrap();
        let out = drain(&mut h);
        let done = notices(&out);
        assert_eq!(done.len(), 1);
        assert_eq!(
            (done[0].opened_ns, done[0].expired_ns, done[0].satisfied),
            (20, 50, satisfied)
        );
        assert_eq!(
            done[0].cause,
            ExpiryCause::ClosedBar {
                logical_sequence: 6
            }
        );
        assert_eq!(count(&h, "a"), prices.len() as u64);
        let closure = h.finish("a").unwrap();
        assert_eq!(
            closure.incomplete_windows.len(),
            if prices.len() == 6 { 1 } else { 0 },
            "expiry bar cannot immediately re-arm"
        );
    }
}
#[test]
fn event_time_timer_fires_after_all_same_deadline_markets_and_without_an_expiry_bar() {
    for prices in [vec![90, 110, 80, 80, 130], vec![90, 110]] {
        let mut h = host(SCHEDULE, limits());
        add(&mut h, "a", "demo.bars", &plan(BAR), Some(TIME));
        for (i, n) in prices.iter().enumerate() {
            h.ingest("demo.bars", bar(i as u64 + 1, *n)).unwrap();
        }
        h.seal("demo.bars").unwrap();
        h.seal("other.ticks").unwrap();
        let out = drain(&mut h);
        let done = notices(&out);
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].expired_ns, 50);
        assert_eq!(done[0].satisfied, prices.len() == 5);
        assert_eq!(done[0].cause, ExpiryCause::Watermark);
        if prices.len() == 5 {
            let market=out.iter().position(|o|matches!(o,DispatchOutcome::Workflow{observation,..} if observation.timestamp_ns==50)).unwrap();
            let timer = out
                .iter()
                .position(|o| matches!(o, DispatchOutcome::Timer { .. }))
                .unwrap();
            assert!(market < timer);
        }
        h.finish("a").unwrap();
    }
}
#[test]
fn watermark_stall_same_time_timer_ties_and_incomplete_count_eof_are_explicit() {
    let mut h = host(SCHEDULE, limits());
    add(
        &mut h,
        "a",
        "demo.bars",
        &plan(BAR),
        Some(include_str!(
            "../../fqe-ir/tests/fixtures/temporal-two-time.json"
        )),
    );
    h.ingest("demo.bars", bar(1, 90)).unwrap();
    h.ingest("demo.bars", bar(2, 110)).unwrap();
    h.advance_watermark("demo.bars", 49).unwrap();
    h.advance_watermark("other.ticks", 49).unwrap();
    let first = drain(&mut h);
    assert!(notices(&first).is_empty());
    assert!(h.drive_one().unwrap().is_none());
    h.advance_watermark("demo.bars", 50).unwrap();
    assert!(h.drive_one().unwrap().is_none());
    h.advance_watermark("other.ticks", 50).unwrap();
    let done = notices(&drain(&mut h));
    assert_eq!(
        done.iter().map(|n| n.window.as_str()).collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    let mut h = host(SCHEDULE, limits());
    add(&mut h, "a", "demo.bars", &plan(BAR), Some(COUNT));
    h.ingest("demo.bars", bar(1, 90)).unwrap();
    h.ingest("demo.bars", bar(2, 110)).unwrap();
    h.seal("demo.bars").unwrap();
    h.seal("other.ticks").unwrap();
    assert!(notices(&drain(&mut h)).is_empty());
    let closure = h.finish("a").unwrap();
    assert_eq!(closure.incomplete_windows.len(), 1);
    assert_eq!(closure.incomplete_windows[0].observed_bars, 0);
}
#[test]
fn shared_model_instances_are_isolated_round_robin_and_repeatable() {
    fn run() -> Vec<DispatchOutcome> {
        let mut h = host(SCHEDULE, limits());
        let p = plan(BAR);
        for id in ["c", "a", "b"] {
            add(&mut h, id, "demo.bars", &p, Some(COUNT));
        }
        for (i, n) in [90, 110, 120, 80, 130].into_iter().enumerate() {
            h.ingest("demo.bars", bar(i as u64 + 1, n)).unwrap();
        }
        h.seal("demo.bars").unwrap();
        h.seal("other.ticks").unwrap();
        let out = drain(&mut h);
        let order = out
            .iter()
            .filter_map(|o| {
                if let DispatchOutcome::Workflow { instance, .. } = o {
                    Some(instance.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            vec![
                "a", "b", "c", "a", "b", "c", "a", "b", "c", "a", "b", "c", "a", "b", "c"
            ]
        );
        for id in ["a", "b", "c"] {
            assert_eq!(count(&h, id), 5);
            h.finish(id).unwrap();
        }
        out
    }
    assert_eq!(run(), run());
}
#[test]
fn temporal_overflow_rolls_back_workflow_and_blocks_only_failed_lane_until_cancel() {
    let mut h = host(TICK_SCHEDULE, limits());
    let p = plan(TICK);
    add(
        &mut h,
        "bad",
        "demo.ticks",
        &p,
        Some(include_str!(
            "../../fqe-ir/tests/fixtures/temporal-overflow.json"
        )),
    );
    add(&mut h, "good", "demo.ticks", &p, None);
    for (i, n) in [90, 110, 120].into_iter().enumerate() {
        h.ingest(
            "demo.ticks",
            tick(i as u64 + 1, (i as u64 + 1) * 1000, n, "DEMO"),
        )
        .unwrap();
    }
    h.seal("demo.ticks").unwrap();
    let out = drain(&mut h);
    assert!(out.iter().any(|o|matches!(o,DispatchOutcome::Rejected{instance,error} if instance=="bad"&&error.code=="temporal_overflow")));
    assert_eq!(h.status("bad"), Some(LaneStatus::Blocked));
    assert_eq!(count(&h, "bad"), 1);
    assert_eq!(h.runtime("bad").unwrap().state().last_sequence, Some(1));
    assert_eq!(count(&h, "good"), 3);
    h.retry("bad").unwrap();
    assert!(matches!(
        h.drive_one().unwrap(),
        Some(DispatchOutcome::Rejected { .. })
    ));
    assert_eq!(count(&h, "bad"), 1);
    let closure = h.cancel("bad").unwrap();
    assert_eq!(closure.discarded_deliveries, 2);
    assert_eq!(h.pending(), 0);
    assert!(h.retry("bad").is_err());
    assert!(h.cancel("bad").is_err());
    h.finish("good").unwrap();
}
#[test]
fn atomic_fanout_backpressure_cancel_and_terminal_lifecycle() {
    let mut h = host(
        TICK_SCHEDULE,
        InstanceLimits {
            instances: 2,
            pending_total: 1,
            pending_per_instance: 1,
        },
    );
    let p = plan(TICK);
    add(&mut h, "a", "demo.ticks", &p, None);
    add(&mut h, "b", "demo.ticks", &p, None);
    h.ingest("demo.ticks", tick(1, 1000, 90, "DEMO")).unwrap();
    h.seal("demo.ticks").unwrap();
    assert_eq!(h.drive_one().unwrap_err().code, "instance_backpressure");
    assert_eq!(h.market().state().published_events, 0);
    assert_eq!(h.pending(), 0);
    h.cancel("b").unwrap();
    let out = drain(&mut h);
    assert_eq!(out.len(), 2);
    assert_eq!(count(&h, "a"), 1);
    h.finish("a").unwrap();
    assert_eq!(h.status("a"), Some(LaneStatus::Completed));
    assert!(h.finish("a").is_err());
    assert!(h.cancel("a").is_err());
    let mut h = host(TICK_SCHEDULE, limits());
    add(
        &mut h,
        "a",
        "demo.ticks",
        &p,
        Some(include_str!(
            "../../fqe-ir/tests/fixtures/temporal-tick-time.json"
        )),
    );
    h.ingest("demo.ticks", tick(1, 1000, 90, "DEMO")).unwrap();
    h.ingest("demo.ticks", tick(2, 2000, 110, "DEMO")).unwrap();
    h.advance_watermark("demo.ticks", 2000).unwrap();
    drain(&mut h);
    assert_eq!(h.windows("a").unwrap().len(), 1);
    assert_eq!(h.cancel("a").unwrap().incomplete_windows.len(), 1);
    h.seal("demo.ticks").unwrap();
    assert!(notices(&drain(&mut h)).is_empty());
}
#[test]
fn attachment_domains_identity_limits_and_late_mutation_are_rejected() {
    let mut h = host(SCHEDULE, limits());
    let p = plan(TICK);
    assert!(h.attach("wrong", "demo.bars", p, "run", None).is_err());
    let bar = plan(BAR);
    add(&mut h, "a", "demo.bars", &bar, None);
    assert!(
        h.attach("a", "demo.bars", Arc::clone(&bar), "duplicate", None)
            .is_err()
    );
    assert!(
        h.attach("b", "demo.bars", Arc::clone(&bar), "run.a", None)
            .is_err()
    );
    let wrong = LoadedTemporalRules::from_bytes(TIME.as_bytes())
        .unwrap()
        .bind(&plan(TICK))
        .unwrap();
    assert!(
        h.attach(
            "b",
            "demo.bars",
            Arc::clone(&bar),
            "b",
            Some(Arc::new(wrong))
        )
        .is_err()
    );
    h.ingest("demo.bars", self::bar(1, 90)).unwrap();
    assert!(h.attach("b", "demo.bars", bar, "b", None).is_err());
    assert!(h.finish("a").is_err());
    let market = MarketScheduler::new(
        Arc::new(ScheduleDefinition::from_bytes(SCHEDULE.as_bytes()).unwrap()),
        "host",
    )
    .unwrap();
    assert!(
        InstanceScheduler::new(
            market,
            InstanceLimits {
                instances: 0,
                pending_total: 1,
                pending_per_instance: 1
            }
        )
        .is_err()
    );
}
