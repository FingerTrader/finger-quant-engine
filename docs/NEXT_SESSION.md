# Next Session — Repository Recovery

Updated 2026-11-14 (Asia/Tokyo). Derived from [Project status](PROJECT_STATUS.md), the unique progress authority.

## Resume sequence

1. Read PROJECT_STATUS, this file, [Delivery plan](DELIVERY_PLAN.md), [ADRs](ARCHITECTURE_DECISIONS.md), [Type/parameter contract](TYPE_PARAMETER_CONTRACT.md), [P1 design](P1_STRATEGY_DESIGN.md), [Strategy](STRATEGY_CONTRACT.md), [Extensibility](EXTENSIBILITY_CONTRACT.md), [Desk integration](DESK_INTEGRATION_PLAN.md), README, AGENTS and [Workflow](WORKFLOW.md).
2. Verify actual cwd `/Users/george/Desktop/finger-quant-engine`, main/upstream/remotes, dirty files and recent commits. No other worktree/branch. Git/code/executed tests override stale records. `git log -1 --format=%H -- docs/PROJECT_STATUS.md` resolves the latest status commit without self-reference.
3. Historical source baseline is `342d8aa76b48d32fa9c9110609b371eac52b8d86`; governance evidence is `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc`; its confirmed push is historical only. GOV-02/GOV-03 are completed local planning refinements; their evidence and previous remote receipt are in PROJECT_STATUS. Do not infer a new push from them.
4. P1-02 is VERIFIED, with local evidence commit/closure pending. Do not start P1-03 before closure. Recover dirty files and actual last commands if interrupted; never repeat a completed implementation as a fresh task.

## Current P1-02 result and delivery

Implemented checked i128 decimal (scale 0–18; explicit Exact/TowardZero/HalfEven rounding; checked result/intermediate overflow), validated scalar/unit descriptors and readiness, immutable parameter schemas/bindings. Source: fqe-core/src/numeric.rs and types.rs; fqe-ir/src/parameters.rs. Public tests: fqe-core/tests/value_contract.rs and fqe-ir/tests/parameter_contract.rs. Offline executable example: fqe-ir/examples/parameters.rs. TYPE_PARAMETER_CONTRACT and ADR-013 record the bounded choice and remaining gates.

All four required Rust checks passed, including strict all-target Clippy and 36 behavioral tests (17 original + 19 new), zero failed/ignored. An earlier Clippy style finding was corrected before the complete final suite passed. CLI demo remains three events/two signals; parameter example demonstrates default 2/override 5/rejected 0. Document checker and whitespace/preservation checks passed; exact counts are in PROJECT_STATUS. Full intended diff reviewed. Evidence commit/closure pending; no GitHub publication requested. Legacy positive PriceTicks/IR_VERSION=1/evaluator/CLI behavior, manifests/lockfile/toolchain/license remain preserved. No new dependency/tool install, external source edit or Desk run.

P1 remains incomplete. The 42-task plan has 40 original parents and two extension acceptance children. No indicator, compiler, loader, scheduler, ledger, worker/provider bridge, plugin loader or FQE/Desk integration is delivered by these primitive APIs. The scalar requests for unsupported shapes/types are rejected, not advertised as implementations. Working precision/tolerances and financial rounding remain P1-06/P3-01 work.

## Next task after closure: P1-03 — Versioned Definition & Model Loading

Deliver an evaluated bounded external-format/model-loading slice that consumes the P1-02 APIs. Do not expand directly into compiler execution, workers or accounting.

Acceptance:

- Evaluate the external encoding and bounded parse/canonical identity approach; record an evidenced ADR before declaring a codec selected. No Protobuf generator/dependency is implicitly authorized.
- Define immutable portable definition/version/content identity versus instance/bindings/runtime state. Represent graph/module/descriptor/requirements declarations with validated identities and bounds; registry semantics/graph compilation remain P1-04/P1-07 work.
- Preserve full-width decimal coefficient/scale, exact u64 and unit identity without float conversion. Repeat P1-02's required/default/absence/null/type/unit/range/enum behavior through the actual decoder. Detect duplicate keys before map conversion loses them.
- Reject missing/unsupported versions, types, shapes and required capabilities, malformed/truncated/oversized/deep input and duplicate IDs with stable diagnostics; do not execute rejected definitions.
- Define the existing Desk visual/hosted schema compatibility dispositions from the dated DESK_INTEGRATION_PLAN inventory; do not reinterpret old dual-SMA models or rewrite Desk records.
- Test actual valid/invalid external fixtures and round-trip/canonical identity, independent expected values and input limits. Run required checks, update contracts/plan/status/handoff and record evidence commit. No new language/plugin/financial support claim.

Expected edit scope: scoped fqe-ir definition/loading modules and fixtures/tests; fqe-protocol only as the evaluated mapping requires; fqe-core only for demonstrated necessary support. Update relevant type/strategy/P1/architecture contracts and governance progress. Cargo manifests/lockfile only after a justified dependency review; default to the existing toolchain and reviewed capabilities.

Protected/out of scope: LICENSE/history, private datasets/runs/secrets, .local-toolchain installation, unrelated dirty work, Flutter/Trading SDK/external repositories, registry/compiler/evaluator redesign, workers/plugins/simulator/accounting and implicit code generation. Keep all computation local and offline. No reset/force-push/extra branch/worktree. Publishing requires session authorization.

## Commands

Activate the existing pin, with no installer:

```sh
export CARGO_HOME="$PWD/.local-toolchain/cargo"
export RUSTUP_HOME="$PWD/.local-toolchain/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
export RUSTUP_AUTO_INSTALL=0
rustc --version
cargo fmt --check
cargo check --workspace --locked --offline
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo run --locked --offline -p fqe-cli -- demo
cargo run --locked --offline -p fqe-ir --example parameters
python3 scripts/check_docs.py
git diff --check
```

Run new loader-specific checks too; record actual new counts, not a copied baseline. At closure follow WORKFLOW; inspect remote main before publishing and preserve remote changes. A local completion receipt is not a push receipt.

## Product target and unresolved risks

First product scope is Desk historical backtesting/local simulation; live trading is separately accepted later. The engine is to be delivered in the existing Flutter Desk package with supported runtimes so customers install Desk once, without an extra engine/interpreter setup. P4 actual native integration, P5 reference languages/fourth-adapter proof and P6 external Java-indicator/Go-predicate graph/final signed installed bundle remain future acceptance. Engine conformance and customer-bundle/platform support are separate matrices.

The existing `/Users/george/Desktop/finger_desk` has a dated dirty-tree inventory and existing Dart/Python financial/hosted paths in DESK_INTEGRATION_PLAN; protect that work and use a freshly pinned paired candidate before integration. Desk owns presentation/drafts and FQE owns computation/financial truth per run; preserve historical records and explicit backend identity. Its P0–P6 labels are independent of FQE stages. No native FQE/Desk run has been verified.

Model codecs/identity, indicator precision, event alignment/leakage, state restore, independent financial correctness, worker/plugin isolation and actual OS/customer-channel support remain open. Before interruption record real dirty/staged/unpushed state, last result/error and exact pending recovery in status/handoff; never label unverified work COMPLETED.
