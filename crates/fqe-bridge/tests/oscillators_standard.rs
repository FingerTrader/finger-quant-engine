use fqe_bridge::research::{
    live::Signals,
    studies::{calculate, describe},
};
use serde_json::{Value, json};
fn bars(count: usize) -> Vec<Value> {
    (0..count).map(|i| {let c=100.0+(i as f64*0.4).sin()*15.0+i as f64/9.0;json!({"time":i.to_string(),"open":format!("{:.6}",c-1.0),"high":format!("{:.6}",c+3.0),"low":format!("{:.6}",c-4.0),"close":format!("{c:.6}"),"volume":"10"})}).collect()
}
fn req(kind: &str, params: Value) -> Value {
    json!({"profile":"finger.indicators/3","bars":bars(140),"studies":[{"id":"study","kind":kind,"parameters":params}]})
}
fn val(v: &Value, i: usize, j: usize) -> Option<f64> {
    v["rows"][i]["values"][j]
        .as_str()
        .map(|s| s.parse().unwrap())
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 2e-10, "{actual} != {expected}");
}
#[test]
fn defaults_and_warmup() {
    let c = describe(&json!({})).unwrap();
    assert_eq!(c["profile"], "finger.indicators/3");
    for (kind, length, port) in [
        ("kdj", 9, "k"),
        ("WILLR", 14, "williams_r"),
        ("cci", 20, "cci"),
    ] {
        let d = c["definitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["kind"] == kind)
            .unwrap();
        assert_eq!(d["parameters"]["length"], length.to_string());
        assert_eq!(d["outputs"][0]["port"], port);
        let r = calculate(&req(kind, json!({}))).unwrap();
        for i in 0..length - 1 {
            assert!(val(&r, i, 0).is_none());
        }
        assert!(val(&r, length - 1, 0).is_some());
    }
}
#[test]
fn invalid_inputs_are_explicit() {
    for kind in ["kdj", "WILLR", "cci"] {
        for p in [
            json!({"length":"0"}),
            json!({"length":"-1"}),
            json!({"length":"2.5"}),
            json!({"length":"01"}),
            json!({"length":"10001"}),
            json!({"unknown":"2"}),
        ] {
            assert!(calculate(&req(kind, p)).is_err());
        }
    }
    for p in [
        json!({"k_smoothing":"0"}),
        json!({"d_smoothing":"1.2"}),
        json!({"smoothing_method":"sma"}),
        json!({"initial_k":"101"}),
        json!({"initial_d":"NaN"}),
    ] {
        assert!(calculate(&req("kdj", p)).is_err());
    }
    for p in [
        json!({"constant":"0"}),
        json!({"constant":"-1"}),
        json!({"constant":"NaN"}),
        json!({"source":"last"}),
    ] {
        assert!(calculate(&req("cci", p)).is_err());
    }
    for kind in ["kdj", "WILLR", "cci"] {
        let mut q = req(kind, json!({}));
        q["bars"][0]["high"] = json!("NaN");
        assert!(calculate(&q).is_err());
        q["bars"][0]["high"] = Value::Null;
        assert!(calculate(&q).is_err());
    }
}
#[test]
fn constant_windows_zero_denominators_are_ready_not_warmup_zero() {
    for (kind, expected) in [
        ("kdj", vec![50.0, 50.0, 50.0]),
        ("WILLR", vec![-50.0]),
        ("cci", vec![0.0]),
    ] {
        let mut q = req(kind, json!({"length":"3"}));
        q["bars"]=json!((0..6).map(|i|json!({"time":i.to_string(),"open":"7","high":"7","low":"7","close":"7","volume":"0"})).collect::<Vec<_>>());
        let r = calculate(&q).unwrap();
        assert!(val(&r, 1, 0).is_none());
        for i in 2..6 {
            for (j, x) in expected.iter().enumerate() {
                near(val(&r, i, j).unwrap(), *x);
            }
        }
    }
}
#[test]
fn kdj_fixed_hand_vector_and_unclipped_j() {
    // length1 gives RSV 100,100,0; K/D recursive fractions from exactly 50.
    let mut q = req("kdj", json!({"length":"1"}));
    q["bars"] = json!([
 {"time":"0","open":"0","high":"10","low":"0","close":"10","volume":"1"},
 {"time":"1","open":"0","high":"10","low":"0","close":"10","volume":"1"},
 {"time":"2","open":"0","high":"10","low":"0","close":"0","volume":"1"}]);
    let r = calculate(&q).unwrap();
    near(val(&r, 0, 0).unwrap(), 200.0 / 3.0);
    near(val(&r, 0, 1).unwrap(), 500.0 / 9.0);
    near(val(&r, 0, 2).unwrap(), 800.0 / 9.0);
    near(val(&r, 1, 0).unwrap(), 700.0 / 9.0);
    near(val(&r, 1, 1).unwrap(), 1700.0 / 27.0);
    assert!(val(&r, 1, 2).unwrap() > 100.0);
    q["studies"][0]["parameters"] = json!({"length":"1","k_smoothing":"1","d_smoothing":"9"});
    let r = calculate(&q).unwrap();
    assert!(val(&r, 2, 2).unwrap() < 0.0);
    for i in 0..3 {
        near(
            val(&r, i, 2).unwrap(),
            3.0 * val(&r, i, 0).unwrap() - 2.0 * val(&r, i, 1).unwrap(),
        );
    }
}
fn price(b: &Value, source: &str) -> f64 {
    let f = |k: &str| b[k].as_str().unwrap().parse::<f64>().unwrap();
    match source {
        "open" => f("open"),
        "high" => f("high"),
        "low" => f("low"),
        "close" => f("close"),
        "hl2" => (f("high") + f("low")) / 2.0,
        "ohlc4" => (f("open") + f("high") + f("low") + f("close")) / 4.0,
        _ => (f("high") + f("low") + f("close")) / 3.0,
    }
}
#[test]
fn independent_window_oracles_custom_parameters_and_bounds() {
    for length in [1, 3, 9, 20] {
        let q = req("WILLR", json!({"length":length.to_string()}));
        let r = calculate(&q).unwrap();
        let bs = q["bars"].as_array().unwrap();
        for i in length - 1..140 {
            let w = &bs[i + 1 - length..=i];
            let h = w.iter().map(|b| price(b, "high")).reduce(f64::max).unwrap();
            let l = w.iter().map(|b| price(b, "low")).reduce(f64::min).unwrap();
            let expected = -100.0 * (h - price(&bs[i], "close")) / (h - l);
            let actual = val(&r, i, 0).unwrap();
            near(actual, expected);
            assert!((-100.0..=0.0).contains(&actual));
        }
        for source in ["open", "high", "low", "close", "hl2", "hlc3", "ohlc4"] {
            let q = req(
                "cci",
                json!({"length":length.to_string(),"source":source,"constant":"0.02"}),
            );
            let r = calculate(&q).unwrap();
            for i in length - 1..140 {
                let x = bs[i + 1 - length..=i]
                    .iter()
                    .map(|b| price(b, source))
                    .collect::<Vec<_>>();
                let mean = x.iter().sum::<f64>() / length as f64;
                let mad = x.iter().map(|v| (v - mean).abs()).sum::<f64>() / length as f64;
                let expected = if mad == 0.0 {
                    0.0
                } else {
                    (x[length - 1] - mean) / (0.02 * mad)
                };
                near(val(&r, i, 0).unwrap(), expected);
            }
        }
    }
    // Fixed asymmetric prices [1,1,1,1,6] have mean2, MAD1.6. Population stddev2 differs.
    let mut q = req("cci", json!({"length":"5","source":"close"}));
    q["bars"]=json!([1,1,1,1,6].iter().enumerate().map(|(i,c)|json!({"time":i.to_string(),"open":c.to_string(),"high":c.to_string(),"low":c.to_string(),"close":c.to_string(),"volume":"1"})).collect::<Vec<_>>());
    let r = calculate(&q).unwrap();
    near(val(&r, 4, 0).unwrap(), 4.0 / (0.015 * 1.6));
    assert!(val(&r, 4, 0).unwrap() > 100.0);
}
#[test]
fn independent_instances_style_free_requests_and_reloads() {
    for kind in ["kdj", "WILLR", "cci"] {
        let mut q = req(kind, json!({"length":"3"}));
        let one = calculate(&q).unwrap();
        q["studies"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"other","kind":kind,"parameters":{"length":"20"}}));
        let both = calculate(&q).unwrap();
        let ports = one["columns"].as_array().unwrap().len();
        for i in 0..140 {
            for j in 0..ports {
                assert_eq!(one["rows"][i]["values"][j], both["rows"][i]["values"][j]);
            }
        }
        assert_eq!(
            both,
            calculate(&serde_json::from_str::<Value>(&q.to_string()).unwrap()).unwrap()
        );
    }
}
#[test]
fn batch_every_prefix_equals_live_and_reload_exactly() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/chart-profile-bars.json")).unwrap();
    for (kind, params) in [
        (
            "kdj",
            json!({"length":"9","k_smoothing":"2","d_smoothing":"5","smoothing_method":"smma","initial_k":"50","initial_d":"50"}),
        ),
        ("WILLR", json!({"length":"7"})),
        (
            "cci",
            json!({"length":"12","source":"ohlc4","constant":"0.02"}),
        ),
    ] {
        let q = req(kind, params.clone());
        let batch = calculate(&q).unwrap();
        let mut model = fixture["model"].clone();
        model["context"]["indicatorProfile"] = json!("finger.indicators/3");
        model["nodes"]
            .as_array_mut()
            .unwrap()
            .retain(|n| n["kind"] == "bars" || n["kind"] == "macd");
        model["nodes"][1]["kind"] = json!(kind);
        model["nodes"][1]["settings"] = params;
        model["wires"]
            .as_array_mut()
            .unwrap()
            .retain(|w| w["to"] == "macd");
        for _ in 0..2 {
            let mut live = Signals::default();
            let opened = live
                .execute("signals.open", &json!({"model":model}))
                .unwrap();
            for (i, b) in q["bars"].as_array().unwrap().iter().enumerate() {
                let mut fields = b.clone();
                fields.as_object_mut().unwrap().remove("time");
                let stamp = (i as u64 + 1) * 60_000_000_000;
                let response=live.execute("signals.advance_multi",&json!({"handle":opened["handle"],"time_ns":stamp.to_string(),"decision":false,"account":{},"positions":[],"events":[{"stream":"live","sample_ns":stamp.to_string(),"values":fields}]})).unwrap();
                for (j, c) in response["chart_columns"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                {
                    if c["node"] != "macd" {
                        continue;
                    }
                    let k = batch["columns"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .position(|p| p["port"] == c["port"])
                        .unwrap();
                    assert_eq!(
                        response["chart_sample"]["values"][j], batch["rows"][i]["values"][k],
                        "{kind} row{i}"
                    );
                }
            }
        }
    }
}
#[test]
fn old_profiles_keep_frozen_rules_and_rsi_macd_are_identical() {
    for kind in ["rsi", "macd"] {
        let mut q = req(kind, json!({}));
        q["profile"] = json!("finger.indicators/2");
        let old = calculate(&q).unwrap();
        q["profile"] = json!("finger.indicators/3");
        let new = calculate(&q).unwrap();
        assert_eq!(old["rows"], new["rows"]);
        assert_eq!(old["columns"], new["columns"]);
    }
    let mut q = req("cci", json!({}));
    q["profile"] = json!("finger.indicators/2");
    let old = calculate(&q).unwrap();
    assert!(val(&old, 0, 0).is_some());
    q["profile"] = json!("finger.indicators/3");
    let new = calculate(&q).unwrap();
    assert!(val(&new, 0, 0).is_none());
}
#[test]
fn independent_kdj_full_window_and_decimal_constant_cci() {
    for (length, ks, ds) in [(9, 3, 3), (4, 2, 5), (20, 1, 9)] {
        let q = req(
            "kdj",
            json!({"length":length.to_string(),"k_smoothing":ks.to_string(),"d_smoothing":ds.to_string()}),
        );
        let r = calculate(&q).unwrap();
        let bs = q["bars"].as_array().unwrap();
        let (mut k, mut d) = (50.0, 50.0);
        for i in length - 1..140 {
            let w = &bs[i + 1 - length..=i];
            let hh = w.iter().map(|b| price(b, "high")).reduce(f64::max).unwrap();
            let ll = w.iter().map(|b| price(b, "low")).reduce(f64::min).unwrap();
            let rsv = 100.0 * (price(&bs[i], "close") - ll) / (hh - ll);
            k = (rsv + (ks - 1) as f64 * k) / ks as f64;
            d = (k + (ds - 1) as f64 * d) / ds as f64;
            near(val(&r, i, 0).unwrap(), k);
            near(val(&r, i, 1).unwrap(), d);
            near(val(&r, i, 2).unwrap(), 3.0 * k - 2.0 * d);
        }
    }
    let mut q = req("cci", json!({}));
    q["bars"]=json!((0..30).map(|i|json!({"time":i.to_string(),"open":"1.1","high":"1.1","low":"1.1","close":"1.1","volume":"1"})).collect::<Vec<_>>());
    let r = calculate(&q).unwrap();
    assert_eq!(val(&r, 19, 0), Some(0.0));
}
