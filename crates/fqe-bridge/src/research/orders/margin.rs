//! Linear shared-margin research, settled in the instrument quote currency.
//! VirtualLedger remains the posting authority. Internal clearing credit funds
//! gross-basis legs; it is deducted from equity and never treated as profit.
use super::*;

pub const MARGIN_PROFILE: &str = "fqe.linear-shared-margin-open/1";
#[derive(Clone)]
pub struct MarginConfig {
    pub leverage: Decimal,
    pub percent: Decimal,
    pub status: &'static str,
}
impl MarginConfig {
    pub fn parse(context: &BTreeMap<String, String>) -> Result<Option<Self>> {
        match context
            .get("capitalModel")
            .map(String::as_str)
            .unwrap_or("cash")
        {
            "cash" => {
                if context
                    .get("leverage")
                    .is_some_and(|v| num(v).ok() != whole(1).ok())
                {
                    return Err(failure("research_margin_required", "$.context.leverage"));
                }
                Ok(None)
            }
            "linear_margin" => {
                if context.get("accountMode").map(String::as_str) != Some("margin") {
                    return Err(failure(
                        "research_margin_configuration",
                        "$.context.accountMode",
                    ));
                }
                let leverage = num(context.get("leverage").map(String::as_str).unwrap_or(""))?;
                let percent = num(context
                    .get("entryPercent")
                    .map(String::as_str)
                    .unwrap_or(""))?;
                if leverage.coefficient() < FACTOR
                    || leverage.coefficient() > 1000 * FACTOR
                    || leverage.coefficient() % FACTOR != 0
                    || percent.coefficient() <= 0
                    || percent.coefficient() > 100 * FACTOR
                {
                    return Err(failure("research_margin_configuration", "$.context"));
                }
                Ok(Some(Self {
                    leverage,
                    percent,
                    status: "healthy",
                }))
            }
            _ => Err(failure("research_capital_model", "$.context.capitalModel")),
        }
    }
}
impl Broker {
    pub fn profile(&self) -> &'static str {
        if self.margin.is_some() {
            MARGIN_PROFILE
        } else {
            PROFILE
        }
    }
    fn money_ceil(&self, amount: Decimal) -> Result<Decimal> {
        Ok(align(
            amount,
            self.hub.config().instrument.declaration().money_step,
            FinancialRounding::Ceiling,
        )?)
    }
    pub fn entry_budget(&self) -> Result<Decimal> {
        let rules = self.margin.as_ref().expect("margin");
        let amount = div(
            mul(maxd(self.equity()?, whole(0)?), rules.percent)?,
            whole(100)?,
        )?;
        Ok(align(
            amount,
            self.hub.config().instrument.declaration().money_step,
            FinancialRounding::Floor,
        )?)
    }
    pub(super) fn entry_cost(&self, gross: Decimal, fee: Decimal) -> Result<Decimal> {
        match &self.margin {
            Some(rules) => add(self.money_ceil(div(gross, rules.leverage)?)?, fee),
            None => add(gross, fee),
        }
    }
    pub fn margin_used(&self) -> Result<Decimal> {
        let rules = self.margin.as_ref().expect("margin");
        let mut exposure = whole(0)?;
        for ((symbol, _), leg) in &self.legs {
            let q = absd(leg.ledger.view().quantity)?;
            if q.coefficient() > 0 {
                exposure = add(
                    exposure,
                    self.instruments[symbol].notional(
                        self.marks[symbol],
                        q,
                        FinancialRounding::Ceiling,
                    )?,
                )?;
            }
        }
        self.money_ceil(div(exposure, rules.leverage)?)
    }
    pub fn free_margin(&self) -> Result<Decimal> {
        let pending = sum(self
            .orders
            .iter()
            .filter(|o| Self::active(o) && o.entry)
            .map(|o| o.held))?;
        sub(sub(self.equity()?, self.margin_used()?)?, pending)
    }
    // Quantity is rounded down to lots; the exact margin+fee check below is
    // monotone, so binary search never overspends a budget through rounding.
    pub fn budget_quantity(
        &self,
        symbol: &str,
        price: Decimal,
        budget: Decimal,
        maximum: Option<Decimal>,
    ) -> Result<Decimal> {
        let i = &self.instruments[symbol];
        let step = i.declaration().quantity_step;
        let max = maximum.unwrap_or(parse("1000000", step.scale())?);
        let mut low = 0_i128;
        let mut high = max
            .rescale(step.scale(), Rounding::Exact)
            .map_err(numeric_error)?
            .coefficient()
            / step.coefficient();
        while low < high {
            let middle = low + (high - low + 1) / 2;
            let q =
                Decimal::new(middle * step.coefficient(), step.scale()).map_err(numeric_error)?;
            let gross = i.notional(price, q, FinancialRounding::HalfAwayFromZero)?;
            let exposure = i.notional(price, q, FinancialRounding::Ceiling)?;
            let cost = self.entry_cost(exposure, self.fee(gross)?)?;
            if wide(cost)?.coefficient() <= wide(budget)?.coefficient() {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        Decimal::new(low * step.coefficient(), step.scale()).map_err(numeric_error)
    }
    pub(super) fn ensure_clearing_cash(&mut self, amount: Decimal, time: u64) -> Result<()> {
        if self.margin.is_some() {
            let deficit = sub(amount, self.hub.view().available_cash)?;
            if deficit.coefficient() > 0 {
                self.post_hub(time, LedgerAction::Borrow { amount: deficit })?;
            }
        }
        Ok(())
    }
    pub(super) fn repay_clearing(&mut self, time: u64) -> Result<()> {
        if self.margin.is_some() {
            let view = self.hub.view();
            let amount = mind(view.debt, view.available_cash);
            if amount.coefficient() > 0 {
                self.post_hub(time, LedgerAction::Repay { amount })?;
            }
        }
        Ok(())
    }
    pub fn settled_balance(&self) -> Result<Decimal> {
        sub(
            add(
                self.initial,
                sum(self.legs.values().map(|l| l.ledger.view().realized_gross))?,
            )?,
            self.fees,
        )
    }
    pub fn margin_snapshot(&self) -> Result<V> {
        let rules = self.margin.as_ref().expect("margin");
        Ok(o([
            ("leverage", ds(rules.leverage)),
            ("entry_percent", ds(rules.percent)),
            ("balance", ds(self.settled_balance()?)),
            ("used", ds(self.margin_used()?)),
            ("available", ds(self.free_margin()?)),
            (
                "maintenance",
                ds(self.money_ceil(div(self.margin_used()?, whole(2)?)?)?),
            ),
            ("status", s(rules.status)),
        ]))
    }
    pub fn check_maintenance(&mut self, time: u64) -> Result<()> {
        if self.margin.is_none() {
            return Ok(());
        }
        let has_position = self
            .legs
            .values()
            .any(|l| l.ledger.view().quantity.coefficient() != 0);
        let equity = self.equity()?;
        if self.margin.as_ref().expect("margin").status != "healthy" {
            if !has_position {
                self.margin.as_mut().expect("margin").status = if equity.coefficient() < 0 {
                    "insolvent"
                } else {
                    "liquidated"
                };
            }
            return Ok(());
        }
        let maintenance = self.money_ceil(div(self.margin_used()?, whole(2)?)?)?;
        if !has_position || wide(equity)?.coefficient() >= wide(maintenance)?.coefficient() {
            return Ok(());
        }
        self.margin.as_mut().expect("margin").status = "liquidating";
        for index in 0..self.orders.len() {
            if Self::active(&self.orders[index]) {
                self.cancel(index, "cancelled_margin_call", time)?;
            }
        }
        let keys: Vec<_> = self.legs.keys().cloned().collect();
        for (symbol, side) in keys {
            let quantity = self.quantity(&symbol, &side)?;
            if quantity.coefficient() == 0 {
                continue;
            }
            self.submit(
                Order {
                    id: format!("__fqe.margin.{}.{}", symbol, side),
                    node: "__fqe.margin".into(),
                    symbol,
                    side,
                    entry: false,
                    kind: "market".into(),
                    quantity,
                    remaining: quantity,
                    limit: None,
                    stop: None,
                    triggered: true,
                    tif: "GTC".into(),
                    ttl: 1,
                    age: 0,
                    tag: "margin_liquidation".into(),
                    status: "accepted".into(),
                    cap: parse("0", self.ms)?,
                    held: parse("0", self.ms)?,
                    hold: None,
                    protection: None,
                    submitted: time,
                },
                time,
            )?;
        }
        Ok(())
    }
}
