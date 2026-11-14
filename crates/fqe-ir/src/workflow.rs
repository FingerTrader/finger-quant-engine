//! Bounded stateful scalar rule modules. Persistence/worker protocols are separate capabilities.
use crate::{
    event_plan::{self, CompiledEventPlan, LoadedEventPlan, PortableExpr},
    exchange::ParameterBindings,
    expressions::{Expr, ExpressionDraft, ExpressionPlan},
    graph::{digest, error, schema, schema_json},
    loading::{self, Obj, descriptor, key, mapped, port, text, type_json, typed, typed_json, uint},
    model::{ContentId, EntryKey, ModelError, Port, identifier},
    parameters::{BoundParameters, BoundValue, ParameterInput, ParameterSchema},
    registry::Registry,
};
use fqe_core::{
    SignalKind,
    types::{Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
type Result<T> = std::result::Result<T, ModelError>;
pub const MAX_INSTANCES: usize = 16;
pub const MAX_STATE_VIEW_BYTES: usize = 65536;
pub const CAPABILITIES: &[&str] = &[
    "workflow.scalar/1",
    "workflow.state/1",
    "workflow.delay-one/1",
    "workflow.rules/1",
    "workflow.stop/1",
];

fn check(condition: bool, code: &'static str, path: &str, detail: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(error(code, path, detail))
    }
}
fn name(v: Value, p: &str) -> Result<String> {
    let n = text(v, p)?;
    identifier(&n, p)?;
    check(
        n.len() <= 120,
        "workflow_limit",
        p,
        "local names are limited to120 bytes",
    )?;
    Ok(n)
}
fn named<T>(items: Vec<(String, T)>, p: &str) -> Result<BTreeMap<String, T>> {
    let mut result = BTreeMap::new();
    for (name, v) in items {
        check(
            result.insert(name, v).is_none(),
            "workflow_duplicate",
            p,
            "duplicate declaration",
        )?;
    }
    check(
        result.len() <= 32,
        "workflow_limit",
        p,
        "at most32 declarations",
    )?;
    Ok(result)
}
fn names(v: Value, p: &str) -> Result<Vec<String>> {
    let mut n = mapped(v, p, name)?;
    n.sort();
    check(
        n.len() <= 16 && !n.is_empty() && !n.windows(2).any(|w| w[0] == w[1]),
        "workflow_modes",
        p,
        "one to16 unique mode names required",
    )?;
    Ok(n)
}
pub fn boolean_type() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Bool).expect("Boolean")
}
pub fn boolean(value: bool) -> TypedValue {
    TypedValue::new(boolean_type(), ScalarValue::Bool(value)).expect("Boolean")
}
fn mode_type() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Text { max_bytes: 128 }).expect("mode")
}
pub fn mode_value(value: &str) -> Result<TypedValue> {
    TypedValue::new(mode_type(), ScalarValue::Text(value.into()))
        .map_err(|_| error("workflow_mode", "$.mode", "mode exceeds text type"))
}
fn readiness(v: Value, p: &str) -> Result<Evaluation<TypedValue>> {
    let mut o = Obj::new(v, p)?;
    let result = match text(o.take("status")?, p)?.as_str() {
        "ready" => Evaluation::Ready(typed(o.take("value")?, p)?),
        "missing" => Evaluation::Missing,
        "warmup" => Evaluation::Warmup,
        _ => {
            return Err(error(
                "workflow_readiness",
                p,
                "ready/missing/warmup required",
            ));
        }
    };
    o.end()?;
    Ok(result)
}
pub fn readiness_json(v: &Evaluation<TypedValue>) -> Value {
    match v {
        Evaluation::Ready(v) => json!({"status":"ready","value":typed_json(v)}),
        Evaluation::Missing => json!({"status":"missing"}),
        Evaluation::Warmup => json!({"status":"warmup"}),
        Evaluation::Error(_) => json!({"status":"error"}),
    }
}
struct Cell {
    typ: TypeDescriptor,
    initial: TypedValue,
}
struct Delay {
    typ: TypeDescriptor,
    initial: Evaluation<TypedValue>,
    next: PortableExpr,
}
struct Transition {
    from: Vec<String>,
    to: Option<String>,
    guard: PortableExpr,
    writes: BTreeMap<String, PortableExpr>,
    path: String,
}
struct Module {
    key: EntryKey,
    state_schema: EntryKey,
    parameters: ParameterSchema,
    inputs: Vec<Port>,
    state: BTreeMap<String, Cell>,
    delays: BTreeMap<String, Delay>,
    modes: Vec<String>,
    initial_mode: String,
    transitions: BTreeMap<String, Transition>,
    outputs: BTreeMap<String, PortableExpr>,
    path: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Event(String),
    Instance { instance: String, output: String },
}
struct Instance {
    id: String,
    module: EntryKey,
    parameters: Value,
    bindings: BTreeMap<String, Source>,
    path: String,
}
#[derive(Clone)]
pub struct SignalRule {
    pub instance: String,
    pub output: String,
    pub kind: SignalKind,
}

fn expressions(v: Value, p: &str, count: &mut usize) -> Result<BTreeMap<String, PortableExpr>> {
    let o = Obj::new(v, p)?;
    named(
        o.fields
            .into_iter()
            .map(|(n, v)| {
                name(json!(n), p)?;
                Ok((n, event_plan::expression(v, p, 0, count)?))
            })
            .collect::<Result<_>>()?,
        p,
    )
}
fn module(v: Value, p: &str, count: &mut usize) -> Result<Module> {
    let mut o = Obj::new(v, p)?;
    check(
        uint(o.take("schema_version")?, p)? == 1,
        "workflow_version",
        p,
        "module schema1 required",
    )?;
    let key = key(o.take("key")?, p)?;
    let state_schema = key_checked(o.take("state_schema")?, p)?;
    identifier(&key.id, p)?;
    identifier(&key.version, p)?;
    let parameters = schema(o.take("parameters")?, p)?;
    let mut inputs = mapped(o.take("inputs")?, p, port)?;
    inputs.sort_by(|a, b| a.name.cmp(&b.name));
    for v in &inputs {
        name(json!(v.name), p)?;
    }
    check(
        inputs.len() <= 32 && !inputs.windows(2).any(|w| w[0].name == w[1].name),
        "workflow_duplicate",
        p,
        "invalid input names/count",
    )?;
    let state = named(
        mapped(o.take("state")?, p, |v, p| {
            let mut v = Obj::new(v, p)?;
            let n = name(v.take("name")?, p)?;
            let typ = descriptor(v.take("type")?, p)?;
            let initial = typed(v.take("initial")?, p)?;
            v.end()?;
            check(
                initial.descriptor() == &typ,
                "workflow_type",
                p,
                "state initial type differs",
            )?;
            Ok((n, Cell { typ, initial }))
        })?,
        p,
    )?;
    let delays = named(
        mapped(o.take("delays")?, p, |v, p| {
            let mut v = Obj::new(v, p)?;
            let n = name(v.take("name")?, p)?;
            let typ = descriptor(v.take("type")?, p)?;
            let initial = readiness(v.take("initial")?, p)?;
            if let Evaluation::Ready(value) = &initial {
                check(
                    value.descriptor() == &typ,
                    "workflow_type",
                    p,
                    "delay initial type differs",
                )?;
            }
            let next = event_plan::expression(v.take("next")?, p, 0, count)?;
            v.end()?;
            Ok((n, Delay { typ, initial, next }))
        })?,
        p,
    )?;
    let modes = names(o.take("modes")?, p)?;
    let initial_mode = name(o.take("initial_mode")?, p)?;
    check(
        modes.contains(&initial_mode),
        "workflow_mode",
        p,
        "unknown initial mode",
    )?;
    let transitions = named(
        mapped(o.take("transitions")?, p, |v, p| {
            let mut v = Obj::new(v, p)?;
            let n = name(v.take("id")?, p)?;
            let from = names(v.take("from")?, p)?;
            let to = match v.take("to")? {
                Value::Null => None,
                v => Some(name(v, p)?),
            };
            check(
                from.iter().all(|n| modes.contains(n))
                    && to.as_ref().is_none_or(|n| modes.contains(n)),
                "workflow_mode",
                p,
                "transition references unknown mode",
            )?;
            let guard = event_plan::expression(v.take("guard")?, p, 0, count)?;
            let writes = expressions(v.take("writes")?, p, count)?;
            v.end()?;
            check(
                writes.keys().all(|n| state.contains_key(n)),
                "workflow_write",
                p,
                "writes must target declared local state",
            )?;
            Ok((
                n,
                Transition {
                    from,
                    to,
                    guard,
                    writes,
                    path: p.into(),
                },
            ))
        })?,
        p,
    )?;
    let outputs = expressions(o.take("outputs")?, p, count)?;
    check(
        !outputs.is_empty(),
        "workflow_outputs",
        p,
        "at least one output required",
    )?;
    o.end()?;
    check(
        inputs.len() + state.len() + delays.len() + transitions.len() < 32,
        "workflow_limit",
        p,
        "input/state/delay/fired/mode slots exceed32",
    )?;
    Ok(Module {
        key,
        state_schema,
        parameters,
        inputs,
        state,
        delays,
        modes,
        initial_mode,
        transitions,
        outputs,
        path: p.into(),
    })
}
fn key_checked(v: Value, p: &str) -> Result<EntryKey> {
    let k = key(v, p)?;
    identifier(&k.id, p)?;
    identifier(&k.version, p)?;
    Ok(k)
}
pub(crate) fn source(v: Value, p: &str) -> Result<Source> {
    let mut o = Obj::new(v, p)?;
    let s = if let Some(v) = o.optional("event") {
        Source::Event(name(v, p)?)
    } else {
        Source::Instance {
            instance: name(o.take("instance")?, p)?,
            output: name(o.take("output")?, p)?,
        }
    };
    o.end()?;
    Ok(s)
}
pub(crate) fn source_json(s: &Source) -> Value {
    match s {
        Source::Event(n) => json!({"event":n}),
        Source::Instance { instance, output } => json!({"instance":instance,"output":output}),
    }
}
fn module_json(m: &Module) -> Value {
    json!({"schema_version":1,"key":{"id":m.key.id,"version":m.key.version},"state_schema":{"id":m.state_schema.id,"version":m.state_schema.version},
        "parameters":schema_json(&m.parameters),"inputs":m.inputs.iter().map(crate::graph::port_json).collect::<Vec<_>>(),
        "state":m.state.iter().map(|(n,c)|json!({"name":n,"type":type_json(&c.typ),"initial":typed_json(&c.initial)})).collect::<Vec<_>>(),
        "delays":m.delays.iter().map(|(n,c)|json!({"name":n,"type":type_json(&c.typ),"initial":readiness_json(&c.initial),"next":event_plan::expr_json(&c.next)})).collect::<Vec<_>>(),
        "modes":m.modes,"initial_mode":m.initial_mode,
        "transitions":m.transitions.iter().map(|(n,t)|json!({"id":n,"from":t.from,"to":t.to,"guard":event_plan::expr_json(&t.guard),"writes":t.writes.iter().map(|(n,e)|(n,event_plan::expr_json(e))).collect::<BTreeMap<_,_>>()})).collect::<Vec<_>>(),
        "outputs":m.outputs.iter().map(|(n,e)|(n,event_plan::expr_json(e))).collect::<BTreeMap<_,_>>()})
}
pub struct LoadedWorkflow {
    canonical: Vec<u8>,
    source: ContentId,
    id: ContentId,
    event: LoadedEventPlan,
    modules: BTreeMap<EntryKey, Module>,
    instances: BTreeMap<String, Instance>,
    signals: Vec<SignalRule>,
}
impl LoadedWorkflow {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut o = Obj::new(loading::decode(bytes)?, "$")?;
        check(
            text(o.take("format")?, "$")? == "fqe.workflow"
                && uint(o.take("schema_version")?, "$")? == 1,
            "workflow_version",
            "$",
            "fqe.workflow/1 required",
        )?;
        let id = name(o.take("id")?, "$.id")?;
        let version = name(o.take("version")?, "$.version")?;
        let mut requirements = mapped(o.take("requirements")?, "$.requirements", text)?;
        requirements.sort();
        check(
            requirements.iter().any(|r| r == "workflow.scalar/1")
                && requirements
                    .iter()
                    .all(|r| CAPABILITIES.contains(&r.as_str()))
                && !requirements.windows(2).any(|w| w[0] == w[1]),
            "workflow_capability",
            "$.requirements",
            "unsupported/missing/duplicate capability",
        )?;
        let event = LoadedEventPlan::from_bytes(
            &serde_json::to_vec(&o.take("event_plan")?)
                .map_err(|_| error("workflow_encoding", "$", "encoding failed"))?,
        )?;
        let mut count = 0;
        let definitions = mapped(o.take("modules")?, "$.modules", |v, p| {
            module(v, p, &mut count)
        })?;
        check(
            !definitions.is_empty() && definitions.len() <= 16,
            "workflow_limit",
            "$.modules",
            "one to16 modules required",
        )?;
        let mut modules = BTreeMap::new();
        for m in definitions {
            check(
                modules.insert(m.key.clone(), m).is_none(),
                "workflow_duplicate",
                "$.modules",
                "duplicate module key",
            )?;
        }
        let instances = named(
            mapped(o.take("instances")?, "$.instances", |v, p| {
                let mut v = Obj::new(v, p)?;
                let id = name(v.take("id")?, p)?;
                let module = key_checked(v.take("module")?, p)?;
                let parameters = v.take("parameters")?;
                ParameterBindings::from_bytes(
                    &serde_json::to_vec(&parameters)
                        .map_err(|_| error("workflow_encoding", p, "encoding failed"))?,
                )?;
                let fields = Obj::new(v.take("bindings")?, p)?;
                let bindings = named(
                    fields
                        .fields
                        .into_iter()
                        .map(|(n, v)| {
                            name(json!(n), p)?;
                            Ok((n, source(v, p)?))
                        })
                        .collect::<Result<_>>()?,
                    p,
                )?;
                v.end()?;
                Ok((
                    id.clone(),
                    Instance {
                        id,
                        module,
                        parameters,
                        bindings,
                        path: p.into(),
                    },
                ))
            })?,
            "$.instances",
        )?;
        check(
            !instances.is_empty() && instances.len() <= MAX_INSTANCES,
            "workflow_limit",
            "$.instances",
            "one to16 instances required",
        )?;
        let mut signals = mapped(o.take("signals")?, "$.signals", |v, p| {
            let mut v = Obj::new(v, p)?;
            let instance = name(v.take("instance")?, p)?;
            let output = name(v.take("output")?, p)?;
            let kind = match text(v.take("kind")?, p)?.as_str() {
                "long" => SignalKind::Long,
                "flat" => SignalKind::Flat,
                _ => return Err(error("workflow_signal", p, "unsupported signal")),
            };
            v.end()?;
            Ok(SignalRule {
                instance,
                output,
                kind,
            })
        })?;
        signals.sort_by(|a, b| (&a.instance, &a.output).cmp(&(&b.instance, &b.output)));
        check(
            signals.len() <= 32
                && !signals
                    .windows(2)
                    .any(|w| w[0].instance == w[1].instance && w[0].output == w[1].output),
            "workflow_signal",
            "$.signals",
            "duplicate/too many signal rules",
        )?;
        o.end()?;
        let canonical=serde_json::to_vec(&json!({"format":"fqe.workflow","schema_version":1,"id":id,"version":version,"requirements":requirements,
            "event_plan":serde_json::from_slice::<Value>(event.canonical_bytes()).expect("canonical event"),
            "modules":modules.values().map(module_json).collect::<Vec<_>>(),
            "instances":instances.values().map(|i|json!({"id":i.id,"module":{"id":i.module.id,"version":i.module.version},"parameters":i.parameters,"bindings":i.bindings.iter().map(|(n,s)|(n,source_json(s))).collect::<BTreeMap<_,_>>() })).collect::<Vec<_>>(),
            "signals":signals.iter().map(|s|json!({"instance":s.instance,"output":s.output,"kind":if s.kind==SignalKind::Long{"long"}else{"flat"}})).collect::<Vec<_>>()
        })).map_err(|_|error("workflow_encoding","$","canonical encoding failed"))?;
        check(
            canonical.len() <= crate::model::MAX_MODEL_BYTES,
            "workflow_limit",
            "$",
            "canonical document too large",
        )?;
        Ok(Self {
            source: digest(b"", bytes),
            id: digest(b"FQE-WORKFLOW-CANONICAL/1\n", &canonical),
            canonical,
            event,
            modules,
            instances,
            signals,
        })
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn content_id(&self) -> ContentId {
        self.id
    }
    pub fn source_id(&self) -> ContentId {
        self.source
    }
    pub fn event_plan(&self) -> &LoadedEventPlan {
        &self.event
    }
    pub fn compile(
        &self,
        registry: &Registry,
        event_parameters: Vec<(String, ParameterInput)>,
    ) -> Result<CompiledWorkflow> {
        let event = Arc::new(self.event.compile(registry, event_parameters)?);
        check(
            event.signals().is_empty(),
            "workflow_event_signals",
            "$.event_plan.signals",
            "workflow owns signals; embedded event rules must be empty",
        )?;
        let mut prepared = BTreeMap::new();
        let mut used = BTreeSet::new();
        let mut budget = 0;
        for (id, i) in &self.instances {
            let m = self
                .modules
                .get(&i.module)
                .ok_or_else(|| error("workflow_module", &i.path, "unknown exact module key"))?;
            used.insert(i.module.clone());
            let parameters = ParameterBindings::from_bytes(
                &serde_json::to_vec(&i.parameters).expect("bounded bindings"),
            )?;
            let bound = m
                .parameters
                .bind(parameters.into_values())
                .map_err(|_| error("workflow_parameter", &i.path, "module parameters invalid"))?;
            prepared.insert(id.clone(), prepare(m, i, &bound, &mut budget)?);
        }
        check(
            used.len() == self.modules.len(),
            "workflow_module",
            "$.modules",
            "unused modules unsupported in this profile",
        )?;
        let event_types: BTreeMap<_, _> = event
            .roots()
            .output_types()
            .iter()
            .map(|p| (p.name.clone(), p.value_type.clone()))
            .collect();
        let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
        for (id, i) in &prepared {
            let mut deps = BTreeSet::new();
            for (n, source) in &i.bindings {
                let typ = match source {
                    Source::Event(n) => event_types.get(n),
                    Source::Instance { instance, output } => {
                        deps.insert(instance.clone());
                        prepared.get(instance).and_then(|i| {
                            i.outputs
                                .output_types()
                                .iter()
                                .find(|p| &p.name == output)
                                .map(|p| &p.value_type)
                        })
                    }
                }
                .ok_or_else(|| error("workflow_source", &i.path, "unknown binding source"))?;
                check(
                    &i.input_types[n] == typ,
                    "workflow_type",
                    &i.path,
                    "binding type/unit differs",
                )?;
            }
            dependencies.insert(id.clone(), deps);
        }
        let mut order = Vec::new();
        let mut done = BTreeSet::new();
        while !dependencies.is_empty() {
            let id = dependencies
                .iter()
                .find(|(_, deps)| deps.is_subset(&done))
                .map(|(n, _)| n.clone())
                .ok_or_else(|| {
                    error("workflow_cycle", "$.instances", "same-event instance cycle")
                })?;
            dependencies.remove(&id);
            done.insert(id.clone());
            order.push(prepared.remove(&id).expect("prepared instance"));
        }
        for s in &self.signals {
            let typ = order
                .iter()
                .find(|i| i.id == s.instance)
                .and_then(|i| i.outputs.output_types().iter().find(|p| p.name == s.output))
                .map(|p| &p.value_type);
            check(
                typ == Some(&boolean_type()),
                "workflow_signal",
                "$.signals",
                "signal source must be a Boolean output",
            )?;
        }
        let execution = digest(
            b"FQE-WORKFLOW-EXECUTION/1\n",
            format!("{}\n{}", self.id, event.content_id()).as_bytes(),
        );
        let values = order
            .iter()
            .map(|i| (i.id.clone(), i.initial.clone()))
            .collect();
        state_view(execution, "compile", &values)?;
        Ok(CompiledWorkflow {
            event,
            instances: order,
            signals: self.signals.clone(),
            artifact: self.id,
            source: self.source,
            execution,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceValues {
    pub module: EntryKey,
    pub state_schema: EntryKey,
    pub mode: String,
    pub state: BTreeMap<String, TypedValue>,
    pub delays: BTreeMap<String, Evaluation<TypedValue>>,
}
pub struct PreparedTransition {
    pub id: String,
    pub from: Vec<String>,
    pub to: Option<String>,
    pub guard: Arc<ExpressionPlan>,
    pub writes: Option<Arc<ExpressionPlan>>,
    pub path: String,
}
pub struct PreparedInstance {
    pub parameters: BoundParameters,
    pub id: String,
    pub module: EntryKey,
    pub state_schema: EntryKey,
    pub initial: InstanceValues,
    pub input_types: BTreeMap<String, TypeDescriptor>,
    pub bindings: BTreeMap<String, Source>,
    pub slots: Vec<Port>,
    pub transitions: Vec<PreparedTransition>,
    pub delays: Option<Arc<ExpressionPlan>>,
    pub outputs: Arc<ExpressionPlan>,
    pub path: String,
}
pub struct CompiledWorkflow {
    event: Arc<CompiledEventPlan>,
    instances: Vec<PreparedInstance>,
    signals: Vec<SignalRule>,
    artifact: ContentId,
    source: ContentId,
    execution: ContentId,
}
impl CompiledWorkflow {
    pub fn event(&self) -> &Arc<CompiledEventPlan> {
        &self.event
    }
    pub fn instances(&self) -> &[PreparedInstance] {
        &self.instances
    }
    pub fn signals(&self) -> &[SignalRule] {
        &self.signals
    }
    pub fn content_id(&self) -> ContentId {
        self.execution
    }
    pub fn artifact_id(&self) -> ContentId {
        self.artifact
    }
    pub fn source_id(&self) -> ContentId {
        self.source
    }
}
fn lower(
    e: &PortableExpr,
    bound: &BoundParameters,
    fired: bool,
    budget: &mut usize,
    p: &str,
) -> Result<Expr> {
    *budget += 1;
    check(
        *budget <= 4096,
        "workflow_limit",
        p,
        "lowered expressions exceed4096 nodes",
    )?;
    Ok(match e {
        PortableExpr::Literal(v) => Expr::Literal(v.clone()),
        PortableExpr::Parameter(n) => match bound.get(n) {
            Some(BoundValue::Value(v)) => Expr::Literal(v.clone()),
            _ => {
                return Err(error(
                    "workflow_parameter",
                    p,
                    "unbound expression parameter",
                ));
            }
        },
        PortableExpr::Input(n) => {
            check(
                fired || !n.starts_with("fired."),
                "workflow_phase",
                p,
                "guards cannot read transition outcomes",
            )?;
            Expr::Input(n.clone())
        }
        PortableExpr::Not(e) => Expr::Not(Box::new(lower(e, bound, fired, budget, p)?)),
        PortableExpr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(lower(left, bound, fired, budget, p)?),
            right: Box::new(lower(right, bound, fired, budget, p)?),
        },
        _ => {
            return Err(error(
                "workflow_expression",
                p,
                "only literal/parameter/input/not/binary allowed; history belongs to event plan",
            ));
        }
    })
}
fn expression_plan<'a>(
    expressions: impl IntoIterator<Item = (&'a String, &'a PortableExpr)>,
    slots: &[Port],
    bound: &BoundParameters,
    fired: bool,
    budget: &mut usize,
    p: &str,
) -> Result<Option<Arc<ExpressionPlan>>> {
    let outputs = expressions
        .into_iter()
        .map(|(n, e)| Ok((n.clone(), lower(e, bound, fired, budget, p)?)))
        .collect::<Result<Vec<_>>>()?;
    if outputs.is_empty() {
        return Ok(None);
    }
    ExpressionPlan::new(ExpressionDraft {
        inputs: slots.to_vec(),
        crossings: vec![],
        outputs,
    })
    .map(Arc::new)
    .map(Some)
    .map_err(|e| error(e.code, p, &e.detail))
}
fn prepare(
    m: &Module,
    i: &Instance,
    bound: &BoundParameters,
    budget: &mut usize,
) -> Result<PreparedInstance> {
    let p = format!("{} via {}", m.path, i.path);
    let mut slots = Vec::new();
    for port in &m.inputs {
        slots.push(Port {
            name: format!("input.{}", port.name),
            value_type: port.value_type.clone(),
        });
    }
    for (n, c) in &m.state {
        slots.push(Port {
            name: format!("state.{n}"),
            value_type: c.typ.clone(),
        });
    }
    for (n, c) in &m.delays {
        slots.push(Port {
            name: format!("delay.{n}"),
            value_type: c.typ.clone(),
        });
    }
    for n in m.transitions.keys() {
        slots.push(Port {
            name: format!("fired.{n}"),
            value_type: boolean_type(),
        });
    }
    slots.push(Port {
        name: "mode".into(),
        value_type: mode_type(),
    });
    let input_types: BTreeMap<_, _> = m
        .inputs
        .iter()
        .map(|p| (p.name.clone(), p.value_type.clone()))
        .collect();
    check(
        input_types.keys().eq(i.bindings.keys()),
        "workflow_binding",
        &i.path,
        "exact named input bindings required",
    )?;
    let mut transitions = Vec::new();
    for (id, t) in &m.transitions {
        let p = format!("{} via {}", t.path, i.path);
        let guard = expression_plan(
            [(&"guard".into(), &t.guard)],
            &slots,
            bound,
            false,
            budget,
            &p,
        )?
        .expect("one guard");
        check(
            guard.output_types()[0].value_type == boolean_type(),
            "workflow_type",
            &p,
            "guard must be Boolean",
        )?;
        let writes = expression_plan(t.writes.iter(), &slots, bound, true, budget, &p)?;
        if let Some(writes) = &writes {
            for port in writes.output_types() {
                check(
                    port.value_type == m.state[&port.name].typ,
                    "workflow_type",
                    &p,
                    "state write type/unit differs",
                )?;
            }
        }
        transitions.push(PreparedTransition {
            id: id.clone(),
            from: t.from.clone(),
            to: t.to.clone(),
            guard,
            writes,
            path: p,
        });
    }
    let next: BTreeMap<_, _> = m
        .delays
        .iter()
        .map(|(n, d)| (n.clone(), d.next.clone()))
        .collect();
    let delays = expression_plan(next.iter(), &slots, bound, true, budget, &p)?;
    if let Some(delays) = &delays {
        for port in delays.output_types() {
            check(
                port.value_type == m.delays[&port.name].typ,
                "workflow_type",
                &p,
                "delay next type differs",
            )?;
        }
    }
    let outputs = expression_plan(m.outputs.iter(), &slots, bound, true, budget, &p)?
        .expect("nonempty outputs");
    Ok(PreparedInstance {
        parameters: bound.clone(),
        id: i.id.clone(),
        module: m.key.clone(),
        state_schema: m.state_schema.clone(),
        initial: InstanceValues {
            module: m.key.clone(),
            state_schema: m.state_schema.clone(),
            mode: m.initial_mode.clone(),
            state: m
                .state
                .iter()
                .map(|(n, c)| (n.clone(), c.initial.clone()))
                .collect(),
            delays: m
                .delays
                .iter()
                .map(|(n, c)| (n.clone(), c.initial.clone()))
                .collect(),
        },
        input_types,
        bindings: i.bindings.clone(),
        slots,
        transitions,
        delays,
        outputs,
        path: p,
    })
}
/// Portable typed workflow-cell inspection, explicitly NOT a complete restorable checkpoint.
pub fn state_view(
    execution: ContentId,
    run: &str,
    values: &BTreeMap<String, InstanceValues>,
) -> Result<Vec<u8>> {
    identifier(run, "$.run")?;
    let result=serde_json::to_vec(&json!({"format":"fqe.workflow-state-view","schema_version":1,"restorable":false,"execution_id":execution.to_string(),"run":run,
        "instances":values.iter().map(|(n,v)|(n,json!({"module":{"id":v.module.id,"version":v.module.version},"state_schema":{"id":v.state_schema.id,"version":v.state_schema.version},"mode":v.mode,"state":v.state.iter().map(|(n,v)|(n,typed_json(v))).collect::<BTreeMap<_,_>>(),"delays":v.delays.iter().map(|(n,v)|(n,readiness_json(v))).collect::<BTreeMap<_,_>>() }))).collect::<BTreeMap<_,_>>()
    })).map_err(|_|error("workflow_encoding","$.state","state view encoding failed"))?;
    check(
        result.len() <= MAX_STATE_VIEW_BYTES,
        "workflow_limit",
        "$.state",
        "workflow cell state exceeds64KiB",
    )?;
    Ok(result)
}
