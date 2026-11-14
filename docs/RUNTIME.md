# Runtime

Status: a synchronous in-memory interpreter is implemented experimentally. Process supervision, live services, asynchronous execution, persistence, and production resource controls are planned.

## Current execution

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
