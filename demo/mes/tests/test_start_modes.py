"""Check that launcher choices constrain destructive operations to this demo."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


@unittest.skipUnless(shutil.which("bash"), "Bash is required for the launcher")
class StartModeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="demo modes ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        scripts = self.root / "demo/scripts"
        scripts.mkdir(parents=True)
        source = Path(__file__).resolve().parents[2] / "scripts/start-demo.sh"
        shutil.copy2(source, scripts / "start-demo.sh")
        shared = self.root / "demo/.local/container"
        shared.mkdir(parents=True)
        (shared / "production-planning.xlsx").write_bytes(b"Synthetic saved edits")
        self.log = self.root / "docker.log"
        self.harness = self.root / "harness.sh"
        self.harness.write_text('''#!/usr/bin/env bash
set -euo pipefail
docker() {
    printf '%s\\n' "$*" >> "$DOCKER_LOG"
    case "$1 $2" in
        'compose up'|'compose stop'|'compose rm') return 0 ;;
        'compose ps') printf '%s\\n' abc123 ;;
        'compose port') printf '%s\\n' 127.0.0.1:18788 ;;
        'compose exec')
            if [[ "$4" == apex ]]; then
                printf 'MCP URL: http://127.0.0.1:18780/mcp\\nHTTP header value: Bearer apx_%064d\\n' 0
            elif [[ "$*" == *demo.onboarding* ]]; then
                cat >/dev/null
                printf '%s\\n' '<html>Synthetic setup</html>'
            else
                return "$MES_RESET_STATUS"
            fi ;;
        'inspect --format')
            if [[ "$3" == *Mounts* ]]; then printf '%s\\n' demo-test_postgres-data
            else printf '%s\\n' demo-test; fi ;;
        'volume inspect')
            if [[ "$4" == *compose.project* ]]; then printf '%s\\n' "$VOLUME_PROJECT"
            else printf '%s\\n' postgres-data; fi ;;
        'ps -aq') printf '%s\\n' "$VOLUME_CONTAINER" ;;
        'volume rm') [[ "$3" == demo-test_postgres-data ]] ;;
        *) return 77 ;;
    esac
}
export -f docker
bash "$(dirname "$0")/demo/scripts/start-demo.sh" --no-build --no-open "$@"
''', encoding="utf-8", newline="\n")
        self.env = dict(os.environ, DOCKER_LOG=self.log.as_posix(), MES_RESET_STATUS="0",
                        VOLUME_PROJECT="demo-test", VOLUME_CONTAINER="abc123")

    def run_mode(self, *options):
        result = subprocess.run([shutil.which("bash"), self.harness.as_posix(), *options],
                                env=self.env, cwd=self.root, capture_output=True, text=True)
        log = self.log.read_text() if self.log.exists() else ""
        return result, log

    def test_resume_preserves_data_without_reset_operations(self):
        result, log = self.run_mode("--resume")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Demo ready", result.stdout)
        self.assertNotIn("/api/reset", log)
        self.assertNotIn("volume rm", log)
        self.assertNotIn("compose stop", log)

    def test_reset_removes_only_managed_database_before_regenerating_page(self):
        result, log = self.run_mode("--reset")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Existing MCP connections can keep using their tokens", result.stdout)
        self.assertLess(log.index("/api/reset"), log.index("volume rm demo-test_postgres-data"))
        self.assertLess(log.index("--force-recreate"), log.index("demo.onboarding"))
        self.assertNotIn("down", log)
        self.assertNotIn("apex-state", log)
        self.assertNotIn("bootstrap", log)

    def test_unowned_or_shared_volume_is_rejected_before_any_reset(self):
        for key, value in (("VOLUME_PROJECT", "another-project"), ("VOLUME_CONTAINER", "another-container")):
            with self.subTest(key=key):
                self.log.write_text("")
                before = self.env[key]
                self.env[key] = value
                result, log = self.run_mode("--reset")
                self.env[key] = before
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("/api/reset", log)
                self.assertNotIn("volume rm", log)
                self.assertNotIn("compose stop", log)

    def test_locked_workbook_leaves_apex_database_running_and_intact(self):
        self.env["MES_RESET_STATUS"] = "1"
        result, log = self.run_mode("--reset")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("/api/reset", log)
        self.assertNotIn("volume rm", log)
        self.assertNotIn("compose stop", log)
        self.assertNotIn("demo.onboarding", log)

    def test_conflicting_modes_fail_before_docker(self):
        result, log = self.run_mode("--resume", "--reset")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(log, "")


if __name__ == "__main__":
    unittest.main()
