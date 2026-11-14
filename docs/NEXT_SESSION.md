# Next Session — Repository Recovery

Updated 2026-11-14 (Asia/Tokyo); derived from [Project status](PROJECT_STATUS.md).

## Recover first

Read PROJECT_STATUS, DELIVERY_PLAN, ARCHITECTURE_DECISIONS, README, [Workflow](WORKFLOW.md), [authority](DOCUMENT_AUTHORITY.md), then task contracts. Verify root, Git status/main/upstream/history and actual evidence; code/tests/history override stale records. Work on main, preserve all unrelated changes/history. User authorizes continued work without routine confirmation. No publishing requested; historical remote receipts are not authorization for new pushes.

P1-01–P1-07 COMPLETED; P1-08 VERIFIED awaiting evidence/closure. P1/P2–P6 gates remain incomplete. Resolve actual HEAD with `git rev-parse HEAD`, latest record with `git log -1 --format=%H -- docs/PROJECT_STATUS.md`. No toolchain installation needed.

## Latest delivered behavior

P1-08 implements [compiled event execution](EVENT_EXECUTION.md)/ADR-019: strict bounded fqe.event-plan/1 wrapper over fqe.graph/1, closed-bar/tick triggers, exact event field bindings, portable typed operator/cross ASTs, immutable compiled identity and per-run fresh state. Every incremental node advances once in compiled dependency order, then all crossing subscriptions run before lazy roots. Deep candidates and all cursors/signals publish only after whole-event success; rejection leaves prior state untouched and permits explicit same-sequence retry. The original Runtime/demo still fails one-shot runs on ordering errors.

146 behavioral tests (119 previous +9 format +18 runtime), fmt/check/locked-offline workspace tests/strict all-target Clippy PASS on macOS ARM64, pinned Rust/Cargo1.99.0. CLI demo, all six fqe-ir examples and fqe-runtime events example PASS. Loaded G-002 MACD/RSI and G-007 SMA signal only at bar5; threshold90 gives no G-002 signal. Bar/domain/unit/time/warmup/gap negatives, prefix/isolation, false guards/reordering/shared references, zero-input unknown providers, missing resets, later-node/root/fork failure rollback/retry, pure lazy errors and unrelated tick filtering covered. Canonical tick event ID `sha256:51796d4b85cc3d682d767416978cdbb9fce2a6f2fc81036232565fdf93c0bc5e` independently generated with Python stdlib. Factory counters prove compile/invalid run start creates no state.

No new dependencies/manifests/Cargo.lock/toolchain changes; reuse approved cached libraries. No new scanners/network advisory queries/private upload. No Desk/SDK/external repo work. Old canonical fixtures and provider source pins unchanged. Prior evidence: P1-07 `c9a097e9a98ef91c3dd5c0f15df9639a72748260`, P1-06 `e772bf0a44f35a5b457cbe10c02eb8cc16689003`, P1-05 `424aacb9faf5d081b72bf0a82f26ef396de16c78`, P1-04 `d0382d365815d233e4f28773995e2ef688f2a656`, P1-03 `b4b7707fc786c5ea5b68dd7f19fb03cb10f5b45d`, P1-02 `a653c105fba5ea6ada15395abc22cc8f82cc4472`.

## Next implementation: P1-09 — CLI Model Execution

First finish P1-08 evidence and three-document closure commits; only then mark P1-09 READY/IN_PROGRESS. Read [event execution](EVENT_EXECUTION.md), [compiler](GRAPH_COMPILER.md), [model loading](MODEL_LOADING.md), [events](EVENT_CONTRACT.md), [expressions](EXPRESSION_CONTRACT.md), [indicators](INDICATOR_CONTRACT.md), [CLI source](../crates/fqe-cli/src/main.rs), plan acceptance and source.

- Add real validate/run commands accepting local event model, parameter bindings and bounded synthetic tick/bar data files. Reuse immutable loader/compiler/registry/EventRuntime; CLI must not contain handwritten MACD/RSI/strategy conditions. Preserve existing demo/exits unless a documented versioned CLI change is justified.
- Design explicit versioned, exact integer/typed input and structured result/diagnostic codecs. Validate unknown/duplicate fields, file/record/line/count/output limits, input order/readiness and source identity; do not silently accept malformed/truncated/empty records or emit private source tokens. Choose bounded IO/results and explicit failure behavior, keeping warmup/source/run/artifact provenance.
- Run actual binary on G-002 and G-007 local fixtures. Changed artifact/threshold must change decisions without rebuilding; threshold60 yields G-002 signalbar5, threshold90 none. Test validate vs run behavior, exact exits/stdout/stderr, missing/invalid files/models/bindings/providers/types/data/sequence/time and offline/no login operation.
- Keep SDK/accounts/orders/ledger/simulation, workers, language installation and Desk out of this CLI slice. P1-10 owns independent caller and P1-11 full gate. Reusing cached serde/JSON through a public scoped IR codec may avoid new dependencies; inspect actual manifests and supply-chain requirements before choosing.

Expected scope: fqe-cli commands and any scoped public loading/result codec API required, local synthetic fixtures, subprocess acceptance tests, CLI/examples/contracts/ADR/status/plan/handoff. Do not edit provider business sources/old canonical goldens just to make new fixtures pass. No checkout/branch/worktree/reset/force-push or external repo writes.

## Numeric and state cautions

Event profile: at most32 total graph-input/incremental-output slots, bounded4096/32 AST expansion, one instrument/fixed continuous timeframe, exact price/volume units/scales. Missing/Warmup input resets node state, missing crossing resets prior pair. Declared warmup firstN matched events and any unready incremental output suppress signals. Caller-supplied dataset ID is not content attestation; new CLI data codec must make source provenance concrete. Tick input is U64 caller-scaled ticks; bar OHLC Decimal Price scale0–12/abs≤1e12 (zero/negative allowed), volume Decimal Quantity scale0–18 nonnegative. Do not silently convert between tick/bar price units.

Indicator work18/output12 HalfEven, periods≤10000; RSI nonzero mean gain+loss≥1e-6 input units or precision error. RSI Ratio0–100; other indicators retain Scalar/Price. History declares initial seed samples, not finite influence or snapshot layouts. Runtime compares exact quantized values. Trusted deep forks/purity are not isolation or crash recovery; no snapshots/ledger or performance claim exists.

## Commands

```sh
export CARGO_HOME="$PWD/.local-toolchain/cargo"
export RUSTUP_HOME="$PWD/.local-toolchain/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
export RUSTUP_AUTO_INSTALL=0
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
cargo run --locked --offline -p fqe-runtime --example events
python3 scripts/check_docs.py
git diff --check
```

Record last actual state/check failures before interruption. At task closure, evidence commit first, then status/plan/handoff closure naming its SHA. Push only when authorized; confirm remote with ls-remote and never force-push. Last remote main read-confirmed `0ac5d7b163e77bddb5f69689a1455e3560aad872`; recheck before publishing.
