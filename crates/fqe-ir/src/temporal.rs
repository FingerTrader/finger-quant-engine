//! Versioned temporal guards bind Boolean workflow outputs; clock/calendar belongs to the host.
use crate::{
    event_plan::Trigger,
    graph::{digest, error},
    loading::{self, Obj, integer, mapped, text, uint},
    model::{ContentId, ModelError, identifier},
    workflow::{self, CompiledWorkflow, Source},
};
use serde_json::json;
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, ModelError>;
pub const MAX_WINDOWS: usize = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowSpan {
    ClosedBars(u64),
    EventTime(u64),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowRule {
    pub id: String,
    pub arm: Source,
    pub predicate: Source,
    pub span: WindowSpan,
}
pub struct LoadedTemporalRules {
    rules: Vec<WindowRule>,
    canonical: Vec<u8>,
    source: ContentId,
    artifact: ContentId,
}
pub struct BoundTemporalRules {
    rules: Vec<WindowRule>,
    workflow: ContentId,
    execution: ContentId,
    artifact: ContentId,
    source: ContentId,
}
impl LoadedTemporalRules {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut o = Obj::new(loading::decode(bytes)?, "$")?;
        if text(o.take("format")?, "$")? != "fqe.temporal-rules"
            || uint(o.take("schema_version")?, "$")? != 1
        {
            return Err(error(
                "temporal_version",
                "$",
                "fqe.temporal-rules/1 required",
            ));
        }
        let id = text(o.take("id")?, "$.id")?;
        identifier(&id, "$.id")?;
        let version = text(o.take("version")?, "$.version")?;
        identifier(&version, "$.version")?;
        if mapped(o.take("requirements")?, "$.requirements", text)? != ["temporal.windows/1"] {
            return Err(error(
                "temporal_capability",
                "$.requirements",
                "exactly temporal.windows/1 required",
            ));
        }
        let rules = mapped(o.take("windows")?, "$.windows", |v, p| {
            let mut o = Obj::new(v, p)?;
            let id = text(o.take("id")?, p)?;
            identifier(&id, p)?;
            let arm = workflow::source(o.take("arm")?, p)?;
            let predicate = workflow::source(o.take("predicate")?, p)?;
            let mut span = Obj::new(o.take("span")?, p)?;
            let kind = text(span.take("kind")?, p)?;
            let span_value = match kind.as_str() {
                "closed-bar-count" => {
                    let bars: u64 = integer(span.take("bars")?, p)?;
                    if bars == 0 || bars > 1_000_000 {
                        return Err(error("temporal_limit", p, "bar count must be1..1000000"));
                    }
                    WindowSpan::ClosedBars(bars)
                }
                "event-time" => {
                    let duration: u64 = integer(span.take("duration_ns")?, p)?;
                    if duration == 0 {
                        return Err(error("temporal_limit", p, "duration must be positive"));
                    }
                    WindowSpan::EventTime(duration)
                }
                _ => return Err(error("temporal_span", p, "unsupported window/calendar")),
            };
            span.end()?;
            o.end()?;
            Ok(WindowRule {
                id,
                arm,
                predicate,
                span: span_value,
            })
        })?;
        o.end()?;
        if rules.len() > MAX_WINDOWS {
            return Err(error("temporal_limit", "$.windows", "at most16 windows"));
        }
        let mut sorted = BTreeMap::new();
        for rule in rules {
            if sorted.insert(rule.id.clone(), rule).is_some() {
                return Err(error(
                    "temporal_duplicate",
                    "$.windows",
                    "duplicate window ID",
                ));
            }
        }
        let rules: Vec<_> = sorted.into_values().collect();
        let canonical=serde_json::to_vec(&json!({"format":"fqe.temporal-rules","schema_version":1,"id":id,"version":version,"requirements":["temporal.windows/1"],"windows":rules.iter().map(|r|json!({"id":r.id,"arm":workflow::source_json(&r.arm),"predicate":workflow::source_json(&r.predicate),"span":match r.span{WindowSpan::ClosedBars(n)=>json!({"kind":"closed-bar-count","bars":n.to_string()}),WindowSpan::EventTime(n)=>json!({"kind":"event-time","duration_ns":n.to_string()})}})).collect::<Vec<_>>()})).map_err(|_|error("temporal_encoding","$","canonical encoding failed"))?;
        Ok(Self {
            rules,
            artifact: digest(b"FQE-TEMPORAL-CANONICAL/1\n", &canonical),
            source: digest(b"", bytes),
            canonical,
        })
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn content_id(&self) -> ContentId {
        self.artifact
    }
    pub fn source_id(&self) -> ContentId {
        self.source
    }
    pub fn bind(&self, workflow: &CompiledWorkflow) -> Result<BoundTemporalRules> {
        for r in &self.rules {
            if matches!(r.span, WindowSpan::ClosedBars(_))
                && !matches!(workflow.event().trigger(), Trigger::ClosedBar { .. })
            {
                return Err(error(
                    "temporal_domain",
                    "$.windows",
                    "closed-bar count requires a closed-bar trigger",
                ));
            }
            for source in [&r.arm, &r.predicate] {
                let typ = match source {
                    Source::Event(n) => workflow
                        .event()
                        .roots()
                        .output_types()
                        .iter()
                        .find(|p| &p.name == n)
                        .map(|p| &p.value_type),
                    Source::Instance { instance, output } => workflow
                        .instances()
                        .iter()
                        .find(|i| &i.id == instance)
                        .and_then(|i| i.outputs.output_types().iter().find(|p| &p.name == output))
                        .map(|p| &p.value_type),
                };
                if typ != Some(&workflow::boolean_type()) {
                    return Err(error(
                        "temporal_type",
                        "$.windows",
                        "arm/predicate must reference an exact Boolean workflow output",
                    ));
                }
            }
        }
        Ok(BoundTemporalRules {
            rules: self.rules.clone(),
            workflow: workflow.content_id(),
            execution: digest(
                b"FQE-TEMPORAL-EXECUTION/1\n",
                format!("{}\n{}", self.artifact, workflow.content_id()).as_bytes(),
            ),
            artifact: self.artifact,
            source: self.source,
        })
    }
}
impl BoundTemporalRules {
    pub fn rules(&self) -> &[WindowRule] {
        &self.rules
    }
    pub fn workflow_id(&self) -> ContentId {
        self.workflow
    }
    pub fn content_id(&self) -> ContentId {
        self.execution
    }
    pub fn artifact_id(&self) -> ContentId {
        self.artifact
    }
    pub fn source_id(&self) -> ContentId {
        self.source
    }
}
