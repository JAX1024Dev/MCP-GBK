"""Release contract checks: layouts, native architecture and executable modes."""
import json
from pathlib import Path
import stat
import struct
import sys
import tarfile
import tempfile
import unittest
import zipfile

from package_plugin import build_package, check_binary, version
from plugin_smoke import extract_plugin_archive


def executable_header(system):
    header = bytearray(70 if system == "windows" else 64)
    if system == "windows":
        header[:2] = b"MZ"
        struct.pack_into("<I", header, 60, 64)
        header[64:] = b"PE\0\0\x64\x86"
    elif system == "linux":
        header[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<H", header, 18, 62)
    else:
        header[:4] = b"\xcf\xfa\xed\xfe"
        struct.pack_into("<I", header, 4, 0x0100000C)
    return bytes(header)


PLATFORMS = [("windows", "x86_64"), ("linux", "x86_64"), ("darwin", "aarch64")]


class PluginPackages(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="gbk package 中文 ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "binary"

    def test_gui_zip_is_one_plugin_with_native_program_and_default_hooks(self):
        for system, arch in PLATFORMS:
            with self.subTest(system=system):
                original = executable_header(system)
                self.binary.write_bytes(original)
                archive = build_package(self.binary, self.root / system, system, arch, "plugin-zip")
                self.assertIn(f"claude-plugin-gui-{system}-{arch}.zip", archive.name)
                name = "gbk-mcp.exe" if system == "windows" else "gbk-mcp"
                with zipfile.ZipFile(archive) as bundle:
                    names = bundle.namelist()
                    self.assertIn(".claude-plugin/plugin.json", names)
                    self.assertNotIn(".claude-plugin/marketplace.json", names)
                    self.assertFalse(any(path.startswith(("bin/", "plugins/", "gbk-plugin/")) for path in names))
                    program = bundle.getinfo(f"server/{name}")
                    self.assertEqual(bundle.read(program), original)
                    self.assertEqual(program.create_system, 3)
                    self.assertEqual(stat.S_IMODE(program.external_attr >> 16), 0o755)
                    manifest = json.loads(bundle.read(".claude-plugin/plugin.json"))
                    self.assertEqual(manifest["version"], version())
                    self.assertEqual(manifest["name"], "gbk-mcp")
                    self.assertNotIn("hooks", manifest)
                    mcp = json.loads(bundle.read(".mcp.json"))["mcpServers"]["gbk"]
                    self.assertEqual(mcp["command"], "${CLAUDE_PLUGIN_ROOT}/server/" + name)
                    self.assertEqual(mcp["args"], ["--root", "${CLAUDE_PROJECT_DIR}"])
                    hooks = json.loads(bundle.read("hooks/hooks.json"))["hooks"]
                    self.assertEqual(set(hooks), {"SessionStart", "SubagentStart", "PreToolUse"})
                    for rules in hooks.values():
                        self.assertEqual(len(rules), 1)
                        self.assertEqual(len(rules[0]["hooks"]), 1)
                        self.assertEqual(rules[0]["hooks"][0]["command"], mcp["command"])
                    self.assertEqual(hooks["PreToolUse"][0]["hooks"][0]["onFailure"], "block")
                    for required in ["LICENSE", "THIRD_PARTY_NOTICES.md", "README.md", "skills/gbk-files/SKILL.md"]:
                        self.assertIn(required, names)
                    self.assertTrue(any(path.startswith("third-party-licenses/") for path in names))

    def test_default_marketplace_layout_remains_compatible(self):
        for system, arch in PLATFORMS:
            with self.subTest(system=system):
                self.binary.write_bytes(executable_header(system))
                archive = build_package(self.binary, self.root / system, system, arch)
                name = "gbk-mcp.exe" if system == "windows" else "gbk-mcp"
                if system == "windows":
                    with zipfile.ZipFile(archive) as bundle:
                        names = bundle.namelist()
                else:
                    with tarfile.open(archive) as bundle:
                        names = bundle.getnames()
                self.assertIn("gbk-plugin/.claude-plugin/marketplace.json", names)
                self.assertIn(f"gbk-plugin/plugins/gbk-mcp/bin/{name}", names)

    def test_native_architecture_mismatch_is_rejected(self):
        for actual_system, _ in PLATFORMS:
            self.binary.write_bytes(executable_header(actual_system))
            for system, arch in PLATFORMS:
                if actual_system == system:
                    check_binary(self.binary, system, arch)
                else:
                    with self.assertRaises(ValueError):
                        check_binary(self.binary, system, arch)
        self.binary.write_bytes(b"invalid")
        with self.assertRaises(ValueError):
            check_binary(self.binary, *PLATFORMS[0])

    def test_gui_smoke_extraction_preserves_paths_bytes_and_execute_permission(self):
        system = {"win32": "windows", "darwin": "darwin"}.get(sys.platform, "linux")
        arch = "aarch64" if system == "darwin" else "x86_64"
        original = executable_header(system)
        self.binary.write_bytes(original)
        archive = build_package(self.binary, self.root / "dist", system, arch, "plugin-zip")
        plugin, marketplace, binary = extract_plugin_archive(archive, self.root / "extracted")
        self.assertEqual(binary.read_bytes(), original)
        self.assertEqual(binary.parent.name, "server")
        if sys.platform != "win32":
            self.assertEqual(stat.S_IMODE(binary.stat().st_mode), 0o755)
        self.assertTrue((marketplace / ".claude-plugin" / "marketplace.json").is_file())
        self.assertNotIn("/bin/", (plugin / ".mcp.json").read_text())

    def test_smoke_extraction_rejects_zip_traversal(self):
        archive = self.root / "unsafe.zip"
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr("../outside", "should not be written")
        with self.assertRaises(ValueError):
            extract_plugin_archive(archive, self.root / "extracted")
        self.assertFalse((self.root / "outside").exists())


if __name__ == "__main__":
    unittest.main()
