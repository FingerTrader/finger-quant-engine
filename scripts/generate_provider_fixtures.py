"""Regenerate reviewed P1-04 source pins / independent JSON+SHA fixtures (stdlib only).

Run deliberately after reviewing implementation-source changes, never as part of tests.
No engine invocation is used to calculate these expected canonical bytes/digests.
"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'crates/fqe-ir/tests/fixtures'

def digest(data):
    return 'sha256:' + hashlib.sha256(data).hexdigest()

def source(name):
    return digest((ROOT / 'crates/fqe-ir' / name).read_bytes())

def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()

u64 = {'version': 1, 'shape': 'scalar', 'kind': 'u64'}
boolean = {'version': 1, 'shape': 'scalar', 'kind': 'bool'}
def value(number):
    return {'type': u64, 'value': str(number)}

def entry(name, artifact):
    return {'key': {'id': name, 'version': '1'}, 'descriptor_version': 1, 'role': 'provider', 'artifact': artifact}

builtin = source('src/builtins.rs')
provider = source('examples/support/providers.rs')
entries = [entry('fqe.constant.u64', builtin), entry('fqe.bool.not', builtin), entry('independent.checked_pair', provider), entry('independent.above', provider)]

def node(name, entry_name, args, inputs, outputs):
    return {'id': name, 'entry': {'id': entry_name, 'version': '1'}, 'arguments': args,
            'inputs': [{'name': key, 'source': {'node': ref[0], 'port': ref[1]}} for key, ref in inputs.items()],
            'outputs': [{'name': key, 'type': type_} for key, type_ in outputs.items()]}

model = {'format': 'fqe.model', 'schema_version': 1, 'ir_version': 2, 'id': 'x001.composition', 'version': '1',
         'parameters': {'version': 1, 'entries': [{'key': 'threshold', 'type': u64, 'required': False, 'default': value(150), 'constraints': {}, 'description': 'Exact threshold'}]},
         'requirements': ['model.scalar-graph/1', 'parameters/1', 'values/1'], 'entries': entries,
         'graph': {'nodes': [
             node('left', 'fqe.constant.u64', {'value': {'literal': value(125)}}, {}, {'result': u64}),
             node('right', 'fqe.constant.u64', {'value': {'literal': value(75)}}, {}, {'result': u64}),
             node('pair', 'independent.checked_pair', {}, {'left': ('left', 'result'), 'right': ('right', 'result')}, {'sum': u64, 'difference': u64}),
             node('predicate', 'independent.above', {'threshold': {'parameter': 'threshold'}}, {'value': ('pair', 'sum')}, {'result': boolean}),
             node('not', 'fqe.bool.not', {}, {'value': ('predicate', 'result')}, {'result': boolean})],
             'outputs': [{'name': 'decision', 'source': {'node': 'predicate', 'port': 'result'}}, {'name': 'inverse', 'source': {'node': 'not', 'port': 'result'}}]},
         'metadata': {'label': 'X-001 reviewed pure provider composition'}, 'modules': [], 'workflow': {}, 'state': [], 'dependencies': []}
for port in ['sum', 'difference']:
    model['graph']['nodes'][3]['inputs'][0]['source']['port'] = port
    (FIXTURES / f'x001-{port}.json').write_text(json.dumps(model, ensure_ascii=False, indent=2) + '\n')

port = lambda name: {'name': name, 'type': u64, 'cardinality': 'one'}
descriptor = {'descriptor_version': 1, 'key': {'id': 'independent.checked_pair', 'version': '1'}, 'artifact': provider, 'role': 'provider',
              'category': 'function', 'compatibility': {'engine_api': 1, 'provider_contract': 1, 'ir_min': 2, 'ir_max': 2},
              'inputs': [port('left'), port('right')], 'outputs': [port('difference'), port('sum')],
              'parameters': {'version': 1, 'entries': [{'key': 'bias', 'type': u64, 'required': False, 'default': value(0), 'constraints': {}, 'description': 'Exact u64'}]},
              'purity': 'pure', 'readiness': 'propagate', 'deterministic': True, 'history': 0, 'state_schema': None,
              'required_capabilities': ['provider.pure-scalar/1'], 'host_services': [], 'dependencies': [],
              'resources': {'max_input_bytes': 65536, 'max_output_bytes': 65536},
              'semantics': 'sum=checked(left+right+bias); difference=checked(left-right); exact u64, overflow fails; pure host readiness propagation.', 'metadata': {}}
raw = canonical(descriptor)
(FIXTURES / 'x001-descriptor.canonical.json').write_bytes(raw)
(FIXTURES / 'x001-descriptor.identity.txt').write_text(digest(b'FQE-DESCRIPTOR-CANONICAL/1\n' + raw) + '\n')
print('Regenerated X-001 source pins and independently encoded descriptor identity')
