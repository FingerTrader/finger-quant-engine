//! First-party, bounded structured Event authoring. No provider-name dispatch or evaluator.
use fqe_ir::{catalog, event_plan::LoadedEventPlan, model::ModelError};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SOURCE_BYTES: usize = 65536;
const MAX_TOKENS: usize = 8192;
type Result<T> = std::result::Result<T, Diagnostic>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub path: String,
    pub offset: usize,
    pub message: String,
}
impl Diagnostic {
    fn new(code: &str, offset: usize, message: &str) -> Self {
        Self {
            code: code.into(),
            path: "$.source".into(),
            offset,
            message: message.into(),
        }
    }
    pub fn value(&self, source: &str) -> Value {
        let prefix = source.get(..self.offset).unwrap_or("");
        json!({"severity":"error","code":self.code,"path":self.path,"message":self.message,
            "file":"strategy.fqe","byte_offset":self.offset.to_string(),
            "line":(prefix.bytes().filter(|b| *b==b'\n').count()+1).to_string(),
            "column":(prefix.rsplit('\n').next().unwrap_or("").chars().count()+1).to_string()})
    }
}
#[derive(Clone, Debug)]
struct Token {
    text: String,
    quoted: bool,
    start: usize,
    end: usize,
}
#[derive(Clone, Debug)]
pub struct Document {
    pub semantic: Value,
    pub source_map: Value,
    pub comments: Vec<String>,
}
fn lex(source: &str) -> Result<(Vec<Token>, Vec<String>)> {
    if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
        return Err(Diagnostic::new(
            "authoring_quota",
            0,
            "source must contain 1..65536 UTF-8 bytes",
        ));
    }
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut tokens = vec![];
    let mut comments = vec![];
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i..].starts_with(b"//") {
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            comments.push(source[start..i].to_owned());
            continue;
        }
        let start = i;
        let quoted = bytes[i] == b'"';
        let text = if quoted {
            i += 1;
            loop {
                if i == bytes.len() {
                    return Err(Diagnostic::new(
                        "authoring_string",
                        start,
                        "unterminated string",
                    ));
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
                if i > bytes.len() {
                    return Err(Diagnostic::new(
                        "authoring_string",
                        start,
                        "unterminated escape",
                    ));
                }
            }
            let value: String = serde_json::from_str(&source[start..i]).map_err(|_| {
                Diagnostic::new("authoring_string", start, "invalid JSON string escape")
            })?;
            if value.len() > 4096 {
                return Err(Diagnostic::new(
                    "authoring_quota",
                    start,
                    "string exceeds 4096 bytes",
                ));
            }
            value
        } else if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || b"._-".contains(&bytes[i]))
            {
                i += 1;
            }
            source[start..i].into()
        } else if b";:=(),@$".contains(&bytes[i]) {
            i += 1;
            source[start..i].into()
        } else {
            return Err(Diagnostic::new(
                "authoring_token",
                i,
                "unexpected token; identifiers are ASCII and text uses quoted strings",
            ));
        };
        tokens.push(Token {
            text,
            quoted,
            start,
            end: i,
        });
        if tokens.len() > MAX_TOKENS {
            return Err(Diagnostic::new(
                "authoring_quota",
                start,
                "token budget exceeded",
            ));
        }
    }
    Ok((tokens, comments))
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    end: usize,
    maps: Vec<Value>,
    names: BTreeSet<String>,
    inputs: Map<String, Value>,
    output_types: BTreeMap<String, Value>,
    descriptors: BTreeMap<String, Value>,
}
impl Parser {
    fn offset(&self) -> usize {
        self.tokens.get(self.pos).map_or(self.end, |t| t.start)
    }
    fn error(&self, message: &str) -> Diagnostic {
        Diagnostic::new("authoring_syntax", self.offset(), message)
    }
    fn take(&mut self) -> Result<Token> {
        let token = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or_else(|| self.error("unexpected end of source"))?;
        self.pos += 1;
        Ok(token)
    }
    fn eat(&mut self, text: &str) -> bool {
        if self
            .tokens
            .get(self.pos)
            .is_some_and(|t| !t.quoted && t.text == text)
        {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, text: &str) -> Result<()> {
        if self.eat(text) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {text}")))
        }
    }
    fn string(&mut self) -> Result<String> {
        let t = self.take()?;
        if t.quoted {
            Ok(t.text)
        } else {
            Err(Diagnostic::new(
                "authoring_string",
                t.start,
                "quoted exact value required",
            ))
        }
    }
    fn id(&mut self) -> Result<String> {
        let t = self.take()?;
        if t.quoted
            || t.text.is_empty()
            || t.text.len() > 120
            || !t
                .text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(Diagnostic::new(
                "authoring_identifier",
                t.start,
                "expected bounded identifier",
            ));
        }
        Ok(t.text)
    }
    fn number(&mut self) -> Result<u32> {
        let t = self.take()?;
        let n = t
            .text
            .parse::<u32>()
            .map_err(|_| self.error("expected small unsigned integer"))?;
        if t.quoted || n.to_string() != t.text {
            return Err(self.error("noncanonical small integer"));
        }
        Ok(n)
    }
    fn mark(&mut self, key: String, path: String, start: usize) -> Result<()> {
        if !self.names.insert(key.clone()) {
            return Err(Diagnostic::new(
                "authoring_duplicate",
                start,
                "duplicate declaration",
            ));
        }
        let end = self.tokens[self.pos - 1].end;
        self.maps
            .push(json!({"id":key,"path":path,"start":start.to_string(),"end":end.to_string()}));
        Ok(())
    }
    fn value(&mut self, ty: &Value) -> Result<Value> {
        let value = if ty["kind"] == "bool" {
            if self.eat("true") {
                json!(true)
            } else if self.eat("false") {
                json!(false)
            } else {
                return Err(self.error("expected true or false"));
            }
        } else if ty["kind"] == "decimal" {
            let scale = ty["scale"]
                .as_u64()
                .and_then(|s| u8::try_from(s).ok())
                .filter(|s| *s <= 18)
                .ok_or_else(|| self.error("decimal scale must be 0..18"))?;
            let source = self.string()?;
            let value = fqe_core::finance::parse_decimal(&source, scale)
                .map_err(|_| self.error("expected an exact fixed-scale decimal string"))?;
            json!(value.coefficient().to_string())
        } else {
            json!(self.string()?)
        };
        Ok(json!({"type":ty,"value":value}))
    }
    fn ty(&mut self) -> Result<Value> {
        let kind = self.id()?;
        match kind.as_str() {
            "u64" | "bool" => Ok(json!({"version":1,"shape":"scalar","kind":kind})),
            "decimal" => {
                self.expect("(")?;
                let scale = self.number()?;
                self.expect(",")?;
                let unit = self.id()?;
                let unit = match unit.as_str() {
                    "ratio" => json!({"kind":"ratio"}),
                    "price" | "quantity" => {
                        self.expect(",")?;
                        let id = self.string()?;
                        json!({"kind":unit,"instrument":id})
                    }
                    "money" => {
                        self.expect(",")?;
                        let id = self.string()?;
                        json!({"kind":unit,"currency":id})
                    }
                    _ => return Err(self.error("unsupported decimal unit")),
                };
                self.expect(")")?;
                Ok(json!({"version":1,"shape":"scalar","kind":"decimal","scale":scale,"unit":unit}))
            }
            _ => Err(self.error("unsupported scalar type")),
        }
    }
    fn wire(&self, name: &str) -> Result<Value> {
        if self.inputs.contains_key(name) {
            return Ok(json!({"input":name}));
        }
        if !self.output_types.contains_key(name) {
            return Err(self.error("unknown node port; declare upstream nodes first"));
        }
        let (node, port) = name
            .rsplit_once('.')
            .ok_or_else(|| self.error("node.port required"))?;
        Ok(json!({"node":node,"port":port}))
    }
    fn expression(&mut self, depth: usize) -> Result<Value> {
        if depth > 16 {
            return Err(self.error("authoring expression nesting exceeds 16"));
        }
        if self.eat("$") {
            return Ok(json!({"parameter":self.id()?}));
        }
        let name = self.id()?;
        if self.eat("(") {
            if name == "cross" {
                let id = self.id()?;
                self.expect(")")?;
                return Ok(json!({"cross":id}));
            }
            if name == "not" {
                let value = self.expression(depth + 1)?;
                self.expect(")")?;
                return Ok(json!({"not":value}));
            }
            if ![
                "add",
                "subtract",
                "multiply",
                "divide",
                "equal",
                "not_equal",
                "less",
                "less_equal",
                "greater",
                "greater_equal",
                "and",
                "or",
            ]
            .contains(&name.as_str())
            {
                return Err(self.error("unknown expression operation"));
            }
            let left = self.expression(depth + 1)?;
            self.expect(",")?;
            let right = self.expression(depth + 1)?;
            self.expect(")")?;
            Ok(json!({"binary":{"op":name,"left":left,"right":right}}))
        } else if self.output_types.contains_key(&name) {
            Ok(json!({"output":name}))
        } else if self.inputs.contains_key(&name) {
            Ok(json!({"input":name}))
        } else {
            Err(self.error("unknown input/node port in expression"))
        }
    }
}

/// Parse source to the one semantic model, preserving source spans/comments separately.
/// This does not execute a provider, create an account or admit a run.
pub fn parse(source: &str) -> Result<Document> {
    parse_using_registry(source, None)
}
/// Use an already reviewed, caller-owned registry. This does not load/install plugins.
/// The desktop RPC deliberately retains its contextual reference registry.
pub fn parse_with_registry(
    source: &str,
    registry: &fqe_ir::registry::Registry,
) -> Result<Document> {
    parse_using_registry(source, Some(registry))
}
fn parse_using_registry(
    source: &str,
    supplied: Option<&fqe_ir::registry::Registry>,
) -> Result<Document> {
    let (tokens, comments) = lex(source)?;
    let mut p = Parser {
        tokens,
        pos: 0,
        end: source.len(),
        maps: vec![],
        names: BTreeSet::new(),
        inputs: Map::new(),
        output_types: BTreeMap::new(),
        descriptors: BTreeMap::new(),
    };
    p.expect("fqe")?;
    p.expect("1")?;
    p.expect(";")?;
    p.expect("strategy")?;
    let id = p.string()?;
    p.expect("version")?;
    let version = p.string()?;
    p.expect(";")?;
    p.expect("context")?;
    let kind = p.id()?;
    let instrument = p.string()?;
    let trigger = match kind.as_str() {
        "tick" => json!({"kind":"tick","instrument":instrument}),
        "closed_bar" => {
            p.expect("timeframe")?;
            let timeframe = p.string()?;
            p.expect("price_scale")?;
            let ps = p.number()?;
            p.expect("volume_scale")?;
            let vs = p.number()?;
            json!({"kind":"closed_bar","instrument":instrument,"timeframe_ns":timeframe,"price_scale":ps,"volume_scale":vs})
        }
        _ => return Err(p.error("unsupported event context")),
    };
    p.expect(";")?;
    let t = super::discovery::context(&trigger).map_err(|e| Diagnostic {
        code: e.code.into(),
        path: e.path,
        offset: 0,
        message: "invalid event context".into(),
    })?;
    let reference;
    let registry = if let Some(registry) = supplied {
        registry
    } else {
        reference = catalog::reference(&t).map_err(|e| native_error(e, &[]))?;
        &reference
    };
    for d in registry.descriptors() {
        let v: Value = serde_json::from_slice(d.canonical_bytes()).expect("native descriptor JSON");
        p.descriptors.insert(
            format!("{}@{}", d.declaration().key.id, d.declaration().key.version),
            v,
        );
    }
    let mut params = vec![];
    let mut nodes = vec![];
    let mut entries = BTreeMap::new();
    let mut crossings = vec![];
    let mut outputs = vec![];
    let mut policies = vec![];
    let mut warmup = "0".to_string();
    while p.pos < p.tokens.len() {
        let start = p.offset();
        let declaration = p.id()?;
        if declaration == "warmup" {
            warmup = p.string()?;
            p.expect(";")?;
            p.mark("warmup".into(), "$.warmup_samples".into(), start)?;
            continue;
        }
        let name = p.id()?;
        if name.contains('.') {
            return Err(p.error(
                "local declaration names cannot contain dots; node.port is a qualified reference",
            ));
        }
        let path = match declaration.as_str() {
            "param" => {
                p.expect(":")?;
                let ty = p.ty()?;
                p.expect("=")?;
                let default = p.value(&ty)?;
                let mut constraints = json!({});
                if p.eat("within") {
                    p.expect("(")?;
                    let lo = p.value(&ty)?;
                    p.expect(",")?;
                    let hi = p.value(&ty)?;
                    p.expect(")")?;
                    constraints = json!({"lower":{"value":lo,"inclusive":true},"upper":{"value":hi,"inclusive":true}});
                }
                let description = if p.eat("description") {
                    p.string()?
                } else {
                    String::new()
                };
                params.push(json!({"key":name,"type":ty,"required":false,"default":default,"constraints":constraints,"description":description}));
                format!("$.parameters.entries[{}]", params.len() - 1)
            }
            "input" => {
                p.expect("=")?;
                let field = p.id()?;
                let ty = match field.as_str() {
                    "price" if kind == "tick" => json!({"version":1,"shape":"scalar","kind":"u64"}),
                    "open" | "high" | "low" | "close" if kind == "closed_bar" => {
                        json!({"version":1,"shape":"scalar","kind":"decimal","scale":trigger["price_scale"],"unit":{"kind":"price","instrument":instrument}})
                    }
                    "volume" if kind == "closed_bar" => {
                        json!({"version":1,"shape":"scalar","kind":"decimal","scale":trigger["volume_scale"],"unit":{"kind":"quantity","instrument":instrument}})
                    }
                    _ => return Err(p.error("field is unavailable in event context")),
                };
                p.inputs
                    .insert(name.clone(), json!({"field":field,"type":ty}));
                format!("$.bindings.{name}")
            }
            "node" => {
                p.expect("=")?;
                let provider = p.string()?;
                p.expect("@")?;
                let provider_version = p.string()?;
                let d = p
                    .descriptors
                    .get(&format!("{provider}@{provider_version}"))
                    .cloned()
                    .ok_or_else(|| {
                        p.error("provider ID/version not present in contextual catalog")
                    })?;
                let mut arguments = Map::new();
                p.expect("(")?;
                if !p.eat(")") {
                    loop {
                        let key = p.id()?;
                        p.expect("=")?;
                        let parameter = d["parameters"]["entries"]
                            .as_array()
                            .expect("descriptor schema")
                            .iter()
                            .find(|v| v["key"] == key)
                            .ok_or_else(|| p.error("unknown provider parameter"))?;
                        let argument = if p.eat("$") {
                            json!({"parameter":p.id()?})
                        } else {
                            json!({"literal":p.value(&parameter["type"])?})
                        };
                        if arguments.insert(key, argument).is_some() {
                            return Err(p.error("duplicate provider argument"));
                        }
                        if p.eat(")") {
                            break;
                        }
                        p.expect(",")?;
                    }
                }
                let mut inputs = vec![];
                let mut names = BTreeSet::new();
                p.expect("(")?;
                if !p.eat(")") {
                    loop {
                        let key = p.id()?;
                        p.expect("=")?;
                        let source = p.id()?;
                        if !names.insert(key.clone()) {
                            return Err(p.error("duplicate input port"));
                        }
                        inputs.push(json!({"name":key,"source":p.wire(&source)?}));
                        if p.eat(")") {
                            break;
                        }
                        p.expect(",")?;
                    }
                }
                let ports = d["outputs"]
                    .as_array()
                    .expect("descriptor ports")
                    .iter()
                    .map(|v| json!({"name":v["name"],"type":v["type"]}))
                    .collect::<Vec<_>>();
                for port in &ports {
                    p.output_types.insert(
                        format!("{name}.{}", port["name"].as_str().expect("port name")),
                        port["type"].clone(),
                    );
                }
                entries.insert(format!("{provider}@{provider_version}"),json!({"key":d["key"],"descriptor_version":d["descriptor_version"],"role":d["role"],"artifact":d["artifact"]}));
                nodes.push(json!({"id":name,"target":{"kind":"provider","key":d["key"]},"arguments":arguments,"inputs":inputs,"outputs":ports}));
                format!("$.graph.nodes[{}]", nodes.len() - 1)
            }
            "cross" => {
                p.expect("=")?;
                let direction = p.id()?;
                if !["above", "below"].contains(&direction.as_str()) {
                    return Err(p.error("cross direction must be above/below"));
                }
                p.expect("(")?;
                let left = p.expression(0)?;
                p.expect(",")?;
                let right = p.expression(0)?;
                p.expect(")")?;
                crossings.push(json!({"id":name,"direction":direction,"left":left,"right":right}));
                format!("$.crossings[{}]", crossings.len() - 1)
            }
            "output" => {
                p.expect("=")?;
                let expression = p.expression(0)?;
                outputs.push(json!({"name":name,"expression":expression}));
                format!("$.outputs[{}]", outputs.len() - 1)
            }
            "policy" => {
                p.expect("=")?;
                let action = p.id()?;
                p.expect("(")?;
                let output = p.id()?;
                let action = match action.as_str() {
                    "buy_if_flat" => {
                        p.expect(",")?;
                        let quantity = p.string()?;
                        p.expect(",")?;
                        let cash_cap = p.string()?;
                        json!({"kind":"buy_if_flat","quantity":quantity,"cash_cap":cash_cap})
                    }
                    "close_long" => json!({"kind":"close_long"}),
                    _ => return Err(p.error("unsupported financial policy")),
                };
                p.expect(")")?;
                policies.push(json!({"id":name,"source":{"kind":"event_boolean","name":output},"action":action}));
                format!("$.policies[{}]", policies.len() - 1)
            }
            _ => return Err(p.error("unsupported declaration")),
        };
        p.expect(";")?;
        p.mark(format!("{declaration}.{name}"), path, start)?;
    }
    let graph_outputs = p
        .output_types
        .iter()
        .map(|(name, ty)| Ok(json!({"name":name,"type":ty,"source":p.wire(name)?})))
        .collect::<Result<Vec<_>>>()?;
    let graph_inputs = p
        .inputs
        .iter()
        .map(|(name, v)| json!({"name":name,"type":v["type"]}))
        .collect::<Vec<_>>();
    let bindings = p
        .inputs
        .iter()
        .map(|(name, v)| (name.clone(), v["field"].clone()))
        .collect::<Map<_, _>>();
    let model = json!({"format":"fqe.event-plan","schema_version":1,"id":id,"version":version,
        "requirements":["event.plan/1","expression.scalar/1"],"trigger":trigger,"bindings":bindings,
        "warmup_samples":warmup,"crossings":crossings,"outputs":outputs,"signals":[],
        "graph":{"format":"fqe.graph","schema_version":1,"ir_version":2,"id":format!("{id}.graph"),"version":version,
            "parameters":{"version":1,"entries":params},"requirements":["model.typed-graph/1"],"entries":entries.into_values().collect::<Vec<_>>(),
            "graph":{"inputs":graph_inputs,"nodes":nodes,"outputs":graph_outputs},"modules":[],"metadata":{},"workflow":{},"state":[],"dependencies":[]}});
    let loaded =
        LoadedEventPlan::from_bytes(&serde_json::to_vec(&model).expect("author model JSON"))
            .map_err(|e| native_error(e, &p.maps))?;
    let compiled = loaded
        .compile(registry, BTreeMap::new())
        .map_err(|e| native_error(e, &p.maps))?;
    for policy in &policies {
        if !compiled.roots().output_types().iter().any(|port| {
            policy["source"]["name"] == port.name
                && *port.value_type.scalar() == fqe_core::types::ScalarType::Bool
        }) {
            return Err(Diagnostic::new(
                "authoring_policy",
                0,
                "policy requires a Boolean Event output",
            ));
        }
        for field in ["quantity", "cash_cap"] {
            if let Some(v) = policy["action"][field].as_str() {
                let amount = fqe_core::finance::parse_decimal(v, 18).map_err(|_| {
                    Diagnostic::new(
                        "authoring_policy",
                        0,
                        "policy requires an exact decimal string",
                    )
                })?;
                if amount.coefficient() <= 0 {
                    return Err(Diagnostic::new(
                        "authoring_policy",
                        0,
                        "policy quantity/cap must be positive",
                    ));
                }
            }
        }
    }
    Ok(Document {
        semantic: json!({"format":"fqe.author-document","schema_version":1,"model":model,"policies":policies}),
        source_map: Value::Array(p.maps),
        comments,
    })
}
fn native_error(error: ModelError, maps: &[Value]) -> Diagnostic {
    let offset = maps
        .iter()
        .filter(|m| error.path.starts_with(m["path"].as_str().unwrap_or("!")))
        .max_by_key(|m| m["path"].as_str().unwrap_or("").len())
        .and_then(|m| m["start"].as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Diagnostic {
        code: error.code.into(),
        path: error.path,
        offset,
        message: error.detail,
    }
}

fn unsupported() -> Diagnostic {
    Diagnostic::new(
        "authoring_unrepresentable",
        0,
        "model is outside the exact authoring/1 subset; preserve the original",
    )
}
fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or_else(unsupported)
}
fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(unsupported)
}
fn quote(v: &Value) -> Result<String> {
    Ok(serde_json::to_string(text(v)?).expect("quoted string"))
}
fn type_text(v: &Value) -> Result<String> {
    match text(&v["kind"])? {
        "u64" | "bool" => Ok(text(&v["kind"])?.into()),
        "decimal" => {
            let scale = v["scale"].as_u64().ok_or_else(unsupported)?;
            let unit = &v["unit"];
            let kind = text(&unit["kind"])?;
            let suffix = match kind {
                "ratio" => String::new(),
                "price" | "quantity" => format!(", {}", quote(&unit["instrument"])?),
                "money" => format!(", {}", quote(&unit["currency"])?),
                _ => return Err(unsupported()),
            };
            Ok(format!("decimal({scale}, {kind}{suffix})"))
        }
        _ => Err(unsupported()),
    }
}
fn value_text(v: &Value) -> Result<String> {
    if v["type"]["kind"] == "bool" {
        v["value"]
            .as_bool()
            .map(|b| b.to_string())
            .ok_or_else(unsupported)
    } else if v["type"]["kind"] == "decimal" {
        let scale = v["type"]["scale"]
            .as_u64()
            .filter(|s| *s <= 18)
            .ok_or_else(unsupported)? as usize;
        let coefficient = text(&v["value"])?
            .parse::<i128>()
            .map_err(|_| unsupported())?;
        let digits = coefficient.unsigned_abs().to_string();
        let sign = if coefficient < 0 { "-" } else { "" };
        let plain = if scale == 0 {
            format!("{sign}{digits}")
        } else {
            let padded = format!("{digits:0>width$}", width = scale + 1);
            let (whole, fraction) = padded.split_at(padded.len() - scale);
            format!("{sign}{whole}.{fraction}")
        };
        Ok(serde_json::to_string(&plain).expect("quoted decimal"))
    } else {
        quote(&v["value"])
    }
}
fn source_text(v: &Value) -> Result<String> {
    if let Some(input) = v.get("input") {
        Ok(text(input)?.into())
    } else {
        Ok(format!("{}.{}", text(&v["node"])?, text(&v["port"])?))
    }
}
fn expression_text(v: &Value, depth: usize) -> Result<String> {
    if depth > 16 {
        return Err(unsupported());
    }
    if let Some(p) = v.get("parameter") {
        return Ok(format!("${}", text(p)?));
    }
    for key in ["input", "output"] {
        if let Some(n) = v.get(key) {
            return Ok(text(n)?.into());
        }
    }
    if let Some(c) = v.get("cross") {
        return Ok(format!("cross({})", text(c)?));
    }
    if let Some(n) = v.get("not") {
        return Ok(format!("not({})", expression_text(n, depth + 1)?));
    }
    if let Some(b) = v.get("binary") {
        if b.get("rounding").is_some_and(|r| r != "exact") {
            return Err(unsupported());
        }
        return Ok(format!(
            "{}({}, {})",
            text(&b["op"])?,
            expression_text(&b["left"], depth + 1)?,
            expression_text(&b["right"], depth + 1)?
        ));
    }
    Err(unsupported())
}
fn canonical_model(model: &Value) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(model).map_err(|_| unsupported())?;
    let loaded = LoadedEventPlan::from_bytes(&bytes).map_err(|e| native_error(e, &[]))?;
    Ok(loaded.canonical_bytes().to_vec())
}

/// Render only exactly representable semantic documents. Canonical round-trip
/// validation rejects lossy imports, unknown metadata or unsupported native modules.
/// Comments are retained in order as a leading comment block, never executed.
pub fn print(semantic: &Value, comments: &[String]) -> Result<String> {
    if semantic["format"] != "fqe.author-document"
        || semantic["schema_version"] != 1
        || semantic.as_object().is_none_or(|m| m.len() != 4)
    {
        return Err(unsupported());
    }
    let model = &semantic["model"];
    let g = &model["graph"];
    let mut lines = vec![];
    for c in comments {
        if !c.starts_with("//") || c.contains(['\r', '\n']) {
            return Err(unsupported());
        }
        lines.push(c.clone());
    }
    lines.push("fqe 1;".into());
    lines.push(format!(
        "strategy {} version {};",
        quote(&model["id"])?,
        quote(&model["version"])?
    ));
    let t = &model["trigger"];
    match text(&t["kind"])? {
        "tick" => lines.push(format!("context tick {};", quote(&t["instrument"])?)),
        "closed_bar" => lines.push(format!(
            "context closed_bar {} timeframe {} price_scale {} volume_scale {};",
            quote(&t["instrument"])?,
            quote(&t["timeframe_ns"])?,
            t["price_scale"],
            t["volume_scale"]
        )),
        _ => return Err(unsupported()),
    }
    lines.push(format!("warmup {};", quote(&model["warmup_samples"])?));
    for p in array(&g["parameters"]["entries"])? {
        if p["required"] != false {
            return Err(unsupported());
        }
        let mut line = format!(
            "param {} : {} = {}",
            text(&p["key"])?,
            type_text(&p["type"])?,
            value_text(&p["default"])?
        );
        let constraints = &p["constraints"];
        if constraints.get("lower").is_some() || constraints.get("upper").is_some() {
            if constraints["lower"]["inclusive"] != true
                || constraints["upper"]["inclusive"] != true
            {
                return Err(unsupported());
            }
            line.push_str(&format!(
                " within ({}, {})",
                value_text(&constraints["lower"]["value"])?,
                value_text(&constraints["upper"]["value"])?
            ));
        }
        let description = text(&p["description"])?;
        if !description.is_empty() {
            line.push_str(&format!(" description {}", quote(&p["description"])?));
        }
        lines.push(format!("{line};"));
    }
    for input in array(&g["graph"]["inputs"])? {
        let name = text(&input["name"])?;
        lines.push(format!(
            "input {name} = {};",
            text(&model["bindings"][name])?
        ));
    }
    // Native canonical order is lexical; author source needs upstream declarations first.
    let mut remaining = array(&g["graph"]["nodes"])?.iter().collect::<Vec<_>>();
    let mut emitted = BTreeSet::new();
    while !remaining.is_empty() {
        let index = remaining
            .iter()
            .position(|n| {
                array(&n["inputs"]).is_ok_and(|ports| {
                    ports.iter().all(|p| {
                        p["source"].get("input").is_some()
                            || p["source"]["node"]
                                .as_str()
                                .is_some_and(|id| emitted.contains(id))
                    })
                })
            })
            .ok_or_else(unsupported)?;
        let n = remaining.remove(index);
        if n["target"]["kind"] != "provider" {
            return Err(unsupported());
        }
        let mut args = vec![];
        for (key, a) in n["arguments"].as_object().ok_or_else(unsupported)? {
            args.push(format!(
                "{key}={}",
                if let Some(p) = a.get("parameter") {
                    format!("${}", text(p)?)
                } else {
                    value_text(&a["literal"])?
                }
            ));
        }
        let inputs = array(&n["inputs"])?
            .iter()
            .map(|v| {
                Ok(format!(
                    "{}={}",
                    text(&v["name"])?,
                    source_text(&v["source"])?
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let name = text(&n["id"])?;
        lines.push(format!(
            "node {name} = {} @ {} ({}) ({});",
            quote(&n["target"]["key"]["id"])?,
            quote(&n["target"]["key"]["version"])?,
            args.join(", "),
            inputs.join(", ")
        ));
        emitted.insert(name.to_owned());
    }
    for c in array(&model["crossings"])? {
        lines.push(format!(
            "cross {} = {}({}, {});",
            text(&c["id"])?,
            text(&c["direction"])?,
            expression_text(&c["left"], 0)?,
            expression_text(&c["right"], 0)?
        ));
    }
    for o in array(&model["outputs"])? {
        lines.push(format!(
            "output {} = {};",
            text(&o["name"])?,
            expression_text(&o["expression"], 0)?
        ));
    }
    for p in array(&semantic["policies"])? {
        if p["source"]["kind"] != "event_boolean" {
            return Err(unsupported());
        }
        let a = &p["action"];
        let root = text(&p["source"]["name"])?;
        let action = match text(&a["kind"])? {
            "buy_if_flat" => format!(
                "buy_if_flat({root}, {}, {})",
                quote(&a["quantity"])?,
                quote(&a["cash_cap"])?
            ),
            "close_long" => format!("close_long({root})"),
            _ => return Err(unsupported()),
        };
        lines.push(format!("policy {} = {action};", text(&p["id"])?));
    }
    let source = lines.join("\n") + "\n";
    let parsed = parse(&source)?;
    if canonical_model(&parsed.semantic["model"])? != canonical_model(model)?
        || parsed.semantic["policies"] != semantic["policies"]
    {
        return Err(unsupported());
    }
    Ok(source)
}

/// Pure authoring RPCs return diagnostics as data; they never admit a financial task.
pub(crate) fn inspect_rpc(p: &Value) -> crate::Result<Value> {
    crate::fields(p, &["source", "parameters"])?;
    let source = crate::text(p, "source")?;
    let document = match parse(source) {
        Ok(d) => d,
        Err(e) => return Ok(json!({"valid":false,"diagnostics":[e.value(source)]})),
    };
    let request = crate::Request {
        id: "authoring".into(),
        method: "model.inspect".into(),
        params: json!({"kind":"event","model":serde_json::to_string(&document.semantic["model"]).expect("model JSON"),"parameters":crate::text(p,"parameters")?}),
    };
    match crate::execute(&request, &std::sync::atomic::AtomicBool::new(false)) {
        Ok(native) => Ok(
            json!({"valid":true,"document":document.semantic,"source_map":document.source_map,
            "comments":document.comments,"native":native,"diagnostics":[]}),
        ),
        Err(e) => Ok(
            json!({"valid":false,"diagnostics":[Diagnostic{code:e.code.into(),path:e.path,offset:0,
            message:"native parameter binding or model inspection failed".into()}.value(source)]}),
        ),
    }
}
pub(crate) fn print_rpc(p: &Value) -> crate::Result<Value> {
    crate::fields(p, &["document", "comments"])?;
    let comments = p["comments"]
        .as_array()
        .ok_or_else(|| crate::fail("authoring_comments"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| crate::fail("authoring_comments"))
        })
        .collect::<crate::Result<Vec<_>>>()?;
    match print(&p["document"], &comments) {
        Ok(source) => Ok(json!({"valid":true,"source":source,"diagnostics":[]})),
        Err(e) => Ok(json!({"valid":false,"diagnostics":[e.value("")]})),
    }
}

/// Reference transaction model for clients: a pending buffer blocks execution
/// and graph edits until explicitly applied or discarded by the user.
pub struct Draft {
    revision: u64,
    generation: u64,
    buffer: String,
    applied_source: String,
    document: Document,
    pending: bool,
}
impl Draft {
    pub fn new(source: &str) -> Result<Self> {
        Ok(Self {
            revision: 0,
            generation: 0,
            buffer: source.into(),
            applied_source: source.into(),
            document: parse(source)?,
            pending: false,
        })
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn buffer(&self) -> &str {
        &self.buffer
    }
    /// Source-only draft persistence; restoring never executes the strategy.
    /// Invalid pending text survives while the valid model is rebuilt natively.
    pub fn snapshot(&self) -> Value {
        json!({"format":"fqe.author-draft","schema_version":1,"revision":self.revision.to_string(),
            "generation":self.generation.to_string(),"applied_source":self.applied_source,
            "buffer":self.buffer,"pending":self.pending})
    }
    pub fn restore(value: &Value) -> Result<Self> {
        if value["format"] != "fqe.author-draft"
            || value["schema_version"] != 1
            || value.as_object().is_none_or(|m| m.len() != 7)
        {
            return Err(unsupported());
        }
        let number = |key: &str| -> Result<u64> {
            let text = text(&value[key])?;
            let n = text.parse::<u64>().map_err(|_| unsupported())?;
            if n.to_string() != text {
                return Err(unsupported());
            }
            Ok(n)
        };
        let applied = text(&value["applied_source"])?;
        let buffer = text(&value["buffer"])?;
        let pending = value["pending"].as_bool().ok_or_else(unsupported)?;
        if buffer.len() > MAX_SOURCE_BYTES || (!pending && buffer != applied) {
            return Err(unsupported());
        }
        Ok(Self {
            revision: number("revision")?,
            generation: number("generation")?,
            buffer: buffer.into(),
            applied_source: applied.into(),
            document: parse(applied)?,
            pending,
        })
    }
    pub fn edit_text(&mut self, source: &str) -> Result<()> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(Diagnostic::new("authoring_quota", 0, "buffer too large"));
        }
        self.generation = self.generation.checked_add(1).ok_or_else(unsupported)?;
        self.buffer = source.into();
        self.pending = true;
        Ok(())
    }
    pub fn apply(&mut self, revision: u64, generation: u64) -> Result<()> {
        self.check(revision, generation)?;
        let candidate = parse(&self.buffer)?;
        self.install(candidate)?;
        self.applied_source = self.buffer.clone();
        Ok(())
    }
    pub fn edit_graph(&mut self, revision: u64, generation: u64, semantic: &Value) -> Result<()> {
        self.check(revision, generation)?;
        if self.pending {
            return Err(Diagnostic::new(
                "authoring_conflict",
                0,
                "resolve unapplied text before graph/parameter edits",
            ));
        }
        let source = print(semantic, &self.document.comments)?;
        let candidate = parse(&source)?;
        let generation = self.generation.checked_add(1).ok_or_else(unsupported)?;
        self.install(candidate)?;
        self.generation = generation;
        self.applied_source = source.clone();
        self.buffer = source;
        Ok(())
    }
    pub fn discard_buffer(&mut self, revision: u64, generation: u64) -> Result<()> {
        self.check(revision, generation)?;
        self.generation = self.generation.checked_add(1).ok_or_else(unsupported)?;
        self.buffer = self.applied_source.clone();
        self.pending = false;
        Ok(())
    }
    pub fn executable(&self) -> Result<&Document> {
        if self.pending {
            Err(Diagnostic::new(
                "authoring_unapplied",
                0,
                "current text is unapplied; previous model cannot run",
            ))
        } else {
            Ok(&self.document)
        }
    }
    fn check(&self, revision: u64, generation: u64) -> Result<()> {
        if revision != self.revision || generation != self.generation {
            Err(Diagnostic::new(
                "authoring_conflict",
                0,
                "stale semantic revision or text generation",
            ))
        } else {
            Ok(())
        }
    }
    fn install(&mut self, candidate: Document) -> Result<()> {
        if canonical_model(&candidate.semantic["model"])?
            != canonical_model(&self.document.semantic["model"])?
            || candidate.semantic["policies"] != self.document.semantic["policies"]
        {
            self.revision = self.revision.checked_add(1).ok_or_else(unsupported)?;
        }
        self.document = candidate;
        self.pending = false;
        Ok(())
    }
}
