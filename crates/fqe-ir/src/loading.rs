//! Bounded, strict experimental JSON model import. No provider execution or resolution.
use crate::{
    model::*,
    parameters::{
        Bound, BoundParameters, Constraints, ParameterInput, ParameterSchema, ParameterSpec,
    },
};
use fqe_core::{
    InstrumentId,
    numeric::Decimal,
    types::{
        CurrencyCode, Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue,
        ValueShape,
    },
};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt, io::Read, path::Path};

pub const MAX_DEPTH: usize = 32;
pub const MAX_COLLECTION: usize = 1024;
pub const MAX_ITEMS: usize = 16384;
pub const CANONICAL_PREFIX: &[u8] = b"FQE-MODEL-CANONICAL/1\n";
type Result<T> = std::result::Result<T, ModelError>;
fn err(code: &'static str, path: &str, detail: &str) -> ModelError {
    ModelError::new(code, path, detail)
}

struct Seed<'a> {
    depth: usize,
    remaining: &'a mut usize,
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> std::result::Result<Value, D::Error> {
        if self.depth > MAX_DEPTH || *self.remaining == 0 {
            return Err(de::Error::custom("resource limit: depth/items"));
        }
        *self.remaining -= 1;
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded model JSON")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Value, E> {
        Ok(Value::from(v))
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> std::result::Result<Value, E> {
        Err(E::custom(
            "signed JSON integer: encode scalar numbers as strings",
        ))
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> std::result::Result<Value, E> {
        Err(E::custom("floating point JSON numbers unsupported"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Value, E> {
        if v.len() > 4096 {
            Err(E::custom("resource limit: string"))
        } else {
            Ok(Value::String(v.to_owned()))
        }
    }
    fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Value, E> {
        self.visit_str(&v)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> std::result::Result<Value, A::Error> {
        let mut out = Vec::new();
        while let Some(v) = a.next_element_seed(Seed {
            depth: self.depth + 1,
            remaining: self.remaining,
        })? {
            if out.len() == MAX_COLLECTION {
                return Err(de::Error::custom("resource limit: array"));
            }
            out.push(v);
        }
        Ok(Value::Array(out))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> std::result::Result<Value, A::Error> {
        let mut out = Map::new();
        while let Some(k) = a.next_key::<String>()? {
            if k.len() > 128 || out.len() == MAX_COLLECTION {
                return Err(de::Error::custom("resource limit: object/key"));
            }
            if out.contains_key(&k) {
                return Err(de::Error::custom("duplicate object key"));
            }
            let v = a.next_value_seed(Seed {
                depth: self.depth + 1,
                remaining: self.remaining,
            })?;
            out.insert(k, v);
        }
        Ok(Value::Object(out))
    }
}
pub(crate) fn decode(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > MAX_MODEL_BYTES {
        return Err(err("resource_limit", "$", "input exceeds 262144 bytes"));
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let mut remaining = MAX_ITEMS;
    let v = Seed {
        depth: 0,
        remaining: &mut remaining,
    }
    .deserialize(&mut d)
    .map_err(|e| parse_error(&e))?;
    d.end().map_err(|e| parse_error(&e))?;
    Ok(v)
}
fn parse_error(e: &serde_json::Error) -> ModelError {
    // Preserve location/category without reflecting untrusted token text into diagnostics.
    let message = e.to_string();
    let code = if message.contains("resource limit") {
        "resource_limit"
    } else if message.contains("duplicate object key") {
        "duplicate_key"
    } else {
        "json"
    };
    ModelError::new(
        code,
        "$",
        format!(
            "invalid bounded JSON at line {}, column {}",
            e.line(),
            e.column()
        ),
    )
}
pub(crate) struct Obj {
    pub(crate) fields: Map<String, Value>,
    path: String,
}
impl Obj {
    pub(crate) fn new(v: Value, path: &str) -> Result<Self> {
        match v {
            Value::Object(fields) => Ok(Self {
                fields,
                path: path.into(),
            }),
            _ => Err(err("type", path, "expected object")),
        }
    }
    pub(crate) fn take(&mut self, k: &str) -> Result<Value> {
        self.fields.remove(k).ok_or_else(|| {
            err(
                "missing_field",
                &format!("{}.{k}", self.path),
                "required field",
            )
        })
    }
    pub(crate) fn optional(&mut self, k: &str) -> Option<Value> {
        self.fields.remove(k)
    }
    pub(crate) fn end(self) -> Result<()> {
        if self.fields.is_empty() {
            Ok(())
        } else {
            Err(err("unknown_field", &self.path, "unknown object field"))
        }
    }
}
pub(crate) fn text(v: Value, p: &str) -> Result<String> {
    match v {
        Value::String(s) => Ok(s),
        _ => Err(err("type", p, "expected string")),
    }
}
pub(crate) fn uint(v: Value, p: &str) -> Result<u32> {
    v.as_u64()
        .and_then(|n| n.try_into().ok())
        .ok_or_else(|| err("type", p, "expected unsigned u32 JSON integer"))
}
fn boolean(v: Value, p: &str) -> Result<bool> {
    v.as_bool()
        .ok_or_else(|| err("type", p, "expected Boolean"))
}
fn array(v: Value, p: &str) -> Result<Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a),
        _ => Err(err("type", p, "expected array")),
    }
}
pub(crate) fn mapped<T>(v: Value, p: &str, f: impl Fn(Value, &str) -> Result<T>) -> Result<Vec<T>> {
    array(v, p)?
        .into_iter()
        .enumerate()
        .map(|(i, v)| f(v, &format!("{p}[{i}]")))
        .collect()
}
fn convert<T, E>(v: std::result::Result<T, E>, p: &str) -> Result<T> {
    v.map_err(|_| err("validation", p, "value/schema validation failed"))
}
pub(crate) fn descriptor(v: Value, p: &str) -> Result<TypeDescriptor> {
    let mut o = Obj::new(v, p)?;
    let version = uint(o.take("version")?, p)?;
    if text(o.take("shape")?, p)? != "scalar" {
        return Err(err("unsupported_type", p, "only scalar shapes implemented"));
    }
    let kind = text(o.take("kind")?, p)?;
    let scalar = match kind.as_str() {
        "bool" => ScalarType::Bool,
        "i64" => ScalarType::I64,
        "u64" => ScalarType::U64,
        "text" => ScalarType::Text {
            max_bytes: uint(o.take("max_bytes")?, p)? as usize,
        },
        "decimal" => {
            let scale = uint(o.take("scale")?, p)?
                .try_into()
                .map_err(|_| err("type", p, "scale exceeds u8"))?;
            let mut u = Obj::new(o.take("unit")?, p)?;
            let k = text(u.take("kind")?, p)?;
            let unit = match k.as_str() {
                "scalar" => NumericUnit::Scalar,
                "ratio" => NumericUnit::Ratio,
                "price" | "quantity" => {
                    let id = convert(InstrumentId::new(text(u.take("instrument")?, p)?), p)?;
                    if k == "price" {
                        NumericUnit::Price(id)
                    } else {
                        NumericUnit::Quantity(id)
                    }
                }
                "money" => NumericUnit::Money(convert(
                    CurrencyCode::new(text(u.take("currency")?, p)?),
                    p,
                )?),
                _ => return Err(err("unsupported_type", p, "unknown numeric unit")),
            };
            u.end()?;
            ScalarType::Decimal { scale, unit }
        }
        _ => return Err(err("unsupported_type", p, "unknown scalar type")),
    };
    o.end()?;
    convert(TypeDescriptor::new(version, ValueShape::Scalar, scalar), p)
}
fn integer<T: std::str::FromStr + ToString>(v: Value, p: &str) -> Result<T> {
    let s = text(v, p)?;
    let n: T = s
        .parse()
        .map_err(|_| err("number", p, "integer string out of range"))?;
    if n.to_string() != s {
        return Err(err(
            "number",
            p,
            "integer string must use canonical decimal notation",
        ));
    }
    Ok(n)
}
pub(crate) fn typed(v: Value, p: &str) -> Result<TypedValue> {
    let mut o = Obj::new(v, p)?;
    let t = descriptor(o.take("type")?, p)?;
    let v = o.take("value")?;
    let value = match t.scalar() {
        ScalarType::Bool => ScalarValue::Bool(boolean(v, p)?),
        ScalarType::I64 => ScalarValue::I64(integer(v, p)?),
        ScalarType::U64 => ScalarValue::U64(integer(v, p)?),
        ScalarType::Text { .. } => ScalarValue::Text(text(v, p)?),
        ScalarType::Decimal { scale, .. } => {
            ScalarValue::Decimal(convert(Decimal::new(integer(v, p)?, *scale), p)?)
        }
        _ => return Err(err("unsupported_type", p, "unsupported scalar")),
    };
    o.end()?;
    convert(TypedValue::new(t, value), p)
}
fn bound(v: Value, p: &str) -> Result<Bound> {
    let mut o = Obj::new(v, p)?;
    let value = typed(o.take("value")?, p)?;
    let inclusive = boolean(o.take("inclusive")?, p)?;
    o.end()?;
    Ok(Bound { value, inclusive })
}
fn constraints(v: Value, p: &str) -> Result<Constraints> {
    let mut o = Obj::new(v, p)?;
    let lower = o.optional("lower").map(|v| bound(v, p)).transpose()?;
    let upper = o.optional("upper").map(|v| bound(v, p)).transpose()?;
    let choices = o
        .optional("choices")
        .map(|v| mapped(v, p, typed))
        .transpose()?;
    o.end()?;
    Ok(Constraints {
        lower,
        upper,
        choices,
    })
}
pub(crate) fn parameter(v: Value, p: &str) -> Result<ParameterSpec> {
    let mut o = Obj::new(v, p)?;
    let key = text(o.take("key")?, p)?;
    let t = descriptor(o.take("type")?, p)?;
    let required = boolean(o.take("required")?, p)?;
    let default = o.optional("default").map(|v| typed(v, p)).transpose()?;
    let c = constraints(o.take("constraints")?, p)?;
    let description = text(o.take("description")?, p)?;
    o.end()?;
    convert(
        ParameterSpec::new(key, t, required, default, c, description),
        p,
    )
}
pub(crate) fn key(v: Value, p: &str) -> Result<EntryKey> {
    let mut o = Obj::new(v, p)?;
    let k = EntryKey {
        id: text(o.take("id")?, p)?,
        version: text(o.take("version")?, p)?,
    };
    o.end()?;
    Ok(k)
}
pub(crate) fn entry(v: Value, p: &str) -> Result<EntryRequirement> {
    let mut o = Obj::new(v, p)?;
    let key = key(o.take("key")?, p)?;
    let descriptor_version = uint(o.take("descriptor_version")?, p)?;
    let role = text(o.take("role")?, p)?;
    let artifact = ContentId::parse(&text(o.take("artifact")?, p)?)?;
    o.end()?;
    Ok(EntryRequirement {
        key,
        descriptor_version,
        role,
        artifact,
    })
}
fn input(v: Value, p: &str) -> Result<Input> {
    let mut o = Obj::new(v, p)?;
    let name = text(o.take("name")?, p)?;
    let mut s = Obj::new(o.take("source")?, p)?;
    let source = PortRef {
        node: text(s.take("node")?, p)?,
        port: text(s.take("port")?, p)?,
    };
    s.end()?;
    o.end()?;
    Ok(Input { name, source })
}
pub(crate) fn port(v: Value, p: &str) -> Result<Port> {
    let mut o = Obj::new(v, p)?;
    let name = text(o.take("name")?, p)?;
    let value_type = descriptor(o.take("type")?, p)?;
    o.end()?;
    Ok(Port { name, value_type })
}
pub(crate) fn argument(v: Value, p: &str) -> Result<Argument> {
    let mut o = Obj::new(v, p)?;
    let a = if let Some(v) = o.optional("parameter") {
        Argument::Parameter(text(v, p)?)
    } else {
        Argument::Literal(typed(o.take("literal")?, p)?)
    };
    o.end()?;
    Ok(a)
}
fn node(v: Value, p: &str) -> Result<Node> {
    let mut o = Obj::new(v, p)?;
    let id = text(o.take("id")?, p)?;
    let entry = key(o.take("entry")?, p)?;
    let args = Obj::new(o.take("arguments")?, p)?;
    let arguments = args
        .fields
        .into_iter()
        .map(|(k, v)| Ok((k, argument(v, &format!("{p}.arguments"))?)))
        .collect::<Result<_>>()?;
    let inputs = mapped(o.take("inputs")?, p, input)?;
    let outputs = mapped(o.take("outputs")?, p, port)?;
    o.end()?;
    Ok(Node {
        id,
        entry,
        arguments,
        inputs,
        outputs,
    })
}
fn definition(v: Value) -> Result<ModelDefinition> {
    let p = "$";
    let mut o = Obj::new(v, p)?;
    if text(o.take("format")?, p)? != "fqe.model" {
        return Err(err("format", p, "expected fqe.model"));
    }
    let schema_version = uint(o.take("schema_version")?, p)?;
    let ir_version = uint(o.take("ir_version")?, p)?;
    let id = text(o.take("id")?, p)?;
    let version = text(o.take("version")?, p)?;
    let mut ps = Obj::new(o.take("parameters")?, "$.parameters")?;
    let pv = uint(ps.take("version")?, "$.parameters.version")?;
    let specs = mapped(ps.take("entries")?, "$.parameters.entries", parameter)?;
    ps.end()?;
    let parameters = convert(ParameterSchema::new(pv, specs), "$.parameters")?;
    let requirements = mapped(o.take("requirements")?, "$.requirements", text)?;
    let entries = mapped(o.take("entries")?, "$.entries", entry)?;
    let mut g = Obj::new(o.take("graph")?, "$.graph")?;
    let nodes = mapped(g.take("nodes")?, "$.graph.nodes", node)?;
    let outputs = mapped(g.take("outputs")?, "$.graph.outputs", input)?;
    g.end()?;
    let m = Obj::new(o.take("metadata")?, "$.metadata")?;
    let metadata = m
        .fields
        .into_iter()
        .map(|(k, v)| Ok((k, text(v, "$.metadata")?)))
        .collect::<Result<_>>()?;
    for k in ["modules", "state", "dependencies"] {
        if !array(o.take(k)?, &format!("$.{k}"))?.is_empty() {
            return Err(err(
                "unsupported_feature",
                &format!("$.{k}"),
                "nonempty future feature unsupported",
            ));
        }
    }
    Obj::new(o.take("workflow")?, "$.workflow")?.end()?;
    o.end()?;
    ModelDefinition::new(ModelDraft {
        schema_version,
        ir_version,
        id,
        version,
        parameters,
        requirements,
        entries,
        nodes,
        outputs,
        metadata,
    })
}
pub(crate) fn type_json(t: &TypeDescriptor) -> Value {
    let mut o = json!({"version":t.version(),"shape":"scalar"});
    let kind = match t.scalar() {
        ScalarType::Bool => "bool",
        ScalarType::I64 => "i64",
        ScalarType::U64 => "u64",
        ScalarType::Text { max_bytes } => {
            o["max_bytes"] = json!(max_bytes);
            "text"
        }
        ScalarType::Decimal { scale, unit } => {
            o["scale"] = json!(scale);
            o["unit"] = match unit {
                NumericUnit::Scalar => json!({"kind":"scalar"}),
                NumericUnit::Ratio => json!({"kind":"ratio"}),
                NumericUnit::Price(i) => json!({"kind":"price","instrument":i.as_str()}),
                NumericUnit::Quantity(i) => json!({"kind":"quantity","instrument":i.as_str()}),
                NumericUnit::Money(c) => json!({"kind":"money","currency":c.as_str()}),
            };
            "decimal"
        }
        ScalarType::Unsupported(_) => unreachable!("validated descriptor"),
    };
    o["kind"] = json!(kind);
    o
}
pub(crate) fn typed_json(v: &TypedValue) -> Value {
    let value = match v.value() {
        ScalarValue::Bool(b) => json!(b),
        ScalarValue::Text(s) => json!(s),
        ScalarValue::I64(n) => json!(n.to_string()),
        ScalarValue::U64(n) => json!(n.to_string()),
        ScalarValue::Decimal(d) => json!(d.coefficient().to_string()),
    };
    json!({"type":type_json(v.descriptor()),"value":value})
}
fn bound_json(b: &Bound) -> Value {
    json!({"value":typed_json(&b.value),"inclusive":b.inclusive})
}
fn input_json(i: &Input) -> Value {
    json!({"name":i.name,"source":{"node":i.source.node,"port":i.source.port}})
}
pub(crate) fn parameter_json(s: &ParameterSpec) -> Value {
    let mut constraints = Map::new();
    if let Some(b) = &s.constraints().lower {
        constraints.insert("lower".into(), bound_json(b));
    }
    if let Some(b) = &s.constraints().upper {
        constraints.insert("upper".into(), bound_json(b));
    }
    if let Some(values) = &s.constraints().choices {
        constraints.insert(
            "choices".into(),
            json!(values.iter().map(typed_json).collect::<Vec<_>>()),
        );
    }
    let mut value = json!({
        "key": s.key(), "type": type_json(s.descriptor()), "required": s.required(),
        "constraints": constraints, "description": s.description()
    });
    if let Some(default) = s.default_value() {
        value["default"] = typed_json(default);
    }
    value
}
fn key_json(k: &EntryKey) -> Value {
    json!({"id": k.id, "version": k.version})
}
fn node_json(n: &Node) -> Value {
    let arguments: Map<_, _> = n
        .arguments
        .iter()
        .map(|(key, value)| {
            let value = match value {
                Argument::Parameter(p) => json!({"parameter": p}),
                Argument::Literal(v) => json!({"literal": typed_json(v)}),
            };
            (key.clone(), value)
        })
        .collect();
    let outputs: Vec<_> = n
        .outputs
        .iter()
        .map(|p| json!({"name": p.name, "type": type_json(&p.value_type)}))
        .collect();
    json!({
        "id": n.id, "entry": key_json(&n.entry), "arguments": arguments,
        "inputs": n.inputs.iter().map(input_json).collect::<Vec<_>>(), "outputs": outputs
    })
}
fn canonical(d: &ModelDefinition) -> Result<Vec<u8>> {
    let d = d.declaration();
    // ParameterSchema and ModelDefinition have already normalized named collections.
    let parameters: Vec<_> = d.parameters.specs().map(parameter_json).collect();
    let entries: Vec<_> = d
        .entries
        .iter()
        .map(|e| {
            json!({
                "key": key_json(&e.key), "descriptor_version": e.descriptor_version,
                "role": e.role, "artifact": e.artifact.to_string()
            })
        })
        .collect();
    let value = json!({
        "format": "fqe.model", "schema_version": d.schema_version, "ir_version": d.ir_version,
        "id": d.id, "version": d.version,
        "parameters": {"version": d.parameters.version(), "entries": parameters},
        "requirements": d.requirements, "entries": entries,
        "graph": {"nodes": d.nodes.iter().map(node_json).collect::<Vec<_>>(),
                  "outputs": d.outputs.iter().map(input_json).collect::<Vec<_>>()},
        "metadata": d.metadata, "modules": [], "state": [], "dependencies": [], "workflow": {}
    });
    serde_json::to_vec(&value).map_err(|_| err("encoding", "$", "canonical encoding failed"))
}
fn digest(bytes: &[u8]) -> ContentId {
    ContentId::from_bytes(Sha256::digest(bytes).into())
}
#[derive(Clone, Debug)]
pub struct LoadedModel {
    definition: ModelDefinition,
    node_sources: BTreeMap<String, String>,
    canonical: Vec<u8>,
    source_id: ContentId,
    content_id: ContentId,
}
impl LoadedModel {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let value = decode(bytes)?;
        let node_sources = value
            .get("graph")
            .and_then(|g| g.get("nodes"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(i, n)| {
                n.get("id")
                    .and_then(Value::as_str)
                    .map(|id| (id.to_string(), format!("$.graph.nodes[{i}]")))
            })
            .collect();
        let definition = definition(value)?;
        let canonical = canonical(&definition)?;
        if canonical.len() > MAX_MODEL_BYTES {
            return Err(err("resource_limit", "$", "canonical model too large"));
        }
        let mut h = Sha256::new();
        h.update(CANONICAL_PREFIX);
        h.update(&canonical);
        let content_id = ContentId::from_bytes(h.finalize().into());
        Ok(Self {
            definition,
            node_sources,
            canonical,
            source_id: digest(bytes),
            content_id,
        })
    }
    pub fn from_reader(reader: impl Read) -> Result<Self> {
        let mut bytes = Vec::new();
        reader
            .take(MAX_MODEL_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| err("io", "$", "model read failed"))?;
        Self::from_bytes(&bytes)
    }
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_reader(
            std::fs::File::open(path).map_err(|_| err("io", "$", "model open failed"))?,
        )
    }
    /// Original JSON node location, excluded from canonical bytes/content identity.
    pub fn node_source(&self, id: &str) -> Option<&str> {
        self.node_sources.get(id).map(String::as_str)
    }
    pub fn definition(&self) -> &ModelDefinition {
        &self.definition
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn source_id(&self) -> ContentId {
        self.source_id
    }
    pub fn content_id(&self) -> ContentId {
        self.content_id
    }
    pub fn bind_json(&self, bytes: &[u8]) -> Result<ModelBinding> {
        let mut o = Obj::new(decode(bytes)?, "$")?;
        if uint(o.take("version")?, "$.version")? != 1 {
            return Err(err("version", "$.version", "unsupported binding version"));
        }
        let values = Obj::new(o.take("values")?, "$.values")?;
        o.end()?;
        let inputs = values
            .fields
            .into_iter()
            .map(|(k, v)| {
                let p = format!("$.values.{k}");
                Ok((
                    k,
                    if v.is_null() {
                        ParameterInput::Null
                    } else {
                        ParameterInput::Value(Evaluation::Ready(typed(v, &p)?))
                    },
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let parameters = self.definition.bind(inputs)?;
        Ok(ModelBinding {
            model_id: self.definition.declaration().id.clone(),
            author_version: self.definition.declaration().version.clone(),
            content_id: self.content_id,
            parameters,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelBinding {
    model_id: String,
    author_version: String,
    content_id: ContentId,
    parameters: BoundParameters,
}
impl ModelBinding {
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
    pub fn author_version(&self) -> &str {
        &self.author_version
    }
    pub fn content_id(&self) -> ContentId {
        self.content_id
    }
    pub fn parameters(&self) -> &BoundParameters {
        &self.parameters
    }
    pub fn ensure_matches(&self, model: &LoadedModel) -> Result<()> {
        if self.content_id == model.content_id {
            Ok(())
        } else {
            Err(err(
                "content_mismatch",
                "$",
                "binding belongs to different model content",
            ))
        }
    }
}
