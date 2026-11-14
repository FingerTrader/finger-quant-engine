# Next Session

Recover [status](PROJECT_STATUS.md), [plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md), [workflow](WORKFLOW.md) and current contracts; verify main/remote/dirty work and actual commits before edits. Work on main, no worktree. User explicitly requests continuous downstream execution without per-task stops. Proceed after each verified closure unless genuinely blocked. Publishing is not requested.

## Current evidence

P1-01–P1-11 and P2-01 COMPLETED; P1 phase gate passed. P2-02 READY; no P2 phase gate passed. Latest implementation evidence `52f6e9ea4a7bc2e4a687796eaf4af2752aee7c11`; resolve current HEAD and latest status commit from Git. Main26 commits ahead after this closure, remote last read-confirmed `0ac5d7b163e77bddb5f69689a1455e3560aad872`; recheck live before publishing. No push authorization.

P2-01 adds [workflow execution](WORKFLOW_EXECUTION.md)/ADR-022 and `fqe-ir/src/workflow.rs`, `fqe-runtime/src/workflow.rs`, private EventRuntime outer transaction. Strict fqe.workflow/1 composes single-trigger event plan with typed parameterized rule instances, host-owned cells/mode/one-event Delay, old-state guards/effects, deterministic equal-priority conflict rejection, whole-event provider/crossing/rule/cursor commit. Boundary finish/stop/abort releases owned provider state; fresh terminal restart needs new run ID. No complete checkpoint: inspection is typed/versioned and explicitly restorable=false; checkpoint returns unsupported. One control region/equal priority and flat module DAG are intentional bounds, not full workflow-format support.

Full offline P1 gate PASS:195 Rust tests (181 preserved +7 workflow IR +7 runtime),23 external caller tests,11 actual process cases, source pins, fmt/check/test/strict clippy/build/docs/diff. Workflow example PASS. [Hand-authored X-002 table](../crates/fqe-runtime/tests/fixtures/p2/x002.expected.tsv) validates separate parameters, read-old/write-next, Delay/warmup and exact signal origins; custom memory provider proves late numeric error rolls back provider history/crossings/rules/delays/cursor and permits corrected same-sequence retry. No dependency/manifest/lockfile/toolchain or prior golden/provider-source changes.

## Next eligible: P2-02 — Scheduler and Market Alignment

Read [event contract](EVENT_CONTRACT.md), [compiled events](EVENT_EXECUTION.md), [state](STATE_CONTRACT.md), [runtime](RUNTIME.md), [P1/P2 design](P1_STRATEGY_DESIGN.md), workflow contract and P2-02 plan row. Mark IN_PROGRESS with concrete versioned scope/acceptance before code.

- Deterministic multi-symbol/multi-timeframe scheduler, bounded buffered ingress and explicit per-stream/global watermark/tie-break contract. Closed/available bars only; no future leakage, partial higher timeframe consumption or silently synthesized market data.
- Explicit duplicate/out-of-order/late/gap policy, supported calendar domain and ordering/source identities. Preserve strict P1 single-domain validation instead of silently changing old artifacts.
- Verify interleaved streams and equal-time ordering independent of arrival order, watermark boundaries, incomplete higher timeframe, gaps/late/duplicate/out-of-order inputs and failure rollback. Provide executable integration/consumption evidence, not a documentation-only scheduler.
- P2-03 owns event-time windows/timers/fair bounded multi-instance scheduling; P2-04 owns complete provider/module/crossing/cursor snapshots and split-run X-002; P2-05 replay/debug traces; P2-06 complete G-006 temporal gate. Financial/backtest/simulation/Desk/Flutter/worker/SDK work remains later tasks.

## Protected evidence and commands

Preserve existing P1 fixture/canonical/expected files and source pins (`indicators.rs`, `builtins.rs`, IR example providers, runtime gate providers). New generic rule/provider IDs must not appear in production dispatch. Reuse reviewed cached dependencies and pinned local toolchain, no installation needed.

```sh
export CARGO_HOME="$PWD/.local-toolchain/cargo"
export RUSTUP_HOME="$PWD/.local-toolchain/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
export RUSTUP_AUTO_INSTALL=0
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p1_gate.py
cargo run --locked --offline -p fqe-runtime --example workflow
python3 scripts/check_docs.py
git diff --check
```

Record actual code/checks/errors/dirty state before interruption. Evidence commit while VERIFIED, then synchronized status/plan/handoff closure recording that SHA. Only an authorized normal push plus matching remote SHA establishes publishing. Continue directly to next eligible task after closure.
