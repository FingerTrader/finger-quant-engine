use fqe_core::types::{ScalarType, TypeDescriptor, ValueShape};
use fqe_ir::{
    event_plan::{LoadedEventPlan, MAX_SLOTS},
    indicators,
    registry::{Registry, RegistryBuilder},
};
use serde_json::{Value, json};
const TICK: &[u8] = include_bytes!("fixtures/tick-events.json");
const G002: &[u8] = include_bytes!("fixtures/g002-events.json");
fn registry() -> Registry {
    let mut b = RegistryBuilder::new();
    let t = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 12,
            unit: fqe_core::types::NumericUnit::Price(fqe_core::InstrumentId::new("DEMO").unwrap()),
        },
    )
    .unwrap();
    indicators::register(&mut b, "reference", t).unwrap();
    b.freeze().unwrap()
}
fn empty() -> Registry {
    RegistryBuilder::new().freeze().unwrap()
}
fn mutate(bytes: &[u8], f: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut v: Value = serde_json::from_slice(bytes).unwrap();
    f(&mut v);
    serde_json::to_vec(&v).unwrap()
}
fn compile_bad(bytes: &[u8], f: impl FnOnce(&mut Value)) -> String {
    let b = mutate(bytes, f);
    match LoadedEventPlan::from_bytes(&b) {
        Err(e) => e.code.into(),
        Ok(a) => match a.compile(&registry(), vec![]) {
            Err(e) => e.code.into(),
            Ok(_) => panic!("unexpected acceptance"),
        },
    }
}
#[test]
fn independent_canonical_bytes_and_hash_roundtrip() {
    let a = LoadedEventPlan::from_bytes(TICK).unwrap();
    assert_eq!(
        a.canonical_bytes(),
        include_bytes!("fixtures/tick-events.canonical.json")
    );
    assert_eq!(
        a.content_id().to_string(),
        include_str!("fixtures/tick-events.identity.txt").trim()
    );
    let b = LoadedEventPlan::from_bytes(a.canonical_bytes()).unwrap();
    assert_eq!(a.content_id(), b.content_id());
    assert_ne!(a.source_id(), b.source_id());
    assert_eq!(
        a.compile(&empty(), vec![]).unwrap().content_id(),
        b.compile(&empty(), vec![]).unwrap().content_id()
    );
}
#[test]
fn syntax_versions_capabilities_unknown_fields_and_canonical_numbers() {
    for f in [
        |v: &mut Value| v["schema_version"] = json!(2),
        |v: &mut Value| v["format"] = json!("wrong"),
        |v: &mut Value| v["requirements"] = json!(["event.plan/1"]),
        |v: &mut Value| {
            v["requirements"] = json!(["event.plan/1", "expression.scalar/1", "future/1"])
        },
        |v: &mut Value| v["unexpected"] = json!(true),
        |v: &mut Value| v["warmup_samples"] = json!(0),
        |v: &mut Value| v["warmup_samples"] = json!("00"),
        |v: &mut Value| v["trigger"]["calendar"] = json!("auto"),
    ] {
        assert!(LoadedEventPlan::from_bytes(&mutate(TICK, f)).is_err());
    }
    assert!(LoadedEventPlan::from_bytes(b"{\"format\":1,\"format\":2}").is_err());
    assert!(LoadedEventPlan::from_bytes(&vec![b' '; 262145]).is_err());
}
#[test]
fn exact_event_field_types_units_scales_and_domains() {
    for f in [
        |v: &mut Value| v["bindings"]["sample"] = json!("future_close"),
        |v: &mut Value| v["bindings"]["extra"] = json!("close"),
        |v: &mut Value| v["bindings"] = json!({}),
        |v: &mut Value| v["trigger"]["instrument"] = json!("OTHER"),
        |v: &mut Value| v["trigger"]["price_scale"] = json!(11),
        |v: &mut Value| v["trigger"]["price_scale"] = json!(13),
        |v: &mut Value| v["trigger"]["volume_scale"] = json!(19),
        |v: &mut Value| v["trigger"]["timeframe_ns"] = json!("0"),
        |v: &mut Value| v["trigger"]["timeframe_ns"] = json!("-1"),
        |v: &mut Value| v["bindings"]["sample"] = json!("volume"),
    ] {
        assert!(LoadedEventPlan::from_bytes(&mutate(G002, f)).is_err());
    }
    assert!(
        LoadedEventPlan::from_bytes(&mutate(TICK, |v| v["bindings"]["sample"] = json!("close")))
            .is_err()
    );
}
#[test]
fn typed_roots_crossings_and_signals_are_checked_before_run() {
    let bad = [
        compile_bad(TICK, |v| {
            v["outputs"][0]["expression"] = json!({"output":"absent"})
        }),
        compile_bad(TICK, |v| {
            v["outputs"][0]["expression"] = json!({"input":"absent"})
        }),
        compile_bad(TICK, |v| {
            v["outputs"][0]["expression"] = json!({"parameter":"absent"})
        }),
        compile_bad(TICK, |v| {
            v["outputs"][0]["expression"] = json!({"cross":"absent"})
        }),
        compile_bad(TICK, |v| v["signals"][0]["output"] = json!("absent")),
        compile_bad(TICK, |v| {
            v["outputs"][0]["expression"] = json!({"input":"sample"})
        }),
        compile_bad(G002, |v| {
            v["crossings"][0]["right"] = json!({"output":"rsi_value"})
        }),
    ];
    assert_eq!(bad.len(), 7);
    assert_eq!(
        compile_bad(TICK, |v| v["outputs"][0]["expression"]["binary"]["op"] =
            json!("python_eval")),
        "event_expression"
    );
    assert_eq!(
        compile_bad(
            TICK,
            |v| v["outputs"][0]["expression"]["binary"]["rounding"] = json!("exact")
        ),
        "event_expression"
    );
}
#[test]
fn duplicate_roots_crossings_signals_and_invalid_cross_dependencies_rejected() {
    for field in ["outputs", "signals"] {
        let b = mutate(TICK, |v| {
            let copy = v[field][0].clone();
            v[field].as_array_mut().unwrap().push(copy);
        });
        assert!(LoadedEventPlan::from_bytes(&b).is_err());
    }
    assert_eq!(
        compile_bad(G002, |v| {
            let c = v["crossings"][0].clone();
            v["crossings"].as_array_mut().unwrap().push(c);
        }),
        "event_duplicate"
    );
    assert_eq!(
        compile_bad(G002, |v| v["crossings"][0]["left"] = json!({"cross":"up"})),
        "expression_type"
    );
    assert_eq!(
        compile_bad(TICK, |v| v["outputs"][0]["expression"] =
            json!({"input":"sample","output":"price"})),
        "event_expression"
    );
}
#[test]
fn canonical_named_reordering_is_stable_but_semantics_and_bindings_are_pinned() {
    let a = LoadedEventPlan::from_bytes(G002).unwrap();
    let r = registry();
    let p = a.compile(&r, vec![]).unwrap();
    let reordered = mutate(G002, |v| {
        v["outputs"].as_array_mut().unwrap().reverse();
        v["graph"]["graph"]["nodes"]
            .as_array_mut()
            .unwrap()
            .reverse();
        v["graph"]["graph"]["outputs"]
            .as_array_mut()
            .unwrap()
            .reverse();
    });
    let b = LoadedEventPlan::from_bytes(&reordered).unwrap();
    assert_eq!(a.content_id(), b.content_id());
    assert_eq!(p.content_id(), b.compile(&r, vec![]).unwrap().content_id());
    assert_eq!(p.stages().len(), 2);
    for f in [
        |v: &mut Value| v["warmup_samples"] = json!("5"),
        |v: &mut Value| v["trigger"]["timeframe_ns"] = json!("2000"),
        |v: &mut Value| v["crossings"][0]["direction"] = json!("below"),
        |v: &mut Value| v["signals"][0]["kind"] = json!("flat"),
    ] {
        let b = LoadedEventPlan::from_bytes(&mutate(G002, f)).unwrap();
        assert_ne!(a.content_id(), b.content_id());
        assert_ne!(p.content_id(), b.compile(&r, vec![]).unwrap().content_id());
    }
}
#[test]
fn source_descriptor_pin_is_required_and_factories_are_deferred() {
    assert!(
        LoadedEventPlan::from_bytes(G002)
            .unwrap()
            .compile(&empty(), vec![])
            .is_err()
    );
    assert_eq!(
        compile_bad(G002, |v| v["graph"]["entries"][0]["artifact"] =
            json!(format!("sha256:{}", "00".repeat(32)))),
        "registry_artifact"
    );
    // Indicator preparation constructs immutable calls; only EventRuntime::new creates history.
    let p = LoadedEventPlan::from_bytes(G002)
        .unwrap()
        .compile(&registry(), vec![])
        .unwrap();
    assert_eq!(
        p.stages()[0].call.descriptor().declaration().key.id,
        "reference.macd"
    );
}
#[test]
fn event_slot_profile_and_expansion_are_bounded() {
    assert_eq!(MAX_SLOTS, 32);
    let b = mutate(G002, |v| {
        let node = v["graph"]["graph"]["nodes"][0].clone();
        let nodes = v["graph"]["graph"]["nodes"].as_array_mut().unwrap();
        nodes.clear();
        for i in 0..11 {
            let mut n = node.clone();
            n["id"] = json!(format!("m{i}"));
            nodes.push(n);
        }
        let o = v["graph"]["graph"]["outputs"][0].clone();
        v["graph"]["graph"]["outputs"] = json!([o]);
        v["graph"]["graph"]["outputs"][0]["source"]["node"] = json!("m0");
    });
    let a = LoadedEventPlan::from_bytes(&b).unwrap();
    assert_eq!(
        a.compile(&registry(), vec![]).err().unwrap().code,
        "event_limit"
    );
    let b = mutate(TICK, |v| {
        let mut e = json!({"input":"sample"});
        for _ in 0..34 {
            e = json!({"not":e});
        }
        v["outputs"][0]["expression"] = e;
    });
    assert!(LoadedEventPlan::from_bytes(&b).is_err());
    assert_eq!(
        compile_bad(TICK, |v| {
            let root = v["outputs"][0].clone();
            v["outputs"] = Value::Array(
                (0..33)
                    .map(|i| {
                        let mut r = root.clone();
                        r["name"] = json!(format!("r{i}"));
                        r
                    })
                    .collect(),
            );
        }),
        "event_limit"
    );
}
#[test]
fn expression_literals_preserve_large_integer_precision_and_rounding_tags() {
    let b = mutate(TICK, |v| {
        v["signals"] = json!([]);
        v["outputs"][0]["expression"] = json!({"literal":{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":u64::MAX.to_string()}});
    });
    let a = LoadedEventPlan::from_bytes(&b).unwrap();
    assert!(
        std::str::from_utf8(a.canonical_bytes())
            .unwrap()
            .contains("18446744073709551615")
    );
    a.compile(&empty(), vec![]).unwrap();
    for op in ["divide", "multiply"] {
        for r in ["exact", "half_even", "toward_zero"] {
            let b = mutate(TICK, |v| {
                v["signals"] = json!([]);
                v["outputs"][0]["expression"]["binary"]["op"] = json!(op);
                v["outputs"][0]["expression"]["binary"]["rounding"] = json!(r);
            });
            let a = LoadedEventPlan::from_bytes(&b).unwrap();
            if r == "exact" {
                a.compile(&empty(), vec![]).unwrap();
            } else {
                assert!(a.compile(&empty(), vec![]).is_err());
            }
        }
    }
}
