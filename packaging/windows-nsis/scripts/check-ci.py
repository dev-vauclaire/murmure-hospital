"""Require a successful hospital CI run for the exact commit being packaged."""

import argparse
import json
import os
from pathlib import Path
import re
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode
from urllib.request import HTTPRedirectHandler, Request, build_opener


PAGE_SIZE = 100
MAX_PAGES = 10
MAX_RESPONSE_BYTES = 8 * 1024 * 1024


class CiCheckError(Exception):
    """The build cannot establish a successful CI result for its source commit."""


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, target):
        # Never forward the workflow's authentication to a redirect target.
        return None


def validate_target(repository: str, sha: str, workflow: str) -> None:
    if (re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is None
            or any(part in (".", "..") for part in repository.split("/"))):
        raise CiCheckError("A GitHub owner/repository is required.")
    if re.fullmatch(r"[a-f0-9]{40}", sha) is None:
        raise CiCheckError("The source commit must be a complete lowercase Git SHA.")
    if re.fullmatch(r"[A-Za-z0-9_-][A-Za-z0-9_.-]*\.ya?ml", workflow) is None:
        raise CiCheckError("The CI workflow must be a YAML filename.")


def fetch_runs(repository: str, sha: str, workflow: str, token: str, open_url=None) -> list:
    validate_target(repository, sha, workflow)
    if not token:
        raise CiCheckError("GH_TOKEN is required with read access to GitHub Actions.")
    if open_url is None:
        open_url = build_opener(NoRedirect()).open
    endpoint = f"https://api.github.com/repos/{repository}/actions/workflows/{quote(workflow, safe='')}/runs"
    runs = []
    for page in range(1, MAX_PAGES + 1):
        query = urlencode({"head_sha": sha, "event": "push", "per_page": PAGE_SIZE, "page": page})
        request = Request(endpoint + "?" + query, headers={
            "Accept": "application/vnd.github+json",
            "Authorization": "Bearer " + token,
            "X-GitHub-Api-Version": "2026-03-10",
            "User-Agent": "murmure-vauclaire-ci-check",
        })
        try:
            with open_url(request, timeout=15) as response:
                if response.status != 200:
                    raise CiCheckError("The GitHub CI API did not return a successful response.")
                data = response.read(MAX_RESPONSE_BYTES + 1)
            if len(data) > MAX_RESPONSE_BYTES:
                raise CiCheckError("The GitHub CI API response exceeds its size limit.")
            result = json.loads(data)
        except HTTPError as error:
            raise CiCheckError(f"GitHub CI API returned HTTP {error.code}; check Actions read access.") from None
        except (URLError, OSError, TimeoutError):
            raise CiCheckError("Unable to contact the GitHub CI API; the build remains blocked.") from None
        except (ValueError, UnicodeError):
            raise CiCheckError("The GitHub CI API returned invalid JSON.") from None
        if not isinstance(result, dict) or not isinstance(result.get("workflow_runs"), list):
            raise CiCheckError("The GitHub CI API returned an unexpected response format.")
        batch = result["workflow_runs"]
        runs.extend(batch)
        if len(batch) < PAGE_SIZE:
            return runs
    raise CiCheckError("Too many CI runs were returned to establish a reliable result.")


def select_successful_run(runs: list, repository: str, sha: str, workflow: str) -> dict:
    validate_target(repository, sha, workflow)
    matching = []
    for run in runs:
        if not isinstance(run, dict):
            raise CiCheckError("The GitHub CI API returned an invalid workflow run.")
        owner = run.get("repository")
        path = run.get("path")
        if (run.get("head_sha") != sha or run.get("event") != "push"
                or not isinstance(owner, dict)
                or str(owner.get("full_name", "")).casefold() != repository.casefold()
                or not isinstance(path, str)
                or path.split("@", 1)[0] != f".github/workflows/{workflow}"):
            continue
        run_id = run.get("id")
        attempt = run.get("run_attempt")
        if type(run_id) is not int or run_id <= 0 or type(attempt) is not int or attempt <= 0:
            raise CiCheckError("The GitHub CI API returned invalid run identifiers.")
        matching.append(run)
    if not matching:
        raise CiCheckError("No hospital CI push run exists for this exact commit. Run CI before building.")
    # An older green run must not hide a more recent failed or ongoing run.
    latest = max(matching, key=lambda run: (run["id"], run["run_attempt"]))
    url = f"https://github.com/{repository}/actions/runs/{latest['id']}"
    if latest.get("status") != "completed":
        raise CiCheckError(f"CI has not finished for this exact commit. Wait before building: {url}")
    if latest.get("conclusion") != "success":
        raise CiCheckError(f"CI did not succeed for this exact commit. Build refused: {url}")
    return {
        "id": latest["id"],
        "url": url,
        "head_sha": sha,
        "run_attempt": latest["run_attempt"],
        "workflow": workflow,
        "event": "push",
        "repository": repository,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True, help="GitHub owner/repository.")
    parser.add_argument("--sha", required=True, help="Exact source commit being packaged.")
    parser.add_argument("--workflow", default="ci-hospital-windows.yml", help="Hospital CI workflow filename.")
    parser.add_argument("--output", required=True, type=Path, help="Where to record the verified CI provenance.")
    args = parser.parse_args()
    try:
        runs = fetch_runs(args.repository, args.sha, args.workflow, os.environ.get("GH_TOKEN", ""))
        result = select_successful_run(runs, args.repository, args.sha, args.workflow)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=4) + "\n", encoding="utf-8")
    except (CiCheckError, OSError) as error:
        parser.exit(1, f"CI validation refused: {error}\n")
    print(f"Hospital CI succeeded for source commit {args.sha}: {result['url']}")


if __name__ == "__main__":
    main()
