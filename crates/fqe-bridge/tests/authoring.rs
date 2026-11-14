use fqe_bridge::{
    authoring::{Draft, parse, print},
    dispatch,
};
use fqe_ir::event_plan::LoadedEventPlan;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
#[path = "../../fqe-runtime/tests/support/gate_providers.rs"]
mod independent_providers;
mod support;
const SOURCE: &str = include_str!("../../../examples/desktop/sma-cross.fqe");
#[test]
fn source_compiles_through_native_descriptor_types_and_keeps_source_map() {
    let doc = parse(SOURCE).unwrap();
    assert_eq!(
        doc.semantic["model"]["graph"]["graph"]["nodes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(doc.semantic["policies"].as_array().unwrap().len(), 2);
    assert_eq!(doc.comments.len(), 1);
    assert!(
        doc.source_map
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "node.fast")
    );
}
fn call(method: &str, params: Value) -> Value {
    let bytes=serde_json::to_vec(&json!({"format":"fqe.desktop-request","schema_version":1,"id":"author.test","method":method,"params":params})).unwrap();
    serde_json::from_slice(&dispatch(&bytes, &AtomicBool::new(false))).unwrap()
}
fn identity(model: &Value) -> String {
    LoadedEventPlan::from_bytes(&serde_json::to_vec(model).unwrap())
        .unwrap()
        .content_id()
        .to_string()
}
#[test]
fn independently_authored_native_graph_matches_text_and_round_trip() {
    let doc = parse(SOURCE).unwrap();
    let graph: Value = serde_json::from_str(include_str!("fixtures/sma-cross-model.json")).unwrap();
    assert_eq!(identity(&graph), identity(&doc.semantic["model"]));
    let mut semantic = doc.semantic.clone();
    semantic["model"] = graph;
    let source = print(&semantic, &doc.comments).unwrap();
    let round = parse(&source).unwrap();
    assert_eq!(
        identity(&round.semantic["model"]),
        identity(&semantic["model"])
    );
    assert_eq!(round.semantic["policies"], doc.semantic["policies"]);
    assert_eq!(round.comments, doc.comments);
    assert_eq!(print(&round.semantic, &round.comments).unwrap(), source);
    semantic["model"]["graph"]["metadata"] = json!({"must_preserve":"original"});
    assert_eq!(
        print(&semantic, &[]).unwrap_err().code,
        "authoring_unrepresentable"
    );
}
#[test]
fn actual_cash_run_obeys_independent_crosses_next_open_fills_and_ledger() {
    let doc = parse(SOURCE).unwrap();
    let mut p = support::params();
    p["model"] = serde_json::to_string(&doc.semantic["model"])
        .unwrap()
        .into();
    p["parameters"] = "{\"version\":1,\"values\":{}}".into();
    p["policies"] = doc.semantic["policies"].clone();
    p["csv"] = include_str!("fixtures/sma-cross-history.csv").into();
    p["history_spec"] = include_str!("fixtures/sma-cross-history-spec.json").into();
    p["end_ns"] = "1704154080000000000".into();
    p["account"]["minimum_fee"] = "0.00".into();
    let result = call("backtest.run", p.clone());
    assert_eq!(result["ok"], true, "{result}");
    let r = &result["body"]["report"];
    assert_eq!(r["metrics"]["initial_equity"], "100.00");
    assert_eq!(r["metrics"]["final_equity"], "98.00");
    assert_eq!(r["metrics"]["net_profit"], "-2.00");
    assert_eq!(r["metrics"]["fees"], "0.00");
    assert_eq!(r["account"]["cash"], "98.00");
    assert_eq!(r["account"]["quantity"], "0.000");
    let fills = r["fills"].as_array().unwrap();
    assert_eq!(fills.len(), 2);
    assert_eq!(fills[0]["source_bar"], "6");
    assert_eq!(fills[0]["price"], "4.00");
    assert_eq!(fills[1]["source_bar"], "8");
    assert_eq!(fills[1]["price"], "2.00");
    // The separately authored graph must produce identical economic outputs.
    p["model"] = include_str!("fixtures/sma-cross-model.json").into();
    let graph = call("backtest.run", p);
    // Raw model source provenance legitimately differs; compare economic outputs.
    assert_eq!(graph["ok"], true, "{graph}");
    for field in ["metrics", "account", "valuation", "equity_curve"] {
        assert_eq!(r[field], graph["body"]["report"][field], "{field}");
    }
}
#[test]
fn invalid_buffers_and_stale_transactions_cannot_run_the_previous_model() {
    let mut d = Draft::new(SOURCE).unwrap();
    let original = d.executable().unwrap().semantic.clone();
    d.edit_text("fqe 1; invalid").unwrap();
    assert!(d.apply(0, 0).is_err());
    assert!(d.apply(0, 1).is_err());
    assert_eq!(d.buffer(), "fqe 1; invalid");
    assert!(d.executable().is_err());
    assert!(d.edit_graph(0, 1, &original).is_err());
    let restored = Draft::restore(&d.snapshot()).unwrap();
    assert_eq!(restored.buffer(), "fqe 1; invalid");
    assert!(restored.executable().is_err());
    let changed = SOURCE.replace("= \"2\" within", "= \"1\" within");
    d.edit_text(&changed).unwrap();
    d.apply(0, 2).unwrap();
    assert_eq!(d.revision(), 1);
    assert!(d.executable().is_ok());
    assert!(d.edit_graph(0, 2, &original).is_err());
    d.edit_graph(1, 2, &original).unwrap();
    assert_eq!(d.revision(), 2);
    d.edit_text(&(String::from("// comment-only revision\n") + d.buffer()))
        .unwrap();
    d.apply(2, 4).unwrap();
    assert_eq!(d.revision(), 2);
}
#[test]
fn generic_u64_nodes_keep_full_width_values_and_lexical_order_is_not_topology() {
    let source = r#"fqe 1;
strategy "generic" version "1";
context tick "DEMO";
param limit : u64 = "18446744073709551615";
node z = "fqe.constant.u64" @ "1" (value=$limit) ();
node a = "fqe.identity.u64" @ "1" () (value=z.result);
output check = equal(a.result, z.result);
"#;
    let d = parse(source).unwrap();
    let printed = print(&d.semantic, &[]).unwrap();
    assert!(printed.find("node z").unwrap() < printed.find("node a").unwrap());
    assert!(printed.contains("18446744073709551615"));
    assert_eq!(
        identity(&d.semantic["model"]),
        identity(&parse(&printed).unwrap().semantic["model"])
    );
}
#[test]
fn rpc_diagnostics_and_source_locations_are_data_not_a_fake_success() {
    let ok = call(
        "authoring.inspect",
        json!({"source":SOURCE,"parameters":"{\"version\":1,\"values\":{}}"}),
    );
    assert_eq!(ok["body"]["valid"], true, "{ok}");
    let bad = call(
        "authoring.inspect",
        json!({"source":SOURCE.replace("above(","unknown("),"parameters":"{\"version\":1,\"values\":{}}"}),
    );
    assert_eq!(bad["body"]["valid"], false);
    assert!(bad["body"].get("native").is_none());
    let diagnostic = &bad["body"]["diagnostics"][0];
    assert!(
        diagnostic["line"]
            .as_str()
            .unwrap()
            .parse::<usize>()
            .unwrap()
            > 1
    );
    assert_eq!(diagnostic["file"], "strategy.fqe");
    assert_eq!(
        call("authoring.inspect", json!({"source":SOURCE}))["ok"],
        false
    );
}
#[test]
fn malformed_types_unknown_providers_cycles_and_budget_fail_closed() {
    for source in [
        SOURCE.replace("reference.sma", "unknown.function"),
        SOURCE.replace("period=$fast_period", "period=\"0\""),
        SOURCE.replace("(close=sample)", "(close=missing.value)"),
        SOURCE.replace("param fast_period : u64", "param fast_period : bool"),
        SOURCE.replace("output enter = cross(up)", "output enter = fast.value"),
        SOURCE.replace(
            "policy exit = close_long(exit);",
            "policy exit = sell_short(exit);",
        ),
        SOURCE.replace(
            "// One semantic strategy, editable as graph or structured text.",
            "/* unknown comment */",
        ),
        String::from(SOURCE) + "param fast_period : u64 = \"2\";\n",
        "x".repeat(65537),
        String::from(SOURCE) + "\"unterminated",
    ] {
        assert!(parse(&source).is_err(), "must reject {source}");
    }
}

#[test]
fn native_errors_locate_nodes_and_explicit_discard_does_not_execute() {
    let invalid = SOURCE.replace("period=$fast_period", "period=\"0\"");
    let error = parse(&invalid).unwrap_err();
    assert!(error.offset > 0, "{error:?}");
    let mut draft = Draft::new(SOURCE).unwrap();
    draft.edit_text("incomplete buffer").unwrap();
    let saved = draft.snapshot();
    let mut restored = Draft::restore(&saved).unwrap();
    assert!(restored.discard_buffer(0, 0).is_err());
    restored.discard_buffer(0, 1).unwrap();
    assert_eq!(restored.buffer(), SOURCE);
    assert!(restored.executable().is_ok());
    let mut corrupted = saved;
    corrupted["pending"] = false.into();
    assert!(Draft::restore(&corrupted).is_err());
}

#[test]
fn decimal_boolean_parameters_comments_and_exact_constraints_round_trip() {
    let source=SOURCE.replace("input sample = close;",r#"
param threshold : decimal(12, ratio) = "60.000000000000" within ("0.000000000000", "100.000000000000") description "RSI threshold";
param enabled : bool = true;
input sample = close;
node strength = "reference.rsi" @ "1" (period="2") (close=sample);
// This comment is retained, including quoted text "and a // marker".
"#).replace("output enter = cross(up);","output enter = and(and(cross(up), greater(strength.value, $threshold)), $enabled);");
    let d = parse(&source).unwrap();
    assert_eq!(
        d.semantic["model"]["graph"]["parameters"]["entries"][2]["default"]["value"],
        "60000000000000"
    );
    let printed = print(&d.semantic, &d.comments).unwrap();
    let restored = parse(&printed).unwrap();
    assert_eq!(restored.comments, d.comments);
    assert_eq!(
        identity(&restored.semantic["model"]),
        identity(&d.semantic["model"])
    );
    assert!(printed.contains("60.000000000000"));
    assert!(parse(&source.replace("60.000000000000", "100.000000000001")).is_err());
    assert!(print(&d.semantic, &[String::from("// safe\nfqe 2;")]).is_err());
}

#[test]
fn caller_registered_unknown_names_and_ports_need_no_frontend_branch_or_evaluation() {
    use std::sync::atomic::Ordering;
    let (registry, calls) = independent_providers::registry();
    let source = r#"fqe 1;
strategy "custom-registry" version "1";
context tick "DEMO";
input sample = price;
node shifted = "acceptance.shift" @ "1" (increment="2") (sample=sample);
node check = "acceptance.window" @ "1" (lower="0", upper="10") (sample=shifted.result);
output enter = check.result;
policy buy = buy_if_flat(enter, "1", "10");
"#;
    let d = fqe_bridge::authoring::parse_with_registry(source, &registry).unwrap();
    assert_eq!(
        d.semantic["model"]["graph"]["graph"]["nodes"][1]["outputs"][0]["type"]["kind"],
        "bool"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert!(
        parse(source).is_err(),
        "desktop reference catalog must not load external providers"
    );
    assert!(
        fqe_bridge::authoring::parse_with_registry(
            &source.replace("sample=shifted.result", "missing=shifted.result"),
            &registry
        )
        .is_err()
    );
}
