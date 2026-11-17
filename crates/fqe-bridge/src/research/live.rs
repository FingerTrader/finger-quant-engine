//! External-account rule evaluation. No broker, fills, ledger or network access.
use super::*;

#[derive(Default)]
pub struct Signals {
    next: u64,
    sessions: BTreeMap<String, Session>,
}
struct Source {
    kind: String,
    symbol: String,
    timeframe: String,
    max_age: u64,
}
struct Session {
    graph: Graph,
    model: V,
    studies: BTreeMap<String, V>,
    last: u64,
    clock: u64,
    conditions: BTreeMap<String, bool>,
    bindings: BTreeMap<String, Source>,
    sources: BTreeMap<String, (u64, BTreeMap<String, Decimal>)>,
    chart_sample: V,
}
impl Signals {
    pub fn execute(&mut self, method: &str, p: &V) -> Result<V> {
        match method {
            "signals.open" => {
                exact(p, &["model"])?;
                if !self.sessions.is_empty() {
                    return Err(failure("live_busy", "$"));
                }
                let graph = Graph::compile(&p["model"])?;
                if graph.context.get("trigger").map(String::as_str) != Some("bar_close") {
                    return Err(failure("live_closed_bars_only", "$"));
                }
                let mut bindings: BTreeMap<String, Source> = BTreeMap::new();
                for n in graph
                    .nodes
                    .iter()
                    .filter(|n| n.kind == "bars" || n.kind == "quotes")
                {
                    let id = n.get("dataset")?;
                    if id.is_empty() {
                        return Err(failure("live_source_binding", &n.id));
                    }
                    let source = Source {
                        kind: n.kind.clone(),
                        symbol: inherit(n, "symbol", &graph.context)?,
                        timeframe: inherit(n, "timeframe", &graph.context)?,
                        max_age: n
                            .get("maxAgeSeconds")?
                            .parse::<u64>()
                            .map_err(|_| failure("research_age", &n.id))?
                            .checked_mul(1_000_000_000)
                            .ok_or_else(|| failure("research_age", &n.id))?,
                    };
                    if let Some(old) = bindings.get(id) {
                        if old.kind != source.kind
                            || old.symbol != source.symbol
                            || old.timeframe != source.timeframe
                        {
                            return Err(failure("live_source_binding", &n.id));
                        }
                        let old = bindings.get_mut(id).expect("source");
                        old.max_age = old.max_age.min(source.max_age);
                    } else {
                        bindings.insert(id.into(), source);
                    }
                }
                if bindings.is_empty() || bindings.len() > 16 {
                    return Err(failure("live_source_quota", "$"));
                }
                self.next += 1;
                let handle = format!("live.{}", self.next);
                self.sessions.insert(
                    handle.clone(),
                    Session {
                        graph,
                        model: p["model"].clone(),
                        studies: BTreeMap::new(),
                        last: 0,
                        clock: 0,
                        conditions: BTreeMap::new(),
                        chart_sample: V::Null,
                        bindings,
                        sources: BTreeMap::new(),
                    },
                );
                Ok(o([
                    ("handle", s(handle)),
                    (
                        "indicator_profile",
                        s(p["model"]["context"]
                            .get("indicatorProfile")
                            .and_then(V::as_str)
                            .unwrap_or("reference/1")),
                    ),
                ]))
            }
            "signals.indicator" => {
                exact(p, &["handle", "study"])?;
                self.sessions
                    .get_mut(strv(p, "handle")?)
                    .ok_or_else(|| failure("live_handle", "$"))?
                    .indicator(&p["study"])
            }
            "signals.close" => {
                exact(p, &["handle"])?;
                self.sessions.remove(strv(p, "handle")?);
                Ok(o([("closed", V::Bool(true))]))
            }
            "signals.advance" | "signals.advance_multi" => {
                let handle = strv(p, "handle")?;
                // Any failed mutation invalidates the session: no ambiguous retry.
                let mut session = self
                    .sessions
                    .remove(handle)
                    .ok_or_else(|| failure("live_handle", "$"))?;
                let result = if method == "signals.advance" {
                    session.advance(p)
                } else {
                    session.advance_multi(p)
                };
                if result.is_ok() {
                    self.sessions.insert(handle.into(), session);
                }
                result
            }
            _ => Err(failure("live_method", "$")),
        }
    }
}
fn slots(v: &V, time: u64) -> Result<BTreeMap<String, Slot>> {
    v.as_object()
        .ok_or_else(|| failure("live_snapshot", "$"))?
        .iter()
        .map(|(k, v)| {
            let value = if matches!(v, V::Null) {
                None
            } else {
                Some(Atom::Number(parse(
                    v.as_str().ok_or_else(|| failure("live_decimal", k))?,
                    SCALE,
                )?))
            };
            Ok((k.clone(), Slot { value, stamp: time }))
        })
        .collect()
}
fn timestamp(v: &V, key: &str) -> Result<u64> {
    strv(v, key)?
        .parse::<u64>()
        .map_err(|_| failure("live_time", key))
}
fn fields(v: &V, quote: bool) -> Result<BTreeMap<String, Decimal>> {
    let keys: &[&str] = if quote {
        &["close", "ask", "bid"]
    } else {
        &["open", "high", "low", "close", "volume"]
    };
    exact(v, keys)?;
    let mut values = keys
        .iter()
        .map(|k| Ok(((*k).into(), parse(strv(v, k)?, SCALE)?)))
        .collect::<Result<BTreeMap<String, Decimal>>>()?;
    if quote {
        if values["close"].coefficient() <= 0
            || values["bid"].coefficient() <= 0
            || values["ask"].coefficient() < values["bid"].coefficient()
        {
            return Err(failure("live_quote", "$"));
        }
        values.insert(
            "mid".into(),
            div(add(values["ask"], values["bid"])?, whole(2)?)?,
        );
    } else if values["low"].coefficient() > values["open"].coefficient()
        || values["low"].coefficient() > values["close"].coefficient()
        || values["high"].coefficient() < values["open"].coefficient()
        || values["high"].coefficient() < values["close"].coefficient()
        || values["low"].coefficient() <= 0
        || values["volume"].coefficient() < 0
    {
        return Err(failure("live_ohlcv", "$"));
    }
    Ok(values)
}
impl Session {
    fn indicator(&mut self, study: &V) -> Result<V> {
        exact(study, &["id", "kind", "parameters"])?;
        let id = strv(study, "id")?;
        fqe_ir::model::identifier(id, "$.study.id")?;
        if let Some(previous) = self.studies.get(id) {
            if previous != study {
                return Err(failure("live_indicator_conflict", id));
            }
        } else {
            if self.clock != 0 || self.studies.len() >= 32 {
                return Err(failure("live_declare_indicators_on_start", id));
            }
            // Only an account-free source graph may be extended by a hosted
            // callback. Authored graph models are immutable and never replaced.
            if self
                .graph
                .nodes
                .iter()
                .any(|n| n.kind != "bars" && !self.studies.contains_key(&n.id))
            {
                return Err(failure("live_indicator_graph_immutable", id));
            }
            if self.graph.nodes.iter().any(|n| n.id == id) {
                return Err(failure("live_indicator_id", id));
            }
            let catalog = studies::describe(&o([] as [(&str, V); 0]))?;
            let definition = catalog["definitions"]
                .as_array()
                .expect("catalog")
                .iter()
                .find(|d| d["kind"] == study["kind"])
                .ok_or_else(|| failure("live_indicator_kind", id))?;
            let mut settings = definition["parameters"]
                .as_object()
                .expect("parameters")
                .clone();
            for (key, value) in study["parameters"]
                .as_object()
                .ok_or_else(|| failure("live_indicator_parameters", id))?
            {
                if !settings.contains_key(key) || !value.is_string() {
                    return Err(failure("live_indicator_parameter", key));
                }
                settings.insert(key.clone(), value.clone());
            }
            let mut model = self.model.clone();
            let source = self
                .graph
                .nodes
                .iter()
                .find(|n| n.kind == "bars")
                .ok_or_else(|| failure("live_indicator_source", id))?;
            model["nodes"].as_array_mut().expect("nodes").push(o([
                ("id", s(id)),
                ("kind", study["kind"].clone()),
                ("label", s(id)),
                ("settings", V::Object(settings)),
                ("literals", o([] as [(&str, V); 0])),
            ]));
            model["wires"].as_array_mut().expect("wires").push(o([
                ("from", s(&source.id)),
                ("output", s("close")),
                ("to", s(id)),
                ("input", s("price")),
            ]));
            let graph = Graph::compile(&model)?;
            self.graph = graph;
            self.model = model;
            self.studies.insert(id.into(), study.clone());
        }
        let columns = self.graph.chart_columns();
        let values = self.chart_sample["values"].as_array();
        Ok(o(columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c["node"] == id)
            .map(|(i, c)| {
                (
                    c["port"].as_str().expect("port"),
                    values
                        .and_then(|row| row.get(i))
                        .cloned()
                        .unwrap_or(V::Null),
                )
            })))
    }

    fn advance(&mut self, p: &V) -> Result<V> {
        exact(p, &["handle", "bars", "account", "positions", "decision"])?;
        let decision = p["decision"]
            .as_bool()
            .ok_or_else(|| failure("live_decision", "$"))?;
        let bars = arr(p, "bars")?;
        if bars.is_empty() || bars.len() > 1000 || (decision && bars.len() != 1) {
            return Err(failure("live_bar_quota", "$"));
        }
        if self.bindings.len() != 1 || self.bindings.values().next().expect("source").kind != "bars"
        {
            return Err(failure("live_multisource_required", "$"));
        }
        let id = self.bindings.keys().next().expect("source").clone();
        let symbol = self.bindings[&id].symbol.clone();
        let mut all = vec![];
        let mut samples = vec![];
        for bar in bars {
            exact(bar, &["time_ns", "open", "high", "low", "close", "volume"])?;
            let time = timestamp(bar, "time_ns")?;
            if time <= self.last {
                return Err(failure("live_bar_order", "$"));
            }
            let mut object = bar.as_object().expect("exact").clone();
            object.remove("time_ns");
            let values = fields(&V::Object(object), false)?;
            let prices = BTreeMap::from([(symbol.clone(), values["close"])]);
            self.sources.insert(id.clone(), (time, values));
            all.extend(self.evaluate(p, time, decision, &prices)?);
            samples.push(self.chart_sample.clone());
            self.last = time;
        }
        Ok(o([
            ("last_ns", s(self.last)),
            ("intents", V::Array(all)),
            ("chart_columns", V::Array(self.graph.chart_columns())),
            ("chart_sample", self.chart_sample.clone()),
            ("chart_samples", V::Array(samples)),
        ]))
    }
    fn advance_multi(&mut self, p: &V) -> Result<V> {
        exact(
            p,
            &[
                "handle",
                "time_ns",
                "events",
                "account",
                "positions",
                "decision",
            ],
        )?;
        let decision = p["decision"]
            .as_bool()
            .ok_or_else(|| failure("live_decision", "$"))?;
        let time = timestamp(p, "time_ns")?;
        if time <= self.last {
            return Err(failure("live_event_order", "$"));
        }
        let events = arr(p, "events")?;
        if events.is_empty() || events.len() > 16 {
            return Err(failure("live_event_quota", "$"));
        }
        let mut changed = BTreeSet::new();
        let mut prices = BTreeMap::new();
        for event in events {
            exact(event, &["stream", "sample_ns", "values"])?;
            let id = strv(event, "stream")?;
            let binding = self
                .bindings
                .get(id)
                .ok_or_else(|| failure("live_source_binding", id))?;
            let stamp = timestamp(event, "sample_ns")?;
            if stamp == 0
                || stamp > time
                || !changed.insert(id.to_owned())
                || self.sources.get(id).is_some_and(|(last, _)| stamp <= *last)
            {
                return Err(failure("live_source_order", id));
            }
            if decision && time - stamp > binding.max_age {
                return Err(failure("live_stale_source", id));
            }
            let values = fields(&event["values"], binding.kind == "quotes")?;
            // Same symbol may carry several periods at this clock. The freshest
            // sample supplies protection prices; stream order must not decide it.
            let candidate = (stamp, binding.kind == "quotes", values["close"]);
            let old = prices.entry(binding.symbol.clone()).or_insert(candidate);
            if (candidate.0, candidate.1) > (old.0, old.1) {
                *old = candidate;
            }
            self.sources.insert(id.into(), (stamp, values));
        }
        let prices = prices.into_iter().map(|(k, (_, _, v))| (k, v)).collect();
        let intents = self.evaluate(p, time, decision, &prices)?;
        self.last = time;
        Ok(o([
            ("last_ns", s(time)),
            ("intents", V::Array(intents)),
            ("chart_columns", V::Array(self.graph.chart_columns())),
            ("chart_sample", self.chart_sample.clone()),
        ]))
    }
    fn evaluate(
        &mut self,
        p: &V,
        time: u64,
        decision: bool,
        prices: &BTreeMap<String, Decimal>,
    ) -> Result<Vec<V>> {
        let mut intents = vec![];
        let account = slots(&p["account"], time)?;
        let mut positions = BTreeMap::new();
        let mut protected = BTreeSet::new();
        for leg in arr(p, "positions")? {
            exact(leg, &["symbol", "side", "values", "entry_node"])?;
            if decision
                && prices.contains_key(strv(leg, "symbol")?)
                && !strv(leg, "entry_node")?.is_empty()
            {
                let symbol = strv(leg, "symbol")?;
                let side = strv(leg, "side")?;
                let node = self
                    .graph
                    .nodes
                    .iter()
                    .find(|n| n.id == strv(leg, "entry_node").unwrap_or("") && n.kind == "entry")
                    .ok_or_else(|| failure("live_entry_node", "$"))?;
                if inherit(node, "symbol", &self.graph.context)? != symbol
                    || node.get("side")? != side
                {
                    return Err(failure("live_protection_binding", "$"));
                }
                let quantity = num(strv(&leg["values"], "quantity")?)?;
                if quantity.coefficient() > 0 {
                    let average = num(strv(&leg["values"], "averagePrice")?)?;
                    let long = side == "long";
                    for kind in ["stopLoss", "takeProfit"] {
                        let mode = node.get(&format!("{kind}Mode"))?;
                        if mode == "none" {
                            continue;
                        }
                        let amount = num(node.get(&format!("{kind}Value"))?)?;
                        let rising =
                            (long && kind == "takeProfit") || (!long && kind == "stopLoss");
                        let target = match mode {
                            "price" => amount,
                            "percent" => {
                                let distance = div(mul(average, amount)?, whole(100)?)?;
                                if rising {
                                    add(average, distance)?
                                } else {
                                    sub(average, distance)?
                                }
                            }
                            _ => return Err(failure("live_protection_mode", &node.id)),
                        };
                        let price = prices[symbol];
                        let hit = if rising {
                            price.coefficient() >= target.coefficient()
                        } else {
                            price.coefficient() <= target.coefficient()
                        };
                        if hit {
                            protected.insert((symbol.to_owned(), side.to_owned()));
                            intents.push(o([
                                ("node", s(format!("{}.{}", node.id, kind))),
                                ("kind", s("exit")),
                                (
                                    "settings",
                                    o([
                                        ("symbol", s(symbol)),
                                        ("side", s(side)),
                                        ("closeMode", s("all")),
                                    ]),
                                ),
                                ("inputs", o([] as [(&str, V); 0])),
                                ("time_ns", s(time)),
                            ]));
                            break;
                        }
                    }
                }
            }
            positions.insert(
                (strv(leg, "symbol")?.into(), strv(leg, "side")?.into()),
                slots(&leg["values"], time)?,
            );
        }
        self.clock = self
            .clock
            .checked_add(1)
            .ok_or_else(|| failure("live_clock", "$"))?;
        let values =
            self.graph
                .evaluate(&self.sources, &account, &positions, time, true, self.clock)?;
        let columns = self.graph.chart_columns();
        self.chart_sample = o([
            ("timestamp_ns", s(time)),
            ("source_sequence", s(self.clock)),
            (
                "values",
                V::Array(
                    columns
                        .iter()
                        .map(|c| {
                            values
                                .get(&(
                                    c["node"].as_str().expect("node").into(),
                                    c["port"].as_str().expect("port").into(),
                                ))
                                .and_then(|v| v.value.as_ref())
                                .map_or(V::Null, Atom::json)
                        })
                        .collect(),
                ),
            ),
        ]);
        for node in self
            .graph
            .nodes
            .iter()
            .filter(|n| ["entry", "exit", "cancelOrders"].contains(&n.kind.as_str()))
        {
            let ready = matches!(
                self.graph.input(node, "condition", &values)?.value,
                Some(Atom::Boolean(true))
            );
            let previous = self
                .conditions
                .insert(node.id.clone(), ready)
                .unwrap_or(false);
            if decision
                && ready
                && (node.get("triggerMode")? != "rising_edge" || !previous)
                && !protected.contains(&(
                    inherit(node, "symbol", &self.graph.context)?,
                    node.get("side")?.to_owned(),
                ))
            {
                let mut settings = node.settings.clone();
                settings.insert(
                    "symbol".into(),
                    inherit(node, "symbol", &self.graph.context)?,
                );
                let mut inputs = BTreeMap::new();
                for port in node.inputs.keys() {
                    let slot = self.graph.input(node, port, &values)?;
                    inputs.insert(port.clone(), slot.value.map_or(V::Null, |v| v.json()));
                }
                intents.push(o([
                    ("node", s(&node.id)),
                    ("kind", s(&node.kind)),
                    (
                        "settings",
                        V::Object(settings.into_iter().map(|(k, v)| (k, s(v))).collect()),
                    ),
                    ("inputs", V::Object(inputs.into_iter().collect())),
                    ("time_ns", s(time)),
                ]));
            }
        }

        Ok(intents)
    }
}
