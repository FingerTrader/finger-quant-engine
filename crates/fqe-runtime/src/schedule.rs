//! Deterministic watermark merge. Accepted ingress and successful publication are separate commits.
use crate::events::{ExecutionError, InputEvent};
use fqe_core::{
    Event,
    bars::ClosedBar,
    types::{NumericUnit, ScalarType},
};
use fqe_ir::{
    event_plan::Trigger,
    model::{ContentId, identifier},
    schedule::{GapPolicy, ScheduleDefinition, StreamDefinition},
};
use std::{collections::BTreeMap, sync::Arc};
type Result<T> = std::result::Result<T, ExecutionError>;
type OrderKey = (u64, String, u8, u64, u64);
fn error(code: &'static str, path: &str, detail: &str) -> ExecutionError {
    ExecutionError {
        code,
        path: path.into(),
        detail: detail.into(),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataGap {
    pub previous_close_ns: u64,
    pub next_open_ns: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamCursor {
    pub last_source_sequence: Option<u64>,
    pub last_timestamp_ns: Option<u64>,
    pub last_bar_close_ns: Option<u64>,
    pub watermark_ns: Option<u64>,
    pub sealed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerState {
    pub accepted_events: u64,
    pub published_events: u64,
    pub queued_events: usize,
    pub last_timestamp_ns: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledEvent {
    pub run: String,
    pub schedule_id: ContentId,
    pub logical_sequence: u64,
    pub stream: String,
    pub dataset_id: ContentId,
    pub source_sequence: u64,
    pub timestamp_ns: u64,
    /// The original payload, with sequence replaced by the deterministic global publication sequence.
    pub event: InputEvent,
    pub gap: Option<DataGap>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alignment {
    timestamp_ns: u64,
    latest: BTreeMap<String, ScheduledEvent>,
}
impl Alignment {
    pub fn timestamp_ns(&self) -> u64 {
        self.timestamp_ns
    }
    /// None means no closed event from this stream has been published yet, never a zero value.
    pub fn latest(&self, stream: &str) -> Option<&ScheduledEvent> {
        self.latest.get(stream)
    }
    pub fn age_ns(&self, stream: &str) -> Option<u64> {
        self.latest(stream)
            .map(|v| self.timestamp_ns - v.timestamp_ns)
    }
    pub fn streams(&self) -> &BTreeMap<String, ScheduledEvent> {
        &self.latest
    }
}
#[derive(Clone)]
struct Pending {
    stream: String,
    source_sequence: u64,
    timestamp_ns: u64,
    event: InputEvent,
    gap: Option<DataGap>,
}
pub struct MarketScheduler {
    definition: Arc<ScheduleDefinition>,
    run: String,
    cursors: BTreeMap<String, StreamCursor>,
    queue: BTreeMap<OrderKey, Pending>,
    state: SchedulerState,
    alignment: Alignment,
}
impl MarketScheduler {
    pub fn new(definition: Arc<ScheduleDefinition>, run: impl Into<String>) -> Result<Self> {
        let run = run.into();
        identifier(&run, "$.run")?;
        let cursors = definition
            .streams()
            .keys()
            .map(|id| {
                (
                    id.clone(),
                    StreamCursor {
                        last_source_sequence: None,
                        last_timestamp_ns: None,
                        last_bar_close_ns: None,
                        watermark_ns: None,
                        sealed: false,
                    },
                )
            })
            .collect();
        Ok(Self {
            definition,
            run,
            cursors,
            queue: BTreeMap::new(),
            state: SchedulerState {
                accepted_events: 0,
                published_events: 0,
                queued_events: 0,
                last_timestamp_ns: None,
            },
            alignment: Alignment {
                timestamp_ns: 0,
                latest: BTreeMap::new(),
            },
        })
    }
    pub fn definition(&self) -> &ScheduleDefinition {
        &self.definition
    }
    pub fn state(&self) -> &SchedulerState {
        &self.state
    }
    pub fn cursors(&self) -> &BTreeMap<String, StreamCursor> {
        &self.cursors
    }
    pub fn alignment(&self) -> &Alignment {
        &self.alignment
    }
    /// Inclusive availability boundary. None stalls publication until every stream has a promise.
    pub fn watermark_ns(&self) -> Option<u64> {
        self.cursors
            .values()
            .try_fold(u64::MAX, |w, s| s.watermark_ns.map(|v| w.min(v)))
    }
    pub fn is_finished(&self) -> bool {
        self.queue.is_empty() && self.cursors.values().all(|s| s.sealed)
    }
    pub fn advance_watermark(&mut self, stream: &str, timestamp_ns: u64) -> Result<()> {
        let state = self
            .cursors
            .get_mut(stream)
            .ok_or_else(|| error("schedule_stream", stream, "unknown stream"))?;
        if state.sealed || state.watermark_ns.is_some_and(|v| timestamp_ns < v) {
            return Err(error(
                "schedule_watermark",
                stream,
                "sealed stream or decreasing watermark",
            ));
        }
        state.watermark_ns = Some(timestamp_ns);
        Ok(())
    }
    /// End-of-input is an irreversible maximum watermark promise, not an inferred timeout.
    pub fn seal(&mut self, stream: &str) -> Result<()> {
        let state = self
            .cursors
            .get_mut(stream)
            .ok_or_else(|| error("schedule_stream", stream, "unknown stream"))?;
        if state.sealed {
            return Err(error("schedule_lifecycle", stream, "stream already sealed"));
        }
        state.sealed = true;
        state.watermark_ns = Some(u64::MAX);
        Ok(())
    }
    pub fn ingest(&mut self, stream: &str, event: InputEvent) -> Result<()> {
        let spec = self
            .definition
            .streams()
            .get(stream)
            .ok_or_else(|| error("schedule_stream", stream, "unknown stream"))?;
        let cursor = &self.cursors[stream];
        if cursor.sealed {
            return Err(error("schedule_lifecycle", stream, "stream sealed"));
        }
        let (sequence, timestamp, bar_close, gap) = validate(spec, cursor, &event)?;
        if cursor.last_source_sequence.is_some_and(|s| sequence <= s) {
            return Err(error(
                "schedule_sequence",
                stream,
                "source sequence must strictly increase",
            ));
        }
        if cursor.last_timestamp_ns.is_some_and(|t| timestamp < t) {
            return Err(error(
                "schedule_order",
                stream,
                "source time moved backwards",
            ));
        }
        if cursor.watermark_ns.is_some_and(|t| timestamp <= t) {
            return Err(error(
                "schedule_late",
                stream,
                "event is at or behind promised inclusive watermark",
            ));
        }
        if self.queue.len() >= self.definition.queue_capacity() {
            return Err(error(
                "schedule_backpressure",
                stream,
                "bounded queue full; retry unchanged input after publication",
            ));
        }
        let accepted = self
            .state
            .accepted_events
            .checked_add(1)
            .ok_or_else(|| error("schedule_limit", stream, "accepted counter overflow"))?;
        let (instrument, family, timeframe) = spec.order_key();
        let key = (timestamp, instrument, family, timeframe, sequence);
        // Validated domain uniqueness and per-stream strict source sequence guarantee key uniqueness.
        self.queue.insert(
            key,
            Pending {
                stream: stream.into(),
                source_sequence: sequence,
                timestamp_ns: timestamp,
                event,
                gap,
            },
        );
        let cursor = self.cursors.get_mut(stream).expect("validated stream");
        cursor.last_source_sequence = Some(sequence);
        cursor.last_timestamp_ns = Some(timestamp);
        cursor.last_bar_close_ns = bar_close;
        self.state.accepted_events = accepted;
        self.state.queued_events = self.queue.len();
        Ok(())
    }
    /// A failing consumer leaves the queued record, publication sequence and alignment unchanged.
    /// Consumer-owned effects must themselves be transactional; the scheduler cannot undo them.
    pub fn dispatch<T>(
        &mut self,
        consumer: impl FnOnce(&ScheduledEvent, &Alignment) -> Result<T>,
    ) -> Result<Option<T>> {
        let Some(watermark) = self.watermark_ns() else {
            return Ok(None);
        };
        let Some((key, pending)) = self.queue.first_key_value() else {
            return Ok(None);
        };
        if pending.timestamp_ns > watermark {
            return Ok(None);
        }
        let key = key.clone();
        let sequence = self.state.published_events.checked_add(1).ok_or_else(|| {
            error(
                "schedule_limit",
                "$.publication",
                "logical sequence overflow",
            )
        })?;
        let event = with_sequence(&pending.event, sequence)?;
        let next = ScheduledEvent {
            run: self.run.clone(),
            schedule_id: self.definition.content_id(),
            logical_sequence: sequence,
            stream: pending.stream.clone(),
            dataset_id: self.definition.streams()[&pending.stream].dataset_id(),
            source_sequence: pending.source_sequence,
            timestamp_ns: pending.timestamp_ns,
            event,
            gap: pending.gap.clone(),
        };
        let mut alignment = self.alignment.clone();
        alignment.timestamp_ns = next.timestamp_ns;
        alignment.latest.insert(next.stream.clone(), next.clone());
        let result = consumer(&next, &alignment)?;
        self.queue.remove(&key);
        self.alignment = alignment;
        self.state.published_events = sequence;
        self.state.last_timestamp_ns = Some(next.timestamp_ns);
        self.state.queued_events = self.queue.len();
        Ok(Some(result))
    }
    /// Unconditionally publish one eligible event to a caller that needs no fallible transaction.
    pub fn next_ready(&mut self) -> Result<Option<ScheduledEvent>> {
        self.dispatch(|event, _| Ok(event.clone()))
    }
}
fn validate(
    spec: &StreamDefinition,
    cursor: &StreamCursor,
    event: &InputEvent,
) -> Result<(u64, u64, Option<u64>, Option<DataGap>)> {
    let path = spec.id();
    match (spec.domain(), event) {
        (Trigger::Tick { instrument }, InputEvent::Tick(e))
            if &e.tick().instrument == instrument =>
        {
            Ok((e.sequence(), e.timestamp_ns(), None, None))
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
            let bar = ClosedBar::new(raw.as_ref().clone())
                .map_err(|e| error("schedule_bar", path, &e.to_string()))?;
            let b = bar.input();
            if &b.instrument != instrument || b.timeframe_ns != *timeframe_ns {
                return Err(error(
                    "schedule_domain",
                    path,
                    "bar instrument/timeframe differs",
                ));
            }
            if bar.price_type().scalar()
                != &(ScalarType::Decimal {
                    scale: *price_scale,
                    unit: NumericUnit::Price(instrument.clone()),
                })
                || bar.volume_type().scalar()
                    != &(ScalarType::Decimal {
                        scale: *volume_scale,
                        unit: NumericUnit::Quantity(instrument.clone()),
                    })
            {
                return Err(error("schedule_type", path, "bar scales differ"));
            }
            if b.open_ns % timeframe_ns != 0 {
                return Err(error(
                    "schedule_calendar",
                    path,
                    "bar is not aligned to the fixed UTC epoch grid",
                ));
            }
            let gap = if let Some(previous) = cursor.last_bar_close_ns {
                if b.open_ns < previous {
                    return Err(error(
                        "schedule_overlap",
                        path,
                        "bar overlaps accepted interval",
                    ));
                }
                if b.open_ns > previous {
                    if spec.gaps() == GapPolicy::Reject {
                        return Err(error("schedule_gap", path, "missing fixed-grid bars"));
                    }
                    Some(DataGap {
                        previous_close_ns: previous,
                        next_open_ns: b.open_ns,
                    })
                } else {
                    None
                }
            } else {
                None
            };
            Ok((b.sequence, b.timestamp_ns, Some(b.close_ns), gap))
        }
        _ => Err(error(
            "schedule_domain",
            path,
            "event family/instrument differs",
        )),
    }
}
fn with_sequence(event: &InputEvent, sequence: u64) -> Result<InputEvent> {
    match event {
        InputEvent::Tick(e) => Ok(Event::new(sequence, e.timestamp_ns(), e.tick().clone())
            .map_err(|_| {
                error(
                    "schedule_sequence",
                    "$.publication",
                    "invalid logical sequence",
                )
            })?
            .into()),
        InputEvent::Bar(b) => {
            let mut b = b.as_ref().clone();
            b.sequence = sequence;
            Ok(InputEvent::from(b))
        }
    }
}
