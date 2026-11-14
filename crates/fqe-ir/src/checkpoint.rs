//! Complete bounded replay-state representation. Not a native provider-buffer serializer.
use crate::{
    exchange::{self, MarketRecord},
    graph::{digest, error},
    loading::{self, Obj, integer, mapped, text, typed_json, uint},
    model::{ContentId, ModelError, identifier},
    parameters::{BoundParameters, BoundValue},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, ModelError>;
pub const MAX_COMMANDS: usize = 1024;
pub const REPRESENTATION: &str = "bounded-deterministic-command-replay/1";
pub type SnapshotPins = BTreeMap<Vec<String>, String>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotCommand {
    Ingest {
        stream: String,
        record: MarketRecord,
    },
    Watermark {
        stream: String,
        timestamp_ns: u64,
    },
    Seal {
        stream: String,
    },
    Drive,
    Retry {
        instance: String,
    },
    Cancel {
        instance: String,
    },
    Finish {
        instance: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEntry {
    pub command: SnapshotCommand,
    pub outcome_id: ContentId,
    pub output_cursor: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotDocument {
    pub pins: SnapshotPins,
    pub entries: Vec<JournalEntry>,
}
pub fn observation_id(bytes: &[u8]) -> ContentId {
    digest(b"FQE-CHECKPOINT-OWNED-OUTCOME/1\n", bytes)
}
pub fn engine_identity(parts: &[(&str, &[u8])]) -> ContentId {
    let hashes = parts
        .iter()
        .map(|(name, bytes)| (*name, digest(b"", bytes).to_string()))
        .collect::<BTreeMap<_, _>>();
    digest(
        b"FQE-CHECKPOINT-ENGINE-SOURCES/1\n",
        &serde_json::to_vec(&hashes).expect("bounded engine sources"),
    )
}
pub fn parameters_id(parameters: &BoundParameters) -> ContentId {
    let values = parameters
        .iter()
        .map(|(key, v)| {
            (
                key,
                match v {
                    BoundValue::Value(v) => typed_json(v),
                    BoundValue::Unbound => Value::Null,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    digest(
        b"FQE-CHECKPOINT-PARAMETERS/1\n",
        &serde_json::to_vec(&values).expect("bounded parameters"),
    )
}
fn record_json(record: &MarketRecord) -> Value {
    match record {
        MarketRecord::Tick(e) => {
            json!({"kind":"tick","sequence":e.sequence().to_string(),"timestamp_ns":e.timestamp_ns().to_string(),"instrument":e.tick().instrument.as_str(),"price":e.tick().price.get().to_string()})
        }
        MarketRecord::Bar(bar) => {
            let b = bar.input();
            json!({"kind":"closed_bar","sequence":b.sequence.to_string(),"timestamp_ns":b.timestamp_ns.to_string(),"instrument":b.instrument.as_str(),"timeframe_ns":b.timeframe_ns.to_string(),"open_ns":b.open_ns.to_string(),"close_ns":b.close_ns.to_string(),"closed":b.closed,"open":typed_json(&b.open),"high":typed_json(&b.high),"low":typed_json(&b.low),"close":typed_json(&b.close),"volume":typed_json(&b.volume)})
        }
    }
}
fn command_json(command: &SnapshotCommand) -> Value {
    match command {
        SnapshotCommand::Ingest { stream, record } => {
            json!({"op":"ingest","stream":stream,"record":record_json(record)})
        }
        SnapshotCommand::Watermark {
            stream,
            timestamp_ns,
        } => json!({"op":"watermark","stream":stream,"timestamp_ns":timestamp_ns.to_string()}),
        SnapshotCommand::Seal { stream } => json!({"op":"seal","stream":stream}),
        SnapshotCommand::Drive => json!({"op":"drive"}),
        SnapshotCommand::Retry { instance } => json!({"op":"retry","instance":instance}),
        SnapshotCommand::Cancel { instance } => json!({"op":"cancel","instance":instance}),
        SnapshotCommand::Finish { instance } => json!({"op":"finish","instance":instance}),
    }
}
fn id(v: Value, p: &str) -> Result<String> {
    let v = text(v, p)?;
    identifier(&v, p)?;
    Ok(v)
}
fn command(v: Value, p: &str) -> Result<SnapshotCommand> {
    let mut o = Obj::new(v, p)?;
    let op = text(o.take("op")?, p)?;
    let command = match op.as_str() {
        "ingest" => SnapshotCommand::Ingest {
            stream: id(o.take("stream")?, p)?,
            record: exchange::record(o.take("record")?, p)?,
        },
        "watermark" => SnapshotCommand::Watermark {
            stream: id(o.take("stream")?, p)?,
            timestamp_ns: integer(o.take("timestamp_ns")?, p)?,
        },
        "seal" => SnapshotCommand::Seal {
            stream: id(o.take("stream")?, p)?,
        },
        "drive" => SnapshotCommand::Drive,
        "retry" => SnapshotCommand::Retry {
            instance: id(o.take("instance")?, p)?,
        },
        "cancel" => SnapshotCommand::Cancel {
            instance: id(o.take("instance")?, p)?,
        },
        "finish" => SnapshotCommand::Finish {
            instance: id(o.take("instance")?, p)?,
        },
        _ => return Err(error("checkpoint_command", p, "unsupported operation")),
    };
    o.end()?;
    Ok(command)
}
impl SnapshotDocument {
    pub fn output_cursor(&self) -> u64 {
        self.entries.last().map_or(0, |e| e.output_cursor)
    }
    fn validate(&self) -> Result<()> {
        if self.entries.len() > MAX_COMMANDS || self.pins.is_empty() || self.pins.len() > 1024 {
            return Err(error(
                "checkpoint_limit",
                "$",
                "commands<=1024 and pins1..1024 required",
            ));
        }
        for (path, value) in &self.pins {
            if path.is_empty() || path.len() > 32 || value.len() > 4096 {
                return Err(error(
                    "checkpoint_limit",
                    "$.pins",
                    "pin size exceeds bounds",
                ));
            }
            for part in path {
                identifier(part, "$.pins")?;
            }
        }
        let mut previous = 0_u64;
        for e in &self.entries {
            match &e.command {
                SnapshotCommand::Ingest { stream, .. }
                | SnapshotCommand::Watermark { stream, .. }
                | SnapshotCommand::Seal { stream } => identifier(stream, "$.journal.stream")?,
                SnapshotCommand::Retry { instance }
                | SnapshotCommand::Cancel { instance }
                | SnapshotCommand::Finish { instance } => {
                    identifier(instance, "$.journal.instance")?
                }
                SnapshotCommand::Drive => {}
            }
            if e.output_cursor < previous || e.output_cursor > previous + 1 {
                return Err(error(
                    "checkpoint_cursor",
                    "$.journal",
                    "output cursor must advance by zero or one",
                ));
            }
            previous = e.output_cursor;
        }
        Ok(())
    }
    fn body(&self) -> Value {
        json!({"format":"fqe.checkpoint","schema_version":1,"state_representation":REPRESENTATION,"materialized_state":false,
        "pins":self.pins.iter().map(|(path,value)|json!({"path":path,"value":value})).collect::<Vec<_>>(),
        "journal":self.entries.iter().map(|e|json!({"command":command_json(&e.command),"outcome_id":e.outcome_id.to_string(),"output_cursor":e.output_cursor.to_string()})).collect::<Vec<_>>(),"output_cursor":self.output_cursor().to_string()})
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut body = self.body();
        let canonical = serde_json::to_vec(&body)
            .map_err(|_| error("checkpoint_encoding", "$", "encoding failed"))?;
        body["checksum"] = json!(digest(b"FQE-CHECKPOINT-CONTENT/1\n", &canonical).to_string());
        let bytes = serde_json::to_vec(&body)
            .map_err(|_| error("checkpoint_encoding", "$", "encoding failed"))?;
        // Enforce the same byte/depth/item/collection budgets used by the reader before mutation.
        loading::decode(&bytes)?;
        Ok(bytes)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut o = Obj::new(loading::decode(bytes)?, "$")?;
        if text(o.take("format")?, "$")? != "fqe.checkpoint"
            || uint(o.take("schema_version")?, "$")? != 1
            || text(o.take("state_representation")?, "$")? != REPRESENTATION
        {
            return Err(error(
                "checkpoint_version",
                "$",
                "unsupported checkpoint representation/version",
            ));
        }
        if o.take("materialized_state")? != json!(false) {
            return Err(error(
                "checkpoint_representation",
                "$",
                "native buffer import is unsupported",
            ));
        }
        let pin_entries = mapped(o.take("pins")?, "$.pins", |v, p| {
            let mut o = Obj::new(v, p)?;
            let path = mapped(o.take("path")?, p, id)?;
            let value = text(o.take("value")?, p)?;
            o.end()?;
            Ok((path, value))
        })?;
        let mut pins = BTreeMap::new();
        for (path, value) in pin_entries {
            if pins.insert(path, value).is_some() {
                return Err(error(
                    "checkpoint_duplicate",
                    "$.pins",
                    "duplicate pin path",
                ));
            }
        }
        let entries = mapped(o.take("journal")?, "$.journal", |v, p| {
            let mut o = Obj::new(v, p)?;
            let command = command(o.take("command")?, p)?;
            let outcome_id = ContentId::parse(&text(o.take("outcome_id")?, p)?)?;
            let output_cursor = integer(o.take("output_cursor")?, p)?;
            o.end()?;
            Ok(JournalEntry {
                command,
                outcome_id,
                output_cursor,
            })
        })?;
        let cursor: u64 = integer(o.take("output_cursor")?, "$.output_cursor")?;
        let checksum = ContentId::parse(&text(o.take("checksum")?, "$.checksum")?)?;
        o.end()?;
        let result = Self { pins, entries };
        result.validate()?;
        if cursor != result.output_cursor() {
            return Err(error(
                "checkpoint_cursor",
                "$.output_cursor",
                "header and journal cursor differ",
            ));
        }
        let canonical = serde_json::to_vec(&result.body())
            .map_err(|_| error("checkpoint_encoding", "$", "encoding failed"))?;
        if checksum != digest(b"FQE-CHECKPOINT-CONTENT/1\n", &canonical) {
            return Err(error(
                "checkpoint_integrity",
                "$.checksum",
                "checkpoint checksum differs",
            ));
        }
        Ok(result)
    }
}
