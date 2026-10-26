#!/usr/bin/env python3
"""Summarize PV_TRACE=1 stderr records; progress/error text is ignored."""
import argparse
import json
from pathlib import Path


def summarize(lines):
    operations, transfers = {}, {}
    for line in lines:
        if 'pv-trace: ' not in line:
            continue
        event = json.loads(line.partition('pv-trace: ')[2])
        if event['schema'] != 1:
            raise ValueError('unsupported trace schema')
        for field in ['bytes', 'total_bytes', 'elapsed_ns', 'duration_ns']:
            if not isinstance(event[field], int) or event[field] < 0:
                raise ValueError(f'invalid {field}')
        transfer = transfers.setdefault(f"{event['pid']}:{event['transfer']}", {'bytes': 0, 'elapsed_ns': 0, 'stopped': False})
        transfer['bytes'] = max(transfer['bytes'], event['total_bytes'])
        transfer['elapsed_ns'] = max(transfer['elapsed_ns'], event['elapsed_ns'])
        key = event['operation']
        transfer['stopped'] |= key == 'stop'
        entry = operations.setdefault(key, {'calls': 0, 'bytes': 0, 'duration_ns': 0, 'max_ns': 0, 'errors': 0})
        entry['calls'] += 1
        entry['bytes'] += event['bytes']
        entry['duration_ns'] += event['duration_ns']
        entry['max_ns'] = max(entry['max_ns'], event['duration_ns'])
        entry['errors'] += event.get('error_kind') is not None
    if not transfers:
        raise ValueError('no pv trace records found')
    return {'schema': 1, 'complete': all(t['stopped'] for t in transfers.values()),
            'transfers': transfers, 'operations': operations}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('trace', type=Path)
    args = parser.parse_args()
    with args.trace.open() as stream:
        result = summarize(stream)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
