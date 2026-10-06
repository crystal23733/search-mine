"""Check PR workflow and the design gate; never mutate GitHub resources."""

import json
import os
import re
import subprocess
import sys
from urllib.request import Request, urlopen


def api(relative):
    request = Request(
        f"https://api.github.com/repos/{os.environ['GITHUB_REPOSITORY']}/{relative}",
        headers={
            "Authorization": f"Bearer {os.environ['GH_TOKEN']}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    with urlopen(request, timeout=30) as response:
        return json.load(response)


def main():
    with open(os.environ["GITHUB_EVENT_PATH"], encoding="utf-8") as source:
        event = json.load(source)
    pr = event.get("pull_request")
    if not pr:
        print("Push event: contribution gate is enforced on PRs.")
        return
    head, base = pr["head"]["ref"], pr["base"]["ref"]
    branch = re.fullmatch(r"(feature|fix|docs|chore|refactor|test|hotfix)/(\d+)-[a-z0-9]+(?:-[a-z0-9]+)*", head)
    release = re.fullmatch(r"release/v\d+\.\d+\.\d+", head)
    if not branch and not release:
        raise ValueError("Branch must use type/issue-kebab-slug or release/vX.Y.Z")
    if base != "develop" and not (base == "main" and (release or (branch and branch[1] == "hotfix"))):
        raise ValueError("Work PRs target develop; main accepts release or hotfix only")
    if not re.match(r"^(feat|fix|docs|chore|refactor|test|perf|ci|build|revert)(\([a-z0-9-]+\))?!?: .+", pr["title"]):
        raise ValueError("PR title must be a Conventional Commit")
    body = pr.get("body") or ""
    issue_ids = re.findall(r"(?im)\b(?:closes|fixes|resolves)\s+#(\d+)\b", body)
    if not issue_ids or (branch and branch[2] not in issue_ids):
        raise ValueError("PR must close its branch issue using Closes #N")
    issue = api(f"issues/{branch[2] if branch else issue_ids[0]}")
    if "pull_request" in issue or not issue.get("milestone"):
        raise ValueError("Linked work issue must have a milestone")
    changed = []
    for page in range(1, 31):
        batch = api(f"pulls/{pr['number']}/files?per_page=100&page={page}")
        changed.extend(batch)
        if len(batch) < 100:
            break
    else:
        raise ValueError("PR exceeds the supported 3000-file review boundary")
    prefixes = ("crates/", "apps/", "packages/", "tests/", "deploy/", "config/", "spikes/")
    manifests = {"Cargo.toml", "Cargo.lock", "package.json", "pnpm-workspace.yaml", "pnpm-lock.yaml", "Dockerfile", "compose.yml", "docker-compose.yml"}
    product = any(
        item["status"] != "removed" and (item["filename"].startswith(prefixes) or item["filename"] in manifests)
        for item in changed
    )
    if product:
        approval = json.loads(subprocess.check_output(
            ["git", "show", f"{pr['base']['sha']}:docs/workflow/design-approval.json"],
            text=True, encoding="utf-8",
        ))
        if approval.get("status") != "approved" or not all(approval.get(x) for x in ("approved_by", "approved_at", "evidence", "reviewed_commit")):
            raise ValueError("Product changes require design approval already merged into base develop")
    print(f"Contribution gate passed: {head} -> {base}, milestone issue #{issue['number']}.")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"Contribution gate failed: {error}", file=sys.stderr)
        sys.exit(1)
