#!/usr/bin/env python3
"""Independent exact-rational formulas; explicit regeneration, never run by tests.
No engine import. Fractions preserve unrounded recurrence; output quantization is
integer round-to-nearest ties-to-even, independent of Decimal/Rust implementation.
"""
from fractions import Fraction as F
import json
from pathlib import Path

FACTOR = 10**12

def even(q):
    whole, rem = divmod(abs(q.numerator), q.denominator)
    if 2*rem > q.denominator or (2*rem == q.denominator and whole % 2):
        whole += 1
    return -whole if q < 0 else whole

def ema(xs, p):
    result, previous = [], None
    alpha = F(2, p+1)
    for k, x in enumerate(xs):
        if k+1 < p:
            result.append(None)
            continue
        previous = sum(xs[:p])/p if previous is None else alpha*x+(1-alpha)*previous
        result.append(previous)
    return result

def reference(xs):
    fast, slow = ema(xs, 2), ema(xs, 3)
    line = [None if s is None else f-s for f, s in zip(fast, slow)]
    ready_signal = iter(ema([x for x in line if x is not None], 2))
    signal = [None if x is None else next(ready_signal) for x in line]
    hist = [None if s is None else l-s for l, s in zip(line, signal)]
    gain, loss = [], []
    for a, b in zip(xs, xs[1:]):
        gain.append(max(b-a, F(0))); loss.append(max(a-b, F(0)))
    rsi, g, l = [None], None, None
    for k in range(len(gain)):
        if k < 1:
            rsi.append(None); continue
        if g is None:
            g, l = sum(gain[:2])/2, sum(loss[:2])/2
        else:
            g, l = (g+gain[k])/2, (l+loss[k])/2
        rsi.append(F(50) if g+l == 0 else 100*g/(g+l))
    sma = lambda p: [None if k+1 < p else sum(xs[k+1-p:k+1])/p for k in range(len(xs))]
    return dict(sma2=sma(2), sma3=sma(3), ema2=fast, ema3=slow, line=line, signal=signal, histogram=hist, rsi2=rsi)

sets = {
    'G-002': [1,2,3,2,4,3], 'G-007': [3,2,1,2,3,4],
    'constant': [7]*8, 'rising': list(range(8)), 'falling': list(range(8,0,-1)),
    'signed': [-3,-2,0,-4,2,-1,5,4], 'short': [2],
    'large': [10**12, -10**12, 10**12, 0, -10**12, 10**12],
    'long': [F(((i*37)%101)-50, 7) for i in range(512)],
}
# Inputs are exact 12-place decimals. References consume these quantized inputs.
fixtures = []
for name, xs in sets.items():
    closes = [even(F(x)*FACTOR) for x in xs]
    refs = reference([F(x, FACTOR) for x in closes])
    fixtures.append(dict(id=name, closes=list(map(str, closes)),
                         expected={k:[None if x is None else str(even(x*FACTOR)) for x in v] for k,v in refs.items()}))
path = Path('crates/fqe-ir/tests/fixtures/indicators.json')
path.write_text(json.dumps(dict(version=1, reference='exact rational recurrence, mean seeds, Wilder/flat50, ties-even output12',
                               max_coefficient_error='1', fixtures=fixtures), indent=2)+'\n')
print(f'{len(fixtures)} independent datasets, {sum(len(x["closes"]) for x in fixtures)} samples -> {path}')

# Compare candidate working scales against exact rational references. This path
# uses Fraction algebra with quantization at formula boundaries (not Rust loops).
def working_reference(xs, scale):
    quant = lambda x: F(even(x*10**scale),10**scale)
    def qe(xs,p):
        out, previous = [], None
        for k,x in enumerate(xs):
            if k+1 < p:
                out.append(None); continue
            previous = quant(sum(xs[:p])/p if previous is None else F(2,p+1)*x+F(p-1,p+1)*previous)
            out.append(previous)
        return out
    fast,slow=qe(xs,2),qe(xs,3)
    line=[None if s is None else f-s for f,s in zip(fast,slow)]
    si=iter(qe([x for x in line if x is not None],2))
    signal=[None if l is None else next(si) for l in line]
    hist=[None if s is None else l-s for l,s in zip(line,signal)]
    rsi,g,l=[None],None,None
    for k in range(1,len(xs)):
        if k<2:
            rsi.append(None);continue
        if g is None:
            ds=[xs[1]-xs[0],xs[2]-xs[1]]
            g,l=quant(sum(max(x,F(0)) for x in ds)/2),quant(sum(max(-x,F(0)) for x in ds)/2)
        else:
            d=xs[k]-xs[k-1]
            g,l=quant((g+max(d,F(0)))/2),quant((l+max(-d,F(0)))/2)
        rsi.append(F(50) if g+l==0 else quant(100*g/(g+l)))
    return dict(ema2=fast,ema3=slow,line=line,signal=signal,histogram=hist,rsi2=rsi)

report = ['# P1-06 Numeric Evaluation', '',
          'Generated deliberately by scripts/generate_indicator_fixtures.py, Python stdlib Fraction; no engine import or binary invocation.', '',
          'Exact unrounded rational recurrences versus candidate quantized working scales; both report HalfEven scale-12 outputs. Inputs are the tracked exact scale-12 coefficients. Maximum absolute errors over the nine datasets (563 samples):', '',
          '| Output | work12 error | work18 error |', '| --- | --- | --- |']
for output in ['ema2','ema3','line','signal','histogram','rsi2']:
    errors=[]
    for scale in [12,18]:
        maximum=F(0)
        for data in fixtures:
            xs=[F(int(x),FACTOR) for x in data['closes']]
            exact=reference(xs)[output]; candidate=working_reference(xs,scale)[output]
            for a,b in zip(exact,candidate):
                if a is not None:
                    maximum=max(maximum,abs(a-F(even(b*FACTOR),FACTOR)))
        errors.append(f'{float(maximum):.12g}')
    report.append(f'| {output} | {errors[0]} | {errors[1]} |')
report += ['', 'Selection: work18/output12, explicit HalfEven at each seed, recurrence and output; preserves six guard digits. Work12 increases accumulated/RSI error. Binary64 cannot distinguish one 1e-12 output increment near 1e12; see public Rust boundary tests. No new decimal dependency is needed in this bounded slice.', '',
           'These measurements cover periods 2/3 and the listed datasets, not every financial series. Contract error bounds, supported range and RSI conditioning guard are in INDICATOR_CONTRACT.md. Public Rust tests separately compare all outputs to the tracked exact-rational rounded coefficients within one output coefficient (1e-12), and assert readiness and strict crossing decisions. The candidate experiment is not a claim that Rust was executed by this script.', '']
Path('docs/P1_06_NUMERIC_EVALUATION.md').write_text('\n'.join(report))
print('\n'.join(report[7:16]))
