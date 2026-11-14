# P2 Temporal Strategy Acceptance

Status: implemented acceptance suite; full execution/closure is recorded in [Project status](PROJECT_STATUS.md). P2-01..05 must be COMPLETED before this gate can run. Passing tests or this report alone never establishes a commit, phase closure, publication or product release. [P1 acceptance](P1_ACCEPTANCE.md) retains its dated scope and source pins.

## Actual loaded strategy

[G-006 workflow](../crates/fqe-runtime/tests/fixtures/p2/g006-workflow.json) uses the existing registered MACD(2,3,2)/RSI(2) implementations, compiled event subscriptions and a generic stateful rule module. Immutable parameter `threshold` controls RSI comparison; wait defaults to3 closed bars. The module declares typed remaining/latch cells and idle/armed modes; existing workflow instructions express arm, decrement/latch and expiry. There are no new production enum variants, financial IDs in engine dispatch, indicator formulas or evaluator paths.

A ready MACD cross at bar k arms remaining3 with latch false. At k+1/k+2, latch ready RSI>threshold and decrement. At k+3, output `fired.expire AND (old_latch OR current_rsi_condition)` through an ordinary workflow Long signal, then reset remaining/latch/mode in the same transaction. All guards/effects/outputs retain old-state semantics. A cross while armed, including expiry, cannot arm that event again; a later fresh cross can. Arming-event RSI and earlier RSI never enter the wait latch.

A separate [declarative count window](../crates/fqe-runtime/tests/fixtures/p2/g006-temporal.json) binds the same cross/RSI sources to3 following closed bars. Its actual expiry/satisfaction notices must match the stateful model's expiry/signal at each event. This validates two existing generic compositions; a WindowNotice is still not automatically rewritten into a signal. The workflow artifact itself emits the real signal. Signals remain strategy observations, not orders/fills/accounting.

## Frozen independent data and expectations

[Case definitions](../crates/fqe-runtime/tests/fixtures/p2/g006-cases.json) and [55-row table](../crates/fqe-runtime/tests/fixtures/p2/g006.expected.tsv) supply exact numerical/readiness, crossing, RSI predicate, old/new remaining/latch, transition, signal and independent window expectations. Tests/gates never generate or rewrite them. [Suite inventory](../crates/fqe-runtime/tests/fixtures/p2/suite.json) pins all new fixture/data/schedule/oracle/test/example/runner bytes and checks novel module/schema IDs are absent from production dispatch.

| Case | Closes after common1/2/3/2/4 prefix | Threshold | Expected decisions |
| --- | --- | --- | --- |
| early |6/3/2 |90 | Only k+1 RSI qualifies inside wait; emit exactly bar8 even though RSI false at expiry |
| expiry |3/4/10 |90 | First qualifying wait RSI at k+3; emit bar8, ignore simultaneous expiry re-cross |
| late |3/2/1/10 |90 | RSI first qualifies at k+4; no signal at8/9; fresh cross9 arms an incomplete wait reported at EOF |
| arming-only |3/2/1 |80 | RSI qualifies at arm5 but none in(5,8]; no signal |
| never |3/4/10 |100 | Strict RSI>100 never qualifies; no signal |
| recross |2/1/6/6/1/10/1/1/1 |80 | Crosses5/8/11/14; emit8, ignore re-cross8, fresh cross11 arms, expiry14 fails and consumes re-cross14 without re-arm |

All cases include warmup/first-ready values and the same exact provider/artifact/formula policy. RSI=100 at a pre-arm bar is outside the qualifying interval. Crosses require a prior complete MACD line/signal pair; first-ready bar4 does not cross. Long signal sequences/times/units and all Boolean/state transitions have zero tolerance.

The [independent standard-library Fraction oracle](../tests/reference/p2_temporal_oracle.py) computes SMA seeds, rational EMA recurrence, compact MACD signal seeding, Wilder RSI2 and a separate imperative remaining/latch reference. Each case asserts hand-reviewed expected crosses/signals independently of the Rust results. `--check` recomputes55 numerical/state rows and compares existing bytes; only manual `--write` creates expectations and is never invoked by tests or the gate. Quantize independent rational indicator values to12 decimals with exact HalfEven; compare coefficients within1 as permitted by the existing work18/output12 indicator contract. Boundary decisions have no tolerance and are well separated from thresholds; no engine output was used to author expected values.

## Executable gate coverage

[P2 runtime gate tests](../crates/fqe-runtime/tests/p2_gate.rs) run all six loaded strategies through actual MarketScheduler/InstanceScheduler/WorkflowRuntime/providers and compare every financial/state/window row and emitted Long. Shared-plan repeated lanes reproduce the exact independent series, decisions and final state with separate provider/crossing/module/window history. Full bounded trace/reference replay matches; no historical receipts are returned.

Restore tests split every active/expiry wait point of all cases and every command of the expiry case, including queued market delivery and EOF. Continuation receipts, output identities, inspected state/windows and final checkpoint bytes equal uninterrupted execution; restored produced sequences exceed the saved cursor. Earlier [checkpoint tests](../crates/fqe-runtime/tests/checkpoint_execution.rs) additionally split every command across X-002/actual indicators/windows/timers/blocked/retry/cancel, and [persistence tests](../crates/fqe-runtime/tests/checkpoint_persistence.rs) preserve committed targets across actual pre-rename process exit.

Incremental-watermark prefix runs equal full runs with future inputs already queued. Duplicate/backward ingress preserves accepted cursors/computational state. An unrelated symbol in the same merger stalls until both watermarks are promised, cannot count as a wait bar and changes only global sequence mapping; emission remains original bar8, now logical sequence9. Every retained alignment frontier is at/before its delivery timestamp. [Scheduler tests](../crates/fqe-runtime/tests/schedule_execution.rs) cover same-time lower/higher-frame availability, unclosed future bars, stream ordering/gaps and rollback; [temporal tests](../crates/fqe-runtime/tests/temporal_execution.rs) cover explicit event-time deadline/ties/absent bars, active/expiry re-arms, fairness/backpressure/quarantine/cancel and incomplete EOF. No resampling/calendar join is claimed.

X-002 [workflow tests](../crates/fqe-runtime/tests/workflow_execution.rs) retain hand-derived old/new/Delay/mode/parameter expectations, late-error provider/crossing/rule rollback, temporal-error rollback/retry and release tests. Added [shared custom-provider model](../crates/fqe-runtime/tests/fixtures/p2/x002-shared-memory-workflow.json) runs an independently registered opaque incremental provider in two separate lanes, each with rule step1/step2 instances. Every command split restores the same suffix/bytes; committed counts are3/6 per lane and all provider state is released after dropping runs. Source pins bind this test implementation/model; the declared test provider artifact ID is provenance, not signed-binary attestation. P1 source-pinned indicators/builtins/custom-provider/goldens remain unchanged.

## Full runner and evidence

`PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p2_gate.py` runs prerequisite/source/generic-dispatch validation, the independent oracle in check-only mode, [negative gate controls](../tests/p2_gate/test_runner.py), full P1/workspace regression (fmt/check/test/strict all-target clippy/build/23 external tests/11 actual process cases/docs/diff) and the actual loaded temporal strategy example. There is no skip-check/partial success mode. At least244 Rust tests are required; missing/incomplete prerequisite/version/pin/boundary negatives cannot silently pass. The JSON report carries raw nested regression evidence, case names, source IDs, actual example output, platform/runtime and completion timestamp. Reports are operational output; committed source fixtures and [status](PROJECT_STATUS.md) record durable evidence.

`cargo run --locked --offline -p fqe-runtime --example temporal_strategy` executes the actual expiry dataset offline and prints one Long at sequence8/time8000 plus wait5→8 satisfied. No Flutter/login/account/Python or customer download is required to execute the compiled Rust example. This does not establish a customer-bundled Desk build.

Final executed counts/commands/platform/commit belong in PROJECT_STATUS. P2 closure requires all P2-01..05 completed, full gate PASS and evidence commit/synchronized closure. Dependencies/manifests/lock/toolchain and financial implementations remain unchanged; all verification uses approved cached packages and the existing project-local Rust toolchain. No scanner/private dependency upload or installer is required.

Experimental limits remain: bounded synchronous trusted in-process computation, fixed UTC grids/explicit watermarks, scalar rule profile, explicit replay permits,1024-command/262144-byte checkpoints,4096-entry/4MiB charged traces and strict same-engine restore. P3 financial historical data/accounting/backtest/simulation/optimization, P4 actual Desk/Flutter integration/bundling and P5/P6 languages/plugins remain unimplemented downstream. P2 does not certify realistic trading, historical data authenticity, native long-history codec, external ack/exactly-once, hostile-code sandboxing or other-platform durability/performance.
