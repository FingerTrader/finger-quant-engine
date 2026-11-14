# Compiled event execution

Status: experimental P1-08 implementation. This contract owns `fqe.event-plan/1` and the in-process `EventRuntime`; [Graph compiler](GRAPH_COMPILER.md), [Expressions](EXPRESSION_CONTRACT.md), [Indicators](INDICATOR_CONTRACT.md) and [Events](EVENT_CONTRACT.md) retain their underlying semantics. The original IR1 `Runtime`/demo remains a separate, unchanged one-shot API. The separate [P1-09 CLI slice](CLI_EXECUTION.md) now provides the bounded local file runner; historical datasets, trading, ledger, worker, checkpoint and Desk integration remain future work.

## Versioned event artifact

`LoadedEventPlan::from_bytes` loads strict JSON with these required fields:

| Field | Version 1 meaning |
| --- | --- |
| format / schema_version | `"fqe.event-plan"` / JSON integer `1` |
| id / version | Validated 1–128 byte logical identifiers |
| requirements | Exactly `event.plan/1` and `expression.scalar/1`, once each |
| graph | An inline, complete `fqe.graph/1` artifact, retaining all of its validation and exact provider requirements |
| trigger | One of the distinct tick/closed-bar declarations below |
| bindings | Object mapping every graph input name to one event field; no missing/extra graph inputs |
| warmup_samples | Canonical unsigned u64 decimal string; the first N matched events cannot emit signals |
| crossings | Named crossing subscriptions `{id,direction,left,right}`; direction `above` or `below` |
| outputs | Nonempty named roots `{name,expression}` |
| signals | `{output,kind}` rules referencing Boolean roots; kind `long` or `flat`; at most one rule per root |

Tick trigger: `{"kind":"tick","instrument":"DEMO"}`. Its only field is `price`, typed U64 in caller-defined ticks. Closed-bar trigger: `{"kind":"closed_bar","instrument":"DEMO","timeframe_ns":"1000","price_scale":12,"volume_scale":0}`. Timeframe is a positive canonical u64 string; scales are JSON integers. Fields `open/high/low/close` are Decimal Price(instrument), scale 0–12; `volume` is Decimal Quantity(instrument), scale 0–18. All bindings require exact types, units and scales. No implicit conversion, future field or tick-to-bar relabeling exists. Multiple graph inputs may deliberately bind the same visible field.

Portable scalar expressions each have exactly one tag:

```json
{"literal":{"type":{"version":1,"shape":"scalar","kind":"u64"},"value":"100"}}
{"parameter":"threshold"}
{"input":"sample"}
{"output":"macd_line"}
{"cross":"up"}
{"not":{"cross":"up"}}
{"binary":{"op":"and","left":{"cross":"up"},"right":{"literal":{"type":{"version":1,"shape":"scalar","kind":"bool"},"value":true}}}}
```

`input` names a graph public input, `output` names a graph public output (not another root), `cross` names a crossing subscription, and `parameter` binds a root graph parameter to an immutable typed literal. Unbound or unknown parameters fail compilation. Provider/module calls remain in the graph; no executable source, language selector, financial-name dispatch, implicit output references or expression-side stateful call exists.

Binary operations: `add`, `subtract`, `multiply`, `divide`, `equal`, `not_equal`, `less`, `less_equal`, `greater`, `greater_equal`, `and`, `or`. Only multiply/divide may carry `rounding`: `exact` (default), `half_even`, `toward_zero`; the type checker further restricts legal numeric/unit/rounding combinations. Non-Boolean numeric payloads remain exact canonical integer strings; no float or JavaScript-number coercion is used. All [P1-05 readiness and short-circuit rules](EXPRESSION_CONTRACT.md) apply.

Unknown fields/tags/versions/capabilities, duplicate JSON keys or named roots/crossings/signal rules, wrong types, missing roots and non-Boolean signals fail before execution. A whole loaded artifact is structurally validated by `from_bytes`; provider resolution, root/cross type checking, bindings and final resource limits are completed by `compile`. Import success alone does not mean runnable.

## Bounds and identity

The entire wrapper retains the loader limits: 262144 source/canonical bytes, JSON depth32, 16384 values, collection1024, key128 and string4096 bytes. Inner graph limits still apply. Version 1 adds at most 32 roots/signals, 256 crossings and **32 total slots**, counting graph inputs plus every output port of every incremental node, including unused subscriptions. This explicit scalar execution profile is narrower than the compiler's 256-node graph bound. Additional scales/shapes/slots require a versioned profile rather than silent acceptance.

Encoded expression ASTs share a 4096-node/32-depth limit, intersected with the JSON depth bound. Lowering pure graph paths into all stage-input and root/cross ASTs shares another 4096-node/32-depth expansion budget; the existing ExpressionPlan limits also apply to each plan. Deep/shared pure DAGs may exceed that budget despite being valid graph artifacts. Pure calls can repeat across expression positions; incremental node state never does. Zero-input incremental providers are supported without a dummy input.

Canonical JSON uses compact sorted object keys, named graph declarations and wrapper crossings/roots/signal rules in lexical order, normalized literals and explicit multiply/divide rounding. Boolean operand order is preserved. Event artifact ID is SHA-256(`FQE-EVENT-PLAN-CANONICAL/1\n` + canonical bytes); source ID is SHA-256(raw source bytes). Execution ID is SHA-256(`FQE-EVENT-EXECUTION/1\n` + compact sorted-key JSON with `artifact` and `graph` IDs). The compiled graph ID already includes frozen registry, descriptors, bindings and module closure. Whitespace/reordering named declarations preserves semantic IDs; changing trigger, expression, warmup, parameter bindings or provider pins changes them. Pins are not signatures or binary attestations.

## Run ownership and processing

`EventRuntime::new(Arc<CompiledEventPlan>, run, dataset_id)` validates the run identifier, then creates fresh per-node state and crossing memory. Reusing an immutable plan shares no mutable engine-owned history. The manifest records run, caller-supplied dataset ID, raw and canonical event artifact IDs, execution/graph/registry IDs, trigger, scales/duration via the trigger, interval convention and declared warmup samples. Dataset ID is a caller assertion; this API does not read/hash a data file or attest to its contents. [P1-09](CLI_EXECUTION.md) now hashes the actual local input bytes for its dataset ID; P3 owns persisted historical sources.

`step(InputEvent)` processes a single event synchronously and returns an `EventObservation` with run/execution ID, accepted source sequence/time, matched/warmup flags, typed/readiness outputs and source-linked signal records. `InputEvent::from(BarInput)` boxes a raw bar; the runtime always validates it rather than trusting a caller's `closed` flag. No output backlog or iterator is retained by the runtime. The caller owns result storage and must handle rejected events explicitly.

1. Validate event family, all closed-bar values/domain/continuity and monotonic sequence/time before touching history. For bars, every OHLC has the declared Price type and absolute value ≤1e12, including zero/negative prices; volume is nonnegative declared Quantity. UTC ns interval is `[open, close)` with `close-open=timeframe` and event timestamp exactly close. Next open equals the last accepted close. No gaps/overlap/unclosed/mixed instrument/timeframe/calendar repair. Logical sequence gaps alone are allowed.
2. Tick sequences/times follow the old ordering rules. An unrelated instrument consumes ordering/counts but evaluates no subscriptions/roots and returns empty outputs/signals. Bar domain mismatches are rejected. Tick/bar families cannot mix within a run.
3. Deep-fork every incremental instance and clone crossing history into candidates. In stable compiled dependency order, evaluate pure arguments and advance **each** incremental node once, storing all validated output ports in slots. Every incremental node is subscribed, including those unused by roots. An upstream Missing/Warmup resets downstream state under the existing provider contract. Provider Error, panic, bad output or numeric error aborts the event.
4. Evaluate all crossing subscriptions once in stable dependency order, then all named roots. Shared crossing references read one result. False guards and operand ordering cannot skip a subscription; pure root expressions retain left-to-right short-circuit, including skipping an unevaluated error. Missing/Warmup crossing inputs reset that crossing's candidate history; the first later ready pair cannot fabricate a crossing.
5. Mark warmup when within the declared first N matched events or **any subscribed incremental output** is not Ready. Roots remain observable, but warmup produces no signals. This conservative policy includes unused node ports. A true, ready Boolean signal root emits once per accepted eligible event; it is an observation, not an order or a position transition.
6. Only after all stages/roots succeed, publish candidates, crossing history, ordering/time/bar cursors, counts and the observation together. Any rejection returns no observation/signals and leaves all engine-owned state unchanged. Same-sequence retry is allowed because the failed event was never accepted. The caller may supply a corrected payload; its dataset provenance must reflect that choice. External side effects and process failures are outside this in-memory guarantee.

New lifecycle: Ready → Running on accepted events → Completed by explicit `finish()`; finishing an empty run is valid. Repeated finish and steps after completion fail. Validation/provider/evaluation errors retain the prior Ready/Running state and return a structured error. This deliberate retry policy differs from legacy `Runtime::run`, whose error terminates that one-shot run as Failed. There is no mixed policy or implicit auto-retry. Node failures carry their original graph declaration path; root errors retain expression diagnostics. Counters use checked arithmetic.

`IncrementalState::fork` must deep-copy mutable state; trusted Rust providers may not retain shared mutable history or perform external side effects. Panics in provider calls/forks become errors, but this is not sandboxing, allocation-failure recovery, process-crash durability or an external transaction protocol. Copying state twice per event (host and node staging) favors correctness in this experimental slice; no throughput/latency claim is made.

## Evidence and limits

[Runtime tests](../crates/fqe-runtime/tests/event_execution.rs) cover G-002 MACD(2,3,2)/RSI(2)>60 and G-007 SMA(2)/SMA(3), both signaling only at bar5, with threshold90 producing no G-002 signal. They also cover chained increments, false guards/reordering/shared crossings, zero-input unknown-name providers, missing resets, pure lazy errors, later-node/root/fork failures and same-sequence retry, isolated runs, prefix invariance, warmup, malformed bars, domain/time/sequence boundaries and unrelated tick filtering. Expected indicator samples remain from the prior independent rational fixtures; event signal expectations are explicit goldens, not generated by running the engine.

[Event format tests](../crates/fqe-ir/tests/event_plan_contract.rs) cover strict decoding, types/capabilities/pins, bounds, canonical identity and semantic changes. [Stdlib fixture builder](../scripts/generate_event_fixtures.py) writes new artifacts and an independently calculated tick canonical byte/hash reference; tests never regenerate reference files. Financial provider sources and earlier graph/model fixtures are unchanged.

```sh
cargo test --locked --offline -p fqe-ir --test event_plan_contract
cargo test --locked --offline -p fqe-runtime --test event_execution
cargo run --locked --offline -p fqe-runtime --example events
```

[P1-09](CLI_EXECUTION.md) exposes actual loaded CLI validation/run, bounded input/result codecs and diagnostics. P1-10 must prove an independent caller; P1-11 owns the complete P1 gate. P2 owns generalized scheduling/delays/cancellation/persistent state; P3 owns backtest accounting/local simulation; P4/P5/P6 own real Desk, language adapters, packaging and installed acceptance. This library path does not complete those milestones.
