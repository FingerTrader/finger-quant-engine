//! Experimental typed strategy IR. This is a Rust model, not a source-language compiler.

use std::fmt;

pub mod builtins;
pub mod compiler;
pub mod event_plan;
pub mod expressions;
pub mod graph;
pub mod indicators;
pub mod loading;
pub mod model;
pub mod parameters;
pub mod registry;

use fqe_core::{InstrumentId, MarketTick, PriceTicks, Signal, SignalKind};

pub const IR_VERSION: u32 = 1;

/// Only price-valued expressions may appear in a price comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceExpr {
    Current,
    Literal(PriceTicks),
}

impl PriceExpr {
    fn evaluate(self, tick: &MarketTick) -> PriceTicks {
        match self {
            Self::Current => tick.price,
            Self::Literal(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Predicate {
    GreaterThan { left: PriceExpr, right: PriceExpr },
}

impl Predicate {
    fn evaluate(self, tick: &MarketTick) -> bool {
        match self {
            Self::GreaterThan { left, right } => left.evaluate(tick) > right.evaluate(tick),
        }
    }
}

/// A validated single-instrument, stateless signal rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strategy {
    id: String,
    instrument: InstrumentId,
    condition: Predicate,
    signal: SignalKind,
}

impl Strategy {
    pub fn new(
        version: u32,
        id: impl Into<String>,
        instrument: InstrumentId,
        condition: Predicate,
        signal: SignalKind,
    ) -> Result<Self, StrategyError> {
        if version != IR_VERSION {
            return Err(StrategyError::UnsupportedVersion(version));
        }
        let id = id.into();
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        {
            return Err(StrategyError::InvalidId);
        }
        Ok(Self {
            id,
            instrument,
            condition,
            signal,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    /// Emits once per matching input event. No position tracking or deduplication.
    pub fn evaluate(&self, tick: &MarketTick) -> Option<Signal> {
        (tick.instrument == self.instrument && self.condition.evaluate(tick)).then(|| Signal {
            instrument: self.instrument.clone(),
            kind: self.signal,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrategyError {
    UnsupportedVersion(u32),
    InvalidId,
}

impl fmt::Display for StrategyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => write!(f, "unsupported IR version: {version}"),
            Self::InvalidId => {
                f.write_str("strategy ID must contain 1..=128 ASCII letters, digits, or ._-")
            }
        }
    }
}

impl std::error::Error for StrategyError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn condition() -> Predicate {
        Predicate::GreaterThan {
            left: PriceExpr::Current,
            right: PriceExpr::Literal(PriceTicks::new(100).unwrap()),
        }
    }

    fn strategy(version: u32, id: &str) -> Result<Strategy, StrategyError> {
        Strategy::new(
            version,
            id,
            InstrumentId::new("DEMO").unwrap(),
            condition(),
            SignalKind::Long,
        )
    }

    #[test]
    fn validation_rejects_unknown_versions_and_invalid_ids() {
        assert_eq!(
            strategy(0, "demo"),
            Err(StrategyError::UnsupportedVersion(0))
        );
        assert_eq!(
            strategy(2, "demo"),
            Err(StrategyError::UnsupportedVersion(2))
        );
        for id in ["", "demo strategy", "../demo"] {
            assert_eq!(strategy(IR_VERSION, id), Err(StrategyError::InvalidId));
        }
        assert!(strategy(IR_VERSION, &"a".repeat(129)).is_err());
    }

    #[test]
    fn greater_than_is_strict_and_has_no_float_rounding() {
        let rule = strategy(IR_VERSION, "demo").unwrap();
        for (price, expected) in [(99, false), (100, false), (101, true), (u64::MAX, true)] {
            let tick = MarketTick {
                instrument: InstrumentId::new("DEMO").unwrap(),
                price: PriceTicks::new(price).unwrap(),
            };
            assert_eq!(rule.evaluate(&tick).is_some(), expected);
        }
    }

    #[test]
    fn unrelated_instruments_do_not_emit_signals() {
        let rule = strategy(IR_VERSION, "demo").unwrap();
        let tick = MarketTick {
            instrument: InstrumentId::new("OTHER").unwrap(),
            price: PriceTicks::new(101).unwrap(),
        };
        assert_eq!(rule.evaluate(&tick), None);
    }
}
