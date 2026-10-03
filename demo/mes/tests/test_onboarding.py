"""Verify private onboarding output and the launcher without touching a live demo."""
from html import unescape
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from demo.onboarding import render

ROOT = Path(__file__).resolve().parents[3]
TOKEN = "apx_" + "a" * 64
CONNECTION = f"APEX client connection\n  MCP URL: http://127.0.0.1:18780/mcp\n  HTTP header value: Bearer {TOKEN}\n"


class OnboardingTests(unittest.TestCase):
    def test_page_contains_current_connection_and_escaped_host_path(self):
        path = '/synthetic/shared folder/"<script>alert(1)</script>&.xlsx'
        page = render(CONNECTION, "http://127.0.0.1:18788", path, "/synthetic/demo", "demo-test")
        self.assertIn("http://127.0.0.1:18780/mcp", page)
        self.assertIn(TOKEN, page)
        self.assertIn(path, unescape(page))
        self.assertNotIn("<script>alert(1)</script>", page)
        self.assertIn("factory import adapter is not implemented yet", page)
        self.assertIn("Reset demo preserves APEX plans and credentials", page)
        self.assertNotIn("{{", page)

    def test_invalid_or_missing_connection_never_renders_a_success_page(self):
        for connection in ("", CONNECTION + CONNECTION,
                           CONNECTION.replace("http://127.0.0.1:18780/mcp", "javascript:alert(1)"),
                           CONNECTION.replace(TOKEN, "missing")):
            with self.subTest(connection=connection[:20]), self.assertRaises(ValueError):
                render(connection, "http://127.0.0.1:18788", "/tmp/workbook.xlsx", "/synthetic/demo", "demo-test")

    @unittest.skipUnless(shutil.which("bash"), "Bash is required for the launcher")
    def test_launcher_uses_existing_connection_and_keeps_page_outside_mount(self):
        with tempfile.TemporaryDirectory(prefix="demo launcher ") as directory:
            root = Path(directory)
            scripts = root / "demo/scripts"
            scripts.mkdir(parents=True)
            shutil.copy2(ROOT / "demo/scripts/start-demo.sh", scripts / "start-demo.sh")
            shared = root / "demo/.local/container"
            shared.mkdir(parents=True)
            workbook = shared / "production-planning.xlsx"
            workbook.write_bytes(b"Saved user edits")
            harness = root / "harness.sh"
            harness.write_text('''#!/usr/bin/env bash
set -euo pipefail
docker() {
    if [[ "$1" == inspect ]]; then printf '%s\\n' custom-demo; return; fi
    [[ "$1" == compose ]] || return 1
    shift
    case "$1" in
        up) [[ "$*" == 'up --no-build -d --wait --wait-timeout 240' ]] ;;
        port) printf '%s\\n' '127.0.0.1:18788' ;;
        ps) printf '%s\\n' abc123 ;;
        exec)
            case "$3" in
                apex) printf 'MCP URL: http://127.0.0.1:18780/mcp\\nHTTP header value: Bearer apx_%064d\\n' 0 ;;
                mes)
                    if [[ "$*" == *demo.mes.desktop* ]]; then printf '%032d\\n' 0; return; fi
                    [[ "$*" == *'--mes-url http://127.0.0.1:18788 --workbook '* ]]
                    [[ "$*" == *'--demo-directory '* && "$*" == *'--compose-project custom-demo' ]]
                    # Stub only rendering: verify private transport over stdin.
                    IFS= read -r line; [[ "$line" == 'MCP URL: http://127.0.0.1:18780/mcp' ]]
                    cat >/dev/null
                    printf '%s\\n' '<html>Private synthetic setup</html>' ;;
                *) return 1 ;;
            esac ;;
        *) return 1 ;;
    esac
}
open() { printf '%s\\n' "$1" >> "$OPEN_LOG"; return "$OPEN_RESULT"; }
xdg-open() { open "$@"; }
powershell.exe() { if [[ "$*" == *'-WindowStyle Hidden'* ]]; then return 0; fi; open "$APEX_DEMO_OPEN_PATH"; }
nohup() { return 0; }
uname() { printf '%s\\n' "$TEST_SYSTEM"; }
export -f docker open xdg-open powershell.exe uname nohup
bash "$(dirname "$0")/demo/scripts/start-demo.sh" --no-build "$@"
''', encoding="utf-8", newline="\n")
            environment = dict(os.environ, TEST_SYSTEM="Linux", OPEN_LOG=(root / "opened.txt").as_posix(), OPEN_RESULT="0")
            result = subprocess.run([shutil.which("bash"), harness.as_posix(), "--no-open"], cwd=root, env=environment,
                                    capture_output=True, text=True, check=True)
            self.assertNotIn("Bearer", result.stdout + result.stderr)
            self.assertIn("http://127.0.0.1:18788", result.stdout)
            self.assertEqual(workbook.read_bytes(), b"Saved user edits")
            self.assertTrue((root / "demo/.local/start.html").is_file())
            self.assertFalse((shared / "start.html").exists())
            if os.name != "nt":
                self.assertEqual((root / "demo/.local/start.html").stat().st_mode & 0o777, 0o600)
            self.assertFalse((root / "opened.txt").exists())
            systems = ["Linux", "Darwin"] + (["MINGW64_NT"] if os.name == "nt" else [])
            for system in systems:
                for status in ("0", "1"):
                    with self.subTest(system=system, opener_status=status):
                        environment.update(TEST_SYSTEM=system, OPEN_RESULT=status)
                        (root / "opened.txt").write_text("")
                        result = subprocess.run([shutil.which("bash"), harness.as_posix()], cwd=root, env=environment,
                                                capture_output=True, text=True, check=True)
                        opened = (root / "opened.txt").read_text().splitlines()
                        self.assertEqual(len(opened), 2)
                        self.assertTrue(opened[0].endswith("start.html"))
                        self.assertTrue(opened[1].endswith("production-planning.xlsx"))
                        self.assertIn("demo launcher ", opened[1])
                        self.assertEqual("Could not open" in result.stderr, status == "1")
                        self.assertNotIn("Bearer", result.stdout + result.stderr)
                        self.assertEqual(workbook.read_bytes(), b"Saved user edits")


if __name__ == "__main__":
    unittest.main()
