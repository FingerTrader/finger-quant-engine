#!/usr/bin/env python3
"""Independent rational metrics and an actual exported seven-bar financial report checker."""
import argparse
import hashlib
import json
from fractions import Fraction as F
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
TARGET=ROOT/'crates/fqe-runtime/tests/fixtures/p3/backtest-metrics.expected.tsv'
CASES=[('round_trip','100','100,100,100,100,100,99.50,97','1'),('peaks','100','120,90,130,65','2'),('insolvent','100','110,-20','3'),('zero','0','','0'),('new_peak_from_zero','0','0,10,5','0'),('empty','100','','0'),('even_tie','2000000000000','2000000000001','0'),('odd_tie','2000000000000','2000000000003','0'),('negative_tie','2000000000000','1999999999999','0')]
def fixed(x,scale=2):
    # Python's exact rational round implements ties-to-even, independently of Rust.
    n=round(x*10**scale)
    return ('-' if n<0 else '')+f'{abs(n)//10**scale}.{abs(n)%10**scale:0{scale}}'
def metrics(initial,curve,fees):
    final=curve[-1] if curve else initial
    peak=initial;drops=[];ratios=[]
    for eq in curve:
        peak=max(peak,eq);drops.append(peak-eq)
        if peak>0:ratios.append((peak-eq)/peak)
    return [fixed(final),fixed(final-initial),fixed(fees),fixed((final-initial)/initial,12) if initial else '-',fixed(max(drops,default=F(0))),fixed(max(ratios,default=F(0)),12) if initial or ratios else '-']
def rows():
    result=['case\tinitial\tcurve\tfees\tfinal\tprofit\tfee_total\treturn\tmax_drawdown\tmax_drawdown_ratio']
    for name,initial,curve,fees in CASES:
        result.append('\t'.join([name,initial,curve,fees]+metrics(F(initial),list(map(F,curve.split(','))) if curve else [],F(fees))))
    return '\n'.join(result)+'\n'
def canonical(v):return json.dumps(v,sort_keys=True,separators=(',',':')).encode()
def check_report(path):
    raw=path.read_bytes();r=json.loads(raw)
    assert canonical(r)==raw,'report JSON is not canonical'
    m=r['manifest'];assert m['financial_execution']['model']=='fqe.next-open-prior-volume/1'
    assert r['manifest_id']=='sha256:'+hashlib.sha256(b'FQE-BACKTEST-INPUTS/1\n'+canonical(m)).hexdigest()
    assert m['bound_parameters']['threshold']['value']=='60000000000000'
    assert r['terminal']=='completed' and r['processed_source_bars']=='7'
    curve=list(map(lambda p:F(p['equity']),r['equity_curve']))
    assert curve==[100,100,100,100,100,F('99.5'),97]
    expected=metrics(F(100),curve,F(1));actual=r['metrics']
    assert [actual[k] for k in ['final_equity','net_profit','fees','total_return','max_drawdown','max_drawdown_ratio']]==expected
    assert [(f['source_bar'],f['side'],f['price'],f['quantity'],f['fee']) for f in r['fills']]==[('6','buy','3.00','2.000','0.50'),('7','sell','2.00','2.000','0.50')]
    by_order={o['intent']['id']:o for o in r['orders']};by_journal={e['sequence']:e for e in r['journal']}
    for f in r['fills']:
        o=by_order[f['order']];j=by_journal[f['ledger_sequence']]
        assert int(o['submitted_after'])<int(f['source_bar'])
        assert j['transaction']['action']['kind']=='trade'
        assert j['transaction']['action']['quantity']==f['quantity']
        assert o['intent']['origin']['kind']=='policy'
        assert any(d['order']==f['order'] and d['binding']==o['intent']['origin']['binding'] for d in r['decisions'])
    for j in r['journal']:
        assert sum(F(p['delta']) for p in j['money'])==0
        a=j['after'];assert F(a['cash'])+F(a['cost'])-F(a['debt'])==F(a['external_capital'])+F(a['realized_gross'])-F(a['fees'])
    a=r['account'];assert (a['cash'],a['quantity'],a['debt'],a['reserved_cash'])==('97.00','0.000','0.00','0.00')
    assert r['valuation']['unrealized']=='0.00' and r['valuation']['total_profit']=='-3.00'
    print('PASS actual canonical report: hashes, parameter binding, 7 equities, 2 fills, balanced journals and provenance')
def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--check',action='store_true');p.add_argument('--report',type=Path);a=p.parse_args()
    if a.write:
        assert not a.check;TARGET.write_text(rows())
    else:assert TARGET.read_text()==rows(),'frozen metrics expectations differ'
    print('PASS independent Fraction portfolio metrics: 9 cases')
    if a.report:check_report(a.report)
if __name__=='__main__':main()
