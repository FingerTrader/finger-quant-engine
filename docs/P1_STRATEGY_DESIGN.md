# P1 Universal Strategy Design

Status: implementation target for P1/P2, **not an implemented file format**. The experimental [P1-02 type/parameter subset](TYPE_PARAMETER_CONTRACT.md) now has executable Rust APIs/tests; all remaining universal graph/model/runtime features below retain their planned status. Accepted architectural constraints come from [ADRs](ARCHITECTURE_DECISIONS.md); representation/codec/numeric choices stay open until their owning tasks validate them. [Current Strategy contract](STRATEGY_CONTRACT.md) remains the accurate description of the experimental price-only model. [Delivery plan](DELIVERY_PLAN.md) assigns every target below to an executable task.

## Definition, version and instance

| Entity | Required meaning and fields | Owner / delivery |
| --- | --- | --- |
| Strategy Definition | Immutable portable artifact: schema/IR version, strategy ID, strategy version, parameter schema, graph, workflow, declared state, modules, requirements, dependencies, metadata | P1-03; no live cursor/account secrets |
| Strategy Version | Author version plus immutable artifact content identity; independent of IR schema and engine release | P1-03; same version with different content must not silently replace a running artifact |
| Strategy Instance | Unique instance/run ID, pinned definition identity, validated bound parameters, instrument bindings, dataset identity, execution mode and owned runtime state | P1-08 basic instance, P2 generalized lifecycle |
| Compiled Plan | Validated/resolved graph, deterministic evaluation order, typed bindings, source map and required capabilities | P1-07; cache key includes definition, bindings and registry versions |
| Snapshot | Versioned instance state, event cursor, buffers, workflow state, timer/window state, seeds and applicable financial state | P2-04; never mistaken for a definition |

A definition can create many instances; no mutable state is shared between them by default. Editing a model creates a new artifact/version, not mutation of a running plan. Hot replacement/migration is unsupported until explicitly specified. A worker strategy declares execution mode and entrypoint/artifact requirements; its arbitrary source code is not a visual graph.

## Universal type system and parameters — P1-02

The type system distinguishes Bool, signed/unsigned integers, numeric scalars with explicit precision policy, String, bounded enums, Timestamp, Duration, InstrumentRef, and domain quantities such as Price, Quantity, Money and Ratio. Units include currency/instrument scale where applicable. P1 implements the bounded subset needed for the first slice and explicitly rejects unsupported types; it must not advertise all types from this list merely through enum variants.

Series values carry event domain (symbol/timeframe), readiness and underlying type. Scalar and series coercions must be explicit or compiler-proven; currency, timeframe and instrument mismatches cannot be silently combined. No string truthiness or implicit price-to-money conversion. Preserve u64 values across external representations. Evaluate exact/fixed/decimal/floating candidates and document supported ranges, rounding and overflow before choosing storage; current positive-only `PriceTicks` is not the final universal numeric type.

Separate **Missing/Warmup** from valid zero and from **Error**. Each function declares minimum history and readiness propagation. Lifted comparisons on missing inputs produce missing Bool; workflow guards fire only on ready true. Logical evaluation has a specified order/short-circuit policy; missing is never silently numeric zero. Runtime arithmetic errors produce structured diagnostics and fail the current event transaction rather than committing partial state. P1-02/P1-05 pin and test these rules.

Parameter schema entries need stable key, type/unit, required flag, optional typed default, bounds/inclusivity, enum set, description and mutability policy. Validate the schema's own defaults and constraints, then user bindings; reject missing required, unknown, duplicate, mistyped or out-of-range values. Bound parameters are immutable during a run. Parameter names are references, never executable source. P1-03 defines canonical serialization and bounded nesting/string/collection limits.

P1-02 must implement and test this binding matrix for every supported parameter type:

| Schema / supplied value | Required result |
| --- | --- |
| Required, no default, omitted | Binding error before instance creation |
| Required and default both declared | Invalid schema; required means the caller must supply the value |
| Optional with valid typed default, omitted | Bind a validated immutable copy of the default |
| Optional with no default, omitted | Remain unbound; a graph reference is rejected unless an explicitly supported optional-value operation handles absence |
| Explicit null, Missing, Warmup or Error | Reject as parameter input in P1; runtime readiness is separate from an omitted binding |
| Supplied value with wrong type, unit or scale | Reject; no implicit integer/float/string/domain conversion |
| Supplied valid value and a default exists | Supplied value takes precedence, then all type/unit/range/enum constraints apply |
| Duplicate schema key/binding key, unknown key, contradictory bounds, or invalid default | Reject before execution; parsers must detect duplicates before map construction loses them |

Cross-parameter constraints such as MACD fast < slow belong to the versioned function validator; individual bounds alone do not satisfy them. Unsupported parameter types are rejected rather than accepted as opaque values. P1-02 tests the in-process matrix; P1-03 repeats it through the selected external encoding.

## Typed graph and registries — P1-04 / P1-07

A graph contains stable node IDs, versioned node-kind references, typed configuration, named input/output ports and edges. Ports specify direction, data type, unit, cardinality, scalar/series domain and required/optional status. Edges name source node/output and destination node/input. Required ports must be bound once unless a descriptor explicitly declares variadic cardinality. Order in a visual editor must not change semantics.

Node descriptors declare ID/version, category, port signatures, parameter schema, purity/statefulness, capabilities, readiness, resource bounds and deterministic behavior. Categories include input, constant/parameter, expression, registered function, state-read/delay, state-write and signal/intent output. Registry lookup is deterministic and version-pinned; duplicate registrations, unknown IDs, incompatible versions, forbidden capabilities and invalid arity fail validation. Registration does not execute an installer.

Indicator descriptors are specialized function descriptors: ID/version, formula, input series, period parameters, output names/types, seed/warm-up rules, buffer needs and numeric policy. MACD, RSI and later KDJ use the same function-call mechanism. There is no `MacdBuyCondition` / `RsiSellCondition` family. A plugin can supply a future compatible descriptor only after plugin gates; P1 registries contain reviewed built-ins and an independently defined reviewed in-process test provider; external loading waits for P6.

The shared [Extensibility contract](EXTENSIBILITY_CONTRACT.md) governs callable providers, data shapes, generic invocation and UI metadata. P1-04 must allow an independently defined reviewed in-process test provider through the same descriptor/interface used by built-ins; P1-07/P1-11 prove X-001 compositions with a name unknown to core/compiler, including a Boolean predicate. No external artifact loading, process execution or sandbox claim is required in P1. Built-ins and test providers cannot rely on per-indicator compiler branches. Descriptor negotiation must identify unsupported required shapes instead of treating them as untyped blobs.

The compiler resolves parameters/modules/functions, validates types/ports/domains, expands bounded modules, detects pure cycles, builds a stable topological order and retains a source map. Tie-break by stable node identity, not hash-map iteration. Direct/indirect pure loops and recursive module expansion fail. State-read returns committed prior-event memory; state-write commits after successful evaluation. Feedback requires an explicit versioned State/Delay node; a delay cannot hide an illegal same-event dependency. P1 may reject generalized control/state nodes pending P2 rather than treating them as no-ops.

## Expressions and indicator functions — P1-05 / P1-06

Expressions include typed literals/references, arithmetic, comparison, Boolean combination and registry calls. Operators validate signature, units and readiness before execution. Comparisons include strict/equality distinctions; arithmetic defines division-by-zero and overflow behavior. Graph condition examples such as `cross_above(macd.line, macd.signal) AND rsi.value > threshold` use ordinary operators on named function outputs.

Cross-above means previous ready `left <= right` and current ready `left > right`; cross-below reverses inequalities. No first-ready event crosses without a prior ready pair. Crossing stores prior accepted values per instance and updates once per accepted event; invalid events do not advance memory. Any gap/readiness reset policy must be declared and tested. P1 uses a single closed-bar series without gaps; P2 owns multi-domain alignment.

P1-05/P1-08 separate history advancement from Boolean short-circuiting. For each matching accepted event, evaluate each subscribed indicator/crossing node exactly once in stable dependency order against committed prior history and current inputs; stage its next history and expose its current output. Pure Boolean expressions then evaluate left to right and may short-circuit, but cannot skip that history advancement or trigger a second update when an output is referenced twice. An error in any subscribed stateful node fails the event even if a later Boolean branch would ignore its output. Signals and staged history commit together only after successful event evaluation. Pure arithmetic in a skipped branch is not evaluated. Missing/Warmup truth tables and diagnostic precedence are specified and tested in P1-05. Regression fixtures must cover false guards across several bars, equivalent AND operand reorderings with ready values, shared node references and failure rollback.

Reference indicator semantics for P1 fixtures (numeric storage still pending evaluation):

- SMA(period p): require positive p; mean of the most recent p accepted close samples, including the current sample; fewer than p samples is warm-up. P1-06 registers this ordinary function for the existing Desk visual-strategy migration, without adding SMA-specific trading-condition enums.
- EMA(period p): first ready value is arithmetic mean of p samples; subsequent value is `2/(p+1) * x + (1-2/(p+1)) * previous`. Fewer than p samples is warm-up.
- MACD(fast, slow, signal): require positive `fast < slow`; line = fast EMA − slow EMA. Signal is EMA of ready line values, with the same mean seed. Histogram = line − signal, without an implicit factor of two.
- RSI(period p): first ready after p price differences; seed mean gains/losses, then Wilder smoothing `(previous*(p-1)+current)/p`. RSI = `100*g/(g+l)`; only gains gives 100, only losses 0, both zero 50. This flat-series convention is explicit and versioned.

Changing a formula, seed, flat-series rule or rounding semantics changes the function contract version. Reference fixtures must derive expected values independently of production functions; floating approximations, if selected, require justified per-output tolerance and stable signal-boundary tests. Financial ledgers in P3 need their own exact accounting policy.

## Event workflow, time windows and state machine

An event trigger declares family (initially closed bar/tick), symbol/timeframe binding and evaluation phase. P1 supplies basic trigger-to-graph-to-signal dispatch; generalized timers, multi-symbol/timeframe scheduling and multi-event workflows are P2.

A workflow is an explicit state machine with named initial/terminal states, guarded transitions, transition priority, typed effects and reset/expiry policy. No unrestricted code is embedded in guards. Evaluate guards against the same pre-event snapshot; select at most one transition per region by declared priority, reject ambiguous equal-priority transitions. Compute effects before atomic commit. Event-time timers are not wall-clock reads during replay. Count-based windows count qualifying closed bars, not arbitrary input events. Time-based windows specify event domain, inclusive/exclusive boundaries, calendar/gap behavior and watermark closure.

The pre-event snapshot rule applies to stored workflow variables. Guards may also read the current event's computed graph outputs; staged writes from another guard/effect remain invisible. For G-006, on each qualifying bar in (k,k+3], compute `satisfied_now = latched_before OR rsi_condition_current` (only ready true qualifies). Before expiry, stage this latch; at k+3, emit once if satisfied_now, then reset/expire in the same atomic transition. This includes RSI first becoming true exactly at k+3 without reading staged writes. A re-cross while armed, including the expiry bar, is ignored; expiry consumes that bar and cannot re-arm until a later fresh cross. P2-01/P2-03/P2-06 must test these priorities explicitly.

State variables declare key/type/default/namespace/version and owner; buffers and indicators belong to the instance. Reads see committed pre-event state; staged writes become visible at the next accepted event. Disallow conflicting writes. Explicit Delay describes initial value and lag; missing history cannot be fabricated. Snapshot captures all state needed for deterministic continuation, including indicator and crossing buffers, pending timers and workflow state. A copied baseline RunState counter is insufficient.

The planned lifecycle is created → validated → ready → running → completed/stopped/failed, with checkpoint/restore at accepted-event boundaries. Cancellation, restored-run identity and failure/retry transitions are specified in P2, not backported by documentation to the existing one-shot runtime. Baseline remains Ready → Running → Completed/Failed.

## Trading intents and reusable modules

P1 emits observations with definition/instance/source-node/event provenance. A future TradingIntent is a separately typed output: explicit intent ID, mode, instrument, side/action, quantity and unit, order policy, source/correlation and required capability. Runtime validates it; simulator or an explicitly authorized SDK adapter handles downstream execution. No signal-to-fill shortcut, hidden account selection or credentials in the model. Intent/account/order/fill contracts are P3/P4 work.

Reusable modules have stable ID/version/content identity, parameter schema, typed public ports, exported state namespace and declared dependencies. Compilation expands or links only a supported bounded form, rejects recursion, and maps diagnostics to module origin. Instances namespace module state; reuse must not share mutable buffers accidentally. P1-07 proves a local pure module; third-party resolution/packaging waits for P6.

A custom rule can be an expression, reusable pure/stateful module, registered typed predicate or whole imperative strategy, with different permissions and lifecycle. A predicate must be usable in ordinary graph guards without a new condition enum. P1 proves expression/pure-module composition; P2 proves stateful modules and portable extension state; P5/P6 map external providers to the same semantic interface. Execution remains capability-gated. New event/type/host-service semantics require explicit versioned evolution rather than an unrestricted generic escape hatch.

Runtime requirements declare supported IR/functions, event families, numeric features and resource budgets. Plugin dependencies declare exact compatible identity/version/capability/artifact requirements. Unknown required capabilities/dependencies fail closed before execution; optional features need explicit fallback semantics. P1 validates declarations and rejects unavailable plugins; it does not implement a loader or registry service.

## Validation pipeline and diagnostics

1. Bounded decode, schema/version/presence/identity validation (P1-03).
2. Parameter binding, type/unit/numeric validation (P1-02).
3. Resolve registries/modules/required capabilities and dependencies (P1-04/P1-07).
4. Validate ports, graph domains, acyclicity, workflow references/state writes and expansion budgets (P1-07; full workflow P2-01).
5. Compile stable plan with source map and declared state/buffer requirements.
6. Validate runtime input/order; evaluate atomically; trace node/event decisions (P1-08, P2 trace).

Diagnostics require stable code, artifact/version, node/port/parameter path, expected/actual and severity, with bounded messages and no secrets. Unsupported features are errors, not silent omissions. A successful parse is not a validated or runnable strategy.

## Example strategies and golden fixtures

These are normative semantic fixture specifications to implement in P1-03 through P1-11, not currently importable files. The external syntax is deliberately not frozen before P1-03. Store actual versioned artifacts/data/expected outputs under a tracked fixture directory during those tasks; include schema/function versions, seed/numeric policy and independent calculation notes.

| Fixture ID | Model / data | Expected result | Gate |
| --- | --- | --- | --- |
| G-001 | Legacy current-price >100, prices 90/100/110/105 | No signal for first two, then signals at 3/4; strict comparison | P1 regression |
| G-002 | EMA(2), EMA(3), MACD(2,3,2), RSI(2); closes 1/2/3/2/4/3 at bars 1..6 | Reference table below; readiness respected | P1-06 |
| G-003 | `cross_above(macd.line, macd.signal) AND rsi > 60`, G-002 data | Exactly one signal at bar 5; threshold 90 gives none; edit artifact/parameter without rebuild | P1-09/P1-11 |
| G-004 | Same valid G-003 model invoked from library, CLI, external caller | Identical event-linked decisions and terminal status | P1-10/P1-11 |
| G-005 | Bad version/type/port/ref, pure cycle, recursive module, unknown function/plugin, missing required parameter, oversized artifact | Stable validation error, zero execution/state mutation | P1-03/P1-07/P1-11 |
| G-006 | MACD cross at k, RSI > threshold occurs within next 3 closed bars | Latch qualifying RSI in (k,k+3]; emit once at k+3 only if latched; otherwise expire. Re-cross while armed is ignored; a later fresh cross can re-arm | P2-06 |
| G-007 | SMA(2), SMA(3), cross-above; closes 3/2/1/2/3/4 | SMA2: warm-up, 5/2, 3/2, 3/2, 5/2, 7/2; SMA3: warm-up, warm-up, 2, 5/3, 2, 3. Exactly one up-cross at bar 5; no cross at first-ready bar 3 | P1-06/P1-11; P4 Desk visual compatibility |

Independent rational reference for G-002 (dash = warm-up):

| Bar | Close | Fast EMA | Slow EMA | MACD line | Signal EMA | Histogram | RSI |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 1 | — | — | — | — | — | — |
| 2 | 2 | 3/2 | — | — | — | — | — |
| 3 | 3 | 5/2 | 2 | 1/2 | — | — | 100 |
| 4 | 2 | 13/6 | 2 | 1/6 | 1/3 | -1/6 | 50 |
| 5 | 4 | 61/18 | 3 | 7/18 | 10/27 | 1/54 | 250/3 |
| 6 | 3 | 169/54 | 3 | 7/54 | 17/81 | -13/162 | 50 |

G-006 must include RSI true only at k+1 (still emits at k+3), true first/only at k+3 (emits at k+3), true only at k+4 (no emission), re-cross on expiry (no same-bar re-arm), failed/expired waits, unrelated-symbol events, duplicate/backward events, and snapshot splits at each wait point. Prefix-run outputs must never change when future bars are appended. The minimum P1 closed-bar contract is owned by P1-08 in [Events](EVENT_CONTRACT.md#p1-closed-bar-target); generalized alignment extends it in P2. This specification does not imply P2 exists.

## P1 completion boundary

P1 succeeds only with real loaded artifacts, generic typed evaluation, registered MACD/RSI, compiled validation, external-call parity, independent golden results and committed evidence. A `.proto` sketch, Rust-constructed demo, registry interface placeholder or this design document alone cannot satisfy it. See [Audit gaps](BASELINE_AUDIT.md) and the P1-11 gate.

P1-11 also requires the X-001 unknown-name provider and reusable-rule composition acceptance in EXTENSIBILITY_CONTRACT. The existing G-001–G-007 financial-indicator examples remain unchanged. External workers/plugins are still later gates; their future interfaces must preserve this generic descriptor/invocation boundary.
