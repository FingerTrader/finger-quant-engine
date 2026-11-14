"""Public-fixture managed research cancellation, Python standard library only."""
import copy, datetime, hashlib, json, struct, subprocess, sys, time
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
PEER = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT / 'target/release/fqe-desktop-peer'
data = json.loads((ROOT/'crates/fqe-bridge/tests/fixtures/research-input.json').read_text())
model = json.loads(data['model'])
model['nodes'] = [n for n in model['nodes'] if n['id'] in ['bars','entry','exit']]
model['nodes'] += [{'id': 'yes','kind':'constant','label':'yes','settings':{'type':'bool','value':'true'},'literals':{}}, {'id':'no','kind':'constant','label':'no','settings':{'type':'bool','value':'false'},'literals':{}}]
model['wires'] = [{'from':'yes','output':'value','to':'entry','input':'condition'}, {'from':'no','output':'value','to':'exit','input':'condition'}]
entry = next(n for n in model['nodes'] if n['id']=='entry')
entry['settings'].update(orderType='limit',limitPrice='0.50',triggerMode='rising_edge')
data['model'] = json.dumps(model)
stream = data['streams'][0]
spec = json.loads(stream['history_spec'])
start = datetime.datetime(2024,1,2,tzinfo=datetime.timezone.utc)
text = lambda d: d.isoformat().replace('+00:00','Z')
rows = [stream['csv'].splitlines()[0]]
for i in range(20000):
 a=start+datetime.timedelta(minutes=i); b=a+datetime.timedelta(minutes=1)
 rows.append(f'DEMO,1m,{text(a)},{text(b)},2.00,2.00,2.00,2.00,1000.000,true')
end=start+datetime.timedelta(minutes=20000)
spec['calendar']['sessions']=[{'open':text(start),'close':text(end)}]
spec['acquired_at']=text(end+datetime.timedelta(minutes=1))
stream.update(end_ns=str(int(end.timestamp())*1000000000),csv='\n'.join(rows)+'\n',history_spec=json.dumps(spec))
attempts=[]
for delay in [0.2,0.4,0.7,1.0]:
 p=subprocess.Popen([str(PEER)],stdin=subprocess.PIPE,stdout=subprocess.PIPE)
 serial=0
 def call(method,params):
  global serial
  serial+=1
  body=json.dumps(dict(format='fqe.desktop-request',schema_version=1,id=str(serial),method=method,params=params),separators=(',',':')).encode()
  p.stdin.write(struct.pack('>I',len(body))+body);p.stdin.flush()
  header=p.stdout.read(4);assert len(header)==4,'disconnected'
  length=struct.unpack('>I',header)[0];raw=p.stdout.read(length);assert len(raw)==length,'partial'
  value=json.loads(raw);assert value['ok'],value
  return value['body']
 try:
  session=call('run.initialize',{})['session_id']
  begun=call('run.start',dict(session_id=session,operation_id='cancel-public',input=data))
  handle=begun['handle']; args=dict(session_id=session,handle=handle)
  running=call('run.status',args);assert running['phase'] in ['accepted','running'],running
  time.sleep(delay)
  stopped=call('run.stop',args)
  deadline=time.monotonic()+20
  while True:
   status=call('run.status',args)
   if status['terminal']:break
   assert time.monotonic()<deadline,'stop deadline'
   time.sleep(0.01)
  result=call('run.result',args);report=result['report'];processed=int(report['processed_source_bars'])
  attempts.append(dict(delay=delay,phase=status['phase'],processed=processed))
  if status['phase']!='cancelled' or processed==0:continue
  assert 0<processed<20000,processed
  assert report['terminal']=='cancelled'
  assert len(report['equity_curve'])==processed
  assert not report['fills']
  assert all(o['status']=='cancelled_terminal' for o in report['orders']),report['orders']
  assert report['account']['cash_hub']['reserved_cash']=='0.00'
  assert report['metrics']['final_equity']=='100.00'
  raw=json.dumps(report,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
  assert result['result_id']=='sha256:'+hashlib.sha256(b'FQE-BACKTEST-RESULT/1\n'+raw).hexdigest()
  call('run.release',args)
  print(json.dumps(dict(format='fqe.research-managed-cancel-check/1',status='passed',peer_sha256=hashlib.sha256(PEER.read_bytes()).hexdigest(),attempts=attempts,result_id=result['result_id'],processed=processed)))
  break
 finally:
  p.stdin.close();p.wait(timeout=30)
else:raise AssertionError(attempts)
