from pathlib import Path
import zipfile

import pytest

import publication
from publication import safe_path


def test_manifest_paths_normalize_windows_separators():
    root = Path.cwd()
    assert safe_path(root, "snapshot\\benchmark\\Cargo.toml") == root / "snapshot/benchmark/Cargo.toml"


def test_manifest_path_cannot_escape_source_root():
    with pytest.raises(AssertionError, match="escapes"):
        safe_path(Path.cwd(), "../outside.json")


def test_archived_sources_extract_with_original_bytes(tmp_path):
    output = tmp_path / "snapshot"
    publication.extract_snapshot(output)
    manifest = publication.read(publication.RUN / "manifest.json")
    for relative, expected in manifest["snapshot_hashes"].items():
        relative = relative.replace("\\", "/")
        if relative.startswith("snapshot/") and relative != publication.OMITTED_BINARY:
            assert publication.digest(output / relative.removeprefix("snapshot/")) == expected
    for relative, entry in publication.archive_assets()["files"].items():
        assert publication.digest(output / relative) == entry["sha256"]


def test_source_extraction_rejects_repository_destination():
    with pytest.raises(AssertionError, match="outside"):
        publication.extract_snapshot(publication.ROOT / "unpacked-sources")


def test_changed_archived_algorithm_is_rejected(tmp_path, monkeypatch):
    altered = tmp_path / "changed.zip"
    with zipfile.ZipFile(publication.SOURCE_ARCHIVE) as original, zipfile.ZipFile(altered, "w") as target:
        for name in original.namelist():
            data = original.read(name)
            if name == "snapshot/src/rust/engine.rs":
                data += b"\n// Changed source\n"
            target.writestr(name, data)
    monkeypatch.setattr(publication, "SOURCE_ARCHIVE", altered)
    with pytest.raises(AssertionError, match="engine.rs"):
        publication.check_sources()
