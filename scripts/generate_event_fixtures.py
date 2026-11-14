#!/usr/bin/env python3
"""Build explicit event artifacts using stdlib only; signal expectations are independent goldens.
Never rewrites the P1-06 rational indicator reference data or earlier canonical fixtures.
"""
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'crates/fqe-ir/tests/fixtures'
U64 = dict(version=1, shape='scalar', kind='u64')
def decimal(unit, scale=12):
    u = dict(kind=unit)
    if unit in ('price', 'quantity'):
        u['instrument'] = 'DEMO'
    return dict(version=1, shape='scalar', kind='decimal', scale=scale, unit=u)
PRICE = decimal('price')
RATIO = decimal('ratio')
def val(t, n):
    return dict(type=t, value=str(n))
def literal(t, n):
    return dict(literal=val(t, n))
def output(n):
    return dict(output=n)
def binary(op, left, right):
    return dict(binary=dict(op=op, left=left, right=right))
def graph(name, input_type):
    return dict(format='fqe.graph', schema_version=1, ir_version=2, id=name,
                version='1', parameters=dict(version=1, entries=[]),
                requirements=['model.typed-graph/1'], entries=[],
                graph=dict(inputs=[dict(name='sample', type=input_type)], nodes=[], outputs=[]),
                modules=[], metadata={}, workflow={}, state=[], dependencies=[])
def entry(g, name, artifact):
    key = dict(id=name, version='1')
    g['entries'].append(dict(key=key, descriptor_version=1, role='provider', artifact=artifact))
    return dict(kind='provider', key=key)
def node(g, node_id, name, params, ports, artifact):
    target = entry(g, name, artifact)
    g['graph']['nodes'].append(dict(id=node_id, target=target,
        arguments={k:literal(U64,v) for k,v in params.items()},
        inputs=[dict(name='close', source=dict(input='sample'))],
        outputs=[dict(name=k,type=t) for k,t in ports.items()]))
    for port,t in ports.items():
        g['graph']['outputs'].append(dict(name=node_id+'_'+port,type=t,source=dict(node=node_id,port=port)))
def wrapper(name, g, tick=False):
    return dict(format='fqe.event-plan', schema_version=1, id=name, version='1',
        requirements=['event.plan/1','expression.scalar/1'], graph=g,
        trigger=dict(kind='tick',instrument='DEMO') if tick else dict(kind='closed_bar',instrument='DEMO',timeframe_ns='1000',price_scale=12,volume_scale=0),
        bindings=dict(sample='price' if tick else 'close'), warmup_samples='0',crossings=[],outputs=[],signals=[])
def write(name,v):
    (OUT/name).write_text(json.dumps(v,indent=2)+'\n')
artifact='sha256:'+hashlib.sha256((ROOT/'crates/fqe-ir/src/indicators.rs').read_bytes()).hexdigest()
g=graph('g002.indicators',PRICE)
g['parameters']['entries']=[dict(key='threshold',type=RATIO,required=False,default=val(RATIO,60*10**12),constraints={},description='RSI threshold')]
node(g,'macd','reference.macd',dict(fast=2,slow=3,signal=2),dict(line=PRICE,signal=PRICE,histogram=PRICE),artifact)
node(g,'rsi','reference.rsi',dict(period=2),dict(value=RATIO),artifact)
e=wrapper('g002.events',g)
e['crossings']=[dict(id='up',direction='above',left=output('macd_line'),right=output('macd_signal'))]
e['outputs']=[dict(name='decision',expression=binary('and',dict(cross='up'),binary('greater',output('rsi_value'),dict(parameter='threshold')))),dict(name='rsi',expression=output('rsi_value'))]
e['signals']=[dict(output='decision',kind='long')]
write('g002-events.json',e)
g=graph('g007.indicators',PRICE)
node(g,'fast','reference.sma',dict(period=2),dict(value=PRICE),artifact)
# Both SMA instances require the same entry, once.
node(g,'slow','reference.sma',dict(period=3),dict(value=PRICE),artifact)
g['entries']=g['entries'][:1]
e=wrapper('g007.events',g)
e['crossings']=[dict(id='up',direction='above',left=output('fast_value'),right=output('slow_value'))]
e['outputs']=[dict(name='decision',expression=dict(cross='up'))]
e['signals']=[dict(output='decision',kind='long')]
write('g007-events.json',e)
g=graph('tick.events',U64)
g['graph']['outputs']=[dict(name='price',type=U64,source=dict(input='sample'))]
e=wrapper('tick.events',g,True)
e['outputs']=[dict(name='decision',expression=binary('greater',output('price'),literal(U64,100)))]
e['signals']=[dict(output='decision',kind='long')]
write('tick-events.json',e)
g=graph('novel.events',U64)
a='sha256:'+('2a'*32)
node(g,'a','novel.counter',{},dict(value=U64),a)
node(g,'b','novel.reject',{},dict(value=U64),a)
e=wrapper('novel.events',g,True)
e['crossings']=[dict(id='up',direction='above',left=output('a_value'),right=literal(U64,1))]
e['outputs']=[dict(name='count_a',expression=output('a_value')),dict(name='count_b',expression=output('b_value')),dict(name='cross',expression=dict(cross='up'))]
write('counter-events.json',e)
# False guard ordering and repeated crossing references share the same subscriptions.
import copy
base=copy.deepcopy(e)
guard=binary('greater',dict(input='sample'),literal(U64,15))
for name,left,right in [('counter-guard-first.json',guard,dict(cross='up')),('counter-cross-first.json',dict(cross='up'),guard)]:
    variant=copy.deepcopy(base)
    variant['outputs'] += [dict(name='decision',expression=binary('and',left,right)),dict(name='shared',expression=dict(cross='up'))]
    variant['signals']=[dict(output='decision',kind='long')]
    write(name,variant)
variant=copy.deepcopy(base)
variant['outputs'] += [dict(name='z_failure',expression=binary('divide',literal(U64,100),binary('subtract',dict(input='sample'),literal(U64,10))))]
write('counter-root-error.json',variant)
variant=json.loads((OUT/'g007-events.json').read_text())
variant['graph']['graph']['nodes'][1]['inputs'][0]['source']=dict(node='fast',port='value')
variant['outputs']=[dict(name='slow',expression=output('slow_value'))]
variant['crossings']=[]
variant['signals']=[]
write('chain-events.json',variant)

canonical=json.loads((OUT/'tick-events.json').read_text())
canonical['outputs'].sort(key=lambda x:x['name'])
canonical['requirements'].sort()
# This fixture's graph contains one input/output and no parameters/nodes/modules.
b=json.dumps(canonical,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
(OUT/'tick-events.canonical.json').write_bytes(b)
(OUT/'tick-events.identity.txt').write_text('sha256:'+hashlib.sha256(b'FQE-EVENT-PLAN-CANONICAL/1\n'+b).hexdigest()+'\n')

variant=copy.deepcopy(base)
for n in variant['graph']['graph']['nodes']:
    n['inputs']=[]
write('counter-zero-input.json',variant)
variant=copy.deepcopy(base)
variant['crossings'][0]['left']=output('b_value')
write('counter-missing.json',variant)
variant=json.loads((OUT/'tick-events.json').read_text())
fault=binary('divide',literal(U64,100),binary('subtract',dict(input='sample'),literal(U64,10)))
variant['outputs'][0]['expression']=binary('and',dict(literal=dict(type=dict(version=1,shape='scalar',kind='bool'),value=False)),binary('greater',fault,literal(U64,0)))
write('tick-lazy-error.json',variant)
