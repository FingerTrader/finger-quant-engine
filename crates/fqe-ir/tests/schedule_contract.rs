use fqe_ir::schedule::ScheduleDefinition;
use serde_json::{Value, json};
const SOURCE: &str = include_str!("fixtures/schedule.json");
fn fixture() -> Value {
    serde_json::from_str(SOURCE).unwrap()
}
fn invalid(change: impl FnOnce(&mut Value)) {
    let mut v = fixture();
    change(&mut v);
    assert!(ScheduleDefinition::from_bytes(&serde_json::to_vec(&v).unwrap()).is_err());
}
#[test]
fn strict_version_capability_calendar_domains_and_queue_limits() {
    invalid(|v| v["schema_version"] = json!(2));
    invalid(|v| v["unknown"] = json!(true));
    invalid(|v| v["requirements"] = json!(["market.schedule/1", "market.exchange-calendar/1"]));
    invalid(|v| v["requirements"] = json!(["market.schedule/1", "market.schedule/1"]));
    invalid(|v| v["calendar"] = json!("NYSE"));
    invalid(|v| v["queue_capacity"] = json!(1));
    invalid(|v| v["queue_capacity"] = json!("0"));
    invalid(|v| v["queue_capacity"] = json!("4097"));
    invalid(|v| v["streams"] = json!([]));
    invalid(|v| v["streams"][0]["domain"]["timeframe_ns"] = json!("0"));
    invalid(|v| v["streams"][0]["domain"]["price_scale"] = json!(13));
    invalid(|v| v["streams"][0]["gap_policy"] = json!("fill"));
    invalid(|v| v["streams"][3]["gap_policy"] = json!("reject"));
    invalid(|v| v["streams"][0]["dataset_id"] = json!("not-a-hash"));
    invalid(|v| v["streams"][0]["domain"]["timezone"] = json!("local"));
    invalid(|v| {
        let mut x = v["streams"][0].clone();
        x["id"] = json!("same-domain");
        x["domain"]["price_scale"] = json!(1);
        v["streams"].as_array_mut().unwrap().push(x);
    });
    invalid(|v| v["streams"][1]["id"] = v["streams"][0]["id"].clone());
    invalid(|v| {
        let s = v["streams"][3].clone();
        v["streams"] = (0..65)
            .map(|i| {
                let mut x = s.clone();
                x["id"] = json!(format!("s{i}"));
                x["domain"]["instrument"] = json!(format!("D{i}"));
                x
            })
            .collect();
    });
    assert!(
        ScheduleDefinition::from_bytes(
            SOURCE
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
fn canonical_identity_is_order_independent_binds_sources_policy_and_limits() {
    let a = ScheduleDefinition::from_bytes(SOURCE.as_bytes()).unwrap();
    let mut v = fixture();
    v["streams"].as_array_mut().unwrap().reverse();
    let b = ScheduleDefinition::from_bytes(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(a.content_id(), b.content_id());
    assert_ne!(a.source_id(), b.source_id());
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    assert_eq!(
        ScheduleDefinition::from_bytes(a.canonical_bytes())
            .unwrap()
            .content_id(),
        a.content_id()
    );
    assert_eq!(a.queue_capacity(), 32);
    assert_eq!(a.streams().len(), 4);
    for (path, value) in [
        (
            "/streams/0/dataset_id",
            json!(format!("sha256:{}", "33".repeat(32))),
        ),
        ("/queue_capacity", json!("33")),
        ("/streams/1/gap_policy", json!("report")),
    ] {
        let mut v = fixture();
        *v.pointer_mut(path).unwrap() = value;
        assert_ne!(
            ScheduleDefinition::from_bytes(&serde_json::to_vec(&v).unwrap())
                .unwrap()
                .content_id(),
            a.content_id()
        );
    }
}
