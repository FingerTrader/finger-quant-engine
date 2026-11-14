use fqe_ir::{
    catalog, checkpoint::SnapshotCommand, exchange::MarketData, schedule::ScheduleDefinition,
    temporal::LoadedTemporalRules, workflow::LoadedWorkflow,
};
use fqe_runtime::{
    checkpoint::{CheckpointRuntime, CheckpointSeed, CommandValue, LaneSeed},
    checkpoint_file,
    instances::InstanceLimits,
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
    original.execute(SnapshotCommand::Drive)?;
    original.execute(SnapshotCommand::Drive)?;
    let bytes = original.checkpoint()?;
    if let Some(path) = std::env::args_os().nth(1) {
        let receipt = checkpoint_file::save(path, &bytes)?;
        println!("saved {} bytes", receipt.bytes);
    }
    let mut restored = CheckpointRuntime::restore(seed, &bytes)?;
    println!(
        "restored commands={} output_cursor={}",
        restored.command_count(),
        restored.output_cursor()
    );
    loop {
        let receipt = restored.execute(SnapshotCommand::Drive)?;
        if matches!(&receipt.result, Ok(CommandValue::NoWork)) {
            break;
        }
        let expected = original.execute(SnapshotCommand::Drive)?;
        assert_eq!(receipt, expected);
        println!("continued output={:?}", receipt.output_sequence);
    }
    let final_state = restored.execute(SnapshotCommand::Finish {
        instance: "demo".into(),
    })?;
    println!("finished {:?}", final_state.output_sequence);
    Ok(())
}
