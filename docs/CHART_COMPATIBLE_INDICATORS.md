# Chart-compatible indicator profile

Owner-scoped 2026-11-16 follow-up to [strategy chart traces](STRATEGY_CHART_ALIGNMENT.md).
New Desk research/live executions explicitly request context `indicatorProfile:
"finger.k-chart/1"`. Missing profile retains the unchanged exact reference providers;
unknown profiles fail admission. Old reports and checkpoints are not rewritten.
Financial/account/order arithmetic remains exact decimal. This opt-in study profile
matches the existing Desk k_chart binary64 arithmetic and serializes finite outputs
at 12 decimal places; nonfinite/undefined outputs are null. Same-source float
comparison tests use absolute tolerance 1e-9. Render native observations for exact
threshold agreement at the serialization boundary, rather than recomputing signals
in the client. The client never owns rule evaluation or creates intents.

## Complete study inventory

| Study | Chart contract and native compatibility | Coverage |
| --- | --- | --- |
| MA / SMA | Add current, subtract expired, divide by p; ready at p. Chart early zero is a hidden drawing sentinel; native is null. | Default and custom periods, every prefix, Python SDK SMA |
| EMA | Native uses the first-price seed and recurrence of chart MACD's EMA components. Chart has no standalone EMA selection. | Independent recurrence, full prefix and quote replacements |
| MACD | First-price fast/slow seeds, zero DEA seed, first sample DIF/DEA/histogram zero; histogram 2*(DIF-DEA). All three periods resolved from native model. | Default 12/26/9 and custom 3/8/4 |
| RSI | Zero-seeded smoothed gain and absolute changes, first value at index p-1; flat/undefined null. | Default14 and custom7, threshold80, every prefix |
| BOLL | Middle rolling mean; bands first at index n, sample variance divisor n-1. Undefined bands null. | Default20/2 and custom7/2.5; stale pre-warmup chart fields cleared after pruning |
| ATR | Native true range plus mean-seeded Wilder smoothing; chart has no standalone ATR selection. | All native outputs checked against independent expected sequence |
| Highest / Lowest | Last p samples preceding offset; null until p+offset samples. No built-in chart selection. | Both extrema, nonzero offset and every prefix |
| KDJ | Chart seed K/D/J50, 9-point H/L, smoothing3/3, partial initial windows, flat RSV0. No native model node today. | Causality, empty/flat/nonfinite handling; not falsely advertised as executable |
| WR | Existing chart index i-14..i inclusive, first displayed calculation at index13; earlier -10 is a legacy drawing sentinel. No native model node. | Causality, warmup/flat/nonfinite cases; legacy window retained explicitly |
| CCI | Existing chart typical-price mean/deviation over up to14 points; undefined NaN becomes0. No native model node. | Causality and flat/nonfinite cases |
| Volume MA5/10 | Quantity-based 5/10 means; early drawing zeros. No quantity-SMA model node. | Causality and window readiness |

KDJ/WR/CCI/volume remain chart-only; this change does not invent authoring controls
or conflate them with another executable indicator. EMA/ATR/extrema remain native
trace outputs when no corresponding built-in chart overlay exists. All existing
native trace nodes/ports/settings remain separate; no SMA is relabeled as EMA.

## Source and rendering agreement

For quote-derived studies, replace the last UTC 500ms bucket on each quote and
recompute the retained 120-second prefix (max244 points), exactly like the current
realtime line. Quote period lengths count these points. Long periods can remain
unready in this bounded chart window; no fabricated historical ticks are supplied.
Explicit bid/ask/mid choices retain their source meanings. The paired Desk adapter
maps its chart-compatible `close` display input to the chart's existing ask-price
series; raw API/SDK payloads stay unchanged. Closed-bar studies retain incremental
initialization across their complete accepted prefix. Confirmed bars remain distinct
from unfinished candles and animation anchors; choosing a different display period
does not silently change a model's source period.

`signals.open` reports `indicator_profile`. Desk fails before trading if the bundled
peer cannot acknowledge the selected profile. Every advance exposes `chart_columns`
and `chart_sample` from the same evaluation as its intents; single-source warmup
also exposes `chart_samples` for every accepted bar. Research full-prefix traces
retain their existing schema and resolved settings. Profile-bearing model bytes
participate in input/execution identity. Desk consumes its accepted source prefix
for the matching product/period, projects native observations including nulls,
uses resolved study labels, and preserves frozen/animated study metadata. Normal
market viewing continues through the same chart formulas without an account.

## Verification and delivery

The synthetic chart fixtures in `crates/fqe-bridge/tests/fixtures/chart-profile-*.json`
are exported from Desk's `indicator_chart_alignment_test.dart` using its actual
DataUtil plus independent EMA/ATR/extrema checks. They contain no user market data.
The engine's standalone tests verify every native study over140 closed bars and700
quote updates (including replacement and pruning). Paired tests also cover custom
parameters, all chart-only studies' causal/empty/flat/reused-buffer behavior,
per-bar warmup projection, nulls, frozen snapshots, and existing live/mock-broker
and replay behavior. Required Rust checks and paired delivery results are recorded
in status/handoff. No dependency, lockfile, trading API, or page-shell changes.
Active user processes and orders are never restarted by validation; replacement
becomes active on the next normal application start. P4-03.03 remains IN_PROGRESS.
