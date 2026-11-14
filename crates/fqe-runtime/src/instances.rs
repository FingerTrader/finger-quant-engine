//! Bounded workflow lanes with deterministic event-time dispatch and explicit failed-lane quarantine.
use crate::{
    events::{ExecutionError, InputEvent},
    schedule::{Alignment, MarketScheduler, ScheduledEvent},
    temporal::{TemporalBook, WindowNotice, WindowState},
    workflow::{WorkflowManifest, WorkflowObservation, WorkflowRuntime, WorkflowState},
};
use fqe_ir::{
    model::{ContentId, identifier},
    temporal::BoundTemporalRules,
    workflow::CompiledWorkflow,
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
type Result<T> = std::result::Result<T, ExecutionError>;
pub const MAX_INSTANCES: usize = 64;
pub const MAX_PENDING: usize = 4096;
fn error(code: &'static str, path: &str, detail: &str) -> ExecutionError {
    ExecutionError {
        code,
        path: path.into(),
        detail: detail.into(),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneStatus {
    Active,
    Blocked,
    Stopped,
    Completed,
}
struct Delivery {
    event: ScheduledEvent,
    alignment: Arc<Alignment>,
}
struct Lane {
    manifest: LaneManifest,
    stream: String,
    runtime: WorkflowRuntime,
    book: Option<TemporalBook>,
    queue: VecDeque<Arc<Delivery>>,
    status: LaneStatus,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceLimits {
    pub instances: usize,
    pub pending_total: usize,
    pub pending_per_instance: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneManifest {
    pub host_run: String,
    pub instance: String,
    pub stream: String,
    pub schedule_id: ContentId,
    pub workflow: WorkflowManifest,
    pub temporal_artifact_id: Option<ContentId>,
    pub temporal_source_id: Option<ContentId>,
    pub temporal_execution_id: Option<ContentId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchOutcome {
    Market {
        logical_sequence: u64,
        recipients: usize,
    },
    Workflow {
        instance: String,
        observation: WorkflowObservation,
        windows: Vec<WindowNotice>,
        alignment: Arc<Alignment>,
    },
    Timer {
        instance: String,
        notice: WindowNotice,
    },
    Rejected {
        instance: String,
        error: ExecutionError,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneClosure {
    pub state: WorkflowState,
    pub discarded_deliveries: usize,
    pub incomplete_windows: Vec<WindowState>,
}
pub struct InstanceScheduler {
    market: MarketScheduler,
    limits: InstanceLimits,
    lanes: BTreeMap<String, Lane>,
    pending: usize,
    last_lane: Option<String>,
}
impl InstanceScheduler {
    pub fn new(market: MarketScheduler, limits: InstanceLimits) -> Result<Self> {
        if limits.instances == 0
            || limits.instances > MAX_INSTANCES
            || limits.pending_total == 0
            || limits.pending_total > MAX_PENDING
            || limits.pending_per_instance == 0
            || limits.pending_per_instance > limits.pending_total
        {
            return Err(error(
                "instance_limit",
                "$.limits",
                "instances1..64, total1..4096, per-instance1..total required",
            ));
        }
        if market.state().accepted_events != 0 || market.state().published_events != 0 {
            return Err(error(
                "instance_lifecycle",
                "$.market",
                "market already accepted input or published",
            ));
        }
        Ok(Self {
            market,
            limits,
            lanes: BTreeMap::new(),
            pending: 0,
            last_lane: None,
        })
    }
    pub fn market(&self) -> &MarketScheduler {
        &self.market
    }
    pub fn limits(&self) -> &InstanceLimits {
        &self.limits
    }
    pub fn manifest(&self, id: &str) -> Option<&LaneManifest> {
        self.lanes.get(id).map(|l| &l.manifest)
    }
    pub fn pending(&self) -> usize {
        self.pending
    }
    pub fn status(&self, id: &str) -> Option<LaneStatus> {
        self.lanes.get(id).map(|v| v.status)
    }
    pub fn runtime(&self, id: &str) -> Option<&WorkflowRuntime> {
        self.lanes.get(id).map(|v| &v.runtime)
    }
    pub fn windows(&self, id: &str) -> Option<&BTreeMap<String, WindowState>> {
        self.lanes
            .get(id)
            .and_then(|v| v.book.as_ref())
            .map(TemporalBook::active)
    }
    pub fn attach(
        &mut self,
        id: &str,
        stream: &str,
        plan: Arc<CompiledWorkflow>,
        run: &str,
        rules: Option<Arc<BoundTemporalRules>>,
    ) -> Result<()> {
        identifier(id, "$.instance")?;
        if self.market.state().accepted_events != 0 || self.market.state().published_events != 0 {
            return Err(error(
                "instance_lifecycle",
                id,
                "attach all instances before ingress/publication",
            ));
        }
        if self.lanes.contains_key(id)
            || self.lanes.len() >= self.limits.instances
            || self.lanes.values().any(|l| l.runtime.manifest().run == run)
        {
            return Err(error(
                "instance_limit",
                id,
                "duplicate instance/run or instance capacity exceeded",
            ));
        }
        let spec = self
            .market
            .definition()
            .streams()
            .get(stream)
            .ok_or_else(|| error("instance_stream", id, "unknown source stream"))?;
        if plan.event().trigger() != spec.domain()
            || rules
                .as_ref()
                .is_some_and(|r| r.workflow_id() != plan.content_id())
        {
            return Err(error(
                "instance_binding",
                id,
                "trigger or bound temporal workflow identity differs",
            ));
        }
        let runtime = WorkflowRuntime::new(plan, run, spec.dataset_id())?;
        let manifest = LaneManifest {
            host_run: self.market.run().into(),
            instance: id.into(),
            stream: stream.into(),
            schedule_id: self.market.definition().content_id(),
            workflow: runtime.manifest().clone(),
            temporal_artifact_id: rules.as_ref().map(|r| r.artifact_id()),
            temporal_source_id: rules.as_ref().map(|r| r.source_id()),
            temporal_execution_id: rules.as_ref().map(|r| r.content_id()),
        };
        self.lanes.insert(
            id.into(),
            Lane {
                manifest,
                stream: stream.into(),
                runtime,
                book: rules.map(TemporalBook::new),
                queue: VecDeque::new(),
                status: LaneStatus::Active,
            },
        );
        Ok(())
    }
    pub fn ingest(&mut self, stream: &str, event: InputEvent) -> Result<()> {
        self.market.ingest(stream, event)
    }
    pub fn advance_watermark(&mut self, stream: &str, time: u64) -> Result<()> {
        self.market.advance_watermark(stream, time)
    }
    pub fn seal(&mut self, stream: &str) -> Result<()> {
        self.market.seal(stream)
    }
    pub fn retry(&mut self, id: &str) -> Result<()> {
        let lane = self
            .lanes
            .get_mut(id)
            .ok_or_else(|| error("instance_unknown", id, "unknown instance"))?;
        if lane.status != LaneStatus::Blocked {
            return Err(error(
                "instance_lifecycle",
                id,
                "only a blocked lane can retry",
            ));
        }
        lane.status = LaneStatus::Active;
        Ok(())
    }
    pub fn cancel(&mut self, id: &str) -> Result<LaneClosure> {
        let lane = self
            .lanes
            .get_mut(id)
            .ok_or_else(|| error("instance_unknown", id, "unknown instance"))?;
        if !matches!(lane.status, LaneStatus::Active | LaneStatus::Blocked) {
            return Err(error("instance_lifecycle", id, "lane terminal"));
        }
        let state = lane.runtime.stop()?;
        lane.status = LaneStatus::Stopped;
        let discarded_deliveries = lane.queue.len();
        self.pending -= discarded_deliveries;
        lane.queue.clear();
        let incomplete_windows = lane
            .book
            .as_mut()
            .map(TemporalBook::clear)
            .unwrap_or_default();
        Ok(LaneClosure {
            state,
            discarded_deliveries,
            incomplete_windows,
        })
    }
    pub fn finish(&mut self, id: &str) -> Result<LaneClosure> {
        let lane = self
            .lanes
            .get_mut(id)
            .ok_or_else(|| error("instance_unknown", id, "unknown instance"))?;
        if lane.status != LaneStatus::Active
            || !self.market.is_finished()
            || !lane.queue.is_empty()
            || lane.book.as_ref().is_some_and(|b| b.next_due().is_some())
        {
            return Err(error(
                "instance_lifecycle",
                id,
                "finish after sealed/drained market, pending jobs and timers; blocked lanes require retry/cancel",
            ));
        }
        let state = lane.runtime.finish()?;
        lane.status = LaneStatus::Completed;
        let incomplete_windows = lane
            .book
            .as_mut()
            .map(TemporalBook::clear)
            .unwrap_or_default();
        Ok(LaneClosure {
            state,
            discarded_deliveries: 0,
            incomplete_windows,
        })
    }
    fn pump(&mut self) -> Result<Option<DispatchOutcome>> {
        let lanes = &mut self.lanes;
        let pending = &mut self.pending;
        let limits = &self.limits;
        self.market.dispatch(|event, alignment| {
            let ids = lanes
                .iter()
                .filter(|(_, l)| {
                    l.stream == event.stream
                        && matches!(l.status, LaneStatus::Active | LaneStatus::Blocked)
                })
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            if ids.len() > limits.pending_total.saturating_sub(*pending)
                || ids
                    .iter()
                    .any(|id| lanes[id].queue.len() >= limits.pending_per_instance)
            {
                return Err(error(
                    "instance_backpressure",
                    "$.delivery",
                    "fanout capacity full; drain, retry or cancel blocked lane",
                ));
            }
            let delivery = Arc::new(Delivery {
                event: event.clone(),
                alignment: Arc::new(alignment.clone()),
            });
            for id in &ids {
                lanes
                    .get_mut(id)
                    .expect("recipient")
                    .queue
                    .push_back(Arc::clone(&delivery));
            }
            *pending += ids.len();
            Ok(DispatchOutcome::Market {
                logical_sequence: event.logical_sequence,
                recipients: ids.len(),
            })
        })
    }
    /// One bounded quantum. Existing same-time jobs precede new market records; timers come last.
    pub fn drive_one(&mut self) -> Result<Option<DispatchOutcome>> {
        let watermark = self.market.watermark_ns();
        let mut candidates = Vec::new();
        for (id, lane) in &self.lanes {
            if lane.status != LaneStatus::Active {
                continue;
            }
            if let Some(delivery) = lane.queue.front() {
                candidates.push((delivery.event.timestamp_ns, 0_u8, id.clone(), None));
            }
            if let Some((time, window)) = lane.book.as_ref().and_then(TemporalBook::next_due)
                && watermark.is_some_and(|w| time <= w)
                && lane
                    .queue
                    .front()
                    .is_none_or(|d| d.event.timestamp_ns > time)
            {
                candidates.push((time, 2, id.clone(), Some(window)));
            }
        }
        let lane_key = candidates.iter().map(|(t, p, _, _)| (*t, *p)).min();
        let market_time = self.market.next_timestamp_ns();
        if market_time.is_some_and(|t| lane_key.is_none_or(|(lt, phase)| (t, 1) < (lt, phase))) {
            return self.pump();
        }
        let Some(key) = lane_key else { return Ok(None) };
        candidates.retain(|(t, p, _, _)| (*t, *p) == key);
        candidates.sort_by(|a, b| a.2.cmp(&b.2));
        let selected = candidates
            .iter()
            .position(|(_, _, id, _)| self.last_lane.as_ref().is_none_or(|last| id > last))
            .unwrap_or(0);
        let (time, _, id, window) = candidates.swap_remove(selected);
        self.last_lane = Some(id.clone());
        let lane = self.lanes.get_mut(&id).expect("candidate lane");
        if let Some(window) = window {
            let notice = lane
                .book
                .as_mut()
                .expect("timer book")
                .expire(time, &window)?;
            return Ok(Some(DispatchOutcome::Timer {
                instance: id,
                notice,
            }));
        }
        let delivery = lane.queue.front().expect("pending job");
        let alignment = Arc::clone(&delivery.alignment);
        // The immutable alignment snapshot is retained with this source event, never replaced by later ingress.
        debug_assert_eq!(
            delivery.alignment.timestamp_ns(),
            delivery.event.timestamp_ns
        );
        let staged = (|| {
            let (runtime, observation) = lane.runtime.fork_step(delivery.event.event.clone())?;
            let mut book = lane.book.clone();
            let windows = if let Some(book) = &mut book {
                book.advance(&observation)?
            } else {
                vec![]
            };
            Ok::<_, ExecutionError>((runtime, book, observation, windows))
        })();
        match staged {
            Ok((runtime, book, observation, windows)) => {
                lane.runtime = runtime;
                lane.book = book;
                lane.queue.pop_front();
                self.pending -= 1;
                Ok(Some(DispatchOutcome::Workflow {
                    instance: id,
                    observation,
                    windows,
                    alignment,
                }))
            }
            Err(error) => {
                lane.status = LaneStatus::Blocked;
                Ok(Some(DispatchOutcome::Rejected {
                    instance: id,
                    error,
                }))
            }
        }
    }
}
