#!/usr/bin/env python3
"""Offline repository documentation checks using only Python's standard library.

Checks inline local Markdown links/heading anchors and delivery task integrity.
It does not fetch external links or establish semantic/implementation correctness.
"""

from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
STATUSES = {
    "NOT_STARTED", "READY", "IN_PROGRESS", "BLOCKED", "IMPLEMENTED",
    "VERIFIED", "COMPLETED",
}
TASK = re.compile(r"\bP[1-6]-\d{2}(?:\.\d{2})?\b")
REQUIRED = (
    "PROJECT_CHARTER", "DOCUMENT_AUTHORITY", "ARCHITECTURE_DECISIONS",
    "DELIVERY_PLAN", "P1_STRATEGY_DESIGN", "PROJECT_STATUS", "NEXT_SESSION",
    "WORKFLOW", "BASELINE_AUDIT",
)


def prose(text):
    """Ignore fenced examples when interpreting links, headings and tables."""
    return re.sub(r"^```[^\n]*\n.*?^```[^\n]*$", "", text, flags=re.M | re.S)


def anchors(text):
    found, counts = set(), {}
    for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", prose(text), re.M):
        slug = re.sub(r"[^\w\- ]", "", heading.lower()).replace(" ", "-")
        count = counts.get(slug, 0)
        counts[slug] = count + 1
        found.add(f"{slug}-{count}" if count else slug)
    return found


def check_links(root, documents, errors):
    count = 0
    for path in documents:
        for target in re.findall(r"!?\[[^\]\n]+\]\(([^)\n]+)\)", prose(path.read_text())):
            target = target.strip().strip("<>")
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc:
                continue
            count += 1
            dest = (path.parent / unquote(parsed.path)).resolve() if parsed.path else path
            label = path.relative_to(root)
            if not dest.is_relative_to(root):
                errors.append(f"{label}: link escapes repository: {target}")
            elif not dest.exists():
                errors.append(f"{label}: missing link target: {target}")
            elif parsed.fragment and dest.suffix == ".md":
                if unquote(parsed.fragment) not in anchors(dest.read_text()):
                    errors.append(f"{label}: missing heading anchor: {target}")
    return count


def check_tasks(plan, ledger, errors):
    tasks = {}
    for line in prose(plan).splitlines():
        cells = [part.strip() for part in line.strip().strip("|").split("|")]
        if not cells or not TASK.fullmatch(cells[0]):
            continue
        task = cells[0]
        if task in tasks:
            errors.append(f"duplicate plan task: {task}")
        if len(cells) != 7 or any(not cell for cell in cells):
            errors.append(f"{task}: expected seven nonempty task columns")
            continue
        deps = TASK.findall(cells[2])
        status = cells[6]
        if status not in STATUSES:
            errors.append(f"{task}: invalid status {status}")
        tasks[task] = (deps, status)

    for phase in range(1, 7):
        parents = sorted(task for task in tasks if task.startswith(f"P{phase}-") and "." not in task)
        expected = [f"P{phase}-{n:02}" for n in range(1, len(parents) + 1)]
        if not parents or parents != expected:
            errors.append(f"P{phase}: missing or non-contiguous parent task index")
    for task in tasks:
        if "." in task and task.split(".")[0] not in tasks:
            errors.append(f"{task}: missing parent task")
    for task, (deps, status) in tasks.items():
        for dep in deps:
            if dep not in tasks:
                errors.append(f"{task}: unknown dependency {dep}")
            elif status in {"READY", "IN_PROGRESS", "IMPLEMENTED", "VERIFIED", "COMPLETED"}:
                if tasks[dep][1] != "COMPLETED":
                    errors.append(f"{task}: active/closed before dependency {dep} is COMPLETED")

    visited, visiting = set(), set()

    def visit(task):
        if task in visiting:
            errors.append(f"dependency cycle at {task}")
            return
        if task in visited or task not in tasks:
            return
        visiting.add(task)
        for dep in tasks[task][0]:
            visit(dep)
        visiting.remove(task)
        visited.add(task)

    for task in tasks:
        visit(task)

    recorded = {}
    for line in prose(ledger).splitlines():
        cells = [part.strip() for part in line.strip().strip("|").split("|")]
        if len(cells) < 2 or cells[1] not in STATUSES:
            continue
        for task in TASK.findall(cells[0]):
            if task in recorded:
                errors.append(f"duplicate status ledger entry: {task}")
            recorded[task] = cells[1]
    for task, (_, status) in tasks.items():
        if recorded.get(task) != status:
            errors.append(f"{task}: plan={status}, ledger={recorded.get(task, 'MISSING')}")
    for task in recorded.keys() - tasks.keys():
        errors.append(f"unknown task in status ledger: {task}")
    gov = re.search(r"GOV-01:.*?Status: (\w+)", plan, re.S)
    gov_ledger = re.search(r"\| GOV-01 \| (\w+) \|", ledger)
    if not gov or not gov_ledger or gov[1] != gov_ledger[1]:
        errors.append("GOV-01 plan/status mismatch or missing status")
    return len(tasks)


def main():
    errors = []
    for name in REQUIRED:
        if not (ROOT / "docs" / f"{name}.md").is_file():
            errors.append(f"required governance document missing: {name}")
    documents = sorted(ROOT.glob("*.md")) + sorted((ROOT / "docs").rglob("*.md"))
    links = check_links(ROOT, documents, errors)
    plan, ledger = ROOT / "docs/DELIVERY_PLAN.md", ROOT / "docs/PROJECT_STATUS.md"
    tasks = check_tasks(plan.read_text(), ledger.read_text(), errors) if plan.exists() and ledger.exists() else 0
    if errors:
        print("Documentation checks FAILED:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print(f"PASS: {len(documents)} Markdown files, {links} local links/anchors, "
          f"{tasks} stage tasks; dependencies and status projections consistent.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
