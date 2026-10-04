"""Restore the shared workbook with the MES transaction, without truncating it."""
from pathlib import Path
import logging
import os
import shutil
import sqlite3
import tempfile
import threading

from .store import Conflict

RESET_LOCK = threading.Lock()
NAME = "production-planning.xlsx"


def ensure_closed(directory):
    # Excel's owner file survives across the Docker Desktop directory mount.
    names = (f"~${NAME}", f"~${NAME[2:]}", f".~lock.{NAME}#")
    if any((directory / name).exists() for name in names):
        raise Conflict("Close the shared Excel workbook before resetting the demo, then try again.")


def reset_demo(store, seed, directory, baseline):
    directory, baseline = Path(directory), Path(baseline)
    workbook = directory / NAME
    with RESET_LOCK:
        ensure_closed(directory)
        if not baseline.is_file() or baseline.is_symlink() or workbook.is_symlink():
            raise Conflict("The workbook baseline is unavailable or the working file is unsafe. The demo was not reset.")
        staged = backup = None
        installed = False
        keep_backup = False
        try:
            def temporary():
                fd, name = tempfile.mkstemp(prefix=".workbook-reset-", dir=directory)
                os.close(fd)
                return Path(name)

            staged = temporary()
            shutil.copy2(baseline, staged)
            if workbook.exists():
                backup = temporary()
                shutil.copy2(workbook, backup)

            def replace_workbook():
                nonlocal installed
                ensure_closed(directory)
                os.replace(staged, workbook)
                installed = True

            try:
                store.seed(seed, reset=True, before_commit=replace_workbook)
            except Exception:
                if installed:
                    try:
                        if backup is not None:
                            os.replace(backup, workbook)
                        else:
                            workbook.unlink()
                    except OSError as error:
                        keep_backup = True
                        raise Conflict("Reset failed and workbook recovery could not finish. Close Excel and restore any .workbook-reset- backup before continuing.") from error
                raise
        except (OSError, sqlite3.Error) as error:
            raise Conflict("Could not reset the shared workbook and MES. Close Excel and check that the working directory is writable, then try again.") from error
        finally:
            for path in (staged, None if keep_backup else backup):
                if path is not None:
                    try:
                        path.unlink(missing_ok=True)
                    except OSError:
                        # Cleanup must not turn a committed reset into an API failure.
                        logging.getLogger(__name__).warning("Could not remove reset temporary file: %s", path.name)
    return {"reset": True, "workbook_reset": True}
