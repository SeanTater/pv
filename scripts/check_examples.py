#!/usr/bin/env python3
"""Exercise the local README/recipe contracts with temporary files and no services."""
import argparse
import os
from summarize_trace import summarize
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', required=True)
args = parser.parse_args()
binary = str(Path(args.binary).resolve())
payload = bytes(range(256)) * 8
with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    (root / 'input.bin').write_bytes(payload)
    (root / 'first.bin').write_bytes(payload[:100])
    (root / 'second.bin').write_bytes(payload[100:])
    (root / 'records.txt').write_bytes(b'one\ntwo\n')
    (root / 'records.bin').write_bytes(b'one\0two\0')
    for flags, data in [
        (['input.bin'], payload), (['-q'], payload),
        (['-q', 'first.bin', 'second.bin'], payload),
        (['-L', '10M', 'input.bin'], payload),
        (['-s', '100', '-S', 'input.bin'], payload[:100]),
        (['-l', '-n', '-b', 'records.txt'], b'one\ntwo\n'),
        (['-0', '-n', '-b', 'records.bin'], b'one\0two\0'),
        (['-n', '-b', 'input.bin'], payload),
        (['-f', '-N', 'backup', '-F', '%N %b %r %e', 'input.bin'], payload),
        (['-f', '-T', '-A', '16', 'input.bin'], payload),
        (['-q', '-v', 'input.bin'], payload),
        (['-X', '-n', '-b', 'input.bin'], b''),
        (['-q', '-U', 'spool.bin', 'input.bin'], payload),
    ]:
        result = subprocess.run([binary, *flags], cwd=root, input=payload,
                                capture_output=True, timeout=30, check=True)
        assert result.stdout == data, flags
        if '-n' in flags:
            count = 2 if '-l' in flags or '-0' in flags else len(payload)
            assert result.stderr.splitlines()[-1] == str(count).encode(), flags
        if '-v' in flags:
            assert b'rate min/avg/max/mdev' in result.stderr
    subprocess.run([binary, '-q', '-o', 'output.bin', 'input.bin'], cwd=root,
                   timeout=30, check=True)
    assert (root / 'output.bin').read_bytes() == payload
    assert (root / 'spool.bin').read_bytes() == payload
    result = subprocess.run([binary, 'input.bin'], cwd=root, capture_output=True,
                            timeout=30, check=True)
    compressed = subprocess.run(['gzip', '-c'], input=result.stdout, capture_output=True, check=True)
    restored = subprocess.run(['gzip', '-dc'], input=compressed.stdout, capture_output=True, check=True)
    assert restored.stdout == payload
print('15 local recipe contracts passed (external services excluded).')

# Exercise the actual Rust/Python trace schema together, not only synthetic data.
trace = subprocess.run([binary, '-q', '-C'], input=payload, capture_output=True,
                       timeout=30, check=True, env={**os.environ, 'PV_TRACE': '1'})
assert trace.stdout == payload
summary = summarize(trace.stderr.decode().splitlines())
assert summary['complete']
assert sum(stream['bytes'] for stream in summary['transfers'].values()) == len(payload)
print('Rust trace and Python summary agree.')
