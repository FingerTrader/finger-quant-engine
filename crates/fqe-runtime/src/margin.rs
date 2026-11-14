//! Explicit bounded zero-interest cash-margin-long risk arithmetic; not a broker SDK.
use fqe_core::{finance::FinancialError, numeric::Decimal};
pub const MARGIN_MODEL: &str = "fqe.next-open-margin-long/1";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginRules {
    pub max_leverage_bps: u32,
    pub maintenance_bps: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarginStatus {
    Healthy,
    Liquidating,
    Liquidated,
    Insolvent,
    LiquidationFailed,
    Stopped,
}
impl MarginRules {
    pub fn validate(&self) -> Result<(), FinancialError> {
        if !(10000..=50000).contains(&self.max_leverage_bps)
            || self.maintenance_bps == 0
            || u64::from(self.maintenance_bps) * u64::from(self.max_leverage_bps) >= 100_000_000
        {
            return Err(FinancialError::Range);
        }
        Ok(())
    }
    pub fn exposure_limit(
        &self,
        equity: Decimal,
        step: Decimal,
    ) -> Result<Decimal, FinancialError> {
        self.validate()?;
        scaled(equity, step, self.max_leverage_bps, false)
    }
    pub fn maintenance(&self, exposure: Decimal, step: Decimal) -> Result<Decimal, FinancialError> {
        self.validate()?;
        if exposure.coefficient() < 0 {
            return Err(FinancialError::Range);
        }
        scaled(exposure, step, self.maintenance_bps, true)
    }
}
fn scaled(value: Decimal, step: Decimal, bps: u32, ceil: bool) -> Result<Decimal, FinancialError> {
    if value.scale() != step.scale() || step.coefficient() <= 0 {
        return Err(FinancialError::Scale);
    }
    let value = value.coefficient().max(0);
    let num = value
        .checked_mul(i128::from(bps))
        .ok_or(FinancialError::Overflow)?;
    let den = step
        .coefficient()
        .checked_mul(10000)
        .ok_or(FinancialError::Overflow)?;
    let mut q = num / den;
    if ceil && num % den != 0 {
        q = q.checked_add(1).ok_or(FinancialError::Overflow)?;
    }
    Decimal::new(
        q.checked_mul(step.coefficient())
            .ok_or(FinancialError::Overflow)?,
        step.scale(),
    )
    .map_err(Into::into)
}
