# P4-03 implementation checkpoint

Status: **IN_PROGRESS**, observed 2026-11-14. This report records partial evidence, not task or phase acceptance. [Project status](PROJECT_STATUS.md) owns progress; [Desk integration](DESK_INTEGRATION_PLAN.md) owns requirements.

## Implemented and checked

The actual Desk checkout contains first-party private-pipe/managed clients, application lifetime binding, generic descriptor/typed parameter/port editing, explicit frozen-data conversion, immutable drafts/input/terminal records and native report/history readers. Store limits are 64 drafts, 128 runs, 256 MiB total and 10 MiB payload; debug pages contain at most 16 rows. Computation uses the native engine exclusively for an FQE run. No runtime checkout fallback, automatic retry, broker integration or language-worker support is claimed.

The bridge adds optional `backtest.validate/1`: native model/history/account/policy preparation returns its manifest without strategy evaluation or job admission. Existing financial source pins and golden results remain unchanged. The full [gate summary](evidence/p4-03/full-gate-summary.json) passes 341 Rust and 23 external tests, prior process/golden/source checks, four independent financial oracles, seven examples, fresh-child restore, fmt/check/test/strict Clippy/docs/diff. Gate completed at 2026-11-14T14:00:06.133717+00:00; no Rust changes followed it.

Actual pinned Dart 3.5.1 [service evidence](evidence/p4-03/desk-service.json) covers exact save/reselect/native bytes, two parameter variants, generic descriptor-selected constant/identity composition with u64::MAX, frozen dataset conversion, overload, stop, disconnect, unavailable-engine historical reopen and corruption preservation. Representative stop was 135.865 ms and historical reopen 5.411 ms. The [store negatives](evidence/p4-03/desk-store-negatives.json) include real write failure, partial files, links, size/count quotas, immutable records, incompatible bundle and child cleanup.

Fourteen targeted Flutter tests pass in [the latest log](evidence/p4-03/flutter-tests.log), including the FQE panel, existing exit guard/navigation/visual document/replay result/order events. The [scoped analyzer](evidence/p4-03/dart-analyze.log) reports no issues. Rendering fakes do not establish native financial parity. No Pub resolution, new registry package, SDK update or lockfile change was performed.

## Observed native journey and discovered defect

Android Studio built and ran the actual Desktop project using Flutter 3.24.1/Dart 3.5.1, `lib/main.dart`, macOS arm64 Debug. A duplicate old Desk process was normally quit through its menu; current PID9538 and its actual bundled helper PID14197 were separately identified. The current source initially failed compilation because concurrent tab edits duplicated `initialIndex`; only the duplicate was removed before the successful build.

The actual UI showed the native confirmation, processed seven bars, displayed completed account/metrics/parameters, two original fill rows and persisted input/terminal entries. Initial equity100, final97, fees1 and result identity `sha256:4c14f944e93837515558c26e3708a9cc0efd01633662a8198dfed1e52f6755ed` match the native baseline. See [native observations](evidence/p4-03/native-journey.json). This app retained an existing login; it is not evidence of a no-login native journey.

Changing the parameter field without Enter exposed an actual defect: save/run still used the prior value. The fix collects visible edits and atomically parses them before save/validate/run/composition; malformed edits fail without admitting a run or saving an old value. Unsaved edits survive panel reconstruction. Targeted tests cover90 without Enter, invalid run/save rejection, corrected80 save and invalid-edit reconstruction. Native verification of this fix remains open.

The [actual bundle inventory](evidence/p4-03/native-bundle.json) verifies arm64, strict helper signature, only system native linkage, exact Sandbox+inherit entitlements and signed helper SHA matching its bundled manifest. [The pin](evidence/p4-03/engine-bundle-pin.json) records bridge/source/financial/artifact/notices identities. Normal Cmd+Q of the owned application was observed and [both actual PIDs were gone](evidence/p4-03/native-exit.json); this is idle exit evidence, not active-run exit or a measured latency result. This development bundle does not establish installed customer-channel acceptance.

## Current consumer conflict and remaining acceptance

After the baseline journey, concurrent Desk edits removed `QuantEngineEditor` and its dashboard import/wrapper. Application binding and Xcode helper staging remain. Preserve that current visual editor; do not restore the removed entry without coordination. The user has a pending integration-choice question. [Current source hashes](evidence/p4-03/desk-current-source.json) identify the observed files and explicitly do not pretend to freeze the earlier native candidate. The [historical integration patch](evidence/p4-03/desk-integration.patch.json) is retained as exact JSON-escaped text with its original SHA, only as implementation history.

Read-only inspection also found Desk's current `StartController` routes users without a saved token/uid to login, and no local research entry exists in the login flow. The FQE service is independent of login, but native application reachability without an account is still undelivered. Resolve this as a dedicated local research entry while preserving trading authorization and the Desk shell contract; do not claim that an SDK-only test proves this UI requirement.

Before closure: coordinate the current editor entry; verify native parameter changes, generic compose/save/reselect, data selection, stop, panel close/reopen, historical reopen without execution and active-run application exit/helper cleanup; establish an actual offline/no-login path; freeze the paired source/artifact identity and complete the cross-project report. Keep P4-03 IN_PROGRESS and do not start dependent P4-04/P4-05. P6 owns installed customer-channel evidence. No Desk commit or GitHub push is credited.

Local partial checkpoint `ebdbbbf` records implementation and evidence, not closure. Its raw patch context whitespace is corrected by storing the exact historical patch as JSON text; final documentation and staged-diff checks must pass. Consumer files remain uncommitted pending coordination.
