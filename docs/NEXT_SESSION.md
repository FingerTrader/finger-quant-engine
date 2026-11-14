# Next Session

Recover [status](PROJECT_STATUS.md), [plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md), [workflow](WORKFLOW.md), README and current contracts. Verify actual main/remote/dirty work/commits before edits; main only, no worktree. User explicitly requests continuous downstream execution without per-task stops. Continue after verified closure unless genuinely blocked. Publishing not requested.

## Current evidence

P1-01–P1-11 and P2-01–P2-03 COMPLETED. P1 gate passed; P2-04 READY, P2 gate still open. Latest evidence `f73082c86622ce40311d7e9ec91eef08402d681f`; P2-02 `daeeb181d99236094daa3272fd4efa3855ddca06`, P2-01 `52f6e9ea4a7bc2e4a687796eaf4af2752aee7c11`. Resolve HEAD/latest status commit from Git. Main30 commits ahead after closure; remote last read-confirmed `0ac5d7b163e77bddb5f69689a1455e3560aad872`. Recheck live; no push authorization.

P2-01 [workflow](WORKFLOW_EXECUTION.md)/ADR-022: typed isolated scalar rule modules/modes/cells/Delay, immutable shared definitions, old-state guards/effects, equal-priority conflict rejection, whole-event provider/crossing/rule/cursor commit, stop/abort/finish and fresh restart. Inspection is explicitly non-restorable; checkpoint unsupported.

P2-02 [market scheduler](MARKET_SCHEDULER.md)/ADR-023: fqe.schedule/1 fixed-UTC streams, queue1..4096/streams1..64, exact source/domain identities, inclusive source promises, deterministic market merge, global/source sequence mapping, explicit gap reject/report and published-only as-of views. Consumer Result error preserves publication state; source ingress and publication are separate. No resampling/exchange calendar/timestamp-batch join.

P2-03 [temporal scheduler](TEMPORAL_SCHEDULER.md)/ADR-024: fqe.temporal-rules/1 exact Boolean source bindings/workflow identity, count/event-time latching windows, open-left/closed-right expiry, ignored active/expiry re-arms, watermark-complete timer phase. `InstanceScheduler` bounds64 lane capacity/4096 pending deliveries/16 windows per lane, atomic fanout, round-robin within earliest time/phase, isolated fresh workflow/provider/crossing/window state and immutable per-source alignment snapshots. Rejected lane becomes Blocked with unchanged head/state; explicit retry/cancel, other lanes progress. Cancel clears queues/windows and releases providers. EOF reports incomplete count windows; time timers dispatch before finish. WindowNotice is an observation, not an automatic workflow signal/order. LaneManifest binds host/lane/stream/schedule/workflow/temporal artifact/source/execution IDs. No hot attach or lane resume.

Private `WorkflowRuntime::fork_step` reuses existing `accept` after base EventRuntime staging, allowing joint temporal/provider/crossing/cell/cursor rollback without a second evaluator. Existing standalone step behavior unchanged. `MarketScheduler::next_timestamp_ns`/run getters expose only scheduling metadata for timer phase/manifest.

Full offline P1 gate PASS:215 Rust tests (181 original +14 workflow +10 scheduler +10 temporal),23 external tests,11 actual process cases, source pins/fmt/check/test/strict all-target clippy/build/docs/diff. Latest documentation check40 Markdown/505 links/42 task projections. Temporal tests cover independent arm2→expiry5 latch/boundaries, absent bars, same-time timer ties, watermark stalls, unrelated streams, repeat/shared-model state isolation/fairness, capacity/lifecycle/EOF, overflow rollback/quarantine/retry. Added real provider/crossing test proves repeated temporal overflow never advances candidate history and cancels with no live provider. Earlier workflow example passed. No dependency/manifest/lockfile/toolchain/old financial provider/golden changes.

## Next eligible: P2-04 — Snapshot and Restore

Read [state](STATE_CONTRACT.md), [runtime](RUNTIME.md), [workflow](WORKFLOW_EXECUTION.md), [market scheduler](MARKET_SCHEDULER.md), [temporal scheduler](TEMPORAL_SCHEDULER.md), [extensibility](EXTENSIBILITY_CONTRACT.md), [provider registry](PROVIDER_REGISTRY.md), [indicator](INDICATOR_CONTRACT.md), P1/P2 design and P2-04 plan row. Inspect actual provider state traits/private structs and source identity pins before selecting representation. Mark IN_PROGRESS with exact scope/acceptance before edits.

- Complete versioned atomic checkpoint, including pinned artifact/parameter/registry/provider/state-schema/data identities, accepted input/output cursors, provider/indicator/crossing/module buffers, scheduler ingress/watermarks/pending queue/alignment, lane FIFO/status/fairness, windows/timers and source/output identities. Copying RunState/cells is insufficient.
- Reject corrupt/truncated/unknown/incompatible formats, missing lifecycle/snapshot capabilities, pointers/language objects and changed dataset/provider/schema/parameters/artifacts. Unsupported providers cannot silently claim restore support. Do not weaken the unsupported checkpoint API or source-artifact validation without concrete complete representation/evidence.
- X-002 split at every event and compare uninterrupted outputs/final state; include active wait/timer/quarantine/queued boundaries. Atomic file write/crash tests must preserve previous committed checkpoint and no duplicate committed decisions on restore. Snapshot/output delivery semantics must distinguish internal replay from publishing decisions.
- Record checkpoint representation and resource/compatibility limits in contract/ADR, then actual full verification before closure. P2-05 owns replay/debug trace and P2-06 owns full MACD/RSI G-006/restore temporal gate. Financial/backtest/simulation/Desk/Flutter/SDK/language/plugin delivery remains later tasks.

## Commands and protected evidence

Reuse reviewed cached dependencies/pinned toolchain, no installation. Preserve old canonical/golden fixtures and protected `indicators.rs`, `builtins.rs`, IR example providers, runtime gate providers source pins unless a concrete reviewed identity change is required. New provider/financial IDs must not enter generic engine dispatch. No package/lockfile changes or private scanner upload without required checks/authorization.

```sh
export CARGO_HOME="$PWD/.local-toolchain/cargo"
export RUSTUP_HOME="$PWD/.local-toolchain/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
export RUSTUP_AUTO_INSTALL=0
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p1_gate.py
cargo test --locked --offline -p fqe-runtime --test temporal_execution
cargo run --locked --offline -p fqe-runtime --example workflow
python3 scripts/check_docs.py
git diff --check
```

Record actual partial code/check/error/dirty state before interruption. Evidence commit while VERIFIED, then synchronized status/plan/handoff closure recording SHA. Continue automatically. Only an authorized normal push plus matching remote SHA establishes publishing.
