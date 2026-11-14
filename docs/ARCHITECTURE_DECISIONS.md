# Architecture Decisions

Authority: [Charter](PROJECT_CHARTER.md) → this ADR register → scoped contracts. Accepted on 2026-11-14 for long-term direction. Accepted does **not** mean implemented. See [Status](PROJECT_STATUS.md) and [Delivery plan](DELIVERY_PLAN.md).

## ADR-001 — Rust Core

**Status:** ACCEPTED. **Decision:** Rust owns validation, execution semantics and authoritative engine state; a standalone Rust CLI remains available. **Reason:** one typed implementation for embedded and standalone consumers. **Alternatives:** putting computation in Flutter or separate evaluators per client. **Consequences:** no UI/language runtime/broker models in core; all clients share conformance fixtures. Baseline Rust workspace exists; universal engine remains P1–P3 work.

## ADR-002 — Local-First Runtime

**Status:** ACCEPTED. **Decision:** strategy execution, backtesting, optimization and debugging run locally, including user-controlled servers. Offline work has no login/account/Flutter dependency; no Finger Trader cloud customer-strategy service. **Reason:** independence, local ownership and reproducibility. **Alternatives:** hosted mandatory execution or Desktop-only runtime. **Consequences:** local resource control, datasets and packaging must be proven; external data/connectivity is an optional boundary.

## ADR-003 — Isolated Language Workers

**Status:** ACCEPTED. **Decision:** arbitrary code executes in supervised isolated language workers; deliver Python → Java → Go, with independent language acceptance. **Reason:** preserve language ecosystems without embedding their semantics into IR. **Alternatives:** embedding all runtimes or pretending arbitrary programs compile losslessly into visual graphs. **Consequences:** versioned lifecycle/state/output contracts and real failure/recovery tests; a process alone is not an OS sandbox. Transport, distribution and restriction mechanisms remain open in P5-01/P5-05.

## ADR-004 — Trading SDK Separation

**Status:** ACCEPTED. **Decision:** optional FQE adapter depends on the independent Trading SDK, never the reverse. SDK owns market/account/order/status/WebSocket APIs; FQE owns quantitative computation. **Reason:** SDK-only users must not install FQE. **Alternatives:** broker API implementations in engine or SDK depending on engine types. **Consequences:** explicit mode/permissions, no broker secrets in IR/workers, simulation cannot fall through to live. P4-04 proves the boundary without requiring live trades.

## ADR-005 — Typed Strategy IR and Event Execution

**Status:** ACCEPTED. **Decision:** a language-neutral typed graph plus event workflow and state machine expresses portable strategies. Use a universal type/parameter system, versioned node/function registries and common expressions. Indicators are registered functions, not MACD/RSI/KDJ-specific trading-condition enums. Pure computation is acyclic; cross-event feedback uses explicit State/Delay. Definitions and instance state are separate. **Reason:** visual and declarative clients share semantics without special-case strategy logic. **Alternatives:** indicator-specific enums, untyped graphs, mutable definitions. **Consequences:** compiler validation, graph/type errors, warm-up and state boundaries need executable tests. The current price-only Rust IR is experimental baseline, not fulfillment of this ADR.

## ADR-006 — Vertical Slice Delivery

**Status:** ACCEPTED. **Decision:** deliver executable model-to-result slices within P1–P6, with dependency gates and independent evidence. **Reason:** avoid empty interface scaffolding being counted as capability. **Alternatives:** completing all abstractions first or expanding the demo into all products at once. **Consequences:** no stage completion from documents alone; each task has acceptance/tests, committed evidence and handoff. This governance work does not complete P1.

## ADR-007 — Local Simulation

**Status:** ACCEPTED. **Decision:** FQE owns local virtual accounts, fills, cash/positions, fees/slippage, leverage/margin and versioned broker simulation profiles. No backend simulated account is required. **Reason:** complete independent local research loop. **Alternatives:** server-owned paper accounts or treating signals as fills. **Consequences:** financial numeric/rounding policy and independently calculated ledger tests gate P3. Supported profiles must be explicit; no realism or live-equivalence promise.

## ADR-008 — Versioned Plugin Extensibility

**Status:** ACCEPTED. **Decision:** versioned plugin manifests/registry, dependencies, capability negotiation, permission model and compatible model packaging support third-party indicators/functions/modules and future marketplace consumers. **Reason:** extensibility without core-specific forks. **Alternatives:** unversioned dynamic loading or marketplace-driven engine dependency. **Consequences:** proven load/execute/unload, resource enforcement and fault isolation precede release claims. Native ABI, WASM or out-of-process execution is not selected; P6-01/P6-02 evaluate mechanisms.

## ADR-009 — Versioned Public Contracts

**Status:** ACCEPTED. **Decision:** IR, protocol, registry entries, modules, worker, snapshot and plugin contracts are versioned independently. Protocol Buffers is the first candidate to evaluate, not a proven codec/transport. **Reason:** cross-language exactness and deliberate compatibility handling. **Alternatives:** exposing internal Rust structs as public ABI or inferring compatibility from engine package versions. **Consequences:** presence, exact integers, limits, unknown required capabilities and malformed payload tests. Existing `.proto` is only an experimental draft; P1-03 evaluates encoding and P4-01/P5-01 evaluate transport.

## ADR-010 — One Engine, Optional Desktop Boundary

**Status:** ACCEPTED. **Decision:** evaluate FFI and IPC for Desktop/external consumers; choose from measured correctness, lifecycle, safety, overhead and packaging evidence. CLI and Desktop use identical engine semantics. **Reason:** embeddability without Flutter dependence. **Alternatives:** a second evaluator in Dart or premature transport commitment. **Consequences:** P1-10 provides a minimal external-process smoke test; stable FFI/IPC selection and actual Desktop parity wait for P4-01/P4-05. Current `unsafe_code = forbid` remains intact; any native-boundary exception needs a separately reviewed scope.

## ADR-011 — Desk Embedded Delivery and Migration

**Status:** ACCEPTED on 2026-11-14, owner-directed product clarification. **Decision:** the existing `finger_desk` application is the primary integration target. Deliver the first backtest/local simulation loop inside its supported app package; preserve standalone/offline FQE. Desk is optional to engine users, while actual Desk delivery is required for project completion. FQE owns computation/financial state for FQE runs; Desk owns presentation, saved drafts and client orchestration. Existing Dart/Python paths and old records require explicit versioned migration/compatibility, with one authoritative backend per run. **Reason:** an independent engine must ultimately power the existing Desk quant product, whose working tree already contains local Python execution, financial computation and durable records. **Alternatives:** a separate demo Desktop, postponing consumer semantics until release, or leaving two authoritative ledgers. **Consequences:** [Desk integration](DESK_INTEGRATION_PLAN.md) binds existing P1/P3/P4/P5/P6 acceptance to real consumer contracts, paired candidates and installed native evidence. First scope is historical backtest/local simulation, as confirmed by the owner; live execution is separately accepted. P4 proves portable-model integration, P5 proves Python integration and P6 repeats final bundled acceptance. FFI vs IPC remains open under ADR-010; this decision does not select or implement a mechanism. Prior governance's initialization-only exclusion of Desk edits remains historical, not a permanent exclusion of integration delivery.

## Open decision register

| Decision still open | Owning tasks | Evidence required to accept a choice |
| --- | --- | --- |
| Numeric representation, scale, rounding, overflow, warm-up precision | P1-02, P1-06, P3-01 | Compare viable representations; exact boundary/overflow and independent indicator/ledger fixtures; document supported domains. Positive u64 ticks alone are insufficient. |
| External model format and Protobuf codec | P1-03 | Bounded parse/round-trip, presence/version/unknown capability failures, exact u64 values and dependency/tooling review; no generator assumed approved. |
| State machine schema and checkpoint format | P2-01, P2-04 | Event-boundary semantics, isolation, corrupt/incompatible snapshot rejection and split-run equivalence. |
| Desktop FFI vs IPC, framing and local authentication | P4-01 | Runnable comparison with cancellation, crash/cleanup, exact values, platform feasibility and overhead measurements. |
| Worker transport, runtime versions and restriction model | P5-01, P5-05 | Handshake/backpressure, language conformance, hostile worker and platform resource tests. |
| Plugin execution/registry trust and permission mechanism | P6-01, P6-02 | Proven dependency resolution, load/unload, denied access, quotas and crash containment; declared permissions alone fail the gate. |

Record experiment results in the owning technical contract and add/supersede an ADR when a durable choice is made. No open entry is marked delivered by this document.
