#!/usr/bin/env python3
"""Independent exact-rational P2 reference; --check never writes tracked expectations.
Manual --write is for reviewed fixture creation, never invoked by tests/gate.
"""
import argparse,json
from pathlib import Path
from fractions import Fraction as F

def ema(values,p):
 r=[]; buf=[]; prev=None
 for v in values:
  if v is None: r.append(None);continue
  if prev is None:
   buf.append(v)
   if len(buf)<p:r.append(None);continue
   prev=sum(buf)/p
  else:prev=(F(2,p+1)*v+F(p-1,p+1)*prev)
  r.append(prev)
 return r

def calculate(xs):
 xs=list(map(F,xs));fast=ema(xs,2);slow=ema(xs,3);line=[a-b if a is not None and b is not None else None for a,b in zip(fast,slow)];signal=ema(line,2)
 rsi=[None]*len(xs); gains=[];losses=[];g=l=None
 for i in range(1,len(xs)):
  delta=xs[i]-xs[i-1];up=max(delta,F(0));down=max(-delta,F(0))
  if g is None:
   gains.append(up);losses.append(down)
   if len(gains)<2:continue
   g=sum(gains)/2;l=sum(losses)/2
  else:g=(g+up)/2;l=(l+down)/2
  rsi[i]=F(50) if g==l==0 else (F(100) if l==0 else 100*g/(g+l))
 cross=[False]*len(xs)
 for i in range(1,len(xs)):
  if all(v is not None for v in [line[i-1],signal[i-1],line[i],signal[i]]):cross[i]=line[i-1]<=signal[i-1] and line[i]>signal[i]
 return fast,slow,line,signal,rsi,cross

def oracle_rows(cases):
 rows=['case\tbar\tclose\tline\tsignal\trsi\tcross\trsi_ok\tbefore_remaining\tbefore_latched\ttransition\temit\tafter_remaining\tafter_latched\twindow_expired\twindow_satisfied']
 for case in cases:
  fast,slow,line,signal,rsi,cross=calculate(case['closes'])
  assert [i+1 for i,v in enumerate(cross) if v]==case['crosses']
  remaining=0;latched=False;emits=[]
  for i,price in enumerate(case['closes']):
   ok=rsi[i] is not None and rsi[i]>case['threshold'];before=(remaining,latched);transition='none';emit=False;expired=False;hit=False
   if remaining:
    latched=latched or ok
    if remaining==1:
     transition='expire';emit=latched;expired=True;hit=latched;remaining=0;latched=False
    else:transition='advance';remaining-=1
   elif cross[i]:transition='arm';remaining=3;latched=False
   if emit:emits.append(i+1)
   q=lambda v:'W' if v is None else str(round(v*10**12))
   flag=lambda v:'true' if v else 'false'
   rows.append('\t'.join(map(str,[case['name'],i+1,price,q(line[i]),q(signal[i]),q(rsi[i]),flag(cross[i]),'W' if rsi[i] is None else flag(ok),before[0],flag(before[1]),transition,flag(emit),remaining,flag(latched),flag(expired),flag(hit)])))
  assert emits==case['signals'], (case['name'],emits,case['signals'])
 return '\n'.join(rows)+'\n'
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');parser.add_argument('--check',action='store_true');args=parser.parse_args()
 root=Path(__file__).resolve().parents[2]/'crates/fqe-runtime/tests/fixtures/p2';cases=json.loads((root/'g006-cases.json').read_bytes());expected=oracle_rows(cases);path=root/'g006.expected.tsv'
 if args.write:path.write_text(expected)
 else:assert path.read_text()==expected, 'independent expected table differs'
 for case in cases:
  data=json.loads((root/f"g006-{case['name']}-data.json").read_bytes())
  assert [int(r['close']['value'])//10**12 for r in data['events']]==case['closes']
 print(f"PASS independent rational oracle: {len(cases)} cases / {len(expected.splitlines())-1} rows")
if __name__=='__main__':main()
