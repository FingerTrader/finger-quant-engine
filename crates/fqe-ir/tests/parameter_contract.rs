use fqe_core::{
    InstrumentId,
    numeric::{Decimal, NumericError},
    types::{
        CurrencyCode, Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue,
        ValueShape,
    },
};
use fqe_ir::parameters::{
    Bound, BoundValue, Constraints, ParameterError, ParameterInput, ParameterSchema, ParameterSpec,
};

fn ty(kind: ScalarType) -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, kind).unwrap()
}
fn uint(n: u64) -> TypedValue {
    TypedValue::new(ty(ScalarType::U64), ScalarValue::U64(n)).unwrap()
}
fn input(v: TypedValue) -> ParameterInput {
    ParameterInput::Value(Evaluation::Ready(v))
}
fn spec(
    key: &str,
    descriptor: TypeDescriptor,
    required: bool,
    default: Option<TypedValue>,
    constraints: Constraints,
) -> Result<ParameterSpec, ParameterError> {
    ParameterSpec::new(key, descriptor, required, default, constraints, "fixture")
}
fn schema(specs: Vec<ParameterSpec>) -> ParameterSchema {
    ParameterSchema::new(1, specs).unwrap()
}
fn examples() -> Vec<TypedValue> {
    let mut values = vec![
        TypedValue::new(ty(ScalarType::Bool), ScalarValue::Bool(false)).unwrap(),
        TypedValue::new(ty(ScalarType::I64), ScalarValue::I64(i64::MIN)).unwrap(),
        uint(u64::MAX),
        TypedValue::new(
            ty(ScalarType::Text { max_bytes: 8 }),
            ScalarValue::Text("EMA".into()),
        )
        .unwrap(),
    ];
    for unit in [
        NumericUnit::Scalar,
        NumericUnit::Ratio,
        NumericUnit::Price(InstrumentId::new("A").unwrap()),
        NumericUnit::Quantity(InstrumentId::new("A").unwrap()),
        NumericUnit::Money(CurrencyCode::new("USD").unwrap()),
    ] {
        values.push(
            TypedValue::new(
                ty(ScalarType::Decimal { scale: 2, unit }),
                ScalarValue::Decimal(Decimal::new(-125, 2).unwrap()),
            )
            .unwrap(),
        );
    }
    values
}

fn alternate(value: &TypedValue) -> TypedValue {
    let scalar = match value.value() {
        ScalarValue::Bool(v) => ScalarValue::Bool(!v),
        ScalarValue::I64(_) => ScalarValue::I64(1),
        ScalarValue::U64(_) => ScalarValue::U64(0),
        ScalarValue::Text(_) => ScalarValue::Text("RSI".into()),
        ScalarValue::Decimal(v) => ScalarValue::Decimal(Decimal::new(0, v.scale()).unwrap()),
    };
    TypedValue::new(value.descriptor().clone(), scalar).unwrap()
}

#[test]
fn required_default_absence_and_invalid_input_matrix_covers_every_supported_type() {
    for value in examples() {
        let descriptor = value.descriptor().clone();
        assert!(matches!(
            spec(
                "p",
                descriptor.clone(),
                true,
                Some(value.clone()),
                Constraints::default()
            ),
            Err(ParameterError::RequiredWithDefault(_))
        ));
        let required = schema(vec![
            spec("p", descriptor.clone(), true, None, Constraints::default()).unwrap(),
        ]);
        assert_eq!(
            required.bind([]),
            Err(ParameterError::MissingRequired("p".into()))
        );
        assert_eq!(
            required
                .bind([("p".into(), input(value.clone()))])
                .unwrap()
                .get("p"),
            Some(&BoundValue::Value(value.clone()))
        );
        let optional = schema(vec![
            spec("p", descriptor.clone(), false, None, Constraints::default()).unwrap(),
        ]);
        let absent = optional.bind([]).unwrap();
        assert_eq!(absent.get("p"), Some(&BoundValue::Unbound));
        assert_eq!(absent.get("unknown"), None);
        let default = schema(vec![
            spec(
                "p",
                descriptor,
                false,
                Some(value.clone()),
                Constraints::default(),
            )
            .unwrap(),
        ]);
        assert_eq!(
            default.bind([]).unwrap().get("p"),
            Some(&BoundValue::Value(value.clone()))
        );
        let supplied = alternate(&value);
        assert_ne!(supplied, value);
        assert_eq!(
            default
                .bind([("p".into(), input(supplied.clone()))])
                .unwrap()
                .get("p"),
            Some(&BoundValue::Value(supplied))
        );
        assert_eq!(
            default.bind([
                ("p".into(), input(value.clone())),
                ("p".into(), input(value.clone()))
            ]),
            Err(ParameterError::DuplicateKey("p".into()))
        );
        assert_eq!(
            default.bind([("unknown".into(), input(value.clone()))]),
            Err(ParameterError::UnknownKey("unknown".into()))
        );
        for invalid in [
            ParameterInput::Null,
            ParameterInput::Value(Evaluation::Missing),
            ParameterInput::Value(Evaluation::Warmup),
            ParameterInput::Value(Evaluation::Error(NumericError::Inexact)),
        ] {
            assert_eq!(
                default.bind([("p".into(), invalid)]),
                Err(ParameterError::InvalidInput("p".into()))
            );
        }
        let wrong = if matches!(value.value(), ScalarValue::Bool(_)) {
            uint(1)
        } else {
            TypedValue::new(ty(ScalarType::Bool), ScalarValue::Bool(true)).unwrap()
        };
        assert_eq!(
            spec(
                "p",
                value.descriptor().clone(),
                false,
                Some(wrong.clone()),
                Constraints::default()
            ),
            Err(ParameterError::InvalidDefault("p".into()))
        );
        assert_eq!(
            required.bind([("p".into(), input(wrong))]),
            Err(ParameterError::TypeMismatch("p".into()))
        );
    }
}

#[test]
fn supplied_value_overrides_default_without_mutating_existing_bindings() {
    let definition = schema(vec![
        spec(
            "period",
            ty(ScalarType::U64),
            false,
            Some(uint(2)),
            Constraints::default(),
        )
        .unwrap(),
    ]);
    let first = definition.bind([]).unwrap();
    let second = definition
        .bind([("period".into(), input(uint(3)))])
        .unwrap();
    assert_eq!(first.get("period"), Some(&BoundValue::Value(uint(2))));
    assert_eq!(second.get("period"), Some(&BoundValue::Value(uint(3))));
    assert!(
        definition
            .bind([("period".into(), ParameterInput::Null)])
            .is_err()
    );
    assert_eq!(definition.bind([]).unwrap(), first);
    assert_eq!(
        definition.specs().next().unwrap().default_value(),
        Some(&uint(2))
    );
}

#[test]
fn schema_and_binding_duplicates_and_unknown_keys_are_rejected_before_map_loss() {
    let p = spec(
        "p",
        ty(ScalarType::U64),
        false,
        None,
        Constraints::default(),
    )
    .unwrap();
    assert_eq!(
        ParameterSchema::new(1, vec![p.clone(), p.clone()]),
        Err(ParameterError::DuplicateKey("p".into()))
    );
    assert_eq!(
        ParameterSchema::new(2, vec![]),
        Err(ParameterError::UnsupportedVersion(2))
    );
    let definition = schema(vec![p]);
    assert_eq!(
        definition.bind([("p".into(), input(uint(1))), ("p".into(), input(uint(2)))]),
        Err(ParameterError::DuplicateKey("p".into()))
    );
    assert_eq!(
        definition.bind([("other".into(), input(uint(1)))]),
        Err(ParameterError::UnknownKey("other".into()))
    );
}

#[test]
fn inclusive_exclusive_bounds_and_defaults_are_checked_at_definition_and_binding() {
    let constraints = Constraints {
        lower: Some(Bound {
            value: uint(1),
            inclusive: false,
        }),
        upper: Some(Bound {
            value: uint(3),
            inclusive: true,
        }),
        choices: None,
    };
    assert_eq!(
        spec(
            "p",
            ty(ScalarType::U64),
            false,
            Some(uint(1)),
            constraints.clone()
        ),
        Err(ParameterError::InvalidDefault("p".into()))
    );
    let definition = schema(vec![
        spec("p", ty(ScalarType::U64), true, None, constraints).unwrap(),
    ]);
    for (n, valid) in [
        (0, false),
        (1, false),
        (2, true),
        (3, true),
        (4, false),
        (u64::MAX, false),
    ] {
        assert_eq!(
            definition.bind([("p".into(), input(uint(n)))]).is_ok(),
            valid
        );
    }
    for (lower, upper, li, ui) in [
        (3, 2, true, true),
        (2, 2, false, true),
        (1, 2, false, false),
        (u64::MAX, u64::MAX, false, true),
        (0, 0, true, false),
    ] {
        let constraints = Constraints {
            lower: Some(Bound {
                value: uint(lower),
                inclusive: li,
            }),
            upper: Some(Bound {
                value: uint(upper),
                inclusive: ui,
            }),
            choices: None,
        };
        assert!(matches!(
            spec("p", ty(ScalarType::U64), false, None, constraints),
            Err(ParameterError::InvalidConstraints(_))
        ));
    }
}

#[test]
fn enum_sets_validate_members_types_duplicates_ranges_and_defaults() {
    let constraints = Constraints {
        choices: Some(vec![uint(2), uint(4)]),
        ..Constraints::default()
    };
    assert!(
        spec(
            "p",
            ty(ScalarType::U64),
            false,
            Some(uint(3)),
            constraints.clone()
        )
        .is_err()
    );
    let definition = schema(vec![
        spec("p", ty(ScalarType::U64), false, Some(uint(2)), constraints).unwrap(),
    ]);
    assert!(definition.bind([("p".into(), input(uint(4)))]).is_ok());
    assert_eq!(
        definition.bind([("p".into(), input(uint(3)))]),
        Err(ParameterError::NotAllowed("p".into()))
    );
    for choices in [
        vec![],
        vec![uint(1), uint(1)],
        vec![TypedValue::new(ty(ScalarType::I64), ScalarValue::I64(1)).unwrap()],
    ] {
        assert!(
            spec(
                "p",
                ty(ScalarType::U64),
                false,
                None,
                Constraints {
                    choices: Some(choices),
                    ..Constraints::default()
                }
            )
            .is_err()
        );
    }
    assert!(
        spec(
            "p",
            ty(ScalarType::U64),
            false,
            None,
            Constraints {
                lower: Some(Bound {
                    value: uint(2),
                    inclusive: true
                }),
                upper: None,
                choices: Some(vec![uint(1)])
            }
        )
        .is_err()
    );
    for v in examples() {
        let definition = schema(vec![
            spec(
                "p",
                v.descriptor().clone(),
                true,
                None,
                Constraints {
                    choices: Some(vec![v.clone()]),
                    ..Constraints::default()
                },
            )
            .unwrap(),
        ]);
        assert!(definition.bind([("p".into(), input(v))]).is_ok());
    }
}

#[test]
fn decimal_domain_scale_currency_and_instrument_mismatches_never_coerce() {
    let make = |unit, scale| {
        TypedValue::new(
            ty(ScalarType::Decimal { scale, unit }),
            ScalarValue::Decimal(Decimal::new(100, scale).unwrap()),
        )
        .unwrap()
    };
    let expected = make(NumericUnit::Price(InstrumentId::new("A").unwrap()), 2);
    let definition = schema(vec![
        spec(
            "p",
            expected.descriptor().clone(),
            true,
            None,
            Constraints::default(),
        )
        .unwrap(),
    ]);
    for wrong in [
        make(NumericUnit::Price(InstrumentId::new("B").unwrap()), 2),
        make(NumericUnit::Price(InstrumentId::new("A").unwrap()), 1),
        make(NumericUnit::Quantity(InstrumentId::new("A").unwrap()), 2),
        make(NumericUnit::Ratio, 2),
        make(NumericUnit::Scalar, 2),
    ] {
        assert_eq!(
            definition.bind([("p".into(), input(wrong.clone()))]),
            Err(ParameterError::TypeMismatch("p".into()))
        );
        assert!(
            spec(
                "p",
                expected.descriptor().clone(),
                false,
                Some(wrong.clone()),
                Constraints::default()
            )
            .is_err()
        );
        assert!(
            spec(
                "p",
                expected.descriptor().clone(),
                false,
                None,
                Constraints {
                    lower: Some(Bound {
                        value: wrong,
                        inclusive: true
                    }),
                    ..Constraints::default()
                }
            )
            .is_err()
        );
    }
    let usd = make(NumericUnit::Money(CurrencyCode::new("USD").unwrap()), 2);
    let eur = make(NumericUnit::Money(CurrencyCode::new("EUR").unwrap()), 2);
    let money = schema(vec![
        spec(
            "p",
            usd.descriptor().clone(),
            true,
            None,
            Constraints::default(),
        )
        .unwrap(),
    ]);
    assert!(money.bind([("p".into(), input(eur))]).is_err());
}

#[test]
fn numeric_ranges_cover_signed_and_decimal_extremes_and_reject_nonnumeric_bounds() {
    for value in examples() {
        let bounded = spec(
            "p",
            value.descriptor().clone(),
            false,
            Some(value.clone()),
            Constraints {
                lower: Some(Bound {
                    value: value.clone(),
                    inclusive: true,
                }),
                upper: Some(Bound {
                    value: value.clone(),
                    inclusive: true,
                }),
                choices: None,
            },
        );
        assert_eq!(bounded.is_ok(), value.numeric_coefficient().is_ok());
    }
    let v = TypedValue::new(
        ty(ScalarType::Decimal {
            scale: 18,
            unit: NumericUnit::Scalar,
        }),
        ScalarValue::Decimal(Decimal::new(i128::MAX, 18).unwrap()),
    )
    .unwrap();
    assert!(
        spec(
            "p",
            v.descriptor().clone(),
            false,
            None,
            Constraints {
                lower: Some(Bound {
                    value: v,
                    inclusive: false
                }),
                ..Constraints::default()
            }
        )
        .is_err()
    );
}

#[test]
fn invalid_keys_descriptions_and_resource_limits_are_rejected() {
    for key in ["", "bad key", "键", &"k".repeat(129)] {
        assert!(
            spec(
                key,
                ty(ScalarType::U64),
                false,
                None,
                Constraints::default()
            )
            .is_err()
        );
    }
    assert!(
        ParameterSpec::new(
            "p",
            ty(ScalarType::U64),
            false,
            None,
            Constraints::default(),
            "x".repeat(4097)
        )
        .is_err()
    );
    let too_many = (0..257)
        .map(|i| {
            spec(
                &format!("p{i}"),
                ty(ScalarType::U64),
                false,
                None,
                Constraints::default(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        ParameterSchema::new(1, too_many),
        Err(ParameterError::TooManyParameters)
    );
    let choices = (0..257).map(uint).collect();
    assert!(
        spec(
            "p",
            ty(ScalarType::U64),
            false,
            None,
            Constraints {
                choices: Some(choices),
                ..Constraints::default()
            }
        )
        .is_err()
    );
}

#[test]
fn maximum_schema_choice_and_binding_sizes_are_usable_but_extra_inputs_fail() {
    let specs = (0..256)
        .map(|i| {
            spec(
                &format!("p{i}"),
                ty(ScalarType::U64),
                true,
                None,
                Constraints::default(),
            )
            .unwrap()
        })
        .collect();
    let definition = schema(specs);
    assert_eq!(definition.version(), 1);
    let inputs: Vec<_> = (0..256)
        .map(|i| (format!("p{i}"), input(uint(i))))
        .collect();
    assert_eq!(definition.bind(inputs.clone()).unwrap().iter().count(), 256);
    let mut excess = inputs;
    excess.push(("p0".into(), input(uint(0))));
    assert_eq!(
        definition.bind(excess),
        Err(ParameterError::TooManyParameters)
    );
    let choices = Constraints {
        choices: Some((0..256).map(uint).collect()),
        ..Constraints::default()
    };
    let definition = schema(vec![
        spec("p", ty(ScalarType::U64), false, Some(uint(255)), choices).unwrap(),
    ]);
    assert!(definition.bind([]).is_ok());
}

#[test]
fn bindings_have_stable_key_order_and_independent_per_run_values() {
    let definition = schema(vec![
        spec(
            "z",
            ty(ScalarType::U64),
            false,
            Some(uint(1)),
            Constraints::default(),
        )
        .unwrap(),
        spec(
            "a",
            ty(ScalarType::U64),
            false,
            None,
            Constraints::default(),
        )
        .unwrap(),
    ]);
    let one = definition.bind([("a".into(), input(uint(2)))]).unwrap();
    let two = definition.bind([("a".into(), input(uint(3)))]).unwrap();
    assert_eq!(one.iter().map(|(k, _)| k).collect::<Vec<_>>(), ["a", "z"]);
    assert_ne!(one.get("a"), two.get("a"));
}
