# Next Session — Repository Recovery

Updated2026-11-14 (Asia/Tokyo); derived from [Project status](PROJECT_STATUS.md).

## Recover first

Read PROJECT_STATUS, DELIVERY_PLAN, ARCHITECTURE_DECISIONS, README, [Workflow](WORKFLOW.md), [authority](DOCUMENT_AUTHORITY.md), then current contracts. Verify Git status/main/upstream/history and real evidence; code/tests/Git override stale records. Main only, preserve unrelated work/history. User authorizes downstream implementation without routine confirmation; no publishing requested. Do not inherit old push receipts as authorization.

P1-01–P1-10 COMPLETED; P1-11 READY; P1 gate and P2–P6 remain incomplete. Resolve actual HEAD with `git rev-parse HEAD`, latest status record with `git log -1 --format=%H -- docs/PROJECT_STATUS.md`. Reuse the existing pinned local toolchain; no installation needed.

## Latest delivered behavior and checks

P1-10 implementation evidence **5c7a7f6cdd1b0fe59bda7d740444da4f5299dcc3**, COMPLETED with synchronized local closure. No push requested/performed.

P1-10 [external invocation](EXTERNAL_INVOCATION.md)/ADR-021: independent stdlib caller/seven request fixtures, strict versioned request/response and complete CLI result/diagnostic parsing, exact integer/type/readiness/source validation, explicit trusted executable without shell, bounded POSIX dual-pipe capture/deadline/group cleanup/direct-child reap. Consumer imports no engine modules and computes no financial expressions. Engine CLI/shared EventRuntime remains authoritative. Python is optional reference/test tooling; no future customer-runtime-download requirement is introduced.

Verification2026-11-14 macOS ARM64, Rust/Cargo1.99.0, Python3.14.6:175 Rust behavioral tests +23 independent external tests PASS; fmt/check/strict all-target Clippy/locked-offline build, CLI demo/all six fqe-ir examples/runtime events PASS. Full JSON parity with actual CLI and existing CLI/library byte parity, independent G-002/G-007 bar5 goldens, threshold/artifact changes without rebuild, u64::MAX/minimum i64/i128/exact units/text, deterministic reruns and empty0 PASS. Strict request/source/result/error/limits, actual source mutation, floods, timeout/closed/inherited pipes, child cleanup and broken caller output-pipe checks PASS. Docs checker36 Markdown/42 tasks and local links/anchors/status dependencies PASS; diff PASS. No remaining test failure. Initial short-deadline and test-readiness-assumption failures are recorded/resolved in status.

No dependency/manifests/lockfile/toolchain/Rust/provider/golden changes. Existing approved cached dependencies only; no new scanner/advisory query/installer/private upload. Source pins remain unchanged. No Desk/SDK/worker/accounting/external repository work. Trusted local POSIX reference supervision is not sandboxing, stable FFI/IPC, Windows packaging, graceful runtime cancellation or installed Desk acceptance.

Previous evidence: P1-09 `9dbac2b14e85298c18131646219a178a4ef8ddea`, P1-08 `cea3f92df2027743711e73a84927396417689042`, P1-07 `c9a097e9a98ef91c3dd5c0f15df9639a72748260`, P1-06 `e772bf0a44f35a5b457cbe10c02eb8cc16689003`. Do not reimplement these verified tasks.

## Next implementation: P1-11 — Golden Suite and P1 Gate

P1-10 is closed. First eligible task is P1-11; mark IN_PROGRESS and state scope/acceptance before editing. Read [P1 golden specifications](P1_STRATEGY_DESIGN.md#example-strategies-and-golden-fixtures), [extensibility](EXTENSIBILITY_CONTRACT.md), [provider registry](PROVIDER_REGISTRY.md), [compiler](GRAPH_COMPILER.md), [compiled events](EVENT_EXECUTION.md), [CLI exchange](CLI_EXECUTION.md), [external invocation](EXTERNAL_INVOCATION.md), plan acceptance and actual tests/artifacts.

- Inventory existing versioned fixtures and valid/invalid evidence before adding code. Preserve G-001–G-007 definitions; G-006 remains P2-06, not a reason to implement workflow/state in this slice. G-003/G-004 loaded MACD/RSI and external parity already have substantial real evidence, but a complete gate needs an explicit consolidated report.
- Prove X-001 with independently defined reviewed unknown-name typed provider/predicate and reusable rule compositions through generic public graph/event APIs, valid and invalid versions/signatures/units. No per-ID core/compiler/UI branches. Existing provider examples/import tests alone must not be relabeled as complete loaded generic gate acceptance.
- Deliver versioned valid/invalid golden artifacts/data/expected results and independent calculation/source notes; expected files must not regenerate themselves during tests. Include G-007, changed model/parameters, loaded validation failures with no execution/state mutation, repeat determinism and source identity. Reuse current financial goldens/reference generators when applicable.
- Consolidate library/CLI/external parity within each supported catalog; the CLI reference catalog does not dynamically load arbitrary test providers. An unknown-name in-process gate must state its public custom registry; do not add a provider-specific CLI branch or pretend to implement an external plugin loader.
- Run all four Rust checks,23 external tests and docs/diff checks, write dated gate/limitations evidence, then close only when all P1 acceptance is actually met. No P2/Desk/worker/backtest/simulation completion from documentation alone.

Expected P1-11 scope: consolidated independent goldens and test/acceptance report, minimal generic fixes only if actual tests reveal a defect, synchronized contracts/plan/status/handoff. Preserve indicator/builtin/test-provider source pins and prior canonical fixtures unless a concrete reviewed defect requires an explicit identity update. No new dependencies, generators, external repository writes, branch/worktree/reset/force-push.

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
cargo build --locked --offline -p fqe-cli
python3 examples/external/caller.py --engine target/debug/fqe --request examples/external/requests/g002.json
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/external -v
python3 scripts/check_docs.py
git diff --check
```

Record actual partial state/check failures before interruption. Evidence commit first, then three-document closure with its SHA; no self-hash. Publishing only when authorized, normal push and confirmed remote SHA. Last remote main independently read-confirmed `0ac5d7b163e77bddb5f69689a1455e3560aad872`; verify live Git state/count and newer dirty work before proceeding. Latest implementation evidence `5c7a7f6cdd1b0fe59bda7d740444da4f5299dcc3`. Local main is twenty-two commits ahead of that remote after this closure; verify the live count and actual HEAD. P1-10 work is committed locally; no publishing requested/performed.
