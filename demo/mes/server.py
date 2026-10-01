"""Local MES server with explicit container binding and host allowlists."""
import argparse
import csv
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
from pathlib import Path
import shutil
import sqlite3
from urllib.parse import parse_qs, unquote, urlparse

from .catalog import CATALOG
from .seed import DEMO, generate
from .store import Conflict, Store
from .table_query import filter_rows

WEB = Path(__file__).parent / "web"


def prepare(directory):
    directory.mkdir(parents=True, exist_ok=True)
    seed_path = directory / "seed.json"
    if not seed_path.exists():
        seed_path.write_text(json.dumps(generate()), encoding="utf-8")
    seed = json.loads(seed_path.read_text(encoding="utf-8"))
    store = Store(directory / "mes.sqlite")
    store.seed(seed)
    workbook = directory / "production-planning.xlsx"
    source = DEMO / "planning/production-planning.xlsx"
    if source.exists() and not workbook.exists():
        shutil.copyfile(source, workbook)
    return store, seed


class Handler(BaseHTTPRequestHandler):
    server_version = "FactoryDemo/1.0"

    def send(self, status, content, content_type="application/json", headers=None):
        if content_type == "application/json":
            content = json.dumps(content, allow_nan=False).encode()
        elif isinstance(content, str):
            content = content.encode()
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(content)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Content-Security-Policy", "default-src 'self'; style-src 'self'; img-src 'self' data:; script-src 'self'; frame-ancestors 'none'; base-uri 'self'")
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(content)

    def guard(self):
        host = self.headers.get("Host", "")
        allowed = {f"127.0.0.1:{self.server.server_port}", f"localhost:{self.server.server_port}"}
        allowed.update(getattr(self.server, "allowed_hosts", ()))
        if host not in allowed:
            raise ValueError("Use a configured demo server address")
        origin = self.headers.get("Origin")
        if origin and origin != f"http://{host}":
            raise ValueError("Cross-origin requests are not permitted")

    def do_GET(self):
        try:
            self.guard()
            self.get()
        except (ValueError, KeyError) as error:
            self.send(400, {"error": str(error)})

    def get(self):
        parsed = urlparse(self.path)
        path = unquote(parsed.path)
        query = {key: value[0] for key, value in parse_qs(parsed.query).items()}
        store = self.server.store
        if path == "/api/meta":
            with store.connect() as db:
                counts = {key: db.execute("SELECT COUNT(*) FROM records WHERE entity=?", (key,)).fetchone()[0] for key in CATALOG}
                self.send(200, dict(factory=store.meta(db, "factory"), catalog=CATALOG, counts=counts,
                                    revision=store.meta(db, "revision"), workbook_available=(self.server.directory / "production-planning.xlsx").exists()))
            return
        if path in ("/api/plan", "/api/skills"):
            with store.connect() as db:
                key = "plan" if path.endswith("plan") else "qualifications"
                rows = store.meta(db, key)
                self.send(200, {"rows": rows, "source_revision": 0, "mes_revision": store.meta(db, "revision")})
            return
        if path == "/api/audit":
            with store.connect() as db:
                rows = [dict(row) for row in db.execute("SELECT * FROM audit ORDER BY id DESC LIMIT 50")]
                self.send(200, {"rows": rows})
            return
        if path.startswith("/api/tables/"):
            entity = path.removeprefix("/api/tables/")
            if entity not in CATALOG:
                raise ValueError("Unknown table")
            with store.connect() as db:
                rows = store.all(db, entity)
            rows, facets = filter_rows(entity, rows, query)
            sort = query.get("sort", "id")
            if sort not in {col["key"] for col in CATALOG[entity]["columns"] if col["type"] != "json"}:
                raise ValueError("Unknown sort field")
            rows.sort(key=lambda row: row.get(sort, ""), reverse=query.get("direction") == "desc")
            if query.get("format") == "csv":
                output = io.StringIO(newline="")
                fields = [col["key"] for col in CATALOG[entity]["columns"]]
                writer = csv.DictWriter(output, fieldnames=fields, extrasaction="ignore")
                writer.writeheader()
                for row in rows:
                    safe = {key: "'"+value if isinstance(value, str) and value.startswith(("=", "+", "-", "@", "\t", "\r")) else value for key, value in row.items()}
                    writer.writerow(safe)
                self.send(200, "\ufeff" + output.getvalue(), "text/csv; charset=utf-8", {"Content-Disposition": f'attachment; filename="{entity}.csv"'})
                return
            limit = min(200, max(1, int(query.get("limit", 50))))
            offset = max(0, int(query.get("offset", 0)))
            self.send(200, {"rows": rows[offset:offset+limit], "total": len(rows), "offset": offset, "limit": limit, **({"facets": facets} if query.get("facets") == "1" else {})})
            return
        if path == "/downloads/production-planning.xlsx":
            file = self.server.directory / "production-planning.xlsx"
            if not file.exists():
                self.send(404, {"error": "Planning workbook has not been installed"})
            else:
                self.send(200, file.read_bytes(), "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                          {"Content-Disposition": 'attachment; filename="Northstar-production-planning.xlsx"'})
            return
        allowed = {"/": ("index.html", "text/html; charset=utf-8"), "/app.js": ("app.js", "text/javascript"),
                   "/details.js": ("details.js", "text/javascript"),
                   **{f"/{name}": (name, "text/javascript") for name in ("filters.js", "production.js", "i18n.js", "locales/en.js", "locales/de.js")},
                   "/showcase.js": ("showcase.js", "text/javascript"),
                   "/showcase.css": ("showcase.css", "text/css"),
                   "/assets/qunevo-logo.svg": ("assets/qunevo-logo.svg", "image/svg+xml"),
                   "/assets/outfit-latin.woff2": ("assets/outfit-latin.woff2", "font/woff2"),
                   "/assets/bricolage-grotesque-latin.woff2": ("assets/bricolage-grotesque-latin.woff2", "font/woff2"),
                   "/styles.css": ("styles.css", "text/css"), "/icons.js": ("icons.js", "text/javascript")}
        if path in allowed:
            file, kind = allowed[path]
            self.send(200, (WEB / file).read_bytes(), kind)
        else:
            self.send(404, {"error": "Not found"})

    def do_POST(self):
        self.mutate()

    def do_PATCH(self):
        self.mutate()

    def mutate(self):
        try:
            self.guard()
            if self.headers.get("Content-Type", "").split(";")[0] != "application/json":
                raise ValueError("Send application/json")
            length = int(self.headers.get("Content-Length", 0))
            if not 0 < length <= 65536:
                raise ValueError("Request body must contain 1–65,536 bytes")
            payload = json.loads(self.rfile.read(length))
            if not isinstance(payload, dict):
                raise ValueError("Request body must be an object")
            path = urlparse(self.path).path
            parts = path.strip("/").split("/")
            if path == "/api/reset" and self.command == "POST":
                if payload.get("confirmation") != "RESET DEMO":
                    raise ValueError("Reset requires the exact confirmation text")
                self.server.store.seed(self.server.seed, reset=True)
                source = DEMO / "planning/production-planning.xlsx"
                if source.exists():
                    shutil.copyfile(source, self.server.directory / "production-planning.xlsx")
                result = {"reset": True}
            elif len(parts) == 4 and parts[:2] == ["api", "progress"] and parts[3] == "report" and self.command == "POST":
                result = self.server.store.report(parts[2], payload)
            elif len(parts) == 4 and parts[:2] == ["api", "workplans"] and self.command == "POST":
                result = self.server.store.routing_action(parts[2], parts[3], payload)
            elif path == "/api/clock" and self.command == "POST":
                result = self.server.store.advance_clock(payload)
            elif len(parts) in (3, 4) and parts[:2] == ["api", "tables"]:
                if (len(parts) == 3) != (self.command == "POST"):
                    raise ValueError("Use POST to create and PATCH to edit")
                result = self.server.store.change(parts[2], payload, parts[3] if len(parts) == 4 else None)
            else:
                self.send(404, {"error": "Not found"})
                return
            self.send(200, result)
        except Conflict as error:
            self.send(409, {"error": str(error)})
        except sqlite3.IntegrityError:
            self.send(409, {"error": "That ID already exists"})
        except (ValueError, KeyError, TypeError) as error:
            self.send(400, {"error": str(error)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8788)
    parser.add_argument("--bind", choices=("127.0.0.1", "0.0.0.0"), default="127.0.0.1")
    parser.add_argument("--allow-host", action="append", default=[], help="Additional exact HTTP Host value, including port")
    parser.add_argument("--data-dir", type=Path, default=DEMO / ".local")
    args = parser.parse_args()
    directory = args.data_dir.resolve()
    store, seed = prepare(directory)
    server = ThreadingHTTPServer((args.bind, args.port), Handler)
    server.allowed_hosts = set(args.allow_host)
    server.store, server.seed, server.directory = store, seed, directory
    print(f"Qunevo Demo MES: http://127.0.0.1:{server.server_port}", flush=True)
    print(f"Synthetic demo state: {directory}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
