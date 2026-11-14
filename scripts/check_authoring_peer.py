#!/usr/bin/env python3
"""Independent framed authoring/financial oracle. Public fixtures and stdlib only."""
import argparse
from fractions import Fraction
import hashlib
import json
import os
from pathlib import Path
import select
import struct
import subprocess
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--peer', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    fixtures = root / 'crates/fqe-bridge/tests/fixtures'
    source = (root / 'examples/desktop/sma-cross.fqe').read_text()
    calls = []

    def rpc(method, params):
        payload = json.dumps(dict(format='fqe.desktop-request', schema_version=1,
                                  id='author.oracle', method=method, params=params)).encode()
        child = subprocess.Popen([str(args.peer.resolve())], stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            child.stdin.write(struct.pack('>I', len(payload))+payload)
            child.stdin.flush()
            deadline = time.monotonic()+15

            def read_exact(count):
                chunks = []
                while count:
                    remaining = deadline-time.monotonic()
                    if remaining <= 0 or not select.select([child.stdout], [], [], remaining)[0]:
                        raise TimeoutError('native response deadline')
                    chunk = os.read(child.stdout.fileno(), count)
                    if not chunk:
                        raise ValueError('partial native frame')
                    chunks.append(chunk)
                    count -= len(chunk)
                return b''.join(chunks)

            size = struct.unpack('>I', read_exact(4))[0]
            assert 0 < size <= 4194304
            response = json.loads(read_exact(size))
            # EOF is a cancellation request: keep the pipe open until the run replies.
            child.stdin.close()
            assert child.wait(timeout=15) == 0 and not child.stderr.read()
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=5)
        assert response['id'] == 'author.oracle' and response['ok'], response
        calls.append(method)
        return response['body']

    rpc('hello', dict(major=1, minor=0, required=['catalog.describe/1', 'authoring.event/1']))
    inspected = rpc('authoring.inspect', dict(source=source, parameters='{"version":1,"values":{}}'))
    assert inspected['valid'] and not inspected['diagnostics']
    document = inspected['document']
    printed = rpc('authoring.print', dict(document=document, comments=inspected['comments']))
    assert printed['valid'] and inspected['comments'][0] in printed['source']
    second = rpc('authoring.inspect', dict(source=printed['source'], parameters='{"version":1,"values":{}}'))
    assert second['native']['execution_id'] == inspected['native']['execution_id']
    request = dict(kind='event', model=json.dumps(document['model']),
                   parameters='{"version":1,"values":{}}',
                   history_spec=(fixtures/'sma-cross-history-spec.json').read_text(),
                   csv=(fixtures/'sma-cross-history.csv').read_text(),
                   start_ns='1704153600000000000', end_ns='1704154080000000000',
                   warmup_bars='0', run='authoring.oracle', seed='18446744073709551615',
                   policies=document['policies'],
                   account=dict(id='offline', profile='cash_long', initial_cash='100.00',
                                participation_bps='10000', fee_bps='0', minimum_fee='0.00',
                                slippage_bps='0', risk=None))
    run = rpc('backtest.run', request)
    report = run['report']
    assert report['metrics']['final_equity'] == '98.00'
    assert report['metrics']['fees'] == '0.00'
    assert [(f['source_bar'], f['price'], f['quantity']) for f in report['fills']] == [
        ('6', '4.00', '1.000'), ('8', '2.00', '1.000')]
    # Fixed hand oracle: buying at4 uses cash96; marking at2/1 gives98/97;
    # next-open sale at2 returns cash98, with no hidden forced close or fee.
    assert [Fraction(p['cash']) for p in report['equity_curve']] == list(map(Fraction, [100]*5+[96,96,98]))
    assert [Fraction(p['equity']) for p in report['equity_curve']] == list(map(Fraction, [100]*5+[98,97,98]))
    request['model'] = (fixtures/'sma-cross-model.json').read_text()
    graph = rpc('backtest.run', request)['report']
    for field in ['metrics', 'account', 'valuation', 'equity_curve']:
        assert graph[field] == report[field]
    invalid = rpc('authoring.inspect', dict(source=source.replace('above(', 'unknown('),
                                           parameters='{"version":1,"values":{}}'))
    assert not invalid['valid'] and 'native' not in invalid
    result = dict(format='fqe.authoring-peer-evidence/1', status='passed', calls=calls,
                  peer_sha256=hashlib.sha256(args.peer.read_bytes()).hexdigest(),
                  source_sha256=hashlib.sha256(source.encode()).hexdigest(),
                  execution_id=inspected['native']['execution_id'], result_id=run['result_id'],
                  final_equity='98.00', fill_bars=['6', '8'],
                  checks=['framed-native-authoring', 'exact-semantic-print-roundtrip',
                          'comments-retained', 'independent-native-graph-parity',
                          'hand-cash-and-equity-curve', 'next-open-fill-prices',
                          'invalid-source-has-no-executable', 'clean-child-exit'])
    args.output.write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
