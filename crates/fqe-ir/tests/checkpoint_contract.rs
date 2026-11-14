use fqe_ir::{
    checkpoint::{JournalEntry, SnapshotCommand, SnapshotDocument},
    model::ContentId,
};
use serde_json::{Value, json};
const SOURCE: &str = include_str!("fixtures/checkpoint-independent.json");
fn invalid(f: impl FnOnce(&mut Value)) {
    let mut v: Value = serde_json::from_str(SOURCE).unwrap();
    f(&mut v);
    assert!(SnapshotDocument::from_bytes(&serde_json::to_vec(&v).unwrap()).is_err());
}
#[test]
fn independently_encoded_checksum_canonical_and_full_width_values() {
    let doc = SnapshotDocument::from_bytes(SOURCE.as_bytes()).unwrap();
    assert_eq!(
        doc.to_bytes().unwrap(),
        include_bytes!("fixtures/checkpoint-independent.canonical.json")
    );
    assert_eq!(doc.output_cursor(), 1);
    match &doc.entries[0].command {
        SnapshotCommand::Ingest {
            record: fqe_ir::exchange::MarketRecord::Tick(e),
            ..
        } => {
            assert_eq!(e.sequence(), u64::MAX);
            assert_eq!(e.timestamp_ns(), u64::MAX);
            assert_eq!(e.tick().price.get(), u64::MAX);
        }
        _ => panic!(),
    }
    assert_eq!(
        SnapshotDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap(),
        doc
    );
}
#[test]
fn strict_checkpoint_versions_shape_integrity_cursors_and_portable_data() {
    invalid(|v| v["schema_version"] = json!(2));
    invalid(|v| v["unknown"] = json!(true));
    invalid(|v| v["materialized_state"] = json!(true));
    invalid(|v| v["state_representation"] = json!("java-serialization"));
    invalid(|v| v["checksum"] = json!(format!("sha256:{}", "00".repeat(32))));
    invalid(|v| v["output_cursor"] = json!("0"));
    invalid(|v| v["journal"][0]["output_cursor"] = json!("2"));
    invalid(|v| v["journal"][0]["command"]["record"]["price"] = json!("0"));
    invalid(|v| v["journal"][0]["command"]["record"]["sequence"] = json!(1));
    invalid(|v| v["journal"][0]["command"]["pointer"] = json!("0x123"));
    invalid(|v| v["journal"][0]["command"]["op"] = json!("shell"));
    invalid(|v| {
        let p = v["pins"][0].clone();
        v["pins"].as_array_mut().unwrap().push(p);
    });
    assert!(SnapshotDocument::from_bytes(&SOURCE.as_bytes()[..SOURCE.len() - 1_000]).is_err());
    assert!(
        SnapshotDocument::from_bytes(
            SOURCE
                .replacen(
                    "\"schema_version\": 1",
                    "\"schema_version\":1,\"schema_version\":1",
                    1
                )
                .as_bytes()
        )
        .is_err()
    );
}
#[test]
fn checkpoint_admission_bounds_and_programmatic_command_validation() {
    let mut doc = SnapshotDocument::from_bytes(SOURCE.as_bytes()).unwrap();
    doc.entries = vec![
        JournalEntry {
            command: SnapshotCommand::Drive,
            outcome_id: ContentId::from_bytes([0; 32]),
            output_cursor: 0
        };
        1025
    ];
    assert!(doc.to_bytes().is_err());
    doc.entries.truncate(1);
    doc.entries[0].command = SnapshotCommand::Seal {
        stream: "invalid name".into(),
    };
    assert!(doc.to_bytes().is_err());
    doc.entries.clear();
    for i in 0..100 {
        doc.pins.insert(vec![format!("big{i}")], "a".repeat(4096));
    }
    assert!(doc.to_bytes().is_err());
}
