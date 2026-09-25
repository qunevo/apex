"""Build and exercise an application source export outside the repository."""

import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import tomllib
from urllib.error import URLError
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parents[2]


def run(args, cwd, env, **kwargs):
    print("+ " + " ".join(map(str, args)), flush=True)
    return subprocess.run(list(map(str, args)), cwd=cwd, env=env, check=True, **kwargs)


def main():
    source = ROOT / "app"
    if (ROOT / "LICENSE").read_bytes() != (source / "LICENSE").read_bytes():
        raise ValueError("app/LICENSE must be byte-identical to the canonical root LICENSE")
    files = subprocess.check_output(
        ["git", "-C", str(ROOT), "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "app/"],
        text=True,
    ).strip("\0").split("\0")
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin" / ("cargo.exe" if os.name == "nt" else "cargo"))
    bash = shutil.which("bash")
    if not bash:
        raise ValueError("Bash (Git Bash on Windows) is required to verify deployment helpers")
    env = dict(os.environ)
    env.pop("CARGO_TARGET_DIR", None)
    # A local service's settings must not affect this independent smoke test.
    for key in ("APEX_API_TOKEN", "APEX_PUBLIC_URL", "APEX_BIND"):
        env.pop(key, None)
    hidden = {"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}
    with tempfile.TemporaryDirectory(prefix="apex standalone ") as directory:
        temporary = Path(directory).resolve()
        assert temporary.is_relative_to(Path(tempfile.gettempdir()).resolve())
        assert not temporary.is_relative_to(ROOT)
        app = temporary / "application source"
        app.mkdir()
        for name in sorted(set(files)):
            relative = Path(name).relative_to("app")
            original, target = source / relative, app / relative
            if not original.resolve().is_relative_to(source) or original.is_symlink():
                raise ValueError(f"Application source must be a regular file: {name}")
            if not original.is_file():
                raise ValueError(f"Missing application source: {name}")
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original, target)
        assert not (temporary / "dev").exists() and not (temporary / "Cargo.toml").exists()
        run([cargo, "build", "--locked", "--release"], app, env)
        binary = app / "target/release" / ("apex.exe" if os.name == "nt" else "apex")
        output = temporary / "planning results"
        output.mkdir()
        problem = output / "production.json"
        run([binary, "expand", "examples/production-orders.json", "--out", problem], app, env)
        completed = []
        for method in ("plan", "hypersearch", "treesearch", "improve", "evolve"):
            schedule = output / f"{method}.json"
            run([binary, method, problem, "--iterations", "8", "--workers", "1", "--out", schedule], app, env)
            run([binary, "validate", problem, schedule], app, env)
            completed.append(method)
        for fixture in ("chain-routing", "dispatch-campaign", "shift-factory"):
            schedule = output / f"{fixture}.json"
            run([binary, "plan", f"examples/{fixture}.json", "--out", schedule], app, env)
            run([binary, "validate", f"examples/{fixture}.json", schedule], app, env)
        schema = run([binary, "schema"], app, env, capture_output=True, text=True)
        assert json.loads(schema.stdout)["$schema"]
        workspace = temporary / "customer workspace"
        workspace.mkdir()
        for helper in sorted((app / "deploy").glob("*.sh")):
            run([bash, "-n", helper.as_posix()], workspace, env)
        setup = [bash, (app / "deploy/setup-mcp.sh").as_posix(), workspace.as_posix()]
        run(setup, workspace, env)
        config = workspace / ".codex/config.toml"
        first = config.read_bytes()
        run(setup, workspace, env)
        assert first == config.read_bytes(), "MCP setup must be idempotent"
        entry = tomllib.loads(first.decode())["mcp_servers"]["apex"]
        assert Path(entry["command"]).resolve() == binary.resolve()
        assert workspace.resolve() == Path(entry["args"][entry["args"].index("--workspace") + 1]).resolve()
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        # Verify the embedded viewer with a directly owned process that can be
        # terminated reliably on Windows as well as Unix.
        process = subprocess.Popen(
            [str(binary), "serve", "--port", str(port), "--workspace", str(workspace)],
            cwd=workspace, env={**env, "APEX_BIND": "127.0.0.1"},
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, **hidden,
        )
        try:
            for attempt in range(100):
                if process.poll() is not None:
                    raise RuntimeError(process.stderr.read().decode())
                try:
                    with urlopen(f"http://127.0.0.1:{port}/", timeout=1) as response:
                        assert response.status == 200 and b"APEX" in response.read()
                    break
                except URLError:
                    time.sleep(0.1)
            else:
                raise RuntimeError("Detached viewer did not start")
        finally:
            process.terminate()
            process.wait(timeout=10)
            process.stderr.close()
        print(json.dumps(dict(passed=True, source_files=len(set(files)), methods=completed,
                              additional_fixtures=3, viewer="embedded HTTP", mcp_setup="separate workspace, idempotent")))


if __name__ == "__main__":
    main()
