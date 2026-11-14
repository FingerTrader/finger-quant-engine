# Local simulation

Status: planned. No execution simulator, paper account, matching engine, or order-routing adapter is implemented.

Product target: users create virtual accounts locally; FQE supplies local fills, cash/position ledgers, fees/slippage, margin/leverage and versioned broker simulation profiles. A Finger Trader backend simulated account is not required. [ADR-007](ARCHITECTURE_DECISIONS.md#adr-007--local-simulation) fixes this boundary; P3 in [Delivery plan](DELIVERY_PLAN.md) owns implementation and independent accounting acceptance.

## Meaning

Simulation will run strategies on the user's device against an explicit virtual execution and account model. It may consume historical replay or a future live data adapter. Simulated fills are model outputs and must be labeled distinctly from live broker confirmations.

The current CLI emits Long signals from synthetic ticks. It does not create orders, close positions, simulate fills, or alter balances.

## Planned execution boundary

A strategy emits observations or, once defined, explicit order intents. A local host validates permissions and risk constraints. A simulation adapter applies a documented execution model and produces execution/account events for accounting.

A separate future live adapter may invoke the independent Finger Trader Trading SDK only after explicit user authorization and capability checks. The simulation adapter must never silently fall back to a live adapter. SDK credentials must not enter strategy IR, reports, or worker event payloads.

Offline simulation must remain available without login. A live adapter's authentication requirements must not become engine-wide requirements.

## Model decisions required before implementation

| Concern | Required decision |
| --- | --- |
| Time | Event time, scheduling, latency, ordering with market inputs |
| Orders | Supported order types and state transitions |
| Fills | Price policy, partial fills, liquidity, tick/lot rules |
| Costs | Commission, taxes if applicable, spread, slippage, rounding |
| Account | Cash, positions, reservations, margin if supported |
| Failure | Rejection, cancel/replace races, data gaps, model faults |
| Reproducibility | Seeded randomness, model version, input and configuration identity |

Backtesting and simulation should share event and accounting contracts. They need not use the same scheduling or data source. Neither mode should duplicate financial truth inside a client UI.

## Acceptance gates

Introduce a small, explicitly scoped model before complex market realism. Validate it with manually calculated order/fill/ledger fixtures; test conservation and reconciliation properties, invalid quantities, insufficient funds, and ordering edge cases.

The mode and model version must appear in every report. Do not market a synthetic rule demo as paper trading or claim equivalence between simulated and live execution.

The first Desk product scope is historical-data backtesting/local simulation. P3/P4 must preserve or explicitly version the existing cash-long model through independent comparison fixtures and one authoritative backend per run; see [Desk migration](DESK_INTEGRATION_PLAN.md#state-semantics-and-migration). A broader FQE partial-fill/margin profile cannot silently replace old cash-long semantics. Real-time market-data simulation and live trading require their own later acceptance; this first offline loop does not claim either.
