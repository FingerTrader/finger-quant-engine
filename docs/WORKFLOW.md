# Development Workflow

Applies to each Codex session and developer handoff. [Document authority](DOCUMENT_AUTHORITY.md) defines canonical owners; [Project charter](PROJECT_CHARTER.md) and [ADRs](ARCHITECTURE_DECISIONS.md) constrain scope. All core development stays in this repository.

## Recover before editing

1. Read [Project status](PROJECT_STATUS.md), [Next session](NEXT_SESSION.md), [Delivery plan](DELIVERY_PLAN.md), then ADRs and current task contracts. Read README and repository agent instructions. Do not rely on previous chat history.
2. Verify actual path, `git status --short --branch`, branch/upstream/remote, `git log -5 --oneline` and `git rev-parse HEAD`. For this initialization work use main, no extra branch/worktree. Preserve unrelated dirty work and all remote commits.
3. Compare code baseline/evidence commits in status to actual history. A later documentation commit is not a code discrepancy. Verify referenced artifacts/tests still exist; use code/tests/Git facts to repair stale records before new implementation. Do not reimplement a completed, verified task.
4. Select the first incomplete numeric task whose dependencies are COMPLETED. If a task is already IN_PROGRESS, recover its actual partial state first. Mark READY then IN_PROGRESS with its scope, acceptance and expected tests in status/handoff; synchronize plan status cells.

## Task status semantics

| Status | Required meaning |
| --- | --- |
| NOT_STARTED | No task execution recorded; prerequisite work may still be needed. |
| READY | All dependencies COMPLETED; acceptance and scope understood; no work begun. |
| IN_PROGRESS | Active scoped changes; record dirty/partial work and pending checks. |
| BLOCKED | A concrete dependency/tool/test/external condition prevents progress; record evidence and resolution, never hide failure. |
| IMPLEMENTED | Required code/document artifacts exist; acceptance or verification is still pending. |
| VERIFIED | All specified acceptance and tests passed with actual evidence; closure bookkeeping/commit is still pending. |
| COMPLETED | Artifacts delivered, all acceptance met, tests passed, docs synchronized, evidence Git commit recorded, PROJECT_STATUS and NEXT_SESSION updated. |

Normal flow: NOT_STARTED → READY → IN_PROGRESS → IMPLEMENTED → VERIFIED → COMPLETED. BLOCKED can interrupt work; resolve to the actual prior state after fixing the cause. New failures regress the status with an explanation. Do not confuse task status with runtime `Completed`, ADR ACCEPTED or technical maturity labels. READY is not proof of implementation, IMPLEMENTED is not VERIFIED, and a phase requires every task COMPLETED plus its gate.

## Develop a vertical slice

- State the current task's acceptance and file scope before edits. Inspect actual implementation and relevant contracts. Only change files required by that task; no unrelated refactors, accounting/workers/plugins added to the demo during governance.
- Deliver real behavior, positive/negative tests and runnable examples. Test independent outcomes rather than copying the implementation into assertions. Document supported/unsupported semantics, especially numeric precision, input ordering and state ownership.
- Preserve Local-First/offline eligibility, SDK separation, Apache-2.0 and Git history. Follow supply-chain rules in [Development](DEVELOPMENT.md) before executing new packages/tools; pin the existing toolchain and commit Cargo.lock with manifest changes. Never upload private code/manifests/graphs to scanners without approval.
- Record material choices and experiments in contracts/ADRs. Do not mark transport, isolation or numeric choices delivered from design intent alone.

## Verify

Use the pinned toolchain; for the existing local install, activate [Development's shell exports](DEVELOPMENT.md#project-local-toolchain). Do not install a toolchain merely because the shell PATH lacks cargo. Verify the local pin first. `RUSTUP_AUTO_INSTALL=0` prevents accidental installer execution during recovery.

Required commands from repository root (locked/offline flags strengthen the specified Cargo checks):

```sh
cargo fmt --check
cargo check --workspace --locked --offline
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
python3 scripts/check_docs.py
git diff --check
```

Also run current task-specific acceptance commands; for a baseline audit, `cargo run --locked --offline -p fqe-cli -- demo`. Record command, exit/result, test count, date, platform/toolchain, fixture/artifact identity and limitations. Never infer success from a command merely having been launched. Stop a sequential verification script on first error or inspect each result separately. No new package is required for the stdlib documentation checker. It validates local Markdown link/anchor targets and task IDs/dependencies/status synchronization; it is not a semantic-proof engine or an external-link availability checker.

## Close, commit and push

1. Record actual test/acceptance evidence in PROJECT_STATUS (or a linked dated report). Update contracts, plan cells and NEXT_SESSION, including remaining risks and next eligible task. Until an evidence commit exists keep the task VERIFIED.
2. Inspect full diff, staged paths and `git diff --check`. Stage only intended files; protect secrets, datasets, generated files and unrelated changes. Commit on main with a concrete title. This task explicitly authorizes the normal GitHub push.
3. Record the resulting evidence commit in status/handoff. Mark COMPLETED only after all six closure conditions above hold. A small closure commit may update these records; do not amend history just to chase a self-referential SHA. Resolve latest status commit using the command in [Document authority](DOCUMENT_AUTHORITY.md).
4. Check remote main with `git ls-remote origin refs/heads/main`; fetch if needed. If remote advanced, inspect and preserve those commits, reconcile safely and rerun affected checks. Never reset, force-push or overwrite unrelated work to resolve divergence.
5. Normal `git push origin main`. Compare `git rev-parse HEAD` with remote main and inspect `git status --short --branch`. Report actual SHA, push result, tests, progress and next task/acceptance. Publish no success claim until the remote confirms it.

Push success is a delivery outcome separate from the seven task states. If push fails, completed local artifacts may remain COMPLETED if all six conditions hold, but record delivery BLOCKED in status/handoff with the unpushed commit and exact error. GOV-01 specifically requires confirmed push, so its acceptance remains VERIFIED until remote receipt is evidenced. Do not mark it complete based on an expected push.

## Interruption or failure

For quota/context exhaustion, tool errors, compilation/test failures or push failures, write actual last state as early as possible: current task/status, files changed, last command and error, what passed/failed/not run, dirty/staged/unpushed state, unresolved risks and exact recovery steps. A partial implementation remains IN_PROGRESS/IMPLEMENTED/BLOCKED as facts warrant. Never replace a failed result with a historical pass.

Before context limits approach, prioritize status/handoff updates over starting a new slice. If file writes themselves fail, report the real limitation and recover from Git/working tree next time. NEXT_SESSION must name files to edit, protected files, tests and next acceptance so a new session can resume without this conversation.
