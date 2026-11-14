use fqe_core::{
    Event, InstrumentId, MarketTick, PriceTicks, SignalKind,
    types::{Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{
    catalog,
    model::{ContentId, EntryKey},
    parameters::{BoundParameters, ParameterSchema},
    registry::{
        CallablePort, Descriptor, DescriptorDraft, IncrementalProvider, IncrementalState,
        NamedValues, ProviderCall, ProviderFailure, Purity, ReadinessPolicy, RegistryBuilder,
    },
    workflow::{CompiledWorkflow, LoadedWorkflow},
};
use fqe_runtime::{
    events::InputEvent,
    workflow::{WorkflowRuntime, WorkflowStatus},
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
const COUNTER: &str = include_str!("../../fqe-ir/tests/fixtures/workflow-counter.json");
const MEMORY: &str = include_str!("../../fqe-ir/tests/fixtures/workflow-memory.json");
fn tick(seq: u64, price: u64) -> InputEvent {
    named_tick(seq, price, "DEMO")
}
fn named_tick(seq: u64, price: u64, instrument: &str) -> InputEvent {
    Event::new(
        seq,
        seq * 1000,
        MarketTick {
            instrument: InstrumentId::new(instrument).unwrap(),
            price: PriceTicks::new(price).unwrap(),
        },
    )
    .unwrap()
    .into()
}
fn plan(src: &str) -> Arc<CompiledWorkflow> {
    let loaded = LoadedWorkflow::from_bytes(src.as_bytes()).unwrap();
    Arc::new(
        loaded
            .compile(
                &catalog::reference(loaded.event_plan().trigger()).unwrap(),
                vec![],
            )
            .unwrap(),
    )
}
fn runtime(src: &str) -> WorkflowRuntime {
    WorkflowRuntime::new(plan(src), "test", ContentId::from_bytes([9; 32])).unwrap()
}
fn scalar(v: &Evaluation<TypedValue>) -> String {
    match v {
        Evaluation::Ready(v) => match v.value() {
            ScalarValue::U64(n) => n.to_string(),
            ScalarValue::Bool(v) => v.to_string(),
            v => panic!("{v:?}"),
        },
        Evaluation::Warmup => "W".into(),
        Evaluation::Missing => "M".into(),
        v => panic!("{v:?}"),
    }
}
fn count(r: &WorkflowRuntime, id: &str, cell: &str) -> u64 {
    match r.instance_values()[id].state[cell].value() {
        ScalarValue::U64(n) => *n,
        v => panic!("{v:?}"),
    }
}
#[test]
fn x002_independent_golden_exact_old_new_delay_modes_signals_and_parameters() {
    let mut r = runtime(COUNTER);
    let rows = include_str!("fixtures/p2/x002.expected.tsv")
        .lines()
        .skip(2);
    for (i, (row, price)) in rows.zip([90, 110, 120, 80, 130]).enumerate() {
        let seq = i as u64 + 1;
        let o = r.step(tick(seq, price)).unwrap();
        let fast = &o.outputs["fast"];
        let slow = &o.outputs["slow"];
        let signals = o
            .signals
            .iter()
            .map(|s| {
                format!(
                    "{}.{}:{}",
                    s.instance,
                    s.output,
                    match s.record.signal.kind {
                        SignalKind::Long => "long",
                        SignalKind::Flat => "flat",
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let actual = [
            seq.to_string(),
            scalar(&fast["count"]),
            scalar(&slow["count"]),
            scalar(&fast["lag"]),
            o.warmup.to_string(),
            r.instance_values()["fast"].mode.clone(),
            r.instance_values()["slow"].mode.clone(),
            scalar(&fast["entry"]),
            scalar(&fast["exit"]),
            scalar(&slow["entry"]),
            if signals.is_empty() {
                "-".into()
            } else {
                signals
            },
        ]
        .join("\t");
        assert_eq!(actual, row);
        assert_eq!(count(&r, "fast", "count"), seq);
        assert_eq!(count(&r, "slow", "count"), seq * 2);
        assert_eq!(
            scalar(&r.instance_values()["fast"].delays["lag"]),
            price.to_string()
        );
        assert_eq!(o.execution_id, r.manifest().execution_id);
        for s in o.signals {
            assert_eq!(s.record.source_sequence, seq);
            assert_eq!(s.record.timestamp_ns, seq * 1000);
        }
    }
    assert_eq!(r.finish().unwrap().status, WorkflowStatus::Completed);
    assert_eq!(r.state().matched_events, 5);
    assert_eq!(r.checkpoint().unwrap_err().code, "workflow_capability");
    let view = String::from_utf8(r.inspect_state().unwrap()).unwrap();
    assert!(view.contains("\"restorable\":false"));
    assert!(view.contains("independent.counter-cells"));
}
#[test]
fn repeat_prefix_fresh_instance_and_unrelated_events() {
    let p = plan(COUNTER);
    let id = ContentId::from_bytes([8; 32]);
    let mut a = WorkflowRuntime::new(Arc::clone(&p), "repeat", id).unwrap();
    let mut b = WorkflowRuntime::new(Arc::clone(&p), "repeat", id).unwrap();
    for (i, n) in [90, 110, 120].into_iter().enumerate() {
        assert_eq!(
            a.step(tick(i as u64 + 1, n)).unwrap(),
            b.step(tick(i as u64 + 1, n)).unwrap()
        );
    }
    let before = a.instance_values().clone();
    let ignored = a.step(named_tick(4, 500, "OTHER")).unwrap();
    assert!(!ignored.matched);
    assert!(ignored.outputs.is_empty());
    assert!(ignored.signals.is_empty());
    assert_eq!(a.instance_values(), &before);
    assert_eq!(a.state().processed_events, 4);
    assert_eq!(a.state().matched_events, 3);
    let o = a.step(tick(5, 80)).unwrap();
    assert_eq!(scalar(&o.outputs["fast"]["lag"]), "120");
    assert_eq!(count(&b, "fast", "count"), 3);
    let fresh = WorkflowRuntime::new(p, "fresh", id).unwrap();
    assert_eq!(count(&fresh, "fast", "count"), 0);
}
#[test]
fn conflicting_effects_modes_and_unready_writes_leave_everything_unchanged() {
    for (src, code, price) in [
        (
            include_str!("fixtures/p2/write-conflict.json"),
            "workflow_write_conflict",
            90,
        ),
        (
            include_str!("fixtures/p2/mode-conflict.json"),
            "workflow_transition_conflict",
            110,
        ),
        (
            include_str!("fixtures/p2/write-unready.json"),
            "workflow_write_unready",
            90,
        ),
    ] {
        let mut r = runtime(src);
        let before = r.inspect_state().unwrap();
        let status = r.state().clone();
        let error = r.step(tick(1, price)).unwrap_err();
        assert_eq!(error.code, code, "{error}");
        assert!(!error.path.is_empty());
        assert_eq!(r.inspect_state().unwrap(), before);
        assert_eq!(r.state(), &status);
    }
}
#[test]
fn disjoint_effects_see_old_state_and_upstream_outputs_stay_old() {
    let mut r = runtime(include_str!("fixtures/p2/old-effects.json"));
    for seq in 1..4 {
        r.step(tick(seq, 90)).unwrap();
        assert_eq!(count(&r, "fast", "count"), seq);
        assert_eq!(count(&r, "fast", "previous"), seq - 1);
    }
    let mut r = runtime(include_str!("fixtures/p2/dag.json"));
    let a = r.step(tick(1, 110)).unwrap();
    assert_eq!(scalar(&a.outputs["slow"]["count"]), "0");
    r.step(tick(2, 110)).unwrap();
    assert_eq!(scalar(&r.instance_values()["slow"].delays["lag"]), "1");
}
#[test]
fn unready_guard_skips_effect_inactive_guard_is_not_evaluated_and_delay_errors_rollback() {
    let mut r = runtime(include_str!("fixtures/p2/guard-unready.json"));
    let a = r.step(tick(1, 110)).unwrap();
    assert!(a.warmup);
    assert_eq!(count(&r, "fast", "count"), 0);
    r.step(tick(2, 110)).unwrap();
    assert_eq!(count(&r, "fast", "count"), 1);
    let mut r = runtime(include_str!("fixtures/p2/inactive-error.json"));
    r.step(tick(1, 110)).unwrap();
    assert_eq!(count(&r, "fast", "count"), 1);
    let mut r = runtime(include_str!("fixtures/p2/delay-error.json"));
    let before = r.inspect_state().unwrap();
    assert!(r.step(tick(1, 110)).is_err());
    assert_eq!(r.inspect_state().unwrap(), before);
    assert_eq!(r.state().last_sequence, None);
}
#[derive(Default)]
struct Metrics {
    creates: AtomicU64,
    live: AtomicU64,
    forks: AtomicU64,
    steps: AtomicU64,
}
struct Memory {
    n: u64,
    metrics: Arc<Metrics>,
}
impl Memory {
    fn owned(n: u64, metrics: &Arc<Metrics>) -> Self {
        metrics.live.fetch_add(1, Ordering::Relaxed);
        Self {
            n,
            metrics: Arc::clone(metrics),
        }
    }
}
impl Drop for Memory {
    fn drop(&mut self) {
        self.metrics.live.fetch_sub(1, Ordering::Relaxed);
    }
}
impl IncrementalState for Memory {
    fn fork(&self) -> Box<dyn IncrementalState> {
        self.metrics.forks.fetch_add(1, Ordering::Relaxed);
        Box::new(Self::owned(self.n, &self.metrics))
    }
    fn step(&mut self, _: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        self.metrics.steps.fetch_add(1, Ordering::Relaxed);
        self.n += 1;
        Ok(vec![(
            "value".into(),
            Evaluation::Ready(TypedValue::new(utype(), ScalarValue::U64(self.n)).unwrap()),
        )])
    }
}
struct Factory(Arc<Metrics>);
impl IncrementalProvider for Factory {
    fn create(&self, _: &BoundParameters) -> Result<Box<dyn IncrementalState>, ProviderFailure> {
        self.0.creates.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(Memory::owned(0, &self.0)))
    }
}
fn utype() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).unwrap()
}
fn memory_plan() -> (Arc<CompiledWorkflow>, Arc<Metrics>) {
    memory_plan_with_permit(false)
}
fn memory_plan_with_permit(replay: bool) -> (Arc<CompiledWorkflow>, Arc<Metrics>) {
    let metrics = Arc::new(Metrics::default());
    let mut b = RegistryBuilder::new();
    let mut d = DescriptorDraft::pure(
        EntryKey {
            id: "independent.memory".into(),
            version: "1".into(),
        },
        ContentId::from_bytes([42; 32]),
        ParameterSchema::new(1, vec![]).unwrap(),
        "independent workflow transaction test/1",
    );
    d.purity = Purity::Stateful;
    d.readiness = ReadinessPolicy::ProviderManaged;
    d.history = 1;
    d.state_schema = Some("independent.memory-state/1".into());
    d.required_capabilities = vec!["provider.incremental-scalar/1".into()];
    d.inputs = vec![CallablePort::one("sample", utype())];
    d.outputs = vec![CallablePort::one("value", utype())];
    b.register_incremental(
        Descriptor::new(d).unwrap(),
        Arc::new(Factory(Arc::clone(&metrics))),
    )
    .unwrap();
    if replay {
        b.permit_checkpoint_replay(&EntryKey {
            id: "independent.memory".into(),
            version: "1".into(),
        })
        .unwrap();
    }
    let registry = b.freeze().unwrap();
    let loaded = LoadedWorkflow::from_bytes(MEMORY.as_bytes()).unwrap();
    let p = Arc::new(loaded.compile(&registry, vec![]).unwrap());
    assert_eq!(metrics.creates.load(Ordering::Relaxed), 0);
    (p, metrics)
}
#[test]
fn late_numeric_error_rolls_back_provider_crossing_rule_delay_and_cursor_then_retry() {
    let (p, m) = memory_plan();
    let mut r = WorkflowRuntime::new(p, "rollback", ContentId::from_bytes([9; 32])).unwrap();
    let a = r.step(tick(1, 11)).unwrap();
    assert_eq!(scalar(&a.event_outputs["price"]), "1");
    assert_eq!(scalar(&a.outputs["fast"]["crossing"]), "false");
    let state = r.state().clone();
    let cells = r.inspect_state().unwrap();
    let error = r.step(tick(2, 10)).unwrap_err();
    assert!(!error.path.is_empty());
    assert_eq!(r.state(), &state);
    assert_eq!(r.inspect_state().unwrap(), cells);
    assert_eq!(m.live.load(Ordering::Relaxed), 1);
    let retry = r.step(tick(2, 12)).unwrap();
    assert_eq!(scalar(&retry.event_outputs["price"]), "2");
    assert_eq!(scalar(&retry.outputs["fast"]["crossing"]), "true");
    assert_eq!(scalar(&retry.outputs["fast"]["z_numeric"]), "50");
    assert_eq!(count(&r, "fast", "count"), 2);
    assert_eq!(
        m.steps.load(Ordering::Relaxed),
        3,
        "one provider invocation per attempted matched event"
    );
    let forks = m.forks.load(Ordering::Relaxed);
    assert!(r.step(tick(2, 12)).is_err());
    assert_eq!(
        m.forks.load(Ordering::Relaxed),
        forks,
        "reject duplicate sequence before provider fork"
    );
    assert!(r.step(tick(3, 9)).is_err(), "unsigned underflow must abort");
    assert_eq!(count(&r, "fast", "count"), 2);
    let third = r.step(tick(3, 14)).unwrap();
    assert_eq!(scalar(&third.event_outputs["price"]), "3");
}
#[test]
fn lifecycle_releases_owned_providers_validates_before_factories_and_restarts_fresh() {
    let (p, m) = memory_plan();
    let id = ContentId::from_bytes([9; 32]);
    assert!(WorkflowRuntime::new(Arc::clone(&p), "invalid run", id).is_err());
    assert_eq!(m.creates.load(Ordering::Relaxed), 0);
    for terminal in [
        WorkflowStatus::Completed,
        WorkflowStatus::Stopped,
        WorkflowStatus::Failed,
    ] {
        let mut r = WorkflowRuntime::new(Arc::clone(&p), "old", id).unwrap();
        r.step(tick(1, 11)).unwrap();
        assert!(r.restart("new").is_err());
        match terminal {
            WorkflowStatus::Completed => {
                r.finish().unwrap();
            }
            WorkflowStatus::Stopped => {
                r.stop().unwrap();
            }
            _ => {
                r.abort().unwrap();
            }
        }
        assert_eq!(m.live.load(Ordering::Relaxed), 0);
        assert_eq!(r.state().status, terminal);
        assert_eq!(count(&r, "fast", "count"), 1);
        assert!(r.step(tick(2, 12)).is_err());
        assert!(r.finish().is_err());
        assert!(r.stop().is_err());
        assert!(r.abort().is_err());
        assert!(r.restart("old").is_err());
        let mut next = r.restart("new").unwrap();
        assert_eq!(count(&next, "fast", "count"), 0);
        assert_eq!(next.state().status, WorkflowStatus::Ready);
        let first = next.step(tick(1, 11)).unwrap();
        assert_eq!(scalar(&first.event_outputs["price"]), "1");
        assert_eq!(scalar(&first.outputs["fast"]["crossing"]), "false");
        drop(next);
        assert_eq!(m.live.load(Ordering::Relaxed), 0);
    }
    assert_eq!(m.creates.load(Ordering::Relaxed), 6);
}
#[test]
fn temporal_failure_discards_candidate_provider_crossing_and_rule_state_together() {
    use fqe_ir::{schedule::ScheduleDefinition, temporal::LoadedTemporalRules};
    use fqe_runtime::{
        instances::{DispatchOutcome, InstanceLimits, InstanceScheduler},
        schedule::MarketScheduler,
    };
    let (plan, metrics) = memory_plan();
    let rules = Arc::new(
        LoadedTemporalRules::from_bytes(include_bytes!(
            "../../fqe-ir/tests/fixtures/temporal-memory-overflow.json"
        ))
        .unwrap()
        .bind(&plan)
        .unwrap(),
    );
    let market = MarketScheduler::new(
        Arc::new(
            ScheduleDefinition::from_bytes(include_bytes!(
                "../../fqe-ir/tests/fixtures/schedule-temporal-ticks.json"
            ))
            .unwrap(),
        ),
        "host",
    )
    .unwrap();
    let mut host = InstanceScheduler::new(
        market,
        InstanceLimits {
            instances: 1,
            pending_total: 4,
            pending_per_instance: 4,
        },
    )
    .unwrap();
    host.attach(
        "memory",
        "demo.ticks",
        plan,
        "temporal.rollback",
        Some(rules),
    )
    .unwrap();
    host.ingest("demo.ticks", tick(1, 11)).unwrap();
    host.ingest("demo.ticks", tick(2, 12)).unwrap();
    host.seal("demo.ticks").unwrap();
    let mut rejected = false;
    for _ in 0..10 {
        match host.drive_one().unwrap() {
            Some(DispatchOutcome::Rejected { error, .. }) => {
                assert_eq!(error.code, "temporal_overflow");
                rejected = true;
            }
            None => break,
            _ => {}
        }
    }
    assert!(rejected);
    assert_eq!(count(host.runtime("memory").unwrap(), "fast", "count"), 1);
    assert_eq!(metrics.live.load(Ordering::Relaxed), 1);
    host.retry("memory").unwrap();
    assert!(
        matches!(host.drive_one().unwrap(),Some(DispatchOutcome::Rejected{error,..}) if error.code=="temporal_overflow"),
        "retry must still cross from provider value1 to2; no candidate history leaked"
    );
    assert_eq!(metrics.steps.load(Ordering::Relaxed), 3);
    assert_eq!(metrics.live.load(Ordering::Relaxed), 1);
    host.cancel("memory").unwrap();
    assert_eq!(metrics.live.load(Ordering::Relaxed), 0);
}
#[test]
fn checkpoint_requires_explicit_provider_capability_before_factory_and_restores_opted_in_extension()
{
    use fqe_ir::{
        checkpoint::SnapshotCommand, exchange::MarketRecord, schedule::ScheduleDefinition,
    };
    use fqe_runtime::{
        checkpoint::{CheckpointRuntime, CheckpointSeed, CommandValue, LaneSeed},
        instances::InstanceLimits,
    };
    let make_seed = |workflow| {
        Arc::new(CheckpointSeed {
            schedule: Arc::new(
                ScheduleDefinition::from_bytes(include_bytes!(
                    "../../fqe-ir/tests/fixtures/schedule-temporal-ticks.json"
                ))
                .unwrap(),
            ),
            host_run: "caps".into(),
            limits: InstanceLimits {
                instances: 1,
                pending_total: 8,
                pending_per_instance: 8,
            },
            lanes: vec![LaneSeed {
                id: "custom".into(),
                stream: "demo.ticks".into(),
                run: "custom.run".into(),
                workflow,
                temporal: None,
            }],
        })
    };
    let (unapproved, metrics) = memory_plan();
    assert_eq!(
        CheckpointRuntime::new(make_seed(unapproved))
            .err()
            .unwrap()
            .code,
        "checkpoint_capability"
    );
    assert_eq!(metrics.creates.load(Ordering::Relaxed), 0);
    let (approved, metrics) = memory_plan_with_permit(true);
    let seed = make_seed(approved);
    let mut original = CheckpointRuntime::new(Arc::clone(&seed)).unwrap();
    for (seq, n) in [(1, 11), (2, 12), (3, 14)] {
        let InputEvent::Tick(e) = tick(seq, n) else {
            panic!()
        };
        original
            .execute(SnapshotCommand::Ingest {
                stream: "demo.ticks".into(),
                record: MarketRecord::Tick(e),
            })
            .unwrap();
    }
    original
        .execute(SnapshotCommand::Seal {
            stream: "demo.ticks".into(),
        })
        .unwrap();
    original.execute(SnapshotCommand::Drive).unwrap();
    original.execute(SnapshotCommand::Drive).unwrap();
    let bytes = original.checkpoint().unwrap();
    let mut restored = CheckpointRuntime::restore(seed, &bytes).unwrap();
    for _ in 0..10 {
        let a = original.execute(SnapshotCommand::Drive).unwrap();
        let b = restored.execute(SnapshotCommand::Drive).unwrap();
        assert_eq!(a, b);
        if matches!(a.result, Ok(CommandValue::NoWork)) {
            break;
        }
    }
    assert_eq!(
        count(original.workflow("custom").unwrap(), "fast", "count"),
        3
    );
    assert_eq!(metrics.live.load(Ordering::Relaxed), 2);
    drop(original);
    drop(restored);
    assert_eq!(metrics.live.load(Ordering::Relaxed), 0);
}
