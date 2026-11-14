use fqe_core::{
    Event, InstrumentId, MarketTick, PriceTicks, SignalKind,
    numeric::Decimal,
    types::{
        Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
    },
};
use fqe_ir::{
    catalog,
    checkpoint::SnapshotCommand,
    exchange::{MarketData, MarketRecord},
    parameters::ParameterInput,
    schedule::ScheduleDefinition,
    temporal::LoadedTemporalRules,
    workflow::LoadedWorkflow,
};
use fqe_runtime::{
    checkpoint::{CheckpointRuntime, CheckpointSeed, CommandReceipt, CommandValue, LaneSeed},
    instances::{DispatchOutcome, InstanceLimits},
    replay::{self, ReplayOutcome, TraceReference},
    trace::{TraceBuffer, TraceLimits},
};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    threshold: i128,
    data: &'static [u8],
    schedule: &'static [u8],
}
macro_rules! case {
    ($name:literal,$threshold:literal) => {
        Case {
            name: $name,
            threshold: $threshold,
            data: include_bytes!(concat!("fixtures/p2/g006-", $name, "-data.json")),
            schedule: include_bytes!(concat!("fixtures/p2/g006-", $name, "-schedule.json")),
        }
    };
}
const CASES: &[Case] = &[
    case!("early", 90),
    case!("expiry", 90),
    case!("late", 90),
    case!("arming-only", 80),
    case!("never", 100),
    case!("recross", 80),
];
fn seed(case: Case, lanes: usize) -> Arc<CheckpointSeed> {
    let loaded =
        LoadedWorkflow::from_bytes(include_bytes!("fixtures/p2/g006-workflow.json")).unwrap();
    let registry = catalog::reference(loaded.event_plan().trigger()).unwrap();
    let threshold = TypedValue::new(
        TypeDescriptor::new(
            1,
            ValueShape::Scalar,
            ScalarType::Decimal {
                scale: 12,
                unit: NumericUnit::Ratio,
            },
        )
        .unwrap(),
        ScalarValue::Decimal(Decimal::new(case.threshold * 1_000_000_000_000, 12).unwrap()),
    )
    .unwrap();
    let workflow = Arc::new(
        loaded
            .compile(
                &registry,
                vec![(
                    "threshold".into(),
                    ParameterInput::Value(Evaluation::Ready(threshold)),
                )],
            )
            .unwrap(),
    );
    let temporal = Arc::new(
        LoadedTemporalRules::from_bytes(include_bytes!("fixtures/p2/g006-temporal.json"))
            .unwrap()
            .bind(&workflow)
            .unwrap(),
    );
    let schedule = Arc::new(ScheduleDefinition::from_bytes(case.schedule).unwrap());
    let data = MarketData::from_bytes(case.data).unwrap();
    assert_eq!(
        schedule.streams()["demo.bars"].dataset_id(),
        data.source_id()
    );
    Arc::new(CheckpointSeed {
        schedule,
        host_run: "p2-gate".into(),
        limits: InstanceLimits {
            instances: 4,
            pending_total: 64,
            pending_per_instance: 32,
        },
        lanes: (0..lanes)
            .map(|i| LaneSeed {
                id: format!("i{i}"),
                stream: "demo.bars".into(),
                run: format!("g006.{i}"),
                workflow: Arc::clone(&workflow),
                temporal: Some(Arc::clone(&temporal)),
            })
            .collect(),
    })
}
fn flag(v: &Evaluation<TypedValue>) -> bool {
    matches!(v,Evaluation::Ready(v) if v.value()==&ScalarValue::Bool(true))
}
fn number(v: &TypedValue) -> u64 {
    let ScalarValue::U64(v) = v.value() else {
        panic!("expected u64")
    };
    *v
}
fn boolean(v: &TypedValue) -> bool {
    let ScalarValue::Bool(v) = v.value() else {
        panic!("expected bool")
    };
    *v
}
fn rows(case: Case) -> Vec<BTreeMap<&'static str, &'static str>> {
    let mut lines = include_str!("fixtures/p2/g006.expected.tsv").lines();
    let headers = lines.next().unwrap().split('\t').collect::<Vec<_>>();
    lines
        .map(|line| {
            headers
                .iter()
                .copied()
                .zip(line.split('\t'))
                .collect::<BTreeMap<_, _>>()
        })
        .filter(|row| row["case"] == case.name)
        .collect()
}
fn numeric(actual: &Evaluation<TypedValue>, expected: &str) {
    if expected == "W" {
        assert_eq!(actual, &Evaluation::Warmup);
    } else {
        let Evaluation::Ready(v) = actual else {
            panic!("expected ready {expected}: {actual:?}")
        };
        let expected = expected.parse::<i128>().unwrap();
        assert!(
            (v.numeric_coefficient().unwrap() - expected).abs() <= 1,
            "{v:?} expected{expected}"
        );
    }
}
fn run(
    case: Case,
    lanes: usize,
) -> (
    Arc<CheckpointSeed>,
    CheckpointRuntime,
    Vec<SnapshotCommand>,
    Vec<CommandReceipt>,
    TraceBuffer,
) {
    let seed = seed(case, lanes);
    let mut runtime = CheckpointRuntime::new(Arc::clone(&seed)).unwrap();
    let mut commands = vec![];
    let mut receipts = vec![];
    let mut trace = TraceBuffer::new(TraceLimits {
        entries: 4096,
        bytes: 4194304,
    })
    .unwrap();
    for record in MarketData::from_bytes(case.data).unwrap().events() {
        let c = SnapshotCommand::Ingest {
            stream: "demo.bars".into(),
            record: record.clone(),
        };
        receipts.push(runtime.execute_traced(c.clone(), &mut trace).unwrap());
        commands.push(c);
    }
    let c = SnapshotCommand::Seal {
        stream: "demo.bars".into(),
    };
    receipts.push(runtime.execute_traced(c.clone(), &mut trace).unwrap());
    commands.push(c);
    let expected = rows(case);
    let mut seen = BTreeMap::<String, usize>::new();
    for _ in 0..200 {
        let r = runtime
            .execute_traced(SnapshotCommand::Drive, &mut trace)
            .unwrap();
        let done = matches!(r.result, Ok(CommandValue::NoWork));
        if let Ok(CommandValue::Dispatch(DispatchOutcome::Workflow {
            instance,
            observation,
            windows,
            alignment,
        })) = &r.result
        {
            let index = *seen.entry(instance.clone()).or_default();
            let row = &expected[index];
            seen.insert(instance.clone(), index + 1);
            let bar = row["bar"].parse::<u64>().unwrap();
            assert_eq!(alignment.latest("demo.bars").unwrap().source_sequence, bar);
            assert_eq!(observation.source_sequence, bar);
            assert_eq!(observation.timestamp_ns, bar * 1000);
            numeric(&observation.event_outputs["line"], row["line"]);
            numeric(&observation.event_outputs["signal"], row["signal"]);
            numeric(&observation.event_outputs["rsi"], row["rsi"]);
            assert_eq!(
                flag(&observation.event_outputs["cross"]),
                row["cross"] == "true"
            );
            assert_eq!(
                flag(&observation.event_outputs["rsi_ok"]),
                row["rsi_ok"] == "true"
            );
            let output = &observation.outputs["wait"];
            assert_eq!(flag(&output["emit"]), row["emit"] == "true");
            assert_eq!(flag(&output["latched"]), row["before_latched"] == "true");
            let Evaluation::Ready(old) = &output["remaining"] else {
                panic!()
            };
            assert_eq!(number(old), row["before_remaining"].parse::<u64>().unwrap());
            let transition = if row["transition"] == "none" {
                vec![]
            } else {
                vec![row["transition"].to_owned()]
            };
            assert_eq!(observation.transitions["wait"], transition);
            let state = &runtime.workflow(instance).unwrap().instance_values()["wait"];
            let remaining = row["after_remaining"].parse::<u64>().unwrap();
            assert_eq!(number(&state.state["remaining"]), remaining);
            assert_eq!(
                boolean(&state.state["latched"]),
                row["after_latched"] == "true"
            );
            assert_eq!(state.mode, if remaining == 0 { "idle" } else { "armed" });
            assert_eq!(
                observation.signals.len(),
                usize::from(row["emit"] == "true")
            );
            for signal in &observation.signals {
                assert_eq!(signal.record.source_sequence, bar);
                assert_eq!(signal.record.signal.kind, SignalKind::Long);
                assert_eq!(signal.record.timestamp_ns, bar * 1000);
            }
            assert_eq!(windows.len(), usize::from(row["window_expired"] == "true"));
            for notice in windows {
                assert_eq!(notice.satisfied, row["window_satisfied"] == "true");
                assert_eq!(notice.expired_ns, bar * 1000);
                assert_eq!(notice.satisfied, !observation.signals.is_empty());
            }
            let active = runtime.host().windows(instance).unwrap();
            assert_eq!(active.len(), usize::from(remaining > 0));
            if remaining > 0 {
                assert_eq!(active["wait"].observed_bars, 3 - remaining);
                assert_eq!(active["wait"].satisfied, boolean(&state.state["latched"]));
            }
        }
        assert!(r.result.is_ok(), "{:?}", r.result);
        commands.push(SnapshotCommand::Drive);
        receipts.push(r);
        if done {
            break;
        }
    }
    assert_eq!(seen.len(), lanes);
    for count in seen.values() {
        assert_eq!(*count, expected.len());
    }
    for lane in &seed.lanes {
        let c = SnapshotCommand::Finish {
            instance: lane.id.clone(),
        };
        let r = runtime.execute_traced(c.clone(), &mut trace).unwrap();
        assert!(r.result.is_ok());
        let Ok(CommandValue::Closed(closure)) = &r.result else {
            panic!("finish closure required")
        };
        assert_eq!(
            closure.incomplete_windows.len(),
            usize::from(case.name == "late")
        );
        if case.name == "late" {
            assert_eq!(closure.incomplete_windows[0].opened_sequence, 9);
            assert_eq!(closure.incomplete_windows[0].observed_bars, 0);
        }
        commands.push(c);
        receipts.push(r);
    }
    (seed, runtime, commands, receipts, trace)
}
#[test]
fn actual_g006_all_six_independent_numeric_state_latch_expiry_recross_cases_and_real_long_signals()
{
    for &case in CASES {
        run(case, 1);
    }
    assert_eq!(CASES.iter().map(|c| rows(*c).len()).sum::<usize>(), 55);
}
#[test]
fn real_indicator_and_rule_instances_are_isolated_shared_plan_repeat_and_trace_replay_identically()
{
    let (seed, runtime, _, receipts, trace) = run(CASES[5], 2);
    let reference = TraceReference::capture(&runtime, &trace).unwrap();
    let ReplayOutcome::Verified(replayed) = replay::replay(
        Arc::clone(&seed),
        &runtime.checkpoint().unwrap(),
        trace.limits(),
        Some(&reference),
    )
    .unwrap() else {
        panic!("replay divergence")
    };
    assert!(replayed.complete_trace_comparison);
    assert_eq!(
        replayed.runtime.checkpoint().unwrap(),
        runtime.checkpoint().unwrap()
    );
    let (_, repeated, _, again, _) = run(CASES[5], 2);
    assert_eq!(receipts, again);
    assert_eq!(
        repeated.checkpoint().unwrap(),
        runtime.checkpoint().unwrap()
    );
    let signals = receipts
        .iter()
        .filter_map(|r| {
            if let Ok(CommandValue::Dispatch(DispatchOutcome::Workflow {
                instance,
                observation,
                ..
            })) = &r.result
            {
                Some((
                    instance.clone(),
                    observation
                        .signals
                        .iter()
                        .map(|s| s.record.source_sequence)
                        .collect::<Vec<_>>(),
                ))
            } else {
                None
            }
        })
        .filter(|(_, s)| !s.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        signals,
        vec![("i0".into(), vec![8]), ("i1".into(), vec![8])]
    );
}
#[test]
fn g006_restore_every_wait_point_and_every_expiry_case_command_preserves_continuation_and_output_identity()
 {
    for &case in CASES {
        let (seed, final_run, commands, expected, _) = run(case, 1);
        let mut split_points = vec![0, commands.len()];
        for (i, r) in expected.iter().enumerate() {
            if let Ok(CommandValue::Dispatch(DispatchOutcome::Workflow { observation, .. })) =
                &r.result
                && observation.source_sequence >= 5
            {
                split_points.push(i + 1);
            }
        }
        if case.name == "expiry" {
            split_points = (0..=commands.len()).collect();
        }
        for split in split_points {
            let mut prefix = CheckpointRuntime::new(Arc::clone(&seed)).unwrap();
            for c in &commands[..split] {
                prefix.execute(c.clone()).unwrap();
            }
            let cursor = prefix.output_cursor();
            let mut restored =
                CheckpointRuntime::restore(Arc::clone(&seed), &prefix.checkpoint().unwrap())
                    .unwrap();
            assert_eq!(
                restored.workflow("i0").unwrap().inspect_state().unwrap(),
                prefix.workflow("i0").unwrap().inspect_state().unwrap()
            );
            assert_eq!(restored.host().windows("i0"), prefix.host().windows("i0"));
            for (c, r) in commands[split..].iter().zip(&expected[split..]) {
                let actual = restored.execute(c.clone()).unwrap();
                assert_eq!(&actual, r, "{} split{split}", case.name);
                if let Some(n) = actual.output_sequence {
                    assert!(n > cursor);
                }
            }
            assert_eq!(
                restored.checkpoint().unwrap(),
                final_run.checkpoint().unwrap(),
                "{} split{split}",
                case.name
            );
        }
    }
}
#[test]
fn queued_future_input_prefixes_and_duplicate_backward_ingress_cannot_change_g006_decisions() {
    let case = CASES[1];
    let (seed, _, _, receipts, _) = run(case, 1);
    let data = MarketData::from_bytes(case.data).unwrap();
    let baseline = receipts
        .iter()
        .filter_map(|r| {
            if let Ok(CommandValue::Dispatch(DispatchOutcome::Workflow { observation, .. })) =
                &r.result
            {
                Some(observation)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let mut stream = CheckpointRuntime::new(seed).unwrap();
    let mut observations = vec![];
    for record in data.events() {
        let c = SnapshotCommand::Ingest {
            stream: "demo.bars".into(),
            record: record.clone(),
        };
        assert!(stream.execute(c.clone()).unwrap().result.is_ok());
        let count = stream.host().market().state().accepted_events;
        let cursor = stream.host().market().cursors().clone();
        assert!(stream.execute(c).unwrap().result.is_err());
        assert_eq!(stream.host().market().state().accepted_events, count);
        assert_eq!(stream.host().market().cursors(), &cursor);
        let MarketRecord::Bar(bar) = record else {
            panic!()
        };
        stream
            .execute(SnapshotCommand::Watermark {
                stream: "demo.bars".into(),
                timestamp_ns: bar.input().close_ns,
            })
            .unwrap();
        loop {
            let r = stream.execute(SnapshotCommand::Drive).unwrap();
            match r.result {
                Ok(CommandValue::NoWork) => break,
                Ok(CommandValue::Dispatch(DispatchOutcome::Workflow {
                    observation,
                    alignment,
                    ..
                })) => {
                    assert!(
                        alignment
                            .streams()
                            .values()
                            .all(|v| v.timestamp_ns <= observation.timestamp_ns)
                    );
                    observations.push(observation)
                }
                Ok(_) => {}
                Err(e) => panic!("{e}"),
            }
        }
        assert_eq!(
            observations.iter().collect::<Vec<_>>(),
            baseline[..observations.len()]
        );
    }
    let before = stream.workflow("i0").unwrap().inspect_state().unwrap();
    assert!(
        stream
            .execute(SnapshotCommand::Ingest {
                stream: "demo.bars".into(),
                record: data.events()[0].clone()
            })
            .unwrap()
            .result
            .is_err()
    );
    assert_eq!(
        stream.workflow("i0").unwrap().inspect_state().unwrap(),
        before
    );
}
#[test]
fn unrelated_symbol_events_do_not_count_as_wait_bars_and_asof_trace_never_reads_future() {
    // An independent second stream shares the host merger, never the subscribed lane.
    let case = CASES[1];
    let base = seed(case, 1);
    let schedule = String::from_utf8(case.schedule.to_vec()).unwrap();
    let extra = r#",{"id":"other.ticks","dataset_id":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","domain":{"kind":"tick","instrument":"OTHER"},"gap_policy":"not-applicable"}"#;
    let at = schedule.rfind(']').unwrap();
    let mut changed = schedule.clone();
    changed.insert_str(at, extra);
    let mut spec = base.as_ref().clone();
    spec.schedule = Arc::new(ScheduleDefinition::from_bytes(changed.as_bytes()).unwrap());
    let mut runtime = CheckpointRuntime::new(Arc::new(spec)).unwrap();
    for record in MarketData::from_bytes(case.data).unwrap().events() {
        runtime
            .execute(SnapshotCommand::Ingest {
                stream: "demo.bars".into(),
                record: record.clone(),
            })
            .unwrap();
    }
    runtime
        .execute(SnapshotCommand::Ingest {
            stream: "other.ticks".into(),
            record: MarketRecord::Tick(
                Event::new(
                    99,
                    6500,
                    MarketTick {
                        instrument: InstrumentId::new("OTHER").unwrap(),
                        price: PriceTicks::new(1).unwrap(),
                    },
                )
                .unwrap(),
            ),
        })
        .unwrap();
    runtime
        .execute(SnapshotCommand::Seal {
            stream: "demo.bars".into(),
        })
        .unwrap();
    assert!(matches!(
        runtime.execute(SnapshotCommand::Drive).unwrap().result,
        Ok(CommandValue::NoWork)
    ));
    runtime
        .execute(SnapshotCommand::Seal {
            stream: "other.ticks".into(),
        })
        .unwrap();
    let mut hits = vec![];
    let mut count = 0;
    loop {
        let r = runtime.execute(SnapshotCommand::Drive).unwrap();
        match r.result {
            Ok(CommandValue::NoWork) => break,
            Ok(CommandValue::Dispatch(DispatchOutcome::Workflow {
                observation,
                alignment,
                windows,
                ..
            })) => {
                count += 1;
                let source = alignment.latest("demo.bars").unwrap().source_sequence;
                assert!(
                    alignment
                        .streams()
                        .values()
                        .all(|e| e.timestamp_ns <= observation.timestamp_ns)
                );
                if !observation.signals.is_empty() {
                    hits.push(source);
                    assert_eq!(observation.source_sequence, 9);
                    assert_eq!(windows.len(), 1);
                }
            }
            Ok(_) => {}
            Err(e) => panic!("{e}"),
        }
    }
    assert_eq!(count, 8);
    assert_eq!(hits, vec![8]);
}
