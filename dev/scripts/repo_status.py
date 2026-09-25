"""Bounded read-only preflight: local work and up to five open pull requests."""

import json
import subprocess

from github_client import GitHub
from repository import commit, git


def main():
    result = {"branch": git("branch", "--show-current"),
              "dirty": bool(git("status", "--porcelain"))}
    base = "origin/dev"
    try:
        commit(base)
    except subprocess.CalledProcessError:
        base = "origin/main"
    result["local_comparison"] = base
    branches = git("for-each-ref", f"--no-merged={base}", "--format=%(refname:short)", "refs/heads").splitlines()
    branches = [name for name in branches if name not in ("main", "dev", result["branch"])]
    result["unmerged_local_branches"] = branches[:5]
    result["additional_local_branches"] = max(0, len(branches) - 5)
    result["local_note"] = "Based on local refs; squash-merged branches may still appear. This is not a conflict diagnosis."
    try:
        api = GitHub()
        prs = api.request("pulls?state=open&per_page=6&sort=updated&direction=desc")
        result["open_prs"] = [{"number": pr["number"], "title": pr["title"],
                               "head": pr["head"]["ref"], "base": pr["base"]["ref"],
                               "draft": pr["draft"]} for pr in prs[:5]]
        result["more_open_prs"] = len(prs) > 5
    except (OSError, RuntimeError, subprocess.SubprocessError, ValueError) as error:
        result["github_unavailable"] = str(error)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
