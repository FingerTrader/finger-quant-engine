# Linear-margin partial-fill repair

Owner-requested repair, 2026-11-16; scoped to P4-03.03, which remains IN_PROGRESS. Verification is in progress. This does not change the supported margin model, liquidity policy or live-trading scope. See [margin contract](LINEAR_RESEARCH_MARGIN.md).

## Failure and correction

The current Desk test uses 100000 initial capital, 100x leverage and a 10 percent per-entry margin-plus-fee budget. The original first60 BTCUSDT bars produced seven false insufficient-margin rejections. A roughly13.193-unit entry needs about10000 margin, but the prior-bar liquidity policy requires partial execution on smaller-volume bars.

Gross-basis settlement can consume all cash in the shared clearing hub. Previously, the remaining admitted order budget was reserved again without backing that cash hold with internal clearing credit. VirtualLedger correctly rejected the cash reservation; the broker then labelled the failed atomic fill as insufficient margin. The original posting is rolled back.

All order reservations now ensure the required internal clearing cash before posting the hold, including reservations renewed after a partial fill. Shared-equity admission, marked used margin, pending budgets and the final free-margin check remain authoritative. Clearing debt is deducted from equity and repaid from released/unreserved cash. Cash-only execution is unchanged.

## Acceptance

- Four new independent flat-price long/short tests fail before the fix and pass after it:100000 capital,100x,10 percent, a10000-unit order executes in five2000-unit fills; intermediate used margin2000 plus pending hold8000 leaves90000 available, with100000 equity.
- When future liquidity is zero, only the first2000 units execute; terminal cancellation releases the remaining8000 hold and its clearing credit. Final available margin98000, used2000, unchanged equity100000; long clearing debt100000 reflects only actual200000 exposure.
- A separate hedge-entry case reserves60000 then rejects a second60000 budget against the remaining40000 margin. Clearing credit does not bypass risk admission.
- Existing fee, liquidation, FOK, cash and incremental-execution checks remain required. Full Rust checks, actual frozen Desk input, paired consumer checks and signed bundle verification are pending.

No dependency or lockfile change. No package/advisory data upload. All checks use the existing pinned offline toolchain and reviewed local artifacts. No GitHub publication is authorized for this repair.

## Full-workload capacity follow-up

The first full7200-bar run after fixing partial fills exposed the previous8192-entry ledger bound (`ledger_limit`); successful partial execution necessarily retains additional clearing/hold postings. Raise the finite journal bound to32768, preserving the existing atomic capacity rejection/replay validation. Keep the existing4096-fill/8192-order bounds. The paired complete-response bound is64MiB; Desk allows128MiB payload records (256MiB total store budget unchanged) so nested exact report encoding remains bounded. Request size, source bars and native executable size limits are unchanged. Full-workload and paired checks must validate these values together; no truncation or reduced source data is an acceptable final success.
