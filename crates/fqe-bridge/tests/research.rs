//! Additive native-profile acceptance: actual accounting, cancellation and strict compilation.
use fqe_bridge::research;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
fn input() -> Value {
    serde_json::from_str(include_str!("fixtures/research-input.json")).unwrap()
}
fn model(p: &Value) -> Value {
    serde_json::from_str(p["model"].as_str().unwrap()).unwrap()
}
fn changed(mut p: Value, change: impl FnOnce(&mut Value)) -> Value {
    let mut m = model(&p);
    change(&mut m);
    p["model"] = serde_json::to_string(&m).unwrap().into();
    p
}
#[test]
fn exact_native_result_and_repeated_run_are_identical() {
    let p = input();
    let a = research::prepare(&p)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let b = research::prepare(&p)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(a.bytes, b.bytes);
    assert_eq!(a.result_id, b.result_id);
    let report: Value = serde_json::from_slice(&a.bytes).unwrap();
    assert_eq!(report["metrics"]["final_equity"], "98.00");
    assert_eq!(report["fills"].as_array().unwrap().len(), 2);
}
#[test]
fn cancellation_is_a_real_empty_prefix_and_not_completed() {
    let r = research::prepare(&input())
        .unwrap()
        .run(&AtomicBool::new(true))
        .unwrap();
    let v: Value = serde_json::from_slice(&r.bytes).unwrap();
    assert!(r.cancelled);
    assert_eq!(r.processed, 0);
    assert_eq!(v["terminal"], "cancelled");
    assert!(v["fills"].as_array().unwrap().is_empty());
    assert_eq!(v["metrics"]["final_equity"], "100.00");
}
#[test]
fn unused_invalid_nodes_and_settings_do_not_disappear() {
    let p = changed(input(), |m| m["nodes"][0]["settings"]["typo"] = "1".into());
    assert_eq!(
        research::prepare(&p).err().unwrap().code,
        "research_unknown_setting"
    );
    let p = changed(input(), |m| m["parameters"][0]["value"] = "251".into());
    assert_eq!(
        research::prepare(&p).err().unwrap().code,
        "research_parameter_range"
    );
}
#[test]
fn graph_cannot_have_an_instantaneous_cycle_or_wrong_unit() {
    let p = changed(input(), |m| {
        let w = &mut m["wires"].as_array_mut().unwrap()[0];
        w["from"] = w["to"].clone();
        w["output"] = "value".into();
    });
    assert!(research::prepare(&p).is_err());
    let p = changed(input(), |m| {
        for n in m["nodes"].as_array_mut().unwrap() {
            if n["kind"] == "entry" {
                n["settings"]["quantity"] = "$fast_period".into();
            }
        }
    });
    assert_eq!(
        research::prepare(&p).err().unwrap().code,
        "research_parameter_type"
    );
}
#[test]
fn run_identity_and_seed_are_validated_independently() {
    let mut p = input();
    p["seed"] = "00".into();
    assert_eq!(research::prepare(&p).err().unwrap().code, "research_seed");
    p["seed"] = "0".into();
    p["run"] = "".into();
    assert!(research::prepare(&p).is_err());
}
