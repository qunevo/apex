"""Verify fixed-file desktop opening, stale helpers and the browser boundary."""
from http.client import HTTPConnection
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import time
import unittest

from demo.mes import desktop
from demo.mes.server import Handler, ThreadingHTTPServer
from demo.mes.store import Conflict


class DesktopTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="demo desktop ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.workbook = self.root / desktop.NAME
        self.workbook.write_bytes(b"Synthetic workbook")
        self.session = desktop.prepare(self.root, self.workbook.as_posix())
        self.bridge = self.root / ".desktop"

    def active(self):
        desktop.write(self.bridge / "heartbeat", f"{self.session} {int(time.time())}")

    def test_no_helper_or_stale_generation_never_accepts_an_open(self):
        for heartbeat in ("", f"{self.session} {int(time.time())-60}", f"{'0'*32} {int(time.time())}"):
            desktop.write(self.bridge / "heartbeat", heartbeat)
            with self.assertRaisesRegex(Conflict, "unavailable"):
                desktop.request_open(self.root, {})
        self.active()
        self.assertTrue(desktop.state(self.root)["opener_active"])
        desktop.prepare(self.root, self.workbook.as_posix())
        self.assertFalse(desktop.state(self.root)["opener_active"])

    def test_request_is_fixed_file_rate_limited_and_tracks_failure(self):
        self.active()
        with self.assertRaisesRegex(ValueError, "path or command"):
            desktop.request_open(self.root, {"path": "/another/file"})
        opened = desktop.request_open(self.root, {})
        identifier = opened["request_id"]
        self.assertEqual(desktop.request_status(self.root, identifier), {"status": "pending"})
        with self.assertRaisesRegex(Conflict, "wait a moment"):
            desktop.request_open(self.root, {})
        desktop.write(self.bridge / "response", f"{self.session} {identifier} failed")
        self.assertEqual(desktop.request_status(self.root, identifier), {"status": "failed"})
        self.assertEqual(self.workbook.read_bytes(), b"Synthetic workbook")

    def test_http_rejects_cross_origin_and_removed_download(self):
        self.active()
        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        server.directory, server.store = self.root, object()
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            def request(method, path, payload=None, origin=None):
                connection = HTTPConnection("127.0.0.1", server.server_port)
                headers = {"Content-Type": "application/json"}
                if origin:
                    headers["Origin"] = origin
                connection.request(method, path, None if payload is None else json.dumps(payload), headers)
                response = connection.getresponse()
                status, body = response.status, json.loads(response.read())
                connection.close()
                return status, body
            self.assertEqual(request("POST", "/api/workbook/open", {}, "http://untrusted.invalid")[0], 400)
            self.assertEqual(request("POST", "/api/workbook/open", {"command": "ignored"})[0], 400)
            status, body = request("POST", "/api/workbook/open", {})
            self.assertEqual(status, 200)
            self.assertEqual(request("GET", "/api/workbook/open?request_id="+body["request_id"])[1]["status"], "pending")
            self.assertEqual(request("GET", "/downloads/production-planning.xlsx")[0], 404)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    @unittest.skipUnless(shutil.which("bash"), "Bash is required for the desktop helper")
    def test_host_helper_opens_only_shared_file_and_retires_on_restart(self):
        script = Path(__file__).resolve().parents[2] / "scripts/workbook-opener.sh"
        harness = self.root / "watcher.sh"
        log = self.root / "opened.txt"
        harness.write_text('''#!/usr/bin/env bash
docker() { printf '%s\\n' true; }
uname() { printf '%s\\n' Linux; }
xdg-open() { printf '%s\\n' "$1" >> "$OPEN_LOG"; }
export -f docker uname xdg-open
bash "$OPENER_SCRIPT"
''', encoding="utf-8", newline="\n")
        env = dict(os.environ, APEX_DEMO_BRIDGE_DIR=self.bridge.as_posix(),
                   APEX_DEMO_WORKBOOK=self.workbook.as_posix(), APEX_DEMO_OPENER_SESSION=self.session,
                   APEX_DEMO_MES_CONTAINER="abc123", OPENER_SCRIPT=script.as_posix(), OPEN_LOG=log.as_posix())
        process = subprocess.Popen([shutil.which("bash"), harness.as_posix()], env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

        def wait_for(condition):
            deadline = time.monotonic() + 6
            while time.monotonic() < deadline:
                if condition():
                    return
                time.sleep(.1)
            self.fail("Desktop helper did not complete the expected operation")

        try:
            wait_for(lambda: desktop.state(self.root)["opener_active"])
            for _ in range(2):
                if (self.bridge / "request").exists():
                    os.utime(self.bridge / "request", (time.time()-3, time.time()-3))
                identifier = desktop.request_open(self.root, {})["request_id"]
                wait_for(lambda: desktop.request_status(self.root, identifier)["status"] == "launched")
            self.assertEqual(log.read_text().splitlines(), [self.workbook.as_posix()]*2)
            desktop.prepare(self.root, self.workbook.as_posix())
            stdout, stderr = process.communicate(timeout=6)
            self.assertEqual(process.returncode, 0, stdout+stderr)
        finally:
            if process.poll() is None:
                desktop.prepare(self.root, self.workbook.as_posix())
                process.communicate(timeout=6)


if __name__ == "__main__":
    unittest.main()
