# Stateful Workflow Execution

Status: experimental P2-01 scalar workflow library profile. This contract owns `fqe.workflow/1`, host-owned stateful rule modules, atomic State/Delay and lifecycle. [Compiled events](EVENT_EXECUTION.md), [expressions](EXPRESSION_CONTRACT.md), [registry](PROVIDER_REGISTRY.md) and [type/parameters](TYPE_PARAMETER_CONTRACT.md) retain authority for their embedded semantics. This profile does not change P1 wire formats or the legacy one-shot runtime.

## Artifact and compilation

`fqe_ir::workflow::LoadedWorkflow::from_bytes` strictly decodes the root fields `format`, `schema_version`, `id`, `version`, `requirements`, `event_plan`, `modules`, `instances`, `signals`. Format is `fqe.workflow`, version is integer1. Require `workflow.scalar/1`; recognized additional capabilities are `workflow.state/1`, `workflow.delay-one/1`, `workflow.rules/1`, `workflow.stop/1`. Unknown/duplicate requirements fail. Requiring snapshot/restore or worker support fails explicitly. Fields are exact, duplicate keys fail, full-width integer values use decimal strings, and existing bounded JSON/typed-value rules apply.

The embedded fqe.event-plan/1 supplies one tick or closed-bar trigger, typed provider graph, incremental indicators and crossing roots. Its signals must be empty: the workflow exclusively owns emitted signals. A root still evaluates once per accepted matching event, independently of rule guards. Compilation reuses the shared typed expression checker, arithmetic, parameter binder and provider registry; it never creates provider state or executes providers. No financial function names are built into the workflow evaluator.

A module declares exactly:

| Field | Meaning |
| --- | --- |
| `schema_version`, `key` | Integer1, exact module `{id,version}` identity |
| `state_schema` | Required portable cell-schema `{id,version}` identity, not a Rust type/pointer |
| `parameters` | Existing version1 typed parameter schema, defaults and constraints |
| `inputs` | Named typed public ports |
| `state` | Named `{name,type,initial}` cells; initial is an exact matching typed value |
| `delays` | Named `{name,type,initial,next}` one-matching-event delays; initial readiness is ready/missing/warmup, with a typed value only for ready |
| `modes`, `initial_mode` | Nonempty unique declared modes and a valid initial mode |
| `transitions` | Named `{id,from,to,guard,writes}` declarations; from is a nonempty set of valid modes, to is a valid mode or null, writes map declared local cells to expressions |
| `outputs` | Nonempty named expression map; public types are inferred and checked |

Each instance declares exactly `id`, `module:{id,version}`, `parameters:{version:1,values:{...}}`, and exact named `bindings`. A binding is `{event:root}` or `{instance:id,output:name}`. Parameter values are immutable and instance-specific. Bindings must match exact output type and numeric unit. Modules are reusable definitions, not live objects; two instances never share mutable cells, delays or mode. Unused module definitions are rejected in this bounded profile.

Instance dependencies form a same-event DAG with lexical instance-ID tie breaks. Explicit local Delay feedback reads prior-event memory and is valid; it does not legalize a same-event instance cycle. There is one control region and all control transitions have equal priority; ambiguous enabled targets fail. Priority levels, terminal workflow modes and parallel regions are rejected as unsupported fields. Flat module instances are supported; recursive/nested stateful-module definitions, arbitrary language callbacks and dynamic loading are separate capabilities.

Expressions reuse the existing wire AST for literal, parameter, input, not and binary operations. Slots are `input.<port>`, `state.<cell>`, `delay.<cell>`, `mode` (text, max128 bytes), and `fired.<transition>`. Provider/root/cross computations remain in the event plan, so direct output/cross AST references in a rule module are rejected. Guards must be Boolean and cannot read fired flags. Writes and delay-next expressions must exactly match declared types; signal rules `{instance,output,kind}` require Boolean outputs and kind long/flat. Duplicate instance/output signal rules fail.

Limits: source/canonical document262144 bytes; shared JSON depth32/items16384/collection1024/string4096; at most16 modules and16 instances; local names120 bytes; per-module modes16 and each named declaration group32; total input/state/delay/fired/mode slots32; public outputs32; workflow source expressions4096/depth32 and aggregate lowered expressions across all instances4096; signal rules32. These are scalar-profile admission bounds, not CPU deadlines or a process sandbox.

## Whole-event transaction

`fqe_runtime::workflow::WorkflowRuntime` owns an immutable compiled definition, a fresh compiled event runtime, and per-instance typed cells/delays/mode. One synchronous `step` proceeds as follows:

1. Reject terminal lifecycle, invalid event domain/order and counter overflow before provider state is forked. Existing event/bar validation remains authoritative.
2. Privately deep-fork provider and crossing history and evaluate the matching event once. The outer transaction currently copies candidates again inside EventRuntime; this favors correctness and makes no throughput claim. Provider financial state is deep-owned; test-only shared atomic counters measure calls/lifetime, not computation.
3. In stable instance order, bind current event/upstream outputs. Read this instance's committed old cells, old delay values and old mode. Evaluate every guard eligible for the old mode in lexical transition-ID order. Missing/warmup guards do not fire. Noneligible guards and unselected effects are not evaluated. Guards cannot observe another guard's selection or staged writes.
4. Publish the selected set as Boolean fired slots for effects/delay-next/public outputs. All enabled effects still read the same old cell/delay values. Multiple enabled control transitions fail even if their targets agree; multiple writes to one cell fail even if their values agree. Disjoint writes are allowed. State writes must be Ready; a missing/warmup write rejects the event. Numeric/provider errors also reject the event.
5. Compute delay-next values for the next accepted matching event; missing/warmup can be stored explicitly. Compute current module outputs from old cells/delays and current inputs/fired slots. A downstream instance sees these current outputs, never upstream staged writes. Selected mode changes and state updates become visible on the next matching event.
6. Check the total encoded cell-state inspection budget, validate/collect signals, then commit provider state, crossings, all instances, counters and cursors together. Any prior Result error discards every candidate. There are no partial observations/signals; the same failed input sequence can be corrected and retried.

An unmatched instrument tick advances accepted source ordering/counts, but does not evaluate rules, advance State/Delay or produce signals. Existing bar-domain rejection remains unchanged. This is not a multi-market scheduler.

Warmup is conservative for the whole workflow: base warmup, any unavailable bound input/old delay, any unavailable eligible guard or public output suppresses all signals for that event. Ready state updates and delay updates still commit during warmup. Signals carry instance/output origin plus the existing instrument/kind/source-sequence/time record; they are observations, not orders or ledger changes. Ordering is canonical instance/output rule order.

## Lifecycle and portability boundary

Ready → Running after a successful step. `finish()` → Completed, `stop()` → Stopped and explicit host `abort()` → Failed are terminal, including from Ready. Rejected step errors preserve Ready/Running and all committed state; the host may retry or explicitly abort. Terminal step/finish/stop/abort calls fail. These names do not change the legacy Failed-on-run-error API.

Terminal operations release owned in-process provider/crossing runtime state via Rust ownership, while retaining final typed workflow cells and counters for inspection. `restart(new_run)` is permitted only after terminal status and with a different valid run ID; it constructs initial cells/delays/mode and fresh provider factories. It never resumes. The caller owns uniqueness of run identities across independent runtime objects. Stop takes effect at synchronous call boundaries; it cannot interrupt a running provider, kill a worker or promise graceful cross-process disposal. Trusted provider Drop/panic, external side effects, allocation/process failure and hostile shared memory remain outside Result-based atomicity.

`inspect_state()` returns bounded UTF-8 JSON `fqe.workflow-state-view/1` with `restorable:false`, workflow execution ID, run and per-instance exact module/state-schema identities, old committed mode, typed cells and delay readiness. It is portable data for inspection, not a snapshot: it deliberately excludes opaque incremental-provider/crossing history and input/output cursors. The total is capped at65536 bytes both before factories and before each commit. No language objects, pointers, implicit serialization or import API exist. `checkpoint()` explicitly returns workflow_capability. Full compatible snapshot/restore, instance-state migrations and X-002 split-run parity belong to P2-04; workers repeat that acceptance in P5/P6.

Canonical bytes normalize module/instance/transition/mode/signal/parameter-schema declaration order and reuse embedded event canonicalization. Artifact ID hashes the canonical workflow with a versioned domain. Source ID hashes the exact outer source bytes. The embedded event source ID refers to the parsed/re-encoded event fragment, not a separately supplied original file. Execution ID binds workflow artifact ID to the embedded event execution/registry/bound-parameter identity. Run/data IDs are recorded separately in the manifest; state views cannot prove complete recovery compatibility.

## Reproducible evidence

[Counter artifact](../crates/fqe-ir/tests/fixtures/workflow-counter.json), [input data](../crates/fqe-runtime/tests/fixtures/p2/x002-data.json) and [independent hand calculation](../crates/fqe-runtime/tests/fixtures/p2/x002.expected.tsv) define the P2-01 X-002 slice. Prices90,110,120,80,130 yield old fast counts0..4 and old slow counts0,2,4,6,8; committed counts become1..5 and2,4,6,8,10. First delay is Warmup, then90,110,120,80. Fast entry fires at2/5 and exit at4; slow entry fires at5. This table is authored independently and never regenerated by tests.

[IR contract tests](../crates/fqe-ir/tests/workflow_contract.rs) cover strict versions/schema/types/capabilities/bounds, canonical identities and instance cycles. [Runtime tests](../crates/fqe-runtime/tests/workflow_execution.rs) cover the table, separate parameters/instances, old/new effects, conflict rollback, missing/warmup behavior, delay errors, terminal release/fresh restart and a test-only provider whose late division error proves provider/crossing/cell/delay/cursor rollback and same-sequence retry. Existing provider source pins and P1 expected files are preserved. `cargo run --locked --offline -p fqe-runtime --example workflow` runs the typed fixture through the public library; no CLI workflow transport is claimed.

P2-02 owns multi-symbol/timeframe watermark ordering; P2-03 owns windows/timers/instance scheduling; P2-04 owns complete snapshots; P2-05 owns deterministic replay/debug trace; P2-06 owns the complete G-006 temporal gate. Current scalar rule composition does not claim those deliverables, Desk/Flutter packaging, language workers, trading SDK integration, accounting, backtesting or simulation.
