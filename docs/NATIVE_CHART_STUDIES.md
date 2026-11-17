MACD version update (2026-11-17): new discovery advertises `finger.indicators/2`; [MACD v2](MACD_STANDARD_V2.md) defines new MACD defaults, canonical fields and warmup. Explicit profile1 remains supported for legacy fixtures. Other indicators retain the rules below.

# Native chart study authority

Owner-approved 2026-11-17 extension within P4-03.03. Experimental additive
`chart.studies/1` capability; no broader phase closure, dependency or financial
arithmetic change. Supersedes client-calculated normal-market studies in
[chart compatibility](CHART_COMPATIBLE_INDICATORS.md).

## Responsibility and protocol

All active indicator formulas/defaults/readiness live in FQE. Desk renders and
collects user parameter overrides; it must not calculate fallback values. Ordinary
market charts do not need an account, run/store lock, model or history record.
A dedicated bundled private peer uses existing framed exact-JSON RPC. `hello`
advertises `chart.studies/1`; `chart.describe` takes an empty object and returns
profile, quotas and definitions with kind/version/parameter defaults/output ports
and line/histogram hints. This is an engine catalog, not a new UI permission.

`chart.calculate` takes exactly `profile`, `bars`, `studies`. Desk sends `bars`
as a JSON array encoded inside a string, decoded by a dedicated strict/bounded
parser (duplicate keys rejected, at most160001 values/20000 array entries).
This preserves the old RPC envelope's4096-entry/32768-value quotas. Small direct
arrays remain accepted within those existing outer quotas. Profile is
`finger.k-chart/1`. Each bar contains decimal-string open/high/low/close/volume
and strictly increasing unsigned integer-string time. No timestamp conversion is
performed. Each study contains unique id, kind and a parameter string map; engine
resolves omitted defaults, rejects unknown keys and invalid ranges. Result has
profile, columns (id/kind/port/resolved parameters/render), and rows
(time/values aligned to columns). Finite values serialize at12 decimal places;
undefined or unready values are null. Negative zero canonicalizes to decimal zero.

Limits:20000 bars,32 study instances,existing4MiB request/64MiB response frames.
Native numeric samples must be finite with absolute value at most1e18; bounded
periods/offsets at most10000, positive periods/smoothing and deviation, fast<slow,
BOLL period>1. Out-of-range results reject rather than invent a clipped value.
No stored run, account access, worker installation or network call is created.

## Kernel and compatibility

Stateless chart prefix calculation and strategy chart-profile evaluation share
`research/chart_compat.rs::Accumulator`. Catalog covers SMA,EMA,MACD,RSI,BOLL,
ATR,Highest,Lowest,KDJ,WR,CCI and Volume MA. New chart support for KDJ/WR/CCI/Volume
does not advertise corresponding executable model nodes: graph admission remains
separate and authoritative. The old exact reference providers and immutable
report/model semantics stay unchanged.

Preserve original published readiness/window conventions: KDJ seed50, partial9
window and3/3 smoothing; WR retains the original inclusive i-14..i window with
first readiness at index13, null flat windows; CCI partial14 typical-price
mean/deviation, zero when undefined-flat; volume means ready at their full period.
MA early zeros and WR early-10 were legacy drawing sentinels, never trading values.

## Desk projection and extension

The paired Desk DataUtil is a transport/projection compatibility facade, not an
indicator calculator. Generic `calculateStudies` / `resultFor` preserve catalog
kinds and port columns for new render adapters; built-in approved menus retain
their present options. A future selection or drawing adapter may map a new port
without copying a formula into Dart. This change does not claim an automatic UI
for every new catalog kind or new graph authoring nodes.

Dedicated native execution observations take priority for model studies, retaining
resolved parameters and nulls; bounded display prefixes must not re-seed and
replace actual execution initialization. Each chart cache keeps current work plus
latest pending snapshot, discards old responses and validates the full response
before publishing. Freeze retains the source and native observations; subsequent
quote replacements cannot alter it. No failure triggers local recomputation.

## Verification

Native chart_studies tests cover the immutable pre-migration140-row fixture,
independent KDJ/CCI/volume readiness, WR upper-bound canonical zero, prefix/reuse,
instance isolation and malformed/quota rejection. Paired Dart tests call the actual
pinned native peer without account/store, compare default/custom closed-bar and
700 quote replacement/prune sequences, and verify supplied-only projection,
coalescing, stale/invalid response rejection and7200-bar transport. Rendering
regressions use supplied observations under Flutter fake time; they do not replace
native numeric verification. Actual restarted GUI acceptance is reported separately.

No new dependency or lockfile change. Packaging must pin source including this
module and the helper's bridge identity/hash. Do not replace an active user's
bundle/helper or restart their run during validation.

Final local evidence: [native chart receipt](evidence/native-chart-studies-20261117.json).
Required fixed/offline Rust checks passed403 tests; dedicated chart suite6;
paired Dart/Flutter suite49; analyzer zero errors/warnings (15 style infos).
Isolated macOS Debug build and deep/strict codesign passed. The actual signed
packaged helper was exercised unchanged from a sandbox-entitled first-party
parent:12 native catalog kinds,7200 rows/18 columns, no account/store, outside
source access denied. This is a packaging diagnostic, not the restarted Desk GUI.
The active user's app/strategy was not restarted or replaced. The candidate is
`/private/tmp/fqe-studies-derived/Build/Products/Debug/finger_desk.app`.
