use fqe_ir::{
    catalog, checkpoint::SnapshotCommand, exchange::MarketData, schedule::ScheduleDefinition,
    temporal::LoadedTemporalRules, workflow::LoadedWorkflow,
};
use fqe_runtime::{
    checkpoint::{CheckpointRuntime, CheckpointSeed, CommandValue, LaneSeed},
    instances::{DispatchOutcome, InstanceLimits},
};
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loaded =
        LoadedWorkflow::from_bytes(include_bytes!("../tests/fixtures/p2/g006-workflow.json"))?;
    let registry = catalog::reference(loaded.event_plan().trigger())?;
    let workflow = Arc::new(loaded.compile(&registry, vec![])?);
    let temporal = Arc::new(
        LoadedTemporalRules::from_bytes(include_bytes!("../tests/fixtures/p2/g006-temporal.json"))?
            .bind(&workflow)?,
    );
    let seed = Arc::new(CheckpointSeed {
        schedule: Arc::new(ScheduleDefinition::from_bytes(include_bytes!(
            "../tests/fixtures/p2/g006-expiry-schedule.json"
        ))?),
        host_run: "g006-example".into(),
        limits: InstanceLimits {
            instances: 1,
            pending_total: 16,
            pending_per_instance: 16,
        },
        lanes: vec![LaneSeed {
            id: "demo".into(),
            stream: "demo.bars".into(),
            run: "demo.run".into(),
            workflow,
            temporal: Some(temporal),
        }],
    });
    let mut runtime = CheckpointRuntime::new(seed)?;
    let data =
        MarketData::from_bytes(include_bytes!("../tests/fixtures/p2/g006-expiry-data.json"))?;
    println!("dataset={}", data.source_id());
    for record in data.events() {
        runtime.execute(SnapshotCommand::Ingest {
            stream: "demo.bars".into(),
            record: record.clone(),
        })?;
    }
    runtime.execute(SnapshotCommand::Seal {
        stream: "demo.bars".into(),
    })?;
    let mut signals = vec![];
    loop {
        match runtime.execute(SnapshotCommand::Drive)?.result? {
            CommandValue::NoWork => break,
            CommandValue::Dispatch(DispatchOutcome::Workflow {
                observation,
                windows,
                ..
            }) => {
                for signal in observation.signals {
                    println!(
                        "Long sequence={} timestamp_ns={}",
                        signal.record.source_sequence, signal.record.timestamp_ns
                    );
                    signals.push(signal.record.source_sequence);
                }
                for window in windows {
                    println!(
                        "window={} opened={} expired_ns={} satisfied={}",
                        window.window, window.opened_sequence, window.expired_ns, window.satisfied
                    );
                }
            }
            CommandValue::Dispatch(DispatchOutcome::Rejected { error, .. }) => {
                return Err(error.into());
            }
            _ => {}
        }
    }
    assert_eq!(signals, vec![8]);
    runtime
        .execute(SnapshotCommand::Finish {
            instance: "demo".into(),
        })?
        .result?;
    println!(
        "completed MACD-cross -> wait3 -> RSI; signal_count={}",
        signals.len()
    );
    Ok(())
}
