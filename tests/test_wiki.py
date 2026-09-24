"""Publication regression tests with deliberately synthetic temporary repositories."""

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("build_wiki", Path(__file__).resolve().parents[1] / "scripts/build_wiki.py")
wiki = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wiki)
REVISION = "a" * 40


class WikiTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "source"
        self.root.mkdir()
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.groups = {"Guides": {"docs/README.md": "Home", "docs/guide.md": "Guide"}}
        self.put("docs/README.md", "# APEX documentation\n\n[Guide](guide.md#setup)\n")
        self.put("docs/guide.md", "# Example guide\n\n## Setup\n")
        self.put("examples/input.json", "{}\n")
        self.put("LICENSE", "Synthetic test license\n")
        self.manifest()

    def put(self, path, content, tracked=True):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")
        if tracked:
            subprocess.run(["git", "-C", str(self.root), "add", "--", path], check=True, capture_output=True)

    def manifest(self):
        self.put("scripts/wiki-pages.json", json.dumps(self.groups))

    def build(self):
        return wiki.build(self.root, "example/synthetic", REVISION)

    def test_page_navigation_sources_and_only_allowlisted_pages(self):
        self.put("docs/not-public.md", "# Not selected\n")
        pages = self.build()
        self.assertEqual(set(pages), {"Home.md", "Guide.md", "_Sidebar.md", "_Footer.md"})
        self.assertIn("/wiki/Guide#setup", pages["Home.md"])
        self.assertIn("[Example guide]", pages["_Sidebar.md"])
        self.assertIn(f"/blob/{REVISION}/docs/guide.md", pages["Guide.md"])
        self.assertIn("/edit/main/docs/guide.md", pages["Guide.md"])

    def test_code_labels_titles_encoded_paths_images_and_reference_links(self):
        self.put("examples/with space.json", "{}")
        self.put("docs/image.svg", "<svg />")
        self.put("docs/README.md", '# Home\n\n[`input`](../examples/input.json "Input")\n[spaced](<../examples/with%20space.json>)\n![image](image.svg)\n[guide][reference]\n\n[reference]: guide.md#setup "Guide"\n[dir](../examples)\n')
        result = self.build()["Home.md"]
        self.assertIn(f'[`input`](https://github.com/example/synthetic/blob/{REVISION}/examples/input.json "Input")', result)
        self.assertIn("with%20space.json>", result)
        self.assertIn(f"https://raw.githubusercontent.com/example/synthetic/{REVISION}/docs/image.svg", result)
        self.assertIn('[reference]: https://github.com/example/synthetic/wiki/Guide#setup "Guide"', result)
        self.assertIn(f"/tree/{REVISION}/examples", result)

    def test_examples_fragments_and_external_links_unchanged(self):
        examples = '```md\n[absent](missing.md)\n```\n~~~md\n[absent](missing.md)\n~~~\n`[absent](missing.md)`\n[section](#section)\n[external](https://example.com/a_(b))\n'
        self.put("docs/README.md", "# Home\n" + examples + "[guide](guide.md)\n")
        result = self.build()["Home.md"]
        self.assertIn(examples, result)
        self.assertIn("/wiki/Guide", result)

    def test_missing_untracked_and_escaping_links_fail(self):
        self.put("docs/local.md", "# Local\n", tracked=False)
        for destination in ["missing.md", "local.md", "../../outside.md", "/etc/passwd", "javascript:alert(1)"]:
            with self.subTest(destination=destination):
                self.put("docs/README.md", f"# Home\n[x]({destination})\n")
                with self.assertRaises(ValueError):
                    self.build()

    def test_untracked_source_and_duplicate_slug_fail(self):
        self.put("docs/private.md", "# Private\n", tracked=False)
        self.groups["Guides"]["docs/private.md"] = "Private"
        self.manifest()
        with self.assertRaisesRegex(ValueError, "tracked Markdown"):
            self.build()
        del self.groups["Guides"]["docs/private.md"]
        self.groups["Guides"]["docs/guide.md"] = "home"
        self.manifest()
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            self.build()

    def test_safe_idempotent_output_and_removed_page_cleanup(self):
        output = Path(self.temp.name) / "wiki"
        output.mkdir()
        (output / "Manual.md").write_text("Keep this", encoding="utf-8")
        pages = self.build()
        wiki.write_output(output, pages)
        first = {p.name: p.read_bytes() for p in output.iterdir()}
        wiki.write_output(output, self.build())
        self.assertEqual(first, {p.name: p.read_bytes() for p in output.iterdir()})
        del pages["Guide.md"]
        wiki.write_output(output, pages)
        self.assertFalse((output / "Guide.md").exists())
        self.assertEqual((output / "Manual.md").read_text(), "Keep this")

    def test_unmanaged_collision_and_tampered_manifest_fail_before_writing(self):
        output = Path(self.temp.name) / "wiki"
        output.mkdir()
        (output / "Home.md").write_text("Handwritten", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unmanaged"):
            wiki.write_output(output, self.build())
        self.assertFalse((output / "Guide.md").exists())
        (output / wiki.MANIFEST).write_text('["../outside.md"]', encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "manifest"):
            wiki.write_output(output, self.build())

    def test_bad_repository_or_revision_rejected(self):
        for repository, revision in [("../private", REVISION), ("example/synthetic", "main")]:
            with self.assertRaises(ValueError):
                wiki.build(self.root, repository, revision)


if __name__ == "__main__":
    unittest.main()
