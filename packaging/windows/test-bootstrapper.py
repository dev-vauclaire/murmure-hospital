"""Check installation safety contracts without executing a Windows installer."""

import json
from pathlib import Path
import re
import unittest
from urllib.parse import urlsplit
import uuid
import xml.etree.ElementTree as ET


DIRECTORY = Path(__file__).resolve().parent
NS = {
    "wix": "http://schemas.microsoft.com/wix/2006/wi",
    "util": "http://schemas.microsoft.com/wix/UtilExtension",
    "bal": "http://schemas.microsoft.com/wix/BalExtension",
    "theme": "http://wixtoolset.org/schemas/thmutil/2010",
    "loc": "http://schemas.microsoft.com/wix/2006/localization",
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

    def test_prerequisites_are_installed_before_the_application_and_all_are_embedded(self):
        chain = self.bundle.find("wix:Chain", NS)
        self.assertEqual(
            [package.get("Id") for package in chain],
            ["VcRedistX64", "MurmureMsi"],
        )
        for package in chain:
            self.assertEqual(package.get("Compressed"), "yes")
            self.assertEqual(package.get("Vital"), "yes")
            self.assertIsNone(package.get("DownloadUrl"))
        self.assertEqual(chain[0].get("SourceFile"), "$(var.VcRedistPath)")
        self.assertEqual(chain[1].get("SourceFile"), "$(var.MsiPath)")

    def test_shared_runtimes_survive_uninstall_and_are_installed_silently(self):
        commands = {
            "VcRedistX64": "/install /quiet /norestart",
        }
        for runtime in self.bundle.findall("wix:Chain/wix:ExePackage", NS):
            with self.subTest(runtime=runtime.get("Id")):
                self.assertEqual(runtime.get("Permanent"), "yes")
                self.assertEqual(runtime.get("PerMachine"), "yes")
                self.assertEqual(runtime.get("InstallCommand"), commands[runtime.get("Id")])
                self.assertIsNone(runtime.get("UninstallCommand"))
                codes = {
                    code.get("Value"): code.get("Behavior")
                    for code in runtime.findall("wix:ExitCode", NS)
                }
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

    def test_webview2_is_embedded_by_tauri_and_not_managed_by_burn(self):
        config = json.loads((DIRECTORY / "tauri.setup.conf.json").read_text(encoding="utf-8"))
        mode = config["bundle"]["windows"]["webviewInstallMode"]
        self.assertEqual(mode, {"type": "offlineInstaller", "silent": True})
        self.assertIsNone(self.bundle.find("wix:Chain/wix:ExePackage[@Id='WebView2RuntimeX64']", NS))
        self.assertIsNone(self.bundle.find("util:RegistrySearch[@Variable='WebView2MachineVersion']", NS))
        self.assertNotIn("webview2", self.dependencies)

    def test_workflow_builds_only_the_setup_using_the_msi_override(self):
        workflow = (
            DIRECTORY.parent.parent / ".github/workflows/build-hospital-windows.yml"
        ).read_text(encoding="utf-8")
        self.assertNotIn("inputs.installer_format", workflow)
        self.assertNotRegex(workflow, r"--bundles\s+nsis\b")
        self.assertRegex(workflow, r"--bundles\s+msi\b")
        self.assertIn("packaging/windows/tauri.setup.conf.json", workflow)
        self.assertRegex(workflow, r"pnpm\.cmd tauri build[^\n]*--verbose\b")

    def test_installer_does_not_launch_murmure_in_the_administrators_profile(self):
        msi = self.bundle.find("wix:Chain/wix:MsiPackage", NS)
        self.assertEqual(msi.get("DisplayInternalUI"), "no")
        self.assertNotIn("AUTOLAUNCHAPP", ET.tostring(self.root, encoding="unicode"))

    def test_install_page_checkboxes_bind_to_numeric_defaults_forwarded_to_the_msi(self):
        theme = ET.parse(DIRECTORY / "bootstrapper-theme.xml").getroot()
        page = theme.find("theme:Page[@Name='Install']", NS)
        msi = self.bundle.find("wix:Chain/wix:MsiPackage", NS)
        for name, default in (("HOSPITAL_DESKTOP_SHORTCUT", "1"), ("HOSPITAL_START_WITH_WINDOWS", "0")):
            with self.subTest(option=name):
                variable = self.bundle.find(f"wix:Variable[@Name='{name}']", NS)
                self.assertEqual(variable.get("Type"), "numeric")
                self.assertEqual(variable.get("Value"), default)
                self.assertIsNotNone(page.find(f"theme:Checkbox[@Name='{name}']", NS))
                self.assertEqual(len(theme.findall(f".//theme:Checkbox[@Name='{name}']", NS)), 1)
                prop = msi.find(f"wix:MsiProperty[@Name='{name}']", NS)
                self.assertEqual(prop.get("Value"), f"[{name}]")

    def test_missing_registry_values_preserve_defaults_and_saved_choices_override_them(self):
        searches = {item.get("Id"): item for item in self.bundle.findall("util:RegistrySearch", NS)}
        for identifier, variable, value in (
            ("HospitalDesktopChoice", "HOSPITAL_DESKTOP_SHORTCUT", "DesktopShortcut"),
            ("HospitalStartupChoice", "HOSPITAL_START_WITH_WINDOWS", "StartWithWindows"),
        ):
            with self.subTest(option=variable):
                read = searches[identifier]
                exists = searches[read.get("After")]
                self.assertEqual(exists.get("Result"), "exists")
                self.assertEqual(read.get("Condition"), exists.get("Variable"))
                self.assertEqual(read.get("Variable"), variable)
                self.assertEqual(read.get("Result"), "value")
                self.assertEqual(read.get("Format"), "raw")
                for search in (exists, read):
                    self.assertEqual(search.get("Root"), "HKLM")
                    self.assertEqual(search.get("Win64"), "yes")
                    self.assertEqual(search.get("Key"), r"Software\Vauclaire\Murmure\Installer")
                    self.assertEqual(search.get("Value"), value)

    def test_custom_theme_is_used_and_all_its_french_labels_resolve(self):
        application = self.bundle.find("wix:BootstrapperApplicationRef/bal:WixStandardBootstrapperApplication", NS)
        self.assertEqual(application.get("ThemeFile"), "$(var.ThemePath)")
        self.assertEqual(application.get("LocalizationFile"), "$(var.LocalizationPath)")
        self.assertEqual(application.get("LogoFile"), "$(var.LogoPath)")
        theme = (DIRECTORY / "bootstrapper-theme.xml").read_text(encoding="utf-8")
        localization = ET.parse(DIRECTORY / "bootstrapper-theme.fr-fr.wxl").getroot()
        self.assertEqual(localization.get("Culture"), "fr-fr")
        strings = {item.get("Id"): item.text for item in localization.findall("loc:String", NS)}
        self.assertFalse(set(re.findall(r"#\(loc\.([A-Za-z0-9_]+)\)", theme)) - strings.keys())
        self.assertIn("tous les utilisateurs", strings["HospitalOptionsScope"])
        self.assertTrue((DIRECTORY.parent.parent / "src/assets/app-icon.png").is_file())
        self.assertTrue((DIRECTORY.parent.parent / "licenses/wix3-MS-RL.txt").is_file())

    def test_bundle_identity_and_version_are_independent_from_the_application_version(self):
        identity = uuid.UUID(self.bundle.get("UpgradeCode"))
        self.assertEqual(identity, uuid.UUID("7F08032B-ECE6-40F1-8615-864F3EF3684C"))
        self.assertEqual(self.bundle.get("Version"), "$(var.BundleVersion)")
        self.assertEqual(self.bundle.get("Compressed"), "yes")


if __name__ == "__main__":
    unittest.main(verbosity=2)
