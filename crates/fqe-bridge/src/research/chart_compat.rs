//! Explicit chart-compatible study profile. Legacy exact providers stay intact.
//! Mirrors k_chart DataUtil's causal binary64 arithmetic, then serializes at 12dp.
use super::graph::uint;
use super::*;

pub const PROFILE: &str = "finger.k-chart/1";
#[derive(Default)]
pub struct ChartState {
    samples: VecDeque<(u64, Vec<f64>)>,
    accumulator: Accumulator,
}
#[derive(Default)]
struct Accumulator {
    count: usize,
    window: VecDeque<Vec<f64>>,
    sum: f64,
    fast: f64,
    slow: f64,
    signal: f64,
    gain: f64,
    absolute: f64,
    previous: Option<f64>,
    atr: Option<f64>,
}
fn float(v: Decimal) -> f64 {
    v.coefficient() as f64 / 10_f64.powi(v.scale() as i32)
}
fn decimal(v: f64, stamp: u64) -> Result<Slot> {
    if !v.is_finite() {
        return Ok(Slot { value: None, stamp });
    }
    Ok(Slot::num(num(&format!("{v:.12}"))?, stamp))
}
impl ChartState {
    pub fn step(
        &mut self,
        node: &Node,
        inputs: &BTreeMap<String, Slot>,
        stamp: u64,
        quote: bool,
    ) -> Result<BTreeMap<String, Slot>> {
        let names: &[&str] = if node.kind == "atr" {
            &["high", "low", "close"]
        } else {
            &["price"]
        };
        let sample = names
            .iter()
            .map(|p| {
                inputs[*p]
                    .value
                    .as_ref()
                    .expect("ready")
                    .number()
                    .map(float)
            })
            .collect::<Result<Vec<_>>>()?;
        let result = if quote {
            // A quote updates the current 500ms point; it is not another candle.
            let bucket = stamp / 500_000_000 * 500_000_000;
            if self.samples.back().is_some_and(|(t, _)| *t == bucket) {
                self.samples.pop_back();
            }
            self.samples.push_back((bucket, sample));
            while self
                .samples
                .front()
                .is_some_and(|(t, _)| *t < bucket.saturating_sub(120_000_000_000))
                || self.samples.len() > 244
            {
                self.samples.pop_front();
            }
            let mut accumulator = Accumulator::default();
            let mut result = BTreeMap::new();
            for (_, value) in &self.samples {
                result = accumulator.step(node, value)?;
            }
            result
        } else {
            self.accumulator.step(node, &sample)?
        };
        node.outputs
            .keys()
            .map(|port| {
                Ok((
                    port.clone(),
                    match result.get(port.as_str()) {
                        Some(value) => decimal(*value, stamp)?,
                        None => Slot { value: None, stamp },
                    },
                ))
            })
            .collect()
    }
}
impl Accumulator {
    fn step(&mut self, n: &Node, x: &[f64]) -> Result<BTreeMap<&'static str, f64>> {
        let mut result = BTreeMap::new();
        let period = if n.kind == "macd" {
            1
        } else {
            uint(n, "period")? as usize
        };
        let offset = if ["highest", "lowest"].contains(&n.kind.as_str()) {
            uint(n, "offset")? as usize
        } else {
            0
        };
        let p = period as f64;
        let value = x[0];
        self.window.push_back(x.to_vec());
        self.sum += value;
        if self.window.len() > period + offset {
            self.sum -= self.window.pop_front().expect("window")[0];
        }
        match n.kind.as_str() {
            "sma" => {
                if self.count + 1 >= period {
                    result.insert("value", self.sum / p);
                }
            }
            "ema" => {
                self.fast = if self.count == 0 {
                    value
                } else {
                    self.fast * (p - 1.0) / (p + 1.0) + value * 2.0 / (p + 1.0)
                };
                result.insert("value", self.fast);
            }
            "macd" => {
                let fast = uint(n, "fast")? as f64;
                let slow = uint(n, "slow")? as f64;
                let signal = uint(n, "signal")? as f64;
                if self.count == 0 {
                    self.fast = value;
                    self.slow = value;
                } else {
                    self.fast =
                        self.fast * (fast - 1.0) / (fast + 1.0) + value * 2.0 / (fast + 1.0);
                    self.slow =
                        self.slow * (slow - 1.0) / (slow + 1.0) + value * 2.0 / (slow + 1.0);
                }
                let line = self.fast - self.slow;
                self.signal =
                    self.signal * (signal - 1.0) / (signal + 1.0) + line * 2.0 / (signal + 1.0);
                result.insert("line", line);
                result.insert("signal", self.signal);
                result.insert("histogram", (line - self.signal) * 2.0);
            }
            "rsi" => {
                if let Some(previous) = self.previous {
                    let diff = value - previous;
                    self.gain = (diff.max(0.0) + (p - 1.0) * self.gain) / p;
                    self.absolute = (diff.abs() + (p - 1.0) * self.absolute) / p;
                }
                if self.count >= period - 1 {
                    result.insert("value", self.gain / self.absolute * 100.0);
                }
            }
            "boll" => {
                // Chart first exposes bands at index n, using sample SD (n-1).
                if self.count >= period {
                    let mean = self.sum / p;
                    let variance: f64 = self
                        .window
                        .iter()
                        .map(|x| {
                            let d = x[0] - mean;
                            d * d
                        })
                        .sum();
                    let deviation = n
                        .get("deviation")?
                        .parse::<f64>()
                        .map_err(|_| failure("research_numeric", &n.id))?;
                    let width = deviation * (variance / (p - 1.0)).sqrt();
                    result.insert("middle", mean);
                    result.insert("upper", mean + width);
                    result.insert("lower", mean - width);
                }
            }
            "highest" | "lowest" => {
                if self.window.len() == period + offset {
                    let value = self
                        .window
                        .iter()
                        .take(period)
                        .map(|x| x[0])
                        .reduce(|a, b| {
                            if n.kind == "highest" {
                                a.max(b)
                            } else {
                                a.min(b)
                            }
                        })
                        .expect("window");
                    result.insert("value", value);
                }
            }
            "atr" => {
                let tr = if let Some(previous) = self.previous {
                    (x[0] - x[1])
                        .max((x[0] - previous).abs())
                        .max((x[1] - previous).abs())
                } else {
                    x[0] - x[1]
                };
                if let Some(previous) = self.atr {
                    self.atr = Some((previous * (p - 1.0) + tr) / p);
                } else {
                    self.gain += tr;
                    if self.count + 1 == period {
                        self.atr = Some(self.gain / p);
                    }
                }
                if let Some(value) = self.atr {
                    result.insert("value", value);
                }
            }
            _ => return Err(failure("research_chart_indicator", &n.id)),
        }
        self.previous = Some(if n.kind == "atr" { x[2] } else { value });
        self.count += 1;
        Ok(result)
    }
}
