//! Immutable, structurally validated declarations. Loading does not compile or run code.

use std::{collections::BTreeMap, collections::BTreeSet, fmt};

use fqe_core::types::{TypeDescriptor, TypedValue};

use crate::parameters::{BoundParameters, BoundValue, ParameterInput, ParameterSchema};

pub const MODEL_SCHEMA_VERSION: u32 = 1;
pub const MODEL_IR_VERSION: u32 = 2;
pub const MAX_MODEL_BYTES: usize = 262_144;
pub const MAX_NODES: usize = 256;
pub const MAX_PORTS: usize = 32;
pub const MAX_EDGES: usize = 4096;
pub const SUPPORTED_CAPABILITIES: &[&str] = &["model.scalar-graph/1", "values/1", "parameters/1"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelError {
    pub code: &'static str,
    pub path: String,
    pub detail: String,
}

impl ModelError {
    pub(crate) fn new(
        code: &'static str,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            detail: detail.into(),
        }
    }
}
impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}: {}", self.code, self.path, self.detail)
    }
}
impl std::error::Error for ModelError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentId([u8; 32]);

impl ContentId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn parse(text: &str) -> Result<Self, ModelError> {
        let Some(hex) = text.strip_prefix("sha256:") else {
            return Err(ModelError::new(
                "identity",
                "$",
                "expected sha256: followed by 64 lowercase hex digits",
            ));
        };
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ModelError::new(
                "identity",
                "$",
                "expected 64 lowercase hex digits",
            ));
        }
        let mut bytes = [0; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|_| ModelError::new("identity", "$", "invalid digest"))?;
        }
        Ok(Self(bytes))
    }
}
impl fmt::Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("sha256:")?;
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntryKey {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryRequirement {
    pub key: EntryKey,
    pub descriptor_version: u32,
    pub role: String,
    /// A pinned declaration, not proof that the referenced provider is installed/trusted.
    pub artifact: ContentId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    pub name: String,
    pub value_type: TypeDescriptor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortRef {
    pub node: String,
    pub port: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    pub name: String,
    pub source: PortRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Argument {
    Literal(TypedValue),
    Parameter(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub entry: EntryKey,
    pub arguments: BTreeMap<String, Argument>,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Port>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelDraft {
    pub schema_version: u32,
    pub ir_version: u32,
    pub id: String,
    pub version: String,
    pub parameters: ParameterSchema,
    pub requirements: Vec<String>,
    pub entries: Vec<EntryRequirement>,
    pub nodes: Vec<Node>,
    pub outputs: Vec<Input>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelDefinition {
    draft: ModelDraft,
}

pub(crate) fn identifier(value: &str, path: &str) -> Result<(), ModelError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-:/".contains(&b))
    {
        return Err(ModelError::new(
            "identifier",
            path,
            "expected 1..128 ASCII identifier bytes",
        ));
    }
    Ok(())
}

fn unique<'a>(items: impl Iterator<Item = &'a str>, path: &str) -> Result<(), ModelError> {
    let mut names = BTreeSet::new();
    for name in items {
        identifier(name, path)?;
        if !names.insert(name) {
            return Err(ModelError::new(
                "duplicate_id",
                path,
                "duplicate identifier",
            ));
        }
    }
    Ok(())
}

impl ModelDefinition {
    pub fn new(mut draft: ModelDraft) -> Result<Self, ModelError> {
        if draft.schema_version != MODEL_SCHEMA_VERSION || draft.ir_version != MODEL_IR_VERSION {
            return Err(ModelError::new(
                "version",
                "$",
                "expected model schema 1 and model IR 2",
            ));
        }
        identifier(&draft.id, "$.id")?;
        identifier(&draft.version, "$.version")?;
        if draft.entries.is_empty()
            || draft.entries.len() > MAX_NODES
            || draft.nodes.is_empty()
            || draft.nodes.len() > MAX_NODES
            || draft.outputs.is_empty()
            || draft.outputs.len() > MAX_PORTS
            || draft.requirements.len() > 32
            || draft.metadata.len() > 32
        {
            return Err(ModelError::new(
                "limit",
                "$",
                "invalid model collection size",
            ));
        }
        unique(
            draft.requirements.iter().map(String::as_str),
            "$.requirements",
        )?;
        for capability in &draft.requirements {
            if !SUPPORTED_CAPABILITIES.contains(&capability.as_str()) {
                return Err(ModelError::new(
                    "capability",
                    "$.requirements",
                    "unsupported required capability",
                ));
            }
        }
        let mut entry_keys = BTreeSet::new();
        for entry in &draft.entries {
            identifier(&entry.key.id, "$.entries.id")?;
            identifier(&entry.key.version, "$.entries.version")?;
            if !entry_keys.insert(entry.key.clone()) {
                return Err(ModelError::new(
                    "duplicate_id",
                    "$.entries",
                    "duplicate entry ID/version",
                ));
            }
            if entry.descriptor_version != 1 {
                return Err(ModelError::new(
                    "version",
                    "$.entries.descriptor_version",
                    "unsupported descriptor version",
                ));
            }
            if entry.role != "provider" {
                return Err(ModelError::new(
                    "unsupported",
                    "$.entries.role",
                    "only provider references are supported",
                ));
            }
        }
        unique(draft.nodes.iter().map(|n| n.id.as_str()), "$.graph.nodes")?;
        unique(
            draft.outputs.iter().map(|p| p.name.as_str()),
            "$.graph.outputs",
        )?;
        let parameter_keys: BTreeSet<_> = draft.parameters.specs().map(|p| p.key()).collect();
        let mut edge_count = 0;
        for node in &mut draft.nodes {
            if !entry_keys.contains(&node.entry) {
                return Err(ModelError::new(
                    "reference",
                    "$.graph.nodes.entry",
                    "entry ID/version is not declared",
                ));
            }
            if node.inputs.len() > MAX_PORTS
                || node.outputs.is_empty()
                || node.outputs.len() > MAX_PORTS
                || node.arguments.len() > 256
            {
                return Err(ModelError::new(
                    "limit",
                    "$.graph.nodes",
                    "invalid port/argument count",
                ));
            }
            edge_count += node.inputs.len();
            unique(
                node.inputs.iter().map(|p| p.name.as_str()),
                "$.graph.nodes.inputs",
            )?;
            unique(
                node.outputs.iter().map(|p| p.name.as_str()),
                "$.graph.nodes.outputs",
            )?;
            for (key, argument) in &node.arguments {
                identifier(key, "$.graph.nodes.arguments")?;
                if let Argument::Parameter(key) = argument
                    && !parameter_keys.contains(key.as_str())
                {
                    return Err(ModelError::new(
                        "reference",
                        "$.graph.nodes.arguments",
                        "parameter is not declared",
                    ));
                }
            }
            node.inputs.sort_by(|a, b| a.name.cmp(&b.name));
            node.outputs.sort_by(|a, b| a.name.cmp(&b.name));
        }
        if edge_count > MAX_EDGES {
            return Err(ModelError::new("limit", "$.graph", "too many edges"));
        }
        for source in draft
            .nodes
            .iter()
            .flat_map(|n| n.inputs.iter())
            .chain(draft.outputs.iter())
            .map(|p| &p.source)
        {
            identifier(&source.node, "$.graph.source.node")?;
            identifier(&source.port, "$.graph.source.port")?;
            if !draft
                .nodes
                .iter()
                .any(|n| n.id == source.node && n.outputs.iter().any(|p| p.name == source.port))
            {
                return Err(ModelError::new(
                    "reference",
                    "$.graph.source",
                    "source node/output is not declared",
                ));
            }
        }
        for (key, value) in &draft.metadata {
            identifier(key, "$.metadata")?;
            if value.len() > 4096 {
                return Err(ModelError::new(
                    "limit",
                    "$.metadata",
                    "metadata text too long",
                ));
            }
        }
        draft.entries.sort_by(|a, b| a.key.cmp(&b.key));
        draft.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        draft.outputs.sort_by(|a, b| a.name.cmp(&b.name));
        draft.requirements.sort();
        Ok(Self { draft })
    }

    pub fn declaration(&self) -> &ModelDraft {
        &self.draft
    }

    /// Prepared parameters only; there is no execution, account, cursor or mutable state.
    pub fn bind(
        &self,
        values: impl IntoIterator<Item = (String, ParameterInput)>,
    ) -> Result<BoundParameters, ModelError> {
        let bound = self
            .draft
            .parameters
            .bind(values)
            .map_err(|e| ModelError::new("parameter", "$.bindings", e.to_string()))?;
        for node in &self.draft.nodes {
            for argument in node.arguments.values() {
                if let Argument::Parameter(key) = argument
                    && matches!(bound.get(key), Some(BoundValue::Unbound))
                {
                    return Err(ModelError::new(
                        "unbound",
                        "$.graph.nodes.arguments",
                        "a graph reference requires a bound parameter",
                    ));
                }
            }
        }
        Ok(bound)
    }
}
