"""Publish an immutable source release only for a successful main CI revision."""

import argparse
import json
import os
from urllib.parse import quote, urlencode

from github_client import GitHub
from release import validate
from repository import ROOT, commit


def verified_main(api, revision):
    query = urlencode({"branch": "main", "event": "push", "head_sha": revision, "status": "success", "per_page": 100})
    runs = api.request(f"actions/workflows/ci.yml/runs?{query}")["workflow_runs"]
    if not any(run["head_sha"] == revision and run["head_branch"] == "main"
               and run["event"] == "push" and run["conclusion"] == "success" for run in runs):
        raise ValueError("This revision has no successful main push run of CI")
    return api.request("git/ref/heads/main")["object"]["sha"] == revision


def publish(api, revision, version, notes):
    tag = f"v{version}"
    ref = api.request(f"git/ref/tags/{quote(tag)}", missing_ok=True)
    if ref:
        obj = ref["object"]
        if obj["type"] == "tag":
            obj = api.request(f"git/tags/{obj['sha']}")["object"]
        if obj["type"] != "commit" or obj["sha"] != revision:
            raise ValueError("Release tag already points to a different revision; never move it")
    existing = api.request(f"releases/tags/{quote(tag)}", missing_ok=True)
    if existing:
        if not ref or existing["draft"] or existing["prerelease"] or existing["body"].strip() != notes.strip():
            raise ValueError("Existing release differs from the approved source release")
        return {"status": "already_published", "url": existing["html_url"]}
    release = api.request("releases", {"tag_name": tag, "target_commitish": revision,
        "name": f"APEX {version}", "body": notes, "draft": False, "prerelease": False,
        "make_latest": "legacy"}, method="POST")
    return {"status": "published", "url": release["html_url"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--require-current", action="store_true")
    args = parser.parse_args()
    try:
        api, revision = GitHub(), commit("HEAD")
        current = verified_main(api, revision)
        if args.verify_only:
            if args.require_current and not current:
                raise ValueError("A newer main revision exists; do not publish an older wiki")
            if os.environ.get("GITHUB_OUTPUT"):
                with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
                    output.write(f"current_main={str(current).lower()}\n")
            print(json.dumps({"verified_main": revision, "current_main": current}))
            return
        result = validate(f"{revision}^1", "main")
        if not result["version_changed"]:
            print(json.dumps({"status": "maintenance_only", "revision": revision}))
            return
        notes = (ROOT / f"dev/releases/{result['version']}.md").read_text(encoding="utf-8")
        print(json.dumps(publish(api, revision, result["version"], notes)))
    except (ValueError, OSError, RuntimeError) as error:
        parser.exit(1, f"Publication failed: {error}\n")


if __name__ == "__main__":
    main()
