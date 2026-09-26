"""Run under dbus-run-session; verifies an isolated desktop, single instance,
and graceful SIGTERM without starting restrictions or changing user data."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time


def windows_for(pid):
    windows = json.loads(subprocess.check_output(["niri", "msg", "-j", "windows"]))
    return [window for window in windows if window.get("pid") == pid]


executable = str(Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/deepify").resolve())
with tempfile.TemporaryDirectory(prefix="deepify-phase5-desktop-") as directory:
    root = Path(directory)
    runtime = root / "runtime"
    runtime.mkdir(mode=0o700)
    display = os.environ.get("WAYLAND_DISPLAY", "wayland-1")
    if not display.startswith("/"):
        display = str(Path(os.environ["XDG_RUNTIME_DIR"]) / display)
    environment = dict(os.environ, XDG_RUNTIME_DIR=str(runtime),
                       XDG_DATA_HOME=str(root / "data"),
                       XDG_CACHE_HOME=str(root / "cache"), XDG_CONFIG_HOME=str(root / "config"),
                       GSETTINGS_BACKEND="memory", WAYLAND_DISPLAY=display)
    with (root / "desktop.log").open("wb") as log:
        first = subprocess.Popen([executable], env=environment, stdout=log, stderr=log)
        second = None
        try:
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                assert first.poll() is None, "desktop exited during startup"
                windows = windows_for(first.pid)
                if windows:
                    break
                time.sleep(0.1)
            else:
                raise AssertionError("desktop window did not appear")
            assert all(w["app_id"] == "com.deepify.desktop" for w in windows)
            second = subprocess.Popen([executable], env=environment, stdout=log, stderr=log)
            assert second.wait(timeout=10) == 0, "second instance did not exit cleanly"
            assert len(windows_for(first.pid)) == len(windows)
            assert not windows_for(second.pid), "second instance created a window"
            first.send_signal(signal.SIGTERM)
            assert first.wait(timeout=10) == 0, "SIGTERM bypassed graceful exit handling"
            assert not windows_for(first.pid)
            print("PASS: exact desktop app ID, one instance, graceful SIGTERM (isolated data and D-Bus)")
        finally:
            for process in (second, first):
                if process is not None and process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
