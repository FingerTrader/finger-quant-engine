#[allow(dead_code)]
#[path = "support/backtests.rs"]
mod support;
use fqe_ir::{catalog, event_plan::LoadedEventPlan};
use fqe_runtime::{backtest::*, checkpoint_file, financial_checkpoint::FinancialSession};
use std::{io::Write, path::PathBuf, sync::Arc};
fn prepared() -> Arc<PreparedBacktest> {
    let loaded =
        LoadedEventPlan::from_bytes(include_bytes!("../tests/fixtures/p3/backtest-events.json"))
            .unwrap();
    let strategy = StrategyPlan::Event(Arc::new(
        loaded
            .compile(&catalog::reference(loaded.trigger()).unwrap(), vec![])
            .unwrap(),
    ));
    Arc::new(
        PreparedBacktest::new(
            support::selection(),
            strategy,
            support::policies(),
            support::config(),
            "financial.checkpoint.example",
            7,
            RunControl::Complete,
        )
        .unwrap(),
    )
}
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let p = prepared();
    if args.len() == 2 && args[0] == "--resume" {
        let bytes = checkpoint_file::read_financial(&args[1])?;
        let mut s = FinancialSession::restore(p, &bytes)?;
        for _ in s.source_cursor()..7 {
            s.advance()?;
        }
        std::io::stdout().write_all(s.finish()?.json())?;
        return Ok(());
    }
    if !args.is_empty() {
        return Err("expected no arguments or --resume checkpoint-path".into());
    }
    let path = std::env::temp_dir().join(format!(
        "fqe-financial-example-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&path)?;
    let dir = Directory(path);
    let file = dir.0.join("checkpoint.json");
    let expected = p.run()?;
    let mut s = FinancialSession::new(p)?;
    for _ in 0..5 {
        s.advance()?;
    }
    assert_eq!(s.current().broker().ledger().reservations().len(), 1);
    checkpoint_file::prepare_financial(&file, &s.checkpoint()?)?.commit()?;
    drop(s);
    let output = std::process::Command::new(std::env::current_exe()?)
        .arg("--resume")
        .arg(&file)
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    assert_eq!(output.stdout, expected.json());
    println!(
        "restored_in_fresh_child=true source_prefix=5 pending_hold=1 continued_bars=2 report={} final_equity=97.00 fees=1.00",
        expected.content_id()
    );
    Ok(())
}
