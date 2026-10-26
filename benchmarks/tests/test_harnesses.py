"""Negative controls prove the monitors reject bad behavior, not just good pv."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from soak import scenario

ROOT = Path(__file__).resolve().parents[1]


def executable(path, body):
    path.write_text('#!/usr/bin/env python3\n' + body)
    path.chmod(0o755)
    return str(path)


class HarnessControls(unittest.TestCase):
    def regression(self, directory, candidate_body):
        base = executable(directory / 'baseline', '''import sys
if '--version' in sys.argv: print('pv fixture')
else: sys.stdout.buffer.write(open(sys.argv[-1], 'rb').read())
''')
        candidate = executable(directory / 'candidate', candidate_body)
        output = directory / 'result.json'
        result = subprocess.run([sys.executable, str(ROOT / 'regression.py'), '--candidate', candidate,
                                 '--baseline', base, '--runs', '5', '--size-mib', '1',
                                 '--cases', 'file-null', '--output', str(output)],
                                capture_output=True, text=True, timeout=30)
        return result, json.loads(output.read_text())

    def test_slow_program_fails_only_after_confirmation(self):
        with tempfile.TemporaryDirectory() as directory:
            result, report = self.regression(Path(directory), '''import sys, time
if '--version' in sys.argv: print('pv fixture')
else:
    time.sleep(0.1)
    sys.stdout.buffer.write(open(sys.argv[-1], 'rb').read())
''')
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertTrue(report['complete'])
            self.assertFalse(report['passed'])
            self.assertTrue(report['cases']['file-null']['confirmed_regression'])
            self.assertEqual(len(report['cases']['file-null']['batches']), 2)

    def test_corrupt_fast_program_cannot_pass_as_an_improvement(self):
        with tempfile.TemporaryDirectory() as directory:
            result, report = self.regression(Path(directory), '''import sys
if '--version' in sys.argv: print('pv fixture')
else: sys.stdout.buffer.write(b'corrupted')
''')
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(report['complete'])
            self.assertIn('payload mismatch', result.stderr)

    def test_leaking_program_preserves_payload_but_fails_memory_monitor(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = executable(Path(directory) / 'leak', '''import sys, time
heap = []
next_allocation = time.monotonic()
while True:
    data = sys.stdin.buffer.read(65536)
    if not data: break
    sys.stdout.buffer.write(data)
    while time.monotonic() >= next_allocation:
        heap.append(bytearray(512 * 1024))
        next_allocation += 0.25
''')
            report = scenario(binary, [], 10, 1, 64, 8)
            self.assertFalse(report['passed'])
            self.assertGreater(report['growth_bytes'], 8 * 2**20)
            self.assertGreater(report['bytes'], 2**20)


if __name__ == '__main__': unittest.main()
