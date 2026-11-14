#!/usr/bin/env python3
"""Independent public-fixture native execution checks, standard library only."""
import copy,json,struct,subprocess,sys,time,hashlib
from decimal import Decimal
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
FIXTURE=ROOT/'crates/fqe-bridge/tests/fixtures/research-input.json'
PEER=Path(sys.argv[1]) if len(sys.argv)>1 else ROOT/'target/debug/fqe-desktop-peer'
BASE=json.loads(FIXTURE.read_text())
checks=[]
def run(model=None,change=None,data_override=None):
 data=copy.deepcopy(BASE if data_override is None else data_override)
 if model is not None:data['model']=json.dumps(model,separators=(',',':'))
 if change:change(data)
 p=subprocess.Popen([str(PEER)],stdin=subprocess.PIPE,stdout=subprocess.PIPE)
 try:
  raw=json.dumps({'format':'fqe.desktop-request','schema_version':1,'id':'research','method':'backtest.run','params':data},separators=(',',':')).encode()
  p.stdin.write(struct.pack('>I',len(raw))+raw);p.stdin.flush();header=p.stdout.read(4)
  if len(header)!=4:raise AssertionError('native peer disconnected')
  size=struct.unpack('>I',header)[0];chunks=bytearray()
  while len(chunks)<size:
   b=p.stdout.read(size-len(chunks))
   if not b:raise AssertionError('partial response')
   chunks.extend(b)
  result=json.loads(chunks)
 finally:
  p.stdin.close();p.wait(timeout=30)
 if not result['ok']:raise AssertionError(result['body'])
 report=result['body']['report']
 # Native report encoding is deterministic sorted compact JSON.
 report_bytes=json.dumps(report,separators=(',',':'),sort_keys=True,ensure_ascii=False).encode()
 assert result['body']['result_id']=='sha256:'+hashlib.sha256(b'FQE-BACKTEST-RESULT/1\n'+report_bytes).hexdigest()
 for journal in report['journal']:
  for entry in journal['entries']:
   assert sum(Decimal(p['delta']) for p in entry['money'])==0,entry
 assert sum(Decimal(j['entries'][-1]['after']['external_capital']) for j in report['journal'])==Decimal('100')
 return report

def graph():return json.loads(BASE['model'])
def node(model,id):return next(n for n in model['nodes'] if n['id']==id)
def add(model,id,kind,settings=None,literals=None):
 n={'id':id,'kind':kind,'label':id,'settings':settings or {},'literals':literals or {}};model['nodes'].append(n);return n

def wire(model,source,output,target,input):
 model['wires']=[w for w in model['wires'] if (w['to'],w['input'])!=(target,input)]
 model['wires'].append({'from':source,'output':output,'to':target,'input':input})
def simple():
 m=graph();m['nodes']=[node(m,'bars'),node(m,'entry'),node(m,'exit')];m['wires']=[]
 add(m,'yes','constant',{'type':'bool','value':'true'});add(m,'no','constant',{'type':'bool','value':'false'})
 wire(m,'yes','value','entry','condition');wire(m,'no','value','exit','condition');node(m,'entry')['settings']['triggerMode']='rising_edge'
 return m

def check(name,fn):
 start=time.monotonic();result=fn();checks.append({'name':name,'status':'passed','seconds':round(time.monotonic()-start,3)});print('PASS '+name,file=sys.stderr);return result

def baseline():
 r=run();assert [x['price'] for x in r['fills']]==['4.00','2.00'];assert r['metrics']['final_equity']=='98.00';return r
check('independent-SMA-long-100-to-98',baseline)
def short():
 m=graph();m['context']['accountMode']='margin'
 for id in ['entry','exit']:node(m,id)['settings']['side']='short'
 r=run(m);assert [x['trade_side'] for x in r['fills']]==['sell','buy'];assert r['metrics']['final_equity']=='102.00';assert r['account']['cash_hub']['reserved_cash']=='0.00'
check('independent-short-100-to-102-and-no-spendable-proceeds',short)
def add_reduce():
 m=simple();addnode=copy.deepcopy(node(m,'entry'));addnode['id']='add';addnode['settings']['entryMode']='add';addnode['settings']['maxAdds']='2';addnode['settings']['triggerMode']='each_bar';m['nodes'].append(addnode);wire(m,'yes','value','add','condition')
 add(m,'low','compare',{'type':'price','operator':'less'},{'b':'2'});wire(m,'bars','close','low','a');wire(m,'low','result','exit','condition');node(m,'exit')['settings']['closeMode']='percent';node(m,'exit')['settings']['closePercent']='50'
 r=run(m);buys=[x for x in r['fills'] if x['trade_side']=='buy'];sells=[x for x in r['fills'] if x['trade_side']=='sell'];assert len(buys)==3,buys;assert sells[0]['quantity']=='1.000';assert sells[1]['quantity']=='1.000';assert r['account']['positions'][0]['account']['quantity']=='1.000',r['account']
check('add-two-times-and-reduce-without-reversing',add_reduce)
def pending(kind,tif,limit=None,stop=None):
 m=simple();a=node(m,'entry')['settings'];a.update(orderType=kind,timeInForce=tif)
 if limit:a['limitPrice']=limit
 if stop:a['stopPrice']=stop
 return run(m)
check('limit-waits-for-next-eligible-opening',lambda: (lambda r: (r['fills'][0]['price']=='1.00' and r['fills'][0]['source_sequence']=='3')or (_ for _ in ()).throw(AssertionError(r['fills'])))(pending('limit','GTC','1')))
check('stop-triggers-at-opening',lambda: (lambda r: r['fills'][0]['source_sequence']=='6' or (_ for _ in ()).throw(AssertionError(r['fills'])))(pending('stop','GTC',stop='4')))
check('stop-limit-retains-activation',lambda: (lambda r: r['fills'][0]['source_sequence']=='7' or (_ for _ in ()).throw(AssertionError(r['fills'])))(pending('stop_limit','GTC','1','4')))
check('IOC-cancels-unfilled-limit',lambda: (lambda r: (not r['fills'] and r['orders'][0]['status']=='cancelled_time_in_force')or (_ for _ in ()).throw(AssertionError(r['orders'])))(pending('limit','IOC','1')))
def cancel():
 m=simple();node(m,'entry')['settings'].update(orderType='limit',limitPrice='0.50',orderTag='entry')
 add(m,'cancel','cancelOrders',{'symbol':'','scope':'tag','orderTag':'entry','triggerMode':'rising_edge'});wire(m,'yes','value','cancel','condition')
 r=run(m);assert not r['fills'];assert r['orders'][0]['status']=='cancelled_rule';assert r['account']['cash_hub']['reserved_cash']=='0.00'
check('cancel-tag-releases-shared-cash',cancel)
def protection():
 m=simple();node(m,'entry')['settings'].update(stopLossMode='percent',stopLossValue='25')
 r=run(m);assert r['fills'][0]['price']=='2.00';assert r['fills'][1]['price']=='2.00';assert r['fills'][1]['source_sequence']=='4';assert r['fills'][1]['node'].startswith('protect.')
check('close-based-protection-next-opening',protection)
def functions():
 m=simple();m['functions']=[{'id':'choose','name':'choose','inputs':{'x':'price'},'outputType':'bool','body':'if(x < x, 1 / 0 > 0, abs(-1) == clamp(1, 0, max(1, 2)))','ast':{'op':'if','type':'bool','args':[{'op':'<','type':'bool','args':[{'op':'input','type':'price','name':'x'},{'op':'input','type':'price','name':'x'}]},{'op':'>','type':'bool','args':[{'op':'/','type':'ratio','args':[{'op':'literal','type':'ratio','value':'1'},{'op':'literal','type':'ratio','value':'0'}]},{'op':'literal','type':'ratio','value':'0'}]},{'op':'==','type':'bool','args':[{'op':'abs','type':'ratio','args':[{'op':'negate','type':'ratio','args':[{'op':'literal','type':'ratio','value':'1'}]}]},{'op':'clamp','type':'ratio','args':[{'op':'literal','type':'ratio','value':'1'},{'op':'literal','type':'ratio','value':'0'},{'op':'max','type':'ratio','args':[{'op':'literal','type':'ratio','value':'1'},{'op':'literal','type':'ratio','value':'2'}]}]}]}]}}]
 add(m,'function','function',{'functionId':'choose'});wire(m,'bars','close','function','x');wire(m,'function','value','entry','condition');r=run(m);assert len(r['fills'])==1
check('pure-function-if-lazy-abs-negate-clamp-max',functions)
def state():
 m=simple();add(m,'wait','wait',{'bars':'3','mode':'expiry','retrigger':'ignore'});wire(m,'yes','value','wait','arm');wire(m,'yes','value','wait','condition');wire(m,'wait','result','entry','condition');r=run(m);assert r['fills'][0]['source_sequence']=='5'
check('wait-latches-and-expires-at-three-subsequent-closes',state)
def consecutive():
 m=simple();add(m,'consecutive','consecutive',{'bars':'3','mode':'once'});wire(m,'yes','value','consecutive','condition');wire(m,'consecutive','result','entry','condition');r=run(m);assert r['fills'][0]['source_sequence']=='4'
check('consecutive-three-closes-before-next-open',consecutive)
def dynamic():
 m=simple();add(m,'account','account');add(m,'atr','atr',{'period':'2'});
 for p in ['high','low','close']:wire(m,'bars',p,'atr',p)
 add(m,'risk','riskSize',{'riskPercent':'1','lotStep':'0.001','minQuantity':'0.001','maxQuantity':'10'});wire(m,'account','equity','risk','equity');wire(m,'atr','value','risk','stopDistance');wire(m,'risk','quantity','entry','quantity');node(m,'entry')['settings']['triggerMode']='each_bar';r=run(m);assert r['fills'][0]['source_sequence']=='3';assert r['fills'][0]['quantity']=='2.000',r['fills']
check('ATR-risk-dynamic-quantity-native-execution',dynamic)
def boll():
 m=simple();add(m,'boll','boll',{'period':'2','deviation':'2'});wire(m,'bars','close','boll','price');add(m,'high','highest',{'period':'2','offset':'1'});wire(m,'bars','high','high','price');add(m,'low','lowest',{'period':'2','offset':'1'});wire(m,'bars','low','low','price');r=run(m);v=r['observations'][1]['outputs'];assert Decimal(v['boll.middle'])==Decimal('2.5');assert Decimal(v['boll.upper'])==Decimal('3.5');assert r['observations'][2]['outputs']['high.value']=='3.000000000000';assert r['observations'][2]['outputs']['low.value']=='2.000000000000'
check('BOLL-and-offset-channel-independent-values',boll)

def multi_stream():
 m=simple();add(m,'higher','bars',{'symbol':'DEMO','timeframe':'3m','dataset':'higher','trainingDataset':'','validationDataset':'','alignment':'latest_closed','maxAgeSeconds':'86400'});add(m,'higher_sma','sma',{'period':'2'});wire(m,'higher','close','higher_sma','price')
 def alter(data):
  stream=copy.deepcopy(data['streams'][0]);stream['id']='higher';spec=json.loads(stream['history_spec']);spec['timeframe']={'id':'3m','duration_ns':'180000000000'};stream['history_spec']=json.dumps(spec)
  import csv,io
  rows=list(csv.reader(io.StringIO(stream['csv'])));result=[rows[0]]
  for group in [rows[1:4],rows[4:7]]:
   a,b=group[0],group[-1];result.append(['DEMO','3m',a[2],b[3],a[4],str(max(Decimal(r[5]) for r in group)),str(min(Decimal(r[6]) for r in group)),b[7],'3000.000','true'])
  spec['calendar']['sessions'][0]['close']='2024-01-02T00:06:00Z';stream['end_ns']=str(int(stream['start_ns'])+360000000000);stream['history_spec']=json.dumps(spec)
  target=io.StringIO();csv.writer(target,lineterminator='\n').writerows(result);stream['csv']=target.getvalue();data['streams'].append(stream)
 r=run(m,alter);v=r['observations'];assert v[0]['outputs']['higher.close'] is None;assert v[1]['outputs']['higher.close'] is None;assert v[2]['outputs']['higher.close']=='1.000000000000';assert v[4]['outputs']['higher_sma.value'] is None;assert v[5]['outputs']['higher_sma.value']=='1.500000000000';assert v[6]['outputs']['higher_sma.value']=='1.500000000000'
check('multi-timeframe-asof-and-indicator-updates-once-per-source',multi_stream)
def multiple(cap='10.00',mode='net'):
 m=simple();node(m,'entry')['settings']['cashCap']=cap;m['context']['positionMode']=mode
 add(m,'second','bars',{'symbol':'SECOND','timeframe':'1m','dataset':'second','trainingDataset':'','validationDataset':'','alignment':'latest_closed','maxAgeSeconds':'86400'})
 second=copy.deepcopy(node(m,'entry'));second['id']='second_entry';second['settings']['symbol']='SECOND';m['nodes'].append(second);wire(m,'yes','value','second_entry','condition')
 def alter(data):
  stream=copy.deepcopy(data['streams'][0]);stream['id']='second';spec=json.loads(stream['history_spec']);spec['instrument']['id']='SECOND';stream['history_spec']=json.dumps(spec);stream['csv']=stream['csv'].replace('DEMO,','SECOND,');data['streams'].append(stream)
 return run(m,alter)
def shared():
 r=multiple();assert len(r['fills'])==2;assert r['metrics']['initial_equity']=='100.00';assert r['metrics']['final_equity']=='100.00'
 r=multiple('60.00');assert len(r['fills'])==1;assert any(o['status']=='rejected_insufficient_cash' for o in r['orders'])
check('two-instruments-share-one-capital-and-pending-reservations',shared)
def hedge():
 m=simple();m['context']['accountMode']='margin';m['context']['positionMode']='net';a=copy.deepcopy(node(m,'entry'));a['id']='short';a['settings']['side']='short';m['nodes'].append(a);wire(m,'yes','value','short','condition');r=run(m);assert len(r['fills'])==1
 m['context']['positionMode']='hedge';r=run(m);assert len(r['fills'])==2;assert {p['side'] for p in r['account']['positions']}=={'long','short'};assert r['metrics']['final_equity']=='100.00'
check('net-blocks-opposite-pending-hedge-retains-two-ledgers',hedge)
def delay_feedback():
 m=simple();add(m,'delay','delay',{'type':'ratio','initial':'0'});add(m,'one','constant',{'type':'ratio','value':'1'});add(m,'sum','add',{'type':'ratio'});wire(m,'delay','previous','sum','a');wire(m,'one','value','sum','b');wire(m,'sum','value','delay','next');r=run(m);assert [Decimal(v['outputs']['delay.previous']) for v in r['observations']]==list(map(Decimal,range(8)))
check('delay-feedback-reads-prior-event-only',delay_feedback)
def long_history():
 import datetime
 m=simple();wire(m,'no','value','entry','condition')
 def alter(data):
  stream=data['streams'][0];spec=json.loads(stream['history_spec']);start=datetime.datetime(2024,1,2,tzinfo=datetime.timezone.utc)
  def text(d):return d.isoformat().replace('+00:00','Z')
  rows=[stream['csv'].splitlines()[0]]
  for i in range(10000):
   a=start+datetime.timedelta(minutes=i);b=a+datetime.timedelta(minutes=1);rows.append(f'DEMO,1m,{text(a)},{text(b)},2.00,2.00,2.00,2.00,1000.000,true')
  end=start+datetime.timedelta(minutes=10000);spec['calendar']['sessions']=[{'open':text(start),'close':text(end)}];spec['acquired_at']=text(end+datetime.timedelta(minutes=1));stream['end_ns']=str(int(end.timestamp())*1000000000);stream['csv']='\n'.join(rows)+'\n';stream['history_spec']=json.dumps(spec)
 r=run(m,alter);assert r['processed_source_bars']=='10000';assert len(r['equity_curve'])==10000;assert len(r['observations'])==256;assert r['metrics']['final_equity']=='100.00'
check('continuous-10000-bars-no-financial-stitching',long_history)
def optimizer():
 import csv,io
 training=copy.deepcopy(BASE);model=graph();model['parameters'][0].update(minimum='2',maximum='3',step='1');model['parameters'][1]['optimize']=False;training['model']=json.dumps(model)
 validation=copy.deepcopy(training)
 for stream in validation['streams']:
  stream['csv']=stream['csv'].replace('2024-01-02','2024-01-03');stream['history_spec']=stream['history_spec'].replace('2024-01-02','2024-01-03')
  stream['start_ns']=str(int(stream['start_ns'])+86400000000000);stream['end_ns']=str(int(stream['end_ns'])+86400000000000)
  rows=list(csv.reader(io.StringIO(stream['csv'])))
  rows[-1][4:8]=['6.00']*4
  target=io.StringIO();csv.writer(target,lineterminator='\n').writerows(rows);stream['csv']=target.getvalue()
 request={'kind':'research_optimization','training':training,'validation':validation}
 result=run(data_override=request);o=result['optimization'];assert len(o['trials'])==2;assert o['winner_parameters']=={'fast_period':'3'};assert o['validation']['metrics']['final_equity']=='100.00';assert o['selection_basis']=='training-only;stable-first-tie';assert o['completed'] is True
 # The other parameter wins on validation, but cannot change the training winner.
 other=json.loads(validation['model']);other['parameters'][0]['value']='2';validation['model']=json.dumps(other);alternative=run(data_override=validation);assert Decimal(alternative['metrics']['final_equity'])>Decimal('100')
check('training-only-grid-winner-and-separate-validation-no-reselection',optimizer)
def reject(name,change,reason):
 def case():
  m=simple();change(m)
  try:run(m)
  except AssertionError as e:assert reason in str(e),str(e)
  else:raise AssertionError('invalid model accepted')
 check(name,case)
reject('unknown-unused-node-setting-rejected',lambda m:node(m,'bars')['settings'].update(surprise='1'),'research_unknown_setting')
reject('invalid-comparison-rejected-before-execution',lambda m:add(m,'bad','compare',{'type':'price','operator':'nonsense'},{'a':'1','b':'2'}),'research_compare_operator')
reject('over-100-percent-close-rejected',lambda m:node(m,'exit')['settings'].update(closePercent='101'),'research_close_percent')
reject('native-parameter-range-rejected',lambda m:m['parameters'][0].update(value='251'),'research_parameter_range')
reject('native-parameter-unit-mismatch-rejected',lambda m:node(m,'entry')['settings'].update(quantity='$fast_period'),'research_parameter_type')
def constant_indicator():
 m=simple();add(m,'constant_sma','sma',{'period':'2'},{'price':'2'});r=run(m);assert r['observations'][0]['outputs']['constant_sma.value'] is None;assert r['observations'][1]['outputs']['constant_sma.value']=='2.000000000000'
check('literal-indicator-advances-primary-clock',constant_indicator)
def costs():
 m=simple();m['context'].update(feeBps='0.5',slippageBps='0.5');r=run(m);assert r['fills'][0]['price']=='2.01';assert Decimal(r['metrics']['final_equity'])<Decimal('100')
check('fractional-basis-point-costs-native-tick-rounding',costs)
print(json.dumps({'format':'fqe.research-peer-checks','status':'passed','peer_sha256':hashlib.sha256(PEER.read_bytes()).hexdigest(),'checks':checks},sort_keys=True))
