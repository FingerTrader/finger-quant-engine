//! External account signal acceptance: real graph state, no simulated fills.
use fqe_bridge::research::live::Signals;
use serde_json::{Value, json};
fn model() -> Value {
    let input: Value = serde_json::from_str(include_str!("fixtures/research-input.json")).unwrap();
    serde_json::from_str(input["model"].as_str().unwrap()).unwrap()
}
fn bar(index: u64, close: &str) -> Value {
    json!({"time_ns":format!("{}",index*60_000_000_000),"open":close,"high":close,"low":close,"close":close,"volume":"10"})
}
fn advance(handle: &Value, bars: Vec<Value>, decision: bool) -> Value {
    json!({"handle":handle,"bars":bars,"decision":decision,"account":{"equity":"100000","cash":"100000"},"positions":[{"symbol":"DEMO","side":"long","entry_node":"","values":{"quantity":"0","averagePrice":null}}]})
}
#[test]
fn streamed_crossings_come_from_the_model_without_a_simulated_account() {
    let mut signals = Signals::default();
    let h = signals
        .execute("signals.open", &json!({"model":model()}))
        .unwrap()["handle"]
        .clone();
    let prices = ["1", "1", "1", "1", "3", "3", "1", "1"];
    let mut observed = vec![];
    for (i, price) in prices.iter().enumerate() {
        let result = signals
            .execute(
                "signals.advance",
                &advance(&h, vec![bar(i as u64 + 1, price)], true),
            )
            .unwrap();
        for intent in result["intents"].as_array().unwrap() {
            observed.push((i + 1, intent["kind"].as_str().unwrap().to_owned()));
        }
        assert!(result.get("fills").is_none());
        assert!(result.get("equity").is_none());
    }
    assert_eq!(observed, vec![(5, "entry".into()), (7, "exit".into())]);
}
#[test]
fn warmup_never_releases_old_signals_and_duplicate_bar_invalidates_session() {
    let mut signals = Signals::default();
    let h = signals
        .execute("signals.open", &json!({"model":model()}))
        .unwrap()["handle"]
        .clone();
    let result = signals
        .execute(
            "signals.advance",
            &advance(
                &h,
                vec![
                    bar(1, "1"),
                    bar(2, "1"),
                    bar(3, "1"),
                    bar(4, "1"),
                    bar(5, "3"),
                ],
                false,
            ),
        )
        .unwrap();
    assert!(result["intents"].as_array().unwrap().is_empty());
    assert!(
        signals
            .execute("signals.advance", &advance(&h, vec![bar(6, "3")], true))
            .unwrap()["intents"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        signals
            .execute("signals.advance", &advance(&h, vec![bar(6, "3")], true))
            .unwrap_err()
            .code,
        "live_bar_order"
    );
    assert_eq!(
        signals
            .execute("signals.advance", &advance(&h, vec![bar(7, "1")], true))
            .unwrap_err()
            .code,
        "live_handle"
    );
    signals
        .execute("signals.close", &json!({"handle":h}))
        .unwrap();
    signals
        .execute("signals.open", &json!({"model":model()}))
        .unwrap();
}
#[test]
fn incompatible_source_is_rejected_before_start_and_bad_ohlcv_invalidates() {
    let mut signals = Signals::default();
    let mut m = model();
    m["nodes"][0]["settings"]["dataset"] = json!("");
    assert!(
        signals
            .execute("signals.open", &json!({"model":m}))
            .is_err()
    );
    let h = signals
        .execute("signals.open", &json!({"model":model()}))
        .unwrap()["handle"]
        .clone();
    let mut bad = bar(1, "1");
    bad["low"] = json!("2");
    assert_eq!(
        signals
            .execute("signals.advance", &advance(&h, vec![bad], true))
            .unwrap_err()
            .code,
        "live_ohlcv"
    );
}

#[test]
fn native_close_protection_uses_actual_fill_price_and_never_reopens_same_bar() {
    let mut signals = Signals::default();
    let mut m = model();
    for n in m["nodes"].as_array_mut().unwrap() {
        if n["kind"] == "entry" {
            n["settings"]["stopLossMode"] = json!("percent");
            n["settings"]["stopLossValue"] = json!("10");
            n["settings"]["takeProfitMode"] = json!("percent");
            n["settings"]["takeProfitValue"] = json!("10");
        }
    }
    let h = signals
        .execute("signals.open", &json!({"model":m}))
        .unwrap()["handle"]
        .clone();
    for (i, price) in ["100", "100", "100", "100"].iter().enumerate() {
        signals
            .execute(
                "signals.advance",
                &advance(&h, vec![bar(i as u64 + 1, price)], false),
            )
            .unwrap();
    }
    let mut p = advance(&h, vec![bar(5, "110")], true);
    p["positions"][0]["entry_node"] = json!("entry");
    p["positions"][0]["values"] = json!({"quantity":"2","averagePrice":"100"});
    let result = signals.execute("signals.advance", &p).unwrap();
    assert_eq!(result["intents"].as_array().unwrap().len(), 1);
    assert_eq!(result["intents"][0]["node"], "entry.takeProfit");
    assert_eq!(result["intents"][0]["kind"], "exit");
    let mut p = advance(&h, vec![bar(6, "90")], true);
    p["positions"] = json!([{"symbol":"DEMO","side":"long","entry_node":"entry","values":{"quantity":"2","averagePrice":"100"}}]);
    assert_eq!(
        signals.execute("signals.advance", &p).unwrap()["intents"][0]["node"],
        "entry.stopLoss"
    );
}

fn multi(handle: &Value, time: u64, events: Vec<Value>, decision: bool) -> Value {
    json!({"handle":handle,"time_ns":time.to_string(),"events":events,"decision":decision,"account":{"equity":"100000","cash":"100000"},"positions":[]})
}
fn candle(stream: &str, stamp: u64, price: &str) -> Value {
    json!({"stream":stream,"sample_ns":stamp.to_string(),"values":{"open":price,"high":price,"low":price,"close":price,"volume":"1"}})
}
fn quote(stream: &str, stamp: u64, price: &str) -> Value {
    json!({"stream":stream,"sample_ns":stamp.to_string(),"values":{"close":price,"ask":price,"bid":price}})
}
#[test]
fn quote_samples_drive_native_sma_crossings_without_bar_warmup() {
    let mut m = model();
    m["nodes"][0]["kind"] = json!("quotes");
    m["nodes"][0]["settings"] =
        json!({"symbol":"","timeframe":"rt","dataset":"quote","maxAgeSeconds":"10"});
    let mut signals = Signals::default();
    let h = signals
        .execute("signals.open", &json!({"model":m}))
        .unwrap()["handle"]
        .clone();
    let mut observed = vec![];
    for (i, price) in ["1", "1", "1", "1", "3", "3", "1", "1"].iter().enumerate() {
        let stamp = (i as u64 + 1) * 1_000_000_000;
        let r = signals
            .execute(
                "signals.advance_multi",
                &multi(&h, stamp, vec![quote("quote", stamp, price)], true),
            )
            .unwrap();
        for intent in r["intents"].as_array().unwrap() {
            observed.push((i + 1, intent["kind"].as_str().unwrap().to_owned()));
        }
    }
    assert_eq!(observed, vec![(5, "entry".into()), (7, "exit".into())]);
}
fn two_periods() -> Value {
    let mut m = model();
    let original = m["nodes"].as_array().unwrap().clone();
    let mut nodes = vec![];
    let mut wires = vec![];
    for (prefix, period) in [("minute", "1m"), ("ten", "10m")] {
        for mut n in original.clone() {
            n["id"] = json!(format!("{prefix}.{}", n["id"].as_str().unwrap()));
            if n["kind"] == "bars" {
                n["settings"]["dataset"] = json!(prefix);
                n["settings"]["timeframe"] = json!(period);
            }
            nodes.push(n);
        }
        for mut w in m["wires"].as_array().unwrap().clone() {
            for key in ["from", "to"] {
                w[key] = json!(format!("{prefix}.{}", w[key].as_str().unwrap()));
            }
            wires.push(w);
        }
    }
    m["nodes"] = json!(nodes);
    m["wires"] = json!(wires);
    m
}
#[test]
fn independent_periods_do_not_resample_unchanged_indicators_or_replay_crosses() {
    let mut signals = Signals::default();
    let h = signals
        .execute("signals.open", &json!({"model":two_periods()}))
        .unwrap()["handle"]
        .clone();
    // Interleaved sample clocks: minute crosses at 5 and 7; ten-minute source
    // receives 1,1,1,1,3 at 10..50 and crosses once at 50, not on other events.
    let mut observed = vec![];
    for minute in 1..=50u64 {
        let stamp = minute * 60_000_000_000;
        let p = match minute {
            5 | 6 => "3",
            _ => "1",
        };
        let mut events = vec![candle("minute", stamp, p)];
        if minute % 10 == 0 {
            events.push(candle("ten", stamp, if minute == 50 { "3" } else { "1" }));
        }
        let r = signals
            .execute("signals.advance_multi", &multi(&h, stamp, events, true))
            .unwrap();
        for intent in r["intents"].as_array().unwrap() {
            observed.push((minute, intent["node"].as_str().unwrap().to_owned()));
        }
    }
    assert_eq!(
        observed,
        vec![
            (5, "minute.entry".into()),
            (7, "minute.exit".into()),
            (50, "ten.entry".into())
        ]
    );
}
#[test]
fn duplicate_future_unknown_and_failed_source_mutations_invalidate_the_session() {
    for invalid in [
        candle("minute", 60_000_000_000, "1"),
        candle("ten", 120_000_000_000, "1"),
        candle("unknown", 61_000_000_000, "1"),
    ] {
        let mut signals = Signals::default();
        let h = signals
            .execute("signals.open", &json!({"model":two_periods()}))
            .unwrap()["handle"]
            .clone();
        signals
            .execute(
                "signals.advance_multi",
                &multi(
                    &h,
                    60_000_000_000,
                    vec![candle("minute", 60_000_000_000, "1")],
                    false,
                ),
            )
            .unwrap();
        assert!(
            signals
                .execute(
                    "signals.advance_multi",
                    &multi(&h, 61_000_000_000, vec![invalid], true)
                )
                .is_err()
        );
        assert_eq!(
            signals
                .execute(
                    "signals.advance_multi",
                    &multi(
                        &h,
                        62_000_000_000,
                        vec![candle("minute", 62_000_000_000, "1")],
                        true
                    )
                )
                .unwrap_err()
                .code,
            "live_handle"
        );
    }
}
#[test]
fn delayed_bar_uses_its_sample_time_without_rewinding_the_decision_clock() {
    let mut signals = Signals::default();
    let h = signals
        .execute("signals.open", &json!({"model":two_periods()}))
        .unwrap()["handle"]
        .clone();
    signals
        .execute(
            "signals.advance_multi",
            &multi(
                &h,
                61_000_000_000,
                vec![candle("minute", 60_000_000_000, "1")],
                false,
            ),
        )
        .unwrap();
    signals
        .execute(
            "signals.advance_multi",
            &multi(
                &h,
                62_000_000_000,
                vec![candle("ten", 60_000_000_000, "1")],
                false,
            ),
        )
        .unwrap();
}

#[test]
fn historical_bars_do_not_impersonate_quote_samples() {
    let mut input: Value =
        serde_json::from_str(include_str!("fixtures/research-input.json")).unwrap();
    let mut m = model();
    m["nodes"][0]["kind"] = json!("quotes");
    m["nodes"][0]["settings"] =
        json!({"symbol":"","timeframe":"rt","dataset":"q","maxAgeSeconds":"10"});
    input["model"] = json!(m.to_string());
    assert_eq!(
        fqe_bridge::research::prepare(&input).err().unwrap().code,
        "research_quote_history_required"
    );
    let mut signals = Signals::default();
    let h = signals
        .execute("signals.open", &json!({"model":m}))
        .unwrap()["handle"]
        .clone();
    assert_eq!(
        signals
            .execute(
                "signals.advance_multi",
                &multi(
                    &h,
                    12_000_000_000,
                    vec![quote("q", 1_000_000_000, "1")],
                    true
                )
            )
            .unwrap_err()
            .code,
        "live_stale_source"
    );
}

#[test]
fn mixed_period_conditions_combine_asof_states_with_and_or() {
    for (operation, expected) in [("and", vec![40u64]), ("or", vec![4u64])] {
        let mut m = two_periods();
        let mut nodes = m["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["kind"] == "bars" || n["kind"] == "sma" || n["id"] == "minute.entry")
            .cloned()
            .collect::<Vec<_>>();
        for n in &mut nodes {
            if n["kind"] == "sma" {
                n["settings"]["period"] = json!("1");
            }
        }
        for prefix in ["minute", "ten"] {
            nodes.push(json!({"id":format!("{prefix}.above"),"kind":"compare","label":"above","literals":{"b":"2"},"settings":{"type":"price","operator":"greater"}}));
        }
        nodes.push(json!({"id":"both","kind":operation,"label":"group","literals":{},"settings":{"inputCount":"2"}}));
        let mut wires = m["wires"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|w| w["input"] == "price")
            .cloned()
            .collect::<Vec<_>>();
        for (prefix, port) in [("minute", "a"), ("ten", "b")] {
            wires.push(json!({"from":format!("{prefix}.fast"),"output":"value","to":format!("{prefix}.above"),"input":"a"}));
            wires.push(json!({"from":format!("{prefix}.above"),"output":"result","to":"both","input":port}));
        }
        wires
            .push(json!({"from":"both","output":"result","to":"minute.entry","input":"condition"}));
        m["nodes"] = json!(nodes);
        m["wires"] = json!(wires);
        let mut signals = Signals::default();
        let h = signals
            .execute("signals.open", &json!({"model":m}))
            .unwrap()["handle"]
            .clone();
        // Both streams have a known last close before live decisions start.
        signals
            .execute(
                "signals.advance_multi",
                &multi(&h, 1, vec![candle("ten", 1, "1")], false),
            )
            .unwrap();
        let mut observed = vec![];
        for minute in 1..=40u64 {
            let time = minute * 60_000_000_000;
            let mut events = vec![candle("minute", time, if minute >= 4 { "3" } else { "1" })];
            if minute % 10 == 0 {
                events.push(candle("ten", time, if minute >= 40 { "3" } else { "1" }));
            }
            let r = signals
                .execute("signals.advance_multi", &multi(&h, time, events, true))
                .unwrap();
            if !r["intents"].as_array().unwrap().is_empty() {
                observed.push(minute);
            }
        }
        assert_eq!(observed, expected, "{operation}");
    }
}

#[test]
fn hosted_live_studies_are_native_isolated_and_declared_before_warmup() {
    let mut signals = Signals::default();
    let mut source = model();
    source["context"]["indicatorProfile"] = json!("finger.indicators/4");
    source["nodes"] = json!(
        source["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["kind"] == "bars")
            .cloned()
            .collect::<Vec<_>>()
    );
    source["wires"] = json!([]);
    let h = signals
        .execute("signals.open", &json!({"model":source}))
        .unwrap()["handle"]
        .clone();
    let query = json!({"handle":h,"study":{"id":"a","kind":"sma","parameters":{"period":"2"}}});
    assert_eq!(
        signals.execute("signals.indicator", &query).unwrap(),
        json!({"value":null})
    );
    let other = json!({"handle":h,"study":{"id":"b","kind":"sma","parameters":{"period":"3"}}});
    signals.execute("signals.indicator", &other).unwrap();
    let mut invalid = query.clone();
    invalid["study"]["parameters"]["period"] = json!("0");
    assert!(signals.execute("signals.indicator", &invalid).is_err());
    for (i, price) in ["1", "3", "5"].iter().enumerate() {
        let r = signals
            .execute(
                "signals.advance",
                &advance(&h, vec![bar(i as u64 + 1, price)], false),
            )
            .unwrap();
        assert!(r["intents"].as_array().unwrap().is_empty());
    }
    assert_eq!(
        signals.execute("signals.indicator", &query).unwrap(),
        json!({"value":"4.000000000000"})
    );
    assert_eq!(
        signals.execute("signals.indicator", &other).unwrap(),
        json!({"value":"3.000000000000"})
    );
    let mut late = query.clone();
    late["study"]["id"] = json!("late");
    assert_eq!(
        signals
            .execute("signals.indicator", &late)
            .unwrap_err()
            .code,
        "live_declare_indicators_on_start"
    );
    signals
        .execute("signals.close", &json!({"handle":h}))
        .unwrap();
    let h = signals
        .execute("signals.open", &json!({"model":model()}))
        .unwrap()["handle"]
        .clone();
    let mut q = query;
    q["handle"] = h;
    assert_eq!(
        signals.execute("signals.indicator", &q).unwrap_err().code,
        "live_indicator_graph_immutable"
    );
}
