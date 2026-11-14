#[path = "../examples/support/optimizations.rs"]
mod support;
use fqe_core::{
    finance::{decimal_text, parse_decimal},
    types::{ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
};
use fqe_ir::{financial_exchange as json, history::HistoricalDataset, workflow::LoadedWorkflow};
use fqe_runtime::{backtest::*, optimization::*};
use std::sync::{Arc, atomic::AtomicBool};
fn m(v: &str) -> fqe_core::numeric::Decimal {
    parse_decimal(v, 2).unwrap()
}
fn completed(v: &TrialOutcome) -> &BacktestResult {
    let TrialOutcome::Completed(r) = v else {
        panic!("expected completed trial")
    };
    r
}
fn r() -> OptimizationResult {
    PreparedOptimization::new(support::request())
        .unwrap()
        .run()
        .unwrap()
}
fn threshold(t: &TrialRecord) -> String {
    match t.parameters["threshold"].value() {
        ScalarValue::Decimal(v) => decimal_text(*v),
        _ => panic!("ratio"),
    }
}
#[test]
fn actual_independent_grid_ranking_and_separate_validation_not_promoted_to_training() {
    let result = r();
    assert_eq!(result.terminal(), SearchTerminal::Completed);
    let expected = [
        ("0.000000000000", "-2.50"),
        ("60.000000000000", "-3.00"),
        ("90.000000000000", "0.00"),
    ];
    for (t, (parameter, profit)) in result.trials().iter().zip(expected) {
        assert_eq!(threshold(t), parameter);
        let r = completed(&t.outcome);
        assert_eq!(r.metrics().net_profit, m(profit));
        assert_eq!(r.metrics().initial_equity, m("100.00"));
        assert_eq!(r.broker().ledger().journal()[0].after.cash, m("100.00"));
    }
    assert_eq!(
        result.ranking(),
        [
            result.trials()[2].id,
            result.trials()[0].id,
            result.trials()[1].id
        ]
    );
    assert_eq!(result.selected(), Some(result.trials()[2].id));
    let val = completed(result.validation());
    assert_eq!(val.metrics().net_profit, m("-11.00"));
    assert_eq!(val.metrics().final_equity, m("89.00"));
    assert_eq!(val.broker().fills().len(), 2);
    assert_eq!(val.metrics().initial_equity, m("100.00"));
    assert_eq!(val.broker().fills()[0].source_bar, 13);
    assert!(val.broker().orders().iter().all(|o| o.submitted_after >= 8));
    assert_eq!(
        result.content_id(),
        json::content_id(b"FQE-OPTIMIZATION-RESULT/1\n", result.json())
    );
    assert_eq!(
        result.manifest_id(),
        json::content_id(b"FQE-OPTIMIZATION-INPUTS/1\n", result.manifest())
    );
}
#[test]
fn identical_seed_bytes_and_different_seed_order_do_not_change_training_winner() {
    let prepared = PreparedOptimization::new(support::request()).unwrap();
    let a = prepared.run().unwrap();
    let b = prepared.run().unwrap();
    assert_eq!(a.json(), b.json());
    assert_eq!(a.content_id(), b.content_id());
    let mut req = support::request();
    req.seed = 44;
    let b = PreparedOptimization::new(req).unwrap().run().unwrap();
    assert_eq!(a.selected(), b.selected());
    assert_eq!(a.ranking(), b.ranking());
    for (x, y) in a.trials().iter().zip(b.trials()) {
        assert_eq!(
            completed(&x.outcome).metrics(),
            completed(&y.outcome).metrics()
        );
    }
    assert_ne!(a.manifest_id(), b.manifest_id());
    let a = json::decode_input(a.manifest()).unwrap();
    let b = json::decode_input(b.manifest()).unwrap();
    assert_ne!(a["schedule"], b["schedule"]);
}
#[test]
fn tied_training_ranking_uses_parameter_identity_independent_of_seed_or_validation() {
    let mut req = support::request();
    req.grid[0].values = vec![support::value("90"), support::value("95")];
    let a = PreparedOptimization::new(req).unwrap().run().unwrap();
    let mut ids = a.trials().iter().map(|t| t.id).collect::<Vec<_>>();
    ids.sort_by(|a, b| a.bytes().cmp(b.bytes()));
    assert_eq!(a.ranking(), ids);
    assert_eq!(a.selected(), Some(ids[0]));
    for t in a.trials() {
        assert_eq!(completed(&t.outcome).metrics().net_profit, m("0.00"));
    }
}
#[test]
fn strictly_later_same_snapshot_split_and_metadata_are_required() {
    let mut req = support::request();
    req.validation = req.training.clone();
    assert_eq!(
        PreparedOptimization::new(req).err().unwrap().code,
        "optimization_split"
    );
    let mut req = support::request();
    let data = support::dataset();
    let spec = Arc::new(data.spec().clone());
    let csv = String::from_utf8(include_bytes!("fixtures/p3/optimization-history.csv").to_vec())
        .unwrap()
        .replace(
            "1.00,1.00,1.00,1.00,2.000,true",
            "1.00,1.00,1.00,1.00,3.000,true",
        );
    let other = Arc::new(HistoricalDataset::from_csv(spec, csv.as_bytes(), None).unwrap());
    req.validation = other
        .select(
            other.bars()[7].input().open_ns,
            other.bars()[13].input().close_ns,
            0,
        )
        .unwrap();
    assert_eq!(
        PreparedOptimization::new(req).err().unwrap().code,
        "optimization_split"
    );
    let mut req = support::request();
    req.simulation.timeframe_ns += 1;
    assert_eq!(
        PreparedOptimization::new(req).err().unwrap().code,
        "optimization_binding"
    );
}
#[test]
fn past_only_validation_warmup_has_no_inherited_position_or_financial_state() {
    let data = support::dataset();
    let mut req = support::request();
    req.validation = data
        .select(
            data.bars()[7].input().open_ns,
            data.bars()[13].input().close_ns,
            3,
        )
        .unwrap();
    let result = PreparedOptimization::new(req).unwrap().run().unwrap();
    let val = completed(result.validation());
    assert_eq!(val.observations().len(), 10);
    assert_eq!(val.curve().len(), 7);
    assert_eq!(val.metrics().initial_equity, m("100.00"));
    assert!(
        val.decisions()
            .iter()
            .filter(|d| d.source_sequence <= 7)
            .all(|d| d.outcome == DecisionOutcome::Warmup)
    );
    assert!(val.broker().orders().iter().all(|o| o.submitted_after >= 8));
}
#[test]
fn one_bad_type_and_late_run_failure_are_isolated_and_never_ranked() {
    let mut req = support::request();
    let wrong = TypedValue::new(
        TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::Bool).unwrap(),
        ScalarValue::Bool(true),
    )
    .unwrap();
    req.grid[0].values.insert(1, wrong);
    let result = PreparedOptimization::new(req).unwrap().run().unwrap();
    assert!(matches!(
        result.trials()[1].outcome,
        TrialOutcome::Failed(_)
    ));
    assert_eq!(result.ranking().len(), 3);
    assert!(!result.ranking().contains(&result.trials()[1].id));
    assert_eq!(
        completed(result.validation()).metrics().net_profit,
        m("-11.00")
    );
    let mut req = support::request();
    req.simulation.limits.fills = 1;
    let result = PreparedOptimization::new(req).unwrap().run().unwrap();
    assert!(
        matches!(&result.trials()[1].outcome,TrialOutcome::Failed(e) if e.code=="simulation_limit")
    );
    assert_eq!(result.ranking().len(), 2);
    assert!(matches!(result.validation(),TrialOutcome::Failed(e) if e.code=="simulation_limit"));
    assert_eq!(result.selected(), Some(result.trials()[2].id));
}
#[test]
fn no_successful_trial_is_distinct_and_cannot_validate_or_choose_a_winner() {
    let mut req = support::request();
    req.grid[0].key = "unknown.parameter".into();
    let result = PreparedOptimization::new(req).unwrap().run().unwrap();
    assert_eq!(result.terminal(), SearchTerminal::NoSuccessfulTrials);
    assert!(result.ranking().is_empty());
    assert_eq!(result.selected(), None);
    assert!(matches!(result.validation(), TrialOutcome::NotRun));
    assert!(
        result
            .trials()
            .iter()
            .all(|t| matches!(t.outcome, TrialOutcome::Failed(_)))
    );
}
#[test]
fn cancellation_records_completed_prefix_and_never_runs_validation() {
    for n in [0, 1, 3] {
        let mut req = support::request();
        req.control = SearchControl::CancelAfterTrials(n);
        let result = PreparedOptimization::new(req).unwrap().run().unwrap();
        assert_eq!(result.terminal(), SearchTerminal::Cancelled);
        assert_eq!(
            result
                .trials()
                .iter()
                .filter(|t| matches!(t.outcome, TrialOutcome::Completed(_)))
                .count(),
            n
        );
        assert_eq!(result.ranking().len(), n);
        assert!(matches!(result.validation(), TrialOutcome::NotRun));
    }
    let p = PreparedOptimization::new(support::request()).unwrap();
    let result = p.run_with_cancel(&AtomicBool::new(true)).unwrap();
    assert_eq!(result.terminal(), SearchTerminal::Cancelled);
    assert!(
        result
            .trials()
            .iter()
            .all(|t| matches!(t.outcome, TrialOutcome::NotRun))
    );
    assert_eq!(p.run().unwrap().terminal(), SearchTerminal::Completed);
}
#[test]
fn dimensions_and_total_trial_source_output_budgets_are_enforced() {
    let mut req = support::request();
    req.grid.clear();
    assert!(PreparedOptimization::new(req).is_err());
    let mut req = support::request();
    let duplicate = req.grid[0].values[0].clone();
    req.grid[0].values.push(duplicate);
    assert!(PreparedOptimization::new(req).is_err());
    let mut req = support::request();
    req.grid.push(req.grid[0].clone());
    assert!(PreparedOptimization::new(req).is_err());
    let mut req = support::request();
    req.budget.max_trials = 2;
    assert_eq!(
        PreparedOptimization::new(req).err().unwrap().code,
        "optimization_limit"
    );
    let mut req = support::request();
    req.budget.max_total_source_bars = 27;
    assert_eq!(
        PreparedOptimization::new(req).err().unwrap().code,
        "optimization_limit"
    );
    let mut req = support::request();
    req.budget.max_total_source_bars = 28;
    assert!(PreparedOptimization::new(req).is_ok());
    let mut req = support::request();
    req.budget.max_report_bytes = 16;
    let p = PreparedOptimization::new(req).unwrap();
    assert_eq!(p.run().err().unwrap().code, "financial_report_limit");
}
#[test]
fn workflow_search_and_validation_share_generic_runner_and_preserve_instance_parameters() {
    let mut req = support::request();
    req.artifact = SearchArtifact::Workflow(Arc::new(
        LoadedWorkflow::from_bytes(include_bytes!("fixtures/p3/backtest-workflow.json")).unwrap(),
    ));
    let result = PreparedOptimization::new(req).unwrap().run().unwrap();
    assert_eq!(
        completed(&result.trials()[1].outcome).metrics().net_profit,
        m("-3.00")
    );
    let manifest = json::decode_input(completed(&result.trials()[1].outcome).manifest()).unwrap();
    assert_eq!(
        manifest["workflow_parameters"]["policy"]["step"]["value"],
        "1"
    );
    assert_eq!(
        completed(result.validation()).metrics().net_profit,
        m("-11.00")
    );
}
#[test]
fn validation_data_does_not_reorder_training_results() {
    let a = r();
    let data = support::dataset();
    let mut rows =
        String::from_utf8(include_bytes!("fixtures/p3/optimization-history.csv").to_vec())
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
    rows[13] =
        "DEMO,1m,2024-01-02T00:12:00Z,2024-01-02T00:13:00Z,15.00,15.00,15.00,15.00,2.000,true"
            .into();
    rows[14] =
        "DEMO,1m,2024-01-02T00:13:00Z,2024-01-02T00:14:00Z,1.00,1.00,1.00,1.00,2.000,true".into();
    let data = Arc::new(
        HistoricalDataset::from_csv(
            Arc::new(data.spec().clone()),
            (rows.join("\n") + "\n").as_bytes(),
            None,
        )
        .unwrap(),
    );
    let mut req = support::request();
    req.training = data
        .select(
            data.bars()[0].input().open_ns,
            data.bars()[6].input().close_ns,
            0,
        )
        .unwrap();
    req.validation = data
        .select(
            data.bars()[7].input().open_ns,
            data.bars()[13].input().close_ns,
            0,
        )
        .unwrap();
    let b = PreparedOptimization::new(req).unwrap().run().unwrap();
    assert_eq!(a.ranking(), b.ranking());
    assert_eq!(a.selected(), b.selected());
    for (a, b) in a.trials().iter().zip(b.trials()) {
        assert_eq!(
            completed(&a.outcome).metrics(),
            completed(&b.outcome).metrics()
        );
    }
    assert_ne!(
        completed(a.validation()).metrics(),
        completed(b.validation()).metrics()
    );
}
#[test]
fn frozen_independent_parameter_ids_and_validation_alternative_do_not_reselect_winner() {
    let result = r();
    for line in include_str!("fixtures/p3/optimization.expected.tsv")
        .lines()
        .skip(1)
    {
        let c = line.split('\t').collect::<Vec<_>>();
        let id = fqe_ir::model::ContentId::parse(c[1]).unwrap();
        let t = result.trials().iter().find(|t| t.id == id).unwrap();
        assert_eq!(t.parameters["threshold"], support::value(c[0]));
        assert_eq!(completed(&t.outcome).metrics().net_profit, m(c[2]));
        assert_eq!(completed(&t.outcome).metrics().max_drawdown, m(c[3]));
        assert_eq!(result.ranking()[c[4].parse::<usize>().unwrap() - 1], id);
    }
    let mut req = support::request();
    req.grid[0].values = vec![support::value("0")];
    let alternative = PreparedOptimization::new(req).unwrap().run().unwrap();
    assert_eq!(
        completed(alternative.validation()).metrics().net_profit,
        m("-10.50")
    );
    assert!(
        completed(alternative.validation())
            .metrics()
            .net_profit
            .coefficient()
            > completed(result.validation())
                .metrics()
                .net_profit
                .coefficient()
    );
    assert_eq!(result.selected(), Some(result.trials()[2].id));
}
#[test]
fn actual_two_dimension_cartesian_grid_is_canonical_and_does_not_special_case_names() {
    let mut req = support::request();
    let mut artifact =
        json::decode_input(include_bytes!("fixtures/p3/backtest-events.json")).unwrap();
    let mut p = artifact["graph"]["parameters"]["entries"][0].clone();
    p["key"] = json::string("unlisted.test.setting");
    artifact["graph"]["parameters"]["entries"]
        .as_array_mut()
        .unwrap()
        .push(p);
    req.artifact = SearchArtifact::Event(Arc::new(
        fqe_ir::event_plan::LoadedEventPlan::from_bytes(&json::encode(&artifact, 262144).unwrap())
            .unwrap(),
    ));
    req.grid.push(ParameterDimension {
        key: "unlisted.test.setting".into(),
        values: vec![support::value("1"), support::value("2")],
    });
    let result = PreparedOptimization::new(req).unwrap().run().unwrap();
    assert_eq!(result.trials().len(), 6);
    assert_eq!(result.ranking().len(), 6);
    assert!(result.trials().iter().all(|t| t.parameters.len() == 2));
    for trial in result.trials() {
        let report = completed(&trial.outcome);
        let manifest = json::decode_input(report.manifest()).unwrap();
        assert_eq!(
            manifest["bound_parameters"]["unlisted.test.setting"],
            json::typed_value(&trial.parameters["unlisted.test.setting"])
        );
    }
}
