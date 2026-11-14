//! Whole-event transactions over base providers/crossings and host-owned stateful rule modules.
use crate::{
    SignalRecord,
    events::{EventObservation, EventRuntime, ExecutionError, InputEvent},
};
use fqe_core::{
    Signal,
    types::{Evaluation, ScalarValue, TypedValue},
};
use fqe_ir::{
    expressions::{ExpressionInstance, ExpressionPlan},
    model::ContentId,
    workflow::{self, CompiledWorkflow, InstanceValues, Source},
};
use std::{collections::BTreeMap, sync::Arc};
type Result<T> = std::result::Result<T, ExecutionError>;
type Values = BTreeMap<String, Evaluation<TypedValue>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowStatus {
    Ready,
    Running,
    Completed,
    Stopped,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowState {
    pub status: WorkflowStatus,
    pub processed_events: u64,
    pub matched_events: u64,
    pub last_sequence: Option<u64>,
    pub last_timestamp_ns: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowManifest {
    pub run: String,
    pub dataset_id: ContentId,
    pub artifact_id: ContentId,
    pub artifact_source_id: ContentId,
    pub execution_id: ContentId,
    pub event_execution_id: ContentId,
    pub registry_id: ContentId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowObservation {
    pub run: String,
    pub execution_id: ContentId,
    pub source_sequence: u64,
    pub timestamp_ns: u64,
    pub matched: bool,
    pub warmup: bool,
    pub event_outputs: Values,
    pub outputs: BTreeMap<String, Values>,
    pub transitions: BTreeMap<String, Vec<String>>,
    pub signals: Vec<WorkflowSignal>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowSignal {
    pub instance: String,
    pub output: String,
    pub record: SignalRecord,
}
fn error(code: &'static str, path: &str, detail: &str) -> ExecutionError {
    ExecutionError {
        code,
        path: path.into(),
        detail: detail.into(),
    }
}
fn evaluate(
    plan: &Arc<ExpressionPlan>,
    run: &str,
    sequence: u64,
    slots: &Values,
    path: &str,
) -> Result<Values> {
    let mut instance = ExpressionInstance::new(Arc::clone(plan), run, "workflow")?;
    instance
        .evaluate(sequence, slots.clone().into_iter().collect())
        .map(|r| r.outputs)
        .map_err(|e| {
            error(
                e.code,
                &format!("{path}{}", e.path.strip_prefix('$').unwrap_or(&e.path)),
                &e.detail,
            )
        })
}
pub struct WorkflowRuntime {
    plan: Arc<CompiledWorkflow>,
    base: Option<EventRuntime>,
    manifest: WorkflowManifest,
    state: WorkflowState,
    values: BTreeMap<String, InstanceValues>,
}
impl WorkflowRuntime {
    pub fn new(
        plan: Arc<CompiledWorkflow>,
        run: impl Into<String>,
        dataset_id: ContentId,
    ) -> Result<Self> {
        let run = run.into();
        let values = plan
            .instances()
            .iter()
            .map(|i| (i.id.clone(), i.initial.clone()))
            .collect();
        // Validate run/portable-state budget before any provider factory is called.
        workflow::state_view(plan.content_id(), &run, &values)?;
        let base = EventRuntime::new(Arc::clone(plan.event()), run.clone(), dataset_id)?;
        let manifest = WorkflowManifest {
            run,
            dataset_id,
            artifact_id: plan.artifact_id(),
            artifact_source_id: plan.source_id(),
            execution_id: plan.content_id(),
            event_execution_id: plan.event().content_id(),
            registry_id: plan.event().graph().registry_id(),
        };
        Ok(Self {
            plan,
            base: Some(base),
            manifest,
            state: WorkflowState {
                status: WorkflowStatus::Ready,
                processed_events: 0,
                matched_events: 0,
                last_sequence: None,
                last_timestamp_ns: None,
            },
            values,
        })
    }
    pub fn manifest(&self) -> &WorkflowManifest {
        &self.manifest
    }
    pub fn state(&self) -> &WorkflowState {
        &self.state
    }
    pub fn instance_values(&self) -> &BTreeMap<String, InstanceValues> {
        &self.values
    }
    pub fn inspect_state(&self) -> Result<Vec<u8>> {
        Ok(workflow::state_view(
            self.plan.content_id(),
            &self.manifest.run,
            &self.values,
        )?)
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>> {
        Err(error(
            "workflow_capability",
            "$.checkpoint",
            "complete provider/crossing/workflow checkpoint unsupported; state view is inspection only",
        ))
    }
    fn active(&self) -> Result<()> {
        if matches!(
            self.state.status,
            WorkflowStatus::Ready | WorkflowStatus::Running
        ) {
            Ok(())
        } else {
            Err(error("workflow_lifecycle", "$.run", "run is terminal"))
        }
    }
    pub fn finish(&mut self) -> Result<WorkflowState> {
        self.active()?;
        self.base.as_mut().expect("active base").finish()?;
        self.state.status = WorkflowStatus::Completed;
        self.base = None;
        Ok(self.state.clone())
    }
    pub fn stop(&mut self) -> Result<WorkflowState> {
        self.active()?;
        self.state.status = WorkflowStatus::Stopped;
        self.base = None;
        Ok(self.state.clone())
    }
    /// Explicit host abort is terminal; rejected event errors themselves remain retryable.
    pub fn abort(&mut self) -> Result<WorkflowState> {
        self.active()?;
        self.state.status = WorkflowStatus::Failed;
        self.base = None;
        Ok(self.state.clone())
    }
    /// A fresh run with initial cells/provider state, never resume or reuse of an output identity.
    pub fn restart(&self, new_run: impl Into<String>) -> Result<Self> {
        if matches!(
            self.state.status,
            WorkflowStatus::Ready | WorkflowStatus::Running
        ) {
            return Err(error(
                "workflow_lifecycle",
                "$.run",
                "stop/finish/abort before restart",
            ));
        }
        let new_run = new_run.into();
        if new_run == self.manifest.run {
            return Err(error(
                "workflow_lifecycle",
                "$.run",
                "fresh restart needs a new run ID",
            ));
        }
        Self::new(Arc::clone(&self.plan), new_run, self.manifest.dataset_id)
    }
    pub fn step(&mut self, event: InputEvent) -> Result<WorkflowObservation> {
        self.active()?;
        let (base, source) = self.base.as_ref().expect("active base").fork_step(event)?;
        self.accept(base, source)
    }
    /// Private candidate for a host transaction that also owns temporal state.
    pub(crate) fn fork_step(&self, event: InputEvent) -> Result<(Self, WorkflowObservation)> {
        self.active()?;
        let (base, source) = self.base.as_ref().expect("active base").fork_step(event)?;
        let mut candidate = Self {
            plan: Arc::clone(&self.plan),
            base: None,
            manifest: self.manifest.clone(),
            state: self.state.clone(),
            values: self.values.clone(),
        };
        let observation = candidate.accept(base, source)?;
        Ok((candidate, observation))
    }
    fn accept(
        &mut self,
        base: EventRuntime,
        source: EventObservation,
    ) -> Result<WorkflowObservation> {
        let mut staged = self.values.clone();
        let mut outputs = BTreeMap::<String, Values>::new();
        let mut transitions = BTreeMap::new();
        let mut warmup = source.warmup;
        if source.matched {
            for prepared in self.plan.instances() {
                let old = &self.values[&prepared.id];
                let mut slots = Values::new();
                for (name, binding) in &prepared.bindings {
                    let value = match binding {
                        Source::Event(name) => &source.outputs[name],
                        Source::Instance { instance, output } => &outputs[instance][output],
                    };
                    warmup |= !matches!(value, Evaluation::Ready(_));
                    slots.insert(format!("input.{name}"), value.clone());
                }
                for (name, value) in &old.state {
                    slots.insert(format!("state.{name}"), Evaluation::Ready(value.clone()));
                }
                for (name, value) in &old.delays {
                    warmup |= !matches!(value, Evaluation::Ready(_));
                    slots.insert(format!("delay.{name}"), value.clone());
                }
                slots.insert(
                    "mode".into(),
                    Evaluation::Ready(workflow::mode_value(&old.mode)?),
                );
                for transition in &prepared.transitions {
                    slots.insert(
                        format!("fired.{}", transition.id),
                        Evaluation::Ready(workflow::boolean(false)),
                    );
                }
                let mut enabled = Vec::new();
                for transition in &prepared.transitions {
                    if !transition.from.contains(&old.mode) {
                        continue;
                    }
                    let guard = evaluate(
                        &transition.guard,
                        &self.manifest.run,
                        source.source_sequence,
                        &slots,
                        &transition.path,
                    )?;
                    match &guard["guard"] {
                        Evaluation::Ready(v) if v.value() == &ScalarValue::Bool(true) => {
                            enabled.push(transition)
                        }
                        Evaluation::Ready(_) => {}
                        _ => warmup = true,
                    }
                }
                // Every guard saw old state and false fired flags. All effects now see the same selected set.
                for transition in &enabled {
                    slots.insert(
                        format!("fired.{}", transition.id),
                        Evaluation::Ready(workflow::boolean(true)),
                    );
                }
                let next = staged.get_mut(&prepared.id).expect("instance state");
                let mut writes = Values::new();
                let mut target = None;
                for transition in &enabled {
                    if let Some(mode) = &transition.to
                        && target.replace(mode.clone()).is_some()
                    {
                        return Err(error(
                            "workflow_transition_conflict",
                            &transition.path,
                            "multiple control transitions enabled",
                        ));
                    }
                    if let Some(plan) = &transition.writes {
                        for (name, value) in evaluate(
                            plan,
                            &self.manifest.run,
                            source.source_sequence,
                            &slots,
                            &transition.path,
                        )? {
                            if writes.insert(name, value).is_some() {
                                return Err(error(
                                    "workflow_write_conflict",
                                    &transition.path,
                                    "multiple enabled effects write the same state",
                                ));
                            }
                        }
                    }
                }
                for (name, value) in writes {
                    let Evaluation::Ready(value) = value else {
                        return Err(error(
                            "workflow_write_unready",
                            &prepared.path,
                            "state writes require a ready typed value",
                        ));
                    };
                    next.state.insert(name, value);
                }
                if let Some(mode) = target {
                    next.mode = mode;
                }
                if let Some(plan) = &prepared.delays {
                    next.delays = evaluate(
                        plan,
                        &self.manifest.run,
                        source.source_sequence,
                        &slots,
                        &prepared.path,
                    )?;
                }
                let current = evaluate(
                    &prepared.outputs,
                    &self.manifest.run,
                    source.source_sequence,
                    &slots,
                    &prepared.path,
                )?;
                warmup |= current.values().any(|v| !matches!(v, Evaluation::Ready(_)));
                outputs.insert(prepared.id.clone(), current);
                transitions.insert(
                    prepared.id.clone(),
                    enabled.iter().map(|t| t.id.clone()).collect(),
                );
            }
        }
        // Bound changing text/state values before publishing provider/crossing/cell/cursor changes.
        workflow::state_view(self.plan.content_id(), &self.manifest.run, &staged)?;
        let mut signals = Vec::new();
        if source.matched && !warmup {
            for rule in self.plan.signals() {
                if matches!(&outputs[&rule.instance][&rule.output],Evaluation::Ready(v) if v.value()==&ScalarValue::Bool(true))
                {
                    signals.push(WorkflowSignal {
                        instance: rule.instance.clone(),
                        output: rule.output.clone(),
                        record: SignalRecord {
                            source_sequence: source.source_sequence,
                            timestamp_ns: source.timestamp_ns,
                            signal: Signal {
                                instrument: self.plan.event().trigger().instrument().clone(),
                                kind: rule.kind,
                            },
                        },
                    });
                }
            }
        }
        let observation = WorkflowObservation {
            run: self.manifest.run.clone(),
            execution_id: self.plan.content_id(),
            source_sequence: source.source_sequence,
            timestamp_ns: source.timestamp_ns,
            matched: source.matched,
            warmup,
            event_outputs: source.outputs,
            outputs,
            transitions,
            signals,
        };
        self.state = WorkflowState {
            status: WorkflowStatus::Running,
            processed_events: base.state().processed_events,
            matched_events: base.matched_events(),
            last_sequence: base.state().last_sequence,
            last_timestamp_ns: base.state().last_timestamp_ns,
        };
        self.values = staged;
        self.base = Some(base);
        Ok(observation)
    }
}
