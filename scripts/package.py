"""Package release binaries and generate a shared SHA-256 manifest."""
import argparse
import hashlib
from pathlib import Path
import platform
import re
import zipfile

TARGETS = {
    "x86_64-pc-windows-msvc": ("windows", "x86_64"),
    "x86_64-unknown-linux-gnu": ("linux", "x86_64"),
    "aarch64-apple-darwin": ("darwin", "aarch64"),
}


def write_checksums(output):
    archives = sorted([*output.glob("gbk-mcp-*.zip"), *output.glob("gbk-mcp-*.tar.gz")])
    if not archives:
        raise SystemExit("No release archives found in output directory")
    checksums = []
    for archive in archives:
        digest = hashlib.sha256()
        with archive.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
        checksums.append(f"{digest.hexdigest()}  {archive.name}\n")
    (output / "SHA256SUMS").write_text("".join(checksums), encoding="utf-8")
    print("".join(checksums), end="")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, help="Cargo target used for the release build")
    parser.add_argument("--checksums-only", action="store_true", help="Hash existing archives without building a package")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    output = root / "dist"
    output.mkdir(exist_ok=True)

    if not args.checksums_only:
        version = re.search(r'^version = "([^"]+)"', (root / "Cargo.toml").read_text(), re.MULTILINE).group(1)
        if args.target:
            system, arch = TARGETS[args.target]
            build = root / "target" / args.target / "release"
        else:
            system = platform.system().lower()
            arch = {"arm64": "aarch64", "amd64": "x86_64"}.get(platform.machine().lower(), platform.machine().lower())
            build = root / "target" / "release"
        binary = build / ("gbk-mcp.exe" if system == "windows" else "gbk-mcp")
        if not binary.is_file():
            raise SystemExit(f"Release binary missing: {binary}; run cargo build --release --locked with the matching target")
        archive = output / f"gbk-mcp-{version}-{system}-{arch}.zip"
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
            bundle.write(binary, binary.name)
            for name in ["LICENSE", "README.md", "AGENTS.md", "CHANGELOG.md", "THIRD_PARTY_NOTICES.md"]:
                bundle.write(root / name, name)
            for directory in ["examples", "docs", "third-party-licenses"]:
                for path in sorted((root / directory).rglob("*")):
                    if path.is_file():
                        bundle.write(path, path.relative_to(root))
        print(archive)

    write_checksums(output)


if __name__ == "__main__":
    main()
