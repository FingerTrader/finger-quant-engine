//! Process-session owned jobs. No UI/window owns computation; no hidden re-execution on reads.
use crate::{Failure, Request, Result, count, fail, fields, prepare, text, unsigned};
use fqe_ir::financial_exchange::{self as json, JsonValue as V, object as o, string as s};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};
pub const MAX_JOBS: usize = 4;
pub const MAX_OPERATIONS: usize = 64;
#[derive(Clone, Default)]
pub struct Admission(Arc<AtomicBool>);
pub struct Permit(Arc<AtomicBool>);
impl Admission {
    pub fn acquire(&self) -> Result<Permit> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| fail("busy"))?;
        Ok(Permit(self.0.clone()))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Accepted,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}
impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Running => "running",
            Self::Cancelling => "cancelling",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
    fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}
struct Stored {
    bytes: Arc<[u8]>,
    result_id: String,
    manifest_id: String,
    processed: String,
    phase: Phase,
}
struct Job {
    operation: String,
    phase: Phase,
    cancel: Arc<AtomicBool>,
    events: Vec<V>,
    result: Option<Stored>,
    error: Option<Failure>,
    worker: Option<JoinHandle<()>>,
}
impl Job {
    fn event(&mut self) {
        self.events.push(o([
            ("cursor", s(self.events.len() + 1)),
            ("phase", s(self.phase.name())),
            (
                "processed_source_bars",
                self.result.as_ref().map_or(V::Null, |r| s(&r.processed)),
            ),
        ]));
    }
    fn status(&self, handle: &str) -> V {
        let error = self
            .error
            .as_ref()
            .map_or(V::Null, |e| o([("code", s(e.code)), ("path", s(&e.path))]));
        o([
            ("handle", s(handle)),
            ("operation_id", s(&self.operation)),
            ("phase", s(self.phase.name())),
            ("terminal", V::Bool(self.phase.terminal())),
            (
                "stop_requested",
                V::Bool(self.cancel.load(Ordering::Acquire)),
            ),
            ("high_watermark", s(self.events.len())),
            (
                "processed_source_bars",
                self.result.as_ref().map_or(V::Null, |r| s(&r.processed)),
            ),
            (
                "result_id",
                self.result.as_ref().map_or(V::Null, |r| s(&r.result_id)),
            ),
            ("error", error),
        ])
    }
}
#[derive(Default)]
struct Inner {
    initialized: bool,
    jobs: BTreeMap<String, Job>,
    operations: BTreeMap<String, (String, String)>,
}
pub struct ManagedRuns {
    session: String,
    inner: Arc<Mutex<Inner>>,
    admission: Admission,
    closed: AtomicBool,
}
impl ManagedRuns {
    pub fn new(admission: Admission) -> Self {
        let stamp = format!(
            "{}:{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("post-epoch system clock")
                .as_nanos()
        );
        // Process-instance identity only; not a cryptographic authentication secret.
        let session = json::content_id(b"FQE-DESKTOP-SESSION/1\n", stamp.as_bytes()).to_string();
        Self {
            session,
            inner: Arc::new(Mutex::new(Inner::default())),
            admission,
            closed: AtomicBool::new(false),
        }
    }
    pub fn session_id(&self) -> &str {
        &self.session
    }
    pub fn execute(&self, r: &Request) -> Result<V> {
        if self.closed.load(Ordering::Acquire) {
            return Err(fail("session_closed"));
        }
        let p = &r.params;
        if r.method == "run.initialize" {
            fields(p, &[])?;
            let mut inner = self.inner.lock().map_err(|_| fail("manager_lock"))?;
            if self.closed.load(Ordering::Acquire) {
                return Err(fail("session_closed"));
            }
            inner.initialized = true;
            return Ok(o([
                ("session_id", s(&self.session)),
                ("max_jobs", s(MAX_JOBS)),
                ("max_operations", s(MAX_OPERATIONS)),
                ("max_active", s(1)),
                ("progress", s("indeterminate-batch/1")),
                ("event_delivery", s("pull-cursor/1")),
            ]));
        }
        if text(p, "session_id")? != self.session {
            return Err(fail("session_mismatch"));
        }
        if !self
            .inner
            .lock()
            .map_err(|_| fail("manager_lock"))?
            .initialized
        {
            return Err(fail("not_initialized"));
        }
        match r.method.as_str() {
            "run.start" => {
                fields(p, &["session_id", "operation_id", "input"])?;
                self.start(text(p, "operation_id")?, &p["input"])
            }
            "run.status" => {
                fields(p, &["session_id", "handle"])?;
                let handle = text(p, "handle")?;
                let inner = self.inner.lock().map_err(|_| fail("manager_lock"))?;
                Ok(inner
                    .jobs
                    .get(handle)
                    .ok_or_else(|| fail("run_unavailable"))?
                    .status(handle))
            }
            "run.stop" => {
                fields(p, &["session_id", "handle"])?;
                let handle = text(p, "handle")?;
                let mut inner = self.inner.lock().map_err(|_| fail("manager_lock"))?;
                let job = inner
                    .jobs
                    .get_mut(handle)
                    .ok_or_else(|| fail("run_unavailable"))?;
                if !job.phase.terminal() {
                    job.cancel.store(true, Ordering::Release);
                    if job.phase != Phase::Cancelling {
                        job.phase = Phase::Cancelling;
                        job.event();
                    }
                }
                Ok(job.status(handle))
            }
            "run.events" => {
                fields(p, &["session_id", "handle", "after", "limit"])?;
                let after = usize::try_from(unsigned(p, "after")?).map_err(|_| fail("cursor"))?;
                let limit = count(p, "limit")? as usize;
                if limit == 0 || limit > 16 {
                    return Err(fail("page_quota"));
                }
                let inner = self.inner.lock().map_err(|_| fail("manager_lock"))?;
                let job = inner
                    .jobs
                    .get(text(p, "handle")?)
                    .ok_or_else(|| fail("run_unavailable"))?;
                if after > job.events.len() {
                    return Err(fail("cursor"));
                }
                let end = (after + limit).min(job.events.len());
                Ok(o([
                    ("session_id", s(&self.session)),
                    ("handle", s(text(p, "handle")?)),
                    ("events", V::Array(job.events[after..end].to_vec())),
                    ("next_cursor", s(end)),
                    ("high_watermark", s(job.events.len())),
                    ("terminal", V::Bool(job.phase.terminal())),
                ]))
            }
            "run.result" => {
                fields(p, &["session_id", "handle"])?;
                let (bytes, id, manifest) = self.result(text(p, "handle")?)?;
                let value = serde_json::from_slice(&bytes).map_err(|_| fail("native_report"))?;
                Ok(o([
                    ("result_id", s(id)),
                    ("manifest_id", s(manifest)),
                    ("report", value),
                ]))
            }
            "run.result_chunk" => {
                fields(
                    p,
                    &["session_id", "handle", "result_id", "offset", "max_bytes"],
                )?;
                let (bytes, id, manifest) = self.result(text(p, "handle")?)?;
                if text(p, "result_id")? != id {
                    return Err(fail("result_identity"));
                }
                let offset = usize::try_from(unsigned(p, "offset")?).map_err(|_| fail("cursor"))?;
                let max = count(p, "max_bytes")? as usize;
                if offset > bytes.len() {
                    return Err(fail("cursor"));
                }
                if max == 0 || max > 65536 {
                    return Err(fail("page_quota"));
                }
                let end = (offset + max).min(bytes.len());
                let mut hex = String::with_capacity((end - offset) * 2);
                const DIGITS: &[u8; 16] = b"0123456789abcdef";
                for b in &bytes[offset..end] {
                    hex.push(DIGITS[(b >> 4) as usize] as char);
                    hex.push(DIGITS[(b & 15) as usize] as char);
                }
                Ok(o([
                    ("result_id", s(id)),
                    ("manifest_id", s(manifest)),
                    ("encoding", s("hex")),
                    ("offset", s(offset)),
                    ("next_offset", s(end)),
                    ("total_bytes", s(bytes.len())),
                    ("eof", V::Bool(end == bytes.len())),
                    ("data", s(hex)),
                ]))
            }
            "run.output" => {
                fields(
                    p,
                    &[
                        "session_id",
                        "handle",
                        "result_id",
                        "section",
                        "after",
                        "limit",
                    ],
                )?;
                let (bytes, id, _) = self.result(text(p, "handle")?)?;
                if text(p, "result_id")? != id {
                    return Err(fail("result_identity"));
                }
                let section = text(p, "section")?;
                if ![
                    "observations",
                    "decisions",
                    "orders",
                    "fills",
                    "journal",
                    "execution_events",
                    "equity_curve",
                ]
                .contains(&section)
                {
                    return Err(fail("output_section"));
                }
                let after = usize::try_from(unsigned(p, "after")?).map_err(|_| fail("cursor"))?;
                let limit = count(p, "limit")? as usize;
                if limit == 0 || limit > 16 {
                    return Err(fail("page_quota"));
                }
                let value: V = serde_json::from_slice(&bytes).map_err(|_| fail("native_report"))?;
                let rows = value[section]
                    .as_array()
                    .ok_or_else(|| fail("native_report"))?;
                if after > rows.len() {
                    return Err(fail("cursor"));
                }
                let end = (after + limit).min(rows.len());
                Ok(o([
                    ("result_id", s(id)),
                    ("section", s(section)),
                    ("rows", V::Array(rows[after..end].to_vec())),
                    ("next_cursor", s(end)),
                    ("total_rows", s(rows.len())),
                    ("eof", V::Bool(end == rows.len())),
                ]))
            }
            "run.release" => {
                fields(p, &["session_id", "handle"])?;
                let handle = text(p, "handle")?;
                let mut inner = self.inner.lock().map_err(|_| fail("manager_lock"))?;
                if let Some(job) = inner.jobs.get(handle) {
                    if !job.phase.terminal() {
                        return Err(fail("run_active"));
                    }
                } else if !inner.operations.values().any(|(_, h)| h == handle) {
                    return Err(fail("run_unavailable"));
                }
                inner.jobs.remove(handle);
                Ok(o([("handle", s(handle)), ("released", V::Bool(true))]))
            }
            _ => Err(fail("unsupported_method")),
        }
    }
    fn result(&self, handle: &str) -> Result<(Arc<[u8]>, String, String)> {
        let inner = self.inner.lock().map_err(|_| fail("manager_lock"))?;
        let job = inner
            .jobs
            .get(handle)
            .ok_or_else(|| fail("run_unavailable"))?;
        let r = job.result.as_ref().ok_or_else(|| {
            fail(if job.phase == Phase::Failed {
                "run_failed"
            } else {
                "result_not_ready"
            })
        })?;
        Ok((r.bytes.clone(), r.result_id.clone(), r.manifest_id.clone()))
    }
    fn start(&self, operation: &str, input: &V) -> Result<V> {
        if operation.is_empty()
            || operation.len() > 64
            || !operation
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(fail("operation_id"));
        }
        let canonical = json::encode(input, crate::MAX_REQUEST)?;
        let fingerprint = json::content_id(b"FQE-DESKTOP-OPERATION/1\n", &canonical).to_string();
        let mut inner = self.inner.lock().map_err(|_| fail("manager_lock"))?;
        // Shutdown may win after execute's initial check while this request waits for the lock.
        if self.closed.load(Ordering::Acquire) {
            return Err(fail("session_closed"));
        }
        if let Some((old, handle)) = inner.operations.get(operation) {
            if old != &fingerprint {
                return Err(fail("operation_conflict"));
            }
            return Ok(o([
                ("session_id", s(&self.session)),
                ("handle", s(handle)),
                ("duplicate", V::Bool(true)),
                ("released", V::Bool(!inner.jobs.contains_key(handle))),
            ]));
        }
        if inner.operations.len() >= MAX_OPERATIONS {
            return Err(fail("operation_quota"));
        }
        if inner.jobs.len() >= MAX_JOBS {
            return Err(fail("run_quota"));
        }
        let permit = self.admission.acquire()?;
        let handle = format!("managed.{}", inner.operations.len() + 1);
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let mut job = Job {
            operation: operation.into(),
            phase: Phase::Accepted,
            cancel,
            events: vec![],
            result: None,
            error: None,
            worker: None,
        };
        job.event();
        inner.jobs.insert(handle.clone(), job);
        inner
            .operations
            .insert(operation.into(), (fingerprint, handle.clone()));
        let jobs = self.inner.clone();
        let worker_handle = handle.clone();
        let input = input.clone();
        let worker = thread::Builder::new()
            .name("fqe-managed-run".into())
            .spawn(move || {
                let _permit = permit;
                if let Ok(mut inner) = jobs.lock()
                    && let Some(job) = inner.jobs.get_mut(&worker_handle)
                    && job.phase == Phase::Accepted
                {
                    job.phase = Phase::Running;
                    job.event();
                }
                let outcome =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Stored> {
                        let prepared = prepare(&input)?;
                        let result = prepared.run_with_cancel(&flag)?;
                        let phase = match result.terminal() {
                            fqe_runtime::backtest::BacktestTerminal::Completed => Phase::Completed,
                            fqe_runtime::backtest::BacktestTerminal::Cancelled => Phase::Cancelled,
                            _ => return Err(fail("native_terminal")),
                        };
                        Ok(Stored {
                            phase,
                            bytes: Arc::from(result.json()),
                            result_id: result.content_id().to_string(),
                            manifest_id: result.manifest_id().to_string(),
                            processed: result.observations().len().to_string(),
                        })
                    }))
                    .unwrap_or_else(|_| Err(fail("engine_panic")));
                if let Ok(mut inner) = jobs.lock()
                    && let Some(job) = inner.jobs.get_mut(&worker_handle)
                {
                    match outcome {
                        Ok(result) => {
                            job.phase = result.phase;
                            job.result = Some(result);
                        }
                        Err(e) => {
                            job.phase = Phase::Failed;
                            job.error = Some(e);
                        }
                    }
                    job.event();
                    // Publish terminal state and release admission under the same registry lock.
                    drop(_permit);
                }
            });
        match worker {
            Ok(worker) => inner.jobs.get_mut(&handle).expect("accepted job").worker = Some(worker),
            Err(_) => {
                inner.jobs.remove(&handle);
                inner.operations.remove(operation);
                return Err(fail("worker_unavailable"));
            }
        }
        Ok(o([
            ("session_id", s(&self.session)),
            ("handle", s(handle)),
            ("duplicate", V::Bool(false)),
            ("released", V::Bool(false)),
        ]))
    }
    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        let workers = if let Ok(mut inner) = self.inner.lock() {
            inner
                .jobs
                .values_mut()
                .filter_map(|j| {
                    j.cancel.store(true, Ordering::Release);
                    j.worker.take()
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        for worker in workers {
            let _ = worker.join();
        }
    }
}
impl Drop for ManagedRuns {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_session_cannot_admit_an_already_dispatched_start() {
        let admission = Admission::default();
        let manager = ManagedRuns::new(admission.clone());
        manager.closed.store(true, Ordering::Release);
        assert_eq!(
            manager
                .start("closing.operation", &o([] as [(&str, V); 0]))
                .unwrap_err()
                .code,
            "session_closed"
        );
        assert!(manager.inner.lock().unwrap().operations.is_empty());
        assert!(admission.acquire().is_ok());
    }
}
