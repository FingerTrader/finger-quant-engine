# Local backtesting

Status: backtesting remains planned. The implemented experimental [financial/history inputs](FINANCIAL_HISTORY.md) validate local CSV, exact units, immutable selection and pinned strategy/run inputs; the loaded history example evaluates a strategy and produces no trading performance.

Implementation and local optimization are gated by P3-01–P3-07 in [Delivery plan](DELIVERY_PLAN.md). [Project status](PROJECT_STATUS.md) is the only progress authority; this guide describes requirements, not delivery claims.

## Scope and independence

Backtesting will replay user-selected historical data on the user's device. Offline datasets and strategies must be usable without Flutter Desktop or a Finger Trader login. Data acquisition is a separate adapter concern; no dataset downloader is implemented.

Experimental [virtual ledger](LEDGER.md) supplies accounting postings and valuation, with independent public comparison fixtures; strategy-to-order/fill/report orchestration remains planned.

The Rust engine will own replay ordering and validation. Clients may configure and display runs without owning financial semantics.

## Planned pipeline

```text
local dataset + metadata
  -> validated ordered market events
  -> typed IR or local language worker
  -> signal / explicit order-intent policy
  -> local execution model and accounting
  -> reproducible report
```

A signal cannot become a fill implicitly. Strategy evaluation, signal-to-order policy, execution modeling, and portfolio accounting require separate contracts. The independent Trading SDK is not required to replay local data.

## Required inputs

A future run manifest must identify the strategy artifact and parameters, engine/IR/protocol versions where applicable, dataset content identity, time range, timestamp semantics, instrument metadata, tick/lot/currency scales, initial balances, fee model, execution model, seeds, and corporate-action treatment.

Specify handling for gaps, duplicate data, out-of-order events, same-time events, missing instruments, calendar boundaries, and data revisions. Dataset adapters must validate these properties before a report can claim reproducibility.

## Financial correctness gates

Before publishing performance results, implement and test:

- Exact financial numeric and rounding rules appropriate to each instrument.
- Cash, positions, realized/unrealized P&L, fees, and any supported leverage or margin.
- Order lifecycle, fill timing, liquidity assumptions, and slippage.
- Prevention of look-ahead and survivorship assumptions that invalidate a strategy result.
- Auditable event-to-order-to-fill-to-ledger provenance.
- Deterministic reference fixtures with hand-computed expected outcomes.

Unsupported asset classes and accounting assumptions must be explicit. The initial positive integer price model does not yet supply a complete financial data model.

## Planned outputs

A report should include the full run manifest, validated input summary, warnings, state transitions, order/fill/ledger artifacts, metrics with formulas, and terminal status. Metrics and charts must derive from the same authoritative ledger.

## Parameter optimization

Local parameter optimization is also planned. Each trial must identify its parameter set, dataset split, seed, resource budget, and engine version. Out-of-sample evaluation and reproducible trial comparison should precede a search UI. There is currently no optimizer, trial scheduler, metric ranking, or backtest CLI command.
