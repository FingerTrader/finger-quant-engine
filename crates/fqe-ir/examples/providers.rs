//! Explicitly driven fixture calls, not a graph compiler or event runtime.
#[path = "support/providers.rs"]
mod providers;
use fqe_core::types::Evaluation;
use fqe_ir::{
    loading::LoadedModel,
    registry::{CallContext, Registry},
};
fn context(call_sequence: u64) -> CallContext {
    CallContext {
        run: "example.run".into(),
        instance: "example.instance".into(),
        event_sequence: 1,
        call_sequence,
    }
}
fn composition(registry: &Registry, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let model = LoadedModel::from_bytes(bytes)?;
    let binding = model.bind_json(br#"{"version":1,"values":{}}"#)?;
    let left = registry
        .prepare_node(&model, &binding, "left")?
        .invoke(context(1), vec![])?;
    let right = registry
        .prepare_node(&model, &binding, "right")?
        .invoke(context(2), vec![])?;
    let pair = registry.prepare_node(&model, &binding, "pair")?.invoke(
        context(3),
        vec![
            ("left".into(), left.outputs()["result"].clone()),
            ("right".into(), right.outputs()["result"].clone()),
        ],
    )?;
    let predicate = model
        .definition()
        .declaration()
        .nodes
        .iter()
        .find(|n| n.id == "predicate")
        .ok_or("fixture predicate missing")?;
    let port = &predicate.inputs[0].source.port;
    let above = registry
        .prepare_node(&model, &binding, "predicate")?
        .invoke(
            context(4),
            vec![("value".into(), pair.outputs()[port].clone())],
        )?;
    let inverted = registry.prepare_node(&model, &binding, "not")?.invoke(
        context(5),
        vec![("value".into(), above.outputs()["result"].clone())],
    )?;
    assert!(matches!(above.outputs()["result"], Evaluation::Ready(_)));
    println!(
        "model={} pair={:?} predicate={:?} not={:?}",
        model.content_id(),
        pair.outputs(),
        above.outputs()["result"],
        inverted.outputs()["result"]
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = providers::registry()?;
    println!(
        "registry={} entries={}",
        registry.content_id(),
        registry.descriptors().count()
    );
    composition(&registry, include_bytes!("../tests/fixtures/x001-sum.json"))?;
    composition(
        &registry,
        include_bytes!("../tests/fixtures/x001-difference.json"),
    )?;
    println!(
        "two pure compositions passed through shared registry; no graph/event runtime or external process"
    );
    Ok(())
}
