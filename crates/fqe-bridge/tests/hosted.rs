use fqe_bridge::research::{self, hosted::Hosted};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
fn input() -> Value {
    serde_json::from_str(include_str!("fixtures/research-input.json")).unwrap()
}
fn call(h: &mut Hosted, handle: &str, method: &str, mut p: Value) -> Value {
    p["handle"] = handle.into();
    h.execute(method, &p).unwrap()
}
fn transaction(h: &mut Hosted, handle: &str, op: Value) -> Value {
    let p = call(h, handle, "hosted.prepare", json!({"operation":op}));
    call(h, handle, "hosted.commit", json!({"token":p["token"]}));
    p
}
fn complete(h: &mut Hosted, handle: &str) -> Value {
    let s = call(h, handle, "hosted.snapshot", json!({}));
    call(
        h,
        handle,
        "hosted.complete",
        json!({"revision":s["revision"]}),
    )
}
#[test]
fn commit_and_callback_barriers_and_clean_restart() {
    let mut h = Hosted::default();
    let handle = h.execute("hosted.open", &json!({"input":input()})).unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    let p = call(
        &mut h,
        &handle,
        "hosted.prepare",
        json!({"operation":{"action":"bar"}}),
    );
    assert_eq!(
        h.execute("hosted.snapshot", &json!({"handle":handle}))
            .unwrap_err()
            .code,
        "hosted_commit_required"
    );
    call(
        &mut h,
        &handle,
        "hosted.commit",
        json!({"token":p["token"]}),
    );
    assert!(
        h.execute(
            "hosted.prepare",
            &json!({"handle":handle,"operation":{"action":"bar"}})
        )
        .is_err()
    );
    // A rejected market advance poisons the session; never replay it silently.
    call(&mut h, &handle, "hosted.close", json!({}));
    let second = h.execute("hosted.open", &json!({"input":input()})).unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    let s = call(&mut h, &second, "hosted.snapshot", json!({}));
    assert_eq!(s["processed"], "0");
}
#[test]
fn python_sma_signals_share_native_fills_fees_and_account() {
    let p = input();
    let graph = research::prepare(&p)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    let graph: Value = serde_json::from_slice(&graph.bytes).unwrap();
    let mut h = Hosted::default();
    let handle = h.execute("hosted.open", &json!({"input":p})).unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    for (id, length) in [("fast", "2"), ("slow", "3")] {
        call(
            &mut h,
            &handle,
            "hosted.indicator",
            json!({"study":{"id":id,"kind":"sma","parameters":{"period":length}}}),
        );
    }
    let mut previous = None;
    let mut count = 0;
    for _ in 0..8 {
        transaction(&mut h, &handle, json!({"action":"bar"}));
        let f = call(
            &mut h,
            &handle,
            "hosted.indicator",
            json!({"study":{"id":"fast","kind":"sma","parameters":{"period":"2"}}}),
        );
        let s = call(
            &mut h,
            &handle,
            "hosted.indicator",
            json!({"study":{"id":"slow","kind":"sma","parameters":{"period":"3"}}}),
        );
        if let (Some(f), Some(s)) = (f["value"].as_str(), s["value"].as_str()) {
            let diff = f.parse::<f64>().unwrap() - s.parse::<f64>().unwrap();
            if let Some(prior) = previous {
                let side = if prior <= 0. && diff > 0. {
                    Some("buy")
                } else if prior >= 0. && diff < 0. {
                    Some("sell")
                } else {
                    None
                };
                if let Some(side) = side {
                    count += 1;
                    transaction(
                        &mut h,
                        &handle,
                        json!({"action":"submit","client_order_id":format!("py.{count}"),"side":side,"quantity":"1.000","kind":"market","limit_price":"0","stop_loss_percent":"0","take_profit_percent":"0"}),
                    );
                }
            }
            previous = Some(diff);
        }
        complete(&mut h, &handle);
    }
    transaction(&mut h, &handle, json!({"action":"finish"}));
    let report = call(&mut h, &handle, "hosted.report", json!({}));
    assert_eq!(report["account"], graph["account"]);
    assert_eq!(report["metrics"], graph["metrics"]);
    for field in ["price", "quantity", "fee", "timestamp_ns", "trade_side"] {
        let a: Vec<_> = report["fills"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f[field].clone())
            .collect();
        let b: Vec<_> = graph["fills"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f[field].clone())
            .collect();
        assert_eq!(a, b, "{field}");
    }
    assert_eq!(
        report["frames"].as_array().unwrap().last().unwrap()["phase"],
        "terminal"
    );
}
#[test]
fn no_hidden_graph_trades_and_parameter_changes_rejected() {
    let mut h = Hosted::default();
    let handle = h.execute("hosted.open", &json!({"input":input()})).unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    for _ in 0..8 {
        transaction(&mut h, &handle, json!({"action":"bar"}));
        complete(&mut h, &handle);
    }
    assert_eq!(
        h.execute(
            "hosted.indicator",
            &json!({"handle":handle,"study":{"id":"late","kind":"macd","parameters":{}}})
        )
        .unwrap_err()
        .code,
        "hosted_declare_indicators_on_start"
    );
    transaction(&mut h, &handle, json!({"action":"finish"}));
    let report = call(&mut h, &handle, "hosted.report", json!({}));
    assert!(report["fills"].as_array().unwrap().is_empty());
}

fn order(client: &str) -> Value {
    json!({"action":"submit","client_order_id":client,"side":"buy","quantity":"1.000","kind":"market","limit_price":"0","stop_loss_percent":"10","take_profit_percent":"20"})
}
#[test]
fn explicit_quantity_is_never_enlarged_by_margin_allocation_and_protection_is_native() {
    let mut p = input();
    let mut model: Value = serde_json::from_str(p["model"].as_str().unwrap()).unwrap();
    model["context"]["accountMode"] = "margin".into();
    model["context"]["capitalModel"] = "linear_margin".into();
    model["context"]["leverage"] = "10".into();
    model["context"]["entryPercent"] = "100".into();
    p["model"] = serde_json::to_string(&model).unwrap().into();
    let mut h = Hosted::default();
    let handle = h.execute("hosted.open", &json!({"input":p})).unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        h.execute(
            "hosted.prepare",
            &json!({"handle":handle,"operation":order("early")})
        )
        .is_err()
    );
    transaction(&mut h, &handle, json!({"action":"bar"}));
    let submitted = transaction(&mut h, &handle, order("bounded"));
    assert_eq!(submitted["result"]["quantity"], "1.000");
    assert!(
        h.execute(
            "hosted.prepare",
            &json!({"handle":handle,"operation":order("bounded")})
        )
        .is_err()
    );
    complete(&mut h, &handle);
    for _ in 1..8 {
        transaction(&mut h, &handle, json!({"action":"bar"}));
        complete(&mut h, &handle);
    }
    transaction(&mut h, &handle, json!({"action":"finish"}));
    let report = call(&mut h, &handle, "hosted.report", json!({}));
    assert!(report["fills"].as_array().unwrap().len() >= 2);
    assert!(
        report["orders"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["node"].as_str().unwrap_or("").starts_with("protect."))
    );
    assert_eq!(report["equity_curve"][0]["source_sequence"], "1");
}

#[test]
fn indicators_validate_parameters_and_keep_instance_identity() {
    let mut h = Hosted::default();
    let handle = h.execute("hosted.open", &json!({"input":input()})).unwrap()["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    let invalid = json!({"handle":handle,"study":{"id":"x","kind":"macd","parameters":{"fast":"26","slow":"12"}}});
    assert!(h.execute("hosted.indicator", &invalid).is_err());
    let initial = call(
        &mut h,
        &handle,
        "hosted.indicator",
        json!({"study":{"id":"x","kind":"macd","parameters":{"fast":"2","slow":"3","signal":"2"}}}),
    );
    assert_eq!(
        initial,
        json!({"macd_line":null,"signal_line":null,"histogram":null})
    );
    assert!(
        h.execute(
            "hosted.indicator",
            &json!({"handle":handle,"study":{"id":"x","kind":"macd","parameters":{}}})
        )
        .is_err()
    );
    call(
        &mut h,
        &handle,
        "hosted.indicator",
        json!({"study":{"id":"other","kind":"macd","parameters":{}}}),
    );
    transaction(&mut h, &handle, json!({"action":"bar"}));
    let a = call(
        &mut h,
        &handle,
        "hosted.indicator",
        json!({"study":{"id":"x","kind":"macd","parameters":{"fast":"2","slow":"3","signal":"2"}}}),
    );
    assert!(a["macd_line"].is_null());
    complete(&mut h, &handle);
}
