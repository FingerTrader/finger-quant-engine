//! Strict outer envelope allows whole model/CSV strings; core model codecs stay unchanged.
use crate::{Failure, MAX_REQUEST};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;
struct Seed<'a> {
    depth: usize,
    remaining: &'a mut usize,
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        if self.depth > 32 || *self.remaining == 0 {
            return Err(de::Error::custom("quota"));
        }
        *self.remaining -= 1;
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded exact JSON")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        if v > u64::from(u32::MAX) {
            Err(E::custom("large integers require strings"))
        } else {
            Ok(v.into())
        }
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Value, E> {
        Err(E::custom("signed numeric literal"))
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Value, E> {
        Err(E::custom("inexact numeric literal"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        if v.len() > MAX_REQUEST {
            Err(E::custom("quota"))
        } else {
            Ok(Value::String(v.into()))
        }
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        self.visit_str(&v)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(v) = a.next_element_seed(Seed {
            depth: self.depth + 1,
            remaining: self.remaining,
        })? {
            if values.len() >= 4096 {
                return Err(de::Error::custom("quota"));
            }
            values.push(v);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = a.next_key::<String>()? {
            if key.len() > 128 || values.len() >= 4096 || values.contains_key(&key) {
                return Err(de::Error::custom("quota or duplicate key"));
            }
            let value = a.next_value_seed(Seed {
                depth: self.depth + 1,
                remaining: self.remaining,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
pub fn decode(bytes: &[u8]) -> Result<Value, Failure> {
    if bytes.is_empty() || bytes.len() > MAX_REQUEST {
        return Err(crate::fail("request_quota"));
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let mut remaining = 32768;
    let value = Seed {
        depth: 0,
        remaining: &mut remaining,
    }
    .deserialize(&mut decoder)
    .map_err(|_| crate::fail("request_json"))?;
    decoder.end().map_err(|_| crate::fail("request_json"))?;
    Ok(value)
}

/// Extended-profile wire writer. Legacy financial report bounds are unchanged.
pub(crate) fn encode(value: &Value, limit: usize) -> crate::Result<Vec<u8>> {
    use std::io::Write;
    if limit == 0 || limit > crate::MAX_RESPONSE {
        return Err(crate::fail("response_quota"));
    }
    let mut pending = vec![(value, 0usize)];
    let mut count = 0;
    while let Some((v, depth)) = pending.pop() {
        count += 1;
        if depth > 40
            || count > 2_000_000
            || matches!(v, Value::Number(n) if !n.is_u64() && !n.is_i64())
        {
            return Err(crate::fail("response_value_quota"));
        }
        match v {
            Value::Array(a) => pending.extend(a.iter().map(|v| (v, depth + 1))),
            Value::Object(m) => pending.extend(m.values().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    struct Writer {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|n| n > self.limit)
            {
                return Err(std::io::Error::other("wire quota"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Writer {
        bytes: vec![],
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| crate::fail("response_quota"))?;
    Ok(writer.bytes)
}
