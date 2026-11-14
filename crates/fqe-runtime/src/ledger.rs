//! Bounded native virtual ledger. These are accounting postings, not order admission or fills.
use fqe_core::{
    finance::{FinancialError, FinancialInstrument, FinancialRounding, PricePolicy, align},
    numeric::{Decimal, Rounding},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
pub const MAX_JOURNAL_ENTRIES: usize = 8192;
pub const MAX_RESERVATIONS: usize = 1024;
type Result<T> = std::result::Result<T, LedgerError>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerError {
    pub code: &'static str,
    pub detail: String,
}
impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}
impl std::error::Error for LedgerError {}
fn error(code: &'static str, detail: &str) -> LedgerError {
    LedgerError {
        code,
        detail: detail.into(),
    }
}
impl From<FinancialError> for LedgerError {
    fn from(e: FinancialError) -> Self {
        error("ledger_units", &e.to_string())
    }
}
fn add(a: i128, b: i128) -> Result<i128> {
    a.checked_add(b)
        .ok_or_else(|| error("ledger_overflow", "checked addition overflow"))
}
fn neg(a: i128) -> Result<i128> {
    a.checked_neg()
        .ok_or_else(|| error("ledger_overflow", "checked negation overflow"))
}
fn decimal(n: i128, scale: u8) -> Result<Decimal> {
    Decimal::new(n, scale).map_err(|_| error("ledger_overflow", "decimal bound"))
}
fn id(value: &str) -> Result<()> {
    fqe_ir::model::identifier(value, "$.id").map_err(|_| error("ledger_id", "invalid identifier"))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerProfile {
    CashLong,
    ProceedsRestrictedShort,
}
impl LedgerProfile {
    pub fn version(self) -> &'static str {
        match self {
            Self::CashLong => "fqe.gross-basis-cash-long/1",
            Self::ProceedsRestrictedShort => "fqe.gross-basis-restricted-short/1",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountConfig {
    pub account: String,
    pub instrument: FinancialInstrument,
    pub profile: LedgerProfile,
    pub initial_cash: Decimal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeSide {
    Buy,
    Sell,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FundingDirection {
    Deposit,
    Withdraw,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reservation {
    pub cash: Decimal,
    pub quantity: Decimal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReservationRelease {
    pub reservation: String,
    pub cash: Decimal,
    pub quantity: Decimal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerAction {
    Funding {
        direction: FundingDirection,
        amount: Decimal,
    },
    Reserve {
        reservation: String,
        amount: Reservation,
    },
    Release {
        reservation: String,
    },
    Trade {
        side: TradeSide,
        price: Decimal,
        quantity: Decimal,
        fee: Decimal,
        release: Option<ReservationRelease>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerTransaction {
    pub id: String,
    pub timestamp_ns: u64,
    pub action: LedgerAction,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoneyAccount {
    Cash,
    InventoryCost,
    ExternalCapital,
    RealizedProfit,
    FeeExpense,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoneyPosting {
    pub account: MoneyAccount,
    pub delta: Decimal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantityPostings {
    pub inventory: Decimal,
    pub counterparty: Decimal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountView {
    pub cash: Decimal,
    pub reserved_cash: Decimal,
    pub restricted_short_proceeds: Decimal,
    pub available_cash: Decimal,
    pub quantity: Decimal,
    pub reserved_quantity: Decimal,
    pub available_long_quantity: Decimal,
    pub cost: Decimal,
    pub realized_gross: Decimal,
    pub fees: Decimal,
    pub external_capital: Decimal,
    pub cash_rounding_total: Decimal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Valuation {
    pub market_value: Decimal,
    pub equity: Decimal,
    pub unrealized: Decimal,
    pub total_profit: Decimal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeAudit {
    pub exact_gross: Decimal,
    pub settled_gross: Decimal,
    pub allocated_basis: Decimal,
    pub cash_rounding: Decimal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEntry {
    pub opening: Option<LedgerOpening>,
    pub sequence: u64,
    pub transaction: LedgerTransaction,
    pub money: Vec<MoneyPosting>,
    pub quantity: Option<QuantityPostings>,
    pub trade: Option<TradeAudit>,
    pub after: AccountView,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerOpening {
    pub config: AccountConfig,
    pub engine_id: fqe_ir::model::ContentId,
}
#[derive(Clone, Debug)]
struct State {
    cash: i128,
    quantity: i128,
    cost: i128,
    realized: i128,
    fees: i128,
    capital: i128,
    rounding: i128,
    reservations: BTreeMap<String, Reservation>,
    reservation_ids: BTreeSet<String>,
}
#[derive(Clone, Debug)]
pub struct VirtualLedger {
    config: AccountConfig,
    state: State,
    journal: Vec<JournalEntry>,
    ids: BTreeSet<String>,
}
impl VirtualLedger {
    pub fn new(config: AccountConfig) -> Result<Self> {
        id(&config.account)?;
        if config.instrument.declaration().prices != PricePolicy::NonNegative {
            return Err(error(
                "ledger_profile",
                "these account profiles require nonnegative-price instruments",
            ));
        }
        let cash = config.instrument.money(config.initial_cash)?.coefficient();
        if cash < 0 {
            return Err(error("ledger_funds", "initial cash must be nonnegative"));
        }
        let state = State {
            cash,
            quantity: 0,
            cost: 0,
            realized: 0,
            fees: 0,
            capital: cash,
            rounding: 0,
            reservations: BTreeMap::new(),
            reservation_ids: BTreeSet::new(),
        };
        let mut ledger = Self {
            config,
            state,
            journal: vec![],
            ids: BTreeSet::from(["opening".into()]),
        };
        let entry = JournalEntry {
            opening: Some(LedgerOpening {
                config: ledger.config.clone(),
                engine_id: crate::source_identity::engine_id(),
            }),
            sequence: 1,
            transaction: LedgerTransaction {
                id: "opening".into(),
                timestamp_ns: 0,
                action: LedgerAction::Funding {
                    direction: FundingDirection::Deposit,
                    amount: ledger.config.initial_cash,
                },
            },
            money: vec![
                ledger.posting(MoneyAccount::Cash, cash)?,
                ledger.posting(MoneyAccount::ExternalCapital, neg(cash)?)?,
            ],
            quantity: None,
            trade: None,
            after: ledger.view_state(&ledger.state)?,
        };
        ledger.journal.push(entry);
        Ok(ledger)
    }
    pub fn config(&self) -> &AccountConfig {
        &self.config
    }
    pub fn journal(&self) -> &[JournalEntry] {
        &self.journal
    }
    pub fn reservations(&self) -> &BTreeMap<String, Reservation> {
        &self.state.reservations
    }
    pub fn view(&self) -> AccountView {
        self.view_state(&self.state).expect("committed invariant")
    }
    fn ms(&self) -> u8 {
        self.config.instrument.declaration().money_step.scale()
    }
    fn qs(&self) -> u8 {
        self.config.instrument.declaration().quantity_step.scale()
    }
    fn money(&self, n: i128) -> Result<Decimal> {
        Ok(self.config.instrument.money(decimal(n, self.ms())?)?)
    }
    fn posting(&self, account: MoneyAccount, n: i128) -> Result<MoneyPosting> {
        Ok(MoneyPosting {
            account,
            delta: self.money(n)?,
        })
    }
    fn checked_reservation(&self, r: &Reservation) -> Result<()> {
        if self.config.instrument.money(r.cash)?.coefficient() < 0 {
            return Err(error("ledger_reservation", "negative cash reservation"));
        }
        self.config.instrument.quantity_lots(r.quantity, true)?;
        Ok(())
    }
    fn view_state(&self, s: &State) -> Result<AccountView> {
        let mut rc = 0;
        let mut rq = 0;
        for r in s.reservations.values() {
            self.checked_reservation(r)?;
            rc = add(rc, r.cash.coefficient())?;
            rq = add(rq, r.quantity.coefficient())?;
        }
        let restricted = if s.quantity < 0 { neg(s.cost)? } else { 0 };
        let available = add(add(s.cash, neg(rc)?)?, neg(restricted)?)?;
        let available_quantity = add(s.quantity.max(0), neg(rq)?)?;
        if s.cash < 0
            || available < 0
            || available_quantity < 0
            || (s.quantity == 0 && s.cost != 0)
            || (s.quantity > 0 && s.cost < 0)
            || (s.quantity < 0 && s.cost > 0)
            || (self.config.profile == LedgerProfile::CashLong && s.quantity < 0)
        {
            return Err(error(
                "ledger_funds",
                "cash/position/reservation/profile invariant",
            ));
        }
        self.config
            .instrument
            .quantity_lots(decimal(s.quantity.abs(), self.qs())?, true)?;
        if add(s.cash, s.cost)? != add(add(s.capital, s.realized)?, neg(s.fees)?)? {
            return Err(error(
                "ledger_conservation",
                "cash+cost != external capital+realized gross-fees",
            ));
        }
        Ok(AccountView {
            cash: self.money(s.cash)?,
            reserved_cash: self.money(rc)?,
            restricted_short_proceeds: self.money(restricted)?,
            available_cash: self.money(available)?,
            quantity: decimal(s.quantity, self.qs())?,
            reserved_quantity: decimal(rq, self.qs())?,
            available_long_quantity: decimal(available_quantity, self.qs())?,
            cost: self.money(s.cost)?,
            realized_gross: self.money(s.realized)?,
            fees: self.money(s.fees)?,
            external_capital: self.money(s.capital)?,
            cash_rounding_total: decimal(s.rounding, 18)?,
        })
    }
    /// Mark is a caller-supplied positive price. This computes views and never changes journal state.
    pub fn value_at(&self, mark: Decimal) -> Result<Valuation> {
        self.config.instrument.price(mark)?;
        if mark.coefficient() <= 0 {
            return Err(error("ledger_mark", "positive mark required"));
        }
        let q = self.state.quantity;
        let exact = decimal(
            mark.coefficient()
                .checked_mul(q)
                .ok_or_else(|| error("ledger_overflow", "mark product"))?,
            mark.scale() + self.qs(),
        )?;
        let market = align(
            exact,
            self.config.instrument.declaration().money_step,
            FinancialRounding::HalfAwayFromZero,
        )?
        .coefficient();
        let equity = add(self.state.cash, market)?;
        let unrealized = add(market, neg(self.state.cost)?)?;
        let total = add(equity, neg(self.state.capital)?)?;
        if total != add(add(self.state.realized, unrealized)?, neg(self.state.fees)?)? {
            return Err(error("ledger_conservation", "valuation identity"));
        }
        Ok(Valuation {
            market_value: self.money(market)?,
            equity: self.money(equity)?,
            unrealized: self.money(unrealized)?,
            total_profit: self.money(total)?,
        })
    }
    /// Entire action and its journal receipt commit together. Failed candidates leave everything intact.
    pub fn apply(&mut self, transaction: LedgerTransaction) -> Result<&JournalEntry> {
        id(&transaction.id)?;
        if self.journal.len() >= MAX_JOURNAL_ENTRIES {
            return Err(error("ledger_limit", "journal capacity exhausted"));
        }
        if self.ids.contains(&transaction.id) {
            return Err(error("ledger_duplicate", "transaction ID already used"));
        }
        if transaction.timestamp_ns
            < self
                .journal
                .last()
                .expect("opening")
                .transaction
                .timestamp_ns
        {
            return Err(error("ledger_time", "transaction time moved backwards"));
        }
        let mut candidate = self.state.clone();
        let mut postings = vec![];
        let mut movement = None;
        let mut trade = None;
        match &transaction.action {
            LedgerAction::Funding { direction, amount } => {
                let n = self.config.instrument.money(*amount)?.coefficient();
                if n <= 0 {
                    return Err(error("ledger_funding", "positive funding amount required"));
                }
                let flow = if *direction == FundingDirection::Deposit {
                    n
                } else {
                    neg(n)?
                };
                candidate.cash = add(candidate.cash, flow)?;
                candidate.capital = add(candidate.capital, flow)?;
                postings.push(self.posting(MoneyAccount::Cash, flow)?);
                postings.push(self.posting(MoneyAccount::ExternalCapital, neg(flow)?)?);
            }
            LedgerAction::Reserve {
                reservation,
                amount,
            } => {
                id(reservation)?;
                self.checked_reservation(amount)?;
                if candidate.reservations.len() >= MAX_RESERVATIONS
                    || candidate.reservation_ids.len() >= MAX_JOURNAL_ENTRIES
                {
                    return Err(error("ledger_limit", "reservation capacity exhausted"));
                }
                if amount.cash.coefficient() == 0 && amount.quantity.coefficient() == 0 {
                    return Err(error("ledger_reservation", "empty reservation"));
                }
                if !candidate.reservation_ids.insert(reservation.clone()) {
                    return Err(error("ledger_duplicate", "reservation ID cannot be reused"));
                }
                candidate
                    .reservations
                    .insert(reservation.clone(), amount.clone());
            }
            LedgerAction::Release { reservation } => {
                if candidate.reservations.remove(reservation).is_none() {
                    return Err(error("ledger_reservation", "unknown reservation"));
                }
            }
            LedgerAction::Trade {
                side,
                price,
                quantity,
                fee,
                release,
            } => {
                self.config.instrument.price(*price)?;
                self.config.instrument.quantity_lots(*quantity, false)?;
                let fee = self.config.instrument.money(*fee)?.coefficient();
                if price.coefficient() <= 0 || fee < 0 {
                    return Err(error(
                        "ledger_trade",
                        "positive price/nonnegative fee required",
                    ));
                }
                if let Some(release) = release {
                    self.checked_reservation(&Reservation {
                        cash: release.cash,
                        quantity: release.quantity,
                    })?;
                    if release.cash.coefficient() == 0 && release.quantity.coefficient() == 0 {
                        return Err(error("ledger_reservation", "empty partial release"));
                    }
                    let r = candidate
                        .reservations
                        .get_mut(&release.reservation)
                        .ok_or_else(|| error("ledger_reservation", "unknown fill reservation"))?;
                    if release.cash.coefficient() > r.cash.coefficient()
                        || release.quantity.coefficient() > r.quantity.coefficient()
                    {
                        return Err(error(
                            "ledger_reservation",
                            "release exceeds owned reservation",
                        ));
                    }
                    r.cash = decimal(r.cash.coefficient() - release.cash.coefficient(), self.ms())?;
                    r.quantity = decimal(
                        r.quantity.coefficient() - release.quantity.coefficient(),
                        self.qs(),
                    )?;
                    if r.cash.coefficient() == 0 && r.quantity.coefficient() == 0 {
                        candidate.reservations.remove(&release.reservation);
                    }
                }
                let sign = if *side == TradeSide::Buy { 1 } else { -1 };
                let qty = quantity.coefficient();
                let old_q = candidate.quantity;
                let exact_gross = decimal(
                    price
                        .coefficient()
                        .checked_mul(qty)
                        .ok_or_else(|| error("ledger_overflow", "gross product"))?,
                    price.scale() + quantity.scale(),
                )?;
                let gross = self.config.instrument.money(align(
                    exact_gross,
                    self.config.instrument.declaration().money_step,
                    FinancialRounding::HalfAwayFromZero,
                )?)?;
                let gross_n = gross.coefficient();
                let mut allocation = 0;
                let cost_delta;
                let mut profit = 0;
                if old_q != 0 && old_q.signum() != sign {
                    if qty > old_q.abs() {
                        return Err(error(
                            "ledger_cross_zero",
                            "close and opposite open require separate postings",
                        ));
                    }
                    allocation = if qty == old_q.abs() {
                        candidate.cost.abs()
                    } else {
                        allocated_basis(
                            candidate.cost.abs(),
                            qty,
                            old_q.abs(),
                            self.config
                                .instrument
                                .declaration()
                                .money_step
                                .coefficient(),
                        )?
                    };
                    cost_delta = neg(allocation * old_q.signum())?;
                    profit = add(gross_n, neg(allocation)?)? * old_q.signum();
                } else {
                    cost_delta = gross_n * sign;
                }
                let flow = add(neg(gross_n * sign)?, neg(fee)?)?;
                candidate.cash = add(candidate.cash, flow)?;
                candidate.quantity = add(old_q, qty * sign)?;
                candidate.cost = add(candidate.cost, cost_delta)?;
                candidate.fees = add(candidate.fees, fee)?;
                candidate.realized = add(candidate.realized, profit)?;
                let exact18 = exact_gross
                    .rescale(18, Rounding::Exact)
                    .map_err(|_| error("ledger_overflow", "gross precision"))?;
                let rounded18 = gross
                    .rescale(18, Rounding::Exact)
                    .map_err(|_| error("ledger_overflow", "money precision"))?;
                // Residual is actual cash flow minus its unrounded equivalent, including side.
                let residual = -sign * add(rounded18.coefficient(), neg(exact18.coefficient())?)?;
                candidate.rounding = add(candidate.rounding, residual)?;
                postings = vec![
                    self.posting(MoneyAccount::Cash, flow)?,
                    self.posting(MoneyAccount::InventoryCost, cost_delta)?,
                    self.posting(MoneyAccount::RealizedProfit, neg(profit)?)?,
                    self.posting(MoneyAccount::FeeExpense, fee)?,
                ];
                movement = Some(QuantityPostings {
                    inventory: decimal(qty * sign, self.qs())?,
                    counterparty: decimal(-qty * sign, self.qs())?,
                });
                trade = Some(TradeAudit {
                    exact_gross,
                    settled_gross: gross,
                    allocated_basis: self.money(allocation)?,
                    cash_rounding: decimal(residual, 18)?,
                });
            }
        }
        let sum = postings
            .iter()
            .try_fold(0_i128, |sum, p| add(sum, p.delta.coefficient()))?;
        if sum != 0 {
            return Err(error("ledger_journal", "unbalanced monetary candidate"));
        }
        let after = self.view_state(&candidate)?;
        let entry = JournalEntry {
            opening: None,
            sequence: self.journal.len() as u64 + 1,
            transaction,
            money: postings,
            quantity: movement,
            trade,
            after,
        };
        self.ids.insert(entry.transaction.id.clone());
        self.state = candidate;
        self.journal.push(entry);
        Ok(self.journal.last().expect("committed"))
    }
    /// Untrusted receipts are recomputed; a failed restore never mutates an existing account.
    pub fn replay(config: AccountConfig, journal: &[JournalEntry]) -> Result<Self> {
        if journal.is_empty() || journal.len() > MAX_JOURNAL_ENTRIES {
            return Err(error(
                "ledger_replay",
                "missing opening or journal exceeds capacity",
            ));
        }
        let mut next = Self::new(config)?;
        if next.journal[0] != journal[0] {
            return Err(error("ledger_replay", "opening entry differs"));
        }
        for recorded in &journal[1..] {
            if next.apply(recorded.transaction.clone())? != recorded {
                return Err(error("ledger_replay", "recomputed journal receipt differs"));
            }
        }
        Ok(next)
    }
}
/// Proportional partial close basis, half-even to a money step; final close sweeps all residual basis.
fn allocated_basis(cost: i128, quantity: i128, total: i128, step: i128) -> Result<i128> {
    let numerator = cost
        .checked_mul(quantity)
        .ok_or_else(|| error("ledger_overflow", "allocation numerator"))?;
    let denominator = total
        .checked_mul(step)
        .ok_or_else(|| error("ledger_overflow", "allocation denominator"))?;
    let mut q = numerator / denominator;
    let rem = numerator % denominator;
    if rem > denominator - rem || (rem == denominator - rem && q % 2 != 0) {
        q = add(q, 1)?;
    }
    q.checked_mul(step)
        .ok_or_else(|| error("ledger_overflow", "allocation result"))
}
