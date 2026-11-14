#[path = "../examples/support/compiled.rs"]
mod compiled;
#[path = "../examples/support/providers.rs"]
mod providers;
use fqe_core::types::{
    Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
};
use fqe_ir::{
    compiler::{self, CompiledCall, CompiledPlan},
    graph::LoadedGraph,
    loading::LoadedModel,
    model::{ContentId, EntryKey, ModelError},
    parameters::{BoundParameters, ParameterSchema},
    registry::{
        CallablePort, Descriptor, DescriptorDraft, IncrementalProvider, IncrementalState,
        NamedValues, ProviderCall, ProviderFailure, Purity, ReadinessPolicy, Registry,
        RegistryBuilder,
    },
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
const FIXTURE: &[u8] = include_bytes!("fixtures/x001-graph-sum.json");
fn document() -> Value {
    serde_json::from_slice(FIXTURE).unwrap()
}
fn load(v: &Value) -> LoadedGraph {
    LoadedGraph::from_bytes(&serde_json::to_vec(v).unwrap()).unwrap()
}
fn plan(v: &Value) -> Result<CompiledPlan, ModelError> {
    let g = load(v);
    compiler::compile_graph(
        &g,
        &g.bind(vec![]).unwrap(),
        &providers::registry().unwrap(),
    )
}
fn root_node<'a>(v: &'a mut Value, id: &str) -> &'a mut Value {
    v["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == id)
        .unwrap()
}
fn rule_node<'a>(v: &'a mut Value, id: &str) -> &'a mut Value {
    v["modules"][0]["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == id)
        .unwrap()
}
fn boolean(v: &Evaluation<TypedValue>) -> bool {
    match v {
        Evaluation::Ready(v) => match v.value() {
            ScalarValue::Bool(b) => *b,
            _ => panic!("not bool"),
        },
        _ => panic!("not ready"),
    }
}
#[test]
fn independent_graph_canonical_identity_and_legacy_format_are_preserved() {
    let g = LoadedGraph::from_bytes(FIXTURE).unwrap();
    assert_eq!(
        g.canonical_bytes(),
        include_bytes!("fixtures/graph.canonical.json")
    );
    assert_eq!(
        g.content_id().to_string(),
        include_str!("fixtures/graph.identity.txt").trim()
    );
    let round = LoadedGraph::from_bytes(g.canonical_bytes()).unwrap();
    assert_eq!(round.content_id(), g.content_id());
    assert_eq!(round.canonical_bytes(), g.canonical_bytes());
    assert_ne!(round.source_id(), g.source_id());
    assert!(LoadedModel::from_bytes(FIXTURE).is_err());
    assert!(LoadedGraph::from_bytes(include_bytes!("fixtures/model.json")).is_err());
}
#[test]
fn compiled_unknown_name_legacy_compositions_execute_without_manual_node_wiring() {
    let r = providers::registry().unwrap();
    for (bytes, want) in [
        (include_bytes!("fixtures/x001-sum.json").as_slice(), true),
        (
            include_bytes!("fixtures/x001-difference.json").as_slice(),
            false,
        ),
    ] {
        let m = LoadedModel::from_bytes(bytes).unwrap();
        let b = m.bind_json(br#"{"version":1,"values":{}}"#).unwrap();
        let p = compiler::compile_model(&m, &b, &r).unwrap();
        assert_eq!(p.nodes().len(), 5);
        let output = compiled::evaluate(&p, BTreeMap::new()).unwrap();
        assert_eq!(boolean(&output["decision"]), want);
        assert_eq!(boolean(&output["inverse"]), !want);
    }
}
#[test]
fn nested_and_reused_modules_bind_parameters_and_expand_named_multi_outputs() {
    let r = providers::registry().unwrap();
    for (bytes, want) in [
        (FIXTURE, true),
        (
            include_bytes!("fixtures/x001-graph-difference.json").as_slice(),
            false,
        ),
    ] {
        let g = LoadedGraph::from_bytes(bytes).unwrap();
        let p = compiler::compile_graph(&g, &g.bind(vec![]).unwrap(), &r).unwrap();
        assert_eq!(p.nodes().len(), 7);
        let output = compiled::evaluate(&p, BTreeMap::new()).unwrap();
        assert_eq!(boolean(&output["first"]), want);
        assert!(!boolean(&output["second"]));
        assert_eq!(boolean(&output["inverted"]), !want);
        let n = p
            .nodes()
            .iter()
            .find(|n| n.path == ["first", "nested", "pair"])
            .unwrap();
        assert_eq!(n.source.call_sites.len(), 2);
        assert!(n.source.definition.starts_with("$.modules[0].graph.nodes["));
        assert!(p.nodes().iter().any(|n| n.path == ["second", "pair"]));
    }
}
fn reorder(v: &mut Value) {
    for key in ["entries", "requirements", "modules"] {
        v[key].as_array_mut().unwrap().reverse();
    }
    fn body(b: &mut Value) {
        for key in ["nodes", "inputs", "outputs"] {
            b[key].as_array_mut().unwrap().reverse();
        }
        for n in b["nodes"].as_array_mut().unwrap() {
            n["inputs"].as_array_mut().unwrap().reverse();
            n["outputs"].as_array_mut().unwrap().reverse();
        }
    }
    body(&mut v["graph"]);
    for m in v["modules"].as_array_mut().unwrap() {
        body(&mut m["graph"]);
    }
}
#[test]
fn declaration_order_and_source_locations_do_not_change_deterministic_plan_identity() {
    let mut v = document();
    let a = plan(&v).unwrap();
    reorder(&mut v);
    let b = plan(&v).unwrap();
    assert_eq!(a.model_id(), b.model_id());
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    assert_eq!(a.content_id(), b.content_id());
    assert_ne!(
        a.nodes()
            .iter()
            .find(|n| n.path == ["first", "nested", "pair"])
            .unwrap()
            .source,
        b.nodes()
            .iter()
            .find(|n| n.path == ["first", "nested", "pair"])
            .unwrap()
            .source
    );
    let paths: Vec<_> = a.nodes().iter().map(|n| n.path.clone()).collect();
    assert_eq!(
        paths,
        vec![
            vec!["left"],
            vec!["right"],
            vec!["first", "nested", "pair"],
            vec!["first", "nested", "predicate"],
            vec!["invert"],
            vec!["second", "pair"],
            vec!["second", "predicate"]
        ]
    );
}
#[test]
fn bindings_model_registry_and_module_changes_are_pinned_in_compiled_identity() {
    let g = load(&document());
    let r = providers::registry().unwrap();
    let a = compiler::compile_graph(&g, &g.bind(vec![]).unwrap(), &r).unwrap();
    let b=g.bind_json(br#"{"version":1,"values":{"threshold":{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"250"}}}"#).unwrap();
    let changed = compiler::compile_graph(&g, &b, &r).unwrap();
    assert_ne!(a.content_id(), changed.content_id());
    assert_eq!(a.model_id(), changed.model_id());
    assert!(!boolean(
        &compiled::evaluate(&changed, BTreeMap::new()).unwrap()["first"]
    ));
    let mut v = document();
    v["metadata"]["revision"] = json!("new");
    let other = load(&v);
    assert!(compiler::compile_graph(&other, &b, &r).is_err());
    let original_module = g.definition().modules[0].content_id();
    v = document();
    rule_node(&mut v, "predicate")["inputs"][0]["source"]["port"] = json!("difference");
    let other = load(&v);
    assert_ne!(original_module, other.definition().modules[0].content_id());
    assert_ne!(a.content_id(), plan(&v).unwrap().content_id());
    let mut rb = RegistryBuilder::new();
    fqe_ir::builtins::register(&mut rb).unwrap();
    rb.register(providers::pair_descriptor(), Arc::new(providers::Pair))
        .unwrap();
    rb.register(
        providers::predicate_descriptor(),
        Arc::new(providers::Above),
    )
    .unwrap();
    let mut extra = providers::pair_descriptor().declaration().clone();
    extra.key.id = "independent.extra".into();
    rb.register(Descriptor::new(extra).unwrap(), Arc::new(providers::Pair))
        .unwrap();
    let r2 = rb.freeze().unwrap();
    let b = compiler::compile_graph(&g, &g.bind(vec![]).unwrap(), &r2).unwrap();
    assert_ne!(a.registry_id(), b.registry_id());
    assert_ne!(a.content_id(), b.content_id());
}
#[test]
fn unknown_versions_artifacts_and_dependency_requirements_fail_before_execution() {
    for (change, expected) in [
        (0, "registry_artifact"),
        (1, "registry_unknown"),
        (2, "compile_entry"),
        (3, "compile_module"),
    ] {
        let mut v = document();
        match change {
            0 => v["entries"][0]["artifact"] = json!(format!("sha256:{}", "00".repeat(32))),
            1 => v["entries"][0]["key"]["version"] = json!("absent"),
            2 => {
                let pair_id = rule_node(&mut v, "pair")["target"]["key"]["id"].clone();
                v["entries"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|e| e["key"]["id"] != pair_id);
            }
            _ => root_node(&mut v, "first")["target"]["key"]["version"] = json!("absent"),
        }
        assert_eq!(plan(&v).err().unwrap().code, expected);
    }
}
#[test]
fn bad_ports_sources_units_scales_and_parameter_types_are_rejected() {
    for change in 0..8 {
        let mut v = document();
        let want = match change {
            0 => {
                rule_node(&mut v, "pair")["inputs"][0]["name"] = json!("wrong");
                "compile_signature"
            }
            1 => {
                rule_node(&mut v, "predicate")["inputs"][0]["source"]["port"] = json!("missing");
                "compile_source"
            }
            2 => {
                rule_node(&mut v, "pair")["inputs"][0]["source"] = json!({"input":"missing"});
                "compile_source"
            }
            3 => {
                root_node(&mut v, "first")["outputs"][0]["type"] =
                    json!({"version":1,"shape":"scalar","kind":"u64"});
                "compile_signature"
            }
            4 => {
                v["modules"][0]["graph"]["inputs"][0]["type"] = json!({"version":1,"shape":"scalar","kind":"decimal","scale":12,"unit":{"kind":"price","instrument":"DEMO"}});
                "compile_type"
            }
            5 => {
                v["modules"][0]["graph"]["outputs"][0]["type"] =
                    json!({"version":1,"shape":"scalar","kind":"u64"});
                "compile_type"
            }
            6 => {
                v["parameters"]["entries"][0]["type"] =
                    json!({"version":1,"shape":"scalar","kind":"i64"});
                v["parameters"]["entries"][0]["default"]["type"] =
                    v["parameters"]["entries"][0]["type"].clone();
                "compile_type"
            }
            _ => {
                rule_node(&mut v, "predicate")["arguments"]["threshold"] =
                    json!({"parameter":"missing"});
                "compile_parameter"
            }
        };
        assert_eq!(plan(&v).err().unwrap().code, want, "case {change}");
    }
}
#[test]
fn required_unknown_range_enum_and_unbound_module_parameters_are_rejected() {
    for change in 0..5 {
        let mut v = document();
        match change {
            0 => {
                root_node(&mut v, "first")["arguments"] = json!({});
            }
            1 => {
                root_node(&mut v, "first")["arguments"]["unknown"] = json!({"literal":{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"2"}})
            }
            2 => v["parameters"]["entries"][0]["default"]["value"] = json!("1000"),
            3 => {
                v["modules"][0]["parameters"]["entries"][0]["constraints"] = json!({"choices":[{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"150"}]});
            }
            _ => {
                v["parameters"]["entries"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("default");
            }
        }
        let e = plan(&v).err().unwrap();
        assert_eq!(
            e.code,
            if change == 4 {
                "compile_unbound"
            } else {
                "compile_parameter"
            },
            "case {change}"
        );
    }
}
#[test]
fn provider_relationship_errors_retain_definition_and_nested_invocation_locations() {
    let mut v = document();
    rule_node(&mut v, "predicate")["arguments"]["ceiling"] =
        json!({"literal":{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"100"}});
    let e = plan(&v).err().unwrap();
    assert_eq!(e.code, "registry_parameter");
    assert!(e.path.contains("$.modules[0].graph.nodes[1]"));
    assert!(
        e.path
            .contains("via $.graph.nodes[2] -> $.modules[1].graph.nodes[0]"),
        "{}",
        e.path
    );
}
#[test]
fn direct_indirect_graph_cycles_and_module_recursion_are_explicit() {
    let mut direct = document();
    rule_node(&mut direct, "pair")["inputs"][0]["source"] = json!({"node":"pair","port":"sum"});
    assert_eq!(plan(&direct).err().unwrap().code, "compile_cycle");
    let r = providers::registry().unwrap();
    let id = r
        .descriptors()
        .find(|d| d.declaration().key.id == "fqe.identity.u64")
        .unwrap();
    let mut v = document();
    v["entries"].as_array_mut().unwrap().push(json!({"key":{"id":"fqe.identity.u64","version":"1"},"descriptor_version":1,"role":"provider","artifact":id.requirement().artifact.to_string()}));
    let t = json!({"version":1,"shape":"scalar","kind":"u64"});
    for (a, b) in [("a", "b"), ("b", "a")] {
        v["graph"]["nodes"].as_array_mut().unwrap().push(json!({"id":a,"target":{"kind":"provider","key":{"id":"fqe.identity.u64","version":"1"}},"arguments":{},"inputs":[{"name":"value","source":{"node":b,"port":"result"}}],"outputs":[{"name":"result","type":t}]}));
    }
    assert_eq!(plan(&v).err().unwrap().code, "compile_cycle");
    for indirect in [false, true] {
        let mut v = document();
        if indirect {
            rule_node(&mut v, "pair")["target"] =
                json!({"kind":"module","key":{"id":"independent.wrapper","version":"1"}});
        } else {
            v["modules"][1]["graph"]["nodes"][0]["target"]["key"]["id"] =
                json!("independent.wrapper");
        }
        assert_eq!(plan(&v).err().unwrap().code, "compile_recursion");
    }
}
#[test]
fn scoped_path_vectors_do_not_collide_with_slashes_in_logical_ids() {
    let mut v = document();
    let mut constant = root_node(&mut v, "left").clone();
    constant["id"] = json!("first/nested/pair");
    v["graph"]["nodes"].as_array_mut().unwrap().push(constant);
    let p = plan(&v).unwrap();
    assert!(p.nodes().iter().any(|n| n.path == ["first/nested/pair"]));
    assert!(
        p.nodes()
            .iter()
            .any(|n| n.path == ["first", "nested", "pair"])
    );
    assert_eq!(p.nodes().len(), 8);
}
fn uint() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).unwrap()
}
struct Factory {
    creates: Arc<AtomicUsize>,
    validates: Arc<AtomicUsize>,
}
#[derive(Clone)]
struct State;
impl IncrementalProvider for Factory {
    fn validate_parameters(&self, _: &BoundParameters) -> Result<(), ProviderFailure> {
        self.validates.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn create(&self, _: &BoundParameters) -> Result<Box<dyn IncrementalState>, ProviderFailure> {
        self.creates.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(State))
    }
}
impl IncrementalState for State {
    fn fork(&self) -> Box<dyn IncrementalState> {
        Box::new(self.clone())
    }
    fn step(&mut self, c: &ProviderCall<'_>) -> Result<NamedValues, ProviderFailure> {
        Ok(vec![(
            "result".into(),
            Evaluation::Ready(c.input("value").unwrap().clone()),
        )])
    }
}
fn incremental_registry() -> (Registry, Descriptor, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let creates = Arc::new(AtomicUsize::new(0));
    let validates = Arc::new(AtomicUsize::new(0));
    let mut d = DescriptorDraft::pure(
        EntryKey {
            id: "independent.incremental".into(),
            version: "1".into(),
        },
        ContentId::from_bytes([77; 32]),
        ParameterSchema::new(1, vec![]).unwrap(),
        "independent staged identity/1",
    );
    d.purity = Purity::Stateful;
    d.readiness = ReadinessPolicy::ProviderManaged;
    d.history = 1;
    d.state_schema = Some("independent.state/1".into());
    d.required_capabilities = vec!["provider.incremental-scalar/1".into()];
    d.inputs = vec![CallablePort::one("value", uint())];
    d.outputs = vec![CallablePort::one("result", uint())];
    let d = Descriptor::new(d).unwrap();
    let mut b = RegistryBuilder::new();
    fqe_ir::builtins::register(&mut b).unwrap();
    b.register(providers::pair_descriptor(), Arc::new(providers::Pair))
        .unwrap();
    b.register(
        providers::predicate_descriptor(),
        Arc::new(providers::Above),
    )
    .unwrap();
    b.register_incremental(
        d.clone(),
        Arc::new(Factory {
            creates: creates.clone(),
            validates: validates.clone(),
        }),
    )
    .unwrap();
    (b.freeze().unwrap(), d, creates, validates)
}
fn add_incremental(v: &mut Value, d: &Descriptor, module: bool) {
    let r = d.requirement();
    v["entries"].as_array_mut().unwrap().push(json!({"key":{"id":r.key.id,"version":r.key.version},"descriptor_version":1,"role":"provider","artifact":r.artifact.to_string()}));
    let source = if module {
        json!({"input":"left"})
    } else {
        json!({"node":"left","port":"result"})
    };
    let node = json!({"id":"state","target":{"kind":"provider","key":{"id":"independent.incremental","version":"1"}},"arguments":{},"inputs":[{"name":"value","source":source}],"outputs":[{"name":"result","type":{"version":1,"shape":"scalar","kind":"u64"}}]});
    if module {
        v["modules"][0]["graph"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(node);
    } else {
        v["graph"]["nodes"].as_array_mut().unwrap().push(node);
    }
}
#[test]
fn incremental_preparation_is_pinned_and_never_creates_or_advances_state_at_compile_time() {
    let (r, d, creates, validates) = incremental_registry();
    let mut v = document();
    add_incremental(&mut v, &d, false);
    let g = load(&v);
    let p = compiler::compile_graph(&g, &g.bind(vec![]).unwrap(), &r).unwrap();
    assert_eq!(creates.load(Ordering::SeqCst), 0);
    assert_eq!(validates.load(Ordering::SeqCst), 1);
    let prepared = p
        .nodes()
        .iter()
        .find_map(|n| match &n.call {
            CompiledCall::Incremental(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(prepared.registry_id(), p.registry_id());
    drop(r);
    let a = prepared.instantiate("run", "one").unwrap();
    let b = prepared.instantiate("run", "two").unwrap();
    assert_eq!(creates.load(Ordering::SeqCst), 2);
    assert_eq!(a.accepted_steps(), 0);
    assert_eq!(b.last_sequence(), None);
}
#[test]
fn pure_modules_reject_stateful_dependencies_even_when_unreachable_from_outputs() {
    let (r, d, creates, _) = incremental_registry();
    let mut v = document();
    add_incremental(&mut v, &d, true);
    let g = load(&v);
    assert_eq!(
        compiler::compile_graph(&g, &g.bind(vec![]).unwrap(), &r)
            .err()
            .unwrap()
            .code,
        "compile_purity"
    );
    assert_eq!(creates.load(Ordering::SeqCst), 0);
}
#[test]
fn typed_graph_inputs_pass_through_nested_modules_without_hidden_constants() {
    let mut v = document();
    let t = json!({"version":1,"shape":"scalar","kind":"u64"});
    v["graph"]["inputs"] = json!([{"name":"external","type":t}]);
    root_node(&mut v, "first")["inputs"][0]["source"] = json!({"input":"external"});
    let p = plan(&v).unwrap();
    assert_eq!(p.inputs().len(), 1);
    let input = TypedValue::new(uint(), ScalarValue::U64(250)).unwrap();
    let values = compiled::evaluate(
        &p,
        BTreeMap::from([("external".into(), Evaluation::Ready(input))]),
    )
    .unwrap();
    assert!(boolean(&values["first"]));
}
#[test]
fn strict_graph_decode_rejects_unknown_duplicate_and_future_features() {
    for change in 0..9 {
        let mut v = document();
        match change {
            0 => v["schema_version"] = json!(2),
            1 => v["requirements"] = json!(["unknown/1"]),
            2 => v["workflow"] = json!({"run":true}),
            3 => root_node(&mut v, "left")["extra"] = json!(true),
            4 => root_node(&mut v, "first")["target"]["kind"] = json!("delay"),
            5 => v["modules"][0]["schema_version"] = json!(2),
            6 => {
                let n = root_node(&mut v, "first")["inputs"][0].clone();
                root_node(&mut v, "first")["inputs"]
                    .as_array_mut()
                    .unwrap()
                    .push(n);
            }
            7 => root_node(&mut v, "left")["arguments"]["value"]["literal"]["value"] = json!(125.5),
            _ => v["dependencies"] = json!(["external"]),
        }
        assert!(
            LoadedGraph::from_bytes(&serde_json::to_vec(&v).unwrap()).is_err(),
            "case {change}"
        );
    }
    assert_eq!(
        LoadedGraph::from_bytes(br#"{"format":"fqe.graph","format":"fqe.graph"}"#)
            .unwrap_err()
            .code,
        "duplicate_key"
    );
    assert!(LoadedGraph::from_bytes(&vec![b' '; 262145]).is_err());
    assert!(LoadedGraph::from_bytes(&FIXTURE[..FIXTURE.len() - 8]).is_err());
}
#[test]
fn expansion_rejects_provider_count_and_nested_depth_exhaustion() {
    let mut v = document();
    let rule = v["modules"][0].clone();
    v["modules"] = json!([rule]);
    let mut calls = vec![];
    for k in 0..129 {
        let mut call = root_node(&mut v, "second").clone();
        call["id"] = json!(format!("call{k:03}"));
        calls.push(call);
    }
    let left = root_node(&mut v, "left").clone();
    let right = root_node(&mut v, "right").clone();
    calls.extend([left, right]);
    v["graph"]["nodes"] = json!(calls);
    v["graph"]["outputs"] = json!([{"name":"out","type":{"version":1,"shape":"scalar","kind":"bool"},"source":{"node":"call000","port":"accepted"}}]);
    assert_eq!(plan(&v).err().unwrap().code, "compile_limit");
    let mut v = document();
    let original = v["modules"][1].clone();
    for k in 0..8 {
        let mut wrapper = original.clone();
        wrapper["key"]["id"] = json!(format!("depth.m{k}"));
        wrapper["graph"]["nodes"][0]["target"]["key"]["id"] = if k == 0 {
            json!("independent.wrapper")
        } else {
            json!(format!("depth.m{}", k - 1))
        };
        v["modules"].as_array_mut().unwrap().push(wrapper);
    }
    root_node(&mut v, "first")["target"]["key"]["id"] = json!("depth.m7");
    assert_eq!(plan(&v).err().unwrap().code, "compile_limit");
}
fn passthrough(depth: usize, width: usize, calls: usize) -> Value {
    let t = json!({"version":1,"shape":"scalar","kind":"u64"});
    let ports: Vec<_> = (0..width)
        .map(|i| json!({"name":format!("x{i}"),"type":t}))
        .collect();
    let wires: Vec<_> = (0..width)
        .map(|i| json!({"name":format!("x{i}"),"source":{"input":format!("x{i}")}}))
        .collect();
    let mut modules = vec![];
    for k in 0..depth {
        let nodes = if k == 0 {
            vec![]
        } else {
            vec![
                json!({"id":"next","target":{"kind":"module","key":{"id":format!("test.pass{}",k-1),"version":"1"}},"arguments":{},"inputs":wires,"outputs":ports}),
            ]
        };
        let outputs:Vec<_>=(0..width).map(|i|json!({"name":format!("x{i}"),"type":t,"source":if k==0{json!({"input":format!("x{i}")})}else{json!({"node":"next","port":format!("x{i}")})}})).collect();
        modules.push(json!({"schema_version":1,"key":{"id":format!("test.pass{k}"),"version":"1"},"parameters":{"version":1,"entries":[]},"graph":{"inputs":ports,"nodes":nodes,"outputs":outputs}}));
    }
    let nodes:Vec<_>=(0..calls).map(|k|json!({"id":format!("call{k:03}"),"target":{"kind":"module","key":{"id":format!("test.pass{}",depth-1),"version":"1"}},"arguments":{},"inputs":wires,"outputs":ports})).collect();
    json!({"format":"fqe.graph","schema_version":1,"ir_version":2,"id":"test.bounded","version":"1","parameters":{"version":1,"entries":[]},"requirements":["model.typed-graph/1","model.pure-modules/1"],"entries":[],"graph":{"inputs":ports,"nodes":nodes,"outputs":[{"name":"out","type":t,"source":{"node":"call000","port":"x0"}}]},"modules":modules,"metadata":{},"workflow":{},"state":[],"dependencies":[]})
}
#[test]
fn empty_modules_alias_inputs_and_visit_boundary_is_enforced_without_provider_nodes() {
    let p = plan(&passthrough(8, 1, 128)).unwrap();
    assert_eq!(p.nodes().len(), 0);
    let v = TypedValue::new(uint(), ScalarValue::U64(7)).unwrap();
    let results = compiled::evaluate(
        &p,
        BTreeMap::from([("x0".into(), Evaluation::Ready(v.clone()))]),
    )
    .unwrap();
    assert_eq!(results["out"], Evaluation::Ready(v));
    let e = plan(&passthrough(8, 1, 129)).err().unwrap();
    assert_eq!(e.code, "compile_limit");
    assert!(e.detail.contains("visit"));
}
#[test]
fn expanded_alias_edge_work_is_bounded_separately_from_source_graph_size() {
    let e = plan(&passthrough(8, 8, 50)).err().unwrap();
    assert_eq!(e.code, "compile_limit");
    assert!(e.detail.contains("edge"));
}
#[test]
fn legacy_source_maps_keep_original_file_positions_after_normalization() {
    let mut v: Value = serde_json::from_slice(include_bytes!("fixtures/x001-sum.json")).unwrap();
    v["graph"]["nodes"].as_array_mut().unwrap().reverse();
    let model = LoadedModel::from_bytes(&serde_json::to_vec(&v).unwrap()).unwrap();
    let p = compiler::compile_model(
        &model,
        &model.bind_json(br#"{"version":1,"values":{}}"#).unwrap(),
        &providers::registry().unwrap(),
    )
    .unwrap();
    for n in p.nodes() {
        let idx = v["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .position(|x| x["id"] == n.path[0])
            .unwrap();
        assert_eq!(n.source.definition, format!("$.graph.nodes[{idx}]"));
    }
}
#[test]
fn module_defaults_are_bound_per_invocation_and_passthroughs_have_no_hidden_state() {
    let mut v = document();
    let spec = &mut v["modules"][1]["parameters"]["entries"][0];
    spec["required"] = json!(false);
    spec["default"] = json!({"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"150"});
    root_node(&mut v, "first")["arguments"] = json!({});
    let p = plan(&v).unwrap();
    let result = compiled::evaluate(&p, BTreeMap::new()).unwrap();
    assert!(boolean(&result["first"]));
    assert!(!boolean(&result["second"]));
}
#[test]
fn real_indicator_plans_retain_price_units_and_reject_scale_or_instrument_changes() {
    use fqe_core::{InstrumentId, types::NumericUnit};
    let input = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: 12,
            unit: NumericUnit::Price(InstrumentId::new("DEMO").unwrap()),
        },
    )
    .unwrap();
    let mut builder = RegistryBuilder::new();
    fqe_ir::indicators::register(&mut builder, "unknown.market", input).unwrap();
    let registry = builder.freeze().unwrap();
    let descriptor = registry
        .descriptors()
        .find(|d| d.declaration().key.id == "unknown.market.sma")
        .unwrap();
    let requirement = descriptor.requirement();
    let t = json!({"version":1,"shape":"scalar","kind":"decimal","scale":12,"unit":{"kind":"price","instrument":"DEMO"}});
    let v = json!({"format":"fqe.graph","schema_version":1,"ir_version":2,"id":"test.price","version":"1","parameters":{"version":1,"entries":[]},"requirements":["model.typed-graph/1"],"entries":[{"key":{"id":requirement.key.id,"version":"1"},"descriptor_version":1,"role":"provider","artifact":requirement.artifact.to_string()}],"graph":{"inputs":[{"name":"close","type":t}],"nodes":[{"id":"average","target":{"kind":"provider","key":{"id":descriptor.declaration().key.id,"version":"1"}},"arguments":{"period":{"literal":{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"2"}}},"inputs":[{"name":"close","source":{"input":"close"}}],"outputs":[{"name":"value","type":t}]}],"outputs":[{"name":"average","type":t,"source":{"node":"average","port":"value"}}]},"modules":[],"metadata":{},"workflow":{},"state":[],"dependencies":[]});
    let g = load(&v);
    let p = compiler::compile_graph(&g, &g.bind(vec![]).unwrap(), &registry).unwrap();
    assert!(matches!(p.nodes()[0].call, CompiledCall::Incremental(_)));
    assert_eq!(
        p.outputs()[0].port.value_type,
        descriptor.declaration().outputs[0].value_type
    );
    for change in 0..2 {
        let mut bad = v.clone();
        if change == 0 {
            bad["graph"]["inputs"][0]["type"]["scale"] = json!(11);
        } else {
            bad["graph"]["inputs"][0]["type"]["unit"]["instrument"] = json!("OTHER");
        }
        let g = load(&bad);
        assert_eq!(
            compiler::compile_graph(&g, &g.bind(vec![]).unwrap(), &registry)
                .err()
                .unwrap()
                .code,
            "compile_type"
        );
    }
}
#[test]
fn compiled_pure_modules_map_to_the_existing_typed_expression_engine() {
    let p = plan(&document()).unwrap();
    let mut evaluator = fqe_ir::expressions::ExpressionInstance::new(
        Arc::new(p.pure_expressions().unwrap()),
        "compiled",
        "expression",
    )
    .unwrap();
    let result = evaluator.evaluate(1, vec![]).unwrap();
    assert_eq!(
        result.outputs,
        compiled::evaluate(&p, BTreeMap::new()).unwrap()
    );
    let (r, d, _, _) = incremental_registry();
    let mut v = document();
    add_incremental(&mut v, &d, false);
    let g = load(&v);
    let p = compiler::compile_graph(&g, &g.bind(vec![]).unwrap(), &r).unwrap();
    assert_eq!(p.pure_expressions().err().unwrap().code, "compile_profile");
}
#[test]
fn repeated_pure_dependencies_cannot_expand_into_an_unbounded_expression_tree() {
    let mut v = document();
    v["modules"] = json!([]);
    let left = root_node(&mut v, "left").clone();
    let mut pair = rule_node(&mut document(), "pair").clone();
    pair["id"] = json!("n00");
    let mut nodes = vec![left];
    for k in 0..20 {
        let mut n = pair.clone();
        n["id"] = json!(format!("n{k:02}"));
        let source = if k == 0 {
            json!({"node":"left","port":"result"})
        } else {
            json!({"node":format!("n{:02}",k-1),"port":"sum"})
        };
        n["inputs"][0]["source"] = source.clone();
        n["inputs"][1]["source"] = source;
        nodes.push(n);
    }
    v["graph"]["nodes"] = json!(nodes);
    v["graph"]["outputs"] = json!([{"name":"out","type":{"version":1,"shape":"scalar","kind":"u64"},"source":{"node":"n19","port":"sum"}}]);
    let p = plan(&v).unwrap();
    assert_eq!(p.nodes().len(), 21);
    assert_eq!(p.pure_expressions().err().unwrap().code, "compile_limit");
}
