"""Exercise exact-commit CI validation using synthetic GitHub API responses."""

from io import BytesIO
import importlib.util
import json
from pathlib import Path
import unittest
from urllib.error import HTTPError, URLError
from urllib.parse import parse_qs, urlsplit


SPEC = importlib.util.spec_from_file_location(
    "check_ci", Path(__file__).resolve().parent.parent / "scripts/check-ci.py"
)
CI = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CI)
REPOSITORY = "synthetic-hospital/example"
SHA = "a" * 40
WORKFLOW = "ci-hospital-windows.yml"
SYNTHETIC_AUTH = "synthetic-auth-value"


def successful_run(**changes):
    return {
        "id": 42, "run_attempt": 1, "repository": {"full_name": REPOSITORY},
        "path": f".github/workflows/{WORKFLOW}", "head_sha": SHA,
        "event": "push", "status": "completed", "conclusion": "success",
    } | changes


class Response(BytesIO):
    status = 200


def response_for(runs):
    return Response(json.dumps({"workflow_runs": runs}).encode())


class ExactCommitGate(unittest.TestCase):
    def select(self, runs):
        return CI.select_successful_run(runs, REPOSITORY, SHA, WORKFLOW)

    def test_success_records_provenance_for_the_exact_commit(self):
        result = self.select([successful_run()])
        self.assertEqual(result["head_sha"], SHA)
        self.assertEqual(result["id"], 42)
        self.assertEqual(result["workflow"], WORKFLOW)
        self.assertEqual(result["url"], f"https://github.com/{REPOSITORY}/actions/runs/42")

    def test_missing_ci_and_success_on_a_different_commit_are_refused(self):
        for runs in ([], [successful_run(head_sha="b" * 40)]):
            with self.subTest(runs=runs), self.assertRaisesRegex(CI.CiCheckError, "exact commit"):
                self.select(runs)

    def test_other_workflow_repository_or_event_cannot_authorize_the_build(self):
        for changes in (
            {"path": ".github/workflows/unrelated.yml"},
            {"repository": {"full_name": "synthetic-other/example"}},
            {"event": "pull_request"},
        ):
            with self.subTest(changes=changes), self.assertRaises(CI.CiCheckError):
                self.select([successful_run(**changes)])

    def test_optional_workflow_ref_suffix_is_supported(self):
        result = self.select([successful_run(path=f".github/workflows/{WORKFLOW}@hospital/main")])
        self.assertEqual(result["id"], 42)

    def test_running_failed_cancelled_or_skipped_ci_is_refused(self):
        for status, conclusion in (
            ("queued", None), ("in_progress", None), ("completed", "failure"),
            ("completed", "cancelled"), ("completed", "skipped"),
        ):
            with self.subTest(status=status, conclusion=conclusion), self.assertRaises(CI.CiCheckError):
                self.select([successful_run(status=status, conclusion=conclusion)])

    def test_an_old_green_run_cannot_hide_a_more_recent_failure(self):
        with self.assertRaisesRegex(CI.CiCheckError, "did not succeed"):
            self.select([successful_run(id=41), successful_run(id=42, conclusion="failure")])

    def test_latest_attempt_must_finish_successfully(self):
        with self.assertRaisesRegex(CI.CiCheckError, "has not finished"):
            self.select([
                successful_run(run_attempt=1),
                successful_run(run_attempt=2, status="in_progress", conclusion=None),
            ])

    def test_malformed_run_identifiers_are_refused(self):
        for run in (None, successful_run(id=True), successful_run(id=0), successful_run(run_attempt=None)):
            with self.subTest(run=run), self.assertRaises(CI.CiCheckError):
                self.select([run])


class GitHubReadOnlyRequest(unittest.TestCase):
    def test_request_filters_the_exact_commit_without_writing_to_github(self):
        def open_url(request, timeout):
            self.assertEqual(request.get_method(), "GET")
            self.assertEqual(urlsplit(request.full_url).netloc, "api.github.com")
            self.assertNotIn(SYNTHETIC_AUTH, request.full_url)
            self.assertEqual(request.get_header("Authorization"), "Bearer " + SYNTHETIC_AUTH)
            self.assertEqual(parse_qs(urlsplit(request.full_url).query)["head_sha"], [SHA])
            self.assertEqual(parse_qs(urlsplit(request.full_url).query)["event"], ["push"])
            self.assertGreater(timeout, 0)
            return response_for([successful_run()])
        runs = CI.fetch_runs(REPOSITORY, SHA, WORKFLOW, SYNTHETIC_AUTH, open_url)
        self.assertEqual(runs, [successful_run()])

    def test_network_and_http_errors_are_refused_without_exposing_error_bodies(self):
        for error in (
            URLError("synthetic internal detail"),
            HTTPError("https://api.github.com", 403, "synthetic private detail", {}, None),
        ):
            def open_url(request, timeout):
                raise error
            with self.subTest(error=type(error).__name__):
                with self.assertRaises(CI.CiCheckError) as caught:
                    CI.fetch_runs(REPOSITORY, SHA, WORKFLOW, SYNTHETIC_AUTH, open_url)
                self.assertNotIn("synthetic private detail", str(caught.exception))
                self.assertNotIn(SYNTHETIC_AUTH, str(caught.exception))

    def test_bad_json_schema_and_oversized_responses_are_refused(self):
        for body in (b"not-json", b"[]", b'{"workflow_runs":null}', b"x" * (CI.MAX_RESPONSE_BYTES + 1)):
            with self.subTest(body_length=len(body)), self.assertRaises(CI.CiCheckError):
                CI.fetch_runs(REPOSITORY, SHA, WORKFLOW, SYNTHETIC_AUTH, lambda request, timeout: Response(body))

    def test_missing_authentication_and_invalid_targets_never_make_a_request(self):
        def unexpected_request(request, timeout):
            self.fail("An invalid request must not be sent.")
        for repository, sha, workflow, token in (
            (REPOSITORY, SHA, WORKFLOW, ""),
            ("../example", SHA, WORKFLOW, SYNTHETIC_AUTH),
            (REPOSITORY, "main", WORKFLOW, SYNTHETIC_AUTH),
            (REPOSITORY, SHA, "../ci.yml", SYNTHETIC_AUTH),
        ):
            with self.subTest(repository=repository, sha=sha, workflow=workflow), self.assertRaises(CI.CiCheckError):
                CI.fetch_runs(repository, sha, workflow, token, unexpected_request)

    def test_all_pages_are_considered_when_a_page_is_full(self):
        pages = iter([
            [successful_run(id=index + 1) for index in range(CI.PAGE_SIZE)],
            [successful_run(id=CI.PAGE_SIZE + 1)],
        ])
        runs = CI.fetch_runs(
            REPOSITORY, SHA, WORKFLOW, SYNTHETIC_AUTH,
            lambda request, timeout: response_for(next(pages)),
        )
        self.assertEqual(len(runs), CI.PAGE_SIZE + 1)

    def test_excessive_pagination_is_refused(self):
        batch = [successful_run(id=index + 1) for index in range(CI.PAGE_SIZE)]
        with self.assertRaisesRegex(CI.CiCheckError, "Too many"):
            CI.fetch_runs(REPOSITORY, SHA, WORKFLOW, SYNTHETIC_AUTH,
                          lambda request, timeout: response_for(batch))


if __name__ == "__main__":
    unittest.main(verbosity=2)
