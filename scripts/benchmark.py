"""Generate real files and benchmark MCP calls; no third-party Python packages.

Data is intentionally retained for repeat runs. Remove --root when finished.
This does not purge OS caches and never labels measurements as cold-cache.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import threading
import time
from mcp_client import Client


def generate(root, count, size):
    root.mkdir(parents=True, exist_ok=True)
    marker = root / ".gbk-benchmark.json"
    if marker.exists():
        data = json.loads(marker.read_text())
        if data == {"files": count, "bytes_per_file": size, "complete": True}:
            return
        raise RuntimeError("Existing benchmark metadata differs; select a new root")
    if any(root.iterdir()):
        raise RuntimeError("Generator requires an empty directory")
    if shutil.disk_usage(root).free < count * size * 1.3:
        raise RuntimeError("Insufficient free space for fixture plus filesystem overhead")
    payloads = {}
    for codec in ["gbk", "utf8"]:
        for hit in [False, True]:
            header = ("// 编码中\n// GBK_MCP_" + ("HIT" if hit else "MISS") + "\n").encode(codec)
            payloads[codec, hit] = header + (b"int benchmark_value = 42; // padding\n" * ((size // 36) + 10))[:size-len(header)-1] + b"\n"
            assert len(payloads[codec, hit]) == size
    started = time.perf_counter()
    for i in range(count):
        codec = "gbk" if i % 2 == 0 else "utf8"
        directory = root / codec / f"{i // 1000:05d}"
        directory.mkdir(parents=True, exist_ok=True)
        (directory / f"file_{i:07d}.c").write_bytes(payloads[codec, i % 100 == 0])
        if i and i % 100000 == 0:
            print(json.dumps({"generated": i, "seconds": round(time.perf_counter()-started, 2)}), flush=True)
    marker.write_text(json.dumps({"files": count, "bytes_per_file": size, "complete": True}))


def rss_kib(pid):
    try:
        if platform.system() == "Linux":
            for line in Path(f"/proc/{pid}/status").read_text().splitlines():
                if line.startswith("VmRSS:"):
                    return int(line.split()[1])
        if platform.system() == "Darwin":
            return int(subprocess.check_output(["ps", "-o", "rss=", "-p", str(pid)], stderr=subprocess.DEVNULL).strip())
    except (OSError, ValueError, subprocess.SubprocessError):
        pass
    return None


def benchmark(binary, root, threads):
    report = {"platform": platform.platform(), "logical_cpus": os.cpu_count(), "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "threads_budget": threads, "cache_state": "warm_or_uncontrolled; OS caches not purged",
              "dataset": json.loads((root / ".gbk-benchmark.json").read_text()), "cases": {}}
    peaks, stop = [], threading.Event()
    with Client(binary, root, threads) as client:
        def sample():
            while not stop.wait(0.05):
                value = rss_kib(client.process.pid)
                if value is not None:
                    peaks.append(value)
        sampler = threading.Thread(target=sample, daemon=True)
        sampler.start()
        cases = {
            "auto_first_100": {"pattern": "GBK_MCP_HIT", "fixed_strings": True},
            "auto_full": {"pattern": "GBK_MCP_ABSENT_NEVER", "fixed_strings": True, "output_mode": "files", "timeout_ms": 300000},
            "gbk_explicit_full": {"pattern": "GBK_MCP_ABSENT_NEVER", "fixed_strings": True, "output_mode": "files", "path": "gbk", "encoding": "gbk", "timeout_ms": 300000},
            "utf8_explicit_full": {"pattern": "GBK_MCP_ABSENT_NEVER", "fixed_strings": True, "output_mode": "files", "path": "utf8", "encoding": "utf8", "timeout_ms": 300000},
        }
        for name, query in cases.items():
            started = time.perf_counter()
            result = client.tool("grep", query)
            report["cases"][name] = {"wall_ms": (time.perf_counter()-started)*1000, **result}
            print(json.dumps({"case": name, "wall_ms": report["cases"][name]["wall_ms"], "complete": result["complete"], "visited_files": result["visited_files"]}), flush=True)
        started = time.perf_counter()
        requests = [client.send("tools/call", {"name": "grep", "arguments": cases[name]}) for name in ["gbk_explicit_full", "utf8_explicit_full"]]
        concurrent = [json.loads(client.receive(i)["content"][0]["text"]) for i in requests]
        report["cases"]["two_concurrent"] = {"wall_ms": (time.perf_counter()-started)*1000, "results": concurrent}
        stop.set()
        sampler.join(timeout=2)
    report["peak_rss_kib"] = max(peaks) if peaks else None
    if shutil.which("rg"):
        for codec in ["gbk", "utf8"]:
            started = time.perf_counter()
            result = subprocess.run(["rg", "--no-mmap", "--threads", str(max(1,threads//2)), "--encoding", codec, "-F", "-l", "GBK_MCP_ABSENT_NEVER", str(root/codec)], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            if result.returncode not in (0, 1):
                raise RuntimeError(result.stderr.decode(errors="replace"))
            elapsed = (time.perf_counter()-started)*1000
            report["cases"][f"rg_{codec}_full"] = {"wall_ms": elapsed}
            report["cases"][f"{codec}_explicit_full"]["rg_ratio"] = report["cases"][f"{codec}_explicit_full"]["wall_ms"] / elapsed
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/release/gbk-mcp"))
    parser.add_argument("--root", type=Path, default=Path("benchmark-data/scale-10000"))
    parser.add_argument("--files", type=int, default=10000)
    parser.add_argument("--bytes-per-file", type=int, default=10240)
    parser.add_argument("--threads", type=int, default=8)
    parser.add_argument("--generate", action="store_true")
    parser.add_argument("--output", type=Path, default=Path("benchmark-results/latest.json"))
    args = parser.parse_args()
    if args.files < 1 or args.bytes_per_file < 256:
        parser.error("files must be positive and bytes-per-file >=256")
    root = args.root.resolve()
    if args.generate:
        generate(root, args.files, args.bytes_per_file)
    report = benchmark(args.binary.resolve(), root, args.threads)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"report": str(args.output), "peak_rss_kib": report["peak_rss_kib"]}), flush=True)
