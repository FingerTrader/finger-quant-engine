use fqe_ir::{
    catalog,
    exchange::{MarketData, MarketRecord},
    workflow::LoadedWorkflow,
};
use fqe_runtime::{events::InputEvent, workflow::WorkflowRuntime};
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loaded = LoadedWorkflow::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/workflow-counter.json"
    ))?;
    let registry = catalog::reference(loaded.event_plan().trigger())?;
    let plan = Arc::new(loaded.compile(&registry, vec![])?);
    let data = MarketData::from_bytes(include_bytes!("../tests/fixtures/p2/x002-data.json"))?;
    let mut runtime = WorkflowRuntime::new(plan, "workflow-example", data.source_id())?;
    for record in data.events() {
        let event = match record {
            MarketRecord::Tick(t) => InputEvent::Tick(t.clone()),
            MarketRecord::Bar(b) => InputEvent::from(b.input().clone()),
        };
        let observation = runtime.step(event)?;
        println!(
            "sequence={} warmup={} transitions={:?} signals={:?}",
            observation.source_sequence,
            observation.warmup,
            observation.transitions,
            observation.signals
        );
    }
    println!("final={:?}", runtime.finish()?);
    println!("{}", String::from_utf8(runtime.inspect_state()?)?);
    Ok(())
}
