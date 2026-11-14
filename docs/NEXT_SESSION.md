# Next Session

Recover [status](PROJECT_STATUS.md), [plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md), [workflow](WORKFLOW.md), README/current contracts and actual Git state. Main only, no worktree. User requests continuous downstream execution without per-task stops. Local commits authorized; publishing not requested.

## Current verified evidence

P1/P2 COMPLETED. P3-01 COMPLETED, evidence `f7d87ed730a74664cb6dc4182fbc6cac62c007e6`; closure recorded. Full P2 gate PASS2026-11-14T11:34:47Z:257 Rust/23 external/11 process cases/all required checks/old financial-temporal source pins and independent55 rows. Docs44 Markdown/570 local links/42 tasks after evidence records. Actual historical_inputs example4 rows/1 warmup/0 signals with exact frozen manifest. [Financial/history contract](FINANCIAL_HISTORY.md)/ADR-027 own exact implementation and dated Desk source identities. No deps/lock/toolchain/protected providers/old goldens changed; Desk read-only, no app/packages/tests/writes.

Remote main read-confirmed0ac5d7b163e77bddb5f69689a1455e3560aad872;38 local commits including closure. Recheck before delivery. Never claim publication.

## Next: P3-02 authoritative virtual ledger

P3-02 READY, first eligible. Read [Financial/history](FINANCIAL_HISTORY.md), [Backtesting](BACKTESTING.md), [Simulation](SIMULATION.md), [Desk integration](DESK_INTEGRATION_PLAN.md) and plan row. Scope: explicit account/instrument/currency/profile; bounded balanced journal, cash/positions/reservations/P&L/valuation, atomic invalid-action rejection and full independently validated journal replay. Independent public synthetic legacy cash-long vs Rust comparison and hand-computed long/short/rounding fixtures required. No order matching/fees model/implicit signal fills before P3-03, no margin/leverage before P3-04. Source-read Desk cash book currently capitalizes buy cash-flow including fees and prorates cost; any differing FQE cost/rounding profile must be explicitly versioned, never claim full legacy parity from one flat trade.

P3-01 code: core finance/utc; IR history/history_csv and public fixtures under IR tests/fixtures/history; runtime history cursor/PreparedHistoricalRun;13 targeted tests; example historical_inputs. History data may contain explicit session gaps; prepared EventRuntime runs reject selected gaps. Input-only manifest does not choose accounts/fills. Canonical identity fixture from independent Python; never regenerate it during tests.

## Checks and boundaries

Use existing CARGO_HOME="$PWD/.local-toolchain/cargo", RUSTUP_HOME="$PWD/.local-toolchain/rustup", PATH="$CARGO_HOME/bin:$PATH", RUSTUP_AUTO_INSTALL=0. Full `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p2_gate.py` runs required Rust checks/docs/diff and all old gates. Reuse approved caches, no installers/private scanner uploads. New implementation .rs files must join runtime/source_identity.rs; coverage test enforces it. Protect indicators.rs/builtins.rs/provider artifact sources, old P1/P2 suite source pins and goldens. Evidence commit while VERIFIED then synchronize closure with actual SHA. Continue automatically; push only if separately authorized.
