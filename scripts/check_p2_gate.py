#!/usr/bin/env python3
"""Offline full P2 gate. No installs, downloads, expected-output rewriting or partial mode."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / 'crates/fqe-runtime/tests/fixtures/p2/suite.json'


def require(condition, detail):
    if not condition:
        raise ValueError(detail)


def prerequisites(status):
    for task in ['P1-11'] + [f'P2-{i:02}' for i in range(1, 6)]:
        require(re.search(r'^\| ' + task + r' \| COMPLETED \|', status, re.M), 'incomplete prerequisite: ' + task)
    require(re.search(r'^\| P2-06 \| (IN_PROGRESS|IMPLEMENTED|VERIFIED|COMPLETED) \|', status, re.M), 'P2-06 must be actively implemented or closed')


def checked_file(root, name, expected):
    path = (root / name).resolve()
    require(path.is_relative_to(root.resolve()) and path.is_file(), 'invalid pinned file: ' + name)
    actual = 'sha256:' + hashlib.sha256(path.read_bytes()).hexdigest()
    require(actual == expected, 'source/fixture pin changed: ' + name)


def validate_suite(root, suite):
    require(suite['format'] == 'fqe.p2-golden-suite' and type(suite['schema_version']) is int and suite['schema_version'] == 1, 'unsupported P2 suite')
    require(suite['cases'] == ['early', 'expiry', 'late', 'arming-only', 'never', 'recross'], 'incomplete G-006 boundaries')
    require(suite['expected_rows'] == 55, 'expected independent row count differs')
    require(type(suite['minimum_rust_tests']) is int and suite['minimum_rust_tests'] >= 244, 'missing P2 test minimum')
    require(suite['novel_names'] == ['acceptance.latched_wait', 'acceptance.wait_cells'], 'missing generic-dispatch controls')
    prefix = 'crates/fqe-runtime/tests/fixtures/p2/'
    required = {prefix + f'g006-{case}-{kind}.json' for case in suite['cases'] for kind in ['data', 'schedule']}
    required.update(prefix + name for name in ['g006-workflow.json', 'g006-temporal.json', 'g006-cases.json', 'g006.expected.tsv', 'x002-shared-memory-workflow.json'])
    required.update(['scripts/check_p2_gate.py', 'tests/reference/p2_temporal_oracle.py', 'tests/p2_gate/test_runner.py', 'crates/fqe-runtime/tests/p2_gate.rs', 'crates/fqe-runtime/tests/workflow_execution.rs', 'crates/fqe-runtime/examples/temporal_strategy.rs'])
    require(required <= set(suite['source_ids']), 'missing required P2 source pins')
    for name, expected in suite['source_ids'].items():
        checked_file(root, name, expected)
    for path in (root / 'crates').glob('*/src/**/*.rs'):
        require(all(name not in path.read_text() for name in suite['novel_names']), 'fixture dispatch ID in production: ' + str(path))


def main():
    report = dict(format='fqe.p2-gate-report', schema_version=1, status='running', full_checks=True, checks=[], cases=[], environment=dict(platform=sys.platform, architecture=platform.machine(), python=platform.python_version()))

    def command(name, args, timeout=420):
        result = subprocess.run(args, cwd=ROOT, capture_output=True, timeout=timeout)
        require(result.returncode == 0, name + ': ' + (result.stdout + result.stderr).decode(errors='replace')[-3000:])
        report['checks'].append(dict(name=name, exit=result.returncode))
        print('PASS ' + name, file=sys.stderr, flush=True)
        return result.stdout

    try:
        suite = json.loads(SUITE.read_bytes())
        prerequisites((ROOT / 'docs/PROJECT_STATUS.md').read_text())
        report['checks'].append(dict(name='completed-prerequisites', exit=0))
        validate_suite(ROOT, suite)
        report['source_ids'] = suite['source_ids']
        report['checks'].append(dict(name='p2-source-pins-and-generic-dispatch', exit=0))
        command('independent-rational-oracle', [sys.executable, 'tests/reference/p2_temporal_oracle.py', '--check'])
        command('gate-negative-controls', [sys.executable, '-m', 'unittest', 'discover', '-s', 'tests/p2_gate', '-v'])
        p1 = json.loads(command('full-p1-and-workspace-regression', [sys.executable, 'scripts/check_p1_gate.py']))
        require(p1['status'] == 'passed' and p1['full_checks'] is True, 'partial/failed regression')
        count = next(c['passed'] for c in p1['checks'] if c['name'] == 'rust-tests')
        require(count >= suite['minimum_rust_tests'], 'P2 Rust acceptance tests missing')
        report['regression'] = p1
        example = command('actual-g006-example', ['cargo', 'run', '--locked', '--offline', '-p', 'fqe-runtime', '--example', 'temporal_strategy']).decode()
        require(example.count('Long sequence=8 timestamp_ns=8000\n') == 1 and example.count('Long sequence=') == 1, 'example signal differs')
        require('window=wait opened=5 expired_ns=8000 satisfied=true\n' in example and 'signal_count=1\n' in example, 'example temporal boundary differs')
        report['example_stdout'] = example
        report['cases'] = [dict(name=name, status='passed-in-workspace', expected_signals=[8] if name in ['early', 'expiry', 'recross'] else []) for name in suite['cases']]
        report['independent_rows'] = suite['expected_rows']
        report['status'] = 'passed'
    except (ValueError, KeyError, OSError, subprocess.TimeoutExpired) as error:
        report['status'] = 'failed'
        report['error'] = str(error)
    report['verified_at'] = datetime.now(timezone.utc).isoformat()
    print(json.dumps(report, sort_keys=True))
    return 0 if report['status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
