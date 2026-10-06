"""Validate the repository's design documents without product dependencies."""

from pathlib import Path
import json
import re
import sys
from urllib.parse import unquote, urlparse

ROOT = Path(__file__).resolve().parents[1]
PRODUCT = [
    "01-vision", "02-competitors", "03-segments-personas", "04-journey",
    "05-value-positioning", "06-lean-canvas", "07-monetization", "08-swot",
    "09-north-star", "10-assumptions", "11-experiments", "12-ost",
    "13-gtm-growth", "14-naming", "15-pre-mortem", "16-PRD", "17-stories",
    "18-prioritization", "19-test-scenarios", "20-metrics", "21-roadmap-sprints",
]
DESIGN = [
    "00-review-checklist", "01-architecture", "02-deployment-network",
    "03-domain-model", "04-game-rules-spec", "05-algorithms", "06-protocol",
    "07-database", "08-client-architecture", "09-screens-ux", "10-i18n",
    "11-ads-consent", "12-security", "13-performance", "14-testing-strategy",
    "15-repo-structure", "16-observability-ops", "17-auth-privacy", "18-design-layout",
]


def markdown_files():
    for folder in ("docs", ".agents", ".claude", ".github"):
        yield from (ROOT / folder).rglob("*.md")
    yield from ROOT.glob("*.md")


def validate():
    errors = []
    for folder, names in (("product", PRODUCT), ("design", DESIGN)):
        for name in names:
            if not (ROOT / "docs" / folder / f"{name}.md").is_file():
                errors.append(f"Missing {folder}/{name}.md")
    files = list(markdown_files())
    for file in files:
        content = file.read_text(encoding="utf-8")
        fences = re.findall(r"^\s*(`{3,}|~{3,})", content, flags=re.M)
        if len(fences) % 2:
            errors.append(f"Unclosed code fence: {file.relative_to(ROOT)}")
        if file.parent == ROOT / "docs/product":
            for heading in ("## 결정 사항", "## 열린 질문"):
                if heading not in content:
                    errors.append(f"Missing {heading}: {file.name}")
        if file.parent == ROOT / "docs/design" and file.name != "00-review-checklist.md":
            if "```mermaid" not in content:
                errors.append(f"Missing diagram: {file.name}")
        # Historical input files can refer to their original environment.
        if file.parent == ROOT / "docs/planning":
            continue
        stripped = re.sub(r"```.*?```", "", content, flags=re.S)
        for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", stripped):
            target = target.strip().strip("<>").split("#", 1)[0]
            if not target or urlparse(target).scheme or target.startswith("/"):
                continue
            resolved = (file.parent / unquote(target)).resolve()
            if not resolved.exists():
                errors.append(f"Broken link: {file.relative_to(ROOT)} -> {target}")
    approval = json.loads((ROOT / "docs/workflow/design-approval.json").read_text(encoding="utf-8"))
    if approval["status"] not in ("pending", "approved"):
        errors.append("Invalid design approval status")
    if approval["status"] == "approved":
        for field in ("approved_by", "approved_at", "evidence", "reviewed_commit"):
            if not approval.get(field):
                errors.append(f"Approved design lacks {field}")
        evidence = (ROOT / approval.get("evidence", "")).resolve()
        if not evidence.is_relative_to(ROOT) or not evidence.is_file():
            errors.append("Approval evidence must be an existing repository file")
        if not re.fullmatch(r"[0-9a-f]{40}", approval.get("reviewed_commit", "")):
            errors.append("Approval reviewed_commit must be a full commit SHA")
    scenarios = (ROOT / "docs/product/19-test-scenarios.md").read_text(encoding="utf-8")
    checklist = (ROOT / "docs/design/00-review-checklist.md").read_text(encoding="utf-8")
    for number in range(1, 37):
        scenario = f"TS{number:02d}"
        if f"## {scenario}:" not in scenarios or scenario not in checklist:
            errors.append(f"Scenario lacks definition or trace: {scenario}")
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print(f"Validated {len(files)} Markdown files, {len(PRODUCT)} product documents, {len(DESIGN) - 1} designs, 36 scenario traces.")
    return 0


if __name__ == "__main__":
    sys.exit(validate())
