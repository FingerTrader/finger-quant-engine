# Next Session — Repository Recovery

Updated 2026-11-14 (Asia/Tokyo). Derived from [Project status](PROJECT_STATUS.md), the unique progress authority.

## Recover first

Read PROJECT_STATUS, DELIVERY_PLAN, ARCHITECTURE_DECISIONS, README, [Workflow](WORKFLOW.md), [authority rules](DOCUMENT_AUTHORITY.md) and current contracts. Verify actual root, Git status/branch/remote/history and evidence commits before editing. Work on main; preserve unrelated work. Code/tests/Git override stale prose. No toolchain installation is needed: activate the existing pin below.

P1-02 is COMPLETED; P1-03 is VERIFIED pending evidence/closure commit. Record that commit and synchronized closure before marking COMPLETED or starting dependent P1-04. P1 remains incomplete. Evidence/status commits are different: `git log -1 --format=%H -- docs/PROJECT_STATUS.md` resolves the latest record; `git rev-parse HEAD` resolves the actual checkout.

## Delivered P1-03 and verification

Experimental bounded IR2 JSON model import, immutable declarations, raw-source SHA-256/canonical-v1 identity, exact typed numeric values and content-pinned prepared parameters. Sources: fqe-ir model.rs/loading.rs; tests model_contract.rs/loading_contract.rs plus tracked fixtures; example load_model.rs. [Model loading](MODEL_LOADING.md) owns syntax/resource/identity/legacy Desk compatibility. ADR-014 accepts only the model artifact codec. Legacy positive-u64 IR1, evaluator/runtime/CLI and protocol draft remain separate and preserved.

Existing Rust 1.99.0 macOS ARM64: fmt/check/locked offline workspace tests/strict all-target Clippy PASS; **56 behavioral tests, 0 failed/ignored, 0 doc tests**. CLI demo (three events/two observations), parameter example (2/5/rejected 0) and real-file loader example (2/5/null rejected, no execution) PASS. Independent Python json/hashlib canonical/source fixture identities match exact Rust bytes/digests and re-import. Resource/malformed/duplicate/version/type/unit/range/required/default/null tests pass. Full evidence and latest document-check counts are in PROJECT_STATUS.

Owner approved dependencies/build scripts/public-only advisory queries. [Dependency review](P1_03_DEPENDENCY_REVIEW.md) records serde 1.0.228, serde_json 1.0.145, sha2 0.10.9 and 21 resolved registry packages, all archives verified, metadata/scripts/current owners inspected and OSV no matching advisories on review date. Cargo.lock is aligned. Feature graph: 16 active registry packages on this target; derive/asm disabled. No private code/manifests/graphs uploaded. Existing cache enables offline builds; no customer-installed runtime is introduced. Other targets/security coverage remain unverified.

Evidence commit pending. Changes are still uncommitted at this record; finish WORKFLOW closure. Remote main last read-confirmed at `0ac5d7b163e77bddb5f69689a1455e3560aad872`; six pre-P1-03 local commits remain unpushed. No GitHub publishing requested. No Desk source/data/run changed. P1-02 evidence `a653c105fba5ea6ada15395abc22cc8f82cc4472` and closure `af0c65b844a4620382926580bacf02a0bcfb76ed` remain historical; do not reimplement them.

## Next task: P1-04 — Node, Function & Provider Registries

Begin after P1-03 COMPLETED. Read [Extensibility](EXTENSIBILITY_CONTRACT.md), [P1 design](P1_STRATEGY_DESIGN.md), [Type/parameters](TYPE_PARAMETER_CONTRACT.md), [Model loading](MODEL_LOADING.md) and plan acceptance.

- Deliver shared versioned node/function/provider descriptors, typed named ports, parameter contracts, generic in-process provider interface and deterministic registry identity. Built-ins and independently defined reviewed test providers use the same registration/invocation path.
- Prove X-001 unknown-name function and Boolean predicate injection without a per-name core/compiler branch. Registry lookup pins namespaced ID/version/signature and artifact identity; model import currently checks declarations only. Define consistent binding to the loaded entry pins.
- Test duplicate/version/type/arity/capability failures, multi-input/multi-output descriptors, unsupported scalar/series/window/record/collection shapes and deterministic registration-order-independent identity. Reject unsupported semantics explicitly rather than accepting opaque types.
- Keep implementations independently usable offline without Flutter/login/account. Do not introduce external loading, workers/plugins, compiler redesign, indicator formulas, workflow/state, accounting/simulator or Trading SDK implementation. Those have later task owners.
- Update task acceptance/status/contracts/ADRs/tests/handoff, run checks and close with evidence commits. P1-05 cannot bypass incomplete P1-04. Preserve the complete 42-task graph including P5-05.01 and P6-03.01.

Expected edit scope: scoped fqe-ir registry/provider modules, descriptors, tests/examples and required model integration, plus canonical contracts/progress. fqe-core changes only if demonstrated necessary. Any added dependency follows the supply-chain skill and the existing authorization scope; new unfamiliar installers/native downloads/generators need their own review/approval.

Protected: LICENSE/history, private datasets/runs/secrets, local-toolchain installation, unrelated dirty work, Flutter/Trading SDK/external repos and existing legacy demo semantics. No worktree/extra branch/reset/force-push. Publishing requires session authorization.

## Commands

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
cargo run --locked --offline -p fqe-ir --example load_model
python3 scripts/check_docs.py
git diff --check
```

Add actual task-specific acceptance commands; record real outputs/counts, not a copied prior pass. At closure inspect scoped staged diff, commit evidence, synchronize COMPLETED/handoff, record closure commit and inspect remote main. A local commit is not a push receipt.

## Product target and unresolved risks

Desk historical backtesting/local simulation is the first delivery; live trading remains separately accepted. Deliver FQE and supported runtimes within the existing Flutter Desk package so customers install once. Standalone/offline engine remains independent; Trading SDK owns broker connectivity. P4 native boundary, P5 Python→Java→Go and fourth-language proof, P6 external Java-indicator/Go-predicate graph and signed installed bundle acceptance are future work. Engine support and distributable customer-platform support are separate matrices.

The existing `/Users/george/Desktop/finger_desk` has a dated dirty-tree inventory and Dart/Python financial/hosted paths in [Desk integration](DESK_INTEGRATION_PLAN.md). Protect them; use a fresh pinned paired candidate when integration begins. Legacy visual models need explicit conversion/new content identity; historical runs/data retain backend identity/read paths. JSON import is not proof of paired native execution or financial equivalence.

Remaining gates include registry/signature/cycle validation, indicator precision, event/history semantics, state restore, independent financial correctness, worker/plugin execution/isolation and actual platform/channel packaging. Before interruption record actual dirty/staged/unpushed state, last command/errors and recovery steps; never label unverified work COMPLETED.
