"""Validate an isolated source snapshot without staging or exporting local output."""

from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from repository import ROOT, commit, source_files


def main():
    revision = commit("HEAD")
    with tempfile.TemporaryDirectory(prefix="apex source review ") as directory:
        snapshot = Path(directory).resolve()
        if not snapshot.is_relative_to(Path(tempfile.gettempdir()).resolve()) or snapshot.is_relative_to(ROOT):
            raise ValueError("Expected a separate system temporary directory")
        files = source_files()
        for name in files:
            source, destination = ROOT / name, snapshot / name
            if source.is_symlink() or not source.resolve().is_relative_to(ROOT):
                raise ValueError(f"Source export does not follow external links: {name}")
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
        subprocess.run(["git", "init", "-q", str(snapshot)], check=True)
        subprocess.run(["git", "-c", "core.safecrlf=false", "-C", str(snapshot), "add", "."], check=True)
        commands = [
            [sys.executable, "-B", "dev/scripts/check_docs.py"],
            [sys.executable, "-B", "dev/scripts/sync_skills.py", "--check"],
            [sys.executable, "-B", "dev/scripts/build_wiki.py", "--revision", revision],
            [sys.executable, "-B", "dev/scripts/check_publication.py"],
        ]
        for command in commands:
            subprocess.run(command, cwd=snapshot, check=True)
        print(f"Source snapshot checks passed: {len(files)} files; original Git index unchanged.")


if __name__ == "__main__":
    main()
