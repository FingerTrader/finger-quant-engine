# Margin-long simulation and financial risk profiles

Implemented experimental P3-04 native Rust profile. [Project status](PROJECT_STATUS.md) owns closure. This contract extends [Ledger](LEDGER.md) and [Order simulation](ORDER_SIMULATION.md), retaining one authoritative account book and the previous-volume next-open matching convention. It does not implement a broker SDK or imply that an exchange/broker uses these rules.

## Explicit supported profile

`fqe.next-open-margin-long/1` requires ledger `fqe.gross-basis-margin-long/1` plus MarginRules. Max leverage is10000..50000 basis points (1..5x). Maintenance is positive and strictly below the initial equity/exposure ratio: maintenance_bps×max_leverage_bps<100000000. Config/profile mismatch, invalid rates, unsupported short margin or signed-price execution fail. The original `fqe.next-open-prior-volume/1` cash-long model requires no margin rules and preserves its economic behavior. The separate proceeds-restricted short ledger still has no broker margin model.

Financing is one currency, principal-only and zero-interest. There is no interest accrual, borrow availability, collateral haircut/FX/netting, portfolio margin, futures/options multiplier, tax, bankruptcy waterfall, insurance, funding rate or broker-specific policy. Ordinary configured per-execution fee/slippage applies to both voluntary and forced trades; no extra liquidation charge is added. Other assumptions need a new profile/version and independent fixtures.

## Debt is authoritative financial state

VirtualLedger adds Borrow/Repay only for MarginLong. Borrow debits Cash and credits LoanDebt; repayment reverses those balanced postings. Amount must be positive, step/range-valid and repayment cannot exceed outstanding principal or spend held cash. Cash and debt remain nonnegative. Borrowed cash never increases external capital or profit. Native ledger creation still starts with zero debt; low-level accounting commands are trusted postings, while the broker owns risk admission.

AccountView reports debt. Every state reconciles `cash + signed_cost - debt = external_capital + realized_gross - fees`; each journal entry separately balances money/quantity and binds complete engine/config pins at opening. Marked equity is `cash + market_value - debt`; net profit is equity minus external capital and equals realized gross plus unrealized minus fees. MarginLong permits a zero mark to expose worthless collateral and negative equity; execution prices still must be positive. Existing cash-long/short profiles retain their positive-mark policy.

The broker reserves full explicit buy cash caps. Pending buy holds count conservatively toward leverage and pending sells do not assume future execution. At known close, current marked long exposure plus all live buy caps plus the proposed cap must fit floor_to_money_step(equity×max_leverage). Nonpositive equity cannot add risk. A permitted cap beyond currently free cash borrows exactly the deficit before placing the named hold, within the same staged command. Cancellation/rejection/full completion releases only the proper hold. All unreserved cash repays principal up to remaining debt; this happens after release/fills and does not consume other holds.

Actual buy execution is rechecked at the known raw open after the staged fee/slippage cash effect. The candidate marked exposure plus remaining buy holds must fit leverage based on its resulting equity. The completed order's unused buffer is excluded; a partial's remaining hold stays included. A failing after-fee check restores that entire trade before releasing the rejected remainder and repaying free financing. No fill/fee/profit is published for that rejected candidate. Borrowed cash and risk checks cannot be bypassed through the broker's read-only ledger getter.

## Maintenance and subsequent-open liquidation

After a whole source bar is matched, free cash is swept to debt and the known close marks the portfolio. Required maintenance is ceil_to_money_step(marked long exposure×maintenance_bps/10000). Equality is healthy; equity strictly below maintenance triggers breach. Negative equity is also a breach. Pending orders are cancelled, their holds released and free cash repaid. A dedicated system sell reserves the actual remaining long quantity, including residuals below order minima, and becomes eligible only after this closed bar. A close price never retroactively forces a sale at the same bar open.

Liquidation uses subsequent opens with the same adverse tick slippage/per-fill fee and previous-closed-volume shared liquidity. It can partially fill or stall at zero previous volume. Once triggered, it continues even if prices recover; this profile has no automatic margin-call cure. New user risk is disabled; the reserved system order cannot be cancelled/replaced individually. Margin profile reserves one additional system order slot beyond the configured user order capacity, so a full user-order history cannot suppress its creation. Reserved `__fqe.` IDs apply only to the margin profile.

| State | Meaning |
| --- | --- |
| Healthy | Supported admission remains available; current maintenance passed |
| Liquidating | A subsequent-open system sell is pending or partial |
| Liquidated | Forced exit completed with zero debt |
| Insolvent | Flat liquidation retains unpaid debt/negative equity, or a failed exit still has negative marked equity |
| LiquidationFailed | Forced exit could not execute under the model while marked equity is nonnegative; position remains explicitly open |
| Stopped | Explicit Finish stopped a healthy/open or pending-liquidation run with exposure still open |

Gap loss is never repaired with invented cash or principal forgiveness. After a successful forced sale, free proceeds repay as much debt as available; flat negative equity remains outstanding debt and Insolvent. A zero-price/model-invalid forced exit records rejection and preserves inventory/debt; it is not a fabricated zero-price fill. LiquidationFailed and Insolvent remain visible through Finish. Finish releases all remaining holds and may leave positions/debt; it never claims they were closed. Configured fill/command quotas can prevent more liquidation work, but atomic rollback/explicit errors and the reserved Finish slot remain. None of these states guarantees real-world liquidation liquidity or limits loss.

## Determinism and verification

Margin status and mark/equity/exposure/maintenance source-bar observations join native simulation receipts. Full model/rules/engine/account pins, loan postings and every order/fill/hold/financial state recompute through existing broker and ledger replay. Each command/whole bar remains atomic, including late quotas after an earlier staged leveraged fill. Public cash-long tests remain unchanged in financial outcome. Native DTO additions are experimental and do not promise a portable schema, financial crash-file persistence, external output acknowledgement or migration.

`python3 tests/reference/p3_margin_oracle.py --check` uses independent Python stdlib Fraction equations and verifies four public synthetic cases/18 frozen rows in [margin expectations](../crates/fqe-runtime/tests/fixtures/p3/margin.expected.tsv). Checks never rewrite expected files. The cases compare every phase of borrowing, buy costs, marked maintenance, partial forced sales, repayments, fees and final debt/equity. The original five-case16-row ledger oracle remains separate.

Hand outcomes for2x/25% maintenance:1000 capital borrows1000 and buys20 at100. A60 close gives exposure1200/equity200/maintenance300, arming a later-open exit. Selling20 at55 gives1100 proceeds, repays1000 and leaves100 cash/net-900. Gapping instead to40 yields800 proceeds, repays800 and leaves200 debt/equity-200 with zero position. Partial forced sales5 then15 at75, even after equity recovers, leave500 cash. A separate fee case buys15 at100 with1 fee/501 debt and voluntarily sells15 at90 with1 fee, repays501 and ends848 cash/net-152. Equality/below-maintenance, actual0.05 rounding, after-fee admission, pending-cap leverage, loan cancellation/replace, unsupported rates/profiles, reserved system slot, zero-volume/zero-price gaps, error-state preservation, late quota rollback and every-command replay/tamper controls supplement the oracle.

`cargo run --locked --offline -p fqe-runtime --example margin_simulation` imports the public three-bar historical spec/CSV, explicitly places the leveraged order, observes close-triggered/subsequent-open forced sale, ends Liquidated/cash100/debt0/quantity0/realized gross-900 and verifies full native replay. The source dataset ID is printed. Full P1/P2/workspace tests and both independent accounting oracles remain required before task closure.

No dependency/manifest/lockfile/toolchain changes, packages/installers, scanners/private uploads or Desk edits/packages/app execution. Full portfolio runner/report/optimizer, complete persisted financial checkpoint, other financial/market models, actual Desk/native bundle, workers/plugins and live trading acceptance remain downstream.
