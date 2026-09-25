"""Create or merge an explicitly authorized PR using the normal GitHub gate."""

import argparse
import json
from pathlib import Path
from urllib.parse import urlencode

from github_client import GitHub


def open_pr(api, head, base, title, body):
    query = urlencode({"state": "open", "head": f"{api.repository.split('/')[0]}:{head}", "base": base})
    existing = api.request(f"pulls?{query}")
    if existing:
        return existing[0]
    return api.request("pulls", {"head": head, "base": base, "title": title, "body": body}, method="POST")


def merge_pr(api, number, expected_head, expected_base):
    pr = api.request(f"pulls/{number}")
    if pr["head"]["sha"] != expected_head or pr["base"]["ref"] != expected_base:
        raise ValueError("PR source or destination changed; review the current revision")
    if pr["state"] != "open" or pr["draft"] or pr["mergeable_state"] != "clean":
        raise ValueError("PR is not ready: inspect required checks, conversations and merge conflicts")
    rules = api.request(f"rules/branches/{expected_base}")
    if not any(rule["type"] == "pull_request" for rule in rules) or not any(
        rule["type"] == "required_status_checks" and any(
            check["context"] == "Required checks"
            for check in rule["parameters"]["required_status_checks"]
        ) for rule in rules
    ):
        raise ValueError("Destination must enforce pull requests and Required checks before merging")
    # GitHub remains authoritative for current required checks and review state.
    return api.request(f"pulls/{number}/merge", {"sha": expected_head, "merge_method": "merge"}, method="PUT")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("open")
    create.add_argument("--head", required=True)
    create.add_argument("--base", choices=("dev", "main"), required=True)
    create.add_argument("--title", required=True)
    create.add_argument("--body-file", type=Path, required=True)
    status = commands.add_parser("status")
    status.add_argument("number", type=int)
    merge = commands.add_parser("merge")
    merge.add_argument("number", type=int)
    merge.add_argument("--expected-head", required=True)
    merge.add_argument("--expected-base", choices=("dev", "main"), required=True)
    args = parser.parse_args()
    try:
        api = GitHub()
        if args.command == "open":
            pr = open_pr(api, args.head, args.base, args.title, args.body_file.read_text(encoding="utf-8"))
        elif args.command == "merge":
            print(json.dumps(merge_pr(api, args.number, args.expected_head, args.expected_base)))
            return
        else:
            pr = api.request(f"pulls/{args.number}")
        print(json.dumps({key: pr.get(key) for key in (
            "number", "html_url", "state", "draft", "mergeable", "mergeable_state"
        )} | {"head": pr["head"]["sha"], "base": pr["base"]["ref"]}, indent=2))
    except (ValueError, OSError, RuntimeError) as error:
        parser.exit(1, f"PR operation failed: {error}\n")


if __name__ == "__main__":
    main()
