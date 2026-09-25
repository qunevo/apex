"""Keep the required CI gate present while skipping runtime checks for prose."""

import json
import os

from repository import changed_files, commit, docs_only, git


def plan(paths, force=False):
    paths = [path for path in paths if path]
    return {"runtime": force or not paths or any(not docs_only(path) for path in paths)}


def main():
    with open(os.environ["GITHUB_EVENT_PATH"], encoding="utf-8") as source:
        event = json.load(source)
    name = os.environ["GITHUB_EVENT_NAME"]
    if name == "pull_request":
        base, target = event["pull_request"]["base"]["sha"], event["pull_request"]["base"]["ref"]
    elif name == "push" and event.get("before", "").strip("0"):
        base, target = event["before"], event["ref"].removeprefix("refs/heads/")
    else:
        base, target = git("rev-parse", "HEAD^"), os.environ.get("GITHUB_REF_NAME", "dev")
    result = {**plan(changed_files(base), force=name == "workflow_dispatch"),
              "base": commit(base), "target": target}
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        for key, value in result.items():
            output.write(f"{key}={str(value).lower() if isinstance(value, bool) else value}\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
