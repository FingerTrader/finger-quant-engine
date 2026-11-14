#[path = "../examples/support/providers.rs"]
mod providers;
use fqe_core::{
    InstrumentId,
    numeric::{Decimal, NumericError, Rounding},
    types::{
        Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
    },
};
use fqe_ir::{
    expressions::*,
    model::{ContentId, EntryKey, Port},
    parameters::{ParameterInput, ParameterSchema},
    registry::*,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
fn t(kind: ScalarType) -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, kind).unwrap()
}
fn bool_t() -> TypeDescriptor {
    t(ScalarType::Bool)
}
fn b(v: bool) -> TypedValue {
    TypedValue::new(bool_t(), ScalarValue::Bool(v)).unwrap()
}
fn lit(n: u64) -> Expr {
    Expr::Literal(providers::value(n))
}
fn bin(op: BinaryOp, left: Expr, right: Expr) -> Expr {
    Expr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn plan(
    inputs: Vec<Port>,
    crossings: Vec<Crossing>,
    outputs: Vec<(String, Expr)>,
) -> Arc<ExpressionPlan> {
    Arc::new(
        ExpressionPlan::new(ExpressionDraft {
            inputs,
            crossings,
            outputs,
        })
        .unwrap(),
    )
}
fn instance(plan: Arc<ExpressionPlan>) -> ExpressionInstance {
    ExpressionInstance::new(plan, "test.run", "test.instance").unwrap()
}
fn port(name: &str, value_type: TypeDescriptor) -> Port {
    Port {
        name: name.into(),
        value_type,
    }
}
fn evaluate(e: Expr) -> Result<Evaluation<TypedValue>, fqe_ir::model::ModelError> {
    let p = plan(vec![], vec![], vec![("value".into(), e)]);
    Ok(instance(p).evaluate(1, vec![])?.outputs["value"].clone())
}
fn read_bool(v: &Evaluation<TypedValue>) -> bool {
    let Evaluation::Ready(v) = v else { panic!() };
    let ScalarValue::Bool(b) = v.value() else {
        panic!()
    };
    *b
}
fn ready(n: u64) -> Evaluation<TypedValue> {
    Evaluation::Ready(providers::value(n))
}
fn crossing(id: &str, direction: CrossDirection) -> Crossing {
    Crossing {
        id: id.into(),
        direction,
        left: Expr::Input("left".into()),
        right: Expr::Input("right".into()),
    }
}
fn cross_plan(output: Expr) -> Arc<ExpressionPlan> {
    plan(
        vec![
            port("left", providers::u64_type()),
            port("right", providers::u64_type()),
        ],
        vec![crossing("cross", CrossDirection::Above)],
        vec![("value".into(), output)],
    )
}
fn inputs(left: u64, right: u64) -> Vec<(String, Evaluation<TypedValue>)> {
    vec![("left".into(), ready(left)), ("right".into(), ready(right))]
}
#[test]
fn independent_boolean_truth_tables_include_left_to_right_error_precedence() {
    let and = ["FFFFF", "FTMWE", "FMMME", "FWMWE", "EEEEE"];
    let or = ["FTMWE", "TTTTT", "MTMME", "WTMWE", "EEEEE"];
    let values = [
        Evaluation::Ready(b(false)),
        Evaluation::Ready(b(true)),
        Evaluation::Missing,
        Evaluation::Warmup,
        Evaluation::Error(NumericError::Overflow),
    ];
    for (op, table) in [(BinaryOp::And, and), (BinaryOp::Or, or)] {
        let p = plan(
            vec![port("a", bool_t()), port("b", bool_t())],
            vec![],
            vec![(
                "value".into(),
                bin(op, Expr::Input("a".into()), Expr::Input("b".into())),
            )],
        );
        for (i, a) in values.iter().enumerate() {
            for (j, b) in values.iter().enumerate() {
                let mut run = instance(p.clone());
                let result =
                    run.evaluate(1, vec![("a".into(), a.clone()), ("b".into(), b.clone())]);
                let expected = table[i].as_bytes()[j];
                let actual = match result {
                    Ok(r) => match &r.outputs["value"] {
                        Evaluation::Ready(v) => match v.value() {
                            ScalarValue::Bool(false) => b'F',
                            ScalarValue::Bool(true) => b'T',
                            _ => panic!(),
                        },
                        Evaluation::Missing => b'M',
                        Evaluation::Warmup => b'W',
                        _ => panic!(),
                    },
                    Err(_) => {
                        assert_eq!(run.last_sequence(), None);
                        b'E'
                    }
                };
                assert_eq!(actual, expected, "{op:?} row {i} col {j}");
            }
        }
    }
}
#[test]
fn not_preserves_readiness_and_never_applies_numeric_truthiness() {
    let p = plan(
        vec![port("a", bool_t())],
        vec![],
        vec![("value".into(), Expr::Not(Box::new(Expr::Input("a".into()))))],
    );
    for value in [
        Evaluation::Ready(b(false)),
        Evaluation::Ready(b(true)),
        Evaluation::Missing,
        Evaluation::Warmup,
    ] {
        let mut r = instance(p.clone());
        let output = r
            .evaluate(1, vec![("a".into(), value.clone())])
            .unwrap()
            .outputs["value"]
            .clone();
        match value {
            Evaluation::Ready(v) => assert_ne!(output, Evaluation::Ready(v)),
            _ => assert_eq!(output, value),
        }
    }
    assert!(
        ExpressionPlan::new(ExpressionDraft {
            inputs: vec![],
            crossings: vec![],
            outputs: vec![("bad".into(), Expr::Not(Box::new(lit(1))))]
        })
        .is_err()
    );
}
#[test]
fn exact_integer_arithmetic_order_and_failure_boundaries() {
    for (op, a, b, expected) in [
        (BinaryOp::Add, 9007199254740993, 2, 9007199254740995),
        (BinaryOp::Subtract, 5, 2, 3),
        (BinaryOp::Multiply(Rounding::Exact), 5, 2, 10),
        (BinaryOp::Divide(Rounding::Exact), 10, 2, 5),
    ] {
        assert_eq!(evaluate(bin(op, lit(a), lit(b))).unwrap(), ready(expected));
    }
    for (op, a, b) in [
        (BinaryOp::Add, u64::MAX, 1),
        (BinaryOp::Subtract, 0, 1),
        (BinaryOp::Multiply(Rounding::Exact), u64::MAX, 2),
        (BinaryOp::Divide(Rounding::Exact), 1, 0),
        (BinaryOp::Divide(Rounding::Exact), 1, 3),
    ] {
        assert_eq!(
            evaluate(bin(op, lit(a), lit(b))).unwrap_err().code,
            "expression_numeric"
        );
    }
    let signed =
        |n| Expr::Literal(TypedValue::new(t(ScalarType::I64), ScalarValue::I64(n)).unwrap());
    assert!(
        evaluate(bin(
            BinaryOp::Divide(Rounding::Exact),
            signed(i64::MIN),
            signed(-1)
        ))
        .is_err()
    );
    assert_eq!(
        evaluate(bin(BinaryOp::Add, signed(-2), signed(3))).unwrap(),
        Evaluation::Ready(TypedValue::new(t(ScalarType::I64), ScalarValue::I64(1)).unwrap())
    );
}
fn decimal(coefficient: i128, scale: u8, unit: NumericUnit) -> TypedValue {
    TypedValue::new(
        t(ScalarType::Decimal { scale, unit }),
        ScalarValue::Decimal(Decimal::new(coefficient, scale).unwrap()),
    )
    .unwrap()
}
#[test]
fn decimal_rounding_and_domain_arithmetic_are_explicit() {
    let d = |n| Expr::Literal(decimal(n, 2, NumericUnit::Scalar));
    assert_eq!(
        evaluate(bin(BinaryOp::Add, d(10), d(20))).unwrap(),
        Evaluation::Ready(decimal(30, 2, NumericUnit::Scalar))
    );
    assert_eq!(
        evaluate(bin(BinaryOp::Divide(Rounding::HalfEven), d(100), d(300))).unwrap(),
        Evaluation::Ready(decimal(33, 2, NumericUnit::Scalar))
    );
    assert_eq!(
        evaluate(bin(BinaryOp::Multiply(Rounding::HalfEven), d(125), d(50))).unwrap(),
        Evaluation::Ready(decimal(62, 2, NumericUnit::Scalar))
    );
    assert!(evaluate(bin(BinaryOp::Multiply(Rounding::Exact), d(125), d(50))).is_err());
    let price = NumericUnit::Price(InstrumentId::new("A").unwrap());
    assert_eq!(
        evaluate(bin(
            BinaryOp::Subtract,
            Expr::Literal(decimal(125, 2, price.clone())),
            Expr::Literal(decimal(75, 2, price.clone()))
        ))
        .unwrap(),
        Evaluation::Ready(decimal(50, 2, price.clone()))
    );
    for op in [
        BinaryOp::Multiply(Rounding::Exact),
        BinaryOp::Divide(Rounding::Exact),
    ] {
        assert!(
            ExpressionPlan::new(ExpressionDraft {
                inputs: vec![],
                crossings: vec![],
                outputs: vec![(
                    "bad".into(),
                    bin(
                        op,
                        Expr::Literal(decimal(125, 2, price.clone())),
                        Expr::Literal(decimal(75, 2, price.clone()))
                    )
                )]
            })
            .is_err()
        );
    }
}
#[test]
fn comparison_equality_limits_and_incompatible_units_fail_at_plan_validation() {
    for (op, a, b, expected) in [
        (BinaryOp::Greater, 2, 2, false),
        (BinaryOp::GreaterEqual, 2, 2, true),
        (BinaryOp::Less, 1, 2, true),
        (BinaryOp::LessEqual, 2, 2, true),
        (BinaryOp::Equal, u64::MAX, u64::MAX, true),
        (BinaryOp::NotEqual, 1, 2, true),
    ] {
        assert_eq!(
            read_bool(&evaluate(bin(op, lit(a), lit(b))).unwrap()),
            expected
        );
    }
    for (left, right) in [
        (
            decimal(1, 2, NumericUnit::Scalar),
            decimal(1, 3, NumericUnit::Scalar),
        ),
        (
            decimal(1, 2, NumericUnit::Price(InstrumentId::new("A").unwrap())),
            decimal(1, 2, NumericUnit::Quantity(InstrumentId::new("A").unwrap())),
        ),
    ] {
        assert!(
            ExpressionPlan::new(ExpressionDraft {
                inputs: vec![],
                crossings: vec![],
                outputs: vec![(
                    "bad".into(),
                    bin(BinaryOp::Equal, Expr::Literal(left), Expr::Literal(right))
                )]
            })
            .is_err()
        );
    }
}
#[test]
fn subscribed_crossing_advances_behind_false_guards_and_with_shared_references() {
    let root = bin(
        BinaryOp::And,
        Expr::Input("guard".into()),
        bin(
            BinaryOp::And,
            Expr::Cross("cross".into()),
            Expr::Cross("cross".into()),
        ),
    );
    let p = plan(
        vec![
            port("left", providers::u64_type()),
            port("right", providers::u64_type()),
            port("guard", bool_t()),
        ],
        vec![crossing("cross", CrossDirection::Above)],
        vec![("value".into(), root)],
    );
    let mut run = instance(p);
    for (seq, left, guard, expected) in [
        (1, 1, false, false),
        (2, 3, false, false),
        (3, 4, true, false),
        (4, 2, true, false),
        (5, 3, true, true),
    ] {
        let mut v = inputs(left, 2);
        v.push(("guard".into(), Evaluation::Ready(b(guard))));
        assert_eq!(
            read_bool(&run.evaluate(seq, v).unwrap().outputs["value"]),
            expected
        );
    }
    assert_eq!(run.accepted_steps(), 5);
}
#[test]
fn equal_edges_first_ready_and_below_crossings_are_exact() {
    let p = plan(
        vec![
            port("left", providers::u64_type()),
            port("right", providers::u64_type()),
        ],
        vec![
            crossing("above", CrossDirection::Above),
            crossing("below", CrossDirection::Below),
        ],
        vec![
            ("above".into(), Expr::Cross("above".into())),
            ("below".into(), Expr::Cross("below".into())),
        ],
    );
    let mut r = instance(p);
    for (i, left, above, below) in [
        (1, 3, false, false),
        (2, 2, false, false),
        (3, 3, true, false),
        (4, 3, false, false),
        (5, 2, false, false),
        (6, 1, false, true),
    ] {
        let out = r.evaluate(i, inputs(left, 2)).unwrap();
        assert_eq!(read_bool(&out.outputs["above"]), above);
        assert_eq!(read_bool(&out.outputs["below"]), below);
    }
}
#[test]
fn nonready_resets_committed_cross_history_but_sequence_gaps_alone_do_not() {
    let p = cross_plan(Expr::Cross("cross".into()));
    let mut r = instance(p);
    assert!(!read_bool(
        &r.evaluate(1, inputs(1, 2)).unwrap().outputs["value"]
    ));
    assert!(read_bool(
        &r.evaluate(3, inputs(3, 2)).unwrap().outputs["value"]
    ));
    for (seq, state) in [(4, Evaluation::Missing), (6, Evaluation::Warmup)] {
        let v = r
            .evaluate(
                seq,
                vec![("left".into(), state.clone()), ("right".into(), ready(2))],
            )
            .unwrap();
        assert_eq!(v.outputs["value"], state);
        assert!(!read_bool(
            &r.evaluate(seq + 1, inputs(3, 2)).unwrap().outputs["value"]
        ));
    }
}
#[test]
fn failed_root_and_subscribed_numeric_errors_roll_back_history_and_cursor() {
    let root = bin(
        BinaryOp::And,
        Expr::Cross("cross".into()),
        bin(
            BinaryOp::Greater,
            bin(
                BinaryOp::Divide(Rounding::Exact),
                lit(2),
                Expr::Input("denominator".into()),
            ),
            lit(0),
        ),
    );
    let p = plan(
        vec![
            port("left", providers::u64_type()),
            port("right", providers::u64_type()),
            port("denominator", providers::u64_type()),
        ],
        vec![crossing("cross", CrossDirection::Above)],
        vec![("value".into(), root)],
    );
    let mut r = instance(p);
    let mut a = inputs(1, 2);
    a.push(("denominator".into(), ready(1)));
    r.evaluate(1, a).unwrap();
    let mut bad = inputs(3, 2);
    bad.push(("denominator".into(), ready(0)));
    assert!(r.evaluate(2, bad).is_err());
    assert_eq!((r.last_sequence(), r.accepted_steps()), (Some(1), 1));
    let mut good = inputs(3, 2);
    good.push(("denominator".into(), ready(1)));
    assert!(read_bool(&r.evaluate(2, good).unwrap().outputs["value"]));
    let mut r = instance(cross_plan(Expr::Literal(b(false))));
    r.evaluate(1, inputs(1, 2)).unwrap();
    assert!(
        r.evaluate(
            2,
            vec![
                ("left".into(), Evaluation::Error(NumericError::Overflow)),
                ("right".into(), ready(2))
            ]
        )
        .is_err()
    );
    assert_eq!(r.last_sequence(), Some(1));
    r.evaluate(2, inputs(3, 2)).unwrap();
}
#[test]
fn pure_arithmetic_in_decisively_skipped_branches_is_not_executed() {
    let bad = bin(
        BinaryOp::Greater,
        bin(BinaryOp::Divide(Rounding::Exact), lit(1), lit(0)),
        lit(0),
    );
    assert!(!read_bool(
        &evaluate(bin(BinaryOp::And, Expr::Literal(b(false)), bad.clone())).unwrap()
    ));
    assert!(read_bool(
        &evaluate(bin(BinaryOp::Or, Expr::Literal(b(true)), bad.clone())).unwrap()
    ));
    assert!(evaluate(bin(BinaryOp::And, bad, Expr::Literal(b(false)))).is_err());
}
#[test]
fn independently_registered_predicate_and_multi_input_function_compose_generically() {
    let r = providers::registry().unwrap();
    let pair = Arc::new(
        r.prepare(&providers::pair_descriptor().requirement(), [])
            .unwrap(),
    );
    let predicate = Arc::new(
        r.prepare(
            &providers::predicate_descriptor().requirement(),
            [("threshold".into(), ParameterInput::Value(ready(150)))],
        )
        .unwrap(),
    );
    let sum = Expr::Call {
        call: pair,
        inputs: vec![("right".into(), lit(75)), ("left".into(), lit(125))],
        output: "sum".into(),
    };
    let above = Expr::Call {
        call: predicate,
        inputs: vec![("value".into(), sum)],
        output: "result".into(),
    };
    assert!(read_bool(
        &evaluate(bin(BinaryOp::And, Expr::Literal(b(true)), above.clone())).unwrap()
    ));
    assert!(!read_bool(&evaluate(Expr::Not(Box::new(above))).unwrap()));
}
#[test]
fn shared_subscriptions_advance_pure_operand_once_per_step_and_instances_are_isolated() {
    struct Count(Arc<AtomicUsize>);
    impl Provider for Count {
        fn invoke(&self, c: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(vec![(
                "result".into(),
                Evaluation::Ready(c.input("value").unwrap().clone()),
            )])
        }
    }
    let count = Arc::new(AtomicUsize::new(0));
    let mut d = DescriptorDraft::pure(
        EntryKey {
            id: "test.counter".into(),
            version: "1".into(),
        },
        ContentId::from_bytes([4; 32]),
        ParameterSchema::new(1, vec![]).unwrap(),
        "Reviewed identity with test-only invocation counter",
    );
    d.inputs
        .push(CallablePort::one("value", providers::u64_type()));
    d.outputs
        .push(CallablePort::one("result", providers::u64_type()));
    let d = Descriptor::new(d).unwrap();
    let pin = d.requirement();
    let mut builder = RegistryBuilder::new();
    builder.register(d, Arc::new(Count(count.clone()))).unwrap();
    let registry = builder.freeze().unwrap();
    let call = Arc::new(registry.prepare(&pin, []).unwrap());
    let c = Crossing {
        id: "cross".into(),
        direction: CrossDirection::Above,
        left: Expr::Call {
            call,
            inputs: vec![("value".into(), Expr::Input("left".into()))],
            output: "result".into(),
        },
        right: Expr::Input("right".into()),
    };
    let p = plan(
        vec![
            port("left", providers::u64_type()),
            port("right", providers::u64_type()),
        ],
        vec![c],
        vec![(
            "value".into(),
            bin(
                BinaryOp::And,
                Expr::Cross("cross".into()),
                Expr::Cross("cross".into()),
            ),
        )],
    );
    let mut a = instance(p.clone());
    let mut b = instance(p);
    a.evaluate(1, inputs(1, 2)).unwrap();
    assert!(read_bool(
        &a.evaluate(2, inputs(3, 2)).unwrap().outputs["value"]
    ));
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert!(!read_bool(
        &b.evaluate(1, inputs(3, 2)).unwrap().outputs["value"]
    ));
    assert_eq!(count.load(Ordering::SeqCst), 3);
}
#[test]
fn bounded_plan_references_duplicates_and_call_signatures_are_checked() {
    let base = ExpressionDraft {
        inputs: vec![],
        crossings: vec![],
        outputs: vec![("value".into(), lit(1))],
    };
    let mut d = base.clone();
    d.outputs.push(d.outputs[0].clone());
    assert!(ExpressionPlan::new(d).is_err());
    let mut d = base.clone();
    d.outputs[0].1 = Expr::Cross("missing".into());
    assert!(ExpressionPlan::new(d).is_err());
    let mut d = base.clone();
    d.outputs[0].1 = Expr::Input("missing".into());
    assert!(ExpressionPlan::new(d).is_err());
    let mut e = Expr::Literal(b(true));
    for _ in 0..34 {
        e = Expr::Not(Box::new(e));
    }
    let mut d = base.clone();
    d.outputs[0].1 = e;
    assert_eq!(
        ExpressionPlan::new(d).err().unwrap().code,
        "expression_limit"
    );
    let r = providers::registry().unwrap();
    let c = Arc::new(
        r.prepare(&providers::pair_descriptor().requirement(), [])
            .unwrap(),
    );
    for args in [
        vec![],
        vec![("left".into(), lit(1)), ("left".into(), lit(2))],
        vec![
            ("left".into(), Expr::Literal(b(true))),
            ("right".into(), lit(2)),
        ],
    ] {
        let mut d = base.clone();
        d.outputs[0].1 = Expr::Call {
            call: c.clone(),
            inputs: args,
            output: "sum".into(),
        };
        assert!(ExpressionPlan::new(d).is_err());
    }
}
#[test]
fn invalid_inputs_and_repeated_sequences_do_not_consume_a_step() {
    let p = cross_plan(Expr::Cross("cross".into()));
    let mut r = instance(p);
    assert!(r.evaluate(0, inputs(1, 2)).is_err());
    assert!(r.evaluate(1, vec![]).is_err());
    assert!(
        r.evaluate(
            1,
            vec![("left".into(), ready(1)), ("left".into(), ready(2))]
        )
        .is_err()
    );
    assert!(
        r.evaluate(
            1,
            vec![
                ("left".into(), Evaluation::Ready(b(true))),
                ("right".into(), ready(2))
            ]
        )
        .is_err()
    );
    assert_eq!(r.last_sequence(), None);
    r.evaluate(1, inputs(1, 2)).unwrap();
    assert!(r.evaluate(1, inputs(3, 2)).is_err());
    assert!(read_bool(
        &r.evaluate(2, inputs(3, 2)).unwrap().outputs["value"]
    ));
}

#[test]
fn ready_boolean_operand_reordering_preserves_cross_history() {
    let make = |reverse| {
        let guard = Expr::Input("guard".into());
        let cross = Expr::Cross("cross".into());
        let e = if reverse {
            bin(BinaryOp::And, cross, guard)
        } else {
            bin(BinaryOp::And, guard, cross)
        };
        instance(plan(
            vec![
                port("left", providers::u64_type()),
                port("right", providers::u64_type()),
                port("guard", bool_t()),
            ],
            vec![crossing("cross", CrossDirection::Above)],
            vec![("value".into(), e)],
        ))
    };
    let mut a = make(false);
    let mut c = make(true);
    for (seq, left, guard) in [
        (1, 1, false),
        (2, 3, false),
        (3, 4, true),
        (4, 2, false),
        (5, 3, true),
    ] {
        let mut v = inputs(left, 2);
        v.push(("guard".into(), Evaluation::Ready(b(guard))));
        assert_eq!(
            a.evaluate(seq, v.clone()).unwrap(),
            c.evaluate(seq, v).unwrap()
        );
    }
}
#[test]
fn crossing_dependencies_use_stable_order_and_reject_pure_cycles() {
    struct BoolNumber;
    impl Provider for BoolNumber {
        fn invoke(&self, c: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
            let ScalarValue::Bool(v) = c.input("value").unwrap().value() else {
                return Err(ProviderFailure::Failed);
            };
            Ok(vec![("result".into(), ready(u64::from(*v)))])
        }
    }
    let mut d = DescriptorDraft::pure(
        EntryKey {
            id: "test.bool_number".into(),
            version: "1".into(),
        },
        ContentId::from_bytes([5; 32]),
        ParameterSchema::new(1, vec![]).unwrap(),
        "Explicit fixture Boolean to u64 conversion",
    );
    d.inputs.push(CallablePort::one("value", bool_t()));
    d.outputs
        .push(CallablePort::one("result", providers::u64_type()));
    let d = Descriptor::new(d).unwrap();
    let pin = d.requirement();
    let mut builder = RegistryBuilder::new();
    builder.register(d, Arc::new(BoolNumber)).unwrap();
    let r = builder.freeze().unwrap();
    let call = Arc::new(r.prepare(&pin, []).unwrap());
    let parent = |reference: &str| Crossing {
        id: "a_parent".into(),
        direction: CrossDirection::Above,
        left: Expr::Call {
            call: call.clone(),
            inputs: vec![("value".into(), Expr::Cross(reference.into()))],
            output: "result".into(),
        },
        right: lit(0),
    };
    let mut d = ExpressionDraft {
        inputs: vec![
            port("left", providers::u64_type()),
            port("right", providers::u64_type()),
        ],
        crossings: vec![
            parent("z_child"),
            crossing("z_child", CrossDirection::Above),
        ],
        outputs: vec![("value".into(), Expr::Cross("a_parent".into()))],
    };
    let p = Arc::new(ExpressionPlan::new(d.clone()).unwrap());
    assert_eq!(
        p.crossing_order(),
        &["z_child".to_string(), "a_parent".to_string()]
    );
    let mut run = instance(p);
    assert!(!read_bool(
        &run.evaluate(1, inputs(1, 2)).unwrap().outputs["value"]
    ));
    assert!(read_bool(
        &run.evaluate(2, inputs(3, 2)).unwrap().outputs["value"]
    ));
    d.crossings[0] = parent("a_parent");
    assert_eq!(
        ExpressionPlan::new(d).err().unwrap().code,
        "expression_cycle"
    );
}
#[test]
fn total_node_text_and_collection_budgets_are_enforced() {
    fn tree(depth: u32) -> Expr {
        if depth == 0 {
            Expr::Literal(b(true))
        } else {
            bin(BinaryOp::And, tree(depth - 1), tree(depth - 1))
        }
    }
    let make = |e| ExpressionDraft {
        inputs: vec![],
        crossings: vec![],
        outputs: vec![("value".into(), e)],
    };
    assert_eq!(
        ExpressionPlan::new(make(tree(12))).err().unwrap().code,
        "expression_limit"
    );
    let text = || {
        Expr::Literal(
            TypedValue::new(
                t(ScalarType::Text { max_bytes: 4096 }),
                ScalarValue::Text("x".repeat(4096)),
            )
            .unwrap(),
        )
    };
    let mut d = ExpressionDraft {
        inputs: vec![],
        crossings: vec![],
        outputs: (0..32)
            .map(|i| (format!("o{i}"), bin(BinaryOp::Equal, text(), text())))
            .collect(),
    };
    ExpressionPlan::new(d.clone()).unwrap();
    d.outputs[0].1 = bin(
        BinaryOp::And,
        d.outputs[0].1.clone(),
        bin(BinaryOp::Equal, text(), text()),
    );
    assert_eq!(
        ExpressionPlan::new(d).err().unwrap().code,
        "expression_limit"
    );
    let mut d = make(Expr::Cross("c0".into()));
    d.crossings = (0..257)
        .map(|i| Crossing {
            id: format!("c{i}"),
            direction: CrossDirection::Above,
            left: lit(0),
            right: lit(1),
        })
        .collect();
    assert_eq!(
        ExpressionPlan::new(d).err().unwrap().code,
        "expression_limit"
    );
}

#[test]
fn observers_read_existing_calls_and_crossings_without_changing_call_count_or_results() {
    struct Count(Arc<AtomicUsize>);
    impl Provider for Count {
        fn invoke(&self, c: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(vec![(
                "result".into(),
                Evaluation::Ready(c.input("value").unwrap().clone()),
            )])
        }
    }
    let count = Arc::new(AtomicUsize::new(0));
    let mut draft = DescriptorDraft::pure(
        EntryKey {
            id: "test.trace.identity".into(),
            version: "1".into(),
        },
        ContentId::from_bytes([9; 32]),
        ParameterSchema::new(1, vec![]).unwrap(),
        "Observed test identity",
    );
    draft
        .inputs
        .push(CallablePort::one("value", providers::u64_type()));
    draft
        .outputs
        .push(CallablePort::one("result", providers::u64_type()));
    let descriptor = Descriptor::new(draft).unwrap();
    let pin = descriptor.requirement();
    let mut builder = RegistryBuilder::new();
    builder
        .register(descriptor, Arc::new(Count(Arc::clone(&count))))
        .unwrap();
    let registry = builder.freeze().unwrap();
    let call = Arc::new(registry.prepare(&pin, []).unwrap());
    let p = plan(
        vec![],
        vec![Crossing {
            id: "rise".into(),
            direction: CrossDirection::Above,
            left: Expr::Call {
                call,
                inputs: vec![("value".into(), lit(3))],
                output: "result".into(),
            },
            right: lit(2),
        }],
        vec![("decision".into(), Expr::Cross("rise".into()))],
    );
    let mut plain = instance(Arc::clone(&p));
    let mut observed = instance(p);
    for seq in 1..=3 {
        let expected = plain.evaluate(seq, vec![]).unwrap();
        let before = count.load(Ordering::SeqCst);
        let mut calls = 0;
        let mut crosses = 0;
        let actual = observed
            .evaluate_observed(seq, vec![], &mut |observation| match observation {
                ExpressionObservation::Call {
                    call_sequence,
                    outputs,
                    ..
                } => {
                    calls += 1;
                    assert_eq!(call_sequence, 1);
                    assert_eq!(outputs["result"], ready(3));
                }
                ExpressionObservation::Crossing {
                    previous,
                    current,
                    output,
                    ..
                } => {
                    crosses += 1;
                    assert_eq!(previous.is_some(), seq > 1);
                    assert!(current.is_some());
                    assert!(!read_bool(output));
                }
            })
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(calls, 1);
        assert_eq!(crosses, 1);
        assert_eq!(count.load(Ordering::SeqCst), before + 1);
    }
}
