use fqe_core::types::ScalarValue;
use fqe_ir::{
    loading::{LoadedModel, MAX_COLLECTION, MAX_DEPTH, MAX_ITEMS},
    model::{Argument, MAX_MODEL_BYTES},
    parameters::BoundValue,
};
use serde_json::{Value, json};
use std::io::{self, Read};
const FIXTURE: &[u8] = include_bytes!("fixtures/model.json");
fn fixture() -> Value {
    serde_json::from_slice(FIXTURE).unwrap()
}
fn load(v: &Value) -> Result<LoadedModel, fqe_ir::model::ModelError> {
    LoadedModel::from_bytes(&serde_json::to_vec(v).unwrap())
}
fn scalar(kind: &str, value: Value) -> Value {
    json!({"type":{"version":1,"shape":"scalar","kind":kind},"value":value})
}
fn bindings(
    m: &LoadedModel,
    values: Value,
) -> Result<fqe_ir::loading::ModelBinding, fqe_ir::model::ModelError> {
    m.bind_json(&serde_json::to_vec(&json!({"version":1,"values":values})).unwrap())
}
fn period(b: &fqe_ir::loading::ModelBinding) -> u64 {
    match b.parameters().get("period").unwrap() {
        BoundValue::Value(v) => match v.value() {
            ScalarValue::U64(n) => *n,
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn real_file_and_independent_canonical_sha256_fixture() {
    let m = LoadedModel::from_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/model.json"
    ))
    .unwrap();
    let ids: Vec<_> = include_str!("fixtures/identities.txt").lines().collect();
    assert_eq!(m.source_id().to_string(), ids[0]);
    assert_eq!(m.content_id().to_string(), ids[1]);
    assert_eq!(
        m.canonical_bytes(),
        include_bytes!("fixtures/canonical.json")
    );
    let round = LoadedModel::from_bytes(m.canonical_bytes()).unwrap();
    assert_eq!(round.definition(), m.definition());
    assert_eq!(round.content_id(), m.content_id());
    let n = &m.definition().declaration().nodes[0];
    let Argument::Literal(v) = &n.arguments["maximum"] else {
        panic!()
    };
    assert_eq!(v.value(), &ScalarValue::U64(u64::MAX));
}
#[test]
fn ordering_whitespace_and_escaped_keys_do_not_change_canonical_identity() {
    let mut f = fixture();
    let original = load(&f).unwrap();
    let duplicate = f["graph"]["nodes"][0].clone();
    f["graph"]["nodes"].as_array_mut().unwrap().push(duplicate);
    f["graph"]["nodes"][1]["id"] = json!("another");
    let a = load(&f).unwrap();
    f["graph"]["nodes"].as_array_mut().unwrap().reverse();
    f["requirements"].as_array_mut().unwrap().reverse();
    assert_eq!(a.content_id(), load(&f).unwrap().content_id());
    let mut raw = String::from_utf8(FIXTURE.to_vec())
        .unwrap()
        .replace("\"format\"", "\"for\\u006dat\"");
    raw.push_str(" \n\t");
    let b = LoadedModel::from_bytes(raw.as_bytes()).unwrap();
    assert_eq!(original.content_id(), b.content_id());
    assert_ne!(original.source_id(), b.source_id());
}
#[test]
fn bound_parameters_are_immutable_and_pinned_to_exact_content() {
    let m = load(&fixture()).unwrap();
    let a = bindings(&m, json!({})).unwrap();
    let b = bindings(&m, json!({"period":scalar("u64",json!("5"))})).unwrap();
    assert_eq!((period(&a), period(&b)), (2, 5));
    assert_eq!(a.model_id(), "example.model");
    assert_eq!(a.author_version(), "1.0.0");
    assert_eq!(a.content_id(), m.content_id());
    a.ensure_matches(&m).unwrap();
    let mut f = fixture();
    f["metadata"]["label"] = json!("changed");
    let changed = load(&f).unwrap();
    assert_ne!(m.content_id(), changed.content_id());
    assert_eq!(
        a.ensure_matches(&changed).unwrap_err().code,
        "content_mismatch"
    );
    assert_eq!(period(&a), 2);
}
#[test]
fn binding_default_required_optional_null_unknown_and_readiness_matrix() {
    let mut f = fixture();
    let p = &mut f["parameters"]["entries"][0];
    p.as_object_mut().unwrap().remove("default");
    p["required"] = json!(true);
    let m = load(&f).unwrap();
    assert!(bindings(&m, json!({})).is_err());
    assert!(bindings(&m, json!({"period":scalar("u64",json!("3"))})).is_ok());
    f["parameters"]["entries"][0]["required"] = json!(false);
    let m = load(&f).unwrap();
    assert_eq!(bindings(&m, json!({})).unwrap_err().code, "unbound");
    f["graph"]["nodes"][0]["arguments"]
        .as_object_mut()
        .unwrap()
        .remove("period");
    let m = load(&f).unwrap();
    let b = bindings(&m, json!({})).unwrap();
    assert_eq!(b.parameters().get("period"), Some(&BoundValue::Unbound));
    for values in [
        json!({"period":null}),
        json!({"unknown":scalar("u64",json!("3"))}),
        json!({"period":{"state":"missing"}}),
        json!({"period":{"state":"warmup"}}),
        json!({"period":{"state":"error"}}),
    ] {
        assert!(bindings(&m, values).is_err());
    }
    assert!(
        m.bind_json(br#"{"version":1,"values":{"period":null,"period":null}}"#)
            .is_err()
    );
    assert!(m.bind_json(br#"{"version":2,"values":{}}"#).is_err());
    assert!(m.bind_json(br#"{"values":{}}"#).is_err());
}
#[test]
fn decoded_range_enum_type_and_integer_representation_matrix() {
    let mut f = fixture();
    f["parameters"]["entries"][0]["constraints"]["choices"] =
        json!([scalar("u64", json!("2")), scalar("u64", json!("5"))]);
    let m = load(&f).unwrap();
    for value in [
        scalar("u64", json!("0")),
        scalar("u64", json!("11")),
        scalar("u64", json!("3")),
        scalar("i64", json!("2")),
        scalar("u64", json!(2)),
        scalar("u64", json!("02")),
        scalar("u64", json!("+2")),
        scalar("u64", json!("2.0")),
        scalar("u64", json!("2e0")),
        scalar("u64", json!("18446744073709551616")),
    ] {
        assert!(bindings(&m, json!({"period":value})).is_err());
    }
    assert_eq!(
        period(&bindings(&m, json!({"period":scalar("u64",json!("5"))})).unwrap()),
        5
    );
    for bad in [
        json!([]),
        json!([scalar("u64", json!("2")), scalar("u64", json!("2"))]),
        json!([scalar("u64", json!("11"))]),
    ] {
        f["parameters"]["entries"][0]["constraints"]["choices"] = bad;
        assert!(load(&f).is_err());
    }
}
#[test]
fn exact_decimal_i128_min_units_scales_bool_text_and_i64() {
    let mut f = fixture();
    let t = json!({"version":1,"shape":"scalar","kind":"decimal","scale":18,"unit":{"kind":"money","currency":"USD"}});
    let v = json!({"type":t,"value":i128::MIN.to_string()});
    f["parameters"]["entries"][0] =
        json!({"key":"period","type":t,"required":true,"constraints":{},"description":"exact"});
    let m = load(&f).unwrap();
    let b = bindings(&m, json!({"period":v})).unwrap();
    let Some(BoundValue::Value(value)) = b.parameters().get("period") else {
        panic!()
    };
    let ScalarValue::Decimal(d) = value.value() else {
        panic!()
    };
    assert_eq!(d.coefficient(), i128::MIN);
    assert_eq!(d.scale(), 18);
    for (pointer, bad) in [
        ("/type/scale", json!(17)),
        ("/type/unit/currency", json!("EUR")),
        ("/type/unit", json!({"kind":"price","instrument":"A"})),
        ("/value", json!("-0")),
        ("/value", json!("-170141183460469231731687303715884105729")),
    ] {
        let mut v = v.clone();
        *v.pointer_mut(pointer).unwrap() = bad;
        assert!(bindings(&m, json!({"period":v})).is_err());
    }
    for v in [
        scalar("i64", json!(i64::MIN.to_string())),
        scalar("bool", json!(true)),
        json!({"type":{"version":1,"shape":"scalar","kind":"text","max_bytes":6},"value":"中文"}),
        json!({"type":{"version":1,"shape":"scalar","kind":"decimal","scale":2,"unit":{"kind":"quantity","instrument":"A"}},"value":"-123"}),
        json!({"type":{"version":1,"shape":"scalar","kind":"decimal","scale":0,"unit":{"kind":"ratio"}},"value":"0"}),
    ] {
        let mut f = fixture();
        f["graph"]["nodes"][0]["arguments"]["literal"] = json!({"literal":v});
        let m = load(&f).unwrap();
        assert_eq!(
            m.content_id(),
            LoadedModel::from_bytes(m.canonical_bytes())
                .unwrap()
                .content_id()
        );
    }
}
#[test]
fn invalid_defaults_constraints_and_duplicate_parameter_ids_are_rejected() {
    for change in 0..7 {
        let mut f = fixture();
        let p = &mut f["parameters"]["entries"][0];
        match change {
            0 => p["default"] = Value::Null,
            1 => p["default"] = scalar("u64", json!("0")),
            2 => p["required"] = json!(true),
            3 => p["constraints"]["lower"]["value"] = scalar("i64", json!("1")),
            4 => p["constraints"]["lower"]["value"] = scalar("u64", json!("11")),
            5 => p["constraints"]["lower"]["inclusive"] = Value::Null,
            _ => {
                let p = p.clone();
                f["parameters"]["entries"].as_array_mut().unwrap().push(p);
            }
        }
        assert!(load(&f).is_err());
    }
}
#[test]
fn missing_unknown_unsupported_versions_features_and_references_fail() {
    for field in fixture().as_object().unwrap().keys() {
        let mut f = fixture();
        f.as_object_mut().unwrap().remove(field);
        assert_eq!(load(&f).unwrap_err().code, "missing_field", "{field}");
    }
    for (p, v) in [
        ("/schema_version", json!(2)),
        ("/ir_version", json!(1)),
        ("/parameters/version", json!(2)),
        ("/entries/0/descriptor_version", json!(2)),
        ("/entries/0/role", json!("adapter")),
        ("/requirements/0", json!("network/1")),
        ("/modules", json!([{}])),
        ("/state", json!([{}])),
        ("/dependencies", json!([{}])),
        ("/workflow", json!({"start":"x"})),
        ("/graph/nodes/0/entry/id", json!("unknown")),
        ("/graph/outputs/0/source/port", json!("missing")),
        ("/entries/0/artifact", json!("sha256:bad")),
        ("/parameters/entries/0/type/shape", json!("series")),
        ("/parameters/entries/0/type/kind", json!("float64")),
        ("/parameters/entries/0/type/version", json!(2)),
    ] {
        let mut f = fixture();
        *f.pointer_mut(p).unwrap() = v;
        assert!(load(&f).is_err(), "{p}");
    }
    let mut f = fixture();
    f["extra"] = json!(true);
    assert_eq!(load(&f).unwrap_err().code, "unknown_field");
    for path in ["/entries", "/graph/nodes", "/graph/outputs"] {
        let mut f = fixture();
        let a = f.pointer_mut(path).unwrap().as_array_mut().unwrap();
        a.push(a[0].clone());
        assert_eq!(load(&f).unwrap_err().code, "duplicate_id");
    }
}
#[test]
fn malformed_truncated_utf8_trailing_and_duplicate_documents_fail() {
    for bytes in [
        include_bytes!("fixtures/truncated.json").as_slice(),
        b"\xff",
        b"{}{}",
        b"null",
        b"{",
        b"{\"version\":1e0}",
        b"{\"version\":-1}",
        b"{\"version\":-0}",
    ] {
        assert!(LoadedModel::from_bytes(bytes).is_err());
    }
    for bytes in [
        include_bytes!("fixtures/duplicate.json").as_slice(),
        br#"{"format":1,"for\u006dat":2}"#,
        br#"{"nested":{"x":1,"x":2}}"#,
    ] {
        let e = LoadedModel::from_bytes(bytes).unwrap_err();
        assert_eq!(e.code, "duplicate_key");
        assert!(e.detail.len() < 100);
        assert!(e.detail.contains("line"));
    }
}
#[test]
fn resource_limits_reject_bytes_depth_strings_keys_collections_and_item_count() {
    assert_eq!(
        LoadedModel::from_bytes(&vec![b' '; MAX_MODEL_BYTES + 1])
            .unwrap_err()
            .code,
        "resource_limit"
    );
    let deep = format!(
        "{}0{}",
        "[".repeat(MAX_DEPTH + 1),
        "]".repeat(MAX_DEPTH + 1)
    );
    assert_eq!(
        LoadedModel::from_bytes(deep.as_bytes()).unwrap_err().code,
        "resource_limit"
    );
    let a = serde_json::to_vec(&vec![Value::Null; MAX_COLLECTION + 1]).unwrap();
    assert_eq!(
        LoadedModel::from_bytes(&a).unwrap_err().code,
        "resource_limit"
    );
    let a = serde_json::to_vec(&vec![
        vec![Value::Null; MAX_COLLECTION];
        MAX_ITEMS / MAX_COLLECTION + 1
    ])
    .unwrap();
    assert_eq!(
        LoadedModel::from_bytes(&a).unwrap_err().code,
        "resource_limit"
    );
    for v in [json!("x".repeat(4097)), json!({"x".repeat(129):0})] {
        assert_eq!(load(&v).unwrap_err().code, "resource_limit");
    }
    let mut f = fixture();
    f["metadata"]["label"] = json!("x".repeat(4096));
    load(&f).unwrap();
    f["metadata"]["label"] = json!("x".repeat(4097));
    assert!(load(&f).is_err());
}
#[test]
fn reader_stops_at_byte_budget_and_io_errors_are_structured() {
    struct Endless {
        read: usize,
    }
    impl Read for Endless {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            b.fill(b' ');
            self.read += b.len();
            Ok(b.len())
        }
    }
    let mut r = Endless { read: 0 };
    assert_eq!(
        LoadedModel::from_reader(&mut r).unwrap_err().code,
        "resource_limit"
    );
    assert_eq!(r.read, MAX_MODEL_BYTES + 1);
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("private detail"))
        }
    }
    let e = LoadedModel::from_reader(Broken).unwrap_err();
    assert_eq!(e.code, "io");
    assert!(!e.detail.contains("private detail"));
}

#[test]
fn each_supported_type_obeys_decoded_default_required_and_absence_rules() {
    let values = [
        scalar("u64", json!(u64::MAX.to_string())),
        scalar("i64", json!(i64::MIN.to_string())),
        scalar("bool", json!(false)),
        json!({"type":{"version":1,"shape":"scalar","kind":"text","max_bytes":6},"value":"中文"}),
        json!({"type":{"version":1,"shape":"scalar","kind":"decimal","scale":4,"unit":{"kind":"scalar"}},"value":"-1234"}),
    ];
    for value in values {
        let mut f = fixture();
        f["parameters"]["entries"][0] = json!({"key":"period","type":value["type"],"required":false,"default":value,"constraints":{},"description":"matrix"});
        let m = load(&f).unwrap();
        let default = bindings(&m, json!({})).unwrap();
        let explicit = bindings(&m, json!({"period":value})).unwrap();
        assert_eq!(default.parameters(), explicit.parameters());
        assert!(bindings(&m, json!({"period":null})).is_err());
        assert!(
            bindings(&m, json!({"period":scalar("u64",json!("1"))})).is_err()
                || value["type"]["kind"] == "u64"
        );
        f["parameters"]["entries"][0]
            .as_object_mut()
            .unwrap()
            .remove("default");
        f["parameters"]["entries"][0]["required"] = json!(true);
        let m = load(&f).unwrap();
        assert!(bindings(&m, json!({})).is_err());
        assert!(bindings(&m, json!({"period":value})).is_ok());
    }
}
