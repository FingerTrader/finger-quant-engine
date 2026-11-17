# KDJ / Williams %R / CCI native standard — v3

Owner scope 2026-11-17: these three indicators only. Extend existing native studies,
configuration, preferences, graph ports and painters. RSI and MACD formulas,
defaults and UI are unchanged. No financial/transaction engine change or dependency.

## Audit

Desktop already used FQE chart.calculate, not Dart formulas. Existing chart definitions:
KDJ period9/smooth_k3/smooth_d3 (first-row50, early partial-window outputs,
flat RSV0); WR period14 (window included15 samples, no flat denominator guard);
CCI period14/HLC3/0.015 (early partial-window outputs). Research graph had no
callable KDJ/WR/CCI ports. Old generic settings persisted numeric overrides in
KlinePreferences, with no per-instance styles/visibility. Existing chart catalog,
DataUtil jobs and immutable preference objects provide the extension points.

## Identity and compatibility

New catalog and ordinary chart requests select `finger.indicators/3`. Version3
changes only KDJ/Williams %R/CCI; RSI is unchanged and MACD retains its version2
kernel. Explicit `finger.k-chart/1` and `finger.indicators/2` retain legacy rules.
Research/live models containing a new oscillator select v3; models without those
nodes keep v2. Old reports retain frozen observations. Editable old per-indicator
period/smooth_k/smooth_d overrides migrate to length/k_smoothing/d_smoothing for
new calculations, retaining configured values; newly created CCI defaults to20.

Menu stays MACD/KDJ/RSI/WR/CCI/None. WR's mathematical/internal native kind is
`WILLR`, dialog title **Williams %R**, output **williams_r**. Old `wr` requests are
legacy only, not a positive 0–100 transformation.

## Mathematics and readiness

All windows contain exactly the last Length accepted bars including the current
bar. Length and smoothing lengths are canonical positive integers1..10000; no
clamping, truncation or silent correction. Warmup outputs are null, not zero.

| Indicator | Calculation defaults | First ready bar | Outputs |
| --- | --- | --- | --- |
| KDJ | length9, k_smoothing3, d_smoothing3, smoothing_method smma, initial_k50, initial_d50 |9|k,d,j|
| WILLR |length14|14|williams_r|
| CCI |length20, source hlc3, constant0.015|20|cci|

KDJ: HH=max(High), LL=min(Low), RSV=100*(Close−LL)/(HH−LL); flat window RSV=50.
At the first complete window PreviousK/PreviousD are50, with no updates during
warmup. K=(RSV+(k_smoothing−1)*PreviousK)/k_smoothing;
D=(K+(d_smoothing−1)*PreviousD)/d_smoothing; J=3*K−2*D. SMMA means this recursion,
not SMA. No J clipping. Initial values are explicit native parameters (0..100),
UI uses the specified50 defaults; no other smoothing variants are admitted.

WILLR: −100*(HH−Close)/(HH−LL); flat window −50. Valid OHLC gives−100..0;
no sign reversal or language/market-specific formula. CCI: choose source from
open/high/low/close/hl2/hlc3/ohlc4; SMA is the mean of the current source window.
MAD is the mean of every window value's absolute difference from that same
current mean. CCI=(current−mean)/(constant*MAD). Constant must be finite and>0.
Identical source windows and zero MAD return0; no fixed bounds or clipping.
Reference lines never modify these raw results.

One FQE oscillator state machine serves chart batches, research backtests and
signals sessions, without Flutter/accounts. Each instance/node owns state. Native
graph reads the actual OHLC fields of the directly connected bars source;
scalar/quote inputs are rejected for these three OHLC indicators rather than
fabricating highs/lows. Live closed-bar feeds remain supported. Missing graph
samples do not advance state; no gap filling. Invalid/nonfinite chart OHLC or
open/close outside low..high rejects the request. Inputs use the existing native
12-decimal admission, binary64 core, and once-only12-decimal exposed serialization.
No intermediate rounding; exact serialized batch/incremental/reload parity is tested.

## UI and persistence

The existing menu summary and wrench open FullDesktopDialog. Parameters/Style/
Visibility tabs reuse approved typography, palette, fields and compact controls.
Parameters contain the listed calculation fields and reference levels. CCI Constant
is under Advanced. KDJ's only method is SMMA. Reference defaults: KDJ80/20,
WR−20/−80, CCI100/−100/midline0. KDJ/WR levels stay in their mathematical range;
lower<upper; CCI midline is between configured lower/upper.

Style: independent K/D/J colors, or single line color; line width0.5..4;
reference-line and indicator visibility toggles. Visibility selects existing chart
periods or all periods. Save validates every instance; Cancel discards nested
edits; outer chart Cancel still discards the complete draft. Reset affects only
the selected instance. Add/remove/select supports up to3 independent instances
per new family in the existing pane, without changing RSI/MACD instance limits.
The existing native32-study request quota still applies across all enabled studies.

Existing KlinePreferences stores stable instance id, calculation parameters,
reference levels, colors, width, visibility and periods. DataUtil sends only id,
kind and calculation settings. Reference/style/visibility changes repaint without
changing the calculation signature; all calculation settings and the negotiated
algorithm version identify cached results. No new framework, database, workbench
shell or navigation changes. J/CCI ranges include actual values beyond reference
levels. Native execution traces project their actual per-node values in replay
and live chart panes without recomputing a cropped display prefix.

## Native strategy interface

Existing research graph style, version3 context:

```json
{"context":{"indicatorProfile":"finger.indicators/3"},
 "nodes":[
  {"id":"k","kind":"kdj","settings":{"length":"9","k_smoothing":"3","d_smoothing":"3","smoothing_method":"smma","initial_k":"50","initial_d":"50"}},
  {"id":"w","kind":"WILLR","settings":{"length":"14"}},
  {"id":"c","kind":"cci","settings":{"length":"20","source":"hlc3","constant":"0.015"}}
 ]}
```

This is a settings excerpt, not a full runnable model. Each price input wires to
a bars node; native validation supplies the same source's actual OHLC. Outputs
are ratios k/d/j, williams_r, cci and connect to existing comparisons, crosses,
conditions and actions. Period/smoothing and constant/initial values can bind
existing typed strategy parameters; same graph is used by optimization. No
external language SDK public API is broken or published by this task.

## Acceptance

Tests and final build/GUI status are recorded in
PROJECT_STATUS / NEXT_SESSION. Do not infer verified status from this
contract alone. Fixed tests include independent window oracles, hand-calculated
KDJ fractions, CCI asymmetric MAD-vs-standard-deviation vector, warmup, zero
denominators, sources, lengths, flat decimal prices, invalid settings, instances,
batch/live/reload exact equality, config serialization, UI save/cancel/reset and
cache separation. No real orders or user history are required for acceptance.
