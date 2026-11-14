//! Versioned scalar subset for parameters and future descriptor ports.

use std::fmt;

use crate::{
    InstrumentId,
    numeric::{Decimal, MAX_SCALE},
};

pub const VALUE_CONTRACT_VERSION: u32 = 1;
pub const MAX_TEXT_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrencyCode(String);

impl CurrencyCode {
    /// A case-sensitive unit identifier, not verification against a currency registry.
    pub fn new(value: impl Into<String>) -> Result<Self, TypeError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 16
            || !value
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(TypeError::InvalidCurrency);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumericUnit {
    Scalar,
    Ratio,
    Price(InstrumentId),
    Quantity(InstrumentId),
    Money(CurrencyCode),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScalarType {
    Bool,
    I64,
    U64,
    Decimal { scale: u8, unit: NumericUnit },
    Text { max_bytes: usize },
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueShape {
    Scalar,
    Series,
    Window,
    Record,
    Collection,
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeError {
    UnsupportedVersion(u32),
    UnsupportedShape(ValueShape),
    UnsupportedScalar(String),
    InvalidScale(u8),
    InvalidTextLimit,
    InvalidCurrency,
    ValueMismatch,
    NotNumeric,
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "value type error: {self:?}")
    }
}
impl std::error::Error for TypeError {}

/// Construction rejects unimplemented capabilities rather than carrying opaque values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeDescriptor {
    scalar: ScalarType,
}

impl TypeDescriptor {
    pub fn new(version: u32, shape: ValueShape, scalar: ScalarType) -> Result<Self, TypeError> {
        if version != VALUE_CONTRACT_VERSION {
            return Err(TypeError::UnsupportedVersion(version));
        }
        if shape != ValueShape::Scalar {
            return Err(TypeError::UnsupportedShape(shape));
        }
        match &scalar {
            ScalarType::Decimal { scale, .. } if *scale > MAX_SCALE => {
                return Err(TypeError::InvalidScale(*scale));
            }
            ScalarType::Text { max_bytes } if *max_bytes == 0 || *max_bytes > MAX_TEXT_BYTES => {
                return Err(TypeError::InvalidTextLimit);
            }
            ScalarType::Unsupported(name) => {
                return Err(TypeError::UnsupportedScalar(name.clone()));
            }
            _ => {}
        }
        Ok(Self { scalar })
    }

    pub fn version(&self) -> u32 {
        VALUE_CONTRACT_VERSION
    }
    pub fn shape(&self) -> ValueShape {
        ValueShape::Scalar
    }
    pub fn scalar(&self) -> &ScalarType {
        &self.scalar
    }

    pub fn numeric_limits(&self) -> Option<(i128, i128)> {
        match self.scalar {
            ScalarType::I64 => Some((i128::from(i64::MIN), i128::from(i64::MAX))),
            ScalarType::U64 => Some((0, i128::from(u64::MAX))),
            ScalarType::Decimal { .. } => Some((i128::MIN, i128::MAX)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScalarValue {
    Bool(bool),
    I64(i64),
    U64(u64),
    Decimal(Decimal),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedValue {
    descriptor: TypeDescriptor,
    value: ScalarValue,
}

impl TypedValue {
    pub fn new(descriptor: TypeDescriptor, value: ScalarValue) -> Result<Self, TypeError> {
        let valid = match (descriptor.scalar(), &value) {
            (ScalarType::Bool, ScalarValue::Bool(_))
            | (ScalarType::I64, ScalarValue::I64(_))
            | (ScalarType::U64, ScalarValue::U64(_)) => true,
            (ScalarType::Decimal { scale, .. }, ScalarValue::Decimal(v)) => *scale == v.scale(),
            (ScalarType::Text { max_bytes }, ScalarValue::Text(v)) => v.len() <= *max_bytes,
            _ => false,
        };
        if !valid {
            return Err(TypeError::ValueMismatch);
        }
        Ok(Self { descriptor, value })
    }

    pub fn descriptor(&self) -> &TypeDescriptor {
        &self.descriptor
    }
    pub fn value(&self) -> &ScalarValue {
        &self.value
    }

    /// Ordered coefficient for bounds, only after exact descriptor equality is checked.
    pub fn numeric_coefficient(&self) -> Result<i128, TypeError> {
        match self.value {
            ScalarValue::I64(v) => Ok(i128::from(v)),
            ScalarValue::U64(v) => Ok(i128::from(v)),
            ScalarValue::Decimal(v) => Ok(v.coefficient()),
            _ => Err(TypeError::NotNumeric),
        }
    }
}

/// Runtime readiness is separate from both a valid zero and an absent parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Evaluation<T> {
    Ready(T),
    Missing,
    Warmup,
    Error(crate::numeric::NumericError),
}
