# P1 strategy foundation acceptance

Date:2026-11-14, macOS ARM64. Task: P1-11. Full gate verification PASS; [Project status](PROJECT_STATUS.md) owns task closure/commit evidence. This report covers the experimental bounded scalar strategy foundation, not production trading or completion of the whole project. [P1 design](P1_STRATEGY_DESIGN.md) and [Extensibility](EXTENSIBILITY_CONTRACT.md) remain normative for requirements.

## Reproduce the gate

Activate the existing [pinned toolchain](DEVELOPMENT.md#project-local-toolchain), then run:

```sh
export RUSTUP_AUTO_INSTALL=0
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check_p1_gate.py
```

The [stdlib gate runner](../scripts/check_p1_gate.py) executes fmt, locked/offline workspace check/tests, strict all-target Clippy, CLI build, independent caller tests, actual process goldens, document checker and diff check. It requires the existing reviewed toolchain/cache; it never installs dependencies or downloads runtimes. Progress goes to stderr; stdout is a version1 `fqe.p1-gate-report` JSON document with status/check exits/counts/case outcomes/identities. Save that output outside tracked source if desired. Any failed command, fixture, pin or comparison makes status failed and exit1. `--skip-checks` requires a built engine, executes process fixtures only and reports **partial**, never a full gate pass. Gate verification does not itself mark a task COMPLETED or commit/push it.

The versioned [suite inventory](../crates/fqe-runtime/tests/fixtures/p1/suite.json) pins source/fixture bytes and names each acceptance. The runner checks source SHA-256 pins before execution and confirms novel test provider/rule IDs do not occur in production Rust source. Pinned hashes establish reproducibility, not authorship, binary authenticity or the truth of financial input data. Expected files are never generated or rewritten by tests or the gate runner.

## Golden coverage

| Specification | Executed evidence and result |
| --- | --- |
| G-001 strict price comparison | [P1 runtime gate](../crates/fqe-runtime/tests/p1_gate.rs) feeds prices90/100/110/105 from the versioned data file to both legacy and loaded event paths; signals exactly3/4. Actual CLI/external process case uses the same data and expected sequences. |
| G-002 registered indicators | [Independent rational indicator fixtures](../crates/fqe-ir/tests/fixtures/indicators.json), [indicator tests](../crates/fqe-ir/tests/indicators.rs) and [numeric evaluation](P1_06_NUMERIC_EVALUATION.md) retain nine datasets/563 samples/eight series, seed/readiness and precision evidence. No financial implementation or old golden is changed in P1-11. |
| G-003 loaded MACD/RSI rule | Shared [financial golden table](../crates/fqe-runtime/tests/fixtures/p1/financial.expected.tsv), loaded runtime tests and process cases verify cross-above AND RSI>60: signalbar5; threshold90 and changed model default90: none. Model/parameter files change without rebuilding the engine. |
| G-004 library/CLI/external parity | [Actual CLI tests](../crates/fqe-cli/tests/loaded_execution.rs) compare full bytes with the public shared library report adapter, using the same reference catalog. [Independent caller tests](../tests/external/test_caller.py) and this gate compare complete parsed CLI/external JSON and deterministic repeated caller bytes. Event results/terminal status/source hashes remain exact. |
| G-005 invalid artifacts | Twelve tracked [invalid expectations](../crates/fqe-runtime/tests/fixtures/p1/invalid.expected.tsv) assert stage/code/path with zero provider invocations. Earlier [loading](../crates/fqe-ir/tests/loading_contract.rs), [compiler](../crates/fqe-ir/tests/compiler_contract.rs), [event-plan](../crates/fqe-ir/tests/event_plan_contract.rs) and CLI tests additionally cover duplicate/truncated/oversized input, required/unsupported capabilities, parameter/type/unit/port errors and bounds. No plan is returned; no runtime instance is created by compilation. Trusted relationship validators may execute and are not sandboxed. |
| G-006 temporal workflow | Deferred to P2-06 by the original specification. Latch/wait/expiry/snapshot tests are not claimed by P1; no workflow semantics or acceptance was weakened. |
| G-007 SMA crossing | Original rational SMA2/SMA3 expectations and the financial signal/readiness table verify one cross atbar5, no fabricated cross at first-ready bar3. Actual loaded runtime/CLI/external cases all execute it. Installed Desk visual compatibility remains P4. |
| X-001 open function/predicate/rule composition | New [reviewed test-only providers](../crates/fqe-runtime/tests/support/gate_providers.rs), two loaded event artifacts, reused rule module, explicit bindings and [manual exact table](../crates/fqe-runtime/tests/fixtures/p1/x001.expected.tsv) run through public registry/compiler/EventRuntime APIs. They prove generic composition at the supported in-process boundary. |

The manifest is an index over the existing tests plus added P1 acceptance, not a replacement for those tests. Full workspace verification executes all of them. No historical implementation count is treated as a new successful run.

## Independent calculation and policy notes

Financial expected values are inherited from the normative rational tables and previously reviewed Fraction-based reference fixtures, not extracted from current engine output. G-003 closes1/2/3/2/4/3 give RSI2 values W/W/100/50/250÷3/50. At output scale12 these ready coefficients are100000000000000,50000000000000,83333333333333,50000000000000. MACD(2,3,2) has its first complete line/signal pair atbar4, so the earlier missing pair cannot generate a cross; bar5 has positive histogram1/54 and RSI250/3>60. Threshold90 rejects that event. G-007 SMA2 values W,5/2,3/2,3/2,5/2,7/2 and SMA3 W,W,2,5/3,2,3 yield the specified first-ready and crossing behavior.

Indicator policy remains checked i128 decimal, work scale18/output12, explicit HalfEven rounding, exact readiness and coefficient tolerance≤1 for the independent broad indicator oracle. The new small financial root table uses explicit exact expected coefficients for these rational cases. Signals, provenance, Boolean decisions and terminal status have zero tolerance. This is not an unbounded rational implementation or a universal error bound; the [indicator contract](INDICATOR_CONTRACT.md) retains period/domain/precision rejection limits.

X-001 uses exact checked u64 arithmetic with no float/rounding/history:

- `acceptance.shift(sample, increment)` produces sample+increment, rejecting overflow.
- `acceptance.window(sample, lower, upper)` produces lower<sample≤upper, with lower<upper checked during binding.
- The reusable `reviewed.window_rule` module composes shift then window and exposes accepted/shifted outputs. Two instances have separate bound parameters. Root decision is first.accepted AND NOT second.accepted.
- Prices are90/95/100/101/105/110/120. First instance has increment10, lower100, upper120; second has increment0, lower105, upper110. First transformed values are100/105/110/111/115/120/130. Second accepts only sequence6; decision signals are2/3/4/5.
- The changed artifact uses first increment0; upper120 remains inclusive, so its signals are4/5/7. The changed binding uses first lower108 with increment10; its signals are3/4/5. These hand-derived rows are stored explicitly in the versioned table. Tests do not calculate their own expected financial or provider outputs.

The new provider source is independently reviewed test code, pinned by [source identity](../crates/fqe-runtime/tests/fixtures/p1/provider-source.sha256) and checked by the runner. The registration pin is a declared artifact identity, not runtime binary attestation. Test counters only observe provider invocation; no result depends on them. Compilation and invalid-artifact tests require zero invocation; trusted cross-parameter validation is allowed under the existing provider contract. Existing built-in/indicator/earlier independent-provider source pins and canonical fixtures are preserved.

## Registry and execution boundaries

The existing CLI uses its explicit reference catalog. X-001 runs with a public custom in-process registry in the library tests. Both use the same generic compiler/evaluator; no production code contains the new provider/rule IDs. The gate deliberately invokes X-001 with the stock CLI and requires registry_unknown before execution. This makes the unsupported discovery/loading boundary observable instead of silently adding test-specific CLI registration. External extension loading and Java/Go/Python workers remain P5/P6 work.

The new library tests also verify repeat runs, prefix invariance, source-formatting versus semantic identity, changed bindings/artifacts, shared module expansion/source paths and a late custom-provider overflow with unchanged event cursor/counts plus successful corrected retry. Existing event tests cover multi-node/crossing atomic rollback, fresh run state, warmup, missing resets, false guards, domain/order rejection and late root/fork failures. CLI/external batch failure returns no successful partial result and never retries automatically.

Eleven actual process cases cover G-001, G-003 default/explicit60/90/changed default90, G-007, full-width ticks, explicit empty0, late numeric failure, unavailable custom catalog and bad schema version. Child execution uses absolute paths, a separate temporary working directory and PATH=/nonexistent; no development tool/interpreter is needed by the compiled engine. The optional Python reference consumer retains its POSIX/trusted-executable/byte/deadline limits from [external invocation](EXTERNAL_INVOCATION.md).

## Dated verification and remaining work

Verified on the pinned Rust/Cargo1.99.0 and installed Python3.14.6: full runner status passed,181 Rust behavioral tests and23 independent caller tests, zero failures/ignored/skips. Eleven actual process cases,38 source/fixture pins and generic-dispatch source review PASS. fmt/check/strict all-target Clippy/locked-offline CLI build/docs/diff PASS. CLI demo, all six fqe-ir examples and runtime events example also executed successfully. Document checker:37 Markdown files/42 stage tasks with valid local links/anchors and status/dependency consistency. Exact implementation/closure SHA and delivery status belong in [Project status](PROJECT_STATUS.md).

Initial test-only expectation issues were resolved: inclusive upper bound120 in the shift0 table, registry-owned overflow diagnostic and exact self-cycle node source path. The initially type-confounded cycle fixture was replaced by a typed self-cycle. Final full verification is clean. No production evaluator or indicator change was used to fit a golden.

No dependency, manifest, Cargo.lock, toolchain, license or production Rust source change is needed. This task reuses the approved reviewed cached packages with locked/offline commands. No new scanner/advisory query, private upload, installer, binding generator or runtime download is part of the gate. Verification remains scoped to the tested platform/profile and trusted local providers; unknown vulnerabilities, hostile extensions and production-volume limits are not certified.

P1 closure makes P2-01 eligible. Generalized state/workflow, snapshots, multi-market scheduling and temporal rules still need P2. Accounting, historical backtest/simulation and optimization need P3; actual Flutter/Desk native integration and bundling need P4; language workers/onboarding and external plugins need P5/P6. No customer-ready Desk package, live trading path or complete quantitative product is delivered by passing P1.
