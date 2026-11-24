"""Check the hospital reference files without reading any user profile."""

import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[3]
DEFAULT_FILES = (
    "default-settings.json",
    "default-dictionnary.json",
    "default-formatting-rules.json",
)


def reject_constant(_value: str) -> None:
    raise ValueError("Non-finite JSON constants are not allowed.")


def check_default_files(root: Path) -> None:
    for name in DEFAULT_FILES:
        relative = Path("resources/hospital") / name
        path = root / relative
        if not path.is_file():
            raise ValueError(f"A hospital default file is missing: {name}")
        tracked = subprocess.run(
            ["git", "-C", str(root), "ls-files", "--error-unmatch", "--", relative.as_posix()],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if tracked.returncode != 0:
            raise ValueError(f"Hospital default files must be tracked by Git: {name}")
        try:
            reference = json.loads(path.read_text(encoding="utf-8-sig"), parse_constant=reject_constant)
        except (UnicodeError, ValueError) as error:
            raise ValueError(f"A hospital default file contains invalid JSON: {name}") from error
        if not isinstance(reference, dict):
            raise ValueError(f"Hospital default files must be JSON objects: {name}")


if __name__ == "__main__":
    try:
        check_default_files(ROOT)
    except (OSError, ValueError) as error:
        raise SystemExit(f"Hospital default file check failed: {error}")
    print("The three hospital default files are tracked JSON objects.")
