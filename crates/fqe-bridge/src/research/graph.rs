//! Bounded native typed graph. No filesystem, external calls or client evaluation.
use super::*;
type PortList = Vec<(String, String)>;
use fqe_core::types::{
    Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
};
use fqe_ir::{
    catalog, event_plan::Trigger, parameters::ParameterInput, registry::IncrementalInstance,
};

#[derive(Clone, Debug)]
pub enum Atom {
    Number(Decimal),
    Boolean(bool),
}
impl Atom {
    pub fn number(&self) -> Result<Decimal> {
        match self {
            Self::Number(n) => Ok(*n),
            _ => Err(failure("research_type", "$")),
        }
    }
    pub fn boolean(&self) -> Result<bool> {
        match self {
            Self::Boolean(n) => Ok(*n),
            _ => Err(failure("research_type", "$")),
        }
    }
    pub fn json(&self) -> V {
        match self {
            Self::Number(n) => ds(*n),
            Self::Boolean(b) => V::Bool(*b),
        }
    }
}
#[derive(Clone, Default)]
pub struct Slot {
    pub value: Option<Atom>,
    pub stamp: u64,
}
impl Slot {
    fn num(n: Decimal, stamp: u64) -> Self {
        Self {
            value: Some(Atom::Number(n)),
            stamp,
        }
    }
    fn boolean(b: bool, stamp: u64) -> Self {
        Self {
            value: Some(Atom::Boolean(b)),
            stamp,
        }
    }
}
#[derive(Clone)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub settings: BTreeMap<String, String>,
    pub literals: BTreeMap<String, String>,
    pub inputs: BTreeMap<String, String>,
    pub outputs: BTreeMap<String, String>,
}
impl Node {
    pub fn get(&self, k: &str) -> Result<&str> {
        self.settings.get(k).map(String::as_str).ok_or_else(|| {
            failure(
                "research_setting",
                format!("$.nodes.{}.settings.{k}", self.id),
            )
        })
    }
}
#[derive(Default)]
struct State {
    last: Option<u64>,
    previous: Option<(Decimal, Decimal)>,
    count: u64,
    active: Option<u64>,
    hit: bool,
    history: VecDeque<Decimal>,
    close: Option<Decimal>,
    smooth: Option<Decimal>,
    delay: Option<Atom>,
    values: BTreeMap<String, Slot>,
    provider: Option<IncrementalInstance>,
}
pub struct Graph {
    pub nodes: Vec<Node>,
    pub context: BTreeMap<String, String>,
    pub functions: BTreeMap<String, V>,
    pub wires: BTreeMap<(String, String), (String, String)>,
    order: Vec<usize>,
    states: Vec<State>,
}
fn map(v: &V) -> Result<BTreeMap<String, String>> {
    v.as_object()
        .ok_or_else(|| failure("research_object", "$"))?
        .iter()
        .map(|(k, v)| {
            Ok((
                k.clone(),
                v.as_str()
                    .ok_or_else(|| failure("research_string", k))?
                    .into(),
            ))
        })
        .collect()
}
fn ports(
    k: &str,
    s: &BTreeMap<String, String>,
    functions: &BTreeMap<String, V>,
) -> Result<(BTreeMap<String, String>, BTreeMap<String, String>)> {
    let typed = || s.get("type").cloned().unwrap_or_else(|| "ratio".into());
    let (inputs, outputs): (PortList, PortList) = match k {
        "bars" => (
            vec![],
            vec![
                ("open".into(), "price".into()),
                ("high".into(), "price".into()),
                ("low".into(), "price".into()),
                ("close".into(), "price".into()),
                ("volume".into(), "quantity".into()),
            ],
        ),
        "constant" | "parameter" => (vec![], vec![("value".into(), typed())]),
        "sma" | "ema" | "rsi" | "highest" | "lowest" | "boll" | "macd" => (
            vec![("price".into(), "price".into())],
            match k {
                "rsi" => vec![("value".into(), "ratio".into())],
                "boll" => ["upper", "middle", "lower"]
                    .map(|p| (p.into(), "price".into()))
                    .to_vec(),
                "macd" => ["line", "signal", "histogram"]
                    .map(|p| (p.into(), "price".into()))
                    .to_vec(),
                _ => vec![("value".into(), "price".into())],
            },
        ),
        "atr" => (
            ["high", "low", "close"]
                .map(|p| (p.into(), "price".into()))
                .to_vec(),
            vec![("value".into(), "price".into())],
        ),
        "compare" | "crossUp" | "crossDown" => (
            ["a", "b"].map(|p| (p.into(), typed())).to_vec(),
            vec![("result".into(), "bool".into())],
        ),
        "add" | "subtract" | "multiply" | "divide" => (
            ["a", "b"].map(|p| (p.into(), typed())).to_vec(),
            vec![("value".into(), typed())],
        ),
        "scale" => (
            vec![("value".into(), typed()), ("factor".into(), "ratio".into())],
            vec![("value".into(), typed())],
        ),
        "and" | "or" => {
            let n = s.get("inputCount").map_or(Ok(2), |n| {
                n.parse::<usize>()
                    .map_err(|_| failure("research_condition_count", "$"))
            })?;
            if !(2..=32).contains(&n) {
                return Err(failure("research_condition_count", "$"));
            }
            (
                (0..n)
                    .map(|i| {
                        (
                            if i == 0 {
                                "a".into()
                            } else if i == 1 {
                                "b".into()
                            } else {
                                format!("condition_{}", i + 1)
                            },
                            "bool".into(),
                        )
                    })
                    .collect(),
                vec![("result".into(), "bool".into())],
            )
        }
        "not" | "consecutive" => (
            vec![("condition".into(), "bool".into())],
            vec![("result".into(), "bool".into())],
        ),
        "wait" => (
            ["arm", "condition", "cancel"]
                .map(|p| (p.into(), "bool".into()))
                .to_vec(),
            vec![("result".into(), "bool".into())],
        ),
        "delay" => (
            vec![("next".into(), typed())],
            vec![("previous".into(), typed())],
        ),
        "account" => (
            vec![],
            ["equity", "cash"]
                .map(|p| (p.into(), "money".into()))
                .to_vec(),
        ),
        "position" => (
            vec![],
            vec![
                ("quantity".into(), "quantity".into()),
                ("averagePrice".into(), "price".into()),
                ("highestPrice".into(), "price".into()),
                ("lowestPrice".into(), "price".into()),
            ],
        ),
        "riskSize" => (
            vec![
                ("equity".into(), "money".into()),
                ("stopDistance".into(), "price".into()),
            ],
            vec![("quantity".into(), "quantity".into())],
        ),
        "entry" | "exit" => {
            let mut i = vec![
                ("condition".into(), "bool".into()),
                ("quantity".into(), "quantity".into()),
                ("limitPrice".into(), "price".into()),
                ("stopPrice".into(), "price".into()),
            ];
            if k == "entry" {
                i.push(("atr".into(), "price".into()));
            }
            (i, vec![])
        }
        "cancelOrders" => (vec![("condition".into(), "bool".into())], vec![]),
        "function" => {
            let f = functions
                .get(s.get("functionId").map(String::as_str).unwrap_or(""))
                .ok_or_else(|| failure("research_function", "$"))?;
            (
                map(&f["inputs"])?.into_iter().collect(),
                vec![("value".into(), strv(f, "outputType")?.into())],
            )
        }
        _ => return Err(failure("research_node_kind", k)),
    };
    if inputs
        .iter()
        .chain(outputs.iter())
        .any(|(_, t)| !KINDS.contains(&t.as_str()))
    {
        return Err(failure("research_type", k));
    }
    Ok((inputs.into_iter().collect(), outputs.into_iter().collect()))
}
const KINDS: &[&str] = &["bool", "integer", "price", "money", "quantity", "ratio"];
fn literal(t: &str, v: &str) -> Result<Atom> {
    if t == "bool" {
        return match v {
            "true" => Ok(Atom::Boolean(true)),
            "false" => Ok(Atom::Boolean(false)),
            _ => Err(failure("research_boolean", v)),
        };
    }
    if !KINDS.contains(&t) {
        return Err(failure("research_type", t));
    }
    let n = num(v)?;
    if t == "integer" && (n.coefficient() < 0 || n.coefficient() % FACTOR != 0) {
        return Err(failure("research_integer", v));
    }
    Ok(Atom::Number(n))
}
impl Graph {
    pub fn compile(v: &V) -> Result<Self> {
        exact(
            v,
            &[
                "format",
                "version",
                "projectId",
                "context",
                "parameters",
                "functions",
                "nodes",
                "wires",
            ],
        )?;
        if v["format"] != "desk.strategy-composition" || v["version"] != 3 {
            return Err(failure("research_model_version", "$"));
        }
        let context = map(&v["context"])?;
        let list = arr(v, "nodes")?;
        if list.is_empty() || list.len() > 200 {
            return Err(failure("research_graph_quota", "$.nodes"));
        }
        let mut parameters: BTreeMap<String, (String, String)> = BTreeMap::new();
        for p in arr(v, "parameters")? {
            exact(
                p,
                &[
                    "id", "label", "type", "value", "minimum", "maximum", "step", "optimize",
                ],
            )?;
            let id = strv(p, "id")?;
            let t = strv(p, "type")?;
            if !KINDS.contains(&t) {
                return Err(failure("research_type", id));
            }
            fqe_ir::model::identifier(id, "$.parameters.id")?;
            let value = literal(t, strv(p, "value")?)?;
            let optimize = p["optimize"]
                .as_bool()
                .ok_or_else(|| failure("research_parameter_optimize", id))?;
            if optimize {
                if t == "bool" {
                    return Err(failure("research_boolean_grid_not_supported", id));
                }
                let lo = literal(t, strv(p, "minimum")?)?.number()?;
                let hi = literal(t, strv(p, "maximum")?)?.number()?;
                let step = literal(t, strv(p, "step")?)?.number()?;
                let value = value.number()?;
                if lo.coefficient() > hi.coefficient()
                    || step.coefficient() <= 0
                    || value.coefficient() < lo.coefficient()
                    || value.coefficient() > hi.coefficient()
                {
                    return Err(failure("research_parameter_range", id));
                }
            }
            if parameters
                .insert(id.into(), (t.into(), strv(p, "value")?.into()))
                .is_some()
            {
                return Err(failure("research_duplicate_parameter", id));
            }
        }
        if parameters.len() > 256 {
            return Err(failure("research_parameter_quota", "$"));
        }
        let mut functions = BTreeMap::new();
        for f in arr(v, "functions")? {
            exact(f, &["id", "name", "inputs", "outputType", "body", "ast"])?;
            fqe_ir::model::identifier(strv(f, "id")?, "$.functions.id")?;
            strv(f, "body")?;
            let inputs = map(&f["inputs"])?;
            for (name, t) in &inputs {
                fqe_ir::model::identifier(name, "$.functions.inputs")?;
                if !KINDS.contains(&t.as_str()) {
                    return Err(failure("research_function_type", name));
                }
            }
            if inputs.is_empty() || inputs.len() > 16 {
                return Err(failure("research_function_inputs", "$"));
            }
            let t = check_ast(&f["ast"], &inputs, 0, &mut 0)?;
            if t != strv(f, "outputType")? {
                return Err(failure("research_function_type", "$"));
            }
            if functions.insert(strv(f, "id")?.into(), f.clone()).is_some() {
                return Err(failure("research_duplicate_function", "$"));
            }
        }
        if functions.len() > 64 {
            return Err(failure("research_function_quota", "$"));
        }
        let mut nodes = vec![];
        let mut ids = BTreeSet::new();
        for n in list {
            exact(n, &["id", "kind", "label", "settings", "literals"])?;
            let id = strv(n, "id")?.to_owned();
            if !ids.insert(id.clone()) {
                return Err(failure("research_duplicate_node", id));
            }
            fqe_ir::model::identifier(&id, "$.node")?;
            let mut settings = map(&n["settings"])?;
            let mut literals = map(&n["literals"])?;
            let kind = strv(n, "kind")?.to_owned();
            let raw_settings = settings.clone();
            for (key, value) in &mut settings {
                if let Some(id) = value.strip_prefix('$') {
                    let parameter = parameters
                        .get(id)
                        .ok_or_else(|| failure("research_parameter_missing", id))?;
                    let expected = match key.as_str() {
                        "period" | "fast" | "slow" | "signal" | "bars" | "offset" | "maxAdds"
                        | "validBars" | "maxAgeSeconds" | "inputCount" => Some("integer"),
                        "quantity" | "lotStep" | "minQuantity" | "maxQuantity" => Some("quantity"),
                        "cashCap" => Some("money"),
                        "limitPrice" | "stopPrice" => Some("price"),
                        "riskPercent" | "deviation" | "closePercent" => Some("ratio"),
                        "initial" | "value" => raw_settings.get("type").map(String::as_str),
                        k if k.ends_with("Value") => Some(
                            if raw_settings
                                .get(&format!("{}Mode", &k[..k.len() - 5]))
                                .map(String::as_str)
                                == Some("price")
                            {
                                "price"
                            } else {
                                "ratio"
                            },
                        ),
                        _ => None,
                    };
                    if expected.is_none_or(|t| t != parameter.0) {
                        return Err(failure("research_parameter_type", format!("{id}.{key}")));
                    }
                    *value = parameter.1.clone();
                }
            }
            let (input_types, _) = ports(&kind, &settings, &functions)?;
            for (key, value) in &mut literals {
                if let Some(id) = value.strip_prefix('$') {
                    let parameter = parameters
                        .get(id)
                        .ok_or_else(|| failure("research_parameter_missing", id))?;
                    if input_types.get(key) != Some(&parameter.0) {
                        return Err(failure("research_parameter_type", format!("{id}.{key}")));
                    }
                    *value = parameter.1.clone();
                }
            }
            if kind == "parameter" {
                let p = parameters
                    .get(settings.get("parameter").map(String::as_str).unwrap_or(""))
                    .ok_or_else(|| failure("research_parameter_missing", &id))?;
                settings.insert("type".into(), p.0.clone());
                settings.insert("value".into(), p.1.clone());
            }
            let (inputs, outputs) = ports(&kind, &settings, &functions)?;
            nodes.push(Node {
                id,
                kind,
                settings,
                literals,
                inputs,
                outputs,
            });
        }
        let mut wires = BTreeMap::new();
        if arr(v, "wires")?.len() > 4096 {
            return Err(failure("research_wire_quota", "$"));
        }
        for w in arr(v, "wires")? {
            exact(w, &["from", "output", "to", "input"])?;
            let f = nodes
                .iter()
                .find(|n| n.id == strv(w, "from").unwrap_or(""))
                .ok_or_else(|| failure("research_wire_node", "$"))?;
            let t = nodes
                .iter()
                .find(|n| n.id == strv(w, "to").unwrap_or(""))
                .ok_or_else(|| failure("research_wire_node", "$"))?;
            let a = f
                .outputs
                .get(strv(w, "output")?)
                .ok_or_else(|| failure("research_output_port", &f.id))?;
            let b = t
                .inputs
                .get(strv(w, "input")?)
                .ok_or_else(|| failure("research_input_port", &t.id))?;
            if a != b {
                return Err(failure(
                    "research_wire_type",
                    format!("{}.{}", t.id, strv(w, "input")?),
                ));
            }
            if wires
                .insert(
                    (t.id.clone(), strv(w, "input")?.into()),
                    (f.id.clone(), strv(w, "output")?.into()),
                )
                .is_some()
            {
                return Err(failure("research_duplicate_input", &t.id));
            }
        }
        for n in &nodes {
            validate_settings(n)?;
            for (p, t) in &n.inputs {
                let optional = (n.kind == "wait" && p == "cancel")
                    || ((n.kind == "entry" || n.kind == "exit") && p != "condition");
                let key = (n.id.clone(), p.clone());
                if n.literals.contains_key(p) && wires.contains_key(&key) {
                    return Err(failure("research_bound_twice", &n.id));
                }
                if let Some(l) = n.literals.get(p) {
                    literal(t, l)?;
                } else if !optional && !wires.contains_key(&key) {
                    return Err(failure("research_input_missing", format!("{}.{}", n.id, p)));
                }
            }
            for p in n.literals.keys() {
                if !n.inputs.contains_key(p) {
                    return Err(failure("research_unknown_literal", &n.id));
                }
            }
        }
        let mut done = BTreeSet::new();
        let mut order = vec![];
        while order.len() < nodes.len() {
            let before = order.len();
            for (i, n) in nodes.iter().enumerate() {
                if done.contains(&n.id) {
                    continue;
                }
                if n.kind == "delay"
                    || wires
                        .iter()
                        .filter(|((to, _), _)| to == &n.id)
                        .all(|(_, (from, _))| done.contains(from))
                {
                    done.insert(n.id.clone());
                    order.push(i);
                }
            }
            if before == order.len() {
                return Err(failure("research_cycle", "$.wires"));
            }
        }
        let states = (0..nodes.len()).map(|_| State::default()).collect();
        Ok(Self {
            nodes,
            context,
            functions,
            wires,
            order,
            states,
        })
    }
    pub fn input(
        &self,
        n: &Node,
        p: &str,
        values: &BTreeMap<(String, String), Slot>,
    ) -> Result<Slot> {
        if let Some(w) = self.wires.get(&(n.id.clone(), p.into())) {
            return Ok(values.get(w).cloned().unwrap_or_default());
        }
        Ok(match n.literals.get(p) {
            Some(v) => Slot {
                value: Some(literal(
                    n.inputs.get(p).ok_or_else(|| failure("research_port", p))?,
                    v,
                )?),
                stamp: 0,
            },
            None => Slot::default(),
        })
    }
    pub fn evaluate(
        &mut self,
        sources: &BTreeMap<String, (u64, BTreeMap<String, Decimal>)>,
        account: &BTreeMap<String, Slot>,
        positions: &BTreeMap<(String, String), BTreeMap<String, Slot>>,
        time: u64,
        decision: bool,
        clock: u64,
    ) -> Result<BTreeMap<(String, String), Slot>> {
        let mut values = BTreeMap::new();
        for i in self.order.clone() {
            let n = &self.nodes[i];
            let path = format!("$.nodes.{}", n.id);
            let mut ins = BTreeMap::new();
            for p in n.inputs.keys() {
                ins.insert(p.clone(), self.input(n, p, &values)?);
            }
            // Literal-only indicators are sampled on the primary decision clock.
            let stamp = ins.values().map(|v| v.stamp).max().unwrap_or(0);
            let stamp = if stamp == 0 { clock } else { stamp };
            let ready = ins
                .iter()
                .filter(|(p, _)| !(n.kind == "wait" && p.as_str() == "cancel"))
                .all(|(_, v)| v.value.is_some());
            let number = |p: &str| -> Result<Decimal> {
                ins.get(p)
                    .and_then(|v| v.value.as_ref())
                    .ok_or_else(|| failure("research_not_ready", &path))?
                    .number()
            };
            let boolean = |p: &str| -> Result<bool> {
                ins.get(p)
                    .and_then(|v| v.value.as_ref())
                    .ok_or_else(|| failure("research_not_ready", &path))?
                    .boolean()
            };
            let state = &mut self.states[i];
            let mut out = BTreeMap::new();
            let mut put = |p: &str, s: Slot| {
                out.insert(p.into(), s);
            };
            match n.kind.as_str() {
                "bars" => {
                    let id = n.get("dataset")?;
                    if let Some((closed, fields)) = sources.get(id) {
                        let max = n
                            .get("maxAgeSeconds")?
                            .parse::<u64>()
                            .map_err(|_| failure("research_age", &path))?
                            .checked_mul(1_000_000_000)
                            .ok_or_else(|| failure("research_age", &path))?;
                        if *closed <= time && time - *closed <= max {
                            for (k, v) in fields {
                                put(k, Slot::num(*v, *closed));
                            }
                        }
                    }
                }
                "constant" | "parameter" => put(
                    "value",
                    Slot {
                        value: Some(literal(n.get("type")?, n.get("value")?)?),
                        stamp: 0,
                    },
                ),
                "account" => out = account.clone(),
                "position" => {
                    let symbol = inherit(n, "symbol", &self.context)?;
                    if let Some(p) = positions.get(&(symbol, n.get("side")?.into())) {
                        out = p.clone();
                    }
                }
                "delay" => put(
                    "previous",
                    Slot {
                        value: if state.count == 0 {
                            Some(literal(n.get("type")?, n.get("initial")?)?)
                        } else {
                            state.delay.clone()
                        },
                        stamp: clock,
                    },
                ),
                "entry" | "exit" | "cancelOrders" => {}
                "function" => {
                    let f = &self.functions[n.get("functionId")?];
                    let value = eval_ast(&f["ast"], &ins)?;
                    put("value", Slot { value, stamp });
                }
                "crossUp" | "crossDown" => {
                    if decision {
                        let mut hit = false;
                        if ready {
                            let a = number("a")?;
                            let b = number("b")?;
                            if let Some((x, y)) = state.previous {
                                hit = if n.kind == "crossUp" {
                                    x.coefficient() <= y.coefficient()
                                        && a.coefficient() > b.coefficient()
                                } else {
                                    x.coefficient() >= y.coefficient()
                                        && a.coefficient() < b.coefficient()
                                };
                            }
                            state.previous = Some((a, b));
                        } else {
                            state.previous = None;
                        }
                        put(
                            "result",
                            if ready {
                                Slot::boolean(hit, clock)
                            } else {
                                Slot::default()
                            },
                        );
                    } else {
                        out = state.values.clone();
                    }
                }
                "wait" | "consecutive" => {
                    if decision {
                        let mut hit = false;
                        let cancel = ins
                            .get("cancel")
                            .and_then(|v| v.value.as_ref())
                            .map(Atom::boolean)
                            .transpose()?
                            .unwrap_or(false);
                        if cancel {
                            state.active = None;
                            state.hit = false;
                            state.count = 0;
                        } else if n.kind == "consecutive" {
                            if ready && boolean("condition")? {
                                state.count = state.count.saturating_add(1);
                                let limit = uint(n, "bars")?;
                                hit = state.count == limit
                                    || (state.count > limit && n.get("mode")? == "every");
                            } else {
                                state.count = 0;
                            }
                        } else {
                            let arm = ready && boolean("arm")?;
                            if state.active.is_some() && arm && n.get("retrigger")? == "restart" {
                                state.active = Some(clock);
                                state.hit = false;
                            } else if let Some(start) = state.active {
                                if clock > start {
                                    state.hit |= ready && boolean("condition")?;
                                    if n.get("mode")? == "first_hit" && state.hit {
                                        hit = true;
                                        state.active = None;
                                    } else if clock - start >= uint(n, "bars")? {
                                        hit = state.hit;
                                        state.active = None;
                                    }
                                }
                            } else if arm {
                                state.active = Some(clock);
                                state.hit = false;
                            }
                        }
                        put(
                            "result",
                            if ready || cancel {
                                Slot::boolean(hit, clock)
                            } else {
                                Slot::default()
                            },
                        );
                    } else {
                        out = state.values.clone();
                    }
                }
                "sma" | "ema" | "rsi" | "macd" => {
                    if !ready {
                        out = n
                            .outputs
                            .keys()
                            .map(|p| (p.clone(), Slot::default()))
                            .collect();
                    } else if state.last == Some(stamp) {
                        out = state.values.clone();
                    } else {
                        if state.provider.is_none() {
                            let trigger = Trigger::ClosedBar {
                                instrument: fqe_core::InstrumentId::new("RESEARCH")
                                    .map_err(|_| failure("research_type", &path))?,
                                timeframe_ns: 1,
                                price_scale: SCALE,
                                volume_scale: 3,
                            };
                            let registry = catalog::reference(&trigger)?;
                            let descriptor = registry
                                .descriptors()
                                .find(|d| d.declaration().key.id == format!("reference.{}", n.kind))
                                .ok_or_else(|| failure("research_provider", &path))?;
                            let mut params = vec![];
                            for p in descriptor.declaration().parameters.specs() {
                                let key = p.key();
                                let value = uint(n, key)?;
                                let t = TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64)
                                    .map_err(|_| failure("research_type", &path))?;
                                params.push((
                                    key.into(),
                                    ParameterInput::Value(Evaluation::Ready(
                                        TypedValue::new(t, ScalarValue::U64(value))
                                            .map_err(|_| failure("research_type", &path))?,
                                    )),
                                ));
                            }
                            state.provider = Some(registry.instantiate(
                                &descriptor.requirement(),
                                params,
                                "research",
                                &n.id,
                            )?);
                        }
                        let descriptor = TypeDescriptor::new(
                            1,
                            ValueShape::Scalar,
                            ScalarType::Decimal {
                                scale: SCALE,
                                unit: NumericUnit::Price(
                                    fqe_core::InstrumentId::new("RESEARCH")
                                        .map_err(|_| failure("research_type", &path))?,
                                ),
                            },
                        )
                        .map_err(|_| failure("research_type", &path))?;
                        let value =
                            TypedValue::new(descriptor, ScalarValue::Decimal(number("price")?))
                                .map_err(|_| failure("research_type", &path))?;
                        let seq = state.provider.as_ref().expect("provider").accepted_steps() + 1;
                        let result = state
                            .provider
                            .as_mut()
                            .expect("provider")
                            .step(seq, vec![("close".into(), Evaluation::Ready(value))])?;
                        for (p, v) in result.outputs() {
                            let value = match v {
                                Evaluation::Ready(v) => match v.value() {
                                    ScalarValue::Decimal(v) => Some(Atom::Number(*v)),
                                    _ => return Err(failure("research_type", &path)),
                                },
                                Evaluation::Warmup | Evaluation::Missing => None,
                                Evaluation::Error(_) => {
                                    return Err(failure("research_provider_numeric", &path));
                                }
                            };
                            put(p, Slot { value, stamp });
                        }
                        state.last = Some(stamp);
                    }
                }
                "atr" | "highest" | "lowest" | "boll" => {
                    if !ready {
                        out = n
                            .outputs
                            .keys()
                            .map(|p| (p.clone(), Slot::default()))
                            .collect();
                    } else if state.last == Some(stamp) {
                        out = state.values.clone();
                    } else {
                        let period = uint(n, "period")? as usize;
                        if n.kind == "atr" {
                            let high = number("high")?;
                            let low = number("low")?;
                            let close = number("close")?;
                            let mut tr = sub(high, low)?;
                            if let Some(previous) = state.close {
                                tr = maxd(
                                    tr,
                                    maxd(absd(sub(high, previous)?)?, absd(sub(low, previous)?)?),
                                );
                            }
                            state.close = Some(close);
                            if let Some(previous) = state.smooth {
                                state.smooth = Some(div(
                                    add(mul(previous, whole((period - 1) as i128)?)?, tr)?,
                                    whole(period as i128)?,
                                )?);
                            } else {
                                state.history.push_back(tr);
                                if state.history.len() == period {
                                    state.smooth = Some(div(
                                        sum(state.history.iter().copied())?,
                                        whole(period as i128)?,
                                    )?);
                                    state.history.clear();
                                }
                            }
                            if let Some(v) = state.smooth {
                                put("value", Slot::num(v, stamp));
                            }
                        } else {
                            let offset = if n.kind == "boll" {
                                0
                            } else {
                                uint(n, "offset")? as usize
                            };
                            state.history.push_back(number("price")?);
                            while state.history.len() > period + offset {
                                state.history.pop_front();
                            }
                            if state.history.len() == period + offset {
                                let samples = state
                                    .history
                                    .iter()
                                    .take(period)
                                    .copied()
                                    .collect::<Vec<_>>();
                                if n.kind == "highest" || n.kind == "lowest" {
                                    let x = samples
                                        .iter()
                                        .copied()
                                        .reduce(|a, b| {
                                            if n.kind == "highest" {
                                                maxd(a, b)
                                            } else {
                                                mind(a, b)
                                            }
                                        })
                                        .expect("period");
                                    put("value", Slot::num(x, stamp));
                                } else {
                                    let mean =
                                        div(sum(samples.iter().copied())?, whole(period as i128)?)?;
                                    let variance = div(
                                        sum(samples
                                            .iter()
                                            .map(|v| sub(*v, mean).and_then(|d| mul(d, d)))
                                            .collect::<Result<Vec<_>>>()?)?,
                                        whole(period as i128)?,
                                    )?;
                                    let width = mul(sqrt(variance)?, num(n.get("deviation")?)?)?;
                                    put("middle", Slot::num(mean, stamp));
                                    put("upper", Slot::num(add(mean, width)?, stamp));
                                    put("lower", Slot::num(sub(mean, width)?, stamp));
                                }
                            }
                        }
                        state.last = Some(stamp);
                    }
                }
                _ if ready => {
                    let value = match n.kind.as_str() {
                        "and" | "or" => Atom::Boolean(if n.kind == "and" {
                            ins.values()
                                .all(|s| matches!(s.value, Some(Atom::Boolean(true))))
                        } else {
                            ins.values()
                                .any(|s| matches!(s.value, Some(Atom::Boolean(true))))
                        }),
                        "not" => Atom::Boolean(!boolean("condition")?),
                        "compare" => Atom::Boolean(compare(
                            n.get("operator")?,
                            ins["a"].value.as_ref().expect("ready"),
                            ins["b"].value.as_ref().expect("ready"),
                        )?),
                        "add" => Atom::Number(add(number("a")?, number("b")?)?),
                        "subtract" => Atom::Number(sub(number("a")?, number("b")?)?),
                        "multiply" => Atom::Number(mul(number("a")?, number("b")?)?),
                        "divide" => Atom::Number(div(number("a")?, number("b")?)?),
                        "scale" => Atom::Number(mul(number("value")?, number("factor")?)?),
                        "riskSize" => {
                            let distance = number("stopDistance")?;
                            let equity = number("equity")?;
                            let step = num(n.get("lotStep")?)?;
                            let mut q = if distance.coefficient() > 0 && equity.coefficient() > 0 {
                                div(
                                    div(mul(equity, num(n.get("riskPercent")?)?)?, whole(100)?)?,
                                    distance,
                                )?
                            } else {
                                whole(0)?
                            };
                            q = step_floor(mind(q, num(n.get("maxQuantity")?)?), step)?;
                            if q.coefficient() < num(n.get("minQuantity")?)?.coefficient() {
                                q = whole(0)?;
                            }
                            Atom::Number(q)
                        }
                        _ => return Err(failure("research_node_kind", &n.kind)),
                    };
                    let p = n.outputs.keys().next().expect("expression output");
                    put(
                        p,
                        Slot {
                            value: Some(value),
                            stamp,
                        },
                    );
                }
                _ => {}
            }
            for p in n.outputs.keys() {
                values.insert(
                    (n.id.clone(), p.clone()),
                    out.get(p).cloned().unwrap_or_default(),
                );
            }
            if !out.is_empty() {
                state.values = out;
            }
        }
        if decision {
            for i in 0..self.nodes.len() {
                if self.nodes[i].kind == "delay" {
                    let v = self.input(&self.nodes[i], "next", &values)?;
                    self.states[i].delay = v.value;
                    self.states[i].count = 1;
                }
            }
        }
        Ok(values)
    }
}
pub fn inherit(n: &Node, k: &str, context: &BTreeMap<String, String>) -> Result<String> {
    let value = n.settings.get(k).map(String::as_str).unwrap_or("");
    Ok(if value.is_empty() {
        context
            .get(k)
            .ok_or_else(|| failure("research_context", k))?
            .clone()
    } else {
        value.into()
    })
}
fn uint(n: &Node, k: &str) -> Result<u64> {
    let v = n.get(k)?;
    let x = v
        .parse::<u64>()
        .map_err(|_| failure("research_unsigned", &n.id))?;
    if x.to_string() != v {
        return Err(failure("research_unsigned", &n.id));
    }
    Ok(x)
}
fn validate_settings(n: &Node) -> Result<()> {
    let keys: &[&str] = match n.kind.as_str() {
        "bars" => &[
            "symbol",
            "timeframe",
            "dataset",
            "trainingDataset",
            "validationDataset",
            "alignment",
            "maxAgeSeconds",
        ],
        "constant" => &["type", "value"],
        "parameter" => &["parameter", "type", "value"],
        "sma" | "ema" | "rsi" | "atr" => &["period"],
        "macd" => &["fast", "slow", "signal"],
        "boll" => &["period", "deviation"],
        "highest" | "lowest" => &["period", "offset"],
        "add" | "subtract" | "scale" => &["type"],
        "multiply" | "divide" => &[],
        "compare" => &["type", "operator"],
        "crossUp" | "crossDown" => &["type"],
        "and" | "or" => &["inputCount"],
        "not" | "account" => &[],
        "delay" => &["type", "initial"],
        "wait" => &["bars", "mode", "retrigger"],
        "consecutive" => &["bars", "mode"],
        "position" => &["symbol", "side"],
        "riskSize" => &["riskPercent", "lotStep", "minQuantity", "maxQuantity"],
        "function" => &["functionId"],
        "cancelOrders" => &["symbol", "scope", "orderTag", "triggerMode"],
        "entry" => &[
            "symbol",
            "side",
            "orderType",
            "limitPrice",
            "stopPrice",
            "timeInForce",
            "validBars",
            "orderTag",
            "triggerMode",
            "entryMode",
            "maxAdds",
            "quantity",
            "cashCap",
            "stopLossMode",
            "stopLossValue",
            "takeProfitMode",
            "takeProfitValue",
            "trailingMode",
            "trailingValue",
        ],
        "exit" => &[
            "symbol",
            "side",
            "orderType",
            "limitPrice",
            "stopPrice",
            "timeInForce",
            "validBars",
            "orderTag",
            "triggerMode",
            "closeMode",
            "quantity",
            "closePercent",
        ],
        _ => return Err(failure("research_node_kind", &n.id)),
    };
    if n.settings
        .keys()
        .any(|k| k != "label" && !keys.contains(&k.as_str()))
    {
        return Err(failure("research_unknown_setting", &n.id));
    }
    for key in ["period", "fast", "slow", "signal", "bars"] {
        if n.settings.contains_key(key) && !(1..=10000).contains(&uint(n, key)?) {
            return Err(failure("research_period", &n.id));
        }
    }
    if n.kind == "macd" && uint(n, "fast")? >= uint(n, "slow")? {
        return Err(failure("research_period", &n.id));
    }
    for (key, choices) in [
        ("side", &["long", "short"][..]),
        ("triggerMode", &["each_bar", "rising_edge"][..]),
        ("orderType", &["market", "limit", "stop", "stop_limit"][..]),
        ("timeInForce", &["GTC", "IOC", "FOK", "GTD"][..]),
        ("entryMode", &["open", "add"][..]),
        ("closeMode", &["all", "quantity", "percent"][..]),
        ("alignment", &["latest_closed"][..]),
        ("scope", &["tag", "symbol", "all"][..]),
    ] {
        if let Some(v) = n.settings.get(key)
            && !choices.contains(&v.as_str())
        {
            return Err(failure(
                "research_setting_choice",
                format!("{}.{}", n.id, key),
            ));
        }
    }
    if n.kind == "compare"
        && ![
            "greater",
            "greater_equal",
            "less",
            "less_equal",
            "equal",
            "not_equal",
        ]
        .contains(&n.get("operator")?)
    {
        return Err(failure("research_compare_operator", &n.id));
    }
    if n.kind == "bars" {
        uint(n, "maxAgeSeconds")?;
    }
    if n.kind == "exit" {
        let percent = num(n.get("closePercent")?)?;
        if percent.coefficient() <= 0 || percent.coefficient() > 100 * FACTOR {
            return Err(failure("research_close_percent", &n.id));
        }
    }
    if n.kind == "riskSize"
        && (num(n.get("riskPercent")?)?.coefficient() > 100 * FACTOR
            || num(n.get("minQuantity")?)?.coefficient()
                > num(n.get("maxQuantity")?)?.coefficient())
    {
        return Err(failure("research_risk_range", &n.id));
    }
    if n.kind == "wait"
        && (!["expiry", "first_hit"].contains(&n.get("mode")?)
            || !["ignore", "restart"].contains(&n.get("retrigger")?))
    {
        return Err(failure("research_wait_mode", &n.id));
    }
    if n.kind == "consecutive" && !["once", "every"].contains(&n.get("mode")?) {
        return Err(failure("research_consecutive_mode", &n.id));
    }
    if n.kind == "constant" || n.kind == "parameter" {
        literal(n.get("type")?, n.get("value")?)?;
    }
    if n.kind == "delay" {
        literal(n.get("type")?, n.get("initial")?)?;
    }
    for key in [
        "deviation",
        "lotStep",
        "minQuantity",
        "maxQuantity",
        "quantity",
        "cashCap",
        "limitPrice",
        "stopPrice",
        "riskPercent",
    ] {
        if let Some(v) = n.settings.get(key)
            && num(v)?.coefficient() <= 0
        {
            return Err(failure("research_positive", format!("{}.{}", n.id, key)));
        }
    }
    for key in ["maxAdds", "validBars"] {
        if n.settings.contains_key(key) && !(1..=10000).contains(&uint(n, key)?) {
            return Err(failure("research_count", &n.id));
        }
    }
    if (n.kind == "highest" || n.kind == "lowest") && uint(n, "offset")? > 10000 {
        return Err(failure("research_offset", &n.id));
    }
    if n.kind == "entry" {
        for key in ["stopLoss", "takeProfit", "trailing"] {
            let mode = n.get(&format!("{key}Mode"))?;
            if !["none", "percent", "atr", "price"].contains(&mode) {
                return Err(failure("research_protection_mode", &n.id));
            }
            if mode != "none" {
                let v = num(n.get(&format!("{key}Value"))?)?;
                if v.coefficient() <= 0
                    || (mode == "percent" && key != "takeProfit" && v.coefficient() >= 100 * FACTOR)
                {
                    return Err(failure("research_protection_value", &n.id));
                }
                if mode == "atr" && !n.literals.contains_key("atr") { /* wire validated at run preparation */
                }
            }
        }
    }
    Ok(())
}
fn check_ast(
    a: &V,
    inputs: &BTreeMap<String, String>,
    depth: usize,
    count: &mut usize,
) -> Result<String> {
    *count += 1;
    if depth > 32 || *count > 2048 {
        return Err(failure("research_expression_quota", "$.functions"));
    }
    let op = strv(a, "op")?;
    let declared = strv(a, "type")?;
    if !KINDS.contains(&declared) {
        return Err(failure("research_expression_type", "$"));
    }
    let t = if op == "input" {
        exact(a, &["op", "name", "type"])?;
        inputs
            .get(strv(a, "name")?)
            .ok_or_else(|| failure("research_function_input", "$"))?
            .clone()
    } else if op == "literal" {
        exact(a, &["op", "value", "type"])?;
        literal(declared, strv(a, "value")?)?;
        declared.into()
    } else {
        exact(a, &["op", "args", "type"])?;
        let args = arr(a, "args")?
            .iter()
            .map(|e| check_ast(e, inputs, depth + 1, count))
            .collect::<Result<Vec<_>>>()?;
        let equal = |i: usize, j: usize| args.get(i) == args.get(j);
        match (op, args.len()) {
            ("not", 1) if args[0] == "bool" => "bool".into(),
            ("negate" | "abs", 1) if args[0] != "bool" && args[0] != "integer" => args[0].clone(),
            ("&&" | "||", 2) if args[0] == "bool" && equal(0, 1) => "bool".into(),
            ("==" | "!=", 2) if equal(0, 1) => "bool".into(),
            (">" | "<" | ">=" | "<=", 2) if args[0] != "bool" && equal(0, 1) => "bool".into(),
            ("+" | "-" | "min" | "max", 2) if args[0] != "bool" && equal(0, 1) => args[0].clone(),
            ("*" | "/", 2) if args[0] == "ratio" && equal(0, 1) => "ratio".into(),
            ("if", 3) if args[0] == "bool" && equal(1, 2) => args[1].clone(),
            ("clamp", 3) if args[0] != "bool" && equal(0, 1) && equal(1, 2) => args[0].clone(),
            _ => return Err(failure("research_expression_operator", op)),
        }
    };
    if t != declared {
        return Err(failure("research_expression_type", op));
    }
    Ok(t)
}
fn compare(op: &str, a: &Atom, b: &Atom) -> Result<bool> {
    match (a, b) {
        (Atom::Boolean(a), Atom::Boolean(b)) => match op {
            "equal" | "==" => Ok(a == b),
            "not_equal" | "!=" => Ok(a != b),
            _ => Err(failure("research_comparison", op)),
        },
        _ => {
            let x = a.number()?.coefficient();
            let y = b.number()?.coefficient();
            match op {
                "less" | "<" => Ok(x < y),
                "less_equal" | "<=" => Ok(x <= y),
                "greater" | ">" => Ok(x > y),
                "greater_equal" | ">=" => Ok(x >= y),
                "equal" | "==" => Ok(x == y),
                "not_equal" | "!=" => Ok(x != y),
                _ => Err(failure("research_comparison", op)),
            }
        }
    }
}
fn eval_ast(a: &V, inputs: &BTreeMap<String, Slot>) -> Result<Option<Atom>> {
    let op = strv(a, "op")?;
    if op == "input" {
        return Ok(inputs.get(strv(a, "name")?).and_then(|v| v.value.clone()));
    }
    if op == "literal" {
        return Ok(Some(literal(strv(a, "type")?, strv(a, "value")?)?));
    }
    let args = arr(a, "args")?;
    let first = eval_ast(&args[0], inputs)?;
    let Some(first) = first else { return Ok(None) };
    // if evaluates only its chosen branch; unused division-by-zero is harmless.
    if op == "if" {
        return eval_ast(&args[if first.boolean()? { 1 } else { 2 }], inputs);
    }
    let mut values = vec![first];
    for a in &args[1..] {
        match eval_ast(a, inputs)? {
            Some(v) => values.push(v),
            None => return Ok(None),
        }
    }
    let n = |i: usize| values[i].number();
    let value = match op {
        "not" => Atom::Boolean(!values[0].boolean()?),
        "negate" => Atom::Number(sub(whole(0)?, n(0)?)?),
        "abs" => Atom::Number(absd(n(0)?)?),
        "&&" => Atom::Boolean(values[0].boolean()? && values[1].boolean()?),
        "||" => Atom::Boolean(values[0].boolean()? || values[1].boolean()?),
        "+" => Atom::Number(add(n(0)?, n(1)?)?),
        "-" => Atom::Number(sub(n(0)?, n(1)?)?),
        "*" => Atom::Number(mul(n(0)?, n(1)?)?),
        "/" => Atom::Number(div(n(0)?, n(1)?)?),
        "min" => Atom::Number(mind(n(0)?, n(1)?)),
        "max" => Atom::Number(maxd(n(0)?, n(1)?)),
        "clamp" => {
            if n(1)?.coefficient() > n(2)?.coefficient() {
                return Err(failure("research_clamp_range", "$"));
            }
            Atom::Number(maxd(n(1)?, mind(n(0)?, n(2)?)))
        }
        _ => Atom::Boolean(compare(op, &values[0], &values[1])?),
    };
    Ok(Some(value))
}
