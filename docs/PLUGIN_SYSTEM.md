# Plugin system

Status: planned. No plugin loader, plugin API/ABI, registry, manifest parser, or dynamic library support is implemented.

[ADR-008](ARCHITECTURE_DECISIONS.md#adr-008--versioned-plugin-extensibility) fixes the versioned registry/extensibility direction. P6 in [Delivery plan](DELIVERY_PLAN.md) requires actual resolution, permission/resource enforcement, load/execute/unload and release evidence. Plugin execution remains undecided; marketplace availability must not gate local execution.

## Intended extension points

Candidate plugins include local dataset readers, indicator/operator extensions, report exporters, simulation models, and the optional Trading SDK execution adapter. Add an extension point only when a concrete implementation justifies it.

Plugins must preserve local execution and the separation between domain semantics, transports, and optional clients. A plugin cannot make Flutter or Finger Trader login mandatory for offline core functionality.

## Planned plugin manifest

A future manifest should declare a stable plugin ID and version, entrypoint/artifact identity, supported engine and plugin-contract versions, capabilities, resource requirements, and explicit filesystem/network/execution permissions. Artifact hashes or signatures require a distribution trust policy; a declared hash alone is not provenance.

Capability negotiation must fail closed on missing required features. Registration must not execute arbitrary package installation or download code implicitly.

## Loading and compatibility

Prefer an out-of-process contract for untrusted extensions where practical. A process boundary still does not provide OS sandboxing. Native in-process plugins would share the engine's address space and privileges; Rust crate types are not a stable cross-version dynamic ABI.

Do not choose a dynamic ABI or expose internal Rust structs as a public plugin interface during initialization. Define extension semantics and conformance tests before freezing a transport or ABI.

Engine package version, Strategy IR version, wire protocol version, and plugin contract version are separate. Compatibility must be explicit rather than inferred from matching package names.

## Authority and faults

An indicator plugin should not obtain order execution rights by default. A dataset plugin should not receive broker credentials. The host should validate plugin outputs and enforce declared capabilities, timeouts, cancellation, bounded logs, and resource budgets.

Planned failure policy must address malformed output, crashes, incompatible versions, resource exhaustion, and abandoned runs. No current API promises fault containment or recovery.

## First implementation gate

Select one narrow extension point, specify its inputs/outputs and trust model, build a reference implementation, and test lifecycle and failure behavior before generalizing to a plugin ecosystem. No plugin crate is created solely as an empty placeholder.
