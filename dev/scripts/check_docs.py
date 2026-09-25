"""Check local Markdown destinations in maintained source, excluding code blocks."""

from pathlib import PurePosixPath
from urllib.parse import unquote, urlsplit

from build_wiki import transform_markdown
from repository import ROOT, source_files


def check(root=ROOT):
    files = set(source_files(root))
    findings = []
    for name in sorted(files):
        if not name.endswith(".md"):
            continue

        def validate(destination, image=False):
            parsed = urlsplit(destination)
            if parsed.scheme or parsed.netloc or not parsed.path:
                return destination
            target = (root / PurePosixPath(name).parent / unquote(parsed.path)).resolve()
            if not target.is_relative_to(root.resolve()):
                findings.append(f"{name}: link escapes repository: {destination}")
            else:
                relative = target.relative_to(root.resolve()).as_posix()
                if relative not in files and not any(p.startswith(relative + "/") for p in files):
                    findings.append(f"{name}: missing source destination: {destination}")
            return destination

        transform_markdown((root / name).read_text(encoding="utf-8"), validate)
    return findings


if __name__ == "__main__":
    errors = check()
    if errors:
        raise SystemExit("\n".join(errors))
    print("Maintained Markdown source destinations passed.")
