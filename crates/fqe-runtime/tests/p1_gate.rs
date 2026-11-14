#[path = "support/gate_providers.rs"]
mod providers;

use fqe_core::{
    Event, InstrumentId, MarketTick, PriceTicks, SignalKind,
    types::{Evaluation, ScalarValue, TypedValue},
};
use fqe_ir::{
    IR_VERSION, Predicate, PriceExpr, Strategy, catalog,
    event_plan::{CompiledEventPlan, LoadedEventPlan},
    exchange::{MarketData, MarketRecord, ParameterBindings},
    model::ModelError,
};
use fqe_runtime::{
    RunStatus, Runtime,
    events::{EventObservation, EventRuntime, InputEvent},
    report::run_report,
};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/p1")
            .join(name),
    )
    .unwrap()
}
fn other(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../")
            .join(name),
    )
    .unwrap()
}
fn input(record: &MarketRecord) -> InputEvent {
    match record {
        MarketRecord::Tick(t) => InputEvent::from(t.clone()),
        MarketRecord::Bar(b) => InputEvent::from(b.input().clone()),
    }
}
fn scalar(value: &Evaluation<TypedValue>) -> String {
    match value {
        Evaluation::Warmup => "W".into(),
        Evaluation::Ready(v) => match v.value() {
            ScalarValue::Bool(b) => b.to_string(),
            _ => v.numeric_coefficient().unwrap().to_string(),
        },
        value => panic!("unexpected readiness {value:?}"),
    }
}
fn observations(
    plan: Arc<CompiledEventPlan>,
    data: &MarketData,
    count: usize,
) -> Vec<EventObservation> {
    let mut runtime = EventRuntime::new(plan, "gate", data.source_id()).unwrap();
    let rows = data
        .events()
        .iter()
        .take(count)
        .map(|e| runtime.step(input(e)).unwrap())
        .collect();
    assert_eq!(runtime.finish().unwrap().status, RunStatus::Completed);
    rows
}
fn custom(name: &str, params: Option<&str>) -> Arc<CompiledEventPlan> {
    let (registry, calls) = providers::registry();
    let loaded = LoadedEventPlan::from_bytes(&fixture(name)).unwrap();
    let bindings = params
        .map(|p| {
            ParameterBindings::from_bytes(&fixture(p))
                .unwrap()
                .into_values()
        })
        .unwrap_or_default();
    let plan = loaded.compile(&registry, bindings).unwrap();
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "compilation must not execute providers"
    );
    assert_eq!(plan.graph().nodes().len(), 4);
    assert!(
        plan.graph()
            .nodes()
            .iter()
            .any(|n| n.path == ["first", "shift"])
    );
    assert!(
        plan.graph()
            .nodes()
            .iter()
            .any(|n| n.path == ["second", "window"])
    );
    Arc::new(plan)
}

#[test]
fn loaded_unknown_name_functions_predicates_and_reused_rules_match_manual_goldens() {
    let table = String::from_utf8(fixture("x001.expected.tsv")).unwrap();
    assert_eq!(table.lines().next(), Some("# fqe.p1-x001-golden/1"));
    let data = MarketData::from_bytes(&fixture("x001-data.json")).unwrap();
    let mut identities = Vec::new();
    for (case, model, params) in [
        ("sum", "x001.json", None),
        ("shift0", "x001-shift0.json", None),
        ("lower108", "x001.json", Some("parameters-108.json")),
    ] {
        let plan = custom(model, params);
        identities.push(plan.content_id());
        let rows = observations(Arc::clone(&plan), &data, 7);
        let expected: Vec<_> = table
            .lines()
            .skip(2)
            .map(|s| s.split('\t').collect::<Vec<_>>())
            .filter(|v| v[0] == case)
            .collect();
        assert_eq!(expected.len(), rows.len());
        for (row, want) in rows.iter().zip(expected) {
            assert_eq!(row.source_sequence.to_string(), want[1]);
            assert_eq!(row.timestamp_ns, row.source_sequence * 1000);
            assert_eq!(row.run, "gate");
            assert_eq!(row.execution_id, plan.content_id());
            assert!(row.matched && !row.warmup);
            for (name, index) in [("shifted", 2), ("first", 3), ("second", 4), ("decision", 5)] {
                assert_eq!(
                    scalar(&row.outputs[name]),
                    want[index],
                    "{case}/{name}/{}",
                    want[1]
                );
            }
            assert_eq!(row.signals.len(), usize::from(want[5] == "true"));
            for signal in &row.signals {
                assert_eq!(signal.source_sequence, row.source_sequence);
                assert_eq!(signal.timestamp_ns, row.timestamp_ns);
                assert_eq!(signal.signal.kind, SignalKind::Long);
                assert_eq!(signal.signal.instrument.as_str(), "DEMO");
            }
        }
        assert_eq!(rows, observations(Arc::clone(&plan), &data, 7));
        assert_eq!(rows[..4], observations(Arc::clone(&plan), &data, 4));
        let source = params.map(|p| {
            ParameterBindings::from_bytes(&fixture(p))
                .unwrap()
                .source_id()
        });
        assert_eq!(
            run_report(Arc::clone(&plan), "gate", &data, source).unwrap(),
            run_report(plan, "gate", &data, source).unwrap()
        );
    }
    assert!(
        identities[0] != identities[1]
            && identities[0] != identities[2]
            && identities[1] != identities[2]
    );
}

#[test]
fn g001_strict_threshold_has_same_legacy_and_loaded_event_signals() {
    let data = MarketData::from_bytes(&fixture("g001-data.json")).unwrap();
    let events: Vec<_> = data
        .events()
        .iter()
        .map(|r| match r {
            MarketRecord::Tick(t) => t.clone(),
            _ => panic!("tick fixture"),
        })
        .collect();
    let strategy = Strategy::new(
        IR_VERSION,
        "g001",
        InstrumentId::new("DEMO").unwrap(),
        Predicate::GreaterThan {
            left: PriceExpr::Current,
            right: PriceExpr::Literal(PriceTicks::new(100).unwrap()),
        },
        SignalKind::Long,
    )
    .unwrap();
    let legacy = Runtime::new(strategy).run(events).unwrap();
    let loaded =
        LoadedEventPlan::from_bytes(&other("fqe-cli/tests/fixtures/tick-model.json")).unwrap();
    let plan = Arc::new(
        loaded
            .compile(&catalog::reference(loaded.trigger()).unwrap(), vec![])
            .unwrap(),
    );
    let rows = observations(plan, &data, 4);
    let expected = vec![3, 4];
    assert_eq!(
        legacy
            .signals
            .iter()
            .map(|s| s.source_sequence)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        rows.iter()
            .flat_map(|r| &r.signals)
            .map(|s| s.source_sequence)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(legacy.state.status, RunStatus::Completed);
}

#[test]
fn loaded_financial_outputs_match_independent_readiness_and_signal_table() {
    let table = String::from_utf8(fixture("financial.expected.tsv")).unwrap();
    assert_eq!(table.lines().next(), Some("# fqe.p1-financial-golden/1"));
    for (case, model, data_name, params) in [
        ("g003", "g002", "g002", "60"),
        ("g003-high", "g002", "g002", "90"),
        ("g007", "g007", "g007", "empty"),
    ] {
        let loaded = LoadedEventPlan::from_bytes(&other(&format!(
            "fqe-ir/tests/fixtures/{model}-events.json"
        )))
        .unwrap();
        let data = MarketData::from_bytes(&other(&format!(
            "fqe-cli/tests/fixtures/{data_name}-data.json"
        )))
        .unwrap();
        let binding = ParameterBindings::from_bytes(&other(&format!(
            "fqe-cli/tests/fixtures/parameters-{params}.json"
        )))
        .unwrap();
        let plan = Arc::new(
            loaded
                .compile(
                    &catalog::reference(loaded.trigger()).unwrap(),
                    binding.into_values(),
                )
                .unwrap(),
        );
        let rows = observations(Arc::clone(&plan), &data, 6);
        let expected: Vec<_> = table
            .lines()
            .skip(2)
            .map(|s| s.split('\t').collect::<Vec<_>>())
            .filter(|v| v[0] == case)
            .collect();
        assert_eq!(expected.len(), rows.len());
        for (row, want) in rows.iter().zip(expected) {
            assert_eq!(row.source_sequence.to_string(), want[1]);
            assert_eq!(row.warmup.to_string(), want[2]);
            assert_eq!(scalar(&row.outputs["decision"]), want[3]);
            if want[4] != "-" {
                assert_eq!(scalar(&row.outputs["rsi"]), want[4]);
            }
            assert_eq!(row.signals.len().to_string(), want[5]);
        }
        assert_eq!(rows[..3], observations(plan, &data, 3));
    }
}

#[test]
fn invalid_loaded_models_fail_before_any_provider_execution() {
    let (registry, calls) = providers::registry();
    let table = String::from_utf8(fixture("invalid.expected.tsv")).unwrap();
    assert_eq!(table.lines().next(), Some("# fqe.p1-invalid-golden/1"));
    let mut count = 0;
    for item in
        fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/p1")).unwrap()
    {
        let path = item.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if !name.starts_with("invalid-") {
            continue;
        }
        let loaded = LoadedEventPlan::from_bytes(&fs::read(&path).unwrap());
        let stage = if loaded.is_ok() { "compile" } else { "load" };
        let result: Result<CompiledEventPlan, ModelError> =
            loaded.and_then(|m| m.compile(&registry, vec![]));
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("invalid model accepted: {name}"),
        };
        println!("{name}\t{}\t{}", error.code, error.path);
        let expected = table
            .lines()
            .skip(2)
            .map(|s| s.split('\t').collect::<Vec<_>>())
            .find(|v| v[0] == name)
            .unwrap();
        assert_eq!(
            (stage, error.code, error.path.as_str()),
            (expected[1], expected[2], expected[3]),
            "{name}"
        );
        assert_eq!(calls.load(Ordering::Relaxed), 0, "{name}");
        count += 1;
    }
    assert_eq!(count, 12);
    assert_eq!(table.lines().skip(2).count(), count);
    LoadedEventPlan::from_bytes(&fixture("x001.json"))
        .unwrap()
        .compile(&registry, vec![])
        .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn source_formatting_preserves_semantics_but_not_raw_identity() {
    let (registry, _) = providers::registry();
    let original = LoadedEventPlan::from_bytes(&fixture("x001.json")).unwrap();
    let normalized = LoadedEventPlan::from_bytes(original.canonical_bytes()).unwrap();
    assert_eq!(original.content_id(), normalized.content_id());
    assert_ne!(original.source_id(), normalized.source_id());
    assert_eq!(
        original.compile(&registry, vec![]).unwrap().content_id(),
        normalized.compile(&registry, vec![]).unwrap().content_id()
    );
}

fn tick(sequence: u64, price: u64) -> InputEvent {
    InputEvent::from(
        Event::new(
            sequence,
            sequence * 1000,
            MarketTick {
                instrument: InstrumentId::new("DEMO").unwrap(),
                price: PriceTicks::new(price).unwrap(),
            },
        )
        .unwrap(),
    )
}
#[test]
fn novel_provider_overflow_rejects_whole_event_and_allows_corrected_retry() {
    let plan = custom("x001.json", None);
    let data = MarketData::from_bytes(&fixture("x001-data.json")).unwrap();
    let mut runtime = EventRuntime::new(plan, "retry", data.source_id()).unwrap();
    runtime.step(tick(1, 90)).unwrap();
    let before = runtime.state().clone();
    let error = runtime.step(tick(2, u64::MAX)).unwrap_err();
    assert_eq!(error.code, "registry_numeric");
    assert_eq!(runtime.state(), &before);
    assert_eq!(runtime.matched_events(), 1);
    let retried = runtime.step(tick(2, 95)).unwrap();
    assert_eq!(scalar(&retried.outputs["shifted"]), "105");
    assert_eq!(retried.signals.len(), 1);
    assert_eq!(runtime.finish().unwrap().processed_events, 2);
}
