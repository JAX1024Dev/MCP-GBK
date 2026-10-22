"""Optional live-client test. Uses installed, authenticated Claude/Codex clients.

This performs small model calls against disposable fixtures and incurs normal
client usage. It does not disable built-in tools; project instructions must route
the five file operations to this MCP server.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

PROJECT = Path(__file__).resolve().parent.parent
PROMPT = "查找项目中的 .c 文件，在其中检索‘中文原文’，读取对应文件，将‘中文原文’精确替换成‘中文修改’，再次读取确认，然后新建 result.txt 写入‘验证完成’（不加末尾换行）。只修改这两个文件，其他内容保持原样。请按项目指令操作，简短报告结果。"


def run_client(name, binary, output):
    with tempfile.TemporaryDirectory(prefix=f"gbk-mcp-{name}-") as folder:
        root = Path(folder)
        before = "// 中文原文\r\nconst int value = 1;\r\n"
        (root / "legacy.c").write_bytes(before.encode("gbk"))
        shutil.copyfile(PROJECT / "examples" / ("CLAUDE.md" if name == "claude" else "AGENTS.md"), root / ("CLAUDE.md" if name == "claude" else "AGENTS.md"))
        if name == "claude":
            config = root / ".mcp.json"
            config.write_text(json.dumps({"mcpServers": {"gbk": {"type": "stdio", "command": str(binary), "args": ["--root", str(root)]}}}))
            command = ["claude", "-p", PROMPT, "--mcp-config", str(config), "--strict-mcp-config",
                       "--no-session-persistence", "--no-chrome",
                       "--output-format", "stream-json", "--verbose", "--max-budget-usd", "1",
                       "--allowedTools", "mcp__gbk__read_file,mcp__gbk__write_file,mcp__gbk__edit_file,mcp__gbk__glob,mcp__gbk__grep"]
        else:
            command = ["codex", "exec", "--ignore-user-config", "--ephemeral", "--skip-git-repo-check",
                       "--sandbox", "workspace-write", "--json", "-C", str(root),
                       "-c", "mcp_servers.gbk.command=" + json.dumps(str(binary)),
                       "-c", "mcp_servers.gbk.args=" + json.dumps(["--root", str(root)]),
                       "-c", "mcp_servers.gbk.required=true", "-c", 'mcp_servers.gbk.tools.edit_file.approval_mode="approve"',
                       "-c", 'mcp_servers.gbk.tools.write_file.approval_mode="approve"', PROMPT]
        started = time.perf_counter()
        result = subprocess.run(command, cwd=root, capture_output=True, text=True, encoding="utf-8", timeout=180)
        output.mkdir(parents=True, exist_ok=True)
        (output / f"{name}.jsonl").write_text(result.stdout, encoding="utf-8")
        (output / f"{name}.stderr.txt").write_text(result.stderr, encoding="utf-8")
        calls = []
        for line in result.stdout.splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("type") == "assistant":
                for block in event.get("message", {}).get("content", []):
                    if isinstance(block, dict) and block.get("type") == "tool_use":
                        calls.append(block.get("name", ""))
            item = event.get("item", {})
            if event.get("type") == "item.completed" and item.get("type") == "mcp_tool_call":
                calls.append(f'mcp__{item.get("server")}__{item.get("tool")}')
        expected = before.replace("中文原文", "中文修改").encode("gbk")
        file_ok = (root / "legacy.c").read_bytes() == expected
        new_ok = (root / "result.txt").exists() and (root / "result.txt").read_bytes() == "验证完成".encode()
        required = {f"mcp__gbk__{tool}" for tool in ["glob", "grep", "read_file", "edit_file", "write_file"]}
        routed = required.issubset(calls) and calls.count("mcp__gbk__read_file") >= 2
        report = {"client": name, "returncode": result.returncode, "wall_ms": (time.perf_counter()-started)*1000,
                  "gbk_bytes_preserved": file_ok, "new_file_utf8": new_ok, "mcp_tool_calls": calls,
                  "all_five_tools_routed": routed, "passed": result.returncode == 0 and file_ok and new_ok and routed}
        (output / f"{name}.summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps(report, ensure_ascii=False), flush=True)
        return report["passed"]


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/release/gbk-mcp"))
    parser.add_argument("--client", choices=["claude", "codex", "both"], default="both")
    parser.add_argument("--output", type=Path, default=Path("benchmark-results/agents"))
    args = parser.parse_args()
    passed = []
    for name in (["claude", "codex"] if args.client == "both" else [args.client]):
        passed.append(run_client(name, args.binary.resolve(), args.output.resolve()))
    raise SystemExit(0 if all(passed) else 1)
