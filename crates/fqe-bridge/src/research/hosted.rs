//! External strategy callbacks share the research broker. No Python/network here.
use super::*;

pub const CAPABILITY: &str = "hosted.execution/1";
#[derive(Default)]
pub struct Hosted {
    serial: u64,
    sessions: BTreeMap<String, Session>,
}
struct Candidate {
    token: String,
    broker: Broker,
    cursor: usize,
    time: u64,
    warmup: bool,
    closed: bool,
    market: bool,
    result: V,
}
struct Session {
    prepared: Prepared,
    model: V,
    cursor: usize,
    revision: u64,
    time: u64,
    warmup: bool,
    closed: bool,
    callback: bool,
    poisoned: bool,
    pending: Option<Candidate>,
    values: BTreeMap<(String, String), Slot>,
    studies: BTreeMap<String, V>,
    study_ports: BTreeMap<String, Vec<String>>,
    clients: BTreeSet<String>,
    frames: Vec<V>,
    curve: Vec<V>,
    rows: Vec<V>,
    fill_cursor: usize,
    event_cursor: usize,
}
impl Hosted {
    pub fn execute(&mut self, method: &str, p: &V) -> Result<V> {
        if method == "hosted.open" {
            exact(p, &["input"])?;
            if !self.sessions.is_empty() {
                return Err(failure("hosted_busy", "$"));
            }
            let mut prepared = Prepared::new(&p["input"])?;
            if prepared.streams.len() != 1 {
                return Err(failure("hosted_single_stream_required", "$"));
            }
            let mut model = json::decode_input(strv(&p["input"], "model")?.as_bytes())?;
            model["context"]["indicatorProfile"] = s(super::professional::PROFILE);
            // Discard graph decisions: Python owns decisions, FQE owns math/ledger.
            model["nodes"] = V::Array(
                model["nodes"]
                    .as_array()
                    .expect("nodes")
                    .iter()
                    .filter(|n| n["kind"] == "bars")
                    .cloned()
                    .collect(),
            );
            model["wires"] = V::Array(vec![]);
            model["parameters"] = V::Array(vec![]);
            model["functions"] = V::Array(vec![]);
            prepared.graph = Graph::compile(&model)?;
            let time = prepared.streams[0].selection.bars()[0].input().open_ns;
            let initial = o([
                ("phase", s("initial")),
                ("timestamp_ns", s(time)),
                ("source_sequence", s(0)),
                ("account", prepared.broker.snapshot()?),
                ("receipts", V::Array(vec![])),
            ]);
            self.serial += 1;
            let handle = format!("hosted.{}", self.serial);
            self.sessions.insert(
                handle.clone(),
                Session {
                    prepared,
                    model,
                    cursor: 0,
                    revision: 0,
                    time,
                    warmup: true,
                    closed: false,
                    callback: false,
                    poisoned: false,
                    pending: None,
                    values: BTreeMap::new(),
                    studies: BTreeMap::new(),
                    study_ports: BTreeMap::new(),
                    clients: BTreeSet::new(),
                    frames: vec![initial],
                    curve: vec![],
                    rows: vec![],
                    fill_cursor: 0,
                    event_cursor: 0,
                },
            );
            return Ok(o([("handle", s(handle)), ("capability", s(CAPABILITY))]));
        }
        let handle = strv(p, "handle")?;
        if method == "hosted.close" {
            exact(p, &["handle"])?;
            self.sessions
                .remove(handle)
                .ok_or_else(|| failure("hosted_unavailable", handle))?;
            return Ok(o([("closed", V::Bool(true))]));
        }
        let session = self
            .sessions
            .get_mut(handle)
            .ok_or_else(|| failure("hosted_unavailable", handle))?;
        if session.poisoned {
            return Err(failure("hosted_poisoned", handle));
        }
        match method {
            "hosted.prepare" => {
                exact(p, &["handle", "operation"])?;
                session.prepare(&p["operation"])
            }
            "hosted.commit" => {
                exact(p, &["handle", "token"])?;
                session.commit(strv(p, "token")?)
            }
            "hosted.complete" => {
                exact(p, &["handle", "revision"])?;
                session.complete(strv(p, "revision")?)
            }
            "hosted.indicator" => {
                exact(p, &["handle", "study"])?;
                session.indicator(&p["study"])
            }
            "hosted.candidate_order" => {
                exact(p, &["handle", "order_id"])?;
                let mut candidate = session
                    .pending
                    .take()
                    .ok_or_else(|| failure("hosted_candidate_required", "$"))?;
                std::mem::swap(&mut session.prepared.broker, &mut candidate.broker);
                let result = session.read("order", strv(p, "order_id")?);
                std::mem::swap(&mut session.prepared.broker, &mut candidate.broker);
                session.pending = Some(candidate);
                result
            }
            "hosted.read" => {
                exact(p, &["handle", "kind", "order_id"])?;
                session.available()?;
                session.read(strv(p, "kind")?, strv(p, "order_id")?)
            }
            "hosted.snapshot" => {
                exact(p, &["handle"])?;
                session.available()?;
                session.snapshot()
            }
            "hosted.report" => {
                exact(p, &["handle"])?;
                session.available()?;
                session.report()
            }
            _ => Err(failure("hosted_method", method)),
        }
    }
}
impl Session {
    fn available(&self) -> Result<()> {
        if self.pending.is_some() {
            Err(failure("hosted_commit_required", "$"))
        } else {
            Ok(())
        }
    }
    fn snapshot(&self) -> Result<V> {
        Ok(o([
            ("revision", s(self.revision)),
            ("processed", s(self.cursor)),
            ("total", s(self.prepared.streams[0].selection.bars().len())),
            ("timestamp_ns", s(self.time)),
            ("warmup", V::Bool(self.warmup)),
            ("closed", V::Bool(self.closed)),
            ("callback_pending", V::Bool(self.callback)),
            ("account", self.prepared.broker.snapshot()?),
        ]))
    }
    fn read(&self, kind: &str, id: &str) -> Result<V> {
        let b = &self.prepared.broker;
        let zero = whole(0)?;
        let symbol = self.prepared.streams[0]
            .selection
            .dataset()
            .spec()
            .instrument()
            .declaration()
            .instrument
            .as_str();
        let views: Vec<_> = b
            .legs
            .iter()
            .filter(|((s, side), _)| s == symbol && side == "long")
            .map(|(_, l)| l.ledger.view())
            .collect();
        let quantity = sum(views.iter().map(|v| v.quantity))?;
        let cost = sum(views.iter().map(|v| v.cost))?;
        let reserved_quantity = sum(views.iter().map(|v| v.reserved_quantity))?;
        let gross = sum(b.legs.values().map(|l| l.ledger.view().realized_gross))?;
        match kind {
            "account" => {
                let hub = b.hub.view();
                let cash = if b.margin.is_some() {
                    b.settled_balance()?
                } else {
                    hub.cash
                };
                let available = if b.margin.is_some() {
                    b.free_margin()?
                } else {
                    hub.available_cash
                };
                Ok(o([
                    ("cash", ds(cash)),
                    ("reserved_cash", ds(sub(cash, available)?)),
                    ("available_cash", ds(available)),
                    ("position_quantity", ds(quantity)),
                    ("position_cost", ds(cost)),
                    ("realized_pnl", ds(sub(gross, b.fees)?)),
                ]))
            }
            "position" => Ok(o([
                ("instrument_id", s(symbol)),
                ("quantity", ds(quantity)),
                ("available_quantity", ds(sub(quantity, reserved_quantity)?)),
                ("reserved_quantity", ds(reserved_quantity)),
                ("cost", ds(cost)),
            ])),
            "order" => {
                let order = b
                    .orders
                    .iter()
                    .find(|o| o.id == id)
                    .ok_or_else(|| failure("hosted_order_not_found", id))?;
                let fills: Vec<_> = b.fills.iter().filter(|f| f["order"] == id).collect();
                let fee = sum(fills
                    .iter()
                    .map(|f| num(f["fee"].as_str().expect("fee")))
                    .collect::<Result<Vec<_>>>()?)?;
                let status = if order.status == "filled" {
                    "filled"
                } else if order.status.starts_with("rejected") {
                    "rejected"
                } else if Broker::active(order) {
                    "accepted"
                } else if order.status == "cancelled_user" {
                    "cancelled"
                } else {
                    "ended"
                };
                let mut result = o([
                    ("order_id", s(&order.id)),
                    ("client_order_id", s(&order.tag)),
                    ("side", s(if order.entry { "buy" } else { "sell" })),
                    ("kind", s(&order.kind)),
                    ("quantity", ds(order.quantity)),
                    ("status", s(status)),
                    ("reason", s(&order.status)),
                    (
                        "run_id",
                        s(self.prepared.manifest["run"].as_str().unwrap_or("python")),
                    ),
                    ("filled_quantity", ds(sub(order.quantity, order.remaining)?)),
                    ("fee", ds(fee)),
                    ("reserved_cash", ds(order.held)),
                    (
                        "reserved_quantity",
                        ds(if !order.entry && Broker::active(order) {
                            order.remaining
                        } else {
                            zero
                        }),
                    ),
                ]);
                if let Some(limit) = order.limit {
                    result["limit_price"] = ds(limit);
                }
                // Aggregate fill price is not fabricated for partial fills.
                if fills.len() == 1 {
                    result["fill_price"] = fills[0]["price"].clone();
                }
                Ok(result)
            }
            _ => Err(failure("hosted_read_kind", kind)),
        }
    }
    fn indicator(&mut self, study: &V) -> Result<V> {
        self.available()?;
        exact(study, &["id", "kind", "parameters"])?;
        let id = strv(study, "id")?;
        fqe_ir::model::identifier(id, "$.study.id")?;
        if let Some(old) = self.studies.get(id) {
            if old != study {
                return Err(failure("hosted_indicator_conflict", id));
            }
        } else {
            if self.cursor != 0 || self.closed || self.studies.len() >= 32 {
                return Err(failure("hosted_declare_indicators_on_start", id));
            }
            let catalog = studies::describe(&o([] as [(&str, V); 0]))?;
            let definition = catalog["definitions"]
                .as_array()
                .expect("catalog")
                .iter()
                .find(|d| d["kind"] == study["kind"])
                .ok_or_else(|| failure("hosted_indicator_kind", id))?;
            let supplied = study["parameters"]
                .as_object()
                .ok_or_else(|| failure("hosted_indicator_parameters", id))?;
            let mut parameters = definition["parameters"]
                .as_object()
                .expect("parameters")
                .clone();
            for (k, v) in supplied {
                if !parameters.contains_key(k) || !v.is_string() {
                    return Err(failure("hosted_indicator_parameter", k));
                }
                parameters.insert(k.clone(), v.clone());
            }
            let mut proposed = self.studies.clone();
            proposed.insert(id.into(), study.clone());
            let mut model = self.model.clone();
            let source = self
                .prepared
                .graph
                .nodes
                .iter()
                .find(|n| n.kind == "bars")
                .ok_or_else(|| failure("hosted_source", "$"))?;
            let source_id = &source.id;
            let mut nodes = vec![o([
                ("id", s(source_id)),
                ("kind", s("bars")),
                ("label", s("Source")),
                (
                    "settings",
                    o(source.settings.iter().map(|(k, v)| (k.clone(), s(v)))),
                ),
                ("literals", o([] as [(&str, V); 0])),
            ])];
            let mut wires = vec![];
            for (key, spec) in &proposed {
                if key == source_id {
                    return Err(failure("hosted_indicator_source_id", key));
                }
                let def = catalog["definitions"]
                    .as_array()
                    .expect("catalog")
                    .iter()
                    .find(|d| d["kind"] == spec["kind"])
                    .expect("registered kind");
                let mut settings = def["parameters"].as_object().expect("parameters").clone();
                for (k, v) in spec["parameters"].as_object().expect("parameters") {
                    settings.insert(k.clone(), v.clone());
                }
                nodes.push(o([
                    ("id", s(key)),
                    ("kind", spec["kind"].clone()),
                    ("label", s(key)),
                    ("settings", V::Object(settings)),
                    ("literals", o([] as [(&str, V); 0])),
                ]));
                wires.push(o([
                    ("from", s(source_id)),
                    ("output", s("close")),
                    ("to", s(key)),
                    ("input", s("price")),
                ]));
            }
            model["nodes"] = V::Array(nodes);
            model["wires"] = V::Array(wires);
            model["parameters"] = V::Array(vec![]);
            model["functions"] = V::Array(vec![]);
            let graph = Graph::compile(&model)?;
            self.prepared.graph = graph;
            self.studies = proposed;
            self.study_ports.insert(
                id.into(),
                definition["outputs"]
                    .as_array()
                    .expect("outputs")
                    .iter()
                    .map(|port| port["port"].as_str().expect("port").to_owned())
                    .collect(),
            );
        }
        let mut outputs = self.study_ports[id]
            .iter()
            .map(|port| (port.clone(), V::Null))
            .collect::<serde_json::Map<String, V>>();
        for ((node, port), value) in &self.values {
            if node == id {
                outputs.insert(
                    port.clone(),
                    value.value.as_ref().map_or(V::Null, Atom::json),
                );
            }
        }
        Ok(V::Object(outputs))
    }

    fn prepare(&mut self, op: &V) -> Result<V> {
        self.available()?;
        if self.closed {
            return Err(failure("hosted_closed", "$"));
        }
        let action = strv(op, "action")?;
        let before = self.prepared.broker.clone();
        let mut cursor = self.cursor;
        let mut time = self.time;
        let mut warmup = self.warmup;
        let mut closed = false;
        let result = (|| -> Result<V> {
            match action {
                "bar" => {
                    exact(op, &["action"])?;
                    if self.callback {
                        return Err(failure("hosted_callback_required", "$"));
                    }
                    let stream = &self.prepared.streams[0];
                    let bar = stream
                        .selection
                        .bars()
                        .get(cursor)
                        .ok_or_else(|| failure("hosted_end_of_data", "$"))?
                        .clone();
                    let symbol = stream
                        .selection
                        .dataset()
                        .spec()
                        .instrument()
                        .declaration()
                        .instrument
                        .as_str()
                        .to_owned();
                    let previous = if cursor == 0 {
                        whole(0)?
                    } else {
                        bar_value(&stream.selection.bars()[cursor - 1].input().volume)?
                    };
                    warmup = cursor < stream.selection.warmup_bars();
                    self.prepared.broker.on_open(&symbol, &bar, previous)?;
                    self.prepared.broker.on_close(&symbol, &bar)?;
                    time = bar.input().close_ns;
                    cursor += 1;
                    let b = bar.input();
                    let sources = BTreeMap::from([(
                        stream.id.clone(),
                        (
                            time,
                            BTreeMap::from([
                                ("open".into(), bar_value(&b.open)?),
                                ("high".into(), bar_value(&b.high)?),
                                ("low".into(), bar_value(&b.low)?),
                                ("close".into(), bar_value(&b.close)?),
                                ("volume".into(), bar_value(&b.volume)?),
                            ]),
                        ),
                    )]);
                    let (account, positions) = self.prepared.broker.graph_views(time)?;
                    self.values = self.prepared.graph.evaluate(
                        &sources,
                        &account,
                        &positions,
                        time,
                        true,
                        cursor as u64,
                    )?;
                    if !warmup {
                        self.prepared.protect(&self.values, time)?;
                    }
                    Ok(o([
                        ("sequence", s(cursor)),
                        ("open_ns", s(b.open_ns)),
                        ("close_ns", s(b.close_ns)),
                        ("warmup", V::Bool(warmup)),
                    ]))
                }
                "submit" => {
                    if !self.callback || self.warmup {
                        return Err(failure("hosted_read_only_phase", "$"));
                    }
                    self.submit(op)
                }
                "cancel" => {
                    exact(op, &["action", "order_id"])?;
                    if !self.callback || self.warmup {
                        return Err(failure("hosted_read_only_phase", "$"));
                    }
                    let id = strv(op, "order_id")?;
                    let index = self
                        .prepared
                        .broker
                        .orders
                        .iter()
                        .position(|o| o.id == id)
                        .ok_or_else(|| failure("hosted_order_not_found", id))?;
                    if Broker::active(&self.prepared.broker.orders[index]) {
                        self.prepared.broker.cancel(index, "cancelled_user", time)?;
                    }
                    Ok(orders::order_json(&self.prepared.broker.orders[index]))
                }
                "finish" => {
                    exact(op, &["action"])?;
                    self.prepared.broker.finish(time)?;
                    closed = true;
                    Ok(o([("closed", V::Bool(true))]))
                }
                _ => Err(failure("hosted_action", action)),
            }
        })();
        let candidate = std::mem::replace(&mut self.prepared.broker, before);
        let result = match result {
            Ok(v) => v,
            Err(e) => {
                if action == "bar" {
                    self.poisoned = true;
                }
                return Err(e);
            }
        };
        let token = format!("candidate.{}", self.revision + 1);
        let proposal = o([
            ("token", s(&token)),
            ("revision", s(self.revision + 1)),
            ("operation", op.clone()),
            ("result", result.clone()),
            ("account", candidate.snapshot()?),
            (
                "events",
                V::Array(candidate.events[self.prepared.broker.events.len()..].to_vec()),
            ),
        ]);
        self.pending = Some(Candidate {
            token,
            broker: candidate,
            cursor,
            time,
            warmup,
            closed,
            market: action == "bar",
            result,
        });
        Ok(proposal)
    }
    fn submit(&mut self, op: &V) -> Result<V> {
        exact(
            op,
            &[
                "action",
                "client_order_id",
                "side",
                "quantity",
                "kind",
                "limit_price",
                "stop_loss_percent",
                "take_profit_percent",
            ],
        )?;
        let client = strv(op, "client_order_id")?;
        fqe_ir::model::identifier(client, "$.client_order_id")?;
        if self.clients.contains(client) {
            return Err(failure("hosted_duplicate_order", client));
        }
        let buy = match strv(op, "side")? {
            "buy" => true,
            "sell" => false,
            _ => return Err(failure("hosted_order_side", "$")),
        };
        let kind = strv(op, "kind")?;
        if !["market", "limit"].contains(&kind) {
            return Err(failure("hosted_order_kind", "$"));
        }
        let symbol = self.prepared.streams[0]
            .selection
            .dataset()
            .spec()
            .instrument()
            .declaration()
            .instrument
            .as_str()
            .to_owned();
        let i = &self.prepared.broker.instruments[&symbol];
        i.quantity(
            align(
                num(strv(op, "quantity")?)?,
                i.declaration().quantity_step,
                FinancialRounding::Exact,
            )?,
            false,
        )?;
        let stop = num(strv(op, "stop_loss_percent")?)?;
        let take = num(strv(op, "take_profit_percent")?)?;
        if stop.coefficient() < 0
            || stop.coefficient() >= whole(100)?.coefficient()
            || take.coefficient() < 0
        {
            return Err(failure("hosted_protection_invalid", "$"));
        }
        let mut settings = BTreeMap::from_iter(
            [
                ("symbol", symbol.as_str()),
                ("side", "long"),
                ("orderType", kind),
                ("limitPrice", "0"),
                ("stopPrice", "0"),
                ("timeInForce", "GTC"),
                ("validBars", "0"),
                ("orderTag", client),
                ("triggerMode", "every"),
                ("quantity", strv(op, "quantity")?),
                (
                    "cashCap",
                    self.prepared.graph.context["initialCash"].as_str(),
                ),
                ("entryMode", "open"),
                ("maxAdds", "10000"),
                ("closeMode", "quantity"),
                ("closePercent", "100"),
                (
                    "stopLossMode",
                    if stop.coefficient() == 0 {
                        "none"
                    } else {
                        "percent"
                    },
                ),
                ("stopLossValue", strv(op, "stop_loss_percent")?),
                (
                    "takeProfitMode",
                    if take.coefficient() == 0 {
                        "none"
                    } else {
                        "percent"
                    },
                ),
                ("takeProfitValue", strv(op, "take_profit_percent")?),
                ("trailingMode", "none"),
                ("trailingValue", "0"),
            ]
            .map(|(k, v)| (k.to_owned(), v.to_owned())),
        );
        if kind == "limit" {
            let limit = strv(op, "limit_price")?;
            if num(limit)?.coefficient() <= 0 {
                return Err(failure("hosted_limit_price", "$"));
            }
            i.price(align(
                num(limit)?,
                i.declaration().price_step,
                FinancialRounding::Exact,
            )?)?;
            settings.insert("limitPrice".into(), limit.into());
        }
        if buy
            && self
                .prepared
                .broker
                .quantity(&symbol, "long")?
                .coefficient()
                > 0
        {
            settings.insert("entryMode".into(), "add".into());
        }
        // The same action admission/sizing/protection path as graph strategies.
        let node = Node {
            id: format!("python.{}", self.revision),
            kind: if buy { "entry" } else { "exit" }.into(),
            settings,
            literals: BTreeMap::new(),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
        };
        let before = self.prepared.broker.orders.len();
        let status = self.prepared.act_sized(
            &node,
            &BTreeMap::new(),
            self.time,
            Some(num(strv(op, "quantity")?)?),
        )?;
        if self.prepared.broker.orders.len() == before {
            return Err(failure("hosted_order_not_admitted", status));
        }
        Ok(orders::order_json(
            self.prepared.broker.orders.last().expect("submitted"),
        ))
    }
    fn commit(&mut self, token: &str) -> Result<V> {
        if self.pending.as_ref().is_none_or(|p| p.token != token) {
            return Err(failure("hosted_candidate_mismatch", "$"));
        }
        let p = self.pending.take().expect("candidate");
        if let Some(client) = p.result.get("tag").and_then(V::as_str) {
            self.clients.insert(client.into());
        }
        self.prepared.broker = p.broker;
        self.cursor = p.cursor;
        self.time = p.time;
        self.warmup = p.warmup;
        self.closed = p.closed;
        self.revision += 1;
        if p.market {
            self.callback = true;
        }
        Ok(o([("revision", s(self.revision))]))
    }
    fn complete(&mut self, revision: &str) -> Result<V> {
        self.available()?;
        if revision != self.revision.to_string() || !self.callback {
            return Err(failure("hosted_callback_revision", "$"));
        }
        self.callback = false;
        let gross = sum(self
            .prepared
            .broker
            .legs
            .values()
            .map(|l| l.ledger.view().realized_gross))?;
        let realized = sub(gross, self.prepared.broker.fees)?;
        let point = o([
            ("source_sequence", s(self.cursor)),
            ("closed_pnl", ds(gross)),
            ("realized_pnl", ds(realized)),
            (
                "unrealized_pnl",
                ds(sub(
                    sub(self.prepared.broker.equity()?, self.prepared.broker.initial)?,
                    realized,
                )?),
            ),
            ("timestamp_ns", s(self.time)),
            ("equity", ds(self.prepared.broker.equity()?)),
            (
                "cash",
                ds(if self.prepared.broker.margin.is_some() {
                    self.prepared.broker.settled_balance()?
                } else {
                    self.prepared.broker.hub.view().cash
                }),
            ),
            ("fees", ds(self.prepared.broker.fees)),
        ]);
        if !self.warmup {
            self.curve.push(point.clone());
        }
        let columns = self.prepared.graph.chart_columns();
        let chart = o([
            ("source_sequence", s(self.cursor)),
            ("timestamp_ns", s(self.time)),
            (
                "values",
                V::Array(
                    columns
                        .iter()
                        .map(|c| {
                            self.values
                                .get(&(
                                    c["node"].as_str().unwrap_or("").into(),
                                    c["port"].as_str().unwrap_or("").into(),
                                ))
                                .and_then(|v| v.value.as_ref())
                                .map_or(V::Null, Atom::json)
                        })
                        .collect(),
                ),
            ),
        ]);
        self.rows.push(chart.clone());
        let frame = o([
            (
                "phase",
                s(if self.warmup {
                    "warmup_close"
                } else {
                    "decision_close"
                }),
            ),
            ("timestamp_ns", s(self.time)),
            ("source_sequence", s(self.cursor)),
            ("account", self.prepared.broker.snapshot()?),
            ("receipts", V::Array(vec![])),
        ]);
        self.frames.push(frame);
        let mut result = self.snapshot()?;
        result["point"] = point;
        result["chart_columns"] = V::Array(columns);
        result["chart_sample"] = chart;
        result["initial_equity"] = ds(self.prepared.broker.initial);
        result["profile"] = s(self.prepared.broker.profile());
        result["execution_events"] =
            V::Array(self.prepared.broker.events[self.event_cursor..].to_vec());
        result["fills"] = V::Array(self.prepared.broker.fills[self.fill_cursor..].to_vec());
        result["marks"] = o(self
            .prepared
            .broker
            .marks
            .iter()
            .map(|(k, v)| (k.clone(), ds(*v))));
        self.fill_cursor = self.prepared.broker.fills.len();
        self.event_cursor = self.prepared.broker.events.len();
        result["decisions"] = V::Array(vec![]);
        Ok(result)
    }
    fn report(&self) -> Result<V> {
        if !self.closed {
            return Err(failure("hosted_finish_required", "$"));
        }
        Ok(o([
            ("format", s("fqe.backtest-result")),
            ("schema_version", V::from(1)),
            ("profile", s(self.prepared.broker.profile())),
            (
                "terminal",
                s(
                    if self.cursor == self.prepared.streams[0].selection.bars().len() {
                        "completed"
                    } else {
                        "cancelled"
                    },
                ),
            ),
            ("manifest", self.prepared.manifest.clone()),
            ("manifest_id", s(&self.prepared.manifest_id)),
            ("processed_source_bars", s(self.cursor)),
            ("account", self.prepared.broker.snapshot()?),
            (
                "metrics",
                metrics(
                    self.prepared.broker.initial,
                    self.prepared.broker.equity()?,
                    self.prepared.broker.fees,
                    &self.curve,
                )?,
            ),
            ("equity_curve", V::Array(self.curve.clone())),
            (
                "chart_trace",
                o([
                    ("schema_version", V::from(1)),
                    ("columns", V::Array(self.prepared.graph.chart_columns())),
                    ("rows", V::Array(self.rows.clone())),
                ]),
            ),
            ("observations", V::Array(vec![])),
            ("decisions", V::Array(vec![])),
            (
                "execution_events",
                V::Array(self.prepared.broker.events.clone()),
            ),
            (
                "orders",
                V::Array(
                    self.prepared
                        .broker
                        .orders
                        .iter()
                        .map(orders::order_json)
                        .collect(),
                ),
            ),
            ("fills", V::Array(self.prepared.broker.fills.clone())),
            ("journal", self.prepared.broker.journals()),
            (
                "frames",
                V::Array({
                    let mut frames = self.frames.clone();
                    frames.push(o([
                        ("phase", s("terminal")),
                        ("timestamp_ns", s(self.time)),
                        ("source_sequence", s(self.cursor)),
                        ("account", self.prepared.broker.snapshot()?),
                        ("receipts", V::Array(vec![])),
                    ]));
                    frames
                }),
            ),
            (
                "warnings",
                V::Array(vec![s(
                    "experimental hosted Python; native research financial policies; no live routing",
                )]),
            ),
        ]))
    }
}
