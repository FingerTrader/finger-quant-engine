# Finger Quant Engine (FQE)

**Universal Local-First Quantitative Strategy Engine** — an independent Rust-based engine for local strategy development, execution, backtesting, optimization, and simulation.

FQE is an independent, **Local-First, Language-Neutral** engine. All strategy computation runs on the user's own device or user-controlled server. The Rust core owns validation and execution semantics; a typed Strategy Intermediate Representation (IR) defines a portable strategy subset. A versioned cross-language protocol is required; Protocol Buffers is the first candidate to evaluate, not an implemented codec or selected transport.

Offline engine use does not require Flutter Desktop, a Finger Trader login, or a trading account. Flutter Desktop is a planned optional client. Trading APIs belong to the independent **Finger Trader Trading SDK**; this repository does not implement that SDK.

The primary consuming product is the existing **Desk** application (`finger_desk`). Project delivery requires FQE bundled into Desk for the first local backtest/simulation loop, while standalone engine use remains independent. Live trading has a separate acceptance scope. The [Desk integration contract](docs/DESK_INTEGRATION_PLAN.md) covers migration, native run/stop/results, packaging and final paired acceptance; this integration is planned, not implemented.

Language and financial-function support are open-ended targets: Python/Java/Go and the first indicators are reference implementations. The [Extensibility contract](docs/EXTENSIBILITY_CONTRACT.md) defines adapter onboarding, callable indicator/predicate providers, rule modules and descriptor-driven Desk use. A new conforming extension within supported capabilities must not require per-name core/compiler/UI changes. These extension mechanisms remain planned and need their own execution/conformance evidence.

## Project status

The unique progress ledger is [PROJECT_STATUS](docs/PROJECT_STATUS.md). New sessions recover from it, [NEXT_SESSION](docs/NEXT_SESSION.md), [DELIVERY_PLAN](docs/DELIVERY_PLAN.md), [ADRs](docs/ARCHITECTURE_DECISIONS.md), then the current task contracts. P1 remains incomplete; the current demo does not meet universal strategy acceptance.

This repository contains the initial engineering foundation, version `0.1.0`. It is not a production trading engine or a published SDK release.

| Area | Status in this repository |
| --- | --- |
| Cargo Workspace and validated core types | Implemented |
| Checked decimal values, scalar/unit descriptors and immutable parameter bindings | Implemented, experimental P1-02 subset; [contract](docs/TYPE_PARAMETER_CONTRACT.md) |
| Typed IR: a single-instrument price comparison producing signals | Implemented, experimental API |
| Synchronous, deterministic in-memory event runtime and run state | Implemented, experimental API |
| Offline CLI demonstration and behavioral tests | Implemented |
| Protocol version-number negotiation helper | Implemented, experimental |
| Protocol Buffers schema | Experimental draft; no codec, generated bindings, or transport |
| Local backtesting, accounting, execution simulation, optimization | Planned |
| Python / Java / Go workers and source-language compilers | Planned |
| Plugins, Desktop IPC, persistence, Trading SDK adapter | Planned |

**Implemented** means executable code is present and tested. **Experimental** means the API or schema can change and has no compatibility guarantee. **Planned** means design intent only. These labels can overlap: an experimental component may have implemented code.

The demonstration evaluates three synthetic price events and emits two signals. Signals are observations, not orders, fills, positions, or investment performance.

## Quick start

Install the Rust toolchain declared in [rust-toolchain.toml](rust-toolchain.toml), including rustfmt and Clippy. A platform linker is required (Xcode Command Line Tools on macOS). No third-party Cargo packages, Python, Java, Go, Flutter, or `protoc` are needed for the Rust foundation.

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

After the toolchain is installed, the current Rust build and demo need no network. No data download, account authentication, or telemetry is implemented.

For the project-local toolchain used during initialization, see [Development](docs/DEVELOPMENT.md#project-local-toolchain).

## Workspace

```text
crates/
  fqe-core/       Validated instrument, integer price, event, and signal types
  fqe-ir/         Versioned typed rule model and pure evaluation
  fqe-runtime/    Ordered event loop, lifecycle, and signal reports
  fqe-protocol/   Draft .proto schema and version negotiation helper
  fqe-cli/        Standalone fqe executable
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

Commit `Cargo.lock` alongside dependency changes. The current lockfile contains only these five workspace packages. See [Development](docs/DEVELOPMENT.md) for offline verification and dependency rules.

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
- [Event contract](docs/EVENT_CONTRACT.md)
- [State contract](docs/STATE_CONTRACT.md)
- [Runtime](docs/RUNTIME.md)
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
