//! Validated, immutable parameter definitions and per-run bindings. No wire decoder.

use std::{collections::BTreeMap, fmt};

use fqe_core::types::{Evaluation, TypeDescriptor, TypedValue};

pub const PARAMETER_SCHEMA_VERSION: u32 = 1;
pub const MAX_PARAMETERS: usize = 256;
pub const MAX_CHOICES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bound {
    pub value: TypedValue,
    pub inclusive: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Constraints {
    pub lower: Option<Bound>,
    pub upper: Option<Bound>,
    /// A finite typed enum set; None is unrestricted, Some(empty) is invalid.
    pub choices: Option<Vec<TypedValue>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterError {
    UnsupportedVersion(u32),
    TooManyParameters,
    InvalidKey(String),
    DescriptionTooLong(String),
    DuplicateKey(String),
    UnknownKey(String),
    RequiredWithDefault(String),
    InvalidConstraints(String),
    InvalidDefault(String),
    MissingRequired(String),
    InvalidInput(String),
    TypeMismatch(String),
    OutOfRange(String),
    NotAllowed(String),
}

impl fmt::Display for ParameterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parameter error: {self:?}")
    }
}
impl std::error::Error for ParameterError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSpec {
    key: String,
    descriptor: TypeDescriptor,
    required: bool,
    default: Option<TypedValue>,
    constraints: Constraints,
    description: String,
}

impl ParameterSpec {
    pub fn new(
        key: impl Into<String>,
        descriptor: TypeDescriptor,
        required: bool,
        default: Option<TypedValue>,
        constraints: Constraints,
        description: impl Into<String>,
    ) -> Result<Self, ParameterError> {
        let key = key.into();
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(ParameterError::InvalidKey(key));
        }
        let description = description.into();
        if description.len() > 4096 {
            return Err(ParameterError::DescriptionTooLong(key));
        }
        if required && default.is_some() {
            return Err(ParameterError::RequiredWithDefault(key));
        }
        let spec = Self {
            key,
            descriptor,
            required,
            default,
            constraints,
            description,
        };
        spec.validate_constraints()?;
        if let Some(default) = &spec.default {
            spec.validate_value(default)
                .map_err(|_| ParameterError::InvalidDefault(spec.key.clone()))?;
        }
        Ok(spec)
    }

    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn descriptor(&self) -> &TypeDescriptor {
        &self.descriptor
    }
    pub fn required(&self) -> bool {
        self.required
    }
    pub fn default_value(&self) -> Option<&TypedValue> {
        self.default.as_ref()
    }
    pub fn constraints(&self) -> &Constraints {
        &self.constraints
    }
    pub fn description(&self) -> &str {
        &self.description
    }

    fn invalid_constraints(&self) -> ParameterError {
        ParameterError::InvalidConstraints(self.key.clone())
    }

    /// Effective integer coefficient limits also detect empty discrete/open intervals.
    fn range(&self) -> Result<Option<(i128, i128)>, ParameterError> {
        if self.constraints.lower.is_none() && self.constraints.upper.is_none() {
            return Ok(None);
        }
        let (mut lower, mut upper) = self
            .descriptor
            .numeric_limits()
            .ok_or_else(|| self.invalid_constraints())?;
        if let Some(bound) = &self.constraints.lower {
            if bound.value.descriptor() != &self.descriptor {
                return Err(self.invalid_constraints());
            }
            let value = bound
                .value
                .numeric_coefficient()
                .map_err(|_| self.invalid_constraints())?;
            lower = if bound.inclusive {
                value
            } else {
                value
                    .checked_add(1)
                    .ok_or_else(|| self.invalid_constraints())?
            };
        }
        if let Some(bound) = &self.constraints.upper {
            if bound.value.descriptor() != &self.descriptor {
                return Err(self.invalid_constraints());
            }
            let value = bound
                .value
                .numeric_coefficient()
                .map_err(|_| self.invalid_constraints())?;
            upper = if bound.inclusive {
                value
            } else {
                value
                    .checked_sub(1)
                    .ok_or_else(|| self.invalid_constraints())?
            };
        }
        if lower > upper {
            return Err(self.invalid_constraints());
        }
        Ok(Some((lower, upper)))
    }

    fn validate_constraints(&self) -> Result<(), ParameterError> {
        self.range()?;
        if let Some(choices) = &self.constraints.choices {
            if choices.is_empty() || choices.len() > MAX_CHOICES {
                return Err(self.invalid_constraints());
            }
            for (i, value) in choices.iter().enumerate() {
                if choices[..i].contains(value) {
                    return Err(self.invalid_constraints());
                }
                self.validate_value(value)
                    .map_err(|_| self.invalid_constraints())?;
            }
        }
        Ok(())
    }

    fn validate_value(&self, value: &TypedValue) -> Result<(), ParameterError> {
        if value.descriptor() != &self.descriptor {
            return Err(ParameterError::TypeMismatch(self.key.clone()));
        }
        if let Some((lower, upper)) = self.range()? {
            let coefficient = value
                .numeric_coefficient()
                .map_err(|_| self.invalid_constraints())?;
            if coefficient < lower || coefficient > upper {
                return Err(ParameterError::OutOfRange(self.key.clone()));
            }
        }
        if let Some(choices) = &self.constraints.choices
            && !choices.contains(value)
        {
            return Err(ParameterError::NotAllowed(self.key.clone()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterInput {
    Value(Evaluation<TypedValue>),
    Null,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundValue {
    Unbound,
    Value(TypedValue),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundParameters {
    values: BTreeMap<String, BoundValue>,
}

impl BoundParameters {
    /// None means unknown key; Some(Unbound) means a declared optional absent value.
    pub fn get(&self, key: &str) -> Option<&BoundValue> {
        self.values.get(key)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &BoundValue)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSchema {
    specs: BTreeMap<String, ParameterSpec>,
}

impl ParameterSchema {
    pub fn new(version: u32, specs: Vec<ParameterSpec>) -> Result<Self, ParameterError> {
        if version != PARAMETER_SCHEMA_VERSION {
            return Err(ParameterError::UnsupportedVersion(version));
        }
        if specs.len() > MAX_PARAMETERS {
            return Err(ParameterError::TooManyParameters);
        }
        let mut map = BTreeMap::new();
        for spec in specs {
            let key = spec.key.clone();
            if map.insert(key.clone(), spec).is_some() {
                return Err(ParameterError::DuplicateKey(key));
            }
        }
        Ok(Self { specs: map })
    }

    pub fn version(&self) -> u32 {
        PARAMETER_SCHEMA_VERSION
    }
    pub fn specs(&self) -> impl Iterator<Item = &ParameterSpec> {
        self.specs.values()
    }

    /// Accept a sequence so duplicate keys are detected before any lossy map conversion.
    /// Construction is all-or-nothing; existing bindings and definitions are never edited.
    pub fn bind(
        &self,
        supplied: impl IntoIterator<Item = (String, ParameterInput)>,
    ) -> Result<BoundParameters, ParameterError> {
        let mut values = BTreeMap::new();
        for (i, (key, input)) in supplied.into_iter().enumerate() {
            if i >= MAX_PARAMETERS {
                return Err(ParameterError::TooManyParameters);
            }
            if values.contains_key(&key) {
                return Err(ParameterError::DuplicateKey(key));
            }
            let spec = self
                .specs
                .get(&key)
                .ok_or_else(|| ParameterError::UnknownKey(key.clone()))?;
            let ParameterInput::Value(Evaluation::Ready(value)) = input else {
                return Err(ParameterError::InvalidInput(key));
            };
            spec.validate_value(&value)?;
            values.insert(key, BoundValue::Value(value));
        }
        for (key, spec) in &self.specs {
            if values.contains_key(key) {
                continue;
            }
            let value = match &spec.default {
                Some(value) => BoundValue::Value(value.clone()),
                None if spec.required => return Err(ParameterError::MissingRequired(key.clone())),
                None => BoundValue::Unbound,
            };
            values.insert(key.clone(), value);
        }
        Ok(BoundParameters { values })
    }
}
