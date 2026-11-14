//! Exact bounded financial result JSON building over existing reviewed codecs; no runtime semantics.
use crate::{
    graph::{digest, error},
    loading,
    model::{ContentId, ModelError},
    parameters::{BoundParameters, BoundValue},
};
pub use serde_json::Value as JsonValue;
use std::io::{self, Write};
pub const MAX_FINANCIAL_REPORT_BYTES: usize = 4 * 1024 * 1024;
pub fn object(fields: impl IntoIterator<Item = (impl Into<String>, JsonValue)>) -> JsonValue {
    JsonValue::Object(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn string(value: impl ToString) -> JsonValue {
    JsonValue::String(value.to_string())
}
pub fn content_id(domain: &[u8], bytes: &[u8]) -> ContentId {
    digest(domain, bytes)
}
pub fn decode_input(bytes: &[u8]) -> Result<JsonValue, ModelError> {
    loading::decode(bytes)
}
pub fn parameters(values: &BoundParameters) -> JsonValue {
    object(values.iter().map(|(key, value)| {
        (
            key,
            match value {
                BoundValue::Value(v) => loading::typed_json(v),
                BoundValue::Unbound => JsonValue::Null,
            },
        )
    }))
}
struct Limited {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit - self.bytes.len() {
            return Err(io::Error::other("financial output byte quota"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub fn encode(value: &JsonValue, limit: usize) -> Result<Vec<u8>, ModelError> {
    if limit == 0 || limit > MAX_FINANCIAL_REPORT_BYTES {
        return Err(error("financial_report_limit", "$", "invalid result quota"));
    }
    let mut pending = vec![(value, 0usize)];
    let mut items = 0usize;
    while let Some((v, depth)) = pending.pop() {
        items += 1;
        if depth > 32
            || items > 262144
            || matches!(v,JsonValue::Number(n) if !n.is_i64() && !n.is_u64())
        {
            return Err(error(
                "financial_report_value",
                "$",
                "depth/item quota or inexact number",
            ));
        }
        match v {
            JsonValue::Array(a) => pending.extend(a.iter().map(|v| (v, depth + 1))),
            JsonValue::Object(m) => pending.extend(m.values().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    let mut writer = Limited {
        bytes: vec![],
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        error(
            "financial_report_limit",
            "$",
            "financial result exceeds byte quota",
        )
    })?;
    Ok(writer.bytes)
}
