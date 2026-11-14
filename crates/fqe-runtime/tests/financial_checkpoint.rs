#[allow(dead_code)]
#[path = "../examples/support/backtests.rs"]
mod support;
use fqe_core::finance::parse_decimal;
use fqe_ir::{
    catalog,
    event_plan::LoadedEventPlan,
    financial_exchange::{self as json, JsonValue as V},
    workflow::LoadedWorkflow,
};
use fqe_runtime::{
    backtest::*, checkpoint_file, financial_checkpoint::*, ledger::LedgerProfile,
    margin::MarginRules,
};
use std::{path::PathBuf, sync::Arc};
fn prepared(workflow: bool, margin: bool, partial: bool) -> Arc<PreparedBacktest> {
    prepared_limited(workflow, margin, partial, 2048)
}
fn prepared_limited(
    workflow: bool,
    margin: bool,
    partial: bool,
    fills: usize,
) -> Arc<PreparedBacktest> {
    let strategy = if workflow {
        let loaded =
            LoadedWorkflow::from_bytes(include_bytes!("fixtures/p3/backtest-workflow.json"))
                .unwrap();
        StrategyPlan::Workflow(Arc::new(
            loaded
                .compile(
                    &catalog::reference(loaded.event_plan().trigger()).unwrap(),
                    vec![],
                )
                .unwrap(),
        ))
    } else {
        let loaded =
            LoadedEventPlan::from_bytes(include_bytes!("fixtures/p3/backtest-events.json"))
                .unwrap();
        StrategyPlan::Event(Arc::new(
            loaded
                .compile(&catalog::reference(loaded.trigger()).unwrap(), vec![])
                .unwrap(),
        ))
    };
    let mut c = support::config();
    c.limits.fills = fills;
    if partial {
        c.participation_bps = 5000;
    }
    if margin {
        c.account.profile = LedgerProfile::MarginLong;
        c.margin = Some(MarginRules {
            max_leverage_bps: 20000,
            maintenance_bps: 2500,
        });
        c.account.initial_cash = parse_decimal("5.00", 2).unwrap();
    }
    let mut p = support::policies();
    if let PolicyAction::BuyIfFlat { cash_cap, .. } = &mut p[0].action {
        *cash_cap = parse_decimal("8.50", 2).unwrap();
    }
    Arc::new(
        PreparedBacktest::new(
            support::selection(),
            strategy,
            p,
            c,
            "restore.test",
            7,
            RunControl::Complete,
        )
        .unwrap(),
    )
}
#[test]
fn all_cash_margin_event_workflow_and_partial_source_splits_continue_byte_identically() {
    for workflow in [false, true] {
        for margin in [false, true] {
            for partial in [false, true] {
                let p = prepared(workflow, margin, partial);
                let full = p.run().unwrap();
                let mut original = FinancialSession::new(Arc::clone(&p)).unwrap();
                assert_eq!(original.current().terminal(), BacktestTerminal::Paused);
                for cursor in 0..=7 {
                    assert_eq!(original.source_cursor(), cursor);
                    let bytes = original.checkpoint().unwrap();
                    let mut restored = FinancialSession::restore(Arc::clone(&p), &bytes).unwrap();
                    assert_eq!(restored.current().json(), original.current().json());
                    for n in cursor + 1..=7 {
                        let new = restored.advance().unwrap();
                        let seq = match new {
                            StrategyObservation::Event(v) => v.source_sequence,
                            StrategyObservation::Workflow(v) => v.source_sequence,
                        };
                        assert_eq!(seq, n as u64);
                    }
                    assert_eq!(restored.finish().unwrap().json(), full.json());
                    assert!(restored.finished());
                    assert!(restored.advance().is_err());
                    assert!(restored.checkpoint().is_err());
                    if cursor < 7 {
                        original.advance().unwrap();
                    }
                }
            }
        }
    }
}
#[test]
fn live_holds_partial_fills_debt_and_no_finish_side_effects_survive_paused_restore() {
    let p = prepared(false, true, true);
    let mut session = FinancialSession::new(Arc::clone(&p)).unwrap();
    assert!(session.finish().is_err());
    for _ in 0..5 {
        session.advance().unwrap();
    }
    let b = session.current().broker();
    assert!(!b.finished());
    assert_eq!(b.ledger().view().debt, parse_decimal("3.50", 2).unwrap());
    assert_eq!(
        b.ledger().view().reserved_cash,
        parse_decimal("8.50", 2).unwrap()
    );
    assert!(b.fills().is_empty());
    let mut session = FinancialSession::restore(p, &session.checkpoint().unwrap()).unwrap();
    session.advance().unwrap();
    assert_eq!(session.current().broker().fills().len(), 1);
    assert_eq!(
        session.current().broker().fills()[0].quantity,
        parse_decimal("1.000", 3).unwrap()
    );
    assert!(
        !session
            .current()
            .broker()
            .ledger()
            .reservations()
            .is_empty()
    );
    assert!(!session.current().broker().finished());
}
#[test]
fn default_deny_capability_unknown_corrupt_versions_cursor_and_full_input_pins() {
    assert_eq!(
        FinancialSession::new(Arc::new(support::prepared()))
            .err()
            .unwrap()
            .code,
        "financial_checkpoint_capability"
    );
    let p = prepared(false, false, false);
    let session = FinancialSession::new(Arc::clone(&p)).unwrap();
    let bytes = session.checkpoint().unwrap();
    for malformed in [
        b"".as_slice(),
        b"{}",
        &bytes[..bytes.len() - 1],
        b"{\"format\":1,\"format\":1}",
    ] {
        assert!(FinancialSession::restore(Arc::clone(&p), malformed).is_err());
    }
    for key in [
        "format",
        "schema_version",
        "representation",
        "manifest_id",
        "engine_id",
        "source_bars",
        "prefix_id",
        "content_id",
    ] {
        let mut v = json::decode_input(&bytes).unwrap();
        v[key] = V::from("bad");
        assert!(
            FinancialSession::restore(Arc::clone(&p), &json::encode(&v, 262144).unwrap()).is_err()
        );
    }
    let mut v = json::decode_input(&bytes).unwrap();
    v["unexpected"] = V::Bool(true);
    assert!(FinancialSession::restore(Arc::clone(&p), &json::encode(&v, 262144).unwrap()).is_err());
    let mut cp = FinancialCheckpoint::from_bytes(&bytes).unwrap();
    cp.source_bars = 1;
    assert_eq!(
        FinancialSession::restore(Arc::clone(&p), &cp.bytes().unwrap())
            .err()
            .unwrap()
            .code,
        "financial_checkpoint_divergence"
    );
    cp.source_bars = 8;
    assert_eq!(
        FinancialSession::restore(Arc::clone(&p), &cp.bytes().unwrap())
            .err()
            .unwrap()
            .code,
        "financial_checkpoint_pins"
    );
    cp.source_bars = 257;
    assert!(FinancialCheckpoint::from_bytes(&cp.bytes().unwrap()).is_err());
    let other = prepared(false, true, false);
    assert_eq!(
        FinancialSession::restore(other, &bytes).err().unwrap().code,
        "financial_checkpoint_pins"
    );
}
#[test]
fn failed_bar_preserves_entire_committed_strategy_and_financial_prefix() {
    let p = prepared_limited(false, false, false, 1);
    let mut session = FinancialSession::new(Arc::clone(&p)).unwrap();
    for _ in 0..6 {
        session.advance().unwrap();
    }
    let bytes = session.checkpoint().unwrap();
    let report = session.current().json().to_vec();
    assert_eq!(session.advance().unwrap_err().code, "simulation_limit");
    assert_eq!(session.source_cursor(), 6);
    assert_eq!(session.checkpoint().unwrap(), bytes);
    assert_eq!(session.current().json(), report);
    let mut restored = FinancialSession::restore(p, &bytes).unwrap();
    assert_eq!(restored.advance().unwrap_err().code, "simulation_limit");
    assert_eq!(restored.current().json(), report);
}
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn directory() -> Temp {
    let p = std::env::temp_dir().join(format!(
        "fqe-financial-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&p).unwrap();
    Temp(p)
}
#[test]
fn financial_persistence_roundtrip_private_atomic_publish_and_actual_child_exit() {
    let d = directory();
    let path = d.0.join("state.json");
    let p = prepared(false, true, false);
    let mut session = FinancialSession::new(Arc::clone(&p)).unwrap();
    for _ in 0..5 {
        session.advance().unwrap();
    }
    let old = session.checkpoint().unwrap();
    checkpoint_file::prepare_financial(&path, &old)
        .unwrap()
        .commit()
        .unwrap();
    session.advance().unwrap();
    let new = session.checkpoint().unwrap();
    let staged = checkpoint_file::prepare_financial(&path, &new).unwrap();
    assert_eq!(checkpoint_file::read_financial(&path).unwrap(), old);
    drop(staged);
    assert_eq!(std::fs::read_dir(&d.0).unwrap().count(), 1);
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "financial_checkpoint_process_child",
            "--nocapture",
        ])
        .env("FQE_FINANCIAL_CHILD_TARGET", &path)
        .env(
            "FQE_FINANCIAL_CHILD_BYTES",
            String::from_utf8(new.clone()).unwrap(),
        )
        .status()
        .unwrap();
    assert_eq!(child.code(), Some(73));
    assert_eq!(checkpoint_file::read_financial(&path).unwrap(), old);
    let mut restored =
        FinancialSession::restore(p, &checkpoint_file::read_financial(&path).unwrap()).unwrap();
    restored.advance().unwrap();
    assert_eq!(restored.current().json(), session.current().json());
    checkpoint_file::prepare_financial(&path, &new)
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(checkpoint_file::read_financial(&path).unwrap(), new);
    assert!(checkpoint_file::read(&path).is_err());
    assert!(checkpoint_file::prepare_financial(d.0.join("bad"), b"{}").is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let symlink = d.0.join("link");
        std::os::unix::fs::symlink(&path, &symlink).unwrap();
        assert!(checkpoint_file::read_financial(&symlink).is_err());
        assert!(checkpoint_file::prepare_financial(&symlink, &new).is_err());
    }
}
#[test]
fn financial_checkpoint_process_child() {
    if let Ok(path) = std::env::var("FQE_FINANCIAL_CHILD_TARGET") {
        let bytes = std::env::var("FQE_FINANCIAL_CHILD_BYTES").unwrap();
        let _pending = checkpoint_file::prepare_financial(path, bytes.as_bytes()).unwrap();
        std::process::exit(73);
    }
}
#[test]
fn close_maintenance_pending_forced_sale_and_liquidated_state_survive_every_split() {
    use fqe_ir::history::{HistoricalDataset, HistorySpec};
    let data = Arc::new(
        HistoricalDataset::from_csv(
            Arc::new(
                HistorySpec::from_bytes(include_bytes!("fixtures/p3/margin-history-spec.json"))
                    .unwrap(),
            ),
            include_bytes!("fixtures/p3/margin-history.csv"),
            None,
        )
        .unwrap(),
    );
    let selection = data
        .select(
            data.bars()[0].input().open_ns,
            data.bars()[2].input().close_ns,
            0,
        )
        .unwrap();
    let loaded =
        LoadedEventPlan::from_bytes(include_bytes!("fixtures/p3/financial-risk-events.json"))
            .unwrap();
    let plan = Arc::new(
        loaded
            .compile(&catalog::reference(loaded.trigger()).unwrap(), vec![])
            .unwrap(),
    );
    let mut config = support::config();
    config.account.initial_cash = parse_decimal("1000.00", 2).unwrap();
    config.account.profile = LedgerProfile::MarginLong;
    config.margin = Some(MarginRules {
        max_leverage_bps: 20000,
        maintenance_bps: 2500,
    });
    config.minimum_fee = parse_decimal("0.00", 2).unwrap();
    let policy = PolicyBinding {
        id: "entry".into(),
        source: PolicySource::LongSignal,
        action: PolicyAction::BuyIfFlat {
            quantity: parse_decimal("20.000", 3).unwrap(),
            cash_cap: parse_decimal("2000.00", 2).unwrap(),
        },
    };
    let p = Arc::new(
        PreparedBacktest::new(
            selection,
            StrategyPlan::Event(plan),
            vec![policy],
            config,
            "risk.restore",
            0,
            RunControl::Complete,
        )
        .unwrap(),
    );
    let full = p.run().unwrap();
    assert_eq!(
        full.metrics().final_equity,
        parse_decimal("100.00", 2).unwrap()
    );
    assert_eq!(
        full.broker().margin_status(),
        Some(fqe_runtime::margin::MarginStatus::Liquidated)
    );
    assert!(
        full.decisions()
            .iter()
            .any(|d| d.outcome == DecisionOutcome::RiskDisabled && d.source_sequence == 3)
    );
    let mut s = FinancialSession::new(Arc::clone(&p)).unwrap();
    for cursor in 0..=3 {
        if cursor == 2 {
            assert_eq!(
                s.current().broker().margin_status(),
                Some(fqe_runtime::margin::MarginStatus::Liquidating)
            );
            assert!(
                s.current()
                    .broker()
                    .orders()
                    .iter()
                    .any(|o| o.liquidation && o.live())
            );
            assert_eq!(
                s.current().broker().ledger().view().debt,
                parse_decimal("1000.00", 2).unwrap()
            );
        }
        let mut resumed =
            FinancialSession::restore(Arc::clone(&p), &s.checkpoint().unwrap()).unwrap();
        for _ in cursor..3 {
            resumed.advance().unwrap();
        }
        assert_eq!(resumed.finish().unwrap().json(), full.json());
        if cursor < 3 {
            s.advance().unwrap();
        }
    }
}
