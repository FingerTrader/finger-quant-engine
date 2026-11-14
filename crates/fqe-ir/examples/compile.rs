#[path = "support/compiled.rs"]
mod compiled;
#[path = "support/providers.rs"]
mod providers;
use fqe_ir::{compiler, graph::LoadedGraph, loading::LoadedModel};
use std::collections::BTreeMap;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = providers::registry()?;
    for (name, bytes) in [
        (
            "sum",
            include_bytes!("../tests/fixtures/x001-sum.json").as_slice(),
        ),
        (
            "difference",
            include_bytes!("../tests/fixtures/x001-difference.json").as_slice(),
        ),
    ] {
        let model = LoadedModel::from_bytes(bytes)?;
        let binding = model.bind_json(br#"{"version":1,"values":{}}"#)?;
        let plan = compiler::compile_model(&model, &binding, &registry)?;
        println!(
            "legacy {name}: plan={} nodes={} outputs={:?}",
            plan.content_id(),
            plan.nodes().len(),
            compiled::evaluate(&plan, BTreeMap::new())?
        );
    }
    for (name, bytes) in [
        (
            "sum",
            include_bytes!("../tests/fixtures/x001-graph-sum.json").as_slice(),
        ),
        (
            "difference",
            include_bytes!("../tests/fixtures/x001-graph-difference.json").as_slice(),
        ),
    ] {
        let model = LoadedGraph::from_bytes(bytes)?;
        let binding = model.bind(vec![])?;
        let plan = compiler::compile_graph(&model, &binding, &registry)?;
        println!(
            "nested/reused {name}: plan={} nodes={} outputs={:?}",
            plan.content_id(),
            plan.nodes().len(),
            compiled::evaluate(&plan, BTreeMap::new())?
        );
    }
    println!("generic compiled pure compositions; incremental event execution remains P1-08");
    Ok(())
}
