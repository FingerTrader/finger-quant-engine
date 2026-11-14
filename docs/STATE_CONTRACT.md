# State contract

Status: an in-memory run lifecycle and counters are implemented experimentally. P1-05 separately implements in-memory [transactional crossing history](EXPRESSION_CONTRACT.md), without integration into the legacy runtime. P1-06 adds separate [owned incremental indicator state](INDICATOR_CONTRACT.md) with single-node staged commits; neither slice is integrated into the legacy runtime. P1-07 now compiles deferred incremental handles without factory calls; P1-08 now integrates mixed graph event transactions in a separate [compiled event runtime](EVENT_EXECUTION.md). Durable/generalized strategy state, portfolio state and recovery are planned.

## Legacy ownership

One `Runtime` instance owns one validated strategy and one `RunState`. Callers receive a shared reference from `state()`; they cannot mutate the runtime through it. Reports contain a cloned state value.

| State field | Initial value | Update rule |
| --- | --- | --- |
| status | Ready | Follows the lifecycle below |
| processed_events | 0 | Incremented for each accepted input, including unrelated instruments |
| last_sequence | None | Set to the last accepted input sequence |
| last_timestamp_ns | None | Set to the last accepted input timestamp |

Counts use checked arithmetic. A counter overflow is a runtime failure. There is no global mutable engine state, account, database, or filesystem checkpoint.

## Lifecycle

```text
Ready -> Running -> Completed
                 -> Failed
```

`run()` may be called only from Ready. The call is synchronous: Running is an internal transition, not a live subscription or background task. Empty input completes successfully with zero events and absent last-event keys.

An ordering or counter error changes status to Failed. Earlier accepted-input counters and ordering keys are retained; the invalid input is not applied. No report or partial signal vector is returned on error. Both Completed and Failed are terminal for that instance.

Calling `run()` again returns `AlreadyStarted` and preserves the existing terminal state. Create a new runtime to replay inputs. Terminal status does not imply durable storage, transaction commit, order settlement, or process recovery. Uncaught panics, allocation failures, and process termination are outside the current Result-based failure model.

## Strategy and financial state

The legacy IR1 evaluator is stateless. It reads only the current tick and constants. The runtime does not hold indicator windows, user dictionaries, cash, margin, positions, orders, P&L, or optimization trials. A repeated Long signal cannot be interpreted as an existing position.

Future state domains must be separated:

| Planned state domain | Intended owner |
| --- | --- |
| Strategy memory and indicators | Strategy execution instance with versioned declarations |
| Run control and event cursor | Rust host |
| Simulated orders and portfolio ledger | Local simulation/accounting components |
| Live execution and account observations | Authorized Trading SDK adapter plus host reconciliation |
| Workspace display and user preferences | Optional client, including Flutter Desktop |

## Planned persistence and recovery

Before checkpoints are supported, define a versioned snapshot containing strategy artifact identity, IR version, dataset identity, accepted event cursor, declared strategy state, seeds, and applicable financial state. Specify atomic write boundaries, event replay, duplicate handling, migration, and corruption detection.

A future resume operation must reject incompatible strategy/schema/data combinations. Neither copying a `RunState` nor retaining a report is sufficient for recovery. The current engine has no pause, resume, snapshot import, crash recovery, or state migration API.

P2-04/P4-02 must include output identity/cursor and replay/deduplication rules at committed boundaries; snapshot atomicity alone does not establish exactly-once delivery to a client. For the existing Desk consumer, [Desk integration](DESK_INTEGRATION_PLAN.md#state-semantics-and-migration) distinguishes pure IR event rollback from legacy hosted callbacks with already committed context operations, and historical result reading from strategy re-execution. A new engine must never silently reinterpret or overwrite old financial state.

Planned callable provider/module state follows [Extensibility](EXTENSIBILITY_CONTRACT.md#provider-invocation-state-and-execution): host-owned per-instance identity, validated portable state-schema/version and pinned implementation/parameter/data identity. P2 validates in-process X-002 split-run parity; P5/P6 repeat it across language runtimes. Unsupported snapshot capability is rejected for resumable runs; pointers, implicit language-object serialization and shared mutable buffers cannot satisfy the contract.

## Compiled event ownership and commit

The P1-08 event runner owns fresh per-node IncrementalInstance state, one ExpressionInstance for crossing roots, and run/event/bar cursors. `try_fork` deeply copies candidate incremental state while cloning immutable prepared handles; cloning ExpressionInstance copies crossing history. All candidates remain private until the entire event succeeds. Missing/Warmup reset behavior belongs to each existing node/crossing contract. Ready/Running states survive rejected events unchanged and the same failed sequence may be retried; explicit finish is terminal. This is a separate lifecycle from legacy Failed-on-run-error above. [Event execution](EVENT_EXECUTION.md) specifies all validation/warmup/output boundaries. No snapshot export/restore, crash recovery, ledger or external-side-effect transaction exists.
