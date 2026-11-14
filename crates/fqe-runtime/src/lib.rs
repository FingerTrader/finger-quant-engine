//! Single-threaded, in-memory event execution. No broker, filesystem, or worker transport.

pub mod events;
pub mod report;
pub mod workflow;

use std::fmt;

use fqe_core::{Event, Signal};
use fqe_ir::Strategy;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Ready,
    Running,
    Completed,
    Failed,
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        })
    }
}

/// An observation of this runtime instance, not a durable checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunState {
    pub status: RunStatus,
    pub processed_events: u64,
    pub last_sequence: Option<u64>,
    pub last_timestamp_ns: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignalRecord {
    pub source_sequence: u64,
    pub timestamp_ns: u64,
    pub signal: Signal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunReport {
    pub state: RunState,
    pub signals: Vec<SignalRecord>,
}

/// A one-shot interpreter for a validated strategy and caller-owned input order.
pub struct Runtime {
    strategy: Strategy,
    state: RunState,
}

impl Runtime {
    pub fn new(strategy: Strategy) -> Self {
        Self {
            strategy,
            state: RunState {
                status: RunStatus::Ready,
                processed_events: 0,
                last_sequence: None,
                last_timestamp_ns: None,
            },
        }
    }

    pub fn state(&self) -> &RunState {
        &self.state
    }

    pub fn run(
        &mut self,
        events: impl IntoIterator<Item = Event>,
    ) -> Result<RunReport, RuntimeError> {
        if self.state.status != RunStatus::Ready {
            return Err(RuntimeError::AlreadyStarted(self.state.status));
        }
        self.state.status = RunStatus::Running;
        let mut signals = Vec::new();
        for event in events {
            if let Err(error) = self.validate_order(&event) {
                self.state.status = RunStatus::Failed;
                return Err(error);
            }
            let Some(processed_events) = self.state.processed_events.checked_add(1) else {
                self.state.status = RunStatus::Failed;
                return Err(RuntimeError::EventCountOverflow);
            };
            if let Some(signal) = self.strategy.evaluate(event.tick()) {
                signals.push(SignalRecord {
                    source_sequence: event.sequence(),
                    timestamp_ns: event.timestamp_ns(),
                    signal,
                });
            }
            self.state.processed_events = processed_events;
            self.state.last_sequence = Some(event.sequence());
            self.state.last_timestamp_ns = Some(event.timestamp_ns());
        }
        self.state.status = RunStatus::Completed;
        Ok(RunReport {
            state: self.state.clone(),
            signals,
        })
    }

    fn validate_order(&self, event: &Event) -> Result<(), RuntimeError> {
        if let Some(previous) = self.state.last_sequence
            && event.sequence() <= previous
        {
            return Err(RuntimeError::NonIncreasingSequence {
                previous,
                received: event.sequence(),
            });
        }
        if let Some(previous) = self.state.last_timestamp_ns
            && event.timestamp_ns() < previous
        {
            return Err(RuntimeError::BackwardTimestamp {
                previous,
                received: event.timestamp_ns(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    AlreadyStarted(RunStatus),
    NonIncreasingSequence { previous: u64, received: u64 },
    BackwardTimestamp { previous: u64, received: u64 },
    EventCountOverflow,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted(status) => {
                write!(f, "runtime has already started (status={status})")
            }
            Self::NonIncreasingSequence { previous, received } => write!(
                f,
                "event sequence must increase: previous={previous}, received={received}"
            ),
            Self::BackwardTimestamp { previous, received } => write!(
                f,
                "event timestamp must not decrease: previous={previous}, received={received}"
            ),
            Self::EventCountOverflow => f.write_str("processed event count overflow"),
        }
    }
}

impl std::error::Error for RuntimeError {}
