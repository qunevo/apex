"""List large maintained source files for a focused refactoring discussion."""

import argparse
import json
from pathlib import Path

from repository import ROOT, source_files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--limit", type=int, default=15)
    args = parser.parse_args()
    findings = []
    for name in source_files():
        path = Path(name)
        if name.startswith(("benchmark/", "app/schemas/", "dev/releases/")):
            continue
        if path.suffix not in (".rs", ".py", ".mjs", ".html", ".md"):
            continue
        lines = len((ROOT / name).read_text(encoding="utf-8").splitlines())
        threshold = 200 if path.suffix == ".md" else 500
        if lines >= threshold:
            findings.append({"path": name, "lines": lines, "review_hint": "Inspect responsibility boundaries and callers"})
    findings.sort(key=lambda item: (-item["lines"], item["path"]))
    print(json.dumps({"candidates": findings[:max(1, args.limit)], "total": len(findings),
                      "note": "Heuristic inventory only; no automatic split or deletion."}, indent=2))


if __name__ == "__main__":
    main()
