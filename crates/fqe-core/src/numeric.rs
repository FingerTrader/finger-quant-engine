//! Checked, exact base-10 values. This is not a financial ledger or indicator policy.

use std::{cmp::Ordering, fmt};

// Storage includes exact price(12) x quantity(8) financial intermediates.
pub const MAX_SCALE: u8 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericError {
    UnsupportedScale(u8),
    ScaleMismatch,
    Overflow,
    DivisionByZero,
    Inexact,
}

impl fmt::Display for NumericError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedScale(scale) => write!(f, "unsupported decimal scale: {scale}"),
            Self::ScaleMismatch => f.write_str("decimal scales differ; rescale explicitly"),
            Self::Overflow => f.write_str("decimal result or intermediate overflows i128"),
            Self::DivisionByZero => f.write_str("decimal division by zero"),
            Self::Inexact => f.write_str("decimal result requires explicit rounding"),
        }
    }
}

impl std::error::Error for NumericError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    Exact,
    TowardZero,
    HalfEven,
}

/// `coefficient * 10^(-scale)`. Scale is retained, including for zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decimal {
    coefficient: i128,
    scale: u8,
}

impl Decimal {
    pub fn new(coefficient: i128, scale: u8) -> Result<Self, NumericError> {
        power(scale)?;
        Ok(Self { coefficient, scale })
    }

    pub fn coefficient(self) -> i128 {
        self.coefficient
    }

    pub fn scale(self) -> u8 {
        self.scale
    }

    pub fn rescale(self, scale: u8, rounding: Rounding) -> Result<Self, NumericError> {
        power(scale)?;
        let coefficient = if scale >= self.scale {
            self.coefficient
                .checked_mul(power(scale - self.scale)?)
                .ok_or(NumericError::Overflow)?
        } else {
            divide(self.coefficient, power(self.scale - scale)?, rounding)?
        };
        Self::new(coefficient, scale)
    }

    fn same_scale(self, rhs: Self) -> Result<(), NumericError> {
        if self.scale == rhs.scale {
            Ok(())
        } else {
            Err(NumericError::ScaleMismatch)
        }
    }

    pub fn checked_cmp(self, rhs: Self) -> Result<Ordering, NumericError> {
        self.same_scale(rhs)?;
        Ok(self.coefficient.cmp(&rhs.coefficient))
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, NumericError> {
        self.same_scale(rhs)?;
        Self::new(
            self.coefficient
                .checked_add(rhs.coefficient)
                .ok_or(NumericError::Overflow)?,
            self.scale,
        )
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, NumericError> {
        self.same_scale(rhs)?;
        Self::new(
            self.coefficient
                .checked_sub(rhs.coefficient)
                .ok_or(NumericError::Overflow)?,
            self.scale,
        )
    }

    /// Both operands and the output use the same scale. Intermediate overflow fails.
    pub fn checked_mul(self, rhs: Self, rounding: Rounding) -> Result<Self, NumericError> {
        self.same_scale(rhs)?;
        let product = self
            .coefficient
            .checked_mul(rhs.coefficient)
            .ok_or(NumericError::Overflow)?;
        Self::new(divide(product, power(self.scale)?, rounding)?, self.scale)
    }

    /// Both operands and the output use the same scale. No implicit precision growth.
    pub fn checked_div(self, rhs: Self, rounding: Rounding) -> Result<Self, NumericError> {
        self.same_scale(rhs)?;
        if rhs.coefficient == 0 {
            return Err(NumericError::DivisionByZero);
        }
        let numerator = self
            .coefficient
            .checked_mul(power(self.scale)?)
            .ok_or(NumericError::Overflow)?;
        Self::new(divide(numerator, rhs.coefficient, rounding)?, self.scale)
    }
}

fn power(scale: u8) -> Result<i128, NumericError> {
    if scale > MAX_SCALE {
        return Err(NumericError::UnsupportedScale(scale));
    }
    Ok(10_i128.pow(u32::from(scale)))
}

fn divide(n: i128, d: i128, rounding: Rounding) -> Result<i128, NumericError> {
    if d == 0 {
        return Err(NumericError::DivisionByZero);
    }
    let q = n.checked_div(d).ok_or(NumericError::Overflow)?;
    let r = n.checked_rem(d).ok_or(NumericError::Overflow)?;
    if r == 0 {
        return Ok(q);
    }
    match rounding {
        Rounding::Exact => Err(NumericError::Inexact),
        Rounding::TowardZero => Ok(q),
        Rounding::HalfEven => {
            // Compare 2*|r| with |d| without overflowing, including i128::MIN.
            let r = r.unsigned_abs();
            let comparison = r.cmp(&(d.unsigned_abs() - r));
            if comparison == Ordering::Greater || (comparison == Ordering::Equal && q % 2 != 0) {
                let step = if (n < 0) == (d < 0) { 1 } else { -1 };
                q.checked_add(step).ok_or(NumericError::Overflow)
            } else {
                Ok(q)
            }
        }
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magnitude = self.coefficient.unsigned_abs();
        let sign = if self.coefficient < 0 { "-" } else { "" };
        if self.scale == 0 {
            write!(f, "{sign}{magnitude}")
        } else {
            let factor = 10_u128.pow(u32::from(self.scale));
            let whole = magnitude / factor;
            let fraction = magnitude % factor;
            write!(
                f,
                "{sign}{whole}.{fraction:0width$}",
                width = usize::from(self.scale)
            )
        }
    }
}
