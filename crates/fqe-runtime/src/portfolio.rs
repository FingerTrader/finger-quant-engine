//! Ledger-derived, close-sampled equity metrics. Ratios are fractions, never percentages.
use fqe_core::{finance::FinancialError, numeric::Decimal};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EquityPoint {
    pub source_sequence: u64,
    pub timestamp_ns: u64,
    pub mark: Decimal,
    pub cash: Decimal,
    pub debt: Decimal,
    pub quantity: Decimal,
    pub equity: Decimal,
    pub fees: Decimal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortfolioMetrics {
    pub initial_equity: Decimal,
    pub final_equity: Decimal,
    pub net_profit: Decimal,
    pub fees: Decimal,
    pub total_return: Option<Decimal>,
    pub max_drawdown: Decimal,
    pub max_drawdown_ratio: Option<Decimal>,
}
fn decimal(n: i128, s: u8) -> Result<Decimal, FinancialError> {
    Decimal::new(n, s).map_err(Into::into)
}
fn ratio(n: i128, d: i128) -> Result<Decimal, FinancialError> {
    let n = n
        .checked_mul(1_000_000_000_000)
        .ok_or(FinancialError::Overflow)?;
    let mut q = n / d;
    let r = (n % d).abs();
    if r > d - r || (r == d - r && q % 2 != 0) {
        q = q.checked_add(n.signum()).ok_or(FinancialError::Overflow)?;
    }
    decimal(q, 12)
}
pub fn calculate(
    initial: Decimal,
    points: &[EquityPoint],
    fees: Decimal,
) -> Result<PortfolioMetrics, FinancialError> {
    if initial.coefficient() < 0 || fees.coefficient() < 0 || initial.scale() != fees.scale() {
        return Err(FinancialError::Range);
    }
    let mut peak = initial.coefficient();
    let mut dd = 0;
    let mut dd_ratio = if peak > 0 {
        Some(decimal(0, 12)?)
    } else {
        None
    };
    let mut previous: Option<(u64, u64)> = None;
    for p in points {
        if p.equity.scale() != initial.scale()
            || p.source_sequence == 0
            || previous.is_some_and(|(s, t)| p.source_sequence <= s || p.timestamp_ns < t)
        {
            return Err(FinancialError::Range);
        }
        previous = Some((p.source_sequence, p.timestamp_ns));
        peak = peak.max(p.equity.coefficient());
        let drop = peak
            .checked_sub(p.equity.coefficient())
            .ok_or(FinancialError::Overflow)?;
        dd = dd.max(drop);
        if peak > 0 {
            let r = ratio(drop, peak)?;
            if dd_ratio.is_none_or(|v| r.coefficient() > v.coefficient()) {
                dd_ratio = Some(r);
            }
        }
    }
    let final_equity = points.last().map_or(initial, |p| p.equity);
    let profit = final_equity
        .coefficient()
        .checked_sub(initial.coefficient())
        .ok_or(FinancialError::Overflow)?;
    Ok(PortfolioMetrics {
        initial_equity: initial,
        final_equity,
        net_profit: decimal(profit, initial.scale())?,
        fees,
        total_return: if initial.coefficient() > 0 {
            Some(ratio(profit, initial.coefficient())?)
        } else {
            None
        },
        max_drawdown: decimal(dd, initial.scale())?,
        max_drawdown_ratio: dd_ratio,
    })
}
