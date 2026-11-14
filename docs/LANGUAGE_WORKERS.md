# Language workers

Status: planned. No Python, Java, or Go worker, launcher, language SDK, or language runtime bundle is included.

The mandatory delivery order is **Python → Java → Go**, each independently accepted by P5-02/P5-03/P5-04 in [Delivery plan](DELIVERY_PLAN.md). A protocol placeholder or example alone is not worker support. See [ADR-003](ARCHITECTURE_DECISIONS.md#adr-003--isolated-language-workers).

## Role

Workers will let strategies authored in Python, Java, and Go participate in local FQE runs. All worker strategy computation executes on the user's device. The Rust host owns run lifecycle, logical event order, validation, and authority over downstream execution.

Language-neutral does not mean arbitrary programs in different languages produce identical results. Portable behavior requires the same explicit contracts and a supported numerical and state model.

## Two intended authoring paths

1. A compiler or adapter lowers a supported declarative subset into typed Strategy IR. The Rust interpreter then evaluates it.
2. A supervised language worker executes imperative strategy code and communicates through a versioned worker protocol.

The present IR is a single price comparison; it cannot represent arbitrary Python, Java, or Go code. A compiler must reject unsupported constructs explicitly. Worker execution may support more language features while carrying separate reproducibility and trust requirements.

## Planned process contract

A future worker session needs:

| Step | Required behavior |
| --- | --- |
| Handshake | Protocol version, worker identity/version, supported capabilities |
| Load | Validated artifact identity, entrypoint, parameter schema, declared permissions |
| Start | Run identity, deterministic configuration and initial declared state |
| Event processing | Ordered input correlation, validated outputs, bounded response size |
| State exchange | Versioned declared state with explicit ownership and checkpoint boundary |
| Shutdown | Cancellation, graceful stop deadline, terminal outcome |
| Fault handling | Timeout, crash, malformed payload, incompatible version, restart policy |

The experimental protobuf file only sketches strategy and event data. It has no worker handshake, lifecycle RPC, transport, or code generation. The implemented version-number helper is not a worker connection.

## Host and worker responsibilities

The host validates event order and each worker output before applying it. Workers must not be treated as the source of broker truth or be given trading credentials by default. Worker logs, metrics, and strategy outputs need separate message types and bounded channels.

Process isolation is not a sandbox. Python imports, JVM code, Go binaries, and native extensions can access local resources under their process permissions. A production design needs explicit trust, filesystem/network permissions, dependency provenance, secret boundaries, deadlines, and resource limits appropriate to the operating system.

## Distribution and conformance

Users may initially supply their own language runtimes. Bundled runtime distribution, package installation, signed worker artifacts, and runtime version matrices remain open design work.

Before claiming support for a language, provide a runnable reference worker, host integration, exact integer serialization tests, shared event/IR fixtures, lifecycle and failure tests, and installation documentation. No language support is released by the presence of these documents.

P5-01/P5-02 also own the existing Desk hosted-Python compatibility path described in [Desk integration](DESK_INTEGRATION_PLAN.md). The existing SDK-hosted callback/API/JSON-RPC candidate is not automatically compatible with a new FQE worker. Record mappings and transaction boundaries, preserve/version accepted operations on later callback failure, and prove actual Desk use with the FQE host. Retaining an explicitly labeled legacy path before this gate does not count as FQE Python support.
