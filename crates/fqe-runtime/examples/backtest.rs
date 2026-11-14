#[path = "support/backtests.rs"]
mod support;
use fqe_core::finance::decimal_text;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = support::prepared().run()?;
    if std::env::args().any(|a| a == "--json") {
        use std::io::Write;
        std::io::stdout().write_all(result.json())?;
    } else {
        println!(
            "report={} manifest={} bars={} orders={} fills={}",
            result.content_id(),
            result.manifest_id(),
            result.observations().len(),
            result.broker().orders().len(),
            result.broker().fills().len()
        );
        println!(
            "cash={} equity={} fees={} net_profit={} total_return={} max_drawdown={} max_drawdown_ratio={}",
            decimal_text(result.broker().ledger().view().cash),
            decimal_text(result.metrics().final_equity),
            decimal_text(result.metrics().fees),
            decimal_text(result.metrics().net_profit),
            decimal_text(result.metrics().total_return.unwrap()),
            decimal_text(result.metrics().max_drawdown),
            decimal_text(result.metrics().max_drawdown_ratio.unwrap())
        );
    }
    Ok(())
}
