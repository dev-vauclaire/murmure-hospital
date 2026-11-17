"""Check installer option patches using synthetic templates and no network calls."""

import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET


DIRECTORY = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("prepare_installer_options", DIRECTORY / "prepare-installer-options.py")
PREPARE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PREPARE)
NS = {"w": "http://schemas.microsoft.com/wix/2006/wi"}


def fixture() -> str:
    return '''<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi"><Product Id="*" UpgradeCode="{{upgrade_code}}">
''' + PREPARE.PROPERTY_ANCHOR + '''
<Directory Id="TARGETDIR"><Directory Id="DesktopFolder">
''' + PREPARE.DESKTOP_COMPONENT + '''
</Directory></Directory><DirectoryRef Id="INSTALLDIR">
''' + PREPARE.COMPONENT_ANCHOR + '''</Component></DirectoryRef><Feature Id="MainProgram">
''' + PREPARE.REFERENCE_ANCHOR + '''</Feature></Product></Wix>'''


class InstallerOptions(unittest.TestCase):
    def setUp(self):
        self.result = PREPARE.patch_template(fixture())
        self.root = ET.fromstring(self.result)

    def test_desktop_option_controls_the_native_public_desktop_shortcut(self):
        component = self.root.find(".//w:Directory[@Id='DesktopFolder']/w:Component", NS)
        self.assertEqual(component.get("Id"), "ApplicationShortcutDesktop")
        self.assertEqual(component.get("Transitive"), "yes")
        self.assertIsNone(component.get("Win64"))
        self.assertEqual(component.find("w:Condition", NS).text, 'HOSPITAL_DESKTOP_SHORTCUT = "1"')
        marker = component.find("w:RegistryValue", NS)
        self.assertEqual(marker.get("Root"), "HKCU")
        original_marker = ET.fromstring('<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">' + PREPARE.DESKTOP_COMPONENT + '</Wix>').find(".//w:RegistryValue", NS)
        self.assertEqual(marker.attrib, original_marker.attrib)
        self.assertEqual(component.find("w:Shortcut", NS).get("Target"), "[!Path]")

    def test_desktop_opt_out_removes_only_the_old_native_shortcut_on_install(self):
        component = self.root.find(".//w:Directory[@Id='DesktopFolder']/w:Component[@Id='HospitalDesktopShortcutCleanup']", NS)
        self.assertEqual(component.get("Transitive"), "yes")
        self.assertEqual(component.find("w:Condition", NS).text, 'HOSPITAL_DESKTOP_SHORTCUT = "0"')
        removal = component.find("w:RemoveFile", NS)
        self.assertEqual(removal.get("Name"), "{{product_name}}.lnk")
        self.assertEqual(removal.get("On"), "install")
        self.assertEqual(component.find("w:RegistryValue", NS).get("Root"), "HKCU")
        self.assertIsNotNone(self.root.find(".//w:ComponentRef[@Id='HospitalDesktopShortcutCleanup']", NS))

    def test_logon_option_is_machine_wide_quoted_and_starts_hidden(self):
        component = self.root.find(".//w:Component[@Id='HospitalStartWithWindows']", NS)
        self.assertEqual(component.get("Transitive"), "yes")
        self.assertEqual(component.get("Win64"), "$(var.Win64)")
        self.assertEqual(component.find("w:Condition", NS).text, 'HOSPITAL_START_WITH_WINDOWS = "1"')
        value = component.find("w:RegistryValue", NS)
        self.assertEqual(value.get("Root"), "HKLM")
        self.assertEqual(value.get("Key"), r"Software\Microsoft\Windows\CurrentVersion\Run")
        self.assertEqual(value.get("Name"), "MurmureHospital")
        self.assertEqual(value.get("Value"), '"[INSTALLDIR]{{main_binary_name}}.exe" --autostart --hidden')
        self.assertEqual(value.get("KeyPath"), "yes")

    def test_logon_command_uses_the_install_directory_without_cross_component_references(self):
        value = self.root.find(".//w:Component[@Id='HospitalStartWithWindows']/w:RegistryValue", NS)
        command = value.get("Value")
        # ICE69 forbids this registry component from referring to another
        # component's file. Directory properties avoid that ownership issue.
        self.assertNotRegex(command, r"\[(?:#|!|\$)[^\]]+\]")
        for directory, binary in (
            ("C:\\Program Files\\Murmure\\", "murmure"),
            ("D:\\Applications locales\\Murmure\\", "murmure-hopital"),
        ):
            with self.subTest(directory=directory, binary=binary):
                rendered = command.replace("{{main_binary_name}}", binary).replace("[INSTALLDIR]", directory)
                self.assertEqual(rendered, f'"{directory}{binary}.exe" --autostart --hidden')

    def test_both_options_are_persisted_as_machine_dwords(self):
        component = self.root.find(".//w:Component[@Id='HospitalInstallerOptions']", NS)
        self.assertEqual(component.get("Win64"), "$(var.Win64)")
        key = component.find("w:RegistryKey", NS)
        self.assertEqual(key.get("Root"), "HKLM")
        self.assertEqual(key.get("Key"), r"Software\Vauclaire\Murmure\Installer")
        values = {v.get("Name"): (v.get("Type"), v.get("Value")) for v in key}
        self.assertEqual(values, {
            "DesktopShortcut": ("integer", "[HOSPITAL_DESKTOP_SHORTCUT]"),
            "StartWithWindows": ("integer", "[HOSPITAL_START_WITH_WINDOWS]"),
        })

    def test_defaults_are_secure_and_components_are_referenced(self):
        for name, default in (("HOSPITAL_DESKTOP_SHORTCUT", "1"), ("HOSPITAL_START_WITH_WINDOWS", "0")):
            prop = self.root.find(f".//w:Property[@Id='{name}']", NS)
            self.assertEqual(prop.get("Value"), default)
            self.assertEqual(prop.get("Secure"), "yes")
        refs = {r.get("Id") for r in self.root.findall(".//w:Feature/w:ComponentRef", NS)}
        self.assertEqual(refs, {"RegistryEntries", "HospitalInstallerOptions", "HospitalStartWithWindows", "HospitalDesktopShortcutCleanup"})
        self.assertEqual(self.root.find("w:Product", NS).get("UpgradeCode"), "{{upgrade_code}}")

    def test_template_changes_and_duplicate_anchors_fail_closed(self):
        for template in (fixture().replace(PREPARE.DESKTOP_COMPONENT, ""), fixture() + PREPARE.PROPERTY_ANCHOR):
            with self.subTest(template=template[-50:]):
                with self.assertRaises(ValueError):
                    PREPARE.patch_template(template)

    def test_modified_or_unpinned_download_is_rejected(self):
        content = b"Synthetic template source."
        digest = hashlib.sha256(content).hexdigest()
        self.assertEqual(PREPARE.verify_source(content, digest), content.decode())
        for payload, expected in ((content + b"modified", digest), (content, "not-a-hash")):
            with self.assertRaises(ValueError):
                PREPARE.verify_source(payload, expected)

    def test_cli_upgrade_requires_template_review_before_network_access(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            package = root / "node_modules/@tauri-apps/cli/package.json"
            package.parent.mkdir(parents=True)
            package.write_text(json.dumps({"version": "99.0.0"}))
            with patch.object(PREPARE, "ROOT", root), patch.object(PREPARE, "urlopen") as request:
                with self.assertRaisesRegex(ValueError, "Tauri CLI changed"):
                    PREPARE.generate()
                request.assert_not_called()

    def test_tauri_config_uses_generated_template_and_keeps_offline_webview(self):
        config = json.loads((DIRECTORY / "tauri.setup.conf.json").read_text())
        windows = config["bundle"]["windows"]
        self.assertEqual(windows["wix"]["template"], "target/hospital-installer/main.wxs")
        self.assertEqual(windows["webviewInstallMode"], {"type": "offlineInstaller", "silent": True})


if __name__ == "__main__":
    unittest.main(verbosity=2)
