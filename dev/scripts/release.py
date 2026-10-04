"""Inspect, prepare and validate product releases without publishing anything."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import tomllib

from repository import ROOT, changed_files, commit, git, product_file


VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\Z")


def manifest_version(manifest):
    version = manifest["package"]["version"]
    if version == {"workspace": True}:
        version = manifest["workspace"]["package"]["version"]
    if not isinstance(version, str) or not VERSION.fullmatch(version):
        raise ValueError("Expected a numeric release version")
    return version


def bump_version(version, kind):
    match = VERSION.fullmatch(version)
    if not match:
        raise ValueError("Releases require a numeric major.minor.patch version")
    major, minor, patch = map(int, match.groups())
    return {"major": f"{major + 1}.0.0", "minor": f"{major}.{minor + 1}.0",
            "patch": f"{major}.{minor}.{patch + 1}"}[kind]


def version_at(ref, root=ROOT):
    revision = commit(ref, root)
    for name in ("app/Cargo.toml", "Cargo.toml"):
        result = subprocess.run(["git", "-C", str(root), "show", f"{revision}:{name}"],
                                capture_output=True, text=True, encoding="utf-8")
        if result.returncode == 0:
            return manifest_version(tomllib.loads(result.stdout))
    raise ValueError("The release base has no APEX Cargo manifest")


def current_version(root=ROOT):
    manifest = tomllib.loads((root / "app/Cargo.toml").read_text(encoding="utf-8"))
    package = manifest["package"]
    if package["name"] != "apex-scheduler":
        raise ValueError("Expected the apex-scheduler package with a numeric release version")
    return manifest_version(manifest)


def product_packages(root=ROOT):
    app = (root / "app").resolve()
    manifest = tomllib.loads((app / "Cargo.toml").read_text(encoding="utf-8"))
    workspace = manifest.get("workspace", {})
    names = []
    for member in [".", *workspace.get("members", [])]:
        path = (app / member / "Cargo.toml").resolve()
        if not path.is_relative_to(app):
            raise ValueError("Workspace members must stay inside app/")
        package = tomllib.loads(path.read_text(encoding="utf-8"))["package"]
        if workspace.get("package", {}).get("version") and package["version"] != {"workspace": True}:
            raise ValueError("Every product package must inherit the workspace version")
        names.append(package["name"])
    return names


def check_lock(root=ROOT):
    version = current_version(root)
    lock = tomllib.loads((root / "app/Cargo.lock").read_text(encoding="utf-8"))
    for name in product_packages(root):
        entries = [p for p in lock["package"] if p["name"] == name and "source" not in p]
        if len(entries) != 1 or entries[0]["version"] != version:
            raise ValueError(f"Cargo.lock must contain the same APEX version for {name} as app/Cargo.toml")
    return version


def inspect(base, root=ROOT):
    paths = [p for p in changed_files(base, root=root) if p]
    old, new = version_at(base, root), check_lock(root)
    return {"base": commit(base, root), "previous_version": old, "version": new,
            "product_changed": any(product_file(p) for p in paths),
            "version_changed": new != old, "changed_files": paths}


def validate(base, target, root=ROOT):
    result = inspect(base, root)
    if target == "main":
        old, new = result["previous_version"], result["version"]
        if result["product_changed"] and not result["version_changed"]:
            raise ValueError("Product changes to main require a prepared release with a version bump")
        if result["version_changed"]:
            if new not in {bump_version(old, kind) for kind in ("patch", "minor", "major")}:
                raise ValueError("The release must increment patch, minor or major exactly once")
            notes = root / f"dev/releases/{new}.md"
            if not notes.is_file() or not notes.read_text(encoding="utf-8").strip():
                raise ValueError(f"Missing release notes: dev/releases/{new}.md")
    return result


def replace_version(text, section, version):
    pattern = re.compile(r"(" + section + r"\s*\n)(.*?)(?=\n\[|\Z)", re.S)
    blocks = list(pattern.finditer(text))
    if len(blocks) != 1:
        raise ValueError("Expected one package section to update")
    block = blocks[0]
    updated, count = re.subn(r'(?m)^version\s*=\s*"[^"]+"', f'version = "{version}"', block[2])
    if count != 1:
        raise ValueError("Expected one package version to update")
    return text[:block.start(2)] + updated + text[block.end(2):]


def prepare(base, kind, notes_path, root=ROOT):
    # The agent creates an isolated release branch and supplies reviewed notes first.
    branch = git("branch", "--show-current", root=root)
    if not branch.startswith(("codex/release-", "codex/hotfix-")):
        raise ValueError("Prepare on a codex/release-* or codex/hotfix-* branch")
    if git("status", "--porcelain", "--untracked-files=no", root=root):
        raise ValueError("Commit the reviewed candidate before preparing its release")
    old = version_at(base, root)
    if check_lock(root) != old:
        raise ValueError("Candidate version differs from main; reconcile before preparing a release")
    version = bump_version(old, kind)
    notes = notes_path.read_text(encoding="utf-8").strip()
    if not notes:
        raise ValueError("Provide completed release notes before preparation")
    destination = root / f"dev/releases/{version}.md"
    if destination.exists():
        raise ValueError("Release notes already exist; inspect the existing release instead of overwriting")
    manifest = root / "app/Cargo.toml"
    lock = root / "app/Cargo.lock"
    manifest_text = manifest.read_text(encoding="utf-8")
    inherited = tomllib.loads(manifest_text)["package"]["version"] == {"workspace": True}
    manifest_text = replace_version(manifest_text, r"\[workspace.package\]" if inherited else r"\[package\]", version)
    lock_text = lock.read_text(encoding="utf-8")
    for name in product_packages(root):
        lock_text = replace_version(lock_text, r'\[\[package\]\]\s*\nname = "' + re.escape(name) + '"', version)
    # Validate both replacements before writing either file. Dependency versions stay fixed.
    tomllib.loads(manifest_text)
    tomllib.loads(lock_text)
    manifest.write_text(manifest_text, encoding="utf-8", newline="\n")
    lock.write_text(lock_text, encoding="utf-8", newline="\n")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(f"# APEX {version}\n\n{notes}\n", encoding="utf-8", newline="\n")
    check_lock(root)
    return {"version": version, "notes": destination.relative_to(root).as_posix()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("inspect", "check", "prepare"))
    parser.add_argument("--base", default="origin/main")
    parser.add_argument("--target", choices=("main", "dev"), default="dev")
    parser.add_argument("--bump", choices=("patch", "minor", "major"))
    parser.add_argument("--notes", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "prepare":
            if not args.bump or not args.notes:
                parser.error("prepare requires --bump and --notes")
            result = prepare(args.base, args.bump, args.notes)
        elif args.command == "check":
            result = validate(args.base, args.target)
        else:
            result = inspect(args.base)
        print(json.dumps(result, indent=2))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release check failed: {error}\n")


if __name__ == "__main__":
    main()
