# Desktop integration

Status: planned. The FQE CLI is implemented; no Flutter package, Dart binding, FFI bridge, local daemon, or Desktop connection is implemented.

## Independence

FQE can be built, tested, and run without Flutter Desktop. Its offline demonstration requires no login or trading account. All strategy computation stays on the user's device.

Flutter Desktop will be an optional presentation and orchestration client. It may configure a run, launch or connect to a local engine, inspect lifecycle, and display results. It must not become a build dependency of the Rust core or the authority for engine state and financial calculations.

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

Before implementation is described as available, provide a real local connection, request/event correlation, version/capability checks, process cleanup rules, bounded data flow, disconnect behavior, and an end-to-end test with Flutter Desktop. No changes to the existing finger-desk application are part of this repository initialization.
