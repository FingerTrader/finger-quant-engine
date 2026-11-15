//! Independent hand outcomes for linear-margin sizing and liquidation.
use fqe_bridge::research;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
fn input(leverage: &str, percent: &str, short: bool) -> Value {
    let mut p: Value = serde_json::from_str(include_str!("fixtures/research-input.json")).unwrap();
    let mut m: Value = serde_json::from_str(p["model"].as_str().unwrap()).unwrap();
    m["context"]["capitalModel"] = json!("linear_margin");
    m["context"]["leverage"] = json!(leverage);
    m["context"]["entryPercent"] = json!(percent);
    m["context"]["accountMode"] = json!("margin");
    if short {
        for node in m["nodes"].as_array_mut().unwrap() {
            if ["entry", "exit"].contains(&node["kind"].as_str().unwrap()) {
                node["settings"]["side"] = json!("short");
            }
        }
    }
    p["model"] = json!(serde_json::to_string(&m).unwrap());
    p
}
fn run(p: &Value) -> Value {
    serde_json::from_slice(
        &research::prepare(p)
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap()
            .bytes,
    )
    .unwrap()
}
#[test]
fn percent_budget_uses_margin_not_full_notional_and_gap_sizes_down() {
    let r = run(&input("10", "10", false));
    assert_eq!(r["profile"], "fqe.linear-shared-margin-open/1");
    // 100 equity -> 10 margin budget; open4 -> 25 units at10x. Close2
    // gives 50 equity; later sell2 returns 50 balance, no extra leverage P&L.
    assert_eq!(r["fills"][0]["quantity"], "25.000");
    assert_eq!(r["fills"][0]["price"], "4.00");
    assert_eq!(r["fills"][0]["gross"], "100.00");
    assert_eq!(r["fills"][1]["quantity"], "25.000");
    assert_eq!(r["metrics"]["final_equity"], "50.00");
    assert_eq!(r["account"]["cash_hub"]["debt"], "0.00");
    let frame = r["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["phase"] == "decision_close" && f["source_sequence"] == "6")
        .unwrap();
    assert_eq!(frame["account"]["margin"]["used"], "5.00");
    assert_eq!(frame["account"]["margin"]["available"], "45.000000000000");
}
#[test]
fn short_margin_releases_collateral_and_settles_real_profit() {
    let r = run(&input("10", "10", true));
    assert_eq!(r["fills"][0]["trade_side"], "sell");
    assert_eq!(r["fills"][0]["quantity"], "25.000");
    assert_eq!(r["metrics"]["final_equity"], "150.00");
    assert_eq!(r["account"]["margin"]["balance"], "150.000000000000");
    assert_eq!(r["account"]["margin"]["used"], "0.00");
    assert_eq!(r["account"]["cash_hub"]["debt"], "0.00");
}
#[test]
fn maintenance_liquidation_occurs_at_later_open_and_preserves_gap_debt() {
    let r = run(&input("10", "100", false));
    assert_eq!(r["fills"][0]["quantity"], "250.000");
    assert_eq!(r["fills"][1]["node"], "__fqe.margin");
    assert_eq!(r["fills"][1]["source_sequence"], "7");
    assert_eq!(r["fills"][1]["price"], "1.00");
    assert_eq!(r["account"]["margin"]["status"], "insolvent");
    assert_eq!(r["metrics"]["final_equity"], "-650.00");
    assert_eq!(r["account"]["cash_hub"]["debt"], "650.00");
    assert!(r["account"]["positions"].as_array().unwrap().is_empty());
}
#[test]
fn invalid_ratios_and_leverage_reject_before_execution() {
    for (l, p) in [
        ("0", "10"),
        ("1001", "10"),
        ("1.5", "10"),
        ("10", "0"),
        ("10", "100.01"),
    ] {
        assert!(research::prepare(&input(l, p, false)).is_err());
    }
    let r = run(&input("1", "10", false));
    assert_eq!(r["metrics"]["final_equity"], "95.00");
}

#[test]
fn fee_is_charged_on_notional_and_included_in_budget() {
    let mut p = input("10", "10", false);
    let mut m: Value = serde_json::from_str(p["model"].as_str().unwrap()).unwrap();
    m["context"]["feeBps"] = json!("100");
    p["model"] = json!(serde_json::to_string(&m).unwrap());
    let r = run(&p);
    // floor lots:22.725 *4 =90.90, margin9.09 +fee0.91 =10.00.
    assert_eq!(r["fills"][0]["quantity"], "22.725");
    assert_eq!(r["fills"][0]["fee"], "0.91");
    // exit gross45.45,fee0.45; loss45.45+fees1.36 =46.81.
    assert_eq!(r["metrics"]["final_equity"], "53.19");
}
#[test]
fn fok_never_partially_fills_after_margin_gap_resize() {
    let mut p = input("10", "10", false);
    let mut m: Value = serde_json::from_str(p["model"].as_str().unwrap()).unwrap();
    for n in m["nodes"].as_array_mut().unwrap() {
        if n["id"] == "entry" {
            n["settings"]["orderType"] = json!("limit");
            n["settings"]["limitPrice"] = json!("4.00");
            n["settings"]["timeInForce"] = json!("FOK");
        }
    }
    p["model"] = json!(serde_json::to_string(&m).unwrap());
    let r = run(&p);
    assert!(r["fills"].as_array().unwrap().is_empty());
    assert_eq!(r["metrics"]["final_equity"], "100.00");
    assert_eq!(r["orders"][0]["status"], "cancelled_time_in_force");
}
