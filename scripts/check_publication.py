"""Detect common publication blockers without printing matching secret values.

This is a conservative file/content check, not proof that all data is synthetic.
Git history requires a separate audit.
"""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat
import zipfile


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
    re.compile(r"gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}"),
]
WORKSTATION_PATH = re.compile(r"[A-Za-z]:[\\/](?:Users|repo_priv)[\\/]|/home/[A-Za-z0-9_.-]+/", re.I)
SOURCE_ARCHIVE = "benchmark/algorithm-9bc9dbf.zip"
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
SOURCE_SUFFIXES = {".rs", ".py", ".txt", ".md", ".html", ".json", ".toml", ".lock"}
BENCHMARK_ARCHIVE = "benchmark/apex-benchmark-2026-09-24.zip"
# The expanded publication was audited before packaging. Any byte change needs
# a new content review and digest; this is not a general allowance for ZIPs.
BENCHMARK_ARCHIVE_SHA256 = "a7299c71ac7e523715ae179d0c4078ff6e30cb0f36b3e317b1ae3d4bae27b8d1"
MAX_BENCHMARK_BYTES = 100 * 1024 * 1024


def text_codes(content: str, suffix: str) -> set[str]:
    codes = set()
    if any(pattern.search(content) for pattern in SECRET_PATTERNS):
        codes.add("POSSIBLE_INLINE_SECRET")
    if WORKSTATION_PATH.search(content):
        codes.add("WORKSTATION_PATH")
    if suffix == ".ipynb":
        try:
            notebook = json.loads(content)
        except ValueError:
            codes.add("INVALID_NOTEBOOK")
        else:
            if any(cell.get("outputs") or cell.get("execution_count") is not None
                   for cell in notebook.get("cells", [])):
                codes.add("SAVED_NOTEBOOK_OUTPUT")
    return codes


def source_archive_codes(root: Path, path: Path) -> set[str]:
    """Inspect the declared frozen source without extracting or executing it."""
    try:
        if path.stat().st_size > MAX_ARCHIVE_BYTES:
            return {"SOURCE_ARCHIVE_LIMIT"}
        manifest = json.loads((root / "benchmark/publication_manifest.json").read_text(encoding="utf-8"))
        declared = manifest["source_archive"]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if (declared["path"] != path.name or declared["sha256"] != digest
                or manifest["file_hashes"][path.name] != digest):
            return {"SOURCE_ARCHIVE_INTEGRITY"}
        codes = set()
        with zipfile.ZipFile(path) as archive:
            members = archive.infolist()
            if (not members or len(members) > 2000
                    or sum(member.file_size for member in members) > MAX_ARCHIVE_BYTES):
                return {"SOURCE_ARCHIVE_LIMIT"}
            seen = set()
            for member in members:
                name = member.orig_filename
                relative = PurePosixPath(name)
                if (name in seen or not relative.parts or name != relative.as_posix()
                        or relative.is_absolute() or "\\" in name or ":" in name
                        or ".." in relative.parts
                        or relative.parts[0] not in {"snapshot", "reproduction-assets"}
                        or stat.S_ISLNK(member.external_attr >> 16)
                        or member.flag_bits & 1
                        or relative.suffix not in SOURCE_SUFFIXES
                        or relative.suffix == ".lock" and relative.name != "Cargo.lock"
                        or any(part.startswith(".") for part in relative.parts)
                        or any(part.lower() in {"target", "node_modules", "__pycache__", "venv", "env"}
                               for part in relative.parts)
                        or any(part == "customizations" and relative.parts[index + 1] != "dummy_customer"
                               for index, part in enumerate(relative.parts[:-1]))):
                    codes.add("SOURCE_ARCHIVE_MEMBER")
                    continue
                seen.add(name)
                try:
                    content = archive.read(member).decode("utf-8-sig")
                except UnicodeError:
                    codes.add("NON_UTF8_TEXT")
                else:
                    codes.update(text_codes(content, relative.suffix))
        return codes
    except (OSError, ValueError, KeyError, TypeError, zipfile.BadZipFile, RuntimeError):
        return {"SOURCE_ARCHIVE_INTEGRITY"}


def benchmark_archive_codes(path: Path) -> set[str]:
    """Accept only the exact reviewed reproduction package, without extraction."""
    try:
        if path.stat().st_size > MAX_BENCHMARK_BYTES:
            return {"BENCHMARK_ARCHIVE_LIMIT"}
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        return set() if digest == BENCHMARK_ARCHIVE_SHA256 else {"BENCHMARK_ARCHIVE_INTEGRITY"}
    except OSError:
        return {"BENCHMARK_ARCHIVE_INTEGRITY"}


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
            if relative.as_posix() == BENCHMARK_ARCHIVE:
                codes.update(benchmark_archive_codes(path))
            elif relative.as_posix() == SOURCE_ARCHIVE:
                codes.update(source_archive_codes(root, path))
            else:
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
                codes.update(text_codes(content, path.suffix))
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
