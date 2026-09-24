"""Detect common publication blockers without printing matching secret values.

This is a conservative file/content check, not proof that all data is synthetic.
Git history requires a separate audit.
"""

import argparse
import json
from pathlib import Path
import re


ARTIFACT_SUFFIXES = {
    ".pyc", ".pyd", ".pdb", ".obj", ".exp", ".lib", ".prof", ".onnx",
    ".faiss", ".pkl", ".pickle", ".pt", ".pth", ".zip", ".pptx",
}
TEXT_SUFFIXES = {
    ".py", ".pyx", ".pxd", ".md", ".txt", ".txt_", ".json", ".ipynb",
    ".toml", ".yaml", ".yml", ".ps1", ".sh", ".bru", ".html", ".svg", ".mmd",
}
SECRET_PATTERNS = [
    re.compile(r"sk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,}"),
    re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    re.compile(r'"(?:apiKey|api_key|access_token|password)"\s*:\s*"[^"<>\s]{8,}"', re.I),
]
WORKSTATION_PATH = re.compile(r"[A-Za-z]:[\\/](?:Users|repo_priv)[\\/]|/home/[A-Za-z0-9_.-]+/", re.I)


def scan(root: Path) -> list[dict[str, str]]:
    """Return finding codes and relative paths, never matching content."""
    findings = []
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if ".git" in relative.parts:
            continue
        if path.is_symlink():
            findings.append({"code": "REVIEW_SYMLINK", "path": relative.as_posix()})
            continue
        if not path.is_file():
            continue
        codes = set()
        if path.suffix.lower() in ARTIFACT_SUFFIXES:
            codes.add("REMOVE_OR_REVIEW_BINARY_ARTIFACT")
        if path.name == ".env" or (path.name.startswith(".env.") and path.name != ".env.example"):
            codes.add("LOCAL_ENV_FILE")
        if relative.parts[:1] == ("customizations",) and len(relative.parts) > 2:
            if relative.parts[1] != "dummy_customer":
                codes.add("NON_DEMO_CUSTOMIZATION")
        if path.suffix in TEXT_SUFFIXES or path.name in {".env-example", ".env.example", "Dockerfile"}:
            try:
                content = path.read_text(encoding="utf-8-sig")
            except UnicodeError:
                codes.add("NON_UTF8_TEXT")
            else:
                if any(pattern.search(content) for pattern in SECRET_PATTERNS):
                    codes.add("POSSIBLE_INLINE_SECRET")
                if WORKSTATION_PATH.search(content):
                    codes.add("WORKSTATION_PATH")
                if path.suffix == ".ipynb":
                    try:
                        notebook = json.loads(content)
                    except ValueError:
                        codes.add("INVALID_NOTEBOOK")
                    else:
                        if any(cell.get("outputs") or cell.get("execution_count") is not None
                               for cell in notebook.get("cells", [])):
                            codes.add("SAVED_NOTEBOOK_OUTPUT")
        findings.extend({"code": code, "path": relative.as_posix()} for code in sorted(codes))
    return findings


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", nargs="?", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--json", action="store_true", help="Print machine-readable findings")
    args = parser.parse_args()
    if not args.root.is_dir():
        parser.error("The source directory does not exist.")
    findings = scan(args.root.resolve())
    if args.json:
        print(json.dumps(findings, indent=2))
    else:
        for finding in findings:
            print(f"{finding['code']}: {finding['path']}")
        print(f"{len(findings)} publication findings. History and semantic data review are separate.")
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
