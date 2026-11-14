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

The experimental source format will have an explicit version and a deliberately bounded grammar: strategy/context, exact typed parameters, field bindings, generic descriptor ID/version calls, named ports, comparisons/Boolean operators and crossings, outputs and ordered policies. No indicator-name dispatch. The initial Event subset precedes Workflow/module syntax in P4-03.04. Unsupported expressions/units/state/order kinds fail with source-linked diagnostics. Native validation is authoritative; successful parsing alone never means runnable. Main-flow language is not Python; arbitrary Python/Java/Go files are legacy attachments or future explicitly supported language modules.

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

- Proposed SMA2/3 closes3,2,1,2,3,2,1,2: upward crossing5, downward7; next opens4 at6 and2 at8, one unit, cash100/no costs gives final98. This proposal needs actual native execution plus graph/text parity; it is not an existing passing test.
- Existing [G-006 independent goldens](P2_ACCEPTANCE.md) cover early/expiry/late/re-cross/EOF and instance isolation.
- Existing [optimization oracle](OPTIMIZATION.md) chooses threshold90 from training profits−2.50/−3.00/0.00; separate validation−11.00/equity89 must not reselect threshold0 even though its validation is better.

## Verification and delivery

Run all required Rust/docs/diff checks and the full financial gate after native edits. No new dependencies for initial discovery/frontend; reuse previously reviewed locked libraries/offline toolchain. Preserve historical evidence and consumer pins until replacement builds are verified. This contract and its accepted direction do not establish any child completion, Desk UI acceptance or GitHub publication.
