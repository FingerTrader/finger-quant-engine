# Type and Parameter Contract

Status: **implemented, experimental P1-02 subset** in [numeric primitives](../crates/fqe-core/src/numeric.rs), [typed values](../crates/fqe-core/src/types.rs) and [parameters](../crates/fqe-ir/src/parameters.rs). This is a Rust API and semantic contract, not a serialized model format, graph compiler, financial ledger or worker protocol. [Project status](PROJECT_STATUS.md) owns acceptance/commit evidence. [P1 design](P1_STRATEGY_DESIGN.md) and [Extensibility](EXTENSIBILITY_CONTRACT.md) define the broader destination.

## Numeric evaluation and decision

The evaluated candidates and their consequences for this bounded slice are:

| Candidate | Strength | Limitation / disposition |
| --- | --- | --- |
| Existing positive u64 ticks | Exact integer comparison, preserves the legacy demo | Cannot express signed/zero prices or fractional indicators; integer 1/3 truncates to zero. Retained only for the existing IR. |
| IEEE binary64 | Broad numerical ecosystem and finite fixed storage | Executed probes show 0.1 + 0.2 differs from 0.3, and adjacent values at u64::MAX collapse to the same float. Not an accepted value representation in this subset. |
| Bounded exact rational | Represents 1/3 and independent SMA/EMA reference fractions exactly | Fixed-width numerator/denominator growth can overflow in repeated recurrences; unbounded rationals require a reviewed dependency and resource policy. Retained as an independent oracle approach, not implemented as a production scalar. |
| Checked i128 decimal coefficient plus per-type scale | Exact base-10 input, explicit precision and rounding, signed values, no dependency | Finite range/precision and conservative intermediate overflow. Selected for this slice; tests cover arithmetic, boundaries and signed rounding. |

No throughput benchmark or arbitrary-precision library evaluation is claimed. Executable candidate probes and independent numeric examples are in [value contract tests](../crates/fqe-core/tests/value_contract.rs). [ADR-013](ARCHITECTURE_DECISIONS.md#adr-013--checked-decimal-and-bounded-parameter-subset) records the accepted bounded choice. P1-06 still owns each indicator's working/output scale, formula/seed, rounding points, error/tolerance and decision-boundary tests. P3-01 still owns financial currency/tick/lot/fee rounding and ledger reconciliation. This choice alone closes neither gate.

## Decimal values

`Decimal = coefficient * 10^(-scale)`, with coefficient in [-2^127, 2^127-1] and scale 0–18 inclusive. Resolution is 10^(-scale); range is the coefficient range multiplied by that resolution. Scale is part of identity: 1 at scale 0 and 10 at scale 1 are not interchangeable without explicit rescaling. Zero retains its scale. Decimal construction permits negative and zero values, including Price/Quantity/Money primitives; execution/profile rules must impose domain-specific constraints later. The old positive-only `PriceTicks`, its input events, IR_VERSION=1 and CLI behavior are unchanged.

- Add/subtract/compare require equal scales. Multiply/divide require equal operand scales and return that same scale. Raw Decimal operations are unit-free; P1-05's typed expression/function validation must additionally enforce units and dimensional semantics.
- Multiplication computes the checked coefficient product, then divides by 10^scale. Division computes the checked numerator times 10^scale, then divides by the denominator. A zero denominator fails. Intermediate overflow fails even when an algebraically reduced final value could fit; there is no cancellation, wide accumulator, wrap, saturation or float fallback.
- Rescaling upward uses checked multiplication; rescaling downward and multiply/divide require explicit `Exact`, `TowardZero` or `HalfEven`. Exact rejects any nonzero remainder. TowardZero truncates symmetrically. HalfEven chooses nearest, with ties to an even coefficient, for positive/negative numerators and denominators. Integer quotient/remainder overflow also fails, including MIN/-1.
- Display emits a base-10 diagnostic value preserving fractional zeros and supports i128::MIN. The primitive has no stand-alone parser; [P1-03 model loading](MODEL_LOADING.md) implements its experimental typed JSON representation. It preserves the full signed coefficient, scale and u64 values without passing through JSON/JavaScript floating-point numbers; this does not select a Desktop/worker codec.

Independent examples: 0.1 + 0.2 = 0.3 exactly; 1/3 at scale 2 is Inexact under Exact and 0.33 under HalfEven; 0.125/0.375/0.625/0.875 at scale 2 round to 0.12/0.38/0.62/0.88. Negative ties are symmetric. These are numerical contract tests, not return/performance or ledger calculations.

## Versioned supported type subset

`VALUE_CONTRACT_VERSION=1` is independent of IR/package/protocol versions. A validated `TypeDescriptor` is constructed from requested version, shape and scalar type; private fields prevent bypass through ordinary safe construction.

| Implemented scalar | Value / validation |
| --- | --- |
| Bool | Boolean only; no truthiness or integer conversion |
| I64 / U64 | Full signed/unsigned 64-bit ranges, distinct types; no implicit conversion |
| Decimal | Validated scale plus NumericUnit; exact descriptor and coefficient scale must match |
| Text | UTF-8 with descriptor max_bytes in 1–4096; empty text allowed, byte length enforced |

NumericUnit distinguishes Scalar, Ratio, Price(instrument), Quantity(instrument) and Money(currency). Instrument uses the existing validated case-sensitive identifier. CurrencyCode accepts 1–16 uppercase ASCII letters/digits as a case-sensitive unit identifier; it neither verifies an ISO registry nor performs conversion. Different instruments, currencies, domains, scales or text capacities are different descriptors. The P1 binder requires exact descriptor equality. A value tagged USD cannot satisfy EUR; a Price cannot satisfy Quantity or Ratio.

Only **scalar shape** is implemented. Series, Window, Record, Collection, unknown shapes, unknown scalar types and unsupported contract versions are rejected at descriptor construction. Timestamp, Duration, InstrumentRef, nominal enum types and floating values remain unsupported scalar requests. A finite typed parameter choice set implements enum restrictions; it is not a nominal enum scalar. Future multiple named scalar ports do not require an opaque record value; P1-04 owns those port signatures. Series/timeframe alignment and window evaluation remain P2 work. No advertised capability is inferred from a request enum variant.

`TypedValue` contains a validated descriptor/value pair with immutable accessors. `Evaluation<T>` distinguishes Ready(T), Missing, Warmup and Error(NumericError). Ready(zero), Missing and Warmup are distinct; absent optional bindings are a separate BoundValue::Unbound. P1-05 now implements separate [expression/readiness and crossing-step staging](EXPRESSION_CONTRACT.md); this enum alone does not implement it. Whole-model event transactions remain P1-08.

## Parameter definition and binding

`PARAMETER_SCHEMA_VERSION=1` independently versions this semantic schema. A ParameterSpec owns key, descriptor, required flag, optional typed default, numeric bounds with inclusivity, optional finite typed choice set and description. Definition construction validates constraints and defaults before accepting bindings. Bound values are immutable for a run; edits create another binding object.

- Keys are 1–128 ASCII letters/digits or `._-`; descriptions are at most 4096 UTF-8 bytes. All parameters in this subset are immutable; no runtime-mutability option is supported.
- Numeric bounds require the exact parameter descriptor and numeric values. Reversed/empty bounds, including empty discrete open intervals and excluded extrema, fail. Bool/Text do not support ordering bounds. All listed enum choices must be correctly typed, unique, within bounds and present in a nonempty set of at most 256 entries. The default must satisfy the entire schema even when a caller intends to override it.
- Required+default is invalid. Missing required input fails; missing optional with default binds a copy; missing optional without default stays Unbound. A supplied valid value overrides the default. Unknown keys and duplicate schema/binding keys fail. Unknown key lookup returns None, separately from Some(Unbound).
- Inputs distinguish explicit Null from Evaluation readiness. Null, Missing, Warmup and Error are rejected as parameter inputs for every supported type; omitted parameters alone follow the default/required rules. Type/unit/scale mismatches fail without conversion.
- A schema accepts at most 256 parameters, and bind inspects at most 257 supplied entries before rejecting over-limit input. BTreeMap provides stable key order after duplicate detection. The supplied input is a sequence, so duplicates are detected before any map conversion can discard them. Binding is all-or-nothing and does not mutate definitions or prior bindings.

The full supported-type binding matrix and unit/range/choice failures are exercised through the public API in [parameter tests](../crates/fqe-ir/tests/parameter_contract.rs). Cross-parameter function rules such as fast < slow are not interpreted as executable text; P1-04/P1-06 own versioned function validators. Parameter description/key strings are never executed. P1-03 enforces separate parse-level byte/nesting/collection bounds and detects wire duplicates before constructing a map; see [Model loading](MODEL_LOADING.md) for import evidence and limits.

## Runnable scope and next integration

Run the existing pinned toolchain offline:

```sh
cargo test --locked --offline -p fqe-core --test value_contract
cargo test --locked --offline -p fqe-ir --test parameter_contract
cargo run --locked --offline -p fqe-ir --example parameters
```

The example binds period=2 from the default and period=5 from an override, then rejects period=0 against [1,200]. It does not run a strategy. P1-03 consumes these APIs in its bounded external loader and explicitly distinguishes IR2 from legacy IR1. P1-02 alone supplies no codec; the P1-03 contract now owns serialization. Neither slice provides automatic legacy conversion or source-language execution.
