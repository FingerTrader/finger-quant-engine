//! Closed continuous-bar primitives; no tick synthesis, exchange calendar or data IO.
use crate::{
    InstrumentId,
    types::{NumericUnit, ScalarType, TypeDescriptor, TypedValue},
};
use std::fmt;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarInput {
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub instrument: InstrumentId,
    pub timeframe_ns: u64,
    pub open_ns: u64,
    pub close_ns: u64,
    pub closed: bool,
    pub open: TypedValue,
    pub high: TypedValue,
    pub low: TypedValue,
    pub close: TypedValue,
    pub volume: TypedValue,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosedBar(BarInput);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarError {
    Sequence,
    Interval,
    NotClosed,
    PriceType,
    PriceRange,
    Ohlc,
    Volume,
}
impl fmt::Display for BarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Sequence => "bar sequence must be positive",
            Self::Interval => "invalid timeframe/open/close/event timestamp",
            Self::NotClosed => "bar is not closed",
            Self::PriceType => "OHLC requires identical Price(instrument) decimal scale 0..12",
            Self::PriceRange => "absolute OHLC price exceeds 1e12",
            Self::Ohlc => "OHLC bounds are inconsistent",
            Self::Volume => "volume requires nonnegative Quantity(instrument) decimal",
        })
    }
}
impl std::error::Error for BarError {}
impl ClosedBar {
    pub fn new(b: BarInput) -> Result<Self, BarError> {
        if b.sequence == 0 {
            return Err(BarError::Sequence);
        }
        if !b.closed {
            return Err(BarError::NotClosed);
        }
        if b.timeframe_ns == 0
            || b.close_ns.checked_sub(b.open_ns) != Some(b.timeframe_ns)
            || b.timestamp_ns != b.close_ns
        {
            return Err(BarError::Interval);
        }
        let scale = match b.close.descriptor().scalar() {
            ScalarType::Decimal {
                scale,
                unit: NumericUnit::Price(i),
            } if *scale <= 12 && i == &b.instrument => *scale,
            _ => return Err(BarError::PriceType),
        };
        let max = 1_000_000_000_000_i128 * 10_i128.pow(u32::from(scale));
        for v in [&b.open, &b.high, &b.low, &b.close] {
            if v.descriptor() != b.close.descriptor() {
                return Err(BarError::PriceType);
            }
            if v.numeric_coefficient()
                .map_err(|_| BarError::PriceType)?
                .unsigned_abs()
                > max as u128
            {
                return Err(BarError::PriceRange);
            }
        }
        let num = |v: &TypedValue| v.numeric_coefficient().expect("checked decimal");
        if num(&b.low) > num(&b.open).min(num(&b.close))
            || num(&b.open).max(num(&b.close)) > num(&b.high)
        {
            return Err(BarError::Ohlc);
        }
        match b.volume.descriptor().scalar() {
            ScalarType::Decimal {
                unit: NumericUnit::Quantity(i),
                ..
            } if i == &b.instrument => {}
            _ => return Err(BarError::Volume),
        }
        if b.volume
            .numeric_coefficient()
            .map_err(|_| BarError::Volume)?
            < 0
        {
            return Err(BarError::Volume);
        }
        Ok(Self(b))
    }
    pub fn input(&self) -> &BarInput {
        &self.0
    }
    pub fn price_type(&self) -> &TypeDescriptor {
        self.0.close.descriptor()
    }
    pub fn volume_type(&self) -> &TypeDescriptor {
        self.0.volume.descriptor()
    }
}
