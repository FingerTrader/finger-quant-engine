# Finger Quant Engine (FQE)

**Universal Local-First Quantitative Strategy Engine** — an independent Rust-based engine for local strategy development, execution, backtesting, optimization, and simulation.

FQE is an independent, **Local-First, Language-Neutral** engine. All strategy computation runs on the user's own device or user-controlled server. The Rust core owns validation and execution semantics; a typed Strategy Intermediate Representation (IR) defines a portable strategy subset. A versioned cross-language protocol is required; Protocol Buffers is the first candidate to evaluate, not an implemented codec or selected transport.

Offline engine use does not require Flutter Desktop, a Finger Trader login, or a trading account. Flutter Desktop is a planned optional client. Trading APIs belong to the independent **Finger Trader Trading SDK**; this repository does not implement that SDK.

The primary consuming product is the existing **Desk** application (`finger_desk`). Project delivery requires FQE bundled into Desk for the first local backtest/simulation loop, while standalone engine use remains independent. Live trading has a separate acceptance scope. The [Desk integration contract](docs/DESK_INTEGRATION_PLAN.md) covers migration, native run/stop/results, packaging and final paired acceptance; this integration is planned, not implemented.

Language and financial-function support are open-ended targets: Python/Java/Go and the first indicators are reference implementations. The [Extensibility contract](docs/EXTENSIBILITY_CONTRACT.md) defines adapter onboarding, callable indicator/predicate providers, rule modules and descriptor-driven Desk use. A new conforming extension within supported capabilities must not require per-name core/compiler/UI changes. The experimental pure scalar in-process [provider registry](docs/PROVIDER_REGISTRY.md) is implemented; [owned incremental indicators](docs/INDICATOR_CONTRACT.md) are implemented in P1-06; external adapters/providers and client discovery retain later execution/conformance gates.

## Project status

The unique progress ledger is [PROJECT_STATUS](docs/PROJECT_STATUS.md). New sessions recover from it, [NEXT_SESSION](docs/NEXT_SESSION.md), [DELIVERY_PLAN](docs/DELIVERY_PLAN.md), [ADRs](docs/ARCHITECTURE_DECISIONS.md), then the current task contracts. [P1 acceptance](docs/P1_ACCEPTANCE.md) maps loaded generic strategy/golden evidence and remaining boundaries; the legacy demo alone does not establish universal strategy acceptance.

This repository contains the initial engineering foundation, version `0.1.0`. It is not a production trading engine or a published SDK release.

| Area | Status in this repository |
| --- | --- |
| Cargo Workspace and validated core types | Implemented |
| Checked decimal values, scalar/unit descriptors and immutable parameter bindings | Implemented, experimental P1-02 subset; [contract](docs/TYPE_PARAMETER_CONTRACT.md) |
| Bounded JSON model import, canonical content identity and pinned parameter preparation | Implemented, experimental P1-03 subset; [contract](docs/MODEL_LOADING.md); old artifact syntax preserved |
| Frozen callable descriptors, typed scalar provider invocation and independent unknown-name example | Implemented, experimental P1-04 subset; [contract](docs/PROVIDER_REGISTRY.md) |
| Typed arithmetic/Boolean expressions and transactional crossing subscriptions | Implemented, experimental P1-05 subset; [contract](docs/EXPRESSION_CONTRACT.md) |
| Registered SMA/EMA/MACD/RSI and instance-owned incremental scalar providers | Implemented, experimental P1-06 library slice; [contract](docs/INDICATOR_CONTRACT.md); event integration below |
| Descriptor-driven graph compiler, typed inputs and bounded pure rule modules | Implemented, experimental P1-07; [contract](docs/GRAPH_COMPILER.md); pure expression projection, deferred indicator state |
| Compiled tick/continuous closed-bar execution with atomic indicator/crossing state and signals | Implemented, experimental P1-08; [contract](docs/EVENT_EXECUTION.md); streaming library/example |
| Typed IR: a single-instrument price comparison producing signals | Implemented, experimental API |
| Synchronous, deterministic in-memory event runtime and run state | Implemented, experimental API |
| Offline CLI demo and loaded model validate/run with structured exact results | Implemented, experimental P1-09; [CLI contract](docs/CLI_EXECUTION.md) |
| Independent local subprocess caller with exact result validation and bounded cleanup | Implemented, experimental P1-10 POSIX/Python-stdlib reference; [contract](docs/EXTERNAL_INVOCATION.md) |
| Versioned P1 financial/unknown-name extension goldens and unified offline gate | Implemented, experimental P1-11 suite; [acceptance/evidence](docs/P1_ACCEPTANCE.md), task closure in PROJECT_STATUS |
| Stateful typed rule modules, State/Delay, multi-market alignment and temporal instance scheduling | Implemented experimental P2 library profiles; [workflow](docs/WORKFLOW_EXECUTION.md), [scheduler](docs/TEMPORAL_SCHEDULER.md) |
| Complete bounded checkpoint reconstruction, atomic local files and observational replay/trace | Implemented experimental P2 profiles with explicit capability/history limits; [checkpoint](docs/CHECKPOINT_RESTORE.md), [trace](docs/REPLAY_TRACE.md) |
| Actual MACD-cross/wait3/RSI temporal signals and unified P2 gate | Implemented suite; [P2 acceptance](docs/P2_ACCEPTANCE.md); phase closure in PROJECT_STATUS |
| Protocol version-number negotiation helper | Implemented, experimental |
| Protocol Buffers schema | Experimental draft; no codec, generated bindings, or transport |
| Exact financial units, bounded immutable historical CSV and pinned run inputs | Implemented experimental P3-01 library; [contract](docs/FINANCIAL_HISTORY.md); closure in PROJECT_STATUS |
| Local virtual account, balanced journal, reservation/valuation and recompute replay | Implemented experimental P3-02 library; [contract](docs/LEDGER.md); closure in PROJECT_STATUS |
| Explicit local cash-long orders, next-open/prior-volume fills, costs, lifecycle and replay | Implemented experimental P3-03 library; [contract](docs/ORDER_SIMULATION.md); closure in PROJECT_STATUS |
| Explicit margin-long debt/leverage/maintenance and subsequent-open liquidation | Implemented experimental P3-04 profile; [contract](docs/MARGIN_PROFILES.md); closure in PROJECT_STATUS |
| Bounded experimental [backtest/report](docs/BACKTESTING.md) | Implemented experimental native Rust; exact financial manifests/reports and close-equity metrics |
| Bounded [local parameter optimization](docs/OPTIMIZATION.md) | Implemented experimental native Rust; isolated typed grid/training-only ranking/separate validation |
| Complete bounded financial continuation/private atomic files and full P3 offline gate | Implemented experimental native Rust; [recovery](docs/FINANCIAL_RECOVERY.md), [acceptance](docs/P3_ACCEPTANCE.md); closure in PROJECT_STATUS |
| Other broker models | Planned |
| Python / Java / Go workers and source-language compilers | Planned |
| Versioned desktop stdio boundary, Dart client stub and native Sandbox feasibility | Implemented experimental P4-01; [boundary](docs/DESKTOP_BOUNDARY.md); actual Desk backtest/run configuration integration active P4-03; native acceptance incomplete |
| Application-owned native jobs, bounded lifecycle/output cursors and SDK-only service | Implemented experimental P4-02; [controls](docs/ENGINE_CONTROLS.md); full task closure in PROJECT_STATUS |
| Consumer immutable draft/run history | Implemented experimental active P4-03; final native acceptance incomplete |
| Optional native extended Desk graph, shared-cash restricted shorts, multi-stream and bounded training-only grid | Implemented experimental; [contract and acceptance](docs/EXTENDED_RESEARCH.md); separate from legacy profiles and full P4 closure |
| Plugins, Trading SDK adapter | Planned |

**Implemented** means executable code is present and tested. **Experimental** means the API or schema can change and has no compatibility guarantee. **Planned** means design intent only. These labels can overlap: an experimental component may have implemented code.

The legacy demonstration evaluates three synthetic price events and emits two signals. The compiled `cargo run --locked --offline -p fqe-runtime --example events` example loads MACD/RSI and SMA event artifacts and verifies each signal at bar5. Signals are observations, not orders, fills, positions, or investment performance.

Run a loaded strategy using `cargo run --locked --offline -p fqe-cli -- run --model crates/fqe-ir/tests/fixtures/g002-events.json --data crates/fqe-cli/tests/fixtures/g002-data.json --params crates/fqe-cli/tests/fixtures/parameters-60.json`. Changing the parameter file to `parameters-90.json` changes the expected signal count from1 to0 without rebuilding the binary. See [CLI execution](docs/CLI_EXECUTION.md) for validate, exact input/results, limits and exits.

After building the CLI, run `python3 examples/external/caller.py --engine target/debug/fqe --request examples/external/requests/g002.json` for an independent consumer. It checks the versioned complete result, source hashes and exact integer strings, and supervises a bounded local subprocess. Python belongs only to this optional reference consumer; Desk bundling and language workers remain later tasks. See [external invocation](docs/EXTERNAL_INVOCATION.md).

## Quick start

Install the Rust toolchain declared in [rust-toolchain.toml](rust-toolchain.toml), including rustfmt and Clippy. A platform linker is required (Xcode Command Line Tools on macOS). Reviewed registry libraries are pinned in Cargo.lock. Fetch them once with `cargo fetch --locked`; subsequent builds/examples work offline. Python, Java, Go, Flutter and `protoc` are not required by the Rust examples.

```sh
cargo run --locked --offline -p fqe-cli -- demo
cargo run --locked --offline -p fqe-cli -- --help
cargo run --locked --offline -p fqe-cli -- --version
```

Expected demo output:

```text
FQE offline demo (experimental; no orders)
status=completed events=3 signals=2
signal source_sequence=2 timestamp_ns=2000 instrument=DEMO:ASSET kind=long
signal source_sequence=3 timestamp_ns=3000 instrument=DEMO:ASSET kind=long
```

After the toolchain and locked dependencies are cached, the current Rust build and demo need no network. No data download, account authentication, or telemetry is implemented.

For the project-local toolchain used during initialization, see [Development](docs/DEVELOPMENT.md#project-local-toolchain).

## Workspace

```text
crates/
  fqe-core/       Validated instrument, integer price, event, and signal types
  fqe-ir/         Versioned typed rule model and pure evaluation
  fqe-runtime/    Ordered event loop, lifecycle, and signal reports
  fqe-protocol/   Draft .proto schema and version negotiation helper
  fqe-cli/        Standalone fqe executable
  fqe-bridge/     Experimental desktop peer and FFI comparison candidate
docs/            Architecture, contracts, development, and roadmap
```

No production backtester, execution simulator, worker, or plugin crate is created prematurely. Crates are not configured for registry publication.

## Validation

```sh
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
python3 scripts/check_docs.py
```

Commit `Cargo.lock` alongside dependency changes. The lockfile contains six workspace packages and 21 reviewed registry packages (16 active on the verified target); [dependency review](docs/P1_03_DEPENDENCY_REVIEW.md). See [Development](docs/DEVELOPMENT.md) for offline verification and dependency rules.

## Governance and recovery

- [Project charter: product goal and boundaries](docs/PROJECT_CHARTER.md)
- [Document authority and conflict rules](docs/DOCUMENT_AUTHORITY.md)
- [Architecture decisions and open experiments](docs/ARCHITECTURE_DECISIONS.md)
- [Delivery plan: P1–P6 task/dependency/acceptance index](docs/DELIVERY_PLAN.md)
- [P1 universal strategy design and golden specifications](docs/P1_STRATEGY_DESIGN.md)
- [Project status: authoritative progress and actual evidence](docs/PROJECT_STATUS.md)
- [Next session: independent handoff and next acceptance](docs/NEXT_SESSION.md)
- [Workflow: recovery, development, verification, commit and push](docs/WORKFLOW.md)
- [Baseline architecture and IR gap audit](docs/BASELINE_AUDIT.md)

## Architecture and contracts

- [Architecture](docs/ARCHITECTURE.md)
- [Strategy contract](docs/STRATEGY_CONTRACT.md)
- [Type/parameter subset and numeric decision](docs/TYPE_PARAMETER_CONTRACT.md)
- [Typed expressions and crossing history](docs/EXPRESSION_CONTRACT.md)
- [Pure provider registry and invocation](docs/PROVIDER_REGISTRY.md)
- [External model loading and content identity](docs/MODEL_LOADING.md)
- [Event contract](docs/EVENT_CONTRACT.md)
- [State contract](docs/STATE_CONTRACT.md)
- [Stateful workflow execution](docs/WORKFLOW_EXECUTION.md)
- [Multi-market scheduler and alignment](docs/MARKET_SCHEDULER.md)
- [Temporal windows and instance scheduling](docs/TEMPORAL_SCHEDULER.md)
- [Bounded checkpoint and restore](docs/CHECKPOINT_RESTORE.md)
- [Replay and bounded debug trace](docs/REPLAY_TRACE.md) — deterministic local replay and observational source/event diagnostics.
- [P2 temporal strategy acceptance](docs/P2_ACCEPTANCE.md) — actual MACD/wait3/RSI, independent boundary fixtures and full recovery gate.
- [Runtime](docs/RUNTIME.md)
- [Local CLI execution](docs/CLI_EXECUTION.md)
- [Independent external invocation](docs/EXTERNAL_INVOCATION.md)
- [Backtesting](docs/BACKTESTING.md)
- [Simulation](docs/SIMULATION.md)
- [Language workers](docs/LANGUAGE_WORKERS.md)
- [Desktop integration](docs/DESKTOP_INTEGRATION.md)
- [Plugin system](docs/PLUGIN_SYSTEM.md)
- [Roadmap](docs/ROADMAP.md)
- [Development](docs/DEVELOPMENT.md)

Further FQE core development belongs in this independent repository. Consumer applications must integrate through explicit engine contracts rather than becoming the engine's source of truth.

## License

[Apache License 2.0](LICENSE). The original repository license and Git history are preserved.
