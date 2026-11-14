#!/usr/bin/env python3
"""Deliberate stdlib-only fixture generation; does not import/execute the engine."""
import copy,hashlib,json
from pathlib import Path
base=Path('crates/fqe-ir/tests/fixtures')
old=json.loads((base/'x001-sum.json').read_text())
u64={'version':1,'shape':'scalar','kind':'u64'}
bool_type={'version':1,'shape':'scalar','kind':'bool'}
key=lambda name:dict(id=name,version='1')
port=lambda name,t:dict(name=name,type=t)
wire=lambda name,source:dict(name=name,source=source)
out=lambda name,t,source:dict(name=name,type=t,source=source)
provider=lambda name:dict(kind='provider',key=key(name))
module=lambda name:dict(kind='module',key=key(name))
literal=lambda n:dict(literal=dict(type=u64,value=str(n)))
params={'version':1,'entries':[dict(key='threshold',type=u64,required=True,constraints={'upper':{'value':{'type':u64,'value':'999'},'inclusive':True}},description='Bounded reusable-rule threshold')]}
outputs=[port('accepted',bool_type),port('difference',u64),port('sum',u64)]
for variant in ['sum','difference']:
 d=copy.deepcopy(old);d['format']='fqe.graph';d['id']='x001.modules';d['requirements']=['model.typed-graph/1','model.pure-modules/1','values/1','parameters/1']
 constants=[dict(n,target=provider(n['entry']['id'])) for n in d['graph']['nodes'] if n['id'] in ['left','right']]
 for n in constants:del n['entry']
 pair=dict(id='pair',target=provider('independent.checked_pair'),arguments={},inputs=[wire('left',{'input':'left'}),wire('right',{'input':'right'})],outputs=[port('sum',u64),port('difference',u64)])
 predicate=dict(id='predicate',target=provider('independent.above'),arguments={'threshold':{'parameter':'threshold'}},inputs=[wire('value',{'node':'pair','port':variant})],outputs=[port('result',bool_type)])
 graph_outputs=[out('accepted',bool_type,{'node':'predicate','port':'result'}),out('difference',u64,{'node':'pair','port':'difference'}),out('sum',u64,{'node':'pair','port':'sum'})]
 rule=dict(schema_version=1,key=key('independent.rule'),parameters=params,graph=dict(inputs=[port('left',u64),port('right',u64)],nodes=[pair,predicate],outputs=graph_outputs))
 nested=dict(id='nested',target=module('independent.rule'),arguments={'threshold':{'parameter':'threshold'}},inputs=[wire('left',{'input':'left'}),wire('right',{'input':'right'})],outputs=outputs)
 wrapper=dict(schema_version=1,key=key('independent.wrapper'),parameters=params,graph=dict(inputs=[port('left',u64),port('right',u64)],nodes=[nested],outputs=[out(p['name'],p['type'],{'node':'nested','port':p['name']}) for p in outputs]))
 def invocation(name,entry,argument):return dict(id=name,target=module(entry),arguments={'threshold':argument},inputs=[wire('left',{'node':'left','port':'result'}),wire('right',{'node':'right','port':'result'})],outputs=outputs)
 inv1=invocation('first','independent.wrapper',{'parameter':'threshold'}); inv2=invocation('second','independent.rule',literal(250))
 invert=dict(id='invert',target=provider('fqe.bool.not'),arguments={},inputs=[wire('value',{'node':'first','port':'accepted'})],outputs=[port('result',bool_type)])
 d['graph']=dict(inputs=[],nodes=constants+[inv1,inv2,invert],outputs=[out('first',bool_type,{'node':'first','port':'accepted'}),out('second',bool_type,{'node':'second','port':'accepted'}),out('inverted',bool_type,{'node':'invert','port':'result'})])
 d['modules']=[rule,wrapper]
 (base/f'x001-graph-{variant}.json').write_text(json.dumps(d,indent=2)+'\n')
 if variant=='sum':
  def normalize_body(b):
   b['inputs'].sort(key=lambda p:p['name']); b['nodes'].sort(key=lambda n:n['id']); b['outputs'].sort(key=lambda p:p['name'])
   for n in b['nodes']:n['inputs'].sort(key=lambda p:p['name']);n['outputs'].sort(key=lambda p:p['name'])
  d['requirements'].sort();d['entries'].sort(key=lambda e:(e['key']['id'],e['key']['version']));d['modules'].sort(key=lambda m:(m['key']['id'],m['key']['version']))
  normalize_body(d['graph'])
  for m in d['modules']:normalize_body(m['graph']);m['parameters']['entries'].sort(key=lambda p:p['key'])
  d['parameters']['entries'].sort(key=lambda p:p['key'])
  canonical=json.dumps(d,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()
  (base/'graph.canonical.json').write_bytes(canonical)
  (base/'graph.identity.txt').write_text('sha256:'+hashlib.sha256(b'FQE-GRAPH-CANONICAL/1\n'+canonical).hexdigest()+'\n')
print('Wrote two nested/reused pure-module graphs and independent canonical identity')
