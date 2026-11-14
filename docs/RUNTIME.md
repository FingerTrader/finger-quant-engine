# Runtime

Status: the legacy synchronous interpreter and P1-08 compiled streaming event runtime are implemented experimentally. [Compiled event execution](EVENT_EXECUTION.md) owns the new API, atomic state and retry policy. The sections below describe the preserved legacy API. Process supervision, live services, asynchronous execution, persistence, and production resource controls are planned.

## Legacy one-shot execution

`Runtime::new(strategy)` starts in Ready. A single `run(events)` call consumes an `IntoIterator<Item = Event>`, validates ordering, evaluates the typed rule, and returns a `RunReport`. The interpreter runs in the calling thread.

For each input:

1. Reject a duplicate/decreasing sequence or backward timestamp.
2. Check the processed-event counter for overflow.
3. Evaluate the strategy for the configured instrument.
4. Append a signal record if the predicate is true.
5. Commit the accepted-input count and ordering keys in memory.

Completion returns the final state and ordered signal vector. An invalid event causes Failed and an error; it is not counted. Partial signals are not returned. Reuse of the same runtime instance is rejected. See [State contract](STATE_CONTRACT.md) for exact transitions.

The CLI uses a small synthetic vector, but the runtime accepts any synchronous iterator. It does not own an event queue, background thread, external data stream, or live wall clock.

## Determinism boundary

Given the same validated strategy, ordered events, and current implementation, fresh runtime instances produce identical reports. Current evaluation uses exact integer comparison and no random source, external IO, or system time. Tests check repeated reports and signal provenance.

This does not promise determinism for future arbitrary language code, parallel execution, hardware-dependent calculations, or external APIs. Before those features are introduced, reproducibility must include strategy artifact/version, data identity and scale, engine/IR versions, configuration, seeds, and any numerical policy.

## Error behavior

Domain constructors reject invalid instruments, zero prices, and zero sequences before execution. Strategy construction rejects invalid IDs and unsupported IR versions. Runtime errors cover input ordering, event-count overflow, and attempts to run a non-Ready instance.

No event is silently reordered or retried. Process panics, allocation failure, and termination are outside the current error model. The synchronous iterator itself can block or panic; there is no timeout, cancellation, or panic containment.

## Resource limits

The current interpreter retains every emitted signal in a Vec. Runtime memory grows with emitted signals, and caller-owned inputs may also occupy memory. There are no enforced event quotas, memory budgets, streaming output, or bounded worker channels. Use the demonstration as a foundation test, not as an unbounded production workload.

## Planned host responsibilities

The Rust host will own version negotiation, artifact validation, event ordering, lifecycle, capability permissions, supervision, resource budgets, and output validation. A future scheduler may route events to the IR interpreter or local workers while preserving one explicit logical ordering per run.

The planned [Extensibility contract](EXTENSIBILITY_CONTRACT.md) separates whole-strategy runners from typed graph-callable providers. Host-owned instances, correlated event/call identities, validated candidate state/outputs and bounded resource policies apply independently of implementation language. Pure-graph atomicity and legacy durably committed host operations have distinct declared policies. These responsibilities do not exist in the experimental interpreter above.

Before concurrent or service execution is supported, define cancellation and terminal-event delivery, backpressure, worker deadlines, crash handling, bounded logs, deterministic scheduling, and checkpoint semantics. No daemon, service port, runtime download manager, or remote compute service is part of this initialization.

## Compiled streaming events

`fqe_runtime::events::EventRuntime` consumes validated tick/closed-bar inputs from fqe.event-plan/1 compiled artifacts. Every stateful subscription and crossing advances once independently of root short-circuit. State/cursors/signals publish together after all outputs succeed. Rejected events leave prior state unchanged and allow explicit retry; `finish()` completes the run. It returns one observation at a time rather than accumulating a signal Vec. See [the normative contract](EVENT_EXECUTION.md) for formats, limits, warmup, manifest, source linkage and failure distinctions. The old one-shot runtime is unchanged.

## Bounded local batch reporting

P1-09 adds `report::run_report` over the same EventRuntime and fqe.market-data/1 decoded records. It never retries a rejected event and returns no partial report; a bounded4MiB writer serializes observations as execution progresses, publishing bytes only after finish. The CLI uses this public library adapter. [CLI exchange](CLI_EXECUTION.md) owns document shapes, local source digest, error stages/exits and stdout-write limitations. The in-process streaming retry policy above remains unchanged.

## Stateful workflows

P2-01 adds `workflow::WorkflowRuntime` over the existing compiled event runtime, with fqe.workflow/1 reusable stateful scalar rules, per-instance typed cells/modes/Delay, whole-event commit and boundary stop/abort/fresh restart. Rejected events are retryable; terminal state release is in-process ownership, not asynchronous cancellation. The library and runnable workflow example use the same generic expression arithmetic and registry. [Workflow execution](WORKFLOW_EXECUTION.md) owns admission limits, phase order, signal origin, typed non-restorable state views and unsupported capabilities. The existing CLI and external process consumer continue to accept fqe.event-plan/1 only.

## Multi-market scheduling

P2-02 adds a separate `schedule::MarketScheduler` over immutable fqe.schedule/1 definitions: bounded multi-symbol/timeframe ingress, inclusive source watermarks, stable total order and committed as-of views. Dispatch exposes only closed/published data and commits publication after a successful transactional consumer. Existing EventRuntime/WorkflowRuntime retain their single-trigger formats; explicit stream routing can feed them global logical sequences while ScheduledEvent preserves original source identity. [Market scheduler](MARKET_SCHEDULER.md) owns calendar/gap/late/duplicate/backpressure/tie rules and the callback transaction boundary. Timers/fair multi-instance scheduling remain P2-03.
