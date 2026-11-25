"""Compile the real NSIS hook; Windows can execute isolated synthetic harnesses.

Only the harness copy's registry reads and redistributable execution are mocked.
No real registry key is changed and no runtime installer is executed.
"""

import argparse
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest


HOOK = Path(__file__).resolve().parents[1] / "hooks.nsh"
COMPILER = None
EXECUTE = False
MINIMUM = "14.52.37029.0"


def literal(value: str) -> str:
    return value.replace("$", "$$").replace('"', '$\\"')


def expect(register: str, expected: str) -> str:
    return f'StrCmp "{register}" "{expected}" +3\nSetErrorLevel 42\nQuit\n'


class VcHook(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if COMPILER is None:
            raise unittest.SkipTest("makensis is not available; run after the NSIS build with --makensis.")

    def compile_harness(self, body, *, views=None, exit_code=None, launch_error=False, expected_exit=0):
        with TemporaryDirectory(prefix="murmure-vc-hook-") as temporary:
            root = Path(temporary)
            hook = root / "packaging/windows-nsis/hooks.nsh"
            hook.parent.mkdir(parents=True)
            definitions = root / "src-tauri/target/nsis-prerequisites/vc-redist.nsh"
            definitions.parent.mkdir(parents=True)
            payload = definitions.parent / "synthetic.exe"
            payload.write_bytes(b"Synthetic payload. Never execute this file.")
            definitions.write_text(
                f'!define HOSPITAL_VC_REDIST_VERSION "{MINIMUM}"\n'
                f'!define HOSPITAL_VC_REDIST_FILE "{literal(str(payload))}"\n',
                encoding="utf-8",
            )
            source = HOOK.read_text(encoding="utf-8")
            if views is not None:
                # These two external reads are replaced only in the temporary
                # copy; the production comparison and decisions remain intact.
                reads = ('    ReadRegDWORD $0 HKLM "${HOSPITAL_VC_REGISTRY_KEY}" "Installed"\n'
                         '    ReadRegStr $1 HKLM "${HOSPITAL_VC_REGISTRY_KEY}" "Version"')
                self.assertEqual(source.count(reads), 1)
                fixture = (
                    '    !if ${VIEW} == 64\n'
                    f'        StrCpy $0 "{views[64][0]}"\n'
                    f'        StrCpy $1 "{views[64][1]}"\n'
                    '    !else\n'
                    f'        StrCpy $0 "{views[32][0]}"\n'
                    f'        StrCpy $1 "{views[32][1]}"\n'
                    '    !endif'
                )
                source = source.replace(reads, fixture)
            if exit_code is not None:
                command = '        ExecWait \'"$PLUGINSDIR\\VC_redist.x64.exe" /install /quiet /norestart\' $0'
                self.assertEqual(source.count(command), 1)
                source = source.replace(
                    command,
                    '        IntOp $HospitalVcTestCalls $HospitalVcTestCalls + 1\n'
                    f'        StrCpy $0 {exit_code}\n'
                    + ('        SetErrors' if launch_error else '        ClearErrors'),
                )
            hook.write_text(source, encoding="utf-8")
            executable = root / "harness.exe"
            script = root / "harness.nsi"
            script.write_text(
                'Unicode true\nName "Synthetic VC prerequisite tests"\n'
                f'OutFile "{literal(str(executable))}"\n'
                'RequestExecutionLevel user\nSilentInstall silent\n'
                'Var PassiveMode\nVar HospitalVcTestCalls\n'
                f'!include "{literal(str(hook))}"\n'
                'Section\n'
                # Compile both real macros, including File, but do not run
                # them unless the test body below explicitly calls a mock.
                '    Goto hospital_test_start\n'
                '    !insertmacro NSIS_HOOK_PREINSTALL\n'
                '    !insertmacro NSIS_HOOK_POSTINSTALL\n'
                '    hospital_test_start:\n'
                '    StrCpy $PassiveMode 0\n'
                '    StrCpy $HospitalVcTestCalls 0\n'
                '    SetRebootFlag false\n'
                + body + '\nSectionEnd\n'
                'Function .onInstSuccess\n'
                # A reboot branch must quit before Tauri's success callback,
                # which would otherwise honor /R and launch the application.
                '    SetErrorLevel 99\nFunctionEnd\n',
                encoding="utf-8",
            )
            verbosity = "/V2" if sys.platform == "win32" else "-V2"
            compiled = subprocess.run(
                [str(COMPILER), verbosity, str(script)], capture_output=True,
                text=True, encoding="utf-8", errors="replace", timeout=60,
            )
            self.assertEqual(compiled.returncode, 0, compiled.stdout + compiled.stderr)
            self.assertTrue(executable.is_file())
            if EXECUTE:
                result = subprocess.run([str(executable), "/S"], capture_output=True, timeout=30)
                self.assertEqual(result.returncode, expected_exit, result.stdout + result.stderr)

    def test_complete_production_hook_compiles(self):
        self.compile_harness('SetErrorLevel 0\nQuit\n')

    def test_version_comparison_and_variable_preservation(self):
        versions = {
            "": 0, "invalid": 0, "v": 0, "14.invalid.0.0": 0,
            "14..52.0": 0, "14.52.37029.0.": 0, "14.52.37029.0.1": 0,
            "65536.0.0.0": 0, "14.9.40000.0": 0, "14.52.37028.99": 0,
            "14.52.37029.0": 1, "v14.52.37029.0": 1, "V14.52.37029.0": 1,
            "14.52.37029": 1, "14.52.37029.1": 1, "14.53.0.0": 1,
            "15.0.0.0": 1,
        }
        body = ''.join(f'StrCpy ${number} "saved-{number}"\n' for number in range(6))
        for version, accepted in versions.items():
            body += f'Push "{version}"\nCall HospitalVcVersionIsSufficient\nPop $R0\n'
            body += expect("$R0", str(accepted))
            body += ''.join(expect(f"${number}", f"saved-{number}") for number in range(6))
        self.compile_harness(body + 'SetErrorLevel 0\nQuit\n')

    def test_runtime_detection_and_installation_decisions(self):
        scenarios = (
            ({64: ("", ""), 32: ("", "")}, 1),
            ({64: ("1", "v14.9.40000.0"), 32: ("", "")}, 1),
            ({64: ("1", "v14.52.37029.0"), 32: ("", "")}, 0),
            ({64: ("", ""), 32: ("1", "v14.53.0.0")}, 0),
            ({64: ("1", "v14.53.0.0"), 32: ("1", "v14.9.0.0")}, 0),
            ({64: ("0", "v14.53.0.0"), 32: ("", "")}, 1),
        )
        for views, calls in scenarios:
            with self.subTest(views=views):
                body = '!insertmacro NSIS_HOOK_PREINSTALL\n'
                body += expect("$HospitalVcTestCalls", str(calls))
                body += 'IfRebootFlag 0 +3\nSetErrorLevel 43\nQuit\nSetErrorLevel 0\nQuit\n'
                self.compile_harness(body, views=views, exit_code=0)

    def test_failed_prerequisite_aborts_before_application_installation(self):
        for code, launch_error, expected in ((1603, False, 1603), (1638, False, 1638), (0, True, 1603)):
            with self.subTest(code=code, launch_error=launch_error):
                self.compile_harness(
                    '!insertmacro NSIS_HOOK_PREINSTALL\nSetErrorLevel 42\nQuit\n',
                    views={64: ("", ""), 32: ("", "")},
                    exit_code=code, launch_error=launch_error, expected_exit=expected,
                )

    def test_reboot_flag_and_silent_or_passive_completion(self):
        for passive in (False, True):
            with self.subTest(passive=passive):
                body = '!insertmacro NSIS_HOOK_PREINSTALL\n'
                body += expect("$HospitalVcTestCalls", "1")
                body += 'IfRebootFlag +3 0\nSetErrorLevel 43\nQuit\n'
                if passive:
                    body += 'StrCpy $PassiveMode 1\nSetSilent normal\n'
                body += '!insertmacro NSIS_HOOK_POSTINSTALL\nSetErrorLevel 42\nQuit\n'
                self.compile_harness(
                    body, views={64: ("", ""), 32: ("", "")},
                    exit_code=3010, expected_exit=3010,
                )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--makensis", type=Path, help="Compiler supplied by Tauri after its Windows build.")
    parser.add_argument("--execute", action="store_true", help="Execute synthetic harnesses on Windows only.")
    arguments = parser.parse_args()
    if arguments.execute and sys.platform != "win32":
        parser.error("--execute requires Windows; no Wine or real prerequisite is used.")
    COMPILER = arguments.makensis or shutil.which("makensis")
    if arguments.makensis is not None and not arguments.makensis.is_file():
        parser.error("The specified makensis compiler does not exist.")
    EXECUTE = arguments.execute
    print("NSIS test mode: compile and execute synthetic harnesses." if EXECUTE
          else "NSIS test mode: compile only; Windows execution is not performed.")
    unittest.main(argv=[sys.argv[0]], verbosity=2)
