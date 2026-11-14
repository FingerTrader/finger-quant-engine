# Typed Expressions and Crossing History

**Implemented, experimental P1-05 subset.** [expressions.rs](../crates/fqe-ir/src/expressions.rs) owns bounded in-memory expression plans, pure registered calls, Boolean/readiness semantics and transactional crossing subscriptions. [Types](TYPE_PARAMETER_CONTRACT.md) owns scalar precision; [Provider registry](PROVIDER_REGISTRY.md) owns call/parameter/output validation; [P1 design](P1_STRATEGY_DESIGN.md) owns the broader graph/event target. [Status](PROJECT_STATUS.md) owns progress.

## Immutable plans and checked types

ExpressionDraft supplies typed named inputs, independently named crossing subscriptions and named root expressions. ExpressionPlan validates these declarations and derives output types; it exposes shared read-only declarations. Each ExpressionInstance owns separate history/cursor/counters while sharing an immutable plan. No event bus, model-graph/module compiler, dataset reader, indicator formula, financial ledger or language worker is added.

Supported expressions: typed literal, declared input reference, named subscribed-crossing result, Boolean not, binary arithmetic/comparison/Boolean operators and a registered PreparedCall with named argument expressions and selected output. Calls are generic: descriptor types, bound parameter/implementation pins and P1-04 validation apply without a business-ID branch. Multi-output provider results are validated completely before the selected output is used. Pure expression calls may be repeated when independently written in several AST positions; subscribed crossing references reuse one cached result per accepted step.

All operands require identical descriptors, including units/scales/text capacity. No truthiness, integer/float conversion or implicit rescale. Comparison equality/inequality supports every implemented scalar. Ordered comparisons support only numeric scalars. Add/subtract preserve identical numeric types/units; signed/unsigned integers use checked arithmetic. Integer multiplication/division require explicit Exact policy; division rejects nonzero remainder, zero divisor and overflow including i64::MIN/-1. Decimal multiply/divide use explicit Exact/TowardZero/HalfEven and P1-02's checked intermediate/scale policy.

Decimal multiplication/division currently accept identical dimensionless Scalar or Ratio descriptors only. Price/Quantity/Money products/quotients, mixed ratio/domain operands and differing instrument/currency/scale are rejected before evaluation. Such dimensional conversions need explicit separately validated functions/policies; the engine does not invent a money currency from price×quantity or call a price quotient another price. This bounded policy does not settle indicator working precision (P1-06) or ledger/tick/lot/fee rounding (P3-01).

Plan bounds: 0–32 inputs, 1–32 root outputs, 0–256 crossings, at most 4096 AST visits across all roots and crossing operands, root depth 0 through 32, at most 262144 aggregate literal-text UTF-8 bytes. IDs follow model logical-ID limits. Unknown/duplicate references, input/output/cross IDs, duplicate/missing/extra call arguments, wrong argument/output types and cyclic crossing subscriptions fail construction. Rust callers construct the AST; no new external expression decoder/codec is claimed. P1-03 model syntax is unchanged; P1-07 must implement mapping/compilation rather than treat this AST as an import format.

## Boolean truth and evaluation order

T/F = ready true/false, M = Missing, W = Warmup, E = numeric error returned as ModelError with no committed result. Both tables use left operand as row and right as column. Left Error fails before evaluating the right. Only ready F on the left of AND and ready T on the left of OR skip the right; type validation still checks all branches when building the plan.

| AND | F | T | M | W | E |
| --- | --- | --- | --- | --- | --- |
| F | F | F | F | F | F |
| T | F | T | M | W | E |
| M | F | M | M | M | E |
| W | F | W | M | W | E |
| E | E | E | E | E | E |

| OR | F | T | M | W | E |
| --- | --- | --- | --- | --- | --- |
| F | F | T | M | W | E |
| T | T | T | T | T | T |
| M | M | T | M | M | E |
| W | W | T | M | W | E |
| E | E | E | E | E | E |

NOT swaps ready Boolean values and preserves Missing/Warmup. Numeric Error fails. Arithmetic/ordinary comparisons evaluate left then right, fail on an evaluated numeric Error and otherwise propagate Missing ahead of Warmup. They do not fabricate zero/false. A decisive right Boolean can establish false/true from a non-ready left as shown; it cannot erase an already evaluated Error. Pure arithmetic/calls in a skipped branch do not execute.

## Subscribed crossing phase and atomic history

Before evaluating any root Boolean expression, evaluate every declared crossing subscription once in deterministic dependency order. A subscription is active even when unused by a root or behind a false guard. Crossing operands can use pure registered calls and other crossing results; dependencies are validated and topologically ordered, choosing the lexically first ready ID. This is local expression subscription ordering, not the whole-model graph compiler. Current-step dependency outputs are cached; prior comparison pairs come from the committed pre-step history.

Cross-above = previous ready left≤right and current ready left>right; cross-below = previous ready left≥right and current ready left<right. The first ready pair returns ready false and establishes history. Repeated ready values do not recross. An accepted Missing/Warmup operand propagates its readiness and resets that subscription's prior pair; the next ready pair returns false. Numeric Error aborts the whole step, even if a later Boolean guard would hide it. Reset happens only on a successful overall step.

History writes are staged. Evaluate subscriptions, then all named root expressions in lexical name order; commit history, accepted-step count and last logical sequence only when every output succeeds. Any error returns no partial result and retains prior committed history/cursor. Invalid type/arity/reference input and zero/duplicate/backward sequences are rejected without consuming a step. Retrying a failed sequence is allowed. Successful sequences increase strictly; sequence gaps alone do not imply a missing market sample and do not reset history. P1-08/P2 own market-domain/gap/alignment semantics; a caller must supply explicit readiness for a known missing sample.

ExpressionInstance uses a caller-chosen fixed run/instance ID and positive logical sequences. Pure provider calls receive deterministic per-step call sequences after argument evaluation. Failed retries can reissue pure calls; no exactly-once external-side-effect guarantee is implied. Repeated provider AST positions are not a shared stateful node. P1-06 adds owned indicator steps; compiled stateful subscriptions, generalized event transactions and snapshots remain P1-07/P1-08/P2. These in-memory crossing histories cannot be exported/restored and are not crash recovery.

A step's Ready input descriptors are validated before branch selection, including unused declared inputs. Runtime Error readiness is observed only if that input is evaluated or consumed by a subscribed crossing. Pure providers remain trusted under P1-04; local staging cannot undo forbidden provider side effects, process abort, allocation failure or a loop.

## Evidence and integration boundaries

[Expression tests](../crates/fqe-ir/tests/expression_contract.rs) independently enumerate both five-state Boolean tables and verify exact integer/decimal results/rounding, numeric failures, comparison edges, incompatible dimensions, generic independent function/predicate composition, false guards, shared references, reordered ready Boolean operands, crossing dependency/cycle behavior, Missing/Warmup reset, sequence gaps/retry, instance isolation and failed-root/subscription rollback. Input, depth/node/text/collection bounds have negative cases. Existing IR1/runtime behavior remains separate.

Run `cargo run --locked --offline -p fqe-ir --example expressions`: left 1/3/4/2/3 against right 2 and guards false/false/true/true/true produces false/false/false/false/true. The first hidden cross is consumed during the second step rather than emitted late when the guard becomes true. All five steps are accepted.

No complete model compiler, runnable loaded indicator strategy, serialized expression schema, MACD/RSI/SMA/EMA, CLI model execution or Flutter integration follows from this slice. Those retain P1-06–P1-11/P4 acceptance. ADR-016 in [ADRs](ARCHITECTURE_DECISIONS.md) records this scoped semantic choice.

P1-06 separately implements [registered indicators](INDICATOR_CONTRACT.md) with owned incremental instances. They are not pure Expr::Call entries; exposing them to a complete compiled event plan still requires P1-07/P1-08. The slice limitations above describe P1-05 itself, not the newer indicator library.
