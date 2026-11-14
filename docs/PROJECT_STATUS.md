# Project Status

**Unique authoritative progress ledger.** Updated 2026-11-14 (Asia/Tokyo). Read [Next session](NEXT_SESSION.md), [Delivery plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md) and current contracts before editing. Actual code/tests/Git override stale records; authority rules: [Document authority](DOCUMENT_AUTHORITY.md).

## Current position

- Phase: **P1 — Universal Strategy Foundation, incomplete**. P2–P6 NOT_STARTED. No functional phase gate has passed.
- Completed: GOV-01 architecture governance, GOV-02 Desk planning and P1-01 baseline audit. No implementation task is currently active.
- Next eligible implementation task: **P1-02 — Universal Types & Parameter Schema**, READY. It has not been implemented.
- Current scoped work: **GOV-03 — Open language, indicator and rule extensibility planning**, VERIFIED, owner-requested. Acceptance: canonical extension contract and ADR, generic language onboarding and computation-provider bridge, early extension tests, explicit child-task dependencies, descriptor-driven Desk acceptance, synchronized status/handoff and required checks. Documentation only; P1-02 implementation remains unstarted.
- Actual root: `/Users/george/Desktop/finger-quant-engine`; main tracking origin/main; remote `https://github.com/FingerTrader/finger-quant-engine.git`.
- Rust source baseline / latest pre-governance commit: `342d8aa76b48d32fa9c9110609b371eac52b8d86` (foundation). Previous commit: `cbcebad`.
- Latest status-record commit: resolve `git log -1 --format=%H -- docs/PROJECT_STATUS.md`. Current checkout: `git rev-parse HEAD`. See the non-self-referential recording rule in DOCUMENT_AUTHORITY.
- Recorded governance/audit evidence commit: `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc`. Normal push succeeded; `git ls-remote origin refs/heads/main` independently confirmed this exact SHA on GitHub. The closure commit containing this record is resolved by the status-log command above; compare current HEAD to remote again when publishing closure.

## Task ledger

| Task(s) | Status | Evidence / next condition |
| --- | --- | --- |
| GOV-01 | COMPLETED | All governance artifacts/checks delivered in `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc`; normal GitHub main push and remote SHA confirmed; closure records synchronized |
| GOV-02 | COMPLETED | Six corrections, ADR-011 and [Desk delivery contract](DESK_INTEGRATION_PLAN.md); checks below passed; evidence `49282a0daf3d08145d2cfb5ab2f42a2184e2a900`; local delivery, not pushed; no stage implementation credited |
| GOV-03 | VERIFIED | Open extensibility contract/ADR and 42-task refinement; checks below passed; evidence commit/closure pending; no implementation capability credited |
| P1-01 | COMPLETED | [Baseline audit](BASELINE_AUDIT.md): all crate/contract inventory, 17 tests, gap/task mapping and document checks; evidence `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc` |
| P1-02 | READY | P1-01 dependency COMPLETED; acceptance below and in NEXT_SESSION; no code work started |
| P1-03, P1-04, P1-05, P1-06, P1-07, P1-08, P1-09, P1-10, P1-11 | NOT_STARTED | Dependencies in DELIVERY_PLAN |
| P2-01, P2-02, P2-03, P2-04, P2-05, P2-06 | NOT_STARTED | P1 gate first |
| P3-01, P3-02, P3-03, P3-04, P3-05, P3-06, P3-07 | NOT_STARTED | P2 gate first |
| P4-01, P4-02, P4-03, P4-04, P4-05 | NOT_STARTED | P3 gate first |
| P5-01, P5-02, P5-03, P5-04, P5-05 | NOT_STARTED | P4 gate first; Python → Java → Go |
| P5-05.01 | NOT_STARTED | Real unlisted-language onboarding after P5-04; prerequisite of parent P5-05 |
| P6-01, P6-02, P6-03, P6-04, P6-05, P6-06 | NOT_STARTED | P5 gate first |
| P6-03.01 | NOT_STARTED | Java-indicator/Go-predicate graph after P6-02; prerequisite of parent P6-03 |

Stage-task totals: **42 = 40 parent tasks + 2 child tasks**: 1 COMPLETED audit, 1 READY, 40 NOT_STARTED. No functional implementation task has been completed under the new plan. No stage task is currently IN_PROGRESS or BLOCKED. Open technical choices are future work, not evidence that a ready task cannot begin. No P1 implementation gap is closed by planning it. GOV-01/GOV-02 verification counts remain historical to their original 40-task plans.

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
| GitHub main receipt | PASS: normal push received `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc`; remote SHA matched. Initial local tracking-ref lock restriction was resolved with `git fetch origin main`; no remote/history rewrite |

Only this baseline's semantics were tested. Other platforms, external codecs, financial calculations, future goldens, Desktop parity, worker/plugin resource isolation and production workloads are unverified. No third-party scanner ran and no private dependency graph was uploaded; Cargo.lock has only five local packages.

## GOV-02 documentation verification

2026-11-14, macOS ARM64, existing local Rust 1.99.0 / Cargo 1.99.0, no downloads or dependencies changed:

- All four required Rust checks passed: formatting, locked/offline workspace check, workspace tests (17 behavioral tests, zero failed/ignored) and strict all-target Clippy.
- Offline CLI demo passed: completed, three events and two Long observations from sequences 2/3.
- `python3 scripts/check_docs.py` passed: 24 Markdown files, 162 local links/anchors, 40 stage tasks and synchronized stage statuses. Three in-memory negative controls caught unknown dependency, status mismatch and premature readiness.
- Independent rational calculation matched new G-007 SMA/cross values. The documented G-006 boundary decision cases k+1/k+3/k+4 and expiry were evaluated as specification checks; they are not tests of an implemented workflow.
- All 40 stage IDs and dependency cells match the pre-change plan. Gate wording excludes self-completion. `git diff --check` and engine/manifest/lockfile/toolchain/license preservation passed.
- Desk source assessment only: actual HEAD and dirty source inventory recorded in DESK_INTEGRATION_PLAN. No Desk tests, app execution, package installation, API calls or source writes were performed. Existing historical native evidence was not relabeled as new evidence.
- No third-party scanner/network advisory check or private dependency upload; only existing standard-library/workspace code and the previously audited installed toolchain were used. Cargo.lock remains the same five local packages. These documentation checks do not establish Desk compatibility, financial correctness or future runtime behavior.

Delivery: evidence committed as `49282a0daf3d08145d2cfb5ab2f42a2184e2a900`; this follow-up records local closure and is resolved by the status-log command above. GitHub publishing was not requested; these documentation commits remain local and do not inherit GOV-01's historical remote receipt. Next implementation remains P1-02 READY. First Desk scope is historical backtesting/local simulation, with P4 actual integration, P5 Python compatibility and P6 final bundle acceptance still NOT_STARTED.

## GOV-03 extensibility planning verification

2026-11-14, macOS ARM64, existing local Rust/Cargo 1.99.0, documentation-only scope:

- Added the planned normative EXTENSIBILITY_CONTRACT and ADR-012. Reviewed shared descriptors, client/runner/provider roles, generic rule composition, state/transaction boundaries, runtime independence, adapter onboarding and Desk discovery/customer packaging against existing contracts.
- All four required Rust checks passed: format, locked/offline workspace check, 17 behavioral tests (zero failed/ignored; zero doc tests), strict all-target Clippy. CLI demo passed: three events/two Long observations, sequences 2/3.
- Document checker passed: 25 Markdown files, 188 local links/anchors, 42 stage tasks; acyclic dependencies and synchronized plan/ledger statuses. Seven negative cases caught unknown dependency, parent/child cycle, status mismatch, premature readiness for each of the two parent gates, missing file and missing anchor.
- Compared with pre-change plan at `ed4aae0`: original 40 task IDs/statuses retained; only P5-05 and P6-03 dependency cells extended, each with its new child. The sole eligible next implementation is still P1-02. GOV-03 scope/status/handoff checked separately because the document checker automatically validates stage tasks and GOV-01, not arbitrary governance rows.
- Independent rational calculation verified X-005 example specification: Warmup/Warmup/2/2/2/2, threshold 1 true on bars 3–6, strict threshold 2 never true. This is a specification check; no Java/Go provider or graph has been implemented/executed.
- Full intended document diff reviewed; diff whitespace checks and Rust/manifests/lockfile/toolchain/LICENSE preservation passed. No Desk writes/execution/tests or new consumer inventory occurred in this slice. No dependency/tool installation, third-party scanner, network advisory check or private dependency upload; Cargo.lock remains the same five local packages.

Delivery: VERIFIED pending local evidence/closure commits. GitHub publication is not requested. P1-02 remains READY; all new capabilities and X-001–X-006 runtime/product evidence remain future work.

## Open questions and risks

1. Numeric representation, signed/zero prices, units, scale, rounding/overflow and readiness: evaluate in P1-02; current positive u64 prices are insufficient as universal finance types.
2. External model format/Protobuf codec: P1-03; code generation/dependencies need supply-chain review before use.
3. State workflow/snapshot semantics: P2; counters are not resumable strategy state.
4. Desktop FFI/IPC, worker transport, plugin execution/isolation: open tasks/experiments in ADR register, no selected mechanism claimed.
5. Ledger realism, data leakage, exact financial results and real platform compatibility remain separate acceptance gates.
6. No toolchain on default PATH; use the existing local exports rather than installing/replacing tools.
7. Required Desk product integration is planned under ADR-011 and [Desk integration](DESK_INTEGRATION_PLAN.md). Existing Desk Dart/Python financial/hosted paths need a pinned paired candidate, versioned migration and single-backend authority. First scope is backtesting/local simulation; no FQE/Desk connection or native customer bundle is verified.
8. Open language/indicator/rule extension support follows ADR-012 and [Extensibility](EXTENSIBILITY_CONTRACT.md). Generic provider, onboarding kit, fourth-language X-004 and cross-language graph X-005 remain unimplemented. Type/event/host-service growth needs versioned negotiation; runtime support and Desk bundle availability need separate evidence.

## Next task acceptance

P1-02 must deliver a documented evaluated numeric/type subset and executable validated types/parameter bindings. Explicitly specify supported units/scales/precision, signed/zero-price scope, overflow/rounding, Missing/Warmup/Error and parameter defaults/required/range/enum/unknown-key behavior. Evaluate descriptor port/parameter needs and identify unsupported shapes under EXTENSIBILITY_CONTRACT; implement only the selected subset, with provider/loader execution left to its owning tasks. Prove boundaries, invalid schemas/bindings and compatibility with baseline tests. Run four Rust checks and documentation checker; update contracts, plan, this ledger and handoff. Do not add indicators, a codec, workers or accounting while completing this slice. Detailed resumption instructions: [NEXT_SESSION](NEXT_SESSION.md).
