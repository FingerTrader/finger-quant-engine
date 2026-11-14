# Versioned Model Loading

**Implemented, experimental P1-03 subset.** This contract owns the external model/binding JSON syntax, normalization, identity and bounded import behavior. [Type/parameters](TYPE_PARAMETER_CONTRACT.md) owns scalar semantics; [P1 design](P1_STRATEGY_DESIGN.md) owns the broader graph target. Import does not compile or run a graph. [Status](PROJECT_STATUS.md) owns progress.

## Version and immutable objects

The new format is `fqe.model`, model schema 1, model IR 2. The legacy `IR_VERSION=1` positive-u64 price evaluator remains separate. Package version 0.1.0, draft Protobuf namespace and protocol negotiation helper do not identify this file format. Unsupported versions fail; there is no automatic migration or fallback.

[Model declarations](../crates/fqe-ir/src/model.rs) construct a validated, immutable ModelDefinition from a mutable ModelDraft. [Loading](../crates/fqe-ir/src/loading.rs) adds LoadedModel with read-only definition, canonical bytes, raw-source identity and canonical content identity. ModelBinding contains the model ID, author version, content ID and immutable BoundParameters. It creates no run, cursor, account, position, ledger, timer or mutable execution state. Bindings and canonical bytes cannot be mutated through the public API. `ensure_matches` checks exact content before reuse; an author's ID/version is not permission to replace content silently.

## External fields and presence

The tracked [model.json](../crates/fqe-ir/tests/fixtures/model.json) is the complete valid syntax reference; all fields shown there are required except parameter `default` and constraint `lower`, `upper`, `choices`. Unknown fields are rejected at every object level, including nested type/unit/value/reference objects. Duplicate JSON object keys, including escaped equivalent keys, fail before map conversion; repeated named declarations fail structural validation. Missing differs from null. No unrecognized property is retained as executable text.

| Object | Fields / rules |
| --- | --- |
| Root | format, schema_version, ir_version, id, version, parameters, requirements, entries, graph, metadata, modules, workflow, state, dependencies |
| parameters | version 1, entries of key/type/required/constraints/description and optional default |
| constraints | optional lower/upper `{value: typed-value, inclusive: bool}`; optional choices array of typed values; P1-02 validates discrete bounds, duplicates, defaults and membership |
| entries | key `{id, version}`, descriptor_version 1, role `provider`, artifact `sha256:` plus 64 lowercase hex digits; declares a pinned provider artifact, not an installed/trusted implementation |
| graph | nodes and outputs; node fields id/entry/arguments/inputs/outputs; arguments are exactly `{parameter: key}` or `{literal: typed-value}` |
| Named input / graph output | name, source `{node, port}`; source must refer to a declared node output |
| Named node output | name, type; registered signature/type compatibility is a later validation gate |
| metadata | bounded identifier-to-string map, retained in identity |
| modules / state / dependencies / workflow | required empty arrays / empty object respectively; nonempty content is explicitly unsupported until its owning task implements it |

IDs and author versions are opaque case-sensitive logical identifiers, 1–128 ASCII letters/digits or `._-:/`; they never become paths, shell commands or download addresses. Parameter keys retain the narrower P1-02 alphabet. Entries are unique by `(id, version)`; descriptor versions are independent. Only `model.scalar-graph/1`, `values/1`, `parameters/1` declaration capabilities are recognized. Unknown required capability or unsupported adapter/worker/plugin role is rejected. Empty requirements are allowed for this baseline declaration format; they do not enable execution privileges. Nonempty external dependencies fail closed.

Provider IDs need not be built into the loader. Artifact digest syntax and declared references are checked, but provider availability, artifact provenance, callable signatures, type compatibility, arity, cycles, recursive modules, graph compilation and execution belong to P1-04/P1-07/P1-08 and later extension tasks. No claim that a structurally valid imported graph is executable follows from loading it.

## Exact values and binding files

A type is `{version: 1, shape: "scalar", kind: "bool"|"i64"|"u64"|"decimal"|"text"}`. Decimal adds `scale` (0–18) and `unit`: `{kind:"scalar"|"ratio"}`, `{kind:"price"|"quantity",instrument:"..."}`, or `{kind:"money",currency:"USD"}`. Text adds `max_bytes` (1–4096). Unknown kinds, unsupported shapes/versions and extra properties fail. Instrument/currency syntax and exact type/unit matching follow P1-02; these IDs are not registry verification.

Typed values are `{type: descriptor, value: payload}`. Bool uses a JSON Boolean; text uses a UTF-8 string; i64/u64 use canonical decimal integer strings; decimal uses its signed i128 coefficient string and descriptor scale/unit. No plus sign, leading zero, negative zero, fractional or exponent notation, implicit scale/unit conversion or floating-point substitution is accepted for these numeric payloads. Versions/limits use bounded unsigned JSON integers. Negative or floating-point JSON number values are unsupported. u64::MAX, i64::MIN and i128::MIN are exact through import, binding and canonical round-trip.

`LoadedModel.bind_json` accepts `{ "version": 1, "values": { ... key: typed-value ... } }`. The same byte/depth/item limits and duplicate/unknown-field rules apply. Omission uses a default, fails a required parameter or leaves an optional unreferenced parameter Unbound. A graph argument referencing an Unbound parameter fails preparation. Null, missing/warmup/error readiness objects, wrong descriptor/shape/unit/scale, out-of-range/disallowed values, unknown keys and duplicates fail atomically. A supplied valid value overrides a default without mutating the definition or previous bindings. Runtime readiness is not a serialized parameter value.

## Resource limits and diagnostics

- Source input and canonical output: at most 262144 bytes; readers consume at most 262145 bytes to detect excess. File import opens only the caller-supplied path; no model property triggers file/network/provider access.
- JSON nesting depth: root 0, maximum 32; at most 16384 visited values and 1024 elements per JSON array/object; keys at most 128 UTF-8 bytes, string values at most 4096 UTF-8 bytes. Keys are not included in the visited-value count. Total input bounds also bound temporary string/key allocations before per-field checks.
- Structural limits: 1–256 entries/nodes, 1–32 external outputs, 0–32 inputs and 1–32 outputs per node, at most 256 arguments per node, 4096 node edges, 32 requirements and 32 metadata fields. P1-02 separately caps parameters/choices at 256. Parsing limits may reject an otherwise structurally legal maximal combination; the effective budget is the intersection of all limits, not a promise that every Cartesian product fits.
- Invalid UTF-8, malformed/truncated/trailing input, wire duplicates, limits and unsupported semantics return ModelError with category/path/detail. Syntax/duplicate/resource parse errors retain line/column and root path without echoing input tokens; semantic errors retain the relevant object/array path (not always the exact leaf). Read errors hide OS/private content. Resource ceilings are deterministic acceptance limits, not measured CPU latency or OS isolation guarantees.

## Canonical content identity v1

Raw identity = SHA-256 of the exact input bytes. Canonical identity = SHA-256 of `FQE-MODEL-CANONICAL/1\n` UTF-8 bytes followed immediately by canonical JSON bytes, with no trailing newline. Display is `sha256:` plus 64 lowercase hex digits. The prefix contains a real LF byte. Digest equality proves content equality under this normalization assumption; it is not a signature, trust decision or complete future run-reproducibility manifest.

Normalize validated definitions: sort entry requirements by `(id, version)`, nodes by ID, named inputs/outputs by name, parameter specs/argument/metadata maps by key, requirements lexically. JSON object keys use lexical order recursively; compact UTF-8 encoding uses the pinned serde_json escaping, no insignificant whitespace or Unicode normalization. Include all required empty future fields, metadata and descriptions; omit absent optional default/constraint fields. Preserve enum-choice order as declared presentation order. Numeric payloads remain canonical decimal strings. This is FQE canonicalization version 1, not RFC 8785. Canonical serializer/library/normalization changes must preserve fixtures or introduce a new canonicalization version.

Reordering object keys, named collections, escaped key spellings and whitespace does not change canonical identity. Changing a value, descriptor/unit/scale, provider artifact, ID/version, choice order, description or metadata changes identity. Binding values do not change the model's identity. An explicit content-pinned binding cannot be silently reused with modified model content carrying the same author ID/version.

[canonical.json](../crates/fqe-ir/tests/fixtures/canonical.json) and [identities.txt](../crates/fqe-ir/tests/fixtures/identities.txt) were calculated independently with Python stdlib `json.dumps(sort_keys=True, ensure_ascii=False, separators=(',', ':'))` and hashlib SHA-256, from the already-normalized tracked fixture. Rust tests compare exact bytes/digests and complete canonical re-import, not an expected digest recomputed by the Rust implementation. The fixture includes Chinese UTF-8 and u64::MAX. The test suite also checks i128::MIN and numeric/unit boundaries.

## Existing Desk compatibility dispositions

The dated inventory in [Desk integration](DESK_INTEGRATION_PLAN.md) is authoritative for the existing application:

| Existing consumer/artifact | Disposition |
| --- | --- |
| Legacy dual-SMA visual model, Dart model IDs/version | Explicit future converter under Desk/P4 tasks, with typed descriptor mapping and a new FQE content identity; do not reinterpret as IR2 or rewrite existing drafts |
| Dart/Python execution and hosted API strategy schemas | Keep historical backend identity. JSON import is local and does not call the hosted executor; language adapters remain P5 work |
| Existing datasets, backtest/simulation records and journals | Preserve original backend/schema read path and records. Future imported runs have explicit engine/model/data identities; no automatic historical recomputation or destructive migration |
| Existing integer quantity/double risk/accounting inputs | Explicit precision/unit mapping or actionable rejection in P3/P4; import alone does not prove financial equivalence |
| Legacy Rust IR1 / protocol .proto draft | Separate APIs, preserved behavior; missing model format/IR2 rejected by the new importer; no generator or wire compatibility inferred |
| Flutter distribution/macOS sandbox | P4/P6 still must demonstrate native boundary, bundle and installed backtest/simulation acceptance; no customer runtime download is added here |

No Desk source/data/run was changed for this task. P1-03 does not close paired Desk acceptance.

## Executed acceptance and remaining gates

Run `cargo run --locked --offline -p fqe-ir --example load_model` to load the tracked file, print both identities, prepare period defaults/override and reject null without executing a strategy. The legacy CLI demo and parameter example remain available. [Public tests](../crates/fqe-ir/tests/loading_contract.rs) and [declaration tests](../crates/fqe-ir/tests/model_contract.rs) cover the stated import/binding/identity/resource boundaries and real invalid fixtures. Dependency decision/review: [report](P1_03_DEPENDENCY_REVIEW.md), ADR-014 in [ADRs](ARCHITECTURE_DECISIONS.md).

Registries/providers, arbitrary indicators/rules, workflow/state, workers/plugins, compiler/runtime integration, CLI model execution and Flutter delivery retain their own tasks. No numeric/financial/extension or phase gate is completed by successful import alone.
