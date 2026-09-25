"""Exercise release preparation and compatibility gates in synthetic Git repos."""

from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest


SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
import release
from ci_plan import plan
from repository import product_file


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="apex release test ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.name", "Synthetic Test")
        self.git("config", "user.email", "test@example.invalid")
        self.put("app/Cargo.toml", '[package]\nname = "apex-scheduler"\nversion = "0.6.0"\n\n[dependencies]\nexample = "1"\n')
        self.put("app/Cargo.lock", 'version = 4\n\n[[package]]\nname = "apex-scheduler"\nversion = "0.6.0"\n\n[[package]]\nname = "example"\nversion = "1.2.3"\n')
        self.put("app/src/lib.rs", "pub fn value() -> u8 { 1 }\n")
        self.save()
        self.base = self.git("rev-parse", "HEAD")

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.root), *args],
                                       text=True, stderr=subprocess.PIPE).strip()

    def put(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def save(self):
        self.git("add", ".")
        self.git("commit", "-qm", "Synthetic change")

    def candidate(self):
        self.git("switch", "-qc", "codex/release-0.6.1")
        self.put("app/src/lib.rs", "pub fn value() -> u8 { 2 }\n")
        self.save()

    def test_product_change_can_enter_dev_but_not_main_without_release(self):
        self.candidate()
        self.assertTrue(release.validate(self.base, "dev", self.root)["product_changed"])
        with self.assertRaisesRegex(ValueError, "prepared release"):
            release.validate(self.base, "main", self.root)

    def test_prepare_keeps_dependency_versions_and_passes_after_commit(self):
        self.candidate()
        notes = self.root / "notes.txt"
        notes.write_text("Correct the synthetic return value. No input migration.\n")
        result = release.prepare(self.base, "patch", notes, self.root)
        self.assertEqual(result["version"], "0.6.1")
        lock = tomllib.loads((self.root / "app/Cargo.lock").read_text())
        self.assertEqual(lock["package"][1]["version"], "1.2.3")
        self.save()
        self.assertTrue(release.validate(self.base, "main", self.root)["version_changed"])
        with self.assertRaisesRegex(ValueError, "differs from main"):
            release.prepare(self.base, "patch", notes, self.root)

    def test_prepare_rejects_main_and_dirty_candidates(self):
        notes = self.root / "notes.txt"
        notes.write_text("Synthetic release notes.")
        with self.assertRaisesRegex(ValueError, "branch"):
            release.prepare(self.base, "patch", notes, self.root)
        self.candidate()
        self.put("app/src/lib.rs", "uncommitted change")
        with self.assertRaisesRegex(ValueError, "Commit the reviewed"):
            release.prepare(self.base, "patch", notes, self.root)

    def test_lock_mismatch_rejected_on_both_branches(self):
        path = self.root / "app/Cargo.lock"
        path.write_text(path.read_text().replace('"0.6.0"', '"0.6.1"'))
        for target in ("dev", "main"):
            with self.subTest(target=target), self.assertRaisesRegex(ValueError, "Cargo.lock"):
                release.validate(self.base, target, self.root)

    def test_maintenance_does_not_require_a_product_release(self):
        self.put("dev/docs/example.md", "# Synthetic guide\n")
        self.save()
        result = release.validate(self.base, "main", self.root)
        self.assertFalse(result["product_changed"])
        self.assertFalse(result["version_changed"])

    def test_missing_notes_and_version_jumps_are_rejected(self):
        self.candidate()
        for version, message in (("0.6.1", "Missing release notes"), ("0.6.8", "exactly once")):
            for filename in ("Cargo.toml", "Cargo.lock"):
                path = self.root / "app" / filename
                path.write_text(path.read_text().replace('"0.6.0"', f'"{version}"').replace('"0.6.1"', f'"{version}"'))
            with self.subTest(version=version), self.assertRaisesRegex(ValueError, message):
                release.validate(self.base, "main", self.root)

    def test_previous_root_layout_is_a_supported_release_base(self):
        self.git("mv", "app/Cargo.toml", "Cargo.toml")
        self.save()
        self.assertEqual(release.version_at("HEAD", self.root), "0.6.0")

    def test_bump_rules_and_invalid_versions(self):
        self.assertEqual(release.bump_version("1.2.3", "patch"), "1.2.4")
        self.assertEqual(release.bump_version("1.2.3", "minor"), "1.3.0")
        self.assertEqual(release.bump_version("1.2.3", "major"), "2.0.0")
        for value in ("1.2", "01.2.3", "1.2.3-rc.1"):
            with self.assertRaises(ValueError):
                release.bump_version(value, "patch")


class ScopeTests(unittest.TestCase):
    def test_prose_can_skip_runtime_but_unknown_and_removed_source_cannot(self):
        self.assertFalse(plan(["app/docs/data-model.md", "dev/skills/example/SKILL.md"])["runtime"])
        for paths in (["app/src/removed.rs"], ["dev/scripts/release.py"], ["unknown.file"], []):
            self.assertTrue(plan(paths)["runtime"])
        self.assertTrue(plan(["readme.md"], force=True)["runtime"])

    def test_product_scope_distinguishes_customer_workflows_from_tooling(self):
        for path in ("app/src/xg.rs", "app/Cargo.lock", "app/skills/example/SKILL.md", "app/LICENSE", "LICENSE"):
            self.assertTrue(product_file(path), path)
        for path in ("dev/scripts/release.py", ".github/workflows/ci.yml", "app/tests/example.rs", "app/docs/data-model.md", "app/package-lock.json"):
            self.assertFalse(product_file(path), path)


if __name__ == "__main__":
    unittest.main()
