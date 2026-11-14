# Next Session

Recover [status](PROJECT_STATUS.md), [plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md), [workflow](WORKFLOW.md), README/current contracts and actual Git. Main only, no worktree. User requests continuous downstream execution without per-task stops. Local commits authorized; publishing not requested.

## Current verified evidence

P1/P2 and P3-01/P3-02 COMPLETED. Latest P3-02 evidence `954d08462eab2195fbb4d4ed27a60367df49ae4f`; closure synchronized. Full P2 gate PASS2026-11-14T11:45:07Z:267 Rust/23 external/11 process/all required checks/old numeric/provider/temporal source pins, independent55 P2 rows. Independent Fraction ledger oracle --check5 cases/16 frozen rows PASS. Actual virtual_ledger example3 entries/replay matched/cash1019/fees1/net19. Docs45 Markdown/583 local links/42 tasks after evidence records. [Ledger](LEDGER.md)/ADR-028 own exact profiles, limits and comparison boundaries. No dependencies/lock/toolchain/protected provider/old goldens changed. No Desk writes/packages/tests/app execution.

P3-01 evidence f7d87ed, closure77095ab; exact finance/UTC/history/manifest/cursor with13 tests. [Financial/history](FINANCIAL_HISTORY.md)/ADR-027 own dated actual Desk source hashes. Native ledgers cost gross and expense fees; Desk fee-capitalizes and prorates Decimal34, so selected cash/flat-net comparisons do not claim intermediate-cost/full-Desk parity.

Remote main last read-confirmed0ac5d7b163e77bddb5f69689a1455e3560aad872;40 local commits including closure, no publication authorization. Recheck remote.

## Next: P3-03 explicit order lifecycle and fills

P3-03 VERIFIED, first eligible. Read Simulation, Backtesting, Ledger, Financial/history, Desk integration and plan row. Scope: explicit signal→intent policy, bounded native cash-long simulation broker, market/limit timing, deterministic liquidity/partial fills, cancel/replace atomicity, exact fee/slippage and journal integration. Select/version an implementable execution model; avoid using the next bar's future close/high/low/volume to make earlier decisions. Test same-bar traps, partials, cancel races, tick/lot/minimum rules, price gaps/budget rejections and independent cash/positions/costs. Unsupported types/models must fail; no implicit fills or live SDK. Margin/broker risk remains P3-04.

P3-02 files: runtime ledger.rs, ledger_accounting.rs, virtual_ledger example, fixtures/p3/ledger.expected.tsv and tests/reference/p3_ledger_oracle.py. Core quantity_lots separates partial holdings from minimum order quantity. Journal opening binds engine+config; replay recomputes all fields, DTO has no stable wire/durable append/financial full-run checkpoint yet. Short profile only restricts sale proceeds, no risk solvency model. History imported session gaps are valid but PreparedHistoricalRun rejects them for continuous EventRuntime.

## Checks and boundaries

Use CARGO_HOME="$PWD/.local-toolchain/cargo", RUSTUP_HOME="$PWD/.local-toolchain/rustup", PATH="$CARGO_HOME/bin:$PATH", RUSTUP_AUTO_INSTALL=0. Full `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p2_gate.py` runs required checks/docs/diff/all old gates. Ledger oracle --check never rewrites expectations. New implementation .rs files must join runtime/source_identity.rs; coverage test enforces it. Protect indicators.rs/builtins.rs/provider sources, old P1/P2 suite pins/goldens. Reuse approved cache, no installers/private scanner uploads. Evidence commit while VERIFIED then synchronized closure with actual SHA; continue automatically. Push only if separately authorized.

P3-03 selected scoped model and tests are active in PROJECT_STATUS: next-open, prior-closed-volume FIFO liquidity; open-only limits (no OHLC touch simulation); explicit cash caps/per-fill costs; cash-long only; bound nonwarmup signal helper remains opt-in. Implement simulator.rs, tests/example/contract and full regression before closure.

P3-03 partial artifacts now exist: runtime simulator.rs/lib/source manifest, order_simulation tests (12 passed before final extra tick test), examples/support/historical.rs and simulated_orders, public6-bar signal-history fixtures, ORDER_SIMULATION contract/ADR-029 proposed/navigation. Actual loaded example PASS Long5→fill6 at3.00/cash93.50/qty2/fees0.50/zero holds/replay matched. Strict Clippy passed before final tick test. Configured low capacities/Finish slot and whole-bar late-fill rollback tested. Full workspace/P2 gate and evidence closure pending; no margin/risk or Desk integration yet.

Final13 order tests PASS and ledger oracle/doc checks passed. Full P2 gate now running (/tmp/fqe-p303-gate.json, stderr). Full regression/strict checks must succeed before VERIFIED/evidence commit/closure. Then continue P3-04 automatically.

P3-03 full gate PASS2026-11-14T11:57:58Z:280 Rust/23 external/11 process/all required checks and old gates;13 simulator tests. ADR-029 accepted. Inspect final intended/protected diff, commit evidence then synchronize closure and activate P3-04. No publication authorized.
