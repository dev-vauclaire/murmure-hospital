"""Check the four application versions before creating a Windows installer."""

import json
from pathlib import Path
import re
import tomllib


ROOT = Path(__file__).resolve().parents[3]


def check_versions(root: Path) -> str:
    package = json.loads((root / "package.json").read_text(encoding="utf-8"))
    tauri = json.loads((root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    cargo = tomllib.loads((root / "src-tauri/Cargo.toml").read_text(encoding="utf-8"))
    lock = tomllib.loads((root / "src-tauri/Cargo.lock").read_text(encoding="utf-8"))
    locked = [
        entry for entry in lock["package"]
        if entry["name"] == cargo["package"]["name"] and "source" not in entry
    ]
    if len(locked) != 1:
        raise ValueError("Expected exactly one application entry in Cargo.lock.")
    versions = [package["version"], tauri["version"], cargo["package"]["version"], locked[0]["version"]]
    if any(version != versions[0] for version in versions):
        raise ValueError("Application versions are not aligned across the four manifests.")
    version = versions[0]
    if not isinstance(version, str) or re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) is None:
        raise ValueError("The setup requires a numeric major.minor.patch application version.")
    if any(int(part) > 65535 for part in version.split(".")):
        raise ValueError("The application version exceeds Windows executable version limits.")
    return version


if __name__ == "__main__":
    try:
        version = check_versions(ROOT)
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise SystemExit(f"Application version check failed: {error}")
    print(f"Application versions aligned: {version}")
