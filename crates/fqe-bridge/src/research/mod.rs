//! Versioned native graph research path. Legacy financial profiles remain unchanged.
mod chart_compat;
mod graph;
pub mod live;
pub mod optimization;
mod orders;
use crate::{Failure, Result};
use fqe_core::{
    bars::ClosedBar,
    finance::{FinancialRounding, align, decimal_text, parse_decimal},
    numeric::{Decimal, Rounding},
    types::ScalarValue,
};
use fqe_ir::{
    financial_exchange::{self as json, JsonValue as V, object as o, string as s},
    history::{HistoricalDataset, HistorySelection, HistorySpec},
};
use graph::{Atom, Graph, Node, Slot, inherit};
use orders::{Broker, Order, Protection};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
type Timeline = BTreeMap<u64, (Vec<(usize, usize)>, Vec<(usize, usize)>)>;
pub const PROFILE: &str = "fqe.shared-cash-restricted-short-open/1";
pub const MAX_BARS: usize = 20000;
const SCALE: u8 = 12;
const FACTOR: i128 = 1_000_000_000_000;
fn failure(code: &'static str, path: impl ToString) -> Failure {
    Failure {
        code,
        path: path.to_string(),
    }
}
fn numeric_error(_: fqe_core::numeric::NumericError) -> Failure {
    failure("research_numeric", "$")
}
impl From<fqe_core::finance::FinancialError> for Failure {
    fn from(_: fqe_core::finance::FinancialError) -> Self {
        failure("research_financial_units", "$")
    }
}
fn exact(v: &V, keys: &[&str]) -> Result<()> {
    let obj = v
        .as_object()
        .ok_or_else(|| failure("research_object", "$"))?;
    if obj.len() != keys.len() || keys.iter().any(|k| !obj.contains_key(*k)) {
        return Err(failure("research_fields", "$"));
    }
    Ok(())
}
fn strv<'a>(v: &'a V, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(V::as_str)
        .ok_or_else(|| failure("research_string", k))
}
fn arr<'a>(v: &'a V, k: &str) -> Result<&'a Vec<V>> {
    v.get(k)
        .and_then(V::as_array)
        .ok_or_else(|| failure("research_array", k))
}
fn parse(t: &str, scale: u8) -> Result<Decimal> {
    Ok(parse_decimal(t, scale)?)
}
fn num(t: &str) -> Result<Decimal> {
    parse(t, SCALE)
}
fn whole(n: i128) -> Result<Decimal> {
    Decimal::new(
        n.checked_mul(FACTOR)
            .ok_or_else(|| failure("research_numeric", "$"))?,
        SCALE,
    )
    .map_err(numeric_error)
}
fn wide(n: Decimal) -> Result<Decimal> {
    n.rescale(SCALE, Rounding::Exact).map_err(numeric_error)
}
fn add(a: Decimal, b: Decimal) -> Result<Decimal> {
    let scale = a.scale().max(b.scale());
    a.rescale(scale, Rounding::Exact)
        .and_then(|a| a.checked_add(b.rescale(scale, Rounding::Exact)?))
        .map_err(numeric_error)
}
fn sub(a: Decimal, b: Decimal) -> Result<Decimal> {
    let scale = a.scale().max(b.scale());
    a.rescale(scale, Rounding::Exact)
        .and_then(|a| a.checked_sub(b.rescale(scale, Rounding::Exact)?))
        .map_err(numeric_error)
}
fn mul(a: Decimal, b: Decimal) -> Result<Decimal> {
    wide(a)?
        .checked_mul(wide(b)?, Rounding::HalfEven)
        .map_err(numeric_error)
}
fn div(a: Decimal, b: Decimal) -> Result<Decimal> {
    wide(a)?
        .checked_div(wide(b)?, Rounding::HalfEven)
        .map_err(numeric_error)
}
fn absd(a: Decimal) -> Result<Decimal> {
    Decimal::new(
        a.coefficient()
            .checked_abs()
            .ok_or_else(|| failure("research_numeric", "$"))?,
        a.scale(),
    )
    .map_err(numeric_error)
}
fn mind(a: Decimal, b: Decimal) -> Decimal {
    if a.rescale(SCALE, Rounding::Exact)
        .expect("bounded decimals")
        .coefficient()
        <= b.rescale(SCALE, Rounding::Exact)
            .expect("bounded decimals")
            .coefficient()
    {
        a
    } else {
        b
    }
}
fn maxd(a: Decimal, b: Decimal) -> Decimal {
    if a.rescale(SCALE, Rounding::Exact)
        .expect("bounded decimals")
        .coefficient()
        >= b.rescale(SCALE, Rounding::Exact)
            .expect("bounded decimals")
            .coefficient()
    {
        a
    } else {
        b
    }
}
fn sum(v: impl IntoIterator<Item = Decimal>) -> Result<Decimal> {
    v.into_iter().try_fold(whole(0)?, add)
}
fn step_floor(n: Decimal, step: Decimal) -> Result<Decimal> {
    if step.coefficient() <= 0 {
        return Err(failure("research_step", "$"));
    }
    Ok(align(n, step, FinancialRounding::Floor)?)
}
fn sqrt(n: Decimal) -> Result<Decimal> {
    if n.coefficient() < 0 {
        return Err(failure("research_sqrt", "$"));
    }
    let v = (n.coefficient() as u128)
        .checked_mul(FACTOR as u128)
        .ok_or_else(|| failure("research_numeric", "$"))?;
    let mut low = 0_u128;
    let mut high = v.min(u64::MAX as u128) + 1;
    while low + 1 < high {
        let m = (low + high) / 2;
        if m <= v / m { low = m } else { high = m }
    }
    Decimal::new(low as i128, SCALE).map_err(numeric_error)
}
fn ds(v: Decimal) -> V {
    s(decimal_text(v))
}
fn bar_value(v: &fqe_core::types::TypedValue) -> Result<Decimal> {
    match v.value() {
        ScalarValue::Decimal(n) => wide(*n),
        _ => Err(failure("research_bar_type", "$")),
    }
}
fn ctx_uint(c: &BTreeMap<String, String>, k: &str, max: u64) -> Result<u64> {
    let v = c.get(k).ok_or_else(|| failure("research_context", k))?;
    let n = v
        .parse::<u64>()
        .map_err(|_| failure("research_unsigned", k))?;
    if n > max || n.to_string() != *v {
        return Err(failure("research_unsigned", k));
    }
    Ok(n)
}
struct Stream {
    id: String,
    selection: HistorySelection,
}
pub struct Prepared {
    graph: Graph,
    streams: Vec<Stream>,
    main: usize,
    broker: Broker,
    manifest: V,
    manifest_id: String,
    execution_id: String,
}
pub struct Output {
    pub bytes: Vec<u8>,
    pub result_id: String,
    pub manifest_id: String,
    pub processed: usize,
    pub cancelled: bool,
}
impl Prepared {
    pub fn new(input: &V) -> Result<Self> {
        exact(input, &["kind", "model", "streams", "main", "run", "seed"])?;
        if input["kind"] != "research" {
            return Err(failure("research_kind", "$"));
        }
        fqe_ir::model::identifier(strv(input, "run")?, "$.run")?;
        let seed = strv(input, "seed")?;
        if seed
            .parse::<u64>()
            .ok()
            .is_none_or(|n| n.to_string() != seed)
        {
            return Err(failure("research_seed", "$.seed"));
        }
        let mut graph = Graph::compile(&json::decode_input(strv(input, "model")?.as_bytes())?)?;
        if graph.nodes.iter().any(|n| n.kind == "quotes") {
            return Err(failure("research_quote_history_required", "$.model"));
        }
        if graph.context.get("trigger").map(String::as_str) != Some("bar_close") {
            return Err(failure("research_trigger", "$.context"));
        }
        for key in ["stopLoss", "takeProfit"] {
            if graph.context.get(key).is_some_and(|v| !v.is_empty()) {
                return Err(failure("research_legacy_protection", key));
            }
        }
        if graph.context.keys().any(|k| {
            ![
                "accountMode",
                "capitalModel",
                "leverage",
                "entryPercent",
                "dataset",
                "direction",
                "end",
                "feeBps",
                "initialCash",
                "indicatorProfile",
                "maxTrials",
                "objective",
                "positionMode",
                "searchMethod",
                "slippageBps",
                "start",
                "stopLoss",
                "symbol",
                "takeProfit",
                "timeframe",
                "trainingDataset",
                "trigger",
                "validationDataset",
                "warmupBars",
            ]
            .contains(&k.as_str())
        }) {
            return Err(failure("research_unknown_context", "$.context"));
        }
        let list = arr(input, "streams")?;
        if list.is_empty() || list.len() > 16 {
            return Err(failure("research_stream_quota", "$.streams"));
        }
        let mut streams = vec![];
        let mut ids = BTreeSet::new();
        let mut instruments = BTreeMap::new();
        let mut total = 0;
        let mut market_data = BTreeMap::new();
        for stream in list {
            exact(
                stream,
                &[
                    "id",
                    "history_spec",
                    "csv",
                    "start_ns",
                    "end_ns",
                    "warmup_bars",
                ],
            )?;
            let id = strv(stream, "id")?.to_owned();
            if !ids.insert(id.clone()) {
                return Err(failure("research_duplicate_stream", id));
            }
            let spec = Arc::new(HistorySpec::from_bytes(
                strv(stream, "history_spec")?.as_bytes(),
            )?);
            let dataset = Arc::new(HistoricalDataset::from_csv(
                spec.clone(),
                strv(stream, "csv")?.as_bytes(),
                None,
            )?);
            let key = (
                spec.instrument()
                    .declaration()
                    .instrument
                    .as_str()
                    .to_owned(),
                spec.duration_ns(),
            );
            if market_data
                .insert(key, dataset.bars_id())
                .is_some_and(|old| old != dataset.bars_id())
            {
                return Err(failure("research_conflicting_market_history", &id));
            }
            let unsigned = |k: &str| -> Result<u64> {
                let t = strv(stream, k)?;
                let n = t
                    .parse::<u64>()
                    .map_err(|_| failure("research_unsigned", k))?;
                if n.to_string() != t {
                    return Err(failure("research_unsigned", k));
                }
                Ok(n)
            };
            let selection = dataset.select(
                unsigned("start_ns")?,
                unsigned("end_ns")?,
                usize::try_from(unsigned("warmup_bars")?)
                    .map_err(|_| failure("research_warmup", "$"))?,
            )?;
            if !selection.contiguous() {
                return Err(failure("research_contiguous_history_required", id));
            }
            total += selection.bars().len();
            if total > MAX_BARS {
                return Err(failure("research_bar_quota", "$.streams"));
            }
            let symbol = spec
                .instrument()
                .declaration()
                .instrument
                .as_str()
                .to_owned();
            if let Some(old) = instruments.insert(symbol, spec.instrument().clone())
                && old != *spec.instrument()
            {
                return Err(failure("research_instrument_conflict", id));
            }
            streams.push(Stream { id, selection });
        }
        let main = streams
            .iter()
            .position(|s| s.id == strv(input, "main").unwrap_or(""))
            .ok_or_else(|| failure("research_main_stream", "$.main"))?;
        let spec = streams[main].selection.dataset().spec();
        if graph.context.get("symbol")
            != Some(
                &spec
                    .instrument()
                    .declaration()
                    .instrument
                    .as_str()
                    .to_owned(),
            )
            || graph.context.get("timeframe").map(String::as_str) != Some(spec.timeframe())
        {
            return Err(failure("research_main_binding", "$.context"));
        }
        let context = graph.context.clone();
        for n in &mut graph.nodes {
            if n.kind == "bars" {
                for key in ["symbol", "timeframe", "dataset"] {
                    if n.settings.get(key).is_none_or(|s| s.is_empty()) {
                        n.settings.insert(
                            key.into(),
                            context
                                .get(key)
                                .ok_or_else(|| failure("research_context", key))?
                                .clone(),
                        );
                    }
                }
                let stream = streams
                    .iter()
                    .find(|s| Some(&s.id) == n.settings.get("dataset"))
                    .ok_or_else(|| failure("research_data_binding", &n.id))?;
                let spec = stream.selection.dataset().spec();
                if n.get("symbol")? != spec.instrument().declaration().instrument.as_str()
                    || n.get("timeframe")? != spec.timeframe()
                {
                    return Err(failure("research_data_binding", &n.id));
                }
            }
            if ["entry", "exit", "position"].contains(&n.kind.as_str()) {
                let symbol = inherit(n, "symbol", &context)?;
                if !instruments.contains_key(&symbol) {
                    return Err(failure("research_action_data_missing", &n.id));
                }
                if n.get("side")? == "short"
                    && context.get("accountMode").map(String::as_str) != Some("margin")
                {
                    return Err(failure("research_short_requires_margin", &n.id));
                }
                if n.kind == "entry" {
                    for key in ["stopLoss", "takeProfit", "trailing"] {
                        if n.get(&format!("{key}Mode"))? == "atr"
                            && !n.literals.contains_key("atr")
                            && !graph.wires.contains_key(&(n.id.clone(), "atr".into()))
                        {
                            return Err(failure("research_atr_missing", &n.id));
                        }
                    }
                }
            }
        }
        // One deterministic execution stream per traded symbol; other timeframes
        // remain observation-only, never a second opportunity to fill the same order.
        for symbol in instruments.keys() {
            let chosen = execution_stream(&streams, symbol, main)?;
            for stream in streams.iter().filter(|s| {
                s.selection
                    .dataset()
                    .spec()
                    .instrument()
                    .declaration()
                    .instrument
                    .as_str()
                    == symbol
            }) {
                if stream.selection.dataset().spec().duration_ns()
                    < streams[chosen].selection.dataset().spec().duration_ns()
                {
                    return Err(failure("research_execution_stream", "$"));
                }
            }
        }
        let broker = Broker::new(instruments, &graph.context)?;
        let execution_id =
            json::content_id(b"FQE-RESEARCH-GRAPH/1\n", strv(input, "model")?.as_bytes())
                .to_string();
        let manifest = o([
            ("format", s("fqe.research-inputs")),
            ("schema_version", V::from(1)),
            (
                "engine_id",
                s(broker.hub.journal()[0]
                    .opening
                    .as_ref()
                    .expect("native opening")
                    .engine_id),
            ),
            ("bridge_id", s(crate::bridge_id())),
            ("profile", s(broker.profile())),
            ("run", input["run"].clone()),
            ("seed", input["seed"].clone()),
            (
                "input_id",
                s(json::content_id(
                    b"FQE-RESEARCH-INPUT/1\n",
                    &json::encode(input, crate::MAX_REQUEST)?,
                )),
            ),
            ("execution_id", s(&execution_id)),
            ("initial_cash", ds(broker.initial)),
            (
                "streams",
                V::Array(
                    streams
                        .iter()
                        .map(|stream| {
                            o([
                                ("id", s(&stream.id)),
                                ("dataset_id", s(stream.selection.dataset().content_id())),
                                ("selection_id", s(stream.selection.content_id())),
                                ("bars", s(stream.selection.bars().len())),
                                ("warmup_bars", s(stream.selection.warmup_bars())),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]);
        let manifest_id = json::content_id(
            b"FQE-RESEARCH-MANIFEST/1\n",
            &json::encode(&manifest, crate::MAX_REQUEST)?,
        )
        .to_string();
        Ok(Self {
            graph,
            streams,
            main,
            broker,
            manifest,
            manifest_id,
            execution_id,
        })
    }
    pub fn inspect(&self) -> V {
        o([
            ("valid", V::Bool(true)),
            ("execution_id", s(&self.execution_id)),
            ("registry_id", s(crate::bridge_id())),
            ("manifest_id", s(&self.manifest_id)),
            ("profile", s(self.broker.profile())),
            (
                "source_bars",
                s(self
                    .streams
                    .iter()
                    .map(|s| s.selection.bars().len())
                    .sum::<usize>()),
            ),
        ])
    }
    pub fn run(self, cancel: &AtomicBool) -> Result<Output> {
        self.run_observed(cancel, |_| Ok(true), |_| Ok(()))
    }
    pub fn run_observed(
        mut self,
        cancel: &AtomicBool,
        mut before: impl FnMut(usize) -> Result<bool>,
        mut committed: impl FnMut(V) -> Result<()>,
    ) -> Result<Output> {
        let mut timeline: Timeline = BTreeMap::new();
        let main_selection = &self.streams[self.main].selection;
        let start = main_selection
            .bars()
            .first()
            .ok_or_else(|| failure("research_history_empty", "$"))?
            .input()
            .open_ns;
        let end = main_selection
            .bars()
            .last()
            .expect("history")
            .input()
            .close_ns;
        for (s, stream) in self.streams.iter().enumerate() {
            for (i, bar) in stream.selection.bars().iter().enumerate() {
                let b = bar.input();
                if b.close_ns <= end {
                    timeline.entry(b.close_ns).or_default().0.push((s, i));
                }
                if b.open_ns >= start && b.open_ns < end {
                    timeline.entry(b.open_ns).or_default().1.push((s, i));
                }
            }
        }
        let mut sources = BTreeMap::new();
        let mut previous_volume: BTreeMap<String, Decimal> = BTreeMap::new();
        let mut frames = vec![o([
            ("phase", s("initial")),
            ("timestamp_ns", s(start)),
            ("source_sequence", s(0)),
            ("account", self.broker.snapshot()?),
            ("receipts", V::Array(vec![])),
        ])];
        let mut curve = vec![];
        // Exact committed indicator samples, retained for the entire processed
        // prefix. The bounded diagnostic observation window is not a chart.
        let chart_columns: Vec<V> = self
            .graph
            .nodes
            .iter()
            .filter(|n| {
                [
                    "sma",
                    "ema",
                    "rsi",
                    "macd",
                    "atr",
                    "boll",
                    "highest",
                    "lowest",
                    "crossUp",
                    "crossDown",
                ]
                .contains(&n.kind.as_str())
            })
            .flat_map(|n| {
                n.outputs.keys().map(|port| {
                    o([
                        ("node", s(&n.id)),
                        ("port", s(port)),
                        ("kind", s(&n.kind)),
                        ("unit", s(&n.outputs[port])),
                        (
                            "settings",
                            o(n.settings.iter().map(|(k, v)| (k.clone(), s(v)))),
                        ),
                        (
                            "inputs",
                            o(self
                                .graph
                                .wires
                                .iter()
                                .filter(|((id, _), _)| id == &n.id)
                                .map(|((_, input), (from, output))| {
                                    (input.clone(), s(format!("{from}.{output}")))
                                })),
                        ),
                    ])
                })
            })
            .collect();
        let mut chart_rows = vec![];
        let mut observations = vec![];
        let mut decisions = vec![];
        let mut last_condition = BTreeMap::new();
        let mut processed = 0;
        let mut clock = 0;
        let mut cancelled = false;
        let mut latest = start;
        let mut emitted_events = 0;
        let mut emitted_fills = 0;
        let mut emitted_decisions = 0;
        for (time, (closes, opens)) in timeline {
            if closes.iter().any(|(stream, _)| *stream == self.main) && !before(processed)? {
                cancelled = true;
                break;
            }
            if cancel.load(Ordering::Acquire) {
                cancelled = true;
                break;
            }
            latest = time;
            let mut main_bar = None;
            let mut values = BTreeMap::new();
            for (s, i) in &closes {
                let stream = &self.streams[*s];
                let bar = &stream.selection.bars()[*i];
                let b = bar.input();
                let symbol = stream
                    .selection
                    .dataset()
                    .spec()
                    .instrument()
                    .declaration()
                    .instrument
                    .as_str();
                let execution = execution_stream(&self.streams, symbol, self.main)?;
                if execution == *s {
                    self.broker.on_close(symbol, bar)?;
                    previous_volume.insert(symbol.into(), bar_value(&b.volume)?);
                }
                sources.insert(
                    stream.id.clone(),
                    (
                        b.close_ns,
                        BTreeMap::from([
                            ("open".into(), bar_value(&b.open)?),
                            ("high".into(), bar_value(&b.high)?),
                            ("low".into(), bar_value(&b.low)?),
                            ("close".into(), bar_value(&b.close)?),
                            ("volume".into(), bar_value(&b.volume)?),
                        ]),
                    ),
                );
                if *s == self.main {
                    main_bar = Some((*i, bar.clone()));
                }
            }
            if let Some((_, bar)) = &main_bar {
                clock += 1;
                processed += 1;
                let (account, positions) = self.broker.graph_views(time)?;
                values = self
                    .graph
                    .evaluate(&sources, &account, &positions, time, true, clock)?;
                if observations.len() == 256 {
                    observations.remove(0);
                }
                chart_rows.push(o([
                    ("source_sequence", s(bar.input().sequence)),
                    ("timestamp_ns", s(time)),
                    (
                        "values",
                        V::Array(
                            chart_columns
                                .iter()
                                .map(|c| {
                                    values
                                        .get(&(
                                            c["node"].as_str().unwrap().into(),
                                            c["port"].as_str().unwrap().into(),
                                        ))
                                        .and_then(|v| v.value.as_ref())
                                        .map_or(V::Null, Atom::json)
                                })
                                .collect(),
                        ),
                    ),
                ]));
                observations.push(o([
                    ("source_sequence", s(bar.input().sequence)),
                    ("timestamp_ns", s(time)),
                    (
                        "outputs",
                        o(values.iter().map(|((n, p), v)| {
                            (
                                format!("{n}.{p}"),
                                v.value.as_ref().map_or(V::Null, Atom::json),
                            )
                        })),
                    ),
                ]));
            } else if !closes.is_empty() {
                let (account, positions) = self.broker.graph_views(time)?;
                self.graph
                    .evaluate(&sources, &account, &positions, time, false, clock)?;
            }
            if let Some((index, bar)) = main_bar {
                let warmup = index < self.streams[self.main].selection.warmup_bars();
                if !warmup {
                    self.protect(&values, time)?;
                    for node in
                        self.graph.nodes.clone().into_iter().filter(|n| {
                            ["entry", "exit", "cancelOrders"].contains(&n.kind.as_str())
                        })
                    {
                        let condition = self.graph.input(&node, "condition", &values)?.value;
                        let ready = matches!(condition, Some(Atom::Boolean(true)));
                        let previous = last_condition
                            .insert(node.id.clone(), ready)
                            .unwrap_or(false);
                        let rising = node.get("triggerMode")? == "rising_edge";
                        let fire = ready && (!rising || !previous);
                        let outcome = if fire {
                            self.act(&node, &values, time)?
                        } else {
                            "condition_false_or_not_ready".into()
                        };
                        decisions.push(o([
                            ("node", s(&node.id)),
                            ("timestamp_ns", s(time)),
                            ("source_sequence", s(bar.input().sequence)),
                            ("outcome", s(outcome)),
                        ]));
                    }
                    let equity = self.broker.equity()?;
                    curve.push(o([
                        ("source_sequence", s(bar.input().sequence)),
                        ("timestamp_ns", s(time)),
                        ("equity", ds(equity)),
                        (
                            "cash",
                            ds(if self.broker.margin.is_some() {
                                self.broker.settled_balance()?
                            } else {
                                self.broker.hub.view().cash
                            }),
                        ),
                        ("fees", ds(self.broker.fees)),
                    ]));
                }
                frames.push(o([
                    (
                        "phase",
                        s(if warmup {
                            "warmup_close"
                        } else {
                            "decision_close"
                        }),
                    ),
                    ("timestamp_ns", s(time)),
                    ("source_sequence", s(bar.input().sequence)),
                    ("account", self.broker.snapshot()?),
                    ("receipts", V::Array(vec![])),
                ]));
            }
            if closes.iter().any(|(stream, _)| *stream == self.main) {
                let account = self.broker.snapshot()?;
                let equity = self.broker.equity()?;
                let gross = if self.broker.margin.is_some() {
                    add(
                        sub(self.broker.settled_balance()?, self.broker.initial)?,
                        self.broker.fees,
                    )?
                } else {
                    let mut value = self.broker.hub.view().realized_gross;
                    for leg in self.broker.legs.values() {
                        value = add(value, leg.ledger.view().realized_gross)?;
                    }
                    value
                };
                let realized = sub(gross, self.broker.fees)?;
                committed(o([
                    ("processed", s(processed)),
                    ("total", s(self.streams[self.main].selection.bars().len())),
                    ("timestamp_ns", s(time)),
                    (
                        "source_sequence",
                        frames.last().expect("close frame")["source_sequence"].clone(),
                    ),
                    ("chart_columns", V::Array(chart_columns.clone())),
                    (
                        "chart_sample",
                        chart_rows.last().cloned().unwrap_or(V::Null),
                    ),
                    ("initial_equity", ds(self.broker.initial)),
                    ("profile", s(self.broker.profile())),
                    ("account", account),
                    (
                        "point",
                        o([
                            ("timestamp_ns", s(time)),
                            ("equity", ds(equity)),
                            (
                                "cash",
                                ds(if self.broker.margin.is_some() {
                                    self.broker.settled_balance()?
                                } else {
                                    self.broker.hub.view().cash
                                }),
                            ),
                            ("fees", ds(self.broker.fees)),
                            ("closed_pnl", ds(gross)),
                            ("realized_pnl", ds(realized)),
                            (
                                "unrealized_pnl",
                                ds(sub(sub(equity, self.broker.initial)?, realized)?),
                            ),
                        ]),
                    ),
                    (
                        "marks",
                        o(self.broker.marks.iter().map(|(k, v)| (k.clone(), ds(*v)))),
                    ),
                    (
                        "fills",
                        V::Array(self.broker.fills[emitted_fills..].to_vec()),
                    ),
                    (
                        "execution_events",
                        V::Array(self.broker.events[emitted_events..].to_vec()),
                    ),
                    (
                        "decisions",
                        V::Array(decisions[emitted_decisions..].to_vec()),
                    ),
                ]))?;
                emitted_fills = self.broker.fills.len();
                emitted_events = self.broker.events.len();
                emitted_decisions = decisions.len();
                // Incoming opens belong to the NEXT bar. Do not match them
                // behind a paused consumer after publishing the prior close.
                if !opens.is_empty() && !before(processed)? {
                    cancelled = true;
                    break;
                }
            }
            let events_before = self.broker.events.len();
            for (s, i) in opens {
                let stream = &self.streams[s];
                let symbol = stream
                    .selection
                    .dataset()
                    .spec()
                    .instrument()
                    .declaration()
                    .instrument
                    .as_str();
                if s == execution_stream(&self.streams, symbol, self.main)? {
                    self.broker.on_open(
                        symbol,
                        &stream.selection.bars()[i],
                        previous_volume.get(symbol).copied().unwrap_or(whole(0)?),
                    )?;
                }
            }
            if self.broker.events.len() > events_before {
                frames.push(o([
                    ("phase", s("incoming_open")),
                    ("timestamp_ns", s(time)),
                    ("source_sequence", s(processed)),
                    ("account", self.broker.snapshot()?),
                    (
                        "receipts",
                        V::Array(self.broker.events[events_before..].to_vec()),
                    ),
                ]));
            }
        }
        let before = self.broker.events.len();
        self.broker.finish(latest)?;
        frames.push(o([
            ("phase", s("terminal")),
            ("timestamp_ns", s(latest)),
            ("source_sequence", s(processed)),
            ("account", self.broker.snapshot()?),
            ("receipts", V::Array(self.broker.events[before..].to_vec())),
        ]));
        let metrics = metrics(
            self.broker.initial,
            self.broker.equity()?,
            self.broker.fees,
            &curve,
        )?;
        let result = o([
            ("format", s("fqe.backtest-result")),
            ("schema_version", V::from(1)),
            ("profile", s(self.broker.profile())),
            (
                "terminal",
                s(if cancelled { "cancelled" } else { "completed" }),
            ),
            ("manifest", self.manifest),
            ("manifest_id", s(&self.manifest_id)),
            ("processed_source_bars", s(processed)),
            ("account", self.broker.snapshot()?),
            ("metrics", metrics),
            ("equity_curve", V::Array(curve)),
            (
                "chart_trace",
                o([
                    ("schema_version", V::from(1)),
                    ("columns", V::Array(chart_columns)),
                    ("rows", V::Array(chart_rows)),
                ]),
            ),
            ("observation_window", s("last-256-decisions")),
            ("observations", V::Array(observations)),
            ("decisions", V::Array(decisions)),
            ("execution_events", V::Array(self.broker.events.clone())),
            (
                "orders",
                V::Array(self.broker.orders.iter().map(orders::order_json).collect()),
            ),
            ("fills", V::Array(self.broker.fills.clone())),
            ("journal", self.broker.journals()),
            ("frames", V::Array(frames)),
            (
                "warnings",
                V::Array(vec![s(
                    "experimental; opening-only fills; fully collateralized restricted shorts; no live routing",
                )]),
            ),
        ]);
        let bytes = crate::codec::encode(&result, crate::MAX_RESPONSE - 1024)?;
        let result_id = json::content_id(b"FQE-BACKTEST-RESULT/1\n", &bytes).to_string();
        Ok(Output {
            bytes,
            result_id,
            manifest_id: self.manifest_id,
            processed,
            cancelled,
        })
    }
    fn value(
        &self,
        n: &Node,
        p: &str,
        values: &BTreeMap<(String, String), Slot>,
    ) -> Result<Option<Decimal>> {
        let slot = self.graph.input(n, p, values)?;
        if let Some(v) = slot.value {
            return Ok(Some(v.number()?));
        }
        if self.graph.wires.contains_key(&(n.id.clone(), p.into())) || n.literals.contains_key(p) {
            return Ok(None);
        }
        n.settings.get(p).map(|v| num(v)).transpose()
    }
    fn act(
        &mut self,
        n: &Node,
        values: &BTreeMap<(String, String), Slot>,
        time: u64,
    ) -> Result<String> {
        let symbol = inherit(n, "symbol", &self.graph.context)?;
        if n.kind == "cancelOrders" {
            let scope = n.get("scope")?;
            let tag = n.get("orderTag")?;
            if scope == "tag" && tag.is_empty() {
                return Err(failure("research_cancel_tag", &n.id));
            }
            for i in 0..self.broker.orders.len() {
                let o = &self.broker.orders[i];
                if Broker::active(o)
                    && o.node != "__fqe.margin"
                    && (scope == "all"
                        || (o.symbol == symbol && (scope == "symbol" || o.tag == tag)))
                {
                    self.broker.cancel(i, "cancelled_rule", time)?;
                }
            }
            return Ok("cancelled_matching_pending".into());
        }
        let side = n.get("side")?;
        // A close signal supersedes outstanding opening intent on THIS leg,
        // including a wholly unfilled order. Cancel before flat/pending guards
        // and quantity selection; never let a stale entry refill behind exit.
        if n.kind == "exit" {
            self.cancel_entries(&symbol, side, "cancelled_exit_signal", time)?;
        }
        let key = (symbol.clone(), side.into());
        let quantity = self.broker.quantity(&symbol, side)?;
        let entry = n.kind == "entry";
        if entry {
            if self.broker.pending(&symbol, side, false) {
                return Ok("pending_exit".into());
            }
            if self.graph.context.get("positionMode").map(String::as_str) == Some("net")
                && (self
                    .broker
                    .quantity(&symbol, if side == "long" { "short" } else { "long" })?
                    .coefficient()
                    > 0
                    || self.broker.pending(
                        &symbol,
                        if side == "long" { "short" } else { "long" },
                        true,
                    ))
            {
                return Ok("opposite_position_open".into());
            }
            if self.broker.pending(&symbol, side, true) {
                return Ok("pending_entry".into());
            }
            if n.get("entryMode")? == "open" && quantity.coefficient() > 0 {
                return Ok("position_open".into());
            }
            if n.get("entryMode")? == "add"
                && (quantity.coefficient() == 0
                    || self.broker.legs[&key].adds
                        >= n.get("maxAdds")?
                            .parse::<u64>()
                            .map_err(|_| failure("research_max_adds", &n.id))?)
            {
                return Ok("add_limit_or_flat".into());
            }
        } else if quantity.coefficient() == 0 || self.broker.pending(&symbol, side, false) {
            return Ok("flat_or_pending_exit".into());
        }
        if entry
            && self
                .broker
                .margin
                .as_ref()
                .is_some_and(|r| r.status != "healthy")
        {
            return Ok("margin_liquidation".into());
        }
        let margin_cap = if entry && self.broker.margin.is_some() {
            Some(self.broker.entry_budget()?)
        } else {
            None
        };
        let mut q = if let Some(cap) = margin_cap {
            let mark = self
                .broker
                .marks
                .get(&symbol)
                .copied()
                .ok_or_else(|| failure("research_mark", &symbol))?;
            self.broker.budget_quantity(
                &symbol,
                self.broker.quote(&symbol, mark, side == "long")?,
                cap,
                None,
            )?
        } else if entry || n.get("closeMode")? == "quantity" {
            let Some(q) = self.value(n, "quantity", values)? else {
                return Ok("quantity_not_ready".into());
            };
            q
        } else if n.get("closeMode")? == "all" {
            wide(quantity)?
        } else {
            div(
                mul(wide(quantity)?, num(n.get("closePercent")?)?)?,
                whole(100)?,
            )?
        };
        if !entry {
            q = mind(q, wide(quantity)?);
        }
        let i = &self.broker.instruments[&symbol];
        let q = align(q, i.declaration().quantity_step, FinancialRounding::Floor)?;
        if i.quantity(q, false).is_err() {
            return Ok("quantity_below_minimum".into());
        }
        let kind = n.get("orderType")?;
        let (limit, stop) = (
            if ["limit", "stop_limit"].contains(&kind) {
                let Some(v) = self.value(n, "limitPrice", values)? else {
                    return Ok("limit_not_ready".into());
                };
                Some(align(
                    v,
                    i.declaration().price_step,
                    FinancialRounding::Exact,
                )?)
            } else {
                None
            },
            if ["stop", "stop_limit"].contains(&kind) {
                let Some(v) = self.value(n, "stopPrice", values)? else {
                    return Ok("stop_not_ready".into());
                };
                Some(align(
                    v,
                    i.declaration().price_step,
                    FinancialRounding::Exact,
                )?)
            } else {
                None
            },
        );
        let cap = if let Some(cap) = margin_cap {
            cap
        } else if entry {
            align(
                num(n.get("cashCap")?)?,
                i.declaration().money_step,
                FinancialRounding::Exact,
            )?
        } else {
            parse("0", i.declaration().money_step.scale())?
        };
        let atr = if entry {
            self.value(n, "atr", values)?
        } else {
            None
        };
        if entry
            && ["stopLoss", "takeProfit", "trailing"]
                .iter()
                .any(|k| n.settings.get(&format!("{k}Mode")).map(String::as_str) == Some("atr"))
            && atr.is_none()
        {
            return Ok("atr_not_ready".into());
        }
        let id = format!("research.order.{}", self.broker.orders.len() + 1);
        self.broker.submit(
            Order {
                id,
                node: n.id.clone(),
                symbol,
                side: side.into(),
                entry,
                kind: kind.into(),
                quantity: q,
                remaining: q,
                limit,
                stop,
                triggered: stop.is_none(),
                tif: n.get("timeInForce")?.into(),
                ttl: n
                    .get("validBars")?
                    .parse()
                    .map_err(|_| failure("research_ttl", &n.id))?,
                age: 0,
                tag: n.get("orderTag")?.into(),
                status: "accepted".into(),
                cap,
                held: parse("0", i.declaration().money_step.scale())?,
                hold: None,
                protection: entry.then(|| Protection {
                    node: n.clone(),
                    atr,
                    stop: None,
                }),
                submitted: time,
            },
            time,
        )?;
        Ok(self.broker.orders.last().expect("submitted").status.clone())
    }
    fn cancel_entries(&mut self, symbol: &str, side: &str, reason: &str, time: u64) -> Result<()> {
        for i in 0..self.broker.orders.len() {
            let order = &self.broker.orders[i];
            if Broker::active(order) && order.entry && order.symbol == symbol && order.side == side
            {
                self.broker.cancel(i, reason, time)?;
            }
        }
        Ok(())
    }
    fn protect(&mut self, values: &BTreeMap<(String, String), Slot>, time: u64) -> Result<()> {
        for key in self.broker.legs.keys().cloned().collect::<Vec<_>>() {
            let leg = &self.broker.legs[&key];
            let Some(mut protection) = leg.protection.clone() else {
                continue;
            };
            let quantity = absd(leg.ledger.view().quantity)?;
            if quantity.coefficient() == 0 || self.broker.pending(&key.0, &key.1, false) {
                continue;
            }
            let price = wide(
                self.broker
                    .marks
                    .get(&key.0)
                    .copied()
                    .ok_or_else(|| failure("research_mark", &key.0))?,
            )?;
            let average = div(wide(absd(leg.ledger.view().cost)?)?, wide(quantity)?)?;
            let long = key.1 == "long";
            let node = &protection.node;
            let mut hit = false;
            for k in ["stopLoss", "takeProfit", "trailing"] {
                let mode = node.get(&format!("{k}Mode"))?;
                if mode == "none" {
                    continue;
                }
                let amount = num(node.get(&format!("{k}Value"))?)?;
                let base = if k == "trailing" {
                    if long {
                        leg.high.unwrap_or(average)
                    } else {
                        leg.low.unwrap_or(average)
                    }
                } else {
                    average
                };
                let distance = match mode {
                    "price" if k != "trailing" => None,
                    "price" => Some(amount),
                    "percent" => Some(div(mul(base, amount)?, whole(100)?)?),
                    "atr" => {
                        let atr = if k == "trailing" {
                            self.value(node, "atr", values)?
                        } else {
                            protection.atr
                        };
                        atr.map(|a| mul(a, amount)).transpose()?
                    }
                    _ => return Err(failure("research_protection_mode", &node.id)),
                };
                let target = if mode == "price" && k != "trailing" {
                    amount
                } else if let Some(d) = distance {
                    if (long && k == "takeProfit") || (!long && k != "takeProfit") {
                        add(base, d)?
                    } else {
                        sub(base, d)?
                    }
                } else {
                    continue;
                };
                let target = if k == "trailing" {
                    let t = protection.stop.map_or(target, |old| {
                        if long {
                            maxd(old, target)
                        } else {
                            mind(old, target)
                        }
                    });
                    protection.stop = Some(t);
                    t
                } else {
                    target
                };
                hit |= if (long && k == "takeProfit") || (!long && k != "takeProfit") {
                    price.coefficient() >= target.coefficient()
                } else {
                    price.coefficient() <= target.coefficient()
                };
            }
            self.broker.legs.get_mut(&key).expect("leg").protection = Some(protection.clone());
            if hit {
                self.cancel_entries(&key.0, &key.1, "cancelled_protection", time)?;
                let i = &self.broker.instruments[&key.0];
                let zero = parse("0", i.declaration().money_step.scale())?;
                let id = format!("research.order.{}", self.broker.orders.len() + 1);
                self.broker.submit(
                    Order {
                        id,
                        node: format!("protect.{}", node.id),
                        symbol: key.0.clone(),
                        side: key.1.clone(),
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
                        tag: "protective".into(),
                        status: "accepted".into(),
                        cap: zero,
                        held: zero,
                        hold: None,
                        protection: None,
                        submitted: time,
                    },
                    time,
                )?;
            }
        }
        Ok(())
    }
}
fn execution_stream(streams: &[Stream], symbol: &str, main: usize) -> Result<usize> {
    streams
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.selection
                .dataset()
                .spec()
                .instrument()
                .declaration()
                .instrument
                .as_str()
                == symbol
        })
        .min_by_key(|(i, s)| (s.selection.dataset().spec().duration_ns(), *i != main, *i))
        .map(|(i, _)| i)
        .ok_or_else(|| failure("research_execution_stream", symbol))
}

fn metrics(initial: Decimal, equity: Decimal, fees: Decimal, curve: &[V]) -> Result<V> {
    let mut peak = wide(initial)?;
    let mut dd = whole(0)?;
    let mut dd_ratio = whole(0)?;
    for p in curve {
        let v = num(strv(p, "equity")?)?;
        peak = maxd(peak, v);
        let drop = sub(peak, v)?;
        dd = maxd(dd, drop);
        if peak.coefficient() > 0 {
            dd_ratio = maxd(dd_ratio, div(drop, peak)?);
        }
    }
    let profit = sub(equity, initial)?;
    Ok(o([
        ("initial_equity", ds(initial)),
        ("final_equity", ds(equity)),
        ("net_profit", ds(profit)),
        ("fees", ds(fees)),
        (
            "total_return",
            if initial.coefficient() > 0 {
                ds(div(profit, initial)?)
            } else {
                V::Null
            },
        ),
        ("max_drawdown", ds(dd)),
        ("max_drawdown_ratio", ds(dd_ratio)),
    ]))
}
pub fn prepare(input: &V) -> Result<Prepared> {
    Prepared::new(input)
}
pub fn capabilities() -> V {
    o([
        ("profile", s(PROFILE)),
        ("linear_margin", V::Bool(true)),
        ("margin_profile", s(orders::MARGIN_PROFILE)),
        ("max_leverage", s(1000)),
        ("artifact_kind", s("research")),
        ("max_source_bars", s(MAX_BARS)),
        ("max_orders", s(orders::MAX_RESEARCH_ORDERS)),
        ("max_streams", s(16)),
        (
            "short_accounting",
            s("fully-collateralized-proceeds-restricted"),
        ),
        ("fill_model", s("opening-only;prior-closed-volume")),
        ("optimization", V::Bool(true)),
        ("max_trials", s(100)),
        ("optimization_source_budget", s(200000)),
    ])
}
