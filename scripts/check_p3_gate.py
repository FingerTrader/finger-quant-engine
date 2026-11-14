#!/usr/bin/env python3
"""Complete offline P3 financial gate; no installs, downloads, golden rewriting or partial mode."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile
ROOT=Path(__file__).resolve().parents[1]
SUITE=ROOT/'crates/fqe-runtime/tests/fixtures/p3/suite.json'
CASES=['exact-finance-history','cash-long-ledger','restricted-short-ledger','margin-debt-liquidation-gap-partial-costs','next-open-order-lifecycle','event-workflow-financial-report','training-only-grid-validation','complete-financial-prefix-restore']
REQUIRED={
 'scripts/check_p3_gate.py','tests/p3_gate/test_runner.py',
 'crates/fqe-core/src/finance.rs','crates/fqe-core/src/utc.rs','crates/fqe-core/tests/financial_contract.rs',
 'crates/fqe-ir/src/history.rs','crates/fqe-ir/src/history_csv.rs','crates/fqe-ir/src/financial_exchange.rs','crates/fqe-ir/tests/history_contract.rs',
 *('crates/fqe-runtime/src/'+n+'.rs' for n in ['ledger','margin','simulator','history','portfolio','backtest','backtest_encoding','optimization','optimization_encoding','financial_checkpoint','checkpoint_file']),
 *('crates/fqe-runtime/tests/'+n+'.rs' for n in ['history_execution','ledger_accounting','order_simulation','margin_simulation','backtest','optimization','financial_checkpoint']),
 *('tests/reference/p3_'+n+'_oracle.py' for n in ['ledger','margin','backtest','optimization']),
 *('crates/fqe-runtime/examples/'+n+'.rs' for n in ['historical_inputs','virtual_ledger','simulated_orders','margin_simulation','backtest','optimization','financial_checkpoint']),
}
def require(condition,detail):
    if not condition:raise ValueError(detail)
def prerequisites(status):
    for task in ['P1-11','P2-06']+[f'P3-{n:02}' for n in range(1,7)]:require(re.search(r'^\| '+task+r' \| COMPLETED \|',status,re.M),'incomplete prerequisite: '+task)
    require(re.search(r'^\| P3-07 \| (IN_PROGRESS|IMPLEMENTED|VERIFIED|COMPLETED) \|',status,re.M),'P3-07 must be actively implemented or closed')
def checked_file(root,name,expected):
    p=(root/name).resolve();require(p.is_relative_to(root.resolve()) and p.is_file(),'invalid pinned path: '+name)
    require('sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()==expected,'source/fixture pin changed: '+name)
def validate_suite(root,suite):
    require(suite['format']=='fqe.p3-financial-suite' and type(suite['schema_version']) is int and suite['schema_version']==1,'unsupported P3 suite')
    require(suite['cases']==CASES,'missing financial profile/case')
    require(type(suite['minimum_rust_tests']) is int and suite['minimum_rust_tests']>=324,'missing financial tests')
    require(suite['independent_cases']=={'ledger':5,'margin':4,'metrics':9,'optimization':3},'incomplete independent oracle coverage')
    required=REQUIRED|{str(p.relative_to(ROOT)) for p in (ROOT/'crates/fqe-runtime/tests/fixtures/p3').iterdir() if p.is_file() and p.name!='suite.json'}|{str(p.relative_to(ROOT)) for p in (ROOT/'crates/fqe-ir/tests/fixtures/history').iterdir() if p.is_file()}
    require(required<=set(suite['source_ids']),'missing required financial pins')
    for name,pin in suite['source_ids'].items():checked_file(root,name,pin)
    for p in (root/'crates').glob('*/src/**/*.rs'):
        require(all(name not in p.read_text() for name in ['public.financial-risk.continuation','unlisted.test.setting']),'fixture-name dispatch in production')
def main():
    report=dict(format='fqe.p3-gate-report',schema_version=1,status='running',full_checks=True,checks=[],environment=dict(platform=sys.platform,architecture=platform.machine(),python=platform.python_version()))
    def command(name,args,timeout=900):
        r=subprocess.run(args,cwd=ROOT,capture_output=True,timeout=timeout);require(r.returncode==0,name+': '+(r.stdout+r.stderr).decode(errors='replace')[-4000:]);report['checks'].append(dict(name=name,exit=r.returncode));print('PASS '+name,file=sys.stderr);return r.stdout
    try:
        prerequisites((ROOT/'docs/PROJECT_STATUS.md').read_text());report['checks'].append(dict(name='completed-prerequisites',exit=0))
        suite=json.loads(SUITE.read_bytes());validate_suite(ROOT,suite);report['checks'].append(dict(name='financial-source-fixture-pins-and-generic-dispatch',exit=0));report['source_ids']=suite['source_ids']
        command('gate-negative-controls',[sys.executable,'-m','unittest','discover','-s','tests/p3_gate','-v'])
        for oracle in ['ledger','margin','backtest','optimization']:command('independent-'+oracle,[sys.executable,f'tests/reference/p3_{oracle}_oracle.py','--check'])
        p2=json.loads(command('full-p2-p1-workspace-regression',[sys.executable,'scripts/check_p2_gate.py']));require(p2['status']=='passed' and p2['full_checks'] is True,'partial/failed prior gate');count=next(c['passed'] for c in p2['regression']['checks'] if c['name']=='rust-tests');require(count>=suite['minimum_rust_tests'],'financial acceptance tests missing');report['regression']=p2
        for example in ['historical_inputs','virtual_ledger','simulated_orders','margin_simulation','financial_checkpoint']:
            output=command('actual-'+example,['cargo','run','--locked','--offline','-p','fqe-runtime','--example',example]).decode();report.setdefault('examples',{})[example]=output
            if example=='financial_checkpoint':require('restored_in_fresh_child=true source_prefix=5 pending_hold=1 continued_bars=2' in output,'missing actual fresh-child financial continuation')
        with tempfile.TemporaryDirectory(prefix='fqe-p3-gate-') as name:
            for example,oracle in [('backtest','backtest'),('optimization','optimization')]:
                output=command('actual-'+example,['cargo','run','--locked','--offline','-p','fqe-runtime','--example',example,'--','--json']);p=Path(name)/(example+'.json');p.write_bytes(output);command('independent-actual-'+example,[sys.executable,f'tests/reference/p3_{oracle}_oracle.py','--check','--report',str(p)]);report.setdefault('report_source_ids',{})[example]='sha256:'+hashlib.sha256(output).hexdigest()
        command('docs-final',[sys.executable,'scripts/check_docs.py']);command('diff-final',['git','diff','--check']);report['independent_cases']=suite['independent_cases'];report['financial_process_cases']=2;report['cases']=[dict(name=n,status='passed') for n in CASES];report['status']='passed'
    except (ValueError,KeyError,OSError,subprocess.TimeoutExpired) as e:report['status']='failed';report['error']=str(e)
    report['verified_at']=datetime.now(timezone.utc).isoformat();print(json.dumps(report,sort_keys=True));return 0 if report['status']=='passed' else 1
if __name__=='__main__':raise SystemExit(main())
