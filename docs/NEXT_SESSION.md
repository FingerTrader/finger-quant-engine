# Next Session — Repository Recovery

Updated 2026-11-14 (Asia/Tokyo), derived from [Project status](PROJECT_STATUS.md).

## Recover first

Read PROJECT_STATUS, DELIVERY_PLAN, ARCHITECTURE_DECISIONS, README, [Workflow](WORKFLOW.md), [authority](DOCUMENT_AUTHORITY.md) and current contracts. Verify actual root/Git status/branch/upstream/history/evidence. Main only, preserve unrelated work. Actual code/tests/Git override stale records. User authorized continuing downstream tasks without routine confirmation; no GitHub publishing requested. Do not install tools just because default PATH lacks Cargo.

P1-01–P1-04 COMPLETED. P1-05 READY at this closure; recover any newer dirty partial implementation before starting. P1 remains incomplete. `git log -1 --format=%H -- docs/PROJECT_STATUS.md` resolves latest progress record; `git rev-parse HEAD` resolves checkout.

## Latest delivered evidence

P1-04 evidence `d0382d365815d233e4f28773995e2ef688f2a656`: registry.rs/builtins.rs, independent examples/support/providers.rs, providers example, registry tests and tracked source-pinned model/canonical fixtures. [Provider registry](PROVIDER_REGISTRY.md)/ADR-015 own descriptor/call/identity/resource semantics. Immutable pure scalar descriptors and frozen exact-pin registry, validated named multi-input/output invocation, parameter relationship validator and model-node origin pin are implemented. Indicator/stateful/provider-managed readiness, foreign execution, compiler/scheduler and Desk integration are future work. X-001 has only its P1-04 in-process portion; full compiled/event/rule proof remains later gates.

macOS ARM64, existing rustc 1.99.0: required fmt/check/locked offline workspace tests/strict all-target Clippy PASS. **69 behavioral tests = 56 prior + 13 registry**, zero failed/ignored/doc tests. CLI demo, parameter example, real-file loader and two provider compositions PASS. Independent expected descriptor bytes/SHA match. Model variants produce numeric 200/50 then true/false and false/true; registry identity eff2b19c3e3a8405db8828ec8dc14a394b206d0b0255983f692064c3c3d130af. Docs check 29 Markdown/267 links/42 tasks and diff checks PASS. No dependency/lockfile/toolchain/core/runtime/CLI/protocol/LICENSE changes; only crate-private loader canonical helpers exposed. No provider-business-ID branch in engine src. No Desk edit/run.

P1-03 evidence b4b7707fc786c5ea5b68dd7f19fb03cb10f5b45d implements the separate [IR2 model loader](MODEL_LOADING.md); P1-02 evidence a653c105fba5ea6ada15395abc22cc8f82cc4472 implements [types/parameters](TYPE_PARAMETER_CONTRACT.md). Legacy IR1 and examples remain preserved. Do not reimplement completed slices.

This status/handoff/plan-only closure records the P1-04 evidence. Remote main read-confirmed at 0ac5d7b163e77bddb5f69689a1455e3560aad872; ten local commits including closure are unpushed. Resolve exact local closure SHA through Git. No push attempted/requested. The three existing reviewed libraries and 21 locked registry packages remain unchanged; use cached locked/offline dependencies, no private uploads or new advisory checks in P1-04.

## Next task: P1-05 — Universal Expression Engine

Read [P1 design](P1_STRATEGY_DESIGN.md), [Type/parameters](TYPE_PARAMETER_CONTRACT.md), [Provider registry](PROVIDER_REGISTRY.md), [Extensibility](EXTENSIBILITY_CONTRACT.md), [State](STATE_CONTRACT.md) and plan acceptance. Implement the bounded typed expression/evaluation slice without a whole-graph compiler or financial/runtime expansion.

- Arithmetic, comparison, Boolean composition, generic registered function/predicate calls and crossing operators with stable diagnostics. Exact type/unit/scale rules; explicit overflow/division/rounding and unsupported dimensional operations. No per-name function/predicate branches.
- Pin and test Missing/Warmup/Error/ready Boolean truth tables, short-circuit ordering and numeric failures. A false guard must not suppress subscribed crossing/history advancement; shared node references advance once. Skipped pure arithmetic branches do not execute. Node-history errors fail even when a Boolean guard would hide them.
- Cross-above: previous ready left≤right and current left>right; cross-below is symmetric. No first-ready cross. Declare reset/gap behavior. Keep per-instance history separate from immutable expressions. Stage changes, publish outputs/history only after successful evaluation, retain prior history on failure. Full event scheduling/indicator state still belongs to P1-08/P2.
- Test truth tables, external unknown-name predicates, false guards/operand reordering/shared references, exact equality and crossing edges, non-ready gaps/reset, arithmetic overflow/division and units, independent repeated instances and failed evaluation rollback.
- Update technical decisions/contracts/plan/status/handoff and run all required checks/examples; record evidence/closure before P1-06. P1-06 owns indicator working precision/formulas, P1-07 graph/module compilation, P1-08 event runtime. Do not bypass them.

Expected scope: fqe-ir expression/evaluator/history modules plus tests/example and canonical docs. Reuse P1-02 numeric primitives and P1-04 registry. No new dependency planned. No worker/plugin/accounting/simulator/Flutter/Trading SDK or external repository change. Protect LICENSE/history/private data/local-toolchain and unrelated work; no branch/worktree/reset/force-push.

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
python3 scripts/check_docs.py
git diff --check
```

Add current-task acceptance commands and record actual outputs/counts. Close under WORKFLOW with scoped evidence/closure commits; check remote main, publish only when authorized and verify receipt. Before interruption record actual dirty/staged/unpushed files, last results/errors and exact remaining work.

## Product constraints

First product delivery is Desk historical backtesting/local simulation, then separately accepted live trading. FQE and supported runtimes must ship within existing Flutter Desk so customers install once. Standalone/offline engine needs no Flutter/login/trading account; broker APIs remain in the independent Trading SDK. [Desk integration](DESK_INTEGRATION_PLAN.md) owns migration, paired native acceptance and installed bundle proof. Preserve existing Desk dirty work, old drafts/data/historical backend identity. No native FQE/Desk run verified yet. Engine compatibility and bundled platform/runtime availability are separate matrices; no universal-language/distribution promise from an in-process Rust trait.
