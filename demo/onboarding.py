"""Render a private local setup page using connection details from stdin."""
import argparse
from html import escape
import json
from pathlib import Path
import re
import shlex
import sys
from urllib.parse import urlsplit


def http_url(value):
    parsed = urlsplit(value)
    if (parsed.scheme not in ("http", "https") or not parsed.hostname
            or parsed.username or parsed.password or parsed.query or parsed.fragment
            or any(c.isspace() or ord(c) < 32 for c in value)):
        raise ValueError("Expected an HTTP address without credentials, query or fragment")
    return value


def render(connection, mes_url, workbook, demo_directory, compose_project):
    fields = {}
    for line in connection.splitlines():
        key, separator, value = line.strip().partition(":")
        if separator and key in ("MCP URL", "HTTP header value"):
            if key in fields:
                raise ValueError("Duplicate connection field")
            fields[key] = value.strip()
    mcp_url = http_url(fields.get("MCP URL", ""))
    authorization = fields.get("HTTP header value", "")
    if not re.fullmatch(r"Bearer apx_[a-f0-9]{64}", authorization):
        raise ValueError("Missing or invalid agent authorization")
    mes_url = http_url(mes_url)
    if not workbook or any(ord(c) < 32 for c in workbook):
        raise ValueError("Invalid workbook path")
    if not demo_directory or any(ord(c) < 32 for c in demo_directory):
        raise ValueError("Invalid demo directory")
    if not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", compose_project):
        raise ValueError("Invalid Compose project name")
    stop_command = (f"cd -- {shlex.quote(demo_directory)} && "
                    f"docker compose --project-name {shlex.quote(compose_project)} down")
    details = f"Transport: Streamable HTTP\nMCP URL: {mcp_url}\nAuthorization: {authorization}"
    factory = json.loads((Path(__file__).parent / "data/factory.json").read_text(encoding="utf-8"))
    snapshot = f"{factory['as_of']} {factory['timezone']}"
    time_context = (
        f"Frozen demo snapshot: {snapshot}. Planning starts at {factory['plan_start']}; "
        f"the source planning period ends at {factory['horizon_end']}. "
        "For this demo, today and now always mean the frozen snapshot, never the real "
        "computer date, chat date or import time. Keep all order, receipt, calendar and "
        "Excel dates unchanged. Check factory.as_of and the saved scenario's time basis "
        "before reporting; flag a different snapshot instead of silently rebasing it. "
        "Distinguish unfinished work already overdue at the snapshot from predicted "
        "lateness in a validated plan (completion minus due date). Do not advance the "
        "demo clock or shift the planning period."
    )
    instructions = (
        "Set up this running APEX demo in my local chat client.\n\n"
        f"{details}\n\nMES: {mes_url}\nShared Excel workbook: {workbook}\n\n"
        f"{time_context}\n\n"
        "Configure the HTTP MCP connection locally, preserving other client settings. "
        "Keep the authorization private and out of Git. Reuse this existing token; "
        "do not reset or restart the demo to connect another chat. "
        "Verify the connection by listing the APEX engines. If the client must reconnect "
        "or reload tools, explain that remaining step instead of claiming it is connected.\n\n"
        "Use local filesystem tools to read the saved workbook at the path above, "
        "and HTTP tools for the MES. Use the optional adapter in "
        "app/customization/demo/adapter to export a canonical input and source report, "
        "then explicitly import the scenario and request views.get for the MCP App. "
        "APEX MCP itself does not expose the raw source files. Do not use the MES preview "
        "as current Excel data or assume that sources have already been imported. "
        "Ask before modifying source data or resetting the demo."
    )
    values = {"mes_url": mes_url, "workbook": workbook, "details": details,
              "instructions": instructions, "stop_command": stop_command, "snapshot": snapshot}
    template = Path(__file__).with_name("onboarding.html").read_text(encoding="utf-8")
    return re.sub(r"\{\{(\w+)\}\}", lambda m: escape(values[m[1]], quote=True), template)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mes-url", required=True)
    parser.add_argument("--workbook", required=True)
    parser.add_argument("--demo-directory", required=True)
    parser.add_argument("--compose-project", required=True)
    args = parser.parse_args()
    try:
        page = render(sys.stdin.read(), args.mes_url, args.workbook,
                      args.demo_directory, args.compose_project)
    except ValueError as error:
        parser.exit(1, f"Could not prepare the setup page: {error}\n")
    sys.stdout.buffer.write(page.encode("utf-8"))


if __name__ == "__main__":
    main()
