//! MACD v2: one causal kernel for chart batches, research and live updates.
use super::*;
pub const PROFILE: &str = "finger.indicators/2";
pub const DEFAULTS: &[(&str, &str)] = &[
    ("fast", "12"),
    ("slow", "26"),
    ("signal", "9"),
    ("source", "close"),
    ("oscillator_ma_type", "ema"),
    ("signal_ma_type", "ema"),
    ("histogram_multiplier", "1.0"),
];
pub fn validate(s: &BTreeMap<String, String>) -> Result<()> {
    for key in ["fast", "slow", "signal"] {
        let text = s
            .get(key)
            .ok_or_else(|| failure("macd_missing_parameter", key))?;
        let n = text
            .parse::<usize>()
            .map_err(|_| failure("macd_positive_integer", key))?;
        if !(1..=10000).contains(&n) || n.to_string() != *text {
            return Err(failure("macd_positive_integer", key));
        }
    }
    if s["fast"].parse::<usize>().unwrap() >= s["slow"].parse::<usize>().unwrap() {
        return Err(failure("macd_fast_must_be_less_than_slow", "fast"));
    }
    for (key, options) in [
        (
            "source",
            &["open", "high", "low", "close", "hl2", "hlc3", "ohlc4"][..],
        ),
        ("oscillator_ma_type", &["ema", "sma"][..]),
        ("signal_ma_type", &["ema", "sma"][..]),
        ("histogram_multiplier", &["1", "1.0", "2", "2.0"][..]),
    ] {
        if !s.get(key).is_some_and(|v| options.contains(&v.as_str())) {
            return Err(failure("macd_invalid_parameter", key));
        }
    }
    Ok(())
}
/// Expand chart binary64 transport's scientific notation without rounding digits.
/// Graph/broker wire values already use plain decimal strings.
pub fn chart_decimal(text: &str) -> Result<Decimal> {
    if text.len() > 64 {
        return Err(failure("macd_input_decimal_precision_12", "source"));
    }
    let Some(index) = text.find('e').or_else(|| text.find('E')) else {
        return num(text);
    };
    let exponent = text[index + 1..]
        .parse::<i32>()
        .map_err(|_| failure("macd_input_decimal_precision_12", "source"))?;
    if !(-32..=32).contains(&exponent) {
        return Err(failure("macd_input_decimal_precision_12", "source"));
    }
    let mantissa = &text[..index];
    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.strip_prefix('-').unwrap_or(mantissa);
    let dot = mantissa.find('.').unwrap_or(mantissa.len());
    let digits = mantissa.replace('.', "");
    if digits.is_empty()
        || mantissa.matches('.').count() > 1
        || !digits.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(failure("macd_input_decimal_precision_12", "source"));
    }
    let point = dot as i32 + exponent;
    let expanded = if point <= 0 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else if point as usize >= digits.len() {
        format!("{}{}", digits, "0".repeat(point as usize - digits.len()))
    } else {
        format!(
            "{}.{}",
            &digits[..point as usize],
            &digits[point as usize..]
        )
    };
    num(&format!("{}{}", if negative { "-" } else { "" }, expanded))
}
pub fn source(s: &str, x: &[f64]) -> f64 {
    if x.len() == 1 {
        return x[0];
    }
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
    sum: f64,
    ema: Option<f64>,
}
impl Average {
    fn step(&mut self, value: f64, period: usize, kind: &str) -> Option<f64> {
        self.window.push_back(value);
        self.sum += value;
        if self.window.len() > period {
            self.sum -= self.window.pop_front().unwrap();
        }
        if self.window.len() < period {
            return None;
        }
        if kind == "sma" {
            return Some(self.sum / period as f64);
        }
        let next = match self.ema {
            None => self.sum / period as f64,
            Some(previous) => previous + (value - previous) * (2.0 / (period as f64 + 1.0)),
        };
        self.ema = Some(next);
        Some(next)
    }
}
#[derive(Default)]
pub struct State {
    fast: Average,
    slow: Average,
    signal: Average,
}
impl State {
    pub fn step(&mut self, s: &BTreeMap<String, String>, x: &[f64]) -> BTreeMap<&'static str, f64> {
        let value = source(&s["source"], x);
        let f = self
            .fast
            .step(value, s["fast"].parse().unwrap(), &s["oscillator_ma_type"]);
        let l = self
            .slow
            .step(value, s["slow"].parse().unwrap(), &s["oscillator_ma_type"]);
        let mut out = BTreeMap::new();
        if let (Some(f), Some(l)) = (f, l) {
            let line = f - l;
            out.insert("macd_line", line);
            out.insert("line", line); // legacy graph port alias
            if let Some(signal) =
                self.signal
                    .step(line, s["signal"].parse().unwrap(), &s["signal_ma_type"])
            {
                out.insert("signal_line", signal);
                out.insert("signal", signal);
                out.insert(
                    "histogram",
                    s["histogram_multiplier"].parse::<f64>().unwrap() * (line - signal),
                );
            }
        }
        out
    }
}
