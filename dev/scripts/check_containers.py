"""Exercise standalone APEX and the composed showcase using disposable Docker projects."""

import argparse
import hashlib
from html import unescape
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


def request(url, payload=None, token=None, method=None, headers=None, timeout=15):
    headers = dict(headers or {})
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if payload is not None:
        headers["Content-Type"] = "application/json"
    req = Request(url, data=None if payload is None else json.dumps(payload).encode(),
                  method=method, headers=headers)
    with urlopen(req, timeout=timeout) as response:
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
            # Exercise the complete public MES + saved Excel adapter, not only a small engine fixture.
            (cwd / ".local/adapter").mkdir(parents=True, exist_ok=True)
            compose("--profile", "adapter", "run", "--build", "--rm", "adapter",
                    "--config", "/config/sources.json", "--output", "/outputs/acceptance",
                    "--horizon-end", "2026-10-30T22:00")
            imported = json.loads((cwd / ".local/adapter/acceptance/scenario.json").read_text())
            imported_ids = {task["id"] for task in imported["content"]["facts"]["tasks"]}
            assert len(imported_ids) == 3705
            factory = request(origin + "/v1/scenarios", imported, token, timeout=120)
            factory_id = factory["scenario"]["id"]
            del imported
            factory_run = tool("runs.start", {"scenario_id": factory_id,
                               "options": {"method": "create", "options": {"strategy": "release"}}})
            for _ in range(300):
                factory_state = tool("runs.get", {"run_id": factory_run["id"]})
                if factory_state["state"] in ("succeeded", "failed", "cancelled"):
                    break
                time.sleep(.2)
            assert factory_state["state"] == "succeeded", factory_state
            factory_result = tool("results.get", {"result_id": factory_state["result"]})
            assert factory_result["validation"]["valid"], factory_result["validation"]
            assert {row["id"] for row in factory_result["view"]["operations"]} == imported_ids
            overview = tool("views.get", {"scenario_id": factory_id, "result_id": factory_state["result"]})
            assert overview["source_summary"]["counts"]["planned_operations"] == 3705
            sources = tool("views.get", {"scenario_id": factory_id, "view_id": "demo.sources"})
            assert sources["view_id"] == "demo.sources"
            insights = rpc("resources/read", {"uri": "ui://apex/insights.html"})
            assert insights["contents"][0]["mimeType"] == "text/html;profile=mcp-app"
            print("demo: MES + Excel -> 3,705 operations -> free release strategy -> valid MCP result; explicit October 30 horizon", flush=True)
            bash = shutil.which("bash")
            assert bash, "Bash is required to verify the demo launcher"
            launched = run([bash, (cwd / "scripts/start-demo.sh").as_posix(), "--resume", "--no-build", "--no-open"],
                           {**env, "COMPOSE_PROJECT_NAME": project}, cwd)
            assert token not in launched and "Demo ready" in launched
            page = unescape((cwd / ".local/start.html").read_text(encoding="utf-8"))
            assert origin + "/mcp" in page and mes in page and token in page
            assert tokens[1] not in page
            assert not (cwd / ".local/container/start.html").exists()
            machine = request(mes + "/api/tables/machines?limit=1")["rows"][0]
            request(mes + "/api/tables/machines/" + machine["id"],
                    {"expected_version": machine["_version"], "data": {"name": "Container persistence check"}}, method="PATCH")
            workbook = directory / "demo/.local/container/production-planning.xlsx"
            with zipfile.ZipFile(workbook, "a") as archive:
                archive.comment = b"Container persistence check"
            digest = hashlib.sha256(workbook.read_bytes()).hexdigest()
            assert compose("exec", "-T", "mes", "sha256sum", "/var/lib/demo/production-planning.xlsx").split()[0] == digest
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
            assert compose("exec", "-T", "mes", "sha256sum", "/var/lib/demo/production-planning.xlsx").split()[0] == digest
            lock = workbook.with_name("~$" + workbook.name)
            lock.write_text("Synthetic Excel owner file", encoding="utf-8")
            try:
                request(mes + "/api/reset", {"confirmation": "RESET DEMO"})
            except HTTPError as error:
                assert error.code == 409
            else:
                raise AssertionError("Reset accepted a workbook open in Excel")
            finally:
                lock.unlink()
            assert hashlib.sha256(workbook.read_bytes()).hexdigest() == digest
            reset = request(mes + "/api/reset", {"confirmation": "RESET DEMO"})
            assert reset == {"reset": True, "workbook_reset": True}
            baseline = hashlib.sha256((directory / "demo/planning/production-planning.xlsx").read_bytes()).hexdigest()
            assert hashlib.sha256(workbook.read_bytes()).hexdigest() == baseline
            assert compose("exec", "-T", "mes", "sha256sum", "/var/lib/demo/production-planning.xlsx").split()[0] == baseline
            assert compose("exec", "-T", "apex", "sha256sum", "/sources/demo/production-planning.xlsx").split()[0] == baseline
            rows = request(mes + "/api/tables/machines?limit=100")["rows"]
            assert next(row for row in rows if row["id"] == machine["id"])["name"] == machine["name"]
            assert compose("exec", "-T", "apex", "apex-container", "access") == access
            assert tool("results.get", {"result_id": result_id})["validation"]["valid"]
            print("demo: local launcher, private setup page and shared workbook reset passed", flush=True)
            reset_command = [bash, (cwd / "scripts/start-demo.sh").as_posix(), "--reset", "--no-build", "--no-open"]
            launcher_env = {**env, "COMPOSE_PROJECT_NAME": project}
            lock.write_text("Synthetic Excel owner file", encoding="utf-8")
            try:
                run(reset_command, launcher_env, cwd)
            except RuntimeError as error:
                assert "Close the shared Excel workbook" in str(error)
            else:
                raise AssertionError("Launcher reset accepted an open workbook")
            finally:
                lock.unlink()
            assert tool("results.get", {"result_id": result_id})["validation"]["valid"]
            assert compose("exec", "-T", "apex", "apex-container", "access") == access
            row = next(row for row in rows if row["id"] == machine["id"])
            request(mes + "/api/tables/machines/" + row["id"],
                    {"expected_version": row["_version"], "data": {"name": "Before full reinitialization"}}, method="PATCH")
            with zipfile.ZipFile(workbook, "a") as archive:
                archive.comment = b"Synthetic edit before full reinitialization"
            print("demo: verifying full reinitialization with retained credentials", flush=True)
            reset_output = run(reset_command, launcher_env, cwd)
            assert token not in reset_output and "Demo reinitialized" in reset_output
            assert compose("exec", "-T", "apex", "apex-container", "access") == access
            assert tool("scenarios.list", {})["items"] == []
            assert hashlib.sha256(workbook.read_bytes()).hexdigest() == baseline
            assert compose("exec", "-T", "apex", "sha256sum", "/sources/demo/production-planning.xlsx").split()[0] == baseline
            rows = request(mes + "/api/tables/machines?limit=100")["rows"]
            assert next(row for row in rows if row["id"] == machine["id"])["name"] == machine["name"]
            page = unescape((cwd / ".local/start.html").read_text(encoding="utf-8"))
            assert token in page and origin + "/mcp" in page and mes in page
            tool("scenarios.create", {"name": "After reinitialization", "engine": "apex", "content": {"facts": facts}})
            assert len(tool("scenarios.list", {})["items"]) == 1
            print("demo: full reset removed planning data, restored MES/Excel and retained working tokens", flush=True)
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
