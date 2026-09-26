"""Keep invocation policy effective in generated skill discovery entries."""

from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from sync_skills import entries


class SkillDiscoveryTests(unittest.TestCase):
    def test_explicit_invocation_policy_is_copied_and_updates(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            skill = root / "dev/skills/example"
            skill.mkdir(parents=True)
            (skill / "SKILL.md").write_text(
                "---\nname: example\ndescription: Example skill.\n---\n", encoding="utf-8")
            self.assertEqual(len(dict(entries(root))), 1)
            metadata = skill / "agents/openai.yaml"
            metadata.parent.mkdir()
            policy = "policy:\n  allow_implicit_invocation: false\n"
            metadata.write_text(policy, encoding="utf-8")
            destination = root / ".agents/skills/example/agents/openai.yaml"
            self.assertEqual(dict(entries(root))[destination], policy)
            revised = 'interface:\n  display_name: "Example"\n' + policy
            metadata.write_text(revised, encoding="utf-8")
            self.assertEqual(dict(entries(root))[destination], revised)

    def test_board_discovery_disables_implicit_invocation(self):
        root = Path(__file__).resolve().parents[2]
        expected = dict(entries(root))
        destination = root / ".agents/skills/apex-board/agents/openai.yaml"
        self.assertIn("allow_implicit_invocation: false", expected[destination])
        self.assertEqual(destination.read_text(encoding="utf-8"), expected[destination])


if __name__ == "__main__":
    unittest.main()
