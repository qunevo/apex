"""Inspect GitHub branch policy; activate the reviewed dev workflow only with --apply."""

import argparse
import json

from github_client import GitHub


def setup(api, apply=False):
    repository = api.request("")
    if repository["default_branch"] != "main":
        raise ValueError("Review the default branch before setup; this workflow expects main")
    main = api.request("git/ref/heads/main")
    dev = api.request("git/ref/heads/dev", missing_ok=True)
    rules = api.request("rules/branches/main")
    checks = [rule for rule in rules if rule["type"] == "required_status_checks"]
    if not checks or not any(check["context"] == "Required checks" for rule in checks
                             for check in rule["parameters"]["required_status_checks"]):
        raise ValueError("Main must already require Required checks; do not weaken protection")
    keep = {"deletion", "non_fast_forward", "pull_request", "required_status_checks"}
    selected = [{key: rule[key] for key in ("type", "parameters") if key in rule}
                for rule in rules if rule["type"] in keep]
    if {rule["type"] for rule in selected} != keep:
        raise ValueError("Main is missing expected pull-request or history protection")
    payload = {"name": "Protect dev", "target": "branch", "enforcement": "active", "bypass_actors": [],
               "conditions": {"ref_name": {"include": ["refs/heads/dev"], "exclude": []}}, "rules": selected}
    existing = [rule for rule in api.request("rulesets?per_page=100") if rule["name"] == "Protect dev"]
    if len(existing) > 1:
        raise ValueError("Multiple Protect dev rulesets need manual reconciliation")
    if existing:
        # Validate before any mutation, including branch creation.
        actual = api.request(f"rulesets/{existing[0]['id']}")
        if any(actual.get(key) != value for key, value in payload.items()):
            raise ValueError("Existing Protect dev differs; review it instead of overwriting its rules")
    result = {"apply": apply, "create_dev": dev is None, "source_sha": main["object"]["sha"],
              "protect_dev": payload, "enable_automatic_branch_deletion": True,
              "enable_dependency_alerts_and_security_updates": True}
    if apply:
        if not dev:
            api.request("git/refs", {"ref": "refs/heads/dev", "sha": main["object"]["sha"]}, method="POST")
        if not existing:
            api.request("rulesets", payload, method="POST")
        # Protect both long-lived branches before enabling PR head cleanup.
        api.request("", {"delete_branch_on_merge": True}, method="PATCH")
        api.request("vulnerability-alerts", method="PUT")
        api.request("automated-security-fixes", method="PUT")
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    try:
        print(json.dumps(setup(GitHub(), args.apply), indent=2))
    except (ValueError, OSError, RuntimeError) as error:
        parser.exit(1, f"Setup failed: {error}\n")
