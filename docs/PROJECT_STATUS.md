# Project Status

**Unique authoritative progress ledger.** Updated 2026-11-14 (Asia/Tokyo). Read [Next session](NEXT_SESSION.md), [Delivery plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md) and current contracts before editing. Actual code/tests/Git override stale records; authority rules: [Document authority](DOCUMENT_AUTHORITY.md).

## Current position

- Phase: **P1 — Universal Strategy Foundation, incomplete**. P2–P6 NOT_STARTED. No functional phase gate has passed.
- Completed: GOV-01/GOV-02/GOV-03 and P1-01 through P1-06; local evidence and closure records synchronized.
- Latest implementation: **P1-06 — Indicator Function Slice**, COMPLETED. Shared registered SMA/EMA/MACD/RSI, bounded decimal policy, owned incremental instances and independent fixtures delivered; 96 workspace tests/all required checks passed. Evidence `e772bf0a44f35a5b457cbe10c02eb8cc16689003`.
- Next eligible implementation: **P1-07 — Strategy Compiler & Typed Graph**, READY. Generic loaded-model compilation/module expansion follows the completed indicator slice. User authorized continuing downstream work without routine confirmation; publishing is not requested.
- P1-02 acceptance met: evaluated numeric policy/limits, strict type/unit/scale checks, unsupported-shape rejection, full binding matrix and tests. Evidence `a653c105fba5ea6ada15395abc22cc8f82cc4472` and local closure recorded below; publishing not requested.
- Actual root: `/Users/george/Desktop/finger-quant-engine`; main tracking origin/main; remote `https://github.com/FingerTrader/finger-quant-engine.git`.
- Rust source baseline / latest pre-governance commit: `342d8aa76b48d32fa9c9110609b371eac52b8d86` (foundation). Previous commit: `cbcebad`.
- Latest status-record commit: resolve `git log -1 --format=%H -- docs/PROJECT_STATUS.md`. Current checkout: `git rev-parse HEAD`. See the non-self-referential recording rule in DOCUMENT_AUTHORITY.
- Recorded governance/audit evidence commit: `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc`. Normal push succeeded; `git ls-remote origin refs/heads/main` independently confirmed this exact SHA on GitHub. The closure commit containing this record is resolved by the status-log command above; compare current HEAD to remote again when publishing closure.

## Task ledger

| Task(s) | Status | Evidence / next condition |
| --- | --- | --- |
| GOV-01 | COMPLETED | All governance artifacts/checks delivered in `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc`; normal GitHub main push and remote SHA confirmed; closure records synchronized |
| GOV-02 | COMPLETED | Six corrections, ADR-011 and [Desk delivery contract](DESK_INTEGRATION_PLAN.md); checks below passed; evidence `49282a0daf3d08145d2cfb5ab2f42a2184e2a900`; local delivery, not pushed; no stage implementation credited |
| GOV-03 | COMPLETED | Open extensibility contract/ADR and 42-task refinement; checks below passed; evidence `5e9f770c01e513598478f4cbdd42a9a5278fb804`; local closure, not pushed; no implementation capability credited |
| P1-01 | COMPLETED | [Baseline audit](BASELINE_AUDIT.md): all crate/contract inventory, 17 tests, gap/task mapping and document checks; evidence `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc` |
| P1-02 | COMPLETED | Checked decimal/type/parameter APIs, contract/ADR and 36 tests/full checks/two examples; evidence `a653c105fba5ea6ada15395abc22cc8f82cc4472`; local closure, not pushed |
| P1-03 | COMPLETED | Strict bounded import/identity/binding/compatibility contract; 56 tests/full checks; evidence `b4b7707fc786c5ea5b68dd7f19fb03cb10f5b45d`; local closure, not pushed |
| P1-04 | COMPLETED | Pure registry/invocation/model-node preparation, two independent X-001 compositions, 69 tests/full checks; evidence `d0382d365815d233e4f28773995e2ef688f2a656`, local closure |
| P1-05 | COMPLETED | Typed expressions/calls and transactional crossings; 86 tests/full checks/all examples PASS; evidence `424aacb9faf5d081b72bf0a82f26ef396de16c78`, local closure |
| P1-06 | COMPLETED | Registered indicators/owned staged state/ADR-017; 96 tests/full checks/all examples PASS; evidence `e772bf0a44f35a5b457cbe10c02eb8cc16689003`, local closure |
| P1-07 | READY | P1-06 closed; generic compiler/typed graphs/bounded reusable modules/source diagnostics, see NEXT_SESSION |
| P1-08, P1-09, P1-10, P1-11 | NOT_STARTED | Dependencies in DELIVERY_PLAN |
| P2-01, P2-02, P2-03, P2-04, P2-05, P2-06 | NOT_STARTED | P1 gate first |
| P3-01, P3-02, P3-03, P3-04, P3-05, P3-06, P3-07 | NOT_STARTED | P2 gate first |
| P4-01, P4-02, P4-03, P4-04, P4-05 | NOT_STARTED | P3 gate first |
| P5-01, P5-02, P5-03, P5-04, P5-05 | NOT_STARTED | P4 gate first; Python → Java → Go |
| P5-05.01 | NOT_STARTED | Real unlisted-language onboarding after P5-04; prerequisite of parent P5-05 |
| P6-01, P6-02, P6-03, P6-04, P6-05, P6-06 | NOT_STARTED | P5 gate first |
| P6-03.01 | NOT_STARTED | Java-indicator/Go-predicate graph after P6-02; prerequisite of parent P6-03 |

Stage-task totals: **42 = 40 parent tasks + 2 child tasks**: 6 COMPLETED, 1 READY, 35 NOT_STARTED. P1-07 is next; none is IN_PROGRESS or BLOCKED. No functional phase gate has passed. GOV-01/GOV-02 historical counts refer to their original 40-task plans.

## Delivered baseline and limitations

Five Rust crates preserve the initial foundation: validated domain values, experimental price-only IR, synchronous one-shot runtime, protocol version-number helper/draft schema and offline CLI demo. These predate the six-stage plan and remain preserved. Technical maturity: implemented but experimental, not stable/public SDK release.

An experimental P1-02 decimal/type/parameter layer exists in fqe-core/fqe-ir. P1-03 adds the separately versioned IR2 bounded JSON importer, immutable model declarations, exact numeric values, canonical/source identity and content-pinned prepared bindings. [Model loading](MODEL_LOADING.md) owns supported/unsupported semantics; [dependency review](P1_03_DEPENDENCY_REVIEW.md) records three direct libraries and all 21 resolved registry packages. Offline builds require previously cached toolchain and locked dependencies.

The separate P1-05 in-memory typed expression/crossing layer is implemented. No complete universal graph evaluator/indicator/compiler, generalized state scheduler/snapshot, backtester/virtual ledger/optimizer, Desktop boundary, SDK adapter, language worker or plugin runtime exists. Signals are observations, not orders/fills/performance. The `.proto` remains a draft, not a codec or transport. [Audit](BASELINE_AUDIT.md) records the dated pre-P1-02 foundation rather than current feature totals.

## Historical baseline verification evidence

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

Delivery: evidence committed as `49282a0daf3d08145d2cfb5ab2f42a2184e2a900`; this follow-up records local closure and is resolved by the status-log command above. GitHub publishing was not requested; these documentation commits remain local and do not inherit GOV-01's historical remote receipt. At that closure, next implementation was P1-02 READY. First Desk scope is historical backtesting/local simulation, with P4 actual integration, P5 Python compatibility and P6 final bundle acceptance still NOT_STARTED.

## GOV-03 extensibility planning verification

2026-11-14, macOS ARM64, existing local Rust/Cargo 1.99.0, documentation-only scope:

- Added the planned normative EXTENSIBILITY_CONTRACT and ADR-012. Reviewed shared descriptors, client/runner/provider roles, generic rule composition, state/transaction boundaries, runtime independence, adapter onboarding and Desk discovery/customer packaging against existing contracts.
- All four required Rust checks passed: format, locked/offline workspace check, 17 behavioral tests (zero failed/ignored; zero doc tests), strict all-target Clippy. CLI demo passed: three events/two Long observations, sequences 2/3.
- Document checker passed: 25 Markdown files, 188 local links/anchors, 42 stage tasks; acyclic dependencies and synchronized plan/ledger statuses. Seven negative cases caught unknown dependency, parent/child cycle, status mismatch, premature readiness for each of the two parent gates, missing file and missing anchor.
- Compared with pre-change plan at `ed4aae0`: original 40 task IDs/statuses retained; only P5-05 and P6-03 dependency cells extended, each with its new child. The sole eligible next implementation at that dated closure was P1-02. GOV-03 scope/status/handoff checked separately because the document checker automatically validates stage tasks and GOV-01, not arbitrary governance rows.
- Independent rational calculation verified X-005 example specification: Warmup/Warmup/2/2/2/2, threshold 1 true on bars 3–6, strict threshold 2 never true. This is a specification check; no Java/Go provider or graph has been implemented/executed.
- Full intended document diff reviewed; diff whitespace checks and Rust/manifests/lockfile/toolchain/LICENSE preservation passed. No Desk writes/execution/tests or new consumer inventory occurred in this slice. No dependency/tool installation, third-party scanner, network advisory check or private dependency upload; Cargo.lock remains the same five local packages.

Delivery: evidence `5e9f770c01e513598478f4cbdd42a9a5278fb804` committed; this follow-up records COMPLETED local closure and is resolved using the status-log command above. GitHub publication is not requested. Remote main was checked on 2026-11-14 and remained `0ac5d7b163e77bddb5f69689a1455e3560aad872`; this is a read-only receipt, not a push of GOV-02/GOV-03. Initial local Git index write was sandbox-restricted; the scoped local commit succeeded with the filesystem permission override. At that closure P1-02 was READY; X-001–X-006 runtime/product evidence remains future work.

## Open questions and risks

1. P1-02 implements the bounded decimal/type/parameter contract under ADR-013. Indicator work18/output12 policy and conditioning limits are selected in ADR-017; financial tick/lot/currency policy remains P3-01 work. The legacy evaluator remains positive-u64-only.
2. ADR-014 selects experimental model JSON; Protobuf mapping/transport/generators remain P4-01/P5-01 choices with separate review.
3. State workflow/snapshot semantics: P2; counters are not resumable strategy state.
4. Desktop FFI/IPC, worker transport, plugin execution/isolation: open tasks/experiments in ADR register, no selected mechanism claimed.
5. Ledger realism, data leakage, exact financial results and real platform compatibility remain separate acceptance gates.
6. No toolchain on default PATH; use the existing local exports rather than installing/replacing tools.
7. Required Desk product integration is planned under ADR-011 and [Desk integration](DESK_INTEGRATION_PLAN.md). Existing Desk Dart/Python financial/hosted paths need a pinned paired candidate, versioned migration and single-backend authority. First scope is backtesting/local simulation; no FQE/Desk connection or native customer bundle is verified.
8. Open language/indicator/rule extension support follows ADR-012 and [Extensibility](EXTENSIBILITY_CONTRACT.md). The pure scalar in-process registry/provider slice now exists under P1-04; external providers/persistent state, onboarding kit, fourth-language X-004 and cross-language graph X-005 remain future gates. Type/event/host-service growth needs versioned negotiation; runtime support and Desk bundle availability need separate evidence.

## P1-02 type/parameter verification

2026-11-14, macOS ARM64, existing local rustc 1.99.0 (b940084d7) / Cargo 1.99.0:

- Implemented [numeric primitives](../crates/fqe-core/src/numeric.rs), [scalar/unit types](../crates/fqe-core/src/types.rs), [immutable parameter bindings](../crates/fqe-ir/src/parameters.rs), [value tests](../crates/fqe-core/tests/value_contract.rs), [parameter tests](../crates/fqe-ir/tests/parameter_contract.rs) and [offline example](../crates/fqe-ir/examples/parameters.rs). [Type/parameter contract](TYPE_PARAMETER_CONTRACT.md) and ADR-013 record the evaluated choice, finite range/precision and unsupported capabilities.
- `cargo fmt --check`, locked/offline workspace check/test and strict all-target Clippy all passed. **36 behavioral tests = 17 unchanged baseline + 19 new** (9 numeric/type, 10 parameter); zero failed/ignored and zero doc tests. A prior Clippy collapsible-if finding was corrected; the complete sequential check suite then passed.
- New cases cover exact decimal inputs/large integers, signed rounding/ties/scale mismatch, overflow including MIN/-1 and conservative intermediate overflow, unsupported shapes/types/version, UTF-8/unit/scale validation, every supported type's binding matrix, finite enum sets, discrete range extrema, duplicate/unknown keys, immutable independent runs and maximum/over-limit parameter/choice/binding counts.
- Existing `cargo run --locked --offline -p fqe-cli -- demo` passed unchanged: three events/two Long observations from sequences 2/3. `cargo run --locked --offline -p fqe-ir --example parameters` passed: period defaults to 2, override 5, period 0 rejected; no strategy execution claimed by this example.
- `python3 scripts/check_docs.py`: PASS: 26 Markdown files, 211 local links/anchors, 42 stage tasks; task dependencies/status projections consistent. `git diff --check` passed. Legacy core/IR source differs only by module exports; runtime/CLI/protocol code, original tests, manifests/lockfile, toolchain and LICENSE are unchanged. No Desk source or app execution in this slice.
- No third-party dependency changes, downloads/installers/generators or network advisory/scanner calls. Existing stdlib/workspace code and pinned toolchain only; Cargo.lock remains five local packages. No private source/manifests/graphs uploaded. The API remains experimental; indicator accuracy, model decoding, accounting, workers/plugins and Desk integration need their own gates.

Delivery: implementation/test/contract evidence committed as `a653c105fba5ea6ada15395abc22cc8f82cc4472`; this follow-up records COMPLETED closure (resolve its containing commit by the status-log command). Local only, no GitHub push requested. Remote main was checked on 2026-11-14 at `0ac5d7b163e77bddb5f69689a1455e3560aad872`; this is not a publication receipt for P1-02 or the preceding local governance commits. At that closure P1-03 was READY, unstarted. P1 remains incomplete.

## P1-03 model-loading verification

2026-11-14, macOS ARM64, existing project-local Rust 1.99.0. Owner explicitly approved the dependency/build-script/public-name advisory request. No toolchain/installer or Desk changes.

- Delivered fqe-ir model.rs/loading.rs, public declaration/loading tests, actual valid/duplicate/truncated JSON files, independent canonical JSON/SHA-256 fixture, runnable load_model example, import/legacy compatibility contract and ADR-014. Schema 1 / IR2 is separate from legacy IR1. Metadata/descriptions/provider pins participate in identity. Same-author-version content changes cannot reuse pinned bindings.
- Required checks PASS: `cargo fmt --check`; `cargo check --workspace --locked --offline`; `cargo test --workspace --locked --offline`: **56 behavioral tests = 36 previous + 8 declaration + 12 loading**, 0 failed/ignored, 0 doc tests; `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`.
- Offline CLI demo PASS (three events/two observations). Parameter example PASS (default 2/override 5/rejected 0). Real-file loader example PASS (period 2/5, null rejected, no execution).
- Fixture source SHA-256 `44536cad09580b72e54b0c9e19c97ff512475918c835b4230140efecffc7fcd2`; canonical-v1 SHA-256 `f858a78dd2b1075cf9cb5ddeee42703df5976157432ec93bc3ed67c255ee7256`; independent Python json/hashlib expected bytes/digests match Rust import and re-import.
- Wire binding tests repeat required/default/omission/null/type/unit/scale/range/enum and exact i64/u64/i128 behavior; malformed/truncated/UTF-8/trailing/duplicate data, unsupported versions/features and bounded byte/depth/item/string/reader cases reject. Unknown provider names are portable declarations; provider existence/signatures/cycles are not import guarantees.
- Three direct packages serde 1.0.228 / serde_json 1.0.145 / sha2 0.10.9 plus 18 transitive lockfile resolutions reviewed; all 21 archive checksums verified, current public owners/repositories/licenses/release age/scripts inspected and OSV name/version-only queries returned no matching advisories. Locked fetch succeeded; active-feature graph has 16 registry packages, no derive/asm. Advisory/source-review limitations remain in the dependency report. No private manifest/code/lockfile/graph uploaded.
- Current document checker PASS: 28 Markdown / 245 local links/anchors / 42 tasks; whitespace checks PASS. Final closure rerun confirmed these counts and consistent status projections. Initial compile found an iterator API mismatch/unused import; corrected before all checks passed. No outstanding build/test failure.
- Legacy core/numeric/types/parameters/evaluator/runtime/CLI/protocol behavior, LICENSE and toolchain remain preserved; only fqe-ir dependencies/exports and scoped new model/import artifacts changed. No registry/compiler/worker/simulator or Flutter run added.
- Evidence `b4b7707fc786c5ea5b68dd7f19fb03cb10f5b45d` is committed; this status/handoff/plan-only closure records COMPLETED after all acceptance/checks. Resolve the closure SHA via the status-log command. Remote main read-confirmed at `0ac5d7b163e77bddb5f69689a1455e3560aad872`; no publish requested or attempted. Including this closure, eight local commits are ahead of origin/main.

## Historical P1-04 acceptance

P1-04 is closed with verified evidence below. Implement shared versioned node/function/provider descriptors and deterministic registries. Resolve namespaced ID/version/signature, typed named ports and parameter contracts generically. Prove X-001 unknown-name function and predicate injection through the same in-process interface as built-ins; reject duplicate/version/type/arity/capability/unsupported-shape failures and cover multi-input/output signatures and deterministic registry identity. No per-name compiler branch or external artifact loader. Detailed scope is [Next session](NEXT_SESSION.md); remain within [Extensibility](EXTENSIBILITY_CONTRACT.md) and the 42-task dependency plan.

## P1-04 execution scope

Started 2026-11-14 from clean main f187a17e1cd7db3bdfca0b89189690171d794af8. Implement fqe-ir registry/provider/built-in modules, shared canonical descriptor helpers and independent example/test fixtures. Acceptance: exact namespaced/version/artifact resolution, typed named ports/parameters, duplicate/arity/unit/capability rejection, deterministic descriptor/registry identity, checked output validation and two manually driven X-001 compositions through the same interface. Scalar pure synchronous calls only; unsupported shapes/state/host services/external loading fail explicitly. Graph compilation/event scheduling and foreign-language execution remain later tasks. Reuse existing reviewed dependencies and pinned offline toolchain; no dependency changes planned. Required Rust/docs checks, prior examples and new provider example/tests must pass before closure.

## P1-04 registry verification

Implemented registry.rs and builtins.rs, independent examples/support/providers.rs, providers example, registry_contract tests and tracked source-pinned models/canonical descriptor fixtures. Loader changes expose only crate-private canonical type/parameter helpers; IR2 encoding/goldens and legacy APIs are unchanged. Targeted registry tests PASS: 13 cases. Provider example PASS: 200/50 numeric outputs, true/false then false/true decisions from two different loaded artifacts. Registry identity eff2b19c3e3a8405db8828ec8dc14a394b206d0b0255983f692064c3c3d130af. Full final Rust/document checks PASS: 69 behavioral tests (56 previous + 13 registry), zero failed/ignored and zero doc tests; fmt/check/strict all-target Clippy, all four examples/demo, document checker 29 Markdown/267 local links/42 tasks and diff checks PASS. Evidence `d0382d365815d233e4f28773995e2ef688f2a656` committed; this status/handoff/plan-only closure records COMPLETED. Current code remains experimental; no compiler/event runtime, indicator state, external loading or Desk change. No dependency/manifests/lockfile/toolchain changes; reuse the previously reviewed cached libraries and pinned Rust.

The intended diff was reviewed; preservation checks confirm no manifest/lockfile/core/runtime/CLI/protocol/toolchain/LICENSE changes. `rg` for both independent provider IDs under engine src returned no matches; example/test source holds the implementation. Remote main read-confirmed unchanged at 0ac5d7b163e77bddb5f69689a1455e3560aad872. No GitHub publishing requested. All checks used cached reviewed dependencies with locked/offline flags; no new advisory query or private upload.

P1-04 closure: evidence commit is recorded; resolve this closure SHA via the status-log command. Including this closure, main is ten commits ahead of the last confirmed origin/main. No publishing requested. Next P1-05 is READY; no phase gate passed.

## P1-05 execution scope

Started after P1-04 closure on 2026-11-14. Implement bounded immutable typed expression plans, exact checked scalar arithmetic/comparison/Boolean composition and generic P1-04 calls. Crossing subscriptions advance in stable dependency order independently of Boolean guards; per-instance memory is staged until whole evaluation succeeds. Tests cover readiness truth tables/short-circuit, numeric/unit boundaries, shared/history-once/false-guard behavior and rollback. No graph/module compiler, indicators, event scheduler, workers, financial ledger or Desk changes; no dependencies planned.

## P1-05 expression verification

Added expressions.rs, 17 public expression tests, expressions example and EXPRESSION_CONTRACT/ADR-016. Tests cover independently enumerated Boolean tables, exact checked numeric/dimensional policy, custom provider/predicate composition, crossing edges/dependencies/cycles, false guards/shared references/operand ordering, gap reset, isolation and failed-step rollback. Targeted 17 tests PASS. Earlier strict Clippy found three collapsible-if style issues; corrected and strict all-target Clippy now PASS. Expression example PASS: false/false/false/false/true over five accepted steps. Full final workspace checks PASS: 86 tests (69 prior + 17 expressions), zero failed/ignored/doc tests, fmt/check/strict all-target Clippy and all five example/demo commands. Document checker PASS: 30 Markdown/285 local links/42 tasks; diff checks PASS. Evidence `424aacb9faf5d081b72bf0a82f26ef396de16c78` committed; this plan/status/handoff-only closure records COMPLETED. No dependency changes or private uploads; no Desk/worker/compiler/accounting expansion.

P1-05 intended diff reviewed; preservation checks confirm manifests/lockfile/core/runtime/CLI/protocol/loader/registry/built-ins/toolchain/LICENSE unchanged. No independent-provider business ID appears under engine src. All builds/checks used the existing reviewed cached dependencies offline; no new dependency, network advisory query, private upload, Desk or external repository change. Remote main remains 0ac5d7b163e77bddb5f69689a1455e3560aad872; publication not requested.

P1-05 closure records actual evidence above. Including this closure, twelve local commits are ahead of last-confirmed origin/main; no publishing requested or attempted. Current source/history is resolved from Git. Next P1-06 is READY and remains unimplemented; no P1 functional gate has passed.

Final P1-05 closure document check PASS: 30 Markdown files, 286 local links/anchors and all 42 synchronized stage tasks; final whitespace check PASS.

## P1-06 indicator verification

2026-11-14, macOS ARM64, existing pinned rustc/Cargo 1.99.0:

- Added [indicator implementation](../crates/fqe-ir/src/indicators.rs), [public tests](../crates/fqe-ir/tests/indicators.rs), [example](../crates/fqe-ir/examples/indicators.rs), [contract](INDICATOR_CONTRACT.md), [independent numeric evaluation](P1_06_NUMERIC_EVALUATION.md) and ADR-017. Shared registry supports exact-pinned owned incremental factories/instances; pure canonical identity is preserved. No per-indicator branch in registry/compiler/expression code.
- Required fmt/check/locked offline workspace tests/strict all-target Clippy all PASS. **96 behavioral tests = 86 prior + 10 indicator tests**, zero failed/ignored/doc tests. CLI demo and parameters/load_model/providers/expressions/indicators examples all PASS. Indicator example: MACD(2,3,2), six accepted steps, line first ready bar3, signal/histogram bar4; final line 0.129629629630, signal 0.209876543210, histogram −0.080246913580.
- Independent Python stdlib Fraction generator, not invoked by tests: nine datasets/563 samples, eight output series each. Rust matches rational reference readiness exactly and ready coefficients within 1 at output scale12. Measured work18 candidate maximum absolute RSI error ~4.973e-13 versus ~1.068e-11 for work12 on these fixtures; see evaluation for all outputs and worst-case limitations. G-007 SMA2/SMA3 emits a cross only at bar5, never first-ready bar3. G-002 named MACD/RSI outputs fed to generic expressions yield bar5 at threshold60 and none at90; this manual library composition does not close loaded G-003/P1-11.
- Covered mean seeds, MACD partial readiness/no ×2, RSI flat/rising/falling, signed/short/512-step/large series; ±one output quantum/half-even ties/strict thresholds, binary64 loss near 1e12, max periods and MACD 19999-sample compound seed, explicit RSI conditioning rejection, parameter/type/profile/canonical-state fields, Missing/Warmup reset, sequence gaps/retries/isolated instances. An unknown-name stateful provider mutates staged state then fails/panics/returns invalid outputs; no state/cursor is committed and retry succeeds.
- Numeric policy: work18/output12 HalfEven, abs(close)≤1e12, period≤10000. RSI rejects nonzero mean gain+loss <1e-6 input units with registry_precision; this bounded version is unsuitable for smaller movements without an explicit unit/provider change. Single-node atomicity only; P1-08 must make all subscribed state and signals atomic together. In-memory deep clones are trusted-provider contracts, not isolation/persistence/heap quotas.
- Indicator source SHA-256 `125c8a17228b1a0f92b99661d2d7a69024672672b86292b4cd91c78086b0a29e`; tracked rational fixture SHA-256 `446071c02c76c144fe4cd80cc396f8af26a4d461b82fd68f6f775d1aadc430c6`. Source pins are not binary/dependency attestations. Fixture generation and numeric policy review are deliberate, not test-time regeneration.
- Document checker PASS: 32 Markdown files and 42 stage tasks; dependency/status projections and local links checked. Whitespace/full intended diff/protected paths review PASS. Initial test helper's Result/Option compile mismatch was corrected; final checks above ran after all code changes. No other test failure, tool installation, package/manifest/lockfile change, new advisory query/scanner/private upload, Desk edit or external repository execution. Existing reviewed pinned dependencies used offline; no new dependencies.

Local implementation evidence committed as `e772bf0a44f35a5b457cbe10c02eb8cc16689003`. This follow-up synchronizes COMPLETED closure and P1-07 READY; resolve the closure SHA through the status-log command above. Fourteen local commits including this closure remain unpushed. Remote main read-confirmed `0ac5d7b163e77bddb5f69689a1455e3560aad872`; no push requested/attempted. P1-07 graph/modules and P1-08 event scheduling remain unimplemented, with P1-09–P1-11/Desk/backtest/simulation gates still open.
