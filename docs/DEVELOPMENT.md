# Development

Status: the commands below build and test the Rust foundation. Protocol generation, workers, packaging, and automated release pipelines are not configured.

Start each session with [Project status](PROJECT_STATUS.md), [Next session](NEXT_SESSION.md) and [Delivery plan](DELIVERY_PLAN.md), then follow [Workflow](WORKFLOW.md). The initialization table below is historical evidence; current evidence belongs in PROJECT_STATUS and the dated [Baseline audit](BASELINE_AUDIT.md).

## Repository and toolchain

This independent repository is the home for FQE core work. The initial development branch is `main`; do not create extra worktrees or branches for initialization. Preserve existing remote commits and never force-push.

Rust is pinned to `1.99.0` in `rust-toolchain.toml`, with the minimal profile, rustfmt, and Clippy. Package metadata declares Rust 1.99 and edition 2024. The initial machine uses macOS ARM64 and an existing Xcode linker. Other operating systems have not yet been validated.

The toolchain pin favors a reproducible baseline. Review security advisories and platform support before upgrading it; update the pin and validation evidence together.

## Project-local toolchain

Initialization installs the approved official toolchain inside the Git-ignored `.local-toolchain/`. From the repository root, activate it in the current shell:

```sh
export CARGO_HOME="$PWD/.local-toolchain/cargo"
export RUSTUP_HOME="$PWD/.local-toolchain/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
cargo --version
rustc --version
```

These exports affect the current shell only. No global toolchain or shell startup configuration is required. Do not commit or copy this machine-specific directory to GitHub. On another computer with a normal Rust installation, rustup can use the repository's pin directly after installing its components.

Toolchain installation is an explicit setup step; offline builds assume it has already completed.

## Required checks

Run from the repository root:

```sh
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
python3 scripts/check_docs.py
git diff --check
```

For reproducibility and to prohibit registry access, the Cargo build/test/lint commands also support:

```sh
cargo check --workspace --locked --offline
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
```

Use `cargo fmt` to fix formatting, then rerun `cargo fmt --check`. Fix code and behavior on test failures; do not remove tests to obtain a passing result.

## Initialization verification baseline

The initialization baseline was verified on macOS ARM64 with Rust 1.99.0:

| Check | Result |
| --- | --- |
| cargo fmt --check | Passed |
| cargo check --workspace | Passed |
| cargo test --workspace | Passed: 17 behavioral tests, none failed or ignored |
| cargo clippy --workspace --all-targets | Passed without warnings |
| Clippy with --locked --offline -- -D warnings | Passed |
| Offline CLI demo | Completed: 3 events, 2 signals |

The toolchain installer came from the official Rust distribution host and its SHA-256 matched the host's checksum. Public Rust/Rustup metadata, licenses, and GitHub advisories were reviewed; the returned Rust advisories describe versions older than this pin. At initialization the Cargo lockfile contained only five local workspace packages; P1-03 adds reviewed pinned registry libraries as recorded below. No private dependency data was uploaded to external scanning services.

A local secret-pattern review found no matches in commit candidates, and relative document links were checked. These checks are limited evidence, not an exhaustive security audit. Other operating systems, protobuf interoperability, financial accounting, worker isolation, and production workloads remain unverified.

## Run the CLI

```sh
cargo run --locked --offline -p fqe-cli -- demo
cargo run --locked --offline -p fqe-cli -- --help
cargo run --locked --offline -p fqe-cli -- --version
```

No arguments display help. Unsupported arguments exit with code 2, a demo error exits with code 1, and success exits with code 0. P1-09 now adds model-file validate/run with typed parameter/data/result codecs and structured new-command exits3/4/5; [CLI execution](CLI_EXECUTION.md) is authoritative. Backtest accounting, simulation, worker and server commands remain planned.

## Independent consumer

After building the CLI with `cargo build --locked --offline -p fqe-cli`, run:

```sh
python3 examples/external/caller.py --engine target/debug/fqe --request examples/external/requests/g002.json
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/external -v
```

The POSIX reference caller/tests use Python standard library only, verified with Python3.14.6 on macOS ARM64. The tests require an actual built binary; `FQE_ENGINE` can select another explicit local executable path. They fail if it is absent rather than silently skipping. Normal examples use existing synthetic files; fault-process tests create temporary trusted local scripts and remove their files. [External invocation](EXTERNAL_INVOCATION.md) owns request/results, exact source validation, process deadlines and cleanup limits. No third-party Python package or engine runtime dependency is introduced.

## Tests and scope

For the consolidated P1 check, run `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p1_gate.py` with the pinned toolchain active. It executes the required Rust checks/build, independent caller suite, eleven actual CLI/external cases, source/fixture pins and docs/diff checks; stdout is a machine-readable report. [P1 acceptance](P1_ACCEPTANCE.md) maps G-001–G-005/G-007 and X-001 to actual independent goldens. `--skip-checks` is a partial process-only check and cannot establish a full gate pass. No expected fixture is regenerated during validation.

Tests cover domain validation, strict price comparison, instrument matching, version rejection, repeatable runtime reports, input ordering, failed and terminal states, empty input, signal provenance, CLI output/exit codes, and protocol version-number compatibility.

Current tests also cover loaded graph/event/indicator execution and exact CLI/independent local-process JSON interoperability. They do not verify financial accounting, Protobuf/worker transport interoperability, worker isolation, Desktop integration, or production workload limits because those features are not implemented. Dated counts and acceptance evidence belong in [Project status](PROJECT_STATUS.md).

## Protocol schema

The experimental source is `crates/fqe-protocol/proto/fqe/v1alpha1/engine.proto`. It is embedded as text by the Rust protocol crate. Cargo does not run protoc or generate bindings.

An already installed protoc may check schema syntax separately:

```sh
protoc --proto_path=crates/fqe-protocol/proto \
  --descriptor_set_out=/tmp/fqe-engine-descriptor.pb \
  fqe/v1alpha1/engine.proto
```

This is optional and proves syntax only. It does not implement or test encoding/decoding, wire/domain conversion, an RPC service, or cross-language support. Review and approve any new compiler or generation tool before installation/execution.

## Dependency and supply-chain rules

P1-03 adds serde 1.0.228, serde_json 1.0.145 and sha2 0.10.9 to fqe-ir, with 21 locked registry packages. Review/features/build scripts/public advisory evidence: [dependency review](P1_03_DEPENDENCY_REVIEW.md). Fetch locked dependencies once with `cargo fetch --locked`, then use locked/offline checks. No repository build script or generated Protobuf binding is introduced; approved dependency build scripts execute during builds. `Cargo.lock` must remain committed, including for this CLI workspace.

Before adding a dependency, establish need, inspect public package metadata, maintainers/repository, release age, license, scripts/native/code-generation behavior, and known advisory signals. Follow supply-chain review requirements before running unfamiliar installers or generators. Keep manifests and lockfile aligned.

Do not upload private source, manifests, package names, dependency graphs, or local scanner reports to third-party services without approval. Public advisory checks are evidence, not proof of absence of vulnerabilities. Record residual risk when dependencies or tools change.

## Data, secrets, and publication

`target/`, `.local-toolchain/`, `data/`, `runs/`, environment files, and private key extensions are ignored. Use synthetic fixtures in tracked tests; keep datasets, credentials, accounts, and private reports out of commits.

Before pushing, inspect `git diff --check`, all intended staged files, the lockfile, secret-like content, and the remote branch tip. Fetch and preserve new remote commits; use a normal fast-forward push to main. Keep existing README intent and repository license. No registry publishing or GitHub release automation is configured.

The model import example is `cargo run --locked --offline -p fqe-ir --example load_model`; it imports/prepares declarations only. See [Model loading](MODEL_LOADING.md) for exact JSON and identity/resource contracts.

Run `cargo run --locked --offline -p fqe-ir --example providers` for two explicitly driven unknown-name provider compositions. The fixture source is reviewed local code; no external extension or language runtime is loaded. `python3 scripts/generate_provider_fixtures.py` deliberately regenerates source pins and independent JSON/SHA expectations after reviewed provider-source changes; tests never regenerate their own expected fixtures. See [Provider registry](PROVIDER_REGISTRY.md).

Run `cargo run --locked --offline -p fqe-ir --example expressions` for typed Boolean/crossing history behavior. [Expression contract](EXPRESSION_CONTRACT.md) distinguishes this in-memory slice from a whole-model compiler/event runtime.
