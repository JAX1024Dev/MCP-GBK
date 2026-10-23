"""Build a self-contained platform-native offline Claude marketplace archive."""
import argparse
import json
import platform
from pathlib import Path
import re
import shutil
import struct
import tarfile
import tempfile
import zipfile

from package import TARGETS, write_checksums

PROJECT = Path(__file__).resolve().parent.parent


def version():
    return re.search(r'^version = "([^"]+)"', (PROJECT / "Cargo.toml").read_text(), re.MULTILINE).group(1)


def native_platform():
    return (platform.system().lower(),
            {"arm64": "aarch64", "amd64": "x86_64"}.get(platform.machine().lower(), platform.machine().lower()))


def stage_marketplace(binary, destination, system=None):
    """Stage files for packaging or host-native integration tests; no downloads."""
    destination.mkdir(parents=True, exist_ok=False)
    plugin = destination / "plugins" / "gbk-mcp"
    shutil.copytree(PROJECT / "plugins" / "gbk-mcp", plugin,
                    ignore=shutil.ignore_patterns("bin", "__pycache__", ".DS_Store"))
    manifest = json.loads((plugin / ".claude-plugin" / "plugin.json").read_text())
    if manifest["version"] != version():
        raise ValueError("Plugin and Cargo versions differ")
    (plugin / "bin").mkdir(exist_ok=True)
    system = system or native_platform()[0]
    executable = "gbk-mcp.exe" if system == "windows" else "gbk-mcp"
    installed = plugin / "bin" / executable
    shutil.copy2(binary, installed)
    if system != "windows":
        installed.chmod(0o755)
    # Keep one template and generate platform paths without a shell wrapper.
    for relative in [".mcp.json", "hooks/hooks.json"]:
        config = plugin / relative
        config.write_text(config.read_text(encoding="utf-8").replace("/bin/gbk-mcp.exe", f"/bin/{executable}"), encoding="utf-8")
    for name in ["LICENSE", "THIRD_PARTY_NOTICES.md", "CHANGELOG.md", "AGENTS.md"]:
        shutil.copyfile(PROJECT / name, plugin / name)
    for name in ["third-party-licenses", "docs", "examples"]:
        shutil.copytree(PROJECT / name, plugin / name)
    (destination / ".claude-plugin").mkdir()
    shutil.copyfile(PROJECT / "examples" / "claude.marketplace.json", destination / ".claude-plugin" / "marketplace.json")
    shutil.copyfile(PROJECT / "docs" / "OFFLINE_PLUGIN.md", destination / "README.md")
    return plugin


def check_binary(binary, system, arch):
    with binary.open("rb") as source:
        header = source.read(64)
        if (system, arch) == ("windows", "x86_64"):
            if len(header) == 64 and header[:2] == b"MZ":
                source.seek(struct.unpack_from("<I", header, 60)[0])
                if source.read(6) == b"PE\0\0\x64\x86":
                    return
        elif (system, arch) == ("linux", "x86_64"):
            if len(header) == 64 and header[:6] == b"\x7fELF\x02\x01" and struct.unpack_from("<H", header, 18)[0] == 62:
                return
        elif (system, arch) == ("darwin", "aarch64"):
            if len(header) == 64 and header[:4] == b"\xcf\xfa\xed\xfe" and struct.unpack_from("<I", header, 4)[0] == 0x0100000C:
                return
        raise ValueError(f"Expected a native {system}/{arch} executable: {binary}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, help="Cargo target used for the release build")
    parser.add_argument("--binary", type=Path, help="Override the native release executable path")
    parser.add_argument("--output-dir", type=Path, default=PROJECT / "dist")
    args = parser.parse_args()
    system, arch = TARGETS[args.target] if args.target else native_platform()
    build = PROJECT / "target" / args.target / "release" if args.target else PROJECT / "target" / "release"
    binary = args.binary or build / ("gbk-mcp.exe" if system == "windows" else "gbk-mcp")
    check_binary(binary, system, arch)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    extension = "zip" if system == "windows" else "tar.gz"
    archive = args.output_dir / f"gbk-mcp-{version()}-claude-plugin-{system}-{arch}.{extension}"
    with tempfile.TemporaryDirectory(prefix="gbk-plugin-stage-") as folder:
        marketplace = Path(folder) / "gbk-plugin"
        stage_marketplace(binary, marketplace, system)
        if system == "windows":
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as package:
                for path in sorted(marketplace.rglob("*")):
                    if path.is_file():
                        package.write(path, path.relative_to(Path(folder)))
        else:
            with tarfile.open(archive, "w:gz") as package:
                package.add(marketplace, arcname="gbk-plugin")
    print(archive)
    write_checksums(args.output_dir)


if __name__ == "__main__":
    main()
