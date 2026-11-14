# Project Status

**Unique authoritative progress ledger.** Updated 2026-11-14 (Asia/Tokyo). Read [Next session](NEXT_SESSION.md), [Delivery plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md) and current contracts before editing. Actual code/tests/Git override stale records; authority rules: [Document authority](DOCUMENT_AUTHORITY.md).

## Current position

- Phase: **P1 — Universal Strategy Foundation, incomplete**. P2–P6 NOT_STARTED. No functional phase gate has passed.
- Current work: GOV-01 architecture governance and P1-01 baseline audit, both VERIFIED pending evidence commit/closure and confirmed push for GOV-01.
- Next eligible implementation task after P1-01 closure: **P1-02 — Universal Types & Parameter Schema**.
- Actual root: `/Users/george/Desktop/finger-quant-engine`; main tracking origin/main; remote `https://github.com/FingerTrader/finger-quant-engine.git`.
- Rust source baseline / latest pre-governance commit: `342d8aa76b48d32fa9c9110609b371eac52b8d86` (foundation). Previous commit: `cbcebad`.
- Latest status-record commit: resolve `git log -1 --format=%H -- docs/PROJECT_STATUS.md`. Current checkout: `git rev-parse HEAD`. See the non-self-referential recording rule in DOCUMENT_AUTHORITY.
- Governance/audit evidence commit: pending. GitHub delivery of new governance documents: pending; initial remote main matched baseline.

## Task ledger

| Task(s) | Status | Evidence / next condition |
| --- | --- | --- |
| GOV-01 | VERIFIED | Eight governance docs plus baseline report, plan/navigation/recovery updates; all checks passed; evidence commit and confirmed push pending |
| P1-01 | VERIFIED | [Baseline audit](BASELINE_AUDIT.md): full inventory, 17 tests and gap/task mapping; document checks passed; recorded evidence commit pending |
| P1-02 | NOT_STARTED | Becomes READY when P1-01 is COMPLETED; start numeric/type/parameter slice |
| P1-03, P1-04, P1-05, P1-06, P1-07, P1-08, P1-09, P1-10, P1-11 | NOT_STARTED | Dependencies in DELIVERY_PLAN |
| P2-01, P2-02, P2-03, P2-04, P2-05, P2-06 | NOT_STARTED | P1 gate first |
| P3-01, P3-02, P3-03, P3-04, P3-05, P3-06, P3-07 | NOT_STARTED | P2 gate first |
| P4-01, P4-02, P4-03, P4-04, P4-05 | NOT_STARTED | P3 gate first |
| P5-01, P5-02, P5-03, P5-04, P5-05 | NOT_STARTED | P4 gate first; Python → Java → Go |
| P6-01, P6-02, P6-03, P6-04, P6-05, P6-06 | NOT_STARTED | P5 gate first |

No functional task is currently IN_PROGRESS or BLOCKED. Open technical choices are future work, not evidence that a ready task cannot begin. No P1 implementation gap is closed by planning it.

## Delivered baseline and limitations

Five Rust crates exist with workspace-only dependencies: validated domain values, experimental price-only IR, synchronous one-shot runtime, protocol version-number helper/draft schema and offline CLI demo. These predate the six-stage plan and remain preserved. Technical maturity: implemented but experimental, not stable/public SDK release.

No universal graph/parameter/indicator/compiler/model loader, generalized state scheduler/snapshot, backtester/virtual ledger/optimizer, Desktop boundary, SDK adapter, language worker or plugin runtime exists. Signals are observations, not orders/fills/performance. The `.proto` is not a codec or transport. [Audit](BASELINE_AUDIT.md) links exact sources/tests and every gap.

## Actual verification evidence

2026-11-14, macOS ARM64, existing project-local Rust 1.99.0; no dependency/install changes:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo check --workspace --locked --offline` | PASS |
| `cargo test --workspace --locked --offline` | PASS: 17 behavioral tests, 0 failed/ignored; 0 doc tests |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | PASS without warnings |
| Offline CLI demo | PASS: 3 events, 2 Long observations from sequence 2/3 |
| `python3 scripts/check_docs.py` | PASS: 23 Markdown files, 142 local links/anchors, 40 tasks; dependencies/status synchronized. Five negative controls caught unknown dependency, cycle, status mismatch, premature readiness, and missing link/anchor |
| `git diff --check` | PASS |
| Rust/manifest/lockfile/LICENSE preservation | PASS: `git diff --exit-code -- crates Cargo.toml Cargo.lock rust-toolchain.toml LICENSE` reports no changes |
| GitHub main receipt | Pending push/remote SHA comparison |

Only this baseline's semantics were tested. Other platforms, external codecs, financial calculations, future goldens, Desktop parity, worker/plugin resource isolation and production workloads are unverified. No third-party scanner ran and no private dependency graph was uploaded; Cargo.lock has only five local packages.

## Open questions and risks

1. Numeric representation, signed/zero prices, units, scale, rounding/overflow and readiness: evaluate in P1-02; current positive u64 prices are insufficient as universal finance types.
2. External model format/Protobuf codec: P1-03; code generation/dependencies need supply-chain review before use.
3. State workflow/snapshot semantics: P2; counters are not resumable strategy state.
4. Desktop FFI/IPC, worker transport, plugin execution/isolation: open tasks/experiments in ADR register, no selected mechanism claimed.
5. Ledger realism, data leakage, exact financial results and real platform compatibility remain separate acceptance gates.
6. No toolchain on default PATH; use the existing local exports rather than installing/replacing tools.

## Next task acceptance

P1-02 must deliver a documented evaluated numeric/type subset and executable validated types/parameter bindings. Explicitly specify supported units/scales/precision, signed/zero-price scope, overflow/rounding, Missing/Warmup/Error and parameter defaults/required/range/enum/unknown-key behavior. Prove boundaries, invalid schemas/bindings and compatibility with baseline tests. Run four Rust checks and documentation checker; update contracts, plan, this ledger and handoff. Do not add indicators, a codec, workers or accounting while completing this slice. Detailed resumption instructions: [NEXT_SESSION](NEXT_SESSION.md).
