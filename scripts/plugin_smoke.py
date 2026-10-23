"""Exercise the real Claude plugin/hook executor with a local scripted model.

No model credentials or external model calls. The loopback server deliberately
attempts native Read, then uses MCP. This tests execution, not LLM tool selection.
--live separately exercises tool selection using installed client authentication.
"""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import threading
import time

from package_plugin import stage_marketplace

PLUGIN_ID = "gbk-mcp@jax1024dev-gbk"
PREFIX = "mcp__plugin_gbk-mcp_gbk__"
BEFORE = "// 中文原文\r\nconst int value = 1;\r\n"


def cli(command, root, env, timeout=120):
    result = subprocess.run(["claude", *command], cwd=root, env=env,
                            capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    if result.returncode:
        raise AssertionError(f"Claude command failed ({command[:3]}): {result.stdout[-4000:]} {result.stderr[-2000:]}")
    return result


def walk_strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, list):
        for item in value:
            yield from walk_strings(item)
    elif isinstance(value, dict):
        for item in value.values():
            yield from walk_strings(item)


class Model(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
        if "count_tokens" in self.path:
            data = json.dumps({"input_tokens": 100}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(data)
            return
        messages = request.get("messages", [])
        child = any("GBK_HOOK_SUBAGENT_PROBE" in block.get("text", "")
                    for message in messages if message.get("role") == "user"
                    for block in (message.get("content", []) if isinstance(message.get("content"), list)
                                  else [{"text": message.get("content", "")}]))
        lane = "child" if child else "main"
        step = self.server.steps[lane]
        self.server.steps[lane] += 1
        results = [block for message in messages if isinstance(message.get("content"), list)
                   for block in message["content"] if block.get("type") == "tool_result"]
        self.server.results.extend(results)
        self.server.results_by_lane[lane].extend(results)
        if step == 0:
            name, args = "Read", {"file_path": str(self.server.root / "legacy.c")}
        elif self.server.variant != "normal":
            name, args = None, None
        elif step == 1 or (not child and step == 3):
            name, args = PREFIX + "read_file", {"path": "legacy.c", "encoding": "gbk"}
        elif not child and step == 2:
            revision = None
            for text in walk_strings(results):
                match = re.search(r'"revision"\s*:\s*"([a-f0-9]{64})"', text)
                if match:
                    revision = match.group(1)
            name, args = PREFIX + "edit_file", {
                "path": "legacy.c", "encoding": "gbk", "expected_revision": revision or "missing-revision",
                "edits": [{"old_string": "中文原文", "new_string": "中文修改"}],
            }
        elif not child and step == 4:
            name, args = "Agent", {
                "subagent_type": "general-purpose", "description": "Check GBK hooks in child",
                "prompt": "GBK_HOOK_SUBAGENT_PROBE: verify native Read denial then MCP read_file.",
            }
        else:
            name, args = None, None
        if name:
            self.server.calls.append({"lane": lane, "tool": name})
        blocks = ([{"type": "tool_use", "id": f"toolu_{lane}_{step}", "name": name, "input": args}]
                  if name else [{"type": "text", "text": "GBK hook probe complete."}])
        response = {"id": f"msg_{lane}_{step}", "type": "message", "role": "assistant",
                    "model": request.get("model", "claude-sonnet-4-6"), "content": blocks,
                    "stop_reason": "tool_use" if name else "end_turn", "stop_sequence": None,
                    "usage": {"input_tokens": 100, "output_tokens": 20}}
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream" if request.get("stream") else "application/json")
        self.end_headers()
        if not request.get("stream"):
            self.wfile.write(json.dumps(response).encode())
            return

        def emit(event, data):
            self.wfile.write(f"event: {event}\ndata: {json.dumps(data)}\n\n".encode())
            self.wfile.flush()
        emit("message_start", {"type": "message_start", "message": {**response, "content": [], "stop_reason": None}})
        for index, block in enumerate(blocks):
            empty = {**block, "input": {}} if name else {"type": "text", "text": ""}
            emit("content_block_start", {"type": "content_block_start", "index": index, "content_block": empty})
            delta = ({"type": "input_json_delta", "partial_json": json.dumps(args)} if name
                     else {"type": "text_delta", "text": block["text"]})
            emit("content_block_delta", {"type": "content_block_delta", "index": index, "delta": delta})
            emit("content_block_stop", {"type": "content_block_stop", "index": index})
        emit("message_delta", {"type": "message_delta", "delta": {"stop_reason": response["stop_reason"], "stop_sequence": None}, "usage": {"output_tokens": 20}})
        emit("message_stop", {"type": "message_stop"})


def run_probe(root, plugin, env, variant, output):
    hooks_path = plugin / "hooks" / "hooks.json"
    original = hooks_path.read_text(encoding="utf-8")
    hooks = json.loads(original)
    guard = hooks["hooks"]["PreToolUse"][0]["hooks"][0]
    if variant == "missing":
        guard["command"] = str(root / "missing-hook.exe")
    if variant == "timeout":
        guard.update(command=sys.executable, args=["-c", "import time; time.sleep(3)"], timeout=1)
    hooks_path.write_text(json.dumps(hooks), encoding="utf-8")
    server = ThreadingHTTPServer(("127.0.0.1", 0), Model)
    server.root, server.variant = root, variant
    server.steps, server.calls, server.results = {"main": 0, "child": 0}, [], []
    server.results_by_lane = {"main": [], "child": []}
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    local_env = {**env, "ANTHROPIC_BASE_URL": f"http://127.0.0.1:{server.server_port}",
                 "ANTHROPIC_API_KEY": "local-scripted-model-no-credentials"}
    # Prevent inherited provider selection from bypassing the loopback model.
    for key in ["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX", "CLAUDE_CODE_USE_FOUNDRY"]:
        local_env.pop(key, None)
    for key in ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_CUSTOM_HEADERS", "CLAUDE_CODE_OAUTH_TOKEN"]:
        local_env.pop(key, None)
    try:
        result = cli(["-p", "GBK_HOOK_MAIN_PROBE: run the local integration probe.",
                      "--setting-sources", "user",
                      "--model", "claude-sonnet-4-6", "--no-session-persistence", "--no-chrome",
                      "--output-format", "stream-json", "--verbose", "--include-hook-events",
                      "--allowedTools", ",".join(["Read", "Agent", PREFIX + "read_file", PREFIX + "edit_file"])],
                     root, local_env)
        output.mkdir(parents=True, exist_ok=True)
        (output / f"{variant}.jsonl").write_text(result.stdout, encoding="utf-8")
        (output / f"{variant}.stderr.txt").write_text(result.stderr, encoding="utf-8")
        errors = [block for block in server.results if block.get("is_error")]
        error_text = "\n".join(walk_strings(errors))
        events = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
        init = next(item for item in events if item.get("subtype") == "init")
        assert any(item.get("source") == PLUGIN_ID for item in init.get("plugins", [])), "Installed plugin did not load automatically"
        if variant == "normal":
            assert "GBK protection" in error_text, error_text
            for lane in ["main", "child"]:
                assert "GBK protection" in "\n".join(walk_strings(server.results_by_lane[lane])), f"Native Read was not denied in {lane}"
            assert server.steps["child"] >= 3, f"Subagent was not exercised: {server.calls}"
            assert "SubagentStart" in result.stdout, "Missing subagent hook event"
            assert "SessionStart" in result.stdout, "Missing startup hook event"
            expected = BEFORE.replace("中文原文", "中文修改").encode("gbk")
            assert (root / "legacy.c").read_bytes() == expected, "GBK edit failed"
            assert sum(call["tool"] == PREFIX + "read_file" for call in server.calls) >= 3
        else:
            assert errors and "hook" in error_text.lower(), error_text
            assert (root / "legacy.c").read_bytes() == BEFORE.encode("gbk")
        return {"variant": variant, "passed": True, "scripted_model": True,
                "tool_calls": server.calls, "child_requests": server.steps["child"]}
    finally:
        hooks_path.write_text(original, encoding="utf-8")
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def live_probe(root, plugin, output):
    """Optional genuine model routing test; uses normal client quota."""
    prompt = ("验证本地 GBK 插件：先尝试一次内置 Read 读取 legacy.c，以确认 Hook 拒绝。"
              "收到拒绝后改用插件 MCP 读取，将‘中文原文’改为‘中文修改’，再次读取确认。"
              "只修改该文件，简短报告。")
    result = cli(["-p", prompt, "--plugin-dir", str(plugin), "--setting-sources", "user",
                  "--no-session-persistence", "--no-chrome", "--max-budget-usd", "1.5",
                  "--output-format", "stream-json", "--verbose", "--include-hook-events",
                  "--allowedTools", ",".join(["Read", PREFIX + "read_file", PREFIX + "edit_file"])], root, os.environ.copy())
    output.mkdir(parents=True, exist_ok=True)
    (output / "live.jsonl").write_text(result.stdout, encoding="utf-8")
    (output / "live.stderr.txt").write_text(result.stderr, encoding="utf-8")
    assert (root / "legacy.c").read_bytes() == BEFORE.replace("中文原文", "中文修改").encode("gbk")
    assert "GBK protection" in result.stdout, "The genuine model did not demonstrate a native Read denial"
    events = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    tools = [block["name"] for item in events if item.get("type") == "assistant"
             for block in item.get("message", {}).get("content", []) if block.get("type") == "tool_use"]
    assert "Read" in tools and tools.count(PREFIX + "read_file") >= 2 and PREFIX + "edit_file" in tools, tools
    return {"passed": True, "scripted_model": False, "gbk_bytes_preserved": True, "tool_calls": tools}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("benchmark-results/plugin"))
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    binary, output = args.binary.resolve(), args.output.resolve()
    with tempfile.TemporaryDirectory(prefix="gbk plugin 中文 ") as folder:
        parent = Path(folder)
        root = parent / "project 空格"
        root.mkdir()
        plugin = stage_marketplace(binary, parent / "marketplace")
        config = parent / "isolated-claude"
        config.mkdir()
        env = {**os.environ, "CLAUDE_CONFIG_DIR": str(config), "DISABLE_AUTOUPDATER": "1",
               "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1"}
        cli(["plugin", "validate", str(plugin), "--strict"], root, env)
        cli(["plugin", "validate", str(parent / "marketplace"), "--strict"], root, env)
        cli(["plugin", "marketplace", "add", str(parent / "marketplace")], root, env)
        cli(["plugin", "install", PLUGIN_ID, "--scope", "user"], root, env)
        listing = cli(["plugin", "list", "--json"], root, env).stdout
        assert PLUGIN_ID in listing and '"enabled": true' in listing, listing
        installed = next(item for item in json.loads(listing) if item["id"] == PLUGIN_ID)
        installed_plugin = Path(installed.get("readFromFolder") or installed["installPath"])
        samples = []
        payload = json.dumps({"hook_event_name":"PreToolUse","cwd":str(root),"tool_name":"Read","tool_input":{"file_path":"legacy.c"}})
        for _ in range(20):
            start = time.perf_counter()
            check = subprocess.run([str(binary), "--root", str(root), "--hook", "pre-tool-use"], input=payload,
                                   capture_output=True, text=True, encoding="utf-8", timeout=5)
            assert check.returncode == 0 and '"deny"' in check.stdout, check.stderr
            samples.append((time.perf_counter() - start) * 1000)
        cases = []
        for variant in ["normal", "missing", "timeout"]:
            (root / "legacy.c").write_bytes(BEFORE.encode("gbk"))
            cases.append(run_probe(root, installed_plugin, env, variant, output))
        cli(["plugin", "update", PLUGIN_ID], root, env)
        cli(["plugin", "uninstall", PLUGIN_ID], root, env)
        if args.live:
            (root / "legacy.c").write_bytes(BEFORE.encode("gbk"))
            cases.append(live_probe(root, plugin, output))
        report = {"passed": True, "claude_version": cli(["--version"], root, env).stdout.strip(),
                  "platform": sys.platform, "native_hook_ms": {"median": sorted(samples)[len(samples)//2], "max": max(samples)},
                  "isolated_install_update_uninstall": True, "cases": cases}
        output.mkdir(parents=True, exist_ok=True)
        (output / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
