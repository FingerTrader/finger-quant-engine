//! Deterministic finite local research; training alone selects parameters, never validation.
use crate::{backtest::*, events::ExecutionError, simulator::SimulationConfig};
use fqe_core::types::{Evaluation, TypedValue};
use fqe_ir::{
    event_plan::LoadedEventPlan, financial_exchange as json, history::HistorySelection,
    model::ContentId, parameters::ParameterInput, registry::Registry, workflow::LoadedWorkflow,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub const MAX_TRIALS: usize = 16;
pub const MAX_TOTAL_SOURCE_BARS: usize = 4096;
type Result<T> = std::result::Result<T, ExecutionError>;
#[derive(Clone)]
pub enum SearchArtifact {
    Event(Arc<LoadedEventPlan>),
    Workflow(Arc<LoadedWorkflow>),
}
impl SearchArtifact {
    pub fn content_id(&self) -> ContentId {
        match self {
            Self::Event(v) => v.content_id(),
            Self::Workflow(v) => v.content_id(),
        }
    }
    pub fn source_id(&self) -> ContentId {
        match self {
            Self::Event(v) => v.source_id(),
            Self::Workflow(v) => v.source_id(),
        }
    }
    fn compile(
        &self,
        registry: &Registry,
        parameters: &BTreeMap<String, TypedValue>,
    ) -> Result<StrategyPlan> {
        let bindings = parameters
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    ParameterInput::Value(Evaluation::Ready(v.clone())),
                )
            })
            .collect::<Vec<_>>();
        Ok(match self {
            Self::Event(v) => StrategyPlan::Event(Arc::new(v.compile(registry, bindings)?)),
            Self::Workflow(v) => StrategyPlan::Workflow(Arc::new(v.compile(registry, bindings)?)),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterDimension {
    pub key: String,
    pub values: Vec<TypedValue>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchBudget {
    pub max_trials: usize,
    pub max_total_source_bars: usize,
    pub max_report_bytes: usize,
}
impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            max_trials: MAX_TRIALS,
            max_total_source_bars: MAX_TOTAL_SOURCE_BARS,
            max_report_bytes: json::MAX_FINANCIAL_REPORT_BYTES,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchControl {
    Complete,
    CancelAfterTrials(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchTerminal {
    Completed,
    Cancelled,
    NoSuccessfulTrials,
}
pub enum TrialOutcome {
    NotRun,
    Failed(ExecutionError),
    Completed(Box<BacktestResult>),
    Cancelled(Box<BacktestResult>),
}
pub struct TrialRecord {
    pub id: ContentId,
    pub parameters: BTreeMap<String, TypedValue>,
    pub outcome: TrialOutcome,
}
pub struct OptimizationRequest {
    pub artifact: SearchArtifact,
    pub registry: Arc<Registry>,
    pub training: HistorySelection,
    pub validation: HistorySelection,
    pub grid: Vec<ParameterDimension>,
    pub policies: Vec<PolicyBinding>,
    pub simulation: SimulationConfig,
    pub run: String,
    pub seed: u64,
    pub budget: SearchBudget,
    pub control: SearchControl,
}
pub struct PreparedOptimization {
    pub(crate) request: OptimizationRequest,
    pub(crate) parameters: Vec<BTreeMap<String, TypedValue>>,
    pub(crate) order: Vec<usize>,
    pub(crate) manifest: Arc<[u8]>,
    pub(crate) manifest_id: ContentId,
}
pub struct OptimizationResult {
    pub(crate) manifest: Arc<[u8]>,
    pub(crate) manifest_id: ContentId,
    pub(crate) trials: Vec<TrialRecord>,
    pub(crate) ranking: Vec<ContentId>,
    pub(crate) selected: Option<ContentId>,
    pub(crate) validation: TrialOutcome,
    pub(crate) terminal: SearchTerminal,
    pub(crate) completed_trials: usize,
    json: Arc<[u8]>,
    id: ContentId,
}
impl OptimizationResult {
    pub fn json(&self) -> &[u8] {
        &self.json
    }
    pub fn content_id(&self) -> ContentId {
        self.id
    }
    pub fn manifest_id(&self) -> ContentId {
        self.manifest_id
    }
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }
    pub fn trials(&self) -> &[TrialRecord] {
        &self.trials
    }
    pub fn ranking(&self) -> &[ContentId] {
        &self.ranking
    }
    pub fn selected(&self) -> Option<ContentId> {
        self.selected
    }
    pub fn validation(&self) -> &TrialOutcome {
        &self.validation
    }
    pub fn terminal(&self) -> SearchTerminal {
        self.terminal
    }
}
pub(crate) fn parameters_json(v: &BTreeMap<String, TypedValue>) -> json::JsonValue {
    json::object(v.iter().map(|(k, v)| (k.as_str(), json::typed_value(v))))
}
fn trial_id(v: &BTreeMap<String, TypedValue>) -> Result<ContentId> {
    Ok(json::content_id(
        b"FQE-OPTIMIZATION-PARAMETERS/1\n",
        &json::encode(&parameters_json(v), 262144)?,
    ))
}
// Fixed SplitMix64+Fisher-Yates definition; seed affects trial order, never financial prices/selection.
fn schedule(count: usize, seed: u64) -> Vec<usize> {
    let mut state = seed;
    let mut order = (0..count).collect::<Vec<_>>();
    for i in (1..count).rev() {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        order.swap(i, (z % (i as u64 + 1)) as usize);
    }
    order
}
impl PreparedOptimization {
    pub fn new(mut request: OptimizationRequest) -> Result<Self> {
        let r = &request;
        let b = &r.budget;
        if r.run.len() > 64
            || fqe_ir::model::identifier(&r.run, "$.search.run").is_err()
            || b.max_trials == 0
            || b.max_trials > MAX_TRIALS
            || b.max_total_source_bars == 0
            || b.max_total_source_bars > MAX_TOTAL_SOURCE_BARS
            || b.max_report_bytes == 0
            || b.max_report_bytes > json::MAX_FINANCIAL_REPORT_BYTES
        {
            return Err(error(
                "optimization_limit",
                "invalid run identifier or search budget",
            ));
        }
        if r.grid.is_empty()
            || r.grid.len() > 8
            || r.training.bars().len() > MAX_BACKTEST_BARS
            || r.validation.bars().len() > MAX_BACKTEST_BARS
        {
            return Err(error(
                "optimization_limit",
                "1..8 dimensions and bounded history selections required",
            ));
        }
        if r.training.dataset().content_id() != r.validation.dataset().content_id()
            || r.training
                .bars()
                .last()
                .expect("nonempty selection")
                .input()
                .close_ns
                > r.validation.performance_start_ns()
        {
            return Err(error(
                "optimization_split",
                "same immutable dataset and training end<=validation performance start required",
            ));
        }
        if !r.training.contiguous()
            || !r.validation.contiguous()
            || &r.simulation.account.instrument != r.training.dataset().spec().instrument()
            || r.simulation.timeframe_ns != r.training.dataset().spec().duration_ns()
            || r.simulation.signal_source.is_some()
        {
            return Err(error(
                "optimization_binding",
                "history/config/source mismatch; trial source must be derived",
            ));
        }
        crate::simulator::SimulationBroker::new(r.simulation.clone())?;
        request.grid.sort_by(|a, b| a.key.cmp(&b.key));
        let mut keys = BTreeSet::new();
        let mut parameters = vec![BTreeMap::new()];
        for dimension in &request.grid {
            fqe_ir::model::identifier(&dimension.key, "$.search.grid.key")?;
            if !keys.insert(&dimension.key)
                || dimension.values.is_empty()
                || dimension.values.len() > MAX_TRIALS
            {
                return Err(error(
                    "optimization_grid",
                    "unique dimension names and1..16 values required",
                ));
            }
            for (i, v) in dimension.values.iter().enumerate() {
                if dimension.values[..i].contains(v) {
                    return Err(error("optimization_grid", "duplicate exact value"));
                }
            }
            if parameters
                .len()
                .checked_mul(dimension.values.len())
                .is_none_or(|n| n > request.budget.max_trials)
            {
                return Err(error(
                    "optimization_limit",
                    "Cartesian grid exceeds trial budget",
                ));
            }
            parameters = parameters
                .into_iter()
                .flat_map(|p| {
                    dimension.values.iter().map(move |v| {
                        let mut p = p.clone();
                        p.insert(dimension.key.clone(), v.clone());
                        p
                    })
                })
                .collect();
        }
        let total = parameters
            .len()
            .checked_mul(request.training.bars().len())
            .and_then(|n| n.checked_add(request.validation.bars().len()))
            .ok_or_else(|| error("optimization_limit", "source count overflow"))?;
        if total > request.budget.max_total_source_bars
            || matches!(request.control,SearchControl::CancelAfterTrials(n) if n>parameters.len())
        {
            return Err(error(
                "optimization_limit",
                "total source budget or cancel boundary exceeded",
            ));
        }
        let order = schedule(parameters.len(), request.seed);
        let mut p = Self {
            request,
            parameters,
            order,
            manifest: Arc::from([]),
            manifest_id: ContentId::from_bytes([0; 32]),
        };
        let bytes = crate::optimization_encoding::manifest(&p)?;
        p.manifest_id = json::content_id(b"FQE-OPTIMIZATION-INPUTS/1\n", &bytes);
        p.manifest = bytes.into();
        Ok(p)
    }
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }
    pub fn manifest_id(&self) -> ContentId {
        self.manifest_id
    }
    fn execute(
        &self,
        parameters: &BTreeMap<String, TypedValue>,
        selection: HistorySelection,
        run: &str,
        seed: u64,
        cancel: &AtomicBool,
    ) -> TrialOutcome {
        let execution = (|| {
            let plan = self
                .request
                .artifact
                .compile(&self.request.registry, parameters)?;
            PreparedBacktest::new(
                selection,
                plan,
                self.request.policies.clone(),
                self.request.simulation.clone(),
                run,
                seed,
                RunControl::Complete,
            )?
            .run_with_cancel(cancel)
        })();
        match execution {
            Ok(r) if r.terminal() == BacktestTerminal::Cancelled => {
                TrialOutcome::Cancelled(Box::new(r))
            }
            Ok(r) => TrialOutcome::Completed(Box::new(r)),
            Err(e) => TrialOutcome::Failed(e),
        }
    }
    pub fn run(&self) -> Result<OptimizationResult> {
        self.run_with_cancel(&AtomicBool::new(false))
    }
    pub fn run_with_cancel(&self, cancel: &AtomicBool) -> Result<OptimizationResult> {
        let mut trials = self
            .parameters
            .iter()
            .map(|p| {
                Ok(TrialRecord {
                    id: trial_id(p)?,
                    parameters: p.clone(),
                    outcome: TrialOutcome::NotRun,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut terminal = SearchTerminal::Completed;
        for (attempted, &index) in self.order.iter().enumerate() {
            if cancel.load(Ordering::Acquire)
                || matches!(self.request.control,SearchControl::CancelAfterTrials(n) if attempted>=n)
            {
                terminal = SearchTerminal::Cancelled;
                break;
            }
            trials[index].outcome = self.execute(
                &self.parameters[index],
                self.request.training.clone(),
                &format!("{}.t{}", self.request.run, index),
                self.request.seed ^ (index as u64),
                cancel,
            );
            if matches!(trials[index].outcome, TrialOutcome::Cancelled(_)) {
                terminal = SearchTerminal::Cancelled;
                break;
            }
        }
        if matches!(self.request.control, SearchControl::CancelAfterTrials(_)) {
            terminal = SearchTerminal::Cancelled;
        }
        let mut rank = trials
            .iter()
            .enumerate()
            .filter_map(|(i, t)| matches!(t.outcome, TrialOutcome::Completed(_)).then_some(i))
            .collect::<Vec<_>>();
        rank.sort_by(|&i, &j| {
            let (TrialOutcome::Completed(a), TrialOutcome::Completed(b)) =
                (&trials[i].outcome, &trials[j].outcome)
            else {
                unreachable!("completed filter")
            };
            b.metrics()
                .net_profit
                .coefficient()
                .cmp(&a.metrics().net_profit.coefficient())
                .then_with(|| {
                    a.metrics()
                        .max_drawdown
                        .coefficient()
                        .cmp(&b.metrics().max_drawdown.coefficient())
                })
                .then_with(|| trials[i].id.bytes().cmp(trials[j].id.bytes()))
        });
        let selected = rank.first().map(|&i| trials[i].id);
        let mut validation = TrialOutcome::NotRun;
        if terminal == SearchTerminal::Completed {
            if let Some(&index) = rank.first() {
                if cancel.load(Ordering::Acquire) {
                    terminal = SearchTerminal::Cancelled;
                } else {
                    validation = self.execute(
                        &self.parameters[index],
                        self.request.validation.clone(),
                        &format!("{}.validation", self.request.run),
                        self.request.seed ^ (index as u64),
                        cancel,
                    );
                    if matches!(validation, TrialOutcome::Cancelled(_)) {
                        terminal = SearchTerminal::Cancelled;
                    }
                }
            } else {
                terminal = SearchTerminal::NoSuccessfulTrials;
            }
        }
        let ranking = rank.iter().map(|&i| trials[i].id).collect();
        let completed_trials = rank.len();
        let mut r = OptimizationResult {
            manifest: Arc::clone(&self.manifest),
            manifest_id: self.manifest_id,
            trials,
            ranking,
            selected,
            validation,
            terminal,
            completed_trials,
            json: Arc::from([]),
            id: ContentId::from_bytes([0; 32]),
        };
        let bytes = crate::optimization_encoding::result(&r, self.request.budget.max_report_bytes)?;
        r.id = json::content_id(b"FQE-OPTIMIZATION-RESULT/1\n", &bytes);
        r.json = bytes.into();
        Ok(r)
    }
}
