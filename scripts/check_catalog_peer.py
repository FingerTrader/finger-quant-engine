#!/usr/bin/env python3
"""Independent stdlib framed consumer for contextual discovery; never installs packages."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--peer', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    model = json.loads((root / 'crates/fqe-runtime/tests/fixtures/p3/backtest-events.json').read_text())
    requests = [
        ('hello', dict(major=1, minor=0, required=['catalog.describe/1'])),
        ('catalog.describe', dict(trigger=model['trigger'])),
        ('model.inspect', dict(kind='event', model=json.dumps(model), parameters=(
            root / 'crates/fqe-cli/tests/fixtures/parameters-60.json').read_text())),
        ('catalog.describe', dict(trigger=dict(kind='tick', instrument='DEMO'))),
        ('catalog.describe', dict(trigger={**model['trigger'], 'unexpected': True})),
    ]
    frames = []
    for i, (method, params) in enumerate(requests):
        payload = json.dumps(dict(format='fqe.desktop-request', schema_version=1,
                                  id=f'catalog.{i}', method=method, params=params)).encode()
        frames.append(struct.pack('>I', len(payload)) + payload)
    child = subprocess.run([str(args.peer.resolve())], input=b''.join(frames),
                           capture_output=True, timeout=15)
    if child.returncode != 0 or child.stderr:
        raise ValueError('peer did not exit cleanly after EOF')
    replies, offset = [], 0
    while offset < len(child.stdout):
        if offset + 4 > len(child.stdout):
            raise ValueError('partial frame header')
        length = struct.unpack('>I', child.stdout[offset:offset+4])[0]
        offset += 4
        if not 0 < length <= 4 * 1024 * 1024 or offset + length > len(child.stdout):
            raise ValueError('bad response frame')
        replies.append(json.loads(child.stdout[offset:offset+length]))
        offset += length
    assert len(replies) == len(requests)
    assert all(r['id'] == f'catalog.{i}' for i, r in enumerate(replies))
    assert all(r['ok'] for r in replies[:4]) and not replies[4]['ok']
    catalog, inspected, tick = [r['body'] for r in replies[1:4]]
    assert catalog['registry_id'] == inspected['registry_id']
    assert catalog['descriptors'] == inspected['descriptors']
    assert catalog['support']['limits']['financial_source_bars'] == '256'
    assert not catalog['support']['optimization']
    assert not catalog['support']['external_languages']
    assert not any(d['category'] == 'indicator' for d in tick['descriptors'])
    result = dict(format='fqe.catalog-peer-evidence/1', status='passed',
                  peer_sha256=hashlib.sha256(args.peer.read_bytes()).hexdigest(),
                  requests=len(requests), registry_id=catalog['registry_id'],
                  descriptor_count=len(catalog['descriptors']),
                  checks=['actual-framed-peer', 'capability-negotiation',
                          'model-independent-discovery', 'old-inspect-registry-parity',
                          'tick-domain-has-no-bar-indicators', 'extra-field-rejection',
                          'truthful-limits-and-unsupported-flags', 'clean-eof-exit'])
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
