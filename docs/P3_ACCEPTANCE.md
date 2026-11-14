# P3 accounting and reproducibility acceptance

Status: full offline gate PASS2026-11-14T12:50:20Z;324 Rust/23 external/11 original+2 financial process cases,64 financial source pins,3 gate-negative controls, all four independent financial oracles/seven examples/actual exports/fresh-child financial continuation. [Project status](PROJECT_STATUS.md) owns closure. P3-01..06 must all be COMPLETED before this gate runs; P3-07 cannot pass from documentation or partial checks.

## Complete offline runner

Run `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p3_gate.py` with the pinned local toolchain exports in [Development](DEVELOPMENT.md). The command never installs/downloads packages, uses a network scanner, rewrites golden expectations or offers a partial/skip mode. It checks prerequisites and a frozen [P3 suite](../crates/fqe-runtime/tests/fixtures/p3/suite.json), source/fixture pins and absence of fixture-name dispatch in production; exercises three negative gate controls; recomputes all four independent financial oracle sets; runs the full P2→P1→workspace/CLI/process/docs/diff regression; runs seven actual public examples, including fresh-child durable financial continuation; independently validates the actual exported backtest/optimization JSON; and runs final docs/diff checks. Complete JSON evidence is emitted only at the end with actual environment/timestamp/check statuses. A failed command yields failed gate status and a nonzero exit.

## Financial evidence matrix

| Scope | Actual artifact and independent acceptance |
| --- | --- |
| Exact units/UTC/history | core financial_contract, IR history_contract/runtime history_execution, exact dataset/spec/source/selection identities and frozen independent fixture; no sort/dedupe/look-ahead/precision loss |
| Cash-long accounting | ledger_accounting and independent Fraction5-case16-row oracle; balanced money/quantity, holds, fees, gross basis, partial allocation, actual money steps, round-trip selected legacy equations |
| Restricted-short accounting | Explicit proceeds lock/signed position and exact independent short case/native ledger replay; broker/backtest reject this unsupported execution profile rather than treating it as margin |
| Margin-long execution | margin_simulation and independent Fraction4-case18-row oracle, principal loans/equity, pending/after-fee leverage, close maintenance, subsequent-open forced exits, partial/zero-volume/gap/debt/negative-equity/failed exits |
| Explicit orders/fills | order_simulation, real loaded Long5→intent→open6 fill, previous-volume liquidity, FIFO/limits/caps/costs/cancel/replace/Finish/whole-command rollback and every native receipt split |
| Authoritative complete reports | backtest Event/Workflow, explicit Boolean exit, seven public CSV bars/2 fills/100→97/fees1/net−3/maxDD3;9 independent Fraction metric cases/actual canonical report hashes and complete journal/provenance checks |
| Isolated optimization | optimization Event/Workflow and generic2D grid; independent3-case parameter IDs/values/training-only ranking, fresh later validation−11, seed/ties/split/failure/cancel/budgets and actual exported aggregate checks |
| Complete financial recovery | [Financial recovery](FINANCIAL_RECOVERY.md), every source split across cash/margin/Event/Workflow/full/partial; pending holds/debt/protected liquidation, source/version/integrity/default-deny/failed-event controls; actual private-file child exit73 and actual fresh-child restore/full report-byte equality |

Original P1 financial indicator/provider sources/goldens and P2 temporal suite remain unchanged and independently checked. Full Rust checks cover all source modules/examples/tests; the engine source-identity coverage test must include every implementation file. The P3 suite additionally freezes financial implementations, public data, expected values, oracles, examples, acceptance tests and its gate/negative-control code. No per-name financial fixture branch belongs in core.

## Meaning and limits

A passing gate establishes the documented bounded native local research/simulation profiles without Flutter, login, SDK/backend/account/network. It does not prove Desk/Decimal34/legacy backend equivalence beyond explicitly compared public equations, production data capacity, native compact long-history snapshots, public stable financial decode/ABI, hostile-provider isolation, SDK/live routing, external-language workers/plugins or signed installed Desk distribution. [Desk integration](DESK_INTEGRATION_PLAN.md), P4+ and later performance/isolation gates retain those actual acceptance obligations. No customer runtime/compiler installation claim follows from a native offline gate.

Exact supported contracts: [Financial/history](FINANCIAL_HISTORY.md), [Ledger](LEDGER.md), [Orders](ORDER_SIMULATION.md), [Margin](MARGIN_PROFILES.md), [Backtesting](BACKTESTING.md), [Optimization](OPTIMIZATION.md), [Financial recovery](FINANCIAL_RECOVERY.md). P3-07's final actual timestamp/test count/commit are recorded in PROJECT_STATUS only after complete execution succeeds.
