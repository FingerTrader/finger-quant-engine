use fqe_core::{
    numeric::Decimal,
    types::{
        Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
    },
};
use fqe_ir::{indicators, parameters::ParameterInput, registry::RegistryBuilder};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 12,
            unit: NumericUnit::Scalar,
        },
    )?;
    let mut builder = RegistryBuilder::new();
    indicators::register(&mut builder, "example", input.clone())?;
    let registry = builder.freeze()?;
    let requirement = registry
        .descriptors()
        .find(|d| d.declaration().key.id == "example.macd")
        .unwrap()
        .requirement();
    let u64_type = TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64)?;
    let params = [("fast", 2), ("slow", 3), ("signal", 2)]
        .into_iter()
        .map(|(k, v)| {
            (
                k.into(),
                ParameterInput::Value(Evaluation::Ready(
                    TypedValue::new(u64_type.clone(), ScalarValue::U64(v)).unwrap(),
                )),
            )
        });
    let mut instance = registry.instantiate(&requirement, params, "G-002", "macd")?;
    for (k, close) in [1, 2, 3, 2, 4, 3].into_iter().enumerate() {
        let value = TypedValue::new(
            input.clone(),
            ScalarValue::Decimal(Decimal::new(close * 1_000_000_000_000, 12)?),
        )?;
        let result = instance.step(
            k as u64 + 1,
            vec![("close".into(), Evaluation::Ready(value))],
        )?;
        print!("bar {}", k + 1);
        for (key, value) in result.outputs() {
            match value {
                Evaluation::Ready(v) => {
                    let ScalarValue::Decimal(d) = v.value() else {
                        unreachable!()
                    };
                    print!(" {key}={d}");
                }
                _ => print!(" {key}=Warmup"),
            }
        }
        println!();
    }
    println!(
        "accepted steps: {} (library slice; no market-event scheduler)",
        instance.accepted_steps()
    );
    Ok(())
}
