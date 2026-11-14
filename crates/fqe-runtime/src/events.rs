//! Transactional in-process event runner for the explicit P1 scalar event profile.
use crate::{RunState, RunStatus, SignalRecord};
use fqe_core::{
    Event, Signal,
    bars::{BarInput, ClosedBar},
    types::{Evaluation, ScalarValue, TypedValue},
};
use fqe_ir::{
    event_plan::{CompiledEventPlan, Trigger},
    expressions::ExpressionInstance,
    model::{ContentId, ModelError},
    registry::IncrementalInstance,
};
use std::{collections::BTreeMap, fmt, sync::Arc};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Tick(Event),
    Bar(Box<BarInput>),
}
impl From<BarInput> for InputEvent {
    fn from(value: BarInput) -> Self {
        Self::Bar(Box::new(value))
    }
}
impl From<Event> for InputEvent {
    fn from(value: Event) -> Self {
        Self::Tick(value)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunManifest {
    pub run: String,
    pub dataset_id: ContentId,
    pub artifact_id: ContentId,
    pub artifact_source_id: ContentId,
    pub execution_id: ContentId,
    pub graph_id: ContentId,
    pub registry_id: ContentId,
    pub trigger: Trigger,
    pub warmup_samples: u64,
    pub interval_convention: &'static str,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventObservation {
    pub run: String,
    pub execution_id: ContentId,
    pub source_sequence: u64,
    pub timestamp_ns: u64,
    pub matched: bool,
    pub warmup: bool,
    pub outputs: BTreeMap<String, Evaluation<TypedValue>>,
    pub signals: Vec<SignalRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionError {
    pub code: &'static str,
    pub path: String,
    pub detail: String,
}
impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}: {}", self.code, self.path, self.detail)
    }
}
impl std::error::Error for ExecutionError {}
impl From<ModelError> for ExecutionError {
    fn from(e: ModelError) -> Self {
        Self {
            code: e.code,
            path: e.path,
            detail: e.detail,
        }
    }
}
type Result<T> = std::result::Result<T, ExecutionError>;
fn error(code: &'static str, detail: &str) -> ExecutionError {
    ExecutionError {
        code,
        path: "$.event".into(),
        detail: detail.into(),
    }
}
/// Each rejected event leaves all engine-owned state/cursors unchanged; the caller may retry.
/// Trusted provider `fork` must deep-copy state; external provider side effects are unsupported.
pub struct EventRuntime {
    plan: Arc<CompiledEventPlan>,
    manifest: RunManifest,
    state: RunState,
    matched_events: u64,
    last_bar_close: Option<u64>,
    instances: Vec<IncrementalInstance>,
    roots: ExpressionInstance,
}
impl EventRuntime {
    pub fn new(
        plan: Arc<CompiledEventPlan>,
        run: impl Into<String>,
        dataset_id: ContentId,
    ) -> Result<Self> {
        let run = run.into();
        // Validate identifiers before any provider factory executes.
        let roots = ExpressionInstance::new(Arc::clone(plan.roots()), run.clone(), "roots")?;
        let instances = plan
            .stages()
            .iter()
            .enumerate()
            .map(|(i, s)| s.call.instantiate(run.clone(), format!("state{i}")))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let manifest = RunManifest {
            run,
            dataset_id,
            artifact_id: plan.artifact_id(),
            artifact_source_id: plan.source_id(),
            execution_id: plan.content_id(),
            graph_id: plan.graph().content_id(),
            registry_id: plan.graph().registry_id(),
            trigger: plan.trigger().clone(),
            warmup_samples: plan.warmup_samples(),
            interval_convention: match plan.trigger() {
                Trigger::ClosedBar { .. } => "UTC ns [open, close); timestamp = close",
                Trigger::Tick { .. } => "UTC ns timestamp; sequence resolves ties; no bar interval",
            },
        };
        Ok(Self {
            plan,
            manifest,
            state: RunState {
                status: RunStatus::Ready,
                processed_events: 0,
                last_sequence: None,
                last_timestamp_ns: None,
            },
            matched_events: 0,
            last_bar_close: None,
            instances,
            roots,
        })
    }
    pub fn manifest(&self) -> &RunManifest {
        &self.manifest
    }
    pub fn state(&self) -> &RunState {
        &self.state
    }
    pub fn matched_events(&self) -> u64 {
        self.matched_events
    }
    pub fn finish(&mut self) -> Result<RunState> {
        if self.state.status == RunStatus::Completed {
            return Err(error("event_lifecycle", "run already completed"));
        }
        self.state.status = RunStatus::Completed;
        Ok(self.state.clone())
    }
    pub fn step(&mut self, event: InputEvent) -> Result<EventObservation> {
        if self.state.status == RunStatus::Completed {
            return Err(error("event_lifecycle", "cannot step a completed run"));
        }
        let (sequence, timestamp, matched, bar_close, fields) = self.fields(event)?;
        if sequence == 0 || self.state.last_sequence.is_some_and(|s| sequence <= s) {
            return Err(error("event_sequence", "sequence must strictly increase"));
        }
        if self.state.last_timestamp_ns.is_some_and(|t| timestamp < t) {
            return Err(error("event_timestamp", "timestamp moved backwards"));
        }
        let processed_events = self
            .state
            .processed_events
            .checked_add(1)
            .ok_or_else(|| error("event_limit", "event counter overflow"))?;
        let matched_events = self
            .matched_events
            .checked_add(u64::from(matched))
            .ok_or_else(|| error("event_limit", "matched counter overflow"))?;
        let mut observation = EventObservation {
            run: self.manifest.run.clone(),
            execution_id: self.plan.content_id(),
            source_sequence: sequence,
            timestamp_ns: timestamp,
            matched,
            warmup: false,
            outputs: BTreeMap::new(),
            signals: vec![],
        };
        if matched {
            let mut staged = self
                .instances
                .iter()
                .map(IncrementalInstance::try_fork)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut roots = self.roots.clone();
            let mut slots: BTreeMap<_, _> = self
                .plan
                .roots()
                .declaration()
                .inputs
                .iter()
                .map(|p| (p.name.clone(), Evaluation::Warmup))
                .collect();
            for (slot, field) in self.plan.bindings() {
                slots.insert(slot.clone(), Evaluation::Ready(fields[field].clone()));
            }
            let mut ready = true;
            for (i, stage) in self.plan.stages().iter().enumerate() {
                let evaluated = (|| -> std::result::Result<_, ModelError> {
                    let inputs = if let Some(plan) = &stage.inputs {
                        let mut args = ExpressionInstance::new(
                            Arc::clone(plan),
                            self.manifest.run.clone(),
                            format!("args{i}"),
                        )?;
                        args.evaluate(sequence, slots.clone().into_iter().collect())?
                            .outputs
                    } else {
                        BTreeMap::new()
                    };
                    staged[i].step(sequence, inputs.into_iter().collect())
                })()
                .map_err(|e| ExecutionError {
                    code: e.code,
                    path: format!(
                        "{}{}",
                        self.plan.graph().nodes()[stage.node_index]
                            .source
                            .definition,
                        e.path.strip_prefix('$').unwrap_or(&e.path)
                    ),
                    detail: e.detail,
                })?;
                for (output, slot) in &stage.outputs {
                    let v = evaluated
                        .outputs()
                        .get(output)
                        .expect("validated provider output")
                        .clone();
                    ready &= matches!(v, Evaluation::Ready(_));
                    slots.insert(slot.clone(), v);
                }
            }
            observation.outputs = roots
                .evaluate(sequence, slots.into_iter().collect())?
                .outputs;
            observation.warmup = matched_events <= self.plan.warmup_samples() || !ready;
            if !observation.warmup {
                for rule in self.plan.signals() {
                    if let Some(Evaluation::Ready(v)) = observation.outputs.get(&rule.output)
                        && v.value() == &ScalarValue::Bool(true)
                    {
                        observation.signals.push(SignalRecord {
                            source_sequence: sequence,
                            timestamp_ns: timestamp,
                            signal: Signal {
                                instrument: self.plan.trigger().instrument().clone(),
                                kind: rule.kind,
                            },
                        });
                    }
                }
            }
            // All fallible validation and evaluation is complete before publishing any state.
            self.instances = staged;
            self.roots = roots;
        }
        self.state = RunState {
            status: RunStatus::Running,
            processed_events,
            last_sequence: Some(sequence),
            last_timestamp_ns: Some(timestamp),
        };
        self.matched_events = matched_events;
        if let Some(close) = bar_close {
            self.last_bar_close = Some(close);
        }
        Ok(observation)
    }
    fn fields(&self, event: InputEvent) -> Result<EventFields> {
        match (self.plan.trigger(), event) {
            (Trigger::Tick { instrument }, InputEvent::Tick(e)) => {
                let matched = &e.tick().instrument == instrument;
                let t = fqe_core::types::TypeDescriptor::new(
                    1,
                    fqe_core::types::ValueShape::Scalar,
                    fqe_core::types::ScalarType::U64,
                )
                .expect("u64");
                let value =
                    TypedValue::new(t, ScalarValue::U64(e.tick().price.get())).expect("u64");
                Ok((
                    e.sequence(),
                    e.timestamp_ns(),
                    matched,
                    None,
                    BTreeMap::from([("price".into(), value)]),
                ))
            }
            (
                Trigger::ClosedBar {
                    instrument,
                    timeframe_ns,
                    price_scale,
                    volume_scale,
                },
                InputEvent::Bar(raw),
            ) => {
                let b = ClosedBar::new(*raw).map_err(|e| error("event_bar", &e.to_string()))?;
                let raw = b.input();
                if &raw.instrument != instrument || raw.timeframe_ns != *timeframe_ns {
                    return Err(error(
                        "event_domain",
                        "instrument/timeframe differs from trigger",
                    ));
                }
                if b.price_type().scalar()
                    != &(fqe_core::types::ScalarType::Decimal {
                        scale: *price_scale,
                        unit: fqe_core::types::NumericUnit::Price(instrument.clone()),
                    })
                    || b.volume_type().scalar()
                        != &(fqe_core::types::ScalarType::Decimal {
                            scale: *volume_scale,
                            unit: fqe_core::types::NumericUnit::Quantity(instrument.clone()),
                        })
                {
                    return Err(error("event_type", "bar scales differ from trigger"));
                }
                if self
                    .last_bar_close
                    .is_some_and(|close| raw.open_ns != close)
                {
                    return Err(error(
                        "event_interval",
                        "bar must start at previous close; no gaps or overlap",
                    ));
                }
                Ok((
                    raw.sequence,
                    raw.timestamp_ns,
                    true,
                    Some(raw.close_ns),
                    BTreeMap::from([
                        ("open".into(), raw.open.clone()),
                        ("high".into(), raw.high.clone()),
                        ("low".into(), raw.low.clone()),
                        ("close".into(), raw.close.clone()),
                        ("volume".into(), raw.volume.clone()),
                    ]),
                ))
            }
            _ => Err(error(
                "event_domain",
                "input event family differs from trigger",
            )),
        }
    }
}
type EventFields = (u64, u64, bool, Option<u64>, BTreeMap<String, TypedValue>);
