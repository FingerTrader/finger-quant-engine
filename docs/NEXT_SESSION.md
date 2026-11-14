# Next Session — Repository Recovery

Updated 2026-11-14 (Asia/Tokyo); derived from [Project status](PROJECT_STATUS.md).

## Recover first

Read PROJECT_STATUS, DELIVERY_PLAN, ARCHITECTURE_DECISIONS, README, [Workflow](WORKFLOW.md), [authority](DOCUMENT_AUTHORITY.md) and current task contracts. Verify root/Git status/branch/upstream/history/evidence before editing. Main only; preserve unrelated work and all history. Actual code/tests/Git override stale prose. User authorized continuing downstream tasks without routine confirmation. No publishing requested; do not inherit a historical push receipt.

P1-01–P1-06 COMPLETED; P1-07 VERIFIED awaiting evidence/closure commit. P1-08 becomes READY only after that closure. P1/P2–P6 gates remain incomplete. Recover newer dirty work before starting. Resolve latest record with `git log -1 --format=%H -- docs/PROJECT_STATUS.md`, actual checkout with `git rev-parse HEAD`. No toolchain installation needed.

## Latest delivered evidence

P1-07 artifacts implemented and fully verified; evidence SHA pending. [Graph compiler contract](GRAPH_COMPILER.md) and ADR-018 own supported import/module/plan/identity/source-map rules. Shared compiler accepts existing fqe.model/1 and separate explicit fqe.graph/1, with typed public inputs and inline pure modules. Both produce exact-pinned deterministic topology, expanded scope-safe path vectors, immutable parameters and source maps. PreparedIncremental validates parameters without factory creation. Pure plans optionally map to existing P1-05 AST; incremental plans are rejected by that pure path.

macOS ARM64, rustc/Cargo 1.99.0: fmt/check/locked offline workspace tests/strict all-target Clippy PASS. **119 behavioral tests = 96 prior + 23 compiler**, zero failed/ignored/doc tests. CLI demo and all six fqe-ir examples PASS. Both original X-001 models and nested/reused multi-output module variants compile and execute through generic pure projection. Sum first=true/second=false/inverted=false; difference first=false/second=false/inverted=true. Independent graph canonical fixture/hash, stable order/identity, parameter/default/constraints/relationship failures, types/units/scales, cycles/recursion, original/nested source diagnostics, alias/node/depth/visit and AST expansion limits pass. Counters prove zero compile-time state factory calls; pure modules reject hidden incremental providers. Doc checker passed 33 Markdown/42 tasks; link count in tool evidence. Diff/protected-path checks PASS.

No dependency/manifest/lockfile/toolchain/LICENSE/core/runtime/CLI/protocol/model-declaration/pure built-in/indicator/expression implementation changes. Existing three reviewed libraries remain cached/pinned; no new security query/scanner/private upload. Loading gains original noncanonical node-source locations and crate-private shared parser helpers, preserving old syntax/bytes/identity. No Desk/SDK or external repository edit/run. Previous implementation evidence: P1-06 `e772bf0a44f35a5b457cbe10c02eb8cc16689003`, P1-05 `424aacb9faf5d081b72bf0a82f26ef396de16c78`, P1-04 `d0382d365815d233e4f28773995e2ef688f2a656`, P1-03 `b4b7707fc786c5ea5b68dd7f19fb03cb10f5b45d`, P1-02 `a653c105fba5ea6ada15395abc22cc8f82cc4472`. Preserve these slices and independent identity fixtures.

No push requested/attempted. Remote main read-confirmed at `0ac5d7b163e77bddb5f69689a1455e3560aad872`; resolve local ahead count from Git. No functional phase gate passed. Finish P1-07 evidence/closure bookkeeping before the next implementation.

## Next task: P1-08 — Basic Universal Event Evaluation

Read [P1 design](P1_STRATEGY_DESIGN.md), [event closed-bar target](EVENT_CONTRACT.md#p1-closed-bar-target), [state](STATE_CONTRACT.md), [runtime](RUNTIME.md), [graph compiler](GRAPH_COMPILER.md), [expressions](EXPRESSION_CONTRACT.md), [indicators](INDICATOR_CONTRACT.md), provider/type/extensibility contracts and plan acceptance. Mark P1-08 IN_PROGRESS after P1-07 closure.

- Build a real compiled-plan run instance with independent per-node state, run/event identity, cursor/readiness/output provenance and actual accepted-event processing. Preserve legacy tick mode semantics and distinct declared trigger families; no relabeling ticks as closed OHLC bars.
- Implement the minimum closed-bar contract: one bound instrument/fixed positive duration, UTC ns/open-inclusive close-exclusive interval, event timestamp=close, typed compatible OHLC/volume, OHLC ordering/nonnegative volume, closed-only visibility, strictly increasing sequences, continuous nonoverlapping bars with next open=prior close. Reject duplicate/gapped/unclosed/mixed-domain/invalid bars before changing history. Define price/volume numeric domain bounds explicitly and retain exact units/scales. No calendar inference, bar synthesis or future reads.
- Stage ALL subscribed incremental/crossing node state once per matching accepted event in stable dependency order, independent of Boolean guards/shared references. Numeric/provider/input/output/cross/root failures commit no candidate state, cursor or signals; failed sequence can be retried if the chosen public lifecycle permits it. Fresh runs isolate state. PreparedIncremental::instantiate currently creates single-node instances; IncrementalInstance::step commits one node and cannot by itself implement an atomic whole graph. Add an explicit staged/fork boundary as needed without sharing mutable state through registry handles.
- Integrate typed expression/crossing roots into the compiled event plan with short-circuit/readiness semantics from P1-05. Current graph format exposes provider/module targets and pure-call projection, **not** standalone operator/cross AST serialization. Choose explicit bounded/versioned encoding or another documented generic mapping needed by loaded MACD/RSI strategy acceptance; do not add per-indicator compiler/runtime branches or silently reinterpret old artifacts. Pure calls may repeat in optional AST projection; that is not the once-per-event stateful subscription implementation. Preserve false-guard, ready-operand-reorder, shared-reference, missing reset and failure rollback tests.
- Bind graph inputs to declared event-visible fields explicitly; graph public inputs are not already bound to a price feed. Signals/observations link to accepted source sequence/time/run/artifact. Warmup advances memory without producing trade intents or performance. No order/account/ledger/simulator. Reject unsupported event-domain/state/timer/workflow nodes; P2 owns generalized scheduling/persistence.
- Test deterministic reruns/prefix invariance, multiple isolated runs, bar/tick routing, first-ready/closed/gapped/OHLC/unit/time failures, global rollback/retry after a later node fails, shared eager state updates behind false guards, independent G-002/G-007 and source-linked decisions. P1-09 owns actual CLI model/data execution; P1-10 external caller; P1-11 full gate. Do not mark those complete from library/example tests.

Expected scope: fqe-runtime compiled execution and any scoped core event/IR expression/staging interfaces needed, tests/fixtures/example, canonical event/runtime/state/compiler contracts/ADR/progress/handoff. No Desk/SDK, workers/plugins, accounting/simulator, toolchain or unnecessary dependencies. Protect unrelated work, historical data/identities/LICENSE/history/private datasets/local-toolchain. No branch/worktree/reset/force-push. Record partial state/checks before interruption; evidence/closure commits required before P1-09.

## Numeric and state cautions

P1-06 work18/output12 HalfEven, abs(close)≤1e12, periods≤10000; RSI nonzero mean gain+loss must be ≥1e-6 input units, otherwise registry_precision with no committed single-node step. RSI Ratio is 0–100. SMA/EMA/MACD preserve Scalar/Price units. History declares maximum initial seed samples (SMA/EMA10000, RSI10001, MACD19999), not a finite recurrence horizon/snapshot layout. Runtime decisions use exact quantized values, not fixture tolerances. Source pins are not binary attestations.

IncrementalInstance is atomic for one node only. Missing/Warmup input resets fresh history, Error/failure retains it; logical sequence-number gaps alone do not reset that API. The stricter P1 closed-bar continuity validation is separate. Deep fork correctness is trusted Rust behavior, not isolation/quotas. General snapshots/external providers remain future gates.

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
cargo run --locked --offline -p fqe-ir --example compile
python3 scripts/check_docs.py
git diff --check
```

Add current-task acceptance commands. Never regenerate goldens during tests. Record real counts/results/platform/identities/limitations. Close under WORKFLOW with reviewed scoped evidence/closure commits; inspect remote main, publish only when authorized and verify receipt. Before interruption record dirty/staged/unpushed state and exact last failure/pending steps.

## Product constraints

Desk historical backtesting/local simulation is first delivery; live trading has separate acceptance. Deliver FQE plus supported runtimes in existing Flutter Desk so customers install once. Standalone/offline FQE needs no Flutter/login/trading account; broker APIs stay in the independent Trading SDK. [Desk integration](DESK_INTEGRATION_PLAN.md) owns migration, native paired runs and installed package acceptance. Preserve Desk dirty work/historical backend identity. No FQE/Desk native integration verified yet. Engine conformance and bundled platform/runtime support remain separate matrices.
