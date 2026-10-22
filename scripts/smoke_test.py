"""Exercise the real compiled server through legacy-compatible MCP stdio."""
import argparse
import json
from pathlib import Path
import tempfile
import time
from mcp_client import Client


def smoke(binary):
    with tempfile.TemporaryDirectory(prefix="gbk-mcp-smoke-") as folder:
        root = Path(folder)
        (root / "legacy.c").write_bytes("前缀\r\n中文原文\r\n尾部".encode("gbk"))
        with Client(binary, root) as client:
            tools = client.call("tools/list", {})["tools"]
            assert {t["name"] for t in tools} == {"read_file", "write_file", "edit_file", "glob", "grep"}
            found = client.tool("glob", {"pattern": "**/*.c"})
            assert found["complete"] and found["items"][0]["path"] == "legacy.c"
            found = client.tool("grep", {"pattern": "中文原文"})
            assert found["complete"] and len(found["items"]) == 1
            read = client.tool("read_file", {"path": "legacy.c"})
            edit = {"path": "legacy.c", "expected_revision": read["revision"],
                    "edits": [{"old_string": "中文原文", "new_string": "中文修改"}]}
            preview = client.tool("edit_file", dict(edit, dry_run=True))
            assert not preview["applied"]
            client.tool("edit_file", edit)
            after = client.tool("read_file", {"path": "legacy.c"})
            assert after["content"] == "前缀\r\n中文修改\r\n尾部"
            assert (root / "legacy.c").read_bytes() == after["content"].encode("gbk")
            assert client.tool("edit_file", edit, error=True)["code"] == "conflict"
            client.tool("write_file", {"path": "new.txt", "content": "默认 UTF-8"})
            assert (root / "new.txt").read_bytes() == "默认 UTF-8".encode()
            (root / "ambiguous.txt").write_bytes(bytes.fromhex("c2a3"))
            amb = client.tool("read_file", {"path": "ambiguous.txt"})
            assert amb["encoding"]["status"] == "ambiguous" and amb["content"] == "拢"
            assert client.tool("write_file", {"path": "ambiguous.txt", "content": "中",
                "expected_revision": amb["revision"]}, error=True)["code"] == "encoding_required"
            # Cancellation is followed by a fresh request to ensure worker release.
            (root / "large.txt").write_bytes(b"plain text\n" * 4_000_000)
            request = client.send("tools/call", {"name": "grep", "arguments": {"path": "large.txt", "pattern": "never_match"}})
            time.sleep(0.01)
            start = time.perf_counter()
            client.notify("notifications/cancelled", {"requestId": request, "reason": "smoke test"})
            client.tool("glob", {"pattern": "*.c"})
            assert time.perf_counter() - start < 0.5
            print(json.dumps({"status": "passed", "tools": len(tools),
                              "transport": "stdio", "protocol": "2025-11-25",
                              "checks": "discovery, search, read, preview, GBK edit, conflict, UTF-8 creation, ambiguity, cancellation"}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/debug/gbk-mcp"))
    args = parser.parse_args()
    smoke(args.binary.resolve())
