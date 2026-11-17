//! Professional indicators v4. One causal kernel for native chart and graphs.
use super::*;
pub const PROFILE: &str = "finger.indicators/4";
pub const KINDS: &[&str] = &["MA", "BOLL", "ATR", "ADX", "VWAP"];
pub fn kinds(kind: &str) -> bool {
    KINDS.contains(&kind)
}
pub fn defaults(kind: &str) -> Vec<(&'static str, &'static str)> {
    match kind {
        "MA" => vec![("type", "ema"), ("length", "20"), ("source", "close")],
        "BOLL" => vec![
            ("length", "20"),
            ("source", "close"),
            ("ma_type", "sma"),
            ("stddev_multiplier", "2.0"),
        ],
        "ATR" => vec![("length", "14"), ("smoothing", "rma")],
        "ADX" => vec![("di_length", "14"), ("adx_smoothing", "14")],
        "VWAP" => vec![
            ("source", "hlc3"),
            ("anchor", "session"),
            ("calendar", "unknown"),
            ("timezone", "UTC"),
            ("volume_type", "unknown"),
            ("bar_interval_seconds", "60"),
        ],
        _ => vec![],
    }
}
pub fn schema(kind: &str) -> V {
    o(defaults(kind).into_iter().map(|(key, value)| {
        let choices: &[&str] = match key {
            "source" => &["open", "high", "low", "close", "hl2", "hlc3", "ohlc4"],
            "type" => &["sma", "ema", "wma"],
            "ma_type" => &["sma"],
            "smoothing" => &["rma", "sma", "ema"],
            "anchor" => &["session", "weekly", "monthly"],
            "calendar" => &["unknown", "24x7"],
            "volume_type" => &["unknown", "real", "tick"],
            _ => &[],
        };
        let integer = matches!(
            key,
            "length" | "di_length" | "adx_smoothing" | "bar_interval_seconds"
        );
        let mut fields = vec![
            ("default", s(value)),
            (
                "type",
                s(if integer {
                    "positive_integer"
                } else if key == "stddev_multiplier" {
                    "positive_number"
                } else if choices.is_empty() {
                    "string"
                } else {
                    "enum"
                }),
            ),
        ];
        if integer {
            fields.extend([
                ("minimum", s("1")),
                (
                    "maximum",
                    s(if key == "bar_interval_seconds" {
                        "86399"
                    } else {
                        "10000"
                    }),
                ),
            ]);
        }
        if key == "stddev_multiplier" {
            fields.extend([("exclusive_minimum", s("0")), ("maximum", s("100"))]);
        }
        if !choices.is_empty() {
            fields.push(("choices", V::Array(choices.iter().map(s).collect())));
        }
        (key, o(fields))
    }))
}
pub fn ports(kind: &str) -> Vec<&'static str> {
    match kind {
        "MA" => vec!["value"],
        "BOLL" => vec!["upper", "middle", "lower"],
        "ATR" => vec!["atr"],
        "ADX" => vec!["adx", "plus_di", "minus_di"],
        "VWAP" => vec!["vwap"],
        _ => vec![],
    }
}
pub fn pane(kind: &str) -> &'static str {
    if matches!(kind, "ATR" | "ADX") {
        "pane"
    } else {
        "overlay"
    }
}
pub fn validate(kind: &str, s: &BTreeMap<String, String>) -> Result<()> {
    for (k, _) in defaults(kind) {
        let v = s
            .get(k)
            .ok_or_else(|| failure("indicator_missing_parameter", k))?;
        if matches!(
            k,
            "length" | "di_length" | "adx_smoothing" | "bar_interval_seconds"
        ) {
            let n = v
                .parse::<usize>()
                .map_err(|_| failure("indicator_positive_integer", k))?;
            if !(1..=if k == "bar_interval_seconds" {
                86399
            } else {
                10000
            })
                .contains(&n)
                || n.to_string() != *v
            {
                return Err(failure("indicator_positive_integer", k));
            }
        }
    }
    for (key, choices) in [
        (
            "source",
            &["open", "high", "low", "close", "hl2", "hlc3", "ohlc4"][..],
        ),
        ("type", &["sma", "ema", "wma"][..]),
        ("ma_type", &["sma"][..]),
        ("smoothing", &["rma", "sma", "ema"][..]),
        ("anchor", &["session", "weekly", "monthly"][..]),
        ("calendar", &["unknown", "24x7"][..]),
        ("volume_type", &["unknown", "real", "tick"][..]),
    ] {
        if let Some(v) = s.get(key)
            && !choices.contains(&v.as_str())
        {
            return Err(failure("indicator_parameter_choice", key));
        }
    }
    if let Some(v) = s.get("stddev_multiplier")
        && !v
            .parse::<f64>()
            .is_ok_and(|n| n.is_finite() && n > 0.0 && n <= 100.0)
    {
        return Err(failure(
            "indicator_multiplier_positive",
            "stddev_multiplier",
        ));
    }
    if kind == "VWAP" {
        timezone_offset(&s["timezone"])?;
        let interval = s["bar_interval_seconds"].parse::<i64>().expect("validated");
        if interval >= 86400 || 86400 % interval != 0 {
            return Err(failure(
                "vwap_intraday_interval_divides_session",
                "bar_interval_seconds",
            ));
        }
    }
    Ok(())
}
fn timezone_offset(s: &str) -> Result<i64> {
    if s == "UTC" {
        return Ok(0);
    }
    let b = s.as_bytes();
    if b.len() != 9
        || &b[..3] != b"UTC"
        || !matches!(b[3], b'+' | b'-')
        || b[6] != b':'
        || !b[4..6].iter().all(u8::is_ascii_digit)
        || !b[7..9].iter().all(u8::is_ascii_digit)
    {
        return Err(failure(
            "vwap_timezone_fixed_utc_offset_required",
            "timezone",
        ));
    }
    let h = s[4..6]
        .parse::<i64>()
        .map_err(|_| failure("vwap_timezone", "timezone"))?;
    let m = s[7..9]
        .parse::<i64>()
        .map_err(|_| failure("vwap_timezone", "timezone"))?;
    if h > 14 || m > 59 || (h == 14 && m != 0) {
        return Err(failure("vwap_timezone", "timezone"));
    }
    Ok((h * 60 + m) * 60 * if b[3] == b'-' { -1 } else { 1 })
}
fn month_start(days: i64) -> i64 {
    // Gregorian civil date, Howard Hinnant era decomposition; no timezone library.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    days - day + 1
}
pub fn anchor_start(time_ms: u64, s: &BTreeMap<String, String>) -> i64 {
    let offset = timezone_offset(&s["timezone"]).expect("validated");
    let day = (time_ms as i64 / 1000 + offset).div_euclid(86400);
    let d = match s["anchor"].as_str() {
        "weekly" => day - (day + 3).rem_euclid(7),
        "monthly" => month_start(day),
        _ => day,
    };
    d * 86400 - offset
}
fn source(x: &[f64], s: &str) -> f64 {
    match s {
        "open" => x[0],
        "high" => x[1],
        "low" => x[2],
        "hl2" => (x[1] + x[2]) / 2.0,
        "hlc3" => (x[1] + x[2] + x[3]) / 3.0,
        "ohlc4" => (x[0] + x[1] + x[2] + x[3]) / 4.0,
        _ => x[3],
    }
}
#[derive(Default)]
struct Average {
    window: VecDeque<f64>,
    value: Option<f64>,
}
impl Average {
    fn step(&mut self, x: f64, n: usize, kind: &str) -> Option<f64> {
        self.window.push_back(x);
        if self.window.len() > n {
            self.window.pop_front();
        }
        if self.window.len() < n {
            return None;
        }
        let value = match kind {
            "sma" => self.window.iter().sum::<f64>() / n as f64,
            "wma" => {
                self.window
                    .iter()
                    .enumerate()
                    .map(|(i, v)| *v * (i + 1) as f64)
                    .sum::<f64>()
                    / (n * (n + 1) / 2) as f64
            }
            _ => {
                let alpha = if kind == "rma" {
                    1.0 / n as f64
                } else {
                    2.0 / (n + 1) as f64
                };
                self.value.map_or_else(
                    || self.window.iter().sum::<f64>() / n as f64,
                    |p| alpha * x + (1.0 - alpha) * p,
                )
            }
        };
        self.value = Some(value);
        Some(value)
    }
}
#[derive(Default)]
pub struct State {
    price: Average,
    tr: Average,
    plus: Average,
    minus: Average,
    dx: Average,
    previous: Option<[f64; 3]>,
    window: VecDeque<f64>,
    anchor: Option<i64>,
    last_time: Option<u64>,
    complete: bool,
    pv: f64,
    volume: f64,
}
impl State {
    pub fn step(
        &mut self,
        kind: &str,
        s: &BTreeMap<String, String>,
        x: &[f64],
        time_ms: u64,
    ) -> BTreeMap<&'static str, f64> {
        let mut out = BTreeMap::new();
        let n = || s["length"].parse::<usize>().expect("validated");
        match kind {
            "MA" => {
                if let Some(v) = self.price.step(source(x, &s["source"]), n(), &s["type"]) {
                    out.insert("value", v);
                }
            }
            "BOLL" => {
                let p = source(x, &s["source"]);
                self.window.push_back(p);
                if self.window.len() > n() {
                    self.window.pop_front();
                }
                if self.window.len() == n() {
                    let mean = self.window.iter().sum::<f64>() / n() as f64;
                    let variance =
                        self.window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n() as f64;
                    let d =
                        variance.sqrt() * s["stddev_multiplier"].parse::<f64>().expect("validated");
                    out.extend([("upper", mean + d), ("middle", mean), ("lower", mean - d)]);
                }
            }
            "ATR" | "ADX" => {
                let tr = self.previous.map_or(x[1] - x[2], |p| {
                    (x[1] - x[2])
                        .max((x[1] - p[2]).abs())
                        .max((x[2] - p[2]).abs())
                });
                if kind == "ATR" {
                    if let Some(v) = self.tr.step(tr, n(), &s["smoothing"]) {
                        out.insert("atr", v);
                    }
                } else {
                    let length = s["di_length"].parse().expect("validated");
                    let (up, down) = self
                        .previous
                        .map_or((0.0, 0.0), |p| (x[1] - p[0], p[1] - x[2]));
                    let plus = if up > down && up > 0.0 { up } else { 0.0 };
                    let minus = if down > up && down > 0.0 { down } else { 0.0 };
                    let tr = self.tr.step(tr, length, "rma");
                    let p = self.plus.step(plus, length, "rma");
                    let m = self.minus.step(minus, length, "rma");
                    if let (Some(t), Some(p), Some(m)) = (tr, p, m) {
                        let p = if t == 0.0 { 0.0 } else { 100.0 * p / t };
                        let m = if t == 0.0 { 0.0 } else { 100.0 * m / t };
                        out.extend([("plus_di", p), ("minus_di", m)]);
                        let dx = if p + m == 0.0 {
                            0.0
                        } else {
                            100.0 * (p - m).abs() / (p + m)
                        };
                        if let Some(v) =
                            self.dx
                                .step(dx, s["adx_smoothing"].parse().expect("validated"), "rma")
                        {
                            out.insert("adx", v);
                        }
                    }
                }
                self.previous = Some([x[1], x[2], x[3]]);
            }
            "VWAP" => {
                let anchor = anchor_start(time_ms, s);
                let interval = s["bar_interval_seconds"].parse::<u64>().expect("validated") * 1000;
                if self.anchor != Some(anchor) {
                    self.anchor = Some(anchor);
                    self.pv = 0.0;
                    self.volume = 0.0;
                    self.complete = time_ms as i64 == anchor * 1000;
                    self.last_time = None;
                }
                if self
                    .last_time
                    .is_some_and(|t| t.checked_add(interval) != Some(time_ms))
                {
                    self.complete = false;
                }
                self.last_time = Some(time_ms);
                if x[4].is_finite() && x[4] >= 0.0 {
                    self.pv += source(x, &s["source"]) * x[4];
                    self.volume += x[4];
                } else {
                    self.complete = false;
                }
                if self.complete
                    && s["calendar"] == "24x7"
                    && matches!(s["volume_type"].as_str(), "real" | "tick")
                    && self.volume > 0.0
                {
                    out.insert("vwap", self.pv / self.volume);
                }
            }
            _ => {}
        }
        out
    }
}
