#[path = "support/optimizations.rs"]
mod support;
use fqe_core::finance::decimal_text;
use fqe_runtime::optimization::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = PreparedOptimization::new(support::request())?.run()?;
    if std::env::args().any(|v| v == "--json") {
        use std::io::Write;
        std::io::stdout().write_all(result.json())?;
    } else {
        println!(
            "report={} manifest={} trials={} selected={}",
            result.content_id(),
            result.manifest_id(),
            result.trials().len(),
            result.selected().unwrap()
        );
        for t in result.trials() {
            if let TrialOutcome::Completed(r) = &t.outcome {
                println!(
                    "trial={} parameters={:?} training_profit={}",
                    t.id,
                    t.parameters,
                    decimal_text(r.metrics().net_profit)
                );
            }
        }
        if let TrialOutcome::Completed(v) = result.validation() {
            println!(
                "separate_validation_profit={} equity={} fills={}",
                decimal_text(v.metrics().net_profit),
                decimal_text(v.metrics().final_equity),
                v.broker().fills().len()
            );
        }
    }
    Ok(())
}
