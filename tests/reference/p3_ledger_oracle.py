#!/usr/bin/env python3
"""Public synthetic accounting oracle using Fraction, independent of Rust/Desk execution.
Default/--check reads frozen expectations; --write is an explicit fixture-authoring action.
Legacy equations are a small documented arithmetic model, not execution of Desk code.
"""
import argparse
import json
from fractions import Fraction as F
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / 'crates/fqe-runtime/tests/fixtures/p3/ledger.expected.tsv'
# Hand-defined cases: (side, price, base quantity, separately supplied fee).
CASES = {
    'cash-long': ('1000', [('buy', '120', '2', '.50'), ('sell', '130', '2', '.50')]),
    'long-partial': ('1000', [('buy','10.01','3','.02'),('buy','11','1','.01'),('sell','12','1','.01'),('sell','9','2','.01'),('sell','13','1','.01')]),
    'short': ('1000', [('sell','100','3','1'),('buy','90','1','.50'),('buy','110','2','.50')]),
    'rounding': ('10', [('buy','1.01','1.500','0'),('sell','1.01','.500','0'),('sell','1.01','1','0')]),
    'basis-half-even': ('1', [('buy','5','.002','0'),('sell','5','.001','0'),('sell','5','.001','0')]),
}


def rounded(value, even=False):
    scaled = abs(value) * 100
    quotient, remainder = divmod(scaled.numerator, scaled.denominator)
    if 2 * remainder > scaled.denominator or (2 * remainder == scaled.denominator and (not even or quotient % 2)):
        quotient += 1
    return F(quotient if value >= 0 else -quotient, 100)


def fixed(value, scale):
    n = value * 10**scale
    assert n.denominator == 1
    n = n.numerator
    sign = '-' if n < 0 else ''
    n = abs(n)
    return sign + (str(n) if scale == 0 else f'{n // 10**scale}.{n % 10**scale:0{scale}}')


def rows():
    result = ['case\tinitial\tstep\tside\tprice\tquantity\tfee\tcash\tposition\tcost\trealized_gross\tfees\trestricted\tavailable\tcash_rounding\tlegacy_cash\tlegacy_cost\tlegacy_realized']
    for name, (initial, commands) in CASES.items():
        cash, q, basis, profit, fees, residual = F(initial), F(0), F(0), F(0), F(0), F(0)
        lc, lq, lb, lp = cash, q, basis, profit
        for index, (side, price, qty, fee) in enumerate(commands, 1):
            p, amount, charge = F(price), F(qty), F(fee)
            direction = 1 if side == 'buy' else -1
            exact = p * amount
            gross = rounded(exact)
            flow = -direction * gross - charge
            # FQE: gross basis, fee expense; close may not cross zero.
            if q and (q > 0) != (direction > 0):
                assert amount <= abs(q)
                allocation = abs(basis) if amount == abs(q) else rounded(abs(basis) * amount / abs(q), even=True)
                position_sign = 1 if q > 0 else -1
                basis -= position_sign * allocation
                profit += position_sign * (gross - allocation)
            else:
                basis += direction * gross
            cash += flow
            q += direction * amount
            fees += charge
            residual += -direction * (gross - exact)
            restricted = -basis if q < 0 else F(0)
            assert cash + basis == F(initial) + profit - fees
            # Selected Desk cash-long equations: buy capitalizes settled gross+fee;
            # proportional cost is exact rational here, not Decimal34 equivalence.
            if name != 'short':
                legacy_flow = -rounded(exact + charge) if side == 'buy' else rounded(exact - charge)
                if side == 'buy':
                    lb -= legacy_flow
                    lq += amount
                else:
                    allocation = lb if amount == lq else lb * amount / lq
                    lb -= allocation
                    lq -= amount
                    lp += legacy_flow - allocation
                lc += legacy_flow
            legacy = ['unsupported'] * 3 if name == 'short' else [str(lc), str(lb), str(lp)]
            result.append('\t'.join([name, fixed(F(initial),2), str(index), side, fixed(p,2), fixed(amount,3), fixed(charge,2), fixed(cash,2), fixed(q,3), fixed(basis,2), fixed(profit,2), fixed(fees,2), fixed(restricted,2), fixed(cash-restricted,2), fixed(residual,18)] + legacy))
        if name == 'cash-long':
            assert (cash, q, profit-fees, lc, lq, lp) == (F(1019),F(0),F(19),F(1019),F(0),F(19))
        if name == 'short':
            assert cash == F(988) and profit == -10 and fees == 2
        if name == 'long-partial':
            assert cash == F('1001.91') and basis == 0
    return '\n'.join(result)+'\n'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--write', action='store_true')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    baseline = json.loads((ROOT / 'crates/fqe-ir/tests/fixtures/history/cash-long-comparison.json').read_bytes())
    assert baseline['format'] == 'fqe.cash-long-comparison' and baseline['schema_version'] == 1
    assert (F(baseline['initial_cash']), F(baseline['quantity'])) == (1000, 2)
    assert (F(baseline['buy']['gross']), F(baseline['buy']['cash_after'])) == (240, F('759.50'))
    assert (F(baseline['sell']['gross']), F(baseline['sell']['cash_after'])) == (260, 1019)
    assert (F(baseline['fees']), F(baseline['net_profit'])) == (1, 19)
    assert (baseline['buy']['fill_at_open_bar'], baseline['sell']['fill_at_open_bar']) == ('2', '4')
    expected = rows()
    if args.write:
        assert not args.check
        TARGET.write_text(expected)
    else:
        assert TARGET.read_text() == expected, 'frozen independent ledger expectations differ'
    print('PASS independent Fraction ledger oracle: 5 cases / 16 transaction rows')


if __name__ == '__main__':
    main()
