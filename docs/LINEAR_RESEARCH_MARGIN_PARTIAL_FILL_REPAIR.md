# Linear-margin partial-fill repair

Owner-requested repair, 2026-11-16; scoped to P4-03.03, which remains IN_PROGRESS. Native financial acceptance and paired consumer checks passed; final actual-window acceptance is pending because CUA returned `cgWindowNotFound`/`noWindowsAvailable`. Status: IMPLEMENTED for the complete desktop handoff. This does not change the supported margin model, liquidity policy or live-trading scope. See [margin contract](LINEAR_RESEARCH_MARGIN.md).

## Failure and correction

The current Desk test uses 100000 initial capital, 100x leverage and a 10 percent per-entry margin-plus-fee budget. The original first60 BTCUSDT bars produced seven false insufficient-margin rejections. A roughly13.193-unit entry needs about10000 margin, but the prior-bar liquidity policy requires partial execution on smaller-volume bars.

Gross-basis settlement can consume all cash in the shared clearing hub. Previously, the remaining admitted order budget was reserved again without backing that cash hold with internal clearing credit. VirtualLedger correctly rejected the cash reservation; the broker then labelled the failed atomic fill as insufficient margin. The original posting is rolled back.

All order reservations now ensure the required internal clearing cash before posting the hold, including reservations renewed after a partial fill. Shared-equity admission, marked used margin, pending budgets and the final free-margin check remain authoritative. Clearing debt is deducted from equity and repaid from released/unreserved cash. Cash-only execution is unchanged.

## Acceptance

- Four new independent flat-price long/short tests fail before the fix and pass after it:100000 capital,100x,10 percent, a10000-unit order executes in five2000-unit fills; intermediate used margin2000 plus pending hold8000 leaves90000 available, with100000 equity.
- When future liquidity is zero, only the first2000 units execute; terminal cancellation releases the remaining8000 hold and its clearing credit. Final available margin98000, used2000, unchanged equity100000; long clearing debt100000 reflects only actual200000 exposure.
- A separate hedge-entry case reserves60000 then rejects a second60000 budget against the remaining40000 margin. Clearing credit does not bypass risk admission.
- Existing fee, liquidation, FOK, cash and incremental-execution regressions passed in the full379-test Rust workspace run. After the final entry-budget distinction, all13 margin tests and fmt/check/strict all-target Clippy passed again. Desk23 native/widget tests, four-file analyzer and final macOS Debug build passed. Signed helper identity matches the final Desk pin and all pinned source hashes match current engine files.

No dependency or lockfile change. No package/advisory data upload. All checks use the existing pinned offline toolchain and reviewed local artifacts. No GitHub publication is authorized for this repair.

## Full-workload capacity follow-up

The first full7200-bar run after fixing partial fills exposed the previous8192-entry ledger bound (`ledger_limit`); successful partial execution necessarily retains additional clearing/hold postings. Raise the finite journal bound to32768, preserving the existing atomic capacity rejection/replay validation. Keep the existing4096-fill/8192-order bounds. The paired complete-response bound is64MiB; Desk allows128MiB payload records (256MiB total store budget unchanged) so nested exact report encoding remains bounded. Request size, source bars and native executable size limits are unchanged. Full-workload and paired checks must validate these values together; no truncation or reduced source data is an acceptable final success.

## Distinct terminal reasons

The complete real workload also contains39 partially filled orders whose remaining0.001-unit tail cannot fit the unspent frozen per-order budget (for example0.361071 remaining cash with over90000 available account margin). These keep their real fills and cancel only the tail as `cancelled_entry_budget`; an initially unfillable budget uses `rejected_entry_budget`. A genuinely binding account free-margin limit still uses `rejected_insufficient_margin`. A below-minimum actual fill notional uses `rejected_minimum_notional`. Legacy cash-profile reasons remain compatible. An independent higher-open fixture spends2000+4000+4000 of its10000 budget, retains6000 filled units, cancels4000 target units, and preserves300000 equity/288000 available margin.

## Recorded delivery evidence

Code commits: `c9bae69`, `d172b37`, `32a3be0`. [Machine evidence](evidence/p4-03/linear-margin-partial-fill-repair.json) records the frozen input digest, full actual7200-bar result, store reopen, checks and final identities. Original100000 USD/100x/10 percent input completed in35.935 seconds with2811 fills,2386 filled orders,39 `cancelled_entry_budget` tails and1 terminal cancellation; zero insufficient-margin rejections. No price/volume modification, source truncation or outcome substitution was used. The complete report is26686907 bytes and immutable stored record40318601 bytes. A fresh Desk application service reopened the record without invoking an engine and verified all7200 bars/2811 fills and the result identity.

Final prepared binary digest: `74b97f38d183eae218aeed262e2c72f565643aebea56fb7163ebf9942335c1cb`. Final bridge `sha256:34042455e16e0f77c0b61d7e787f5fefbd6385ea13dbd9788cc2bcac94b308d7`; financial engine `sha256:87bb3e8ec3c3db84389de803955ecb82ee498032500ef13374f529cff4ac12c7`. Signed app helper hash `ff0841a7585cd8959b73be878b1b7c152ee835d2c06a7bb6838d3e31bda85ec4` matches its manifest. Final macOS Debug build exited0; no publication.

Actual-window limitation: the old app was normally quit, and Android Studio Run was attempted. Subsequent IDE and explicit-path app observations failed with `noWindowsAvailable` and `cgWindowNotFound`. Do not claim a final GUI run/restart succeeded. A later read-only process/record check found a running app and a00:43 input using a different bridge identity (same financial engine); this is not final-bundle GUI acceptance. Do not terminate an unseen active user run. When the screen is available, normally stop/quit, run the prepared Debug app from Android Studio, verify the final bridge identity, then observe partial fills and pause. Preserve the user's current saved30 percent configuration; the complete diagnostic fixture was10 percent. P4-03.03 remains IN_PROGRESS.
