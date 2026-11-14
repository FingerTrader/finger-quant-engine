use fqe_core::{
    Event, InstrumentId, MarketTick, PriceTicks,
    bars::{BarInput, ClosedBar},
    numeric::Decimal,
    types::{NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
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
    instances::InstanceLimits,
};
use std::sync::Arc;
const BAR: &str = include_str!("../../fqe-ir/tests/fixtures/workflow-bar-counter.json");
const COUNT: &str = include_str!("../../fqe-ir/tests/fixtures/temporal-count.json");
const SCHEDULE: &str = include_str!("../../fqe-ir/tests/fixtures/schedule-temporal.json");
fn seed(
    workflow: &str,
    temporal: Option<&str>,
    schedule: &str,
    lanes: usize,
) -> Arc<CheckpointSeed> {
    let loaded = LoadedWorkflow::from_bytes(workflow.as_bytes()).unwrap();
    let registry = catalog::reference(loaded.event_plan().trigger()).unwrap();
    let plan = Arc::new(loaded.compile(&registry, vec![]).unwrap());
    let rules = temporal.map(|src| {
        Arc::new(
            LoadedTemporalRules::from_bytes(src.as_bytes())
                .unwrap()
                .bind(&plan)
                .unwrap(),
        )
    });
    let schedule = Arc::new(ScheduleDefinition::from_bytes(schedule.as_bytes()).unwrap());
    let stream = if matches!(
        plan.event().trigger(),
        fqe_ir::event_plan::Trigger::Tick { .. }
    ) {
        "demo.ticks"
    } else {
        "demo.bars"
    };
    Arc::new(CheckpointSeed {
        schedule,
        host_run: "checkpoint-test".into(),
        limits: InstanceLimits {
            instances: 4,
            pending_total: 32,
            pending_per_instance: 16,
        },
        lanes: (0..lanes)
            .map(|i| LaneSeed {
                id: format!("i{i}"),
                stream: stream.into(),
                run: format!("run.{i}"),
                workflow: Arc::clone(&plan),
                temporal: rules.clone(),
            })
            .collect(),
    })
}
fn bar(seq: u64, n: i128) -> MarketRecord {
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
    MarketRecord::Bar(Box::new(
        ClosedBar::new(BarInput {
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
        })
        .unwrap(),
    ))
}
fn fingerprint(runtime: &CheckpointRuntime) -> String {
    let mut result = format!(
        "{:?}{:?}{:?}{}",
        runtime.host().market().state(),
        runtime.host().market().cursors(),
        runtime.host().market().alignment(),
        runtime.host().pending()
    );
    for lane in &runtime.seed().lanes {
        let r = runtime.workflow(&lane.id).unwrap();
        result.push_str(&format!(
            "{:?}{:?}{:?}{:?}",
            runtime.host().status(&lane.id),
            r.state(),
            r.instance_values(),
            runtime.host().windows(&lane.id)
        ));
    }
    result
}
fn run_program(
    seed: Arc<CheckpointSeed>,
    initial: Vec<SnapshotCommand>,
) -> (Vec<SnapshotCommand>, Vec<CommandReceipt>, String) {
    let mut runtime = CheckpointRuntime::new(Arc::clone(&seed)).unwrap();
    let mut commands = initial;
    let streams = seed.schedule.streams().keys().cloned().collect::<Vec<_>>();
    for stream in streams {
        commands.push(SnapshotCommand::Seal { stream });
    }
    let mut receipts = commands
        .iter()
        .cloned()
        .map(|c| runtime.execute(c).unwrap())
        .collect::<Vec<_>>();
    for _ in 0..200 {
        let command = SnapshotCommand::Drive;
        let receipt = runtime.execute(command.clone()).unwrap();
        let no_work = matches!(receipt.result, Ok(CommandValue::NoWork));
        assert!(receipt.result.is_ok(), "{:?}", receipt.result);
        commands.push(command);
        receipts.push(receipt);
        if no_work {
            break;
        }
    }
    for lane in &seed.lanes {
        let command = SnapshotCommand::Finish {
            instance: lane.id.clone(),
        };
        let receipt = runtime.execute(command.clone()).unwrap();
        assert!(receipt.result.is_ok());
        commands.push(command);
        receipts.push(receipt);
    }
    let final_state = fingerprint(&runtime);
    (commands, receipts, final_state)
}
fn split_every_command(
    seed: Arc<CheckpointSeed>,
    commands: &[SnapshotCommand],
    expected: &[CommandReceipt],
    final_state: &str,
) {
    for split in 0..=commands.len() {
        let mut prefix = CheckpointRuntime::new(Arc::clone(&seed)).unwrap();
        for command in &commands[..split] {
            prefix.execute(command.clone()).unwrap();
        }
        let before = fingerprint(&prefix);
        let bytes = prefix.checkpoint().unwrap();
        let saved_cursor = prefix.output_cursor();
        let mut restored = CheckpointRuntime::restore(Arc::clone(&seed), &bytes).unwrap();
        assert_eq!(fingerprint(&restored), before, "split={split}");
        assert_eq!(restored.output_cursor(), saved_cursor);
        assert_eq!(restored.command_count(), split);
        for (command, receipt) in commands[split..].iter().zip(&expected[split..]) {
            let actual = restored.execute(command.clone()).unwrap();
            assert_eq!(&actual, receipt, "split={split}");
            if let Some(sequence) = actual.output_sequence {
                assert!(sequence > saved_cursor, "historical receipt republished");
            }
        }
        assert_eq!(fingerprint(&restored), final_state, "split={split}");
    }
}
#[test]
fn x002_every_command_restore_preserves_queued_fairness_cells_delay_count_windows_and_output_cursor()
 {
    let seed = seed(BAR, Some(COUNT), SCHEDULE, 2);
    let mut commands = [90, 110, 120, 80, 130]
        .into_iter()
        .enumerate()
        .map(|(i, n)| SnapshotCommand::Ingest {
            stream: "demo.bars".into(),
            record: bar(i as u64 + 1, n),
        })
        .collect::<Vec<_>>();
    commands.push(SnapshotCommand::Ingest {
        stream: "other.ticks".into(),
        record: MarketRecord::Tick(
            Event::new(
                77,
                25,
                MarketTick {
                    instrument: InstrumentId::new("OTHER").unwrap(),
                    price: PriceTicks::new(1).unwrap(),
                },
            )
            .unwrap(),
        ),
    });
    let (commands, receipts, final_state) = run_program(Arc::clone(&seed), commands);
    split_every_command(seed, &commands, &receipts, &final_state);
}
#[test]
fn actual_indicators_crossing_and_count_time_buffers_restore_at_every_boundary() {
    let seed = seed(
        include_str!("../../fqe-ir/tests/fixtures/checkpoint-indicator-workflow.json"),
        Some(include_str!(
            "../../fqe-ir/tests/fixtures/checkpoint-indicator-temporal.json"
        )),
        include_str!("../../fqe-ir/tests/fixtures/checkpoint-indicator-schedule.json"),
        1,
    );
    let data = MarketData::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/checkpoint-indicator-data.json"
    ))
    .unwrap();
    assert_eq!(
        seed.schedule.streams()["demo.bars"].dataset_id(),
        data.source_id()
    );
    let commands = data
        .events()
        .iter()
        .cloned()
        .map(|record| SnapshotCommand::Ingest {
            stream: "demo.bars".into(),
            record,
        })
        .collect();
    let (commands, receipts, final_state) = run_program(Arc::clone(&seed), commands);
    assert!(receipts.iter().any(|r| matches!(
        &r.result,
        Ok(CommandValue::Dispatch(
            fqe_runtime::instances::DispatchOutcome::Timer { .. }
        ))
    )));
    assert!(receipts.iter().any(|r|matches!(&r.result,Ok(CommandValue::Dispatch(fqe_runtime::instances::DispatchOutcome::Workflow{windows,..})) if !windows.is_empty())));
    split_every_command(seed, &commands, &receipts, &final_state);
}
#[test]
fn corruption_compatibility_and_recomputed_checksum_divergence_never_replace_live_state() {
    let seed = seed(BAR, Some(COUNT), SCHEDULE, 1);
    let mut live = CheckpointRuntime::new(Arc::clone(&seed)).unwrap();
    live.execute(SnapshotCommand::Ingest {
        stream: "demo.bars".into(),
        record: bar(1, 90),
    })
    .unwrap();
    let bytes = live.checkpoint().unwrap();
    let mut corrupt = bytes.clone();
    let end = corrupt.len() - 5;
    corrupt[end] = b'X';
    assert!(CheckpointRuntime::restore(Arc::clone(&seed), &corrupt).is_err());
    assert!(CheckpointRuntime::restore(Arc::clone(&seed), &bytes[..bytes.len() / 2]).is_err());
    let mut document = SnapshotDocument::from_bytes(&bytes).unwrap();
    document.entries[0].outcome_id = ContentId::from_bytes([7; 32]);
    let valid_integrity = document.to_bytes().unwrap();
    assert_eq!(
        CheckpointRuntime::restore(Arc::clone(&seed), &valid_integrity)
            .err()
            .unwrap()
            .code,
        "checkpoint_divergence"
    );
    for (path, value) in [
        (vec!["engine".into()], "different"),
        (
            vec!["stream".into(), "demo.bars".into(), "dataset".into()],
            "sha256:wrong",
        ),
        (
            vec![
                "lane".into(),
                "i0".into(),
                "module".into(),
                "fast".into(),
                "state-schema-version".into(),
            ],
            "2",
        ),
    ] {
        let mut doc = SnapshotDocument::from_bytes(&bytes).unwrap();
        doc.pins.insert(path, value.into());
        assert_eq!(
            CheckpointRuntime::restore(Arc::clone(&seed), &doc.to_bytes().unwrap())
                .err()
                .unwrap()
                .code,
            "checkpoint_incompatible"
        );
    }
    let mut changed = seed.as_ref().clone();
    changed.host_run = "another".into();
    assert_eq!(
        CheckpointRuntime::restore(Arc::new(changed), &bytes)
            .err()
            .unwrap()
            .code,
        "checkpoint_incompatible"
    );
    let changed = BAR.replace("\"value\": \"120\"", "\"value\": \"121\"");
    let changed_seed = self::seed(&changed, Some(COUNT), SCHEDULE, 1);
    assert_eq!(
        CheckpointRuntime::restore(changed_seed, &bytes)
            .err()
            .unwrap()
            .code,
        "checkpoint_incompatible"
    );
    assert_eq!(live.checkpoint().unwrap(), bytes);
}
#[test]
fn checkpoint_budget_failure_precedes_state_mutation_and_retains_last_complete_image() {
    let seed = seed(BAR, None, SCHEDULE, 0);
    let mut runtime = CheckpointRuntime::new(seed).unwrap();
    for _ in 0..1024 {
        runtime.execute(SnapshotCommand::Drive).unwrap();
    }
    let bytes = runtime.checkpoint().unwrap();
    assert!(runtime.execute(SnapshotCommand::Drive).is_err());
    assert_eq!(runtime.command_count(), 1024);
    assert_eq!(runtime.checkpoint().unwrap(), bytes);
    let state = fingerprint(&runtime);
    assert!(
        runtime
            .execute(SnapshotCommand::Seal {
                stream: "invalid name".into()
            })
            .is_err()
    );
    assert_eq!(fingerprint(&runtime), state);
}
#[test]
fn blocked_lane_retry_cancel_and_failed_control_commands_restore_at_each_boundary() {
    let base = seed(
        include_str!("../../fqe-ir/tests/fixtures/workflow-counter.json"),
        Some(include_str!(
            "../../fqe-ir/tests/fixtures/temporal-overflow.json"
        )),
        include_str!("../../fqe-ir/tests/fixtures/schedule-temporal-ticks.json"),
        2,
    );
    let mut spec = base.as_ref().clone();
    spec.lanes[1].temporal = None;
    let spec = Arc::new(spec);
    let mut runtime = CheckpointRuntime::new(Arc::clone(&spec)).unwrap();
    let mut commands = vec![];
    let mut receipts = vec![];
    let mut execute = |runtime: &mut CheckpointRuntime, c: SnapshotCommand| {
        let r = runtime.execute(c.clone()).unwrap();
        commands.push(c);
        receipts.push(r.clone());
        r
    };
    for (i, n) in [90, 110, 120].into_iter().enumerate() {
        execute(
            &mut runtime,
            SnapshotCommand::Ingest {
                stream: "demo.ticks".into(),
                record: MarketRecord::Tick(
                    Event::new(
                        i as u64 + 1,
                        (i as u64 + 1) * 1000,
                        MarketTick {
                            instrument: InstrumentId::new("DEMO").unwrap(),
                            price: PriceTicks::new(n).unwrap(),
                        },
                    )
                    .unwrap(),
                ),
            },
        );
    }
    execute(
        &mut runtime,
        SnapshotCommand::Seal {
            stream: "demo.ticks".into(),
        },
    );
    for _ in 0..30 {
        if matches!(
            execute(&mut runtime, SnapshotCommand::Drive).result,
            Ok(CommandValue::NoWork)
        ) {
            break;
        }
    }
    assert_eq!(
        runtime.host().status("i0"),
        Some(fqe_runtime::instances::LaneStatus::Blocked)
    );
    assert_eq!(runtime.host().pending(), 2);
    execute(
        &mut runtime,
        SnapshotCommand::Retry {
            instance: "i0".into(),
        },
    );
    execute(&mut runtime, SnapshotCommand::Drive);
    execute(
        &mut runtime,
        SnapshotCommand::Cancel {
            instance: "i0".into(),
        },
    );
    assert!(
        execute(
            &mut runtime,
            SnapshotCommand::Retry {
                instance: "i0".into()
            }
        )
        .result
        .is_err()
    );
    execute(
        &mut runtime,
        SnapshotCommand::Finish {
            instance: "i1".into(),
        },
    );
    let final_state = fingerprint(&runtime);
    split_every_command(spec, &commands, &receipts, &final_state);
}
