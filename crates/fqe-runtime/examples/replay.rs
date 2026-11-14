use fqe_ir::{
    catalog, checkpoint::SnapshotCommand, exchange::MarketData, schedule::ScheduleDefinition,
    temporal::LoadedTemporalRules, workflow::LoadedWorkflow,
};
use fqe_runtime::{
    checkpoint::{CheckpointRuntime, CheckpointSeed, CommandValue, LaneSeed},
    checkpoint_file,
    instances::InstanceLimits,
    replay::{self, ReplayOutcome, TraceReference},
    trace::TraceLimits,
};
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loaded = LoadedWorkflow::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/workflow-counter.json"
    ))?;
    let registry = catalog::reference(loaded.event_plan().trigger())?;
    let workflow = Arc::new(loaded.compile(&registry, vec![])?);
    let temporal = Arc::new(
        LoadedTemporalRules::from_bytes(include_bytes!(
            "../../fqe-ir/tests/fixtures/temporal-tick-time.json"
        ))?
        .bind(&workflow)?,
    );
    let seed = Arc::new(CheckpointSeed {
        schedule: Arc::new(ScheduleDefinition::from_bytes(include_bytes!(
            "../../fqe-ir/tests/fixtures/schedule-temporal-ticks.json"
        ))?),
        host_run: "checkpoint-example".into(),
        limits: InstanceLimits {
            instances: 1,
            pending_total: 16,
            pending_per_instance: 16,
        },
        lanes: vec![LaneSeed {
            id: "demo".into(),
            stream: "demo.ticks".into(),
            run: "demo.run".into(),
            workflow,
            temporal: Some(temporal),
        }],
    });
    let bytes = if let Some(path) = std::env::args_os().nth(1) {
        checkpoint_file::read(path)?
    } else {
        let mut original = CheckpointRuntime::new(Arc::clone(&seed))?;
        let data = MarketData::from_bytes(include_bytes!("../tests/fixtures/p2/x002-data.json"))?;
        for record in data.events() {
            original.execute(SnapshotCommand::Ingest {
                stream: "demo.ticks".into(),
                record: record.clone(),
            })?;
        }
        original.execute(SnapshotCommand::Seal {
            stream: "demo.ticks".into(),
        })?;
        loop {
            if matches!(
                original.execute(SnapshotCommand::Drive)?.result,
                Ok(CommandValue::NoWork)
            ) {
                break;
            }
        }
        original.checkpoint()?
    };
    let limits = Some(TraceLimits {
        entries: 4096,
        bytes: 4194304,
    });
    let ReplayOutcome::Verified(first) = replay::replay(Arc::clone(&seed), &bytes, limits, None)?
    else {
        return Err("checkpoint outcomes diverged".into());
    };
    println!(
        "verified commands={} output_cursor={} trace_entries={} trace_bytes={} truncated={}",
        first.verified_commands,
        first.runtime.output_cursor(),
        first.trace.entries().len(),
        first.trace.used_bytes(),
        first.trace.truncated()
    );
    if let Some(record) = first
        .trace
        .entries()
        .iter()
        .find(|r| r.entry.phase == "guard-candidate")
    {
        println!(
            "source run={} event={:?} node={:?} path={}",
            record.entry.context.run,
            record.entry.context.sequence,
            record.entry.context.node,
            record.entry.context.source.definition
        );
    }
    let reference = TraceReference::capture(&first.runtime, &first.trace)?;
    let ReplayOutcome::Verified(second) = replay::replay(seed, &bytes, limits, Some(&reference))?
    else {
        return Err("trace diverged".into());
    };
    assert!(second.complete_trace_comparison);
    println!("repeat trace comparison=complete; historical receipts suppressed");
    Ok(())
}
