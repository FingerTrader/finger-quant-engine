# Next Session — Repository Recovery

Updated 2026-11-14 (Asia/Tokyo); derived from [Project status](PROJECT_STATUS.md).

## Recover first

Read PROJECT_STATUS, DELIVERY_PLAN, ARCHITECTURE_DECISIONS, README, [Workflow](WORKFLOW.md), [authority](DOCUMENT_AUTHORITY.md) and current task contracts. Verify root/Git status/branch/upstream/history/evidence before editing. Main only; preserve unrelated work and all history. Actual code/tests/Git override stale prose. User authorized continuing downstream tasks without routine confirmation. No publishing requested; do not inherit a historical push receipt.

P1-01–P1-05 COMPLETED; P1-06 READY at this closure and has no implementation. P1/P2–P6 gates remain incomplete. Recover any newer dirty partial task before starting. `git log -1 --format=%H -- docs/PROJECT_STATUS.md` resolves latest record; `git rev-parse HEAD` resolves actual checkout. No toolchain installation needed; use the existing pin below.

## Latest delivered evidence

P1-05 implementation evidence **424aacb9faf5d081b72bf0a82f26ef396de16c78**: expressions.rs, 17 public expression tests, expressions example, [Expression contract](EXPRESSION_CONTRACT.md) and ADR-016. Bounded immutable typed ASTs, exact checked arithmetic/comparison/Boolean logic, generic P1-04 function/predicate calls, subscribed crossing dependencies, per-instance staged history/cursor are implemented. False guards/shared references do not skip/repeat crossing updates. Missing/Warmup reset committed pairs; failed evaluation preserves history and permits retry. No external expression codec, whole-model compiler/scheduler, indicators or snapshots are delivered.

macOS ARM64, existing rustc 1.99.0: fmt/check/locked offline workspace tests/strict all-target Clippy PASS. **86 behavioral tests = 69 prior + 17 expressions**, zero failed/ignored/doc tests. CLI demo plus parameter/load_model/providers/expressions examples PASS. New example decisions false/false/false/false/true across five accepted steps. Boolean tables, exact arithmetic/rounding/dimensional failures, dependency/cycle/limits, false guards/shared references/reordered ready operands, retry/isolation/rollback tests pass. Three initial Clippy style findings were corrected before final full suite passed. Final closure document checker passed 30 Markdown/286 links/42 tasks. Diff checks and intended file review PASS.

No dependency/manifest/lockfile/toolchain/LICENSE/core/runtime/CLI/protocol/registry/built-ins/loader changes in P1-05. Existing three reviewed libraries remain cached/pinned; no new security query/scanner/private upload. No Desk or external repository edit/run. P1-04 evidence **d0382d365815d233e4f28773995e2ef688f2a656** and closure 76d6d58d2cb19631a63aa7a587d713b3028d0e88 supply the [pure provider registry](PROVIDER_REGISTRY.md), source-pinned independent example and model-node preparation. Independent business IDs have no branch under engine src. P1-03 evidence b4b7707fc786c5ea5b68dd7f19fb03cb10f5b45d supplies the [IR2 model loader](MODEL_LOADING.md); P1-02 a653c105fba5ea6ada15395abc22cc8f82cc4472 supplies [types/parameters](TYPE_PARAMETER_CONTRACT.md). Preserve these completed slices and legacy IR1 behavior.

This plan/status/handoff-only closure records P1-05 COMPLETED. Remote main last read-confirmed at 0ac5d7b163e77bddb5f69689a1455e3560aad872; twelve local commits including closure are unpushed. Resolve closure SHA through Git. No push requested/attempted. No phase gate has passed.

## Next task: P1-06 — Indicator Function Slice

Read [P1 design](P1_STRATEGY_DESIGN.md), [Types](TYPE_PARAMETER_CONTRACT.md), [Provider registry](PROVIDER_REGISTRY.md), [Expressions](EXPRESSION_CONTRACT.md), [Extensibility](EXTENSIBILITY_CONTRACT.md), state/event contracts and plan acceptance. P1-05 is closed; mark P1-06 IN_PROGRESS before implementation.

- Register SMA, EMA, MACD and RSI as ordinary versioned functions with typed input/output/parameter descriptors; retain unknown-name provider extensibility and no indicator-specific trading-condition enum/compiler branch. SMA supports existing Desk visual migration; MACD/RSI must expose ordinary named outputs for later graph expressions.
- Evaluate working/output precision, overflow limits, formula rounding points/error tolerances and signal-boundary sensitivity using independent expected calculations. P1-02's primitive representation alone does not prove indicator accuracy. Record an evidenced technical decision before claiming numeric policy selected. New dependency/tool execution follows supply-chain instructions and existing approval scope.
- Implement declared seed/history/readiness behavior: SMA mean of current last p closes; EMA initial p-sample mean then alpha=2/(p+1); MACD fast<slow, line=fastEMA−slowEMA, signal mean-seeded EMA of ready line values, histogram=line−signal with no implicit ×2; RSI after p differences, Wilder smoothing, only gains=100/only losses=0/flat=50.
- P1-04 currently accepts only pure/Propagate/history=0 and rejects Indicator/Stateful/ProviderManaged. Extend the shared contract explicitly for the necessary instance-owned indicator profile; do not route stateful instances through repeated pure AST calls or globally shared mutable provider objects. Preserve pure descriptor canonical fixtures, existing scalar type behavior and exact identities. Unsupported lifecycle/snapshot/foreign execution remains explicit. P1-08 will advance subscriptions once per accepted event; P2 owns generalized state/persistence.
- Test independent constant/rising/falling/short inputs, warm-up/first-ready positions, EMA seeds, MACD first-ready sample, RSI flat/lossless/gainless edges, parameter relationships/numeric bounds, repeated instances and failure state retention. Implement G-007 independent SMA/cross fixture. No hidden future samples, account/order/simulator or UI expansion.
- P1-07 owns full loaded-graph/module compilation and X-001 compiled reuse; P1-08 owns market event runtime; P1-09–P1-11 own actual CLI/external caller/golden gate. Do not label the P1-06 library slice a complete loaded strategy or Desk integration.

Expected scope: scoped fqe-ir indicator/provider state interfaces and implementations, descriptor capability/identity evolution as justified, tests/fixtures/example, numeric evaluation and canonical progress/contracts/ADR. Protect unrelated work, LICENSE/history/private datasets/local-toolchain, external Desk/SDK repos and existing demo. No branch/worktree/reset/force-push. Update partial state/checks before interruption; evidence/closure commits required before P1-07.

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
python3 scripts/check_docs.py
git diff --check
```

Add current-task-specific checks. Record real commands/counts/platform/fixture IDs/limitations. Close under WORKFLOW with reviewed scoped evidence/closure commits; inspect remote main, publish only when authorized and independently verify receipt. Before interruption record actual dirty/staged/unpushed state and exact last error/pending steps.

## Product constraints

Desk historical backtesting/local simulation is first delivery; live trading has separate acceptance. Deliver FQE plus supported runtimes in existing Flutter Desk so customers install once. Standalone/offline FQE needs no Flutter/login/trading account; broker APIs stay in the independent Trading SDK. [Desk integration](DESK_INTEGRATION_PLAN.md) owns old visual/data/result migration, native paired runs and signed installed package acceptance. Preserve Desk dirty work and historical backend identity. No FQE/Desk native integration verified yet. Engine support and bundled platform/runtime support remain separate matrices.
