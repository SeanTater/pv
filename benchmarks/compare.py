#!/usr/bin/env python3
"""Repeatable, fail-closed comparison of explicit pv binaries.

Integrity checks run outside timed sections. Timings include process startup and
pipeline producers/consumers; CPU time is measured for all child processes.
"""

import argparse
import hashlib
import json
import os
import platform
import random
import resource
import signal
import statistics
import subprocess
import tempfile
import time
from contextlib import ExitStack
from pathlib import Path


def version(binary):
    return subprocess.check_output([binary, "--version"], text=True).splitlines()[0]


def execute(binary, flags, source, scenario, destination, capture=False):
    producers = []
    children = []
    with ExitStack() as resources:
        stderr_file = resources.enter_context(tempfile.TemporaryFile())
        if capture:
            out = resources.enter_context(tempfile.TemporaryFile())
        else:
            out = resources.enter_context(open(os.devnull, "wb"))
        try:
            signal.setitimer(signal.ITIMER_REAL, 120)
            if scenario in ["pipe-pipe", "slow-consumer"]:
                input_file = resources.enter_context(open(source, "rb"))
                producer = subprocess.Popen(
                    ["cat"], stdin=input_file, stdout=subprocess.PIPE
                )
                producers.append(producer)
                children.append(producer)
                pv = subprocess.Popen(
                    [binary, *flags],
                    stdin=producer.stdout,
                    stdout=subprocess.PIPE,
                    stderr=stderr_file,
                )
                children.append(pv)
                producer.stdout.close()
                slow_consumer = (
                    "import sys, time\n"
                    "while True:\n"
                    "    data = sys.stdin.buffer.read(4096)\n"
                    "    if not data:\n"
                    "        break\n"
                    "    sys.stdout.buffer.write(data)\n"
                    "    time.sleep(0.0005)\n"
                )
                consumer_command = (
                    ["cat"]
                    if scenario == "pipe-pipe"
                    else ["python3", "-c", slow_consumer]
                )
                consumer = subprocess.Popen(
                    consumer_command, stdin=pv.stdout, stdout=out
                )
                children.append(consumer)
                pv.stdout.close()
                status = pv.wait()
                if consumer.wait():
                    raise RuntimeError("consumer failed")
            else:
                command = [binary, *flags, str(source)]
                if scenario == "file-file":
                    command += ["-o", str(destination)]
                status = subprocess.run(
                    command, stdout=out, stderr=stderr_file, check=False
                ).returncode
            stderr_file.seek(0)
            stderr = stderr_file.read().decode(errors="replace")
            if status:
                raise RuntimeError(f"{binary}: exit {status}: {stderr}")
            for producer in producers:
                if producer.wait():
                    raise RuntimeError("producer failed")
            if capture:
                out.seek(0)
                return out.read(), stderr
            return None, stderr
        finally:
            signal.setitimer(signal.ITIMER_REAL, 0)
            for child in children:
                if child.poll() is None:
                    child.kill()
                    child.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", default="target/release/pv")
    parser.add_argument("--reference", default="/usr/bin/pv")
    parser.add_argument("--runs", type=int, default=9)
    parser.add_argument("--size-mib", type=int, default=256)
    parser.add_argument("--output", default="benchmarks/results.json")
    parser.add_argument(
        "--cases",
        nargs="+",
        choices=[
            "file-null",
            "pipe-pipe",
            "file-file",
            "line",
            "display",
            "numeric",
            "format",
            "rate",
            "slow-consumer",
        ],
        default=[
            "file-null",
            "pipe-pipe",
            "file-file",
            "line",
            "display",
            "numeric",
            "format",
            "rate",
            "slow-consumer",
        ],
    )
    args = parser.parse_args()
    if args.runs < 3 or args.size_mib < 1:
        parser.error("use at least three runs and a positive data size")
    rust, reference = (
        str(Path(args.rust).resolve()),
        str(Path(args.reference).resolve()),
    )
    if os.path.samefile(rust, reference):
        parser.error("Rust and reference binaries must differ")
    versions = {"rust": version(rust), "original": version(reference)}
    if versions["original"] != "pv 1.12.0":
        parser.error(f"reference must be pv 1.12.0, got {versions['original']}")
    variants = {
        "rust": (rust, []),
        "rust-buffered": (rust, ["-C"]),
        "original": (reference, []),
        "original-buffered": (reference, ["-C"]),
    }
    results = {
        "versions": versions,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpu": next(
            (
                line.split(":", 1)[1].strip()
                for line in Path("/proc/cpuinfo").read_text().splitlines()
                if line.startswith("model name")
            ),
            platform.processor(),
        )
        if Path("/proc/cpuinfo").exists()
        else platform.processor(),
        "date_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "runs": args.runs,
        "size_mib": args.size_mib,
        "cache": "warm; one warmup per variant and case",
        "cases": {},
    }

    def timeout_handler(signum, frame):
        raise TimeoutError("benchmark command exceeded 120 seconds")

    signal.signal(signal.SIGALRM, timeout_handler)
    results["binary_sha256"] = {
        name: hashlib.sha256(Path(binary).read_bytes()).hexdigest()
        for name, binary in [("rust", rust), ("original", reference)]
    }
    results["git_revision"] = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], text=True
    ).strip()
    results["git_dirty"] = bool(
        subprocess.check_output(["git", "status", "--porcelain"])
    )
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    results["complete"] = False
    results["requested_cases"] = args.cases

    def save():
        output.write_text(json.dumps(results, indent=2) + "\n")

    save()
    rng = random.Random(20260930)
    with tempfile.TemporaryDirectory(prefix="pv-bench-") as directory:
        source, destination = Path(directory) / "input", Path(directory) / "output"
        block = (bytes(range(251)) * 5)[:1023] + b"\n"
        with source.open("wb") as f:
            for _ in range(args.size_mib):
                f.write(block * 1024)
        full_source = source
        small_source = Path(directory) / "small"
        rate_source = Path(directory) / "rate"
        with full_source.open("rb") as f:
            small_source.write_bytes(f.read(min(args.size_mib, 4) * 1024 * 1024))
        with full_source.open("rb") as f:
            rate_source.write_bytes(f.read(min(args.size_mib, 64) * 1024 * 1024))
        for case in args.cases:
            source = (
                small_source
                if case == "slow-consumer"
                else rate_source
                if case == "rate"
                else full_source
            )
            expected_hash = hashlib.sha256(source.read_bytes()).hexdigest()
            flags = {
                "line": [
                    "-q",
                    "-l",
                    "-s",
                    str(args.size_mib * 1024 * block.count(b"\n")),
                ],
                "display": ["-f", "-i", "0.01"],
                "numeric": ["-n", "-b", "-i", "0.01"],
                "format": ["-f", "-i", "0.01", "-F", "%t %b %r %p"],
                "rate": ["-q", "-L", "64M"],
            }.get(case, ["-q"])
            scenario = (
                case
                if case in ["pipe-pipe", "file-file", "slow-consumer"]
                else "file-null"
            )
            timings, cpus = (
                {name: [] for name in variants},
                {name: [] for name in variants},
            )
            for name, (binary, extra) in variants.items():
                # Validate exact content, and verify numeric final counters.
                payload, stderr = execute(
                    binary, extra + flags, source, scenario, destination, capture=True
                )
                checked = (
                    destination.read_bytes() if scenario == "file-file" else payload
                )
                if hashlib.sha256(checked).hexdigest() != expected_hash:
                    raise RuntimeError(f"payload mismatch: {case}/{name}")
                if case == "numeric" and stderr.splitlines()[-1] != str(
                    source.stat().st_size
                ):
                    raise RuntimeError(f"numeric count mismatch: {case}/{name}")
                execute(binary, extra + flags, source, scenario, destination)
            for _ in range(args.runs):
                order = list(variants)
                rng.shuffle(order)
                for name in order:
                    binary, extra = variants[name]
                    before = resource.getrusage(resource.RUSAGE_CHILDREN)
                    start = time.perf_counter()
                    execute(binary, extra + flags, source, scenario, destination)
                    duration = time.perf_counter() - start
                    if case == "rate" and duration + 0.11 < source.stat().st_size / (
                        64 * 1024 * 1024
                    ):
                        raise RuntimeError(f"throttle completed too quickly: {name}")
                    timings[name].append(duration)
                    after = resource.getrusage(resource.RUSAGE_CHILDREN)
                    cpus[name].append(
                        after.ru_utime
                        + after.ru_stime
                        - before.ru_utime
                        - before.ru_stime
                    )
            results["cases"][case] = {}
            for name, samples in timings.items():
                median = statistics.median(samples)
                results["cases"][case][name] = {
                    "seconds": samples,
                    "median_seconds": median,
                    "min_seconds": min(samples),
                    "max_seconds": max(samples),
                    "median_cpu_seconds": statistics.median(cpus[name]),
                    "gib_per_second": source.stat().st_size / 2**30 / median,
                    "payload_bytes": source.stat().st_size,
                }
            # Derive comparison from measured medians, never table row order.
            original = results["cases"][case]["original"]["median_seconds"]
            ours = results["cases"][case]["rust"]["median_seconds"]
            save()
            print(
                f"{case}: Rust {ours * 1000:.2f} ms, original {original * 1000:.2f} ms; original/Rust {original / ours:.3f}x",
                flush=True,
            )
    results["complete"] = True
    save()
    print(f"Raw results: {output}")


if __name__ == "__main__":
    main()
