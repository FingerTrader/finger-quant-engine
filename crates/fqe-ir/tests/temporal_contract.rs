use fqe_ir::{
    catalog,
    temporal::LoadedTemporalRules,
    workflow::{CompiledWorkflow, LoadedWorkflow},
};
use serde_json::{Value, json};
const COUNT: &str = include_str!("fixtures/temporal-count.json");
fn plan(source: &str) -> CompiledWorkflow {
    let v = LoadedWorkflow::from_bytes(source.as_bytes()).unwrap();
    v.compile(
        &catalog::reference(v.event_plan().trigger()).unwrap(),
        vec![],
    )
    .unwrap()
}
fn bad(f: impl FnOnce(&mut Value)) {
    let mut v: Value = serde_json::from_str(COUNT).unwrap();
    f(&mut v);
    let result = LoadedTemporalRules::from_bytes(&serde_json::to_vec(&v).unwrap())
        .and_then(|r| r.bind(&plan(include_str!("fixtures/workflow-bar-counter.json"))));
    assert!(result.is_err());
}
#[test]
fn strict_temporal_versions_types_domains_and_bounds() {
    bad(|v| v["schema_version"] = json!(2));
    bad(|v| v["unknown"] = json!(true));
    bad(|v| v["requirements"] = json!(["temporal.wall-clock/1"]));
    bad(|v| v["windows"][0]["span"]["bars"] = json!("0"));
    bad(|v| v["windows"][0]["span"]["bars"] = json!("1000001"));
    bad(|v| v["windows"][0]["span"]["bars"] = json!(3));
    bad(|v| v["windows"][0]["span"] = json!({"kind":"event-time","duration_ns":"0"}));
    bad(|v| v["windows"][0]["span"] = json!({"kind":"calendar-months","months":"1"}));
    bad(|v| v["windows"][0]["arm"] = json!({"instance":"fast","output":"count"}));
    bad(|v| v["windows"][0]["predicate"] = json!({"event":"unknown"}));
    bad(|v| {
        let x = v["windows"][0].clone();
        v["windows"].as_array_mut().unwrap().push(x);
    });
    bad(|v| {
        let x = v["windows"][0].clone();
        v["windows"] = (0..17)
            .map(|i| {
                let mut x = x.clone();
                x["id"] = json!(format!("w{i}"));
                x
            })
            .collect();
    });
    assert!(
        LoadedTemporalRules::from_bytes(COUNT.as_bytes())
            .unwrap()
            .bind(&plan(include_str!("fixtures/workflow-counter.json")))
            .is_err()
    );
    assert!(
        LoadedTemporalRules::from_bytes(
            COUNT
                .replacen(
                    "\"schema_version\": 1",
                    "\"schema_version\":1,\"schema_version\":1",
                    1
                )
                .as_bytes()
        )
        .is_err()
    );
}
#[test]
fn canonical_temporal_order_and_bound_workflow_identity() {
    let source = include_str!("fixtures/temporal-two-time.json");
    let a = LoadedTemporalRules::from_bytes(source.as_bytes()).unwrap();
    let mut v: Value = serde_json::from_str(source).unwrap();
    v["windows"].as_array_mut().unwrap().reverse();
    let b = LoadedTemporalRules::from_bytes(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(a.content_id(), b.content_id());
    assert_ne!(a.source_id(), b.source_id());
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    assert_eq!(
        LoadedTemporalRules::from_bytes(a.canonical_bytes())
            .unwrap()
            .content_id(),
        a.content_id()
    );
    let bar = plan(include_str!("fixtures/workflow-bar-counter.json"));
    let tick = plan(include_str!("fixtures/workflow-counter.json"));
    let bound = a.bind(&bar).unwrap();
    assert_eq!(bound.workflow_id(), bar.content_id());
    assert_eq!(bound.artifact_id(), a.content_id());
    assert_eq!(bound.source_id(), a.source_id());
    assert_ne!(bound.content_id(), a.bind(&tick).unwrap().content_id());
}
