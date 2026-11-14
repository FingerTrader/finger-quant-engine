# Document Authority

## Facts and normative authority

Actual code, executed tests and Git history establish **what exists**. Documents establish **what is required** within their assigned scope. A future design cannot turn an unimplemented feature into a fact. If records disagree with repository evidence, correct [Project status](PROJECT_STATUS.md) and [Next session](NEXT_SESSION.md) before continuing; record the discrepancy rather than silently redefining behavior.

Normative precedence, within scope:

1. [Project charter](PROJECT_CHARTER.md): unique product goal and boundaries.
2. [Architecture decisions](ARCHITECTURE_DECISIONS.md): accepted durable technical directions; supersession requires a new ADR.
3. Technical contracts: [Strategy](STRATEGY_CONTRACT.md), [Events](EVENT_CONTRACT.md), [State](STATE_CONTRACT.md), [Runtime](RUNTIME.md), [P1 design](P1_STRATEGY_DESIGN.md), [Type/parameters](TYPE_PARAMETER_CONTRACT.md), [Model loading](MODEL_LOADING.md), [Provider registry](PROVIDER_REGISTRY.md), [Expressions](EXPRESSION_CONTRACT.md), [Indicators](INDICATOR_CONTRACT.md), [Graph compiler](GRAPH_COMPILER.md), [Compiled events](EVENT_EXECUTION.md), [Stateful workflows](WORKFLOW_EXECUTION.md), [Market scheduler](MARKET_SCHEDULER.md), [Temporal scheduler](TEMPORAL_SCHEDULER.md), [Checkpoint/restore](CHECKPOINT_RESTORE.md), [Replay/trace](REPLAY_TRACE.md), [CLI exchange](CLI_EXECUTION.md), [External invocation](EXTERNAL_INVOCATION.md), [Extensibility](EXTENSIBILITY_CONTRACT.md) and [Desk integration](DESK_INTEGRATION_PLAN.md). Current-behavior sections describe the experimental baseline; future sections specify implementation targets. P1 design governs the new universal model, not a silent reinterpretation of IR_VERSION=1. Extensibility owns shared adapter/provider roles, descriptors, rule composition and onboarding acceptance; worker/plugin guides specialize it without redefining those semantics. Desk integration owns cross-project delivery/migration acceptance; Desk's own source and contracts remain authoritative for its existing behavior and app shell.
4. [Delivery plan](DELIVERY_PLAN.md): task IDs, dependencies, deliverables, acceptance and tests. It cannot waive higher-level contracts.
5. [Workflow](WORKFLOW.md): execution, verification, commit and handoff procedure, including the shared task-status vocabulary.
6. README, [Architecture](ARCHITECTURE.md), [Roadmap](ROADMAP.md), [Development](DEVELOPMENT.md) and topic guides: navigation and scoped explanations. They must link to canonical definitions rather than introduce competing requirements.

[Project status](PROJECT_STATUS.md) is the **only authoritative progress ledger**. Delivery-plan status cells are synchronized projections of it. [Next session](NEXT_SESSION.md) is an operational handoff derived from that ledger, not a second backlog. [Baseline audit](BASELINE_AUDIT.md) is dated evidence, not permanently current progress. Git is the source for exact commit identities and remote reachability.

Repository [agent instructions](../AGENTS.md) make the workflow discoverable to new sessions. They summarize operational rules without duplicating technical specifications. Direct authorized user instructions take precedence over repository workflow; record scoped departures and resulting evidence.

## Document labels and versions

Task statuses are the seven uppercase values in [Workflow](WORKFLOW.md). Separately, technical maturity labels mean: **implemented** = executable behavior exists; **experimental** = API/semantics can change; **planned** = no usable implementation claimed. These labels are not task statuses. Experimental code may be tested without being stable or completing a phase.

Public IR, protocol, node/function, worker, snapshot and plugin contracts have independent versions. Draft examples are not published file formats. Package `0.1.0`, Rust `IR_VERSION=1`, wire namespace `v1alpha1`, and protocol helper `1.0` do not imply mutual compatibility or stable release support.

## Conflict and change procedure

1. Capture the conflicting statements, paths and code/test/commit evidence.
2. Identify the scope owner above. Preserve existing behavior until a scoped behavioral task explicitly changes it.
3. For a product/architecture change, record rationale, alternatives, consequences and a superseding ADR before claiming a new standard. For open implementation choices, record experiment and acceptance evidence before marking the choice accepted.
4. Update the canonical document, dependent contracts, plan, status and handoff in the same work item; verify links and tests appropriate to the change.
5. Preserve historical evidence with its date/commit. Mark obsolete guidance **Superseded**, link to its replacement and give the mapping. Do not retain two active phase-numbering systems or silently recycle task/ADR IDs.

An ADR's “accepted” status means direction approved, never implementation completed. A phase completes only through its task gates. New tasks need unique IDs and dependencies; removed tasks retain a tombstone and replacement reference. Outstanding decisions must state the owning task and required validation.

## Commit references without self-reference

A document cannot contain the hash of the commit that contains its own final bytes. Status records explicit code-baseline and evidence/delivery commits. The commit that last changed the status is resolved with `git log -1 --format=%H -- docs/PROJECT_STATUS.md`; current checkout is `git rev-parse HEAD`. These are different concepts and are labeled accordingly. A follow-up closure commit may record a previous evidence commit without rewriting history. Push success is confirmed by comparing local HEAD to `git ls-remote origin refs/heads/main`; do not invent a future SHA or a future push result.
