#!/usr/bin/env python3
"""Independent managed session/256-bar benchmark, exact native parity and child-only RSS."""
import argparse,copy,csv,io,json,resource,subprocess,sys,time
from datetime import datetime,timedelta,timezone
from fractions import Fraction as Number
from pathlib import Path
from check_desktop_peer import encode,send,receive

def workload(base,threshold):
    p=copy.deepcopy(base);start=datetime(2024,1,2,tzinfo=timezone.utc);end=start+timedelta(minutes=256)
    spec=json.loads(p['history_spec']);spec['calendar']['sessions'][0]['close']=end.isoformat().replace('+00:00','Z');spec['acquired_at']=(end+timedelta(minutes=1)).isoformat().replace('+00:00','Z')
    p['history_spec']=json.dumps(spec);p['end_ns']=str((end-datetime(1970,1,1,tzinfo=timezone.utc)).days*86400*10**9+(end-datetime(1970,1,1,tzinfo=timezone.utc)).seconds*10**9)
    rows=io.StringIO();writer=csv.writer(rows,lineterminator='\n');writer.writerow(['instrument_id','timeframe','open_time','close_time','open','high','low','close','volume','is_closed'])
    prices=[1,2,3,2,4,3,2]
    for i in range(256):
        price=f'{prices[i%7]}.00';writer.writerow(['DEMO','1m',(start+timedelta(minutes=i)).isoformat().replace('+00:00','Z'),(start+timedelta(minutes=i+1)).isoformat().replace('+00:00','Z'),price,price,price,price,'2.000','true'])
    p['csv']=rows.getvalue();bindings=json.loads(p['parameters']);bindings['values']['threshold']['value']=str(threshold*10**12);p['parameters']=json.dumps(bindings);p['account']['initial_cash']='1000000.00'
    return p

def reconcile(report):
    cash=Number(report['manifest']['account']['initial_cash']);quantity=Number(0);fees=Number(0)
    for fill in report['fills']:
        gross=Number(fill['price'])*Number(fill['quantity']);fee=Number(fill['fee']);fees+=fee
        if fill['side']=='buy':cash-=gross+fee;quantity+=Number(fill['quantity'])
        else:cash+=gross-fee;quantity-=Number(fill['quantity'])
    assert cash==Number(report['account']['cash'])
    assert fees==Number(report['metrics']['fees'])
    assert cash+quantity*Number(report['equity_curve'][-1]['mark'])==Number(report['metrics']['final_equity'])

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--peer',type=Path,required=True);parser.add_argument('--request',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);a=parser.parse_args()
    peer=subprocess.Popen([str(a.peer.resolve())],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env={},cwd='/private/tmp');serial=0;session=None;cases=[];metrics=[]
    def call(method,params,ok=True,managed=True):
        nonlocal serial
        serial+=1;identity=f'independent.{serial}'
        if managed:params={'session_id':session,**params}
        send(peer,encode(identity,method,params));result=receive(peer);assert result['id']==identity
        assert result['ok'] is ok,(method,result)
        return result['body']
    def wait(handle):
        deadline=time.monotonic()+5;samples=[]
        while True:
            start=time.perf_counter();status=call('run.status',dict(handle=handle));samples.append((time.perf_counter()-start)*1000)
            if status['terminal']:return status,samples
            assert time.monotonic()<deadline,'256-bar terminal deadline';time.sleep(.001)
    try:
        hello=call('hello',dict(major=1,minor=0,required=['run.managed/1']),managed=False);session=hello['session_id']
        assert call('run.status',dict(handle='managed.1'),ok=False)['code']=='not_initialized'
        init=call('run.initialize',{},managed=False);assert init['session_id']==session;cases.append('initialize-version-capabilities-session')
        base=json.loads(a.request.read_bytes())['params']
        for label,threshold in [('no-trade',90),('many-fill',60)]:
            p=workload(base,threshold);started=time.perf_counter();job=call('run.start',dict(operation_id=label,input=p));h=job['handle'];status,controls=wait(h);elapsed=time.perf_counter()-started
            assert status['phase']=='completed' and status['processed_source_bars']=='256'
            result=call('run.result',dict(handle=h));report=result['report'];reconcile(report)
            assert len(report['fills'])==0 if label=='no-trade' else len(report['fills'])>=32
            direct=call('backtest.run',p,managed=False);assert direct==result,'one-shot/managed complete native parity'
            duplicate=call('run.start',dict(operation_id=label,input=p));assert duplicate['handle']==h and duplicate['duplicate'] is True
            events=call('run.events',dict(handle=h,after='0',limit='16'));last=events['next_cursor'];assert len(events['events'])<=4 and events['terminal'] is True
            assert call('run.events',dict(handle=h,after=last,limit='16'))['events']==[]
            controls.sort();p95=controls[int((len(controls)-1)*.95)]
            assert elapsed<=5 and p95<=250
            metrics.append(dict(workload=label,bars=256,fills=len(report['fills']),elapsed_ms=elapsed*1000,control_p95_ms=p95,control_samples=len(controls),result_id=result['result_id'],final_equity=report['metrics']['final_equity']))
            cases+=['actual-'+label+'-native-parity','idempotent-'+label,'stable-cursor-'+label]
        cancel=call('run.start',dict(operation_id='cancel',input=workload(base,60)));h=cancel['handle'];clock=time.perf_counter();call('run.stop',dict(handle=h));status,_=wait(h);assert status['phase'] in ['completed','cancelled'];assert time.perf_counter()-clock<=2;cases.append('stop-race-terminal')
        failed=call('run.start',dict(operation_id='invalid',input={}));bad=failed['handle'];assert wait(bad)[0]['phase']=='failed';assert call('run.result',dict(handle=bad),ok=False)['code']=='run_failed';cases.append('failed-no-stale-result')
        assert call('run.start',dict(operation_id='overload',input={}),ok=False)['code']=='run_quota';cases.append('retained-run-quota')
        call('run.release',dict(handle=bad));assert call('run.start',dict(operation_id='invalid',input={}))['released'] is True;cases.append('release-retains-operation-tombstone')
        serial+=1;send(peer,encode(f'independent.{serial}','run.status',dict(session_id='old.process',handle=h)));assert receive(peer)['body']['code']=='session_mismatch';cases.append('stale-process-session-rejected')
        peer.stdin.close();assert peer.wait(timeout=2)==0;cases.append('graceful-native-shutdown')
        assert len(peer.stderr.read())<=65536
        rss=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss*(1 if sys.platform=='darwin' else 1024)
        assert rss<=192*1024*1024,'native child RSS budget'
        result=dict(format='fqe.managed-native-peer/1',status='passed',cases=cases,workloads=metrics,peak_native_rss_bytes=rss,rss_limit_bytes=192*1024*1024,platform=sys.platform)
        a.output.write_text(json.dumps(result,sort_keys=True,indent=2)+'\n');print(json.dumps(result,sort_keys=True))
    finally:
        if peer.poll() is None:peer.kill();peer.wait(timeout=2)
        for stream in [peer.stdin,peer.stdout,peer.stderr]:stream.close()
if __name__=='__main__':main()
