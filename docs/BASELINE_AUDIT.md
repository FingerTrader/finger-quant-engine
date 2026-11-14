# Baseline Architecture Audit & Strategy IR Gap Analysis

Task: P1-01. Audit date: 2026-11-14 (Asia/Tokyo). Baseline commit: `342d8aa76b48d32fa9c9110609b371eac52b8d86`. This is a dated evidence report; [Project status](PROJECT_STATUS.md) owns live progress. The governance changes do not change Rust source, manifests, lockfile, draft protobuf or LICENSE.

## Repository and toolchain evidence

Actual root: `/Users/george/Desktop/finger-quant-engine`, confirmed with `pwd`. Initial `git status --short --branch`: clean main tracking origin/main. Origin fetch/push: `https://github.com/FingerTrader/finger-quant-engine.git`. `git ls-remote origin refs/heads/main` matched the baseline. History contained `342d8aa` (foundation) and `cbcebad` (initial commit). No reset, branch, worktree or history rewrite was needed.

Workspace resolver 3, edition 2024, rust-version 1.99, package 0.1.0, Apache-2.0, publish=false. Pin is Rust 1.99.0 with rustfmt/Clippy. Shell initially had no global rustup; the existing ignored `.local-toolchain/` contains the pin and was activated without installation. Actual versions: `rustc 1.99.0 (b940084d7 2026-09-28)`; `cargo 1.99.0 (5f94df478 2026-08-27)`; target aarch64-apple-darwin.

All five manifests and Cargo.lock inspected. Only workspace path dependencies and standard library; no registry packages, build.rs, generators or native extensions. No dependency/toolchain change or private data upload. No external vulnerability scanner run; no new vulnerability-free assertion is made. Historical installer/advisory statements in DEVELOPMENT are initialization evidence, not a new security audit. Ignored toolchain/build/data/run paths remain ignored.

## Implementation / tests trace

| Crate and inspected source | Actual behavior | Existing verification | Not implemented |
| --- | --- | --- | --- |
| [fqe-core](../crates/fqe-core/src/lib.rs) | Validated InstrumentId, positive exact u64 PriceTicks, positive event sequence, nonnegative timestamp; Long/Flat signals | 3 unit tests: identifier bounds, price exactness including u64::MAX, event validation | Units/scales/currency/quantity metadata, bars, intent/account/order types |
| [fqe-ir](../crates/fqe-ir/src/lib.rs) | IR_VERSION=1 check, validated strategy ID, one instrument, Current/Literal prices, strict GreaterThan, signal per matching tick | 3 unit tests: version/ID rejection, strict exact comparison, instrument filtering | Graph, parameters, serialization, indicators, Boolean composition, compiler, state/workflow/modules |
| [fqe-runtime](../crates/fqe-runtime/src/lib.rs) | One-shot synchronous iterator, increasing sequence/nondecreasing time, counters, terminal lifecycle, ordered signal provenance; error retains accepted counters but returns no partial report | [6 integration tests](../crates/fqe-runtime/tests/runtime.rs): repeatable report/provenance, duplicate/backward sequence, backward time, ties/gaps, instrument counts, empty/one-shot terminal | Scheduler, streaming/cancel, user state, timers, checkpoint, quotas, durable replay |
| [fqe-protocol](../crates/fqe-protocol/src/lib.rs) | Embed schema text; negotiate major=1 to minor=0 only | 2 unit tests: same/newer minor, incompatible major | Codec, generated bindings, message validation, handshake/transport/capabilities |
| [fqe-cli](../crates/fqe-cli/src/main.rs) | `demo`, help, version, argument exit codes; strategy and synthetic input constructed in Rust | [3 integration tests](../crates/fqe-cli/tests/cli.rs): repeated demo, invalid args, help/version | Model/data files, generic run/backtest, external request/result protocol |

[engine.proto](../crates/fqe-protocol/proto/fqe/v1alpha1/engine.proto) inspected: experimental v1alpha1 price-comparison model and market/signal envelope, not a universal graph or working worker connection. Protocol helper 1.0 is separate from package/IR versions. Schema syntax generation was not rerun; no protoc was installed/executed.

## Target gap matrix

| Required capability | Baseline gap / architectural constraint | Owning tasks |
| --- | --- | --- |
| Definition / instance / strategy version | Rust Strategy only; immutable artifact identity and run binding absent | P1-03, P1-08 |
| Universal types / Parameter Schema | Only positive integer price plus a predicate; numeric representation, bounds, units/readiness undecided | P1-02 |
| Typed Graph / ports / Node Registry | No nodes, edges, ports, descriptors or dependency/type checking | P1-04, P1-07 |
| Expressions / indicators | One GreaterThan; no shared function registry, arithmetic, Boolean or crossing | P1-05, P1-06 |
| Event triggers / workflows / state | Tick-only stateless rule; no explicit strategy memory/Delay/time windows/transition schema | P1-08, P2-01, P2-02, P2-03 |
| Compiler / model loading / golden tests | Constructor validation only; no parser, compile plan or independent loaded-model goldens | P1-03, P1-07, P1-09, P1-11 |
| Reusable modules / requirements / plugin dependencies | No declarations/resolution; unavailable features must fail closed | P1-03, P1-04, P1-07, P6-01, P6-03 |
| External-call baseline / Desktop | Existing CLI demo is not model invocation; no FFI/IPC decision or real Desktop path | P1-10, P4-01–P4-05 |
| Persistent instances / restore / trace | Counters are not strategy state/checkpoint; no generalized scheduler, snapshot or source trace | P2-01–P2-06 |
| Local data / simulation / accounting / optimization | No historical loader, virtual account/order/fill/ledger/profile/report/trial implementation | P3-01–P3-07 |
| Python / Java / Go | No workers, launchers, lifecycle/state/recovery or conformance | P5-01–P5-05 |
| Plugin lifecycle / packaging / release | No manifest/registry/permissions/isolation, actual third-party loading or OS matrix | P6-01–P6-06 |

## Complete document review and resolutions

All pre-existing docs were read before edits; no design document was treated as implemented evidence.

| Document | Audit result / governance action |
| --- | --- |
| README | Capability table matches code; add canonical governance/recovery navigation, qualify Protobuf as first evaluation candidate. |
| ARCHITECTURE | Dependency graph/ownership match manifests; link charter/ADRs and explicit graph/workflow/state target without expanding current scope. |
| STRATEGY_CONTRACT | Current price-only semantics accurate; link new design as target, not an already-supported IR extension. |
| EVENT_CONTRACT | Current ordering/error semantics accurate; qualify wire choice and reference future scheduling tasks. |
| STATE_CONTRACT | Correctly separates counters from durable/financial state; retain current transitions. |
| RUNTIME | Current one-shot/resource limitations accurate; preserve all exclusions. |
| BACKTESTING | Planned and no performance claims; assign P3 as authoritative scope. |
| SIMULATION | Planned separation accurate; clarify local virtual accounts and profile scope without backend requirement. |
| DESKTOP_INTEGRATION | Prior preference for process protocol could appear predetermined; replace with FFI/IPC evaluation and ADR-010 gate. |
| LANGUAGE_WORKERS | No implementation falsely claimed; fix delivery order to Python → Java → Go. |
| PLUGIN_SYSTEM | Planned extension points accurate; keep execution mechanism open and map to P6. |
| ROADMAP | Old Phase 1–6 ordering conflicts with confirmed six stages; retire old numbering and provide explicit intent mapping to DELIVERY_PLAN. |
| DEVELOPMENT | Historical 17-test baseline matches rerun; preserve as historical evidence; link current status/workflow and governance checker. |

No contradiction between existing Rust behavior and its current-behavior contracts was found. Main gaps are capability absence, stage numbering and prematurely preferred boundary language. Planned specification changes are explicitly labeled; no public stable codec/transport/numeric/plugin mechanism is claimed.

## Verification rerun

Existing project-local pinned toolchain, `RUSTUP_AUTO_INSTALL=0`, macOS ARM64, no downloads:

| Command | Observed result on 2026-11-14 |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo check --workspace --locked --offline` | PASS |
| `cargo test --workspace --locked --offline` | PASS: 17 behavioral tests (core 3, IR 3, protocol 2, runtime integration 6, CLI integration 3); 0 failed/ignored; 0 doc tests |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | PASS, no warnings |
| `cargo run --locked --offline -p fqe-cli -- demo` | PASS: completed, events=3, signals=2, sequences 2/3 at 2000/3000 ns |

Governance verification also passed: `python3 scripts/check_docs.py` checked 23 Markdown files, 142 local links/anchors, all 40 stage task IDs/dependencies and synchronized status projections. Five negative controls detected unknown dependencies, cycles, status mismatch, premature readiness and broken links/anchors. `git diff --check` passed; `git diff --exit-code -- crates Cargo.toml Cargo.lock rust-toolchain.toml LICENSE` confirmed no implementation/toolchain/license changes. These results are also recorded in PROJECT_STATUS. Tests prove only the audited subset, not indicator correctness, financial correctness, wire conformance, process isolation, platform support or production safety. Direct source review confirms the demo is hardcoded and therefore does **not** satisfy P1's artifact-loading gate.

## P1-01 acceptance and next dependency

Repository/crates/contracts/test history were rechecked; all universal IR elements have mapped gaps; current-vs-target semantics and conflicts are recorded; four required checks and CLI rerun passed. P1-01's audit acceptance is satisfied: document checks passed and evidence commit `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc` was recorded and confirmed on GitHub main. Task closure is recorded in PROJECT_STATUS/NEXT_SESSION. No implementation task is completed by this audit. Next: **P1-02 — Universal Types & Parameter Schema**. Do not repeat P1-01 unless subsequent code/history invalidates its evidence.
