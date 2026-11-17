# MACD standard v2

Owner-requested scoped implementation within P4-03.03 (2026-11-17). Only MACD
changes; other study rules, RSI, financial arithmetic and legacy profiles remain.
See [native chart transport](NATIVE_CHART_STUDIES.md) for bounded account-free
requests and [legacy chart profile](CHART_COMPATIBLE_INDICATORS.md) for historical
version 1 arithmetic. The latter's MACD rule is superseded for new Desk runs by
this explicit version; legacy profile requests remain executable unchanged.

## Audit and identity

`reference.macd@1` has fixed-point mean-seeded EMA, a warmup and 1× histogram.
`finger.k-chart/1` has binary64 first-price oscillator seeds, zero signal seed,
first-bar output and fixed 2× histogram. These are different numerical identities.
Neither is silently redefined. New chart.describe advertises
`finger.indicators/2`, MACD definition version 2, canonical ports and full defaults.
New Desk chart/research/live executions explicitly select that profile. Old saved
reports retain their frozen traces; reopening is not recalculation. Old editable
Desk drafts acquire the v2 default configuration fields for a newly identified run.

Native graphs require profile 2 when the new MACD parameter set is present;
incompatible versions fail preflight. Legacy line/signal graph ports remain exact
aliases of macd_line/signal_line for old connections; chart.calculate v2 emits only
three canonical columns. FQE graph observations may contain aliases, and consumers
must suppress duplicate curves while keeping alias connections resolvable.

## Calculation parameters

```json
{"fast":"12","slow":"26","signal":"9","source":"close",
 "oscillator_ma_type":"ema","signal_ma_type":"ema","histogram_multiplier":"1.0"}
```

The native wire uses string values. Lengths must be canonical positive integers
1..10000 and fast < slow. Source is open/high/low/close/hl2/hlc3/ohlc4. Both MA types
are independently ema or sma. Histogram multiplier is 1.0 or 2.0; integer 1/2
spellings are also admitted. Unknown parameters, invalid numbers, nonfinite input,
unsupported sources/types and multipliers fail; no coercion/clamping occurs.

MA source definitions: hl2=(high+low)/2, hlc3=(high+low+close)/3,
ohlc4=(open+high+low+close)/4. MACD Line = oscillator MA(fast)-oscillator MA(slow).
Signal Line = signal MA(MACD Line, signal). Histogram = multiplier*(MACD Line-
Signal Line). Standard (1×) and Scaled (2×) are the only UI Histogram Mode options.
Multiplier participates in the model/input identity and strategy conditions;
it never changes either line. No region-based names are used.

## One core and readiness

`research/macd.rs` owns all v2 MA state. Chart batch requests and graph ChartState
use this same kernel. EMA seeds at the mean of its first complete window, then
uses previous+(value-previous)*2/(length+1). SMA uses a rolling sum/window.
Signal MA consumes only valid MACD Line observations. Default line first becomes
ready on bar 26 and signal/histogram on bar 34. Outputs before readiness are null;
there is no zero sentinel, early crossing or fabricated ready value.

MACD OHLC accepts the same plain-decimal domain as graphs, at most 12 fractional
digits; excess precision is rejected, not rounded at admission. Decimal-to-binary64
conversion goes through canonical decimal text in the v2 graph only, so other
indicators retain their previous arithmetic. All indicator intermediates use
binary64; exposed finite values serialize at 12 decimal places
with no intermediate rounding. Ledger values remain exact decimal. Batch and
incremental equality is asserted on serialized strings, not separate client
formulas. For an equation assembled from already-rounded output fields, use 2e-12
absolute tolerance. Reload/replay of the same accepted prefix is deterministic.

Missing graph inputs do not advance the state or fill gaps. Chart requests reject
invalid/nonfinite OHLC rows. Duplicate source stamps are not resampled. Existing
quote semantics replace the current 500ms bucket and recompute the retained
120-second bounded prefix. Equality requires the same prefix and parameters;
different warmup history is not the same input.

A directly connected bars node supplies original OHLC to the shared kernel,
regardless of its historical price-port field. Source selects the intended input.
A quote/scalar input supports close as its connected scalar. Other source values
require a bars node and fail compile admission otherwise, never fabricate OHLC.

## Consumers and instances

chart.calculate accepts up to 32 independent study ids; each owns separate kernel
state. Duplicate ids fail admission. Desktop persists independent parameters,
colors, width, zero line and timeframe visibility and excludes presentation data
from the native cache request. New MACD settings propagate identically through
research execution/optimization and signals.open/advance. Single-source Desk MACD
input is lowered to research rather than falling back to reference.macd@1.

A language-neutral adapter invocation can expose
indicators.macd(fast=12, slow=26, signal=9, source="close",
oscillator_ma_type="ema", signal_ma_type="ema", histogram_multiplier=1.0), lowering
to the seven settings and reading macd_line, signal_line and histogram.
This task implements native graph/chart APIs; it does not publish or modify an
external language SDK. Immutable reference-provider SDK users must explicitly
migrate their adapter/version, never assume v1 has changed.

## Acceptance

`crates/fqe-bridge/tests/macd_standard.rs`: native default/warmup independent oracle,
invalid inputs, isolated instances/reloads, histogram-only scaling and all seven
sources × four MA combinations, each140-bar prefix compared batch versus actual
signals sessions twice. Legacy chart-profile fixtures continue to pass unchanged.
Paired Desk tests assert7200-row native projection, configuration persistence,
Save/Cancel/Reset, instance isolation, style-cache separation and trace aliases.
Final checks and remaining boundaries are recorded in PROJECT_STATUS/NEXT_SESSION.
No new dependencies or Cargo.lock change.
