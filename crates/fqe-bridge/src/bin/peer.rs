//! One private stdio peer per host; at most one financial worker. EOF joins and exits.
use fqe_bridge::{
    Request, dispatch,
    framing::{read_frame, write_frame},
    managed::{Admission, ManagedRuns},
    response,
};
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
fn main() -> std::process::ExitCode {
    if std::env::args_os().len() != 1 {
        return std::process::ExitCode::from(2);
    }
    let output = Arc::new(Mutex::new(io::stdout()));
    let admission = Admission::default();
    let managed = ManagedRuns::new(admission.clone());
    let mut signals = fqe_bridge::research::live::Signals::default();
    let mut worker: Option<(String, Arc<AtomicBool>, thread::JoinHandle<()>)> = None;
    let input = io::stdin();
    let mut input = input.lock();
    let send = |bytes: &[u8]| {
        output
            .lock()
            .map_err(|_| io::Error::other("stdout lock"))
            .and_then(|mut w| write_frame(&mut *w, bytes))
    };
    let result = (|| -> io::Result<()> {
        while let Some(bytes) = read_frame(&mut input)? {
            if worker.as_ref().is_some_and(|(_, _, w)| w.is_finished()) {
                let (_, _, w) = worker.take().unwrap();
                let _ = w.join();
            }
            let r = match Request::parse(&bytes) {
                Ok(r) => r,
                Err(_) => {
                    send(&dispatch(&bytes, &AtomicBool::new(false)))?;
                    continue;
                }
            };
            if r.method == "hello" {
                let mut capabilities = fqe_bridge::CAPABILITIES.to_vec();
                capabilities.push("run.managed/1");
                capabilities.push("run.incremental/1");
                capabilities.push("signals.external-account/1");
                capabilities.push("signals.multisource/1");
                let result = fqe_bridge::negotiate(&r.params, &capabilities).map(|mut v| {
                    v.as_object_mut().expect("hello object").insert(
                        "session_id".into(),
                        fqe_ir::financial_exchange::string(managed.session_id()),
                    );
                    v
                });
                send(&response(Some(&r.id), result))?;
            } else if r.method.starts_with("signals.") {
                send(&response(
                    Some(&r.id),
                    signals.execute(&r.method, &r.params),
                ))?;
            } else if r.method.starts_with("run.") {
                send(&response(Some(&r.id), managed.execute(&r)))?;
            } else if r.method == "cancel" {
                let target = r.params.get("request_id").and_then(|v| v.as_str());
                let valid = r.params.as_object().is_some_and(|m| m.len() == 1) && target.is_some();
                if !valid {
                    send(&response(
                        Some(&r.id),
                        Err(fqe_bridge::Failure {
                            code: "cancel_shape",
                            path: "$.params".into(),
                        }),
                    ))?;
                    continue;
                }
                let accepted = valid
                    && worker.as_ref().is_some_and(|(id, flag, _)| {
                        if Some(id.as_str()) == target {
                            flag.store(true, Ordering::Release);
                            true
                        } else {
                            false
                        }
                    });
                let body = fqe_ir::financial_exchange::object([(
                    "accepted",
                    fqe_ir::financial_exchange::JsonValue::Bool(accepted),
                )]);
                send(&response(Some(&r.id), Ok(body)))?;
            } else if r.method == "backtest.run" {
                if worker.is_some() {
                    send(&response(
                        Some(&r.id),
                        Err(fqe_bridge::Failure {
                            code: "busy",
                            path: "$".into(),
                        }),
                    ))?;
                    continue;
                }
                let permit = match admission.acquire() {
                    Ok(permit) => permit,
                    Err(error) => {
                        send(&response(Some(&r.id), Err(error)))?;
                        continue;
                    }
                };
                let flag = Arc::new(AtomicBool::new(false));
                let thread_flag = flag.clone();
                let out = output.clone();
                let id = r.id.clone();
                let w = thread::spawn(move || {
                    let _permit = permit;
                    let reply = dispatch(&bytes, &thread_flag);
                    if let Ok(mut out) = out.lock() {
                        let _ = write_frame(&mut *out, &reply);
                    }
                });
                worker = Some((id, flag, w));
            } else {
                send(&dispatch(&bytes, &AtomicBool::new(false)))?;
            }
        }
        Ok(())
    })();
    managed.shutdown();
    if let Some((_, flag, w)) = worker {
        flag.store(true, Ordering::Release);
        let _ = w.join();
    }
    if result.is_ok() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(2)
    }
}
