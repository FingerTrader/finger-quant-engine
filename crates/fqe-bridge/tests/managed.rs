use fqe_bridge::{
    Failure, Request,
    managed::{Admission, ManagedRuns},
    prepare,
};
use serde_json::{Value, json};
use std::{
    thread,
    time::{Duration, Instant},
};
mod support;
fn raw(method: &str, p: Value) -> Request {
    Request::parse(&serde_json::to_vec(&json!({"format":"fqe.desktop-request","schema_version":1,"id":"test.1","method":method,"params":p})).unwrap()).unwrap()
}
fn call(m: &ManagedRuns, method: &str, mut p: Value) -> Result<Value, Failure> {
    if method != "run.initialize" {
        p.as_object_mut()
            .unwrap()
            .insert("session_id".into(), m.session_id().into());
    }
    m.execute(&raw(method, p))
}
fn manager() -> ManagedRuns {
    let m = ManagedRuns::new(Admission::default());
    call(&m, "run.initialize", json!({})).unwrap();
    m
}
fn start(m: &ManagedRuns, op: &str, input: Value) -> Value {
    call(m, "run.start", json!({"operation_id":op,"input":input})).unwrap()
}
fn wait(m: &ManagedRuns, h: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = call(m, "run.status", json!({"handle":h})).unwrap();
        if state["terminal"] == true {
            return state;
        }
        assert!(Instant::now() < deadline, "job deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn initialize_session_presence_and_shared_admission_are_real() {
    let admission = Admission::default();
    let m = ManagedRuns::new(admission.clone());
    let r = raw(
        "run.start",
        json!({"session_id":m.session_id(),"operation_id":"a","input":{}}),
    );
    assert_eq!(m.execute(&r).unwrap_err().code, "not_initialized");
    let init = call(&m, "run.initialize", json!({})).unwrap();
    assert_eq!(init["session_id"], m.session_id());
    assert_eq!(init["max_active"], "1");
    let bad = raw(
        "run.status",
        json!({"session_id":"previous.process","handle":"managed.1"}),
    );
    assert_eq!(m.execute(&bad).unwrap_err().code, "session_mismatch");
    let permit = admission.acquire().unwrap();
    assert_eq!(m.execute(&r).unwrap_err().code, "busy");
    drop(permit);
    let job = m.execute(&r).unwrap();
    assert_eq!(wait(&m, job["handle"].as_str().unwrap())["phase"], "failed");
    assert!(admission.acquire().is_ok());
}
#[test]
fn repeated_operations_are_pinned_and_survive_release_as_tombstones() {
    let m = manager();
    let p = support::params();
    let first = start(&m, "stable.operation", p.clone());
    let duplicate = start(&m, "stable.operation", p.clone());
    assert_eq!(first["handle"], duplicate["handle"]);
    assert_eq!(duplicate["duplicate"], true);
    let mut changed = p.clone();
    changed["seed"] = "7".into();
    assert_eq!(
        call(
            &m,
            "run.start",
            json!({"operation_id":"stable.operation","input":changed})
        )
        .unwrap_err()
        .code,
        "operation_conflict"
    );
    let h = first["handle"].as_str().unwrap();
    assert_eq!(wait(&m, h)["phase"], "completed");
    let before = call(
        &m,
        "run.events",
        json!({"handle":h,"after":"0","limit":"16"}),
    )
    .unwrap();
    let again = start(&m, "stable.operation", p.clone());
    assert_eq!(again["duplicate"], true);
    assert_eq!(
        call(
            &m,
            "run.events",
            json!({"handle":h,"after":"0","limit":"16"})
        )
        .unwrap(),
        before
    );
    for _ in 0..2 {
        assert_eq!(
            call(&m, "run.release", json!({"handle":h})).unwrap()["released"],
            true
        );
    }
    let tombstone = start(&m, "stable.operation", p);
    assert_eq!(tombstone["released"], true);
    assert_eq!(tombstone["handle"], first["handle"]);
    assert_eq!(
        call(&m, "run.status", json!({"handle":h}))
            .unwrap_err()
            .code,
        "run_unavailable"
    );
}
#[test]
fn result_chunks_pages_and_repeated_reads_match_native_bytes_without_rerun() {
    let m = manager();
    let p = support::params();
    let h = start(&m, "result", p.clone())["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    let status = wait(&m, &h);
    assert_eq!(status["processed_source_bars"], "7");
    let result = call(&m, "run.result", json!({"handle":h})).unwrap();
    assert_eq!(result["report"]["metrics"]["final_equity"], "97.00");
    let direct = prepare(&p).unwrap().run().unwrap();
    assert_eq!(
        serde_json::to_vec(&result["report"]).unwrap(),
        direct.json()
    );
    assert_eq!(call(&m, "run.result", json!({"handle":h})).unwrap(), result);
    let id = result["result_id"].as_str().unwrap();
    let mut offset = 0usize;
    let mut bytes = vec![];
    loop {
        let chunk = call(
            &m,
            "run.result_chunk",
            json!({"handle":h,"result_id":id,"offset":offset.to_string(),"max_bytes":"109"}),
        )
        .unwrap();
        let hex = chunk["data"].as_str().unwrap();
        for pair in hex.as_bytes().as_chunks::<2>().0 {
            bytes.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
        }
        offset = chunk["next_offset"].as_str().unwrap().parse().unwrap();
        if chunk["eof"] == true {
            break;
        }
    }
    assert_eq!(bytes, direct.json());
    let first = call(
        &m,
        "run.output",
        json!({"handle":h,"result_id":id,"section":"fills","after":"0","limit":"1"}),
    )
    .unwrap();
    assert_eq!(first["rows"][0]["source_bar"], "6");
    assert_eq!(first["next_cursor"], "1");
    assert_eq!(first["eof"], false);
    let second = call(
        &m,
        "run.output",
        json!({"handle":h,"result_id":id,"section":"fills","after":"1","limit":"1"}),
    )
    .unwrap();
    assert_eq!(second["rows"][0]["source_bar"], "7");
    assert_eq!(second["eof"], true);
    for (method, p) in [
        (
            "run.output",
            json!({"handle":h,"result_id":"stale","section":"fills","after":"0","limit":"1"}),
        ),
        (
            "run.result_chunk",
            json!({"handle":h,"result_id":"stale","offset":"0","max_bytes":"1"}),
        ),
        (
            "run.result_chunk",
            json!({"handle":h,"result_id":id,"offset":"99999999","max_bytes":"1"}),
        ),
        (
            "run.output",
            json!({"handle":h,"result_id":id,"section":"unknown","after":"0","limit":"1"}),
        ),
    ] {
        assert!(call(&m, method, p).is_err());
    }
}
#[test]
fn terminal_events_cursor_and_stop_races_never_rewrite_outcomes() {
    let m = manager();
    let h = start(&m, "stop", support::params())["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    call(&m, "run.stop", json!({"handle":h})).unwrap();
    call(&m, "run.stop", json!({"handle":h})).unwrap();
    let state = wait(&m, &h);
    assert!(["completed", "cancelled"].contains(&state["phase"].as_str().unwrap()));
    let final_state = call(&m, "run.stop", json!({"handle":h})).unwrap();
    assert_eq!(final_state, state);
    let events = call(
        &m,
        "run.events",
        json!({"handle":h,"after":"0","limit":"16"}),
    )
    .unwrap();
    let rows = events["events"].as_array().unwrap();
    assert!((2..=4).contains(&rows.len()));
    assert_eq!(rows[0]["phase"], "accepted");
    assert_eq!(rows.last().unwrap()["phase"], state["phase"]);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row["cursor"], (i + 1).to_string());
    }
    let n = rows.len().to_string();
    assert_eq!(
        call(&m, "run.events", json!({"handle":h,"after":n,"limit":"1"})).unwrap()["events"],
        json!([])
    );
    for (after, limit) in [("99", "1"), ("01", "1"), ("0", "0"), ("0", "17")] {
        assert!(
            call(
                &m,
                "run.events",
                json!({"handle":h,"after":after,"limit":limit})
            )
            .is_err()
        );
    }
    let report = call(&m, "run.result", json!({"handle":h})).unwrap();
    assert_eq!(report["report"]["terminal"], state["phase"]);
}
#[test]
fn failures_are_retained_explicitly_and_quotas_do_not_evict_old_results() {
    let m = manager();
    let mut handles = vec![];
    for i in 0..4 {
        let h = start(&m, &format!("bad.{i}"), json!({}))["handle"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(wait(&m, &h)["phase"], "failed");
        assert_eq!(
            call(&m, "run.result", json!({"handle":h}))
                .unwrap_err()
                .code,
            "run_failed"
        );
        handles.push(h);
    }
    assert_eq!(
        call(&m, "run.start", json!({"operation_id":"fifth","input":{}}))
            .unwrap_err()
            .code,
        "run_quota"
    );
    for h in &handles {
        assert_eq!(
            call(&m, "run.status", json!({"handle":h})).unwrap()["phase"],
            "failed"
        );
    }
    call(&m, "run.release", json!({"handle":handles[0]})).unwrap();
    let fifth = start(&m, "fifth", json!({}));
    assert_eq!(fifth["handle"], "managed.5");
}
#[test]
fn operation_receipts_are_bounded_without_forgetting_duplicate_identity() {
    let m = manager();
    for i in 0..64 {
        let h = start(&m, &format!("op.{i}"), json!({}))["handle"]
            .as_str()
            .unwrap()
            .to_owned();
        wait(&m, &h);
        call(&m, "run.release", json!({"handle":h})).unwrap();
    }
    assert_eq!(
        call(
            &m,
            "run.start",
            json!({"operation_id":"overflow","input":{}})
        )
        .unwrap_err()
        .code,
        "operation_quota"
    );
    let old = start(&m, "op.0", json!({}));
    assert_eq!(old["duplicate"], true);
    assert_eq!(old["released"], true);
    assert_eq!(old["handle"], "managed.1");
}
#[test]
fn shutdown_joins_current_work_and_closes_the_session() {
    let admission = Admission::default();
    let m = ManagedRuns::new(admission.clone());
    call(&m, "run.initialize", json!({})).unwrap();
    start(&m, "shutdown", support::params());
    m.shutdown();
    assert!(admission.acquire().is_ok());
    assert_eq!(
        call(&m, "run.initialize", json!({})).unwrap_err().code,
        "session_closed"
    );
}
#[test]
fn model_inspection_includes_generic_workflow_modules_and_parameters() {
    let mut p = support::params();
    p["kind"] = "workflow".into();
    p["model"] = include_str!("../../fqe-runtime/tests/fixtures/p3/backtest-workflow.json").into();
    let r = raw(
        "model.inspect",
        json!({"kind":p["kind"],"model":p["model"],"parameters":p["parameters"]}),
    );
    let result = fqe_bridge::execute(&r, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    assert!(!result["artifact"]["modules"].as_array().unwrap().is_empty());
    assert!(result["artifact"]["instances"][0]["parameters"]["values"]["step"].is_object());
    assert_eq!(
        result["bound_parameters"]["threshold"]["value"],
        "60000000000000"
    );
    assert!(result["descriptors"].as_array().unwrap().len() >= 4);
}

#[test]
fn extended_graph_uses_the_same_managed_lifecycle_and_exact_result_bytes() {
    let m = manager();
    let p: Value = serde_json::from_str(include_str!("fixtures/research-input.json")).unwrap();
    let job = start(&m, "research.managed", p.clone());
    let h = job["handle"].as_str().unwrap();
    assert_eq!(wait(&m, h)["phase"], "completed");
    let result = call(&m, "run.result", json!({"handle":h})).unwrap();
    let direct = fqe_bridge::research::prepare(&p)
        .unwrap()
        .run(&std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    assert_eq!(serde_json::to_vec(&result["report"]).unwrap(), direct.bytes);
    assert_eq!(result["result_id"], direct.result_id);
    assert_eq!(call(&m, "run.result", json!({"handle":h})).unwrap(), result);
    assert_eq!(
        call(&m, "run.release", json!({"handle":h})).unwrap()["released"],
        true
    );
}
