use fqe_ir::{catalog, model::ModelError, workflow::LoadedWorkflow};
use serde_json::{Value, json};
const SOURCE: &str = include_str!("fixtures/workflow-counter.json");
fn fixture() -> Value {
    serde_json::from_str(SOURCE).unwrap()
}
fn compile(v: &Value) -> Result<fqe_ir::workflow::CompiledWorkflow, ModelError> {
    let loaded = LoadedWorkflow::from_bytes(&serde_json::to_vec(v).unwrap())?;
    let registry = catalog::reference(loaded.event_plan().trigger())?;
    loaded.compile(&registry, vec![])
}
fn rejected(mutator: impl FnOnce(&mut Value)) {
    let mut v = fixture();
    mutator(&mut v);
    assert!(compile(&v).is_err(), "accepted invalid workflow: {v}");
}
#[test]
fn canonical_order_source_identity_and_exact_bindings() {
    let loaded = LoadedWorkflow::from_bytes(SOURCE.as_bytes()).unwrap();
    let mut v = fixture();
    v["instances"].as_array_mut().unwrap().reverse();
    v["modules"][0]["transitions"]
        .as_array_mut()
        .unwrap()
        .reverse();
    v["modules"][0]["modes"].as_array_mut().unwrap().reverse();
    v["modules"][0]["parameters"]["entries"]
        .as_array_mut()
        .unwrap()
        .reverse();
    v["signals"].as_array_mut().unwrap().reverse();
    let shuffled = LoadedWorkflow::from_bytes(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(loaded.content_id(), shuffled.content_id());
    assert_eq!(loaded.canonical_bytes(), shuffled.canonical_bytes());
    assert_ne!(loaded.source_id(), shuffled.source_id());
    let again = LoadedWorkflow::from_bytes(loaded.canonical_bytes()).unwrap();
    assert_eq!(again.content_id(), loaded.content_id());
    let a = compile(&v).unwrap();
    v["instances"][0]["parameters"]["values"]["step"]["value"] = json!("3");
    let b = compile(&v).unwrap();
    assert_ne!(a.content_id(), b.content_id());
    assert_ne!(a.artifact_id(), b.artifact_id());
    assert_eq!(
        a.instances()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["fast", "slow"]
    );
}
#[test]
fn strict_envelopes_capabilities_and_duplicate_keys() {
    rejected(|v| v["schema_version"] = json!(2));
    rejected(|v| v["unexpected"] = json!(true));
    rejected(|v| v["requirements"] = json!(["workflow.scalar/1", "workflow.snapshot/1"]));
    rejected(|v| v["requirements"] = json!(["workflow.state/1"]));
    rejected(|v| v["requirements"] = json!(["workflow.scalar/1", "workflow.scalar/1"]));
    rejected(|v| v["modules"][0]["schema_version"] = json!(2));
    rejected(|v| v["modules"][0]["state_schema"]["version"] = json!(""));
    rejected(|v| v["instances"][0]["module"]["version"] = json!("2"));
    rejected(|v| v["instances"][0]["parameters"]["version"] = json!(2));
    rejected(|v| v["modules"][0]["state"][0]["initial"]["value"] = json!(1));
    assert!(
        LoadedWorkflow::from_bytes(
            SOURCE
                .replacen(
                    "\"schema_version\": 1",
                    "\"schema_version\": 1, \"schema_version\": 1",
                    1
                )
                .as_bytes()
        )
        .is_err()
    );
    assert!(LoadedWorkflow::from_bytes(&vec![b' '; 262145]).is_err());
}
#[test]
fn invalid_types_bindings_expressions_and_phases() {
    rejected(|v| v["modules"][0]["state"][0]["initial"]["type"]["kind"] = json!("i64"));
    rejected(|v| v["modules"][0]["transitions"][0]["guard"] = json!({"input":"state.count"}));
    rejected(|v| v["modules"][0]["transitions"][0]["guard"] = json!({"input":"fired.arm"}));
    rejected(|v| {
        v["modules"][0]["transitions"][0]["writes"]["unknown"] = json!({"input":"state.count"})
    });
    rejected(|v| v["modules"][0]["outputs"]["bad"] = json!({"cross":"up"}));
    rejected(|v| v["modules"][0]["outputs"]["bad"] = json!({"output":"price"}));
    rejected(|v| v["modules"][0]["outputs"]["bad"] = json!({"input":"state.unknown"}));
    rejected(|v| v["modules"][0]["outputs"] = json!({}));
    rejected(|v| v["modules"][0]["delays"][0]["initial"] = json!({"status":"error"}));
    rejected(|v| v["modules"][0]["delays"][0]["next"] = json!({"input":"fired.arm"}));
    rejected(|v| v["signals"][0]["output"] = json!("count"));
    rejected(|v| v["instances"][0]["bindings"] = json!({}));
    rejected(|v| v["instances"][0]["bindings"]["extra"] = json!({"event":"price"}));
    rejected(|v| v["instances"][0]["bindings"]["sample"] = json!({"event":"decision"}));
    rejected(|v| v["instances"][0]["bindings"]["sample"] = json!({"event":"unknown"}));
    rejected(|v| {
        v["instances"][0]["parameters"]["values"]["step"] =
            json!({"type":{"version":1,"shape":"scalar","kind":"bool"},"value":true})
    });
    rejected(|v| v["event_plan"]["signals"] = json!([{"output":"decision","kind":"long"}]));
}
#[test]
fn declaration_duplicates_modes_and_state_limits() {
    rejected(|v| {
        let x = v["instances"][0].clone();
        v["instances"].as_array_mut().unwrap().push(x);
    });
    rejected(|v| {
        let x = v["modules"][0].clone();
        v["modules"].as_array_mut().unwrap().push(x);
    });
    rejected(|v| v["modules"][0]["modes"] = json!(["idle", "idle"]));
    rejected(|v| v["modules"][0]["initial_mode"] = json!("unknown"));
    rejected(|v| v["modules"][0]["transitions"][0]["from"] = json!(["unknown"]));
    rejected(|v| v["modules"][0]["transitions"][0]["to"] = json!("unknown"));
    rejected(|v| v["modules"][0]["state"][0]["name"] = json!("a".repeat(121)));
    rejected(|v| {
        let mut x = v["modules"][0].clone();
        x["key"]["id"] = json!("unused");
        v["modules"].as_array_mut().unwrap().push(x);
    });
    rejected(|v| {
        let source = v["instances"][0].clone();
        v["instances"] = (0..17)
            .map(|i| {
                let mut x = source.clone();
                x["id"] = json!(format!("i{i}"));
                x
            })
            .collect();
    });
    rejected(|v| {
        let x = v["modules"][0]["state"][0].clone();
        v["modules"][0]["state"] = (0..28)
            .map(|i| {
                let mut x = x.clone();
                x["name"] = json!(format!("s{i}"));
                x
            })
            .chain(std::iter::once(x.clone()))
            .collect();
    });
    assert!(fqe_ir::workflow::mode_value(&"a".repeat(129)).is_err());
}
#[test]
fn typed_instance_dag_and_explicit_delay_feedback() {
    let mut v = fixture();
    v["instances"][1]["bindings"]["sample"] = json!({"instance":"fast","output":"count"});
    assert!(compile(&v).is_ok());
    v["instances"][0]["bindings"]["sample"] = json!({"instance":"slow","output":"count"});
    assert_eq!(compile(&v).err().unwrap().code, "workflow_cycle");
    v["instances"][0]["bindings"]["sample"] = json!({"instance":"fast","output":"count"});
    assert!(compile(&v).is_err());
    let mut v = fixture();
    v["modules"][0]["delays"][0]["next"] = json!({"input":"delay.lag"});
    assert!(
        compile(&v).is_ok(),
        "explicit previous-event delay feedback is valid"
    );
}
#[test]
fn state_view_is_typed_versioned_bounded_and_not_restorable() {
    let plan = compile(&fixture()).unwrap();
    let mut cells = plan
        .instances()
        .iter()
        .map(|i| (i.id.clone(), i.initial.clone()))
        .collect();
    let view = fqe_ir::workflow::state_view(plan.content_id(), "inspection", &cells).unwrap();
    let view: Value = serde_json::from_slice(&view).unwrap();
    assert_eq!(view["restorable"], json!(false));
    assert_eq!(
        view["instances"]["fast"]["state_schema"]["version"],
        json!("1")
    );
    assert_eq!(
        view["instances"]["fast"]["state"]["count"]["value"],
        json!("0")
    );
    assert_eq!(
        view["instances"]["fast"]["delays"]["lag"]["status"],
        json!("warmup")
    );
    use fqe_core::types::{ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape};
    let big = TypedValue::new(
        TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Text { max_bytes: 4096 }).unwrap(),
        ScalarValue::Text("a".repeat(4096)),
    )
    .unwrap();
    for i in 0..17 {
        cells
            .get_mut("fast")
            .unwrap()
            .state
            .insert(format!("big{i}"), big.clone());
    }
    assert!(fqe_ir::workflow::state_view(plan.content_id(), "inspection", &cells).is_err());
}
#[test]
fn expanded_expression_and_initial_cell_budgets_are_admission_checks() {
    fn tree(depth: usize) -> Value {
        if depth == 0 {
            json!({"input":"state.count"})
        } else {
            let child = tree(depth - 1);
            json!({"binary":{"op":"add","left":child,"right":child}})
        }
    }
    let mut v = fixture();
    v["signals"] = json!([]);
    let source = v["instances"][0].clone();
    v["instances"] = (0..16)
        .map(|i| {
            let mut x = source.clone();
            x["id"] = json!(format!("i{i}"));
            x
        })
        .collect();
    v["modules"][0]["outputs"]["expanded"] = tree(7);
    assert_eq!(compile(&v).err().unwrap().code, "workflow_limit");
    let mut v = fixture();
    for i in 0..17 {
        v["modules"][0]["state"].as_array_mut().unwrap().push(json!({"name":format!("big{i}"),"type":{"version":1,"shape":"scalar","kind":"text","max_bytes":4096},"initial":{"type":{"version":1,"shape":"scalar","kind":"text","max_bytes":4096},"value":"a".repeat(4096)}}));
    }
    assert_eq!(compile(&v).err().unwrap().code, "workflow_limit");
}
