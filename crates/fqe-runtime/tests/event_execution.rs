#[path = "../examples/support/events.rs"]
mod support;
use fqe_core::{
    Event, MarketTick, PriceTicks, SignalKind,
    numeric::{Decimal, NumericError},
    types::{
        Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
    },
};
use fqe_ir::{
    event_plan::LoadedEventPlan,
    model::{ContentId, EntryKey},
    parameters::{BoundParameters, ParameterInput, ParameterSchema},
    registry::{
        CallablePort, Descriptor, DescriptorDraft, IncrementalProvider, IncrementalState,
        NamedValues, ProviderCall, ProviderFailure, Purity, ReadinessPolicy, RegistryBuilder,
    },
};
use fqe_runtime::{
    RunStatus,
    events::{EventRuntime, InputEvent},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use support::*;
const TICK: &str = include_str!("../../fqe-ir/tests/fixtures/tick-events.json");
const COUNTER: &str = include_str!("../../fqe-ir/tests/fixtures/counter-events.json");
fn tick(sequence: u64, timestamp: u64, n: u64) -> InputEvent {
    InputEvent::Tick(
        Event::new(
            sequence,
            timestamp,
            MarketTick {
                instrument: instrument(),
                price: PriceTicks::new(n).unwrap(),
            },
        )
        .unwrap(),
    )
}
fn utype() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).unwrap()
}
fn uint(n: u64) -> TypedValue {
    TypedValue::new(utype(), ScalarValue::U64(n)).unwrap()
}
fn number(o: &fqe_runtime::events::EventObservation, key: &str) -> Option<i128> {
    match &o.outputs[key] {
        Evaluation::Ready(v) => Some(v.numeric_coefficient().unwrap()),
        Evaluation::Warmup => None,
        v => panic!("unexpected {v:?}"),
    }
}
fn flag(o: &fqe_runtime::events::EventObservation, key: &str) -> bool {
    matches!(o.outputs.get(key),Some(Evaluation::Ready(v)) if v.value()==&ScalarValue::Bool(true))
}
#[test]
fn independent_g002_g007_signals_and_provenance() {
    let g = run(G002, &[1, 2, 3, 2, 4, 3]);
    assert_eq!(
        g.iter()
            .flat_map(|o| &o.signals)
            .map(|s| s.source_sequence)
            .collect::<Vec<_>>(),
        vec![5]
    );
    for (i, o) in g.iter().enumerate() {
        assert_eq!(o.source_sequence, i as u64 + 1);
        assert_eq!(o.timestamp_ns, (i as u64 + 1) * 1000);
        assert_eq!(o.warmup, i < 3);
        for s in &o.signals {
            assert_eq!(s.timestamp_ns, o.timestamp_ns);
            assert_eq!(s.signal.kind, SignalKind::Long);
            assert_eq!(s.signal.instrument, instrument());
        }
    }
    // RSI(2) after [1,2,3,2,4] is 250/3, quantized to 12 places.
    assert_eq!(number(&g[4], "rsi"), Some(83_333_333_333_333));
    let sma = run(G007, &[3, 2, 1, 2, 3, 4]);
    assert_eq!(
        sma.iter()
            .flat_map(|o| &o.signals)
            .map(|s| s.source_sequence)
            .collect::<Vec<_>>(),
        vec![5]
    );
}
#[test]
fn bound_parameter_changes_decisions_without_rebuild() {
    let artifact = LoadedEventPlan::from_bytes(G002.as_bytes()).unwrap();
    let reg = registry();
    let t = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 12,
            unit: NumericUnit::Ratio,
        },
    )
    .unwrap();
    let threshold = TypedValue::new(
        t,
        ScalarValue::Decimal(Decimal::new(90_000_000_000_000, 12).unwrap()),
    )
    .unwrap();
    let high = artifact
        .compile(
            &reg,
            vec![(
                "threshold".into(),
                ParameterInput::Value(Evaluation::Ready(threshold)),
            )],
        )
        .unwrap();
    assert_ne!(
        high.content_id(),
        artifact.compile(&reg, vec![]).unwrap().content_id()
    );
    let mut r = EventRuntime::new(Arc::new(high), "high", ContentId::from_bytes([7; 32])).unwrap();
    for (i, n) in [1, 2, 3, 2, 4, 3].iter().enumerate() {
        assert!(
            r.step(InputEvent::from(bar(i as u64 + 1, *n)))
                .unwrap()
                .signals
                .is_empty()
        );
    }
}
#[test]
fn deterministic_reruns_prefix_invariance_and_manifest() {
    let closes = [1, 2, 3, 2, 4, 3];
    let a = run(G002, &closes);
    assert_eq!(a, run(G002, &closes));
    assert_eq!(a[..4], run(G002, &closes[..4]));
    let r = runtime(G002, "manifest");
    assert_eq!(r.manifest().warmup_samples, 0);
    assert_eq!(r.manifest().dataset_id, ContentId::from_bytes([7; 32]));
    assert_eq!(
        r.manifest().interval_convention,
        "UTC ns [open, close); timestamp = close"
    );
}
#[test]
fn run_memory_is_independent_and_close_only() {
    let mut a = runtime(G002, "a");
    let mut b = runtime(G002, "b");
    for (i, n) in [1, 2, 3, 2].iter().enumerate() {
        a.step(InputEvent::from(bar(i as u64 + 1, *n))).unwrap();
    }
    assert_eq!(b.state().processed_events, 0);
    let first = b.step(InputEvent::from(bar(1, 4))).unwrap();
    assert!(first.warmup);
    assert!(first.signals.is_empty());
    assert_eq!(
        a.step(InputEvent::from(bar(5, 4))).unwrap().signals.len(),
        1
    );
    let mut future = bar(2, 4);
    future.closed = false;
    let before = b.state().clone();
    assert!(b.step(InputEvent::from(future)).is_err());
    assert_eq!(b.state(), &before);
}
#[test]
fn declared_warmup_suppresses_signals_but_advances_crossings() {
    let json = G007.replace("\"warmup_samples\": \"0\"", "\"warmup_samples\": \"5\"");
    let o = run(&json, &[3, 2, 1, 2, 3, 4]);
    assert!(flag(&o[4], "decision"));
    assert!(o[4].warmup);
    assert!(o.iter().all(|o| o.signals.is_empty()));
    assert!(!o[5].warmup);
    assert!(!flag(&o[5], "decision"));
}
#[test]
fn incremental_dependencies_are_evaluated_in_topological_order() {
    let o = run(
        include_str!("../../fqe-ir/tests/fixtures/chain-events.json"),
        &[3, 2, 1, 2, 3, 4],
    );
    assert_eq!(number(&o[2], "slow"), None);
    assert_eq!(number(&o[3], "slow"), Some(1_833_333_333_333));
}
#[test]
fn tick_order_filter_domains_and_lifecycle() {
    let mut r = runtime(TICK, "ticks");
    let first = r.step(tick(2, 0, 100)).unwrap();
    assert!(!flag(&first, "decision"));
    let other = InputEvent::Tick(
        Event::new(
            4,
            0,
            MarketTick {
                instrument: fqe_core::InstrumentId::new("OTHER").unwrap(),
                price: PriceTicks::new(200).unwrap(),
            },
        )
        .unwrap(),
    );
    assert!(!r.step(other).unwrap().matched);
    assert_eq!(r.matched_events(), 1);
    assert_eq!(r.state().processed_events, 2);
    let before = r.state().clone();
    for bad in [
        tick(4, 1, 101),
        tick(3, 1, 101),
        InputEvent::from(bar(1, 2)),
    ] {
        assert!(r.step(bad).is_err());
        assert_eq!(r.state(), &before);
    }
    let signal = r.step(tick(8, 0, 101)).unwrap();
    assert_eq!(signal.signals[0].source_sequence, 8);
    assert_eq!(r.finish().unwrap().status, RunStatus::Completed);
    assert!(r.finish().is_err());
    assert!(r.step(tick(9, 1, 101)).is_err());
    let mut empty = runtime(TICK, "empty");
    assert_eq!(empty.finish().unwrap().processed_events, 0);
    let mut backwards = runtime(TICK, "backwards");
    backwards.step(tick(1, 10, 101)).unwrap();
    assert!(backwards.step(tick(2, 9, 101)).is_err());
    backwards.step(tick(2, 10, 101)).unwrap();
}
#[test]
fn every_invalid_bar_leaves_state_untouched_and_can_retry() {
    let mut r = runtime(G007, "invalid");
    r.step(InputEvent::from(bar(1, 3))).unwrap();
    let good = bar(2, 2);
    let mut bad = Vec::new();
    macro_rules! case {
        ($field:ident,$value:expr) => {{
            let mut b = good.clone();
            b.$field = $value;
            bad.push(b);
        }};
    }
    case!(sequence, 0);
    case!(sequence, 1);
    case!(closed, false);
    case!(timeframe_ns, 0);
    case!(timestamp_ns, 1999);
    case!(close_ns, 2001);
    case!(open_ns, 1001);
    let mut gap = bar(3, 2);
    gap.sequence = 2;
    bad.push(gap);
    bad.push(bar(1, 2));
    case!(open, price(3));
    case!(high, price(1));
    case!(low, price(3));
    case!(close, uint(2));
    case!(volume, price(0));
    let mut wrong_scale = good.clone();
    wrong_scale.volume = TypedValue::new(
        TypeDescriptor::new(
            1,
            ValueShape::Scalar,
            ScalarType::Decimal {
                scale: 1,
                unit: NumericUnit::Quantity(instrument()),
            },
        )
        .unwrap(),
        ScalarValue::Decimal(Decimal::new(0, 1).unwrap()),
    )
    .unwrap();
    bad.push(wrong_scale);
    let mut negative = good.clone();
    negative.volume = TypedValue::new(
        good.volume.descriptor().clone(),
        ScalarValue::Decimal(Decimal::new(-1, 0).unwrap()),
    )
    .unwrap();
    bad.push(negative);
    case!(instrument, fqe_core::InstrumentId::new("OTHER").unwrap());
    let mut duration = good.clone();
    duration.timeframe_ns = 2000;
    duration.close_ns = 3000;
    duration.timestamp_ns = 3000;
    bad.push(duration);
    let too_high = bar(2, 1_000_000_000_001);
    bad.push(too_high);
    let before = r.state().clone();
    for b in bad {
        assert!(r.step(InputEvent::from(b.clone())).is_err(), "{b:?}");
        assert_eq!(r.state(), &before);
        assert_eq!(r.matched_events(), 1);
    }
    r.step(InputEvent::from(good)).unwrap();
    r.step(InputEvent::from(bar(3, 1))).unwrap();
    r.step(InputEvent::from(bar(4, 2))).unwrap();
    assert_eq!(
        r.step(InputEvent::from(bar(5, 3))).unwrap().signals.len(),
        1
    );
}
#[derive(Clone)]
struct Counter {
    n: u64,
    reject: bool,
    fork_panic: Arc<AtomicBool>,
    calls: Arc<AtomicU64>,
}
impl IncrementalState for Counter {
    fn fork(&self) -> Box<dyn IncrementalState> {
        assert!(
            !self.fork_panic.load(Ordering::Relaxed),
            "injected fork panic"
        );
        Box::new(self.clone())
    }
    fn step(&mut self, call: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        self.n += 1;
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.reject
            && call
                .input("close")
                .is_some_and(|v| v.value() == &ScalarValue::U64(13))
        {
            return Ok(vec![(
                "value".into(),
                Evaluation::Error(NumericError::DivisionByZero),
            )]);
        }
        if self.reject
            && call
                .input("close")
                .is_some_and(|v| v.value() == &ScalarValue::U64(17))
        {
            return Ok(vec![("value".into(), Evaluation::Missing)]);
        }
        Ok(vec![("value".into(), Evaluation::Ready(uint(self.n)))])
    }
}
struct Factory(Counter, Arc<AtomicU64>);
impl IncrementalProvider for Factory {
    fn create(&self, _: &BoundParameters) -> Result<Box<dyn IncrementalState>, ProviderFailure> {
        self.1.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(self.0.clone()))
    }
}
fn counters_with_inputs(
    json: &str,
    inputs: bool,
) -> (EventRuntime, Arc<AtomicBool>, Arc<AtomicU64>) {
    let mut b = RegistryBuilder::new();
    let panic = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicU64::new(0));
    let creates = Arc::new(AtomicU64::new(0));
    for (name, reject) in [("novel.counter", false), ("novel.reject", true)] {
        let mut d = DescriptorDraft::pure(
            EntryKey {
                id: name.into(),
                version: "1".into(),
            },
            ContentId::from_bytes([42; 32]),
            ParameterSchema::new(1, vec![]).unwrap(),
            "test counter/1",
        );
        d.purity = Purity::Stateful;
        d.readiness = ReadinessPolicy::ProviderManaged;
        d.history = 1;
        d.state_schema = Some("test.counter/1".into());
        d.required_capabilities = vec!["provider.incremental-scalar/1".into()];
        d.inputs = if inputs {
            vec![CallablePort::one("close", utype())]
        } else {
            vec![]
        };
        d.outputs = vec![CallablePort::one("value", utype())];
        b.register_incremental(
            Descriptor::new(d).unwrap(),
            Arc::new(Factory(
                Counter {
                    n: 0,
                    reject,
                    fork_panic: Arc::clone(&panic),
                    calls: Arc::clone(&calls),
                },
                Arc::clone(&creates),
            )),
        )
        .unwrap();
    }
    let r = b.freeze().unwrap();
    let loaded = LoadedEventPlan::from_bytes(json.as_bytes()).unwrap();
    let plan = Arc::new(loaded.compile(&r, vec![]).unwrap());
    assert_eq!(
        creates.load(Ordering::Relaxed),
        0,
        "event compilation must not create state"
    );
    assert!(
        EventRuntime::new(
            Arc::clone(&plan),
            "invalid run",
            ContentId::from_bytes([8; 32])
        )
        .is_err()
    );
    assert_eq!(
        creates.load(Ordering::Relaxed),
        0,
        "invalid run must fail before factories"
    );
    let runtime = EventRuntime::new(plan, "counter", ContentId::from_bytes([8; 32])).unwrap();
    assert_eq!(creates.load(Ordering::Relaxed), 2);
    (runtime, panic, calls)
}
#[test]
fn later_node_failure_rolls_back_all_nodes_crossings_and_cursor() {
    let (mut r, _, calls) = counters(COUNTER);
    r.step(tick(1, 1, 11)).unwrap();
    let before = r.state().clone();
    assert!(r.step(tick(2, 2, 13)).is_err());
    assert_eq!(r.state(), &before);
    assert_eq!(r.matched_events(), 1);
    let retry = r.step(tick(2, 2, 20)).unwrap();
    assert_eq!(number(&retry, "count_a"), Some(2));
    assert_eq!(number(&retry, "count_b"), Some(2));
    assert!(flag(&retry, "cross"));
    assert_eq!(calls.load(Ordering::Relaxed), 6);
}
#[test]
fn late_root_error_rolls_back_nodes_and_already_computed_crossing() {
    let (mut r, _, _) = counters(include_str!(
        "../../fqe-ir/tests/fixtures/counter-root-error.json"
    ));
    r.step(tick(1, 1, 11)).unwrap();
    let before = r.state().clone();
    assert!(r.step(tick(2, 2, 10)).is_err());
    assert_eq!(r.state(), &before);
    let retry = r.step(tick(2, 2, 12)).unwrap();
    assert_eq!(number(&retry, "count_a"), Some(2));
    assert!(flag(&retry, "cross"));
    assert_eq!(number(&retry, "z_failure"), Some(50));
}
#[test]
fn false_guards_reordering_and_shared_crossings_advance_state_once() {
    for json in [
        include_str!("../../fqe-ir/tests/fixtures/counter-guard-first.json"),
        include_str!("../../fqe-ir/tests/fixtures/counter-cross-first.json"),
    ] {
        let (mut r, _, calls) = counters(json);
        let first = r.step(tick(1, 1, 11)).unwrap();
        assert!(!flag(&first, "decision"));
        let second = r.step(tick(2, 2, 20)).unwrap();
        assert!(flag(&second, "decision"));
        assert!(flag(&second, "shared"));
        assert_eq!(number(&second, "count_a"), Some(2));
        assert_eq!(calls.load(Ordering::Relaxed), 4);
        assert_eq!(second.signals.len(), 1);
    }
}
#[test]
fn subscribed_failures_cannot_be_hidden_by_false_guards() {
    let (mut r, _, _) = counters(include_str!(
        "../../fqe-ir/tests/fixtures/counter-guard-first.json"
    ));
    let before = r.state().clone();
    assert!(r.step(tick(1, 1, 13)).is_err());
    assert_eq!(r.state(), &before);
    assert_eq!(number(&r.step(tick(1, 1, 11)).unwrap(), "count_a"), Some(1));
}
#[test]
fn fork_panic_is_an_error_and_does_not_poison_runtime() {
    let (mut r, panic, _) = counters(COUNTER);
    r.step(tick(1, 1, 11)).unwrap();
    panic.store(true, Ordering::Relaxed);
    assert!(r.step(tick(2, 2, 20)).is_err());
    assert_eq!(r.state().last_sequence, Some(1));
    panic.store(false, Ordering::Relaxed);
    assert_eq!(number(&r.step(tick(2, 2, 20)).unwrap(), "count_b"), Some(2));
}

fn counters(json: &str) -> (EventRuntime, Arc<AtomicBool>, Arc<AtomicU64>) {
    counters_with_inputs(json, true)
}
#[test]
fn zero_input_stateful_providers_are_generic_event_subscriptions() {
    let (mut r, _, calls) = counters_with_inputs(
        include_str!("../../fqe-ir/tests/fixtures/counter-zero-input.json"),
        false,
    );
    assert_eq!(number(&r.step(tick(1, 1, 11)).unwrap(), "count_a"), Some(1));
    assert_eq!(number(&r.step(tick(2, 2, 20)).unwrap(), "count_b"), Some(2));
    assert_eq!(calls.load(Ordering::Relaxed), 4);
}
#[test]
fn missing_output_resets_cross_history_and_suppresses_signals() {
    let (mut r, _, _) = counters(include_str!(
        "../../fqe-ir/tests/fixtures/counter-missing.json"
    ));
    assert!(!flag(&r.step(tick(1, 1, 11)).unwrap(), "cross"));
    let missing = r.step(tick(2, 2, 17)).unwrap();
    assert_eq!(missing.outputs["count_b"], Evaluation::Missing);
    assert!(missing.warmup);
    assert_eq!(missing.outputs["cross"], Evaluation::Missing);
    let next = r.step(tick(3, 3, 20)).unwrap();
    assert_eq!(number(&next, "count_b"), Some(3));
    assert!(!flag(&next, "cross"));
}
#[test]
fn skipped_pure_root_error_stays_lazy_while_event_is_accepted() {
    let mut r = runtime(
        include_str!("../../fqe-ir/tests/fixtures/tick-lazy-error.json"),
        "lazy",
    );
    let o = r.step(tick(1, 1, 10)).unwrap();
    assert!(!flag(&o, "decision"));
    assert_eq!(r.state().processed_events, 1);
}

#[test]
fn unrelated_ticks_consume_order_but_never_advance_subscriptions() {
    let (mut r, _, calls) = counters(COUNTER);
    r.step(tick(1, 1, 11)).unwrap();
    let other = Event::new(
        2,
        2,
        MarketTick {
            instrument: fqe_core::InstrumentId::new("OTHER").unwrap(),
            price: PriceTicks::new(13).unwrap(),
        },
    )
    .unwrap();
    let skipped = r.step(InputEvent::from(other)).unwrap();
    assert!(!skipped.matched);
    assert!(skipped.outputs.is_empty());
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    let next = r.step(tick(3, 3, 20)).unwrap();
    assert_eq!(number(&next, "count_a"), Some(2));
    assert!(flag(&next, "cross"));
}
#[test]
fn bar_domain_endpoints_and_logical_sequence_gaps_are_explicit() {
    let mut r = runtime(G007, "bounds");
    for (i, n) in [-1_000_000_000_000, 0, 1_000_000_000_000]
        .iter()
        .enumerate()
    {
        let mut b = bar(i as u64 + 1, *n);
        b.sequence = (i as u64 + 1) * 7;
        assert!(r.step(InputEvent::from(b)).is_ok());
    }
    assert_eq!(r.state().last_sequence, Some(21));
    assert_eq!(r.state().processed_events, 3);
}
