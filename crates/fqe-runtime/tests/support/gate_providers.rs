//! Reviewed P1 gate-only providers. No engine/compiler source imports this module.
use fqe_core::{
    numeric::NumericError,
    types::{Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{
    model::{ContentId, EntryKey},
    parameters::{BoundParameters, BoundValue, Constraints, ParameterSchema, ParameterSpec},
    registry::{
        CallablePort, Category, Descriptor, DescriptorDraft, NamedValues, Provider, ProviderCall,
        ProviderFailure, Registry, RegistryBuilder,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub const SHIFT: &str = "acceptance.shift";
pub const WINDOW: &str = "acceptance.window";
pub fn unsigned() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).unwrap()
}
pub fn boolean() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Bool).unwrap()
}
pub fn value(n: u64) -> TypedValue {
    TypedValue::new(unsigned(), ScalarValue::U64(n)).unwrap()
}
fn parameter(name: &str) -> ParameterSpec {
    ParameterSpec::new(
        name,
        unsigned(),
        true,
        None,
        Constraints::default(),
        "Exact required u64",
    )
    .unwrap()
}
fn get(parameters: &BoundParameters, name: &str) -> Result<u64, ProviderFailure> {
    match parameters.get(name) {
        Some(BoundValue::Value(v)) => match v.value() {
            ScalarValue::U64(n) => Ok(*n),
            _ => Err(ProviderFailure::InvalidParameters),
        },
        _ => Err(ProviderFailure::InvalidParameters),
    }
}
struct Function {
    predicate: bool,
    calls: Arc<AtomicUsize>,
}
impl Provider for Function {
    fn validate_parameters(&self, p: &BoundParameters) -> Result<(), ProviderFailure> {
        if self.predicate && get(p, "lower")? >= get(p, "upper")? {
            return Err(ProviderFailure::InvalidParameters);
        }
        Ok(())
    }
    fn invoke(&self, call: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        // Test instrumentation only; no decision depends on this counter.
        self.calls.fetch_add(1, Ordering::Relaxed);
        let Some(v) = call.input("sample") else {
            return Err(ProviderFailure::Failed);
        };
        let ScalarValue::U64(n) = v.value() else {
            return Err(ProviderFailure::Failed);
        };
        let result = if self.predicate {
            TypedValue::new(
                boolean(),
                ScalarValue::Bool(
                    *n > get(call.parameters, "lower")? && *n <= get(call.parameters, "upper")?,
                ),
            )
            .unwrap()
        } else {
            value(
                n.checked_add(get(call.parameters, "increment")?)
                    .ok_or(ProviderFailure::Numeric(NumericError::Overflow))?,
            )
        };
        Ok(vec![("result".into(), Evaluation::Ready(result))])
    }
}
pub fn registry() -> (Registry, Arc<AtomicUsize>) {
    let artifact =
        ContentId::parse(include_str!("../fixtures/p1/provider-source.sha256").trim()).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut builder = RegistryBuilder::new();
    for (name, predicate, params) in [
        (SHIFT, false, vec![parameter("increment")]),
        (WINDOW, true, vec![parameter("lower"), parameter("upper")]),
    ] {
        let mut draft = DescriptorDraft::pure(
            EntryKey {
                id: name.into(),
                version: "1".into(),
            },
            artifact,
            ParameterSchema::new(1, params).unwrap(),
            if predicate {
                "lower < sample <= upper; lower < upper; exact u64; no history"
            } else {
                "checked(sample + increment); overflow rejects; exact u64; no history"
            },
        );
        if predicate {
            draft.category = Category::Predicate;
        }
        draft.inputs = vec![CallablePort::one("sample", unsigned())];
        draft.outputs = vec![CallablePort::one(
            "result",
            if predicate { boolean() } else { unsigned() },
        )];
        builder
            .register(
                Descriptor::new(draft).unwrap(),
                Arc::new(Function {
                    predicate,
                    calls: Arc::clone(&calls),
                }),
            )
            .unwrap();
    }
    (builder.freeze().unwrap(), calls)
}
