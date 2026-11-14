use fqe_bridge::{MAX_REQUEST, MAX_RESPONSE, Request, dispatch, ffi::*, framing::*, prepare};
use serde_json::{Value, json};
use std::{io::Cursor, sync::atomic::AtomicBool};
fn request(method: &str, params: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"format":"fqe.desktop-request","schema_version":1,"id":"test.1","method":method,"params":params})).unwrap()
}
fn call(method: &str, params: Value) -> Value {
    serde_json::from_slice(&dispatch(&request(method, params), &AtomicBool::new(false))).unwrap()
}
mod support;
use support::params;

#[test]
fn preflight_validates_full_frozen_financial_inputs_without_running() {
    let p = params();
    let preflight = call("backtest.validate", p.clone());
    assert_eq!(preflight["ok"], true);
    let run = call("backtest.run", p.clone());
    assert_eq!(preflight["body"]["manifest_id"], run["body"]["manifest_id"]);
    assert_eq!(
        preflight["body"]["manifest"],
        run["body"]["report"]["manifest"]
    );
    assert!(preflight["body"].get("report").is_none());
    for key in ["initial_cash", "participation_bps", "minimum_fee"] {
        let mut invalid = p.clone();
        invalid["account"][key] = "invalid".into();
        assert_eq!(call("backtest.validate", invalid)["ok"], false);
    }
    let mut invalid = p.clone();
    invalid["warmup_bars"] = "257".into();
    assert_eq!(call("backtest.validate", invalid)["ok"], false);
    let mut invalid = p;
    invalid["policies"][0]["source"]["kind"] = "legacy_python".into();
    assert_eq!(call("backtest.validate", invalid)["ok"], false);
}

#[test]
fn strict_outer_codec_presence_versions_and_exact_strings() {
    for raw in [
        b"{\"a\":1,\"a\":2}".as_slice(),
        b"[]",
        b"{",
        b"{} {}",
        b"{\"x\":1.5}",
        b"{\"x\":-1}",
        b"\xff",
    ] {
        let reply: Value = serde_json::from_slice(&dispatch(raw, &AtomicBool::new(false))).unwrap();
        assert_eq!(reply["ok"], false);
        assert!(reply["id"].is_null());
    }
    assert!(Request::parse(&vec![b' '; MAX_REQUEST + 1]).is_err());
    let deep = format!("{}0{}", "[".repeat(34), "]".repeat(34));
    assert!(Request::parse(deep.as_bytes()).is_err());
    let mut v: Value = serde_json::from_slice(&request("echo", json!({}))).unwrap();
    for key in ["format", "schema_version", "id", "method", "params"] {
        let mut absent = v.clone();
        absent.as_object_mut().unwrap().remove(key);
        assert!(Request::parse(&serde_json::to_vec(&absent).unwrap()).is_err());
    }
    v["schema_version"] = 2.into();
    assert!(Request::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    assert_eq!(
        call("echo", json!({"u64":"18446744073709551615","missing":null}))["body"]["u64"],
        "18446744073709551615"
    );
    let oversized_string = "x".repeat(8192);
    assert_eq!(call("echo", json!({"model":oversized_string}))["ok"], true);
}
#[test]
fn capability_negotiation_rejects_incompatible_and_unknown_methods() {
    assert_eq!(
        call(
            "hello",
            json!({"major":1,"minor":99,"required":["backtest.cash-long/1"]})
        )["body"]["minor"],
        0
    );
    for p in [
        json!({"major":2,"minor":0,"required":[]}),
        json!({"major":1,"minor":0,"required":["external.untrusted"]}),
        json!({"major":1,"minor":0}),
        json!({"major":1,"minor":0,"required":[],"unexpected":true}),
    ] {
        assert_eq!(call("hello", p)["ok"], false);
    }
    assert_eq!(
        call("live.submit", json!({"credential":"do-not-echo"}))["body"]["code"],
        "unsupported_method"
    );
}
#[test]
fn generic_catalog_validate_and_actual_event_financial_parity() {
    let p = params();
    let model = json!({"kind":p["kind"],"model":p["model"],"parameters":p["parameters"]});
    let validation = call("model.validate", model.clone());
    assert_eq!(validation["ok"], true);
    let descriptors = call("descriptor.list", model);
    assert_eq!(descriptors["ok"], true);
    let ds = descriptors["body"]["descriptors"].as_array().unwrap();
    assert!(ds.len() >= 4);
    for d in ds {
        assert!(d["key"].is_object());
        assert!(d["outputs"].is_array());
        assert!(d["parameters"].is_object());
    }
    assert_eq!(
        descriptors["body"]["registry_id"],
        validation["body"]["registry_id"]
    );
    let result = call("backtest.run", p.clone());
    assert_eq!(result["ok"], true, "{result}");
    let report = &result["body"]["report"];
    assert_eq!(report["metrics"]["final_equity"], "97.00");
    assert_eq!(report["metrics"]["fees"], "1.00");
    assert_eq!(report["fills"].as_array().unwrap().len(), 2);
    assert_eq!(report["manifest"]["seed"], "18446744073709551615");
    let direct = prepare(&p).unwrap().run().unwrap();
    assert_eq!(result["body"]["result_id"], direct.content_id().to_string());
    assert_eq!(serde_json::to_vec(report).unwrap(), direct.json());
}
#[test]
fn workflow_and_margin_dispatch_remain_engine_owned() {
    let mut p = params();
    p["kind"] = "workflow".into();
    p["model"] = include_str!("../../fqe-runtime/tests/fixtures/p3/backtest-workflow.json").into();
    p["policies"][1]["source"] =
        json!({"kind":"workflow_boolean","instance":"exit_rule","output":"exit"});
    // Use actual workflow output identity from the artifact, never rewrite the model for a transport.
    let model: Value = serde_json::from_str(p["model"].as_str().unwrap()).unwrap();
    let instance = model["instances"][0]["id"].as_str().unwrap();
    p["policies"][1]["source"]["instance"] = instance.into();
    let r = call("backtest.run", p.clone());
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["body"]["report"]["metrics"]["final_equity"], "97.00");
    p["account"]["profile"] = "margin_long".into();
    p["account"]["risk"] = json!({"max_leverage_bps":"20000","maintenance_bps":"2500"});
    let r = call("backtest.run", p);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["body"]["report"]["metrics"]["final_equity"], "97.00");
}
#[test]
fn financial_preflight_and_precancel_are_explicit() {
    let original = params();
    for (key, bad) in [
        ("seed", json!(18446744073709551615u64)),
        ("seed", json!("01")),
        ("end_ns", json!("1704153600000000000")),
        ("warmup_bars", json!("4294967296")),
        ("kind", json!("python")),
    ] {
        let mut p = original.clone();
        p[key] = bad;
        let r = call("backtest.run", p);
        assert_eq!(r["ok"], false, "{key}: {r}");
    }
    let mut p = original.clone();
    p["account"]["risk"] = json!({});
    assert_eq!(call("backtest.run", p)["ok"], false);
    let mut p = original.clone();
    p["policies"][0]["source"] = json!({"kind":"event_boolean","name":"unknown"});
    assert_eq!(call("backtest.run", p)["ok"], false);
    let r: Value = serde_json::from_slice(&dispatch(
        &request("backtest.run", original),
        &AtomicBool::new(true),
    ))
    .unwrap();
    assert_eq!(r["ok"], true);
    assert_eq!(r["body"]["report"]["terminal"], "cancelled");
    assert_eq!(r["body"]["report"]["processed_source_bars"], "0");
    assert_eq!(r["body"]["report"]["metrics"]["final_equity"], "100.00");
}
#[test]
fn framing_rejects_quota_truncation_and_preserves_multiple_frames() {
    for input in [
        vec![0],
        vec![0, 0, 0, 0],
        ((MAX_REQUEST + 1) as u32).to_be_bytes().to_vec(),
        vec![0, 0, 0, 2, 1],
    ] {
        assert!(read_frame(Cursor::new(input)).is_err());
    }
    assert!(read_frame(Cursor::new([])).unwrap().is_none());
    let mut bytes = vec![];
    write_frame(&mut bytes, b"first").unwrap();
    write_frame(&mut bytes, b"second").unwrap();
    let mut input = Cursor::new(bytes);
    assert_eq!(read_frame(&mut input).unwrap().unwrap(), b"first");
    assert_eq!(read_frame(&mut input).unwrap().unwrap(), b"second");
    assert!(write_frame(vec![], &vec![0; MAX_RESPONSE + 1]).is_err());
}
#[test]
fn native_owned_pool_and_task_quotas_cleanup_invalid_handles() {
    assert_eq!(fqe_bridge_abi(), 1);
    assert_eq!(fqe_buffer_new(0), 0);
    assert_eq!(fqe_buffer_new(MAX_REQUEST + 1), 0);
    assert_eq!(fqe_buffer_free(1), 0);
    let mut buffers = vec![];
    for _ in 0..16 {
        let address = fqe_buffer_new(32);
        assert_ne!(address, 0);
        buffers.push(address);
    }
    assert_eq!(fqe_buffer_new(1), 0);
    let mut tasks = vec![];
    for _ in 0..8 {
        let t = fqe_task_new();
        assert_ne!(t, 0);
        tasks.push(t);
    }
    assert_eq!(fqe_task_new(), 0);
    assert_eq!(fqe_call(0, 0, tasks[0]), 0);
    assert_eq!(fqe_call(buffers[0], 31, tasks[0]), 0);
    assert_eq!(fqe_call(buffers[0], 32, 0), 0);
    assert_eq!(fqe_task_cancel(tasks[0]), 1);
    for task in tasks {
        assert_eq!(fqe_task_free(task), 1);
        assert_eq!(fqe_task_free(task), 0);
    }
    for address in buffers {
        assert_eq!(fqe_buffer_len(address), 32);
        assert_eq!(fqe_buffer_free(address), 1);
        assert_eq!(fqe_buffer_free(address), 0);
    }
    assert_eq!(fqe_task_count(), 0);
    assert_eq!(fqe_buffer_count(), 0);
}
