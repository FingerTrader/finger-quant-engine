#!/usr/bin/env python3
"""Independent public margin/debt cases; Fraction only, no Rust or Desk execution."""
import argparse
from fractions import Fraction as F
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
TARGET=ROOT/'crates/fqe-runtime/tests/fixtures/p3/margin.expected.tsv'

def fixed(v,scale=2):
    n=v*10**scale
    assert n.denominator==1
    n=n.numerator
    return ('-' if n<0 else '')+f'{abs(n)//10**scale}.{abs(n)%10**scale:0{scale}}'

def rows():
    output=['case\tphase\tcash\tdebt\tquantity\tcost\tfees\tequity\tmaintenance\tstatus']
    for case in ['full','gap','partial','costs']:
        cash,debt,qty,cost,fees,realized=F(1000),F(0),F(0),F(0),F(0),F(0)
        def row(phase,mark,status):
            equity=cash+qty*mark-debt
            assert cash+cost-debt==1000+realized-fees
            output.append('\t'.join([case,str(phase),fixed(cash),fixed(debt),fixed(qty,3),fixed(cost),fixed(fees),fixed(equity),fixed(qty*mark/4),status]))
        def sell(amount,price,fee=F(0)):
            nonlocal cash,debt,qty,cost,fees,realized
            allocated=cost*amount/qty
            cash+=amount*price-fee
            realized+=amount*price-allocated
            cost-=allocated
            qty-=amount
            fees+=fee
            repayment=min(cash,debt)
            debt-=repayment
            cash-=repayment
        row(1,F(100),'Healthy')
        cap=F(1501 if case=='costs' else 2000)
        debt=cap-cash
        cash=cap
        row(2,F(100),'Healthy')
        qty=F(15 if case=='costs' else 20)
        cost=qty*100
        fees=F(1 if case=='costs' else 0)
        cash-=cost+fees
        row(3,F(80 if case=='costs' else 60),'Healthy' if case=='costs' else 'Liquidating')
        if case=='costs':
            row(4,F(80),'Healthy')
            sell(qty,F(90),F(1))
            row(5,F(90),'Healthy')
            assert cash==848 and debt==0 and fees==2
        elif case=='partial':
            sell(F(5),F(75));row(4,F(75),'Liquidating')
            sell(qty,F(75));row(5,F(75),'Liquidated')
            assert cash==500 and debt==0
        else:
            sell(qty,F(40 if case=='gap' else 55))
            row(4,F(40 if case=='gap' else 55),'Insolvent' if debt else 'Liquidated')
            assert (cash,debt)==((F(0),F(200)) if case=='gap' else (F(100),F(0)))
    return '\n'.join(output)+'\n'

def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--check',action='store_true');args=p.parse_args();value=rows()
    if args.write:
        assert not args.check
        TARGET.write_text(value)
    else:
        assert TARGET.read_text()==value,'frozen margin expectations differ'
    print('PASS independent Fraction margin oracle: 4 cases / 18 rows')
if __name__=='__main__':main()
