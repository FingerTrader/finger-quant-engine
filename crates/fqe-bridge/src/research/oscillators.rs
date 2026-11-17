//! Versioned KDJ / Williams %R / CCI. Shared by batches and causal graph updates.
use super::*;
pub const PROFILE: &str = "finger.indicators/3";
pub fn kinds(kind: &str) -> bool {
    matches!(kind, "kdj" | "WILLR" | "cci")
}
pub fn defaults(kind: &str) -> Vec<(&'static str, &'static str)> {
    match kind {
        "kdj" => vec![
            ("length", "9"),
            ("k_smoothing", "3"),
            ("d_smoothing", "3"),
            ("smoothing_method", "smma"),
            ("initial_k", "50"),
            ("initial_d", "50"),
        ],
        "WILLR" => vec![("length", "14")],
        "cci" => vec![("length", "20"), ("source", "hlc3"), ("constant", "0.015")],
        _ => vec![],
    }
}
pub fn ports(kind: &str) -> Vec<&'static str> {
    match kind {
        "kdj" => vec!["k", "d", "j"],
        "WILLR" => vec!["williams_r"],
        "cci" => vec!["cci"],
        _ => vec![],
    }
}
pub fn validate(kind: &str, settings: &BTreeMap<String, String>) -> Result<()> {
    for key in if kind == "kdj" {
        vec!["length", "k_smoothing", "d_smoothing"]
    } else {
        vec!["length"]
    } {
        let text = settings
            .get(key)
            .ok_or_else(|| failure("indicator_missing_parameter", key))?;
        let n = text
            .parse::<usize>()
            .map_err(|_| failure("indicator_positive_integer", key))?;
        if !(1..=10000).contains(&n) || n.to_string() != *text {
            return Err(failure("indicator_positive_integer", key));
        }
    }
    if kind == "kdj" {
        if settings.get("smoothing_method").map(String::as_str) != Some("smma") {
            return Err(failure("kdj_smoothing_method_smma", "smoothing_method"));
        }
        for key in ["initial_k", "initial_d"] {
            let v = settings
                .get(key)
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite() && (0.0..=100.0).contains(v));
            if v.is_none() {
                return Err(failure("kdj_initial_range_0_100", key));
            }
        }
    }
    if kind == "cci" {
        if !settings.get("source").is_some_and(|s| {
            ["open", "high", "low", "close", "hl2", "hlc3", "ohlc4"].contains(&s.as_str())
        }) {
            return Err(failure("cci_source", "source"));
        }
        if !settings
            .get("constant")
            .and_then(|s| s.parse::<f64>().ok())
            .is_some_and(|v| v.is_finite() && v > 0.0)
        {
            return Err(failure("cci_constant_positive", "constant"));
        }
    }
    Ok(())
}
#[derive(Default)]
pub struct State {
    window: VecDeque<Vec<f64>>,
    k: Option<f64>,
    d: Option<f64>,
}
impl State {
    pub fn step(
        &mut self,
        kind: &str,
        s: &BTreeMap<String, String>,
        x: &[f64],
    ) -> BTreeMap<&'static str, f64> {
        let length = s["length"].parse::<usize>().expect("validated");
        self.window.push_back(x.to_vec());
        if self.window.len() > length {
            self.window.pop_front();
        }
        let mut out = BTreeMap::new();
        if self.window.len() < length {
            return out;
        }
        if kind == "cci" {
            let prices = self
                .window
                .iter()
                .map(|x| super::macd::source(&s["source"], x))
                .collect::<Vec<_>>();
            if prices.iter().all(|p| *p == prices[0]) {
                out.insert("cci", 0.0);
                return out;
            }
            let mean = prices.iter().sum::<f64>() / length as f64;
            let md = prices.iter().map(|p| (p - mean).abs()).sum::<f64>() / length as f64;
            let value = if md == 0.0 {
                0.0
            } else {
                (prices[length - 1] - mean) / (s["constant"].parse::<f64>().unwrap() * md)
            };
            out.insert("cci", value);
            return out;
        }
        let hh = self
            .window
            .iter()
            .map(|x| x[1])
            .fold(f64::NEG_INFINITY, f64::max);
        let ll = self
            .window
            .iter()
            .map(|x| x[2])
            .fold(f64::INFINITY, f64::min);
        if kind == "WILLR" {
            out.insert(
                "williams_r",
                if hh == ll {
                    -50.0
                } else {
                    -100.0 * (hh - x[3]) / (hh - ll)
                },
            );
        } else {
            let rsv = if hh == ll {
                50.0
            } else {
                100.0 * (x[3] - ll) / (hh - ll)
            };
            let ks = s["k_smoothing"].parse::<f64>().unwrap();
            let ds = s["d_smoothing"].parse::<f64>().unwrap();
            let k =
                (rsv + (ks - 1.0) * self.k.unwrap_or_else(|| s["initial_k"].parse().unwrap())) / ks;
            let d =
                (k + (ds - 1.0) * self.d.unwrap_or_else(|| s["initial_d"].parse().unwrap())) / ds;
            self.k = Some(k);
            self.d = Some(d);
            out.insert("k", k);
            out.insert("d", d);
            out.insert("j", 3.0 * k - 2.0 * d);
        }
        out
    }
}
