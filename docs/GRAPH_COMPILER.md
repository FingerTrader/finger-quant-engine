# Typed Graph Compiler and Pure Modules — Contract 1

**Implemented, experimental P1-07.** [Graph import](../crates/fqe-ir/src/graph.rs) and [compiler](../crates/fqe-ir/src/compiler.rs) produce immutable content-pinned plans from real artifacts. [Provider registry](PROVIDER_REGISTRY.md) owns callable validation, [expressions](EXPRESSION_CONTRACT.md) owns pure AST evaluation and [indicators](INDICATOR_CONTRACT.md) owns incremental single-node state. [Project status](PROJECT_STATUS.md) owns progress. The separate [P1-08 event integration](EVENT_EXECUTION.md) now executes these plans with atomic multi-node state. CLI model execution, accounting and Desk integration remain downstream.

## Two explicit artifact formats

`compile_model(LoadedModel, ModelBinding, Registry)` compiles the existing **fqe.model / schema1 / IR2** scalar provider graph. Its accepted syntax, required empty future fields and canonical bytes remain unchanged. Nonempty modules are still rejected by that importer. Original JSON node positions are now retained as noncanonical source metadata; declaration normalization does not erase diagnostics.

`LoadedGraph` separately accepts **fqe.graph / schema1 / IR2**. An old importer rejects this format, rather than silently interpreting new graph fields under fqe.model/1. Use `from_bytes`, bounded `from_reader`/`from_file`, `bind` or `bind_json`, then `compile_graph`. Bindings retain exact graph content identity; bindings for different content fail even when author ID/version match. Neither importer resolves an external module, installs a provider or runs computation. Existing [exact types, parameter and binding encodings](MODEL_LOADING.md) are reused, with the same wire duplicate detection and bounded parser.

The complete tracked syntax is [nested sum](../crates/fqe-ir/tests/fixtures/x001-graph-sum.json) and [nested difference](../crates/fqe-ir/tests/fixtures/x001-graph-difference.json). All object fields are required unless the shared parameter encoding explicitly permits omission. Unknown keys, duplicated declarations/object keys, float/signed JSON numeric tokens, null-as-value and unsupported fields fail closed.

| Object | Fields / meaning |
| --- | --- |
| Root | format=fqe.graph, schema_version=1, ir_version=2, id, version, parameters, requirements, entries, graph, modules, metadata, workflow={}, state=[], dependencies=[] |
| requirements | mandatory model.typed-graph/1; model.pure-modules/1 required for inline definitions or calls; optional values/1, parameters/1; unknown requirements rejected |
| entries | same exact provider ID/version/descriptor-version/role/artifact requirements as fqe.model; unused declared providers must also resolve |
| graph | inputs: typed named ports; nodes: declarations; outputs: typed named sources; graph inputs are placeholders for a later caller/event binding, not hidden price feeds |
| node | id; target `{kind: provider or module, key: {id,version}}`; arguments; inputs: named sources; outputs: typed named ports |
| source | exactly `{input: name}` for a public input, or `{node: id, port: name}` for a local node output |
| graph output | name, type, source; declared type must exactly match the resolved source |
| module | schema_version=1, namespaced key `{id,version}`, parameters, graph; pure reusable inline definition with typed public ports |

Provider targets require an exact entry from the root manifest. Module targets resolve an exact inline ID/version; the complete inline definition set is pinned by the root content identity and compiled manifest. There is no latest-version lookup or external file/package loading. Module content IDs identify their own definition bytes, **not** a standalone transitive dependency package; the root's module set/provider requirements pin that closure. A wrapper's local hash need not change when another module changes, but root/compiled identities do.

Module instances bind separate immutable parameters using the shared required/default/absent/range/enum/type rules. Definition arguments are literals or enclosing parameter references; unknown references and mismatched declared types fail static validation. Literal constraints and missing required target parameters are checked even in unused definitions. Actual module constraints and provider cross-parameter validators run for each expanded invocation. Optional unbound parameters may remain unused; referencing one fails compilation. A broad parent parameter constraint never bypasses a narrower child constraint. Parameter descriptions and IDs are data, not executable instructions.

## Validation and expansion

1. Check exact binding content and resolve every declared provider requirement against one frozen registry. Signature/role/version/artifact/capability failures return no compiled plan.
2. Validate all module definitions, including unused definitions: exact target resolution, ports/arity/types/units/scales, parameter declarations, graph output types and same-event dependencies. Modules may reference other pure modules. Incremental providers anywhere in a module are rejected, even when their outputs are unused. Generalized stateful modules are P2 work.
3. Reject direct/indirect recursion across the entire inline module set. Detect cycles in each graph before expansion. Every provider, including incremental providers, consumes current-event inputs; it is not an implicit Delay. State/Delay/workflow node kinds are explicitly unsupported, so feedback cannot disguise an illegal same-event cycle.
4. Bind actual parameters and expand modules in stable topological order. Rewire public input/output aliases to leaf sources; empty passthrough modules are valid. Preserve call-site and definition source locations. Expansion budgets apply before returning any plan. A failure discards partially prepared handles.
5. Sort expanded provider leaves topologically, breaking ready-node ties by lexical **path vector**, never declaration/editor/hash-map order. Paths are arrays of local node IDs, for example `[first,nested,pair]`; a root ID containing `/` cannot collide with that scoped path.
6. Bind/validate providers through the shared pure/incremental registry interfaces. No provider-name switch exists in compiler/import code. A new conforming reviewed provider with supported signatures follows the same path.

Static validation accepts only exactly-one supported scalar ports; units, scale and instrument/currency IDs must match exactly. No casts, implicit rescale, domain inference or parameter coercion. Multi-output functions retain separate named ports. All same-event edges must form a DAG. No module silently grants services, isolation or external capabilities beyond the registry's implemented subset.

## Prepared calls and state boundary

A CompiledNode contains its stable path, resolved inputs, source map and either a shared PreparedCall or PreparedIncremental. Both retain immutable descriptor/implementation/parameter/registry pins. `Registry::prepare_incremental` binds parameters and executes the reviewed relationship validator **without** calling its factory. `PreparedIncremental::instantiate(run,instance)` later creates fresh owned state. Existing `Registry::instantiate` remains a convenience wrapper and validates IDs before callbacks. Dropping the registry does not invalidate prepared handles.

Compilation never calls Provider::invoke, IncrementalProvider::create or IncrementalState::step. Relationship validators are trusted Rust code and may execute during preparation. Their side effects, loops/aborts and malicious implementations are not isolated or transactional. A failure returns no plan or mutable strategy state; it is not rollback of arbitrary validator effects.

The optional `CompiledPlan::pure_expressions` lowers provider-call DAG outputs into the existing P1-05 typed AST, with module aliases already resolved. It rejects **any** incremental entry rather than copying history into pure calls. Pure subgraphs may be duplicated in the AST and therefore invoked repeatedly; purity is a reviewed contract. It enforces P1-05's 32-depth/4096-AST-node ceilings while constructing the projection, so a shared exponential DAG cannot allocate an unbounded tree. A valid larger compiled graph can fail this optional projection and remain usable by a later graph runtime. Output-driven AST evaluation can skip unused pure nodes; all declarations were still validated/prepared at compile time. The [pure example driver](../crates/fqe-ir/examples/support/compiled.rs) uses this projection and the real expression evaluator, with no handwritten business-node wiring.

This graph format contains provider/module targets, preserving its original bytes. The separate [fqe.event-plan/1 wrapper](EVENT_EXECUTION.md) supplies the explicit bounded/versioned BinaryOp/NOT/cross codec, field bindings and signal roots, retaining short-circuit and atomic once-per-event stateful semantics. A compiled incremental handle alone remains insufficient to execute a market strategy without that profile.

## Limits and identities

| Limit | Bound |
| --- | --- |
| Source and normalized artifact | ≤262144 bytes; shared parser depth32/items16384/collection1024/string4096/key128 |
| Inline modules | ≤32, exact unique ID/version; module nesting ≤8 |
| Declarations | ≤256 nodes across root and all module definitions; each graph 0–32 inputs, 1–32 outputs, each node ≤32 inputs/outputs and ≤256 arguments |
| Declaration edges | ≤4096 node-input wires plus graph-output aliases across all definitions |
| Expanded work | ≤1024 node/call visits including empty modules; ≤256 actual provider leaves; ≤4096 input/output-alias edges counting repeated module invocations |
| Compiled manifest | ≤1048576 bytes; checked after bounded construction; not a measured peak-memory/CPU quota |
| Metadata / parameters | shared limits; metadata ≤32 fields/4096 bytes each, identifier length ≤128 |

An input can satisfy one limit and still fail another. These bounds may reject mathematically finite compositions. They are deterministic supported-capability limits, not OS isolation or performance guarantees. Source paths and expansion are bounded by declarations/depth; provider registration/dependencies/payload budgets retain their own limits.

Graph canonicalization uses compact sorted-key UTF-8 JSON and the shared exact numeric strings. Sort provider requirements/modules by ID/version; parameter schemas/maps by key; requirements lexically; graph nodes by ID and named inputs/outputs by name. Include metadata, schemas, constraints, descriptions, explicit empty future fields and all module bodies. Canonical graph identity is SHA-256 of `FQE-GRAPH-CANONICAL/1\n` plus canonical bytes. Module-local identity uses `FQE-PURE-MODULE-CANONICAL/1\n` plus its normalized definition. Prefixes contain real LF bytes. Raw source identity is SHA-256 of original bytes.

Compiled identity is SHA-256 of `FQE-COMPILED-GRAPH/1\n` plus compact sorted-key plan JSON: compiler_version=1, source format, model content ID, entire registry ID, bound root parameters (including explicit Unbound markers), all module-local identities, typed public inputs, ordered leaf paths/descriptors/bound parameters/resolved inputs, and typed resolved outputs. Changing bindings, any registry descriptor, root/module content or wiring changes that identity. Actual syntax positions/call-site paths are noncanonical source metadata: reordering declarations/whitespace preserves graph/plan identities while source maps reflect the actual new file. These hashes are not binary attestations, signatures or complete run/dataset identities.

`graph_*` diagnostics report import format/structure/capabilities/limits; `compile_*` report binding/signature/type/reference/purity/cycle/recursion/expansion/projection failures. Provider validation retains its `registry_*` code. Node-specific errors identify original definition positions; per-invocation binding/provider failures additionally retain nested call sites using `via`. Static module errors point to their definition, rather than picking one of several invocations. Global manifest/collection errors may identify a collection rather than an individual leaf. No untrusted provider output/private JSON tokens are copied into diagnostic details.

## Executed acceptance and remaining gates

[Compiler tests](../crates/fqe-ir/tests/compiler_contract.rs) compile/run both existing X-001 artifacts and two new nested/reused multi-output rule graphs, including typed predicates and independent parameter bindings. Generic compiled pure outputs agree with the known arithmetic: 125+75=200, difference50; threshold150 yields true for sum and false for difference; threshold250 yields false for both; NOT reverses the first decision. No provider business ID appears in compiler or graph source.

[Independent fixture generator](../scripts/generate_compiler_fixtures.py) uses only Python json/hashlib, importing no engine. [Expected canonical bytes](../crates/fqe-ir/tests/fixtures/graph.canonical.json) and [identity](../crates/fqe-ir/tests/fixtures/graph.identity.txt) are tracked and never regenerated by tests. Tests also cover default/range/enum/unbound/relationship constraints, bad ports/versions/artifacts/units/scales, direct/indirect cycles/recursion, nested source locations, slash-safe paths, real indicator compilation, stable identity/order, pure AST mapping/expansion limits, 1024-visit exact boundary, independent alias-edge/node/depth limits, and zero compile-time state creation.

```sh
cargo test -p fqe-ir --test compiler_contract --locked --offline
cargo run --locked --offline -p fqe-ir --example compile
```

P1-08 event evaluation is delivered separately in the library. P1-09 loaded CLI execution, P1-10 external caller, P1-11 full X-001/indicator golden gate, P2 persistence and P4/P6 Desk distribution remain separate acceptance. No package or toolchain dependency is added.
