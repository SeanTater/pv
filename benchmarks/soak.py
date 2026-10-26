#!/usr/bin/env python3
"""Sustained Linux payload/hash and RSS-growth checks, with child-only limits."""
import argparse
import hashlib
import json
import math
import os
import platform
import resource
import statistics
import subprocess
import tempfile
import threading
import time
from pathlib import Path


def assess(samples, warmup_seconds, max_rss_mib, growth_mib):
    warm = [rss for elapsed, rss in samples if elapsed >= warmup_seconds]
    if len(warm) < 5:
        raise ValueError('insufficient post-warmup samples')
    baseline = statistics.median(warm[:5])
    peak = max(rss for _, rss in samples)
    growth = max(warm) - baseline
    return {'baseline_rss_bytes': baseline, 'peak_rss_bytes': peak, 'growth_bytes': growth,
            'passed': peak <= max_rss_mib * 2**20 and growth <= growth_mib * 2**20}


def scenario(binary, flags, duration, rate_mib, max_rss_mib, growth_mib):
    block = bytes(range(256)) * 256
    sent, received = hashlib.sha256(), hashlib.sha256()
    counts = {'sent': 0, 'received': 0}
    failures = []
    stop = threading.Event()
    started = time.monotonic()
    def limit():
        resource.setrlimit(resource.RLIMIT_AS, (128 * 2**20, 128 * 2**20))
    with tempfile.TemporaryFile() as diagnostics:
        child = subprocess.Popen([binary, *flags], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=diagnostics, preexec_fn=limit)
        def producer():
            next_send = time.monotonic()
            try:
                while not stop.is_set() and time.monotonic() - started < duration:
                    child.stdin.write(block)
                    sent.update(block)
                    counts['sent'] += len(block)
                    next_send += len(block) / (rate_mib * 2**20)
                    stop.wait(max(0, next_send - time.monotonic()))
                child.stdin.close()
            except Exception as error:
                failures.append(str(error))
        def consumer():
            try:
                while True:
                    data = child.stdout.read(65536)
                    if not data:
                        break
                    received.update(data)
                    counts['received'] += len(data)
            except Exception as error:
                failures.append(str(error))
        writer = threading.Thread(target=producer, daemon=True)
        reader = threading.Thread(target=consumer, daemon=True)
        samples = []
        try:
            writer.start()
            reader.start()
            while child.poll() is None:
                elapsed = time.monotonic() - started
                if elapsed > duration + 15:
                    raise TimeoutError('soak transfer did not finish within its watchdog')
                try:
                    status = Path(f'/proc/{child.pid}/status').read_text()
                    rss = next(int(line.split()[1]) * 1024 for line in status.splitlines() if line.startswith('VmRSS:'))
                    samples.append((elapsed, rss))
                except (FileNotFoundError, StopIteration):
                    pass  # Process can exit between poll and reading /proc.
                time.sleep(0.1)
            writer.join(timeout=5)
            reader.join(timeout=5)
            diagnostics.seek(0)
            if child.returncode or writer.is_alive() or reader.is_alive() or failures:
                raise RuntimeError(f'child={child.returncode}; threads/errors={failures}; {diagnostics.read().decode(errors="replace")[-2000:]}')
            if counts['sent'] != counts['received'] or sent.digest() != received.digest():
                raise RuntimeError('sustained transfer changed or lost payload bytes')
            if time.monotonic() - started < duration or counts['sent'] < 2**20:
                raise RuntimeError('soak completed before its required duration/volume')
            assessment = assess(samples, max(2, duration * 0.2), max_rss_mib, growth_mib)
            return {'seconds': time.monotonic() - started, 'bytes': counts['sent'], 'sha256': sent.hexdigest(),
                    'samples': samples, **assessment}
        finally:
            stop.set()
            if child.poll() is None:
                child.kill()
            child.wait()
            writer.join(timeout=5)
            reader.join(timeout=5)
            child.stdin.close()
            child.stdout.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--duration', type=float, default=60, help='seconds per scenario')
    parser.add_argument('--rate-mib', type=float, default=20)
    parser.add_argument('--max-rss-mib', type=float, default=64)
    parser.add_argument('--growth-mib', type=float, default=8)
    parser.add_argument('--output', default='soak.json')
    args = parser.parse_args()
    if platform.system() != 'Linux':
        parser.error('RSS collection and child resource limits require Linux')
    if any(not math.isfinite(value) or value <= 0 for value in [args.duration, args.rate_mib, args.max_rss_mib, args.growth_mib]) or not 10 <= args.duration <= 3600:
        parser.error('use positive finite limits and a duration between 10 and 3600 seconds')
    binary = str(Path(args.binary).resolve())
    report = {'schema': 1, 'complete': False, 'platform': platform.platform(), 'cases': {},
              'binary_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest()}
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + '\n')
    for name, flags in [('buffered', ['-q', '-C', '-B', '64K']),
                        ('history-rates', ['-n', '-C', '-B', '64K', '-i', '0.05', '-m', '1', '-F', '%b|%32A|%L'])]:
        report['cases'][name] = scenario(binary, flags, args.duration, args.rate_mib, args.max_rss_mib, args.growth_mib)
        output.write_text(json.dumps(report, indent=2) + '\n')
        case = report['cases'][name]
        print(f'{name}: {case["bytes"]} bytes, RSS peak {case["peak_rss_bytes"]/2**20:.2f} MiB, growth {case["growth_bytes"]/2**20:.2f} MiB', flush=True)
    report['complete'] = True
    report['passed'] = all(case['passed'] for case in report['cases'].values())
    output.write_text(json.dumps(report, indent=2) + '\n')
    raise SystemExit(0 if report['passed'] else 1)


if __name__ == '__main__':
    main()
