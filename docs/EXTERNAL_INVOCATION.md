# Independent local process invocation

Status: implemented, experimental P1-10 reference profile. This contract owns the independent caller's request, response and subprocess supervision. [CLI execution](CLI_EXECUTION.md) owns the engine command/file/result schemas. [Event execution](EVENT_EXECUTION.md) owns financial evaluation. The consumer imports no engine modules and does not implement indicators, expressions, crossing history or a second compiler.

## Run the reference consumer

The [caller](../examples/external/caller.py) uses Python's standard library and a separately compiled `fqe` executable. Python is required only for this reference consumer/tests; the Rust engine remains independently runnable. Verified platform: macOS ARM64, Python3.14.6, Rust/Cargo1.99.0. The process supervisor requires POSIX; Windows integration remains unimplemented. No package installation, runtime download, login, network, Flutter or trading account is involved in these examples.

From the repository root with the [pinned toolchain](DEVELOPMENT.md#project-local-toolchain) active:

```sh
cargo build --locked --offline -p fqe-cli
python3 examples/external/caller.py --engine target/debug/fqe --request examples/external/requests/g002.json
python3 examples/external/caller.py --engine target/debug/fqe --request examples/external/requests/g002-high.json
python3 examples/external/caller.py --engine target/debug/fqe --request examples/external/requests/g007.json
python3 examples/external/caller.py --engine target/debug/fqe --request examples/external/requests/validate.json
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/external -v
```

The first and third runs produce one long signal at bar5; the second uses threshold90 and produces none, without rebuilding the engine. [Tick](../examples/external/requests/tick.json), [empty](../examples/external/requests/empty.json) and [engine failure](../examples/external/requests/engine-error.json) requests demonstrate exact full-width values, explicit completed0 and a late numeric failure with no successful partial result. The last request deliberately exits11 with a structured engine_error on stderr.

## Invocation request version1

`--engine` explicitly selects the trusted local executable outside the request. Both CLI options are required. The engine path and request path resolve against the caller's current directory; model/parameter/data paths resolve against the request file's directory. Unicode, spaces and shell metacharacters are literal paths. An argument vector with `shell=False` is used; request text cannot add CLI flags or a shell command. No executable search, installer or extension discovery is implemented.

The UTF-8 JSON request has exactly these required fields; unknown/missing fields or unsupported versions fail:

```json
{
  "format": "fqe.invoke-request",
  "schema_version": 1,
  "request_id": "external-g002",
  "operation": "run",
  "model": "../../../crates/fqe-ir/tests/fixtures/g002-events.json",
  "parameters": "../../../crates/fqe-cli/tests/fixtures/parameters-60.json",
  "data": "../../../crates/fqe-cli/tests/fixtures/g002-data.json",
  "run_id": "external-g002",
  "timeout_ms": 5000
}
```

This example uses the location of [g002.json](../examples/external/requests/g002.json). `request_id` and non-null `run_id` are1–128 ASCII letters/digits/`._-`; they are logical labels, not secrets or filesystem paths. `operation` is exactly run or validate. `model` is a nonempty path. `parameters` is a path or null, selecting explicit bindings or model defaults respectively. Run requires data and run_id; validate requires both fields null. Paths are nonempty UTF-8 strings≤4096 bytes with no NUL. Absolute paths are permitted; paths are not restricted to this repository. Timeout is an actual JSON integer100–60000 milliseconds; Boolean coercion is rejected. It covers process launch/capture/exit, excluding source-file pre-read, result validation and bounded cleanup.

Request size≤16384 bytes; each source file≤262144 bytes. All must be readable regular local files. Empty files are not silently treated as empty data; the engine owns document validation. The consumer pre-reads/hashes source bytes, starts the engine with paths, then compares successful result source IDs with the bytes it supplied. If a file changes before the engine reads it, a differing reported digest fails input_changed. This detects inconsistent successful execution; it is neither an atomic filesystem snapshot nor file authenticity verification. Engine errors do not provide a success manifest to compare.

## Response and caller exits

The response has exactly format, schema_version, request_id, status, engine_exit, result, diagnostic and error. Format is `fqe.invoke-response`, schema_version is integer1. The caller emits one compact UTF-8 JSON object plus newline. A successfully validated request supplies request_id; invalid/unreadable requests use null. Source-read failures after request validation retain request_id.

| Caller exit | status | engine_exit | result | diagnostic | error | Stream |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | ok | 0 | Complete supported engine result | null | null | stdout |
| 10 | caller_error | null | null | null | code/message | stderr |
| 11 | engine_error | 2,3,4 or5 | null | Complete supported engine diagnostic | null | stderr |
| 12 | caller_error | null | null | null | code/message | stderr |

Exit10 covers invalid request, file_io and file_limit. Exit12 covers platform, launch, timeout, output_limit, process, protocol, input_changed and cleanup. Messages for caller errors are fixed and do not echo raw child output, file paths or OS exception strings. A supported engine diagnostic is forwarded using the [engine's diagnostic contract](CLI_EXECUTION.md#results-and-diagnostics); it is not converted into success. engine_exit is only populated after a supported complete response is accepted. Output writing failure exits12 but cannot promise a complete diagnostic on a broken stream. argparse help/usage retain text and usage exit2. Normal successful output is deterministic for unchanged request labels/files/engine.

Success requires all of: exit0 from the selected engine, empty stderr, exactly one newline-terminated JSON document on stdout, a supported complete result schema, and successful identity/type/consistency validation. A valid-looking prefix or complete JSON followed by junk is never success. Supported nonzero engine exits require empty stdout and exactly one supported diagnostic on stderr, including the exit-appropriate stage. Unknown exits, signal termination, conflicting streams, malformed diagnostics and empty/truncated output fail as caller errors. The caller does not retry a failed process or return accumulated observations as a partial success.

## Exact result validation

The consumer retains the complete engine result without converting numeric strings to floats. JSON decoding rejects duplicate keys, NaN/Infinity, float syntax, signed JSON numeric tokens, invalid UTF-8 and excessive nesting. Small JSON numeric fields are unsigned32 values further constrained by their schemas. Full-width u64/i64/i128 values use canonical decimal strings, with no leading zeros, plus sign or negative zero. Boolean values do not coerce to integers. Decimal coefficient/scale/unit, text byte bounds and explicit ready/missing/warmup values are checked against the existing scalar contract. Schema/type versions, scalar shapes and exact fields are checked. Model/root/port names preserve the engine's1–128 ASCII `._-:/` alphabet in addition to letters/digits; currency unit identifiers preserve1–16 uppercase ASCII letters/digits. Neither is an external registry lookup.

For validate, the consumer checks supported validation format/status, manifest source hashes, trigger/warmup agreement with the supplied event plan, typed public inputs/outputs, root names and bounded node count. It does not independently recompile the graph. For run, it additionally checks:

- Actual raw model/parameter/data SHA-256 IDs, request run ID, all manifest identity syntax and supported trigger/interval convention. Null parameters_source_id is required when no bindings file was supplied.
- One observation per supplied event, exact sequence/time, order, instrument matching, run/execution linkage, complete matched root names and empty unmatched-tick outputs/signals. Ready types for a root remain stable within a run.
- Declared firstN matched-event warmup and signal suppression on every reported warmup event. The consumer accepts later provider warmup flags without reproducing indicator readiness computation.
- Signal records agree with the engine-reported Boolean roots, declared signal mappings and event provenance. This checks report consistency, not the truth of a financial formula. The shared Rust evaluator remains authoritative.
- Terminal completed status, exact processed/matched/signal counts and last cursor, including null cursors for explicitly empty data.

Semantic artifact/graph/execution/registry IDs are checked for supported SHA-256 syntax and linkage, not independently derived by a second compiler. Selected engine code and provider registrations are trusted; these hashes are not signatures, executable attestation or authentication of market data. A malicious selected executable can invent internally consistent outputs. Result validation is interoperable parsing, not a security sandbox.

## Resource limits and process ownership

Source JSON uses the existing262144-byte profile with depth32/items16384/collection1024/string4096/key128. Request parsing uses a256-value budget. Response parsing is bounded by4MiB+newline, depth32, up to1048576 decoded values,1024 per collection,4096 bytes per string and128 bytes per key. These are this reference consumer's profile; larger histories require separately designed storage/streaming APIs.

The supervisor uses nonblocking pipes and selectors to drain stdout and stderr together. Captured stdout≤4194305 bytes and stderr≤16384 bytes; reads check remaining budget before extending buffers. A separate process session/group is owned by each invocation. A monotonic deadline applies even after pipes close or when an inherited pipe prevents EOF. No interactive stdin is supplied. On success or failure, remaining processes in that owned group receive SIGKILL; the direct child is waited/reaped, with an additional1-second cleanup deadline, and pipes are closed. This is one-shot process supervision, not graceful runtime stop/resume or model cancellation. Process creation, OS scheduling and uninterruptible kernel IO are not hard real-time guarantees.

The tests observe direct-child removal after normal/error/timeout/overflow paths and owned-group descendant termination when pipes are inherited. The caller can reap its direct child; the OS owns reaping orphaned grandchildren. Descendants that deliberately escape the owned process group, hostile executables/providers, CPU/memory sandboxing and general resource isolation are outside this trusted local reference profile.

## Acceptance evidence and remaining integration

[Independent process tests](../tests/external/test_caller.py) run the actual caller as a fresh process with absolute engine path and PATH=/nonexistent; they also use reviewed local fault executables. Actual G-002 default/60 and threshold90, changed model default90, G-007, full-width ticks, empty data, validate and late execution failure are exercised. Full result JSON equals direct CLI results; [existing CLI tests](../crates/fqe-cli/tests/loaded_execution.rs) establish byte parity with the shared public library report path. Independent expected signal bar5 and RSI coefficient83333333333333 remain asserted; tests do not generate their own financial expectations. Repeated caller responses are byte-identical.

Negative coverage includes strict requests, source limits, malformed documents, literal Unicode/metacharacter paths, launch failure, unsupported platform, unknown exit/version/field, duplicate keys, truncated/non-UTF-8/float/deep responses, invalid types/readiness/counts/provenance, input-file mutation, conflicting streams, stdout/stderr floods, timeout after closed pipes and inherited-pipe cleanup. Type interoperability also covers namespaced roots, minimum i64/i128, non-three-letter currency identifiers and UTF-8 text. Current dated check counts and commit evidence belong in [Project status](PROJECT_STATUS.md).

No Rust source, indicator/provider source pin, model golden, dependency, manifest or Cargo.lock change is needed. The independent caller proves a minimal language-neutral file/process boundary. It does not implement Python strategy workers, Java/Go runtimes, FFI, IPC authentication, installed Desk integration or customer packaging. P1-11 owns the full golden/generic-engine gate; P4 owns actual Desktop transport/native packaging and P5 owns language execution. [Desk integration](DESK_INTEGRATION_PLAN.md) still requires bundling approved engine/runtime artifacts so end users need no manual development-runtime downloads. This Python example does not change that product requirement.
