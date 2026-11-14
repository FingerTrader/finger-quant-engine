# Financial units and historical inputs

Implemented experimental P3-01 library profile. [Project status](PROJECT_STATUS.md) owns task closure. This contract owns financial primitives and historical inputs; [Backtesting](BACKTESTING.md), [Simulation](SIMULATION.md) and [Desk integration](DESK_INTEGRATION_PLAN.md) retain downstream account/fill/app acceptance. No data download, broker credentials, file discovery, login or Flutter runtime is required.

## Exact financial values

`fqe_core::finance` uses checked i128 Decimal coefficients and explicit scales, never floats. InstrumentId and CurrencyCode identify units. FinancialInstrument validates positive price/quantity/money steps, minimum quantity and nonnegative minimum notional. Prices have scale0..12 and absolute value<=1e12, order quantities scale0..6/value0..1e6, money scale0..6/absolute value<=1e18. Values must have the declared scale and be step multiples; no silent alignment occurs. Positive quantities must meet minimum quantity; zero requires explicit permission. Historical volume uses quantity scale but permits0..1e12 base quantity without trading-lot/minimum-order constraints.

Plain decimal parsing accepts an optional minus, canonical integer digits and an optional nonempty fraction. It rejects whitespace, exponent, plus, leading integer zeroes, negative zero, nonfinite values and nonzero precision loss; extra fractional zeroes may be removed exactly. Input is capped at64 bytes. `decimal_text` emits the exact fixed scale. PricePolicy NonNegative accepts zero; Signed permits negative prices explicitly. The cash-long history profile requires NonNegative; the signed-history profile requires Signed. Importing signed data does not claim a corresponding execution/account model.

`align` rounds to actual step multiples using Exact, TowardZero, Floor, Ceiling, HalfEven or HalfAwayFromZero. HalfAwayFromZero matches the inspected Desk halfUp tie direction, including negatives. No universal accounting rounding is implicit. `notional` checks price/quantity, multiplies exactly then applies the selected money-step rounding and checked money range. Minimum notional is validated metadata for later order admission; calculating a notional does not admit an order. Fees, reservations, ledger balances, margin and costs require separately versioned P3 profiles. Independent signed half-tie and extreme-overflow tests cover the primitives.

## Explicit UTC and calendars

`fqe_core::utc` accepts Gregorian `YYYY-MM-DDTHH:MM:SS[.1..6 digits]Z`, post-epoch dates only. Invalid days/leaps, offsets, local timestamps, leap seconds and nanosecond precision beyond six fractional digits are rejected. Checked microsecond→u64 nanosecond conversion fails on overflow; reverse conversion rejects sub-microsecond values. This preserves Desk microsecond precision without truncation and does not load a timezone database.

`fqe.history-spec/1` requires every field in the public [spec fixture](../crates/fqe-ir/tests/fixtures/history/cash-long-spec.json), rejecting unknown fields/versions through the shared bounded strict JSON loader. Calendar `utc-explicit-sessions/1` contains sorted, nonoverlapping, positive UTC intervals with epoch-aligned boundaries. Supported timeframe IDs are1m,5m,15m,30m,1h,4h,1d, with exact declared duration. Expected coverage is calculated before CSV allocation; maximum200000 bars and shared parser array/byte quotas apply. Sessions can be adjacent or separated by declared closures. Every slot in each session must be present exactly once. Acquisition must be at or after the final close; metadata performance start must be a declared bar open. No holidays, local session inference, fill-forward, split/dividend adjustment, instrument discovery or survivorship correction is supplied. Adjustment must be `raw-no-actions`; volume unit must be `base_quantity`.

## Bounded immutable dataset and revisions

`HistoricalDataset::from_csv` accepts host-supplied UTF-8 bytes, maximum64MiB. The exact ten column names/order are defined by CSV_HEADER and the [public four-bar CSV](../crates/fqe-ir/tests/fixtures/history/cash-long.csv). Rows are capped at8192 bytes. CRLF and correctly quoted single-line fields/escaped quotes are supported; embedded quoted newlines are unsupported. Instrument/timeframe, explicit true closed flag, OHLC ordering, tick/scale/range, nonnegative base volume, timestamps and exact declared session coverage are checked. Duplicate, backward, missing, extra and out-of-session records fail. Data is never silently sorted or deduplicated.

Source revision1 requires no parent/previous dataset. Revision n+1 requires the exact previous dataset content ID, previous object, compatible instrument/source label/timeframe/profile and nondecreasing acquisition. A new Arc-owned snapshot cannot mutate an existing dataset. Revised sessions/values retain their new identity. This is an in-memory lineage check, not persistent storage or authenticated vendor provenance.

Identity domains and exact encodings:

| Identity | Definition |
| --- | --- |
| spec source / CSV source | SHA256 of actual input bytes |
| spec ID | SHA256 of `FQE-HISTORY-SPEC/1` plus newline plus shared sorted JSON canonical bytes; original numeric string spellings remain significant |
| bars ID | SHA256 of `FQE-HISTORY-BARS/1` plus newline plus each compact JSON array of decimal strings `[sequence,open_ns,close_ns,open,high,low,close,volume]` followed by newline; prices/volume are declared-scale coefficients |
| dataset ID | SHA256 of `FQE-HISTORY-DATASET/1` plus newline plus compact JSON array `[specID,specSourceID,csvSourceID,barsID]` |
| selection ID | SHA256 of `FQE-HISTORY-SELECTION/1` plus newline plus compact JSON array `[datasetID,startOpenNs,endCloseNs,warmupCount]`, all strings |

Normalized bars ID alone does not identify units or provenance; use dataset ID. Quoting/line-ending/extra-zero changes may preserve bars ID while changing source/dataset ID. [Independent identity fixture](../crates/fqe-ir/tests/fixtures/history/independent-identities.json) was computed separately with Python stdlib sorted JSON/SHA256 and explicit UTC fixture times. Tests compare frozen values and never regenerate expectations.

`select` requires an existing bar-open start, bar-close end, positive range and sufficient preceding warmup<=10000 bars. It preserves original sequences and shares immutable data, including warmup before the selected performance interval. Selection explicitly freezes its own performance start, without modifying the dataset's original metadata. No selection or revision mutates another run.

## Frozen preparation and no-future cursor

HistoryStrategy can only be constructed from a validated compiled event/workflow plan. PreparedHistoricalRun validates exact instrument/timeframe/price/volume scales, and freezes sorted `fqe.history-run-inputs/1` bytes containing engine source identity, compiled artifact/source/execution/registry identities (therefore bound parameters), dataset/source/spec/bars/selection IDs, revision/parent/acquisition, financial units/profile, range/warmup, run ID and seed. Seeds are recorded; no randomness is introduced. `financial_execution:not-selected` and `stage:historical-inputs-only` explicitly exclude account/fee/fill selection. P3 later extends a financial run manifest rather than treating this input-only document as a complete backtest report.

HistoricalCursor emits at most one closed bar per call at an inclusive caller-supplied availability frontier. It rejects a backwards frontier without advancing, returns none before a close, marks warmup and declared gaps, preserves source sequence and gives each cursor independent position. The caller is trusted to supply availability correctly. The immutable dataset is intentionally inspectable; this cursor is an execution convention, not hostile-strategy memory isolation. Future workers need their separate permission boundary. Hosts must prevent order generation during warmup in later financial execution.

The current compiled EventRuntime requires continuous bars. Preparation rejects any selected gap even though the data importer/cursor can represent declared sessions. There is no implicit calendar-aware runtime join. `cargo run --locked --offline -p fqe-runtime --example historical_inputs` loads the public CSV/spec and compiled MACD/RSI plan, checks each close boundary, evaluates4 bars/1 warmup/0 signals and prints the frozen inputs. It produces no orders, fills or performance.

## Desk mapping and comparison baseline

Read-only source inspection on2026-11-14 found Desk main HEAD `f8e8b8ae0c8b88137ac38bb472dfc3fa17e14cb7`, origin finger-cloud/finger_desk,156 modified tracked and252 untracked files. These are dated observations, not a clean-commit guarantee. No Desk files were edited and no Desk packages/app/tests were executed. Selected actual dirty-source SHA256 values:

| Desk path relative to its root | SHA256 |
| --- | --- |
| lib/app/quant/market/quant_historical_data.dart | 68096ea5616b2251224eed53337124d0a3c4656ddac02d5412a148770240f5f9 |
| lib/app/quant/adapters/quant_dataset_repository.dart | 916322468a1a107473fefeeb99005ed9e0d7ff950617b53e092c5a4547dcd07e |
| contracts/quant/v1/schema.json | bc9548fd67eeb8c8c691266155fe829abc91fa1ca6b8a75e92d52b1a39eeab06 |
| lib/app/quant/backtest/quant_decimal.dart | c7359105c26b108ce7494dc0167ca8da307a61884b491bc97b4c7329af968265 |
| lib/app/quant/backtest/quant_cash_book.dart | e2947a0901f9bf351e635aabab54fd5ff5859057d4a50bed2444d147cbe4fe4d |

Desk uses decimal strings, UTC microseconds, these ten CSV columns, explicit sessions/raw-no-actions/base quantity, and frozen selection ownership. Conversion requires checked microsecond↔nanosecond and exact declared-scale strings. FQE source/spec/dataset identities differ from Desk `finger.quant.csv-dataset/1`/csv-v1; adapters must retain both identities and establish mapping. Desk currently sorts rows and deduplicates identical rows; FQE rejects either condition. Any adapter normalization must be explicit with new original/normalized identities. Decimal34 and FQE bounded i128 ranges also differ. These differences must be diagnosed before P4 migration; they are not silent parity claims.

[Versioned cash-long comparison fixture](../crates/fqe-ir/tests/fixtures/history/cash-long-comparison.json) uses only public synthetic prices and a hand-calculated1000.00 initial cash, buy2 at120 plus0.50 fee, sell2 at130 less0.50 fee, final1019.00/net19.00. Intent after bar1 fills at next bar2 open; intent after bar3 fills at bar4 open. P3-01 freezes this baseline; P3-02 must run actual Rust journal calculations against it and independently implement the documented legacy cash-long arithmetic comparison. Actual Desk backend execution/parity and UI migration remain P4; broader short/margin/partial-fill models require their own versions and fixtures.

## Verification boundary

Financial primitive, strict history, independent identity, immutable revision/selection, calendar/quote/limits, no-future/warmup, manifest determinism/binding and loaded example tests establish the implemented input profile. Full P1/P2 regression protects old numeric and temporal behavior. No dependency/manifest/lockfile/toolchain changes or new external scanners/uploads are required. Long-history storage/indexing, corporate actions, calendar-aware execution, virtual accounts, matching, performance reports, optimizer, app bundle, other-language execution and production workloads remain later acceptance.
