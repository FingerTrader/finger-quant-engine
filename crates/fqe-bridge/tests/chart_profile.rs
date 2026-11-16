//! Public synthetic chart oracle exported by Desk's DataUtil comparison test.
use fqe_bridge::research::live::Signals;
use serde_json::{Value, json};
fn compare(text: &str) {
    let fixture: Value = serde_json::from_str(text).unwrap();
    let mut signals = Signals::default();
    let opened = signals
        .execute("signals.open", &json!({"model":fixture["model"]}))
        .unwrap();
    assert_eq!(opened["indicator_profile"], fixture["profile"]);
    for (index, sample) in fixture["samples"].as_array().unwrap().iter().enumerate() {
        let response = signals.execute("signals.advance_multi", &json!({
            "handle":opened["handle"],"time_ns":sample["stamp"],"decision":false,
            "account":{},"positions":[],"events":[{"stream":"live","sample_ns":sample["stamp"],"values":sample["fields"]}]
        })).unwrap();
        assert!(response["intents"].as_array().unwrap().is_empty());
        for (column, actual) in response["chart_columns"]
            .as_array()
            .unwrap()
            .iter()
            .zip(response["chart_sample"]["values"].as_array().unwrap())
        {
            let key = format!(
                "{}.{}",
                column["node"].as_str().unwrap(),
                column["port"].as_str().unwrap()
            );
            let expected = &sample["expected"][&key];
            if expected.is_null() {
                assert!(actual.is_null(), "{key} sample {index}: {actual}");
            } else {
                let actual = actual.as_str().unwrap().parse::<f64>().unwrap();
                assert!(
                    (actual - expected.as_f64().unwrap()).abs() <= 1e-9,
                    "{key} sample {index}: {actual} vs {expected}"
                );
            }
        }
    }
}
#[test]
fn chart_closed_bar_oracle_covers_every_native_study() {
    compare(include_str!("fixtures/chart-profile-bars.json"));
}
#[test]
fn chart_quote_oracle_covers_bucket_replacement_and_window_pruning() {
    compare(include_str!("fixtures/chart-profile-quotes.json"));
}
