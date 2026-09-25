"""No-network checks of publication immutability and expected-head PR guards."""

from pathlib import Path
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from publish_release import publish, verified_main
from pull_request import merge_pr, open_pr
from configure_github import setup
from github_client import GitHub


SHA = "a" * 40
OTHER = "b" * 40
RULES = [{"type": "pull_request"}, {"type": "required_status_checks", "parameters": {
    "required_status_checks": [{"context": "Required checks"}]
}}]


class ClientTests(unittest.TestCase):
    def test_repository_root_and_nested_endpoints_use_valid_urls(self):
        with patch("github_client.token", return_value="synthetic-test-token"):
            api = GitHub(repository="example/synthetic")
        response = Mock()
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)
        response.read.return_value = b'{}'
        with patch("github_client.urlopen", return_value=response) as send:
            api.request("")
            self.assertEqual(send.call_args.args[0].full_url, "https://api.github.com/repos/example/synthetic")
            api.request("", {"delete_branch_on_merge": True}, method="PATCH")
            self.assertEqual(send.call_args.args[0].full_url, "https://api.github.com/repos/example/synthetic")
            self.assertEqual(send.call_args.args[0].method, "PATCH")
            api.request("pulls/1")
            self.assertEqual(send.call_args.args[0].full_url, "https://api.github.com/repos/example/synthetic/pulls/1")


class PublicationTests(unittest.TestCase):
    def test_new_release_targets_verified_revision(self):
        api = Mock()
        api.request.side_effect = [None, None, {"html_url": "https://example.invalid/release"}]
        self.assertEqual(publish(api, SHA, "1.2.3", "Notes")["status"], "published")
        args = api.request.call_args
        self.assertEqual(args.args[1]["target_commitish"], SHA)
        self.assertEqual(args.kwargs["method"], "POST")

    def test_rerun_does_not_publish_again(self):
        api = Mock()
        api.request.side_effect = [{"object": {"type": "commit", "sha": SHA}},
            {"draft": False, "prerelease": False, "body": "Notes", "html_url": "https://example.invalid/release"}]
        self.assertEqual(publish(api, SHA, "1.2.3", "Notes")["status"], "already_published")
        self.assertEqual(api.request.call_count, 2)

    def test_release_order_uses_semver_instead_of_completion_time(self):
        api = Mock()
        api.request.side_effect = [None, None, {"html_url": "https://example.invalid/release"}]
        publish(api, SHA, "1.2.3", "Notes")
        self.assertEqual(api.request.call_args.args[1]["make_latest"], "legacy")

    def test_annotated_tag_is_resolved_and_conflicting_tag_fails(self):
        api = Mock()
        api.request.side_effect = [{"object": {"type": "tag", "sha": OTHER}},
                                   {"object": {"type": "commit", "sha": OTHER}}]
        with self.assertRaisesRegex(ValueError, "different revision"):
            publish(api, SHA, "1.2.3", "Notes")
        self.assertEqual(api.request.call_count, 2)

    def test_existing_notes_cannot_be_rewritten(self):
        api = Mock()
        api.request.side_effect = [{"object": {"type": "commit", "sha": SHA}},
            {"draft": False, "prerelease": False, "body": "Other notes"}]
        with self.assertRaisesRegex(ValueError, "differs"):
            publish(api, SHA, "1.2.3", "Notes")

    def test_pr_ci_does_not_authorize_publication(self):
        api = Mock()
        api.request.return_value = {"workflow_runs": [{"head_sha": SHA, "head_branch": "main",
                                                       "event": "pull_request", "conclusion": "success"}]}
        with self.assertRaisesRegex(ValueError, "no successful main push"):
            verified_main(api, SHA)

    def test_current_main_distinguished_from_older_successful_run(self):
        api = Mock()
        api.request.side_effect = [{"workflow_runs": [{"head_sha": SHA, "head_branch": "main",
            "event": "push", "conclusion": "success"}]}, {"object": {"sha": OTHER}}]
        self.assertFalse(verified_main(api, SHA))


class SetupTests(unittest.TestCase):
    def api(self, existing=None):
        api = Mock()
        rules = RULES + [{"type": "deletion"}, {"type": "non_fast_forward"}]
        responses = {"": {"default_branch": "main"}, "git/ref/heads/main": {"object": {"sha": SHA}},
                     "git/ref/heads/dev": None, "rules/branches/main": rules,
                     "rulesets?per_page=100": [{"name": "Protect dev", "id": 7}] if existing else [],
                     "rulesets/7": existing}
        api.request.side_effect = lambda path, *args, **kwargs: responses.get(path)
        return api

    def test_dry_run_never_mutates_github(self):
        api = self.api()
        result = setup(api)
        self.assertTrue(result["create_dev"])
        self.assertTrue(result["enable_automatic_branch_deletion"])
        self.assertTrue(all(len(call.args) == 1 and "method" not in call.kwargs
                            for call in api.request.call_args_list))

    def test_conflicting_rules_rejected_before_branch_creation(self):
        for apply in (False, True):
            api = self.api(existing={"name": "Protect dev", "enforcement": "disabled"})
            with self.subTest(apply=apply), self.assertRaisesRegex(ValueError, "differs"):
                setup(api, apply=apply)
            self.assertTrue(all("method" not in call.kwargs for call in api.request.call_args_list))

    def test_apply_creates_dev_at_exact_main_and_preserves_long_lived_branches(self):
        api = self.api()
        setup(api, apply=True)
        mutations = [call for call in api.request.call_args_list if "method" in call.kwargs]
        self.assertEqual([call.kwargs["method"] for call in mutations], ["POST", "POST", "PATCH", "PUT", "PUT"])
        self.assertEqual(mutations[0].args, ("git/refs", {"ref": "refs/heads/dev", "sha": SHA}))
        self.assertEqual(mutations[1].args[1]["enforcement"], "active")
        self.assertEqual(mutations[1].args[1]["bypass_actors"], [])
        self.assertIn({"type": "deletion"}, mutations[1].args[1]["rules"])
        self.assertEqual(mutations[2].args, ("", {"delete_branch_on_merge": True}))
        self.assertEqual([call.args[0] for call in mutations[3:]], ["vulnerability-alerts", "automated-security-fixes"])

    def test_missing_deletion_protection_prevents_cleanup_setting_and_all_mutations(self):
        for apply in (False, True):
            api = self.api()
            responses = api.request.side_effect
            api.request.side_effect = lambda path, *args, **kwargs: (
                [rule for rule in responses(path) if rule["type"] != "deletion"]
                if path == "rules/branches/main" else responses(path, *args, **kwargs)
            )
            with self.subTest(apply=apply), self.assertRaisesRegex(ValueError, "history protection"):
                setup(api, apply=apply)
            self.assertTrue(all("method" not in call.kwargs for call in api.request.call_args_list))


class MergeTests(unittest.TestCase):
    def ready(self):
        return {"head": {"sha": SHA}, "base": {"ref": "dev"}, "state": "open", "draft": False,
                "mergeable_state": "clean"}

    def test_changed_head_or_destination_never_reaches_mutation(self):
        for head, base in ((OTHER, "dev"), (SHA, "main")):
            api = Mock()
            api.request.return_value = self.ready()
            with self.assertRaisesRegex(ValueError, "changed"):
                merge_pr(api, 1, head, base)
            self.assertEqual(api.request.call_count, 1)

    def test_failed_pending_or_blocked_checks_never_merge(self):
        for state in ("blocked", "behind", "dirty", "unknown", "unstable"):
            api = Mock()
            api.request.return_value = {**self.ready(), "mergeable_state": state}
            with self.assertRaisesRegex(ValueError, "not ready"):
                merge_pr(api, 1, SHA, "dev")
            self.assertEqual(api.request.call_count, 1)

    def test_unprotected_destination_is_rejected(self):
        api = Mock()
        api.request.side_effect = [self.ready(), []]
        with self.assertRaisesRegex(ValueError, "must enforce"):
            merge_pr(api, 1, SHA, "dev")
        self.assertEqual(api.request.call_count, 2)

    def test_merge_sends_exact_head_to_normal_endpoint(self):
        api = Mock()
        api.request.side_effect = [self.ready(), RULES, {"merged": True}]
        self.assertTrue(merge_pr(api, 1, SHA, "dev")["merged"])
        self.assertEqual(api.request.call_args.args, ("pulls/1/merge", {"sha": SHA, "merge_method": "merge"}))

    def test_reuses_existing_pr(self):
        api = Mock(repository="example/synthetic")
        api.request.return_value = [{"number": 1}]
        self.assertEqual(open_pr(api, "codex/example", "dev", "Title", "Body"), {"number": 1})
        self.assertEqual(api.request.call_count, 1)


if __name__ == "__main__":
    unittest.main()
