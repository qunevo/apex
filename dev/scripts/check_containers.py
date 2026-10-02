"""Exercise standalone APEX and the composed showcase using disposable Docker projects."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[2]


def run(args, env, cwd, timeout=1800):
    result = subprocess.run(args, env=env, cwd=cwd, capture_output=True, text=True,
                            encoding="utf-8", errors="replace", timeout=timeout)
    if result.returncode:
        detail = re.sub(r"apx_[a-f0-9]+", "<redacted>", result.stdout + result.stderr)
        raise RuntimeError(f"Command failed: {' '.join(map(str, args))}\n{detail[-16000:]}")
    return result.stdout


def port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def request(url, payload=None, token=None, method=None, headers=None):
    headers = dict(headers or {})
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if payload is not None:
        headers["Content-Type"] = "application/json"
    req = Request(url, data=None if payload is None else json.dumps(payload).encode(),
                  method=method, headers=headers)
    with urlopen(req, timeout=15) as response:
        body = response.read()
        return json.loads(body) if "application/json" in response.headers.get("Content-Type", "") else body


def copy_source(destination, area):
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", area], cwd=ROOT
    ).decode().split("\0")
    for name in sorted(set(filter(None, paths))):
        source, target = ROOT / name, destination / name
        if not source.exists():
            continue
        assert source.resolve().is_relative_to(ROOT) and not source.is_symlink()
        assert target.resolve().is_relative_to(destination)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)


def verify(directory, kind, image, build):
    project = f"apex-check-{kind}-{uuid.uuid4().hex[:10]}"
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("APEX_", "COMPOSE_", "DEMO_"))}
    env.update(APEX_HTTP_PORT=str(port()), DEMO_MES_PORT=str(port()), APEX_IMAGE=image)
    if hasattr(os, "getuid"):
        env.update(DEMO_UID=str(os.getuid()), DEMO_GID=str(os.getgid()))
    cwd = directory / kind
    if kind == "demo":
        # Keep parent directories owned by the caller instead of the Docker daemon.
        (cwd / ".local/container").mkdir(parents=True, exist_ok=True)
    command = ["docker", "compose", "--project-name", project]
    compose = lambda *args: run(command + list(args), env, cwd)
    model = json.loads(compose("config", "--format", "json"))
    assert model["services"]["apex"]["build"]["context"] == str(directory / "app")
    if kind == "app":
        assert not (directory / "demo").exists()
        assert "mes" not in model["services"]
        assert not model["services"]["apex"]["environment"].get("APEX_CONFIG")
    origin = f"http://127.0.0.1:{env['APEX_HTTP_PORT']}"
    mes = f"http://127.0.0.1:{env['DEMO_MES_PORT']}"
    try:
        print(f"{kind}: building and starting isolated Compose project", flush=True)
        compose("up", "-d", "--build" if build else "--no-build", "--wait", "--wait-timeout", "240")
        access = compose("exec", "-T", "apex", "apex-container", "access")
        tokens = re.findall(r"apx_[a-f0-9]+", access)
        assert len(tokens) == 2
        token = tokens[0]
        connection = compose("exec", "-T", "apex", "apex-container", "connect")
        assert origin + "/mcp" in connection and f"Bearer {token}" in connection
        assert "second terminal" in connection
        startup = compose("logs", "--no-color", "apex")
        assert origin + "/mcp" in startup
        assert (token in startup) == (kind == "demo")
        assert tokens[1] not in startup
        try:
            request(origin + "/v1/scenarios")
        except HTTPError as error:
            assert error.code == 401
        else:
            raise AssertionError("Unauthenticated request was accepted")

        def rpc(method, params):
            reply = request(origin + "/mcp", {"jsonrpc": "2.0", "id": 1,
                            "method": method, "params": params}, token)
            assert "error" not in reply, reply
            return reply["result"]

        def tool(name, args):
            result = rpc("tools/call", {"name": name, "arguments": args})
            assert not result.get("isError"), result
            return result["structuredContent"]

        facts = json.loads((directory / "app/tests/fixtures/shift-factory.json").read_text())
        created = tool("scenarios.create", {"name": "Synthetic container verification",
                       "engine": "apex", "content": {"facts": facts}})
        scenario = created["scenario"]["id"]
        revision = tool("revisions.get", {"scenario_id": scenario, "number": 1})
        expected = {"id": "demo", "version": "1"} if kind == "demo" else None
        assert revision["content"].get("customization_package") == expected
        queued = tool("runs.start", {"scenario_id": scenario})
        for _ in range(150):
            state = tool("runs.get", {"run_id": queued["id"]})
            if state["state"] in ("succeeded", "failed", "cancelled"):
                break
            time.sleep(.2)
        assert state["state"] == "succeeded", state
        result_id = state["result"]
        result = tool("results.get", {"result_id": result_id})
        assert result["validation"]["valid"] and result["view"]["operations"]
        resource = rpc("resources/read", {"uri": "ui://apex/plan.html"})
        assert resource["contents"][0]["mimeType"] == "text/html;profile=mcp-app"
        assert "ui/initialize" in resource["contents"][0]["text"]
        privileges = compose("exec", "-T", "apex", "bash", "-c",
            'psql "$(cat /var/lib/apex/database-url)" -Atc '
            '"SELECT current_user, rolsuper, rolbypassrls FROM pg_roles WHERE rolname=current_user"')
        assert privileges.strip() == "apex_app|f|f", privileges
        compose("exec", "-T", "apex", "test", "!", "-e", "/run/apex-bootstrap/postgres-password")

        if kind == "demo":
            meta = request(mes + "/api/meta")
            assert meta["counts"]["orders"] == 120 and meta["workbook_available"]
            machine = request(mes + "/api/tables/machines?limit=1")["rows"][0]
            request(mes + "/api/tables/machines/" + machine["id"],
                    {"expected_version": machine["_version"], "data": {"name": "Container persistence check"}}, method="PATCH")
            workbook = directory / "demo/.local/container/production-planning.xlsx"
            with zipfile.ZipFile(workbook, "a") as archive:
                archive.comment = b"Container persistence check"
            digest = hashlib.sha256(workbook.read_bytes()).hexdigest()
            assert hashlib.sha256(request(mes + "/downloads/production-planning.xlsx")).hexdigest() == digest
            mounted = compose("exec", "-T", "apex", "sha256sum", "/sources/demo/production-planning.xlsx")
            assert mounted.split()[0] == digest
            compose("exec", "-T", "apex", "curl", "--fail", "--silent", "http://mes:8788/api/meta")
            try:
                request(mes + "/api/meta", headers={"Origin": "https://example.invalid"})
            except HTTPError as error:
                assert error.code == 400
            else:
                raise AssertionError("Cross-origin MES request was accepted")

        # Recreate every service, including setup/migration, while retaining volumes.
        compose("down")
        compose("up", "-d", "--no-build", "--wait", "--wait-timeout", "240")
        assert compose("exec", "-T", "apex", "apex-container", "access") == access
        assert tool("results.get", {"result_id": result_id})["validation"]["valid"]
        if kind == "demo":
            rows = request(mes + "/api/tables/machines?limit=100")["rows"]
            assert next(row for row in rows if row["id"] == machine["id"])["name"] == "Container persistence check"
            assert hashlib.sha256(request(mes + "/downloads/production-planning.xlsx")).hexdigest() == digest
        print(f"{kind}: startup, authenticated planning, MCP App, database role and retained state passed", flush=True)
    finally:
        # Only this script's uniquely named disposable project is removed.
        compose("down", "--volumes", "--remove-orphans")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stack", choices=("app", "demo", "all"), default="all")
    parser.add_argument("--no-build", action="store_true")
    args = parser.parse_args()
    image = os.environ.get("APEX_IMAGE", "apex-local:dev")
    subprocess.run(["docker", "info"], stdout=subprocess.DEVNULL, check=True, timeout=30)
    with tempfile.TemporaryDirectory(prefix="apex container check ") as temporary:
        directory = Path(temporary).resolve()
        assert directory.is_relative_to(Path(tempfile.gettempdir()).resolve())
        copy_source(directory, "app/")
        if args.stack in ("app", "all"):
            verify(directory, "app", image, not args.no_build)
        if args.stack in ("demo", "all"):
            copy_source(directory, "demo/")
            verify(directory, "demo", image, not args.no_build)


if __name__ == "__main__":
    main()
