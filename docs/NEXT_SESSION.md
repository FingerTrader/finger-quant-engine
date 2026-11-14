# Next Session

Recover [status](PROJECT_STATUS.md), [plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md), [workflow](WORKFLOW.md), README and current contracts. Main only; inspect actual Git/remote/dirty work. User requests continuous downstream execution without per-task stops. Publishing not requested.

## Current evidence

P1 and P2-01–P2-04 COMPLETED. P2-05 READY. P2-04 evidence `4dffdfe235eba1f698a34b3e0df6c2198a066ba3`; local closure synchronized. Checkpoint implementation and contracts passed the full offline P1 gate:229 Rust tests,23 external tests,11 actual process cases, source pins/fmt/check/test/strict clippy/build/docs/diff. Checkpoint example restored8 commands/cursor2 and continued3..12 identically. MacOS ARM64/Rust1.99.0/Python3.14.6. No new dependency/lock/toolchain or protected financial/golden changes. Remote last read-confirmed0ac5d7b163e77bddb5f69689a1455e3560aad872; no push requested.

[Checkpoint contract](CHECKPOINT_RESTORE.md)/ADR-025 owns explicit bounded-deterministic-command-replay/1 representation, default-denied provider permits, engine/full artifact/schema/parameter/data pins, complete private reconstruction, outcome/output-cursor checks,1024 commands/262144 bytes preflight and atomic local file publication. All owned scheduler/provider/crossing/module/window state is reconstructed, not natively serialized. Every-command splits cover actual indicators, X-002, queues/fairness/windows/timers/blocked/retry/cancel. Native compact long-history/migration/external ack are unsupported. Standalone workflow checkpoint remains unsupported. New implementation files must join source_identity.rs; coverage test enforces it.

## Next eligible: P2-05 — Replay and Debug Trace

Mark P2-05 IN_PROGRESS with scoped acceptance before edits. Read workflow/event/expression/temporal/checkpoint contracts and P2-05 plan row. Implement actual deterministic user replay with bounded node/workflow/state diagnostics and source node/event divergence locations. Trace must observe already computed values without changing provider calls, state/signals, scheduling or checkpoint outcome identities. Test trace on/off parity, bounded exhaustion, repeat/snapshot replay equivalence and intentional divergence localization. Checkpoint private reconstruction alone is not P2-05 completion. P2-06 complete G-006 model/gate follows; P3 financial/backtest/simulation and P4 actual Desk/Flutter bundle remain later.

## Checks and delivery

Reuse reviewed cached dependencies/pinned toolchain; no installation. Export CARGO_HOME="$PWD/.local-toolchain/cargo", RUSTUP_HOME="$PWD/.local-toolchain/rustup", PATH="$CARGO_HOME/bin:$PATH", RUSTUP_AUTO_INSTALL=0. Full check `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p1_gate.py`; required Rust checks are included. Run python3 scripts/check_docs.py and git diff --check after records. Preserve all old canonical/goldens and indicators.rs/builtins.rs/example/gate provider pins. Stage intended paths only, evidence commit while VERIFIED, then closure recording actual SHA; continue automatically. Local commits authorized; no GitHub publishing authorization.
