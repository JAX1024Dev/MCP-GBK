"""Small stdlib-only MCP stdio client for reproducible smoke tests/benchmarks."""
import json
import queue
import subprocess
import threading


class Client:
    def __init__(self, binary, root, threads=8):
        self.process = subprocess.Popen(
            [str(binary), "--root", str(root), "--threads", str(threads)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            text=True, encoding="utf-8", bufsize=1,
        )
        self.messages = queue.Queue()
        self.pending = {}
        self.next_id = 0
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()
        result = self.call("initialize", {
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "gbk-mcp-validation", "version": "1.0"},
        })
        assert result["capabilities"]["tools"] is not None
        assert result["serverInfo"]["name"] == "gbk-mcp", result
        assert "GBK" in result["instructions"]
        self.notify("notifications/initialized", {})

    def _read(self):
        for line in self.process.stdout:
            try:
                self.messages.put(json.loads(line))
            except json.JSONDecodeError as error:
                self.messages.put(error)
        self.messages.put(EOFError("MCP server closed stdout"))

    def notify(self, method, params):
        self.process.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method, "params": params}) + "\n")
        self.process.stdin.flush()

    def send(self, method, params):
        self.next_id += 1
        message = {"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}
        self.process.stdin.write(json.dumps(message, ensure_ascii=False) + "\n")
        self.process.stdin.flush()
        return self.next_id

    def receive(self, request_id, timeout=330):
        while request_id not in self.pending:
            message = self.messages.get(timeout=timeout)
            if isinstance(message, Exception):
                raise message
            if "id" in message:
                self.pending[message["id"]] = message
        response = self.pending.pop(request_id)
        if "error" in response:
            raise RuntimeError(response["error"])
        return response["result"]

    def call(self, method, params):
        return self.receive(self.send(method, params))

    def tool(self, name, arguments, error=False):
        result = self.call("tools/call", {"name": name, "arguments": arguments})
        assert bool(result.get("isError")) == error, result
        text = result["content"][0]["text"]
        assert len(text.encode("utf-8")) <= 65536
        return json.loads(text)

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=5)
        self.process.stdout.close()
        self.reader.join(timeout=1)

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()
