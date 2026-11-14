use fqe_core::types::{
    Evaluation, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
};
use fqe_ir::{
    model::*,
    parameters::{BoundValue, Constraints, ParameterInput, ParameterSchema, ParameterSpec},
};
use std::collections::BTreeMap;

fn u64_type() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).unwrap()
}
fn value(n: u64) -> TypedValue {
    TypedValue::new(u64_type(), ScalarValue::U64(n)).unwrap()
}
fn key() -> EntryKey {
    EntryKey {
        id: "example.custom".into(),
        version: "1.0".into(),
    }
}
fn draft() -> ModelDraft {
    ModelDraft {
        schema_version: 1,
        ir_version: 2,
        id: "example.model".into(),
        version: "1.0".into(),
        parameters: ParameterSchema::new(
            1,
            vec![
                ParameterSpec::new(
                    "period",
                    u64_type(),
                    false,
                    Some(value(2)),
                    Constraints::default(),
                    "History length",
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        requirements: vec!["values/1".into(), "model.scalar-graph/1".into()],
        entries: vec![EntryRequirement {
            key: key(),
            descriptor_version: 1,
            role: "provider".into(),
            artifact: ContentId::from_bytes([7; 32]),
        }],
        nodes: vec![Node {
            id: "n".into(),
            entry: key(),
            arguments: BTreeMap::from([("period".into(), Argument::Parameter("period".into()))]),
            inputs: vec![],
            outputs: vec![Port {
                name: "value".into(),
                value_type: u64_type(),
            }],
        }],
        outputs: vec![Input {
            name: "signal".into(),
            source: PortRef {
                node: "n".into(),
                port: "value".into(),
            },
        }],
        metadata: BTreeMap::from([("label".into(), "示例".into())]),
    }
}

#[test]
fn model_definitions_and_bindings_are_independent_and_immutable() {
    let model = ModelDefinition::new(draft()).unwrap();
    let default = model.bind([]).unwrap();
    let changed = model
        .bind([(
            "period".into(),
            ParameterInput::Value(Evaluation::Ready(value(5))),
        )])
        .unwrap();
    assert_eq!(default.get("period"), Some(&BoundValue::Value(value(2))));
    assert_eq!(changed.get("period"), Some(&BoundValue::Value(value(5))));
    assert_eq!(model.bind([]).unwrap(), default);
    assert_eq!(model.declaration().id, "example.model");
}

#[test]
fn declarations_are_normalized_by_identity_without_reordering_port_meaning() {
    let mut a = draft();
    let mut other = a.nodes[0].clone();
    other.id = "a".into();
    a.nodes.push(other);
    a.nodes[0].outputs.push(Port {
        name: "alpha".into(),
        value_type: u64_type(),
    });
    let mut b = a.clone();
    b.nodes.reverse();
    b.requirements.reverse();
    b.nodes[1].outputs.reverse();
    assert_eq!(
        ModelDefinition::new(a).unwrap(),
        ModelDefinition::new(b).unwrap()
    );
}

#[test]
fn unknown_versions_capabilities_and_roles_are_not_silently_accepted() {
    let mut d = draft();
    d.schema_version = 0;
    assert_eq!(ModelDefinition::new(d).unwrap_err().code, "version");
    let mut d = draft();
    d.ir_version = 1;
    assert_eq!(ModelDefinition::new(d).unwrap_err().code, "version");
    let mut d = draft();
    d.requirements.push("remote.compute/1".into());
    assert_eq!(ModelDefinition::new(d).unwrap_err().code, "capability");
    let mut d = draft();
    d.entries[0].descriptor_version = 2;
    assert_eq!(ModelDefinition::new(d).unwrap_err().code, "version");
    let mut d = draft();
    d.entries[0].role = "runner".into();
    assert_eq!(ModelDefinition::new(d).unwrap_err().code, "unsupported");
}

#[test]
fn duplicate_entry_node_port_output_and_capability_identities_fail() {
    let mut ds = vec![];
    let mut d = draft();
    d.entries.push(d.entries[0].clone());
    ds.push(d);
    let mut d = draft();
    d.nodes.push(d.nodes[0].clone());
    ds.push(d);
    let mut d = draft();
    let duplicate = d.nodes[0].outputs[0].clone();
    d.nodes[0].outputs.push(duplicate);
    ds.push(d);
    let mut d = draft();
    d.outputs.push(d.outputs[0].clone());
    ds.push(d);
    let mut d = draft();
    d.requirements.push(d.requirements[0].clone());
    ds.push(d);
    for d in ds {
        assert_eq!(ModelDefinition::new(d).unwrap_err().code, "duplicate_id");
    }
}

#[test]
fn missing_entry_parameter_node_and_port_references_fail_structurally() {
    let mut ds = vec![];
    let mut d = draft();
    d.nodes[0].entry.version = "2".into();
    ds.push(d);
    let mut d = draft();
    d.nodes[0]
        .arguments
        .insert("p".into(), Argument::Parameter("undeclared".into()));
    ds.push(d);
    let mut d = draft();
    d.outputs[0].source.node = "missing".into();
    ds.push(d);
    let mut d = draft();
    d.outputs[0].source.port = "missing".into();
    ds.push(d);
    for d in ds {
        assert_eq!(ModelDefinition::new(d).unwrap_err().code, "reference");
    }
}

#[test]
fn unbound_optional_graph_parameter_cannot_be_prepared_for_execution() {
    let mut d = draft();
    d.parameters = ParameterSchema::new(
        1,
        vec![
            ParameterSpec::new(
                "period",
                u64_type(),
                false,
                None,
                Constraints::default(),
                "",
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let model = ModelDefinition::new(d).unwrap();
    assert_eq!(model.bind([]).unwrap_err().code, "unbound");
    assert!(
        model
            .bind([(
                "period".into(),
                ParameterInput::Value(Evaluation::Ready(value(3)))
            )])
            .is_ok()
    );
}

#[test]
fn invalid_identifiers_and_declared_resource_limits_fail() {
    let mut d = draft();
    d.id = "bad id".into();
    assert_eq!(ModelDefinition::new(d).unwrap_err().code, "identifier");
    let mut d = draft();
    d.nodes.clear();
    assert_eq!(ModelDefinition::new(d).unwrap_err().code, "limit");
    let mut d = draft();
    d.nodes[0].outputs = (0..33)
        .map(|i| Port {
            name: format!("p{i}"),
            value_type: u64_type(),
        })
        .collect();
    assert_eq!(ModelDefinition::new(d).unwrap_err().code, "limit");
    let mut d = draft();
    d.metadata.insert("label".into(), "x".repeat(4097));
    assert_eq!(ModelDefinition::new(d).unwrap_err().code, "limit");
}

#[test]
fn digest_text_has_one_exact_algorithm_and_lowercase_representation() {
    let id = ContentId::from_bytes([0xab; 32]);
    assert_eq!(ContentId::parse(&id.to_string()).unwrap(), id);
    assert_eq!(id.to_string(), format!("sha256:{}", "ab".repeat(32)));
    for bad in [
        "sha256:00".to_string(),
        format!("sha256:{}", "AB".repeat(32)),
        format!("md5:{}", "ab".repeat(32)),
        format!("sha256:{}", "资".repeat(64)),
    ] {
        assert!(ContentId::parse(&bad).is_err());
    }
}
