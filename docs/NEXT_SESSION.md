# Next Session — Repository Recovery

Updated 2026-11-14 (Asia/Tokyo); derived from [Project status](PROJECT_STATUS.md).

## Recover first

Read PROJECT_STATUS, DELIVERY_PLAN, ARCHITECTURE_DECISIONS, README, [Workflow](WORKFLOW.md), [authority](DOCUMENT_AUTHORITY.md) and current task contracts. Verify root/Git status/branch/upstream/history/evidence before editing. Main only; preserve unrelated work and all history. Actual code/tests/Git override stale prose. User authorized continuing downstream tasks without routine confirmation. No publishing requested; do not inherit a historical push receipt.

P1-01–P1-05 COMPLETED; P1-06 VERIFIED awaiting evidence/closure commit. P1-07 becomes READY only after that closure. P1/P2–P6 gates remain incomplete. Recover newer dirty work before starting. Resolve latest record with `git log -1 --format=%H -- docs/PROJECT_STATUS.md`, actual checkout with `git rev-parse HEAD`. No toolchain installation needed.

## Latest delivered evidence

P1-06 artifacts implemented and fully verified; evidence SHA pending. [Indicator contract](INDICATOR_CONTRACT.md), [numeric evaluation](P1_06_NUMERIC_EVALUATION.md) and ADR-017 own formulas/precision/limits/state. Shared exact-pinned registry adds incremental factories and owned deep-staged instances; SMA/EMA/MACD/RSI use ordinary versioned parameter and named-port descriptors, not name branches. Pure descriptor canonical bytes and legacy APIs remain unchanged.

macOS ARM64, existing rustc 1.99.0: fmt/check/locked offline workspace tests/strict all-target Clippy PASS. **96 behavioral tests = 86 prior + 10 indicators**, zero failed/ignored/doc tests. CLI demo and all five fqe-ir examples PASS. Independent Fraction fixtures: nine datasets/563 inputs, eight output series, exact readiness and ≤1 output coefficient error. G-007 crosses only at bar5; manual G-002 MACD/RSI composition crosses at bar5 with threshold60 and none at90. Parameter/profile/units/precision limits, signed ties/large-value decision boundaries, max-period compound seeds, isolation/reset and provider error/panic/invalid-output rollback verified. Doc checker passed 32 Markdown/42 tasks; final link count is in tool evidence. Diff/protected-path review passed.

No dependency/manifest/lockfile/toolchain/LICENSE/core/runtime/CLI/protocol/loader/pure built-in/expression code changes. Existing three reviewed libraries remain cached/pinned; no new security query/scanner/private upload. No Desk or external repository edit/run. Previous evidence: P1-05 `424aacb9faf5d081b72bf0a82f26ef396de16c78`, P1-04 `d0382d365815d233e4f28773995e2ef688f2a656`, P1-03 `b4b7707fc786c5ea5b68dd7f19fb03cb10f5b45d`, P1-02 `a653c105fba5ea6ada15395abc22cc8f82cc4472`. Preserve these verified slices and independent pure canonical/model identity fixtures.

No push requested/attempted. Remote main read-confirmed at `0ac5d7b163e77bddb5f69689a1455e3560aad872`; resolve current local ahead count from Git. No functional phase gate passed. Before next implementation finish P1-06 evidence/closure bookkeeping under WORKFLOW.

## Next task: P1-07 — Strategy Compiler & Typed Graph

Read [P1 design](P1_STRATEGY_DESIGN.md), [Extensibility](EXTENSIBILITY_CONTRACT.md), [Model loading](MODEL_LOADING.md), [Provider registry](PROVIDER_REGISTRY.md), [Expressions](EXPRESSION_CONTRACT.md), [Indicators](INDICATOR_CONTRACT.md), type/parameter and state/event contracts. Mark P1-07 IN_PROGRESS after P1-06 closure, not before.

- Implement a real loaded-model compiler: generic descriptor-driven signatures, exact artifact/version/capability pins, typed named edges/parameters, deterministic topology/compiled identity and source diagnostics. Reject unresolved references, bad ports/units/scales, direct/indirect same-event cycles and unsupported node/state kinds before execution. No indicator-specific compiler branches.
- Add bounded reusable pure rule-module definitions, typed parameter/port binding and expansion, direct/indirect recursion/cycle/resource rejection and source maps retaining original invocation/node paths. The loader currently rejects nonempty future module fields: choose explicit schema/capability evolution, preserve accepted scalar model canonical fixtures and reject unknown features. Do not silently treat modules as executable opaque metadata or bypass expansion limits.
- Compile two independent X-001 unknown-name provider/predicate compositions and reusable modules using the shared registry; prove stable ordering/identity independent of editor/declaration order. Test invalid versions/types/signatures/units, module relationships/bindings, recursion, node/depth/expanded graph limits and source diagnostics. Retain all prior examples/tests.
- Stateful indicators need a generic compiled subscription boundary. Pure prepare/prepare_node currently reject stateful entries; instantiate creates actual state. Separate compile-time validation/pinned preparation from run-owned state creation as needed. Compilation/discovery must not accidentally advance state. No repeated stateful Expr::Call or global shared history. P1-08 owns once-per-accepted-event eager subscriptions and atomic all-node/signal commit, independent of Boolean guards.
- Generalized State/Delay/workflow can be explicitly rejected until P2. Preserve Local-First/offline standalone APIs; no Desk, workers/plugins, broker SDK, accounting/simulator or CLI whole-model runner expansion in this task. P1-09–P1-11 own loaded CLI/external-caller/golden acceptance; manually stepped library tests do not close those gates.

Expected scope: fqe-ir compiler/module/pinned-plan interfaces, bounded explicit model codec evolution as required, fixtures/tests/example, scoped contracts/ADR and canonical progress/handoff. Protect unrelated work, LICENSE/history/private datasets/local-toolchain, Desk/SDK repos and legacy demo. No branch/worktree/reset/force-push. Record partial state/checks before interruption; evidence/closure commits required before P1-08.

## Numeric and state cautions

P1-06 work18/output12 HalfEven, abs(close)≤1e12, periods≤10000; RSI's nonzero mean gain+loss must be ≥1e-6 input units, otherwise registry_precision with no committed step. RSI uses Ratio 0–100. SMA/EMA/MACD preserve Scalar/Price units. History declares maximum initial seed samples (SMA/EMA10000, RSI10001, MACD19999), not a finite recurrence horizon or snapshot layout. Output tolerances are fixture assertions; runtime decisions compare exact quantized values. Source pins are not binary attestations.

IncrementalInstance is atomic for one node only; manually sequencing indicators/crossings is not an atomic graph event. Missing/Warmup input resets fresh history, Error/failure retains it; sequence-number gaps alone do not reset. Deep fork correctness is trusted Rust behavior, not isolation/quotas. General snapshots/external providers remain future gates.

## Commands

```sh
export CARGO_HOME="$PWD/.local-toolchain/cargo"
export RUSTUP_HOME="$PWD/.local-toolchain/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
export RUSTUP_AUTO_INSTALL=0
rustc --version
cargo fmt --check
cargo check --workspace --locked --offline
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo run --locked --offline -p fqe-cli -- demo
cargo run --locked --offline -p fqe-ir --example parameters
cargo run --locked --offline -p fqe-ir --example load_model
cargo run --locked --offline -p fqe-ir --example providers
cargo run --locked --offline -p fqe-ir --example expressions
cargo run --locked --offline -p fqe-ir --example indicators
python3 scripts/check_docs.py
git diff --check
```

Add current-task acceptance commands. Do not regenerate goldens during tests. Record real counts/results/platform/identities/limitations. Close under WORKFLOW with reviewed scoped evidence/closure commits; inspect remote main, publish only when authorized and verify receipt. Before interruption record dirty/staged/unpushed state and exact last failure/pending steps.

## Product constraints

Desk historical backtesting/local simulation is first delivery; live trading has separate acceptance. Deliver FQE plus supported runtimes in existing Flutter Desk so customers install once. Standalone/offline FQE needs no Flutter/login/trading account; broker APIs stay in the independent Trading SDK. [Desk integration](DESK_INTEGRATION_PLAN.md) owns migration, native paired runs and installed package acceptance. Preserve Desk dirty work and historical backend identity. No FQE/Desk native integration verified yet. Engine conformance and bundled platform/runtime support remain separate matrices.
