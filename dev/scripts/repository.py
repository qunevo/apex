"""Small Git primitives shared by contributor tools; no runtime dependency."""

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]


def git(*args, root=ROOT):
    return subprocess.check_output(
        ["git", "-c", "core.safecrlf=false", "-C", str(root), *args],
        text=True, encoding="utf-8", stderr=subprocess.PIPE
    ).strip()


def commit(ref, root=ROOT):
    return git("rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}", root=root)


def source_files(root=ROOT):
    names = git("ls-files", "--cached", "--others", "--exclude-standard", "-z", root=root)
    return sorted({name for name in names.split("\0") if name and (root / name).is_file()})


def changed_files(base, head="HEAD", root=ROOT):
    return git("diff", "--name-only", "--no-renames", "-z", commit(base, root),
               commit(head, root), "--", root=root).split("\0")


def docs_only(path):
    return (path.endswith(".md") or path.startswith(("dev/skills/", ".agents/skills/"))
            or path.startswith(".github/ISSUE_TEMPLATE/"))


def product_file(path):
    """Release impact is separate from whether a change needs runtime tests."""
    if not path.startswith("app/"):
        return path.startswith(("src/rust/", "web/", "schemas/", "customizations/")) or path in {
            "Cargo.toml", "Cargo.lock", "LICENSE", "LICENSING.md"
        }
    relative = path[4:]
    return not (relative.startswith(("tests/", "docs/")) or relative in {
        "README.md", "AGENTS.md", "package.json", "package-lock.json",
        ".editorconfig", ".gitattributes", ".gitignore"
    })
