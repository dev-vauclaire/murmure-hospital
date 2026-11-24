"""Exercise NSIS preflight failures using only temporary synthetic repositories."""

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parent.parent / "scripts"


def load_script(filename: str):
    spec = importlib.util.spec_from_file_location(filename.replace("-", "_"), SCRIPTS / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


versions = load_script("check-versions.py")
defaults = load_script("check-default-files.py")


class ApplicationVersions(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="murmure-version-check-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "src-tauri").mkdir()
        self.write_manifests()

    def write_manifests(self, version="2.0.4", locked_entries=1):
        (self.root / "package.json").write_text(json.dumps({"version": version}), encoding="utf-8")
        (self.root / "src-tauri/tauri.conf.json").write_text(json.dumps({"version": version}), encoding="utf-8")
        (self.root / "src-tauri/Cargo.toml").write_text(
            f'[package]\nname = "synthetic-app"\nversion = "{version}"\n', encoding="utf-8"
        )
        entry = f'[[package]]\nname = "synthetic-app"\nversion = "{version}"\n'
        (self.root / "src-tauri/Cargo.lock").write_text(
            "version = 4\n" + entry * locked_entries +
            '[[package]]\nname = "synthetic-dependency"\nversion = "1.0.0"\nsource = "registry+https://example.invalid"\n',
            encoding="utf-8",
        )

    def test_matching_versions_and_windows_upper_bound_are_accepted(self):
        self.assertEqual(versions.check_versions(self.root), "2.0.4")
        self.write_manifests(version="65535.65535.65535")
        self.assertEqual(versions.check_versions(self.root), "65535.65535.65535")

    def test_one_mismatched_manifest_is_rejected(self):
        (self.root / "src-tauri/tauri.conf.json").write_text('{"version":"2.0.5"}', encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "not aligned"):
            versions.check_versions(self.root)

    def test_missing_or_ambiguous_application_entry_is_rejected(self):
        for count in (0, 2):
            with self.subTest(count=count):
                self.write_manifests(locked_entries=count)
                with self.assertRaisesRegex(ValueError, "exactly one"):
                    versions.check_versions(self.root)

    def test_prerelease_and_out_of_range_versions_are_rejected(self):
        for version in ("2.0.4-beta.1", "2.0", "-1.0.0", "65536.0.0", "0.65536.0", "0.0.65536"):
            with self.subTest(version=version):
                self.write_manifests(version=version)
                with self.assertRaises(ValueError):
                    versions.check_versions(self.root)


class HospitalDefaultFiles(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="murmure-default-check-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.directory = self.root / "resources/hospital"
        self.directory.mkdir(parents=True)
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True, capture_output=True)
        for name in defaults.DEFAULT_FILES:
            (self.directory / name).write_text('{"synthetic":true}', encoding="utf-8")
        self.track_files()

    def track_files(self):
        subprocess.run(["git", "-C", str(self.root), "add", "--", "resources/hospital"], check=True, capture_output=True)

    def test_tracked_objects_and_utf8_bom_are_accepted(self):
        defaults.check_default_files(self.root)
        (self.directory / defaults.DEFAULT_FILES[0]).write_text('{"synthetic":true}', encoding="utf-8-sig")
        defaults.check_default_files(self.root)

    def test_missing_file_is_rejected(self):
        (self.directory / defaults.DEFAULT_FILES[0]).unlink()
        with self.assertRaisesRegex(ValueError, "missing"):
            defaults.check_default_files(self.root)

    def test_untracked_file_is_rejected(self):
        subprocess.run(
            ["git", "-C", str(self.root), "rm", "--cached", "--", f"resources/hospital/{defaults.DEFAULT_FILES[0]}"],
            check=True, capture_output=True,
        )
        with self.assertRaisesRegex(ValueError, "tracked by Git"):
            defaults.check_default_files(self.root)

    def test_invalid_json_and_non_finite_constants_are_rejected(self):
        for text in ('{"synthetic":', '{"synthetic":NaN}', '{"synthetic":Infinity}'):
            with self.subTest(text=text):
                (self.directory / defaults.DEFAULT_FILES[0]).write_text(text, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "invalid JSON"):
                    defaults.check_default_files(self.root)

    def test_arrays_and_scalars_are_rejected(self):
        for text in ("[]", "null", "true", "42", '"synthetic"'):
            with self.subTest(text=text):
                (self.directory / defaults.DEFAULT_FILES[0]).write_text(text, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "JSON objects"):
                    defaults.check_default_files(self.root)


if __name__ == "__main__":
    unittest.main(verbosity=2)
