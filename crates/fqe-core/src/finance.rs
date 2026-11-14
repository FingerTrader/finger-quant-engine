//! Exact bounded instrument units and rounding; no balances, orders, or market-data acquisition.
use crate::{
    InstrumentId,
    numeric::{Decimal, NumericError, Rounding},
    types::CurrencyCode,
};
use std::{cmp::Ordering, fmt};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinancialError {
    Syntax,
    Scale,
    Step,
    Range,
    Inexact,
    Overflow,
}
impl fmt::Display for FinancialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "financial value error: {self:?}")
    }
}
impl std::error::Error for FinancialError {}
impl From<NumericError> for FinancialError {
    fn from(e: NumericError) -> Self {
        match e {
            NumericError::Overflow => Self::Overflow,
            NumericError::Inexact => Self::Inexact,
            _ => Self::Scale,
        }
    }
}
type Result<T> = std::result::Result<T, FinancialError>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinancialRounding {
    Exact,
    TowardZero,
    Floor,
    Ceiling,
    HalfEven,
    HalfAwayFromZero,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PricePolicy {
    NonNegative,
    Signed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstrumentDraft {
    pub instrument: InstrumentId,
    pub currency: CurrencyCode,
    pub price_step: Decimal,
    pub quantity_step: Decimal,
    pub money_step: Decimal,
    pub minimum_quantity: Decimal,
    pub minimum_notional: Decimal,
    pub prices: PricePolicy,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinancialInstrument(InstrumentDraft);
fn power(n: u8) -> i128 {
    10_i128.pow(u32::from(n))
}
impl FinancialInstrument {
    pub fn new(d: InstrumentDraft) -> Result<Self> {
        if d.price_step.scale() > 12 || d.quantity_step.scale() > 6 || d.money_step.scale() > 6 {
            return Err(FinancialError::Scale);
        }
        if [d.price_step, d.quantity_step, d.money_step]
            .iter()
            .any(|v| v.coefficient() <= 0)
        {
            return Err(FinancialError::Step);
        }
        let result = Self(d);
        result.price(result.0.price_step)?;
        result.base_quantity(result.0.quantity_step, false)?;
        result.base_quantity(result.0.minimum_quantity, true)?;
        result.money(result.0.money_step)?;
        result.money(result.0.minimum_notional)?;
        if result.0.minimum_notional.coefficient() < 0 {
            return Err(FinancialError::Range);
        }
        Ok(result)
    }
    pub fn declaration(&self) -> &InstrumentDraft {
        &self.0
    }
    pub fn price(&self, v: Decimal) -> Result<Decimal> {
        if v.scale() != self.0.price_step.scale() {
            return Err(FinancialError::Scale);
        }
        if (self.0.prices == PricePolicy::NonNegative && v.coefficient() < 0)
            || v.coefficient().unsigned_abs() > (1_000_000_000_000 * power(v.scale())) as u128
        {
            return Err(FinancialError::Range);
        }
        multiple(v, self.0.price_step)
    }
    pub fn quantity(&self, v: Decimal, allow_zero: bool) -> Result<Decimal> {
        self.base_quantity(v, allow_zero)?;
        if v.coefficient() != 0 && v.coefficient() < self.0.minimum_quantity.coefficient() {
            return Err(FinancialError::Range);
        }
        Ok(v)
    }
    fn base_quantity(&self, v: Decimal, allow_zero: bool) -> Result<Decimal> {
        if v.scale() != self.0.quantity_step.scale() {
            return Err(FinancialError::Scale);
        }
        if v.coefficient() < i128::from(!allow_zero)
            || v.coefficient() > 1_000_000 * power(v.scale())
        {
            return Err(FinancialError::Range);
        }
        multiple(v, self.0.quantity_step)
    }
    pub fn money(&self, v: Decimal) -> Result<Decimal> {
        if v.scale() != self.0.money_step.scale() {
            return Err(FinancialError::Scale);
        }
        if v.coefficient().unsigned_abs() > (1_000_000_000_000_000_000 * power(v.scale())) as u128 {
            return Err(FinancialError::Range);
        }
        multiple(v, self.0.money_step)
    }
    pub fn notional(
        &self,
        price: Decimal,
        quantity: Decimal,
        rounding: FinancialRounding,
    ) -> Result<Decimal> {
        self.price(price)?;
        self.quantity(quantity, false)?;
        let coefficient = price
            .coefficient()
            .checked_mul(quantity.coefficient())
            .ok_or(FinancialError::Overflow)?;
        let exact = Decimal::new(coefficient, price.scale() + quantity.scale())?;
        self.money(align(exact, self.0.money_step, rounding)?)
    }
}
fn multiple(v: Decimal, step: Decimal) -> Result<Decimal> {
    if v.scale() != step.scale() {
        return Err(FinancialError::Scale);
    }
    if v.coefficient() % step.coefficient() != 0 {
        return Err(FinancialError::Step);
    }
    Ok(v)
}
/// Canonical plain decimal source -> an explicit fixed scale, without rounding or floats.
pub fn parse_decimal(text: &str, scale: u8) -> Result<Decimal> {
    if text.is_empty() || text.len() > 64 || scale > 18 {
        return Err(FinancialError::Syntax);
    }
    let negative = text.starts_with('-');
    let unsigned = if negative { &text[1..] } else { text };
    let mut parts = unsigned.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next();
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || (whole.len() > 1 && whole.starts_with('0'))
    {
        return Err(FinancialError::Syntax);
    }
    let fraction = fraction.unwrap_or("");
    if (unsigned.contains('.') && fraction.is_empty())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(FinancialError::Syntax);
    }
    if fraction.len() > usize::from(scale)
        && fraction[usize::from(scale)..].bytes().any(|b| b != b'0')
    {
        return Err(FinancialError::Inexact);
    }
    let fraction = &fraction[..fraction.len().min(usize::from(scale))];
    let digits = format!("{}{}{}", if negative { "-" } else { "" }, whole, fraction);
    let n = digits
        .parse::<i128>()
        .map_err(|_| FinancialError::Overflow)?;
    if negative && n == 0 {
        return Err(FinancialError::Syntax);
    }
    let n = n
        .checked_mul(power(scale - fraction.len() as u8))
        .ok_or(FinancialError::Overflow)?;
    Ok(Decimal::new(n, scale)?)
}
/// Step alignment over an exact integer ratio; half-away matches Desk halfUp for signed values.
pub fn align(value: Decimal, step: Decimal, rounding: FinancialRounding) -> Result<Decimal> {
    if step.coefficient() <= 0 {
        return Err(FinancialError::Step);
    }
    let scale = value.scale().max(step.scale());
    let value = value.rescale(scale, Rounding::Exact)?;
    let denominator = step.rescale(scale, Rounding::Exact)?.coefficient();
    let mut q = value.coefficient() / denominator;
    let rem = value.coefficient() % denominator;
    if rem != 0 {
        let half = rem
            .unsigned_abs()
            .cmp(&(denominator as u128 - rem.unsigned_abs()));
        let increment = match rounding {
            FinancialRounding::Exact => return Err(FinancialError::Inexact),
            FinancialRounding::TowardZero => false,
            FinancialRounding::Floor => rem < 0,
            FinancialRounding::Ceiling => rem > 0,
            FinancialRounding::HalfEven => {
                half == Ordering::Greater || (half == Ordering::Equal && q % 2 != 0)
            }
            FinancialRounding::HalfAwayFromZero => half != Ordering::Less,
        };
        if increment {
            q = q
                .checked_add(if rem < 0 { -1 } else { 1 })
                .ok_or(FinancialError::Overflow)?;
        }
    }
    let n = q.checked_mul(denominator).ok_or(FinancialError::Overflow)?;
    Ok(Decimal::new(n, scale)?.rescale(step.scale(), Rounding::Exact)?)
}
/// Canonical plain display without float conversion; zero keeps an unsigned spelling.
pub fn decimal_text(value: Decimal) -> String {
    let digits = value.coefficient().unsigned_abs().to_string();
    let scale = usize::from(value.scale());
    let sign = if value.coefficient() < 0 { "-" } else { "" };
    if scale == 0 {
        return format!("{sign}{digits}");
    }
    let padded = format!("{:0>width$}", digits, width = scale + 1);
    let split = padded.len() - scale;
    format!("{sign}{}.{}", &padded[..split], &padded[split..])
}
