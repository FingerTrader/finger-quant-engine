# Pure Provider Registry Contract

**Implemented, experimental P1-04 subset.** This owns current descriptor validation, frozen in-process registries, pure callable preparation/invocation and local model-node signature checks. The broader [Extensibility contract](EXTENSIBILITY_CONTRACT.md) remains normative for later languages/state/plugins; [Model loading](MODEL_LOADING.md) owns IR2 file syntax and model identity. [Project status](PROJECT_STATUS.md) owns progress.

## Descriptors and frozen registries

[registry.rs](../crates/fqe-ir/src/registry.rs) defines DescriptorDraft → immutable Descriptor, RegistryBuilder → immutable Registry, and exact-pinned PreparedCall → CallResult. Callers can inspect/clone declarations but cannot mutate registered descriptors or an existing frozen registry through these APIs. Discovery and registration never call provider code. Individual failed registration leaves existing entries intact. Registration of a group through a helper is sequential; callers discard the builder if a batch fails.

Descriptors identify a namespaced logical ID, opaque author contract version, descriptor schema 1, provider contract 1, engine API 1 and inclusive IR compatibility range containing current IR2 (positive bounds). Engine API is independent of package version. Exact ID/version is resolved; no latest-version fallback or silent artifact replacement. P1-03 artifact pins must match the registered implementation artifact. Unknown IDs/versions, role/schema mismatches and changed artifact pins fail.

Descriptors contain category, named typed input/output signatures, parameter schema, explicit purity/readiness/determinism/history/state/capability/service/dependency declarations, scalar-payload resource limits, versioned formula/precision/missing semantics and bounded display metadata. Current accepted subset:

| Field | Implemented rule |
| --- | --- |
| Role/category | provider; function or predicate; predicates have exactly one Boolean output |
| Ports | 0–32 named inputs, 1–32 outputs; exactly-one cardinality; P1-02 scalar types/units/scales; named multi-output values are separate scalar ports, not an opaque record |
| Parameters | P1-02 schema/default/range/enum/type rules, plus provider relationship validator at preparation; immutable values thereafter |
| Purity/readiness | deterministic pure stateless synchronous calls; host propagates Error/Missing/Warmup, invokes only with ready inputs, requires ready provider outputs |
| History/state | history 0, no state schema; Indicator category/stateful/provider-managed readiness rejected until later tasks implement them |
| Capabilities | known declaration requirements provider.pure-scalar/1, values/1, parameters/1; no granted host services or external dependencies |
| Metadata | at most 32 identifier/string fields, string values ≤4096 bytes; nonempty semantics ≤4096 bytes; no executable UI/text |
| Limits | at most 256 registry entries; ≤32 requirements; descriptor canonical bytes ≤262144; conservative parameter encoding preflight estimate ≤262144; scalar payload input/output budgets each 1–262144 |

Unknown required capabilities, adapters/runners, optional/variadic cardinality, unsupported scalar/series/window/record/collection types, history/state/dependencies/services and nondeterminism fail explicitly. Existing TypeDescriptor construction rejects unsupported shapes before registration. Logical IDs never launch code or become paths. New names using supported signatures use the same generic interface; they do not introduce per-name dispatch branches.

`max_input_bytes` counts bound parameter payload plus ready input payload: Bool=1, i64/u64=8, decimal coefficient=16, text=UTF-8 length; unbound/readiness markers cost 0. `max_output_bytes` applies to all returned ready payloads. These are logical scalar budgets, not serialized frame sizes, memory quotas or CPU timeouts. Type/identity metadata has its own bounded construction limits. The conservative schema preflight also caps temporary descriptor encoding before the final byte check; it may reject some otherwise individually legal parameter combinations.

## Identity and exact implementation pins

Descriptor canonical JSON is compact UTF-8 with sorted object keys using pinned serde_json. Normalize named ports by name, parameters/maps by key and capability requirements lexically; retain enum-choice order, all semantics/display metadata and all explicit compatibility/resource fields. Reject unsupported declarations before canonicalization. Canonical descriptor identity is SHA-256 of `FQE-DESCRIPTOR-CANONICAL/1\n` (real LF) followed by canonical bytes. Source hashes and descriptor identity are separate.

Registry canonical JSON is `{version:1,entries:[...]}` ordered by `(entry ID, author version)`, with each row containing id/version/descriptor/artifact identities. Registry identity is SHA-256 of `FQE-REGISTRY-CANONICAL/1\n` plus these bytes. Display is the P1-03 lowercase sha256 representation. Registration/port order does not change identity; implementation pins, signatures, constraints, semantics or metadata changes do. Future canonical encoding changes preserve the tracked fixtures or require explicit version evolution.

The registered implementation artifact is supplied by reviewed integration code; the trait cannot attest that an arbitrary Rust object implements its stated source hash. Built-ins and reviewed X-001 fixtures hash their actual tracked provider source file via include_bytes; these are source-artifact pins, not compiled binary signatures, complete dependency closures or evidence of third-party trust. The frozen registry and descriptor content identities expose the exact selected manifest even when an author reuses an ID/version. Future external artifact trust/resolution belongs to P6.

[Independent canonical descriptor](../crates/fqe-ir/tests/fixtures/x001-descriptor.canonical.json) and [expected SHA](../crates/fqe-ir/tests/fixtures/x001-descriptor.identity.txt) are calculated with Python stdlib json/hashlib by [the fixture generator](../scripts/generate_provider_fixtures.py), without invoking Rust encoding. Tests compare exact canonical bytes and digest. Run the generator deliberately after reviewing source changes; it is not run by tests and must not conceal fixture drift.

## Prepare and invoke

1. Resolve the exact entry requirement, bind parameters through P1-02, enforce the parameter payload budget and run the registered provider's parameter-relationship validator. Preparation may execute reviewed validator code; discovery does not. Errors/panics reject preparation without returning a partially prepared call.
2. PreparedCall owns immutable parameters, descriptor/implementation provider handles and registry identity. A call prepared through `prepare_node` additionally retains model-content and node identity. Dropping the Registry does not change or invalidate a prepared handle. No run/account/event scheduler or mutable indicator state is created.
3. Invoke with caller-provided run/instance IDs and positive event/call sequences. Validate full named arity, duplicates, exact type/unit/scale and payload budget before dispatch. Duplicate/stale sequencing across separate invocations is not tracked by this pure service; the later runtime owns event ordering/correlation/commit.
4. Any numeric Error input fails. Otherwise Missing dominates Warmup; the host propagates one readiness state to every declared output without calling provider code. Only an all-ready input set reaches Provider::invoke. Errors/precedence are stable in sorted port-name order, independent of supplied map order. P1-05 [expressions](EXPRESSION_CONTRACT.md) separately implement Boolean short-circuit semantics; this eager call policy remains unchanged.
5. Validate all returned names, duplicates, arity, exact types/units/scales, readiness and output budget before returning any outputs. Numeric failure, returned Error/non-ready output on all-ready inputs, malformed output, provider rejection or unwinding panic returns a bounded ModelError category/path/detail. CallResult contains context, descriptor/registry pins and optional model/node pin, plus read-only validated outputs. No partial result object is published.

Pure providers are reviewed trusted code. The trait's Send+Sync and immutable input references do not prevent interior mutation, filesystem/network access, a loop, allocation, process exit or abort. Catching unwinding panics is not OS isolation, timeout enforcement or rollback of external side effects. Provider diagnostics use fixed bounded categories; panic hooks retain the process's normal behavior. These limitations are explicit; worker/plugin enforcement and transactional state require later gates.

## Model-node preparation and compiler boundary

`validate_requirements` resolves all declared P1-03 entry requirements. `prepare_node` first checks binding/model content identity and requirements, then exact local input/output signature names/counts/types, source-port type and argument bindings to registered parameters. A graph parameter reference copies the validated bound value; unknown provider parameters or invalid cross-parameter relationships fail. The returned ModelNodePin identifies the original content/node.

This is local callable validation, not whole-graph compilation: it does not validate other nodes' complete signatures, perform cycle detection, topological ordering, module expansion, event/history advancement, output deduplication or atomic event commit. P1-07/P1-08 must reuse these checks and pin the full registry/compiled-plan identity. Cyclic declarations can still load; they must not be advertised as executable from local preparation alone. No IR2 format or legacy IR1 evaluator behavior changes here.

## Built-ins and independent X-001 evidence

[builtins.rs](../crates/fqe-ir/src/builtins.rs) supplies exact u64 identity, required-parameter u64 constant and Boolean not. They register through the same public Descriptor/Provider interface as the independent [reviewed provider source](../crates/fqe-ir/examples/support/providers.rs), which lives outside engine src. Registry dispatch does not inspect any indicator/provider business ID.

The independent function takes left/right u64, an optional bias default 0, and returns named checked sum/difference outputs. Its separate predicate validates threshold < ceiling and returns the strict comparison to a supplied threshold. No order/ledger operation or history is present. Two actual [sum](../crates/fqe-ir/tests/fixtures/x001-sum.json)/[difference](../crates/fqe-ir/tests/fixtures/x001-difference.json) model artifacts use provider source pins plus built-ins. The caller explicitly drives constants → pair → predicate → not; the two results are independently known: 125+75=200, 125−75=50; threshold 150 gives true/false, then false/true after not. Model wiring changes model content identity without changing engine code.

Run `cargo run --locked --offline -p fqe-ir --example providers`. [Registry tests](../crates/fqe-ir/tests/registry_contract.rs) verify both compositions and descriptor/registry/parameter/type/readiness/resource/panic failures, exact u64::MAX, independent canonical identity and frozen prepared-handle lifetime. This completes only the P1-04 in-process portion of X-001. Compiled generic compositions, reusable rules, event execution and full X-001/P1-gate acceptance remain P1-07/P1-08/P1-11. External Java/Go providers, Desk discovery and signed customer bundles remain later tasks.
