"""Check installation safety contracts without executing a Windows installer."""

import json
from pathlib import Path
import unittest
from urllib.parse import urlsplit
import uuid
import xml.etree.ElementTree as ET


DIRECTORY = Path(__file__).resolve().parent
NS = {
    "wix": "http://schemas.microsoft.com/wix/2006/wi",
    "util": "http://schemas.microsoft.com/wix/UtilExtension",
}


class BootstrapperContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = ET.parse(DIRECTORY / "bootstrapper.wxs").getroot()
        cls.bundle = cls.root.find("wix:Bundle", NS)
        cls.dependencies = json.loads(
            (DIRECTORY / "bootstrapper-dependencies.json").read_text(encoding="utf-8")
        )

    def test_dependencies_are_identified_and_pinned_to_official_https_sources(self):
        sources = {
            "wix": ("github.com", "/wixtoolset/wix3/releases/download/"),
            "vc_redist": ("download.visualstudio.microsoft.com", "/download/"),
        }
        for name, (host, prefix) in sources.items():
            with self.subTest(dependency=name):
                dependency = self.dependencies[name]
                self.assertRegex(dependency["version"], r"^\d+\.\d+\.\d+(?:\.\d+)?$")
                self.assertRegex(dependency["sha256"], r"^[a-f0-9]{64}$")
                self.assertNotEqual(dependency["sha256"], "0" * 64)
                uri = urlsplit(dependency["url"])
                self.assertEqual(uri.scheme, "https")
                self.assertEqual(uri.hostname, host)
                self.assertIsNone(uri.username)
                self.assertIsNone(uri.password)
                self.assertTrue(uri.path.startswith(prefix))
                self.assertFalse(uri.query or uri.fragment)
                self.assertNotRegex(uri.path.lower(), r"(?:^|/)latest(?:/|$)")
        self.assertTrue(self.dependencies["vc_redist"]["url"].endswith("/VC_redist.x64.exe"))

    def test_runtime_is_installed_before_the_application_and_both_are_embedded(self):
        chain = self.bundle.find("wix:Chain", NS)
        self.assertEqual([package.get("Id") for package in chain], ["VcRedistX64", "MurmureMsi"])
        for package in chain:
            self.assertEqual(package.get("Compressed"), "yes")
            self.assertEqual(package.get("Vital"), "yes")
            self.assertIsNone(package.get("DownloadUrl"))
        self.assertEqual(chain[0].get("SourceFile"), "$(var.VcRedistPath)")
        self.assertEqual(chain[1].get("SourceFile"), "$(var.MsiPath)")

    def test_shared_runtime_survives_uninstall_and_never_forces_a_restart(self):
        runtime = self.bundle.find("wix:Chain/wix:ExePackage", NS)
        self.assertEqual(runtime.get("Permanent"), "yes")
        self.assertEqual(runtime.get("PerMachine"), "yes")
        self.assertEqual(runtime.get("InstallCommand"), "/install /quiet /norestart")
        self.assertIsNone(runtime.get("UninstallCommand"))
        codes = {code.get("Value"): code.get("Behavior") for code in runtime.findall("wix:ExitCode", NS)}
        self.assertEqual(codes, {"0": "success", "3010": "scheduleReboot", None: "error"})

    def test_detection_checks_installed_status_and_numeric_minimum_version_in_machine_x64_registry(self):
        runtime = self.bundle.find("wix:Chain/wix:ExePackage", NS)
        self.assertEqual(
            runtime.get("DetectCondition"),
            "VcRuntimeX64Installed = 1 AND VcRuntimeX64Version >= v$(var.VcRedistVersion)",
        )
        searches = {item.get("Variable"): item for item in self.bundle.findall("util:RegistrySearch", NS)}
        for variable, value in [("VcRuntimeX64Installed", "Installed"), ("VcRuntimeX64Version", "Version")]:
            search = searches[variable]
            self.assertEqual(search.get("Root"), "HKLM")
            self.assertEqual(search.get("Win64"), "yes")
            self.assertEqual(search.get("Key"), r"SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64")
            self.assertEqual(search.get("Value"), value)

    def test_installer_does_not_launch_murmure_in_the_administrators_profile(self):
        msi = self.bundle.find("wix:Chain/wix:MsiPackage", NS)
        self.assertEqual(msi.get("DisplayInternalUI"), "no")
        self.assertNotIn("AUTOLAUNCHAPP", ET.tostring(self.root, encoding="unicode"))

    def test_bundle_identity_and_version_are_independent_from_the_application_version(self):
        identity = uuid.UUID(self.bundle.get("UpgradeCode"))
        self.assertNotEqual(identity.int, 0)
        self.assertEqual(self.bundle.get("Version"), "$(var.BundleVersion)")
        self.assertEqual(self.bundle.get("Compressed"), "yes")


if __name__ == "__main__":
    unittest.main(verbosity=2)
