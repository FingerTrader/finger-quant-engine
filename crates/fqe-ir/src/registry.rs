//! Frozen, exact-pinned registry for reviewed pure/incremental scalar providers.
//! This is neither a graph compiler nor an execution sandbox or external loader.
use crate::{
    loading::{LoadedModel, ModelBinding, parameter_json, type_json},
    model::{
        Argument, ContentId, EntryKey, EntryRequirement, MAX_MODEL_BYTES, MAX_NODES, MAX_PORTS,
        ModelError, identifier,
    },
    parameters::{BoundParameters, BoundValue, ParameterInput, ParameterSchema},
};
use fqe_core::{
    numeric::NumericError,
    types::{Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

type Result<T> = std::result::Result<T, ModelError>;
pub const DESCRIPTOR_VERSION: u32 = 1;
/// Bound on declared initial source samples, not recurrence influence or heap quota.
pub const MAX_INITIAL_HISTORY: u32 = 20000;
pub const PROVIDER_CONTRACT_VERSION: u32 = 1;
pub const ENGINE_API_VERSION: u32 = 1;
pub const REGISTRY_CAPABILITIES: &[&str] = &[
    "provider.pure-scalar/1",
    "provider.incremental-scalar/1",
    "values/1",
    "parameters/1",
];
pub const DESCRIPTOR_PREFIX: &[u8] = b"FQE-DESCRIPTOR-CANONICAL/1\n";
pub const REGISTRY_PREFIX: &[u8] = b"FQE-REGISTRY-CANONICAL/1\n";
fn error(code: &'static str, path: &str, detail: &str) -> ModelError {
    ModelError::new(code, path, detail)
}
fn hash(prefix: &[u8], bytes: &[u8]) -> ContentId {
    let mut h = Sha256::new();
    h.update(prefix);
    h.update(bytes);
    ContentId::from_bytes(h.finalize().into())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cardinality {
    One,
    Optional,
    Many { max: usize },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallablePort {
    pub name: String,
    pub value_type: TypeDescriptor,
    pub cardinality: Cardinality,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Category {
    Function,
    Predicate,
    Indicator,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Purity {
    Pure,
    Stateful,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadinessPolicy {
    Propagate,
    ProviderManaged,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compatibility {
    pub engine_api: u32,
    pub provider_contract: u32,
    pub ir_min: u32,
    pub ir_max: u32,
}
impl Default for Compatibility {
    fn default() -> Self {
        Self {
            engine_api: ENGINE_API_VERSION,
            provider_contract: PROVIDER_CONTRACT_VERSION,
            ir_min: 2,
            ir_max: 2,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
}
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 65536,
            max_output_bytes: 65536,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptorDraft {
    pub descriptor_version: u32,
    pub key: EntryKey,
    /// SHA-256 of a reviewed implementation artifact, not a trust signature.
    pub artifact: ContentId,
    pub role: String,
    pub category: Category,
    pub compatibility: Compatibility,
    pub inputs: Vec<CallablePort>,
    pub outputs: Vec<CallablePort>,
    pub parameters: ParameterSchema,
    pub purity: Purity,
    pub readiness: ReadinessPolicy,
    pub deterministic: bool,
    pub history: u32,
    pub state_schema: Option<String>,
    pub required_capabilities: Vec<String>,
    pub host_services: Vec<String>,
    pub dependencies: Vec<EntryKey>,
    pub resources: ResourceLimits,
    /// Versioned formula, missing and precision semantics; never executable text.
    pub semantics: String,
    pub metadata: BTreeMap<String, String>,
}
impl CallablePort {
    pub fn one(name: impl Into<String>, value_type: TypeDescriptor) -> Self {
        Self {
            name: name.into(),
            value_type,
            cardinality: Cardinality::One,
        }
    }
}
impl DescriptorDraft {
    /// A pure scalar draft; callers must supply at least one output before validation.
    pub fn pure(
        key: EntryKey,
        artifact: ContentId,
        parameters: ParameterSchema,
        semantics: impl Into<String>,
    ) -> Self {
        Self {
            descriptor_version: DESCRIPTOR_VERSION,
            key,
            artifact,
            role: "provider".into(),
            category: Category::Function,
            compatibility: Compatibility::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            parameters,
            purity: Purity::Pure,
            readiness: ReadinessPolicy::Propagate,
            deterministic: true,
            history: 0,
            state_schema: None,
            required_capabilities: vec!["provider.pure-scalar/1".into()],
            host_services: Vec::new(),
            dependencies: Vec::new(),
            resources: ResourceLimits::default(),
            semantics: semantics.into(),
            metadata: BTreeMap::new(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    draft: DescriptorDraft,
    canonical: Vec<u8>,
    content_id: ContentId,
}
fn validate_ports(ports: &[CallablePort], path: &str) -> Result<()> {
    if ports.len() > MAX_PORTS {
        return Err(error("registry_limit", path, "too many ports"));
    }
    let mut names = BTreeSet::new();
    for p in ports {
        identifier(&p.name, path)?;
        if !names.insert(&p.name) {
            return Err(error("registry_duplicate", path, "duplicate port name"));
        }
        if p.cardinality != Cardinality::One {
            return Err(error(
                "registry_unsupported",
                path,
                "only exactly-one scalar ports are implemented",
            ));
        }
    }
    Ok(())
}
fn payload_size(v: &TypedValue) -> usize {
    match v.value() {
        ScalarValue::Bool(_) => 1,
        ScalarValue::I64(_) | ScalarValue::U64(_) => 8,
        ScalarValue::Decimal(_) => 16,
        ScalarValue::Text(s) => s.len(),
    }
}
fn schema_budget(schema: &ParameterSchema) -> Result<()> {
    // Conservative preflight bounds the temporary JSON tree before serialization.
    let mut cost = 0;
    for s in schema.specs() {
        cost += 1024 + s.description().len();
        let c = s.constraints();
        for v in s
            .default_value()
            .into_iter()
            .chain(c.lower.iter().map(|b| &b.value))
            .chain(c.upper.iter().map(|b| &b.value))
            .chain(c.choices.iter().flatten())
        {
            cost += 512 + payload_size(v);
            if cost > MAX_MODEL_BYTES {
                return Err(error(
                    "registry_limit",
                    "$.parameters",
                    "descriptor parameter budget exceeded",
                ));
            }
        }
        if cost > MAX_MODEL_BYTES {
            return Err(error(
                "registry_limit",
                "$.parameters",
                "descriptor parameter budget exceeded",
            ));
        }
    }
    Ok(())
}
impl Descriptor {
    pub fn new(mut draft: DescriptorDraft) -> Result<Self> {
        identifier(&draft.key.id, "$.key.id")?;
        identifier(&draft.key.version, "$.key.version")?;
        if !draft
            .key
            .id
            .split_once(['.', ':'])
            .is_some_and(|(namespace, name)| !namespace.is_empty() && !name.is_empty())
        {
            return Err(error(
                "registry_identifier",
                "$.key.id",
                "provider IDs must be namespaced",
            ));
        }
        let c = &draft.compatibility;
        if draft.descriptor_version != DESCRIPTOR_VERSION
            || c.engine_api != ENGINE_API_VERSION
            || c.provider_contract != PROVIDER_CONTRACT_VERSION
            || c.ir_min == 0
            || c.ir_min > 2
            || c.ir_max < 2
            || c.ir_min > c.ir_max
        {
            return Err(error(
                "registry_version",
                "$.compatibility",
                "unsupported descriptor/API/provider/IR requirement",
            ));
        }
        let pure = draft.purity == Purity::Pure
            && draft.readiness == ReadinessPolicy::Propagate
            && draft.history == 0
            && draft.state_schema.is_none()
            && !draft
                .required_capabilities
                .iter()
                .any(|c| c == "provider.incremental-scalar/1");
        let incremental = draft.purity == Purity::Stateful
            && draft.readiness == ReadinessPolicy::ProviderManaged
            && draft.history > 0
            && draft.history <= MAX_INITIAL_HISTORY
            && draft
                .state_schema
                .as_ref()
                .is_some_and(|s| identifier(s, "$.state_schema").is_ok())
            && draft
                .required_capabilities
                .iter()
                .any(|c| c == "provider.incremental-scalar/1")
            && !draft
                .required_capabilities
                .iter()
                .any(|c| c == "provider.pure-scalar/1");
        if draft.role != "provider"
            || !(pure || incremental)
            || !draft.deterministic
            || !draft.host_services.is_empty()
            || !draft.dependencies.is_empty()
        {
            return Err(error(
                "registry_unsupported",
                "$",
                "unsupported or inconsistent pure/incremental profile; services/dependencies are unsupported",
            ));
        }
        validate_ports(&draft.inputs, "$.inputs")?;
        validate_ports(&draft.outputs, "$.outputs")?;
        if draft.outputs.is_empty() {
            return Err(error(
                "registry_arity",
                "$.outputs",
                "at least one output is required",
            ));
        }
        if draft.category == Category::Indicator && !incremental {
            return Err(error(
                "registry_unsupported",
                "$.category",
                "indicators require the incremental profile",
            ));
        }
        if draft.category == Category::Predicate
            && (draft.outputs.len() != 1
                || draft.outputs[0].value_type.scalar() != &ScalarType::Bool)
        {
            return Err(error(
                "registry_type",
                "$.outputs",
                "predicate requires one Boolean output",
            ));
        }
        if draft.required_capabilities.len() > 32
            || draft.metadata.len() > 32
            || draft.semantics.is_empty()
            || draft.semantics.len() > 4096
        {
            return Err(error(
                "registry_limit",
                "$",
                "invalid metadata/capability/semantics size",
            ));
        }
        let mut capabilities = BTreeSet::new();
        for c in &draft.required_capabilities {
            if !REGISTRY_CAPABILITIES.contains(&c.as_str()) {
                return Err(error(
                    "registry_capability",
                    "$.required_capabilities",
                    "unsupported provider capability",
                ));
            }
            if !capabilities.insert(c) {
                return Err(error(
                    "registry_duplicate",
                    "$.required_capabilities",
                    "duplicate capability",
                ));
            }
        }
        for (k, v) in &draft.metadata {
            identifier(k, "$.metadata")?;
            if v.len() > 4096 {
                return Err(error(
                    "registry_limit",
                    "$.metadata",
                    "metadata value too long",
                ));
            }
        }
        for size in [
            draft.resources.max_input_bytes,
            draft.resources.max_output_bytes,
        ] {
            if size == 0 || size > MAX_MODEL_BYTES {
                return Err(error(
                    "registry_limit",
                    "$.resources",
                    "invalid scalar payload budget",
                ));
            }
        }
        schema_budget(&draft.parameters)?;
        draft.inputs.sort_by(|a, b| a.name.cmp(&b.name));
        draft.outputs.sort_by(|a, b| a.name.cmp(&b.name));
        draft.required_capabilities.sort();
        let canonical = serde_json::to_vec(&descriptor_json(&draft))
            .map_err(|_| error("registry_encoding", "$", "descriptor encoding failed"))?;
        if canonical.len() > MAX_MODEL_BYTES {
            return Err(error(
                "registry_limit",
                "$",
                "descriptor exceeds byte budget",
            ));
        }
        let content_id = hash(DESCRIPTOR_PREFIX, &canonical);
        Ok(Self {
            draft,
            canonical,
            content_id,
        })
    }
    pub fn declaration(&self) -> &DescriptorDraft {
        &self.draft
    }
    pub fn content_id(&self) -> ContentId {
        self.content_id
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn requirement(&self) -> EntryRequirement {
        EntryRequirement {
            key: self.draft.key.clone(),
            descriptor_version: DESCRIPTOR_VERSION,
            role: "provider".into(),
            artifact: self.draft.artifact,
        }
    }
}
fn descriptor_json(d: &DescriptorDraft) -> Value {
    let ports = |ps: &[CallablePort]| {
        ps.iter()
            .map(|p| json!({"name":p.name,"type":type_json(&p.value_type),"cardinality":"one"}))
            .collect::<Vec<_>>()
    };
    json!({"descriptor_version":d.descriptor_version,"key":{"id":d.key.id,"version":d.key.version},"artifact":d.artifact.to_string(),"role":d.role,
        "category":match d.category{Category::Function=>"function",Category::Predicate=>"predicate",Category::Indicator=>"indicator"},
        "compatibility":{"engine_api":d.compatibility.engine_api,"provider_contract":d.compatibility.provider_contract,"ir_min":d.compatibility.ir_min,"ir_max":d.compatibility.ir_max},
        "inputs":ports(&d.inputs),"outputs":ports(&d.outputs),"parameters":{"version":d.parameters.version(),"entries":d.parameters.specs().map(parameter_json).collect::<Vec<_>>()},
        "purity":match d.purity {Purity::Pure=>"pure",Purity::Stateful=>"stateful"},"readiness":match d.readiness {ReadinessPolicy::Propagate=>"propagate",ReadinessPolicy::ProviderManaged=>"provider-managed"},"deterministic":d.deterministic,"history":d.history,"state_schema":d.state_schema,"required_capabilities":d.required_capabilities,
        "host_services":[],"dependencies":[],"resources":{"max_input_bytes":d.resources.max_input_bytes,"max_output_bytes":d.resources.max_output_bytes},"semantics":d.semantics,"metadata":d.metadata})
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallContext {
    pub run: String,
    pub instance: String,
    pub event_sequence: u64,
    pub call_sequence: u64,
}
impl CallContext {
    fn validate(&self) -> Result<()> {
        identifier(&self.run, "$.call.run")?;
        identifier(&self.instance, "$.call.instance")?;
        if self.event_sequence == 0 || self.call_sequence == 0 {
            return Err(error(
                "registry_call",
                "$.call",
                "sequences must be positive",
            ));
        }
        Ok(())
    }
}
pub type NamedValues = Vec<(String, Evaluation<TypedValue>)>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderFailure {
    InvalidParameters,
    PrecisionLimit,
    Numeric(NumericError),
    Failed,
}
/// Only reviewed in-process implementations. Interior mutation, I/O and time limits
/// cannot be policed by this Rust trait; external isolation is a separate gate.
pub trait Provider: Send + Sync {
    fn validate_parameters(&self, _: &BoundParameters) -> std::result::Result<(), ProviderFailure> {
        Ok(())
    }
    fn invoke(&self, call: &ProviderCall<'_>) -> std::result::Result<NamedValues, ProviderFailure>;
}
pub struct ProviderCall<'a> {
    pub context: &'a CallContext,
    pub parameters: &'a BoundParameters,
    inputs: &'a BTreeMap<String, Evaluation<TypedValue>>,
}
impl ProviderCall<'_> {
    pub fn input(&self, name: &str) -> Option<&TypedValue> {
        match self.inputs.get(name) {
            Some(Evaluation::Ready(v)) => Some(v),
            _ => None,
        }
    }
}
enum Implementation {
    Pure(Arc<dyn Provider>),
    Incremental(Arc<dyn IncrementalProvider>),
}
struct Registration {
    descriptor: Arc<Descriptor>,
    implementation: Implementation,
}
/// Reviewed in-process state. Fork must deeply copy all mutable instance history.
/// This is a trust contract, not isolation or a persistent snapshot format.
pub trait IncrementalState: Send {
    fn fork(&self) -> Box<dyn IncrementalState>;
    fn step(
        &mut self,
        call: &ProviderCall<'_>,
    ) -> std::result::Result<NamedValues, ProviderFailure>;
}
pub trait IncrementalProvider: Send + Sync {
    fn validate_parameters(&self, _: &BoundParameters) -> std::result::Result<(), ProviderFailure> {
        Ok(())
    }
    fn create(
        &self,
        parameters: &BoundParameters,
    ) -> std::result::Result<Box<dyn IncrementalState>, ProviderFailure>;
}
#[derive(Default)]
pub struct RegistryBuilder {
    entries: BTreeMap<EntryKey, Registration>,
}
impl RegistryBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    /// Registration and discovery do not call provider code.
    pub fn register(&mut self, descriptor: Descriptor, provider: Arc<dyn Provider>) -> Result<()> {
        if descriptor.draft.purity != Purity::Pure {
            return Err(error(
                "registry_profile",
                "$",
                "pure registration requires pure descriptor",
            ));
        }
        self.insert(descriptor, Implementation::Pure(provider))
    }
    pub fn register_incremental(
        &mut self,
        descriptor: Descriptor,
        provider: Arc<dyn IncrementalProvider>,
    ) -> Result<()> {
        if descriptor.draft.purity != Purity::Stateful {
            return Err(error(
                "registry_profile",
                "$",
                "incremental registration requires stateful descriptor",
            ));
        }
        self.insert(descriptor, Implementation::Incremental(provider))
    }
    fn insert(&mut self, descriptor: Descriptor, implementation: Implementation) -> Result<()> {
        let key = descriptor.draft.key.clone();
        if self.entries.contains_key(&key) {
            return Err(error(
                "registry_duplicate",
                "$.key",
                "entry ID/version is already registered",
            ));
        }
        if self.entries.len() == MAX_NODES {
            return Err(error(
                "registry_limit",
                "$",
                "registry entry budget exceeded",
            ));
        }
        self.entries.insert(
            key,
            Registration {
                descriptor: Arc::new(descriptor),
                implementation,
            },
        );
        Ok(())
    }
    pub fn freeze(self) -> Result<Registry> {
        let identities: Vec<_> = self
            .entries
            .values()
            .map(|e| {
                json!({"id": e.descriptor.draft.key.id, "version": e.descriptor.draft.key.version,
                   "descriptor": e.descriptor.content_id().to_string(),
                   "artifact": e.descriptor.draft.artifact.to_string()})
            })
            .collect();
        let canonical = serde_json::to_vec(&json!({"version":1,"entries":identities}))
            .map_err(|_| error("registry_encoding", "$", "registry encoding failed"))?;
        let content_id = hash(REGISTRY_PREFIX, &canonical);
        Ok(Registry {
            entries: self.entries,
            canonical,
            content_id,
        })
    }
}
pub struct Registry {
    entries: BTreeMap<EntryKey, Registration>,
    canonical: Vec<u8>,
    content_id: ContentId,
}
impl Registry {
    pub fn content_id(&self) -> ContentId {
        self.content_id
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn descriptors(&self) -> impl Iterator<Item = &Descriptor> {
        self.entries.values().map(|e| e.descriptor.as_ref())
    }
    fn registration(&self, r: &EntryRequirement) -> Result<&Registration> {
        if r.descriptor_version != DESCRIPTOR_VERSION || r.role != "provider" {
            return Err(error(
                "registry_version",
                "$.entry",
                "unsupported descriptor version/role",
            ));
        }
        let e = self.entries.get(&r.key).ok_or_else(|| {
            error(
                "registry_unknown",
                "$.entry",
                "exact entry ID/version is unavailable",
            )
        })?;
        if r.artifact != e.descriptor.draft.artifact {
            return Err(error(
                "registry_artifact",
                "$.entry.artifact",
                "implementation content differs from requested pin",
            ));
        }
        Ok(e)
    }
    pub fn resolve(&self, r: &EntryRequirement) -> Result<&Descriptor> {
        Ok(self.registration(r)?.descriptor.as_ref())
    }
    pub fn validate_requirements(&self, model: &LoadedModel) -> Result<()> {
        for r in &model.definition().declaration().entries {
            self.resolve(r)?;
        }
        Ok(())
    }
    pub fn prepare(
        &self,
        r: &EntryRequirement,
        values: impl IntoIterator<Item = (String, ParameterInput)>,
    ) -> Result<PreparedCall> {
        let e = self.registration(r)?;
        let Implementation::Pure(provider) = &e.implementation else {
            return Err(error(
                "registry_profile",
                "$",
                "incremental entries require an owned instance",
            ));
        };
        let parameters = e.descriptor.draft.parameters.bind(values).map_err(|_| {
            error(
                "registry_parameter",
                "$.parameters",
                "invalid provider parameter bindings",
            )
        })?;
        let bytes = parameters
            .iter()
            .map(|(_, v)| match v {
                BoundValue::Value(v) => payload_size(v),
                BoundValue::Unbound => 0,
            })
            .sum::<usize>();
        if bytes > e.descriptor.draft.resources.max_input_bytes {
            return Err(error(
                "registry_limit",
                "$.parameters",
                "parameter payload budget exceeded",
            ));
        }
        provider_result(catch_unwind(AssertUnwindSafe(|| {
            provider.validate_parameters(&parameters)
        })))?;
        Ok(PreparedCall {
            descriptor: Arc::clone(&e.descriptor),
            provider: Arc::clone(provider),
            parameters,
            parameter_bytes: bytes,
            model_pin: None,
            registry_id: self.content_id,
        })
    }
    /// Bind and validate without creating or advancing state.
    pub fn prepare_incremental(
        &self,
        r: &EntryRequirement,
        values: impl IntoIterator<Item = (String, ParameterInput)>,
    ) -> Result<PreparedIncremental> {
        let e = self.registration(r)?;
        let Implementation::Incremental(provider) = &e.implementation else {
            return Err(error("registry_profile", "$", "pure entries use prepare"));
        };
        let parameters = e.descriptor.draft.parameters.bind(values).map_err(|_| {
            error(
                "registry_parameter",
                "$.parameters",
                "invalid provider parameter bindings",
            )
        })?;
        let parameter_bytes = parameters
            .iter()
            .map(|(_, v)| match v {
                BoundValue::Value(v) => payload_size(v),
                BoundValue::Unbound => 0,
            })
            .sum();
        if parameter_bytes > e.descriptor.draft.resources.max_input_bytes {
            return Err(error(
                "registry_limit",
                "$.parameters",
                "parameter payload budget exceeded",
            ));
        }
        provider_result(catch_unwind(AssertUnwindSafe(|| {
            provider.validate_parameters(&parameters)
        })))?;
        Ok(PreparedIncremental {
            descriptor: Arc::clone(&e.descriptor),
            provider: Arc::clone(provider),
            parameters,
            parameter_bytes,
            registry_id: self.content_id,
        })
    }
    /// Creates a fresh owned state; never usable through repeated pure AST calls.
    pub fn instantiate(
        &self,
        r: &EntryRequirement,
        values: impl IntoIterator<Item = (String, ParameterInput)>,
        run: impl Into<String>,
        instance: impl Into<String>,
    ) -> Result<IncrementalInstance> {
        let run = run.into();
        let instance = instance.into();
        identifier(&run, "$.call.run")?;
        identifier(&instance, "$.call.instance")?;
        self.prepare_incremental(r, values)?
            .instantiate(run, instance)
    }
    /// Local signature/parameter preparation only; no cycle/topology compilation.
    pub fn prepare_node(
        &self,
        model: &LoadedModel,
        binding: &ModelBinding,
        node_id: &str,
    ) -> Result<PreparedCall> {
        binding.ensure_matches(model)?;
        self.validate_requirements(model)?;
        let d = model.definition().declaration();
        let node = d
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .ok_or_else(|| error("registry_node", "$.graph.nodes", "node unavailable"))?;
        let r = d
            .entries
            .iter()
            .find(|r| r.key == node.entry)
            .ok_or_else(|| {
                error(
                    "registry_unknown",
                    "$.graph.nodes.entry",
                    "undeclared entry",
                )
            })?;
        let descriptor = self.resolve(r)?.declaration();
        if node.outputs.len() != descriptor.outputs.len()
            || node.inputs.len() != descriptor.inputs.len()
        {
            return Err(error(
                "registry_arity",
                "$.graph.nodes",
                "node port count differs from descriptor",
            ));
        }
        for p in &descriptor.outputs {
            if !node
                .outputs
                .iter()
                .any(|n| n.name == p.name && n.value_type == p.value_type)
            {
                return Err(error(
                    "registry_type",
                    "$.graph.nodes.outputs",
                    "node output signature differs from descriptor",
                ));
            }
        }
        for p in &descriptor.inputs {
            let i = node
                .inputs
                .iter()
                .find(|i| i.name == p.name)
                .ok_or_else(|| {
                    error(
                        "registry_arity",
                        "$.graph.nodes.inputs",
                        "required input name is missing",
                    )
                })?;
            let output = d
                .nodes
                .iter()
                .find(|n| n.id == i.source.node)
                .and_then(|n| n.outputs.iter().find(|o| o.name == i.source.port))
                .ok_or_else(|| {
                    error(
                        "registry_node",
                        "$.graph.nodes.inputs",
                        "source is unavailable",
                    )
                })?;
            if output.value_type != p.value_type {
                return Err(error(
                    "registry_type",
                    "$.graph.nodes.inputs",
                    "edge value type differs from input signature",
                ));
            }
        }
        let values = node
            .arguments
            .iter()
            .map(|(key, arg)| {
                let v = match arg {
                    Argument::Literal(v) => v.clone(),
                    Argument::Parameter(p) => match binding.parameters().get(p) {
                        Some(BoundValue::Value(v)) => v.clone(),
                        _ => {
                            return Err(error(
                                "registry_parameter",
                                "$.graph.nodes.arguments",
                                "unbound model parameter",
                            ));
                        }
                    },
                };
                Ok((key.clone(), ParameterInput::Value(Evaluation::Ready(v))))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut call = self.prepare(r, values)?;
        call.model_pin = Some(ModelNodePin {
            model: model.content_id(),
            node: node.id.clone(),
        });
        Ok(call)
    }
}
fn provider_result<T>(
    v: std::thread::Result<std::result::Result<T, ProviderFailure>>,
) -> Result<T> {
    v.map_err(|_| {
        error(
            "registry_provider_panic",
            "$.provider",
            "reviewed provider panicked",
        )
    })?
    .map_err(|e| match e {
        ProviderFailure::InvalidParameters => error(
            "registry_parameter",
            "$.provider",
            "provider parameter relationship rejected",
        ),
        ProviderFailure::PrecisionLimit => error(
            "registry_precision",
            "$.provider",
            "provider numerical conditioning limit rejected the input",
        ),
        ProviderFailure::Numeric(n) => {
            ModelError::new("registry_numeric", "$.provider", n.to_string())
        }
        ProviderFailure::Failed => error(
            "registry_provider",
            "$.provider",
            "provider rejected the call",
        ),
    })
}
fn validate_values(
    values: NamedValues,
    ports: &[CallablePort],
    budget: usize,
    path: &str,
) -> Result<BTreeMap<String, Evaluation<TypedValue>>> {
    if values.len() != ports.len() {
        return Err(error(
            "registry_arity",
            path,
            "named value count differs from signature",
        ));
    }
    let mut map = BTreeMap::new();
    let mut size = 0;
    for (k, v) in values {
        let p = ports
            .iter()
            .find(|p| p.name == k)
            .ok_or_else(|| error("registry_port", path, "unknown named port"))?;
        if let Evaluation::Ready(t) = &v {
            if t.descriptor() != &p.value_type {
                return Err(error(
                    "registry_type",
                    path,
                    "exact type/unit/scale mismatch",
                ));
            }
            size += payload_size(t);
        }
        if map.insert(k, v).is_some() {
            return Err(error("registry_duplicate", path, "duplicate named port"));
        }
    }
    if size > budget {
        return Err(error(
            "registry_limit",
            path,
            "scalar payload budget exceeded",
        ));
    }
    Ok(map)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelNodePin {
    pub model: ContentId,
    pub node: String,
}

pub struct PreparedCall {
    descriptor: Arc<Descriptor>,
    provider: Arc<dyn Provider>,
    parameters: BoundParameters,
    parameter_bytes: usize,
    model_pin: Option<ModelNodePin>,
    registry_id: ContentId,
}
impl PreparedCall {
    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
    pub fn registry_id(&self) -> ContentId {
        self.registry_id
    }
    pub fn model_pin(&self) -> Option<&ModelNodePin> {
        self.model_pin.as_ref()
    }
    pub fn parameters(&self) -> &BoundParameters {
        &self.parameters
    }
    pub fn invoke(&self, context: CallContext, inputs: NamedValues) -> Result<CallResult> {
        context.validate()?;
        let d = &self.descriptor.draft;
        let inputs = validate_values(
            inputs,
            &d.inputs,
            d.resources.max_input_bytes - self.parameter_bytes,
            "$.inputs",
        )?;
        for value in inputs.values() {
            if let Evaluation::Error(n) = value {
                return Err(ModelError::new(
                    "registry_numeric",
                    "$.inputs",
                    n.to_string(),
                ));
            }
        }
        let missing = inputs.values().any(|v| matches!(v, Evaluation::Missing));
        let warmup = inputs.values().any(|v| matches!(v, Evaluation::Warmup));
        let outputs = if missing || warmup {
            d.outputs
                .iter()
                .map(|p| {
                    (
                        p.name.clone(),
                        if missing {
                            Evaluation::Missing
                        } else {
                            Evaluation::Warmup
                        },
                    )
                })
                .collect()
        } else {
            let call = ProviderCall {
                context: &context,
                parameters: &self.parameters,
                inputs: &inputs,
            };
            let values = provider_result(catch_unwind(AssertUnwindSafe(|| {
                self.provider.invoke(&call)
            })))?;
            let outputs = validate_values(
                values,
                &d.outputs,
                d.resources.max_output_bytes,
                "$.outputs",
            )?;
            for value in outputs.values() {
                match value {
                    Evaluation::Ready(_) => {}
                    Evaluation::Error(n) => {
                        return Err(ModelError::new(
                            "registry_numeric",
                            "$.outputs",
                            n.to_string(),
                        ));
                    }
                    _ => {
                        return Err(error(
                            "registry_readiness",
                            "$.outputs",
                            "pure ready inputs require ready outputs",
                        ));
                    }
                }
            }
            outputs
        };
        Ok(CallResult {
            context,
            descriptor_id: self.descriptor.content_id,
            model_pin: self.model_pin.clone(),
            registry_id: self.registry_id,
            outputs,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallResult {
    context: CallContext,
    descriptor_id: ContentId,
    model_pin: Option<ModelNodePin>,
    registry_id: ContentId,
    outputs: BTreeMap<String, Evaluation<TypedValue>>,
}
impl CallResult {
    pub fn model_pin(&self) -> Option<&ModelNodePin> {
        self.model_pin.as_ref()
    }
    pub fn context(&self) -> &CallContext {
        &self.context
    }
    pub fn descriptor_id(&self) -> ContentId {
        self.descriptor_id
    }
    pub fn registry_id(&self) -> ContentId {
        self.registry_id
    }
    pub fn outputs(&self) -> &BTreeMap<String, Evaluation<TypedValue>> {
        &self.outputs
    }
}

/// Single-node transaction; event hosts use `try_fork` to stage cross-node transactions.
pub struct IncrementalInstance {
    descriptor: Arc<Descriptor>,
    provider: Arc<dyn IncrementalProvider>,
    parameters: BoundParameters,
    parameter_bytes: usize,
    state: Box<dyn IncrementalState>,
    run: String,
    instance: String,
    last_sequence: Option<u64>,
    accepted_steps: u64,
    registry_id: ContentId,
}
impl IncrementalInstance {
    /// Deep candidate copy for host whole-event transactions; never creates fresh seed state.
    pub fn try_fork(&self) -> Result<Self> {
        let state = provider_result(catch_unwind(AssertUnwindSafe(|| Ok(self.state.fork()))))?;
        Ok(Self {
            descriptor: Arc::clone(&self.descriptor),
            provider: Arc::clone(&self.provider),
            parameters: self.parameters.clone(),
            parameter_bytes: self.parameter_bytes,
            state,
            run: self.run.clone(),
            instance: self.instance.clone(),
            last_sequence: self.last_sequence,
            accepted_steps: self.accepted_steps,
            registry_id: self.registry_id,
        })
    }
    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
    pub fn parameters(&self) -> &BoundParameters {
        &self.parameters
    }
    pub fn last_sequence(&self) -> Option<u64> {
        self.last_sequence
    }
    pub fn accepted_steps(&self) -> u64 {
        self.accepted_steps
    }
    pub fn step(&mut self, sequence: u64, inputs: NamedValues) -> Result<CallResult> {
        if sequence == 0 || self.last_sequence.is_some_and(|s| sequence <= s) {
            return Err(error(
                "registry_sequence",
                "$.call",
                "expected strictly increasing positive sequence",
            ));
        }
        let next_steps = self
            .accepted_steps
            .checked_add(1)
            .ok_or_else(|| error("registry_limit", "$.call", "step counter overflow"))?;
        let d = &self.descriptor.draft;
        let inputs = validate_values(
            inputs,
            &d.inputs,
            d.resources.max_input_bytes - self.parameter_bytes,
            "$.inputs",
        )?;
        for v in inputs.values() {
            if let Evaluation::Error(n) = v {
                return Err(ModelError::new(
                    "registry_numeric",
                    "$.inputs",
                    n.to_string(),
                ));
            }
        }
        let missing = inputs.values().any(|v| matches!(v, Evaluation::Missing));
        let warmup = inputs.values().any(|v| matches!(v, Evaluation::Warmup));
        let context = CallContext {
            run: self.run.clone(),
            instance: self.instance.clone(),
            event_sequence: sequence,
            call_sequence: next_steps,
        };
        let (state, outputs) = if missing || warmup {
            // An accepted unavailable sample breaks continuity and resets all history.
            let state = provider_result(catch_unwind(AssertUnwindSafe(|| {
                self.provider.create(&self.parameters)
            })))?;
            let outputs = d
                .outputs
                .iter()
                .map(|p| {
                    (
                        p.name.clone(),
                        if missing {
                            Evaluation::Missing
                        } else {
                            Evaluation::Warmup
                        },
                    )
                })
                .collect();
            (state, outputs)
        } else {
            let (state, values) = provider_result(catch_unwind(AssertUnwindSafe(|| {
                let mut state = self.state.fork();
                let values = state.step(&ProviderCall {
                    context: &context,
                    parameters: &self.parameters,
                    inputs: &inputs,
                })?;
                Ok((state, values))
            })))?;
            let outputs = validate_values(
                values,
                &d.outputs,
                d.resources.max_output_bytes,
                "$.outputs",
            )?;
            for v in outputs.values() {
                if let Evaluation::Error(n) = v {
                    return Err(ModelError::new(
                        "registry_numeric",
                        "$.outputs",
                        n.to_string(),
                    ));
                }
            }
            (state, outputs)
        };
        self.state = state;
        self.last_sequence = Some(sequence);
        self.accepted_steps = next_steps;
        Ok(CallResult {
            context,
            descriptor_id: self.descriptor.content_id(),
            registry_id: self.registry_id,
            model_pin: None,
            outputs,
        })
    }
}

/// Immutable pinned factory/parameters. Preparation executes only relationship validation.
pub struct PreparedIncremental {
    descriptor: Arc<Descriptor>,
    provider: Arc<dyn IncrementalProvider>,
    parameters: BoundParameters,
    parameter_bytes: usize,
    registry_id: ContentId,
}
impl PreparedIncremental {
    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
    pub fn parameters(&self) -> &BoundParameters {
        &self.parameters
    }
    pub fn registry_id(&self) -> ContentId {
        self.registry_id
    }
    pub fn instantiate(
        &self,
        run: impl Into<String>,
        instance: impl Into<String>,
    ) -> Result<IncrementalInstance> {
        let run = run.into();
        let instance = instance.into();
        identifier(&run, "$.call.run")?;
        identifier(&instance, "$.call.instance")?;
        let state = provider_result(catch_unwind(AssertUnwindSafe(|| {
            self.provider.create(&self.parameters)
        })))?;
        Ok(IncrementalInstance {
            descriptor: Arc::clone(&self.descriptor),
            provider: Arc::clone(&self.provider),
            parameters: self.parameters.clone(),
            parameter_bytes: self.parameter_bytes,
            state,
            run,
            instance,
            last_sequence: None,
            accepted_steps: 0,
            registry_id: self.registry_id,
        })
    }
}
