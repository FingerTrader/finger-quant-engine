//! Exact bounded historical CSV parsing; duplicate/conflicting rows are rejected, never repaired.
use crate::{
    graph::{digest, error},
    history::{CSV_HEADER, HistoricalDataset, HistorySpec, MAX_HISTORY_BYTES, Result},
};
use fqe_core::{
    bars::{BarInput, ClosedBar},
    finance,
    types::{NumericUnit, ScalarType, ScalarValue, TypeDescriptor, TypedValue, ValueShape},
    utc,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
fn fields(line: &str, path: &str) -> Result<Vec<String>> {
    if line.len() > 8192 {
        return Err(error("history_csv_limit", path, "row exceeds8192 bytes"));
    }
    let mut fields = vec![];
    let mut value = String::new();
    let mut chars = line.chars().peekable();
    let mut quoted = false;
    let mut ended = false;
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    value.push('"')
                } else {
                    quoted = false;
                    ended = true
                }
            } else {
                value.push(c)
            }
        } else if c == ',' {
            fields.push(std::mem::take(&mut value));
            ended = false;
        } else if c == '"' && value.is_empty() && !ended {
            quoted = true
        } else if c == '"' || ended {
            return Err(error(
                "history_csv",
                path,
                "invalid quoting or content after closing quote",
            ));
        } else {
            value.push(c)
        }
        if fields.len() > 10 {
            return Err(error("history_csv", path, "too many fields"));
        }
    }
    if quoted {
        return Err(error(
            "history_csv",
            path,
            "unterminated quote; embedded newlines unsupported",
        ));
    }
    fields.push(value);
    if fields.len() != 10 {
        return Err(error("history_csv", path, "exactly10 columns required"));
    }
    Ok(fields)
}
pub(crate) fn load(
    spec: Arc<HistorySpec>,
    bytes: &[u8],
    previous: Option<&HistoricalDataset>,
) -> Result<HistoricalDataset> {
    if bytes.len() > MAX_HISTORY_BYTES {
        return Err(error("history_limit", "$.csv", "CSV exceeds64MiB"));
    }
    match (spec.parent, previous) {
        (None, None) => {}
        (Some(parent), Some(previous))
            if parent == previous.content_id()
                && previous.spec.revision.checked_add(1) == Some(spec.revision)
                && spec.instrument == previous.spec.instrument
                && spec.duration_ns == previous.spec.duration_ns
                && spec.label == previous.spec.label
                && spec.profile == previous.spec.profile
                && spec.acquired_ns >= previous.spec.acquired_ns => {}
        _ => {
            return Err(error(
                "history_revision",
                "$.source",
                "revision requires exact immutable parent and compatible instrument/source/profile/nondecreasing acquisition",
            ));
        }
    }
    let source = digest(b"", bytes);
    let text =
        std::str::from_utf8(bytes).map_err(|_| error("history_csv", "$.csv", "UTF-8 required"))?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| error("history_csv", "$.csv", "missing header"))?;
    if fields(header, "$.csv.header")?
        != CSV_HEADER.split(',').map(str::to_owned).collect::<Vec<_>>()
    {
        return Err(error(
            "history_csv",
            "$.csv.header",
            "versioned column names/order required",
        ));
    }
    let d = spec.instrument.declaration();
    let mut bars = Vec::with_capacity(spec.expected_rows);
    let mut session = 0;
    let mut expected_open = spec.sessions[0].open_ns;
    let mut hash = Sha256::new();
    hash.update(b"FQE-HISTORY-BARS/1\n");
    for (index, line) in lines.enumerate() {
        let path = format!("$.csv.rows[{}]", index + 1);
        if index >= spec.expected_rows {
            return Err(error(
                "history_rows",
                &path,
                "extra/duplicate/unexpected row",
            ));
        }
        let f = fields(line, &path)?;
        if f[0] != d.instrument.as_str() || f[1] != spec.timeframe || f[9] != "true" {
            return Err(error(
                "history_binding",
                &path,
                "bound instrument/timeframe and explicitly closed bar required",
            ));
        }
        let open_ns =
            utc::parse_utc_ns(&f[2]).map_err(|e| error("history_time", &path, &e.to_string()))?;
        let close_ns =
            utc::parse_utc_ns(&f[3]).map_err(|e| error("history_time", &path, &e.to_string()))?;
        if open_ns != expected_open || close_ns.checked_sub(open_ns) != Some(spec.duration_ns) {
            return Err(error(
                "history_order_gap",
                &path,
                "duplicate/backward/missing/out-of-session or malformed interval",
            ));
        }
        let price = |s: &str| -> Result<TypedValue> {
            let v = finance::parse_decimal(s, d.price_step.scale())
                .map_err(|e| error("history_decimal", &path, &e.to_string()))?;
            spec.instrument
                .price(v)
                .map_err(|e| error("history_price", &path, &e.to_string()))?;
            typed(v, NumericUnit::Price(d.instrument.clone()), &path)
        };
        let volume = finance::parse_decimal(&f[8], d.quantity_step.scale())
            .map_err(|e| error("history_decimal", &path, &e.to_string()))?;
        if volume.coefficient() < 0
            || volume.coefficient()
                > 1_000_000_000_000_i128 * 10_i128.pow(u32::from(volume.scale()))
        {
            return Err(error(
                "history_volume",
                &path,
                "volume must be0..1e12 base quantity",
            ));
        }
        let b = ClosedBar::new(BarInput {
            sequence: index as u64 + 1,
            timestamp_ns: close_ns,
            instrument: d.instrument.clone(),
            timeframe_ns: spec.duration_ns,
            open_ns,
            close_ns,
            closed: true,
            open: price(&f[4])?,
            high: price(&f[5])?,
            low: price(&f[6])?,
            close: price(&f[7])?,
            volume: typed(volume, NumericUnit::Quantity(d.instrument.clone()), &path)?,
        })
        .map_err(|e| error("history_bar", &path, &e.to_string()))?;
        let input = b.input();
        let values = [
            input.sequence.to_string(),
            open_ns.to_string(),
            close_ns.to_string(),
            input.open.numeric_coefficient().expect("price").to_string(),
            input.high.numeric_coefficient().expect("price").to_string(),
            input.low.numeric_coefficient().expect("price").to_string(),
            input
                .close
                .numeric_coefficient()
                .expect("price")
                .to_string(),
            volume.coefficient().to_string(),
        ];
        hash.update(serde_json::to_vec(&values).expect("bounded exact row"));
        hash.update(b"\n");
        bars.push(b);
        if close_ns == spec.sessions[session].close_ns {
            session += 1;
            if session < spec.sessions.len() {
                expected_open = spec.sessions[session].open_ns
            }
        } else {
            expected_open = close_ns
        }
    }
    if bars.len() != spec.expected_rows || session != spec.sessions.len() {
        return Err(error(
            "history_rows",
            "$.csv",
            "missing expected session bars",
        ));
    }
    let bars_id = crate::model::ContentId::from_bytes(hash.finalize().into());
    let content = digest(
        b"FQE-HISTORY-DATASET/1\n",
        &serde_json::to_vec(&(
            spec.content_id().to_string(),
            spec.source_id().to_string(),
            source.to_string(),
            bars_id.to_string(),
        ))
        .expect("bounded identity"),
    );
    Ok(HistoricalDataset {
        spec,
        bars: bars.into(),
        source,
        bars_id,
        content,
    })
}
fn typed(v: fqe_core::numeric::Decimal, unit: NumericUnit, path: &str) -> Result<TypedValue> {
    let t = TypeDescriptor::new(
        1,
        ValueShape::Scalar,
        ScalarType::Decimal {
            scale: v.scale(),
            unit,
        },
    )
    .map_err(|_| error("history_type", path, "unsupported numeric unit/scale"))?;
    TypedValue::new(t, ScalarValue::Decimal(v))
        .map_err(|_| error("history_type", path, "typed value mismatch"))
}
