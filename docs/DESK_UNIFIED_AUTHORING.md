# Unified Desk authoring and research integration

Owner-approved scope, 2026-11-15; ADR-036. This contract extends [Desk integration](DESK_INTEGRATION_PLAN.md), [managed controls](ENGINE_CONTROLS.md) and the [delivery plan](DELIVERY_PLAN.md). It does not redefine Event/Workflow, provider types or financial rules. The external consumer design is `finger_desk/docs/quant/desk-fqe-product-integration-design.md`, discussion v1; this repository owns its actual native implementation. P4-03 remains IN_PROGRESS until all seven children and the actual native journey close.

## Ownership and identity

One semantic document supplies visual and structured-text views. Stable project ID is independent of name; copying/importing creates a new local ID with provenance. Stable semantic node/parameter/module IDs supply source locations. Presentation (coordinates, selection, folding, file tabs) and comment-bearing invalid buffers are separate from executable artifacts. Execution derives a fixed Event or Workflow, exact parameter bindings and ordered explicit trading policies, never a second independently editable model. Native canonical/execution/registry/artifact identities are preserved in a versioned outer association with project/revision/data/run/trial identities.

The first-party native authoring frontend owns bounded parsing, printing and lowering. Desk owns user buffers, conflict decisions, undo and saved projects. Text is applied atomically only against the expected semantic revision. Invalid/unapplied text is saved without replacing the last valid semantic snapshot and prevents execution; an explicit resolution is required before graph/parameter edits can overwrite text. Comments must survive text round-trips. Layout-only edits cannot change executable semantic identity. Formatter/generated text must not silently erase authored comments.

## Contextual reference discovery (P4-03.01)

Add optional capability `catalog.describe/1`, method `catalog.describe`, exact params `{trigger: ...}`. Trigger uses the existing Event trigger shape: `tick` with instrument, or `closed_bar` with instrument, exact string `timeframe_ns`, integer price_scale/volume_scale. Unknown/missing fields, noncanonical numeric strings and unsupported domains reject. No model, parameter binding, dummy execution or provider invocation is needed. Reference registration depends on this domain; this is not external package discovery.

Response contains `format=fqe.catalog-description`, integer `schema_version=1`, `registry_id`, original normalized `trigger`, full native `descriptors`, and `support`: executable artifact kinds, financial profiles, explicit unsupported authoring/optimization/external-language/temporal-scheduler flags, and exact string limits. These flags describe this peer, not every library feature. Output descriptors remain native canonical bytes decoded as JSON, including artifact pins, units, ports, parameter constraints, readiness and resource semantics. Registry creation performs no provider state creation/evaluation. Existing `descriptor.list` continues requiring model/bindings; old methods are unchanged.

Limits use actual implementation constants wherever public. Request262144 bytes, response4MiB, financial source bars256 including warmup, graph nodes256, expanded nodes256, event slots32, model bytes262144, pure module nesting8. Managed jobs4/active1/operation receipts64 remain process-session quotas. Advertising a library limit does not grant that capability over the desktop boundary. Input/report/retained-store limits intersect; no implicit truncation, split-run financial stitching or runtime download.

## Initial structured language (P4-03.02)

The implemented experimental `authoring.event/1` source begins `fqe 1;` and has a deliberately bounded grammar: strategy/context, exact typed parameters, field bindings, generic descriptor ID/version calls, named ports, comparisons/Boolean operators and crossings, outputs and ordered policies. No indicator-name dispatch. The initial Event subset precedes Workflow/module syntax in P4-03.04. Unsupported expressions/units/state/order kinds fail with source-linked diagnostics. Native validation is authoritative; successful parsing alone never means runnable. Main-flow language is not Python; arbitrary Python/Java/Go files are legacy attachments or future explicitly supported language modules.

Native frontend interface must provide parse/inspect/print with a semantic representation and byte/line/column spans. Imported raw models must either be fully representable in the negotiated subset or reject printing; never lose unsupported modules/state/metadata. Round-trip tests compare canonical native semantics and actual output, not only source strings. Revision conflict/invalid buffer tests prove stale models cannot start.

## Project-to-run and replay (P4-03.03)

Preserve Desk's five stages, workspace/panel IDs, top/bottom navigation and FullDesktopDialog. Application-level task/storage owns execution; views only subscribe. Project-scoped settings replace the single global editor draft. Save/validate/confirm/start pin the exact project revision, parameter bindings, frozen history/range/warmup, engine, policy and cost configuration. Any change invalidates pending approval; late asynchronous results cannot replace a new project/run selection. Existing Python backends retain explicit identity, never automatic fallback.

Reports and immutable bytes drive result/replay views. Read cursors must specify source sequence/time, open-fill versus close-strategy phase and terminal cleanup. Use native journal-after account state, execution receipts and close equity; do not implement a second client ledger. Playback does not execute strategies. Unsupported metrics display unavailable. Cancelled prefixes and absent/damaged/unknown results are distinct. Actual offline/no-login, two variants/projects, current bundled helper, stop, close/reopen/history and active-quit cleanup require native evidence.

## State, optimization, exchange and scale

P4-03.04 exposes only actual supported modules/Workflow. G-006 means arm at k, observe k+1 through k+3 inclusive, latch qualifying RSI, decide at k+3, ignore re-arms while active including expiry; late/arming-only/EOF cases remain authoritative. General temporal scheduler/multimarket financial support is not inferred.

P4-03.05 adds optional native optimizer control and report/trial reads. Existing grid limits8 dimensions/16 choices each/16 total trials/4096 source budget, training-only profit/drawdown/content-ID ranking and fresh winner validation are unchanged. No validation reselection, hidden parallel trials, fabricated progress or fixed sample rankings. Session lifecycle/cancel/resource/idempotence and real UI evidence are required.

P4-03.06 versions the new project package separately from `.fstrategy` v1. Preserve originals, ask the migration flow to choose independent legacy graph/Python sources, reject unsupported mapping, retain old report backend/accounting. Import/download never executes code, installs dependencies or grants account access. Marketplace remains a separately contracted service.

P4-03.07 establishes a new scalable profile before claiming ordinary long-history research: at least10000 continuous source bars, measured memory/time/report/storage budgets, native continuity and cancellation, exact small-oracle parity. Existing256-bar profile remains available with its exact limit; no client splitting and concatenating separate financial results. A new engine identity/profile and paired consumer bundle require verification. These are acceptance targets, not implemented limits.

## Independent acceptance anchors

- Proposed SMA2/3 closes3,2,1,2,3,2,1,2: upward crossing5, downward7; next opens4 at6 and2 at8, one unit, cash100/no costs gives final98. Implemented under P4-03.02: independent graph/text canonical parity and actual native execution pass; the complete cash/equity curve is independently checked.
- Existing [G-006 independent goldens](P2_ACCEPTANCE.md) cover early/expiry/late/re-cross/EOF and instance isolation.
- Existing [optimization oracle](OPTIMIZATION.md) chooses threshold90 from training profits−2.50/−3.00/0.00; separate validation−11.00/equity89 must not reselect threshold0 even though its validation is better.

## Verification and delivery

Run all required Rust/docs/diff checks and the full financial gate after native edits. No new dependencies for initial discovery/frontend; reuse previously reviewed locked libraries/offline toolchain. Preserve historical evidence and consumer pins until replacement builds are verified. This contract and its accepted direction do not establish any child completion, Desk UI acceptance or GitHub publication.

## Experimental Event frontend reference (P4-03.02)

Source: first-party `fqe_bridge::authoring`, no parser dependency or financial-source edits. Capability `authoring.event/1` adds pure `authoring.inspect {source,parameters}` and `authoring.print {document,comments}`. Exact outer fields are mandatory. Inspect returns `valid`, diagnostics, and only on success document/source_map/comments/native inspection. Invalid source returns no executable/native field. Native parameter failures return diagnostics; parsing/inspection never starts a job. Print returns valid/source or diagnostics and rejects any lossy conversion by canonical native-model and exact ordered-policy round-trip comparison. Existing raw native models remain available independently.

Use [the complete sample](../examples/desktop/sma-cross.fqe). Grammar in source order:

```text
fqe 1;
strategy "stable.semantic.id" version "1";
context closed_bar "DEMO" timeframe "60000000000" price_scale 2 volume_scale 3;
warmup "0";
param fast_period : u64 = "2" within ("1", "10000");
input sample = close;
node fast = "reference.sma" @ "1" (period=$fast_period) (close=sample);
```

Tick context is `context tick "DEMO";`. Types are u64/bool/decimal(scale,ratio), decimal(scale,price,"instrument"), decimal(scale,quantity,"instrument"), decimal(scale,money,"currency"). Parameters have explicit defaults, optional inclusive `within(lower,upper)` and optional `description "text"`. Full-width integer/decimal defaults and node literal arguments are quoted exact strings; Boolean literals are unquoted. Required/absent parameters, enum/exclusive/one-sided constraints, arbitrary expression literals, rounded multiply/divide and unsupported types are not silently translated; they remain outside this authoring subset. Native model APIs retain their broader contracts.

Nodes use provider ID/version and contextual descriptor-supplied types/pins. First parentheses bind descriptor parameters from exact literals or `$parameter`; second parentheses bind input ports to an earlier node.port or named field input. Every native output port is exported under node.port. Local declaration names are dot-free ASCII identifiers; provider/strategy IDs remain quoted/native validated. There are no SMA/RSI/MACD-specific execution branches. Cross declarations are `cross up = above(fast.value, slow.value);` or below. Outputs use input/node.port, `$parameter`, `cross(id)`, `not(expr)` and two-argument add/subtract/multiply/divide/equal/not_equal/less/less_equal/greater/greater_equal/and/or. Expressions are typed by the existing native compiler; default numeric operation rounding is exact. Policies in declaration order are `policy entry = buy_if_flat(output, "quantity", "cash_cap");` and `policy exit = close_long(output);`; their roots must be Boolean. Monetary/quantity scale and step validation belongs to financial preflight.

Bound source65536 UTF-8 bytes,8192 tokens,4096-byte strings, expression nesting16 intersect existing native source/JSON/graph/slot bounds. Unknown syntax, versions, field ports, providers, arguments, duplicate declarations, cycles/forward wires, type/unit mismatches and unrepresentable print imports fail. Diagnostics include code, native path, byte offset and 1-based Unicode line/column in strategy.fqe; source maps retain stable declaration IDs and spans. A native diagnostic without a matching declaration keeps its original path and start-of-file location rather than inventing a node.

`//` comments are preserved as separate ordered strings. Printing currently moves them to a leading comment block, retaining content/order; it does not preserve original whitespace or inline placement. Original source remains in the draft; comment anchoring and richer CST editing are future refinements, not claimed current behavior. Graph presentation stays in the Desk outer project and is never lowered into financial metadata.

The reference `Draft` transaction API tracks semantic revision and independent text generation, retains invalid buffers, refuses executable access or graph edits while text is unapplied, rejects stale responses, and requires explicit apply/discard. Source-only snapshots restore the valid document by native parsing without execution and retain pending invalid text. Semantic revisions do not change for comment-only edits; text generation still prevents stale application. Desk must implement the same rules in its project service in child03; this Rust reference alone is not actual Flutter integration.

Nine targeted Rust tests and an independent actual-peer consumer cover graph/text native identity, economic output parity with the separate graph fixture, exact full u64/decimal/Boolean constraints, comment retention, unsupported/lossy rejection, source-linked native errors, stale/invalid/persisted draft transactions. The first Python peer harness attempt closed stdin immediately and correctly cancelled the asynchronous backtest; the corrected harness keeps stdin open through the final response and observes clean EOF afterward. This was a harness lifecycle error, not hidden as a passing run. Complete current workspace gate and evidence commit are required before child closure.

The reusable Rust `parse_with_registry` accepts an already reviewed caller-owned registry; it never loads packages. An independent gate-only shift/window registry proves unknown provider names, parameter names, ports and output types compile without frontend changes or provider evaluation. Desktop authoring RPCs still expose only their contextual reference registry. Decimal source literals use exact human notation and are converted to signed integer coefficients at the declared scale; printing reverses this without floating point or rounding. A fresh regression initially exposed this conversion defect in the new decimal test; it was fixed before closure, with an independent coefficient assertion.
