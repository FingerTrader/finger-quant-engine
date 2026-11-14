# Open Language, Indicator and Rule Extensibility

Status: **normative target with an implemented experimental pure scalar subset** in [Provider registry](PROVIDER_REGISTRY.md); remaining adapter/state/plugin mechanisms are planned. Accepted direction under [ADR-012](ARCHITECTURE_DECISIONS.md#adr-012--open-language-and-computation-extensions). P1-04 implements shared in-process descriptors/registration/invocation; no external adapter/provider bridge or extension loader is implemented. [Project status](PROJECT_STATUS.md) owns progress; [Delivery plan](DELIVERY_PLAN.md) assigns executable acceptance. Existing experimental Rust IR remains unchanged.

## Goal and compatibility boundary

FQE supports an open-ended set of language adapters, indicators, functions and rules. Python, Java and Go are the initial reference languages; SMA/EMA/MACD/RSI are initial reference functions. These are neither an exhaustive catalog nor special cases in the compiler. Within supported types, events and capabilities, a new conforming adapter/provider/module must be usable without editing or rebuilding engine core, changing IR enums for its name, or adding a dedicated Desk business page. The extension itself may need compilation and its own runtime/package.

New languages require an adapter and actual conformance evidence. New indicators/rules require definitions or executable providers and validated data requirements; listing a name is not implementation. A genuinely new value shape, event family, host service or isolation mechanism may require a versioned contract/engine change. Unsupported required features fail before execution with a stable diagnostic, rather than being accepted as opaque data or converted silently. No promise of lossless arbitrary-code-to-graph conversion, unlimited resources or identical arbitrary programs across languages is made.

## Three entry roles

| Role | Public boundary / owner | Required independence |
| --- | --- | --- |
| Engine client | Versioned validate/compile/run/control/result interface; P1-10/P4-01/P4-02 | Any program able to implement the chosen boundary can invoke a model; doing so does not make that language a supported strategy runtime. |
| Strategy runner | Versioned language adapter and worker lifecycle; P5-01–P5-05 | Runs an entire code strategy, uses declared host context services and emits validated observations/intents. No Python/JVM objects or Rust layouts in the public contract. |
| Computation provider | Versioned typed function/indicator/predicate invocation; P1-04 contract, P2 state, P5 protocol mapping and P6 external execution | Exposes callable entries to the graph and other strategies through a host function service. Its implementation language is independent of the caller's language; it does not own orders or financial truth. |

A package can implement several roles, but each role is negotiated explicitly with separate permissions/lifecycle. A whole-strategy `on_bar` callback is not an indicator invocation API. Worker support for a language alone does not prove that its functions can be graph nodes. [Language workers](LANGUAGE_WORKERS.md) owns the strategy-runner protocol; [Plugins](PLUGIN_SYSTEM.md) owns external artifact resolution/lifecycle/isolation. Both implement this shared semantic boundary.

## Shared descriptors and typed data

Descriptors are versioned data that engine validation, external invocation and Desk inspect through the same discovery API. Required fields are:

- Namespaced stable entry ID, semantic contract version, implementation/artifact content identity and compatible engine/IR/provider-contract range. Resolve exact versions before a run; no hidden upgrade of running definitions or registry contents.
- Role/category, named input/output ports, cardinality, types, units/scales, instrument/timeframe domains, null/readiness handling and required capabilities. Descriptors express supported scalar, record/multiple-output, series/window or bounded collection shapes; unsupported shapes are rejected. P1-02 implements only its evaluated subset and a way to identify unsupported required shapes.
- Parameter schema, defaults/ranges/enums, cross-parameter validation and immutable bound values, using the [P1 binding rules](P1_STRATEGY_DESIGN.md#universal-type-system-and-parameters--p1-02). No parameter name or description is executable code.
- Data requirements: selected OHLCV fields, instrument/timeframe bindings, history lookback, warm-up/readiness, aligned input dependencies and any external dataset/factor schema/content identity. P2/P3 own availability and time alignment; a provider cannot fetch future data or silently download missing inputs.
- Purity/statefulness, incremental/batch capabilities, state-schema version, initialization/reset/step/snapshot/restore/dispose support and explicit unsupported lifecycle operations. Formula/seed/rounding/missing rules and determinism class are versioned semantics.
- Resource bounds and host-service permissions, dependency closure, diagnostics/source mapping and optional display metadata: label/description/localization keys, parameter hints and output/plot labels. Display metadata grants no execution permission and contains no executable UI.

P1-03 defines bounded model/reference serialization; P1-04 defines canonical callable descriptor/registry identities and validates/exposes in-process lookup; P1-07 compiles generic signatures, not indicator-specific cases. P4-02 exposes discovery to clients. P6-01 resolves external artifacts. Descriptor compatibility is independent of the implementation language and package release number.

## Provider invocation, state and execution

The semantic invocation identifies run/instance, pinned entry/version, bound parameters, logical event/call identity, typed visible input values, prior provider-state identity and granted capabilities. The response carries named typed values/readiness, candidate state and bounded diagnostics. The selected P5/P6 encoding/transport must preserve this contract; this document does not freeze method names or choose FFI/IPC/WASM/native ABI.

The host creates and owns provider instance identities. No mutable indicator buffers are shared between strategy instances implicitly. Indicator/predicate providers have read-only declared data access; no order submission, account mutation or undeclared filesystem/network permission. A custom predicate returns a typed Boolean/readiness value, not an order. A whole-strategy runner can request explicitly granted host operations, which remain validated and committed by FQE.

Subscribed stateful graph nodes advance exactly once per matching accepted event, independently of Boolean short-circuiting; shared references reuse that event's output. Responses are correlated with run/entry/instance/event/call identities. Reject duplicate, stale or out-of-order responses before commit. Provider failure, wrong output types/units, timeout or non-finite/overflow result follows the declared event-failure policy and cannot commit partial pure-graph state/signals. See [P1 event evaluation](P1_STRATEGY_DESIGN.md#expressions-and-indicator-functions--p1-05--p1-06).

State is portable, versioned data validated by the host, not a pointer, language object or implicit pickle/Java serialization. Snapshots bind provider implementation, entry/state schema, parameters and input cursor. P2-04 verifies in-process state continuity; P5/P6 repeat it for external runtimes. A provider without snapshot support cannot be used in a run requiring restore. Legacy hosted context operations already durably committed survive later callback failure under their versioned policy; pure-graph rollback must not undo or repeat them.

Incremental and batch/window evaluation, where both are declared, must produce the same ready outputs under the same formula/numeric policy and visible data prefix. Batches cannot expose future samples to an earlier decision. P5/P6 measure call overhead, serialization cost, memory and scheduling budgets with many nodes and multiple instances; a per-node subprocess or unbounded cross-runtime call cycle cannot be assumed acceptable. Nested provider dependencies are declared and bounded; recursive provider/service cycles are rejected or explicitly unsupported. Choice of batching/caching is evidence-gated and cannot change semantics.

## Rules and reusable composition

| Rule form | Meaning | Implementation / acceptance |
| --- | --- | --- |
| Declarative condition | Typed arithmetic/comparison/Boolean and registered predicate calls | P1-05/P1-07; a new formula composed from supported functions needs no new engine condition enum. |
| Reusable rule module | Parameterized graph with typed public ports, dependency closure and namespaced state | P1-07 pure modules; P2-01/P2-04 stateful modules; recursion/conflicting writes and incompatible dependencies fail validation. |
| Code-defined predicate/function | External provider entry consumed by graph guards or other functions | P5 protocol mapping and P6-03.01 actual foreign-language graph invocation; retains typed results, permissions and state boundaries. |
| Imperative strategy | Whole-strategy program using the runner contract | P5 language gates; may be a single opaque strategy node to the editor, without a promise to recover its internal visual graph. |

Temporal rules use the host scheduler, explicit State/Delay and declared event-time windows; providers do not redefine event ordering or wall-clock replay. Composed rules produce observations or explicit validated intents. Plugins cannot bypass trading permissions or introduce a second ledger. Novel host-side risk/execution models need their own versioned extension contract and financial gates; arbitrary user rules are not privileged engine policies.

## Language adapter onboarding

P5-01 delivers an adapter manifest/registry and an onboarding kit. A manifest identifies language/runtime constraints, adapter/artifact identity, entrypoint, roles, supported contract/capability versions and resource/supervision requirements. It contains bounded structured launch data, never an arbitrary shell fragment. The host resolves a reviewed local pinned artifact; registration does not install dependencies, download runtimes or execute discovery-time code.

The kit includes a normative protocol/state-machine specification, reference adapter, synthetic golden request/result fixtures, exact-value conversion examples, lifecycle/error/cleanup tests, a language-independent conformance runner and packaging instructions. The conformance runner's own development tooling is distinct from runtime dependencies: using a Python test tool must not make the Java/Go production path require Python. Missing runtimes/roles produce explicit preflight failures. Native launches use a generic supervisor plus declared platform capabilities; a new runtime requiring a new host primitive goes through contract evolution.

Python → Java → Go remains the reference delivery sequence, not a runtime dependency chain. Java and Go each pass with the other reference language runtimes unavailable to the launched strategy. P5-05.01 adds a real adapter implemented in a fourth language absent from the reference language set. Select it against available reviewed tooling during that task; new tool installation follows supply-chain policy. A renamed adapter backed by a reference-language process is not acceptable proof.

For the fourth-language test, freeze an engine binary/hash first, then register the adapter and run a real strategy using the existing public protocol, with output/state/control and fault checks. Record engine core source/binary unchanged, independent adapter source/artifact and complete conformance results. This proves the mechanism for that added language, not that all future languages have already been implemented or bundled.

## Desk discovery and customer support

P4-02/P4-03/P4-05 deliver generic discovery, parameter/port presentation, validation and output/diagnostic rendering using built-in and reviewed test descriptors. They do not require P5/P6 external execution. Adding an entry within these supported descriptor types must not require a new hardcoded Desk panel. Unsupported editors, types or runtime capabilities are shown accurately before a run; editing and saved instances preserve exact entry/artifact identity.

P5-02/P5-03/P5-04 each include actual Desk language selection/configuration, run/stop and historical-result reopening for the supported platform/runtime combination. P6-03.01 supplies a new foreign-language indicator and predicate to the same editor/graph; P6-06 repeats that journey with the final installed package. [Desk integration](DESK_INTEGRATION_PLAN.md) owns the existing workbench, migration and native evidence.

Engine-compatible adapters, CLI-supported runtime combinations and customer-bundled Desk capabilities have separate recorded matrices. A language can be conforming without being available in the signed Desk distribution. A platform/store restriction cannot be hidden by claiming engine conformance; unsupported distribution is reported honestly and required product acceptance remains open. Do not bypass signing/Sandbox or download/execute arbitrary code to satisfy a demo.

## Acceptance evidence and examples

| Evidence ID | Owning tasks | Required proof |
| --- | --- | --- |
| X-001 Unknown-name provider | P1-04/P1-07/P1-11 | Independently defined reviewed test descriptor/function injected through the public provider interface; core/compiler contain no branch for its ID. Compile/run two compositions, including a typed Boolean output, and reject bad signatures/units/versions. This is an in-process test provider, not a third-party loader claim. |
| X-002 Stateful extension | P2-01/P2-04/P2-06 | Parameterized stateful rule/indicator, multiple isolated instances, split at each event and compare uninterrupted output/state. Failure leaves prior committed state and history unchanged. |
| X-003 Reference languages | P5-02/P5-03/P5-04/P5-05 | Actual Python/JVM/Go programs pass shared exact-value, lifecycle/state/intent/error fixtures and each supported Desk journey; Java/Go need no Python process. No cross-language arbitrary-code determinism claim. |
| X-004 Unlisted adapter | P5-05.01/P5-05 | Real fourth-language adapter added after the engine binary is frozen, unchanged core/binary, complete onboarding/conformance and failure/cleanup evidence. |
| X-005 Cross-language graph | P6-03.01/P6-03/P6-06 | Java implements a rolling indicator with named value/readiness outputs; Go implements a new predicate. Their external packages combine with built-ins in one portable graph and reusable rule, loaded offline. Verify typed data, state restore, failures, lifecycle and independent expected decisions, and actual Desk discovery/use. |
| X-006 Final package | P6-04/P6-05/P6-06 | Final installed Desk uses the declared bundled adapters/providers and preserves supported old artifacts/results; publish measured resource budgets and the exact engine/adapter/provider/platform matrix. |

X-005's tracked synthetic reference: closes 1/2/3/4/5/6; Java `example.rolling_delta(window=2)` yields current close minus close two accepted bars earlier: Warmup/Warmup/2/2/2/2. Go `example.greater_than` compares ready delta to a typed compatible threshold. Threshold 1 produces ready true at bars 3–6, threshold 2 produces none; missing readiness never becomes false zero. Integrate a built-in readiness-compatible Boolean expression and a reusable rule module. Invalid units/version, wrong output shape, duplicated responses, crash at an event and split-run restore are mandatory negatives. IDs are fixture examples, not compiler special cases. A separate multi-input/multi-output fixture covers typed port binding; additional shapes remain capability-gated.

Exact integers, financial values, units, event ordering and host accounting follow shared contracts. Numeric algorithm parity applies to entries implementing the same versioned formula/seed/precision policy, with exact comparisons or justified output tolerances; strategy decisions need explicit boundary tests. Vendor libraries with different conventions are versioned implementations, not automatically equivalent because they use the same indicator name.

All acceptance reports record frozen engine/Desk commits, adapter/provider artifact hashes, descriptor/schema/formula versions, runtime/platform/toolchain, fixtures, actual commands/results and limitations. Adding an extension is successful only when generic validation and execution work; listing/describing it, compiling a stub or passing one happy-path example cannot close the relevant gate.
