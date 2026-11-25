"""Check the distribution identity and updater removal without launching the app."""

import json
from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[3]


class HospitalIdentity(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.config = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))

    def test_public_identity_is_consistent(self):
        self.assertEqual(self.config["productName"], "Murmure Vauclaire")
        self.assertEqual(self.config["mainBinaryName"], "murmure-vauclaire")
        self.assertEqual(self.config["identifier"], "lan.ch-vauclaire.murmure")
        self.assertEqual(self.config["app"]["windows"][0]["title"], self.config["productName"])
        self.assertEqual(self.config["bundle"]["publisher"], "ch-vauclaire")
        self.assertEqual(self.config["bundle"]["homepage"], "https://github.com/dev-vauclaire/murmure-hospital")
        self.assertIn(f'<title>{self.config["productName"]}</title>', (ROOT / "index.html").read_text(encoding="utf-8"))

    def test_updater_has_no_configuration_dependency_or_registered_backend(self):
        self.assertNotIn("updater", self.config.get("plugins", {}))
        package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
        for section in ("dependencies", "devDependencies"):
            self.assertNotIn("@tauri-apps/plugin-updater", package.get(section, {}))
        cargo = tomllib.loads((ROOT / "src-tauri/Cargo.toml").read_text(encoding="utf-8"))
        self.assertNotIn("tauri-plugin-updater", cargo.get("dependencies", {}))
        for target in cargo.get("target", {}).values():
            self.assertNotIn("tauri-plugin-updater", target.get("dependencies", {}))
        locked = tomllib.loads((ROOT / "src-tauri/Cargo.lock").read_text(encoding="utf-8"))
        self.assertNotIn("tauri-plugin-updater", {package["name"] for package in locked["package"]})
        self.assertNotIn("@tauri-apps/plugin-updater", (ROOT / "pnpm-lock.yaml").read_text(encoding="utf-8"))
        self.assertNotIn("tauri_plugin_updater", (ROOT / "src-tauri/src/lib.rs").read_text(encoding="utf-8"))
        self.assertFalse((ROOT / "src/features/update-checker/update-checker.tsx").exists())

    def test_updater_permissions_are_removed(self):
        for directory, pattern in (("src-tauri/capabilities", "*.json"), ("src-tauri/permissions", "*.toml")):
            for path in (ROOT / directory).glob(pattern):
                with self.subTest(path=path.name):
                    self.assertNotIn("updater:", path.read_text(encoding="utf-8"))

    def test_native_namespaces_and_linux_resources_match_the_application_id(self):
        identifier = self.config["identifier"]
        main = (ROOT / "src-tauri/src/main.rs").read_text(encoding="utf-8")
        self.assertIn(f'"GLOBAL_HOTKEY_APP_ID", "{identifier}"', main)
        keyring = (ROOT / "src-tauri/src/llm/helpers.rs").read_text(encoding="utf-8")
        self.assertIn(f'KEYRING_SERVICE: &str = "{identifier}"', keyring)
        for hook in ("postinst", "prerm", "postrm"):
            self.assertIn(f"{identifier}.desktop", (ROOT / "packaging/linux" / hook).read_text(encoding="utf-8"))
        for target in ("deb", "rpm"):
            files = self.config["bundle"]["linux"][target]["files"]
            rule = files["/usr/lib/udev/rules.d/60-murmure-vauclaire-uinput.rules"]
            self.assertTrue((ROOT / "src-tauri" / rule).is_file())


if __name__ == "__main__":
    unittest.main(verbosity=2)
