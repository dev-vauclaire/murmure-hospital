"""Exercise model verification using small synthetic archives without networking."""

import hashlib
import importlib.util
import json
from pathlib import Path
import stat
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
import zipfile


DIRECTORY = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("prepare_model", DIRECTORY.parent / "scripts/prepare-model.py")
MODEL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODEL)


class VerifiedModel(unittest.TestCase):
    def setUp(self):
        self.temporary = TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / "synthetic.zip"
        self.resources = self.root / "resources"
        self.resources.mkdir()
        (self.resources / "tokenizer.json").write_text("{}", encoding="utf-8")

    def write_archive(self, extra=None, omit=None):
        with zipfile.ZipFile(self.archive, "w") as archive:
            for filename in MODEL.MODEL_FILES:
                if filename != omit:
                    archive.writestr(f"{MODEL.MODEL_DIRECTORY}/{filename}", b"synthetic model")
            if extra is not None:
                entry = zipfile.ZipInfo(extra)
                # Keep the unsafe name in the archive even on Windows, where
                # ZipInfo normally replaces backslashes before writing it.
                entry.filename = extra
                archive.writestr(entry, b"synthetic extra")
        return hashlib.sha256(self.archive.read_bytes()).hexdigest()

    def test_verified_archive_extracts_the_required_files(self):
        digest = self.write_archive()
        MODEL.verify_and_extract_archive(self.archive, digest, self.resources)
        for filename in MODEL.MODEL_FILES:
            self.assertEqual((self.resources / MODEL.MODEL_DIRECTORY / filename).read_bytes(), b"synthetic model")

    def test_modified_archive_fails_before_any_extraction(self):
        digest = self.write_archive()
        with self.archive.open("ab") as archive:
            archive.write(b"tampered")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            MODEL.verify_and_extract_archive(self.archive, digest, self.resources)
        self.assertEqual([path.name for path in self.resources.iterdir()], ["tokenizer.json"])

    def test_unsafe_paths_fail_before_any_extraction_on_either_platform(self):
        for name in ("../escape", "/absolute", "C:/escape", "..\\escape", "folder\\escape", "file:stream"):
            with self.subTest(name=name):
                digest = self.write_archive(extra=name)
                with zipfile.ZipFile(self.archive) as archive:
                    self.assertIn(name, {member.orig_filename for member in archive.infolist()})
                with self.assertRaisesRegex(ValueError, "unsafe path"):
                    MODEL.verify_and_extract_archive(self.archive, digest, self.resources)
                self.assertFalse((self.resources / MODEL.MODEL_DIRECTORY).exists())

    def test_unsafe_fixture_preserves_its_raw_name_with_windows_separators(self):
        with patch.object(zipfile.os, "sep", "\\"):
            digest = self.write_archive(extra="folder\\escape")
            with zipfile.ZipFile(self.archive) as archive:
                self.assertIn("folder\\escape", {member.orig_filename for member in archive.infolist()})
            with self.assertRaisesRegex(ValueError, "unsafe path"):
                MODEL.verify_and_extract_archive(self.archive, digest, self.resources)
        self.assertFalse((self.resources / MODEL.MODEL_DIRECTORY).exists())

    def test_symlink_is_rejected_before_extraction(self):
        self.write_archive()
        with zipfile.ZipFile(self.archive, "a") as archive:
            link = zipfile.ZipInfo("link")
            link.external_attr = (stat.S_IFLNK | 0o777) << 16
            archive.writestr(link, "../outside")
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        with self.assertRaisesRegex(ValueError, "unsafe path"):
            MODEL.verify_and_extract_archive(self.archive, digest, self.resources)
        self.assertFalse((self.resources / MODEL.MODEL_DIRECTORY).exists())

    def test_missing_model_file_is_rejected_before_extraction(self):
        digest = self.write_archive(omit="vocab.txt")
        with self.assertRaisesRegex(ValueError, "required file"):
            MODEL.verify_and_extract_archive(self.archive, digest, self.resources)
        self.assertFalse((self.resources / MODEL.MODEL_DIRECTORY).exists())

    def test_missing_tokenizer_is_rejected_before_extraction(self):
        digest = self.write_archive()
        (self.resources / "tokenizer.json").unlink()
        with self.assertRaisesRegex(ValueError, "tokenizer"):
            MODEL.verify_and_extract_archive(self.archive, digest, self.resources)
        self.assertFalse((self.resources / MODEL.MODEL_DIRECTORY).exists())

    def test_source_requires_matching_official_https_url_and_pinned_digest(self):
        source = {
            "version": "1.3.0",
            "url": "https://github.com/Kieirra/murmure-model/releases/download/1.3.0/parakeet-tdt-0.6b-v3-int8.zip",
            "sha256": "a" * 64,
        }
        manifest = self.root / "source.json"
        manifest.write_text(json.dumps(source), encoding="utf-8")
        self.assertEqual(MODEL.load_source(manifest), source)
        for change in ({"version": "latest"}, {"url": source["url"].replace("https://", "http://")}, {"sha256": ""}):
            with self.subTest(change=change):
                manifest.write_text(json.dumps(source | change), encoding="utf-8")
                with self.assertRaises(ValueError):
                    MODEL.load_source(manifest)


if __name__ == "__main__":
    unittest.main(verbosity=2)
