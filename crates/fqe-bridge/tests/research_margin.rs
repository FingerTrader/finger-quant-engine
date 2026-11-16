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

// Flat prices make the accounting oracle independent of indicators and P&L.
// One 10,000-unit entry must consume five distinct 2,000-unit liquidity bars.
fn partial_input(short: bool, exhaust_liquidity: bool) -> Value {
    let mut p = input("100", "10", short);
    let mut m: Value = serde_json::from_str(p["model"].as_str().unwrap()).unwrap();
    m["context"]["initialCash"] = json!("100000.00");
    m["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|n| ["bars", "entry", "exit"].contains(&n["id"].as_str().unwrap()));
    for (id, value) in [("yes", "true"), ("no", "false")] {
        m["nodes"].as_array_mut().unwrap().push(json!({
            "id": id, "kind": "constant", "label": id,
            "settings": {"type": "bool", "value": value}, "literals": {}
        }));
    }
    m["wires"] = json!([
        {"from":"yes","output":"value","to":"entry","input":"condition"},
        {"from":"no","output":"value","to":"exit","input":"condition"}
    ]);
    p["model"] = json!(serde_json::to_string(&m).unwrap());
    let csv = p["streams"][0]["csv"].as_str().unwrap();
    let mut lines = csv.lines();
    let mut rows = vec![lines.next().unwrap().to_owned()];
    for (index, line) in lines.enumerate() {
        let mut fields: Vec<_> = line.split(',').collect();
        fields[4..8].fill("100.00");
        fields[8] = if exhaust_liquidity && index > 0 {
            "0.000"
        } else {
            "2000.000"
        };
        rows.push(fields.join(","));
    }
    p["streams"][0]["csv"] = json!(rows.join("\n") + "\n");
    p
}
fn check_partial_margin(short: bool, exhaust_liquidity: bool) {
    let r = run(&partial_input(short, exhaust_liquidity));
    let first_partial = r["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["phase"] == "decision_close" && f["source_sequence"] == "2")
        .unwrap();
    // Credit backing a pending hold cannot count as equity or free margin.
    assert_eq!(first_partial["account"]["equity"], "100000.00");
    assert_eq!(
        first_partial["account"]["cash_hub"]["reserved_cash"],
        "8000.00"
    );
    assert_eq!(first_partial["account"]["margin"]["used"], "2000.00");
    assert_eq!(
        first_partial["account"]["margin"]["available"],
        "90000.000000000000"
    );
    let fills = r["fills"].as_array().unwrap();
    assert_eq!(fills.len(), if exhaust_liquidity { 1 } else { 5 });
    for (index, fill) in fills.iter().enumerate() {
        assert_eq!(fill["source_sequence"], (index + 2).to_string());
        assert_eq!(fill["quantity"], "2000.000");
        assert_eq!(fill["price"], "100.00");
    }
    assert_eq!(r["orders"].as_array().unwrap().len(), 1);
    assert_eq!(
        r["orders"][0]["status"],
        if exhaust_liquidity {
            "cancelled_terminal"
        } else {
            "filled"
        }
    );
    assert_eq!(r["metrics"]["final_equity"], "100000.00");
    assert_eq!(r["account"]["cash_hub"]["reserved_cash"], "0.00");
    assert_eq!(
        r["account"]["margin"]["used"],
        if exhaust_liquidity {
            "2000.00"
        } else {
            "10000.00"
        }
    );
    assert_eq!(
        r["account"]["margin"]["available"],
        if exhaust_liquidity {
            "98000.000000000000"
        } else {
            "90000.000000000000"
        }
    );
    if !short {
        assert_eq!(
            r["account"]["cash_hub"]["debt"],
            if exhaust_liquidity {
                "100000.00"
            } else {
                "900000.00"
            }
        );
    }
}
#[test]
fn long_margin_entry_survives_five_partial_fills_without_false_funds_rejection() {
    check_partial_margin(false, false);
}
#[test]
fn short_margin_entry_survives_five_partial_fills_without_false_funds_rejection() {
    check_partial_margin(true, false);
}
#[test]
fn long_partial_terminal_cancel_releases_remaining_hold_and_clearing_credit() {
    check_partial_margin(false, true);
}
#[test]
fn short_partial_terminal_cancel_releases_remaining_hold_and_clearing_credit() {
    check_partial_margin(true, true);
}

#[test]
fn clearing_credit_does_not_admit_orders_beyond_free_margin() {
    let mut p = partial_input(false, false);
    let mut m: Value = serde_json::from_str(p["model"].as_str().unwrap()).unwrap();
    m["context"]["entryPercent"] = json!("60");
    m["context"]["positionMode"] = json!("hedge");
    let mut other = m["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "entry")
        .unwrap()
        .clone();
    other["id"] = json!("other");
    other["settings"]["side"] = json!("short");
    m["nodes"].as_array_mut().unwrap().push(other);
    m["wires"].as_array_mut().unwrap().push(json!({
        "from":"yes","output":"value","to":"other","input":"condition"
    }));
    p["model"] = json!(serde_json::to_string(&m).unwrap());
    let r = run(&p);
    let first = r["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["phase"] == "decision_close" && f["source_sequence"] == "1")
        .unwrap();
    assert_eq!(first["account"]["cash_hub"]["reserved_cash"], "60000.00");
    assert_eq!(
        first["account"]["margin"]["available"],
        "40000.000000000000"
    );
    let other = r["orders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["node"] == "other")
        .unwrap();
    assert_eq!(other["status"], "rejected_insufficient_margin");
    assert_eq!(r["metrics"]["final_equity"], "100000.00");
}

#[test]
fn below_minimum_partial_notional_is_not_misreported_as_insufficient_margin() {
    let mut p = partial_input(false, false);
    let mut spec: Value =
        serde_json::from_str(p["streams"][0]["history_spec"].as_str().unwrap()).unwrap();
    // The whole order is1,000,000, but each available2,000-unit slice is200,000.
    spec["instrument"]["minimum_notional"] = json!("300000.00");
    p["streams"][0]["history_spec"] = json!(serde_json::to_string(&spec).unwrap());
    let r = run(&p);
    assert!(r["fills"].as_array().unwrap().is_empty());
    assert_eq!(r["orders"][0]["status"], "rejected_minimum_notional");
    assert_eq!(r["metrics"]["final_equity"], "100000.00");
    assert_eq!(r["account"]["cash_hub"]["reserved_cash"], "0.00");
}

#[test]
fn exhausted_partial_entry_budget_cancels_only_the_tail_not_account_margin() {
    let mut p = partial_input(false, false);
    let csv = p["streams"][0]["csv"].as_str().unwrap();
    let mut lines = csv.lines();
    let mut rows = vec![lines.next().unwrap().to_owned()];
    for (index, line) in lines.enumerate() {
        let mut fields: Vec<_> = line.split(',').collect();
        if index >= 2 {
            fields[4..8].fill("200.00");
        }
        rows.push(fields.join(","));
    }
    p["streams"][0]["csv"] = json!(rows.join("\n") + "\n");
    let r = run(&p);
    // Costs2000 +4000 +4000 exhaust the frozen10000 budget after6,000 units.
    // The original10,000-unit target cannot be filled further at the higher open.
    assert_eq!(r["fills"].as_array().unwrap().len(), 3);
    assert_eq!(r["orders"][0]["status"], "cancelled_entry_budget");
    assert_eq!(r["orders"][0]["remaining"], "4000.000");
    assert_eq!(r["orders"][0]["cash_cap_remaining"], "0.00");
    assert_eq!(r["metrics"]["final_equity"], "300000.00");
    assert_eq!(r["account"]["margin"]["available"], "288000.000000000000");
}

// An exit supersedes pending opening intent on the same leg, not merely the
// quantity that happened to have filled when the exit was submitted.
fn interrupted_entry(short: bool, protective: bool, unfilled: bool) -> Value {
    let mut p = partial_input(short, false);
    let mut m: Value = serde_json::from_str(p["model"].as_str().unwrap()).unwrap();
    for n in m["nodes"].as_array_mut().unwrap() {
        if n["id"] == "entry" {
            if protective {
                n["settings"][if short {
                    "stopLossMode"
                } else {
                    "takeProfitMode"
                }] = json!("percent");
                n["settings"][if short {
                    "stopLossValue"
                } else {
                    "takeProfitValue"
                }] = json!("1");
            }
            if unfilled {
                n["settings"]["orderType"] = json!("limit");
                n["settings"]["limitPrice"] = json!(if short { "200.00" } else { "1.00" });
            }
        }
    }
    if !protective {
        m["nodes"].as_array_mut().unwrap().push(json!({
            "id":"close_signal", "kind":"compare", "label":"close signal",
            "settings":{"type":"price", "operator":"greater"}, "literals":{"b":"100.00"}
        }));
        m["wires"]
            .as_array_mut()
            .unwrap()
            .retain(|w| w["to"] != "exit");
        m["wires"].as_array_mut().unwrap().extend([
            json!({"from":"bars","output":"close","to":"close_signal","input":"a"}),
            json!({"from":"close_signal","output":"result","to":"exit","input":"condition"}),
        ]);
    }
    p["model"] = json!(serde_json::to_string(&m).unwrap());
    let csv = p["streams"][0]["csv"].as_str().unwrap();
    let mut rows: Vec<String> = csv.lines().map(str::to_owned).collect();
    let mut fields: Vec<String> = rows[2].split(',').map(str::to_owned).collect();
    fields[5] = "101.00".into();
    fields[7] = "101.00".into();
    rows[2] = fields.join(",");
    p["streams"][0]["csv"] = json!(rows.join("\n") + "\n");
    p
}
#[test]
fn exit_cancels_partial_long_and_short_entry_before_sizing_close() {
    for short in [false, true] {
        let r = run(&interrupted_entry(short, false, false));
        assert_eq!(r["orders"][0]["status"], "cancelled_exit_signal");
        assert_eq!(r["orders"][0]["remaining"], "8000.000");
        assert_eq!(r["fills"].as_array().unwrap().len(), 2);
        assert_eq!(r["fills"][0]["quantity"], "2000.000");
        assert_eq!(r["fills"][1]["quantity"], "2000.000");
        assert_eq!(r["fills"][1]["source_sequence"], "3");
        assert_eq!(r["metrics"]["final_equity"], "100000.00");
        assert_eq!(r["account"]["margin"]["used"], "0.00");
        assert_eq!(r["account"]["cash_hub"]["reserved_cash"], "0.00");
    }
}
#[test]
fn protection_cancels_partial_entry_and_never_refills_after_close() {
    for short in [false, true] {
        let r = run(&interrupted_entry(short, true, false));
        assert_eq!(r["orders"][0]["status"], "cancelled_protection");
        assert_eq!(r["fills"].as_array().unwrap().len(), 2);
        assert_eq!(r["fills"][1]["node"], "protect.entry");
        assert_eq!(r["fills"][1]["quantity"], "2000.000");
        assert_eq!(r["account"]["margin"]["used"], "0.00");
    }
}
#[test]
fn exit_cancels_wholly_unfilled_entry_even_while_flat() {
    for short in [false, true] {
        let r = run(&interrupted_entry(short, false, true));
        assert_eq!(r["orders"].as_array().unwrap().len(), 1);
        assert_eq!(r["orders"][0]["status"], "cancelled_exit_signal");
        assert!(r["fills"].as_array().unwrap().is_empty());
        assert_eq!(r["account"]["cash_hub"]["reserved_cash"], "0.00");
    }
}

#[test]
fn exit_does_not_cancel_the_opposite_hedged_leg() {
    let mut p = interrupted_entry(false, false, false);
    let mut m: Value = serde_json::from_str(p["model"].as_str().unwrap()).unwrap();
    m["context"]["positionMode"] = json!("hedge");
    let mut other = m["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "entry")
        .unwrap()
        .clone();
    other["id"] = json!("other");
    other["settings"]["side"] = json!("short");
    m["nodes"].as_array_mut().unwrap().push(other);
    m["wires"]
        .as_array_mut()
        .unwrap()
        .push(json!({"from":"yes","output":"value","to":"other","input":"condition"}));
    p["model"] = json!(serde_json::to_string(&m).unwrap());
    let r = run(&p);
    let other = r["orders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["node"] == "other")
        .unwrap();
    assert_eq!(other["status"], "filled");
    assert_eq!(r["orders"][0]["status"], "cancelled_exit_signal");
}
