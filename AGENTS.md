# Finger Quant Engine agent instructions

This independent repository is the home for FQE core development.

## Recover every session

- Before editing, read `docs/PROJECT_STATUS.md`, `docs/NEXT_SESSION.md`, `docs/DELIVERY_PLAN.md`, `docs/ARCHITECTURE_DECISIONS.md`, then current task contracts. Follow `docs/WORKFLOW.md` and authority rules in `docs/DOCUMENT_AUTHORITY.md`.
- Check Git status, branch/remote and actual code/evidence commits. Code, executed tests and Git history override stale status records; repair records first. Do not reimplement a completed verified task or bypass incomplete dependencies.
- Select the first eligible incomplete task. Update acceptance, status, tests and handoff during work and before interruption. IMPLEMENTED is not VERIFIED; COMPLETED requires all closure conditions in WORKFLOW.
- At task closure, synchronize plan/status/handoff, record an evidence commit, push normally to GitHub main when authorized, and verify the remote SHA. Never claim a push or test that did not succeed.
- Run `python3 scripts/check_docs.py` and `git diff --check` alongside the required Rust checks. Do not complete P1 from documentation planning alone.

## Engineering boundaries

- Read README.md and the relevant docs/ contracts before changing behavior.
- Preserve Local-First and Language-Neutral boundaries. All strategy computation runs on the user's device.
- Offline functionality must work without Flutter, Finger Trader login, or a trading account.
- Trading APIs belong to the independent Finger Trader Trading SDK. Do not embed broker credentials or SDK implementations here.
- Distinguish implemented, experimental, and planned functionality in code-facing documentation.
- Do not expand the initialization demo into accounting, a simulator, workers, or plugins without a scoped task.
- Use the pinned Rust toolchain. Keep Cargo.lock committed and aligned with manifests.
- Before adding or executing third-party packages, inspect provenance, license, scripts, release age, and advisory information. Ask before running unfamiliar installers, native downloads, or code generators. Do not upload private code, manifests, or dependency graphs to external scanners without approval.
- Do not overwrite unrelated work. Work on main for this initialization; do not create a worktree or another branch.
- Required checks: cargo fmt --check; cargo check --workspace; cargo test --workspace; cargo clippy --workspace --all-targets.
- Never force-push or discard remote commits.
