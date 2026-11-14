use fqe_core::{
    numeric::NumericError,
    types::{Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{
    catalog,
    event_plan::LoadedEventPlan,
    exchange::{
        self, MarketData, MarketRecord, ParameterBindings, ReportContext, ReportObservation,
        ReportSummary, ReportWriter,
    },
    model::ContentId,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
const TICK: &[u8] = include_bytes!("../../fqe-cli/tests/fixtures/tick-data.json");
const BAR: &[u8] = include_bytes!("../../fqe-cli/tests/fixtures/g002-data.json");
const MODEL: &[u8] = include_bytes!("fixtures/tick-events.json");
fn changed(bytes: &[u8], f: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut v = serde_json::from_slice(bytes).unwrap();
    f(&mut v);
    serde_json::to_vec(&v).unwrap()
}
fn plan() -> fqe_ir::event_plan::CompiledEventPlan {
    let a = LoadedEventPlan::from_bytes(MODEL).unwrap();
    a.compile(&catalog::reference(a.trigger()).unwrap(), vec![])
        .unwrap()
}
#[test]
fn raw_source_hash_and_full_width_tick_integers_are_exact() {
    let data = MarketData::from_bytes(TICK).unwrap();
    assert_eq!(
        data.source_id().to_string(),
        include_str!("../../fqe-cli/tests/fixtures/tick-data.identity.txt").trim()
    );
    let MarketRecord::Tick(t) = &data.events()[1] else {
        panic!("tick")
    };
    assert_eq!(t.sequence(), u64::MAX);
    assert_eq!(t.timestamp_ns(), u64::MAX);
    assert_eq!(t.tick().price.get(), u64::MAX);
    let whitespace = [TICK, b" "].concat();
    let copy = MarketData::from_bytes(&whitespace).unwrap();
    assert_eq!(copy.events(), data.events());
    assert_ne!(copy.source_id(), data.source_id());
}
#[test]
fn typed_closed_bar_fields_and_source_reference_are_preserved() {
    let data = MarketData::from_bytes(BAR).unwrap();
    assert_eq!(data.events().len(), 6);
    assert_eq!(
        data.source_id().to_string(),
        include_str!("../../fqe-cli/tests/fixtures/g002-data.identity.txt").trim()
    );
    let MarketRecord::Bar(b) = &data.events()[4] else {
        panic!("bar")
    };
    assert_eq!(
        b.input().close.numeric_coefficient().unwrap(),
        4_000_000_000_000
    );
    assert_eq!(b.input().volume.numeric_coefficient().unwrap(), 0);
    assert_eq!(b.input().open_ns, 4000);
    assert_eq!(b.input().close_ns, 5000);
}
#[test]
fn data_versions_unknown_duplicate_fields_and_noncanonical_integers_reject() {
    let mutations: [fn(&mut Value); 9] = [
        |v| v["schema_version"] = json!(2),
        |v| v["format"] = json!("other"),
        |v| v["extra"] = json!(null),
        |v| v["events"][0]["extra"] = json!(true),
        |v| v["events"][0]["kind"] = json!("future"),
        |v| v["events"][0]["price"] = json!(1),
        |v| v["events"][0]["sequence"] = json!("01"),
        |v| v["events"][0]["timestamp_ns"] = json!("-1"),
        |v| v["events"][0]["price"] = json!("0"),
    ];
    for f in mutations {
        assert!(MarketData::from_bytes(&changed(TICK, f)).is_err());
    }
    for bytes in [
        b"".as_slice(),
        b" ",
        b"{}",
        b"{\"format\":1,\"format\":2}",
        b"{\"events\":[{}]}",
    ] {
        assert!(MarketData::from_bytes(bytes).is_err());
    }
    assert!(MarketData::from_bytes(&changed(TICK, |v| v["events"][0] = json!({}))).is_err());
}
#[test]
fn record_bar_validation_precedes_runtime() {
    let mutations: [fn(&mut Value); 8] = [
        |v| v["events"][0]["closed"] = json!(false),
        |v| v["events"][0]["timestamp_ns"] = json!("999"),
        |v| v["events"][0]["timeframe_ns"] = json!("0"),
        |v| v["events"][0]["high"]["value"] = json!("0"),
        |v| v["events"][0]["volume"]["value"] = json!("-1"),
        |v| v["events"][0]["low"]["type"]["unit"]["kind"] = json!("quantity"),
        |v| v["events"][0]["open"]["type"]["scale"] = json!(11),
        |v| v["events"][0]["sequence"] = json!("0"),
    ];
    for f in mutations {
        assert_eq!(
            MarketData::from_bytes(&changed(BAR, f)).err().unwrap().code,
            "data_bar"
        );
    }
}
#[test]
fn explicit_empty_dataset_is_valid_but_record_and_shared_budgets_are_bounded() {
    let empty = MarketData::from_bytes(include_bytes!(
        "../../fqe-cli/tests/fixtures/empty-data.json"
    ))
    .unwrap();
    assert!(empty.events().is_empty());
    let byte_limit = vec![b' '; exchange::MAX_FILE_BYTES + 1];
    assert_eq!(
        MarketData::from_bytes(&byte_limit).err().unwrap().code,
        "resource_limit"
    );
    let many = changed(TICK, |v| {
        v["events"] = Value::Array(vec![v["events"][0].clone(); 1025])
    });
    assert_eq!(
        MarketData::from_bytes(&many).err().unwrap().code,
        "resource_limit"
    );
    let depth = format!("{}0{}", "[".repeat(34), "]".repeat(34));
    assert!(MarketData::from_bytes(depth.as_bytes()).is_err());
}
#[test]
fn bindings_use_existing_typed_document_and_reject_malformed_payloads() {
    let good = include_bytes!("../../fqe-cli/tests/fixtures/parameters-90.json");
    let params = ParameterBindings::from_bytes(good).unwrap();
    let a = LoadedEventPlan::from_bytes(include_bytes!("fixtures/g002-events.json")).unwrap();
    a.compile(
        &catalog::reference(a.trigger()).unwrap(),
        params.into_values(),
    )
    .unwrap();
    for f in [
        |v: &mut Value| v["version"] = json!(2),
        |v: &mut Value| v["extra"] = json!(true),
        |v: &mut Value| v["values"]["threshold"]["value"] = json!(90),
        |v: &mut Value| v["values"]["threshold"]["value"] = json!("090"),
    ] {
        assert!(ParameterBindings::from_bytes(&changed(good, f)).is_err());
    }
    assert!(
        ParameterBindings::from_bytes(br#"{"version":1,"values":{"x":null,"x":null}}"#).is_err()
    );
    let params =
        ParameterBindings::from_bytes(br#"{"version":1,"values":{"absent":null}}"#).unwrap();
    assert!(
        a.compile(
            &catalog::reference(a.trigger()).unwrap(),
            params.into_values()
        )
        .is_err()
    );
}
#[test]
fn validation_result_json_is_typed_and_deterministic() {
    let p = plan();
    let a = exchange::validation_result(&p, None).unwrap();
    assert_eq!(a, exchange::validation_result(&p, None).unwrap());
    let value: Value = serde_json::from_slice(&a).unwrap();
    assert_eq!(value["format"], "fqe.validation-result");
    assert_eq!(value["status"], "valid");
    assert_eq!(value["outputs"][0]["type"]["kind"], "bool");
    assert_eq!(value["manifest"]["warmup_samples"], "0");
    assert!(value["manifest"]["parameters_source_id"].is_null());
    assert!(value.get("observations").is_none());
}
fn writer(p: &fqe_ir::event_plan::CompiledEventPlan) -> ReportWriter {
    ReportWriter::new(ReportContext {
        plan: p,
        run: "test",
        dataset_id: ContentId::from_bytes([8; 32]),
        parameters_source_id: None,
        engine_version: "0.1.0",
        interval_convention: "test",
    })
    .unwrap()
}
#[test]
fn report_json_preserves_readiness_and_precision() {
    let p = plan();
    let mut w = writer(&p);
    let t = TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).unwrap();
    let outputs = BTreeMap::from([
        (
            "ready".into(),
            Evaluation::Ready(TypedValue::new(t, ScalarValue::U64(u64::MAX)).unwrap()),
        ),
        ("missing".into(), Evaluation::Missing),
        ("warmup".into(), Evaluation::Warmup),
    ]);
    w.push(ReportObservation {
        source_sequence: u64::MAX,
        timestamp_ns: u64::MAX,
        matched: true,
        warmup: true,
        outputs: &outputs,
        signals: vec![],
    })
    .unwrap();
    let b = w
        .finish(ReportSummary {
            processed_events: 1,
            matched_events: 1,
            last_sequence: Some(u64::MAX),
            last_timestamp_ns: Some(u64::MAX),
        })
        .unwrap();
    let v: Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(
        v["observations"][0]["outputs"]["ready"]["value"]["value"],
        u64::MAX.to_string()
    );
    assert_eq!(
        v["observations"][0]["outputs"]["missing"],
        json!({"status":"missing"})
    );
    assert_eq!(v["summary"]["signals"], "0");
    assert_eq!(v["summary"]["last_sequence"], u64::MAX.to_string());
}
#[test]
fn failed_output_writer_cannot_be_finished_or_reused() {
    let p = plan();
    let mut w = writer(&p);
    let outputs = BTreeMap::from([("bad".into(), Evaluation::Error(NumericError::Overflow))]);
    assert!(
        w.push(ReportObservation {
            source_sequence: 1,
            timestamp_ns: 1,
            matched: true,
            warmup: false,
            outputs: &outputs,
            signals: vec![]
        })
        .is_err()
    );
    assert_eq!(
        w.push(ReportObservation {
            source_sequence: 1,
            timestamp_ns: 1,
            matched: true,
            warmup: false,
            outputs: &BTreeMap::new(),
            signals: vec![]
        })
        .unwrap_err()
        .code,
        "report_state"
    );
    assert_eq!(
        w.finish(ReportSummary {
            processed_events: 0,
            matched_events: 0,
            last_sequence: None,
            last_timestamp_ns: None
        })
        .unwrap_err()
        .code,
        "report_state"
    );
}
#[test]
fn report_size_is_bounded_and_summary_must_match() {
    let p = plan();
    let mut w = writer(&p);
    let t =
        TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Text { max_bytes: 4096 }).unwrap();
    let value = TypedValue::new(t, ScalarValue::Text("x".repeat(4096))).unwrap();
    let outputs = (0..32)
        .map(|i| (format!("out{i}"), Evaluation::Ready(value.clone())))
        .collect();
    let mut rejected = false;
    for n in 1..=40 {
        if w.push(ReportObservation {
            source_sequence: n,
            timestamp_ns: n,
            matched: true,
            warmup: false,
            outputs: &outputs,
            signals: vec![],
        })
        .is_err()
        {
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    assert!(
        w.finish(ReportSummary {
            processed_events: 40,
            matched_events: 40,
            last_sequence: Some(40),
            last_timestamp_ns: Some(40)
        })
        .is_err()
    );
    assert_eq!(
        writer(&p)
            .finish(ReportSummary {
                processed_events: 1,
                matched_events: 0,
                last_sequence: None,
                last_timestamp_ns: None
            })
            .unwrap_err()
            .code,
        "report_summary"
    );
}
#[test]
fn diagnostics_encode_only_supplied_public_context() {
    let d = exchange::diagnostic("data", "json", "$", "invalid input");
    let v: Value = serde_json::from_slice(&d).unwrap();
    assert_eq!(v["format"], "fqe.diagnostic");
    assert_eq!(v["message"], "invalid input");
    assert_eq!(v["schema_version"], 1);
}

#[test]
fn reference_catalog_rejects_invalid_public_trigger_values() {
    let instrument = fqe_core::InstrumentId::new("DEMO").unwrap();
    for (timeframe_ns, price_scale, volume_scale) in
        [(0, 12, 0), (1000, 13, 0), (1000, 12, 19), (1000, 255, 0)]
    {
        assert_eq!(
            catalog::reference(&fqe_ir::event_plan::Trigger::ClosedBar {
                instrument: instrument.clone(),
                timeframe_ns,
                price_scale,
                volume_scale
            })
            .err()
            .unwrap()
            .code,
            "catalog_trigger"
        );
    }
}
