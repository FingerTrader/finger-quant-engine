# Desktop integration

Status: planned. The FQE CLI is implemented; no Flutter package, Dart binding, FFI bridge, local daemon, or Desktop connection is implemented.

## Independence

FQE can be built, tested, and run without Flutter Desktop. Its offline demonstration requires no login or trading account. All strategy computation stays on the user's device.

Flutter Desktop will be an optional presentation and orchestration client. It may configure a run, launch or connect to a local engine, inspect lifecycle, and display results. It must not become a build dependency of the Rust core or the authority for engine state and financial calculations.

The concrete consumer is the existing `finger_desk` project. “Optional” describes the standalone engine dependency, not permission to omit Desk delivery. The owner-selected first scope is backtesting and local simulation; real-time/live trading is separately accepted. [Desk integration](DESK_INTEGRATION_PLAN.md) is the canonical contract for the observed consumer, migration, single-backend ownership, native user journey, bundle and paired-release evidence.

P4 discovery and editing are descriptor-driven under the [Extensibility contract](EXTENSIBILITY_CONTRACT.md): built-in/reviewed test entries expose language/runtime capabilities, parameters, typed ports, diagnostics and output labels through a generic client API. P4 does not load P6 external code. P5 independently proves actual Python/Java/Go Desk journeys for supported runtime/platform combinations; P6 proves foreign-language indicator/predicate discovery and graph use in the final installed app. Per-name Desk page additions are not the supported extension mechanism.

## Intended boundary

The required boundary is versioned; FFI and IPC are candidates to evaluate in P4-01 under [ADR-010](ARCHITECTURE_DECISIONS.md#adr-010--one-engine-optional-desktop-boundary). Protocol Buffers is the first cross-language encoding candidate, not a chosen transport. Framing, process discovery, lifecycle ownership, and authentication remain undecided. No port, executable download path, or service endpoint is reserved here.

A future integration must negotiate engine/protocol capabilities and distinguish ready, unsupported, failed, and disconnected states. The current version helper compares version numbers only; it does not establish a connection or validate a Desktop client.

The selected FFI/IPC mechanism must follow measured correctness, safety, lifecycle, overhead and packaging evidence. It must not bypass domain contracts or tie engine releases to Flutter implementation details. P1-10 proves minimal external invocation; P4 proves actual Desktop integration and CLI/Desktop parity with the same models, parameters and data.

## Client responsibilities

- Present mode, strategy, parameters, input-data provenance, and explicit user actions.
- Treat the engine's accepted state and results as authoritative.
- Preserve u64 integers exactly across Dart, Protocol Buffers, and any diagnostic JSON.
- Handle incompatible versions, worker faults, cancellation, reconnect, and partial progress explicitly.
- Keep UI preferences and workspace layout separate from engine run/checkpoint state.
- Keep login and account onboarding outside offline engine eligibility.

A Desktop client's authentication can enable its own product features. It must not gate local engine validation, offline backtesting, or simulation when those features are implemented.

## Trading boundary

Trading APIs come from the independent Finger Trader Trading SDK. A future authorized execution adapter must make mode, account selection, permissions, and order submission explicit. Neither launching FQE nor receiving a Long signal grants live trading permission.

Do not pass broker secrets through strategy definitions or generic UI log streams. A live execution session is separate from an offline engine session.

## Integration acceptance gates

Before implementation is described as available, provide a real local connection, request/event correlation, version/capability checks, process cleanup rules, bounded data flow, disconnect behavior, and an end-to-end test with the actual Desk application. P4-03/P4-05 include saved model/parameter/data binding, explicit simulation intents, verified financial results, native run/stop/reopen and legacy-record handling. P5-02/P5-05 add FQE-managed Python strategies; P6-04/P6-06 repeat the final installed-bundle journey with the final engine/worker combination. The original initialization made no Desk edits; future paired implementation belongs to the owning repositories. This documentation assessment also leaves Desk untouched.
