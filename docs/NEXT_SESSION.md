# Next Session — Repository Recovery

Updated 2026-11-14 (Asia/Tokyo); derived from [Project status](PROJECT_STATUS.md).

## Recover first

Read PROJECT_STATUS, DELIVERY_PLAN, ARCHITECTURE_DECISIONS, README, [Workflow](WORKFLOW.md), [authority](DOCUMENT_AUTHORITY.md), then current contracts. Verify Git status/main/upstream/history and real evidence; code/tests/Git override stale records. Main only, preserve unrelated work/history. User authorizes downstream implementation without routine confirmation; no publishing requested. Do not inherit old remote push receipts as authorization.

P1-01–P1-09 COMPLETED; P1-10 READY. P1 gate and P2–P6 remain incomplete. Resolve actual HEAD with `git rev-parse HEAD`, latest status record with `git log -1 --format=%H -- docs/PROJECT_STATUS.md`. Reuse the existing pinned local toolchain; no install needed.

## Latest delivered behavior and checks

P1-09 implementation evidence **9dbac2b14e85298c18131646219a178a4ef8ddea**; COMPLETED with synchronized status/plan/handoff closure. Local only; no push requested/performed.

P1-09 [CLI execution](CLI_EXECUTION.md)/ADR-020: validate compiles loaded event plans/typed parameters without evaluating data; run reads a bounded fqe.market-data/1 file and delegates to the shared EventRuntime/report adapter. Strict fqe.validation-result/fqe.run-result/fqe.diagnostic schema1 JSON, full-width string integers, raw model/parameter/data SHA-256 provenance, explicit reference catalog and deterministic results. CLI code owns options/local regular-file IO/exits, not financial decisions. Whole batch result is buffered up to4MiB and withheld on any validation/execution/encoding failure. The underlying streaming retry API and legacy demo behavior remain unchanged.

175 behavioral tests (146 prior +12 exchange +17 actual-binary CLI), fmt/check/locked-offline workspace tests/strict all-target Clippy PASS on macOS ARM64, Rust/Cargo1.99.0. CLI demo, all six fqe-ir examples, fqe-runtime events and offline CLI build PASS. Seven actual binary cases executed twice with PATH=/nonexistent: G-002 default/60→signalbar5,90 and changed model default90→none, G-007→bar5, full-width ticks/filtering and explicit empty events. Independent stdlib JSON/SHA checks and library/CLI byte parity PASS. Strict malformed/unknown/bounds/failure/no partial stdout, Unicode paths and output pipe errors covered. Document checker35 Markdown/383 local links/42 tasks and diff checks PASS; no outstanding failure.

No dependencies/manifests/lockfile/toolchain changes, no scanners/new public advisory queries/private uploads. Old provider source hashes/canonical goldens preserved; no external repo/Desk/SDK/accounting/worker work. Prior evidence: P1-08 `cea3f92df2027743711e73a84927396417689042`, P1-07 `c9a097e9a98ef91c3dd5c0f15df9639a72748260`, P1-06 `e772bf0a44f35a5b457cbe10c02eb8cc16689003`, P1-05 `424aacb9faf5d081b72bf0a82f26ef396de16c78`, P1-04 `d0382d365815d233e4f28773995e2ef688f2a656`, P1-03 `b4b7707fc786c5ea5b68dd7f19fb03cb10f5b45d`, P1-02 `a653c105fba5ea6ada15395abc22cc8f82cc4472`.

## Next implementation: P1-10 — Minimal External Invocation

P1-09 is closed. Mark P1-10 IN_PROGRESS before implementation. Read [CLI exchange](CLI_EXECUTION.md), [compiled events](EVENT_EXECUTION.md), [compiler](GRAPH_COMPILER.md), [events](EVENT_CONTRACT.md), plan acceptance, [ADRs](ARCHITECTURE_DECISIONS.md) and actual CLI/report sources.

- Deliver a separately runnable independent caller fixture that supplies model/parameter/data through the CLI process boundary and parses the versioned result. Existing P1-09 tests already spawn the executable; re-labeling them alone does not complete P1-10. A Python-stdlib reference caller is a possible implementation, with no third-party packages or runtime dependency for the Rust engine itself.
- Define/document exact caller request/config and result/diagnostic validation, full-width integer handling, model/run/source identities and success criteria. Require exit0 plus a complete supported result, reject malformed/unknown-version/truncated/unexpected output and never interpret a partial prefix as successful execution. Do not add another financial evaluator.
- Verify actual external caller results equal CLI/library and independent G-002/G-007 goldens for changed parameters, source provenance and full-width values. Cover invalid requests/files/options/models/data, process nonzero/malformed stdout, bounded IO, timeout and child process cleanup appropriate to a one-shot subprocess. No shell interpolation, login/network or worker transport required.
- Explain that this smoke boundary does not choose stable FFI/IPC, package Desk, implement language workers or deliver runtime scheduling/cancellation. P4 owns actual native integration/transport and P5 language execution; P1-11 still owns the complete P1 gate.

Expected scope: independent caller/example and request contract, subprocess success/failure/cleanup tests/fixtures, relevant CLI docs/ADR/status/plan/handoff. Reuse the CLI/report semantics; avoid unrelated core/event/indicator changes, new dependencies and external repository writes. Preserve provider source pins and old canonical/golden fixtures. No branch/worktree/reset/force-push.

## Interface and numeric cautions

New CLI: `validate --model PATH [--params PATH]`; `run --model PATH --data PATH [--params PATH] [--run-id ID]`, run ID defaults cli. New-command exits0/2/3/4/5; stderr diagnostic stages/paths are structured, fixed messages avoid raw tokens/OS paths. Legacy demo retains text/exit1 on error. Input file cap262144 bytes and parser depth32/items16384/collection1024/string4096; result cap4MiB before newline. Empty events array is explicit valid completed0; empty/truncated files/records fail. Output IO failure may leave an OS-written prefix; consumers must check exit and complete JSON. Reference catalog is not arbitrary plugin discovery; use the same catalog for semantic ID parity.

Event profile: at most32 graph-input/incremental-output slots, bounded4096/32 AST expansion, one instrument/continuous fixed timeframe. Bar OHLC Decimal Price scale0–12/abs≤1e12 including zero/negative; nonnegative Quantity volume scale0–18. Tick U64 caller-scaled price must not silently become Decimal. Warmup firstN matched events or any unready incremental output suppresses signals. Missing/Warmup input resets state; missing crossing resets prior pair. Indicator work18/output12 HalfEven, periods≤10000; RSI nonzero mean movement≥1e-6 or precision error, Ratio0–100. Exact quantized comparison and trusted deep fork remain; no snapshots/ledger/isolation/performance claim.

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
cargo build --locked --offline -p fqe-cli
./target/debug/fqe validate --model crates/fqe-ir/tests/fixtures/g002-events.json
./target/debug/fqe run --model crates/fqe-ir/tests/fixtures/g002-events.json --data crates/fqe-cli/tests/fixtures/g002-data.json --params crates/fqe-cli/tests/fixtures/parameters-60.json --run-id acceptance
python3 scripts/check_docs.py
git diff --check
```

Record last actual partial state/check failures before interruption. Evidence commit first, then three-document closure with its SHA; no self-hash. Publishing only when authorized, normal push and confirmed remote SHA, never force-push. Last remote main read-confirmed `0ac5d7b163e77bddb5f69689a1455e3560aad872`; verify live before publishing.

Local main is twenty commits ahead of last read-confirmed remote after this closure. Verify live Git state/count and any newer dirty work before proceeding.
