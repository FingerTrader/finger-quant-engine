use fqe_bridge::{
    Request,
    managed::{Admission, ManagedRuns},
    research,
};
use serde_json::{Value, json};
use std::{
    sync::atomic::AtomicBool,
    thread,
    time::{Duration, Instant},
};
fn input() -> Value {
    serde_json::from_str(include_str!("fixtures/research-input.json")).unwrap()
}
fn call(m: &ManagedRuns, method: &str, mut params: Value) -> Value {
    params
        .as_object_mut()
        .unwrap()
        .insert("session_id".into(), json!(m.session_id()));
    let r = Request::parse(&serde_json::to_vec(&json!({"format":"fqe.desktop-request","schema_version":1,"id":"live.test","method":method,"params":params})).unwrap()).unwrap();
    m.execute(&r).unwrap()
}
fn manager() -> ManagedRuns {
    let m = ManagedRuns::new(Admission::default());
    m.execute(&Request::parse(&serde_json::to_vec(&json!({"format":"fqe.desktop-request","schema_version":1,"id":"init","method":"run.initialize","params":{}})).unwrap()).unwrap()).unwrap();
    m
}
fn advance(m: &ManagedRuns, h: &str, cursor: usize) -> Value {
    let mut v = call(
        m,
        "run.advance",
        json!({"handle":h,"after":cursor.to_string()}),
    );
    let until = Instant::now() + Duration::from_secs(2);
    while v["snapshot"].is_null() {
        assert!(Instant::now() < until, "committed bar deadline");
        thread::sleep(Duration::from_millis(1));
        v = call(
            m,
            "run.live",
            json!({"handle":h,"after":cursor.to_string()}),
        );
    }
    v
}
fn terminal(m: &ManagedRuns, h: &str) -> Value {
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        let v = call(m, "run.status", json!({"handle":h}));
        if v["terminal"] == true {
            return v;
        }
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn acknowledged_bar_gate_is_real_and_financial_result_matches_batch() {
    let m = manager();
    let p = input();
    let h = call(
        &m,
        "run.start_live",
        json!({"operation_id":"live.one","input":p}),
    )["handle"]
        .as_str()
        .unwrap()
        .to_string();
    thread::sleep(Duration::from_millis(20));
    let first = call(&m, "run.live", json!({"handle":h,"after":"0"}));
    assert_eq!(first["cursor"], "0");
    assert!(first["snapshot"].is_null());
    let mut cursor = 0;
    let mut snapshots = vec![];
    loop {
        let v = advance(&m, &h, cursor);
        cursor += 1;
        assert_eq!(v["cursor"], cursor.to_string());
        let frame = v["snapshot"].clone();
        assert_eq!(frame["processed"], cursor.to_string());
        let total = frame["total"].as_str().unwrap().parse::<usize>().unwrap();
        snapshots.push(frame);
        if cursor == total {
            break;
        }
        // Paused means native graph/account do not advance behind a frozen UI.
        thread::sleep(Duration::from_millis(3));
        assert_eq!(
            call(
                &m,
                "run.live",
                json!({"handle":h,"after":cursor.to_string()})
            )["cursor"],
            cursor.to_string()
        );
    }
    assert_eq!(terminal(&m, &h)["phase"], "completed");
    let actual = call(&m, "run.result", json!({"handle":h}))["report"].clone();
    let expected: Value = serde_json::from_slice(
        &research::prepare(&p)
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert_eq!(actual, expected);
    for frame in snapshots {
        let at = actual["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| {
                f["phase"] == "decision_close" && f["source_sequence"] == frame["source_sequence"]
            })
            .unwrap();
        assert_eq!(frame["account"], at["account"]);
    }
}
#[test]
fn stop_while_gated_releases_worker_and_next_operation_starts_empty() {
    let m = manager();
    let p = input();
    let h = call(
        &m,
        "run.start_live",
        json!({"operation_id":"stopped","input":p}),
    )["handle"]
        .as_str()
        .unwrap()
        .to_string();
    advance(&m, &h, 0);
    call(&m, "run.stop", json!({"handle":h}));
    assert_eq!(terminal(&m, &h)["phase"], "cancelled");
    call(&m, "run.release", json!({"handle":h}));
    let fresh = call(
        &m,
        "run.start_live",
        json!({"operation_id":"fresh","input":p}),
    )["handle"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(h, fresh);
    let v = call(&m, "run.live", json!({"handle":fresh,"after":"0"}));
    assert_eq!(v["cursor"], "0");
    assert!(v["snapshot"].is_null());
    let first = advance(&m, &fresh, 0);
    assert_eq!(first["snapshot"]["point"]["equity"], "100.00");
    assert!(first["snapshot"]["fills"].as_array().unwrap().is_empty());
    m.shutdown(); // blocked at a bar boundary must not hang application exit.
}
