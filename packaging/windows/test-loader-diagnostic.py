"""Check diagnostic safety contracts without executing Windows tools or downloading files.

These static checks do not validate PowerShell syntax or the Windows DLL loader.
The workflow YAML is also parsed separately during local validation.
"""

from pathlib import Path
import re
import unittest


DIRECTORY = Path(__file__).resolve().parent
ROOT = DIRECTORY.parent.parent


def step_blocks(workflow: str) -> list[str]:
    """Read top-level step boundaries without adding a YAML dependency to CI."""
    starts = list(re.finditer(r"(?m)^([ ]*)- (?:name|uses):", workflow))
    if not starts:
        return []
    indentation = min(len(match[1]) for match in starts)
    starts = [match for match in starts if len(match[1]) == indentation]
    return [
        workflow[match.start():starts[index + 1].start() if index + 1 < len(starts) else len(workflow)]
        for index, match in enumerate(starts)
    ]


def scalar_list(block: str, key: str) -> list[str]:
    """Read a simple scalar list used by the branch/path trigger restrictions."""
    match = re.search(rf"(?m)^([ ]*){re.escape(key)}:[ \t]*(.*)$", block)
    if match is None:
        return []
    if match[2].strip().startswith("["):
        return [value.strip().strip("'\"") for value in match[2].strip()[1:-1].split(",")]
    values = []
    indentation = len(match[1])
    for line in block[match.end():].splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if len(line) - len(line.lstrip()) <= indentation:
            break
        item = re.match(r"[ ]+-[ \t]+(.+)$", line)
        if item:
            values.append(item[1].strip().strip("'\""))
    return values


class LoaderDiagnosticContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workflow = (ROOT / ".github/workflows/diagnose-hospital-windows.yml").read_text(encoding="utf-8")
        cls.script = (DIRECTORY / "diagnose-test-loader.ps1").read_text(encoding="utf-8")
        cls.steps = step_blocks(cls.workflow)

    def test_workflow_is_manual_or_strictly_scoped_read_only_and_does_not_publish(self):
        self.assertRegex(self.workflow, r"(?m)^[ ]+workflow_dispatch:")
        self.assertNotRegex(self.workflow, r"(?m)^[ ]+(?:pull_request|pull_request_target|schedule|workflow_run):")
        triggers = self.workflow.split("\npermissions:", 1)[0]
        if re.search(r"(?m)^[ ]+push:", triggers):
            self.assertEqual(scalar_list(triggers, "branches"), ["feature/windows-loader-diagnostic"])
            self.assertEqual(set(scalar_list(triggers, "paths")), {
                ".github/workflows/diagnose-hospital-windows.yml",
                "packaging/windows/diagnose-test-loader.ps1",
                "packaging/windows/test-loader-diagnostic.py",
            })
        self.assertRegex(self.workflow, r"(?m)^permissions:[ \t]*\n[ ]+contents:[ \t]*read[ \t]*$")
        self.assertNotRegex(self.workflow, r"(?m)^[ ]+[\w-]+:[ \t]*write[ \t]*$")
        self.assertIn("persist-credentials: false", self.workflow)
        combined = self.workflow + self.script
        self.assertNotRegex(combined, r"(?i)\bgit\s+push\b|\bgh\s+release\b|\btauri(?:\.cmd)?\s+build\b")
        self.assertNotIn("secrets.", combined)
        self.assertNotRegex(combined, r"(?i)\bGet-ChildItem\s+Env:|!peb\b|!envvar\b|(?<!\w)\.dump\b")

    def test_comparison_uses_the_same_runner_and_independent_build_directories(self):
        self.assertEqual(len(re.findall(r"(?m)^[ ]+runs-on:", self.workflow)), 1)
        self.assertIn("runs-on: windows-2022", self.workflow)
        self.assertIn("1.98.1", self.workflow)
        self.assertIn("4d30219d4d69b2d2bc2366127277b8b2e7f80795", self.workflow)
        self.assertIn("d5e382cee4dbdeb312a3492271d5782cdae98528", self.workflow)
        diagnostics = [step for step in self.steps if "-SourceDirectory" in step and "-TraceOnly" not in step]
        self.assertEqual(len(diagnostics), 2)
        labels = []
        targets = []
        for step in diagnostics:
            labels.append(re.search(r"-Label\s+['\"]?(before|after)\b", step)[1])
            targets.append(re.search(r"(?m)^[ ]+CARGO_TARGET_DIR:[ \t]*(.+)$", step)[1].strip())
        self.assertEqual(labels, ["before", "after"])
        self.assertEqual(len(set(targets)), 2)
        for step in self.steps:
            if "rust-cache@" in step:
                self.assertIn("cache-targets: false", step)
            if "actions/cache@" in step:
                self.assertNotRegex(step, r"(?m)^[ ]+(?:path|workspaces):[^\n]*target")

    def test_failed_variants_do_not_prevent_collection_of_reports(self):
        diagnostics = [step for step in self.steps if "-SourceDirectory" in step and "-TraceOnly" not in step]
        for step in diagnostics:
            self.assertIn("continue-on-error: true", step)
        self.assertRegex(diagnostics[1], r"if:[ \t]*(?:\$\{\{[ \t]*)?always\(\)")
        uploads = [step for step in self.steps if "actions/upload-artifact@" in step]
        self.assertEqual(len(uploads), 1)
        self.assertRegex(uploads[0], r"if:[ \t]*(?:\$\{\{[ \t]*)?always\(\)")
        self.assertRegex(uploads[0], r"(?m)^[ ]+path:[^\n]*diagnostics")
        self.assertNotRegex(uploads[0], r"src-tauri[/\\]target|resources[/\\]parakeet")

    def test_test_program_is_only_listed_and_external_processes_are_bounded(self):
        for argument in ("--locked", "--lib", "--no-run", "--message-format=json", "--list"):
            self.assertIn(argument, self.script)
        self.assertIn("compiler-artifact", self.script)
        self.assertIn("executable", self.script)
        self.assertRegex(self.script, r"(?i)WaitForExit\([^)]*\S[^)]*\)|Wait-Process[^\n]*-Timeout\b")
        self.assertRegex(self.script, r"(?i)\.Kill\(\$true\)|taskkill(?:\.exe)?[^\n]*/T\b")
        self.assertNotIn("--nocapture", self.script)
        self.assertNotRegex(self.script, r"(?i)\bCopy-Item[^\n]*\.dll|\bSetDllDirectory\b|\bAddDllDirectory\b")

    def test_loader_flags_are_image_scoped_and_removed_in_finally(self):
        self.assertIn("+sls", self.script)
        self.assertIn("-sls", self.script)
        self.assertRegex(self.script, r"['\"]/i['\"]")
        self.assertNotRegex(self.script, r"['\"]/(?:r|k)['\"]")
        self.assertRegex(self.script, r"(?is)\bfinally\s*\{.*?-sls")


if __name__ == "__main__":
    unittest.main(verbosity=2)
