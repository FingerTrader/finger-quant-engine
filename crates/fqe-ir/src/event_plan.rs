//! Versioned closed-bar/tick event profile, typed expression codec and staged plan.
use crate::{
    compiler::{self, CompiledCall, CompiledPlan, ResolvedSource},
    expressions::{BinaryOp, CrossDirection, Crossing, Expr, ExpressionDraft, ExpressionPlan},
    graph::{LoadedGraph, digest, error},
    loading::{self, Obj, integer, mapped, text, typed, typed_json, uint},
    model::{ContentId, MAX_MODEL_BYTES, ModelError, Port, identifier},
    parameters::{BoundParameters, BoundValue, ParameterInput},
    registry::{PreparedIncremental, Registry},
};
use fqe_core::{
    InstrumentId, SignalKind,
    numeric::Rounding,
    types::{NumericUnit, ScalarType, TypeDescriptor, TypedValue, ValueShape},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
type Result<T> = std::result::Result<T, ModelError>;
pub const EVENT_PREFIX: &[u8] = b"FQE-EVENT-PLAN-CANONICAL/1\n";
pub const EXECUTION_PREFIX: &[u8] = b"FQE-EVENT-EXECUTION/1\n";
/// This version's expression input budget includes graph inputs and all state outputs.
pub const MAX_SLOTS: usize = 32;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    Tick {
        instrument: InstrumentId,
    },
    ClosedBar {
        instrument: InstrumentId,
        timeframe_ns: u64,
        price_scale: u8,
        volume_scale: u8,
    },
}
impl Trigger {
    pub fn instrument(&self) -> &InstrumentId {
        match self {
            Self::Tick { instrument } | Self::ClosedBar { instrument, .. } => instrument,
        }
    }
    fn value_type(&self, field: &str) -> Result<TypeDescriptor> {
        let scalar = match self {
            Self::Tick { .. } if field == "price" => ScalarType::U64,
            Self::ClosedBar {
                instrument,
                price_scale,
                ..
            } if ["open", "high", "low", "close"].contains(&field) => ScalarType::Decimal {
                scale: *price_scale,
                unit: NumericUnit::Price(instrument.clone()),
            },
            Self::ClosedBar {
                instrument,
                volume_scale,
                ..
            } if field == "volume" => ScalarType::Decimal {
                scale: *volume_scale,
                unit: NumericUnit::Quantity(instrument.clone()),
            },
            _ => {
                return Err(error(
                    "event_binding",
                    "$.bindings",
                    "field is unavailable in trigger domain",
                ));
            }
        };
        TypeDescriptor::new(1, ValueShape::Scalar, scalar)
            .map_err(|_| error("event_type", "$.trigger", "invalid field type"))
    }
}
fn trigger(v: Value, p: &str) -> Result<Trigger> {
    let mut o = Obj::new(v, p)?;
    let kind = text(o.take("kind")?, p)?;
    let instrument = InstrumentId::new(text(o.take("instrument")?, p)?)
        .map_err(|_| error("event_instrument", p, "invalid instrument"))?;
    let t = match kind.as_str() {
        "tick" => Trigger::Tick { instrument },
        "closed_bar" => {
            let timeframe_ns: u64 = integer(o.take("timeframe_ns")?, p)?;
            let price_scale = uint(o.take("price_scale")?, p)?;
            let volume_scale = uint(o.take("volume_scale")?, p)?;
            if timeframe_ns == 0 || price_scale > 12 || volume_scale > 18 {
                return Err(error(
                    "event_trigger",
                    p,
                    "positive duration, price scale 0..12 and volume scale 0..18 required",
                ));
            }
            Trigger::ClosedBar {
                instrument,
                timeframe_ns,
                price_scale: price_scale as u8,
                volume_scale: volume_scale as u8,
            }
        }
        _ => return Err(error("event_trigger", p, "unsupported trigger")),
    };
    o.end()?;
    Ok(t)
}
fn trigger_json(t: &Trigger) -> Value {
    match t {
        Trigger::Tick { instrument } => json!({"kind":"tick","instrument":instrument.as_str()}),
        Trigger::ClosedBar {
            instrument,
            timeframe_ns,
            price_scale,
            volume_scale,
        } => {
            json!({"kind":"closed_bar","instrument":instrument.as_str(),"timeframe_ns":timeframe_ns.to_string(),"price_scale":price_scale,"volume_scale":volume_scale})
        }
    }
}
#[derive(Clone)]
enum PortableExpr {
    Literal(TypedValue),
    Parameter(String),
    Input(String),
    Output(String),
    Cross(String),
    Not(Box<Self>),
    Binary {
        op: BinaryOp,
        left: Box<Self>,
        right: Box<Self>,
    },
}
fn operation(name: &str, rounding: Option<Value>, p: &str) -> Result<BinaryOp> {
    let r = match rounding {
        Some(v) => match text(v, p)?.as_str() {
            "exact" => Rounding::Exact,
            "half_even" => Rounding::HalfEven,
            "toward_zero" => Rounding::TowardZero,
            _ => return Err(error("event_expression", p, "unknown rounding")),
        },
        None => Rounding::Exact,
    };
    match name {
        "add" => Ok(BinaryOp::Add),
        "subtract" => Ok(BinaryOp::Subtract),
        "multiply" => Ok(BinaryOp::Multiply(r)),
        "divide" => Ok(BinaryOp::Divide(r)),
        "equal" => Ok(BinaryOp::Equal),
        "not_equal" => Ok(BinaryOp::NotEqual),
        "less" => Ok(BinaryOp::Less),
        "less_equal" => Ok(BinaryOp::LessEqual),
        "greater" => Ok(BinaryOp::Greater),
        "greater_equal" => Ok(BinaryOp::GreaterEqual),
        "and" => Ok(BinaryOp::And),
        "or" => Ok(BinaryOp::Or),
        _ => Err(error("event_expression", p, "unknown binary operation")),
    }
}
fn expression(v: Value, p: &str, depth: usize, count: &mut usize) -> Result<PortableExpr> {
    *count += 1;
    if depth > 32 || *count > 4096 {
        return Err(error("event_limit", p, "expression budget exceeded"));
    }
    let o = Obj::new(v, p)?;
    if o.fields.len() != 1 {
        return Err(error("event_expression", p, "one expression tag required"));
    }
    let (tag, v) = o.fields.into_iter().next().expect("one field");
    Ok(match tag.as_str() {
        "literal" => PortableExpr::Literal(typed(v, p)?),
        "parameter" | "input" | "output" | "cross" => {
            let n = text(v, p)?;
            identifier(&n, p)?;
            match tag.as_str() {
                "parameter" => PortableExpr::Parameter(n),
                "input" => PortableExpr::Input(n),
                "output" => PortableExpr::Output(n),
                _ => PortableExpr::Cross(n),
            }
        }
        "not" => PortableExpr::Not(Box::new(expression(v, p, depth + 1, count)?)),
        "binary" => {
            let mut b = Obj::new(v, p)?;
            let name = text(b.take("op")?, p)?;
            let rounding = b.optional("rounding");
            if rounding.is_some() && !["multiply", "divide"].contains(&name.as_str()) {
                return Err(error(
                    "event_expression",
                    p,
                    "rounding is only valid for multiply/divide",
                ));
            }
            let op = operation(&name, rounding, p)?;
            let left = Box::new(expression(b.take("left")?, p, depth + 1, count)?);
            let right = Box::new(expression(b.take("right")?, p, depth + 1, count)?);
            b.end()?;
            PortableExpr::Binary { op, left, right }
        }
        _ => return Err(error("event_expression", p, "unknown expression tag")),
    })
}
fn op_json(op: BinaryOp) -> (&'static str, Option<&'static str>) {
    let r = |r| {
        Some(match r {
            Rounding::Exact => "exact",
            Rounding::HalfEven => "half_even",
            Rounding::TowardZero => "toward_zero",
        })
    };
    match op {
        BinaryOp::Add => ("add", None),
        BinaryOp::Subtract => ("subtract", None),
        BinaryOp::Multiply(a) => ("multiply", r(a)),
        BinaryOp::Divide(a) => ("divide", r(a)),
        BinaryOp::Equal => ("equal", None),
        BinaryOp::NotEqual => ("not_equal", None),
        BinaryOp::Less => ("less", None),
        BinaryOp::LessEqual => ("less_equal", None),
        BinaryOp::Greater => ("greater", None),
        BinaryOp::GreaterEqual => ("greater_equal", None),
        BinaryOp::And => ("and", None),
        BinaryOp::Or => ("or", None),
    }
}
fn expr_json(e: &PortableExpr) -> Value {
    match e {
        PortableExpr::Literal(v) => json!({"literal":typed_json(v)}),
        PortableExpr::Parameter(n) => json!({"parameter":n}),
        PortableExpr::Input(n) => json!({"input":n}),
        PortableExpr::Output(n) => json!({"output":n}),
        PortableExpr::Cross(n) => json!({"cross":n}),
        PortableExpr::Not(e) => json!({"not":expr_json(e)}),
        PortableExpr::Binary { op, left, right } => {
            let (n, r) = op_json(*op);
            let mut b = json!({"op":n,"left":expr_json(left),"right":expr_json(right)});
            if let Some(r) = r {
                b["rounding"] = json!(r);
            }
            json!({"binary":b})
        }
    }
}
struct PortableCross {
    id: String,
    direction: CrossDirection,
    left: PortableExpr,
    right: PortableExpr,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignalRule {
    pub output: String,
    pub kind: SignalKind,
}
pub struct LoadedEventPlan {
    graph: LoadedGraph,
    trigger: Trigger,
    bindings: BTreeMap<String, String>,
    warmup_samples: u64,
    crossings: Vec<PortableCross>,
    outputs: Vec<(String, PortableExpr)>,
    signals: Vec<SignalRule>,
    canonical: Vec<u8>,
    content_id: ContentId,
    source_id: ContentId,
}
fn named(n: &str, names: &mut BTreeSet<String>, p: &str) -> Result<()> {
    identifier(n, p)?;
    if !names.insert(n.into()) {
        return Err(error("event_duplicate", p, "duplicate name"));
    }
    Ok(())
}
impl LoadedEventPlan {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut o = Obj::new(loading::decode(bytes)?, "$")?;
        if text(o.take("format")?, "$")? != "fqe.event-plan"
            || uint(o.take("schema_version")?, "$")? != 1
        {
            return Err(error("event_version", "$", "expected fqe.event-plan/1"));
        }
        let id = text(o.take("id")?, "$.id")?;
        identifier(&id, "$.id")?;
        let version = text(o.take("version")?, "$.version")?;
        identifier(&version, "$.version")?;
        let mut requirements = mapped(o.take("requirements")?, "$.requirements", text)?;
        requirements.sort();
        if requirements != ["event.plan/1", "expression.scalar/1"] {
            return Err(error(
                "event_capability",
                "$.requirements",
                "exact supported capabilities event.plan/1 and expression.scalar/1 required",
            ));
        }
        let graph = LoadedGraph::from_bytes(&serde_json::to_vec(&o.take("graph")?).expect("JSON"))?;
        let trigger = trigger(o.take("trigger")?, "$.trigger")?;
        let b = Obj::new(o.take("bindings")?, "$.bindings")?;
        let mut bindings = BTreeMap::new();
        for (n, v) in b.fields {
            identifier(&n, "$.bindings")?;
            bindings.insert(n, text(v, "$.bindings")?);
        }
        if bindings.len() != graph.definition().graph.inputs.len() {
            return Err(error(
                "event_binding",
                "$.bindings",
                "each graph input must bind exactly one event field",
            ));
        }
        for p in &graph.definition().graph.inputs {
            let field = bindings
                .get(&p.name)
                .ok_or_else(|| error("event_binding", "$.bindings", "missing graph input"))?;
            if trigger.value_type(field)? != p.value_type {
                return Err(error(
                    "event_type",
                    "$.bindings",
                    "event field type/unit/scale differs from graph input",
                ));
            }
        }
        let warmup_samples = integer(o.take("warmup_samples")?, "$.warmup_samples")?;
        let mut count = 0;
        let mut names = BTreeSet::new();
        let crossings = mapped(o.take("crossings")?, "$.crossings", |v, p| {
            let mut c = Obj::new(v, p)?;
            let id = text(c.take("id")?, p)?;
            named(&id, &mut names, p)?;
            let direction = match text(c.take("direction")?, p)?.as_str() {
                "above" => CrossDirection::Above,
                "below" => CrossDirection::Below,
                _ => return Err(error("event_crossing", p, "unknown direction")),
            };
            let left = expression(c.take("left")?, p, 1, &mut count)?;
            let right = expression(c.take("right")?, p, 1, &mut count)?;
            c.end()?;
            Ok(PortableCross {
                id,
                direction,
                left,
                right,
            })
        })?;
        names.clear();
        let outputs = mapped(o.take("outputs")?, "$.outputs", |v, p| {
            let mut r = Obj::new(v, p)?;
            let n = text(r.take("name")?, p)?;
            named(&n, &mut names, p)?;
            let e = expression(r.take("expression")?, p, 1, &mut count)?;
            r.end()?;
            Ok((n, e))
        })?;
        names.clear();
        let signals = mapped(o.take("signals")?, "$.signals", |v, p| {
            let mut s = Obj::new(v, p)?;
            let output = text(s.take("output")?, p)?;
            named(&output, &mut names, p)?;
            let kind = match text(s.take("kind")?, p)?.as_str() {
                "long" => SignalKind::Long,
                "flat" => SignalKind::Flat,
                _ => return Err(error("event_signal", p, "unknown signal kind")),
            };
            s.end()?;
            Ok(SignalRule { output, kind })
        })?;
        o.end()?;
        if outputs.is_empty() || outputs.len() > 32 || crossings.len() > 256 || signals.len() > 32 {
            return Err(error(
                "event_limit",
                "$",
                "root/crossing/signal limit exceeded",
            ));
        }
        let mut result = Self {
            graph,
            trigger,
            bindings,
            warmup_samples,
            crossings,
            outputs,
            signals,
            canonical: vec![],
            content_id: ContentId::from_bytes([0; 32]),
            source_id: digest(b"", bytes),
        };
        result.crossings.sort_by(|a, b| a.id.cmp(&b.id));
        result.outputs.sort_by(|a, b| a.0.cmp(&b.0));
        result.signals.sort_by(|a, b| a.output.cmp(&b.output));
        let canonical = json!({"format":"fqe.event-plan","schema_version":1,"id":id,"version":version,"requirements":requirements,"graph":serde_json::from_slice::<Value>(result.graph.canonical_bytes()).expect("JSON"),"trigger":trigger_json(&result.trigger),"bindings":result.bindings,"warmup_samples":result.warmup_samples.to_string(),"crossings":result.crossings.iter().map(|c|json!({"id":c.id,"direction":match c.direction{CrossDirection::Above=>"above",CrossDirection::Below=>"below"},"left":expr_json(&c.left),"right":expr_json(&c.right)})).collect::<Vec<_>>(),"outputs":result.outputs.iter().map(|(n,e)|json!({"name":n,"expression":expr_json(e)})).collect::<Vec<_>>(),"signals":result.signals.iter().map(|s|json!({"output":s.output,"kind":s.kind.to_string()})).collect::<Vec<_>>()});
        result.canonical = serde_json::to_vec(&canonical).expect("JSON");
        if result.canonical.len() > MAX_MODEL_BYTES {
            return Err(error(
                "event_limit",
                "$",
                "canonical artifact exceeds limit",
            ));
        }
        result.content_id = digest(EVENT_PREFIX, &result.canonical);
        Ok(result)
    }
    pub fn trigger(&self) -> &Trigger {
        &self.trigger
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn content_id(&self) -> ContentId {
        self.content_id
    }
    pub fn source_id(&self) -> ContentId {
        self.source_id
    }
    pub fn compile(
        &self,
        registry: &Registry,
        values: impl IntoIterator<Item = (String, ParameterInput)>,
    ) -> Result<CompiledEventPlan> {
        let binding = self.graph.bind(values)?;
        let graph = Arc::new(compiler::compile_graph(&self.graph, &binding, registry)?);
        let mut slots = Vec::new();
        let mut input_slots = BTreeMap::new();
        let mut state_slots = BTreeMap::new();
        for (i, p) in graph.inputs().iter().enumerate() {
            let name = format!("i{i}");
            slots.push(Port {
                name: name.clone(),
                value_type: p.value_type.clone(),
            });
            input_slots.insert(p.name.clone(), name);
        }
        for (i, n) in graph.nodes().iter().enumerate() {
            if matches!(n.call, CompiledCall::Incremental(_)) {
                for (j, p) in n.call.descriptor().declaration().outputs.iter().enumerate() {
                    let name = format!("s{i}.{j}");
                    slots.push(Port {
                        name: name.clone(),
                        value_type: p.value_type.clone(),
                    });
                    state_slots.insert((n.path.clone(), p.name.clone()), name);
                }
            }
        }
        if slots.len() > MAX_SLOTS {
            return Err(error(
                "event_limit",
                "$.graph",
                "event profile supports at most 32 graph-input/state-output slots",
            ));
        }
        let mut lower = Lower {
            graph: &graph,
            input_slots: &input_slots,
            state_slots: &state_slots,
            parameters: binding.parameters(),
            count: 0,
        };
        let mut stages = Vec::new();
        for (i, n) in graph.nodes().iter().enumerate() {
            if let CompiledCall::Incremental(call) = &n.call {
                let outputs = n
                    .inputs
                    .iter()
                    .map(|(name, source)| Ok((name.clone(), lower.source(source, 1)?)))
                    .collect::<Result<Vec<_>>>()?;
                let inputs = if outputs.is_empty() {
                    None
                } else {
                    Some(Arc::new(ExpressionPlan::new(ExpressionDraft {
                        inputs: slots.clone(),
                        crossings: vec![],
                        outputs,
                    })?))
                };
                let outputs = call
                    .descriptor()
                    .declaration()
                    .outputs
                    .iter()
                    .map(|p| {
                        (
                            p.name.clone(),
                            state_slots[&(n.path.clone(), p.name.clone())].clone(),
                        )
                    })
                    .collect();
                stages.push(EventStage {
                    node_index: i,
                    call: Arc::clone(call),
                    inputs,
                    outputs,
                });
            }
        }
        let crossings = self
            .crossings
            .iter()
            .map(|c| {
                Ok(Crossing {
                    id: c.id.clone(),
                    direction: c.direction,
                    left: lower.expr(&c.left, 1)?,
                    right: lower.expr(&c.right, 1)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let outputs = self
            .outputs
            .iter()
            .map(|(n, e)| Ok((n.clone(), lower.expr(e, 1)?)))
            .collect::<Result<Vec<_>>>()?;
        let roots = Arc::new(ExpressionPlan::new(ExpressionDraft {
            inputs: slots,
            crossings,
            outputs,
        })?);
        for s in &self.signals {
            if !roots
                .output_types()
                .iter()
                .any(|p| p.name == s.output && p.value_type.scalar() == &ScalarType::Bool)
            {
                return Err(error(
                    "event_signal",
                    "$.signals",
                    "signal must reference a Boolean root",
                ));
            }
        }
        let content_id=digest(EXECUTION_PREFIX,&serde_json::to_vec(&json!({"artifact":self.content_id.to_string(),"graph":graph.content_id().to_string()})).expect("JSON"));
        let bindings = self
            .bindings
            .iter()
            .map(|(input, field)| (input_slots[input].clone(), field.clone()))
            .collect();
        Ok(CompiledEventPlan {
            artifact: self.content_id,
            source: self.source_id,
            content_id,
            graph,
            trigger: self.trigger.clone(),
            bindings,
            warmup_samples: self.warmup_samples,
            stages,
            roots,
            signals: self.signals.clone(),
        })
    }
}
pub struct EventStage {
    pub node_index: usize,
    pub call: Arc<PreparedIncremental>,
    pub inputs: Option<Arc<ExpressionPlan>>,
    pub outputs: Vec<(String, String)>,
}
pub struct CompiledEventPlan {
    artifact: ContentId,
    source: ContentId,
    content_id: ContentId,
    graph: Arc<CompiledPlan>,
    trigger: Trigger,
    bindings: BTreeMap<String, String>,
    warmup_samples: u64,
    stages: Vec<EventStage>,
    roots: Arc<ExpressionPlan>,
    signals: Vec<SignalRule>,
}
impl CompiledEventPlan {
    pub fn content_id(&self) -> ContentId {
        self.content_id
    }
    pub fn artifact_id(&self) -> ContentId {
        self.artifact
    }
    pub fn source_id(&self) -> ContentId {
        self.source
    }
    pub fn graph(&self) -> &CompiledPlan {
        &self.graph
    }
    pub fn trigger(&self) -> &Trigger {
        &self.trigger
    }
    pub fn bindings(&self) -> &BTreeMap<String, String> {
        &self.bindings
    }
    pub fn warmup_samples(&self) -> u64 {
        self.warmup_samples
    }
    pub fn stages(&self) -> &[EventStage] {
        &self.stages
    }
    pub fn roots(&self) -> &Arc<ExpressionPlan> {
        &self.roots
    }
    pub fn signals(&self) -> &[SignalRule] {
        &self.signals
    }
}
struct Lower<'a> {
    graph: &'a CompiledPlan,
    input_slots: &'a BTreeMap<String, String>,
    state_slots: &'a BTreeMap<(Vec<String>, String), String>,
    parameters: &'a BoundParameters,
    count: usize,
}
impl Lower<'_> {
    fn budget(&mut self, depth: usize) -> Result<()> {
        self.count += 1;
        if depth > 32 || self.count > 4096 {
            return Err(error(
                "event_limit",
                "$.expressions",
                "expanded expression budget exceeded",
            ));
        }
        Ok(())
    }
    fn source(&mut self, s: &ResolvedSource, depth: usize) -> Result<Expr> {
        self.budget(depth)?;
        match s {
            ResolvedSource::Input(n) => Ok(Expr::Input(self.input_slots[n].clone())),
            ResolvedSource::Output { node, port } => {
                if let Some(slot) = self.state_slots.get(&(node.clone(), port.clone())) {
                    return Ok(Expr::Input(slot.clone()));
                }
                let n = self
                    .graph
                    .nodes()
                    .iter()
                    .find(|n| &n.path == node)
                    .ok_or_else(|| error("event_source", "$.graph", "compiled node unavailable"))?;
                let CompiledCall::Pure(call) = &n.call else {
                    return Err(error("event_source", "$.graph", "state slot unavailable"));
                };
                let call = Arc::clone(call);
                let sources = n.inputs.clone();
                let inputs = sources
                    .iter()
                    .map(|(n, s)| Ok((n.clone(), self.source(s, depth + 1)?)))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Call {
                    call,
                    inputs,
                    output: port.clone(),
                })
            }
        }
    }
    fn expr(&mut self, e: &PortableExpr, depth: usize) -> Result<Expr> {
        self.budget(depth)?;
        Ok(match e {
            PortableExpr::Literal(v) => Expr::Literal(v.clone()),
            PortableExpr::Parameter(n) => match self.parameters.get(n) {
                Some(BoundValue::Value(v)) => Expr::Literal(v.clone()),
                _ => {
                    return Err(error(
                        "event_parameter",
                        "$.expressions",
                        "root parameter must be bound",
                    ));
                }
            },
            PortableExpr::Input(n) => Expr::Input(
                self.input_slots
                    .get(n)
                    .ok_or_else(|| error("event_source", "$.expressions", "unknown graph input"))?
                    .clone(),
            ),
            PortableExpr::Output(name) => {
                let s = self
                    .graph
                    .outputs()
                    .iter()
                    .find(|o| &o.port.name == name)
                    .ok_or_else(|| error("event_source", "$.expressions", "unknown graph output"))?
                    .source
                    .clone();
                return self.source(&s, depth + 1);
            }
            PortableExpr::Cross(n) => Expr::Cross(n.clone()),
            PortableExpr::Not(e) => Expr::Not(Box::new(self.expr(e, depth + 1)?)),
            PortableExpr::Binary { op, left, right } => Expr::Binary {
                op: *op,
                left: Box::new(self.expr(left, depth + 1)?),
                right: Box::new(self.expr(right, depth + 1)?),
            },
        })
    }
}
