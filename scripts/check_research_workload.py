#!/usr/bin/env python3
"""Bounded native research workload oracle and per-child RSS, stdlib only.

Budgets fixed before measurement: 20 s end-to-end, 512 MiB native child RSS,
4 MiB request and 32 MiB response. RSS is measured with wait4 (Darwin bytes,
Linux KiB), never the Python consumer or cumulative prior child maximum.
"""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import selectors
import struct
import subprocess
import sys
import time
import threading

ROOT = Path(__file__).resolve().parents[1]
PEER = Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / 'target/release/fqe-desktop-peer').resolve()
TIME_LIMIT = 20
RSS_LIMIT = 512 * 1024 * 1024


def fixture(count):
    data = json.loads((ROOT / 'crates/fqe-bridge/tests/fixtures/research-input.json').read_text())
    model = json.loads(data['model'])
    model['nodes'] = [n for n in model['nodes'] if n['id'] in ('bars', 'entry', 'exit')]
    for name, value in [('yes', 'true'), ('no', 'false')]:
        model['nodes'].append(dict(id=name, kind='constant', label=name,
                                   settings=dict(type='bool', value=value), literals={}))
    model['wires'] = [dict(**{'from': name}, output='value', to=target, input='condition')
                      for name, target in [('yes', 'entry'), ('no', 'exit')]]
    data['model'] = json.dumps(model)
    stream = data['streams'][0]
    spec = json.loads(stream['history_spec'])
    start = dt.datetime(2024, 1, 2, tzinfo=dt.timezone.utc)
    text = lambda date: date.isoformat().replace('+00:00', 'Z')
    end = start + dt.timedelta(minutes=count)
    rows = [stream['csv'].splitlines()[0]]
    for i in range(count):
        a = start + dt.timedelta(minutes=i)
        b = a + dt.timedelta(minutes=1)
        price = '3.00' if i == count - 1 else '2.00'
        rows.append(f'DEMO,1m,{text(a)},{text(b)},{price},{price},{price},{price},1000.000,true')
    spec['calendar']['sessions'] = [dict(open=text(start), close=text(end))]
    spec['acquired_at'] = text(end + dt.timedelta(minutes=1))
    stream.update(csv='\n'.join(rows) + '\n', history_spec=json.dumps(spec),
                  end_ns=str(int(end.timestamp()) * 1000000000))
    return data


def measure(count):
    data = fixture(count)
    start = time.monotonic()
    child = subprocess.Popen([str(PEER)], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    watchdog = threading.Timer(TIME_LIMIT, child.kill)
    watchdog.daemon = True
    watchdog.start()
    selector = selectors.DefaultSelector()
    selector.register(child.stdout, selectors.EVENT_READ)
    serial, largest_response, largest_request = 0, 0, 0

    def read(size):
        result = bytearray()
        while len(result) < size:
            remaining = TIME_LIMIT - (time.monotonic() - start)
            assert remaining > 0 and selector.select(remaining), 'response deadline'
            part = os.read(child.stdout.fileno(), size - len(result))
            assert part, 'disconnected'
            result.extend(part)
        return bytes(result)

    def call(method, params):
        nonlocal serial, largest_response, largest_request
        serial += 1
        raw = json.dumps(dict(format='fqe.desktop-request', schema_version=1,
                              id=str(serial), method=method, params=params), separators=(',', ':')).encode()
        largest_request = max(largest_request, len(raw))
        assert len(raw) <= 4 * 1024 * 1024
        child.stdin.write(struct.pack('>I', len(raw)) + raw)
        child.stdin.flush()
        size = struct.unpack('>I', read(4))[0]
        assert size <= 32 * 1024 * 1024, size
        largest_response = max(largest_response, size)
        value = json.loads(read(size))
        assert value['ok'], value
        return value['body']

    try:
        session = call('run.initialize', {})['session_id']
        handle = call('run.start', dict(session_id=session, operation_id='workload', input=data))['handle']
        args = dict(session_id=session, handle=handle)
        while not (status := call('run.status', args))['terminal']:
            assert time.monotonic() - start < TIME_LIMIT
            time.sleep(0.01)
        if status['phase'] == 'failed':
            # Source-count admission is independent of encoded report capacity.
            # Report encoding remains bounded even when source admission succeeds.
            assert count == 20000 and status['error']['code'] == 'response_quota', status
            call('run.release', args)
            child.stdin.close()
            _, exit_status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(exit_status)
            assert child.returncode == 0
            rss = int(usage.ru_maxrss * (1 if sys.platform == 'darwin' else 1024))
            elapsed = time.monotonic() - start
            assert elapsed < TIME_LIMIT and rss <= RSS_LIMIT, (elapsed, rss)
            return dict(source_bars=count, outcome='response_quota', seconds=round(elapsed, 3),
                        native_peak_rss_bytes=rss, max_request_bytes=largest_request,
                        max_response_bytes=largest_response, result_available=False)
        assert status['phase'] == 'completed', status
        result = call('run.result', args)
        report = result['report']
        assert report['processed_source_bars'] == str(count)
        assert len(report['equity_curve']) == count
        assert [p['equity'] for p in report['equity_curve']] == ['100.00'] * (count - 1) + ['101.00']
        assert all(p['fees'] == '0.00' for p in report['equity_curve'])
        assert len(report['observations']) == min(count, 256)
        assert len(report['fills']) == 1, report['fills']
        assert report['fills'][0]['price'] == '2.00'
        assert report['fills'][0]['quantity'] == '1.000'
        assert report['metrics']['final_equity'] == '101.00', report['metrics']
        assert report['account']['cash_hub']['reserved_cash'] == '0.00'
        encoded = json.dumps(report, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
        assert result['result_id'] == 'sha256:' + hashlib.sha256(b'FQE-BACKTEST-RESULT/1\n' + encoded).hexdigest()
        assert call('run.result', args)['result_id'] == result['result_id']
        assert call('run.status', args)['high_watermark'] == status['high_watermark']
        call('run.release', args)
        child.stdin.close()
        _, exit_status, usage = os.wait4(child.pid, 0)
        child.returncode = os.waitstatus_to_exitcode(exit_status)
        assert child.returncode == 0
        rss = int(usage.ru_maxrss * (1 if sys.platform == 'darwin' else 1024))
        elapsed = time.monotonic() - start
        assert elapsed < TIME_LIMIT and rss <= RSS_LIMIT, (elapsed, rss)
        return dict(source_bars=count, outcome='completed', seconds=round(elapsed, 3), native_peak_rss_bytes=rss,
                    max_request_bytes=largest_request, max_response_bytes=largest_response,
                    final_equity='101.00', fills=1, result_id=result['result_id'])
    finally:
        watchdog.cancel()
        selector.close()
        if child.returncode is None:
            child.kill()
            child.wait(timeout=5)
        child.stdout.close()
        if not child.stdin.closed:
            child.stdin.close()


if __name__ == '__main__':
    assert hasattr(os, 'wait4'), 'this workload measurement needs POSIX wait4'
    rows = [measure(n) for n in (8, 10000, 20000)]
    print(json.dumps(dict(format='fqe.research-workload/1', status='passed',
                         measured_at=dt.datetime.now(dt.timezone.utc).isoformat(),
                         platform=platform.platform(), peer_sha256=hashlib.sha256(PEER.read_bytes()).hexdigest(),
                         budgets=dict(seconds=TIME_LIMIT, native_peak_rss_bytes=RSS_LIMIT), measurements=rows), indent=2))
