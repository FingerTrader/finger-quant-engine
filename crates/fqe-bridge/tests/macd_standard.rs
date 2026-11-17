use fqe_bridge::research::{
    live::Signals,
    studies::{calculate, describe},
};
use serde_json::{Value, json};
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/chart-profile-bars.json")).unwrap()
}
fn request(parameters: Value) -> Value {
    let v = fixture();
    json!({"profile":"finger.indicators/2", "bars":v["samples"].as_array().unwrap().iter().enumerate().map(|(i,s)| {
 let mut fields=s["fields"].clone(); fields["time"]=json!(i.to_string()); fields
 }).collect::<Vec<_>>(), "studies":[{"id":"a","kind":"macd","parameters":parameters}]})
}
fn val(r: &Value, i: usize, col: usize) -> Option<f64> {
    r["rows"][i]["values"][col]
        .as_str()
        .map(|v| v.parse().unwrap())
}
#[test]
fn defaults_warmup_and_independent_oracle() {
    let catalog = describe(&json!({})).unwrap();
    let d = catalog["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "macd")
        .unwrap();
    assert_eq!(d["parameters"]["histogram_multiplier"], "1.0");
    assert_eq!(d["parameters"].as_object().unwrap().len(), 7);
    let r = calculate(&request(json!({}))).unwrap();
    for i in 0..25 {
        assert!(val(&r, i, 0).is_none());
    }
    assert!(val(&r, 25, 0).is_some());
    assert!(val(&r, 32, 1).is_none());
    assert!(val(&r, 33, 1).is_some());
    // Independent explicit EMA oracle with mean seeds (not the production kernel).
    let bars = request(json!({}))["bars"].as_array().unwrap().clone();
    let xs = bars
        .iter()
        .map(|b| b["close"].as_str().unwrap().parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    let mut fast = xs[..12].iter().sum::<f64>() / 12.0;
    let mut slow = xs[..26].iter().sum::<f64>() / 26.0;
    let mut lines = vec![];
    let mut signal = 0.0;
    for (i, value) in xs.iter().enumerate().skip(12) {
        fast += (value - fast) * 2.0 / 13.0;
        if i < 25 {
            continue;
        }
        if i > 25 {
            slow += (value - slow) * 2.0 / 27.0;
        }
        let line = fast - slow;
        lines.push(line);
        assert!((val(&r, i, 0).unwrap() - line).abs() < 1e-10);
        if lines.len() < 9 {
            continue;
        }
        if lines.len() == 9 {
            signal = lines.iter().sum::<f64>() / 9.0;
        } else {
            signal += (line - signal) * 2.0 / 10.0;
        }
        assert!((val(&r, i, 1).unwrap() - signal).abs() < 1e-10);
    }
}
#[test]
fn rejects_bad_parameters_without_fixing() {
    for p in [
        json!({"fast":"0"}),
        json!({"signal":"2.5"}),
        json!({"fast":"26"}),
        json!({"fast":"27"}),
        json!({"slow":"-2"}),
        json!({"source":"median"}),
        json!({"signal_ma_type":"wma"}),
        json!({"histogram_multiplier":"3"}),
        json!({"fast":"01"}),
    ] {
        assert!(calculate(&request(p.clone())).is_err(), "{p}");
    }
}
#[test]
fn histogram_only_scales_and_instances_do_not_share_state() {
    let one = calculate(&request(json!({}))).unwrap();
    let two = calculate(&request(json!({"histogram_multiplier":"2.0"}))).unwrap();
    for i in 0..140 {
        assert_eq!(val(&one, i, 0), val(&two, i, 0));
        assert_eq!(val(&one, i, 1), val(&two, i, 1));
        if let (Some(l), Some(s), Some(h), Some(h2)) = (
            val(&one, i, 0),
            val(&one, i, 1),
            val(&one, i, 2),
            val(&two, i, 2),
        ) {
            assert!((h - (l - s)).abs() < 2e-12);
            assert!((h2 - 2.0 * h).abs() < 2e-12);
        }
    }
    let mut p = request(json!({}));
    p["studies"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"b","kind":"macd","parameters":{"fast":"6","slow":"19","signal":"5"}}));
    let both = calculate(&p).unwrap();
    for i in 0..140 {
        for j in 0..3 {
            assert_eq!(both["rows"][i]["values"][j], one["rows"][i]["values"][j]);
        }
    }
    assert_ne!(val(&both, 80, 0), val(&both, 80, 3));
    assert_eq!(both, calculate(&p).unwrap());
}
#[test]
fn all_sources_ma_combinations_batch_live_and_reload_match() {
    for source in ["open", "high", "low", "close", "hl2", "hlc3", "ohlc4"] {
        for oscillator in ["ema", "sma"] {
            for smooth in ["ema", "sma"] {
                let p = json!({"fast":"3","slow":"8","signal":"4","source":source,"oscillator_ma_type":oscillator,"signal_ma_type":smooth,"histogram_multiplier":"2.0"});
                let batch = calculate(&request(p.clone())).unwrap();
                let fixture = fixture();
                let mut model = fixture["model"].clone();
                model["context"]["indicatorProfile"] = json!("finger.indicators/2");
                model["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|n| n["kind"] == "bars" || n["kind"] == "macd");
                model["nodes"][1]["settings"] = p;
                model["wires"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|w| w["to"] == "macd");
                for _reload in 0..2 {
                    let mut live = Signals::default();
                    let opened = live
                        .execute("signals.open", &json!({"model":model}))
                        .unwrap();
                    for (i, sample) in fixture["samples"].as_array().unwrap().iter().enumerate() {
                        let response=live.execute("signals.advance_multi",&json!({"handle":opened["handle"],"time_ns":sample["stamp"],"decision":false,"account":{},"positions":[],"events":[{"stream":"live","sample_ns":sample["stamp"],"values":sample["fields"]}]})).unwrap();
                        for (j, c) in response["chart_columns"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .enumerate()
                        {
                            let port = c["port"].as_str().unwrap();
                            let k = match port {
                                "macd_line" | "line" => 0,
                                "signal_line" | "signal" => 1,
                                "histogram" => 2,
                                _ => continue,
                            };
                            assert_eq!(
                                response["chart_sample"]["values"][j],
                                batch["rows"][i]["values"][k],
                                "{source} {oscillator} {smooth} row {i} port {port}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn derived_sources_and_sma_have_independent_numeric_oracles() {
    for (source, expected) in [
        ("open", 1.0),
        ("high", 4.0),
        ("low", 1.0),
        ("close", 2.0),
        ("hl2", 2.5),
        ("hlc3", 7.0 / 3.0),
        ("ohlc4", 2.0),
    ] {
        let mut p = request(json!({"fast":"1","slow":"2","signal":"1","source":source}));
        p["bars"] = json!([
            {"time":"0","open":"1","high":"10","low":"0","close":"4","volume":"1"},
            {"time":"1","open":"3","high":"18","low":"2","close":"8","volume":"1"}
        ]);
        let r = calculate(&p).unwrap();
        assert!(
            (val(&r, 1, 0).unwrap() - expected).abs() < 1e-11,
            "{source}"
        );
        assert_eq!(val(&r, 1, 0), val(&r, 1, 1));
        assert_eq!(val(&r, 1, 2), Some(0.0));
    }
    let p = request(
        json!({"fast":"3","slow":"8","signal":"4","oscillator_ma_type":"sma","signal_ma_type":"sma"}),
    );
    let r = calculate(&p).unwrap();
    let xs = p["bars"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["close"].as_str().unwrap().parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    let mut lines = vec![];
    for i in 7..xs.len() {
        let line =
            xs[i - 2..=i].iter().sum::<f64>() / 3.0 - xs[i - 7..=i].iter().sum::<f64>() / 8.0;
        lines.push(line);
        assert!((val(&r, i, 0).unwrap() - line).abs() < 1e-10);
        if lines.len() >= 4 {
            let signal = lines[lines.len() - 4..].iter().sum::<f64>() / 4.0;
            assert!((val(&r, i, 1).unwrap() - signal).abs() < 1e-10);
        }
    }
}
#[test]
fn missing_or_nonfinite_chart_inputs_fail_instead_of_fabricating_zero() {
    let mut p = request(json!({}));
    p["bars"][5]["close"] = json!("NaN");
    assert!(calculate(&p).is_err());
    p["bars"][5]["close"] = Value::Null;
    assert!(calculate(&p).is_err());
}

#[test]
fn full_decimal_input_precision_matches_live_and_rejects_inexact_inputs() {
    let mut fixture = fixture();
    for (i, sample) in fixture["samples"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        for (key, offset) in [("open", 0), ("high", 2), ("low", -2), ("close", 1)] {
            sample["fields"][key] = json!(format!(
                "{}.{:012}",
                75800 + i as i64 + offset,
                (i * 7919 + 123456789) % 1000000000000_usize
            ));
        }
    }
    let mut model = fixture["model"].clone();
    model["context"]["indicatorProfile"] = json!("finger.indicators/2");
    model["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|n| n["kind"] == "bars" || n["kind"] == "macd");
    model["wires"]
        .as_array_mut()
        .unwrap()
        .retain(|w| w["to"] == "macd");
    let mut p = request(json!({}));
    p["bars"] = json!(
        fixture["samples"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mut b = s["fields"].clone();
                b["time"] = json!(i.to_string());
                b
            })
            .collect::<Vec<_>>()
    );
    let batch = calculate(&p).unwrap();
    let mut live = Signals::default();
    let opened = live
        .execute("signals.open", &json!({"model":model}))
        .unwrap();
    for (i, sample) in fixture["samples"].as_array().unwrap().iter().enumerate() {
        let row=live.execute("signals.advance_multi",&json!({"handle":opened["handle"],"time_ns":sample["stamp"],"decision":false,"account":{},"positions":[],"events":[{"stream":"live","sample_ns":sample["stamp"],"values":sample["fields"]}]})).unwrap();
        for (j, c) in row["chart_columns"].as_array().unwrap().iter().enumerate() {
            let k = match c["port"].as_str().unwrap() {
                "line" | "macd_line" => 0,
                "signal" | "signal_line" => 1,
                "histogram" => 2,
                _ => continue,
            };
            assert_eq!(
                row["chart_sample"]["values"][j], batch["rows"][i]["values"][k],
                "sample {i}"
            );
        }
    }
    p["bars"][0]["close"] = json!("1.0000000000001");
    assert_eq!(
        calculate(&p).unwrap_err().code,
        "macd_input_decimal_precision_12"
    );
}

#[test]
fn small_prices_in_chart_scientific_notation_preserve_exact_decimal_value() {
    let mut p = request(json!({"fast":"1","slow":"2","signal":"1"}));
    for (i, b) in p["bars"].as_array_mut().unwrap().iter_mut().enumerate() {
        for field in ["open", "high", "low", "close"] {
            b[field] = json!(if i % 2 == 0 { "1e-8" } else { "2e-8" });
        }
    }
    let scientific = calculate(&p).unwrap();
    for b in p["bars"].as_array_mut().unwrap() {
        for field in ["open", "high", "low", "close"] {
            b[field] = json!(if b[field] == "1e-8" {
                "0.00000001"
            } else {
                "0.00000002"
            });
        }
    }
    assert_eq!(scientific, calculate(&p).unwrap());
    p["bars"][0]["close"] = json!("1e-13");
    assert!(calculate(&p).is_err());
}
