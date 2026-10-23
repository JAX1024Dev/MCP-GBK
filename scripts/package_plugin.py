"""Build a native offline marketplace archive or a Desktop GUI plugin ZIP."""
import argparse
import json
import platform
from pathlib import Path
import re
import shutil
import stat
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


def stage_plugin(binary, plugin, system=None, executable_dir="bin"):
    """Stage one plugin from the shared template, without downloading anything."""
    shutil.copytree(PROJECT / "plugins" / "gbk-mcp", plugin,
                    ignore=shutil.ignore_patterns("bin", "server", "__pycache__", ".DS_Store"))
    manifest = json.loads((plugin / ".claude-plugin" / "plugin.json").read_text())
    if manifest["version"] != version():
        raise ValueError("Plugin and Cargo versions differ")
    (plugin / executable_dir).mkdir(exist_ok=True)
    system = system or native_platform()[0]
    executable = "gbk-mcp.exe" if system == "windows" else "gbk-mcp"
    installed = plugin / executable_dir / executable
    shutil.copy2(binary, installed)
    if system != "windows":
        installed.chmod(0o755)
    # Keep one template and generate platform paths without a shell wrapper.
    for relative in [".mcp.json", "hooks/hooks.json"]:
        config = plugin / relative
        config.write_text(config.read_text(encoding="utf-8").replace("/bin/gbk-mcp.exe", f"/{executable_dir}/{executable}"), encoding="utf-8")
    for name in ["LICENSE", "THIRD_PARTY_NOTICES.md", "CHANGELOG.md", "AGENTS.md"]:
        shutil.copyfile(PROJECT / name, plugin / name)
    for name in ["third-party-licenses", "docs", "examples"]:
        shutil.copytree(PROJECT / name, plugin / name)
    return plugin


def stage_marketplace(binary, destination, system=None):
    """Stage the existing local marketplace layout; keep its interface stable."""
    destination.mkdir(parents=True, exist_ok=False)
    plugin = stage_plugin(binary, destination / "plugins" / "gbk-mcp", system)
    (destination / ".claude-plugin").mkdir()
    shutil.copyfile(PROJECT / "examples" / "claude.marketplace.json", destination / ".claude-plugin" / "marketplace.json")
    shutil.copyfile(PROJECT / "docs" / "OFFLINE_PLUGIN.md", destination / "README.md")
    return plugin


def write_zip(source, archive):
    """Store POSIX modes explicitly so Desktop extracts native Unix executables."""
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as package:
        for path in sorted(source.rglob("*")):
            if path.is_file():
                entry = zipfile.ZipInfo.from_file(path, path.relative_to(source))
                entry.create_system = 3
                executable = path.parent.name in ("bin", "server") and path.name in ("gbk-mcp", "gbk-mcp.exe")
                entry.external_attr = (stat.S_IFREG | (0o755 if executable else 0o644)) << 16
                entry.compress_type = zipfile.ZIP_DEFLATED
                with path.open("rb") as stream, package.open(entry, "w") as output:
                    shutil.copyfileobj(stream, output)


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


def build_package(binary, output_dir, system, arch, package_format="marketplace"):
    if package_format not in ("marketplace", "plugin-zip"):
        raise ValueError(f"Unsupported plugin package format: {package_format}")
    check_binary(binary, system, arch)
    output_dir.mkdir(parents=True, exist_ok=True)
    gui = package_format == "plugin-zip"
    extension = "zip" if gui or system == "windows" else "tar.gz"
    label = "claude-plugin-gui" if gui else "claude-plugin"
    archive = output_dir / f"gbk-mcp-{version()}-{label}-{system}-{arch}.{extension}"
    with tempfile.TemporaryDirectory(prefix="gbk-plugin-stage-") as folder:
        staging = Path(folder)
        if gui:
            plugin = stage_plugin(binary, staging / "plugin", system, "server")
            write_zip(plugin, archive)
        else:
            marketplace = staging / "gbk-plugin"
            stage_marketplace(binary, marketplace, system)
            if system == "windows":
                write_zip(staging, archive)
            else:
                with tarfile.open(archive, "w:gz") as package:
                    package.add(marketplace, arcname="gbk-plugin")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, help="Cargo target used for the release build")
    parser.add_argument("--binary", type=Path, help="Override the native release executable path")
    parser.add_argument("--output-dir", type=Path, default=PROJECT / "dist")
    parser.add_argument("--format", choices=["marketplace", "plugin-zip"], default="marketplace",
                        help="Existing marketplace bundle (default), or a single plugin ZIP for Desktop GUI upload")
    args = parser.parse_args()
    system, arch = TARGETS[args.target] if args.target else native_platform()
    build = PROJECT / "target" / args.target / "release" if args.target else PROJECT / "target" / "release"
    binary = args.binary or build / ("gbk-mcp.exe" if system == "windows" else "gbk-mcp")
    archive = build_package(binary, args.output_dir, system, arch, args.format)
    print(archive)
    write_checksums(args.output_dir)


if __name__ == "__main__":
    main()
