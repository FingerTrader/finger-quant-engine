# Indicator Functions — Contract 1

**Implemented, experimental P1-06 library slice.** [Indicators](../crates/fqe-ir/src/indicators.rs) register SMA, EMA, MACD and RSI through the shared [provider registry](PROVIDER_REGISTRY.md); [tests](../crates/fqe-ir/tests/indicators.rs) and an [example](../crates/fqe-ir/examples/indicators.rs) exercise that public interface. This owns their formulas, numeric policy and the incremental-scalar/1 profile. [P1 design](P1_STRATEGY_DESIGN.md) remains the strategy target; [status](PROJECT_STATUS.md) owns progress. No market scheduler, complete model execution, snapshots, external provider ABI, backtest or Desk bundle is delivered here.

## Registration and signatures

`indicators::register(builder, namespace, input_type)` adds four ordinary entries `<namespace>.sma`, `.ema`, `.macd`, `.rsi`, author version `1`. There is no financial-name branch in the registry or expression evaluator. The formula selector is private implementation code for these four reference providers; arbitrary conforming providers use the same `register_incremental` interface. Multiple input types require distinct namespaces. Registration/discovery executes no factory or strategy code. Family registration is sequential; discard the builder if a batch fails.

Input `close` is exactly one Decimal scalar, scale 0–12, unit Scalar or Price(instrument). Zero and signed prices are accepted by this scalar profile; legacy positive-u64 tick behavior remains unchanged. No implicit unit/scale conversion at the call boundary. Internally input promotion to work scale 18 is exact. Quantity/Money/Ratio inputs, floats, other shapes and scales 13–18 fail registration. Decimal output scale is always 12. SMA/EMA/MACD retain the input unit; RSI uses Ratio with values in 0–100, **not** 0–1. Output ports are separate typed scalars.

| Entry | Required U64 parameters | Outputs and readiness |
| --- | --- | --- |
| SMA | period p, 1–10000 | value; first ready at accepted contiguous close p |
| EMA | period p, 1–10000 | value; first ready at close p |
| MACD | fast, slow, signal, each 1–10000; fast < slow | line first ready at slow; signal/histogram first ready at slow+signal−1 |
| RSI | period p, 1–10000 | value; first ready at close p+1 (p differences) |

Required/default/unknown/null/readiness/bounds rules come from the immutable parameter schema. MACD's cross-parameter relationship is validated before creating an instance. No defaults, optional parameters, dynamic period changes or prefix look-ahead. Changing parameters requires a fresh instance. Gaps in logical sequence numbers alone do not imply missing samples.

## Formula and numeric policy

All arithmetic uses checked signed i128 coefficients. Work scale **18**, output scale **12**, rounding **nearest, ties to even** (HalfEven). No binary float arithmetic. Accepted `abs(close) <= 10^12`; an out-of-range input or checked intermediate overflow returns `registry_numeric` without committing state. No saturation, wrap, silent fallback, inferred tick size or ledger policy. RSI's conditioning bound below is also required.

- SMA maintains the current last p samples and exact checked sum. Round sum/p to work18, then round the ready output to output12.
- EMA seeds with the work18-rounded arithmetic mean of its first p samples. Thereafter round `(2*x + (p−1)*previous)/(p+1)` once to work18. This equals alpha=2/(p+1) mathematically; alpha itself is not pre-rounded. The output is rounded separately; subsequent state uses work18, never displayed output12.
- MACD runs mean-seeded fast/slow EMAs. Its work18 line is fast−slow. Feed only ready work18 lines into the mean-seeded signal EMA; histogram is work18 line−signal, no factor of two. Each exposed port rounds independently to output12. Do not reconstruct histogram from separately rounded output ports and expect bitwise equality.
- RSI takes consecutive close differences. Seed mean nonnegative gain/loss from the first p differences, each rounded to work18; thereafter round `(previous*(p−1)+current)/p` for each Wilder mean. If both means are zero return 50; otherwise `100*gain/(gain+loss)`, with only gains 100, only losses 0. Compute the positive ratio by bounded long decimal division with final work18 HalfEven rounding; avoid a `numerator*10^18` intermediate. Round the ready RSI to output12.

RSI requires a nonzero work18 mean gain+loss **at least 10^-6 in input price/scalar units**. A smaller positive sum fails with `registry_precision` (a conditioning-limit diagnostic); it is not treated as flat or rounded to a decisive oscillator. Exact flat data remains 50. This deliberately limits version 1 for numerical conditioning and prevents gradual decay into false flat readings. Users must choose an explicit supported input unit/series or a different future provider contract; this profile does not automatically normalize prices or bypass the guard. These limits affect usability and must be surfaced during future client preflight/errors. Very small price movements are not generally supported by this RSI version.

With the accepted bounds, close work coefficients are at most 10^30 in magnitude, sums/weighted numerators at most O(10^34), safely inside i128; every operation is still checked. SMA history is at most 10000 samples; the other formulas keep fixed-size accumulators. Counts saturate by stopping at the seed length, rather than growing with the dataset. Host accepted-step counters are checked U64.

## Accuracy and decisions

[Independent evaluation](P1_06_NUMERIC_EVALUATION.md) compares work12 and work18 against exact rational recurrences. [Fixture generator](../scripts/generate_indicator_fixtures.py) imports no engine and uses Python stdlib Fraction, exact mean seeds and unrounded rational alpha/Wilder algebra. It writes the tracked [nine datasets](../crates/fqe-ir/tests/fixtures/indicators.json), including G-002/G-007, constant/rising/falling/signed/short/large and 512-sample sequences. Tests never regenerate their expectations. The 563 input samples produce eight output series; readiness must match exactly, ready coefficients may differ by at most **1 at scale12 (10^-12)** from independently rounded references. This fixture tolerance is a verification bound, not a runtime comparison epsilon.

For all supported periods, elementary rounding-error propagation gives work18 absolute error ≤p/2 quanta per Wilder mean and ≤(p+1)/4 per EMA, since each recurrence contracts the prior error and adds ≤0.5 work quantum. SMA adds ≤0.5 work quantum. MACD propagates errors of both EMAs into signal/histogram. Including final output rounding and double-rounding ties, a conservative absolute bound of **6×10^-13 input units** covers SMA/EMA/MACD ports for periods ≤10000. RSI divides by small means: using the ≥10^-6 nonzero denominator guard, a conservative absolute bound **2.1×10^-6 RSI points** covers mean-conditioning error at the largest period, plus ≤0.5×10^-12 output rounding. The much smaller measured fixture errors do not strengthen this worst-case bound. No relative-error guarantee near a zero line/histogram is claimed.

Comparisons/crossings use exact output12 values and strict documented operators, never the fixture tolerance. A mathematically distinct value within the rounding/error bound of a threshold can map to equality or the other side; no universal agreement with unrounded real-number decisions is promised. Tests preserve ±one output quantum at thresholds and near the 10^12 input limit (binary64 loses it), signed HalfEven ties, G-007's bar-5 cross and absence of first-ready crosses. P1-11 must also verify loaded complete-strategy decisions; ledger/tick/lot/currency rounding remains P3-01.

## Incremental scalar profile and transactions

Shared Descriptor schema/API/provider versions remain 1, with the negotiated new required capability **provider.incremental-scalar/1**. A supporting old host rejects that unknown capability; pure descriptor canonical bytes and their SHA fixture remain unchanged. Stateful descriptors require deterministic=true, Purity::Stateful, ProviderManaged readiness, history 1–20000, a valid bounded state_schema identity, no pure capability, host services or dependencies. Indicator category requires this profile; functions/predicates may also use it. Reference history declares the maximum initial contiguous samples needed to seed all outputs across valid parameter bindings: SMA/EMA=10000, RSI=10001, MACD=19999. Recurrences can retain the influence of older samples indefinitely; this field is not a finite total dependency horizon, serialized buffer layout or resource meter. The actual fields, including purity/readiness/history/state_schema, participate in descriptor canonicalization and identity.

`Registry::instantiate` resolves the exact artifact/version, binds immutable parameters, validates payload/relationships/IDs, calls a reviewed IncrementalProvider factory and creates a fresh IncrementalInstance. Every instance owns its state, run/instance identity and cursor; the registry holds only a shared factory. Discovery does not create state. Stateful entries are rejected by pure `prepare`/`prepare_node`; they cannot enter repeated pure Expr::Call positions. [P1-07](GRAPH_COMPILER.md) adds pinned PreparedIncremental handles without factory creation; P1-08 still must schedule/commit generic stateful subscriptions, with no name-specific expression branches.

`IncrementalInstance::step` requires positive strictly increasing logical sequence. It checks names/arity/types/unit/scale/payload/Error inputs before provider execution. For ready inputs it forks the committed state, invokes the staged state, validates **all** named outputs and output budget, rejects numeric errors, and commits staged state/cursor/count only on success. Provider-managed Warmup/Missing outputs are allowed. No partial output is returned on failure. Errors, caught unwinding panics and invalid outputs retain state/cursor; retry at the same failed sequence is allowed. Duplicate/backward/zero sequences fail before state advancement. Accepted Missing/Warmup inputs reset via a fresh factory state and propagate to every output (Missing dominates); reset commits only if creation succeeds. An Error input does not reset or advance.

`IncrementalState::fork` must deeply copy mutable history; built-ins use deep clones. Traits require reviewed trusted Rust implementations. Malicious shallow clones/interior mutation, factory side effects, I/O, loops, aborts and allocation/process failure are not isolated or rolled back. Payload budgets and history declarations do not enforce CPU/heap quotas. A state_schema string identifies a declared in-memory state contract; it is **not** an implemented snapshot/restore codec. External workers/plugins, permission enforcement and durable state remain later tasks.

This is an atomic **single-node** step. Manually stepping two indicators and a crossing, as in G-007, does not make the whole multi-node event atomic. P1-08 must stage all subscriptions and signals together and advance each once independent of Boolean guards; P2 owns generalized state persistence. The old synchronous runtime and offline demo retain their original behavior.

## Runnable acceptance

Use the pinned toolchain exports in [Development](DEVELOPMENT.md#project-local-toolchain):

```sh
cargo test -p fqe-ir --test indicators --locked --offline
cargo run --locked --offline -p fqe-ir --example indicators
```

The example drives MACD(2,3,2) over G-002 through the public registry: line first ready bar3; signal/histogram first ready bar4; six accepted steps. Existing unknown-name pure tests and independent pure canonical identity stay unchanged. An independent unknown-name stateful counter test proves the same generic interface and rollback after provider failure, panic and invalid output, with separate instances. It does not prove external language/plugin execution or binary loading without rebuilding the host.
