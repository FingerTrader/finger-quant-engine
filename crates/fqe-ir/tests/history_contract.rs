use fqe_ir::history::{HistoricalDataset, HistorySpec};
use std::sync::Arc;
const SPEC: &[u8] = include_bytes!("fixtures/history/cash-long-spec.json");
const CSV: &[u8] = include_bytes!("fixtures/history/cash-long.csv");
fn spec(text: &[u8]) -> Arc<HistorySpec> {
    Arc::new(HistorySpec::from_bytes(text).unwrap())
}
fn load() -> Arc<HistoricalDataset> {
    Arc::new(HistoricalDataset::from_csv(spec(SPEC), CSV, None).unwrap())
}
#[test]
fn independent_python_identity_fixture_and_exact_ohlc_scales_survive_immutable_range_selection() {
    let expected: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/history/independent-identities.json"
    ))
    .unwrap();
    let data = load();
    for (name, actual) in [
        ("spec_source", data.spec().source_id()),
        ("spec_id", data.spec().content_id()),
        ("source", data.source_id()),
        ("bars_id", data.bars_id()),
        ("dataset_id", data.content_id()),
    ] {
        assert_eq!(actual.to_string(), expected[name].as_str().unwrap());
    }
    assert_eq!(data.bars().len(), 4);
    assert_eq!(
        data.bars()[1].input().open.numeric_coefficient().unwrap(),
        12000
    );
    assert_eq!(
        data.bars()[0].input().volume.numeric_coefficient().unwrap(),
        100000
    );
    let selection = data
        .select(
            data.bars()[1].input().open_ns,
            data.bars()[3].input().close_ns,
            1,
        )
        .unwrap();
    assert_eq!(
        selection.content_id().to_string(),
        expected["selection_id"].as_str().unwrap()
    );
    assert_eq!(selection.bars().len(), 4);
    assert_eq!(selection.warmup_bars(), 1);
    assert!(selection.contiguous());
    assert_eq!(data.bars().len(), 4);
    assert!(
        data.select(
            data.bars()[0].input().open_ns,
            data.bars()[3].input().close_ns,
            1
        )
        .is_err()
    );
}
#[test]
fn malformed_duplicate_backward_gap_unclosed_step_precision_and_unknown_metadata_fail_without_repair()
 {
    let s = spec(SPEC);
    let csv = String::from_utf8(CSV.to_vec()).unwrap();
    let first = csv.lines().nth(1).unwrap();
    for changed in [
        csv.replace(first, ""),
        format!("{csv}{first}\n"),
        csv.replacen("100.00", "100.001", 1),
        csv.replacen(",true", ",false", 1),
        csv.replacen("DEMO,", "OTHER,", 1),
        csv.replacen("00:01:00Z", "00:02:00Z", 1),
        csv.replacen("100.000", "-1.000", 1),
        csv.replacen("100.00", "NaN", 1),
    ] {
        assert!(HistoricalDataset::from_csv(Arc::clone(&s), changed.as_bytes(), None).is_err());
    }
    let mut value: serde_json::Value = serde_json::from_slice(SPEC).unwrap();
    value["secret_unknown"] = true.into();
    assert!(HistorySpec::from_bytes(&serde_json::to_vec(&value).unwrap()).is_err());
    value.as_object_mut().unwrap().remove("secret_unknown");
    value["adjustment"] = "adjusted-verified".into();
    assert!(HistorySpec::from_bytes(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn revision_parent_and_source_content_changes_are_exact_and_never_overwrite_previous_dataset() {
    let previous = load();
    let mut value: serde_json::Value = serde_json::from_slice(SPEC).unwrap();
    value["source"]["revision"] = "2".into();
    value["source"]["parent"] = previous.content_id().to_string().into();
    let revised_spec = spec(&serde_json::to_vec(&value).unwrap());
    let changed = String::from_utf8(CSV.to_vec())
        .unwrap()
        .replace("100.00,101.00,99.00,100.00", "100.00,102.00,99.00,100.00");
    assert!(
        HistoricalDataset::from_csv(Arc::clone(&revised_spec), changed.as_bytes(), None).is_err()
    );
    let revised =
        HistoricalDataset::from_csv(revised_spec, changed.as_bytes(), Some(&previous)).unwrap();
    assert_ne!(revised.content_id(), previous.content_id());
    assert_ne!(revised.bars_id(), previous.bars_id());
    assert_eq!(
        previous.bars()[0]
            .input()
            .high
            .numeric_coefficient()
            .unwrap(),
        10100
    );
    let formatted = String::from_utf8(CSV.to_vec())
        .unwrap()
        .replace("100.00", "100.0000");
    let same = HistoricalDataset::from_csv(spec(SPEC), formatted.as_bytes(), None).unwrap();
    assert_eq!(same.bars_id(), previous.bars_id());
    assert_ne!(same.source_id(), previous.source_id());
    assert_ne!(same.content_id(), previous.content_id());
}
#[test]
fn signed_history_and_declared_session_gaps_are_validated_without_claiming_continuous_runtime_support()
 {
    let mut v: serde_json::Value = serde_json::from_slice(SPEC).unwrap();
    v["profile"] = "fqe.signed-history/1".into();
    v["instrument"]["price_policy"] = "signed".into();
    let csv = String::from_utf8(CSV.to_vec())
        .unwrap()
        .replace("100.00,101.00,99.00,100.00", "-1.00,0.00,-2.00,-1.00");
    let signed =
        HistoricalDataset::from_csv(spec(&serde_json::to_vec(&v).unwrap()), csv.as_bytes(), None)
            .unwrap();
    assert_eq!(
        signed.bars()[0]
            .input()
            .close
            .numeric_coefficient()
            .unwrap(),
        -100
    );
    assert!(HistoricalDataset::from_csv(spec(SPEC), csv.as_bytes(), None).is_err());
    v["calendar"]["sessions"] = serde_json::json!([{"open":"2024-01-02T00:00:00Z","close":"2024-01-02T00:02:00Z"},{"open":"2024-01-02T00:03:00Z","close":"2024-01-02T00:04:00Z"}]);
    let csv = String::from_utf8(CSV.to_vec())
        .unwrap()
        .lines()
        .enumerate()
        .filter(|(n, _)| *n != 3)
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let data = Arc::new(
        HistoricalDataset::from_csv(spec(&serde_json::to_vec(&v).unwrap()), csv.as_bytes(), None)
            .unwrap(),
    );
    assert_eq!(data.bars().len(), 3);
    assert!(
        !data
            .select(
                data.bars()[0].input().open_ns,
                data.bars()[2].input().close_ns,
                0
            )
            .unwrap()
            .contiguous()
    );
}
#[test]
fn calendar_acquisition_profile_precision_and_quota_boundaries_fail_before_import() {
    let original: serde_json::Value = serde_json::from_slice(SPEC).unwrap();
    for (path, value) in [
        ("/schema_version", serde_json::json!(2)),
        (
            "/calendar/sessions/0/close",
            serde_json::json!("2024-01-02T00:04:01Z"),
        ),
        (
            "/calendar/sessions/0/close",
            serde_json::json!("2025-01-02T00:04:00Z"),
        ),
        (
            "/calendar/sessions/0/open",
            serde_json::json!("2024-01-02T00:00:00+09:00"),
        ),
        ("/acquired_at", serde_json::json!("2024-01-02T00:03:00Z")),
        (
            "/performance_start",
            serde_json::json!("2024-01-02T00:00:01Z"),
        ),
        ("/instrument/price_scale", serde_json::json!(13)),
        ("/instrument/price_step", serde_json::json!("0.00")),
        ("/instrument/minimum_quantity", serde_json::json!("0.0001")),
        ("/instrument/price_policy", serde_json::json!("signed")),
        ("/source/revision", serde_json::json!("0")),
        (
            "/source/parent",
            serde_json::json!(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            ),
        ),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = value;
        assert!(
            HistorySpec::from_bytes(&serde_json::to_vec(&v).unwrap()).is_err(),
            "{path}"
        );
    }
    let mut v = original;
    let s = v["calendar"]["sessions"][0].clone();
    v["calendar"]["sessions"].as_array_mut().unwrap().push(s);
    assert!(HistorySpec::from_bytes(&serde_json::to_vec(&v).unwrap()).is_err());
}
#[test]
fn csv_quoting_and_crlf_preserve_values_but_oversized_rows_and_bad_quotes_are_rejected() {
    let original = load();
    let csv = String::from_utf8(CSV.to_vec()).unwrap();
    let quoted = csv.replace("DEMO,", "\"DEMO\",").replace('\n', "\r\n");
    let parsed = HistoricalDataset::from_csv(spec(SPEC), quoted.as_bytes(), None).unwrap();
    assert_eq!(parsed.bars_id(), original.bars_id());
    assert_ne!(parsed.content_id(), original.content_id());
    for bad in [
        csv.replacen("DEMO,", "\"DEMO,", 1),
        csv.replacen("DEMO,", "\"DEMO\"oops,", 1),
        csv.replacen("DEMO,", &(String::from("X").repeat(8193) + ","), 1),
        csv.replace("100.00", "1e2"),
        csv.replace("100.00", "0100.00"),
    ] {
        assert!(HistoricalDataset::from_csv(spec(SPEC), bad.as_bytes(), None).is_err());
    }
}
