use fqe_bridge::research::{
    live::Signals,
    studies::{calculate, describe},
};
use serde_json::{Value, json};
fn req(kind: &str, p: Value) -> Value {
    json!({"profile":"finger.indicators/4","bars":(0..70).map(|i|{let c=10.0+(i as f64*0.31).sin()*4.0+i as f64*0.1;json!({"time":(i*60000).to_string(),"open":format!("{c:.6}"),"high":format!("{:.6}",c+2.0),"low":format!("{:.6}",c-1.0),"close":format!("{c:.6}"),"volume":(i+1).to_string()})}).collect::<Vec<_>>(),"studies":[{"id":"x","kind":kind,"parameters":p}]})
}
fn v(r: &Value, i: usize, p: &str) -> Option<f64> {
    let j = r["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["port"] == p)
        .unwrap();
    r["rows"][i]["values"][j]
        .as_str()
        .map(|s| s.parse().unwrap())
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
#[test]
fn catalog_and_parameter_admission() {
    let d = describe(&json!({})).unwrap();
    assert_eq!(d["profile"], "finger.indicators/3");
    assert_eq!(d["professional_profile"], "finger.indicators/4");
    for k in ["MA", "BOLL", "ATR", "ADX", "VWAP"] {
        assert!(
            d["definitions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["kind"] == k && v["version"] == "4")
        );
        assert!(calculate(&req(k, json!({"unknown":"1"}))).is_err());
    }
    for k in ["MA", "BOLL", "ATR"] {
        for l in ["0", "-1", "2.5", "01", "10001"] {
            assert!(calculate(&req(k, json!({"length":l}))).is_err());
        }
    }
    for (k, p) in [
        ("MA", json!({"type":"rma"})),
        ("BOLL", json!({"stddev_multiplier":"0"})),
        ("ATR", json!({"smoothing":"smma"})),
        ("ADX", json!({"di_length":"0"})),
        ("VWAP", json!({"timezone":"America/New_York"})),
        ("VWAP", json!({"timezone":"UTC+-1:00"})),
        ("VWAP", json!({"timezone":"UTC+00:-1"})),
        ("VWAP", json!({"bar_interval_seconds":"3601"})),
    ] {
        assert!(calculate(&req(k, p)).is_err());
    }
}
#[test]
fn ma_hand_vector_all_types_and_warmup() {
    for (t, expected) in [
        ("sma", 3.0),
        ("ema", 3.0),
        ("wma", (2.0 + 6.0 + 12.0) / 6.0),
    ] {
        let mut q = req("MA", json!({"length":"3","type":t}));
        for (i, b) in q["bars"].as_array_mut().unwrap().iter_mut().enumerate() {
            let c = (i + 1).to_string();
            b["open"] = json!(c);
            b["high"] = json!(c);
            b["low"] = json!(c);
            b["close"] = json!(c);
        }
        let r = calculate(&q).unwrap();
        assert!(v(&r, 1, "value").is_none());
        near(
            v(&r, 2, "value").unwrap(),
            if t == "wma" { 14.0 / 6.0 } else { 2.0 },
        );
        near(v(&r, 3, "value").unwrap(), expected);
    }
}
#[test]
fn boll_population_deviation_and_multiplier() {
    let mut q = req("BOLL", json!({"length":"3"}));
    for (i, b) in q["bars"].as_array_mut().unwrap()[..3]
        .iter_mut()
        .enumerate()
    {
        let c = [1, 2, 3][i].to_string();
        for f in ["open", "high", "low", "close"] {
            b[f] = json!(c);
        }
    }
    let r = calculate(&q).unwrap();
    near(v(&r, 2, "middle").unwrap(), 2.0);
    near(
        v(&r, 2, "upper").unwrap(),
        2.0 + 2.0 * (2.0f64 / 3.0).sqrt(),
    );
    q["studies"][0]["parameters"]["stddev_multiplier"] = json!("1");
    let s = calculate(&q).unwrap();
    near(v(&s, 2, "upper").unwrap(), 2.0 + (2.0f64 / 3.0).sqrt());
    assert_eq!(r["rows"][2]["values"][1], s["rows"][2]["values"][1]);
}
#[test]
fn atr_true_range_wilder_seed_and_gap() {
    let mut q = req("ATR", json!({"length":"3"}));
    let rows = [
        (10, 12, 9, 11),
        (11, 14, 10, 13),
        (16, 18, 15, 17),
        (17, 19, 16, 18),
    ];
    q["bars"]=json!(rows.iter().enumerate().map(|(i,(o,h,l,c))|json!({"time":(i*60000).to_string(),"open":o.to_string(),"high":h.to_string(),"low":l.to_string(),"close":c.to_string(),"volume":"1"})).collect::<Vec<_>>());
    let r = calculate(&q).unwrap();
    assert!(v(&r, 1, "atr").is_none());
    near(v(&r, 2, "atr").unwrap(), 4.0);
    near(v(&r, 3, "atr").unwrap(), 11.0 / 3.0);
}
#[test]
fn adx_independent_wilder_windows_and_zero() {
    let q = req("ADX", json!({"di_length":"3","adx_smoothing":"2"}));
    let r = calculate(&q).unwrap();
    assert!(v(&r, 1, "plus_di").is_none());
    assert!(v(&r, 2, "adx").is_none());
    let mut tr = vec![];
    let mut ps = vec![];
    let mut ms = vec![];
    let mut st = 0.0;
    let mut sp = 0.0;
    let mut sm = 0.0;
    let mut dx = vec![];
    let mut adx = 0.0;
    for (i, b) in q["bars"].as_array().unwrap().iter().enumerate() {
        let f = |b: &Value, k: &str| b[k].as_str().unwrap().parse::<f64>().unwrap();
        let (h, l, c) = (f(b, "high"), f(b, "low"), f(b, "close"));
        let (t, p, m) = if i == 0 {
            (h - l, 0.0, 0.0)
        } else {
            let prev = &q["bars"][i - 1];
            let u = h - f(prev, "high");
            let d = f(prev, "low") - l;
            (
                (h - l)
                    .max((h - f(prev, "close")).abs())
                    .max((l - f(prev, "close")).abs()),
                if u > d && u > 0.0 { u } else { 0.0 },
                if d > u && d > 0.0 { d } else { 0.0 },
            )
        };
        let _ = c;
        tr.push(t);
        ps.push(p);
        ms.push(m);
        if i < 2 {
            continue;
        }
        if i == 2 {
            st = tr.iter().sum::<f64>() / 3.0;
            sp = ps.iter().sum::<f64>() / 3.0;
            sm = ms.iter().sum::<f64>() / 3.0;
        } else {
            st = (st * 2.0 + t) / 3.0;
            sp = (sp * 2.0 + p) / 3.0;
            sm = (sm * 2.0 + m) / 3.0;
        }
        let (pd, md) = (100.0 * sp / st, 100.0 * sm / st);
        near(v(&r, i, "plus_di").unwrap(), pd);
        near(v(&r, i, "minus_di").unwrap(), md);
        dx.push(if pd + md == 0.0 {
            0.0
        } else {
            100.0 * (pd - md).abs() / (pd + md)
        });
        if dx.len() == 2 {
            adx = (dx[0] + dx[1]) / 2.0;
        } else if dx.len() > 2 {
            adx = (adx + dx.last().unwrap()) / 2.0;
        }
        if dx.len() >= 2 {
            near(v(&r, i, "adx").unwrap(), adx);
        }
    }
    let mut flat = q;
    for b in flat["bars"].as_array_mut().unwrap() {
        for f in ["open", "high", "low", "close"] {
            b[f] = json!("7");
        }
    }
    let z = calculate(&flat).unwrap();
    near(v(&z, 3, "adx").unwrap(), 0.0);
    near(v(&z, 2, "plus_di").unwrap(), 0.0);
}
#[test]
fn vwap_explicit_real_and_tick_volume_anchor_prefix_and_unknown() {
    let mut q = req("VWAP", json!({"calendar":"24x7","volume_type":"real"}));
    let r = calculate(&q).unwrap();
    let mut pv = 0.0;
    let mut volume = 0.0;
    for (i, b) in q["bars"].as_array().unwrap().iter().enumerate() {
        let n = |k: &str| b[k].as_str().unwrap().parse::<f64>().unwrap();
        pv += (n("high") + n("low") + n("close")) / 3.0 * n("volume");
        volume += n("volume");
        near(v(&r, i, "vwap").unwrap(), pv / volume);
    }
    q["studies"][0]["parameters"]["volume_type"] = json!("tick");
    assert_eq!(calculate(&q).unwrap()["rows"], r["rows"]);
    q["studies"][0]["parameters"]["volume_type"] = json!("unknown");
    assert!(v(&calculate(&q).unwrap(), 0, "vwap").is_none());
    q["studies"][0]["parameters"]["volume_type"] = json!("tick");
    q["bars"].as_array_mut().unwrap().remove(0);
    assert!(
        calculate(&q).unwrap()["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["values"][0].is_null())
    );
    q["studies"][0]["parameters"]["volume_type"] = json!("tick");
    assert!(v(&calculate(&q).unwrap(), 0, "vwap").is_none());
}
#[test]
fn vwap_session_week_month_timezone_resets() {
    for (anchor, start, next) in [
        ("session", 0u64, 86400000u64),
        ("weekly", 345600000, 950400000),
        ("monthly", 0, 2678400000),
    ] {
        let mut q = req(
            "VWAP",
            json!({"anchor":anchor,"calendar":"24x7","volume_type":"real"}),
        );
        q["bars"].as_array_mut().unwrap().truncate(2);
        q["bars"][0]["time"] = json!(start.to_string());
        q["bars"][1]["time"] = json!(next.to_string());
        let r = calculate(&q).unwrap();
        assert!(v(&r, 0, "vwap").is_some());
        let b = &q["bars"][1];
        let n = |k: &str| b[k].as_str().unwrap().parse::<f64>().unwrap();
        near(
            v(&r, 1, "vwap").unwrap(),
            (n("high") + n("low") + n("close")) / 3.0,
        );
    }
    let mut q = req(
        "VWAP",
        json!({"calendar":"24x7","volume_type":"real","timezone":"UTC+08:00"}),
    );
    q["bars"].as_array_mut().unwrap().truncate(1);
    q["bars"][0]["time"] = json!("57600000");
    assert!(v(&calculate(&q).unwrap(), 0, "vwap").is_some());
    q["bars"][0]["volume"] = json!("0");
    assert!(v(&calculate(&q).unwrap(), 0, "vwap").is_none());
}
#[test]
fn all_native_batches_equal_incremental_live_and_reload() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/chart-profile-bars.json")).unwrap();
    for (kind, p) in [
        ("MA", json!({"type":"wma","length":"7"})),
        ("BOLL", json!({"length":"7"})),
        ("ATR", json!({"length":"7"})),
        ("ADX", json!({"di_length":"7","adx_smoothing":"3"})),
        ("VWAP", json!({"calendar":"24x7","volume_type":"real"})),
        ("VWAP", json!({"calendar":"24x7","volume_type":"tick"})),
    ] {
        let q = req(kind, p.clone());
        let batch = calculate(&q).unwrap();
        let mut model = fixture["model"].clone();
        model["context"]["indicatorProfile"] = json!("finger.indicators/4");
        model["nodes"]
            .as_array_mut()
            .unwrap()
            .retain(|n| n["kind"] == "bars" || n["kind"] == "macd");
        model["nodes"][1]["kind"] = json!(kind);
        model["nodes"][1]["settings"] = p;
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
                let stamp = (i as u64 + 1) * 60000000000;
                let r=live.execute("signals.advance_multi",&json!({"handle":opened["handle"],"time_ns":stamp.to_string(),"decision":false,"account":{},"positions":[],"events":[{"stream":"live","sample_ns":stamp.to_string(),"values":fields}]})).unwrap();
                for (j, c) in r["chart_columns"].as_array().unwrap().iter().enumerate() {
                    if c["node"] == "macd" {
                        let k = batch["columns"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .position(|p| p["port"] == c["port"])
                            .unwrap();
                        assert_eq!(
                            r["chart_sample"]["values"][j], batch["rows"][i]["values"][k],
                            "{kind} row{i}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn prior_indicators_unchanged_across_v3_v4() {
    for k in ["macd", "rsi", "kdj", "WILLR", "cci"] {
        let mut q = req(k, json!({}));
        let new = calculate(&q).unwrap();
        q["profile"] = json!("finger.indicators/3");
        let old = calculate(&q).unwrap();
        assert_eq!(new["rows"], old["rows"]);
        assert_eq!(new["columns"], old["columns"]);
    }
}
#[test]
fn sources_and_atr_smoothing_hand_vectors() {
    for (source, expected) in [
        ("open", 10.0),
        ("high", 14.0),
        ("low", 8.0),
        ("close", 12.0),
        ("hl2", 11.0),
        ("hlc3", 34.0 / 3.0),
        ("ohlc4", 11.0),
    ] {
        let mut q = req("MA", json!({"length":"1","source":source}));
        q["bars"] =
            json!([{"time":"0","open":"10","high":"14","low":"8","close":"12","volume":"1"}]);
        near(v(&calculate(&q).unwrap(), 0, "value").unwrap(), expected);
    }
    // TR = 2,4,8,16. Full-window seed 14/3, then SMA=28/3,
    // EMA=(14/3+16)/2, RMA=(28/3+16)/3.
    for (smoothing, expected) in [
        ("sma", 28.0 / 3.0),
        ("ema", 31.0 / 3.0),
        ("rma", 76.0 / 9.0),
    ] {
        let mut q = req("ATR", json!({"length":"3","smoothing":smoothing}));
        q["bars"]=json!([2,4,8,16].iter().enumerate().map(|(i,h)| json!({"time":(i*60000).to_string(),"open":"0","high":h.to_string(),"low":"0","close":"0","volume":"1"})).collect::<Vec<_>>());
        near(v(&calculate(&q).unwrap(), 3, "atr").unwrap(), expected);
    }
}
#[test]
fn vwap_gap_missing_volume_and_declared_interval() {
    let mut q = req("VWAP", json!({"calendar":"24x7","volume_type":"real"}));
    q["bars"].as_array_mut().unwrap().remove(1);
    let r = calculate(&q).unwrap();
    assert!(v(&r, 0, "vwap").is_some());
    assert!(v(&r, 1, "vwap").is_none());
    q["bars"][0].as_object_mut().unwrap().remove("volume");
    assert!(calculate(&q).is_err());
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/chart-profile-bars.json")).unwrap();
    let mut model = fixture["model"].clone();
    model["context"]["indicatorProfile"] = json!("finger.indicators/4");
    model["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|n| n["kind"] == "bars" || n["kind"] == "macd");
    model["nodes"][1]["kind"] = json!("VWAP");
    model["nodes"][1]["settings"] = json!({"bar_interval_seconds":"300"});
    model["wires"]
        .as_array_mut()
        .unwrap()
        .retain(|w| w["to"] == "macd");
    let result = Signals::default().execute("signals.open", &json!({"model":model}));
    assert!(format!("{:?}", result.unwrap_err()).contains("vwap_bar_interval_mismatch"));
}
#[test]
fn multi_instance_batch_isolation_and_schemas() {
    let mut q = req("BOLL", json!({"length":"3"}));
    let single = calculate(&q).unwrap();
    q["studies"].as_array_mut().unwrap().push(
        json!({"id":"other","kind":"BOLL","parameters":{"length":"7","stddev_multiplier":"3"}}),
    );
    let multi = calculate(&q).unwrap();
    for i in 0..70 {
        for j in 0..3 {
            assert_eq!(
                multi["rows"][i]["values"][j],
                single["rows"][i]["values"][j]
            );
        }
    }
    let catalog = describe(&json!({})).unwrap();
    let ma = catalog["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["kind"] == "MA")
        .unwrap();
    assert_eq!(
        ma["parameter_schema"]["type"]["choices"],
        json!(["sma", "ema", "wma"])
    );
    assert_eq!(ma["parameter_schema"]["length"]["minimum"], "1");
}

#[test]
fn tick_volume_hand_vector_is_weighted_not_unweighted() {
    let mut q = req(
        "VWAP",
        json!({"calendar":"24x7","volume_type":"tick","source":"close"}),
    );
    q["bars"] = json!([
        {"time":"0","open":"10","high":"10","low":"10","close":"10","volume":"2"},
        {"time":"60000","open":"20","high":"20","low":"20","close":"20","volume":"1"}
    ]);
    let r = calculate(&q).unwrap();
    near(v(&r, 0, "vwap").unwrap(), 10.0);
    near(v(&r, 1, "vwap").unwrap(), 40.0 / 3.0);
    q["bars"][0]["volume"] = json!("0");
    assert!(v(&calculate(&q).unwrap(), 0, "vwap").is_none());
}
