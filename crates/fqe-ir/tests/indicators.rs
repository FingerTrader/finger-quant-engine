use fqe_core::{
    numeric::{Decimal, NumericError},
    types::{
        Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
    },
};
use fqe_ir::{
    expressions::{
        BinaryOp, CrossDirection, Crossing, Expr, ExpressionDraft, ExpressionInstance,
        ExpressionPlan,
    },
    indicators,
    model::{ContentId, EntryKey, Port},
    parameters::{ParameterInput, ParameterSchema},
    registry::{
        CallablePort, Category, Descriptor, DescriptorDraft, IncrementalInstance,
        IncrementalProvider, IncrementalState, NamedValues, ProviderCall, ProviderFailure, Purity,
        ReadinessPolicy, Registry, RegistryBuilder,
    },
};
use std::sync::Arc;
const F: i128 = 1_000_000_000_000;
fn ty(scale: u8, unit: NumericUnit) -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Decimal { scale, unit }).unwrap()
}
fn input_type() -> TypeDescriptor {
    ty(12, NumericUnit::Scalar)
}
fn value(n: i128) -> TypedValue {
    TypedValue::new(
        input_type(),
        ScalarValue::Decimal(Decimal::new(n, 12).unwrap()),
    )
    .unwrap()
}
fn params(values: &[(&str, u64)]) -> Vec<(String, ParameterInput)> {
    let t = TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).unwrap();
    values
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                ParameterInput::Value(Evaluation::Ready(
                    TypedValue::new(t.clone(), ScalarValue::U64(*v)).unwrap(),
                )),
            )
        })
        .collect()
}
fn registry() -> Registry {
    let mut b = RegistryBuilder::new();
    indicators::register(&mut b, "test", input_type()).unwrap();
    b.freeze().unwrap()
}
fn instance(r: &Registry, name: &str, p: &[(&str, u64)]) -> IncrementalInstance {
    let d = r
        .descriptors()
        .find(|d| d.declaration().key.id == format!("test.{name}"))
        .unwrap();
    r.instantiate(&d.requirement(), params(p), "run", name)
        .unwrap()
}
fn step(i: &mut IncrementalInstance, n: u64, x: i128) -> fqe_ir::registry::CallResult {
    i.step(n, vec![("close".into(), Evaluation::Ready(value(x)))])
        .unwrap()
}
fn coefficient(v: &Evaluation<TypedValue>) -> Option<i128> {
    match v {
        Evaluation::Ready(v) => Some(v.numeric_coefficient().unwrap()),
        Evaluation::Warmup => None,
        _ => panic!("unexpected readiness"),
    }
}
#[test]
fn independent_rational_goldens_cover_seeds_warmup_edges_and_long_series() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/indicators.json")).unwrap();
    let r = registry();
    for data in fixture["fixtures"].as_array().unwrap() {
        let mut calls = vec![
            (
                instance(&r, "sma", &[("period", 2)]),
                vec![("value", "sma2")],
            ),
            (
                instance(&r, "sma", &[("period", 3)]),
                vec![("value", "sma3")],
            ),
            (
                instance(&r, "ema", &[("period", 2)]),
                vec![("value", "ema2")],
            ),
            (
                instance(&r, "ema", &[("period", 3)]),
                vec![("value", "ema3")],
            ),
            (
                instance(&r, "macd", &[("fast", 2), ("slow", 3), ("signal", 2)]),
                vec![
                    ("line", "line"),
                    ("signal", "signal"),
                    ("histogram", "histogram"),
                ],
            ),
            (
                instance(&r, "rsi", &[("period", 2)]),
                vec![("value", "rsi2")],
            ),
        ];
        for (k, close) in data["closes"].as_array().unwrap().iter().enumerate() {
            for (call, ports) in &mut calls {
                let result = step(call, k as u64 + 1, close.as_str().unwrap().parse().unwrap());
                for (port, golden) in ports.iter() {
                    let actual = coefficient(&result.outputs()[*port]);
                    let expected = data["expected"][golden][k]
                        .as_str()
                        .map(|s| s.parse::<i128>().unwrap());
                    match (actual, expected) {
                        (Some(a), Some(e)) => assert!(
                            (a - e).abs() <= 1,
                            "{} bar{} {golden}: {a} != {e}",
                            data["id"],
                            k + 1
                        ),
                        _ => assert_eq!(actual, expected, "{} bar{} {golden}", data["id"], k + 1),
                    }
                }
            }
        }
    }
}
#[test]
fn g007_registered_sma_outputs_drive_generic_crossing_at_bar_five_only() {
    let r = registry();
    let mut a = instance(&r, "sma", &[("period", 2)]);
    let mut b = instance(&r, "sma", &[("period", 3)]);
    let t = input_type();
    let plan = ExpressionPlan::new(ExpressionDraft {
        inputs: vec![
            Port {
                name: "a".into(),
                value_type: t.clone(),
            },
            Port {
                name: "b".into(),
                value_type: t,
            },
        ],
        crossings: vec![Crossing {
            id: "up".into(),
            direction: CrossDirection::Above,
            left: Expr::Input("a".into()),
            right: Expr::Input("b".into()),
        }],
        outputs: vec![("up".into(), Expr::Cross("up".into()))],
    })
    .unwrap();
    let mut cross = ExpressionInstance::new(Arc::new(plan), "run", "cross").unwrap();
    let mut signals = vec![];
    for (k, x) in [3, 2, 1, 2, 3, 4].iter().enumerate() {
        let n = k as u64 + 1;
        let av = step(&mut a, n, x * F).outputs()["value"].clone();
        let bv = step(&mut b, n, x * F).outputs()["value"].clone();
        let result = cross
            .evaluate(n, vec![("a".into(), av), ("b".into(), bv)])
            .unwrap();
        if matches!(&result.outputs["up"],Evaluation::Ready(v) if v.value()==&ScalarValue::Bool(true))
        {
            signals.push(n);
        }
    }
    assert_eq!(signals, vec![5]);
}
#[test]
fn invalid_parameters_and_profiles_fail_before_instances() {
    let r = registry();
    for name in ["sma", "ema", "rsi"] {
        let d = r
            .descriptors()
            .find(|d| d.declaration().key.id == format!("test.{name}"))
            .unwrap();
        for p in [
            vec![],
            params(&[("period", 0)]),
            params(&[("period", 10001)]),
            params(&[("other", 2)]),
        ] {
            assert!(r.instantiate(&d.requirement(), p, "run", "node").is_err());
        }
        assert!(
            r.prepare(&d.requirement(), params(&[("period", 2)]))
                .is_err()
        );
    }
    let d = r
        .descriptors()
        .find(|d| d.declaration().key.id == "test.macd")
        .unwrap();
    for (fast, slow) in [(2, 2), (3, 2)] {
        assert!(
            r.instantiate(
                &d.requirement(),
                params(&[("fast", fast), ("slow", slow), ("signal", 2)]),
                "run",
                "m"
            )
            .is_err()
        );
    }
    let mut wrong = d.declaration().clone();
    wrong.state_schema = None;
    assert!(Descriptor::new(wrong).is_err());
    let json: serde_json::Value = serde_json::from_slice(d.canonical_bytes()).unwrap();
    assert_eq!(json["purity"], "stateful");
    assert_eq!(json["history"], 19999);
    assert_eq!(json["readiness"], "provider-managed");
    assert!(json["state_schema"].is_string());
    let mut changed = d.declaration().clone();
    changed.state_schema = Some("different/1".into());
    assert_ne!(
        d.content_id(),
        Descriptor::new(changed).unwrap().content_id()
    );
}
#[test]
fn error_sequence_and_input_failures_preserve_history_for_same_sequence_retry() {
    let r = registry();
    for name in ["sma", "ema", "macd", "rsi"] {
        let p = if name == "macd" {
            vec![("fast", 2), ("slow", 3), ("signal", 2)]
        } else {
            vec![("period", 2)]
        };
        let mut a = instance(&r, name, &p);
        let mut reference = instance(&r, name, &p);
        step(&mut a, 1, F);
        step(&mut reference, 1, F);
        for bad in [
            Evaluation::Ready(value((indicators::MAX_CLOSE + 1) * F)),
            Evaluation::Ready(value(i128::MAX)),
            Evaluation::Error(NumericError::Overflow),
        ] {
            assert!(a.step(2, vec![("close".into(), bad)]).is_err());
            assert_eq!(a.last_sequence(), Some(1));
            assert_eq!(a.accepted_steps(), 1);
        }
        assert!(a.step(2, vec![]).is_err());
        assert!(
            a.step(1, vec![("close".into(), Evaluation::Ready(value(2 * F)))])
                .is_err()
        );
        for n in 2..=6 {
            assert_eq!(
                step(&mut a, n, n as i128 * F).outputs(),
                step(&mut reference, n, n as i128 * F).outputs()
            );
        }
    }
}
#[test]
fn missing_and_warmup_reset_each_owned_instance_and_sequence_gaps_do_not() {
    let r = registry();
    let mut a = instance(&r, "ema", &[("period", 2)]);
    let mut b = instance(&r, "ema", &[("period", 2)]);
    step(&mut a, 1, F);
    assert_eq!(
        coefficient(&step(&mut a, 8, 3 * F).outputs()["value"]),
        Some(2 * F)
    );
    assert_eq!(
        coefficient(&step(&mut b, 1, 9 * F).outputs()["value"]),
        None
    );
    for unavailable in [Evaluation::Missing, Evaluation::Warmup] {
        let n = a.last_sequence().unwrap() + 1;
        assert_eq!(
            a.step(n, vec![("close".into(), unavailable.clone())])
                .unwrap()
                .outputs()["value"],
            unavailable
        );
        assert_eq!(
            coefficient(&step(&mut a, n + 1, 5 * F).outputs()["value"]),
            None
        );
        assert_eq!(
            coefficient(&step(&mut a, n + 2, 7 * F).outputs()["value"]),
            Some(6 * F)
        );
    }
    assert_eq!(
        coefficient(&step(&mut b, 2, 11 * F).outputs()["value"]),
        Some(10 * F)
    );
}
#[test]
fn precision_limits_half_even_output_and_strict_decision_boundary() {
    let r = registry();
    for (xs, expected) in [([0, 1], 0), ([1, 2], 2), ([-1, 0], 0), ([-2, -1], -2)] {
        let mut a = instance(&r, "sma", &[("period", 2)]);
        step(&mut a, 1, xs[0]);
        assert_eq!(
            coefficient(&step(&mut a, 2, xs[1]).outputs()["value"]),
            Some(expected)
        );
    }
    let mut a = instance(&r, "ema", &[("period", 1)]);
    for (n, x) in [
        (1, indicators::MAX_CLOSE * F),
        (2, -indicators::MAX_CLOSE * F),
        (3, 1),
    ] {
        assert_eq!(coefficient(&step(&mut a, n, x).outputs()["value"]), Some(x));
    }
    let mut rsi = instance(&r, "rsi", &[("period", 1)]);
    step(&mut rsi, 1, 0);
    assert_eq!(
        rsi.step(2, vec![("close".into(), Evaluation::Ready(value(1)))])
            .unwrap_err()
            .code,
        "registry_precision"
    );
    assert_eq!(rsi.last_sequence(), Some(1));
    assert_eq!(
        coefficient(&step(&mut rsi, 2, 1_000_000).outputs()["value"]),
        Some(100 * F)
    );
    // Binary64 loses a single output quantum here; the decimal provider retains it.
    assert_eq!(
        1_000_000_000_000_f64 - 0.000000000001_f64,
        1_000_000_000_000_f64
    );
    let mut large = instance(&r, "ema", &[("period", 1)]);
    assert_eq!(
        coefficient(&step(&mut large, 1, indicators::MAX_CLOSE * F - 1).outputs()["value"]),
        Some(indicators::MAX_CLOSE * F - 1)
    );
    // Comparison is on quantized outputs, never a tolerance/epsilon comparison.
    let mut a = instance(&r, "sma", &[("period", 1)]);
    let xs = [2 * F - 1, 2 * F, 2 * F + 1];
    let decisions: Vec<_> = xs
        .into_iter()
        .enumerate()
        .map(|(k, x)| {
            coefficient(&step(&mut a, k as u64 + 1, x).outputs()["value"]).unwrap() > 2 * F
        })
        .collect();
    assert_eq!(decisions, vec![false, false, true]);
}
#[test]
fn period_one_maximum_period_and_numeric_units_are_explicit() {
    let r = registry();
    let mut sma = instance(&r, "sma", &[("period", 10000)]);
    for n in 1..10000 {
        assert_eq!(coefficient(&step(&mut sma, n, F).outputs()["value"]), None);
    }
    assert_eq!(
        coefficient(&step(&mut sma, 10000, F).outputs()["value"]),
        Some(F)
    );
    let mut m = instance(&r, "macd", &[("fast", 1), ("slow", 2), ("signal", 1)]);
    step(&mut m, 1, F);
    let out = step(&mut m, 2, 3 * F);
    assert_eq!(coefficient(&out.outputs()["line"]), Some(F));
    assert_eq!(coefficient(&out.outputs()["histogram"]), Some(0));
    let price = NumericUnit::Price(fqe_core::InstrumentId::new("DEMO").unwrap());
    let mut b = RegistryBuilder::new();
    indicators::register(&mut b, "price", ty(2, price.clone())).unwrap();
    let p = b.freeze().unwrap();
    assert_eq!(
        p.descriptors()
            .find(|d| d.declaration().key.id == "price.sma")
            .unwrap()
            .declaration()
            .outputs[0]
            .value_type,
        ty(12, price)
    );
    assert_eq!(
        p.descriptors()
            .find(|d| d.declaration().key.id == "price.rsi")
            .unwrap()
            .declaration()
            .outputs[0]
            .value_type,
        ty(12, NumericUnit::Ratio)
    );
    for t in [ty(13, NumericUnit::Scalar), ty(12, NumericUnit::Ratio)] {
        assert!(indicators::register(&mut RegistryBuilder::new(), "bad", t).is_err());
    }
    let mut a = instance(&r, "sma", &[("period", 1)]);
    let wrong = TypedValue::new(
        ty(2, NumericUnit::Scalar),
        ScalarValue::Decimal(Decimal::new(100, 2).unwrap()),
    )
    .unwrap();
    assert!(
        a.step(1, vec![("close".into(), Evaluation::Ready(wrong))])
            .is_err()
    );
    assert_eq!(a.accepted_steps(), 0);
}
// Independent unknown-name provider deliberately mutates staged state before failure.
struct CounterFactory;
#[derive(Clone)]
struct Counter(u64);
impl IncrementalProvider for CounterFactory {
    fn create(
        &self,
        _: &fqe_ir::parameters::BoundParameters,
    ) -> Result<Box<dyn IncrementalState>, ProviderFailure> {
        Ok(Box::new(Counter(0)))
    }
}
impl IncrementalState for Counter {
    fn fork(&self) -> Box<dyn IncrementalState> {
        Box::new(self.clone())
    }
    fn step(&mut self, call: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        self.0 += 1;
        let n = call.input("close").unwrap().numeric_coefficient().unwrap();
        if n == -1 {
            return Err(ProviderFailure::Failed);
        }
        if n == -2 {
            panic!("reviewed test panic");
        }
        if n == -3 {
            return Ok(vec![]);
        }
        Ok(vec![(
            "value".into(),
            Evaluation::Ready(value(self.0 as i128 * F)),
        )])
    }
}
#[test]
fn generic_unknown_stateful_provider_rolls_back_errors_panics_and_invalid_outputs() {
    let mut draft = DescriptorDraft::pure(
        EntryKey {
            id: "independent.counter".into(),
            version: "1".into(),
        },
        ContentId::from_bytes([19; 32]),
        ParameterSchema::new(1, vec![]).unwrap(),
        "test owned counter/1",
    );
    draft.category = Category::Function;
    draft.purity = Purity::Stateful;
    draft.readiness = ReadinessPolicy::ProviderManaged;
    draft.history = 1;
    draft.state_schema = Some("independent.state/1".into());
    draft.required_capabilities = vec!["provider.incremental-scalar/1".into()];
    draft.inputs = vec![CallablePort::one("close", input_type())];
    draft.outputs = vec![CallablePort::one("value", input_type())];
    let d = Descriptor::new(draft).unwrap();
    let pin = d.requirement();
    let mut b = RegistryBuilder::new();
    b.register_incremental(d, Arc::new(CounterFactory)).unwrap();
    let r = b.freeze().unwrap();
    let mut a = r.instantiate(&pin, vec![], "run", "one").unwrap();
    let mut b = r.instantiate(&pin, vec![], "run", "two").unwrap();
    assert_eq!(coefficient(&step(&mut a, 1, 0).outputs()["value"]), Some(F));
    for n in [-1, -2, -3] {
        assert!(
            a.step(2, vec![("close".into(), Evaluation::Ready(value(n)))])
                .is_err()
        );
        assert_eq!(a.last_sequence(), Some(1));
    }
    assert_eq!(
        coefficient(&step(&mut a, 2, 0).outputs()["value"]),
        Some(2 * F)
    );
    assert_eq!(coefficient(&step(&mut b, 1, 0).outputs()["value"]), Some(F));
}

#[test]
fn g002_named_macd_and_rsi_outputs_have_stable_threshold_decisions() {
    let r = registry();
    for (threshold, expected) in [(60, vec![5]), (90, vec![])] {
        let mut macd = instance(&r, "macd", &[("fast", 2), ("slow", 3), ("signal", 2)]);
        let mut rsi = instance(&r, "rsi", &[("period", 2)]);
        let ratio = ty(12, NumericUnit::Ratio);
        let threshold = TypedValue::new(
            ratio.clone(),
            ScalarValue::Decimal(Decimal::new(threshold * F, 12).unwrap()),
        )
        .unwrap();
        let plan = ExpressionPlan::new(ExpressionDraft {
            inputs: vec![
                Port {
                    name: "line".into(),
                    value_type: input_type(),
                },
                Port {
                    name: "signal".into(),
                    value_type: input_type(),
                },
                Port {
                    name: "rsi".into(),
                    value_type: ratio,
                },
            ],
            crossings: vec![Crossing {
                id: "up".into(),
                direction: CrossDirection::Above,
                left: Expr::Input("line".into()),
                right: Expr::Input("signal".into()),
            }],
            outputs: vec![(
                "decision".into(),
                Expr::Binary {
                    op: BinaryOp::And,
                    left: Box::new(Expr::Cross("up".into())),
                    right: Box::new(Expr::Binary {
                        op: BinaryOp::Greater,
                        left: Box::new(Expr::Input("rsi".into())),
                        right: Box::new(Expr::Literal(threshold)),
                    }),
                },
            )],
        })
        .unwrap();
        let mut expressions = ExpressionInstance::new(Arc::new(plan), "run", "decision").unwrap();
        let mut decisions = vec![];
        for (k, x) in [1, 2, 3, 2, 4, 3].iter().enumerate() {
            let n = k as u64 + 1;
            let m = step(&mut macd, n, x * F);
            let r = step(&mut rsi, n, x * F);
            let result = expressions
                .evaluate(
                    n,
                    vec![
                        ("line".into(), m.outputs()["line"].clone()),
                        ("signal".into(), m.outputs()["signal"].clone()),
                        ("rsi".into(), r.outputs()["value"].clone()),
                    ],
                )
                .unwrap();
            if matches!(&result.outputs["decision"],Evaluation::Ready(v) if v.value()==&ScalarValue::Bool(true))
            {
                decisions.push(n);
            }
        }
        assert_eq!(decisions, expected);
    }
}

#[test]
fn maximum_period_compound_warmup_matches_declared_initial_history() {
    let r = registry();
    let mut m = instance(
        &r,
        "macd",
        &[("fast", 9999), ("slow", 10000), ("signal", 10000)],
    );
    assert_eq!(m.descriptor().declaration().history, 19999);
    for n in 1..19999 {
        let out = step(&mut m, n, F);
        assert_eq!(coefficient(&out.outputs()["signal"]), None);
        assert_eq!(
            coefficient(&out.outputs()["line"]),
            if n < 10000 { None } else { Some(0) }
        );
    }
    let out = step(&mut m, 19999, F);
    assert_eq!(coefficient(&out.outputs()["signal"]), Some(0));
    assert_eq!(coefficient(&out.outputs()["histogram"]), Some(0));
    let mut rsi = instance(&r, "rsi", &[("period", 10000)]);
    for n in 1..=10000 {
        assert_eq!(coefficient(&step(&mut rsi, n, F).outputs()["value"]), None);
    }
    assert_eq!(
        coefficient(&step(&mut rsi, 10001, F).outputs()["value"]),
        Some(50 * F)
    );
}
