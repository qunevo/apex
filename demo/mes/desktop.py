"""Exchange fixed workbook-open requests with the local demo starter."""
import argparse
import os
from pathlib import Path
import re
import secrets
import tempfile
import threading
import time

from .store import Conflict

LOCK = threading.Lock()
IDENTIFIER = re.compile(r"[a-f0-9]{32}")
NAME = "production-planning.xlsx"
UNAVAILABLE = "The desktop opener is unavailable. Run the demo starter again without --no-open."


def bridge(directory):
    path = Path(directory) / ".desktop"
    if path.is_symlink():
        raise Conflict("The desktop opener directory is unsafe.")
    return path


def read(path):
    if path.is_symlink():
        raise Conflict("The desktop opener file is unsafe.")
    try:
        return path.read_text(encoding="utf-8").strip()
    except FileNotFoundError:
        return ""


def write(path, value):
    fd, temporary = tempfile.mkstemp(prefix=".desktop-", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as output:
            output.write(value + "\n")
        os.chmod(temporary, 0o644)
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


def prepare(directory, host_path):
    if not host_path or any(ord(c) < 32 for c in host_path):
        raise ValueError("Invalid workbook path")
    path = bridge(directory)
    path.mkdir(mode=0o755, exist_ok=True)
    session = secrets.token_hex(16)
    write(path / "workbook-path", host_path)
    # A new generation retires any earlier helper without killing unrelated processes.
    write(path / "session", session)
    return session


def state(directory):
    path = bridge(directory)
    session = read(path / "session")
    heartbeat = read(path / "heartbeat").split()
    active = False
    if IDENTIFIER.fullmatch(session) and len(heartbeat) == 2 and heartbeat[0] == session:
        try:
            active = 0 <= time.time() - int(heartbeat[1]) <= 15
        except ValueError:
            pass
    return {"available": (Path(directory) / NAME).is_file(), "opener_active": active,
            "host_path": read(path / "workbook-path")}


def request_open(directory, payload):
    if payload:
        raise ValueError("Workbook open does not accept a path or command.")
    with LOCK:
        path = bridge(directory)
        status = state(directory)
        if not status["available"]:
            raise Conflict("Planning workbook has not been installed")
        if not status["opener_active"]:
            raise Conflict(UNAVAILABLE)
        request = path / "request"
        if request.exists() and time.time() - request.stat().st_mtime < 2:
            raise Conflict("An Excel open request was just sent. Please wait a moment.")
        identifier = secrets.token_hex(16)
        write(request, f"{read(path / 'session')} {identifier}")
        return {"request_id": identifier, "status": "pending"}


def request_status(directory, identifier):
    if not IDENTIFIER.fullmatch(identifier):
        raise ValueError("Invalid workbook open request")
    path = bridge(directory)
    session = read(path / "session")
    response = read(path / "response").split()
    if len(response) == 3 and response[:2] == [session, identifier]:
        if response[2] in ("launched", "failed"):
            return {"status": response[2]}
    if not state(directory)["opener_active"] or read(path / "request") != f"{session} {identifier}":
        return {"status": "unavailable"}
    return {"status": "pending"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=Path("/var/lib/demo"))
    parser.add_argument("--host-path", required=True)
    args = parser.parse_args()
    print(prepare(args.directory, args.host_path))


if __name__ == "__main__":
    main()
