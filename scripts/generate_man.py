#!/usr/bin/env python3
"""Generate the option reference from the built binary, without extra packages."""
import argparse
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def escape(text):
    text = text.replace('\\', r'\e').replace('-', r'\-')
    return r'\&' + text if text.startswith(('.', "'")) else text


def generate(binary):
    help_text = subprocess.check_output([binary, '--help'], text=True)
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    options = []
    for line in help_text.split('Options:\n', 1)[1].splitlines():
        if re.match(r'  -', line):
            options.append([line.strip(), []])
        elif line.strip() and options:
            options[-1][1].append(line.strip())
    assert options, 'no CLI options found'
    sections = [f'.TH PV 1 "" "pv {version}" "User Commands"',
                '.SH NAME', 'pv \\- monitor and manage data flowing through a pipe',
                '.SH SYNOPSIS', '.B pv', '[OPTIONS] [FILES]...',
                '.SH DESCRIPTION',
                'Copy input files in order to standard output and report progress on standard error.',
                'A missing input list or a filename of \\- reads standard input.',
                'Use \\-o to write a destination file. Progress is hidden when stderr is not a terminal,',
                'unless \\-f or \\-n is selected. Binary data is passed through unchanged.',
                '.SH OPTIONS']
    for flags, description in options:
        sections += ['.TP', '.B ' + escape(flags),
                     escape(' '.join(description) or 'Print version information.')]
    sections += [(ROOT / 'docs/man-sections.roff').read_text().rstrip()]
    return '\n'.join(sections) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    generated = generate(str(Path(args.binary).resolve()))
    target = ROOT / 'docs/pv.1'
    if args.check:
        if not target.exists() or target.read_text() != generated:
            parser.exit(1, 'docs/pv.1 is stale; run scripts/generate_man.py --binary PATH\n')
    else:
        target.write_text(generated)


if __name__ == '__main__':
    main()
