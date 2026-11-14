# Finger Quant Engine agent instructions

This independent repository is the home for FQE core development.

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
