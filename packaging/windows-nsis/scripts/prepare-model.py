"""Download the pinned speech model and verify its archive before extraction."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
from urllib.parse import urlsplit
from urllib.request import urlopen
import zipfile


ROOT = Path(__file__).resolve().parents[3]
SOURCE = Path(__file__).resolve().parents[1] / "model-source.json"
MODEL_DIRECTORY = "parakeet-tdt-0.6b-v3-int8"
MODEL_FILES = (
    "encoder-model.int8.onnx",
    "decoder_joint-model.int8.onnx",
    "nemo128.onnx",
    "vocab.txt",
)


def load_source(path: Path = SOURCE) -> dict:
    source = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(source, dict):
        raise ValueError("The model source must be a JSON object.")
    version = source.get("version")
    if not isinstance(version, str) or re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) is None:
        raise ValueError("The model source must specify a numeric release version.")
    expected_url = f"https://github.com/Kieirra/murmure-model/releases/download/{version}/{MODEL_DIRECTORY}.zip"
    if source.get("url") != expected_url:
        raise ValueError("The model URL must match its pinned official release.")
    digest = source.get("sha256")
    if not isinstance(digest, str) or re.fullmatch(r"[a-f0-9]{64}", digest) is None:
        raise ValueError("The model source requires a pinned SHA-256.")
    return source


def verify_and_extract_archive(archive: Path, expected_sha256: str, resources: Path) -> None:
    with archive.open("rb") as stream:
        actual_sha256 = hashlib.file_digest(stream, "sha256").hexdigest()
    if actual_sha256 != expected_sha256:
        raise ValueError("The speech model SHA-256 differs from its pinned value; nothing was extracted.")

    with zipfile.ZipFile(archive) as model:
        destination = resources.resolve()
        members = model.infolist()
        for member in members:
            name = member.orig_filename
            path = PurePosixPath(name)
            # Reject Windows separators/drives as well as POSIX traversal, even
            # when this validation is running on Ubuntu.
            if ("\\" in name or ":" in name or "\x00" in name or path.is_absolute()
                    or ".." in path.parts or stat.S_ISLNK(member.external_attr >> 16)):
                raise ValueError("The speech model archive contains an unsafe path.")
            target = destination.joinpath(*path.parts).resolve()
            if not target.is_relative_to(destination):
                raise ValueError("The speech model archive would write outside the resource directory.")

        files = {member.filename: member for member in members if not member.is_dir()}
        for filename in MODEL_FILES:
            member = files.get(f"{MODEL_DIRECTORY}/{filename}")
            if member is None or member.file_size == 0:
                raise ValueError(f"The speech model archive is missing a required file: {filename}")
        if not (resources / "tokenizer.json").is_file():
            raise ValueError("The tokenizer resource is missing.")

        resources.mkdir(parents=True, exist_ok=True)
        model.extractall(resources)
    print(f"Speech model SHA-256 verified: {actual_sha256}; required resources extracted.")


def prepare_model(archive: Path) -> None:
    source = load_source()
    if not archive.exists():
        archive.parent.mkdir(parents=True, exist_ok=True)
        print(f"Downloading speech model {source['version']}.")
        created = False
        try:
            with urlopen(source["url"], timeout=60) as response:
                if urlsplit(response.geturl()).scheme != "https":
                    raise ValueError("The model download must remain on HTTPS.")
                with archive.open("xb") as output:
                    created = True
                    shutil.copyfileobj(response, output, length=1024 * 1024)
        except (OSError, ValueError):
            # A failed download must not look like a reusable complete archive.
            if created:
                archive.unlink(missing_ok=True)
            raise
    verify_and_extract_archive(archive, source["sha256"], ROOT / "resources")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True, help="Archive path in the build runner's temporary directory.")
    args = parser.parse_args()
    try:
        prepare_model(args.archive)
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        parser.exit(1, f"Speech model preparation failed: {error}\n")


if __name__ == "__main__":
    main()
