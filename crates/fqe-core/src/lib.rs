//! Language-independent domain values. No networking, account, UI, or broker access.

use std::fmt;

pub mod bars;
pub mod numeric;
pub mod types;

/// A case-sensitive, bounded identifier shared by market inputs and strategies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstrumentId(String);

impl InstrumentId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b":/._-".contains(&byte))
        {
            return Err(ValidationError::InvalidInstrument);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A positive integer price in an instrument-specific tick size.
/// The caller must supply a consistent scale; no floating point or currency inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PriceTicks(u64);

impl PriceTicks {
    pub fn new(value: u64) -> Result<Self, ValidationError> {
        if value == 0 {
            return Err(ValidationError::ZeroPrice);
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketTick {
    pub instrument: InstrumentId,
    pub price: PriceTicks,
}

/// An input event, ordered by caller-assigned sequence within one run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    sequence: u64,
    timestamp_ns: u64,
    tick: MarketTick,
}

impl Event {
    pub fn new(
        sequence: u64,
        timestamp_ns: u64,
        tick: MarketTick,
    ) -> Result<Self, ValidationError> {
        if sequence == 0 {
            return Err(ValidationError::ZeroSequence);
        }
        Ok(Self {
            sequence,
            timestamp_ns,
            tick,
        })
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn timestamp_ns(&self) -> u64 {
        self.timestamp_ns
    }

    pub fn tick(&self) -> &MarketTick {
        &self.tick
    }
}

/// A strategy observation, never an order or execution instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalKind {
    Long,
    Flat,
}

impl fmt::Display for SignalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Long => "long",
            Self::Flat => "flat",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signal {
    pub instrument: InstrumentId,
    pub kind: SignalKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationError {
    InvalidInstrument,
    ZeroPrice,
    ZeroSequence,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInstrument => {
                "instrument must contain 1..=128 ASCII letters, digits, or :/._-"
            }
            Self::ZeroPrice => "price ticks must be positive",
            Self::ZeroSequence => "event sequence must be positive",
        })
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_reject_ambiguous_or_unbounded_values() {
        for value in ["", " DEMO", "DEMO ASSET", "../asset\n", "资产"] {
            assert_eq!(
                InstrumentId::new(value),
                Err(ValidationError::InvalidInstrument)
            );
        }
        assert!(InstrumentId::new("a".repeat(129)).is_err());
        assert_eq!(
            InstrumentId::new("DEMO:ASSET/1.0_a-b").unwrap().as_str(),
            "DEMO:ASSET/1.0_a-b"
        );
    }

    #[test]
    fn prices_are_positive_and_exact_including_large_values() {
        assert_eq!(PriceTicks::new(0), Err(ValidationError::ZeroPrice));
        assert_eq!(PriceTicks::new(u64::MAX).unwrap().get(), u64::MAX);
        assert!(PriceTicks::new(100).unwrap() < PriceTicks::new(101).unwrap());
    }

    #[test]
    fn zero_sequence_is_invalid_but_epoch_zero_is_valid() {
        let tick = MarketTick {
            instrument: InstrumentId::new("DEMO").unwrap(),
            price: PriceTicks::new(1).unwrap(),
        };
        assert_eq!(
            Event::new(0, 0, tick.clone()),
            Err(ValidationError::ZeroSequence)
        );
        let event = Event::new(1, 0, tick).unwrap();
        assert_eq!(event.timestamp_ns(), 0);
        assert_eq!(event.sequence(), 1);
    }
}
