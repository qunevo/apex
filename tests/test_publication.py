"""Publication checks for frozen benchmark packages and their source archives."""

import hashlib
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from scripts import check_publication as publication


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.archive = self.root / publication.SOURCE_ARCHIVE
        self.archive.parent.mkdir()

    def declare(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        manifest = {"source_archive": {"path": self.archive.name, "sha256": digest},
                    "file_hashes": {self.archive.name: digest}}
        (self.archive.parent / "publication_manifest.json").write_text(json.dumps(manifest))

    def bundle(self, name="snapshot/src/rust/lib.rs", content="pub fn example() {}"):
        if not isinstance(name, zipfile.ZipInfo):
            member = zipfile.ZipInfo("placeholder")
            member.filename = name
        else:
            member = name
        with zipfile.ZipFile(self.archive, "w") as archive:
            archive.writestr(member, content)
        self.declare()

    def codes(self):
        return {finding["code"] for finding in publication.scan(self.root)}

    def test_declared_source_is_accepted_and_unrelated_zip_is_not(self):
        self.bundle()
        self.assertEqual(self.codes(), set())
        (self.root / "other.zip").write_bytes(self.archive.read_bytes())
        self.assertEqual(self.codes(), {"REMOVE_OR_REVIEW_BINARY_ARTIFACT"})

    def test_missing_manifest_and_changed_bytes_fail(self):
        self.bundle()
        self.archive.write_bytes(self.archive.read_bytes() + b"changed")
        self.assertEqual(self.codes(), {"SOURCE_ARCHIVE_INTEGRITY"})
        (self.archive.parent / "publication_manifest.json").unlink()
        self.assertEqual(self.codes(), {"SOURCE_ARCHIVE_INTEGRITY"})

    def test_invalid_zip_fails_even_when_its_hash_matches(self):
        self.archive.write_bytes(b"invalid ZIP")
        self.declare()
        self.assertEqual(self.codes(), {"SOURCE_ARCHIVE_INTEGRITY"})

    def test_unsafe_members_and_local_artifacts_fail_without_extraction(self):
        for name in ("../escape.py", "snapshot/../escape.py", "/snapshot/a.py",
                     "snapshot/C:/a.py", "snapshot\\a.py", "snapshot/./a.py",
                     "snapshot/.env", "snapshot/.codex/config.toml",
                     "snapshot/target/result.txt", "snapshot/cache.pyc",
                     "snapshot/TARGET/result.txt", "snapshot/session.lock",
                     "snapshot/customizations/private/policy.rs",
                     "snapshot/private.pem", "unlisted/a.py"):
            with self.subTest(name=name):
                self.bundle(name, "example")
                self.assertEqual(self.codes(), {"SOURCE_ARCHIVE_MEMBER"})
        self.assertFalse((self.root / "escape.py").exists())
        self.assertFalse((self.root / "snapshot").exists())

    def test_symlink_is_rejected(self):
        member = zipfile.ZipInfo("snapshot/link.py")
        member.create_system = 3
        member.external_attr = (stat.S_IFLNK | 0o777) << 16
        self.bundle(member, "../../outside")
        self.assertEqual(self.codes(), {"SOURCE_ARCHIVE_MEMBER"})

    def test_secrets_and_workstation_paths_are_scanned_inside_rust_source(self):
        self.bundle(content="ghp_" + "z" * 30)
        self.assertEqual(self.codes(), {"POSSIBLE_INLINE_SECRET"})
        self.bundle(content="/home/" + "example/project")
        self.assertEqual(self.codes(), {"WORKSTATION_PATH"})

    def test_archive_size_limit_is_enforced(self):
        self.bundle(content="x" * 32)
        with patch.object(publication, "MAX_ARCHIVE_BYTES", 16):
            self.assertEqual(self.codes(), {"SOURCE_ARCHIVE_LIMIT"})


class BenchmarkPackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.archive = self.root / publication.BENCHMARK_ARCHIVE
        self.archive.parent.mkdir()
        with zipfile.ZipFile(self.archive, "w") as bundle:
            bundle.writestr("apex-benchmark-2026-09-24/README.md", "Synthetic publication fixture")
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.digest_patch = patch.object(publication, "BENCHMARK_ARCHIVE_SHA256", digest)
        self.digest_patch.start()
        self.addCleanup(self.digest_patch.stop)

    def codes(self):
        return {finding["code"] for finding in publication.scan(self.root)}

    def test_reviewed_package_is_accepted_without_extracting(self):
        self.assertEqual(self.codes(), set())
        self.assertFalse((self.archive.parent / "apex-benchmark-2026-09-24").exists())
        (self.root / "other.zip").write_bytes(self.archive.read_bytes())
        self.assertEqual(self.codes(), {"REMOVE_OR_REVIEW_BINARY_ARTIFACT"})

    def test_changed_payload_and_appended_data_require_review(self):
        original = self.archive.read_bytes()
        for payload in (original + b"extra", b"invalid ZIP"):
            with self.subTest(payload_size=len(payload)):
                self.archive.write_bytes(payload)
                self.assertEqual(self.codes(), {"BENCHMARK_ARCHIVE_INTEGRITY"})
        with zipfile.ZipFile(self.archive, "w") as bundle:
            bundle.writestr("apex-benchmark-2026-09-24/.env", "synthetic-local-setting")
        self.assertEqual(self.codes(), {"BENCHMARK_ARCHIVE_INTEGRITY"})

    def test_package_above_distribution_limit_is_rejected(self):
        with patch.object(publication, "MAX_BENCHMARK_BYTES", 16):
            self.assertEqual(self.codes(), {"BENCHMARK_ARCHIVE_LIMIT"})


if __name__ == "__main__":
    unittest.main()
