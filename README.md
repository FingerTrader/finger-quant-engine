# Finger Quant Engine (FQE)

Core Rust-based quantitative strategy engine for local execution, backtesting, optimization, and simulation.

FQE is an independent, **Local-First, Language-Neutral** engine. All strategy computation runs on the user's own device. The Rust core owns validation and execution semantics; a typed Strategy Intermediate Representation (IR) defines a portable strategy subset. Protocol Buffers are the intended cross-language boundary.

Offline engine use does not require Flutter Desktop, a Finger Trader login, or a trading account. Flutter Desktop is a planned optional client. Trading APIs belong to the independent **Finger Trader Trading SDK**; this repository does not implement that SDK.

## Project status

This repository contains the initial engineering foundation, version `0.1.0`. It is not a production trading engine or a published SDK release.

| Area | Status in this repository |
| --- | --- |
| Cargo Workspace and validated core types | Implemented |
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
```

Commit `Cargo.lock` alongside dependency changes. The current lockfile contains only these five workspace packages. See [Development](docs/DEVELOPMENT.md) for offline verification and dependency rules.

## Architecture and contracts

- [Architecture](docs/ARCHITECTURE.md)
- [Strategy contract](docs/STRATEGY_CONTRACT.md)
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
