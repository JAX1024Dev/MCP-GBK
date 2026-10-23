"""Build a self-contained Windows x64 offline Claude marketplace ZIP."""
import argparse
import json
from pathlib import Path
import re
import shutil
import struct
import tempfile
import zipfile

from package import write_checksums

PROJECT = Path(__file__).resolve().parent.parent


def version():
    return re.search(r'^version = "([^"]+)"', (PROJECT / "Cargo.toml").read_text(), re.MULTILINE).group(1)


def stage_marketplace(binary, destination):
    """Stage files for packaging or host-native integration tests; no downloads."""
    destination.mkdir(parents=True, exist_ok=False)
    plugin = destination / "plugins" / "gbk-mcp"
    shutil.copytree(PROJECT / "plugins" / "gbk-mcp", plugin,
                    ignore=shutil.ignore_patterns("bin", "__pycache__", ".DS_Store"))
    manifest = json.loads((plugin / ".claude-plugin" / "plugin.json").read_text())
    if manifest["version"] != version():
        raise ValueError("Plugin and Cargo versions differ")
    (plugin / "bin").mkdir(exist_ok=True)
    shutil.copy2(binary, plugin / "bin" / "gbk-mcp.exe")
    for name in ["LICENSE", "THIRD_PARTY_NOTICES.md", "CHANGELOG.md", "AGENTS.md"]:
        shutil.copyfile(PROJECT / name, plugin / name)
    for name in ["third-party-licenses", "docs", "examples"]:
        shutil.copytree(PROJECT / name, plugin / name)
    (destination / ".claude-plugin").mkdir()
    shutil.copyfile(PROJECT / "examples" / "claude.marketplace.json", destination / ".claude-plugin" / "marketplace.json")
    shutil.copyfile(PROJECT / "docs" / "OFFLINE_PLUGIN.md", destination / "README.md")
    return plugin


def check_windows_x64(binary):
    with binary.open("rb") as source:
        header = source.read(64)
        if len(header) != 64 or header[:2] != b"MZ":
            raise ValueError("Offline release requires a Windows x64 PE executable")
        source.seek(struct.unpack_from("<I", header, 60)[0])
        if source.read(6) != b"PE\0\0\x64\x86":
            raise ValueError("Offline release requires a Windows x64 PE executable")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=PROJECT / "target" / "release" / "gbk-mcp.exe")
    parser.add_argument("--output-dir", type=Path, default=PROJECT / "dist")
    args = parser.parse_args()
    check_windows_x64(args.binary)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    archive = args.output_dir / f"gbk-mcp-{version()}-claude-plugin-windows-x86_64.zip"
    with tempfile.TemporaryDirectory(prefix="gbk-plugin-stage-") as folder:
        marketplace = Path(folder) / "gbk-plugin"
        stage_marketplace(args.binary, marketplace)
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as package:
            for path in sorted(marketplace.rglob("*")):
                if path.is_file():
                    package.write(path, path.relative_to(Path(folder)))
    print(archive)
    write_checksums(args.output_dir)


if __name__ == "__main__":
    main()
