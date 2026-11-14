# Next Session — Repository Recovery

Updated2026-11-14 (Asia/Tokyo); derived from [Project status](PROJECT_STATUS.md).

## Recover first

Read PROJECT_STATUS, DELIVERY_PLAN, ARCHITECTURE_DECISIONS, README, [Workflow](WORKFLOW.md), [authority](DOCUMENT_AUTHORITY.md), then current contracts. Verify actual Git status/main/upstream/history/evidence; code/tests/Git override stale records. Main only, preserve unrelated work/history. User authorizes downstream implementation without routine confirmation; no publishing requested. Old push receipts are not authorization.

P1-01–P1-11 COMPLETED; P1 phase gate passed; P2-01 READY. P2–P6 remain incomplete. Resolve HEAD with `git rev-parse HEAD` and latest status commit with `git log -1 --format=%H -- docs/PROJECT_STATUS.md`. Reuse the project-local pinned toolchain/cache; no installation needed.

## Latest implementation and checks

P1-11 implementation evidence **949a286df391c5bc23eebd5b2e11dfb19268d390**, COMPLETED with synchronized local closure; P1 gate passed. No push requested/performed.

P1-11 [acceptance](P1_ACCEPTANCE.md): versioned suite/index/38 pins, shared exact financial/provider/diagnostic tables,12 invalid artifacts, six loaded runtime gate tests and stdlib unified checker. New unknown-name checked-u64 function/predicate compose in a reusable rule instantiated twice; model/binding edits change exact signals through public generic APIs. No production core/CLI name branches or edits. Compilation/invalidation performs zero provider invocations, numeric failure/retry preserves committed event state, deterministic/prefix/identity/source tests pass. Stock CLI rejects unregistered X-001 deliberately; custom library catalog is explicit, not an external loader.

Verification2026-11-14 macOS ARM64, Rust/Cargo1.99.0, Python3.14.6: full gate status passed/exit0;181 Rust tests +23 independent external tests +11 real process scenarios PASS; fmt/check/strict all-target Clippy/locked-offline build, CLI demo/all six fqe-ir examples/runtime events PASS. Independent G-001–G-005/G-007/X-001 expectations, exact CLI/library/external parity within supported registries, changed model/parameters and repeated bytes/source IDs PASS. Docs checker37 Markdown/42 tasks/links/anchors/status DAG PASS; diff PASS. Initial new-test bound/diagnostic/source-path expectations were corrected; no outstanding failures. G-006 remains P2-06.

No new dependency/manifests/lockfile/toolchain/production Rust/provider-old-source/golden changes. Approved reviewed cached packages only; no new advisory scan/installer/private upload/runtime download. New gate-only provider source4a8427ed8f4936e78f96e583f422c5577f2d4355042087c85da6a1df6cf5e733 is pinned; prior protected pins preserved. No Desk/SDK/accounting/worker/external repository writes. No sandbox/production-volume/platform expansion claim.

Previous evidence: P1-10 `5c7a7f6cdd1b0fe59bda7d740444da4f5299dcc3`, P1-09 `9dbac2b14e85298c18131646219a178a4ef8ddea`, P1-08 `cea3f92df2027743711e73a84927396417689042`. Do not reimplement completed verified tasks.

## Next implementation: P2-01 — Stateful Workflow and Lifecycle

P1-11 is closed. First eligible task is P2-01. Read [state](STATE_CONTRACT.md), [runtime](RUNTIME.md), [events](EVENT_CONTRACT.md), [compiled events](EVENT_EXECUTION.md), [P1/P2 design](P1_STRATEGY_DESIGN.md), [extensibility/state](EXTENSIBILITY_CONTRACT.md), [compiler](GRAPH_COMPILER.md), plan row and actual state/provider/event code. Mark IN_PROGRESS with a concrete scoped design/acceptance before edits; this task is substantially broader than the completed pure P1 graph.

- Define typed strategy-state declarations and bounded workflow/State/Delay semantics with explicit schema/capabilities. Preserve existing artifacts and old evaluator/lifecycle; unsupported future constructs must fail rather than silently change meaning.
- Separate immutable definition from instance-owned state. Guards read committed old state and current event outputs; writes become visible only at successful event commit. Specify deterministic transition/guard/effect ordering and reject conflicting writes/types/cycles. Delay is an explicit prior-event boundary, never permission for an ordinary same-event cycle.
- Implement generic in-process X-002 stateful extension/rule instance isolation and lifecycle. Make portable state-schema/version/identity and unsupported snapshot/restore capabilities explicit. P2-04 owns durable snapshot codecs/restore acceptance; do not claim persistence from deep-fork memory or arbitrary language-object serialization.
- Verify exact guard/effect old/new behavior, stop/error/restart semantics, failed-event all-state/history/cursor rollback and isolated repeated instances. Preserve once-per-event subscriptions, numeric/readiness rules, source diagnostics and existing P1 goldens. Stop semantics need a contract; P1 finish is currently terminal and failed step retryable, while legacy run errors are terminal Failed.
- Keep scheduler/multi-market alignment(P2-02), generalized graph/state execution(P2-03), durable snapshots(P2-04), metrics(P2-05) and G-006 temporal full-gate acceptance(P2-06) in their owning tasks. Scope/dependency overlaps should be made explicit before implementation, not bypassed or declared done from this handoff.

No speculative accounting, SDK, Desk UI, external language runtime/loader or new dependencies required by this handoff. If new wire/state capabilities are needed, version them and record actual decision/evidence. Preserve protected provider pins and old canonical/golden files unless a concrete defect requires reviewed identity changes.

## Commands

```sh
export CARGO_HOME="$PWD/.local-toolchain/cargo"
export RUSTUP_HOME="$PWD/.local-toolchain/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
export RUSTUP_AUTO_INSTALL=0
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p1_gate.py
cargo test --locked --offline -p fqe-runtime --test p1_gate
python3 scripts/check_docs.py
git diff --check
```

The full runner executes the four required Rust checks/build,23 external tests and11 actual process cases. `--skip-checks` reports partial only. It never rewrites expected files. Record actual failures/dirty work before interruption. Evidence first, then three-document closure with its SHA, no self-hash. Publishing requires authorization, normal push and confirmed remote SHA. Last remote main read-confirmed `0ac5d7b163e77bddb5f69689a1455e3560aad872`; latest implementation evidence `949a286df391c5bc23eebd5b2e11dfb19268d390`, main24 commits ahead after this closure. P1-11 work is committed locally; verify live HEAD/count/dirty work before action. Recheck live before action.
