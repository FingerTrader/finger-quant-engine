#!/usr/bin/env python3
"""Independent bounded private-stdio lifecycle/correlation failures; standard library only."""
import argparse, json, os, selectors, struct, subprocess, time
from pathlib import Path
MAX=262144

def encode(identity,method,params):
    return json.dumps(dict(format='fqe.desktop-request',schema_version=1,id=identity,method=method,params=params),separators=(',',':')).encode()
def send(p,raw):p.stdin.write(struct.pack('>I',len(raw))+raw);p.stdin.flush()
def exact(stream,n,deadline):
    result=bytearray()
    with selectors.DefaultSelector() as s:
        s.register(stream,selectors.EVENT_READ)
        while len(result)<n:
            if not s.select(max(0,deadline-time.monotonic())):raise TimeoutError('response deadline')
            chunk=os.read(stream.fileno(),min(n-len(result),65536))
            if not chunk:raise RuntimeError('disconnected')
            result.extend(chunk)
    return bytes(result)
def receive(p):
    deadline=time.monotonic()+5
    n=struct.unpack('>I',exact(p.stdout,4,deadline))[0];assert 0<n<=4*1024*1024
    return json.loads(exact(p.stdout,n,deadline))
def main():
    p=argparse.ArgumentParser();p.add_argument('--peer',type=Path,required=True);p.add_argument('--request',type=Path,required=True);a=p.parse_args()
    exe=str(a.peer.resolve());request=json.loads(a.request.read_bytes());cases=[]
    def launch():return subprocess.Popen([exe],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env={},cwd='/private/tmp')
    def close(child):
        try:
            if child.poll() is None:child.kill()
            child.wait(timeout=2)
        finally:
            for f in [child.stdin,child.stdout,child.stderr]:f.close()
    # Frames rejected before body allocation; EOF/truncation never hangs a session.
    for name,raw,code in [('empty',b'',0),('truncated-header',b'\x00',2),('zero-frame',b'\x00'*4,2),('huge-frame',struct.pack('>I',0xffffffff),2),('truncated-body',struct.pack('>I',3)+b'{',2)]:
        child=launch()
        try:
            out,err=child.communicate(raw,timeout=3);assert child.returncode==code and out==b'' and len(err)<65536
            cases.append(name)
        finally:close(child)
    child=launch()
    try:
        send(child,b'{"format":"fqe.desktop-request","format":"duplicate"}');r=receive(child);assert r['id'] is None and r['ok'] is False
        send(child,encode('echo.1','echo',{'maximum_u64':'18446744073709551615'}));r=receive(child);assert r['id']=='echo.1' and r['body']['maximum_u64']=='18446744073709551615'
        send(child,encode('v.1','hello',dict(major=2,minor=0,required=[])));assert receive(child)['body']['code']=='incompatible_protocol'
        send(child,encode('c.1','cancel',dict(request_id='absent')));assert receive(child)['body']['accepted'] is False
        child.stdin.close();child.wait(timeout=2);assert child.returncode==0
        cases+=['malformed-then-healthy','exact-u64-correlation','wrong-major','idle-cancel','graceful-eof']
    finally:close(child)
    child=launch()
    try:
        send(child,encode('run.1','backtest.run',request['params']))
        send(child,encode('run.2','backtest.run',request['params']))
        send(child,encode('cancel.1','cancel',dict(request_id='run.1')))
        results={r['id']:r for r in [receive(child),receive(child),receive(child)]}
        # A run may finish before another request arrives; completed vs busy are both explicit.
        assert results['run.1']['ok'] is True
        assert results['run.2']['ok'] is True or results['run.2']['body']['code']=='busy'
        assert isinstance(results['cancel.1']['body']['accepted'],bool)
        assert results['run.1']['body']['report']['terminal'] in ['cancelled','completed']
        cases.append('overlap-cancel-correlation')
    finally:close(child)
    child=launch()
    try:
        send(child,encode('run.eof','backtest.run',request['params']));child.stdin.close()
        assert receive(child)['body']['report']['terminal'] in ['cancelled','completed'];child.wait(timeout=2);assert child.returncode==0
        cases.append('eof-active-cleanup')
    finally:close(child)
    print(json.dumps(dict(format='fqe.desktop-peer-tests/1',status='passed',cases=cases,process_cases=len(cases))))
if __name__=='__main__':main()
