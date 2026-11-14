//! Independently defined, reviewed X-001 fixtures. Not imported by engine source.
use fqe_core::{
    numeric::NumericError,
    types::{Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{
    builtins,
    model::{ContentId, EntryKey, ModelError},
    parameters::{BoundParameters, BoundValue, Constraints, ParameterSchema, ParameterSpec},
    registry::{
        CallablePort, Category, Descriptor, DescriptorDraft, NamedValues, Provider, ProviderCall,
        ProviderFailure, Registry, RegistryBuilder,
    },
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
pub const PAIR_ID: &str = "independent.checked_pair";
pub const PREDICATE_ID: &str = "independent.above";
pub fn artifact() -> ContentId {
    ContentId::from_bytes(Sha256::digest(include_bytes!("providers.rs")).into())
}
pub fn u64_type() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).unwrap()
}
pub fn value(n: u64) -> TypedValue {
    TypedValue::new(u64_type(), ScalarValue::U64(n)).unwrap()
}
fn param(key: &str, default: Option<u64>) -> ParameterSpec {
    ParameterSpec::new(
        key,
        u64_type(),
        default.is_none(),
        default.map(value),
        Constraints::default(),
        "Exact u64",
    )
    .unwrap()
}
fn get_parameter(p: &BoundParameters, key: &str) -> Result<u64, ProviderFailure> {
    let Some(BoundValue::Value(v)) = p.get(key) else {
        return Err(ProviderFailure::InvalidParameters);
    };
    let ScalarValue::U64(n) = v.value() else {
        return Err(ProviderFailure::InvalidParameters);
    };
    Ok(*n)
}
fn input(call: &ProviderCall<'_>, key: &str) -> Result<u64, ProviderFailure> {
    let Some(v) = call.input(key) else {
        return Err(ProviderFailure::Failed);
    };
    let ScalarValue::U64(n) = v.value() else {
        return Err(ProviderFailure::Failed);
    };
    Ok(*n)
}
pub fn pair_descriptor() -> Descriptor {
    let mut d = DescriptorDraft::pure(
        EntryKey {
            id: PAIR_ID.into(),
            version: "1".into(),
        },
        artifact(),
        ParameterSchema::new(1, vec![param("bias", Some(0))]).unwrap(),
        "sum=checked(left+right+bias); difference=checked(left-right); exact u64, overflow fails; pure host readiness propagation.",
    );
    d.inputs = vec![
        CallablePort::one("left", u64_type()),
        CallablePort::one("right", u64_type()),
    ];
    d.outputs = vec![
        CallablePort::one("sum", u64_type()),
        CallablePort::one("difference", u64_type()),
    ];
    Descriptor::new(d).unwrap()
}
pub fn predicate_descriptor() -> Descriptor {
    let mut d = DescriptorDraft::pure(
        EntryKey {
            id: PREDICATE_ID.into(),
            version: "1".into(),
        },
        artifact(),
        ParameterSchema::new(
            1,
            vec![param("threshold", None), param("ceiling", Some(1000))],
        )
        .unwrap(),
        "Require threshold<ceiling; result=value>threshold strictly, exact u64, no orders or history; pure host readiness propagation.",
    );
    d.category = Category::Predicate;
    d.inputs = vec![CallablePort::one("value", u64_type())];
    d.outputs = vec![CallablePort::one(
        "result",
        TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Bool).unwrap(),
    )];
    Descriptor::new(d).unwrap()
}
pub struct Pair;
impl Provider for Pair {
    fn invoke(&self, call: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        let left = input(call, "left")?;
        let right = input(call, "right")?;
        let bias = get_parameter(call.parameters, "bias")?;
        let sum = left
            .checked_add(right)
            .and_then(|n| n.checked_add(bias))
            .ok_or(ProviderFailure::Numeric(NumericError::Overflow))?;
        let difference = left
            .checked_sub(right)
            .ok_or(ProviderFailure::Numeric(NumericError::Overflow))?;
        Ok(vec![
            ("sum".into(), Evaluation::Ready(value(sum))),
            ("difference".into(), Evaluation::Ready(value(difference))),
        ])
    }
}
pub struct Above;
impl Provider for Above {
    fn validate_parameters(&self, p: &BoundParameters) -> Result<(), ProviderFailure> {
        if get_parameter(p, "threshold")? < get_parameter(p, "ceiling")? {
            Ok(())
        } else {
            Err(ProviderFailure::InvalidParameters)
        }
    }
    fn invoke(&self, call: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        let result = input(call, "value")? > get_parameter(call.parameters, "threshold")?;
        let t = TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Bool).unwrap();
        Ok(vec![(
            "result".into(),
            Evaluation::Ready(TypedValue::new(t, ScalarValue::Bool(result)).unwrap()),
        )])
    }
}
pub fn registry() -> Result<Registry, ModelError> {
    let mut b = RegistryBuilder::new();
    builtins::register(&mut b)?;
    b.register(pair_descriptor(), Arc::new(Pair))?;
    b.register(predicate_descriptor(), Arc::new(Above))?;
    b.freeze()
}
