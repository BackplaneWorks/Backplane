#!/usr/bin/env python3
"""Start an extracted Linux desktop release in an isolated temporary home."""

from __future__ import annotations

import argparse
import ctypes
import os
import re
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path


BASE_URL_RE = re.compile(r"baseUrl: http://127\.0\.0\.1:(\d+)/")


def set_parent_death_signal() -> None:
    if sys.platform == "linux":
        ctypes.CDLL(None).prctl(1, signal.SIGTERM)


def process_group_ids(root_pid: int) -> set[int]:
    """Capture groups belonging to the app and descendants before reparenting."""
    processes: dict[int, tuple[int, int]] = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            fields = (entry / "stat").read_text().split(") ", 1)[1].split()
            processes[int(entry.name)] = (int(fields[1]), int(fields[2]))
        except (FileNotFoundError, IndexError, ValueError):
            continue
    owned = {root_pid}
    changed = True
    while changed:
        changed = False
        for pid, (parent, _group) in processes.items():
            if parent in owned and pid not in owned:
                owned.add(pid)
                changed = True
    return {processes[pid][1] for pid in owned if pid in processes}


def terminate_process_group(process: subprocess.Popen[bytes], groups: set[int]) -> None:
    groups.add(process.pid)
    for group in groups:
        try:
            os.killpg(group, signal.SIGTERM)
        except ProcessLookupError:
            pass
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        pass
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        if all(not _process_group_exists(group) for group in groups):
            return
        time.sleep(0.1)
    for group in groups:
        try:
            os.killpg(group, signal.SIGKILL)
        except ProcessLookupError:
            pass
    try:
        process.wait(timeout=8)
    except subprocess.TimeoutExpired:
        pass


def _process_group_exists(group: int) -> bool:
    try:
        os.killpg(group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def read_log(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def fetch_app(base_url: str) -> tuple[int, str]:
    with urllib.request.urlopen(base_url, timeout=5) as response:
        return response.status, response.read().decode("utf-8", errors="replace")


def run_smoke(app_root: Path, timeout_seconds: float, headless: bool) -> int:
    app_root = app_root.resolve()
    executable = app_root / "backplane"
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise RuntimeError(f"packaged executable is missing or not executable: {executable}")

    with tempfile.TemporaryDirectory(prefix="backplane-release-smoke-") as temporary:
        root = Path(temporary)
        home = root / "home"
        user_data = root / "user-data"
        xdg_config = root / "xdg-config"
        xdg_data = root / "xdg-data"
        xdg_cache = root / "xdg-cache"
        xdg_state = root / "xdg-state"
        xdg_runtime = root / "xdg-runtime"
        for directory in (home, user_data, xdg_config, xdg_data, xdg_cache, xdg_state, xdg_runtime):
            directory.mkdir(parents=True)
        xdg_runtime.chmod(0o700)
        log_path = root / "desktop.log"

        environment = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": str(home),
            "T3CODE_HOME": str(home),
            "T3CODE_MODE": "desktop",
            "XDG_CONFIG_HOME": str(xdg_config),
            "XDG_DATA_HOME": str(xdg_data),
            "XDG_CACHE_HOME": str(xdg_cache),
            "XDG_STATE_HOME": str(xdg_state),
            "XDG_RUNTIME_DIR": str(xdg_runtime),
        }
        for name in ("DISPLAY", "XAUTHORITY"):
            value = os.environ.get(name)
            if value:
                environment[name] = value

        command = [
            str(executable),
            "--no-sandbox",
            "--disable-gpu",
            f"--user-data-dir={user_data}",
        ]
        if headless:
            command.append("--headless")

        with log_path.open("wb") as log_file:
            process = subprocess.Popen(
                command,
                cwd=app_root,
                env=environment,
                stdout=log_file,
                stderr=subprocess.STDOUT,
                start_new_session=True,
                preexec_fn=set_parent_death_signal if sys.platform == "linux" else None,
            )
        owned_groups = {process.pid}
        try:
            deadline = time.monotonic() + timeout_seconds
            base_url: str | None = None
            while time.monotonic() < deadline:
                owned_groups.update(process_group_ids(process.pid))
                log = read_log(log_path)
                match = BASE_URL_RE.search(log)
                if match:
                    base_url = f"http://127.0.0.1:{match.group(1)}/"
                if "backend ready" in log and "main window created" in log and base_url:
                    try:
                        status, body = fetch_app(base_url)
                    except (urllib.error.URLError, TimeoutError, ConnectionError):
                        status, body = 0, ""
                    if status == 200 and "Backplane" in body:
                        print(f"desktop release smoke passed: {base_url}")
                        return 0
                if process.poll() is not None:
                    break
                time.sleep(0.25)
            tail = read_log(log_path)[-8000:]
            raise RuntimeError(f"desktop release smoke timed out or failed; log: {log_path}\n{tail}")
        finally:
            owned_groups.update(process_group_ids(process.pid))
            terminate_process_group(process, owned_groups)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app_root", type=Path, help="extracted AppImage squashfs-root directory")
    parser.add_argument("--timeout", type=float, default=60, help="startup timeout in seconds")
    parser.add_argument("--headless", action="store_true", help="pass Electron's headless flag")
    args = parser.parse_args()
    try:
        return run_smoke(args.app_root, args.timeout, args.headless)
    except (OSError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
