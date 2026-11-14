# Roadmap

Status describes repository capabilities, not a release schedule. All computation in every phase remains local to the user's device.

## Status vocabulary

- **Implemented:** code exists and can be exercised by the current build/tests.
- **Experimental:** implementation or schema exists but its API/semantics are not stable.
- **Planned:** no usable implementation is claimed.

An implemented component can also be experimental. The Cargo package version `0.1.0` does not imply a published binary, SDK, or production release.

## Phase 0 — engineering foundation (implemented)

- Independent repository on main, preserving original history and Apache-2.0 license.
- Five-crate Cargo Workspace with no third-party Cargo dependencies.
- Validated core types and a small experimental typed strategy IR.
- Deterministic in-memory event loop, explicit run state, signal provenance.
- Standalone offline CLI demonstration, help, and version commands.
- Experimental Protocol Buffers schema and a version-number negotiation helper.
- Architecture/contracts and behavioral tests.

The demo is not a backtest, a simulator, or a language worker. The schema has no generated bindings, codec, IPC, or stable compatibility guarantee.

## Phase 1 — contract validation (planned; recommended next)

Scope a single vertical slice for strategy/event wire conversion. Validate protobuf tooling and dependencies before introduction. Add bounded decode/validation, version and presence handling, exact u64 conformance, and shared fixtures that match the current Rust evaluator.

Specify instrument price-scale metadata and a reproducible run manifest before accepting real financial datasets. Decide whether to keep the initial positive-only price domain or revise it for explicitly supported instruments.

Exit gate: one small offline strategy and event fixture round-trips through a real codec and produces the same validated result as the typed Rust model. A transport or service is not required to prove this first.

## Phase 2 — local replay and accounting baseline (planned)

Add a narrow local dataset format and deterministic replay. Define order intents, signal-to-order policy, one explicit execution model, and an exact ledger with manually calculated fixtures.

Exit gate: a reproducible offline backtest with independently checkable fills, cash, positions, costs, and report provenance. No performance claim precedes this gate.

## Phase 3 — simulation and host controls (planned)

Introduce bounded resources, cancellation, observable lifecycle, fault handling, and a documented local simulation model. Define persistence/recovery requirements before claiming resumable runs.

Exit gate: consistent event/accounting contracts across replay and simulation, with fault and reconciliation tests.

## Phase 4 — language workers (planned)

Build one reference worker first. Define process protocol, supervision, artifact handling, capability checks, declared state, and conformance. Expand to Python, Java, and Go based on validated implementation needs.

Exit gate per language: runnable worker plus host integration, lifecycle/failure tests, distribution instructions, and shared exact-value fixtures. Do not claim all languages from one prototype.

## Phase 5 — local optimization and plugins (planned)

Add reproducible bounded parameter trials and out-of-sample evaluation. Introduce one proven plugin extension point before a general registry or loader.

Exit gate: deterministic trial provenance and explicit plugin authority/failure semantics.

## Phase 6 — optional clients and execution adapters (planned)

Integrate Flutter Desktop over a defined local protocol without coupling the engine to Flutter or login. Add a separately authorized Trading SDK adapter only after execution, credential, and account boundaries are specified.

Exit gate: end-to-end local client tests and explicit mode/permission controls. Offline engine capabilities remain independently usable.

Phases may be refined as contracts become concrete. There are no dates or feature availability promises attached to this roadmap.
