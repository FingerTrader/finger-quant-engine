# Event contract

Status: validated in-process market events and signal records are implemented experimentally. Wire envelopes are an experimental schema draft; delivery transports and durable event storage are planned.

## Implemented input event

`fqe-core::Event` has private fields and a validating constructor:

| Field | Type | Meaning |
| --- | --- | --- |
| sequence | Positive u64 | Caller-assigned ordering key within one run |
| timestamp_ns | u64 | Nonnegative input time in nanoseconds |
| tick.instrument | InstrumentId | Case-sensitive instrument identifier |
| tick.price | PriceTicks | Positive exact integer price in caller-defined ticks |

For real datasets the intended timestamp convention is UTC Unix epoch nanoseconds. The demo uses synthetic values and never reads the wall clock. No timestamp parsing, timezone conversion, exchange calendar, tick-size metadata, or historical data loader exists.

Instrument identifiers contain 1–128 ASCII letters, digits, or `:/._-`. They are opaque identifiers, not filesystem paths. Prices cannot be zero or negative in the initial subset. A caller must use one consistent price scale for each strategy instrument.

## Runtime ordering

Input order is authoritative; the runtime does not sort events.

1. The first event may have any positive sequence.
2. Each later sequence must be strictly greater than the last accepted sequence. Gaps are allowed; duplicate and decreasing sequences fail the run.
3. Timestamps must be nondecreasing. Equal timestamps are allowed; sequence resolves their order.
4. A zero timestamp is allowed. A backward timestamp fails the run even if the sequence increases.
5. Events for unrelated instruments still participate in ordering and event counts.

The runtime validates an event before evaluating its rule and updating state. An invalid event is not counted. Processing stops at the first ordering error. There is no retry, implicit repair, deduplication store, or exactly-once delivery claim.

## Implemented output

A `SignalRecord` contains `source_sequence`, the source `timestamp_ns`, and a typed signal with instrument and kind. It links a decision to the accepted input. The vector preserves input order.

The current report has no run ID or independent output sequence. A signal is not an order, fill, execution report, or account mutation. On a runtime error, no report or partial signal vector is returned; the runtime's accepted-input state remains inspectable.

## Experimental wire envelope

The `fqe.v1alpha1.EventEnvelope` schema adds protocol version, run ID, stream sequence, time, and a market-tick or signal payload. This is a design artifact, not an active serialization path.

Input and output streams would each own their envelope sequence; `Signal.source_sequence` would retain the input link. The wire's run ID would prevent cross-run ambiguity. Incoming payload variants, enum values, message presence, length limits, u64 precision, and domain invariants require validation when a codec is introduced.

Protocol Buffers are the intended wire format, not a commitment to gRPC. Framing, local transport, authentication, size limits, acknowledgements, backpressure, and reconnect behavior are undecided and unimplemented.

## Planned event families

Timers, bars, data gaps, worker lifecycle, order intents, execution reports, account updates, checkpoints, metrics, and terminal run events need future contracts. Add event families only with explicit ordering, ownership, failure, versioning, and replay rules. No broker event schema is defined by this initialization.
