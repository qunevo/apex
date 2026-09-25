"""GitHub REST access for local agents and Actions; never persist credentials."""

import json
import os
import re
import shutil
import subprocess
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from repository import git


class GitHubError(RuntimeError):
    pass


def repository_name():
    value = os.environ.get("GITHUB_REPOSITORY")
    if not value:
        remote = git("remote", "get-url", "origin")
        match = re.fullmatch(r"(?:https://github\.com/|git@github\.com:)([^/]+/[^/]+?)(?:\.git)?", remote)
        if not match:
            raise ValueError("Expected a github.com origin or GITHUB_REPOSITORY")
        value = match[1]
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", value):
        raise ValueError("Expected an owner/repository identifier")
    return value


def token():
    value = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if value:
        return value
    if shutil.which("gh"):
        result = subprocess.run(["gh", "auth", "token", "--hostname", "github.com"],
                                capture_output=True, text=True, timeout=10)
        if result.returncode == 0:
            return result.stdout.strip()
    env = {**os.environ, "GIT_TERMINAL_PROMPT": "0", "GCM_INTERACTIVE": "never"}
    result = subprocess.run(["git", "credential", "fill"],
                            input="protocol=https\nhost=github.com\n\n",
                            capture_output=True, text=True, timeout=10, env=env)
    if result.returncode == 0:
        return dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line).get("password")
    return None


class GitHub:
    def __init__(self, repository=None):
        self.repository = repository or repository_name()
        self._token = token()

    def request(self, path, payload=None, method="GET", missing_ok=False):
        if method != "GET" and not self._token:
            raise GitHubError("Authenticate GitHub CLI or provide GH_TOKEN before changing GitHub")
        headers = {"Accept": "application/vnd.github+json", "User-Agent": "apex-contributor-tools",
                   "X-GitHub-Api-Version": "2022-11-28"}
        if self._token:
            headers["Authorization"] = f"Bearer {self._token}"
        data = None if payload is None else json.dumps(payload).encode()
        suffix = f"/{path}" if path else ""
        request = Request(f"https://api.github.com/repos/{self.repository}{suffix}",
                          data=data, headers=headers, method=method)
        try:
            with urlopen(request, timeout=20) as response:
                body = response.read()
                return json.loads(body) if body else None
        except HTTPError as error:
            if missing_ok and error.code == 404:
                return None
            # Do not echo request headers, tokens or arbitrary remote response bodies.
            raise GitHubError(f"GitHub {method} {path} returned HTTP {error.code}") from None
