# Desk Embedded Quant Integration

Status: **planned acceptance contract**, with a read-only consumer assessment on 2026-11-14 (Asia/Tokyo). No FQE integration, migration, packaging or Desk test is claimed by this document. Product authority: [Charter](PROJECT_CHARTER.md); direction: [ADR-011](ARCHITECTURE_DECISIONS.md#adr-011--desk-embedded-delivery-and-migration). Progress remains in [Project status](PROJECT_STATUS.md).

## Product outcome and scope

The primary consuming product is the existing Desktop repository `finger_desk`. FQE must be delivered inside its supported application package so a user can select/create a strategy, edit validated parameters, select local historical data, run a backtest/local simulation, inspect decisions/orders/fills/cash/positions/performance, stop a run and reopen its historical results. The owner confirmed that the first delivery is the **backtest and local simulation loop**; live trading is a later separately accepted integration. No live order is required to accept this first delivery.

Desk is optional for standalone FQE users, but successful Desk integration is a required project deliverable. Engine tests alone cannot close it. “Embedded” means delivered and managed with the application; it does not preselect same-process FFI over a bundled engine process. FFI/IPC remains an evidence-based P4-01 decision. Normal customer use cannot depend on a development checkout, terminal command, system Rust/Python or a manual engine download.

## Observed consumer baseline

Read-only inspection: `/Users/george/Desktop/finger_desk`, main, HEAD `f8e8b8ae0c8b88137ac38bb472dfc3fa17e14cb7`, ahead of its local origin/main by one commit. The working tree also contained 156 modified tracked files and 353 untracked files; HEAD is not the identity of all inspected source. These counts are dated observations, not acceptance evidence. No complete master ancestor context was found, so no BFF/production API or port is inferred. No Desk files or user datasets were changed or executed.

| Observed source / record in Desk | Integration implication / owner |
| --- | --- |
| `lib/app/modules/home/workbench/strategy/strategy_definition.dart`, `python_strategy_generator.dart` | Existing visual schema is dual-SMA-specific, uses integer quantity/double risk fields and generates Python observations. P1-03/P1-07 define the portable model; P4-03 implements an explicit import/editor adapter with decimal quantity and feature validation. No direct reinterpretation as universal FQE IR. |
| `lib/app/quant/adapters/quant_application_service.dart`, `host/quant_host_service.dart` | Existing application service owns hosted runs and Python callback orchestration. P4-02/P4-03 preserve application-level ownership and attach an engine client; disposing a panel must not terminate a run. |
| `lib/app/quant/backtest/quant_cash_book.dart`, `quant_cash_report.dart`, `adapters/quant_cash_store.dart` | Executable Dart financial computation and durable records already exist. P3/P4 require a migration and comparison path to Rust authority, not a second active ledger for the same run. |
| `lib/app/quant/adapters/quant_dataset_repository.dart`, `market/quant_historical_data.dart` | Reuse the data-management UI through an explicit adapter. Freeze exact data identity, metadata, timeframe, range and warm-up; P3-01/P4-03 must prove conversion parity. |
| `lib/app/quant/presentation/quant_application_binding.dart`, `quant_exit_guard.dart` | Application owns service lifetime and shutdown. P4-02/P4-03 test panel close, application exit, cancellation, crash and cleanup separately. |
| `lib/app/quant/adapters/quant_bundled_environment.dart`, `ci/desk-toolchain.json`, `ci/quant-toolchain.json` | Source implements versioned bundled environments; customer-channel acceptance remains open. Current app pin is Flutter 3.24.1/Dart 3.5.1; separate tools pin 3.27.0/3.6.0, Python 3.12.14. P4/P5/P6 must record actual paired toolchains rather than mix evidence. |
| `docs/quant/hosted-strategy-contract-v1.md`, `contracts/quant/v1/`, `ci/quant-peer.json` | Existing hosted API 1, business IPC 1.0 over JSON-RPC 2.0, storage 1, `cash-long-ohlcv/1`, SDK/worker identities need explicit compatibility mappings. The recorded SDK peer is `06cf10c7a44b780f9d8ee894391ac361028b9d6c`; it is not automatically an FQE worker contract. |
| `macos/Runner/Release.entitlements`, `docs/quant/current-status.md`, `p4-store-submission-verification.md` | Current first platform/channel is macOS arm64 / Mac App Store, with App Sandbox. Signed bundle, worker permissions, installation, upgrade and clean-customer execution need actual evidence; a local build or submission package is insufficient. |

Desk's records describe historical native runs and remaining stop/performance/distribution issues; none were rerun here. Its status page still names an older HEAD, reinforcing the need for a freshly pinned paired candidate. Desk's P0–P6 names are independent of FQE's P1–P6: always qualify project plus task/version in cross-project reports.

## State, semantics and migration

| Domain | Target authority | Transition requirement |
| --- | --- | --- |
| Definitions and parameter schemas | Versioned FQE model for portable strategies; declared worker artifact for code strategies | One saved identity supplies editor, run selection and frozen run parameters. Validate imported legacy visual models; unsupported rules get actionable errors. Never execute a different strategy from the one shown. |
| Drafts and workspace layout | Desk | Preserve drafts, windows, navigation and preferences; unsaved edits do not mutate a running or historical artifact. Respect Desk's existing workbench/dialog contract. |
| Event order, strategy memory, simulation orders/ledger and report formulas | FQE for FQE runs | Rust owns financial truth. Desk displays engine outputs; no independent Dart recomputation or worker-owned account for that run. A legacy run may keep its explicitly labeled legacy backend during migration. |
| Stored historical records | Original versioned backend/reader | Retain original bytes, identity and read-only verification. Reading old results, replaying recorded results and re-executing a strategy are separate capabilities. Unsupported re-execution must not prevent supported historical reading. |
| Credentials and external orders | Independent Trading SDK adapter | Never in IR or generic worker messages; first offline delivery has no live route. Existing SDK-hosted research facilities need explicit worker/host boundary review before reuse. |

P4-03 records a compatibility matrix for visual schema, hosted API, model, dataset, journal/report and worker versions, with dispositions: preserve/read-only, explicitly convert with a new identity, or reject execution with a reason. P1-06 includes generic SMA with independent G-007 results because the existing visual builder uses dual-SMA; SMA is not interchangeable with EMA. Quantity and risk settings need actual execution mappings or explicit rejection, not preservation as decorative metadata. New engine selection is frozen per run. There is no automatic fallback between backends after validation or execution failure, no automatic upgrade of old run semantics and no destructive in-place journal rewrite. Imports are repeatable and leave originals intact.

P3-01/P3-02 and P4-03 use a shared, public synthetic cash-long baseline to compare existing and new results independently: bar visibility, next-open fills, fees/slippage, reservations, partial/terminal order policy, protection priority, cost/P&L, end holdings, warm-up and metric sampling. Preserve `cash-long-ohlcv/1` only if all its declared semantics match. FQE's broader partial-fill/margin profiles need distinct versions; matching one cash fixture does not establish all-profile compatibility. The four-bar example (initial cash 1000, buy/sell two units at 120/130, fees 0.5 each, final cash 1019) is an initial comparison fixture, not the whole gate. No user data is copied into tracked fixtures.

Pure IR events stage internal state/signals atomically. Legacy hosted callbacks can include individually committed external/context operations that survive a later callback failure. P5-01/P5-02 must specify these different transaction boundaries and preserve or version legacy behavior; event rollback must never undo a committed fill or repeat a non-idempotent order.

All adapters specify exact integer/decimal and timestamp mappings. Desk decimal strings and microsecond UTC timestamps are not assumed equal to FQE numeric units/nanoseconds. Reject overflow, precision loss, unknown fields/capabilities and unrepresentable values. Plotting conversion is display-only. A preflight summary shows the final bound instrument/timeframe, range/warm-up, model/profile, parameters and costs; dataset-driven overrides are explicit and do not rewrite the saved strategy.

## Delivery milestones within the existing plan

The six-stage order is retained; these are integration acceptance slices, not a second task backlog or permission to skip dependencies.

| Milestone | Owning existing tasks | Required evidence |
| --- | --- | --- |
| Consumer contract preparation | P1-03, P1-10, P3-01/P3-02 | Portable artifacts/exact values, independent external invocation, frozen synthetic data/financial fixtures and legacy compatibility requirements; no Desk support claimed. |
| First Desk offline loop | P4-01–P4-05 | Actual Desk consumes the built FQE artifact through the chosen boundary; loaded MACD/RSI model plus explicit signal-to-intent policy runs history→fills→ledger→report. Two parameter sets, cancellation, restart/read-only results and native UI observed. Existing Python strategies remain explicitly legacy until P5. |
| Existing code-strategy path | P5-01/P5-02, P5-05 | Actual Desk uses a conforming Python worker through FQE. Validated legacy hosted strategy and context calls, exact values, state/output ownership, stop/failure/recovery and no duplicate committed operations. Java/Go still pass their separate engine gates. |
| Final bundled project delivery | P6-04–P6-06 | Repeat Desk end-to-end acceptance against the final engine/worker/plugin versions, installed signed app, offline clean-customer setup and upgrade/rollback evidence. Earlier P4 native evidence cannot stand in for the final combination. |

P4-04's SDK mock adapter is a planned engineering deliverable, optional at runtime. It does not require credentials or a live trade and does not turn the first offline loop into a live-trading release. Real-time market-data simulation, reconnect/gap policy, account authorization/reconciliation and live execution require a later scoped delivery; success on historical replay alone cannot claim them. Their absence does not invalidate the explicitly scoped first historical-data backtest/simulation loop.

## Native user journey and pass conditions

P4-05 and the final P6-06 rerun must observe this journey in the actual Desk application, with failures visible in the existing workbench:

1. Install/open the supported package and enter local quant without a trading login; inspect engine version/capabilities and actionable missing/incompatible-artifact errors.
2. Create/import/select a saved portable strategy, change parameters, save/reselect and see identical validated values. Invalid types/units/ranges and unsupported legacy features are rejected before a run.
3. Select/import a local synthetic dataset, range, warm-up and cash/profile settings; confirm the frozen run summary. Changing a draft later cannot change this run.
4. Run two genuine parameter/model variants. Match CLI/library decisions and independently expected orders/fills/cash/positions/fees/metrics; no demo result may fill a missing engine field.
5. Inspect progress, diagnostics, node/signal provenance, results and recorded replay; changing panels preserves the run and chart replay cannot trade or rewrite journals.
6. Cancel during load, warm-up, active execution and report writing; distinguish stopped/failed/completed, prove bounded terminal delivery and cleanup. Closing the final window or quitting the app follows the declared stop/checkpoint policy with no orphan process or hidden auto-resume.
7. Reopen the app and the same historical result without re-running strategy code. Detect corrupt/partial/missing/incompatible records, clear stale success displays and preserve supported old results.
8. Test a deliberately incompatible engine, disconnect/crash, full disk/denied write and overload. No silent backend switch, duplicate accepted order, unbounded log queue or UI-thread computation.

P4-01/P4-02/P6-05 define quantitative budgets before measurement: first-run/restore latency, events per second, peak memory, log/disk growth, result read/seek latency, cancellation deadline, UI responsiveness and progress frequency. Measure representative no-trade and many-fill workloads on the same hardware/data and record warm/cold reads; reducing trades, skipping durability or reusing unrelated candidate numbers cannot satisfy the gate. Threshold values are owned by those tasks and remain unselected here.

## Bundle and paired-release evidence

First supported Desk target is macOS arm64 with its existing Mac App Store channel. FQE's Windows/macOS/Linux CLI release matrix remains a separate P6-04 obligation; it does not establish Desk support on every OS. P4 records a runnable packaged candidate; P6 requires installed customer-channel evidence with appropriate signatures, Sandbox permissions, relocation and app-container data access. Do not disable signing/Sandbox to obtain a pass or require customers to install a compiler/interpreter.

Desk bundles a pinned engine and supported worker/runtime artifacts, dependencies, licenses and content identities. Compatibility negotiation happens before loading/running. Updates preserve local data and supported old records/bindings; rollback rejects newer unsupported state rather than downgrading it silently. Each release records Desk commit plus any reviewed patch identity, FQE commit/artifact hash, worker/SDK/runtime/contract/profile versions, platform/architecture, actual toolchains, build/signature identity and fixture checksums. A dirty development checkout must be frozen into an identifiable candidate before its results become release evidence.

Engine-owned interface/goldens belong here; Desk-owned bindings, app lifecycle, UI and bundle changes belong in Desk, coordinated in one paired acceptance report. Neither repository may declare cross-project delivery complete from its own unit tests. This documentation task changes only FQE; the substantial unrelated Desk work is preserved.
