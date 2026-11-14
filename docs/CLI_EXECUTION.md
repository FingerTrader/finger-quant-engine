# Local CLI execution and file exchange

Status: implemented, experimental P1-09. This contract owns the new `validate`/`run` commands and version-1 local parameter/data/result/diagnostic mappings. [Event execution](EVENT_EXECUTION.md) owns strategy/event semantics; [model loading](MODEL_LOADING.md), [graph compiler](GRAPH_COMPILER.md), [expressions](EXPRESSION_CONTRACT.md) and [indicators](INDICATOR_CONTRACT.md) remain authoritative for their existing APIs. No historical database loader, external provider loader, backtest accounting, simulator, worker transport or Desk boundary is implied.

## Commands

```sh
cargo build --locked --offline -p fqe-cli
./target/debug/fqe validate --model crates/fqe-ir/tests/fixtures/g002-events.json
./target/debug/fqe run --model crates/fqe-ir/tests/fixtures/g002-events.json --data crates/fqe-cli/tests/fixtures/g002-data.json --params crates/fqe-cli/tests/fixtures/parameters-60.json --run-id research-001
./target/debug/fqe run --model crates/fqe-ir/tests/fixtures/g002-events.json --data crates/fqe-cli/tests/fixtures/g002-data.json --params crates/fqe-cli/tests/fixtures/parameters-90.json --run-id research-002
./target/debug/fqe run --model crates/fqe-ir/tests/fixtures/g007-events.json --data crates/fqe-cli/tests/fixtures/g007-data.json
```

`--model` is required for both commands; `--data` is required only for run. `--params` is optional and absent means model defaults/unbound parameter policy. Run's optional `--run-id` defaults to `cli`, accepts 1–128 ASCII letters/digits/`._-`, and is a caller-chosen provenance label, not a generated unique ID. Use separate IDs when retaining separate runs. Validation accepts no data/run ID and never creates provider state or evaluates market events. Both compile the same loaded artifact/parameters against the same catalog before success. Validation does not prove that every future input will evaluate successfully.

Options use separate flag/value arguments in any order. Unknown/duplicate flags, missing/empty values, positional arguments and `--flag=value` forms fail. A value starting `--` is rejected; use a relative `./` or absolute path for such a filename. Paths use OS strings, including Unicode/spaces; filesystems may reject non-UTF-8 paths, which return structured IO errors rather than argument-decoding panics. Only regular local files are accepted (symlinks to regular files are permitted); no stdin, FIFO, directory, network source or inferred neighboring file. Files are not modified. `fqe --help`, `fqe run --help` and `fqe validate --help` display usage.

The original `demo`, help/version and unsupported legacy command behavior are preserved. New command results are JSON; Cargo's own build messages are separate stderr from the `cargo run` wrapper. Invoke the built `fqe` binary directly when consuming its result contract.

## Reviewed catalog and generic execution

[fqe_ir::catalog::reference](../crates/fqe-ir/src/catalog.rs) registers the existing three reviewed pure built-ins. For a closed-bar trigger it also registers the existing four incremental indicator descriptors under `reference`, typed to that trigger's instrument and price scale. Tick mode has U64 caller-scaled prices, so it does not silently reinterpret them as Decimal indicator inputs. Unknown entries or wrong source-artifact pins are rejected.

The CLI contains no indicator-name strategy condition, crossing implementation or financial evaluator. Catalog registration is intentionally separate from the generic loader/compiler/runtime; adding a conforming provider to a library caller does not require per-name compiler/runtime changes. This executable's reference catalog is not arbitrary plugin discovery, source compilation or Java/Go installation. Those integrations retain later acceptance gates. CLI and library parity requires the same catalog/bindings because the entire frozen registry contributes to compiled identity.

## Parameter document

Use the existing version-1 typed binding shape:

```json
{"version":1,"values":{"threshold":{"type":{"version":1,"shape":"scalar","kind":"decimal","scale":12,"unit":{"kind":"ratio"}},"value":"90000000000000"}}}
```

All exact value/unit/scale/default/required/null/range/relationship rules remain unchanged. `null` is explicitly distinct from an omitted binding and is accepted or rejected by the schema. Parameter names/values must be valid before compilation. The result records SHA-256(raw parameter file bytes), or null when no parameter file was supplied; the compiled identity separately pins bound parameter semantics.

## Market data document

Strict JSON, not JSON Lines or CSV:

```json
{"format":"fqe.market-data","schema_version":1,"events":[{"kind":"tick","sequence":"1","timestamp_ns":"1000","instrument":"DEMO","price":"101"}]}
```

A tick record requires exactly `kind`, `sequence`, `timestamp_ns`, `instrument`, `price`. Sequence is positive canonical u64 decimal text; timestamp is canonical nonnegative u64 text and price is positive canonical u64 text in caller-defined ticks. Integers beyond 2^53 and through u64::MAX remain strings throughout input/output.

A closed-bar record requires exactly `kind:"closed_bar"`, `sequence`, `timestamp_ns`, `instrument`, `timeframe_ns`, `open_ns`, `close_ns`, `closed`, `open`, `high`, `low`, `close`, `volume`. Times/duration/sequence are canonical u64 strings; `closed` is Boolean true. Each OHLC/volume field is a full typed value as in model/binding documents; decimal `value` is the signed coefficient string, with explicit type/scale/unit/instrument. [G-002 synthetic input](../crates/fqe-cli/tests/fixtures/g002-data.json) is a complete example. No float, decimal-point string, date parsing, scale inference, missing-to-zero conversion or future field is accepted.

`MarketData::from_bytes` enforces structure and individual tick/closed-bar invariants. EventRuntime enforces trigger domain, instrument policy, declared scales, continuous intervals and sequence/time ordering. The batch adapter stops at the first rejected event, does not retry or repair, and prefixes runtime diagnostic paths with zero-based `$.events[index]`. A later failure discards the complete batch result, including observations/signals computed earlier. The underlying streaming runtime's retry capability is unchanged; this one-shot CLI chooses not to use it.

An explicitly empty `events:[]` is valid and returns a completed zero-event report with null cursors. Empty files, blank records, `{}` entries, truncated documents, trailing documents, duplicate fields and unsupported versions/kinds fail. Input order is preserved, never sorted. Unrelated ticks are counted but produce no evaluated outputs/signals; mixed bar/tick modes are rejected.

## Bounds and publication

Each model, parameter and data file is capped at **262144 bytes** before parsing, including whitespace. A bounded `max+1` read verifies the cap even if file size changes. The shared strict parser also enforces depth32, values16384, collection1024, object-key128 and string4096-byte limits. A dataset therefore has at most1024 events, with the byte/item limits often tighter for typed bars. Event-plan32-slot/root and AST bounds still apply. These are synthetic local-file profile limits; P3 owns scalable persisted historical ingestion.

The complete result is capped at **4MiB** before its final newline. A bounded serializer collects bytes while consuming one runtime observation at a time; it does not retain an unbounded observation vector. A failed writer cannot be finished/reused. Validation/execution/encoding failure publishes no result bytes on stdout. Successful encoding is followed by one JSON document and one newline. If OS stdout writing itself fails, the CLI returns output error5; a partial OS write cannot be rolled back. Consumers must require exit0 and parse the complete document, never treat an incomplete prefix as success. File and output caps are not CPU time, peak process memory or untrusted-provider isolation guarantees.

## Results and diagnostics

Validation stdout is `fqe.validation-result`, schema1, with status `valid`, manifest, typed public inputs/root outputs and string node_count. No observation array, runtime state or signal decision is fabricated by validate.

Run stdout is `fqe.run-result`, schema1:

- `manifest`: run, dataset_id, engine_version, artifact_id, artifact_source_id, execution_id, graph_id, registry_id, parameters_source_id, trigger, warmup_samples and interval_convention. Dataset ID is SHA-256 of the **actual data bytes read**, including formatting; this identifies local input, not authenticity of market history. Trigger carries duration and price/volume scales for bars. No wall clock, random value or absolute file path enters the report.
- `observations`: ordered accepted-event records with run/execution ID, source_sequence, timestamp_ns, matched/warmup flags, named outputs and signal records. A ready output is `{"status":"ready","value":<typed value>}`; missing/warmup carry only their status. Error readiness is rejected rather than encoded as an accepted observation. Warmup roots remain observable but produce no signals. Signals retain source sequence/time/instrument/kind and remain observations, not orders/fills.
- `summary`: status `completed`, processed_events, matched_events, signals, last_sequence and last_timestamp_ns. All counts/sequences/times/value coefficients are canonical strings; absent cursors are null. Schema versions/type scales remain small JSON integers; Boolean payloads remain JSON booleans. Named object keys and output declarations have deterministic order. Same files/catalog/run ID produce identical bytes; formatting-only source changes may change source IDs even when semantic plan identity is unchanged.

New-command failures produce one JSON diagnostic on stderr and no stdout result, except the OS-write caveat above. Shape: `{"format":"fqe.diagnostic","schema_version":1,"stage":"compile","code":"...","path":"...","message":"..."}`. Stages distinguish arguments, model_io/model, parameters_io/parameters, catalog/compile, data_io/data, execution, result and output. Messages are fixed public descriptions; OS path/error text and raw input values are not copied. Diagnostic path is capped at512 characters and preserves the field/node/record location where available.

| Exit | New-command meaning |
| --- | --- |
| 0 | Complete validation/run JSON written successfully |
| 2 | Invalid command options/run ID |
| 3 | Unreadable/oversized/nonregular file, invalid model/binding/data document, unavailable provider or compile/type failure |
| 4 | Syntactically valid data rejected during event execution, including order/domain, provider or numeric failures |
| 5 | Result encoding/size failure or stdout IO failure |

Legacy demo errors retain exit1; unsupported legacy commands retain text stderr/exit2. Help/version retain text output. This experimental contract is independently versioned from draft Protobuf/IPC and is not a stable FFI/worker ABI.

## Evidence and next boundaries

[Exchange tests](../crates/fqe-ir/tests/exchange_contract.rs) parse result JSON, verify independent byte digests/full-width integers, strict versions/duplicates/types/limits, typed bars/bindings, readiness, failed-writer poisoning and4MiB enforcement. [Actual binary tests](../crates/fqe-cli/tests/loaded_execution.rs) verify G-002/G-007 signalbar5, threshold60/90 and changed model defaults without rebuild, repeat determinism, byte equality with the public library report adapter using the same catalog, structured failure/no partial batch result, byte-limit exact boundary, Unicode paths, broken output pipe and full-width tick filtering. The original CLI tests still run.

[Fixture generator](../scripts/generate_cli_fixtures.py) uses only Python stdlib, writes new synthetic files and independent raw SHA references, and never runs during tests. Earlier financial provider sources and identity/golden fixtures are unchanged. [Runtime report adapter](../crates/fqe-runtime/src/report.rs) is the shared library path; it delegates decisions to EventRuntime.

P1-10 must deliver a separately runnable independent caller and its versioned process success/failure/cleanup acceptance. Existing CLI subprocess tests are P1-09 evidence, not closure of P1-10. P1-11 still owns the complete strategy/golden gate; P3 owns real backtest/local simulation and P4–P6 own installed Desk/language packaging. No network/login/Flutter/trading account is needed to execute these local examples after the reviewed build dependencies have been cached.
