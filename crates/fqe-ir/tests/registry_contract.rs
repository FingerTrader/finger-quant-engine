#[path = "../examples/support/providers.rs"]
mod providers;
use fqe_core::{
    InstrumentId,
    numeric::{Decimal, NumericError},
    types::{
        Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
    },
};
use fqe_ir::{
    builtins,
    loading::LoadedModel,
    model::{ContentId, EntryKey, ModelError},
    parameters::{Constraints, ParameterInput, ParameterSchema, ParameterSpec},
    registry::*,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn context(n: u64) -> CallContext {
    CallContext {
        run: "test.run".into(),
        instance: "test.instance".into(),
        event_sequence: 1,
        call_sequence: n,
    }
}
fn ready(n: u64) -> Evaluation<TypedValue> {
    Evaluation::Ready(providers::value(n))
}
fn input(n: u64) -> ParameterInput {
    ParameterInput::Value(ready(n))
}
fn empty() -> ParameterSchema {
    ParameterSchema::new(1, vec![]).unwrap()
}
fn draft() -> DescriptorDraft {
    let mut d = DescriptorDraft::pure(
        EntryKey {
            id: "test.echo".into(),
            version: "1".into(),
        },
        ContentId::from_bytes([1; 32]),
        empty(),
        "Return exact value unchanged",
    );
    d.inputs
        .push(CallablePort::one("value", providers::u64_type()));
    d.outputs
        .push(CallablePort::one("result", providers::u64_type()));
    d
}
struct Echo {
    calls: Arc<AtomicUsize>,
}
impl Provider for Echo {
    fn invoke(&self, c: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![(
            "result".into(),
            Evaluation::Ready(c.input("value").ok_or(ProviderFailure::Failed)?.clone()),
        )])
    }
}
fn registry(d: DescriptorDraft, p: Arc<dyn Provider>) -> Registry {
    let mut b = RegistryBuilder::new();
    b.register(Descriptor::new(d).unwrap(), p).unwrap();
    b.freeze().unwrap()
}
fn descriptor_error(d: DescriptorDraft) -> ModelError {
    Descriptor::new(d).err().unwrap()
}

#[test]
fn independently_encoded_descriptor_bytes_and_digest_match() {
    let d = providers::pair_descriptor();
    assert_eq!(
        d.canonical_bytes(),
        include_bytes!("fixtures/x001-descriptor.canonical.json")
    );
    assert_eq!(
        d.content_id().to_string(),
        include_str!("fixtures/x001-descriptor.identity.txt").trim()
    );
}
#[test]
fn discovery_and_registration_do_not_execute_provider_code() {
    struct Forbidden;
    impl Provider for Forbidden {
        fn validate_parameters(
            &self,
            _: &fqe_ir::parameters::BoundParameters,
        ) -> Result<(), ProviderFailure> {
            panic!("validation executed")
        }
        fn invoke(&self, _: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
            panic!("provider executed")
        }
    }
    let descriptor = Descriptor::new(draft()).unwrap();
    let pin = descriptor.requirement();
    let mut b = RegistryBuilder::new();
    b.register(descriptor, Arc::new(Forbidden)).unwrap();
    let r = b.freeze().unwrap();
    assert_eq!(r.descriptors().count(), 1);
    r.resolve(&pin).unwrap();
    assert_eq!(
        r.prepare(&pin, []).err().unwrap().code,
        "registry_provider_panic"
    );
}
#[test]
fn descriptor_normalization_and_registry_identity_are_order_independent() {
    let a = providers::pair_descriptor();
    let mut b = a.declaration().clone();
    b.inputs.reverse();
    b.outputs.reverse();
    let b = Descriptor::new(b).unwrap();
    assert_eq!(a, b);
    let other = providers::predicate_descriptor();
    let mut x = RegistryBuilder::new();
    x.register(a.clone(), Arc::new(providers::Pair)).unwrap();
    x.register(other.clone(), Arc::new(providers::Above))
        .unwrap();
    let x = x.freeze().unwrap();
    let mut y = RegistryBuilder::new();
    y.register(other, Arc::new(providers::Above)).unwrap();
    y.register(b, Arc::new(providers::Pair)).unwrap();
    let y = y.freeze().unwrap();
    assert_eq!(x.content_id(), y.content_id());
    assert_eq!(x.canonical_bytes(), y.canonical_bytes());
    let mut changed = a.declaration().clone();
    changed.metadata.insert("label".into(), "changed".into());
    let changed = Descriptor::new(changed).unwrap();
    assert_ne!(a.content_id(), changed.content_id());
    let mut z = RegistryBuilder::new();
    z.register(changed, Arc::new(providers::Pair)).unwrap();
    assert_ne!(x.content_id(), z.freeze().unwrap().content_id());
}
#[test]
fn exact_version_artifact_role_and_duplicate_pins_are_checked() {
    let d = Descriptor::new(draft()).unwrap();
    let pin = d.requirement();
    let p = Arc::new(Echo {
        calls: Arc::new(AtomicUsize::new(0)),
    });
    let mut b = RegistryBuilder::new();
    b.register(d.clone(), p.clone()).unwrap();
    assert_eq!(
        b.register(d, p.clone()).unwrap_err().code,
        "registry_duplicate"
    );
    let mut version2 = draft();
    version2.key.version = "2".into();
    b.register(Descriptor::new(version2).unwrap(), p).unwrap();
    let r = b.freeze().unwrap();
    r.resolve(&pin).unwrap();
    for case in 0..5 {
        let mut p = pin.clone();
        match case {
            0 => p.key.id = "unknown.entry".into(),
            1 => p.key.version = "3".into(),
            2 => p.artifact = ContentId::from_bytes([2; 32]),
            3 => p.role = "adapter".into(),
            _ => p.descriptor_version = 2,
        }
        assert!(r.resolve(&p).is_err());
    }
}
#[test]
fn descriptor_versions_capabilities_state_roles_cardinality_and_bounds_fail_closed() {
    for case in 0..20 {
        let mut d = draft();
        match case {
            0 => d.descriptor_version = 2,
            1 => d.compatibility.engine_api = 2,
            2 => d.compatibility.provider_contract = 2,
            3 => d.compatibility.ir_max = 1,
            4 => d.compatibility.ir_min = 3,
            5 => d.required_capabilities.push("network/1".into()),
            6 => d
                .required_capabilities
                .push(d.required_capabilities[0].clone()),
            7 => d.purity = Purity::Stateful,
            8 => d.readiness = ReadinessPolicy::ProviderManaged,
            9 => d.history = 1,
            10 => d.state_schema = Some("v1".into()),
            11 => d.host_services.push("account".into()),
            12 => d.dependencies.push(d.key.clone()),
            13 => d.deterministic = false,
            14 => d.role = "runner".into(),
            15 => d.inputs[0].cardinality = Cardinality::Optional,
            16 => d.inputs[0].cardinality = Cardinality::Many { max: 2 },
            17 => d.resources.max_input_bytes = 0,
            18 => d.resources.max_output_bytes = 262145,
            _ => d.key.id = "unqualified".into(),
        }
        assert!(!descriptor_error(d).code.is_empty());
    }
    let mut d = draft();
    d.outputs.clear();
    assert_eq!(descriptor_error(d).code, "registry_arity");
    let mut d = draft();
    d.inputs.push(d.inputs[0].clone());
    assert_eq!(descriptor_error(d).code, "registry_duplicate");
    let mut d = draft();
    d.category = Category::Predicate;
    assert_eq!(descriptor_error(d).code, "registry_type");
    let mut d = draft();
    d.category = Category::Indicator;
    assert_eq!(descriptor_error(d).code, "registry_unsupported");
    for shape in [
        ValueShape::Series,
        ValueShape::Window,
        ValueShape::Record,
        ValueShape::Collection,
        ValueShape::Unsupported("future".into()),
    ] {
        assert!(TypeDescriptor::new(1, shape, ScalarType::U64).is_err());
    }
}
#[test]
fn missing_warmup_and_numeric_errors_do_not_call_provider() {
    let calls = Arc::new(AtomicUsize::new(0));
    let r = registry(
        draft(),
        Arc::new(Echo {
            calls: calls.clone(),
        }),
    );
    let p = r.descriptors().next().unwrap().requirement();
    let c = r.prepare(&p, []).unwrap();
    for value in [Evaluation::Missing, Evaluation::Warmup] {
        let out = c
            .invoke(context(1), vec![("value".into(), value.clone())])
            .unwrap();
        assert_eq!(out.outputs()["result"], value);
        assert_eq!(out.context(), &context(1));
        assert_eq!(out.registry_id(), r.content_id());
        assert_eq!(out.descriptor_id(), c.descriptor().content_id());
    }
    assert_eq!(
        c.invoke(
            context(1),
            vec![("value".into(), Evaluation::Error(NumericError::Overflow))]
        )
        .unwrap_err()
        .code,
        "registry_numeric"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let out = c
        .invoke(context(2), vec![("value".into(), ready(u64::MAX))])
        .unwrap();
    assert_eq!(out.outputs()["result"], ready(u64::MAX));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[test]
fn arity_names_types_units_scale_and_payloads_fail_before_provider() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut d = draft();
    let t = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 2,
            unit: NumericUnit::Price(InstrumentId::new("A").unwrap()),
        },
    )
    .unwrap();
    d.inputs[0].value_type = t.clone();
    d.outputs[0].value_type = t.clone();
    let r = registry(
        d,
        Arc::new(Echo {
            calls: calls.clone(),
        }),
    );
    let p = r.descriptors().next().unwrap().requirement();
    let c = r.prepare(&p, []).unwrap();
    for inputs in [
        vec![],
        vec![("extra".into(), ready(1))],
        vec![("value".into(), ready(1))],
        vec![
            ("value".into(), Evaluation::Missing),
            ("value".into(), Evaluation::Missing),
        ],
    ] {
        assert!(c.invoke(context(1), inputs).is_err());
    }
    for (scale, instrument) in [(2, "B"), (3, "A")] {
        let t = TypeDescriptor::new(
            1,
            ValueShape::Scalar,
            ScalarType::Decimal {
                scale,
                unit: NumericUnit::Price(InstrumentId::new(instrument).unwrap()),
            },
        )
        .unwrap();
        let v =
            TypedValue::new(t, ScalarValue::Decimal(Decimal::new(123, scale).unwrap())).unwrap();
        assert_eq!(
            c.invoke(context(1), vec![("value".into(), Evaluation::Ready(v))])
                .unwrap_err()
                .code,
            "registry_type"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut d = draft();
    d.resources.max_input_bytes = 7;
    let r = registry(
        d,
        Arc::new(Echo {
            calls: calls.clone(),
        }),
    );
    assert_eq!(
        r.prepare(&r.descriptors().next().unwrap().requirement(), [])
            .unwrap()
            .invoke(context(1), vec![("value".into(), ready(1))])
            .unwrap_err()
            .code,
        "registry_limit"
    );
    let mut ctx = context(1);
    ctx.call_sequence = 0;
    assert!(
        c.invoke(ctx, vec![("value".into(), Evaluation::Missing)])
            .is_err()
    );
}

#[test]
fn provider_wrong_outputs_panics_and_failures_never_publish_partial_results() {
    struct Bad(u8);
    impl Provider for Bad {
        fn invoke(&self, _: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
            Ok(match self.0 {
                0 => vec![],
                1 => vec![("unknown".into(), ready(1))],
                2 => vec![(
                    "result".into(),
                    Evaluation::Ready(
                        TypedValue::new(
                            TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Bool).unwrap(),
                            ScalarValue::Bool(true),
                        )
                        .unwrap(),
                    ),
                )],
                3 => vec![("result".into(), Evaluation::Warmup)],
                4 => vec![("result".into(), Evaluation::Error(NumericError::Inexact))],
                5 => panic!("fixture panic"),
                6 => return Err(ProviderFailure::Failed),
                _ => vec![("result".into(), ready(1)), ("result".into(), ready(2))],
            })
        }
    }
    for mode in 0..8 {
        let r = registry(draft(), Arc::new(Bad(mode)));
        let p = r
            .prepare(&r.descriptors().next().unwrap().requirement(), [])
            .unwrap();
        assert!(
            p.invoke(context(1), vec![("value".into(), ready(1))])
                .is_err()
        );
    }
    let mut d = draft();
    d.resources.max_output_bytes = 7;
    let r = registry(
        d,
        Arc::new(Echo {
            calls: Arc::new(AtomicUsize::new(0)),
        }),
    );
    assert_eq!(
        r.prepare(&r.descriptors().next().unwrap().requirement(), [])
            .unwrap()
            .invoke(context(1), vec![("value".into(), ready(1))])
            .unwrap_err()
            .code,
        "registry_limit"
    );
}
#[test]
fn parameters_are_bound_once_with_schema_and_provider_relationship_validation() {
    let r = providers::registry().unwrap();
    let pin = providers::predicate_descriptor().requirement();
    for args in [
        vec![],
        vec![("threshold".into(), ParameterInput::Null)],
        vec![("threshold".into(), input(1000))],
        vec![("threshold".into(), input(1)), ("unknown".into(), input(2))],
        vec![
            ("threshold".into(), input(1)),
            ("threshold".into(), input(2)),
        ],
    ] {
        assert!(r.prepare(&pin, args).is_err());
    }
    let a = r.prepare(&pin, [("threshold".into(), input(150))]).unwrap();
    let b = r.prepare(&pin, [("threshold".into(), input(250))]).unwrap();
    assert_eq!(
        a.invoke(context(1), vec![("value".into(), ready(200))])
            .unwrap()
            .outputs()["result"],
        Evaluation::Ready(
            TypedValue::new(
                TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Bool).unwrap(),
                ScalarValue::Bool(true)
            )
            .unwrap()
        )
    );
    assert_ne!(a.parameters(), b.parameters());
    let mut d = draft();
    d.parameters = ParameterSchema::new(
        1,
        vec![
            ParameterSpec::new(
                "x",
                providers::u64_type(),
                true,
                None,
                Constraints::default(),
                "x",
            )
            .unwrap(),
        ],
    )
    .unwrap();
    d.resources.max_input_bytes = 7;
    let r = registry(
        d,
        Arc::new(Echo {
            calls: Arc::new(AtomicUsize::new(0)),
        }),
    );
    assert_eq!(
        r.prepare(
            &r.descriptors().next().unwrap().requirement(),
            [("x".into(), input(1))]
        )
        .err()
        .unwrap()
        .code,
        "registry_limit"
    );
}
fn fixture() -> Value {
    serde_json::from_slice(include_bytes!("fixtures/x001-sum.json")).unwrap()
}
fn load(v: &Value) -> LoadedModel {
    LoadedModel::from_bytes(&serde_json::to_vec(v).unwrap()).unwrap()
}
#[test]
fn loaded_model_preparation_checks_signature_edges_parameters_and_implementation_pins() {
    let r = providers::registry().unwrap();
    let model = load(&fixture());
    let binding = model.bind_json(br#"{"version":1,"values":{}}"#).unwrap();
    r.validate_requirements(&model).unwrap();
    let prepared = r.prepare_node(&model, &binding, "left").unwrap();
    assert_eq!(prepared.model_pin().unwrap().model, model.content_id());
    assert_eq!(
        prepared
            .invoke(context(1), vec![])
            .unwrap()
            .model_pin()
            .unwrap()
            .node,
        "left"
    );
    for id in ["left", "right", "pair", "predicate", "not"] {
        r.prepare_node(&model, &binding, id).unwrap();
    }
    for (path, value) in [
        (
            "/entries/2/artifact",
            json!("sha256:".to_owned() + &"ff".repeat(32)),
        ),
        ("/entries/2/key/version", json!("2")),
        ("/graph/nodes/2/outputs/0/type/kind", json!("bool")),
        ("/graph/nodes/2/inputs/0/name", json!("wrong")),
        ("/graph/nodes/2/outputs/0/name", json!("wrong")),
    ] {
        let mut f = fixture();
        *f.pointer_mut(path).unwrap() = value;
        if path == "/entries/2/key/version" {
            f["graph"]["nodes"][2]["entry"]["version"] = json!("2");
        }
        if path == "/graph/nodes/2/outputs/0/name" {
            f["graph"]["nodes"][3]["inputs"][0]["source"]["port"] = json!("wrong");
        }
        let m = load(&f);
        let b = m.bind_json(br#"{"version":1,"values":{}}"#).unwrap();
        assert!(r.prepare_node(&m, &b, "pair").is_err(), "{path}");
    }
    let mut f = fixture();
    f["graph"]["nodes"][0]["outputs"][0]["type"]["kind"] = json!("bool");
    let m = load(&f);
    let b = m.bind_json(br#"{"version":1,"values":{}}"#).unwrap();
    assert_eq!(
        r.prepare_node(&m, &b, "pair").err().unwrap().code,
        "registry_type"
    );
    let mut f = fixture();
    f["metadata"]["label"] = json!("modified same author version");
    let changed = load(&f);
    assert_eq!(
        r.prepare_node(&changed, &binding, "pair")
            .err()
            .unwrap()
            .code,
        "content_mismatch"
    );
    let mut f = fixture();
    f["graph"]["nodes"][3]["arguments"]["bad"] =
        json!({"literal":{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"1"}});
    let m = load(&f);
    let b = m.bind_json(br#"{"version":1,"values":{}}"#).unwrap();
    assert_eq!(
        r.prepare_node(&m, &b, "predicate").err().unwrap().code,
        "registry_parameter"
    );
}
fn composition(bytes: &[u8]) -> (u64, u64, bool, bool) {
    let r = providers::registry().unwrap();
    let m = LoadedModel::from_bytes(bytes).unwrap();
    let b = m.bind_json(br#"{"version":1,"values":{}}"#).unwrap();
    let left = r
        .prepare_node(&m, &b, "left")
        .unwrap()
        .invoke(context(1), vec![])
        .unwrap();
    let right = r
        .prepare_node(&m, &b, "right")
        .unwrap()
        .invoke(context(2), vec![])
        .unwrap();
    let pair = r
        .prepare_node(&m, &b, "pair")
        .unwrap()
        .invoke(
            context(3),
            vec![
                ("right".into(), right.outputs()["result"].clone()),
                ("left".into(), left.outputs()["result"].clone()),
            ],
        )
        .unwrap();
    let port = &m
        .definition()
        .declaration()
        .nodes
        .iter()
        .find(|n| n.id == "predicate")
        .unwrap()
        .inputs[0]
        .source
        .port;
    let p = r
        .prepare_node(&m, &b, "predicate")
        .unwrap()
        .invoke(
            context(4),
            vec![("value".into(), pair.outputs()[port].clone())],
        )
        .unwrap();
    let n = r
        .prepare_node(&m, &b, "not")
        .unwrap()
        .invoke(
            context(5),
            vec![("value".into(), p.outputs()["result"].clone())],
        )
        .unwrap();
    let u = |v: &Evaluation<TypedValue>| match v {
        Evaluation::Ready(v) => match v.value() {
            ScalarValue::U64(n) => *n,
            _ => panic!(),
        },
        _ => panic!(),
    };
    let boolean = |v: &Evaluation<TypedValue>| match v {
        Evaluation::Ready(v) => match v.value() {
            ScalarValue::Bool(n) => *n,
            _ => panic!(),
        },
        _ => panic!(),
    };
    (
        u(&pair.outputs()["sum"]),
        u(&pair.outputs()["difference"]),
        boolean(&p.outputs()["result"]),
        boolean(&n.outputs()["result"]),
    )
}
#[test]
fn two_independent_unknown_name_compositions_use_the_shared_builtin_call_interface() {
    // Independent expected arithmetic/strict decisions: 125+75=200,125-75=50,
    // 200>150=true,50>150=false. Caller drives sequence; no compiler/runtime claim.
    assert_eq!(
        composition(include_bytes!("fixtures/x001-sum.json")),
        (200, 50, true, false)
    );
    assert_eq!(
        composition(include_bytes!("fixtures/x001-difference.json")),
        (200, 50, false, true)
    );
    let r = providers::registry().unwrap();
    let p = r
        .prepare(&providers::pair_descriptor().requirement(), [])
        .unwrap();
    assert_eq!(
        p.invoke(
            context(1),
            vec![("left".into(), ready(u64::MAX)), ("right".into(), ready(1))]
        )
        .unwrap_err()
        .code,
        "registry_numeric"
    );
}
#[test]
fn readiness_precedence_and_multi_output_duplicates_are_deterministic() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut d = draft();
    d.inputs
        .push(CallablePort::one("other", providers::u64_type()));
    d.outputs
        .push(CallablePort::one("second", providers::u64_type()));
    let r = registry(
        d,
        Arc::new(Echo {
            calls: calls.clone(),
        }),
    );
    let p = r
        .prepare(&r.descriptors().next().unwrap().requirement(), [])
        .unwrap();
    let a = p
        .invoke(
            context(1),
            vec![
                ("value".into(), Evaluation::Warmup),
                ("other".into(), Evaluation::Missing),
            ],
        )
        .unwrap();
    assert!(
        a.outputs()
            .values()
            .all(|v| matches!(v, Evaluation::Missing))
    );
    let b = p
        .invoke(
            context(1),
            vec![
                (
                    "other".into(),
                    Evaluation::Error(NumericError::DivisionByZero),
                ),
                ("value".into(), Evaluation::Missing),
            ],
        )
        .unwrap_err();
    assert_eq!(b.code, "registry_numeric");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    struct Duplicate;
    impl Provider for Duplicate {
        fn invoke(&self, _: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
            Ok(vec![
                ("result".into(), ready(1)),
                ("result".into(), ready(2)),
            ])
        }
    }
    let mut d = draft();
    d.outputs
        .push(CallablePort::one("second", providers::u64_type()));
    let r = registry(d, Arc::new(Duplicate));
    assert_eq!(
        r.prepare(&r.descriptors().next().unwrap().requirement(), [])
            .unwrap()
            .invoke(context(1), vec![("value".into(), ready(1))])
            .unwrap_err()
            .code,
        "registry_duplicate"
    );
}
#[test]
fn descriptor_and_registry_resource_limits_and_frozen_lifetime_are_explicit() {
    let mut d = draft();
    d.semantics = "x".repeat(4097);
    assert_eq!(descriptor_error(d).code, "registry_limit");
    let mut d = draft();
    d.metadata = BTreeMap::from([("label".into(), "x".repeat(4097))]);
    assert_eq!(descriptor_error(d).code, "registry_limit");
    let mut d = draft();
    d.inputs = (0..33)
        .map(|i| CallablePort::one(format!("p{i}"), providers::u64_type()))
        .collect();
    assert_eq!(descriptor_error(d).code, "registry_limit");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut b = RegistryBuilder::new();
    for i in 0..256 {
        let mut d = draft();
        d.key.id = format!("test.p{i}");
        b.register(
            Descriptor::new(d).unwrap(),
            Arc::new(Echo {
                calls: calls.clone(),
            }),
        )
        .unwrap();
    }
    let mut d = draft();
    d.key.id = "test.extra".into();
    assert_eq!(
        b.register(Descriptor::new(d).unwrap(), Arc::new(Echo { calls }))
            .unwrap_err()
            .code,
        "registry_limit"
    );
    let r = b.freeze().unwrap();
    assert_eq!(r.descriptors().count(), 256);
    let prepared = r
        .prepare(&r.descriptors().next().unwrap().requirement(), [])
        .unwrap();
    let pinned = prepared.registry_id();
    drop(r);
    assert_eq!(
        prepared
            .invoke(context(1), vec![("value".into(), ready(9))])
            .unwrap()
            .registry_id(),
        pinned
    );
    let r = builtins::registry().unwrap();
    assert_eq!(r.descriptors().count(), 3);
}
