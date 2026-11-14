# Next Session

Recover [status](PROJECT_STATUS.md), [plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md), [workflow](WORKFLOW.md), README and current contracts. Verify main/remote/dirty work/actual commits before edits; use main, no worktree. User explicitly requests continuous downstream execution without per-task stops. Proceed after verified closure unless genuinely blocked. Publishing is not requested.

## Current evidence

P1-01–P1-11, P2-01 and P2-02 COMPLETED. P1 gate passed; P2-03 VERIFIED, P2 phase gate still open. Latest implementation evidence `daeeb181d99236094daa3272fd4efa3855ddca06`; previous P2-01 `52f6e9ea4a7bc2e4a687796eaf4af2752aee7c11`. Resolve HEAD/latest status commit from Git. Main28 commits ahead after closure; remote last read-confirmed `0ac5d7b163e77bddb5f69689a1455e3560aad872`. Recheck live before publishing; no push authorization.

P2-01 [workflow](WORKFLOW_EXECUTION.md)/ADR-022: fqe.workflow/1 parameterized typed stateful scalar rules, isolated mode/cells/one-event Delay, old-state guards/effects, equal-priority control/write conflict rejection, whole-event provider/crossing/rule/cursor commit, boundary stop/abort/finish and fresh restart. State inspection is explicitly non-restorable; checkpoint unsupported. Flat instances/single control region are explicit limits.

P2-02 [market scheduler](MARKET_SCHEDULER.md)/ADR-023: strict fqe.schedule/1 immutable fixed-UTC definitions, 1..64 exact source/domain streams, queue1..4096, explicit gap reject/report, per-stream source ordering, inclusive watermark promises, deterministic timestamp/instrument/family/duration/source-sequence merge, global logical sequence with original provenance, atomic dispatch and current/published-only alignment. Queued same-time later-phase/future records are hidden. No exchange calendar, resampling, implicit gap fill or timestamp-batch join. Callback-owned effects must be transactional; existing WorkflowRuntime::step is the final fallible consumer action. Definition/source metadata is not authenticated dataset IO.

Full offline P1 gate PASS:205 Rust tests (181 original +14 workflow +10 scheduler),23 external tests,11 actual process cases, protected source pins, fmt/check/test/strict clippy/build/docs/diff. Documentation check39 Markdown/486 links/42 task projections. Independent [merge hand table](../crates/fqe-runtime/tests/fixtures/p2/alignment.expected.tsv), interleaving/watermark/future/gap/order/backpressure/rollback/full-width tests and actual workflow consumption validate P2-02. P2-01 workflow example also passed. No dependency/manifest/lockfile/toolchain/earlier golden/provider-source changes.

## Next eligible: P2-03 — Time Windows, Timers and Instance Scheduling

Read [P1/P2 design](P1_STRATEGY_DESIGN.md), [market scheduler](MARKET_SCHEDULER.md), [workflow](WORKFLOW_EXECUTION.md), [events](EVENT_CONTRACT.md), [state](STATE_CONTRACT.md), [runtime](RUNTIME.md), [extensibility](EXTENSIBILITY_CONTRACT.md) and P2-03 plan row. Mark IN_PROGRESS with explicit versioned scope/acceptance before code.

- Event-time windows/timers, bounded queues and multi-instance scheduling. No wall-clock replay dependency. Explicit count/calendar domains, inclusive/exclusive expiry and same-time market/timer ordering. Count windows count qualifying closed bars, not unrelated ticks/streams/missing bars.
- Independent instances sharing an immutable model must preserve isolated state/provider histories, deterministic fair dispatch, bounded pending work and cancellation behavior. Verify overload/backpressure without silent drop, timer ties, expiry boundaries and absent bars.
- G-006 target: cross at k; latch ready RSI true in (k,k+3]; emit once at k+3 including first true at expiry; ignore re-cross while armed and do not re-arm on expiry. P2-03 tests generic temporal primitives/boundaries; full MACD/RSI model, all snapshot splits and gate remain P2-06 after P2-04/P2-05.
- P2-04 must checkpoint scheduler ingress/watermarks/pending queue/alignment, providers/crossings/modules/cursors, output identity and temporal state compatibly. Current copied state/cells are not recovery. P2-05 owns replay/debug traces. Financial/backtest/simulation/Desk/Flutter/SDK/worker/plugin delivery remains later tasks.

## Commands and protected evidence

Reuse approved cached dependencies and pinned toolchain; no installation needed. Preserve all earlier canonical/golden fixtures and protected indicators/builtins/example/gate provider source pins. New financial/provider IDs must never enter generic production dispatch.

```sh
export CARGO_HOME="$PWD/.local-toolchain/cargo"
export RUSTUP_HOME="$PWD/.local-toolchain/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
export RUSTUP_AUTO_INSTALL=0
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p1_gate.py
cargo test --locked --offline -p fqe-runtime --test schedule_execution
cargo run --locked --offline -p fqe-runtime --example workflow
python3 scripts/check_docs.py
git diff --check
```

Record real code/check/error/dirty state before interruption. Evidence commit while VERIFIED, then synchronized status/plan/handoff closure recording SHA. Continue next eligible task automatically. Only an authorized normal push and matching remote SHA establishes publishing.

Active P2-03 checkpoint: scoped typed temporal bindings, per-lane count/event-time latching windows, deadline-after-market ordering and bounded fair coordinator selected. Implement IR temporal format/runtime windows+lanes; preserve workflow arithmetic and add private candidate-step reuse for combined rollback. P2-03 code now exists in IR temporal.rs and runtime temporal.rs/instances.rs, with private WorkflowRuntime candidate-step extraction and MarketScheduler next-timestamp metadata. Initial workspace compile PASS; focused IR/runtime temporal tests currently running. New temporal/count/time/overflow/bar-workflow/stream fixtures are uncommitted; no prior fixtures/providers/dependencies modified. Full checks/docs/ADR still pending.

P2-03 focused checks PASS:2 temporal IR +7 coordinator tests +1 new real-provider combined rollback test (10 new tests). New typed bound-rule manifests/as-of outcome snapshots added; full workspace/P1 gate/docs/clippy after final edits pending. Docs/TEMPORAL_SCHEDULER.md and ADR-024 drafted. Still IN_PROGRESS, no task closure or snapshot delivery claimed.

P2-03 IMPLEMENTED → VERIFIED checkpoint: full offline gate PASS215 Rust/23 external/11 process cases; all source pins/fmt/check/test/clippy/build/docs/diff PASS. Record evidence commit and three-document closure before continuing P2-04. ADR-024 accepted. No snapshot or complete temporal financial gate claimed.
