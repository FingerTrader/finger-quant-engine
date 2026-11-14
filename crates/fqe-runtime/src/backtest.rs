//! Bounded local batch orchestration; every financial output comes from the simulator's ledger.
use crate::{
    events::{EventObservation, EventRuntime, ExecutionError},
    history::PreparedHistoricalRun,
    ledger::TradeSide,
    portfolio::{self, EquityPoint, PortfolioMetrics},
    simulator::*,
    workflow::{WorkflowObservation, WorkflowRuntime},
};
use fqe_core::{
    finance::FinancialError,
    numeric::Decimal,
    types::{Evaluation, ScalarType, ScalarValue},
};
use fqe_ir::{
    event_plan::CompiledEventPlan,
    history::{HistorySelection, HistoryStrategy},
    model::ContentId,
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub const MAX_BACKTEST_BARS: usize = 256;
pub const MAX_POLICY_BINDINGS: usize = 16;
type Result<T> = std::result::Result<T, ExecutionError>;
pub(crate) fn error(code: &'static str, detail: impl ToString) -> ExecutionError {
    ExecutionError {
        code,
        path: "$.backtest".into(),
        detail: detail.to_string(),
    }
}
impl From<SimulationError> for ExecutionError {
    fn from(e: SimulationError) -> Self {
        error(e.code, e.detail)
    }
}
impl From<FinancialError> for ExecutionError {
    fn from(e: FinancialError) -> Self {
        error("backtest_financial", e)
    }
}
#[derive(Clone)]
pub enum StrategyPlan {
    Event(Arc<CompiledEventPlan>),
    Workflow(Arc<fqe_ir::workflow::CompiledWorkflow>),
}
impl StrategyPlan {
    pub fn event(&self) -> &CompiledEventPlan {
        match self {
            Self::Event(p) => p,
            Self::Workflow(p) => p.event(),
        }
    }
    pub fn execution_id(&self) -> ContentId {
        match self {
            Self::Event(p) => p.content_id(),
            Self::Workflow(p) => p.content_id(),
        }
    }
    fn history(&self) -> HistoryStrategy {
        match self {
            Self::Event(p) => HistoryStrategy::from(p.as_ref()),
            Self::Workflow(p) => HistoryStrategy::from(p.as_ref()),
        }
    }
    fn validate_source(&self, source: &PolicySource) -> bool {
        match source {
            PolicySource::LongSignal => true,
            PolicySource::EventBoolean { name } => self
                .event()
                .roots()
                .output_types()
                .iter()
                .any(|p| p.name == *name && *p.value_type.scalar() == ScalarType::Bool),
            PolicySource::WorkflowBoolean { instance, output } => match self {
                Self::Workflow(w) => w
                    .instances()
                    .iter()
                    .find(|i| i.id == *instance)
                    .is_some_and(|i| {
                        i.outputs.output_types().iter().any(|p| {
                            p.name == *output && *p.value_type.scalar() == ScalarType::Bool
                        })
                    }),
                _ => false,
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicySource {
    LongSignal,
    EventBoolean { name: String },
    WorkflowBoolean { instance: String, output: String },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyAction {
    BuyIfFlat {
        quantity: Decimal,
        cash_cap: Decimal,
    },
    CloseLong,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyBinding {
    pub id: String,
    pub source: PolicySource,
    pub action: PolicyAction,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunControl {
    Complete,
    StopAfter(usize),
    CancelAfter(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BacktestTerminal {
    Paused,
    Completed,
    Stopped,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StrategyObservation {
    Event(EventObservation),
    Workflow(WorkflowObservation),
}
impl StrategyObservation {
    fn eligible(&self) -> bool {
        match self {
            Self::Event(v) => v.matched && !v.warmup,
            Self::Workflow(v) => v.matched && !v.warmup,
        }
    }
    fn fires(&self, source: &PolicySource) -> bool {
        let boolean = |v: Option<&Evaluation<fqe_core::types::TypedValue>>| matches!(v,Some(Evaluation::Ready(v)) if matches!(v.value(),ScalarValue::Bool(true)));
        match (self, source) {
            (Self::Event(v), PolicySource::LongSignal) => v
                .signals
                .iter()
                .any(|s| s.signal.kind == fqe_core::SignalKind::Long),
            (Self::Workflow(v), PolicySource::LongSignal) => v
                .signals
                .iter()
                .any(|s| s.record.signal.kind == fqe_core::SignalKind::Long),
            (Self::Event(v), PolicySource::EventBoolean { name }) => boolean(v.outputs.get(name)),
            (Self::Workflow(v), PolicySource::EventBoolean { name }) => {
                boolean(v.event_outputs.get(name))
            }
            (Self::Workflow(v), PolicySource::WorkflowBoolean { instance, output }) => {
                boolean(v.outputs.get(instance).and_then(|m| m.get(output)))
            }
            _ => false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionOutcome {
    RiskDisabled,
    Warmup,
    NotReadyOrFalse,
    PositionOrPending,
    FlatOrPending,
    ResidualBelowMinimum,
    Submitted,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyDecision {
    pub source_sequence: u64,
    pub timestamp_ns: u64,
    pub binding: String,
    pub outcome: DecisionOutcome,
    pub order: Option<String>,
}
#[derive(Clone)]
pub struct PreparedBacktest {
    pub(crate) selection: HistorySelection,
    pub(crate) strategy: StrategyPlan,
    pub(crate) policies: Vec<PolicyBinding>,
    pub(crate) config: SimulationConfig,
    pub(crate) run: String,
    pub(crate) seed: u64,
    pub(crate) control: RunControl,
    manifest: Arc<[u8]>,
    manifest_id: ContentId,
}
pub struct BacktestResult {
    pub(crate) manifest: Arc<[u8]>,
    pub(crate) manifest_id: ContentId,
    pub(crate) terminal: BacktestTerminal,
    pub(crate) observations: Vec<StrategyObservation>,
    pub(crate) decisions: Vec<PolicyDecision>,
    pub(crate) curve: Vec<EquityPoint>,
    pub(crate) metrics: PortfolioMetrics,
    pub(crate) valuation: crate::ledger::Valuation,
    pub(crate) broker: SimulationBroker,
    json: Arc<[u8]>,
    id: ContentId,
}
impl BacktestResult {
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }
    pub fn manifest_id(&self) -> ContentId {
        self.manifest_id
    }
    pub fn content_id(&self) -> ContentId {
        self.id
    }
    pub fn json(&self) -> &[u8] {
        &self.json
    }
    pub fn terminal(&self) -> BacktestTerminal {
        self.terminal
    }
    pub fn observations(&self) -> &[StrategyObservation] {
        &self.observations
    }
    pub fn decisions(&self) -> &[PolicyDecision] {
        &self.decisions
    }
    pub fn curve(&self) -> &[EquityPoint] {
        &self.curve
    }
    pub fn metrics(&self) -> &PortfolioMetrics {
        &self.metrics
    }
    pub fn valuation(&self) -> &crate::ledger::Valuation {
        &self.valuation
    }
    pub fn broker(&self) -> &SimulationBroker {
        &self.broker
    }
}
enum Runner {
    Event(Box<EventRuntime>),
    Workflow(Box<WorkflowRuntime>),
}
impl Runner {
    fn step(&mut self, bar: &fqe_core::bars::ClosedBar) -> Result<StrategyObservation> {
        match self {
            Self::Event(v) => v
                .step(bar.input().clone().into())
                .map(StrategyObservation::Event),
            Self::Workflow(v) => v
                .step(bar.input().clone().into())
                .map(StrategyObservation::Workflow),
        }
    }
    fn finish(&mut self) -> Result<()> {
        match self {
            Self::Event(v) => {
                v.finish()?;
            }
            Self::Workflow(v) => {
                v.finish()?;
            }
        }
        Ok(())
    }
}
impl PreparedBacktest {
    pub fn new(
        selection: HistorySelection,
        strategy: StrategyPlan,
        policies: Vec<PolicyBinding>,
        mut config: SimulationConfig,
        run: &str,
        seed: u64,
        control: RunControl,
    ) -> Result<Self> {
        if selection.bars().len() > MAX_BACKTEST_BARS || policies.len() > MAX_POLICY_BINDINGS {
            return Err(error(
                "backtest_limit",
                "source bars<=256 and policy bindings<=16 required",
            ));
        }
        if matches!(control,RunControl::StopAfter(n)|RunControl::CancelAfter(n) if n>selection.bars().len())
        {
            return Err(error("backtest_control", "prefix exceeds selected bars"));
        }
        let spec = selection.dataset().spec();
        if &config.account.instrument != spec.instrument()
            || config.timeframe_ns != spec.duration_ns()
        {
            return Err(error(
                "backtest_binding",
                "financial instrument/timeframe differs from immutable dataset",
            ));
        }
        let source = SignalSource {
            run: run.into(),
            execution_id: strategy.execution_id(),
        };
        if config.signal_source.as_ref().is_some_and(|v| v != &source) {
            return Err(error(
                "backtest_binding",
                "configured strategy source differs",
            ));
        }
        config.signal_source = Some(source);
        let mut ids = BTreeSet::new();
        for binding in &policies {
            fqe_ir::model::identifier(&binding.id, "$.policy.id")?;
            if binding.id.len() > 64
                || !ids.insert(&binding.id)
                || !strategy.validate_source(&binding.source)
            {
                return Err(error(
                    "backtest_policy",
                    "duplicate/oversize binding or missing/non-Boolean output",
                ));
            }
            if let PolicyAction::BuyIfFlat { quantity, cash_cap } = binding.action {
                config.account.instrument.quantity(quantity, false)?;
                config.account.instrument.money(cash_cap)?;
                if cash_cap.coefficient() <= 0 {
                    return Err(error("backtest_policy", "positive cash cap required"));
                }
            }
        }
        let history =
            PreparedHistoricalRun::new(selection.clone(), &strategy.history(), run, seed)?;
        SimulationBroker::new(config.clone())?;
        let mut prepared = Self {
            selection,
            strategy,
            policies,
            config,
            run: run.into(),
            seed,
            control,
            manifest: Arc::from([]),
            manifest_id: ContentId::from_bytes([0; 32]),
        };
        let bytes = crate::backtest_encoding::manifest(&prepared, history.manifest())?;
        prepared.manifest_id =
            fqe_ir::financial_exchange::content_id(b"FQE-BACKTEST-INPUTS/1\n", &bytes);
        prepared.manifest = bytes.into();
        Ok(prepared)
    }
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }
    pub fn manifest_id(&self) -> ContentId {
        self.manifest_id
    }
    pub fn run(&self) -> Result<BacktestResult> {
        self.run_with_cancel(&AtomicBool::new(false))
    }
    /// Cancellation is observed between source bars; it releases pending holds, never erases fills.
    pub fn run_with_cancel(&self, cancel: &AtomicBool) -> Result<BacktestResult> {
        self.execute_prefix(self.selection.bars().len(), true, cancel)
    }
    pub(crate) fn reconstruct_prefix(&self, count: usize, finish: bool) -> Result<BacktestResult> {
        if count > self.selection.bars().len() || self.control != RunControl::Complete {
            return Err(error(
                "financial_checkpoint_control",
                "complete-run preparation and valid source prefix required",
            ));
        }
        if finish && count != self.selection.bars().len() {
            return Err(error(
                "financial_checkpoint_control",
                "finish requires every selected source bar",
            ));
        }
        self.execute_prefix(count, finish, &AtomicBool::new(false))
    }
    fn execute_prefix(
        &self,
        count: usize,
        finish: bool,
        cancel: &AtomicBool,
    ) -> Result<BacktestResult> {
        let mut broker = SimulationBroker::new(self.config.clone())?;
        let dataset = self.selection.dataset().content_id();
        let mut runner = match &self.strategy {
            StrategyPlan::Event(p) => Runner::Event(Box::new(EventRuntime::new(
                Arc::clone(p),
                &self.run,
                dataset,
            )?)),
            StrategyPlan::Workflow(p) => Runner::Workflow(Box::new(WorkflowRuntime::new(
                Arc::clone(p),
                &self.run,
                dataset,
            )?)),
        };
        let mut observations = vec![];
        let mut decisions = vec![];
        let mut curve = vec![];
        let mut terminal = BacktestTerminal::Completed;
        for (i, bar) in self.selection.bars().iter().take(count).enumerate() {
            if cancel.load(Ordering::Acquire) {
                terminal = BacktestTerminal::Cancelled;
                break;
            }
            match self.control {
                RunControl::StopAfter(n) if i >= n => {
                    terminal = BacktestTerminal::Stopped;
                    break;
                }
                RunControl::CancelAfter(n) if i >= n => {
                    terminal = BacktestTerminal::Cancelled;
                    break;
                }
                _ => {}
            }
            broker.apply(SimulationCommand::Bar(Box::new(bar.clone())))?;
            let observation = runner.step(bar)?;
            let input = bar.input();
            for binding in &self.policies {
                let mut decision = PolicyDecision {
                    source_sequence: input.sequence,
                    timestamp_ns: input.close_ns,
                    binding: binding.id.clone(),
                    outcome: DecisionOutcome::NotReadyOrFalse,
                    order: None,
                };
                if i < self.selection.warmup_bars() || !observation.eligible() {
                    decision.outcome = DecisionOutcome::Warmup;
                } else if observation.fires(&binding.source) {
                    let account = broker.ledger().view();
                    let (side, quantity, cash_cap) = match binding.action {
                        PolicyAction::BuyIfFlat { quantity, cash_cap } => {
                            if broker
                                .margin_status()
                                .is_some_and(|s| s != crate::margin::MarginStatus::Healthy)
                            {
                                decision.outcome = DecisionOutcome::RiskDisabled;
                                decisions.push(decision);
                                continue;
                            }
                            if account.quantity.coefficient() != 0
                                || broker
                                    .orders()
                                    .iter()
                                    .any(|o| o.live() && o.intent.side == TradeSide::Buy)
                            {
                                decision.outcome = DecisionOutcome::PositionOrPending;
                                decisions.push(decision);
                                continue;
                            }
                            (TradeSide::Buy, quantity, Some(cash_cap))
                        }
                        PolicyAction::CloseLong => {
                            if account.available_long_quantity.coefficient() <= 0 {
                                decision.outcome = DecisionOutcome::FlatOrPending;
                                decisions.push(decision);
                                continue;
                            }
                            if self
                                .config
                                .account
                                .instrument
                                .quantity(account.available_long_quantity, false)
                                .is_err()
                            {
                                decision.outcome = DecisionOutcome::ResidualBelowMinimum;
                                decisions.push(decision);
                                continue;
                            }
                            (TradeSide::Sell, account.available_long_quantity, None)
                        }
                    };
                    let id = format!("p:{}:{}", input.sequence, binding.id);
                    let intent = OrderIntent {
                        id: id.clone(),
                        side,
                        kind: OrderKind::Market,
                        quantity,
                        cash_cap,
                        origin: IntentOrigin::Policy {
                            source: self.config.signal_source.clone().expect("prepared source"),
                            sequence: input.sequence,
                            timestamp_ns: input.close_ns,
                            binding: binding.id.clone(),
                        },
                    };
                    broker.apply(SimulationCommand::Submit(intent))?;
                    decision.order = Some(id);
                    decision.outcome = DecisionOutcome::Submitted;
                }
                decisions.push(decision);
            }
            if i >= self.selection.warmup_bars() {
                let mark = match input.close.value() {
                    ScalarValue::Decimal(v) => *v,
                    _ => return Err(error("backtest_price", "nondecimal close")),
                };
                let view = broker.ledger().view();
                // A flat account has no price-dependent valuation, including valid zero close prices.
                let equity = if view.quantity.coefficient() == 0 {
                    Decimal::new(
                        view.cash
                            .coefficient()
                            .checked_sub(view.debt.coefficient())
                            .ok_or(FinancialError::Overflow)?,
                        view.cash.scale(),
                    )
                    .map_err(FinancialError::from)?
                } else {
                    broker
                        .ledger()
                        .value_at(mark)
                        .map_err(|e| error(e.code, e.detail))?
                        .equity
                };
                curve.push(EquityPoint {
                    source_sequence: input.sequence,
                    timestamp_ns: input.close_ns,
                    mark,
                    cash: view.cash,
                    debt: view.debt,
                    quantity: view.quantity,
                    equity,
                    fees: view.fees,
                });
            }
            observations.push(observation);
        }
        // A prefix equal to the full length is still an explicitly requested stop/cancel.
        if terminal == BacktestTerminal::Completed {
            terminal = match self.control {
                RunControl::Complete => BacktestTerminal::Completed,
                RunControl::StopAfter(_) => BacktestTerminal::Stopped,
                RunControl::CancelAfter(_) => BacktestTerminal::Cancelled,
            };
        }
        if finish {
            runner.finish()?;
            broker.apply(SimulationCommand::Finish)?;
        } else {
            terminal = BacktestTerminal::Paused;
        }
        let metrics = portfolio::calculate(
            self.config.account.initial_cash,
            &curve,
            broker.ledger().view().fees,
        )?;
        let valuation = if broker.ledger().view().quantity.coefficient() == 0 {
            let v = broker.ledger().view();
            let z = Decimal::new(0, v.cash.scale()).map_err(FinancialError::from)?;
            let equity = Decimal::new(
                v.cash
                    .coefficient()
                    .checked_sub(v.debt.coefficient())
                    .ok_or(FinancialError::Overflow)?,
                v.cash.scale(),
            )
            .map_err(FinancialError::from)?;
            let profit = Decimal::new(
                equity
                    .coefficient()
                    .checked_sub(self.config.account.initial_cash.coefficient())
                    .ok_or(FinancialError::Overflow)?,
                v.cash.scale(),
            )
            .map_err(FinancialError::from)?;
            crate::ledger::Valuation {
                market_value: z,
                equity,
                unrealized: z,
                total_profit: profit,
            }
        } else {
            broker
                .ledger()
                .value_at(
                    curve
                        .last()
                        .expect("held position has performance mark")
                        .mark,
                )
                .map_err(|e| error(e.code, e.detail))?
        };
        if valuation.equity != metrics.final_equity || valuation.total_profit != metrics.net_profit
        {
            return Err(error(
                "backtest_reconciliation",
                "terminal valuation differs from sampled equity",
            ));
        }
        let mut result = BacktestResult {
            manifest: Arc::clone(&self.manifest),
            manifest_id: self.manifest_id,
            terminal,
            observations,
            decisions,
            curve,
            metrics,
            valuation,
            broker,
            json: Arc::from([]),
            id: ContentId::from_bytes([0; 32]),
        };
        let bytes = crate::backtest_encoding::result(&result)?;
        result.id = fqe_ir::financial_exchange::content_id(b"FQE-BACKTEST-RESULT/1\n", &bytes);
        result.json = bytes.into();
        Ok(result)
    }
}
