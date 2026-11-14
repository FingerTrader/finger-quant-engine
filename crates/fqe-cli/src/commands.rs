//! CLI orchestration only; strategy decisions belong to the compiler/event runtime.
use fqe_ir::{
    catalog,
    event_plan::LoadedEventPlan,
    exchange::{self, MarketData, ParameterBindings},
    model::ModelError,
};
use fqe_runtime::report::run_report;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
};
struct Failure {
    exit: u8,
    bytes: Vec<u8>,
}
type Result<T> = std::result::Result<T, Failure>;
fn failure(exit: u8, stage: &str, code: &str, path: &str, message: &str) -> Failure {
    Failure {
        exit,
        bytes: exchange::diagnostic(stage, code, path, message),
    }
}
fn model_failure(stage: &str, message: &str, e: ModelError) -> Failure {
    failure(3, stage, e.code, &e.path, message)
}
struct Options {
    run: bool,
    model: PathBuf,
    params: Option<PathBuf>,
    data: Option<PathBuf>,
    run_id: String,
}
impl Options {
    fn parse(args: &[OsString]) -> Result<Self> {
        let run = args.first().is_some_and(|a| a == "run");
        let bad = || {
            failure(
                2,
                "arguments",
                "arguments",
                "$",
                "invalid options; see fqe --help",
            )
        };
        if args.is_empty() || args.len() > 9 || !(args.len() - 1).is_multiple_of(2) {
            return Err(bad());
        }
        let mut options = BTreeMap::new();
        for pair in args[1..].as_chunks::<2>().0 {
            let key = pair[0].to_str().ok_or_else(bad)?;
            if !["--model", "--params"].contains(&key)
                && !(run && ["--data", "--run-id"].contains(&key))
            {
                return Err(bad());
            }
            if pair[1].is_empty()
                || pair[1].to_str().is_some_and(|s| s.starts_with("--"))
                || options.insert(key, pair[1].clone()).is_some()
            {
                return Err(bad());
            }
        }
        let model = PathBuf::from(options.remove("--model").ok_or_else(bad)?);
        let params = options.remove("--params").map(PathBuf::from);
        let data = options.remove("--data").map(PathBuf::from);
        if run && data.is_none() {
            return Err(bad());
        }
        let run_id = options
            .remove("--run-id")
            .unwrap_or_else(|| OsString::from("cli"))
            .into_string()
            .map_err(|_| bad())?;
        if run_id.is_empty()
            || run_id.len() > 128
            || !run_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        {
            return Err(bad());
        }
        Ok(Self {
            run,
            model,
            params,
            data,
            run_id,
        })
    }
}
fn read(path: &Path, stage: &str) -> Result<Vec<u8>> {
    let io_error = || {
        failure(
            3,
            stage,
            "file_io",
            "$",
            "cannot read the requested regular file",
        )
    };
    let limit_error = || failure(3, stage, "file_limit", "$", "file exceeds 262144 bytes");
    let metadata = fs::metadata(path).map_err(|_| io_error())?;
    if !metadata.is_file() {
        return Err(io_error());
    }
    if metadata.len() > exchange::MAX_FILE_BYTES as u64 {
        return Err(limit_error());
    }
    let file = File::open(path).map_err(|_| io_error())?;
    if !file.metadata().map_err(|_| io_error())?.is_file() {
        return Err(io_error());
    }
    let mut bytes = Vec::new();
    file.take(exchange::MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io_error())?;
    if bytes.len() > exchange::MAX_FILE_BYTES {
        return Err(limit_error());
    }
    Ok(bytes)
}
fn execute(args: &[OsString]) -> Result<Vec<u8>> {
    let opts = Options::parse(args)?;
    let bytes = read(&opts.model, "model_io")?;
    let artifact = LoadedEventPlan::from_bytes(&bytes)
        .map_err(|e| model_failure("model", "model could not be loaded", e))?;
    let (values, parameters_source_id) = if let Some(path) = &opts.params {
        let bytes = read(path, "parameters_io")?;
        let bindings = ParameterBindings::from_bytes(&bytes)
            .map_err(|e| model_failure("parameters", "parameter file is invalid", e))?;
        let id = bindings.source_id();
        (bindings.into_values(), Some(id))
    } else {
        (vec![], None)
    };
    let registry = catalog::reference(artifact.trigger())
        .map_err(|e| model_failure("catalog", "reference catalog could not be prepared", e))?;
    let plan = artifact.compile(&registry, values).map_err(|e| {
        model_failure(
            "compile",
            "model or parameter bindings could not be compiled",
            e,
        )
    })?;
    if !opts.run {
        return exchange::validation_result(&plan, parameters_source_id).map_err(|e| {
            failure(
                5,
                "result",
                e.code,
                &e.path,
                "validation result could not be encoded",
            )
        });
    }
    let data = MarketData::from_bytes(&read(
        opts.data.as_ref().expect("run requires data"),
        "data_io",
    )?)
    .map_err(|e| model_failure("data", "market data file is invalid", e))?;
    run_report(Arc::new(plan), &opts.run_id, &data, parameters_source_id).map_err(|e| {
        let report = e.code.starts_with("report_");
        failure(
            if report { 5 } else { 4 },
            if report { "result" } else { "execution" },
            e.code,
            &e.path,
            if report {
                "result could not be encoded within limits"
            } else {
                "event was rejected; no complete run result was produced"
            },
        )
    })
}
fn stderr(bytes: &[u8]) {
    let mut out = io::stderr().lock();
    let _ = out.write_all(bytes).and_then(|()| out.write_all(b"\n"));
}
pub fn dispatch(args: &[OsString]) -> ExitCode {
    match execute(args) {
        Ok(bytes) => {
            let mut out = io::stdout().lock();
            if out
                .write_all(&bytes)
                .and_then(|()| out.write_all(b"\n"))
                .is_err()
            {
                stderr(&exchange::diagnostic(
                    "output",
                    "output_io",
                    "$",
                    "cannot write complete result",
                ));
                return ExitCode::from(5);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            stderr(&e.bytes);
            ExitCode::from(e.exit)
        }
    }
}
