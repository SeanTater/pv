#!/usr/bin/env python3
"""Paired same-runner revision comparison with integrity checks and confirmation.

A gate is intentionally broad: >25% slowdown, a one-sided bootstrap lower bound
>10%, and >5 ms median increase, reproduced in a second independent batch.
"""
import argparse
import hashlib
import json
import math
import platform
import random
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from compare import execute


def assess(candidate, baseline, max_ratio=1.25, min_seconds=0.005):
    if len(candidate) != len(baseline) or len(candidate) < 5:
        raise ValueError('at least five paired samples required')
    if any(not math.isfinite(x) or x <= 0 for x in candidate + baseline):
        raise ValueError('samples must be finite and positive')
    logs = [math.log(a / b) for a, b in zip(candidate, baseline)]
    rng = random.Random(20261026)
    bootstrap = sorted(statistics.median(rng.choices(logs, k=len(logs))) for _ in range(2000))
    lower = math.exp(bootstrap[int(0.05 * len(bootstrap))])
    ratio = math.exp(statistics.median(logs))
    delta = statistics.median(candidate) - statistics.median(baseline)
    return {'median_ratio': ratio, 'lower_95_ratio': lower, 'median_delta_seconds': delta,
            'regression': ratio > max_ratio and lower > 1.10 and delta > min_seconds}


def run(args):
    paths = {name: str(Path(getattr(args, name)).resolve()) for name in ['candidate', 'baseline']}
    report = {'schema': 1, 'complete': False, 'platform': platform.platform(),
              'runs': args.runs, 'size_mib': args.size_mib, 'cases': {},
              'binaries': {name: {'version': subprocess.check_output([binary, '--version'], text=True).splitlines()[0],
                                    'sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest()}
                           for name, binary in paths.items()}}
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    def save():
        output.write_text(json.dumps(report, indent=2) + '\n')
    save()
    rng = random.Random(20261026)
    with tempfile.TemporaryDirectory(prefix='pv-regression-') as directory:
        source, destination = Path(directory) / 'input', Path(directory) / 'output'
        block = bytes(range(256)) * 4096
        with source.open('wb') as stream:
            for _ in range(args.size_mib):
                stream.write(block)
        expected = hashlib.sha256(source.read_bytes()).digest()
        for case in args.cases:
            flags = {'line': ['-q', '-l'], 'numeric': ['-n', '-b'],
                     'format': ['-f', '-F', '%t %b %r %p']}.get(case, ['-q'])
            scenario = 'pipe-pipe' if case == 'pipe-pipe' else 'file-null'
            for name, binary in paths.items():
                payload, stderr = execute(binary, flags, source, scenario, destination, capture=True)
                if hashlib.sha256(payload).digest() != expected:
                    raise RuntimeError(f'{case}/{name}: payload mismatch')
                if case == 'numeric' and stderr.splitlines()[-1] != str(source.stat().st_size):
                    raise RuntimeError(f'{case}/{name}: numeric count mismatch')
                execute(binary, flags, source, scenario, destination)  # Warm up.
            batches = []
            for attempt in range(2):
                samples = {name: [] for name in paths}
                for _ in range(args.runs):
                    order = list(paths)
                    rng.shuffle(order)
                    for name in order:
                        start = time.perf_counter()
                        execute(paths[name], flags, source, scenario, destination)
                        samples[name].append(time.perf_counter() - start)
                result = assess(samples['candidate'], samples['baseline'], args.max_ratio)
                batches.append({'samples': samples, **result})
                if not result['regression']:
                    break
            confirmed = len(batches) == 2 and all(batch['regression'] for batch in batches)
            report['cases'][case] = {'confirmed_regression': confirmed, 'batches': batches}
            save()
            print(f'{case}: ratio={batches[-1]["median_ratio"]:.3f}, confirmed regression={confirmed}', flush=True)
    report['complete'] = True
    report['passed'] = not any(case['confirmed_regression'] for case in report['cases'].values())
    save()
    return report['passed']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--baseline', required=True)
    parser.add_argument('--runs', type=int, default=7)
    parser.add_argument('--size-mib', type=int, default=64)
    parser.add_argument('--max-ratio', type=float, default=1.25)
    parser.add_argument('--output', default='regression.json')
    parser.add_argument('--cases', nargs='+', choices=['file-null', 'pipe-pipe', 'line', 'numeric', 'format'],
                        default=['file-null', 'pipe-pipe', 'line', 'numeric', 'format'])
    args = parser.parse_args()
    if args.runs < 5 or args.size_mib < 1 or not math.isfinite(args.max_ratio) or args.max_ratio <= 1:
        parser.error('use at least five runs, a positive size and a finite ratio above one')
    raise SystemExit(0 if run(args) else 1)


if __name__ == '__main__':
    # execute() uses a bounded alarm; turn it into an exception so its finally
    # block kills pipeline children on a hung binary.
    import signal
    def expired(*_):
        raise TimeoutError('benchmark command exceeded 120 seconds')
    signal.signal(signal.SIGALRM, expired)
    main()
