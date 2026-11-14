use fqe_ir::{loading::LoadedModel, parameters::BoundValue};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/model.json").into()
    });
    let m = LoadedModel::from_file(path)?;
    println!(
        "model={} version={} content={} source={}",
        m.definition().declaration().id,
        m.definition().declaration().version,
        m.content_id(),
        m.source_id()
    );
    for bytes in [br#"{"version":1,"values":{}}"#.as_slice(),br#"{"version":1,"values":{"period":{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"5"}}}"#]{let b=m.bind_json(bytes)?;let Some(BoundValue::Value(v))=b.parameters().get("period")else{return Err("expected fixture period".into())};println!("bound period={:?}",v.value());b.ensure_matches(&m)?;}
    assert!(
        m.bind_json(br#"{"version":1,"values":{"period":null}}"#)
            .is_err()
    );
    println!("null rejected; declarations loaded, no strategy executed");
    Ok(())
}
