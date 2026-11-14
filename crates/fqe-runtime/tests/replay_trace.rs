use fqe_core::{Event, InstrumentId, MarketTick, PriceTicks};
use fqe_ir::{
    catalog,
    checkpoint::{SnapshotCommand, SnapshotDocument},
    exchange::{MarketData, MarketRecord},
    model::ContentId,
    schedule::ScheduleDefinition,
    temporal::LoadedTemporalRules,
    workflow::LoadedWorkflow,
};
use fqe_runtime::{
    checkpoint::{CheckpointRuntime, CheckpointSeed, CommandReceipt, CommandValue, LaneSeed},
    instances::{DispatchOutcome, InstanceLimits, LaneStatus},
    replay::{self, DivergenceKind, ReplayOutcome, TraceReference},
    trace::{TraceBuffer, TraceData, TraceLimits},
};
use std::sync::Arc;
fn full() -> TraceLimits {
    TraceLimits {
        entries: 4096,
        bytes: 4 * 1024 * 1024,
    }
}
fn seed(indicators: bool, overflow: bool) -> Arc<CheckpointSeed> {
    let (model, rules, schedule) = if indicators {
        (
            include_bytes!("../../fqe-ir/tests/fixtures/checkpoint-indicator-workflow.json")
                .as_slice(),
            include_bytes!("../../fqe-ir/tests/fixtures/checkpoint-indicator-temporal.json")
                .as_slice(),
            include_bytes!("../../fqe-ir/tests/fixtures/checkpoint-indicator-schedule.json")
                .as_slice(),
        )
    } else {
        (
            include_bytes!("../../fqe-ir/tests/fixtures/workflow-counter.json").as_slice(),
            if overflow {
                include_bytes!("../../fqe-ir/tests/fixtures/temporal-overflow.json").as_slice()
            } else {
                include_bytes!("../../fqe-ir/tests/fixtures/temporal-tick-time.json").as_slice()
            },
            include_bytes!("../../fqe-ir/tests/fixtures/schedule-temporal-ticks.json").as_slice(),
        )
    };
    let loaded = LoadedWorkflow::from_bytes(model).unwrap();
    let registry = catalog::reference(loaded.event_plan().trigger()).unwrap();
    let workflow = Arc::new(loaded.compile(&registry, vec![]).unwrap());
    let temporal = Arc::new(
        LoadedTemporalRules::from_bytes(rules)
            .unwrap()
            .bind(&workflow)
            .unwrap(),
    );
    Arc::new(CheckpointSeed {
        schedule: Arc::new(ScheduleDefinition::from_bytes(schedule).unwrap()),
        host_run: "trace-test".into(),
        limits: InstanceLimits {
            instances: 2,
            pending_total: 32,
            pending_per_instance: 16,
        },
        lanes: (0..if indicators { 1 } else { 2 })
            .map(|i| LaneSeed {
                id: format!("i{i}"),
                stream: if indicators {
                    "demo.bars"
                } else {
                    "demo.ticks"
                }
                .into(),
                run: format!("run.{i}"),
                workflow: Arc::clone(&workflow),
                temporal: if i == 0 {
                    Some(Arc::clone(&temporal))
                } else {
                    None
                },
            })
            .collect(),
    })
}
fn input(indicators: bool) -> Vec<SnapshotCommand> {
    let data = MarketData::from_bytes(if indicators {
        include_bytes!("../../fqe-ir/tests/fixtures/checkpoint-indicator-data.json").as_slice()
    } else {
        include_bytes!("fixtures/p2/x002-data.json").as_slice()
    })
    .unwrap();
    data.events()
        .iter()
        .cloned()
        .map(|record| SnapshotCommand::Ingest {
            stream: if indicators {
                "demo.bars"
            } else {
                "demo.ticks"
            }
            .into(),
            record,
        })
        .collect()
}
fn program(
    seed: Arc<CheckpointSeed>,
    initial: Vec<SnapshotCommand>,
    limits: Option<TraceLimits>,
) -> (
    CheckpointRuntime,
    Vec<SnapshotCommand>,
    Vec<CommandReceipt>,
    TraceBuffer,
) {
    let mut runtime = CheckpointRuntime::new(Arc::clone(&seed)).unwrap();
    let mut trace = limits.map_or_else(TraceBuffer::off, |l| TraceBuffer::new(l).unwrap());
    let mut commands = initial;
    commands.push(SnapshotCommand::Seal {
        stream: seed.lanes[0].stream.clone(),
    });
    let mut receipts = vec![];
    for command in &commands {
        receipts.push(runtime.execute_traced(command.clone(), &mut trace).unwrap());
    }
    for _ in 0..100 {
        let r = runtime
            .execute_traced(SnapshotCommand::Drive, &mut trace)
            .unwrap();
        let done = matches!(r.result, Ok(CommandValue::NoWork));
        assert!(r.result.is_ok());
        commands.push(SnapshotCommand::Drive);
        receipts.push(r);
        if done {
            break;
        }
    }
    for lane in &seed.lanes {
        let command = if runtime.host().status(&lane.id) == Some(LaneStatus::Blocked) {
            SnapshotCommand::Cancel {
                instance: lane.id.clone(),
            }
        } else {
            SnapshotCommand::Finish {
                instance: lane.id.clone(),
            }
        };
        receipts.push(runtime.execute_traced(command.clone(), &mut trace).unwrap());
        commands.push(command);
    }
    (runtime, commands, receipts, trace)
}
fn verified(outcome: ReplayOutcome) -> replay::ReplaySuccess {
    match outcome {
        ReplayOutcome::Verified(s) => *s,
        ReplayOutcome::Diverged(d) => panic!("unexpected divergence {d:?}"),
    }
}
#[test]
fn observational_trace_and_entry_byte_exhaustion_preserve_all_receipts_state_and_checkpoint_bytes()
{
    let spec = seed(false, false);
    let (plain, commands, receipts, off) = program(Arc::clone(&spec), input(false), None);
    assert!(off.entries().is_empty());
    assert_eq!(off.dropped(), 0);
    let (traced, actual_commands, actual, trace) =
        program(Arc::clone(&spec), input(false), Some(full()));
    assert_eq!(commands, actual_commands);
    assert_eq!(receipts, actual);
    assert_eq!(plain.checkpoint().unwrap(), traced.checkpoint().unwrap());
    assert!(!trace.truncated());
    assert!(trace.entries().iter().any(|r| matches!(&r.entry.data, TraceData::State { before, after } if before != after && before.delays != after.delays)));
    assert!(
        trace
            .entries()
            .iter()
            .any(|r| r.entry.phase == "guard-candidate" && r.entry.context.sequence == Some(2))
    );
    assert!(
        trace
            .entries()
            .iter()
            .any(|r| r.entry.phase == "timer-committed")
    );
    for limits in [
        TraceLimits {
            entries: 1,
            bytes: full().bytes,
        },
        TraceLimits {
            entries: full().entries,
            bytes: 1,
        },
    ] {
        let (limited, _, actual, trace) = program(Arc::clone(&spec), input(false), Some(limits));
        assert_eq!(receipts, actual);
        assert_eq!(plain.checkpoint().unwrap(), limited.checkpoint().unwrap());
        assert!(trace.truncated());
        assert!(trace.dropped() > 0);
        assert!(trace.entries().len() <= limits.entries);
        assert!(trace.used_bytes() <= limits.bytes);
        let reference = TraceReference::capture(&limited, &trace).unwrap();
        let replay = verified(
            replay::replay(
                Arc::clone(&spec),
                &limited.checkpoint().unwrap(),
                Some(limits),
                Some(&reference),
            )
            .unwrap(),
        );
        assert!(!replay.complete_trace_comparison);
        assert_eq!(
            replay.runtime.checkpoint().unwrap(),
            plain.checkpoint().unwrap()
        );
    }
}
#[test]
fn real_indicator_crossing_workflow_window_traces_repeat_with_identical_pins_and_suppress_replay_receipts()
 {
    let spec = seed(true, false);
    let (runtime, commands, _, trace) = program(Arc::clone(&spec), input(true), Some(full()));
    assert!(
        trace
            .entries()
            .iter()
            .any(|r| r.entry.phase == "provider-candidate" && r.entry.context.sequence == Some(5))
    );
    assert!(trace.entries().iter().any(|r| matches!(
        &r.entry.data,
        TraceData::Crossing {
            previous: Some(_),
            ..
        }
    )));

    assert!(trace.entries().iter().any(
        |r| matches!(&r.entry.data, TraceData::Windows { notices, .. } if !notices.is_empty())
    ));
    assert!(
        trace
            .entries()
            .iter()
            .filter(|r| r.entry.phase == "provider-candidate")
            .all(|r| !r.entry.context.source.definition.is_empty()
                && !r.entry.context.node.is_empty()
                && r.entry.context.timestamp_ns.is_some())
    );
    let reference = TraceReference::capture(&runtime, &trace).unwrap();
    let saved = runtime.checkpoint().unwrap();
    let replay = verified(
        replay::replay(Arc::clone(&spec), &saved, Some(full()), Some(&reference)).unwrap(),
    );
    assert!(replay.complete_trace_comparison);
    assert_eq!(replay.verified_commands, commands.len());
    assert_eq!(replay.trace.entries(), trace.entries());
    assert_eq!(replay.runtime.checkpoint().unwrap(), saved);
    assert_eq!(replay.runtime.output_cursor(), runtime.output_cursor());
    let off = verified(replay::replay(spec, &saved, None, None).unwrap());
    assert!(off.trace.entries().is_empty());
    assert_eq!(off.runtime.checkpoint().unwrap(), saved);
}
#[test]
fn every_command_restore_keeps_trace_suffix_equal_without_recollecting_historical_computation() {
    let spec = seed(true, false);
    let (final_run, commands, expected, trace) =
        program(Arc::clone(&spec), input(true), Some(full()));
    for split in 0..=commands.len() {
        let mut runtime = CheckpointRuntime::new(Arc::clone(&spec)).unwrap();
        let mut actual_trace = TraceBuffer::new(full()).unwrap();
        for command in &commands[..split] {
            runtime
                .execute_traced(command.clone(), &mut actual_trace)
                .unwrap();
        }
        let prefix_trace = actual_trace.entries().to_vec();
        let mut resumed =
            CheckpointRuntime::restore(Arc::clone(&spec), &runtime.checkpoint().unwrap()).unwrap();
        assert_eq!(actual_trace.entries(), prefix_trace);
        for (command, expected) in commands[split..].iter().zip(&expected[split..]) {
            assert_eq!(
                &resumed
                    .execute_traced(command.clone(), &mut actual_trace)
                    .unwrap(),
                expected
            );
        }
        assert_eq!(actual_trace.entries(), trace.entries(), "split{split}");
        assert_eq!(
            resumed.checkpoint().unwrap(),
            final_run.checkpoint().unwrap()
        );
    }
}
#[test]
fn changed_input_reports_first_observed_source_event_while_wrong_outcome_without_reference_stays_command_only()
 {
    let spec = seed(false, false);
    let (original, _, _, trace) = program(Arc::clone(&spec), input(false), Some(full()));
    let reference = TraceReference::capture(&original, &trace).unwrap();
    let mut changed = input(false);
    changed[2] = SnapshotCommand::Ingest {
        stream: "demo.ticks".into(),
        record: MarketRecord::Tick(
            Event::new(
                3,
                3000,
                MarketTick {
                    instrument: InstrumentId::new("DEMO").unwrap(),
                    price: PriceTicks::new(1).unwrap(),
                },
            )
            .unwrap(),
        ),
    };
    let (changed, _, _, _) = program(Arc::clone(&spec), changed, None);
    let outcome = replay::replay(
        Arc::clone(&spec),
        &changed.checkpoint().unwrap(),
        Some(full()),
        Some(&reference),
    )
    .unwrap();
    let ReplayOutcome::Diverged(d) = outcome else {
        panic!("expected observed divergence")
    };
    assert_eq!(d.kind, DivergenceKind::ObservedTrace);
    let location = d.location.unwrap();
    assert_eq!(location.sequence, Some(3));
    assert!(location.source.definition.contains("outputs"));
    assert_eq!(location.run, "run.0");
    assert_ne!(d.expected, d.actual);
    let mut doc = SnapshotDocument::from_bytes(&original.checkpoint().unwrap()).unwrap();
    doc.entries[0].outcome_id = ContentId::from_bytes([7; 32]);
    let ReplayOutcome::Diverged(d) =
        replay::replay(spec, &doc.to_bytes().unwrap(), Some(full()), None).unwrap()
    else {
        panic!("expected outcome divergence")
    };
    assert_eq!(d.kind, DivergenceKind::CommandOutcome);
    assert_eq!(d.command_index, 1);
    assert!(d.location.is_none());
}
#[test]
fn rejected_temporal_candidates_are_labeled_and_do_not_change_rollback_or_other_lanes() {
    let spec = seed(false, true);
    let (off, _, expected, _) = program(Arc::clone(&spec), input(false), None);
    let (runtime, _, receipts, trace) = program(Arc::clone(&spec), input(false), Some(full()));
    assert_eq!(receipts, expected);
    assert_eq!(off.checkpoint().unwrap(), runtime.checkpoint().unwrap());
    let rejection = trace
        .entries()
        .iter()
        .find(|r| r.entry.phase == "lane-rejected")
        .unwrap();
    assert!(matches!(&rejection.entry.data, TraceData::Error(e) if e.code == "temporal_overflow"));
    let command = rejection.command_index.unwrap();
    assert!(
        trace
            .entries()
            .iter()
            .any(|r| r.command_index == Some(command)
                && matches!(
                    &r.entry.data,
                    TraceData::Command {
                        outcome: "lane-rejected",
                        ..
                    }
                ))
    );
    assert!(receipts.iter().any(|r| matches!(&r.result, Ok(CommandValue::Dispatch(DispatchOutcome::Workflow { instance, .. })) if instance == "i1")));
    let reference = TraceReference::capture(&runtime, &trace).unwrap();
    assert!(
        verified(
            replay::replay(
                spec,
                &runtime.checkpoint().unwrap(),
                Some(full()),
                Some(&reference)
            )
            .unwrap()
        )
        .complete_trace_comparison
    );
}
#[test]
fn invalid_trace_configuration_and_mismatched_reference_reject_without_changing_live_state() {
    for limits in [
        TraceLimits {
            entries: 0,
            bytes: 1,
        },
        TraceLimits {
            entries: 4097,
            bytes: 1,
        },
        TraceLimits {
            entries: 1,
            bytes: 4194305,
        },
    ] {
        assert_eq!(TraceBuffer::new(limits).unwrap_err().code, "trace_limit");
    }
    let spec = seed(false, false);
    let (runtime, _, _, trace) = program(Arc::clone(&spec), input(false), Some(full()));
    let reference = TraceReference::capture(&runtime, &trace).unwrap();
    let bytes = runtime.checkpoint().unwrap();
    assert_eq!(
        replay::replay(spec, &bytes, None, Some(&reference))
            .err()
            .unwrap()
            .code,
        "replay_reference"
    );
    assert_eq!(runtime.checkpoint().unwrap(), bytes);
    let mut foreign = TraceBuffer::new(full()).unwrap();
    let loaded = LoadedWorkflow::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/workflow-counter.json"
    ))
    .unwrap();
    let registry = catalog::reference(loaded.event_plan().trigger()).unwrap();
    let mut standalone = fqe_runtime::workflow::WorkflowRuntime::new(
        Arc::new(loaded.compile(&registry, vec![]).unwrap()),
        "standalone",
        ContentId::from_bytes([1; 32]),
    )
    .unwrap();
    let SnapshotCommand::Ingest {
        record: MarketRecord::Tick(event),
        ..
    } = input(false).remove(0)
    else {
        panic!()
    };
    standalone.step_traced(event.into(), &mut foreign).unwrap();
    assert_eq!(
        TraceReference::capture(&runtime, &foreign)
            .err()
            .unwrap()
            .code,
        "replay_reference"
    );
}

#[test]
fn pure_graph_calls_use_the_existing_evaluator_and_trace_the_enclosing_source_and_call_ordinal() {
    let loaded = fqe_ir::event_plan::LoadedEventPlan::from_bytes(include_bytes!(
        "fixtures/p2/trace-pure-event.json"
    ))
    .unwrap();
    let registry = catalog::reference(loaded.trigger()).unwrap();
    let plan = Arc::new(loaded.compile(&registry, vec![]).unwrap());
    let mut plain = fqe_runtime::events::EventRuntime::new(
        Arc::clone(&plan),
        "pure",
        ContentId::from_bytes([3; 32]),
    )
    .unwrap();
    let mut traced =
        fqe_runtime::events::EventRuntime::new(plan, "pure", ContentId::from_bytes([3; 32]))
            .unwrap();
    let mut trace = TraceBuffer::new(full()).unwrap();
    for command in input(false) {
        let SnapshotCommand::Ingest {
            record: MarketRecord::Tick(event),
            ..
        } = command
        else {
            panic!()
        };
        assert_eq!(
            plain.step(event.clone().into()).unwrap(),
            traced.step_traced(event.into(), &mut trace).unwrap()
        );
    }
    let calls = trace.entries().iter().filter(|r| matches!(&r.entry.data, TraceData::PureCall { provider, call_sequence: 1, .. } if provider == "fqe.identity.u64")).collect::<Vec<_>>();
    assert_eq!(calls.len(), 5);
    assert!(
        calls
            .iter()
            .all(|r| r.entry.context.source.definition.contains("outputs")
                && r.entry.context.node.ends_with(&["call".into(), "1".into()]))
    );
}

#[test]
fn exhausted_diagnostics_cannot_hide_a_later_command_outcome_divergence_or_invent_a_node_location()
{
    let spec = seed(false, false);
    let limits = TraceLimits {
        entries: 1,
        bytes: full().bytes,
    };
    let (runtime, _, _, trace) = program(Arc::clone(&spec), input(false), Some(limits));
    let reference = TraceReference::capture(&runtime, &trace).unwrap();
    let mut saved = SnapshotDocument::from_bytes(&runtime.checkpoint().unwrap()).unwrap();
    let count = saved.entries.len();
    saved.entries.last_mut().unwrap().outcome_id = ContentId::from_bytes([12; 32]);
    let ReplayOutcome::Diverged(d) = replay::replay(
        spec,
        &saved.to_bytes().unwrap(),
        Some(limits),
        Some(&reference),
    )
    .unwrap() else {
        panic!("late outcome must diverge")
    };
    assert_eq!(d.kind, DivergenceKind::CommandOutcome);
    assert_eq!(d.command_index, count as u64);
    assert!(d.location.is_none());
    assert!(d.trace.truncated());
}
