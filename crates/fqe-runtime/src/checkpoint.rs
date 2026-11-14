//! Bounded deterministic reconstruction checkpoints; historical outputs are verified and suppressed.
use crate::{
    events::{ExecutionError, InputEvent},
    instances::{DispatchOutcome, InstanceLimits, InstanceScheduler, LaneClosure},
    schedule::MarketScheduler,
    workflow::WorkflowRuntime,
};
use fqe_ir::{
    checkpoint::{self, JournalEntry, SnapshotCommand, SnapshotDocument, SnapshotPins},
    exchange::MarketRecord,
    model::{ContentId, identifier},
    schedule::ScheduleDefinition,
    temporal::BoundTemporalRules,
    workflow::CompiledWorkflow,
};
use std::{collections::BTreeSet, sync::Arc};
type Result<T> = std::result::Result<T, ExecutionError>;
fn error(code: &'static str, path: &str, detail: &str) -> ExecutionError {
    ExecutionError {
        code,
        path: path.into(),
        detail: detail.into(),
    }
}
#[derive(Clone)]
pub struct LaneSeed {
    pub id: String,
    pub stream: String,
    pub run: String,
    pub workflow: Arc<CompiledWorkflow>,
    pub temporal: Option<Arc<BoundTemporalRules>>,
}
#[derive(Clone)]
pub struct CheckpointSeed {
    pub schedule: Arc<ScheduleDefinition>,
    pub host_run: String,
    pub limits: InstanceLimits,
    pub lanes: Vec<LaneSeed>,
}
impl CheckpointSeed {
    fn pins(&self) -> Result<SnapshotPins> {
        identifier(&self.host_run, "$.host_run")?;
        // Validate limits without creating a strategy/provider instance.
        let market = MarketScheduler::new(Arc::clone(&self.schedule), self.host_run.clone())?;
        InstanceScheduler::new(market, self.limits.clone())?;
        if self.lanes.len() > self.limits.instances {
            return Err(error(
                "checkpoint_limit",
                "$.lanes",
                "lane capacity exceeded",
            ));
        }
        let mut pins = SnapshotPins::new();
        let mut put = |path: Vec<String>, value: String| {
            pins.insert(path, value);
        };
        put(
            vec!["engine".into()],
            crate::source_identity::engine_id().to_string(),
        );
        put(
            vec!["profile".into()],
            "fqe.checkpoint-replay/1;owned-debug-outcomes/1".into(),
        );
        put(vec!["host-run".into()], self.host_run.clone());
        put(
            vec!["schedule".into(), "artifact".into()],
            self.schedule.content_id().to_string(),
        );
        put(
            vec!["schedule".into(), "source".into()],
            self.schedule.source_id().to_string(),
        );
        for (k, v) in [
            ("instances", self.limits.instances),
            ("pending-total", self.limits.pending_total),
            ("pending-per-instance", self.limits.pending_per_instance),
        ] {
            put(vec!["limits".into(), k.into()], v.to_string());
        }
        for (id, spec) in self.schedule.streams() {
            put(
                vec!["stream".into(), id.clone(), "dataset".into()],
                spec.dataset_id().to_string(),
            );
        }
        let mut ids = BTreeSet::new();
        let mut runs = BTreeSet::new();
        for lane in &self.lanes {
            identifier(&lane.id, "$.lane.id")?;
            identifier(&lane.run, "$.lane.run")?;
            if !ids.insert(&lane.id) || !runs.insert(&lane.run) {
                return Err(error(
                    "checkpoint_duplicate",
                    "$.lanes",
                    "duplicate lane/run",
                ));
            }
            let spec = self
                .schedule
                .streams()
                .get(&lane.stream)
                .ok_or_else(|| error("checkpoint_stream", &lane.id, "unknown stream"))?;
            if lane.workflow.event().trigger() != spec.domain()
                || lane
                    .temporal
                    .as_ref()
                    .is_some_and(|t| t.workflow_id() != lane.workflow.content_id())
            {
                return Err(error(
                    "checkpoint_binding",
                    &lane.id,
                    "trigger/temporal binding differs",
                ));
            }
            let path = |field: &str| vec!["lane".into(), lane.id.clone(), field.into()];
            for (k, v) in [
                ("run", lane.run.clone()),
                ("stream", lane.stream.clone()),
                ("workflow-execution", lane.workflow.content_id().to_string()),
                ("workflow-artifact", lane.workflow.artifact_id().to_string()),
                ("workflow-source", lane.workflow.source_id().to_string()),
                (
                    "registry",
                    lane.workflow.event().graph().registry_id().to_string(),
                ),
            ] {
                put(path(k), v);
            }
            if let Some(temporal) = &lane.temporal {
                for (k, v) in [
                    ("temporal-execution", temporal.content_id()),
                    ("temporal-artifact", temporal.artifact_id()),
                    ("temporal-source", temporal.source_id()),
                ] {
                    put(path(k), v.to_string());
                }
            } else {
                put(path("temporal-execution"), "none".into());
            }
            for instance in lane.workflow.instances() {
                let p = |field: &str| {
                    vec![
                        "lane".into(),
                        lane.id.clone(),
                        "module".into(),
                        instance.id.clone(),
                        field.into(),
                    ]
                };
                put(p("id"), instance.module.id.clone());
                put(p("version"), instance.module.version.clone());
                put(p("state-schema-id"), instance.state_schema.id.clone());
                put(
                    p("state-schema-version"),
                    instance.state_schema.version.clone(),
                );
            }
            for node in lane.workflow.event().graph().nodes() {
                if !node.call.permits_checkpoint_replay() {
                    return Err(error(
                        "checkpoint_capability",
                        &node.source.definition,
                        "provider has no explicit deterministic checkpoint-replay permit",
                    ));
                }
                let descriptor = node.call.descriptor();
                let d = descriptor.declaration();
                let p = |field: &str| {
                    let mut p = vec!["lane".into(), lane.id.clone(), "provider".into()];
                    p.extend(node.path.iter().cloned());
                    p.push(field.into());
                    p
                };
                for (k, v) in [
                    ("id", d.key.id.clone()),
                    ("version", d.key.version.clone()),
                    ("artifact", d.artifact.to_string()),
                    ("descriptor", descriptor.content_id().to_string()),
                    (
                        "parameters",
                        checkpoint::parameters_id(node.call.parameters()).to_string(),
                    ),
                    (
                        "state-schema",
                        d.state_schema.clone().unwrap_or_else(|| "none".into()),
                    ),
                    ("capability", "checkpoint.replay/1".into()),
                ] {
                    put(p(k), v);
                }
            }
        }
        SnapshotDocument {
            pins: pins.clone(),
            entries: vec![],
        }
        .to_bytes()?;
        Ok(pins)
    }
    fn build(&self) -> Result<InstanceScheduler> {
        let market = MarketScheduler::new(Arc::clone(&self.schedule), self.host_run.clone())?;
        let mut host = InstanceScheduler::new(market, self.limits.clone())?;
        for lane in &self.lanes {
            host.attach(
                &lane.id,
                &lane.stream,
                Arc::clone(&lane.workflow),
                &lane.run,
                lane.temporal.clone(),
            )?;
        }
        Ok(host)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandValue {
    Accepted,
    NoWork,
    Dispatch(DispatchOutcome),
    Closed(LaneClosure),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandReceipt {
    pub command_index: u64,
    pub output_cursor: u64,
    pub output_sequence: Option<u64>,
    pub result: std::result::Result<CommandValue, ExecutionError>,
}
pub struct CheckpointRuntime {
    seed: Arc<CheckpointSeed>,
    host: InstanceScheduler,
    document: SnapshotDocument,
}
impl CheckpointRuntime {
    pub fn new(seed: Arc<CheckpointSeed>) -> Result<Self> {
        let pins = seed.pins()?;
        let host = seed.build()?;
        Ok(Self {
            seed,
            host,
            document: SnapshotDocument {
                pins,
                entries: vec![],
            },
        })
    }
    pub fn host(&self) -> &InstanceScheduler {
        &self.host
    }
    pub fn seed(&self) -> &CheckpointSeed {
        &self.seed
    }
    pub fn output_cursor(&self) -> u64 {
        self.document.output_cursor()
    }
    pub fn command_count(&self) -> usize {
        self.document.entries.len()
    }
    pub fn workflow(&self, id: &str) -> Option<&WorkflowRuntime> {
        self.host.runtime(id)
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>> {
        Ok(self.document.to_bytes()?)
    }
    pub fn execute(&mut self, command: SnapshotCommand) -> Result<CommandReceipt> {
        let cursor = self.output_cursor();
        let mut reserved = self.document.clone();
        reserved.entries.push(JournalEntry {
            command: command.clone(),
            outcome_id: ContentId::from_bytes([0; 32]),
            output_cursor: cursor + 1,
        });
        reserved.to_bytes()?;
        let result = apply(&mut self.host, &command);
        let produced = matches!(
            &result,
            Ok(CommandValue::Dispatch(_) | CommandValue::Closed(_))
        );
        let output_cursor = cursor + u64::from(produced);
        let outcome_id = checkpoint::observation_id(format!("{result:?}").as_bytes());
        self.document.entries.push(JournalEntry {
            command,
            outcome_id,
            output_cursor,
        });
        Ok(CommandReceipt {
            command_index: self.document.entries.len() as u64,
            output_cursor,
            output_sequence: produced.then_some(output_cursor),
            result,
        })
    }
    /// Validate representation/integrity/compatibility/capabilities before creating provider state.
    /// Replay receipts stay private; the caller receives only the restored continuation.
    pub fn restore(seed: Arc<CheckpointSeed>, bytes: &[u8]) -> Result<Self> {
        let saved = SnapshotDocument::from_bytes(bytes)?;
        if saved.pins != seed.pins()? {
            return Err(error(
                "checkpoint_incompatible",
                "$.pins",
                "engine/artifact/parameters/schema/registry/data/temporal/limits identity differs",
            ));
        }
        let mut restored = Self::new(seed)?;
        for (i, expected) in saved.entries.iter().enumerate() {
            restored.execute(expected.command.clone())?;
            if restored.document.entries.last() != Some(expected) {
                return Err(error(
                    "checkpoint_divergence",
                    &format!("$.journal[{i}]"),
                    "reconstructed outcome/output cursor differs",
                ));
            }
        }
        Ok(restored)
    }
}
fn apply(host: &mut InstanceScheduler, command: &SnapshotCommand) -> Result<CommandValue> {
    match command {
        SnapshotCommand::Ingest { stream, record } => {
            let event = match record {
                MarketRecord::Tick(e) => InputEvent::Tick(e.clone()),
                MarketRecord::Bar(b) => InputEvent::from(b.input().clone()),
            };
            host.ingest(stream, event)?;
            Ok(CommandValue::Accepted)
        }
        SnapshotCommand::Watermark {
            stream,
            timestamp_ns,
        } => {
            host.advance_watermark(stream, *timestamp_ns)?;
            Ok(CommandValue::Accepted)
        }
        SnapshotCommand::Seal { stream } => {
            host.seal(stream)?;
            Ok(CommandValue::Accepted)
        }
        SnapshotCommand::Drive => Ok(host
            .drive_one()?
            .map_or(CommandValue::NoWork, CommandValue::Dispatch)),
        SnapshotCommand::Retry { instance } => {
            host.retry(instance)?;
            Ok(CommandValue::Accepted)
        }
        SnapshotCommand::Cancel { instance } => Ok(CommandValue::Closed(host.cancel(instance)?)),
        SnapshotCommand::Finish { instance } => Ok(CommandValue::Closed(host.finish(instance)?)),
    }
}
