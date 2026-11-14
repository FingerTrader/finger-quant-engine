//! Descriptor-driven scalar compiler. No provider-name dispatch or event execution.
use crate::{
    graph::{
        self, Body, GraphBinding, GraphDefinition, GraphNode, LoadedGraph, Module, Source, Target,
        error, port_json,
    },
    loading::{LoadedModel, ModelBinding, typed_json},
    model::{
        Argument, ContentId, EntryKey, EntryRequirement, MAX_EDGES, MAX_NODES, ModelError, Port,
    },
    parameters::{BoundParameters, BoundValue, ParameterInput, ParameterSchema},
    registry::{Descriptor, PreparedCall, PreparedIncremental, Purity, Registry},
};
use fqe_core::types::{Evaluation, TypeDescriptor};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
type Result<T> = std::result::Result<T, ModelError>;
pub const MAX_MODULE_DEPTH: usize = 8;
pub const MAX_EXPANSION_VISITS: usize = 1024;
pub const PLAN_PREFIX: &[u8] = b"FQE-COMPILED-GRAPH/1\n";
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedSource {
    Input(String),
    Output { node: Vec<String>, port: String },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMap {
    pub definition: String,
    pub call_sites: Vec<String>,
}
#[derive(Clone)]
pub enum CompiledCall {
    Pure(Arc<PreparedCall>),
    Incremental(Arc<PreparedIncremental>),
}
impl CompiledCall {
    pub fn permits_checkpoint_replay(&self) -> bool {
        match self {
            Self::Pure(p) => p.permits_checkpoint_replay(),
            Self::Incremental(p) => p.permits_checkpoint_replay(),
        }
    }
    pub fn descriptor(&self) -> &Descriptor {
        match self {
            Self::Pure(p) => p.descriptor(),
            Self::Incremental(p) => p.descriptor(),
        }
    }
    pub fn parameters(&self) -> &BoundParameters {
        match self {
            Self::Pure(p) => p.parameters(),
            Self::Incremental(p) => p.parameters(),
        }
    }
}
#[derive(Clone)]
pub struct CompiledNode {
    pub path: Vec<String>,
    pub inputs: BTreeMap<String, ResolvedSource>,
    pub call: CompiledCall,
    pub source: SourceMap,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledOutput {
    pub port: Port,
    pub source: ResolvedSource,
}
pub struct CompiledPlan {
    parameters: BoundParameters,
    model: ContentId,
    registry: ContentId,
    canonical: Vec<u8>,
    content_id: ContentId,
    inputs: Vec<Port>,
    nodes: Vec<CompiledNode>,
    outputs: Vec<CompiledOutput>,
}
impl CompiledPlan {
    /// Actual immutable top-level bindings used by this compilation (already included in identity).
    pub fn parameters(&self) -> &BoundParameters {
        &self.parameters
    }
    pub fn content_id(&self) -> ContentId {
        self.content_id
    }
    pub fn model_id(&self) -> ContentId {
        self.model
    }
    pub fn registry_id(&self) -> ContentId {
        self.registry
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn inputs(&self) -> &[Port] {
        &self.inputs
    }
    pub fn nodes(&self) -> &[CompiledNode] {
        &self.nodes
    }
    pub fn outputs(&self) -> &[CompiledOutput] {
        &self.outputs
    }
}
struct Context<'a> {
    registry: &'a Registry,
    entries: BTreeMap<EntryKey, &'a EntryRequirement>,
    modules: BTreeMap<EntryKey, &'a Module>,
}
impl<'a> Context<'a> {
    fn provider(&self, key: &EntryKey, path: &str) -> Result<&Descriptor> {
        let r = self.entries.get(key).ok_or_else(|| {
            error(
                "compile_entry",
                path,
                "provider reference lacks an exact requirement",
            )
        })?;
        self.registry.resolve(r).map_err(|mut e| {
            e.path = path.into();
            e
        })
    }
    fn module(&self, key: &EntryKey, path: &str) -> Result<&'a Module> {
        self.modules
            .get(key)
            .copied()
            .ok_or_else(|| error("compile_module", path, "module ID/version is unavailable"))
    }
    fn signature(&self, n: &GraphNode) -> Result<(Vec<Port>, Vec<Port>, &ParameterSchema)> {
        match &n.target {
            Target::Provider(k) => {
                let d = self.provider(k, &n.source_path)?.declaration();
                Ok((
                    d.inputs
                        .iter()
                        .map(|p| Port {
                            name: p.name.clone(),
                            value_type: p.value_type.clone(),
                        })
                        .collect(),
                    d.outputs
                        .iter()
                        .map(|p| Port {
                            name: p.name.clone(),
                            value_type: p.value_type.clone(),
                        })
                        .collect(),
                    &d.parameters,
                ))
            }
            Target::Module(k) => {
                let m = self.module(k, &n.source_path)?;
                Ok((
                    m.graph.inputs.clone(),
                    m.graph.outputs.iter().map(|o| o.port.clone()).collect(),
                    &m.parameters,
                ))
            }
        }
    }
}
fn source_type<'a>(source: &Source, b: &'a Body, path: &str) -> Result<&'a TypeDescriptor> {
    match source {
        Source::Input(n) => b
            .inputs
            .iter()
            .find(|p| &p.name == n)
            .map(|p| &p.value_type),
        Source::Output { node, port } => b
            .nodes
            .iter()
            .find(|n| &n.id == node)
            .and_then(|n| n.outputs.iter().find(|p| &p.name == port))
            .map(|p| &p.value_type),
    }
    .ok_or_else(|| error("compile_source", path, "unknown source input/node/port"))
}
fn validate_body(
    c: &Context<'_>,
    b: &Body,
    parameters: &ParameterSchema,
    pure_module: bool,
) -> Result<()> {
    for n in &b.nodes {
        let (inputs, outputs, schema) = c.signature(n)?;
        if pure_module
            && let Target::Provider(k) = &n.target
            && c.provider(k, &n.source_path)?.declaration().purity != Purity::Pure
        {
            return Err(error(
                "compile_purity",
                &n.source_path,
                "pure modules cannot contain incremental providers",
            ));
        }
        if inputs.len() != n.inputs.len() || outputs != n.outputs {
            return Err(error(
                "compile_signature",
                &n.source_path,
                "declared ports differ from exact descriptor/module signature",
            ));
        }
        for p in inputs {
            let i =
                n.inputs.iter().find(|i| i.name == p.name).ok_or_else(|| {
                    error("compile_signature", &n.source_path, "missing named input")
                })?;
            if source_type(&i.source, b, &n.source_path)? != &p.value_type {
                return Err(error(
                    "compile_type",
                    &n.source_path,
                    "input type/unit/scale differs",
                ));
            }
        }
        for (k, a) in &n.arguments {
            let s = schema.specs().find(|s| s.key() == k).ok_or_else(|| {
                error(
                    "compile_parameter",
                    &n.source_path,
                    "unknown target parameter",
                )
            })?;
            match a {
                Argument::Literal(v) => {
                    ParameterSchema::new(1, vec![s.clone()])
                        .expect("valid subset")
                        .bind(vec![(
                            k.clone(),
                            ParameterInput::Value(Evaluation::Ready(v.clone())),
                        )])
                        .map_err(|_| {
                            error(
                                "compile_parameter",
                                &n.source_path,
                                "literal violates target parameter schema",
                            )
                        })?;
                }
                Argument::Parameter(key) => {
                    let parent = parameters.specs().find(|p| p.key() == key).ok_or_else(|| {
                        error(
                            "compile_parameter",
                            &n.source_path,
                            "unknown enclosing parameter",
                        )
                    })?;
                    if parent.descriptor() != s.descriptor() {
                        return Err(error(
                            "compile_type",
                            &n.source_path,
                            "parameter type/unit/scale differs",
                        ));
                    }
                }
            }
        }
        for s in schema.specs() {
            if s.required() && !n.arguments.contains_key(s.key()) {
                return Err(error(
                    "compile_parameter",
                    &n.source_path,
                    "required target parameter is missing",
                ));
            }
        }
    }
    for o in &b.outputs {
        if source_type(&o.source, b, &b.source_path)? != &o.port.value_type {
            return Err(error(
                "compile_type",
                &b.source_path,
                "graph output type differs from source",
            ));
        }
    }
    topology(b)?;
    Ok(())
}
fn topology(b: &Body) -> Result<Vec<&GraphNode>> {
    let mut remaining: BTreeMap<&str, BTreeSet<&str>> = b
        .nodes
        .iter()
        .map(|n| {
            (
                n.id.as_str(),
                n.inputs
                    .iter()
                    .filter_map(|i| match &i.source {
                        Source::Output { node, .. } => Some(node.as_str()),
                        _ => None,
                    })
                    .collect(),
            )
        })
        .collect();
    let mut order = Vec::new();
    while !remaining.is_empty() {
        let Some(key) = remaining
            .iter()
            .find_map(|(k, v)| v.is_empty().then_some(*k))
        else {
            let k = *remaining.keys().next().expect("nonempty");
            let n = b.nodes.iter().find(|n| n.id == k).expect("known");
            return Err(error(
                "compile_cycle",
                &n.source_path,
                "same-event graph cycle; State/Delay is unsupported",
            ));
        };
        remaining.remove(key);
        for deps in remaining.values_mut() {
            deps.remove(key);
        }
        order.push(b.nodes.iter().find(|n| n.id == key).expect("known"));
    }
    Ok(order)
}
fn module_height(
    key: &EntryKey,
    c: &Context<'_>,
    stack: &mut BTreeSet<EntryKey>,
    memo: &mut BTreeMap<EntryKey, usize>,
) -> Result<usize> {
    if let Some(height) = memo.get(key) {
        return Ok(*height);
    }
    let m = c.module(key, "$.modules")?;
    if !stack.insert(key.clone()) {
        return Err(error(
            "compile_recursion",
            &m.source_path,
            "direct or indirect recursive module",
        ));
    }
    let mut height = 1;
    for n in &m.graph.nodes {
        if let Target::Module(k) = &n.target {
            height = height.max(1 + module_height(k, c, stack, memo)?);
        }
    }
    stack.remove(key);
    if height > MAX_MODULE_DEPTH {
        return Err(error(
            "compile_limit",
            &m.source_path,
            "module nesting exceeds eight levels",
        ));
    }
    memo.insert(key.clone(), height);
    Ok(height)
}
fn values(n: &GraphNode, bound: &BoundParameters) -> Result<Vec<(String, ParameterInput)>> {
    n.arguments
        .iter()
        .map(|(key, a)| {
            let v = match a {
                Argument::Literal(v) => v.clone(),
                Argument::Parameter(p) => match bound.get(p) {
                    Some(BoundValue::Value(v)) => v.clone(),
                    _ => {
                        return Err(error(
                            "compile_unbound",
                            &n.source_path,
                            "referenced parameter is unbound",
                        ));
                    }
                },
            };
            Ok((key.clone(), ParameterInput::Value(Evaluation::Ready(v))))
        })
        .collect()
}
fn resolve(
    s: &Source,
    inputs: &BTreeMap<String, ResolvedSource>,
    nodes: &BTreeMap<String, BTreeMap<String, ResolvedSource>>,
) -> ResolvedSource {
    match s {
        Source::Input(n) => inputs[n].clone(),
        Source::Output { node, port } => nodes[node][port].clone(),
    }
}
struct Expander<'a> {
    context: Context<'a>,
    nodes: Vec<CompiledNode>,
    visits: usize,
    edges: usize,
}
impl Expander<'_> {
    fn expand(
        &mut self,
        b: &Body,
        bound: &BoundParameters,
        inputs: &BTreeMap<String, ResolvedSource>,
        path: &[String],
        sites: &[String],
    ) -> Result<BTreeMap<String, ResolvedSource>> {
        self.edges += b.outputs.len();
        if self.edges > MAX_EDGES {
            return Err(contextual(
                error(
                    "compile_limit",
                    &b.source_path,
                    "expanded output edge limit exceeded",
                ),
                &b.source_path,
                sites,
            ));
        }
        let mut aliases = BTreeMap::new();
        for n in topology(b)? {
            self.visits += 1;
            self.edges += n.inputs.len();
            if self.visits > MAX_EXPANSION_VISITS || self.edges > MAX_EDGES {
                return Err(error(
                    "compile_limit",
                    &n.source_path,
                    "expanded visit/edge limit exceeded",
                ));
            }
            let mut node_path = path.to_vec();
            node_path.push(n.id.clone());
            let wires = n
                .inputs
                .iter()
                .map(|i| (i.name.clone(), resolve(&i.source, inputs, &aliases)))
                .collect();
            let params = values(n, bound).map_err(|e| contextual(e, &n.source_path, sites))?;
            let result = match &n.target {
                Target::Module(k) => {
                    if sites.len() >= MAX_MODULE_DEPTH {
                        return Err(error(
                            "compile_limit",
                            &n.source_path,
                            "module expansion depth exceeded",
                        ));
                    }
                    let m = self.context.module(k, &n.source_path)?;
                    let bound = m
                        .parameters
                        .bind(params)
                        .map_err(|_| {
                            error(
                                "compile_parameter",
                                &n.source_path,
                                "module bindings violate constraints",
                            )
                        })
                        .map_err(|e| contextual(e, &n.source_path, sites))?;
                    let mut next_sites = sites.to_vec();
                    next_sites.push(n.source_path.clone());
                    self.expand(&m.graph, &bound, &wires, &node_path, &next_sites)?
                }
                Target::Provider(k) => {
                    if self.nodes.len() == MAX_NODES {
                        return Err(error(
                            "compile_limit",
                            &n.source_path,
                            "expanded provider node limit exceeded",
                        ));
                    }
                    let requirement = self.context.entries[k];
                    let d = self.context.provider(k, &n.source_path)?;
                    let call = match d.declaration().purity {
                        Purity::Pure => CompiledCall::Pure(Arc::new(
                            self.context
                                .registry
                                .prepare(requirement, params)
                                .map_err(|e| contextual(e, &n.source_path, sites))?,
                        )),
                        Purity::Stateful => CompiledCall::Incremental(Arc::new(
                            self.context
                                .registry
                                .prepare_incremental(requirement, params)
                                .map_err(|e| contextual(e, &n.source_path, sites))?,
                        )),
                    };
                    let outputs = n
                        .outputs
                        .iter()
                        .map(|p| {
                            (
                                p.name.clone(),
                                ResolvedSource::Output {
                                    node: node_path.clone(),
                                    port: p.name.clone(),
                                },
                            )
                        })
                        .collect();
                    self.nodes.push(CompiledNode {
                        path: node_path,
                        inputs: wires,
                        call,
                        source: SourceMap {
                            definition: n.source_path.clone(),
                            call_sites: sites.to_vec(),
                        },
                    });
                    outputs
                }
            };
            aliases.insert(n.id.clone(), result);
        }
        Ok(b.outputs
            .iter()
            .map(|o| (o.port.name.clone(), resolve(&o.source, inputs, &aliases)))
            .collect())
    }
}
fn contextual(mut e: ModelError, path: &str, sites: &[String]) -> ModelError {
    e.path = if sites.is_empty() {
        path.into()
    } else {
        format!("{path} via {}", sites.join(" -> "))
    };
    e
}
fn resolved_json(s: &ResolvedSource) -> Value {
    match s {
        ResolvedSource::Input(n) => json!({"input":n}),
        ResolvedSource::Output { node, port } => json!({"node_path":node,"port":port}),
    }
}
fn parameters_json(p: &BoundParameters) -> Value {
    Value::Object(
        p.iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    match v {
                        BoundValue::Unbound => json!({"unbound":true}),
                        BoundValue::Value(v) => typed_json(v),
                    },
                )
            })
            .collect(),
    )
}
fn order_flat(nodes: Vec<CompiledNode>) -> Result<Vec<CompiledNode>> {
    let mut remaining: BTreeMap<_, _> = nodes.into_iter().map(|n| (n.path.clone(), n)).collect();
    let mut accepted = BTreeSet::new();
    let mut order = Vec::new();
    while !remaining.is_empty() {
        let key = remaining
            .iter()
            .find_map(|(k, n)| {
                n.inputs
                    .values()
                    .all(|s| match s {
                        ResolvedSource::Input(_) => true,
                        ResolvedSource::Output { node, .. } => accepted.contains(node),
                    })
                    .then(|| k.clone())
            })
            .ok_or_else(|| error("compile_cycle", "$.graph", "expanded graph cycle"))?;
        let n = remaining.remove(&key).expect("known");
        accepted.insert(key);
        order.push(n);
    }
    Ok(order)
}
fn compile(
    d: &GraphDefinition,
    model: ContentId,
    format: &str,
    bound: &BoundParameters,
    registry: &Registry,
) -> Result<CompiledPlan> {
    let context = Context {
        registry,
        entries: d.entries.iter().map(|e| (e.key.clone(), e)).collect(),
        modules: d.modules.iter().map(|m| (m.key.clone(), m)).collect(),
    };
    for r in &d.entries {
        registry.resolve(r).map_err(|mut e| {
            e.path = "$.entries".into();
            e
        })?;
    }
    let mut memo = BTreeMap::new();
    for key in context.modules.keys() {
        module_height(key, &context, &mut BTreeSet::new(), &mut memo)?;
    }
    for m in &d.modules {
        validate_body(&context, &m.graph, &m.parameters, true)?;
    }
    validate_body(&context, &d.graph, &d.parameters, false)?;
    let mut expander = Expander {
        context,
        nodes: Vec::new(),
        visits: 0,
        edges: 0,
    };
    let inputs = d
        .graph
        .inputs
        .iter()
        .map(|p| (p.name.clone(), ResolvedSource::Input(p.name.clone())))
        .collect();
    let results = expander.expand(&d.graph, bound, &inputs, &[], &[])?;
    let nodes = order_flat(expander.nodes)?;
    let outputs: Vec<_> = d
        .graph
        .outputs
        .iter()
        .map(|o| CompiledOutput {
            port: o.port.clone(),
            source: results[&o.port.name].clone(),
        })
        .collect();
    let canonical=serde_json::to_vec(&json!({"compiler_version":1,"source_format":format,"model":model.to_string(),"registry":registry.content_id().to_string(),"parameters":parameters_json(bound),"modules":d.modules.iter().map(|m|json!({"id":m.key.id,"version":m.key.version,"content":m.content_id().to_string()})).collect::<Vec<_>>(),"inputs":d.graph.inputs.iter().map(port_json).collect::<Vec<_>>(),"nodes":nodes.iter().map(|n|json!({"path":n.path,"descriptor":n.call.descriptor().content_id().to_string(),"parameters":parameters_json(n.call.parameters()),"inputs":n.inputs.iter().map(|(k,s)|(k.clone(),resolved_json(s))).collect::<BTreeMap<_,_>>() })).collect::<Vec<_>>(),"outputs":outputs.iter().map(|o|json!({"port":port_json(&o.port),"source":resolved_json(&o.source)})).collect::<Vec<_>>() })).map_err(|_|error("compile_encoding","$","plan encoding failed"))?;
    // Bound a returned plan manifest independently of the smaller source artifact.
    if canonical.len() > 1_048_576 {
        return Err(error(
            "compile_limit",
            "$",
            "compiled manifest exceeds 1MiB",
        ));
    }
    let content_id = graph::digest(PLAN_PREFIX, &canonical);
    Ok(CompiledPlan {
        parameters: bound.clone(),
        model,
        registry: registry.content_id(),
        canonical,
        content_id,
        inputs: d.graph.inputs.clone(),
        nodes,
        outputs,
    })
}
pub fn compile_graph(
    graph: &LoadedGraph,
    binding: &GraphBinding,
    registry: &Registry,
) -> Result<CompiledPlan> {
    binding.ensure_matches(graph)?;
    compile(
        graph.definition(),
        graph.content_id(),
        "fqe.graph/1",
        binding.parameters(),
        registry,
    )
}
/// Compile the preserved scalar model schema without reinterpreting its empty future fields.
pub fn compile_model(
    model: &LoadedModel,
    binding: &ModelBinding,
    registry: &Registry,
) -> Result<CompiledPlan> {
    binding.ensure_matches(model)?;
    let d = model.definition().declaration();
    let nodes = d
        .nodes
        .iter()
        .map(|n| GraphNode {
            id: n.id.clone(),
            target: Target::Provider(n.entry.clone()),
            arguments: n.arguments.clone(),
            inputs: n
                .inputs
                .iter()
                .map(|i| graph::Wire {
                    name: i.name.clone(),
                    source: Source::Output {
                        node: i.source.node.clone(),
                        port: i.source.port.clone(),
                    },
                })
                .collect(),
            outputs: n.outputs.clone(),
            source_path: model
                .node_source(&n.id)
                .expect("validated source location")
                .into(),
        })
        .collect();
    let outputs = d
        .outputs
        .iter()
        .map(|o| graph::Output {
            port: Port {
                name: o.name.clone(),
                value_type: d
                    .nodes
                    .iter()
                    .find(|n| n.id == o.source.node)
                    .expect("validated source")
                    .outputs
                    .iter()
                    .find(|p| p.name == o.source.port)
                    .expect("validated port")
                    .value_type
                    .clone(),
            },
            source: Source::Output {
                node: o.source.node.clone(),
                port: o.source.port.clone(),
            },
        })
        .collect();
    let definition = GraphDefinition {
        id: d.id.clone(),
        version: d.version.clone(),
        parameters: d.parameters.clone(),
        requirements: Vec::new(),
        entries: d.entries.clone(),
        graph: Body {
            inputs: vec![],
            nodes,
            outputs,
            source_path: "$.graph".into(),
        },
        modules: vec![],
        metadata: d.metadata.clone(),
    };
    compile(
        &definition,
        model.content_id(),
        "fqe.model/1",
        binding.parameters(),
        registry,
    )
}

impl CompiledPlan {
    /// Optional pure DAG-to-P1-05 AST projection; repeated pure references may repeat calls.
    /// Incremental entries are never copied into pure ASTs. P1-05's AST budgets still apply.
    pub fn pure_expressions(&self) -> Result<crate::expressions::ExpressionPlan> {
        use crate::expressions::{ExpressionDraft, ExpressionPlan};
        if self
            .nodes
            .iter()
            .any(|n| matches!(n.call, CompiledCall::Incremental(_)))
        {
            return Err(error(
                "compile_profile",
                "$.graph",
                "incremental plans need an event runtime, not pure expressions",
            ));
        }
        let mut visits = 0;
        let outputs = self
            .outputs
            .iter()
            .map(|o| {
                Ok((
                    o.port.name.clone(),
                    self.expression(&o.source, 0, &mut visits)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        ExpressionPlan::new(ExpressionDraft {
            inputs: self.inputs.clone(),
            crossings: vec![],
            outputs,
        })
    }
    fn expression(
        &self,
        source: &ResolvedSource,
        depth: usize,
        visits: &mut usize,
    ) -> Result<crate::expressions::Expr> {
        use crate::expressions::{Expr, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES};
        *visits += 1;
        if depth > MAX_EXPRESSION_DEPTH || *visits > MAX_EXPRESSION_NODES {
            return Err(error(
                "compile_limit",
                "$.graph",
                "pure expression projection exceeds AST depth/node budget",
            ));
        }
        match source {
            ResolvedSource::Input(name) => Ok(Expr::Input(name.clone())),
            ResolvedSource::Output { node, port } => {
                let n = self
                    .nodes
                    .iter()
                    .find(|n| &n.path == node)
                    .expect("compiled source");
                let CompiledCall::Pure(call) = &n.call else {
                    return Err(error(
                        "compile_profile",
                        &n.source.definition,
                        "incremental entry is not a pure call",
                    ));
                };
                Ok(Expr::Call {
                    call: Arc::clone(call),
                    inputs: n
                        .inputs
                        .iter()
                        .map(|(k, s)| Ok((k.clone(), self.expression(s, depth + 1, visits)?)))
                        .collect::<Result<Vec<_>>>()?,
                    output: port.clone(),
                })
            }
        }
    }
}
