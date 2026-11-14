# Next Session — Repository Recovery

Updated 2026-11-14 (Asia/Tokyo). This handoff is derived from [Project status](PROJECT_STATUS.md), the only progress authority. No prior chat is needed.

## Resume sequence

1. Read PROJECT_STATUS, this file, [DELIVERY_PLAN](DELIVERY_PLAN.md), [ARCHITECTURE_DECISIONS](ARCHITECTURE_DECISIONS.md), then [P1 design](P1_STRATEGY_DESIGN.md), [Strategy contract](STRATEGY_CONTRACT.md) and [Baseline audit](BASELINE_AUDIT.md). Also read README, AGENTS and [Workflow](WORKFLOW.md).
2. Verify cwd is the actual desktop checkout `/Users/george/Desktop/finger-quant-engine`; inspect Git status, branch/remotes and recent commits. Work on main; no second project/worktree/branch. Compare baseline `342d8aa76b48d32fa9c9110609b371eac52b8d86` and recorded governance evidence with actual history. `git log -1 --format=%H -- docs/PROJECT_STATUS.md` identifies latest status commit without a self-referential hash.
3. Governance/audit evidence is committed as `f07fe8ccd4c12b46783b67ff0a2421172d05d5dc` and its normal GitHub main push was confirmed by remote SHA. Check current closure commit/remote status as well; if code/tests/history disagree with records, repair status/handoff from facts first. Never infer future push success from this historical receipt.
4. P1-01 is COMPLETED and P1-02 is READY. Set P1-02 IN_PROGRESS in status and plan before scoped changes. Do not repeat the completed audit as a new feature task unless facts have changed.

## Work completed in governance session

Inspected all five crates/manifests, lockfile, README, all 12 existing docs, tests and Git history. Reconfirmed 17 behavioral tests, formatting/check/strict Clippy and CLI baseline. Added charter, authority, ADR register, six-stage plan, P1 target design, status, handoff and workflow, plus audit evidence and a standard-library document checker. Updated old roadmap/navigation/transport wording and agent recovery entry points. No Rust source, dependencies, draft schema or license changed. P1 remains incomplete.

Current stop: governance and audit are COMPLETED, with all checks and a confirmed GitHub evidence commit. The final closure record uses that existing evidence hash; resolve its containing commit from Git. P1-02 is READY; implementation has not begun. Exact current committed/delivery state is in PROJECT_STATUS and Git.

## Next task: P1-02 — Universal Types & Parameter Schema

Deliver an evaluated numeric/type contract and a small executable validated domain/parameter layer, not the rest of the engine. Start from actual `fqe-core` positive-u64 types and `fqe-ir` price comparison; preserve existing experimental behavior until an explicit contract revision and tests justify change.

Acceptance:

- Compare viable numeric representations for price/ratio/indicator and future money; record precision/range/rounding/overflow and signed/zero-price decisions. Do not mark the final financial ledger implementation complete.
- Define a versioned implemented type subset with explicit units/domain and Missing/Warmup/Error semantics; reject unsupported types/conversions rather than silently accepting them.
- Validate parameter schema and bindings: required, typed defaults, ranges/enums, duplicate/unknown keys, invalid schema defaults and wrong types/units. Instance bindings are immutable for a run.
- Add meaningful boundary, overflow, invalid-schema/binding and regression tests. Four Rust checks and docs checker pass; contracts, plan, status, handoff and evidence commit updated.

## Expected edit scope

- `crates/fqe-core/src/lib.rs` (or scoped new modules) for validated type/numeric primitives.
- `crates/fqe-ir/src/lib.rs` (or scoped new modules) for parameter schema/binding and tests.
- `docs/P1_STRATEGY_DESIGN.md`, `docs/STRATEGY_CONTRACT.md` and, only if affected, event/state contracts.
- `docs/ARCHITECTURE_DECISIONS.md` for an evidenced numeric decision; `docs/DELIVERY_PLAN.md`, `docs/PROJECT_STATUS.md`, `docs/NEXT_SESSION.md` for progress and recovery.
- Cargo manifests/Cargo.lock **only if a justified dependency change is reviewed**. Prefer existing standard-library capabilities for this slice; do not assume a new numeric library is authorized without review.

Protected/out of scope: LICENSE and existing history; `.local-toolchain/`, ignored private datasets/runs/secrets; unrelated dirty work; Flutter/Trading SDK/external repositories; worker/plugin/simulator/accounting implementations; schema code generation or broad CLI/runtime redesign. Do not install tools because default PATH lacks Rust. No force push, reset or extra worktree/branch. Minimal consumers may be adapted only if the current type change requires it, with evidence.

## Commands

Activate the already installed pin (no installer):

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
python3 scripts/check_docs.py
git diff --check
```

Run task-specific numeric/schema tests too. Baseline evidence was 17 tests and CLI demo events=3/signals=2; new meaningful tests will change the count. Record current results rather than copying that number. Before/after push compare `git rev-parse HEAD` and `git ls-remote origin refs/heads/main`. Follow WORKFLOW closure sequence.

## Unresolved risks

Numeric and external format decisions are unvalidated; new packages/generators require supply-chain checks. Current `.proto` and IR_VERSION=1 are experimental. Data leakage, multi-timeframe order, state restoration, independent ledger correctness, FFI/IPC and actual worker/plugin isolation have future gates. No other OS or real Desktop has been verified. If interrupted, preserve actual dirty files, last result/error, pending checks, unpushed SHA and recovery step here and in status; never mark unverified work COMPLETED.
