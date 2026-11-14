#!/usr/bin/env python3
"""Independent explicit grid accounting/ranking/identity plus actual exported research report."""
import argparse
import hashlib
import json
from fractions import Fraction as F
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
TARGET=ROOT/'crates/fqe-runtime/tests/fixtures/p3/optimization.expected.tsv'
def canonical(v):return json.dumps(v,sort_keys=True,separators=(',',':')).encode()
def identity(domain,v):return 'sha256:'+hashlib.sha256(domain.encode()+b'\n'+canonical(v)).hexdigest()
def params(threshold):return {'threshold':{'type':{'version':1,'shape':'scalar','kind':'decimal','scale':12,'unit':{'kind':'ratio'}},'value':str(threshold*10**12)}}
def fixed(v):
    v=F(v);n=v*100;assert n.denominator==1;n=n.numerator
    return ('-' if n<0 else '')+f'{abs(n)//100}.{abs(n)%100:02}'
def expected():
    rows=[]
    for threshold in [0,60,90]:
        # Public fixture's independently accepted crossing occurs at the fifth close.
        # Threshold0 keeps the long open; threshold60 exits;90 has no training entry.
        training=(100-2*3-F('0.5')+2*2-100 if threshold==0 else 100-2*3-F('0.5')+2*2-F('0.5')-100 if threshold==60 else F(0))
        validation=100-2*10-F('0.5')+2*5-(F(0) if threshold==0 else F('0.5'))-100
        rows.append(dict(threshold=threshold,identity=identity('FQE-OPTIMIZATION-PARAMETERS/1',params(threshold)),training=training,drawdown=-training,validation=validation))
    ranks=sorted(rows,key=lambda r:(-r['training'],r['drawdown'],r['identity']))
    for row in rows:row['rank']=ranks.index(row)+1
    assert [r['threshold'] for r in ranks]==[90,0,60]
    assert ranks[0]['validation']==-11 and rows[0]['validation']==F('-10.5')
    return rows
def rendered():
    return 'threshold\tparameter_id\ttraining_profit\ttraining_drawdown\trank\tvalidation_profit\n'+''.join('\t'.join([str(r['threshold']),r['identity'],fixed(r['training']),fixed(r['drawdown']),str(r['rank']),fixed(r['validation'])])+'\n' for r in expected())
def schedule(n,seed):
    order=list(range(n));mask=2**64-1;state=seed
    for i in range(n-1,0,-1):
        state=(state+0x9e3779b97f4a7c15)&mask;z=state
        z=((z^(z>>30))*0xbf58476d1ce4e5b9)&mask;z=((z^(z>>27))*0x94d049bb133111eb)&mask;z^=z>>31
        j=z%(i+1);order[i],order[j]=order[j],order[i]
    return list(map(str,order))
def check_report(path):
    raw=path.read_bytes();r=json.loads(raw);assert raw==canonical(r)
    assert r['manifest_id']==identity('FQE-OPTIMIZATION-INPUTS/1',r['manifest'])
    assert r['manifest']['schedule']==schedule(3,42)==['0','2','1']
    assert r['terminal']=='completed' and r['completed_trials']=='3'
    oracle=expected()
    for t,e in zip(r['trials'],oracle):
        assert t['id']==e['identity'] and t['parameters']==params(e['threshold'])
        assert t['training']['status']=='completed';b=t['training']['report']
        assert t['training']['report_id']==identity('FQE-BACKTEST-RESULT/1',b)
        assert b['metrics']['net_profit']==fixed(e['training'])
        assert b['metrics']['max_drawdown']==fixed(e['drawdown'])
        assert b['metrics']['initial_equity']=='100.00'
        assert int(b['manifest']['history_inputs']['range']['last_close_ns'])<=int(r['manifest']['validation']['performance_start_ns'])
    assert r['ranking']==[e['identity'] for e in sorted(oracle,key=lambda e:e['rank'])]
    assert r['selected']==oracle[2]['identity']
    val=r['validation'];assert val['status']=='completed' and val['report_id']==identity('FQE-BACKTEST-RESULT/1',val['report'])
    assert val['report']['metrics']['net_profit']=='-11.00' and val['report']['metrics']['initial_equity']=='100.00'
    assert val['report']['manifest']['bound_parameters']==params(90)
    assert val['report']['fills'][0]['source_bar']=='13' and val['report']['fills'][1]['source_bar']=='14'
    print('PASS actual optimization JSON: canonical hashes, seeded order, 3 independent training values, isolated capital, training-only ranking and separate -11 validation')
def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--check',action='store_true');p.add_argument('--report',type=Path);a=p.parse_args()
    if a.write:assert not a.check;TARGET.write_text(rendered())
    else:assert TARGET.read_text()==rendered(),'frozen independent optimization fixture differs'
    print('PASS independent Fraction optimization grid: 3 training/validation cases')
    if a.report:check_report(a.report)
if __name__=='__main__':main()
