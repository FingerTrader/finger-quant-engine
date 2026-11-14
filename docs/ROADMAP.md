# Roadmap

**The only active phase/task plan is [DELIVERY_PLAN](DELIVERY_PLAN.md).** Progress is recorded only in [PROJECT_STATUS](PROJECT_STATUS.md). This page is a navigation and migration guide, not a second backlog. The [Charter](PROJECT_CHARTER.md) defines product boundaries; all computation remains local.

## Existing foundation

The original engineering foundation remains intact: five-crate Rust workspace, validated values, experimental price-only IR, one-shot event interpreter, CLI demo, draft protobuf and 17 baseline behavioral tests. See [Audit](BASELINE_AUDIT.md) for source and rerun evidence. This is not completion of Universal Strategy Foundation. Package 0.1.0 is not a stable SDK or published production release.

## Active six-stage route

| Stage | Task index | Required exit |
| --- | --- | --- |
| P1 Universal Strategy Foundation | P1-01–P1-11 | Loaded typed MACD/RSI model, generic expressions/compiled graph, independent goldens and minimal external invocation |
| P2 Event Runtime & State Engine | P2-01–P2-06 | Multi-event strategy, correct ordering/isolation and snapshot/replay parity without future-data leakage |
| P3 Backtesting & Local Simulation | P3-01–P3-07 | Historical data→orders→fills→ledger→report, local optimization and independent financial reconciliation |
| P4 Desktop & External Integration | P4-01–P4-05 | Proven FFI/IPC boundary, actual CLI/Desktop/external parity and optional separate Trading SDK adapter |
| P5 Language Workers | P5-01–P5-05 | Python, then Java, then Go independently accepted with real lifecycle/recovery/isolation tests |
| P6 Plugin Ecosystem & Release | P6-01–P6-06 | Verified third-party load/execute/unload, enforced permissions/resources/fault isolation and Windows/macOS/Linux release evidence |

Each stage depends on the preceding gate. Task descriptions, dependencies, deliverables, acceptance, tests and status live in DELIVERY_PLAN. No dates or feature availability promises follow from this roadmap.

The required product outcome is the [Desk embedded backtest/simulation loop](DESK_INTEGRATION_PLAN.md). P4 delivers actual portable-model Desk integration; P5 integrates Python strategy workers; P6 revalidates the final installed Desk combination. Desk's independent historical P0–P6 numbering is not this engine's stage system. CLI correctness alone does not establish Desk delivery, and the first offline scope does not require live trading.

## Superseded numbering migration

The pre-governance “Phase 0–6” ordering is **Superseded on 2026-11-14** by P1–P6. Preserve its technical intent through this mapping; never interpret an old phase number as a new completion claim.

| Former phase / intent | Active destination |
| --- | --- |
| Phase 0 engineering foundation | Historical baseline in BASELINE_AUDIT; prerequisites, not completed P1 |
| Phase 1 contract/wire validation and price metadata | P1-02/P1-03/P1-11, with full external boundary validation P4-01/P5-01 |
| Phase 2 local replay and accounting | P2 scheduling/replay and P3 data/ledger/fill/report tasks |
| Phase 3 simulation, cancellation and persistence | P2-01/P2-03/P2-04 plus P3 simulation/profiles |
| Phase 4 language workers | P5-01–P5-05; explicit Python → Java → Go order |
| Phase 5 optimization and plugins | Optimization P3-06; plugin registry/isolation/packaging P6 |
| Phase 6 clients and execution adapters | P4 Desktop/external integration and optional Trading SDK adapter |

The old suggestion to start solely with protobuf wire conversion is retired: next work follows the audited dependency order, starting with universal types/parameters after P1-01. Specific serialization, transport, numerical and plugin mechanisms remain evidence-gated decisions in [ADRs](ARCHITECTURE_DECISIONS.md).
