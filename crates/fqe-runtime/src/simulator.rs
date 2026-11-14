//! Explicit cash-long next-open/prior-closed-volume simulator. No live routing or intrabar fills.
use crate::margin::{MARGIN_MODEL, MarginRules, MarginStatus};
use crate::{events::EventObservation, ledger::*};
use fqe_core::{
    bars::ClosedBar,
    finance::{FinancialError, FinancialRounding, align},
    numeric::Decimal,
    types::ScalarValue,
};
use fqe_ir::model::ContentId;
use std::{collections::BTreeSet, fmt};
pub const MODEL: &str = "fqe.next-open-prior-volume/1";
pub const MAX_ORDERS: usize = 128;
pub const MAX_COMMANDS: usize = 2048;
pub const MAX_FILLS: usize = 2048;
type Result<T> = std::result::Result<T, SimulationError>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationError {
    pub code: &'static str,
    pub detail: String,
}
impl fmt::Display for SimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}
impl std::error::Error for SimulationError {}
fn err(code: &'static str, detail: &str) -> SimulationError {
    SimulationError {
        code,
        detail: detail.into(),
    }
}
impl From<FinancialError> for SimulationError {
    fn from(e: FinancialError) -> Self {
        err("simulation_units", &e.to_string())
    }
}
impl From<LedgerError> for SimulationError {
    fn from(e: LedgerError) -> Self {
        err(e.code, &e.detail)
    }
}
fn decimal(n: i128, scale: u8) -> Result<Decimal> {
    Decimal::new(n, scale).map_err(|_| err("simulation_overflow", "decimal bound"))
}
fn id(s: &str) -> Result<()> {
    fqe_ir::model::identifier(s, "$.id").map_err(|_| err("simulation_id", "invalid identifier"))
}
fn price(v: &fqe_core::types::TypedValue) -> Result<Decimal> {
    match v.value() {
        ScalarValue::Decimal(v) => Ok(*v),
        _ => Err(err("simulation_bar", "expected decimal")),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignalSource {
    pub run: String,
    pub execution_id: ContentId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationConfig {
    pub margin: Option<MarginRules>,
    pub limits: SimulationLimits,
    pub account: AccountConfig,
    pub timeframe_ns: u64,
    pub participation_bps: u32,
    pub fee_bps: u32,
    pub minimum_fee: Decimal,
    pub slippage_bps: u32,
    pub signal_source: Option<SignalSource>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationLimits {
    pub orders: usize,
    pub commands: usize,
    pub fills: usize,
}
impl Default for SimulationLimits {
    fn default() -> Self {
        Self {
            orders: MAX_ORDERS,
            commands: MAX_COMMANDS,
            fills: MAX_FILLS,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignalOrigin {
    pub source: SignalSource,
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub index: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntentOrigin {
    Manual,
    Signal(SignalOrigin),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderKind {
    Market,
    Limit(Decimal),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderIntent {
    pub id: String,
    pub side: TradeSide,
    pub kind: OrderKind,
    pub quantity: Decimal,
    pub cash_cap: Option<Decimal>,
    pub origin: IntentOrigin,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LongBuyPolicy {
    pub quantity: Decimal,
    pub cash_cap: Decimal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderStatus {
    Accepted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Replaced,
    Rejected,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    MarginLimit,
    InsufficientCash,
    InsufficientPosition,
    CashCap,
    PriceOrModelRange,
    AccountLimits,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimOrder {
    pub liquidation: bool,
    pub number: u64,
    pub intent: OrderIntent,
    pub submitted_after: u64,
    pub filled_quantity: Decimal,
    pub status: OrderStatus,
    pub rejection: Option<Rejection>,
}
impl SimOrder {
    pub fn live(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Accepted | OrderStatus::PartiallyFilled
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimFill {
    pub number: u64,
    pub order: String,
    pub source_bar: u64,
    pub timestamp_ns: u64,
    pub side: TradeSide,
    pub price: Decimal,
    pub quantity: Decimal,
    pub settled_gross: Decimal,
    pub fee: Decimal,
    pub ledger_sequence: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimulationCommand {
    Bar(Box<ClosedBar>),
    Submit(OrderIntent),
    Cancel {
        order: String,
    },
    Replace {
        order: String,
        replacement: OrderIntent,
    },
    Finish,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimulationEvent {
    Margin {
        status: MarginStatus,
        source_bar: u64,
        mark: Decimal,
        equity: Decimal,
        exposure: Decimal,
        required: Decimal,
    },
    Order(SimOrder),
    Fill(SimFill),
    Finished,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationOpening {
    pub model: String,
    pub config: SimulationConfig,
    pub engine_id: ContentId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationReceipt {
    pub margin: Option<MarginStatus>,
    pub sequence: u64,
    pub opening: Option<SimulationOpening>,
    pub command: SimulationCommand,
    pub events: Vec<SimulationEvent>,
    pub account: AccountView,
}
#[derive(Clone, Debug)]
pub struct SimulationBroker {
    margin: Option<MarginStatus>,
    config: SimulationConfig,
    ledger: VirtualLedger,
    orders: Vec<SimOrder>,
    fills: Vec<SimFill>,
    last: Option<ClosedBar>,
    signal_ids: BTreeSet<String>,
    receipts: Vec<SimulationReceipt>,
    finished: bool,
}
impl SimulationBroker {
    pub fn new(config: SimulationConfig) -> Result<Self> {
        match (&config.margin, config.account.profile) {
            (None, LedgerProfile::CashLong) => {}
            (Some(rules), LedgerProfile::MarginLong) => rules.validate()?,
            _ => {
                return Err(err(
                    "simulation_config",
                    "account and margin profile are incompatible",
                ));
            }
        }
        if config.limits.orders == 0
            || config.limits.orders > MAX_ORDERS
            || config.limits.commands < 2
            || config.limits.commands > MAX_COMMANDS
            || config.limits.fills == 0
            || config.limits.fills > MAX_FILLS
        {
            return Err(err("simulation_limit", "invalid configured capacities"));
        }
        if config.timeframe_ns == 0
            || config.participation_bps > 10000
            || config.fee_bps > 10000
            || config.slippage_bps >= 10000
        {
            return Err(err(
                "simulation_config",
                "cash-long, positive timeframe, bounded bps required",
            ));
        }
        if config
            .account
            .instrument
            .money(config.minimum_fee)?
            .coefficient()
            < 0
        {
            return Err(err("simulation_config", "minimum fee must be nonnegative"));
        }
        if let Some(source) = &config.signal_source {
            id(&source.run)?;
        }
        let ledger = VirtualLedger::new(config.account.clone())?;
        Ok(Self {
            margin: config.margin.as_ref().map(|_| MarginStatus::Healthy),
            config,
            ledger,
            orders: vec![],
            fills: vec![],
            last: None,
            signal_ids: BTreeSet::new(),
            receipts: vec![],
            finished: false,
        })
    }
    pub fn config(&self) -> &SimulationConfig {
        &self.config
    }
    pub fn model(&self) -> &'static str {
        if self.config.margin.is_some() {
            MARGIN_MODEL
        } else {
            MODEL
        }
    }
    pub fn ledger(&self) -> &VirtualLedger {
        &self.ledger
    }
    pub fn orders(&self) -> &[SimOrder] {
        &self.orders
    }
    pub fn fills(&self) -> &[SimFill] {
        &self.fills
    }
    pub fn receipts(&self) -> &[SimulationReceipt] {
        &self.receipts
    }
    pub fn finished(&self) -> bool {
        self.finished
    }
    pub fn margin_status(&self) -> Option<MarginStatus> {
        self.margin
    }
    pub fn order(&self, id: &str) -> Option<&SimOrder> {
        self.orders.iter().find(|o| o.intent.id == id)
    }
    fn qs(&self) -> u8 {
        self.config
            .account
            .instrument
            .declaration()
            .quantity_step
            .scale()
    }
    fn ms(&self) -> u8 {
        self.config
            .account
            .instrument
            .declaration()
            .money_step
            .scale()
    }
    fn time(&self) -> Result<u64> {
        self.last
            .as_ref()
            .map(|b| b.input().close_ns)
            .ok_or_else(|| {
                err(
                    "simulation_frontier",
                    "observe a closed bar before placing orders",
                )
            })
    }
    fn hold(order: &SimOrder) -> String {
        format!("sim.hold.{}", order.number)
    }
    fn journal(&mut self, order: u64, kind: &str, time: u64, action: LedgerAction) -> Result<u64> {
        let receipt = self.receipts.len() + 1;
        let n = self.ledger.journal().len() + 1;
        self.ledger.apply(LedgerTransaction {
            id: format!("sim:{receipt}:{order}:{kind}:{n}"),
            timestamp_ns: time,
            action,
        })?;
        Ok(self.ledger.journal().last().expect("posted").sequence)
    }
    /// Returns an intent only. The caller must explicitly submit it; no order/fill/hold is created here.
    pub fn intent_from_long(
        &self,
        policy: &LongBuyPolicy,
        observation: &EventObservation,
        index: usize,
        order: &str,
    ) -> Result<OrderIntent> {
        if self.finished
            || self.margin.is_some_and(|s| s != MarginStatus::Healthy)
            || observation.warmup
            || !observation.matched
        {
            return Err(err(
                "simulation_signal",
                "terminal, warmup or unmatched observation",
            ));
        }
        let source = self
            .config
            .signal_source
            .as_ref()
            .ok_or_else(|| err("simulation_signal", "signal source is not configured"))?;
        let last = self
            .last
            .as_ref()
            .ok_or_else(|| err("simulation_frontier", "no closed bar"))?
            .input();
        if observation.run != source.run
            || observation.execution_id != source.execution_id
            || observation.source_sequence != last.sequence
            || observation.timestamp_ns != last.close_ns
        {
            return Err(err(
                "simulation_signal",
                "observation provenance/frontier differs",
            ));
        }
        let signal = observation
            .signals
            .get(index)
            .ok_or_else(|| err("simulation_signal", "signal index absent"))?;
        if signal.signal.kind != fqe_core::SignalKind::Long
            || signal.source_sequence != last.sequence
            || signal.timestamp_ns != last.close_ns
            || signal.signal.instrument != last.instrument
        {
            return Err(err("simulation_signal", "signal provenance differs"));
        }
        // SignalKind currently contains only Long. The explicit policy selects a market buy.
        let intent = OrderIntent {
            id: order.into(),
            side: TradeSide::Buy,
            kind: OrderKind::Market,
            quantity: policy.quantity,
            cash_cap: Some(policy.cash_cap),
            origin: IntentOrigin::Signal(SignalOrigin {
                source: source.clone(),
                sequence: last.sequence,
                timestamp_ns: last.close_ns,
                index,
            }),
        };
        self.validate_intent(&intent)?;
        Ok(intent)
    }
    fn signal_key(&self, intent: &OrderIntent) -> Result<Option<String>> {
        let IntentOrigin::Signal(origin) = &intent.origin else {
            return Ok(None);
        };
        let last = self
            .last
            .as_ref()
            .ok_or_else(|| err("simulation_frontier", "no closed bar"))?
            .input();
        if Some(&origin.source) != self.config.signal_source.as_ref()
            || origin.sequence != last.sequence
            || origin.timestamp_ns != last.close_ns
            || origin.index >= 1024
        {
            return Err(err(
                "simulation_signal",
                "signal origin is not current configured source",
            ));
        }
        Ok(Some(format!(
            "{}\n{}\n{}\n{}",
            origin.source.run, origin.source.execution_id, origin.sequence, origin.index
        )))
    }
    fn validate_intent(&self, intent: &OrderIntent) -> Result<()> {
        id(&intent.id)?;
        if self.config.margin.is_some() && intent.id.starts_with("__fqe.") {
            return Err(err("simulation_id", "reserved system order prefix"));
        }
        self.time()?;
        let instrument = &self.config.account.instrument;
        instrument.quantity(intent.quantity, false)?;
        let reference = match intent.kind {
            OrderKind::Market => price(&self.last.as_ref().expect("frontier").input().close)?,
            OrderKind::Limit(p) => instrument.price(p)?,
        };
        if reference.coefficient() <= 0 {
            return Err(err(
                "simulation_price",
                "positive order reference price required",
            ));
        }
        match (intent.side, intent.cash_cap) {
            (TradeSide::Buy, Some(cap)) if instrument.money(cap)?.coefficient() > 0 => {}
            (TradeSide::Sell, None) => {}
            _ => {
                return Err(err(
                    "simulation_cap",
                    "buy requires positive money cap; sell must omit it",
                ));
            }
        }
        let exact = decimal(
            reference
                .coefficient()
                .checked_mul(intent.quantity.coefficient())
                .ok_or_else(|| err("simulation_overflow", "reference notional"))?,
            reference.scale() + intent.quantity.scale(),
        )?;
        let min = instrument.declaration().minimum_notional;
        let scale = exact.scale().max(min.scale());
        let exact = exact
            .rescale(scale, fqe_core::numeric::Rounding::Exact)
            .map_err(|_| err("simulation_overflow", "reference precision"))?;
        let min = min
            .rescale(scale, fqe_core::numeric::Rounding::Exact)
            .map_err(|_| err("simulation_overflow", "minimum precision"))?;
        if exact.coefficient() < min.coefficient() {
            return Err(err(
                "simulation_minimum",
                "reference order notional is below minimum",
            ));
        }
        self.signal_key(intent)?;
        Ok(())
    }
    fn repay_free_cash(&mut self, time: u64) -> Result<()> {
        if self.config.margin.is_some() {
            let view = self.ledger.view();
            let n = view
                .available_cash
                .coefficient()
                .min(view.debt.coefficient());
            if n > 0 {
                self.journal(
                    0,
                    "repay",
                    time,
                    LedgerAction::Repay {
                        amount: decimal(n, self.ms())?,
                    },
                )?;
            }
        }
        Ok(())
    }
    fn margin_admissible(
        &self,
        mark: Decimal,
        new_cap: Option<Decimal>,
        completed: Option<usize>,
    ) -> Result<bool> {
        let Some(rules) = &self.config.margin else {
            return Ok(true);
        };
        let value = self.ledger.value_at(mark)?;
        if value.equity.coefficient() <= 0 {
            return Ok(false);
        }
        let mut exposure = value.market_value.coefficient();
        for (index, order) in self.orders.iter().enumerate() {
            if Some(index) != completed
                && order.live()
                && order.intent.side == TradeSide::Buy
                && let Some(hold) = self.ledger.reservations().get(&Self::hold(order))
            {
                exposure = exposure
                    .checked_add(hold.cash.coefficient())
                    .ok_or_else(|| err("simulation_overflow", "pending exposure"))?;
            }
        }
        exposure = exposure
            .checked_add(new_cap.map(|v| v.coefficient()).unwrap_or(0))
            .ok_or_else(|| err("simulation_overflow", "new exposure"))?;
        let limit = rules.exposure_limit(
            value.equity,
            self.config.account.instrument.declaration().money_step,
        )?;
        Ok(exposure <= limit.coefficient())
    }
    fn check_margin_close(
        &mut self,
        bar: &ClosedBar,
        events: &mut Vec<SimulationEvent>,
    ) -> Result<()> {
        let Some(rules) = self.config.margin.clone() else {
            return Ok(());
        };
        let b = bar.input();
        let mark = price(&b.close)?;
        let value = self.ledger.value_at(mark)?;
        let step = self.config.account.instrument.declaration().money_step;
        let required = rules.maintenance(value.market_value, step)?;
        if self.margin == Some(MarginStatus::Healthy)
            && (value.equity.coefficient() < required.coefficient()
                || value.equity.coefficient() < 0)
        {
            for index in 0..self.orders.len() {
                if self.orders[index].live() {
                    self.terminate(index, OrderStatus::Cancelled, None, b.close_ns, events)?;
                }
            }
            let quantity = self.ledger.view().quantity;
            if quantity.coefficient() > 0 {
                // One reserved system order lies outside the user order cap; it cannot be replaced/cancelled.
                let order = SimOrder {
                    liquidation: true,
                    number: self.orders.len() as u64 + 1,
                    intent: OrderIntent {
                        id: "__fqe.liquidation".into(),
                        side: TradeSide::Sell,
                        kind: OrderKind::Market,
                        quantity,
                        cash_cap: None,
                        origin: IntentOrigin::Manual,
                    },
                    submitted_after: b.sequence,
                    filled_quantity: decimal(0, self.qs())?,
                    status: OrderStatus::Accepted,
                    rejection: None,
                };
                self.journal(
                    order.number,
                    "liquidation_hold",
                    b.close_ns,
                    LedgerAction::Reserve {
                        reservation: Self::hold(&order),
                        amount: Reservation {
                            cash: decimal(0, self.ms())?,
                            quantity,
                        },
                    },
                )?;
                events.push(SimulationEvent::Order(order.clone()));
                self.orders.push(order);
                self.margin = Some(MarginStatus::Liquidating);
            } else {
                self.margin = Some(MarginStatus::Insolvent);
            }
        } else if self.margin == Some(MarginStatus::Liquidating) {
            if self.ledger.view().quantity.coefficient() == 0 {
                self.margin = Some(if self.ledger.view().debt.coefficient() == 0 {
                    MarginStatus::Liquidated
                } else {
                    MarginStatus::Insolvent
                });
            } else if self.orders.iter().any(|o| o.liquidation && !o.live()) {
                self.margin = Some(if value.equity.coefficient() < 0 {
                    MarginStatus::Insolvent
                } else {
                    MarginStatus::LiquidationFailed
                });
            }
        }
        let value = self.ledger.value_at(mark)?;
        events.push(SimulationEvent::Margin {
            status: self.margin.expect("configured"),
            source_bar: b.sequence,
            mark,
            equity: value.equity,
            exposure: value.market_value,
            required: rules.maintenance(value.market_value, step)?,
        });
        Ok(())
    }
    fn quote(&self, raw: Decimal, side: TradeSide, kind: &OrderKind) -> Result<Decimal> {
        let instrument = &self.config.account.instrument;
        let step = instrument.declaration().price_step;
        let factor = if side == TradeSide::Buy {
            10000 + self.config.slippage_bps
        } else {
            10000 - self.config.slippage_bps
        };
        let numerator = raw
            .coefficient()
            .checked_mul(i128::from(factor))
            .ok_or_else(|| err("simulation_overflow", "slippage numerator"))?;
        let denominator = step.coefficient() * 10000;
        let mut units = numerator / denominator;
        if side == TradeSide::Buy && numerator % denominator != 0 {
            units += 1;
        }
        let mut n = units * step.coefficient();
        if let OrderKind::Limit(limit) = kind {
            n = if side == TradeSide::Buy {
                n.min(limit.coefficient())
            } else {
                n.max(limit.coefficient())
            };
        }
        let result = instrument.price(decimal(n, step.scale())?)?;
        if result.coefficient() <= 0 {
            return Err(err("simulation_price", "nonpositive execution price"));
        }
        Ok(result)
    }
    fn cost(&self, price: Decimal, quantity: Decimal) -> Result<(Decimal, Decimal)> {
        let i = &self.config.account.instrument;
        let step = i.declaration().money_step;
        let exact = decimal(
            price
                .coefficient()
                .checked_mul(quantity.coefficient())
                .ok_or_else(|| err("simulation_overflow", "gross product"))?,
            price.scale() + quantity.scale(),
        )?;
        let gross = i.money(align(exact, step, FinancialRounding::HalfAwayFromZero)?)?;
        if gross.coefficient() <= 0 {
            return Err(err("simulation_price", "zero money-step gross unsupported"));
        }
        // Fee rate applies to settled gross in this model, then half-away to the money step.
        let num = gross.coefficient() * i128::from(self.config.fee_bps);
        let den = step.coefficient() * 10000;
        let mut units = num / den;
        let rem = num % den;
        if rem >= den - rem {
            units += 1;
        }
        let fee = i.money(decimal(
            (units * step.coefficient()).max(self.config.minimum_fee.coefficient()),
            self.ms(),
        )?)?;
        Ok((gross, fee))
    }
    fn submit(&mut self, intent: OrderIntent, events: &mut Vec<SimulationEvent>) -> Result<bool> {
        self.validate_intent(&intent)?;
        if self.orders.len() >= self.config.limits.orders {
            return Err(err("simulation_limit", "order capacity exhausted"));
        }
        if self.order(&intent.id).is_some() {
            return Err(err("simulation_duplicate", "order ID already used"));
        }
        let key = self.signal_key(&intent)?;
        if key.as_ref().is_some_and(|k| self.signal_ids.contains(k)) {
            return Err(err("simulation_duplicate", "signal already consumed"));
        }
        let mut order = SimOrder {
            liquidation: false,
            number: self.orders.len() as u64 + 1,
            intent,
            submitted_after: self.last.as_ref().expect("frontier").input().sequence,
            filled_quantity: decimal(0, self.qs())?,
            status: OrderStatus::Accepted,
            rejection: None,
        };
        let view = self.ledger.view();
        let known_mark = price(&self.last.as_ref().expect("frontier").input().close)?;
        let rejection = if order.intent.side == TradeSide::Buy
            && !self.margin_admissible(known_mark, order.intent.cash_cap, None)?
        {
            Some(Rejection::MarginLimit)
        } else {
            match order.intent.side {
                TradeSide::Buy => {
                    let cap = order.intent.cash_cap.expect("validated");
                    let reference = match order.intent.kind {
                        OrderKind::Market => {
                            price(&self.last.as_ref().expect("frontier").input().close)?
                        }
                        OrderKind::Limit(p) => p,
                    };
                    let quote = self.quote(reference, order.intent.side, &order.intent.kind);
                    match quote.and_then(|p| self.cost(p, order.intent.quantity)) {
                        Err(_) => Some(Rejection::PriceOrModelRange),
                        Ok((gross, fee))
                            if gross.coefficient() + fee.coefficient() > cap.coefficient() =>
                        {
                            Some(Rejection::CashCap)
                        }
                        Ok(_)
                            if self.config.margin.is_none()
                                && cap.coefficient() > view.available_cash.coefficient() =>
                        {
                            Some(Rejection::InsufficientCash)
                        }
                        _ => None,
                    }
                }
                TradeSide::Sell
                    if order.intent.quantity.coefficient()
                        > view.available_long_quantity.coefficient() =>
                {
                    Some(Rejection::InsufficientPosition)
                }
                _ => None,
            }
        };
        if let Some(reason) = rejection {
            order.status = OrderStatus::Rejected;
            order.rejection = Some(reason);
        } else {
            if self.config.margin.is_some() && order.intent.side == TradeSide::Buy {
                let cap = order.intent.cash_cap.expect("validated");
                let needed = cap.coefficient() - view.available_cash.coefficient();
                if needed > 0 {
                    self.journal(
                        order.number,
                        "borrow",
                        self.time()?,
                        LedgerAction::Borrow {
                            amount: decimal(needed, self.ms())?,
                        },
                    )?;
                }
            }
            let amount = Reservation {
                cash: order.intent.cash_cap.unwrap_or(decimal(0, self.ms())?),
                quantity: if order.intent.side == TradeSide::Sell {
                    order.intent.quantity
                } else {
                    decimal(0, self.qs())?
                },
            };
            self.journal(
                order.number,
                "reserve",
                self.time()?,
                LedgerAction::Reserve {
                    reservation: Self::hold(&order),
                    amount,
                },
            )?;
        }
        let accepted = order.live();
        if let Some(key) = key {
            self.signal_ids.insert(key);
        }
        events.push(SimulationEvent::Order(order.clone()));
        self.orders.push(order);
        Ok(accepted)
    }
    fn terminate(
        &mut self,
        index: usize,
        status: OrderStatus,
        reason: Option<Rejection>,
        time: u64,
        events: &mut Vec<SimulationEvent>,
    ) -> Result<()> {
        let order = &self.orders[index];
        let hold = Self::hold(order);
        let number = order.number;
        if self.ledger.reservations().contains_key(&hold) {
            self.journal(
                number,
                "release",
                time,
                LedgerAction::Release { reservation: hold },
            )?;
        }
        let order = &mut self.orders[index];
        order.status = status;
        order.rejection = reason;
        events.push(SimulationEvent::Order(order.clone()));
        self.repay_free_cash(time)?;
        Ok(())
    }
    fn bar(&mut self, bar: ClosedBar, events: &mut Vec<SimulationEvent>) -> Result<()> {
        let b = bar.input();
        let i = &self.config.account.instrument;
        let d = i.declaration();
        if b.instrument != d.instrument
            || b.timeframe_ns != self.config.timeframe_ns
            || price(&b.close)?.scale() != d.price_step.scale()
            || price(&b.volume)?.scale() != self.qs()
        {
            return Err(err("simulation_bar", "instrument/timeframe/scales differ"));
        }
        for value in [&b.open, &b.high, &b.low, &b.close] {
            i.price(price(value)?)?;
        }
        if price(&b.volume)?.coefficient()
            > 1_000_000_000_000_i128 * 10_i128.pow(u32::from(self.qs()))
        {
            return Err(err("simulation_bar", "volume limit"));
        }
        if let Some(last) = &self.last
            && (b.sequence <= last.input().sequence || b.open_ns != last.input().close_ns)
        {
            return Err(err(
                "simulation_bar",
                "continuous increasing source bars required",
            ));
        }
        let previous_volume = match &self.last {
            Some(last) => price(&last.input().volume)?.coefficient(),
            None => 0,
        };
        let lot = d.quantity_step.coefficient();
        let raw = previous_volume * i128::from(self.config.participation_bps) / 10000;
        let mut liquidity = raw / lot * lot;
        for index in 0..self.orders.len() {
            if !self.orders[index].live() || liquidity == 0 {
                continue;
            }
            let order = self.orders[index].clone();
            if order.submitted_after >= b.sequence {
                return Err(err("simulation_timing", "same-bar order cannot execute"));
            }
            let raw_open = price(&b.open)?;
            if let OrderKind::Limit(limit) = order.intent.kind
                && ((order.intent.side == TradeSide::Buy
                    && raw_open.coefficient() > limit.coefficient())
                    || (order.intent.side == TradeSide::Sell
                        && raw_open.coefficient() < limit.coefficient()))
            {
                continue;
            }
            let n = (order.intent.quantity.coefficient() - order.filled_quantity.coefficient())
                .min(liquidity);
            let quantity = decimal(n, self.qs())?;
            let costs = self
                .quote(raw_open, order.intent.side, &order.intent.kind)
                .and_then(|p| self.cost(p, quantity).map(|(g, f)| (p, g, f)));
            let (fill_price, gross, fee) = match costs {
                Ok(values) => values,
                Err(_) => {
                    self.terminate(
                        index,
                        OrderStatus::Rejected,
                        Some(Rejection::PriceOrModelRange),
                        b.open_ns,
                        events,
                    )?;
                    continue;
                }
            };
            let hold = self
                .ledger
                .reservations()
                .get(&Self::hold(&order))
                .cloned()
                .ok_or_else(|| err("simulation_invariant", "live order has no reservation"))?;
            let spend = gross.coefficient() + fee.coefficient();
            if order.intent.side == TradeSide::Buy && spend > hold.cash.coefficient() {
                self.terminate(
                    index,
                    OrderStatus::Rejected,
                    Some(Rejection::CashCap),
                    b.open_ns,
                    events,
                )?;
                continue;
            }
            if self.fills.len() >= self.config.limits.fills {
                return Err(err("simulation_limit", "fill capacity exhausted"));
            }
            let release = ReservationRelease {
                reservation: Self::hold(&order),
                cash: if order.intent.side == TradeSide::Buy {
                    decimal(spend, self.ms())?
                } else {
                    decimal(0, self.ms())?
                },
                quantity: if order.intent.side == TradeSide::Sell {
                    quantity
                } else {
                    decimal(0, self.qs())?
                },
            };
            let action = LedgerAction::Trade {
                side: order.intent.side,
                price: fill_price,
                quantity,
                fee,
                release: Some(release),
            };
            let before_margin_buy =
                if self.config.margin.is_some() && order.intent.side == TradeSide::Buy {
                    Some(self.ledger.clone())
                } else {
                    None
                };
            let ledger_sequence = match self.journal(order.number, "fill", b.open_ns, action) {
                Ok(n) => n,
                Err(e) if matches!(e.code, "ledger_funds" | "ledger_units") => {
                    self.terminate(
                        index,
                        OrderStatus::Rejected,
                        Some(Rejection::AccountLimits),
                        b.open_ns,
                        events,
                    )?;
                    continue;
                }
                Err(e) => return Err(e),
            };
            let filled = order.filled_quantity.coefficient() + n;
            if let Some(previous) = before_margin_buy {
                let completed = if filled == order.intent.quantity.coefficient() {
                    Some(index)
                } else {
                    None
                };
                if !matches!(self.margin_admissible(raw_open, None, completed), Ok(true)) {
                    self.ledger = previous;
                    self.terminate(
                        index,
                        OrderStatus::Rejected,
                        Some(Rejection::MarginLimit),
                        b.open_ns,
                        events,
                    )?;
                    continue;
                }
            }
            liquidity -= n;
            self.orders[index].filled_quantity = decimal(filled, self.qs())?;
            let fill = SimFill {
                number: self.fills.len() as u64 + 1,
                order: order.intent.id,
                source_bar: b.sequence,
                timestamp_ns: b.open_ns,
                side: order.intent.side,
                price: fill_price,
                quantity,
                settled_gross: gross,
                fee,
                ledger_sequence,
            };
            events.push(SimulationEvent::Fill(fill.clone()));
            self.fills.push(fill);
            if filled == order.intent.quantity.coefficient() {
                self.terminate(index, OrderStatus::Filled, None, b.open_ns, events)?;
            } else if !self
                .ledger
                .reservations()
                .contains_key(&Self::hold(&self.orders[index]))
            {
                self.terminate(
                    index,
                    OrderStatus::Rejected,
                    Some(Rejection::CashCap),
                    b.open_ns,
                    events,
                )?;
            } else {
                self.orders[index].status = OrderStatus::PartiallyFilled;
                events.push(SimulationEvent::Order(self.orders[index].clone()));
            }
        }
        self.repay_free_cash(b.close_ns)?;
        self.check_margin_close(&bar, events)?;
        self.last = Some(bar);
        Ok(())
    }
    /// Whole commands, including multi-order bar fills, are staged and committed atomically.
    pub fn apply(&mut self, command: SimulationCommand) -> Result<&SimulationReceipt> {
        if self.finished {
            return Err(err("simulation_finished", "simulation is terminal"));
        }
        if self.margin.is_some_and(|s| s != MarginStatus::Healthy)
            && matches!(
                &command,
                SimulationCommand::Submit(_) | SimulationCommand::Replace { .. }
            )
        {
            return Err(err(
                "simulation_margin",
                "new risk is disabled after margin breach",
            ));
        }
        if self.receipts.len() >= self.config.limits.commands
            || (!matches!(&command, SimulationCommand::Finish)
                && self.receipts.len() + 1 >= self.config.limits.commands)
        {
            return Err(err("simulation_limit", "command capacity exhausted"));
        }
        let mut next = self.clone();
        let mut events = vec![];
        match &command {
            SimulationCommand::Bar(bar) => next.bar((**bar).clone(), &mut events)?,
            SimulationCommand::Submit(intent) => {
                next.submit(intent.clone(), &mut events)?;
            }
            SimulationCommand::Cancel { order } => {
                let index = next
                    .orders
                    .iter()
                    .position(|o| o.intent.id == *order && o.live() && !o.liquidation)
                    .ok_or_else(|| err("simulation_order", "order is absent or terminal"))?;
                next.terminate(
                    index,
                    OrderStatus::Cancelled,
                    None,
                    next.time()?,
                    &mut events,
                )?;
            }
            SimulationCommand::Replace { order, replacement } => {
                if *order == replacement.id {
                    return Err(err("simulation_duplicate", "replacement needs a fresh ID"));
                }
                let index = next
                    .orders
                    .iter()
                    .position(|o| o.intent.id == *order && o.live() && !o.liquidation)
                    .ok_or_else(|| err("simulation_order", "replace needs live order"))?;
                next.terminate(
                    index,
                    OrderStatus::Replaced,
                    None,
                    next.time()?,
                    &mut events,
                )?;
                if !next.submit(replacement.clone(), &mut events)? {
                    return Err(err(
                        "simulation_replace",
                        "replacement was rejected; original remains unchanged",
                    ));
                }
            }
            SimulationCommand::Finish => {
                let time = next.last.as_ref().map(|b| b.input().close_ns).unwrap_or(0);
                for index in 0..next.orders.len() {
                    if next.orders[index].live() {
                        next.terminate(index, OrderStatus::Cancelled, None, time, &mut events)?;
                    }
                }
                next.finished = true;
                if matches!(
                    next.margin,
                    Some(MarginStatus::Healthy | MarginStatus::Liquidating)
                ) && next.ledger.view().quantity.coefficient() > 0
                {
                    next.margin = Some(MarginStatus::Stopped);
                }
                events.push(SimulationEvent::Finished);
            }
        }
        let opening = if next.receipts.is_empty() {
            Some(SimulationOpening {
                model: next.model().into(),
                config: next.config.clone(),
                engine_id: crate::source_identity::engine_id(),
            })
        } else {
            None
        };
        next.receipts.push(SimulationReceipt {
            margin: next.margin,
            sequence: next.receipts.len() as u64 + 1,
            opening,
            command,
            events,
            account: next.ledger.view(),
        });
        *self = next;
        Ok(self.receipts.last().expect("committed"))
    }
    pub fn replay(config: SimulationConfig, receipts: &[SimulationReceipt]) -> Result<Self> {
        if receipts.len() > config.limits.commands {
            return Err(err("simulation_limit", "replay capacity exhausted"));
        }
        let mut next = Self::new(config)?;
        for recorded in receipts {
            if next.apply(recorded.command.clone())? != recorded {
                return Err(err(
                    "simulation_replay",
                    "receipt differs from complete recomputation",
                ));
            }
        }
        Ok(next)
    }
}
