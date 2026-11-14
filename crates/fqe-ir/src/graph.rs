//! Explicit fqe.graph/1 artifact; legacy fqe.model/1 remains unchanged.
use crate::{
    loading::{
        self, Obj, argument, entry, key, mapped, parameter, parameter_json, port, text, type_json,
        typed_json, uint,
    },
    model::{
        Argument, ContentId, EntryKey, EntryRequirement, MAX_EDGES, MAX_MODEL_BYTES, MAX_NODES,
        MAX_PORTS, ModelError, Port, identifier,
    },
    parameters::{BoundParameters, ParameterInput, ParameterSchema},
};
use fqe_core::types::Evaluation;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};
type Result<T> = std::result::Result<T, ModelError>;
pub const GRAPH_PREFIX: &[u8] = b"FQE-GRAPH-CANONICAL/1\n";
pub const MODULE_PREFIX: &[u8] = b"FQE-PURE-MODULE-CANONICAL/1\n";
pub const MAX_MODULES: usize = 32;
pub(crate) fn error(code: &'static str, path: &str, detail: &str) -> ModelError {
    ModelError::new(code, path, detail)
}
pub(crate) fn digest(prefix: &[u8], bytes: &[u8]) -> ContentId {
    let mut h = Sha256::new();
    h.update(prefix);
    h.update(bytes);
    ContentId::from_bytes(h.finalize().into())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Input(String),
    Output { node: String, port: String },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wire {
    pub name: String,
    pub source: Source,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub port: Port,
    pub source: Source,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Provider(EntryKey),
    Module(EntryKey),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphNode {
    pub id: String,
    pub target: Target,
    pub arguments: BTreeMap<String, Argument>,
    pub inputs: Vec<Wire>,
    pub outputs: Vec<Port>,
    /// Original syntax location; excluded from canonical identity.
    pub source_path: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body {
    pub inputs: Vec<Port>,
    pub nodes: Vec<GraphNode>,
    pub outputs: Vec<Output>,
    pub source_path: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub key: EntryKey,
    pub parameters: ParameterSchema,
    pub graph: Body,
    pub source_path: String,
}
impl Module {
    pub fn content_id(&self) -> ContentId {
        digest(
            MODULE_PREFIX,
            &serde_json::to_vec(&module_json(self)).expect("validated JSON"),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphDefinition {
    pub id: String,
    pub version: String,
    pub parameters: ParameterSchema,
    pub requirements: Vec<String>,
    pub entries: Vec<EntryRequirement>,
    pub graph: Body,
    pub modules: Vec<Module>,
    pub metadata: BTreeMap<String, String>,
}
pub(crate) fn schema(v: Value, p: &str) -> Result<ParameterSchema> {
    let mut o = Obj::new(v, p)?;
    if uint(o.take("version")?, p)? != 1 {
        return Err(error("graph_version", p, "unsupported parameter version"));
    }
    let entries = mapped(o.take("entries")?, &format!("{p}.entries"), parameter)?;
    o.end()?;
    ParameterSchema::new(1, entries)
        .map_err(|_| error("graph_parameter", p, "invalid parameter schema"))
}
fn source(v: Value, p: &str) -> Result<Source> {
    let mut o = Obj::new(v, p)?;
    let s = if let Some(v) = o.optional("input") {
        Source::Input(text(v, p)?)
    } else {
        Source::Output {
            node: text(o.take("node")?, p)?,
            port: text(o.take("port")?, p)?,
        }
    };
    o.end()?;
    match &s {
        Source::Input(n) => identifier(n, p)?,
        Source::Output { node, port } => {
            identifier(node, p)?;
            identifier(port, p)?;
        }
    };
    Ok(s)
}
fn wire(v: Value, p: &str) -> Result<Wire> {
    let mut o = Obj::new(v, p)?;
    let w = Wire {
        name: text(o.take("name")?, p)?,
        source: source(o.take("source")?, p)?,
    };
    o.end()?;
    Ok(w)
}
fn output(v: Value, p: &str) -> Result<Output> {
    let mut o = Obj::new(v, p)?;
    let port = Port {
        name: text(o.take("name")?, p)?,
        value_type: loading::descriptor(o.take("type")?, p)?,
    };
    let source = source(o.take("source")?, p)?;
    o.end()?;
    Ok(Output { port, source })
}
fn node(v: Value, p: &str) -> Result<GraphNode> {
    let mut o = Obj::new(v, p)?;
    let id = text(o.take("id")?, p)?;
    let mut t = Obj::new(o.take("target")?, p)?;
    let kind = text(t.take("kind")?, p)?;
    let k = key(t.take("key")?, p)?;
    t.end()?;
    identifier(&k.id, p)?;
    identifier(&k.version, p)?;
    let target = match kind.as_str() {
        "provider" => Target::Provider(k),
        "module" => Target::Module(k),
        _ => return Err(error("graph_target", p, "unsupported node target kind")),
    };
    let args = Obj::new(o.take("arguments")?, p)?;
    let mut arguments = BTreeMap::new();
    for (k, v) in args.fields {
        identifier(&k, p)?;
        arguments.insert(k, argument(v, &format!("{p}.arguments"))?);
    }
    let inputs = mapped(o.take("inputs")?, &format!("{p}.inputs"), wire)?;
    let outputs = mapped(o.take("outputs")?, &format!("{p}.outputs"), port)?;
    o.end()?;
    Ok(GraphNode {
        id,
        target,
        arguments,
        inputs,
        outputs,
        source_path: p.into(),
    })
}
fn names<'a>(it: impl Iterator<Item = &'a str>, p: &str) -> Result<()> {
    let mut names = BTreeSet::new();
    for n in it {
        identifier(n, p)?;
        if !names.insert(n) {
            return Err(error("graph_duplicate", p, "duplicate named declaration"));
        }
    }
    Ok(())
}
fn body(v: Value, p: &str) -> Result<Body> {
    let mut o = Obj::new(v, p)?;
    let mut b = Body {
        inputs: mapped(o.take("inputs")?, p, port)?,
        nodes: mapped(o.take("nodes")?, &format!("{p}.nodes"), node)?,
        outputs: mapped(o.take("outputs")?, p, output)?,
        source_path: p.into(),
    };
    o.end()?;
    if b.inputs.len() > MAX_PORTS
        || b.outputs.is_empty()
        || b.outputs.len() > MAX_PORTS
        || b.nodes.len() > MAX_NODES
    {
        return Err(error("graph_limit", p, "invalid graph collection size"));
    }
    names(b.inputs.iter().map(|p| p.name.as_str()), p)?;
    names(b.nodes.iter().map(|n| n.id.as_str()), p)?;
    names(b.outputs.iter().map(|p| p.port.name.as_str()), p)?;
    for n in &mut b.nodes {
        if n.inputs.len() > MAX_PORTS
            || n.outputs.is_empty()
            || n.outputs.len() > MAX_PORTS
            || n.arguments.len() > 256
        {
            return Err(error(
                "graph_limit",
                &n.source_path,
                "invalid node collection size",
            ));
        }
        names(n.inputs.iter().map(|p| p.name.as_str()), &n.source_path)?;
        names(n.outputs.iter().map(|p| p.name.as_str()), &n.source_path)?;
        n.inputs.sort_by(|a, b| a.name.cmp(&b.name));
        n.outputs.sort_by(|a, b| a.name.cmp(&b.name));
    }
    b.inputs.sort_by(|a, b| a.name.cmp(&b.name));
    b.nodes.sort_by(|a, b| a.id.cmp(&b.id));
    b.outputs.sort_by(|a, b| a.port.name.cmp(&b.port.name));
    Ok(b)
}
fn module(v: Value, p: &str) -> Result<Module> {
    let mut o = Obj::new(v, p)?;
    if uint(o.take("schema_version")?, p)? != 1 {
        return Err(error("graph_version", p, "unsupported module schema"));
    }
    let key = key(o.take("key")?, p)?;
    identifier(&key.id, p)?;
    identifier(&key.version, p)?;
    if !key
        .id
        .split_once(['.', ':'])
        .is_some_and(|(namespace, name)| !namespace.is_empty() && !name.is_empty())
    {
        return Err(error(
            "graph_identifier",
            p,
            "module IDs must be namespaced",
        ));
    }
    let parameters = schema(o.take("parameters")?, p)?;
    let graph = body(o.take("graph")?, &format!("{p}.graph"))?;
    o.end()?;
    Ok(Module {
        key,
        parameters,
        graph,
        source_path: p.into(),
    })
}
fn definition(v: Value) -> Result<GraphDefinition> {
    let mut o = Obj::new(v, "$")?;
    if text(o.take("format")?, "$.format")? != "fqe.graph"
        || uint(o.take("schema_version")?, "$")? != 1
        || uint(o.take("ir_version")?, "$")? != 2
    {
        return Err(error(
            "graph_version",
            "$",
            "expected fqe.graph schema1/IR2",
        ));
    }
    let id = text(o.take("id")?, "$.id")?;
    let version = text(o.take("version")?, "$.version")?;
    identifier(&id, "$.id")?;
    identifier(&version, "$.version")?;
    let parameters = schema(o.take("parameters")?, "$.parameters")?;
    let mut requirements = mapped(o.take("requirements")?, "$.requirements", text)?;
    names(requirements.iter().map(String::as_str), "$.requirements")?;
    if !requirements.iter().any(|x| x == "model.typed-graph/1")
        || requirements.iter().any(|x| {
            ![
                "model.typed-graph/1",
                "model.pure-modules/1",
                "values/1",
                "parameters/1",
            ]
            .contains(&x.as_str())
        })
    {
        return Err(error(
            "graph_capability",
            "$.requirements",
            "missing or unsupported graph capability",
        ));
    }
    let mut entries = mapped(o.take("entries")?, "$.entries", entry)?;
    let mut keys = BTreeSet::new();
    if entries.len() > MAX_NODES {
        return Err(error("graph_limit", "$.entries", "too many requirements"));
    }
    for e in &entries {
        identifier(&e.key.id, "$.entries")?;
        identifier(&e.key.version, "$.entries")?;
        if !keys.insert(e.key.clone()) {
            return Err(error(
                "graph_duplicate",
                "$.entries",
                "duplicate entry requirement",
            ));
        }
        if e.descriptor_version != 1 || e.role != "provider" {
            return Err(error(
                "graph_version",
                "$.entries",
                "unsupported entry role/version",
            ));
        }
    }
    let graph = body(o.take("graph")?, "$.graph")?;
    let mut modules = mapped(o.take("modules")?, "$.modules", module)?;
    if modules.len() > MAX_MODULES {
        return Err(error("graph_limit", "$.modules", "too many modules"));
    }
    if (!modules.is_empty()
        || graph
            .nodes
            .iter()
            .any(|n| matches!(n.target, Target::Module(_))))
        && !requirements.iter().any(|x| x == "model.pure-modules/1")
    {
        return Err(error(
            "graph_capability",
            "$.requirements",
            "module use requires model.pure-modules/1",
        ));
    }
    let mut module_keys = BTreeSet::new();
    for m in &modules {
        if !module_keys.insert(m.key.clone()) {
            return Err(error(
                "graph_duplicate",
                &m.source_path,
                "duplicate module ID/version",
            ));
        }
    }
    let nodes = graph.nodes.len() + modules.iter().map(|m| m.graph.nodes.len()).sum::<usize>();
    let edges = std::iter::once(&graph)
        .chain(modules.iter().map(|m| &m.graph))
        .map(|b| b.nodes.iter().map(|n| n.inputs.len()).sum::<usize>() + b.outputs.len())
        .sum::<usize>();
    if nodes > MAX_NODES || edges > MAX_EDGES {
        return Err(error(
            "graph_limit",
            "$",
            "aggregate declaration limit exceeded",
        ));
    }
    let meta = Obj::new(o.take("metadata")?, "$.metadata")?;
    let mut metadata = BTreeMap::new();
    if meta.fields.len() > 32 {
        return Err(error(
            "graph_limit",
            "$.metadata",
            "too many metadata fields",
        ));
    }
    for (k, v) in meta.fields {
        identifier(&k, "$.metadata")?;
        let v = text(v, "$.metadata")?;
        if v.len() > 4096 {
            return Err(error("graph_limit", "$.metadata", "metadata too long"));
        }
        metadata.insert(k, v);
    }
    for (field, expected) in [
        ("workflow", json!({})),
        ("state", json!([])),
        ("dependencies", json!([])),
    ] {
        if o.take(field)? != expected {
            return Err(error(
                "graph_unsupported",
                "$",
                "workflow/state/external dependencies remain unsupported",
            ));
        }
    }
    o.end()?;
    requirements.sort();
    entries.sort_by(|a, b| a.key.cmp(&b.key));
    modules.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(GraphDefinition {
        id,
        version,
        parameters,
        requirements,
        entries,
        graph,
        modules,
        metadata,
    })
}
pub(crate) fn source_json(s: &Source) -> Value {
    match s {
        Source::Input(n) => json!({"input":n}),
        Source::Output { node, port } => json!({"node":node,"port":port}),
    }
}
pub(crate) fn port_json(p: &Port) -> Value {
    json!({"name":p.name,"type":type_json(&p.value_type)})
}
fn key_json(k: &EntryKey) -> Value {
    json!({"id":k.id,"version":k.version})
}
pub(crate) fn schema_json(p: &ParameterSchema) -> Value {
    json!({"version":1,"entries":p.specs().map(parameter_json).collect::<Vec<_>>()})
}
fn body_json(b: &Body) -> Value {
    json!({"inputs":b.inputs.iter().map(port_json).collect::<Vec<_>>(),"nodes":b.nodes.iter().map(|n|json!({"id":n.id,"target":match &n.target{Target::Provider(k)=>json!({"kind":"provider","key":key_json(k)}),Target::Module(k)=>json!({"kind":"module","key":key_json(k)})},"arguments":n.arguments.iter().map(|(k,a)|(k.clone(),match a{Argument::Literal(v)=>json!({"literal":typed_json(v)}),Argument::Parameter(p)=>json!({"parameter":p})})).collect::<BTreeMap<_,_>>(),"inputs":n.inputs.iter().map(|i|json!({"name":i.name,"source":source_json(&i.source)})).collect::<Vec<_>>(),"outputs":n.outputs.iter().map(port_json).collect::<Vec<_>>() })).collect::<Vec<_>>(),"outputs":b.outputs.iter().map(|o|json!({"name":o.port.name,"type":type_json(&o.port.value_type),"source":source_json(&o.source)})).collect::<Vec<_>>()})
}
fn module_json(m: &Module) -> Value {
    json!({"schema_version":1,"key":key_json(&m.key),"parameters":schema_json(&m.parameters),"graph":body_json(&m.graph)})
}
fn definition_json(d: &GraphDefinition) -> Value {
    json!({"format":"fqe.graph","schema_version":1,"ir_version":2,"id":d.id,"version":d.version,"parameters":schema_json(&d.parameters),"requirements":d.requirements,"entries":d.entries.iter().map(|e|json!({"key":key_json(&e.key),"descriptor_version":e.descriptor_version,"role":e.role,"artifact":e.artifact.to_string()})).collect::<Vec<_>>(),"graph":body_json(&d.graph),"modules":d.modules.iter().map(module_json).collect::<Vec<_>>(),"metadata":d.metadata,"workflow":{},"state":[],"dependencies":[]})
}
#[derive(Clone, Debug)]
pub struct LoadedGraph {
    definition: GraphDefinition,
    canonical: Vec<u8>,
    content_id: ContentId,
    source_id: ContentId,
}
impl LoadedGraph {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let definition = definition(loading::decode(bytes)?)?;
        let canonical = serde_json::to_vec(&definition_json(&definition))
            .map_err(|_| error("graph_encoding", "$", "encoding failed"))?;
        if canonical.len() > MAX_MODEL_BYTES {
            return Err(error("graph_limit", "$", "canonical artifact too large"));
        }
        let content_id = digest(GRAPH_PREFIX, &canonical);
        Ok(Self {
            definition,
            canonical,
            content_id,
            source_id: digest(b"", bytes),
        })
    }
    pub fn from_reader(reader: impl Read) -> Result<Self> {
        let mut bytes = Vec::new();
        reader
            .take(MAX_MODEL_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| error("io", "$", "graph read failed"))?;
        Self::from_bytes(&bytes)
    }
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_reader(
            std::fs::File::open(path).map_err(|_| error("io", "$", "graph open failed"))?,
        )
    }
    pub fn definition(&self) -> &GraphDefinition {
        &self.definition
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
    pub fn bind(
        &self,
        values: impl IntoIterator<Item = (String, ParameterInput)>,
    ) -> Result<GraphBinding> {
        let parameters = self
            .definition
            .parameters
            .bind(values)
            .map_err(|_| error("graph_parameter", "$.bindings", "invalid graph bindings"))?;
        Ok(GraphBinding {
            content_id: self.content_id,
            parameters,
        })
    }
    pub fn bind_json(&self, bytes: &[u8]) -> Result<GraphBinding> {
        let mut o = Obj::new(loading::decode(bytes)?, "$")?;
        if uint(o.take("version")?, "$")? != 1 {
            return Err(error("graph_version", "$", "unsupported binding version"));
        }
        let vals = Obj::new(o.take("values")?, "$.values")?;
        o.end()?;
        let values = vals
            .fields
            .into_iter()
            .map(|(k, v)| {
                Ok((
                    k,
                    if v.is_null() {
                        ParameterInput::Null
                    } else {
                        ParameterInput::Value(Evaluation::Ready(loading::typed(v, "$.values")?))
                    },
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        self.bind(values)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphBinding {
    content_id: ContentId,
    parameters: BoundParameters,
}
impl GraphBinding {
    pub fn content_id(&self) -> ContentId {
        self.content_id
    }
    pub fn parameters(&self) -> &BoundParameters {
        &self.parameters
    }
    pub fn ensure_matches(&self, g: &LoadedGraph) -> Result<()> {
        if self.content_id != g.content_id {
            return Err(error(
                "graph_binding",
                "$.bindings",
                "binding content differs",
            ));
        }
        Ok(())
    }
}
