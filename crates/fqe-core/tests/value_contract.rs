use fqe_core::{
    InstrumentId,
    numeric::{Decimal, NumericError, Rounding},
    types::{
        CurrencyCode, Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypeError,
        TypedValue, VALUE_CONTRACT_VERSION,
    },
};

fn decimal(n: i128, scale: u8) -> Decimal {
    Decimal::new(n, scale).unwrap()
}
fn descriptor(scalar: ScalarType) -> TypeDescriptor {
    TypeDescriptor::new(
        VALUE_CONTRACT_VERSION,
        fqe_core::types::ValueShape::Scalar,
        scalar,
    )
    .unwrap()
}

#[test]
fn decimal_is_exact_above_javascript_integer_precision_and_across_signs() {
    let value = decimal(i128::from(u64::MAX), 0);
    assert_eq!(value.to_string(), "18446744073709551615");
    assert_eq!(
        decimal(i128::MIN, 0).to_string(),
        "-170141183460469231731687303715884105728"
    );
    assert_eq!(decimal(-5, 3).to_string(), "-0.005");
    assert_eq!(decimal(0, 2).to_string(), "0.00");
    assert_eq!(decimal(1, 18).to_string(), "0.000000000000000001");
    assert_eq!(Decimal::new(1, 21), Err(NumericError::UnsupportedScale(21)));
    assert_eq!(
        Decimal::new(0, u8::MAX),
        Err(NumericError::UnsupportedScale(u8::MAX))
    );
}

#[test]
fn numeric_candidates_expose_integer_float_and_decimal_tradeoffs() {
    assert_eq!(u64::MAX.checked_add(1), None);
    assert_eq!((1_u64 / 3), 0);
    assert_ne!(0.1_f64 + 0.2_f64, 0.3_f64);
    assert_eq!(u64::MAX as f64, (u64::MAX - 1) as f64);
    assert_eq!(
        decimal(1, 1).checked_add(decimal(2, 1)).unwrap(),
        decimal(3, 1)
    );
    assert_eq!(
        decimal(100, 2).checked_div(decimal(300, 2), Rounding::Exact),
        Err(NumericError::Inexact)
    );
    assert_eq!(
        decimal(100, 2)
            .checked_div(decimal(300, 2), Rounding::HalfEven)
            .unwrap()
            .to_string(),
        "0.33"
    );
}

#[test]
fn rescaling_requires_an_explicit_precision_and_rounding_decision() {
    assert_eq!(
        decimal(-12, 1).rescale(3, Rounding::Exact).unwrap(),
        decimal(-1200, 3)
    );
    assert_eq!(
        decimal(1200, 3).rescale(1, Rounding::Exact).unwrap(),
        decimal(12, 1)
    );
    assert_eq!(
        decimal(125, 2).rescale(1, Rounding::Exact),
        Err(NumericError::Inexact)
    );
    assert_eq!(
        decimal(-129, 2).rescale(1, Rounding::TowardZero).unwrap(),
        decimal(-12, 1)
    );
    for (n, expected) in [
        (125, 12),
        (135, 14),
        (-125, -12),
        (-135, -14),
        (124, 12),
        (126, 13),
        (-126, -13),
    ] {
        assert_eq!(
            decimal(n, 2).rescale(1, Rounding::HalfEven).unwrap(),
            decimal(expected, 1)
        );
    }
    assert_eq!(
        decimal(i128::MAX, 0).rescale(1, Rounding::Exact),
        Err(NumericError::Overflow)
    );
}

#[test]
fn decimal_arithmetic_has_checked_results_and_no_implicit_scale_conversion() {
    assert_eq!(
        decimal(-125, 2).checked_add(decimal(50, 2)).unwrap(),
        decimal(-75, 2)
    );
    assert_eq!(
        decimal(-125, 2).checked_sub(decimal(50, 2)).unwrap(),
        decimal(-175, 2)
    );
    assert_eq!(
        decimal(125, 2)
            .checked_mul(decimal(200, 2), Rounding::Exact)
            .unwrap(),
        decimal(250, 2)
    );
    assert_eq!(
        decimal(125, 2)
            .checked_div(decimal(-50, 2), Rounding::Exact)
            .unwrap(),
        decimal(-250, 2)
    );
    assert_eq!(
        decimal(1, 0).checked_add(decimal(10, 1)),
        Err(NumericError::ScaleMismatch)
    );
    assert_eq!(
        decimal(1, 0).checked_sub(decimal(10, 1)),
        Err(NumericError::ScaleMismatch)
    );
    assert_eq!(
        decimal(1, 0).checked_mul(decimal(10, 1), Rounding::Exact),
        Err(NumericError::ScaleMismatch)
    );
    assert_eq!(
        decimal(1, 0).checked_div(decimal(10, 1), Rounding::Exact),
        Err(NumericError::ScaleMismatch)
    );
    assert_eq!(
        decimal(1, 0).checked_cmp(decimal(10, 1)),
        Err(NumericError::ScaleMismatch)
    );
    assert_eq!(
        decimal(-1, 0).checked_cmp(decimal(0, 0)).unwrap(),
        std::cmp::Ordering::Less
    );
}

#[test]
fn arithmetic_never_wraps_or_saturates_and_intermediate_limits_are_explicit() {
    assert_eq!(
        decimal(i128::MAX, 0).checked_add(decimal(1, 0)),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        decimal(i128::MIN, 0).checked_sub(decimal(1, 0)),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        decimal(i128::MIN, 0).checked_div(decimal(-1, 0), Rounding::Exact),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        decimal(1, 0).checked_div(decimal(0, 0), Rounding::Exact),
        Err(NumericError::DivisionByZero)
    );
    assert_eq!(
        decimal(i128::MAX, 1).checked_div(decimal(10, 1), Rounding::Exact),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        decimal(i128::MAX, 1).checked_mul(decimal(10, 1), Rounding::Exact),
        Err(NumericError::Overflow)
    );
    assert_eq!(
        decimal(i128::MIN, 0)
            .checked_div(decimal(i128::MIN, 0), Rounding::HalfEven)
            .unwrap(),
        decimal(1, 0)
    );
    assert_eq!(
        decimal(1, 0)
            .checked_div(decimal(i128::MIN, 0), Rounding::HalfEven)
            .unwrap(),
        decimal(0, 0)
    );
}

#[test]
fn independent_eighths_round_to_nearest_even_for_both_denominator_signs() {
    for (numerator, expected) in [(1, 12), (3, 38), (5, 62), (7, 88)] {
        for sign_n in [-1, 1] {
            for sign_d in [-1, 1] {
                let result = decimal(numerator * sign_n * 100, 2)
                    .checked_div(decimal(800 * sign_d, 2), Rounding::HalfEven)
                    .unwrap();
                assert_eq!(result, decimal(expected * sign_n * sign_d, 2));
            }
        }
    }
    assert_eq!(
        decimal(15, 1)
            .checked_mul(decimal(15, 1), Rounding::HalfEven)
            .unwrap(),
        decimal(22, 1)
    );
    assert_eq!(
        decimal(15, 1).checked_mul(decimal(15, 1), Rounding::Exact),
        Err(NumericError::Inexact)
    );
}

#[test]
fn unsupported_shapes_types_and_versions_fail_at_descriptor_construction() {
    use fqe_core::types::ValueShape;
    for shape in [
        ValueShape::Series,
        ValueShape::Window,
        ValueShape::Record,
        ValueShape::Collection,
        ValueShape::Unsupported("tensor".into()),
    ] {
        assert_eq!(
            TypeDescriptor::new(1, shape.clone(), ScalarType::Bool),
            Err(TypeError::UnsupportedShape(shape))
        );
    }
    assert_eq!(
        TypeDescriptor::new(2, ValueShape::Scalar, ScalarType::Bool),
        Err(TypeError::UnsupportedVersion(2))
    );
    for name in ["timestamp", "duration", "instrument-ref", "f64"] {
        assert_eq!(
            TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Unsupported(name.into())),
            Err(TypeError::UnsupportedScalar(name.into()))
        );
    }
    assert_eq!(
        TypeDescriptor::new(
            1,
            ValueShape::Scalar,
            ScalarType::Decimal {
                scale: 19,
                unit: NumericUnit::Scalar
            }
        ),
        Err(TypeError::InvalidScale(19))
    );
    for max_bytes in [0, 4097] {
        assert_eq!(
            TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Text { max_bytes }),
            Err(TypeError::InvalidTextLimit)
        );
    }
}

#[test]
fn typed_values_preserve_units_scale_and_utf8_byte_limits() {
    assert!(TypedValue::new(descriptor(ScalarType::U64), ScalarValue::I64(1)).is_err());
    let price = descriptor(ScalarType::Decimal {
        scale: 2,
        unit: NumericUnit::Price(InstrumentId::new("TEST:ASSET").unwrap()),
    });
    for n in [0, -100, i128::MAX] {
        assert!(TypedValue::new(price.clone(), ScalarValue::Decimal(decimal(n, 2))).is_ok());
    }
    assert_eq!(
        TypedValue::new(price, ScalarValue::Decimal(decimal(1, 0))),
        Err(TypeError::ValueMismatch)
    );
    let text = descriptor(ScalarType::Text { max_bytes: 3 });
    assert!(TypedValue::new(text.clone(), ScalarValue::Text("量".into())).is_ok());
    assert!(TypedValue::new(text, ScalarValue::Text("量化".into())).is_err());
    assert_eq!(CurrencyCode::new("USD").unwrap().as_str(), "USD");
    for invalid in ["", "usd", "US D", "C".repeat(17).as_str()] {
        assert!(CurrencyCode::new(invalid).is_err());
    }
}

#[test]
fn readiness_cannot_be_confused_with_valid_zero() {
    let zero = TypedValue::new(descriptor(ScalarType::U64), ScalarValue::U64(0)).unwrap();
    let ready = Evaluation::Ready(zero);
    assert_ne!(ready, Evaluation::Missing);
    assert_ne!(ready, Evaluation::Warmup);
    assert_ne!(ready, Evaluation::Error(NumericError::Overflow));
    assert_ne!(Evaluation::<TypedValue>::Missing, Evaluation::Warmup);
}
