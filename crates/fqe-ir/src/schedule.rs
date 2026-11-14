//! Bounded fixed-UTC stream admission and stable scheduler identity; no event evaluation.
use crate::{
    event_plan::{self, Trigger},
    graph::{digest, error},
    loading::{self, Obj, integer, mapped, text, uint},
    model::{ContentId, ModelError, identifier},
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, ModelError>;
pub const MAX_STREAMS: usize = 64;
pub const MAX_QUEUE: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapPolicy {
    NotApplicable,
    Reject,
    Report,
}
impl GapPolicy {
    fn name(self) -> &'static str {
        match self {
            Self::NotApplicable => "not-applicable",
            Self::Reject => "reject",
            Self::Report => "report",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamDefinition {
    id: String,
    dataset: ContentId,
    domain: Trigger,
    gaps: GapPolicy,
}
impl StreamDefinition {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn dataset_id(&self) -> ContentId {
        self.dataset
    }
    pub fn domain(&self) -> &Trigger {
        &self.domain
    }
    pub fn gaps(&self) -> GapPolicy {
        self.gaps
    }
    /// Price/volume scale cannot create a second feed for the same logical domain.
    pub fn order_key(&self) -> (String, u8, u64) {
        match &self.domain {
            Trigger::Tick { instrument } => (instrument.to_string(), 0, 0),
            Trigger::ClosedBar {
                instrument,
                timeframe_ns,
                ..
            } => (instrument.to_string(), 1, *timeframe_ns),
        }
    }
}
pub struct ScheduleDefinition {
    streams: BTreeMap<String, StreamDefinition>,
    queue_capacity: usize,
    source: ContentId,
    content: ContentId,
    canonical: Vec<u8>,
}
impl ScheduleDefinition {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut o = Obj::new(loading::decode(bytes)?, "$")?;
        if text(o.take("format")?, "$")? != "fqe.schedule"
            || uint(o.take("schema_version")?, "$")? != 1
        {
            return Err(error("schedule_version", "$", "fqe.schedule/1 required"));
        }
        let id = text(o.take("id")?, "$.id")?;
        identifier(&id, "$.id")?;
        let version = text(o.take("version")?, "$.version")?;
        identifier(&version, "$.version")?;
        let requirements = mapped(o.take("requirements")?, "$.requirements", text)?;
        if requirements != ["market.schedule/1"] {
            return Err(error(
                "schedule_capability",
                "$.requirements",
                "exactly market.schedule/1 required; other capabilities unsupported",
            ));
        }
        if text(o.take("calendar")?, "$.calendar")? != "utc-epoch-fixed/1" {
            return Err(error(
                "schedule_calendar",
                "$.calendar",
                "only fixed UTC epoch alignment supported",
            ));
        }
        let cap: u64 = integer(o.take("queue_capacity")?, "$.queue_capacity")?;
        if cap == 0 || cap > MAX_QUEUE as u64 {
            return Err(error(
                "schedule_limit",
                "$.queue_capacity",
                "queue capacity must be1..4096",
            ));
        }
        let declarations = mapped(o.take("streams")?, "$.streams", |v, p| {
            let mut o = Obj::new(v, p)?;
            let id = text(o.take("id")?, p)?;
            identifier(&id, p)?;
            let dataset = ContentId::parse(&text(o.take("dataset_id")?, p)?)?;
            let domain = event_plan::trigger(o.take("domain")?, p)?;
            let gaps = match text(o.take("gap_policy")?, p)?.as_str() {
                "not-applicable" if matches!(domain, Trigger::Tick { .. }) => {
                    GapPolicy::NotApplicable
                }
                "reject" if matches!(domain, Trigger::ClosedBar { .. }) => GapPolicy::Reject,
                "report" if matches!(domain, Trigger::ClosedBar { .. }) => GapPolicy::Report,
                _ => {
                    return Err(error(
                        "schedule_gap_policy",
                        p,
                        "unsupported or inapplicable gap policy",
                    ));
                }
            };
            o.end()?;
            Ok(StreamDefinition {
                id,
                dataset,
                domain,
                gaps,
            })
        })?;
        o.end()?;
        if declarations.is_empty() || declarations.len() > MAX_STREAMS {
            return Err(error(
                "schedule_limit",
                "$.streams",
                "one to64 streams required",
            ));
        }
        let mut streams = BTreeMap::new();
        let mut domains = BTreeSet::new();
        for stream in declarations {
            if !domains.insert(stream.order_key())
                || streams.insert(stream.id.clone(), stream).is_some()
            {
                return Err(error(
                    "schedule_duplicate",
                    "$.streams",
                    "duplicate stream ID or logical domain",
                ));
            }
        }
        let canonical=serde_json::to_vec(&json!({"format":"fqe.schedule","schema_version":1,"id":id,"version":version,"requirements":["market.schedule/1"],"calendar":"utc-epoch-fixed/1","queue_capacity":cap.to_string(),"streams":streams.values().map(|s|json!({"id":s.id,"dataset_id":s.dataset.to_string(),"domain":event_plan::trigger_json(&s.domain),"gap_policy":s.gaps.name()})).collect::<Vec<_>>()})).map_err(|_|error("schedule_encoding","$","canonical encoding failed"))?;
        Ok(Self {
            streams,
            queue_capacity: cap as usize,
            source: digest(b"", bytes),
            content: digest(b"FQE-SCHEDULE-CANONICAL/1\n", &canonical),
            canonical,
        })
    }
    pub fn streams(&self) -> &BTreeMap<String, StreamDefinition> {
        &self.streams
    }
    pub fn queue_capacity(&self) -> usize {
        self.queue_capacity
    }
    pub fn source_id(&self) -> ContentId {
        self.source
    }
    pub fn content_id(&self) -> ContentId {
        self.content
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
}
