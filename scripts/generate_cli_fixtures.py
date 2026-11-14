#!/usr/bin/env python3
"""Build synthetic local CLI files and independent raw-byte SHA references (stdlib only)."""
from pathlib import Path
import copy
import hashlib
import json
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'crates/fqe-cli/tests/fixtures'
OUT.mkdir(parents=True,exist_ok=True)
def write(name,value):
    data=(json.dumps(value,indent=2)+'\n').encode()
    (OUT/name).write_bytes(data)
    if name.endswith('-data.json'):
        (OUT/(name.removesuffix('.json')+'.identity.txt')).write_text('sha256:'+hashlib.sha256(data).hexdigest()+'\n')
def decimal(kind,scale,n):
    return dict(type=dict(version=1,shape='scalar',kind='decimal',scale=scale,unit=dict(kind=kind,instrument='DEMO')),value=str(n))
def bars(values):
    events=[]
    for i,n in enumerate(values,1):
        events.append(dict(kind='closed_bar',sequence=str(i),timestamp_ns=str(i*1000),instrument='DEMO',timeframe_ns='1000',open_ns=str((i-1)*1000),close_ns=str(i*1000),closed=True,
            **{f:decimal('price',12,n*10**12) for f in ['open','high','low','close']},volume=decimal('quantity',0,0)))
    return dict(format='fqe.market-data',schema_version=1,events=events)
write('g002-data.json',bars([1,2,3,2,4,3]))
write('g007-data.json',bars([3,2,1,2,3,4]))
for threshold in [60,90]:
    write(f'parameters-{threshold}.json',dict(version=1,values=dict(threshold=dict(type=dict(version=1,shape='scalar',kind='decimal',scale=12,unit=dict(kind='ratio')),value=str(threshold*10**12)))))
write('parameters-empty.json',dict(version=1,values={}))
model=json.loads((ROOT/'crates/fqe-ir/tests/fixtures/g002-events.json').read_text())
model['graph']['parameters']['entries'][0]['default']['value']=str(90*10**12)
write('g002-high-model.json',model)
model=json.loads((ROOT/'crates/fqe-ir/tests/fixtures/tick-events.json').read_text())
model['outputs'].append(dict(name='price',expression=dict(output='price')))
write('tick-model.json',model)
write('tick-data.json',dict(format='fqe.market-data',schema_version=1,events=[
    dict(kind='tick',sequence='9007199254740993',timestamp_ns='9007199254740993',instrument='OTHER',price='101'),
    dict(kind='tick',sequence=str(2**64-1),timestamp_ns=str(2**64-1),instrument='DEMO',price=str(2**64-1))]))
write('empty-data.json',dict(format='fqe.market-data',schema_version=1,events=[]))

model=json.loads((OUT/'tick-model.json').read_text())
model['outputs'].append(dict(name='z_numeric',expression=dict(binary=dict(op='divide',left=dict(literal=dict(type=dict(version=1,shape='scalar',kind='u64'),value='100')),right=dict(binary=dict(op='subtract',left=dict(input='sample'),right=dict(literal=dict(type=dict(version=1,shape='scalar',kind='u64'),value='10'))))))))
write('tick-error-model.json',model)
write('tick-error-data.json',dict(format='fqe.market-data',schema_version=1,events=[dict(kind='tick',sequence=str(i),timestamp_ns=str(i),instrument='DEMO',price=str(n)) for i,n in [(1,110),(2,10)]]))
