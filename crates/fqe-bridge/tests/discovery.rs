use fqe_bridge::dispatch;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn call(method: &str, params: Value) -> Value {
    let bytes = serde_json::to_vec(&json!({"format":"fqe.desktop-request","schema_version":1,
        "id":"catalog.test","method":method,"params":params}))
    .unwrap();
    serde_json::from_slice(&dispatch(&bytes, &AtomicBool::new(false))).unwrap()
}
fn trigger() -> Value {
    json!({"kind":"closed_bar","instrument":"DEMO","timeframe_ns":"60000000000",
        "price_scale":2,"volume_scale":3})
}
#[test]
fn blank_project_discovers_domain_bound_descriptors_without_a_model() {
    let a = call("catalog.describe", json!({"trigger":trigger()}));
    assert_eq!(a["ok"], true, "{a}");
    let a = &a["body"];
    assert_eq!(a["format"], "fqe.catalog-description");
    assert_eq!(a["trigger"], trigger());
    assert_eq!(a["support"]["limits"]["financial_source_bars"], "256");
    assert_eq!(a["support"]["optimization"], false);
    assert_eq!(a["support"]["external_languages"], false);
    let descriptors = a["descriptors"].as_array().unwrap();
    assert!(descriptors.len() > 4);
    assert!(
        descriptors
            .iter()
            .any(|d| d["key"]["id"] == "reference.sma")
    );
    let repeat = call("catalog.describe", json!({"trigger":trigger()}));
    assert_eq!(a, &repeat["body"]);
    let mut other = trigger();
    other["instrument"] = "OTHER".into();
    let b = call("catalog.describe", json!({"trigger":other}));
    assert_eq!(b["ok"], true);
    assert_ne!(a["registry_id"], b["body"]["registry_id"]);
    let tick = call(
        "catalog.describe",
        json!({"trigger":{"kind":"tick","instrument":"DEMO"}}),
    );
    assert_eq!(tick["ok"], true);
    assert!(
        !tick["body"]["descriptors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["key"]["id"] == "reference.sma")
    );
}
#[test]
fn explicit_capability_and_existing_model_based_discovery_remain_distinct() {
    assert_eq!(
        call(
            "hello",
            json!({"major":1,"minor":0,"required":["catalog.describe/1"]})
        )["ok"],
        true
    );
    assert_eq!(
        call("descriptor.list", json!({"trigger":trigger()}))["ok"],
        false
    );
    assert_eq!(
        call(
            "hello",
            json!({"major":1,"minor":0,"required":["optimization.managed/1"]})
        )["ok"],
        false
    );
}
#[test]
fn unknown_extra_missing_and_inexact_contexts_reject() {
    for params in [
        json!({}),
        json!({"trigger":trigger(),"model":"ignored"}),
        json!({"trigger":null}),
    ] {
        assert_eq!(call("catalog.describe", params)["ok"], false);
    }
    for (key, value) in [
        ("kind", json!("unavailable")),
        ("instrument", json!("")),
        ("timeframe_ns", json!("0")),
        ("timeframe_ns", json!("01")),
        ("timeframe_ns", json!(600000)),
        ("timeframe_ns", json!("18446744073709551616")),
        ("price_scale", json!(13)),
        ("price_scale", json!("2")),
        ("volume_scale", json!(19)),
        ("unexpected", json!(true)),
    ] {
        let mut t = trigger();
        t[key] = value;
        assert_eq!(
            call("catalog.describe", json!({"trigger":t}))["ok"],
            false,
            "{key}"
        );
    }
    for key in [
        "kind",
        "instrument",
        "timeframe_ns",
        "price_scale",
        "volume_scale",
    ] {
        let mut t = trigger();
        t.as_object_mut().unwrap().remove(key);
        assert_eq!(call("catalog.describe", json!({"trigger":t}))["ok"], false);
    }
}
