use fqe_ir::{
    catalog,
    event_plan::LoadedEventPlan,
    exchange::{self, MarketData, ParameterBindings},
};
use fqe_runtime::report::run_report;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn fixture(name: &str) -> PathBuf {
    root().join("crates/fqe-cli/tests/fixtures").join(name)
}
fn model(name: &str) -> PathBuf {
    root().join("crates/fqe-ir/tests/fixtures").join(name)
}
fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fqe"))
}
fn run(model: &Path, data: &Path, params: Option<&Path>) -> Output {
    let mut c = command();
    c.arg("run")
        .arg("--model")
        .arg(model)
        .arg("--data")
        .arg(data)
        .args(["--run-id", "acceptance"]);
    if let Some(p) = params {
        c.arg("--params").arg(p);
    }
    c.output().unwrap()
}
fn success(o: &Output) -> String {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(o.stderr.is_empty());
    let s = String::from_utf8(o.stdout.clone()).unwrap();
    assert_eq!(s.lines().count(), 1);
    assert!(s.ends_with("}\n"));
    s
}
fn failure(o: &Output, exit: i32, stage: &str) -> String {
    assert_eq!(
        o.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.stdout.is_empty());
    let s = String::from_utf8(o.stderr.clone()).unwrap();
    assert_eq!(s.lines().count(), 1);
    assert!(s.contains("\"format\":\"fqe.diagnostic\""));
    assert!(s.contains(&format!("\"stage\":\"{stage}\"")));
    s
}
fn signal5(s: &str) -> bool {
    s.contains("\"instrument\":\"DEMO\",\"kind\":\"long\",\"source_sequence\":\"5\",\"timestamp_ns\":\"5000\"")
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fqe-cli-p109-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let p = self.0.join(name);
        fs::write(&p, bytes).unwrap();
        p
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn actual_binary_runs_g002_g007_offline_and_repeats_identically() {
    for (name, data) in [
        ("g002-events.json", "g002-data.json"),
        ("g007-events.json", "g007-data.json"),
    ] {
        let first = run(&model(name), &fixture(data), None);
        let text = success(&first);
        assert!(signal5(&text));
        assert!(text.contains("\"signals\":\"1\""));
        assert!(text.contains("\"processed_events\":\"6\""));
        assert_eq!(first.stdout, run(&model(name), &fixture(data), None).stdout);
    }
}
#[test]
fn same_binary_changes_decisions_with_parameters_and_artifact() {
    let m = model("g002-events.json");
    let d = fixture("g002-data.json");
    let low = success(&run(&m, &d, Some(&fixture("parameters-60.json"))));
    assert!(signal5(&low));
    let high = success(&run(&m, &d, Some(&fixture("parameters-90.json"))));
    assert!(!signal5(&high));
    assert!(high.contains("\"signals\":\"0\""));
    assert_ne!(low, high);
    let changed = success(&run(&fixture("g002-high-model.json"), &d, None));
    assert!(changed.contains("\"signals\":\"0\""));
    assert!(!signal5(&changed));
}
#[test]
fn validate_compiles_without_data_and_matches_library_contract() {
    let path = model("g002-events.json");
    let p = fixture("parameters-90.json");
    let o = command()
        .arg("validate")
        .arg("--model")
        .arg(&path)
        .arg("--params")
        .arg(&p)
        .output()
        .unwrap();
    let s = success(&o);
    assert!(s.contains("\"status\":\"valid\""));
    assert!(!s.contains("observations"));
    let artifact = LoadedEventPlan::from_bytes(&fs::read(path).unwrap()).unwrap();
    let params = ParameterBindings::from_bytes(&fs::read(p).unwrap()).unwrap();
    let id = params.source_id();
    let plan = artifact
        .compile(
            &catalog::reference(artifact.trigger()).unwrap(),
            params.into_values(),
        )
        .unwrap();
    let mut expected = exchange::validation_result(&plan, Some(id)).unwrap();
    expected.push(b'\n');
    assert_eq!(o.stdout, expected);
}
#[test]
fn actual_binary_report_matches_same_public_library_execution() {
    for (m, d) in [
        ("g002-events.json", "g002-data.json"),
        ("g007-events.json", "g007-data.json"),
    ] {
        let artifact = LoadedEventPlan::from_bytes(&fs::read(model(m)).unwrap()).unwrap();
        let plan = artifact
            .compile(&catalog::reference(artifact.trigger()).unwrap(), vec![])
            .unwrap();
        let data = MarketData::from_bytes(&fs::read(fixture(d)).unwrap()).unwrap();
        let mut expected = run_report(Arc::new(plan), "acceptance", &data, None).unwrap();
        expected.push(b'\n');
        assert_eq!(run(&model(m), &fixture(d), None).stdout, expected);
    }
}
#[test]
fn manifest_uses_independent_data_hash_and_record_source_links() {
    let text = success(&run(
        &model("g002-events.json"),
        &fixture("g002-data.json"),
        None,
    ));
    let sha = fs::read_to_string(fixture("g002-data.identity.txt")).unwrap();
    assert!(text.contains(&format!("\"dataset_id\":\"{}\"", sha.trim())));
    assert!(text.contains("\"run\":\"acceptance\""));
    assert!(text.contains("\"source_sequence\":\"5\",\"timestamp_ns\":\"5000\""));
    assert!(text.contains("\"warmup_samples\":\"0\""));
    assert!(text.contains("\"parameters_source_id\":null"));
}
#[test]
fn full_width_tick_values_and_unrelated_filter_survive_json() {
    let text = success(&run(
        &fixture("tick-model.json"),
        &fixture("tick-data.json"),
        None,
    ));
    assert!(text.contains("\"value\":\"18446744073709551615\""));
    assert!(text.contains("\"last_sequence\":\"18446744073709551615\""));
    assert!(text.contains("\"matched_events\":\"1\""));
    assert!(text.contains("\"processed_events\":\"2\""));
    assert!(text.contains("\"matched\":false"));
    assert!(text.contains("\"source_sequence\":\"9007199254740993\""));
}
#[test]
fn explicit_empty_array_completes_with_zero_events() {
    let text = success(&run(
        &fixture("tick-model.json"),
        &fixture("empty-data.json"),
        None,
    ));
    assert!(text.contains("\"observations\":[]"));
    assert!(text.contains("\"processed_events\":\"0\""));
    assert!(text.contains("\"last_sequence\":null"));
}
#[test]
fn invalid_missing_duplicate_and_extra_options_fail_before_io() {
    for args in [
        vec!["run"],
        vec!["validate"],
        vec!["run", "--model", "missing"],
        vec!["validate", "--model", "x", "--data", "y"],
        vec!["run", "--model", "x", "--data", "y", "--run-id", "bad id"],
        vec!["validate", "--model", "x", "--model", "y"],
        vec!["run", "--model", "x", "--data", "y", "--mystery", "z"],
        vec!["validate", "--model", "--params"],
    ] {
        failure(&command().args(args).output().unwrap(), 2, "arguments");
    }
}
#[test]
fn missing_directory_oversized_and_empty_files_are_structured_errors() {
    let temp = Temp::new();
    let missing = temp.0.join("missing");
    failure(
        &command()
            .arg("validate")
            .arg("--model")
            .arg(&missing)
            .output()
            .unwrap(),
        3,
        "model_io",
    );
    failure(
        &command()
            .arg("validate")
            .arg("--model")
            .arg(&temp.0)
            .output()
            .unwrap(),
        3,
        "model_io",
    );
    let large = temp.file("large", vec![b' '; exchange::MAX_FILE_BYTES + 1]);
    assert!(
        failure(
            &run(&fixture("tick-model.json"), &large, None),
            3,
            "data_io"
        )
        .contains("file_limit")
    );
    let empty = temp.file("empty", b"");
    failure(&run(&fixture("tick-model.json"), &empty, None), 3, "data");
}
#[test]
fn malformed_unknown_duplicate_data_and_secret_values_never_emit_partial_stdout() {
    let temp = Temp::new();
    for (i,bytes) in [b"{".as_slice(),br#"{"format":"fqe.market-data","format":"fqe.market-data"}"#,br#"{"format":"fqe.market-data","schema_version":1,"events":[{"kind":"DO_NOT_LEAK_SECRET"}]}"#,br#"{"format":"fqe.market-data","schema_version":1,"events":[{}]}"#].iter().enumerate(){let path=temp.file(&format!("bad{i}"),bytes);let text=failure(&run(&fixture("tick-model.json"),&path,None),3,"data");assert!(!text.contains("DO_NOT_LEAK_SECRET"));}
}
#[test]
fn malformed_models_unknown_providers_and_wrong_binding_types_reject() {
    let temp = Temp::new();
    let original = fs::read_to_string(model("g002-events.json")).unwrap();
    let unknown = temp.file(
        "unknown",
        original.replace("reference.macd", "unavailable.macd"),
    );
    failure(
        &command()
            .arg("validate")
            .arg("--model")
            .arg(unknown)
            .output()
            .unwrap(),
        3,
        "compile",
    );
    let future = temp.file(
        "future",
        original.replacen("\"schema_version\": 1", "\"schema_version\": 2", 1),
    );
    failure(&run(&future, &fixture("g002-data.json"), None), 3, "model");
    let params=temp.file("bad-params",br#"{"version":1,"values":{"threshold":{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"90"}}}"#);
    failure(
        &run(
            &model("g002-events.json"),
            &fixture("g002-data.json"),
            Some(&params),
        ),
        3,
        "compile",
    );
    let raw = temp.file(
        "bad-json-params",
        br#"{"version":1,"values":{"threshold":"DO_NOT_LEAK_SECRET"}}"#,
    );
    let text = failure(
        &run(
            &model("g002-events.json"),
            &fixture("g002-data.json"),
            Some(&raw),
        ),
        3,
        "parameters",
    );
    assert!(!text.contains("DO_NOT_LEAK_SECRET"));
}
#[test]
fn later_event_order_domain_and_numeric_errors_discard_entire_batch() {
    let temp = Temp::new();
    let data = fs::read_to_string(fixture("g002-data.json")).unwrap();
    let duplicate = temp.file(
        "duplicate",
        data.replace("\"sequence\": \"2\"", "\"sequence\": \"1\""),
    );
    let error = failure(
        &run(&model("g002-events.json"), &duplicate, None),
        4,
        "execution",
    );
    assert!(error.contains("events[1]"));
    assert!(error.contains("event_sequence"));
    failure(
        &run(
            &fixture("tick-model.json"),
            &fixture("g002-data.json"),
            None,
        ),
        4,
        "execution",
    );
    let numeric = failure(
        &run(
            &fixture("tick-error-model.json"),
            &fixture("tick-error-data.json"),
            None,
        ),
        4,
        "execution",
    );
    assert!(numeric.contains("events[1]"));
    assert!(numeric.contains("expression_numeric"));
}
#[cfg(unix)]
#[test]
fn non_utf8_path_arguments_return_structured_errors_without_panicking() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let path = OsString::from_vec(b"/nonexistent/model-\xff.json".to_vec());
    failure(
        &command()
            .arg("validate")
            .arg("--model")
            .arg(path)
            .output()
            .unwrap(),
        3,
        "model_io",
    );
}
#[test]
fn unicode_and_space_paths_are_read_as_paths() {
    let temp = Temp::new();
    let path = temp.file(
        "本地 模型.json",
        fs::read(fixture("tick-model.json")).unwrap(),
    );
    success(&run(&path, &fixture("tick-data.json"), None));
}
#[cfg(unix)]
#[test]
fn closed_output_pipe_returns_io_status_instead_of_panicking() {
    use std::{
        os::{fd::OwnedFd, unix::net::UnixStream},
        process::Stdio,
    };
    let (reader, writer) = UnixStream::pair().unwrap();
    drop(reader);
    let o = command()
        .arg("validate")
        .arg("--model")
        .arg(fixture("tick-model.json"))
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .output()
        .unwrap();
    failure(&o, 5, "output");
}
#[test]
fn oversized_batch_result_is_rejected_without_publishing_a_prefix() {
    let temp = Temp::new();
    let original = fs::read_to_string(fixture("tick-model.json")).unwrap();
    let start = original.rfind("\n  \"outputs\": [").unwrap();
    let roots=(0..32).map(|i|format!(r#"{{"name":"text{i}","expression":{{"literal":{{"type":{{"version":1,"shape":"scalar","kind":"text","max_bytes":4096}},"value":"{}"}}}}}}"#,"x".repeat(4096))).collect::<Vec<_>>().join(",");
    let artifact = temp.file(
        "wide-model",
        format!(
            "{}\n  \"outputs\": [{roots}],\n  \"signals\": []\n}}",
            &original[..start]
        ),
    );
    let events=(1..=100).map(|n|format!(r#"{{"kind":"tick","sequence":"{n}","timestamp_ns":"{n}","instrument":"DEMO","price":"101"}}"#)).collect::<Vec<_>>().join(",");
    let data = temp.file(
        "many-data",
        format!(r#"{{"format":"fqe.market-data","schema_version":1,"events":[{events}]}}"#),
    );
    let error = failure(&run(&artifact, &data, None), 5, "result");
    assert!(error.contains("report_limit"));
}

#[test]
fn exact_file_byte_limit_is_accepted_but_one_more_is_rejected() {
    let temp = Temp::new();
    let mut bytes = fs::read(fixture("empty-data.json")).unwrap();
    bytes.resize(exchange::MAX_FILE_BYTES, b' ');
    let path = temp.file("limit-data", &bytes);
    success(&run(&fixture("tick-model.json"), &path, None));
    bytes.push(b' ');
    fs::write(&path, bytes).unwrap();
    assert!(
        failure(&run(&fixture("tick-model.json"), &path, None), 3, "data_io")
            .contains("file_limit")
    );
}
