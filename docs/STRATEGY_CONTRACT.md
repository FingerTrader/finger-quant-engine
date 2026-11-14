# Strategy contract

Status: a small typed IR and evaluator are implemented experimentally. P1-02 also implements an independent [type/parameter subset](TYPE_PARAMETER_CONTRACT.md), exposed through fqe-core numeric/types and fqe-ir parameters, with its own contract versions. It is not yet consumed by the legacy strategy evaluator. Serialized strategy ingestion, source compilers, and worker execution are planned.

The universal target contract is [P1 strategy design](P1_STRATEGY_DESIGN.md): definition/instance separation, types/parameters, registered nodes/functions, typed graph, explicit workflow/state, modules and requirements. It does not change the current API below. The [Extensibility contract](EXTENSIBILITY_CONTRACT.md) defines the planned open descriptor/provider/rule boundary: new conforming names use generic signatures rather than new Rust enum variants; unsupported host types/events still require versioned evolution. Implementation gaps and owning tasks are in [Baseline audit](BASELINE_AUDIT.md); progress belongs to [Project status](PROJECT_STATUS.md).

## Current typed model

`fqe-ir::Strategy::new` validates an explicit IR version, strategy ID, instrument, predicate, and output signal kind. Its private fields prevent bypassing constructor validation in safe Rust.

| Field | Current rule |
| --- | --- |
| IR version | Exactly `IR_VERSION = 1`; all other values are rejected |
| Strategy ID | 1–128 ASCII letters, digits, `.`, `_`, or `-`; case-sensitive |
| Instrument | Validated `InstrumentId`; 1–128 ASCII letters, digits, or `:/._-` |
| Predicate | `GreaterThan { left: PriceExpr, right: PriceExpr }` |
| Price expression | Current event price or a positive integer `PriceTicks` literal |
| Output | `SignalKind::Long` or `SignalKind::Flat` |

Price and predicate types are distinct. A comparison consumes two price-valued expressions and yields a predicate result. There are no implicit string conversions, floating point values, or truthiness rules.

On an event for the configured instrument, the rule evaluates a strict greater-than comparison. Equality is false. Every matching event emits one signal. Unrelated instruments emit no signal. There is no deduplication, indicator memory, account state, or position sizing.

The integer tick scale is supplied by the caller and must match the instrument's dataset. Scale metadata is not implemented, so the demo is not a financial pricing model. Zero and negative prices are unsupported in this initial subset; instruments requiring those values need a future type revision.

## Example semantics

The demo strategy compares current price to 100 ticks and emits Long:

| Input price | Signal |
| --- | --- |
| 90 | None |
| 100 | None |
| 110 | Long |
| 105 after 110 | Long again |

Long and Flat are observations only. Flat does not cancel orders or close positions. Neither signal contains a quantity, account, order type, or trading authorization.

## Language-neutral meaning

The contract describes data and behavior rather than a Python callback, JVM interface, Go function, or Flutter object. The Rust enum model is the first executable reference for this restricted IR. Cross-language conformance is planned and has not been demonstrated.

A future compiler may lower a supported declarative subset into IR. A future worker may execute imperative language code behind the worker contract. An arbitrary source program is not automatically portable IR; unsupported features must be rejected explicitly or retained in worker execution mode.

## Draft wire mapping

The experimental `StrategyDefinition` in [engine.proto](../crates/fqe-protocol/proto/fqe/v1alpha1/engine.proto) sketches the same comparison model. Protobuf scalar defaults and optional message presence do not satisfy domain validation automatically.

Before a decoder can be considered implemented, it must reject unsupported versions, missing expressions, unspecified signal enums, invalid IDs, zero price literals, oversized input, and unknown required capabilities. No decoder or Rust/wire conversion exists today. The Rust demo constructs the typed model directly.

## Planned extensions

Future versions may add explicit parameter schemas, indicator types, historical windows, boolean composition, instrument metadata, state declarations, resource limits, and execution permissions. Each extension needs semantics, validation, compatibility policy, and conformance fixtures before publication.

Strategy IR version is distinct from package version and protocol version. Version 1 is an experimental development marker, not a stable compatibility promise. Do not advertise an importable strategy format or language SDK until its parser and compatibility tests exist.
