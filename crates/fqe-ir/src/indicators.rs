//! Version-1 bounded local indicator providers. See docs/INDICATOR_CONTRACT.md.
use crate::{
    model::{ContentId, EntryKey, ModelError},
    parameters::{Bound, BoundParameters, BoundValue, Constraints, ParameterSchema, ParameterSpec},
    registry::{
        CallablePort, Category, Descriptor, DescriptorDraft, IncrementalProvider, IncrementalState,
        NamedValues, ProviderCall, ProviderFailure, Purity, ReadinessPolicy, RegistryBuilder,
    },
};
use fqe_core::{
    numeric::{Decimal, NumericError, Rounding},
    types::{
        Evaluation, NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape,
    },
};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, sync::Arc};

pub const WORK_SCALE: u8 = 18;
pub const OUTPUT_SCALE: u8 = 12;
pub const MAX_PERIOD: u64 = 10000;
pub const MAX_CLOSE: i128 = 1_000_000_000_000;
const WORK_FACTOR: i128 = 1_000_000_000_000_000_000;
const MIN_RSI_TOTAL: i128 = 1_000_000_000_000; // 0.000001 in working units

type Result<T> = std::result::Result<T, ProviderFailure>;
fn numeric(n: NumericError) -> ProviderFailure {
    ProviderFailure::Numeric(n)
}
fn add(a: i128, b: i128) -> Result<i128> {
    a.checked_add(b).ok_or(numeric(NumericError::Overflow))
}
fn sub(a: i128, b: i128) -> Result<i128> {
    a.checked_sub(b).ok_or(numeric(NumericError::Overflow))
}
fn mul(a: i128, b: i128) -> Result<i128> {
    a.checked_mul(b).ok_or(numeric(NumericError::Overflow))
}
fn mean(n: i128, d: i128) -> Result<i128> {
    Decimal::new(n, 0)
        .and_then(|v| v.checked_div(Decimal::new(d, 0)?, Rounding::HalfEven))
        .map(Decimal::coefficient)
        .map_err(numeric)
}
// Long decimal division avoids multiplying a working coefficient by 10^18.
fn scaled_ratio(n: i128, d: i128) -> Result<i128> {
    if n < 0 || d <= 0 {
        return Err(ProviderFailure::Failed);
    }
    let mut q = n / d;
    let mut r = n % d;
    for _ in 0..WORK_SCALE {
        r = mul(r, 10)?;
        q = add(mul(q, 10)?, r / d)?;
        r %= d;
    }
    if r > d - r || (r == d - r && q % 2 != 0) {
        q = add(q, 1)?;
    }
    Ok(q)
}
fn uint_type() -> TypeDescriptor {
    TypeDescriptor::new(1, ValueShape::Scalar, ScalarType::U64).expect("u64")
}
fn uint(v: u64) -> TypedValue {
    TypedValue::new(uint_type(), ScalarValue::U64(v)).expect("u64")
}
fn period_schema(names: &[&str]) -> ParameterSchema {
    ParameterSchema::new(
        1,
        names
            .iter()
            .map(|name| {
                ParameterSpec::new(
                    *name,
                    uint_type(),
                    true,
                    None,
                    Constraints {
                        lower: Some(Bound {
                            value: uint(1),
                            inclusive: true,
                        }),
                        upper: Some(Bound {
                            value: uint(MAX_PERIOD),
                            inclusive: true,
                        }),
                        choices: None,
                    },
                    "Positive period in accepted contiguous samples; 1..10000",
                )
                .expect("period")
            })
            .collect(),
    )
    .expect("schema")
}
fn period(p: &BoundParameters, key: &str) -> Result<usize> {
    match p.get(key) {
        Some(BoundValue::Value(v)) => match v.value() {
            ScalarValue::U64(n) if (1..=MAX_PERIOD).contains(n) => Ok(*n as usize),
            _ => Err(ProviderFailure::InvalidParameters),
        },
        _ => Err(ProviderFailure::InvalidParameters),
    }
}
fn output_type(unit: NumericUnit) -> TypeDescriptor {
    TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: OUTPUT_SCALE,
            unit,
        },
    )
    .expect("decimal")
}
#[derive(Clone, Copy)]
enum Formula {
    Sma,
    Ema,
    Macd,
    Rsi,
}
struct Factory {
    formula: Formula,
    input: TypeDescriptor,
    output: TypeDescriptor,
}
impl IncrementalProvider for Factory {
    fn validate_parameters(&self, p: &BoundParameters) -> Result<()> {
        match self.formula {
            Formula::Macd => {
                if period(p, "fast")? >= period(p, "slow")? {
                    return Err(ProviderFailure::InvalidParameters);
                }
                period(p, "signal")?;
            }
            _ => {
                period(p, "period")?;
            }
        }
        Ok(())
    }
    fn create(&self, p: &BoundParameters) -> Result<Box<dyn IncrementalState>> {
        let algo = match self.formula {
            Formula::Sma => Algorithm::Sma {
                p: period(p, "period")?,
                window: VecDeque::new(),
                sum: 0,
            },
            Formula::Ema => Algorithm::Ema(Ema::new(period(p, "period")?)),
            Formula::Macd => Algorithm::Macd {
                fast: Ema::new(period(p, "fast")?),
                slow: Ema::new(period(p, "slow")?),
                signal: Ema::new(period(p, "signal")?),
            },
            Formula::Rsi => Algorithm::Rsi {
                p: period(p, "period")?,
                previous: None,
                count: 0,
                gain: 0,
                loss: 0,
            },
        };
        Ok(Box::new(State {
            algo,
            input: self.input.clone(),
            output: self.output.clone(),
        }))
    }
}
#[derive(Clone)]
struct Ema {
    p: usize,
    count: usize,
    seed_sum: i128,
    value: Option<i128>,
}
impl Ema {
    fn new(p: usize) -> Self {
        Self {
            p,
            count: 0,
            seed_sum: 0,
            value: None,
        }
    }
    fn next(&mut self, x: i128) -> Result<Option<i128>> {
        if let Some(previous) = self.value {
            self.value = Some(mean(
                add(mul(x, 2)?, mul(previous, self.p as i128 - 1)?)?,
                self.p as i128 + 1,
            )?);
        } else {
            self.seed_sum = add(self.seed_sum, x)?;
            self.count += 1;
            if self.count == self.p {
                self.value = Some(mean(self.seed_sum, self.p as i128)?);
                self.seed_sum = 0;
            }
        }
        Ok(self.value)
    }
}
#[derive(Clone)]
enum Algorithm {
    Sma {
        p: usize,
        window: VecDeque<i128>,
        sum: i128,
    },
    Ema(Ema),
    Macd {
        fast: Ema,
        slow: Ema,
        signal: Ema,
    },
    Rsi {
        p: usize,
        previous: Option<i128>,
        count: usize,
        gain: i128,
        loss: i128,
    },
}
#[derive(Clone)]
struct State {
    algo: Algorithm,
    input: TypeDescriptor,
    output: TypeDescriptor,
}
impl IncrementalState for State {
    fn fork(&self) -> Box<dyn IncrementalState> {
        Box::new(self.clone())
    }
    fn step(&mut self, call: &ProviderCall<'_>) -> Result<NamedValues> {
        let input = call.input("close").ok_or(ProviderFailure::Failed)?;
        if input.descriptor() != &self.input {
            return Err(ProviderFailure::Failed);
        }
        let ScalarValue::Decimal(close) = input.value() else {
            return Err(ProviderFailure::Failed);
        };
        let x = close
            .rescale(WORK_SCALE, Rounding::Exact)
            .map_err(numeric)?
            .coefficient();
        if x.unsigned_abs() > (MAX_CLOSE * WORK_FACTOR) as u128 {
            return Err(numeric(NumericError::Overflow));
        }
        let values = match &mut self.algo {
            Algorithm::Sma { p, window, sum } => {
                if window.len() == *p {
                    *sum = sub(*sum, window.pop_front().ok_or(ProviderFailure::Failed)?)?;
                }
                window.push_back(x);
                *sum = add(*sum, x)?;
                vec![(
                    "value",
                    if window.len() == *p {
                        Some(mean(*sum, *p as i128)?)
                    } else {
                        None
                    },
                )]
            }
            Algorithm::Ema(e) => vec![("value", e.next(x)?)],
            Algorithm::Macd { fast, slow, signal } => {
                let f = fast.next(x)?;
                let s = slow.next(x)?;
                let line = match (f, s) {
                    (Some(f), Some(s)) => Some(sub(f, s)?),
                    _ => None,
                };
                let sig = if let Some(line) = line {
                    signal.next(line)?
                } else {
                    None
                };
                let histogram = match (line, sig) {
                    (Some(l), Some(s)) => Some(sub(l, s)?),
                    _ => None,
                };
                vec![("line", line), ("signal", sig), ("histogram", histogram)]
            }
            Algorithm::Rsi {
                p,
                previous,
                count,
                gain,
                loss,
            } => {
                let value = if let Some(prev) = previous {
                    let diff = sub(x, *prev)?;
                    let g = diff.max(0);
                    let l = (-diff).max(0);
                    if *count < *p {
                        *gain = add(*gain, g)?;
                        *loss = add(*loss, l)?;
                        *count += 1;
                        if *count == *p {
                            *gain = mean(*gain, *p as i128)?;
                            *loss = mean(*loss, *p as i128)?;
                        }
                    } else {
                        *gain = mean(add(mul(*gain, *p as i128 - 1)?, g)?, *p as i128)?;
                        *loss = mean(add(mul(*loss, *p as i128 - 1)?, l)?, *p as i128)?;
                    }
                    if *count < *p {
                        None
                    } else {
                        let total = add(*gain, *loss)?;
                        Some(if total == 0 {
                            50 * WORK_FACTOR
                        } else {
                            if total < MIN_RSI_TOTAL {
                                return Err(ProviderFailure::PrecisionLimit);
                            }
                            scaled_ratio(mul(*gain, 100)?, total)?
                        })
                    }
                } else {
                    None
                };
                *previous = Some(x);
                vec![("value", value)]
            }
        };
        values
            .into_iter()
            .map(|(name, value)| {
                let value = match value {
                    None => Evaluation::Warmup,
                    Some(v) => Evaluation::Ready(
                        TypedValue::new(
                            self.output.clone(),
                            ScalarValue::Decimal(
                                Decimal::new(v, WORK_SCALE)
                                    .and_then(|v| v.rescale(OUTPUT_SCALE, Rounding::HalfEven))
                                    .map_err(numeric)?,
                            ),
                        )
                        .map_err(|_| ProviderFailure::Failed)?,
                    ),
                };
                Ok((name.into(), value))
            })
            .collect()
    }
}
/// Source identity, not authentication, machine-code identity or a security signature.
pub fn implementation_artifact() -> ContentId {
    ContentId::from_bytes(Sha256::digest(include_bytes!("indicators.rs")).into())
}
/// Register a typed family. Choose a unique namespace when registering multiple input types.
/// Input decimal scale 0..12, Scalar or Price; output decimal scale 12 (RSI uses Ratio).
pub fn register(
    builder: &mut RegistryBuilder,
    namespace: &str,
    input: TypeDescriptor,
) -> std::result::Result<(), ModelError> {
    let unit = match input.scalar() {
        ScalarType::Decimal {
            scale: 0..=12,
            unit: NumericUnit::Scalar,
        } => NumericUnit::Scalar,
        ScalarType::Decimal {
            scale: 0..=12,
            unit: NumericUnit::Price(id),
        } => NumericUnit::Price(id.clone()),
        _ => {
            return Err(ModelError::new(
                "indicator_type",
                "$.input",
                "requires decimal scale 0..12, Scalar or Price",
            ));
        }
    };
    // Validate all family descriptors before mutating the builder.
    let mut entries = Vec::new();
    for (name, formula, params, ports) in [
        ("sma", Formula::Sma, vec!["period"], vec!["value"]),
        ("ema", Formula::Ema, vec!["period"], vec!["value"]),
        (
            "macd",
            Formula::Macd,
            vec!["fast", "slow", "signal"],
            vec!["line", "signal", "histogram"],
        ),
        ("rsi", Formula::Rsi, vec!["period"], vec!["value"]),
    ] {
        let output = output_type(if matches!(formula, Formula::Rsi) {
            NumericUnit::Ratio
        } else {
            unit.clone()
        });
        let mut draft = DescriptorDraft::pure(
            EntryKey {
                id: format!("{namespace}.{name}"),
                version: "1".into(),
            },
            implementation_artifact(),
            period_schema(&params),
            format!(
                "{name}/1; mean seeds; Wilder RSI flat=50; work18/output12 HalfEven; abs(close)<=1e12; periods<=10000; RSI nonzero mean gain+loss>=1e-6; accepted Missing/Warmup resets; see INDICATOR_CONTRACT/1"
            ),
        );
        draft.category = Category::Indicator;
        draft.purity = Purity::Stateful;
        draft.readiness = ReadinessPolicy::ProviderManaged;
        draft.history = match formula {
            Formula::Macd => 19999,
            Formula::Rsi => 10001,
            _ => 10000,
        };
        draft.state_schema = Some(format!("fqe.{name}.state/1"));
        draft.required_capabilities = vec!["provider.incremental-scalar/1".into()];
        draft.inputs = vec![CallablePort::one("close", input.clone())];
        draft.outputs = ports
            .iter()
            .map(|p| CallablePort::one(*p, output.clone()))
            .collect();
        entries.push((
            Descriptor::new(draft)?,
            Arc::new(Factory {
                formula,
                input: input.clone(),
                output,
            }) as Arc<dyn IncrementalProvider>,
        ));
    }
    for (descriptor, factory) in entries {
        builder.register_incremental(descriptor, factory)?;
    }
    Ok(())
}
