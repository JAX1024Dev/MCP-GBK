"""Copy upstream notices from Cargo's local registry for the locked dependency graph."""
import json
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--offline", "--format-version", "1"], cwd=root))
used = {node["id"] for node in metadata["resolve"]["nodes"]}
rows = ["# Third-party notices", "", "Generated from Cargo.lock by scripts/collect_licenses.py.",
        "Includes dependencies for supported target platforms. Upstream license texts",
        "are retained under third-party-licenses/. The root LICENSE covers GBK MCP only.",
        "", "| Package | Version | Declared license | Notices |", "| --- | --- | --- | --- |"]
missing = []
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["id"] not in used or package["source"] is None:
        continue
    source = Path(package["manifest_path"]).parent
    files = [f for f in source.iterdir() if f.is_file() and f.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "UNLICENSE"))]
    # r-efi distributes its copyright/license declaration in AUTHORS and README.
    if not files and package["name"] == "r-efi":
        files = [source / "AUTHORS", source / "README.md"]
    destination = root / "third-party-licenses" / (package["name"] + "-" + package["version"])
    destination.mkdir(parents=True, exist_ok=True)
    for file in files:
        shutil.copyfile(file, destination / file.name)
    license_name = package.get("license") or "see upstream"
    if not files:
        missing.append(package["name"])
    rows.append(f'| {package["name"]} | {package["version"]} | {license_name.replace("|", "/")} | [{len(files)} files](third-party-licenses/{destination.name}/) |')
(root / "THIRD_PARTY_NOTICES.md").write_text("\n".join(rows) + "\n", encoding="utf-8")
if missing:
    raise SystemExit("No top-level license text found for: " + ", ".join(missing))
print(f"Collected notices for {len(rows)-9} dependency packages")
