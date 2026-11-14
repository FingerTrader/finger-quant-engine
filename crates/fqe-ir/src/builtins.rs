//! Minimal reviewed pure providers; no graph dispatch or indicator-specific branches.
use crate::{
    model::{ContentId, EntryKey, ModelError},
    parameters::{BoundValue, Constraints, ParameterSchema, ParameterSpec},
    registry::{
        CallablePort, Category, Descriptor, DescriptorDraft, NamedValues, Provider, ProviderCall,
        ProviderFailure, Registry, RegistryBuilder,
    },
};
use fqe_core::types::{
    Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub fn implementation_artifact() -> ContentId {
    ContentId::from_bytes(Sha256::digest(include_bytes!("builtins.rs")).into())
}
fn scalar(kind: ScalarType) -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, kind).expect("supported built-in scalar")
}
fn draft(id: &str, parameters: ParameterSchema, semantics: &str) -> DescriptorDraft {
    DescriptorDraft::pure(
        EntryKey {
            id: id.into(),
            version: "1".into(),
        },
        implementation_artifact(),
        parameters,
        semantics,
    )
}
struct Identity;
impl Provider for Identity {
    fn invoke(&self, call: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        let v = call.input("value").ok_or(ProviderFailure::Failed)?;
        Ok(vec![("result".into(), Evaluation::Ready(v.clone()))])
    }
}
struct Constant;
impl Provider for Constant {
    fn invoke(&self, call: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        let Some(BoundValue::Value(v)) = call.parameters.get("value") else {
            return Err(ProviderFailure::InvalidParameters);
        };
        Ok(vec![("result".into(), Evaluation::Ready(v.clone()))])
    }
}
struct Not;
impl Provider for Not {
    fn invoke(&self, call: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        let Some(v) = call.input("value") else {
            return Err(ProviderFailure::Failed);
        };
        let ScalarValue::Bool(b) = v.value() else {
            return Err(ProviderFailure::Failed);
        };
        let value = TypedValue::new(scalar(ScalarType::Bool), ScalarValue::Bool(!b))
            .map_err(|_| ProviderFailure::Failed)?;
        Ok(vec![("result".into(), Evaluation::Ready(value))])
    }
}
pub fn register(builder: &mut RegistryBuilder) -> Result<(), ModelError> {
    let empty = ParameterSchema::new(1, vec![]).expect("empty built-in schema");
    let mut identity = draft(
        "fqe.identity.u64",
        empty.clone(),
        "Return the exact ready u64 input unchanged; host propagates Error/Missing/Warmup.",
    );
    identity
        .inputs
        .push(CallablePort::one("value", scalar(ScalarType::U64)));
    identity
        .outputs
        .push(CallablePort::one("result", scalar(ScalarType::U64)));
    builder.register(Descriptor::new(identity)?, Arc::new(Identity))?;
    let schema = ParameterSchema::new(
        1,
        vec![
            ParameterSpec::new(
                "value",
                scalar(ScalarType::U64),
                true,
                None,
                Constraints::default(),
                "Exact constant",
            )
            .expect("valid constant parameter"),
        ],
    )
    .expect("valid built-in schema");
    let mut constant = draft(
        "fqe.constant.u64",
        schema,
        "Return the required immutable u64 value parameter exactly; no input/history or implicit conversion.",
    );
    constant
        .outputs
        .push(CallablePort::one("result", scalar(ScalarType::U64)));
    builder.register(Descriptor::new(constant)?, Arc::new(Constant))?;
    let mut not = draft(
        "fqe.bool.not",
        empty,
        "Return the Boolean negation of the ready input; host propagates Error/Missing/Warmup, never converts absence to false.",
    );
    not.category = Category::Predicate;
    not.inputs
        .push(CallablePort::one("value", scalar(ScalarType::Bool)));
    not.outputs
        .push(CallablePort::one("result", scalar(ScalarType::Bool)));
    builder.register(Descriptor::new(not)?, Arc::new(Not))?;
    Ok(())
}
pub fn registry() -> Result<Registry, ModelError> {
    let mut b = RegistryBuilder::new();
    register(&mut b)?;
    b.freeze()
}
