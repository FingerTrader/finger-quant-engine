//! Native shared-capital broker. All cash/position postings use VirtualLedger.
use super::*;
mod margin;
pub use margin::{MARGIN_PROFILE, MarginConfig};
// Retain a bounded order history large enough for ordinary multi-day research.
pub const MAX_RESEARCH_ORDERS: usize = 8192;
type GraphViews = (
    BTreeMap<String, Slot>,
    BTreeMap<(String, String), BTreeMap<String, Slot>>,
);
use fqe_core::finance::{FinancialInstrument, FinancialRounding, align};
use fqe_runtime::ledger::*;
use graph::{Atom, Node, Slot};
#[derive(Clone)]
pub struct Leg {
    pub ledger: VirtualLedger,
    pub high: Option<Decimal>,
    pub low: Option<Decimal>,
    pub adds: u64,
    collateral: Option<String>,
    pub protection: Option<Protection>,
}
#[derive(Clone)]
pub struct Protection {
    pub node: Node,
    pub atr: Option<Decimal>,
    pub stop: Option<Decimal>,
}
#[derive(Clone)]
pub struct Order {
    pub id: String,
    pub node: String,
    pub symbol: String,
    pub side: String,
    pub entry: bool,
    pub kind: String,
    pub quantity: Decimal,
    pub remaining: Decimal,
    pub limit: Option<Decimal>,
    pub stop: Option<Decimal>,
    pub triggered: bool,
    pub tif: String,
    pub ttl: u64,
    pub age: u64,
    pub tag: String,
    pub status: String,
    pub cap: Decimal,
    pub held: Decimal,
    pub hold: Option<String>,
    pub protection: Option<Protection>,
    pub submitted: u64,
}
#[derive(Clone)]
pub struct Broker {
    pub hub: VirtualLedger,
    pub legs: BTreeMap<(String, String), Leg>,
    pub instruments: BTreeMap<String, FinancialInstrument>,
    pub orders: Vec<Order>,
    pub fills: Vec<V>,
    pub events: Vec<V>,
    pub marks: BTreeMap<String, Decimal>,
    pub fees: Decimal,
    pub initial: Decimal,
    pub margin: Option<MarginConfig>,
    serial: u64,
    fee_bps: Decimal,
    slippage: Decimal,
    ms: u8,
}
fn ledger_error(e: LedgerError) -> Failure {
    failure(e.code, format!("$.account.{}", e.detail))
}
impl Broker {
    pub fn new(
        instruments: BTreeMap<String, FinancialInstrument>,
        context: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let instrument = instruments
            .values()
            .next()
            .ok_or_else(|| failure("research_no_instrument", "$"))?
            .clone();
        let ms = instrument.declaration().money_step.scale();
        for i in instruments.values() {
            let a = i.declaration();
            let b = instrument.declaration();
            if a.currency != b.currency || a.money_step != b.money_step {
                return Err(failure(
                    "research_currency_conversion_required",
                    "$.streams",
                ));
            }
        }
        let initial = instrument.money(parse(
            context.get("initialCash").map(String::as_str).unwrap_or(""),
            ms,
        )?)?;
        let margin = MarginConfig::parse(context)?;
        let hub = VirtualLedger::new(AccountConfig {
            account: "research.cash".into(),
            instrument,
            profile: if margin.is_some() {
                LedgerProfile::MarginLong
            } else {
                LedgerProfile::CashLong
            },
            initial_cash: initial,
        })
        .map_err(ledger_error)?;
        let rate = |key: &str, max: i128| -> Result<Decimal> {
            let value = num(context
                .get(key)
                .ok_or_else(|| failure("research_context", key))?)?;
            if value.coefficient() < 0 || value.coefficient() > max * FACTOR {
                return Err(failure("research_cost_rate", key));
            }
            Ok(value)
        };
        let fee_bps = rate("feeBps", 10000)?;
        let slippage = rate("slippageBps", 9999)?;
        if !["cash", "margin"]
            .contains(&context.get("accountMode").map(String::as_str).unwrap_or(""))
            || !["net", "hedge"].contains(
                &context
                    .get("positionMode")
                    .map(String::as_str)
                    .unwrap_or(""),
            )
        {
            return Err(failure("research_account_mode", "$.context"));
        }
        let mut legs = BTreeMap::new();
        for (symbol, i) in &instruments {
            for side in ["long", "short"] {
                legs.insert(
                    (symbol.clone(), side.into()),
                    Leg {
                        ledger: VirtualLedger::new(AccountConfig {
                            account: format!("leg.{}.{side}", legs.len()),
                            instrument: i.clone(),
                            profile: if side == "short" {
                                LedgerProfile::ProceedsRestrictedShort
                            } else {
                                LedgerProfile::CashLong
                            },
                            initial_cash: parse("0", ms)?,
                        })
                        .map_err(ledger_error)?,
                        high: None,
                        low: None,
                        adds: 0,
                        collateral: None,
                        protection: None,
                    },
                );
            }
        }
        Ok(Self {
            hub,
            legs,
            instruments,
            orders: vec![],
            fills: vec![],
            events: vec![],
            marks: BTreeMap::new(),
            fees: parse("0", ms)?,
            initial,
            margin,
            serial: 0,
            fee_bps,
            slippage,
            ms,
        })
    }
    fn id(&mut self, kind: &str) -> String {
        self.serial += 1;
        format!("research.{kind}.{}", self.serial)
    }
    fn post_hub(&mut self, time: u64, action: LedgerAction) -> Result<()> {
        let id = self.id("hub");
        self.hub
            .apply(LedgerTransaction {
                id,
                timestamp_ns: time,
                action,
            })
            .map_err(ledger_error)?;
        Ok(())
    }
    fn post_leg(&mut self, key: &(String, String), time: u64, action: LedgerAction) -> Result<()> {
        let id = self.id("leg");
        self.legs
            .get_mut(key)
            .ok_or_else(|| failure("research_leg", "$"))?
            .ledger
            .apply(LedgerTransaction {
                id,
                timestamp_ns: time,
                action,
            })
            .map_err(ledger_error)?;
        Ok(())
    }
    fn transfer(
        &mut self,
        key: &(String, String),
        amount: Decimal,
        to_leg: bool,
        time: u64,
    ) -> Result<()> {
        if amount.coefficient() == 0 {
            return Ok(());
        }
        self.post_hub(
            time,
            LedgerAction::Funding {
                direction: if to_leg {
                    FundingDirection::Withdraw
                } else {
                    FundingDirection::Deposit
                },
                amount,
            },
        )?;
        self.post_leg(
            key,
            time,
            LedgerAction::Funding {
                direction: if to_leg {
                    FundingDirection::Deposit
                } else {
                    FundingDirection::Withdraw
                },
                amount,
            },
        )?;
        Ok(())
    }
    fn release(&mut self, index: usize, time: u64) -> Result<()> {
        if let Some(id) = self.orders[index].hold.take() {
            self.post_hub(time, LedgerAction::Release { reservation: id })?;
        }
        self.orders[index].held = parse("0", self.ms)?;
        Ok(())
    }
    fn reserve(&mut self, index: usize, amount: Decimal, time: u64) -> Result<()> {
        if amount.coefficient() > 0 {
            // A partial fill may consume all hub cash for gross-basis clearing.
            // Back the remaining admitted hold too; clearing debt is excluded
            // from equity, and the normal free-margin check still limits risk.
            self.ensure_clearing_cash(amount, time)?;
            let id = self.id("orderhold");
            self.post_hub(
                time,
                LedgerAction::Reserve {
                    reservation: id.clone(),
                    amount: Reservation {
                        cash: amount,
                        quantity: parse(
                            "0",
                            self.hub
                                .config()
                                .instrument
                                .declaration()
                                .quantity_step
                                .scale(),
                        )?,
                    },
                },
            )?;
            self.orders[index].hold = Some(id);
            self.orders[index].held = amount;
        }
        Ok(())
    }
    pub fn cancel(&mut self, index: usize, reason: &str, time: u64) -> Result<()> {
        self.release(index, time)?;
        self.orders[index].status = reason.into();
        self.repay_clearing(time)?;
        self.events.push(o([
            ("kind", s("order")),
            ("order", order_json(&self.orders[index])),
            ("timestamp_ns", s(time)),
        ]));
        Ok(())
    }
    pub fn active(o: &Order) -> bool {
        ["accepted", "partially_filled"].contains(&o.status.as_str())
    }
    pub fn quantity(&self, symbol: &str, side: &str) -> Result<Decimal> {
        let l = self
            .legs
            .get(&(symbol.into(), side.into()))
            .ok_or_else(|| failure("research_symbol", symbol))?;
        absd(l.ledger.view().quantity)
    }
    pub fn pending(&self, symbol: &str, side: &str, entry: bool) -> bool {
        self.orders
            .iter()
            .any(|o| Self::active(o) && o.symbol == symbol && o.side == side && o.entry == entry)
    }
    pub fn submit(&mut self, mut order: Order, time: u64) -> Result<()> {
        let system = order.node == "__fqe.margin";
        let quota = MAX_RESEARCH_ORDERS
            + if system {
                2 * self.instruments.len()
            } else {
                0
            };
        if self.orders.len() >= quota {
            return Err(failure("research_order_quota", "$.orders"));
        }
        let instrument = self
            .instruments
            .get(&order.symbol)
            .ok_or_else(|| failure("research_symbol", &order.symbol))?;
        if system {
            instrument.quantity_lots(order.quantity, false)?;
        } else {
            instrument.quantity(order.quantity, false)?;
        }
        if let Some(p) = order.limit
            && instrument.price(p)?.coefficient() <= 0
        {
            return Err(failure("research_order_price", &order.node));
        }
        if let Some(p) = order.stop
            && instrument.price(p)?.coefficient() <= 0
        {
            return Err(failure("research_order_price", &order.node));
        }
        let held = if order.entry {
            order.cap
        } else {
            parse("0", self.ms)?
        };
        if self.margin.is_some() {
            if !system && self.margin.as_ref().expect("margin").status != "healthy" {
                order.status = "rejected_margin_liquidation".into();
            } else if order.entry
                && wide(held)?.coefficient() > wide(self.free_margin()?)?.coefficient()
            {
                order.status = "rejected_insufficient_margin".into();
            } else if order.entry {
                self.ensure_clearing_cash(held, time)?;
            }
        } else if held.coefficient() > self.hub.view().available_cash.coefficient() {
            order.status = "rejected_insufficient_cash".into();
        }
        let accepted = Self::active(&order);
        self.orders.push(order);
        let index = self.orders.len() - 1;
        if accepted {
            self.reserve(index, held, time)?;
        }
        self.events.push(o([
            ("kind", s("order")),
            ("order", order_json(&self.orders[index])),
            ("timestamp_ns", s(time)),
        ]));
        Ok(())
    }
    pub fn quote(&self, symbol: &str, price: Decimal, buy: bool) -> Result<Decimal> {
        let i = &self.instruments[symbol];
        let ps = i.declaration().price_step.scale();
        let factor = if buy {
            add(whole(10000)?, self.slippage)?
        } else {
            sub(whole(10000)?, self.slippage)?
        };
        let p = mul(price, factor)?;
        let value = div(p, whole(10000)?)?;
        let rounded = align(
            value,
            i.declaration().price_step,
            if buy {
                FinancialRounding::Ceiling
            } else {
                FinancialRounding::Floor
            },
        )?;
        parse(&decimal_text(rounded), ps)
    }
    fn fee(&self, gross: Decimal) -> Result<Decimal> {
        let rate = mul(wide(gross)?, self.fee_bps)?;
        let fee = div(rate, whole(10000)?)?;
        Ok(align(
            fee,
            self.hub.config().instrument.declaration().money_step,
            FinancialRounding::HalfAwayFromZero,
        )?)
    }
    pub fn on_open(
        &mut self,
        symbol: &str,
        bar: &ClosedBar,
        previous_volume: Decimal,
    ) -> Result<()> {
        let b = bar.input();
        if self.margin.is_some() {
            self.marks.insert(
                symbol.into(),
                align(
                    bar_value(&b.open)?,
                    self.instruments[symbol].declaration().price_step,
                    FinancialRounding::Exact,
                )?,
            );
        }
        let mut liquidity = step_floor(
            div(mul(previous_volume, whole(10000)?)?, whole(10000)?)?,
            wide(self.instruments[symbol].declaration().quantity_step)?,
        )?; // deterministic full prior-volume participation
        for index in 0..self.orders.len() {
            if !Self::active(&self.orders[index])
                || self.orders[index].symbol != symbol
                || self.orders[index].submitted > b.open_ns
            {
                continue;
            }
            self.orders[index].age += 1;
            let order = self.orders[index].clone();
            if order.tif == "GTD" && order.age > order.ttl {
                self.cancel(index, "expired", b.open_ns)?;
                continue;
            }
            let open = bar_value(&b.open)?;
            let buy =
                (order.entry && order.side == "long") || (!order.entry && order.side == "short");
            let touched = order.stop.is_none_or(|p| {
                if buy {
                    open.coefficient() >= wide(p).expect("price precision").coefficient()
                } else {
                    open.coefficient() <= wide(p).expect("price precision").coefficient()
                }
            });
            if !order.triggered && touched {
                self.orders[index].triggered = true;
            }
            let eligible = self.orders[index].triggered
                && order.limit.is_none_or(|p| {
                    if buy {
                        open.coefficient() <= wide(p).expect("price precision").coefficient()
                    } else {
                        open.coefficient() >= wide(p).expect("price precision").coefficient()
                    }
                });
            let mut quantity = if eligible {
                mind(wide(order.remaining)?, liquidity)
            } else {
                whole(0)?
            };
            if order.tif == "FOK" && quantity.coefficient() < wide(order.remaining)?.coefficient() {
                quantity = whole(0)?;
            }
            if quantity.coefficient() > 0 {
                if !order.entry {
                    quantity = mind(quantity, wide(self.quantity(&order.symbol, &order.side)?)?);
                }
                if quantity.coefficient() > 0 && order.entry && self.margin.is_some() {
                    let price = self.quote(symbol, open, buy)?;
                    let price = order
                        .limit
                        .map_or(price, |p| if buy { mind(price, p) } else { maxd(price, p) });
                    let free = add(self.free_margin()?, order.held)?;
                    let budget = mind(order.cap, mind(self.entry_budget()?, maxd(free, whole(0)?)));
                    quantity = self.budget_quantity(symbol, price, budget, Some(quantity))?;
                    if order.tif == "FOK"
                        && quantity.coefficient() < wide(order.remaining)?.coefficient()
                    {
                        self.cancel(index, "cancelled_time_in_force", b.open_ns)?;
                        continue;
                    }
                    if self.instruments[symbol].quantity(quantity, false).is_err() {
                        self.cancel(index, "rejected_insufficient_margin", b.open_ns)?;
                        continue;
                    }
                }
                if quantity.coefficient() > 0 {
                    let mut trial = self.clone();
                    match trial.fill(index, quantity, open, b.open_ns, b.sequence) {
                        Ok(()) => {
                            *self = trial;
                            liquidity = sub(liquidity, quantity)?;
                        }
                        Err(e)
                            if [
                                "ledger_funds",
                                "research_cash_cap",
                                "research_insufficient_cash",
                                "research_minimum",
                                "research_insufficient_margin",
                            ]
                            .contains(&e.code) =>
                        {
                            self.cancel(
                                index,
                                if self.margin.is_some() {
                                    "rejected_insufficient_margin"
                                } else {
                                    "rejected_execution_funds"
                                },
                                b.open_ns,
                            )?;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            if Self::active(&self.orders[index])
                && self.orders[index].triggered
                && ["IOC", "FOK"].contains(&order.tif.as_str())
            {
                self.cancel(index, "cancelled_time_in_force", b.open_ns)?;
            }
        }
        Ok(())
    }
    fn fill(
        &mut self,
        index: usize,
        q: Decimal,
        open: Decimal,
        time: u64,
        sequence: u64,
    ) -> Result<()> {
        if self.fills.len() >= 4096 {
            return Err(failure("research_fill_quota", "$.fills"));
        }
        let order = self.orders[index].clone();
        let key = (order.symbol.clone(), order.side.clone());
        let i = self.instruments[&order.symbol].clone();
        let quantity = align(q, i.declaration().quantity_step, FinancialRounding::Floor)?;
        i.quantity_lots(quantity, false)?;
        let buy = (order.entry && order.side == "long") || (!order.entry && order.side == "short");
        let mut price = self.quote(&order.symbol, open, buy)?;
        if let Some(limit) = order.limit {
            price = if buy {
                mind(price, limit)
            } else {
                maxd(price, limit)
            };
        }
        let gross = i.notional(price, quantity, FinancialRounding::HalfAwayFromZero)?;
        if gross.coefficient() <= 0
            || gross.coefficient() < i.declaration().minimum_notional.coefficient()
        {
            return Err(failure("research_minimum", &order.id));
        }
        let fee = self.fee(gross)?;
        let before = self.legs[&key].ledger.view();
        let previous_q = absd(before.quantity)?;
        self.release(index, time)?;
        let exposure = i.notional(price, quantity, FinancialRounding::Ceiling)?;
        let consumed = self.entry_cost(
            if self.margin.is_some() {
                exposure
            } else {
                gross
            },
            fee,
        )?;
        let cost = align(
            consumed,
            i.declaration().money_step,
            FinancialRounding::Exact,
        )?;
        if order.entry && cost.coefficient() > order.cap.coefficient() {
            return Err(failure("research_cash_cap", &order.id));
        }
        if let Some(collateral) = self.legs.get_mut(&key).expect("leg").collateral.take() {
            self.post_leg(
                &key,
                time,
                LedgerAction::Release {
                    reservation: collateral,
                },
            )?;
        }
        let after_quantity = if order.entry {
            add(wide(previous_q)?, wide(quantity)?)?
        } else {
            sub(wide(previous_q)?, wide(quantity)?)?
        };
        if after_quantity.coefficient() < 0 {
            return Err(failure("research_reduce_only", &order.id));
        }
        // Restricted short proceeds are unavailable for new orders. Releasing the
        // covered basis/collateral is part of the atomic trade, not spendable twice.
        let clearing_cost = add(gross, fee)?;
        let available = self.legs[&key].ledger.view().cash.coefficient();
        let retained_basis =
            if !order.entry && order.side == "short" && after_quantity.coefficient() > 0 {
                align(
                    mul(
                        wide(absd(before.cost)?)?,
                        div(after_quantity, wide(previous_q)?)?,
                    )?,
                    i.declaration().money_step,
                    FinancialRounding::Ceiling,
                )?
                .coefficient()
            } else {
                0
            };
        let needed = if buy {
            (clearing_cost
                .coefficient()
                .checked_add(
                    retained_basis
                        .checked_mul(2)
                        .ok_or_else(|| failure("research_numeric", "$"))?,
                )
                .ok_or_else(|| failure("research_numeric", "$"))?
                - available)
                .max(0)
        } else if order.entry {
            clearing_cost.coefficient()
        } else {
            0
        };
        self.ensure_clearing_cash(Decimal::new(needed, self.ms).map_err(numeric_error)?, time)?;
        if needed > self.hub.view().available_cash.coefficient() {
            return Err(failure("research_insufficient_cash", &order.id));
        }
        self.transfer(
            &key,
            Decimal::new(needed, self.ms).map_err(numeric_error)?,
            true,
            time,
        )?;
        self.post_leg(
            &key,
            time,
            LedgerAction::Trade {
                side: if buy { TradeSide::Buy } else { TradeSide::Sell },
                price,
                quantity,
                fee,
                release: None,
            },
        )?;
        if order.side == "short" && after_quantity.coefficient() > 0 {
            let collateral = absd(self.legs[&key].ledger.view().cost)?;
            let id = self.id("collateral");
            self.post_leg(
                &key,
                time,
                LedgerAction::Reserve {
                    reservation: id.clone(),
                    amount: Reservation {
                        cash: collateral,
                        quantity: parse("0", i.declaration().quantity_step.scale())?,
                    },
                },
            )?;
            self.legs.get_mut(&key).expect("leg").collateral = Some(id);
        }
        let sweep = self.legs[&key].ledger.view().available_cash;
        self.transfer(&key, sweep, false, time)?;
        self.fees = add(self.fees, fee)?;
        self.orders[index].remaining = sub(order.remaining, quantity)?;
        self.orders[index].status = if self.orders[index].remaining.coefficient() == 0 {
            "filled".into()
        } else {
            "partially_filled".into()
        };
        self.orders[index].cap = if order.entry {
            sub(order.cap, cost)?
        } else {
            order.cap
        };
        if Self::active(&self.orders[index]) && order.entry {
            let remaining = self.orders[index].cap;
            self.reserve(index, remaining, time)?;
        }
        self.repay_clearing(time)?;
        if self.margin.is_some() && order.entry && self.free_margin()?.coefficient() < 0 {
            return Err(failure("research_insufficient_margin", &order.id));
        }
        let leg = self.legs.get_mut(&key).expect("leg");
        if order.entry {
            if previous_q.coefficient() == 0 {
                leg.high = Some(wide(price)?);
                leg.low = Some(wide(price)?);
                leg.adds = 0;
            } else {
                leg.adds += 1;
            }
            if let Some(p) = order.protection {
                leg.protection = Some(p);
            }
        } else if after_quantity.coefficient() == 0 {
            leg.high = None;
            leg.low = None;
            leg.adds = 0;
            leg.protection = None;
        }
        self.fills.push(o([
            ("number", s(self.fills.len() + 1)),
            ("order", s(&order.id)),
            ("node", s(&order.node)),
            ("symbol", s(&order.symbol)),
            ("position_side", s(&order.side)),
            ("trade_side", s(if buy { "buy" } else { "sell" })),
            ("source_sequence", s(sequence)),
            ("timestamp_ns", s(time)),
            ("price", ds(price)),
            ("quantity", ds(quantity)),
            ("gross", ds(gross)),
            ("fee", ds(fee)),
            ("remaining", ds(self.orders[index].remaining)),
        ]));
        self.events.push(o([
            ("kind", s("fill")),
            ("fill", self.fills.last().expect("fill").clone()),
            ("timestamp_ns", s(time)),
        ]));
        Ok(())
    }
    pub fn on_close(&mut self, symbol: &str, bar: &ClosedBar) -> Result<()> {
        let b = bar.input();
        let close = bar_value(&b.close)?;
        self.marks.insert(
            symbol.into(),
            align(
                close,
                self.instruments[symbol].declaration().price_step,
                FinancialRounding::Exact,
            )?,
        );
        for side in ["long", "short"] {
            let leg = self
                .legs
                .get_mut(&(symbol.into(), side.into()))
                .expect("configured leg");
            if leg.ledger.view().quantity.coefficient() != 0 {
                leg.high = Some(maxd(leg.high.unwrap_or(close), bar_value(&b.high)?));
                leg.low = Some(mind(leg.low.unwrap_or(close), bar_value(&b.low)?));
            }
        }
        self.check_maintenance(b.close_ns)?;
        Ok(())
    }
    pub fn equity(&self) -> Result<Decimal> {
        let mut total = sub(self.hub.view().cash, self.hub.view().debt)?;
        for ((symbol, _), leg) in &self.legs {
            let v = leg.ledger.view();
            let equity = if v.quantity.coefficient() == 0 {
                v.cash
            } else {
                leg.ledger
                    .value_at(
                        self.marks
                            .get(symbol)
                            .copied()
                            .ok_or_else(|| failure("research_mark", symbol))?,
                    )
                    .map_err(ledger_error)?
                    .equity
            };
            total = add(total, equity)?;
        }
        Ok(total)
    }
    pub fn snapshot(&self) -> Result<V> {
        let positions = self
            .legs
            .iter()
            .filter(|(_, l)| l.ledger.view().quantity.coefficient() != 0)
            .map(|((symbol, side), l)| {
                o([
                    ("symbol", s(symbol)),
                    ("side", s(side)),
                    ("account", account(&l.ledger.view())),
                    ("highest_price", l.high.map_or(V::Null, ds)),
                    ("lowest_price", l.low.map_or(V::Null, ds)),
                ])
            })
            .collect();
        let mut snapshot = o([
            ("cash_hub", account(&self.hub.view())),
            ("equity", ds(self.equity()?)),
            ("equity_basis", s("latest-available-closed-marks")),
            ("positions", V::Array(positions)),
            (
                "pending_orders",
                V::Array(
                    self.orders
                        .iter()
                        .filter(|o| Self::active(o))
                        .map(order_json)
                        .collect(),
                ),
            ),
        ]);
        if self.margin.is_some() {
            snapshot
                .as_object_mut()
                .expect("object")
                .insert("margin".into(), self.margin_snapshot()?);
        }
        Ok(snapshot)
    }
    pub fn graph_views(&self, time: u64) -> Result<GraphViews> {
        let mut positions = BTreeMap::new();
        for ((symbol, side), leg) in &self.legs {
            let view = leg.ledger.view();
            let quantity = absd(view.quantity)?;
            let mut p = BTreeMap::from([(
                "quantity".into(),
                Slot {
                    value: Some(Atom::Number(wide(quantity)?)),
                    stamp: time,
                },
            )]);
            if quantity.coefficient() > 0 {
                let average = div(wide(absd(view.cost)?)?, wide(quantity)?)?;
                for (k, v) in [
                    ("averagePrice", Some(average)),
                    ("highestPrice", leg.high),
                    ("lowestPrice", leg.low),
                ] {
                    p.insert(
                        k.into(),
                        Slot {
                            value: v.map(Atom::Number),
                            stamp: time,
                        },
                    );
                }
            }
            positions.insert((symbol.clone(), side.clone()), p);
        }
        Ok((
            BTreeMap::from([
                (
                    "equity".into(),
                    Slot {
                        value: Some(Atom::Number(wide(self.equity()?)?)),
                        stamp: time,
                    },
                ),
                (
                    "cash".into(),
                    Slot {
                        value: Some(Atom::Number(wide(if self.margin.is_some() {
                            self.free_margin()?
                        } else {
                            self.hub.view().available_cash
                        })?)),
                        stamp: time,
                    },
                ),
            ]),
            positions,
        ))
    }
    pub fn finish(&mut self, time: u64) -> Result<()> {
        for i in 0..self.orders.len() {
            if Self::active(&self.orders[i]) {
                self.cancel(i, "cancelled_terminal", time)?;
            }
        }
        Ok(())
    }
    pub fn journals(&self) -> V {
        let mut entries = vec![o([
            ("account", s("cash_hub")),
            (
                "entries",
                V::Array(self.hub.journal().iter().map(journal).collect()),
            ),
        ])];
        for ((symbol, side), leg) in &self.legs {
            entries.push(o([
                ("account", s(format!("{symbol}.{side}"))),
                (
                    "entries",
                    V::Array(leg.ledger.journal().iter().map(journal).collect()),
                ),
            ]));
        }
        V::Array(entries)
    }
}
pub fn order_json(v: &Order) -> V {
    o([
        ("id", s(&v.id)),
        ("node", s(&v.node)),
        ("symbol", s(&v.symbol)),
        ("side", s(&v.side)),
        ("entry", V::Bool(v.entry)),
        ("kind", s(&v.kind)),
        ("quantity", ds(v.quantity)),
        ("remaining", ds(v.remaining)),
        ("limit", v.limit.map_or(V::Null, ds)),
        ("stop", v.stop.map_or(V::Null, ds)),
        ("triggered", V::Bool(v.triggered)),
        ("time_in_force", s(&v.tif)),
        ("valid_bars", s(v.ttl)),
        ("age", s(v.age)),
        ("tag", s(&v.tag)),
        ("status", s(&v.status)),
        ("cash_cap_remaining", ds(v.cap)),
        ("reserved_cash", ds(v.held)),
        ("submitted_ns", s(v.submitted)),
    ])
}
pub fn account(v: &AccountView) -> V {
    o([
        ("cash", ds(v.cash)),
        ("reserved_cash", ds(v.reserved_cash)),
        ("restricted_short_proceeds", ds(v.restricted_short_proceeds)),
        ("available_cash", ds(v.available_cash)),
        ("quantity", ds(v.quantity)),
        ("reserved_quantity", ds(v.reserved_quantity)),
        ("cost", ds(v.cost)),
        ("realized_gross", ds(v.realized_gross)),
        ("fees", ds(v.fees)),
        ("external_capital", ds(v.external_capital)),
        ("debt", ds(v.debt)),
    ])
}
fn journal(v: &JournalEntry) -> V {
    let action = match &v.transaction.action {
        LedgerAction::Funding { direction, amount } => o([
            (
                "kind",
                s(match direction {
                    FundingDirection::Deposit => "deposit",
                    FundingDirection::Withdraw => "withdraw",
                }),
            ),
            ("amount", ds(*amount)),
        ]),
        LedgerAction::Reserve {
            reservation,
            amount,
        } => o([
            ("kind", s("reserve")),
            ("reservation", s(reservation)),
            ("cash", ds(amount.cash)),
            ("quantity", ds(amount.quantity)),
        ]),
        LedgerAction::Release { reservation } => {
            o([("kind", s("release")), ("reservation", s(reservation))])
        }
        LedgerAction::Trade {
            side,
            price,
            quantity,
            fee,
            ..
        } => o([
            ("kind", s("trade")),
            (
                "side",
                s(match side {
                    TradeSide::Buy => "buy",
                    TradeSide::Sell => "sell",
                }),
            ),
            ("price", ds(*price)),
            ("quantity", ds(*quantity)),
            ("fee", ds(*fee)),
        ]),
        _ => o([("kind", s("unsupported"))]),
    };
    let money = v
        .money
        .iter()
        .map(|p| {
            o([
                (
                    "account",
                    s(match p.account {
                        MoneyAccount::LoanDebt => "debt",
                        MoneyAccount::Cash => "cash",
                        MoneyAccount::InventoryCost => "inventory_cost",
                        MoneyAccount::ExternalCapital => "external_capital",
                        MoneyAccount::RealizedProfit => "realized_profit",
                        MoneyAccount::FeeExpense => "fee_expense",
                    }),
                ),
                ("delta", ds(p.delta)),
            ])
        })
        .collect();
    o([
        ("sequence", s(v.sequence)),
        ("id", s(&v.transaction.id)),
        ("timestamp_ns", s(v.transaction.timestamp_ns)),
        ("action", action),
        ("money", V::Array(money)),
        ("after", account(&v.after)),
    ])
}

#[cfg(test)]
mod quota_tests {
    use super::*;

    #[test]
    fn full_order_history_is_bounded_without_discarding_old_orders() {
        let input =
            serde_json::from_str(include_str!("../../tests/fixtures/research-input.json")).unwrap();
        let mut broker = Prepared::new(&input).unwrap().broker;
        let quantity = parse("1.000", 3).unwrap();
        let zero = parse("0.00", 2).unwrap();
        let order = Order {
            id: String::new(),
            node: "entry".into(),
            symbol: "DEMO".into(),
            side: "long".into(),
            entry: true,
            kind: "market".into(),
            quantity,
            remaining: quantity,
            limit: None,
            stop: None,
            triggered: true,
            tif: "GTC".into(),
            ttl: 1,
            age: 0,
            tag: String::new(),
            status: "accepted".into(),
            cap: zero,
            held: zero,
            hold: None,
            protection: None,
            submitted: 0,
        };
        for index in 0..MAX_RESEARCH_ORDERS {
            let mut next = order.clone();
            next.id = format!("quota.{index}");
            broker.submit(next, 0).unwrap();
        }
        assert_eq!(broker.orders.first().unwrap().id, "quota.0");
        assert_eq!(broker.orders.len(), 8192);
        let before = broker.events.len();
        assert_eq!(
            broker.submit(order, 0).unwrap_err().code,
            "research_order_quota"
        );
        assert_eq!(broker.orders.len(), 8192);
        assert_eq!(broker.events.len(), before);
    }
}
