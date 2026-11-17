use fqe_bridge::research::studies::{calculate, describe};
use serde_json::{Value, json};
fn flat(n: usize) -> Value {
    json!({"profile":"finger.k-chart/1","bars":(0..n).map(|i|json!({"time":i.to_string(),"open":"100","high":"100","low":"100","close":"100","volume":(i+1).to_string()})).collect::<Vec<_>>(),"studies":[
 {"id":"kdj","kind":"kdj","parameters":{}},{"id":"wr","kind":"wr","parameters":{}},{"id":"cci","kind":"cci","parameters":{}},{"id":"vol5","kind":"volume","parameters":{"period":"5"}}, {"id":"vol10","kind":"volume","parameters":{"period":"10"}}]})
}
fn number(v: &Value) -> Option<f64> {
    v.as_str().map(|s| s.parse().unwrap())
}
#[test]
fn independent_flat_studies_and_volume_readiness() {
    let r = calculate(&flat(20)).unwrap();
    let rows = r["rows"].as_array().unwrap();
    assert_eq!(number(&rows[0]["values"][0]), Some(50.0));
    assert!((number(&rows[1]["values"][0]).unwrap() - 100.0 / 3.0).abs() < 1e-11);
    assert!((number(&rows[1]["values"][1]).unwrap() - 400.0 / 9.0).abs() < 1e-11);
    assert!((number(&rows[1]["values"][2]).unwrap() - 100.0 / 9.0).abs() < 1e-11);
    for row in rows {
        assert!(row["values"][3].is_null());
        assert_eq!(number(&row["values"][4]), Some(0.0));
    }
    assert!(r["rows"][3]["values"][5].is_null());
    assert_eq!(number(&r["rows"][4]["values"][5]), Some(3.0));
    assert_eq!(number(&r["rows"][9]["values"][6]), Some(5.5));
}
#[test]
fn chart_catalog_and_native_strategy_fixture_agree() {
    let v: Value = serde_json::from_str(include_str!("fixtures/chart-profile-bars.json")).unwrap();
    let studies = v["model"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| !["bars", "quotes"].contains(&n["kind"].as_str().unwrap()))
        .map(|n| json!({"id":n["id"],"kind":n["kind"],"parameters":n["settings"]}))
        .collect::<Vec<_>>();
    let bars=v["samples"].as_array().unwrap().iter().enumerate().map(|(i,s)|json!({"time":i.to_string(),"open":s["fields"]["open"],"high":s["fields"]["high"],"low":s["fields"]["low"],"close":s["fields"]["close"],"volume":s["fields"]["volume"]})).collect::<Vec<_>>();
    let result =
        calculate(&json!({"profile":"finger.k-chart/1","bars":bars,"studies":studies})).unwrap();
    for (i, row) in result["rows"].as_array().unwrap().iter().enumerate() {
        for (j, c) in result["columns"].as_array().unwrap().iter().enumerate() {
            let key = format!(
                "{}.{}",
                c["id"].as_str().unwrap(),
                c["port"].as_str().unwrap()
            );
            let expected = &v["samples"][i]["expected"][&key];
            if expected.is_null() {
                assert!(row["values"][j].is_null(), "{key} {i}");
            } else {
                assert!(
                    (number(&row["values"][j]).unwrap() - expected.as_f64().unwrap()).abs() < 1e-9,
                    "{key} {i}"
                );
            }
        }
    }
    let catalog = describe(&json!({})).unwrap();
    assert_eq!(catalog["definitions"].as_array().unwrap().len(), 17);
}
#[test]
fn rejects_unknown_parameters_nonfinite_time_and_quotas_without_fallback() {
    let mut p = flat(2);
    p["studies"][0]["parameters"] = json!({"unknown":"1"});
    assert!(calculate(&p).is_err());
    let mut p = flat(2);
    p["bars"][1]["time"] = json!("0");
    assert!(calculate(&p).is_err());
    let mut p = flat(2);
    p["bars"][0]["close"] = json!("NaN");
    assert!(calculate(&p).is_err());
    let mut p = flat(2);
    p["profile"] = json!("unknown");
    assert!(calculate(&p).is_err());
    let mut p = flat(2);
    p["studies"][0]["kind"] = json!("unknown");
    assert!(calculate(&p).is_err());
    let mut p = flat(2);
    p["studies"] = json!((0..33).map(|_| p["studies"][0].clone()).collect::<Vec<_>>());
    assert!(calculate(&p).is_err());
    assert!(calculate(&flat(20001)).is_err());
}
#[test]
fn prefixes_replacements_and_study_instances_are_isolated() {
    let a = calculate(&flat(20)).unwrap();
    let b = calculate(&flat(7)).unwrap();
    assert_eq!(a["rows"][6], b["rows"][6]);
    let mut p = flat(20);
    p["bars"][19]["volume"] = json!("100");
    let c = calculate(&p).unwrap();
    assert_eq!(a["rows"][18], c["rows"][18]);
    assert_ne!(a["rows"][19], c["rows"][19]);
    assert_eq!(calculate(&flat(20)).unwrap(), a);
    assert!(
        calculate(&flat(0)).unwrap()["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn williams_upper_bound_and_rounded_negative_zero_are_canonical() {
    let mut p = flat(20);
    for bar in p["bars"].as_array_mut().unwrap() {
        bar["low"] = json!("90");
    }
    p["studies"] = json!([{"id":"wr","kind":"wr","parameters":{}}]);
    let r = calculate(&p).unwrap();
    assert_eq!(number(&r["rows"][19]["values"][0]), Some(0.0));
    assert!(
        !r["rows"][19]["values"][0]
            .as_str()
            .unwrap()
            .starts_with('-')
    );
}

#[test]
fn bounded_chart_payload_preserves_outer_decoder_limits() {
    let mut p = flat(7200);
    let bars = p["bars"].to_string();
    p["bars"] = json!(bars);
    assert_eq!(
        calculate(&p).unwrap()["rows"].as_array().unwrap().len(),
        7200
    );
    p["bars"] = json!("[{\"time\":\"0\",\"time\":\"1\"}]");
    assert!(calculate(&p).is_err(), "duplicate keys remain rejected");
}
