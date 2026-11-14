//! Offline parameter binding example, not a model loader or strategy execution.
use fqe_core::types::{
    Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
};
use fqe_ir::parameters::{
    Bound, BoundValue, Constraints, ParameterInput, ParameterSchema, ParameterSpec,
};

fn integer(value: u64) -> TypedValue {
    TypedValue::new(
        TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).unwrap(),
        ScalarValue::U64(value),
    )
    .unwrap()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let period = ParameterSpec::new(
        "period",
        integer(2).descriptor().clone(),
        false,
        Some(integer(2)),
        Constraints {
            lower: Some(Bound {
                value: integer(1),
                inclusive: true,
            }),
            upper: Some(Bound {
                value: integer(200),
                inclusive: true,
            }),
            choices: None,
        },
        "History length in accepted bars",
    )?;
    let schema = ParameterSchema::new(1, vec![period])?;
    let default_run = schema.bind([])?;
    let custom_run = schema.bind([(
        "period".into(),
        ParameterInput::Value(Evaluation::Ready(integer(5))),
    )])?;
    for (label, bindings) in [("default", default_run), ("custom", custom_run)] {
        if let Some(BoundValue::Value(value)) = bindings.get("period") {
            println!("{label}: period={:?}", value.value());
        }
    }
    assert!(
        schema
            .bind([(
                "period".into(),
                ParameterInput::Value(Evaluation::Ready(integer(0)))
            )])
            .is_err()
    );
    println!("out-of-range input rejected; schema/default bindings preserved");
    Ok(())
}
