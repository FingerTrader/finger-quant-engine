# Professional Indicator Library P0 — native profile 4

Owner-requested scope, 2026-11-17. This is an additive implementation inside P4-03.03, not completion of that phase or a public SDK release. See ADR-043, PROJECT_STATUS and the paired Desk evidence at `docs/quant/acceptance/professional-indicators-p0-20261117/evidence.json`.

## Audit and compatibility decision

The existing bridge already supplies `chart.describe`, `chart.calculate`, graph research, backtest, optimization and causal signal state. Desktop has shared indicator configuration and two secondary pane slots. Existing lowercase SMA/EMA/BOLL/ATR calculations include different seed/standard-deviation semantics, so silently changing those APIs would alter old models and reports. New canonical kinds `MA`, `BOLL`, `ATR`, `ADX`, `VWAP` require `finger.indicators/4`. Old profile admission and formulas remain available. RSI, MACD and the version3 oscillators are unchanged; v3/v4 regression compares their exact output.

One native `research/professional.rs::State` supplies both batch chart calculation and event graph evaluation. Dart and the optional Python adapter contain configuration/transport/rendering, not a second formula implementation. No financial core, ledger, margin or order execution changes. No new dependencies or lockfile changes.

## Registry and parameters

`chart.describe.professional_profile` advertises v4 without replacing the old default profile. Each new entry provides kind, display name, category, defaults, parameter schema, outputs, version=4, default pane and style schema. Values on the wire are explicit decimal/integer strings. Unknown/invalid parameters are rejected by native admission, not silently repaired.

| Kind | Defaults | Outputs | Units / default placement |
| --- | --- | --- | --- |
| MA | type=ema, length=20, source=close | value | price / overlay |
| BOLL | length=20, source=close, ma_type=sma, stddev_multiplier=2.0 | upper, middle, lower | price / overlay |
| ATR | length=14, smoothing=rma | atr | price / pane |
| ADX | di_length=14, adx_smoothing=14 | adx, plus_di, minus_di | ratio / pane |
| VWAP | source=hlc3, anchor=session, calendar=unknown, timezone=UTC, volume_type=unknown, bar_interval_seconds=60 | vwap | price / overlay |

Sources: open/high/low/close/hl2/hlc3/ohlc4. Lengths are canonical positive integer strings 1..10000. BOLL multiplier is finite, >0 and <=100. MA types sma/ema/wma; BOLL ma_type is currently sma only; ATR smoothing rma/sma/ema. VWAP anchors session/weekly/monthly. The extra VWAP fields are necessary data-admission metadata, not cosmetic options.

Example chart study (within `chart.calculate` profile4):

```json
{"id":"ema20","kind":"MA","parameters":{"type":"ema","length":"20","source":"close"}}
```

Graph nodes use the same kind/settings, a direct OHLCV `bars.close -> indicator.price` connection, stable typed output ports and the existing parameter-reference mechanism. VWAP cannot reconstruct volume/high/low through arbitrary scalar transforms. Its declared interval must equal the source stream timeframe. Native graph calculation runs independently of Flutter; optional Desktop lowering preserves canonical kinds and profile4 for both single and multiple source execution. Existing numeric parameter binding/grid validation applies to optimizable numeric parameters; enum/calendar fields remain explicit settings.

## Mathematics and readiness

All new indicators use binary64 working state and the existing checked decimal12 input/output boundary. No per-step UI rounding feeds state. Batch/live invoke the same step operation in the same order. A missing warmup output is null/not ready, never a fabricated zero. Invalid OHLC, non-finite numeric input and malformed/missing required fields fail admission.

MA: SMA is the full N-window mean. EMA initializes with the full N-window SMA at bar N, then alpha=2/(N+1). WMA oldest-to-newest weights are 1..N. SMA/EMA/WMA first valid at N.

BOLL: middle=SMA(source,N); variance=sum((x-middle)^2)/N; upper/lower=middle +/- multiplier*sqrt(variance). Full window required. Population, not sample standard deviation. All bands use the same middle.

ATR: first TR=high-low; subsequent TR=max(high-low,abs(high-previous close),abs(low-previous close)). RMA initializes with SMA of the first N TRs then alpha=1/N; SMA/EMA alternatives have the same full-window readiness (EMA alpha=2/(N+1)). Raw price-distance output is usable by strategies; this task does not change stop/order/risk execution.

ADX: first DM=0; subsequent +DM is upMove only if upMove>downMove and >0, -DM is downMove only if downMove>upMove and >0. TR, +DM and -DM each use SMA-seeded Wilder RMA(di_length). DI=100*smoothed DM/smoothed TR. TR=0 -> both DI=0. DX=100*abs(plusDI-minusDI)/(plusDI+minusDI); zero denominator ->0. ADX=RMA(DX,adx_smoothing). DI first valid at di_length, ADX at di_length+adx_smoothing-1 (defaults14/27). Reference25 is display metadata; ADX is strength, not automatic buy direction.

## VWAP supported calendar and data contract

VWAP=sum(source*declared volume)/sum(declared volume), reset by actual anchor. The current OHLCV protocol does not provide a trusted universal exchange calendar or volume provenance. P0 therefore requires an explicit caller declaration `calendar=24x7`, `volume_type=real` or explicitly selected `volume_type=tick`; defaults unknown remain unavailable. Tick counts are weights when tick is selected, never represented as real traded volume. Manually declaring a field does not verify its provenance.

Supported: UTC or fixed UTC+/-HH:MM offsets up to14:00; session local midnight, weekly Monday, monthly day1. Input bars are intraday, interval positive and <86400 seconds and dividing86400. Opening Unix milliseconds identify chart bars. Graph events use closing nanoseconds and subtract the declared interval to recover the same opening instant. Daily/weekly/monthly input bars are rejected; weekly/monthly anchors use intraday bars.

The first input in an anchor must be exactly the anchor opening. Each subsequent input must be contiguous at the declared interval. A cropped prefix, missing bar or invalid volume makes that entire anchor unavailable until the next complete anchor. Zero cumulative volume ->null. Missing volume is rejected; unknown calendar/volume ->null. Consumers must load complete anchor history then crop outputs. A previously valid point is not recomputed from a different cropped first bar. Replay/reload with identical input prefixes is exact.

Not implemented: exchange opens/closes/overnight breaks, holidays, IANA timezones/DST, data-source volume attestation or reconstruction from missing lower-granularity bars. Such markets must remain unavailable rather than receive a misleading Session VWAP. This bounded calendar support is not universal exchange-calendar acceptance.

## State, cache and client configuration

Each study ID/node owns isolated native state. Existing quotas still apply. Source timestamp/OHLCV prefix, kind, calculation profile/version and all mathematical parameters identify results; colors, reference lines, widths and visibility do not enter calculation requests. Changing parameters opens/recalculates the relevant native state. Refresh/reload uses the same complete input prefix.

Desktop reuses its Parameters / Style / Visibility dialog with preview/save/cancel/reset and persisted instance IDs, colors, pane and timeframes. Main overlays coexist. Current UI limit: three instances per family and two preexisting secondary pane slots; incompatible ATR-price and ADX-ratio families cannot share one pane. Existing MA7/25/99 settings remain. Per-window separate preference stores and unlimited panes are not added.

The Python SDK adds an optional stdlib-only `NativeIndicators` client and ma/boll/atr/adx/vwap factories. It requires a caller-provided trusted helper and reviewed SHA256, negotiates profile4, uses framed existing native messages and preserves decimal strings/null. It downloads nothing, has no UI/broker/server calls and leaves existing SDK SMA/crossing APIs intact. `calculate` is batch; `stream(...).update(closed_bar)` retains causal native state. A helper supports one active signal stream; independent helpers support independent streams. The adapter is local experimental code, not a published SDK version. SDK-only users need not install or execute FQE.

## Verification and performance

2026-11-17 final frozen candidate:

- Rust full workspace:432 passed,0 failed, including12 new professional tests.
- Paired Desk:57 passed across professional configuration/painter/native projection, legacy indicator and replay alignment, and actual compiled/backtested/persisted strategies for all five canonical kinds.
- SDK:92 tests and1285 subtests passed; native7200-bar batch/stream/reload exact equality and independent hand vectors.
- fmt, cargo check, strict Clippy, macOS Debug build and deep strict signature verification passed. Relevant Dart analyzer:0 errors/warnings;373 info-level style messages retained.
- Actual rebuilt Desk: all five dialogs, SMA/EMA/WMA selection, BOLL and ATR previews, style and timeframe/pane controls, simultaneous BOLL+ATR+ADX rendering, restart restoration and deletion exercised. Temporary instances removed after verification.
- Existing finance source identity stays `sha256:87bb3e8ec3c3db84389de803955ecb82ee498032500ef13374f529cff4ac12c7`.

Independent fixed vectors cover BOLL population variance, weighted MA, Wilder ATR/ADX, flat/zero cases and sources; VWAP covers session/week/month/fixed offset reset, gaps/cropping/missing volume, invalid granularity and interval mismatch. Old oscillator outputs are compared unchanged across profiles.

Each incremental update advances retained state, not a replay of the full history. VWAP is O(1)/bar; recursive EMA/RMA arithmetic is O(1) after seeding, while SMA/WMA/BOLL scan bounded windows O(N). No large-window/device-independent latency claim follows from the7200-bar correctness fixture. Future rolling-statistics optimization must preserve this version's tested numeric behavior or introduce a new calculation version.

Owner-approved maintenance 2026-11-17: explicit Tick Volume support is additive within profile4; real inputs and existing indicator mathematics are unchanged. The source-derived bridge identity changes and volume_type remains in each instance/cache request. Tick/real declarations are not automatically converted or inferred.
