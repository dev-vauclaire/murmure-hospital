"""Generate the hospital MSI template from one verified upstream Tauri template.

The generated file stays in target/. Upstream template changes require an explicit
version/hash review; each hospital patch must match exactly once.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
from urllib.parse import urlsplit
from urllib.request import urlopen


DIRECTORY = Path(__file__).resolve().parent
ROOT = DIRECTORY.parent.parent
OUTPUT = ROOT / "src-tauri/target/hospital-installer/main.wxs"

PROPERTY_ANCHOR = '        <Property Id="REINSTALLMODE" Value="amus" />'
PROPERTIES = '''        <!-- These choices are supplied by the setup and apply to the whole machine. -->
        <Property Id="HOSPITAL_DESKTOP_SHORTCUT" Value="1" Secure="yes" />
        <Property Id="HOSPITAL_START_WITH_WINDOWS" Value="0" Secure="yes" />
        <Condition Message="Les options d'installation doivent valoir 0 ou 1."><![CDATA[
            (HOSPITAL_DESKTOP_SHORTCUT = "0" OR HOSPITAL_DESKTOP_SHORTCUT = "1")
            AND (HOSPITAL_START_WITH_WINDOWS = "0" OR HOSPITAL_START_WITH_WINDOWS = "1")
        ]]></Condition>
'''

DESKTOP_COMPONENT = r'''                <Component Id="ApplicationShortcutDesktop" Guid="*">
                    <Shortcut Id="ApplicationDesktopShortcut" Name="{{product_name}}" Description="Runs {{product_name}}" Target="[!Path]" WorkingDirectory="INSTALLDIR" />
                    <RemoveFolder Id="DesktopFolder" On="uninstall" />
                    <RegistryValue Root="HKCU" Key="Software\\{{manufacturer}}\\{{product_name}}" Name="Desktop Shortcut" Type="integer" Value="1" KeyPath="yes" />
                </Component>'''

# ALLUSERS resolves DesktopFolder to the public desktop. Keep Tauri's HKCU
# key path (and its original registry view): ICE43 requires this metadata for
# non-advertised shortcuts. The actual machine-wide choices are stored below.
DESKTOP_OPTION = DESKTOP_COMPONENT.replace(
    'Guid="*">', 'Guid="*" Transitive="yes">\n                    <Condition>HOSPITAL_DESKTOP_SHORTCUT = "1"</Condition>', 1
)
# The old Tauri MSI skips RemoveShortcuts during major upgrades. Remove only
# its known shortcut when opting out, otherwise an older .lnk would survive.
DESKTOP_OPTION += '''
                <Component Id="HospitalDesktopShortcutCleanup" Guid="*" Transitive="yes">
                    <Condition>HOSPITAL_DESKTOP_SHORTCUT = "0"</Condition>
                    <RemoveFile Id="HospitalPreviousDesktopShortcut" Name="{{product_name}}.lnk" On="install" />
                    <RegistryValue Root="HKCU" Key="Software\\Vauclaire\\Murmure\\Installer" Name="DesktopShortcutRemoved" Type="integer" Value="1" KeyPath="yes" />
                </Component>'''

COMPONENT_ANCHOR = '            <Component Id="RegistryEntries" Guid="*">'
COMPONENTS = '''            <Component Id="HospitalInstallerOptions" Guid="*" Win64="$(var.Win64)">
                <RegistryKey Root="HKLM" Key="Software\\Vauclaire\\Murmure\\Installer">
                    <RegistryValue Name="DesktopShortcut" Type="integer" Value="[HOSPITAL_DESKTOP_SHORTCUT]" KeyPath="yes" />
                    <RegistryValue Name="StartWithWindows" Type="integer" Value="[HOSPITAL_START_WITH_WINDOWS]" />
                </RegistryKey>
            </Component>
            <Component Id="HospitalStartWithWindows" Guid="*" Win64="$(var.Win64)" Transitive="yes">
                <Condition>HOSPITAL_START_WITH_WINDOWS = "1"</Condition>
                <RegistryValue Root="HKLM" Key="Software\\Microsoft\\Windows\\CurrentVersion\\Run"
                               Name="MurmureHospital" Type="string" Value="&quot;[#Path]&quot; --autostart --hidden" KeyPath="yes" />
            </Component>
'''

REFERENCE_ANCHOR = '            <ComponentRef Id="RegistryEntries"/>'
REFERENCES = '''            <ComponentRef Id="HospitalInstallerOptions" />
            <ComponentRef Id="HospitalStartWithWindows" />
            <ComponentRef Id="HospitalDesktopShortcutCleanup" />
'''


def replace_once(text: str, old: str, new: str) -> str:
    if text.count(old) != 1:
        raise ValueError("The upstream MSI template changed; review the hospital installer patches.")
    return text.replace(old, new, 1)


def patch_template(text: str) -> str:
    text = replace_once(text, PROPERTY_ANCHOR, PROPERTIES + PROPERTY_ANCHOR)
    text = replace_once(text, DESKTOP_COMPONENT, DESKTOP_OPTION)
    text = replace_once(text, COMPONENT_ANCHOR, COMPONENTS + COMPONENT_ANCHOR)
    text = replace_once(text, REFERENCE_ANCHOR, REFERENCE_ANCHOR + "\n" + REFERENCES.rstrip())
    return '''<!-- Generated from Tauri CLI's pinned main.wxs; do not edit this target file.
     Copyright (c) 2017 - Present Tauri Apps Contributors.
     Upstream template licensed under MIT; see licenses/tauri-installer-MIT.txt.
     Hospital modifications: machine-wide desktop shortcut and logon options. -->
''' + text


def verify_source(data: bytes, expected_hash: str) -> str:
    if re.fullmatch(r"[a-f0-9]{64}", expected_hash) is None:
        raise ValueError("The upstream MSI template requires a pinned SHA256.")
    if hashlib.sha256(data).hexdigest() != expected_hash:
        raise ValueError("The upstream MSI template does not match its pinned SHA256.")
    return data.decode("utf-8")


def generate(source: Path | None = None) -> Path:
    manifest = json.loads((DIRECTORY / "msi-template-source.json").read_text(encoding="utf-8"))
    version = manifest["tauri_cli_version"]
    installed = json.loads((ROOT / "node_modules/@tauri-apps/cli/package.json").read_text(encoding="utf-8"))
    if installed["version"] != version:
        raise ValueError("Tauri CLI changed; review the MSI template source and hospital patches first.")
    url = manifest["url"]
    expected_url = f"https://raw.githubusercontent.com/tauri-apps/tauri/tauri-cli-v{version}/crates/tauri-bundler/src/bundle/windows/msi/main.wxs"
    if url != expected_url or urlsplit(url).scheme != "https":
        raise ValueError("The MSI template must come from the pinned official Tauri release.")
    if source is None:
        with urlopen(url, timeout=60) as response:
            data = response.read(1_048_577)
    else:
        data = source.read_bytes()
    if len(data) > 1_048_576:
        raise ValueError("The upstream MSI template exceeds its size limit.")
    result = patch_template(verify_source(data, manifest["sha256"]))
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(result, encoding="utf-8")
    return OUTPUT


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, help="Use an already downloaded template; its SHA256 is still verified.")
    args = parser.parse_args()
    try:
        output = generate(args.source)
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f"MSI option template preparation failed: {error}\n")
    print(f"Prepared verified hospital MSI template: {output}")


if __name__ == "__main__":
    main()
