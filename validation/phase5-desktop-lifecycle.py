"""Actual Tauri/WebKit lifecycle validation in disposable data/private D-Bus.

Uses a protocol browser peer (real Firefox/Zen coverage is in the production
browser harness), real read-only Niri IPC, and the real Noctalia DND adapter.
The Niri command wrapper refuses ALL close/focus actions. No system sleep or
shutdown is performed: login1 signals are emitted on the private test bus.
Run: nix develop -c dbus-run-session -- python3 validation/phase5-desktop-lifecycle.py
"""
import json
import base64
import os
import re
from pathlib import Path
import secrets
import shutil
import signal
import socket
import sqlite3
import struct
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request

REPO = Path(__file__).resolve().parents[1]


def wait_for(label, check, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        time.sleep(0.05)
    raise AssertionError(f"timed out: {label}")


class BrowserPeer:
    def __init__(self, path, token):
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.connect(str(path))
        self.active = None
        self.timer = "inactive"
        self.error = None
        self.connected = True
        self.stops = []
        self.respond_heartbeat = True
        self.send("hello", token=token, extension_id="focus@deepify.local",
                  browser_kind="firefox", profile_label="Disposable lifecycle fixture",
                  capabilities=["top_level_web_request", "tab_restore", "container_tabs"])
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()

    def send(self, kind, request_id=None, **payload):
        value = dict(version=1, type=kind, message_id=secrets.token_hex(12), **payload)
        if request_id:
            value["request_id"] = request_id
        data = json.dumps(value).encode()
        self.socket.sendall(struct.pack("<I", len(data)) + data)

    def read(self, size):
        data = b""
        while len(data) < size:
            part = self.socket.recv(size - len(data))
            if not part:
                raise EOFError()
            data += part
        return data

    def run(self):
        try:
            while True:
                length, = struct.unpack("<I", self.read(4))
                assert length <= 256 * 1024
                message = json.loads(self.read(length))
                kind, correlation = message["type"], message["message_id"]
                if kind == "status":
                    payload = dict(health="active" if self.active else "healthy_idle",
                                   timer_state=self.timer, remaining_seconds=30 if self.active else 0)
                    if self.active:
                        payload["session_id"] = self.active
                    self.send("state", correlation, **payload)
                elif kind == "heartbeat":
                    if self.respond_heartbeat:
                        self.send("heartbeat_ack", correlation)
                elif kind == "start_session":
                    self.active, self.timer = message["session_id"], "working"
                    self.send("start_result", correlation, session_id=self.active, accepted=True, blocked_count=0)
                elif kind == "state":
                    self.timer = message["timer_state"]
                elif kind == "stop_session":
                    self.stops.append(message["session_id"])
                    self.active, self.timer = None, "inactive"
                    self.send("stop_result", correlation, session_id=message["session_id"], accepted=True, restored_count=0)
        except (EOFError, OSError):
            pass
        except Exception as error:
            self.error = error
        finally:
            self.connected = False
            self.active = None

    def close(self):
        try:
            self.socket.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.socket.close()
        self.thread.join(timeout=2)
        assert self.error is None, str(self.error)


class AudioServer:
    """Owns only the temporary PipeWire socket and a policy-only session manager."""
    def __init__(self, root, environment):
        self.root = root
        self.environment = environment
        self.processes = []
        self.log = (root / "audio-server.log").open("wb")

    def start(self, label):
        config = (REPO / "validation/phase5-pipewire.conf").read_text()
        path = self.root / "pipewire.conf"
        path.write_text(config.replace("deepify-validation-output", f"deepify-validation-{label}"))
        self.processes.append(subprocess.Popen(["pipewire", "-c", str(path)], env=self.environment,
                                              stdout=self.log, stderr=self.log))
        wait_for("private audio socket", lambda: (self.root / "runtime/pipewire-0").exists())
        self.processes.append(subprocess.Popen(["wireplumber", "--profile", "policy"], env=self.environment,
                                              stdout=self.log, stderr=self.log))
        time.sleep(0.5)
        assert all(p.poll() is None for p in self.processes), (self.root / "audio-server.log").read_text()

    def stop(self):
        for process in reversed(self.processes):
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)
        self.processes.clear()

    def close(self):
        self.stop()
        self.log.close()


def main():
    original = dict(os.environ)
    niri = shutil.which("niri")
    noctalia = shutil.which("noctalia-shell")
    webdriver = shutil.which("WebKitWebDriver")
    if not webdriver:
        # The pinned dev shell's WebKit package may expose only libraries on PATH.
        candidates = list(Path("/nix/store").glob("*webkitgtk-2.52.5+abi=4.1/bin/WebKitWebDriver"))
        assert len(candidates) == 1, "Set PATH to the pinned WebKitWebDriver"
        webdriver = str(candidates[0])
    assert niri and noctalia
    # Restoring the real runtime is enough for Noctalia's local IPC. The private
    # session bus belongs to this harness, not to the user's desktop.
    def dnd(value=None):
        if value is None:
            return json.loads(subprocess.check_output([noctalia, "ipc", "call", "state", "all"], env=original))["state"]["doNotDisturb"]
        subprocess.check_call([noctalia, "ipc", "call", "notifications", "enableDND" if value else "disableDND"], env=original, stdout=subprocess.DEVNULL)

    prior_dnd = dnd()
    with tempfile.TemporaryDirectory(prefix="deepify-desktop-lifecycle-", dir="/tmp") as directory:
        root = Path(directory)
        runtime = root / "runtime"
        runtime.mkdir(mode=0o700)
        bindir = root / "bin"
        bindir.mkdir()
        wrapper = """#!{python}
import os, sys
args = sys.argv[1:]
name = os.path.basename(sys.argv[0])
if name == 'niri':
    if args not in [['msg','-j','windows'], ['msg','-j','event-stream']]:
        raise SystemExit('Validation refuses all window mutations')
    os.execv({niri!r}, [{niri!r}, *args])
for key, value in {noctalia_environment!r}.items():
    if value is None: os.environ.pop(key, None)
    else: os.environ[key] = value
os.execv({noctalia!r}, [{noctalia!r}, *args])
""".format(python=shutil.which("python3"), niri=niri, noctalia=noctalia, noctalia_environment={key: original.get(key) for key in ("XDG_RUNTIME_DIR", "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "DBUS_SYSTEM_BUS_ADDRESS", "WAYLAND_DISPLAY")})
        for name in ("niri", "noctalia-shell"):
            path = bindir / name
            path.write_text(wrapper)
            path.chmod(0o700)
        display = original.get("WAYLAND_DISPLAY", "wayland-1")
        if not display.startswith("/"):
            display = str(Path(original["XDG_RUNTIME_DIR"]) / display)
        environment = dict(original, XDG_RUNTIME_DIR=str(runtime), XDG_DATA_HOME=str(root / "data"),
                           XDG_CACHE_HOME=str(root / "cache"), XDG_CONFIG_HOME=str(root / "config"),
                           GSETTINGS_BACKEND="memory", WAYLAND_DISPLAY=display,
                           TAURI_WEBVIEW_AUTOMATION="true", PATH=f"{bindir}:{original['PATH']}",
                           DBUS_SYSTEM_BUS_ADDRESS=original["DBUS_SESSION_BUS_ADDRESS"])
        probe = subprocess.run([str(bindir / "noctalia-shell"), "ipc", "call", "state", "all"],
                               env=environment, capture_output=True, text=True)
        assert probe.returncode == 0, f"Noctalia test wrapper failed ({probe.returncode}): {probe.stderr or probe.stdout}"
        audio_server = None
        if os.environ.get("DEEPIFY_AUDIO_VALIDATION") == "1":
            # Limit ALSA to this private PipeWire output; Rodio cannot fall back
            # to a physical device or the user's live audio server.
            alsa_module = re.search(r"libs.native = ([^;]+)", Path("/etc/alsa/conf.d/49-pipewire-modules.conf").read_text()).group(1).strip()
            alsa = root / "alsa.conf"
            alsa.write_text('pcm_type.pipewire { lib "' + alsa_module + '" }\npcm.!default { type pipewire server "pipewire-0" playback_node "-1" capture_node "-1" hint { show on description "Disposable output" } }\n')
            environment.update(ALSA_CONFIG_PATH=str(alsa), PIPEWIRE_RUNTIME_DIR=str(runtime),
                               XDG_STATE_HOME=str(root / "state"))
            audio_server = AudioServer(root, environment)
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        bus_lines = []
        bus = subprocess.Popen([str(REPO / "target/debug/examples/phase5_bus")],
                               env=dict(original, DEEPIFY_PRIVATE_BUS_FIXTURE="1"),
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        threading.Thread(target=lambda: bus_lines.extend(iter(bus.stdout.readline, "")), daemon=True).start()
        driver_log = (root / "driver.log").open("wb")
        driver = subprocess.Popen([webdriver, f"--port={port}"], env=environment, stdout=driver_log, stderr=driver_log)
        session = None
        peer = None
        token = secrets.token_urlsafe(32)
        dbpath = root / "data/com.deepify.desktop/deepify.sqlite"

        def request(method, path, body=None):
            req = urllib.request.Request(f"http://127.0.0.1:{port}{path}",
                data=json.dumps(body).encode() if body is not None else None, method=method,
                headers={"Content-Type": "application/json"})
            try:
                with urllib.request.urlopen(req, timeout=30) as response:
                    return json.load(response)["value"]
            except urllib.error.HTTPError as error:
                raise RuntimeError(error.read().decode()) from error

        def script(source, args=None, asynchronous=False):
            return request("POST", f"/session/{session}/execute/{'async' if asynchronous else 'sync'}",
                           {"script": source, "args": args or []})

        def invoke(command, args=None, reject=False):
            result = script("const [command,args,done]=arguments; window.__TAURI_INTERNALS__.invoke(command,args).then(value=>done({value})).catch(error=>done({error:String(error)}));", [command, args or {}], True)
            if reject:
                assert "error" in result, f"{command} unexpectedly succeeded"
                return result["error"]
            assert "error" not in result, f"{command}: {result.get('error')}"
            return result["value"]

        captured = set()
        def capture(name):
            destination = os.environ.get("DEEPIFY_SCREENSHOTS")
            if not destination or name in captured:
                return
            time.sleep(0.2)
            directory = Path(destination)
            directory.mkdir(parents=True, exist_ok=True)
            (directory / name).write_bytes(base64.b64decode(request("GET", f"/session/{session}/screenshot")))
            captured.add(name)

        def launch():
            nonlocal session, peer
            started = time.monotonic()
            data = request("POST", "/session", {"capabilities": {"alwaysMatch": {
                "webkitgtk:browserOptions": {"binary": os.environ.get("DEEPIFY_DESKTOP_BIN", str(REPO / "target/debug/deepify"))}}}})
            session = data["sessionId"]
            wait_for("desktop IPC", lambda: script("return location.href !== 'about:blank' && document.readyState === 'complete' && Boolean(window.__TAURI_INTERNALS__)"))
            print(f"MEASURED: desktop startup to loaded IPC {(time.monotonic()-started)*1000:.0f} ms", flush=True)
            capture("01-setup.png")
            peer = BrowserPeer(runtime / "deepify/browser-v1.sock", token)
            value = wait_for("pairing", lambda: (v if any(h["detail"].startswith("Pairing required") or h["detail"].startswith("Paired profile connected") for h in v["health"]) else None) if (v := invoke("app_snapshot")) else None)
            if any(h["detail"].startswith("Pairing required") for h in value["health"]):
                invoke("browser_accept_pairing")
            invoke("repair_integrations")
            # Only disposable configuration is changed. The wrapper still denies
            # every close/focus action, including any unexpected new user window.
            for app_id in invoke("resolve_blocked_apps")["blockedApps"]:
                invoke("add_whitelist", {"kind": "application", "value": app_id})
            health = invoke("app_snapshot")["health"]
            assert all(h["status"] == "healthy" for h in health), health
            invoke("complete_setup")

        def finished(reason):
            value = wait_for(reason, lambda: (s if s["summary"] else None) if (s := invoke("app_snapshot")) else None, timeout=35)
            assert value["session"]["state"] == "not_working"
            assert value["summary"]["reason"] == reason, value["summary"]
            assert value["summary"]["cleanupComplete"], value["summary"]
            assert dnd() == prior_dnd, "DND was not restored"
            assert value["history"][0]["reason"] == reason, (value["summary"], value["history"])
            return value

        def start(seconds=60):
            invoke("dismiss_summary")
            value = invoke("session_start", {"seconds": seconds})
            assert value["session"]["state"] == "working"
            assert dnd() is True

        def app_pid():
            windows = json.loads(subprocess.check_output([niri, "msg", "-j", "windows"]))
            pids = {w["pid"] for w in windows if w["app_id"] == "com.deepify.desktop"}
            for pid in pids:
                try:
                    data = Path(f"/proc/{pid}/environ").read_bytes().split(b"\0")
                    if f"XDG_DATA_HOME={root / 'data'}".encode() in data:
                        return pid
                except FileNotFoundError:
                    pass
            raise AssertionError("isolated desktop PID not found")

        def end_driver_session():
            nonlocal session, peer
            if peer:
                peer.close()
                peer = None
            if session:
                try:
                    request("DELETE", f"/session/{session}")
                except (RuntimeError, OSError):
                    pass
                session = None

        def descendants(pid):
            result = {pid}
            for child in Path(f"/proc/{pid}/task/{pid}/children").read_text().split():
                try:
                    result.update(descendants(int(child)))
                except FileNotFoundError:
                    pass
            return result

        def measure(label):
            pids = descendants(app_pid())
            def usage():
                ticks, rss = {}, 0
                for pid in pids:
                    try:
                        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
                        ticks[pid] = int(fields[11]) + int(fields[12])
                        rss += int(fields[21]) * os.sysconf("SC_PAGE_SIZE")
                    except FileNotFoundError:
                        pass
                return ticks, rss
            before, _ = usage()
            began = time.monotonic()
            time.sleep(5)
            after, rss = usage()
            cpu = sum(value-before.get(pid, value) for pid, value in after.items()) / os.sysconf("SC_CLK_TCK") / (time.monotonic()-began) * 100
            latencies = []
            for _ in range(10):
                began = time.monotonic()
                invoke("app_snapshot")
                latencies.append((time.monotonic()-began)*1000)
            print(f"MEASURED: {label}; persistent desktop tree CPU {cpu:.2f}% of one core; summed RSS {rss/1024/1024:.1f} MiB; 10 snapshot round trips median {sorted(latencies)[5]:.1f} ms, max {max(latencies):.1f} ms", flush=True)

        try:
            wait_for("private bus", lambda: "PHASE5_BUS_READY\n" in bus_lines)
            time.sleep(0.3)
            launch()
            capture("02-idle-obsidian.png")
            if os.environ.get("DEEPIFY_PERFORMANCE_VALIDATION") == "1":
                time.sleep(5)
                measure("idle, empty queue")
            if os.environ.get("DEEPIFY_PORTAL_VALIDATION") == "1":
                before = {w["id"] for w in json.loads(subprocess.check_output([niri, "msg", "-j", "windows"]))}
                script("window.__chooserDone=false; window.__chooserError=null; window.__TAURI_INTERNALS__.invoke('import_music_files').catch(error=>{window.__chooserError=String(error)}).finally(()=>{window.__chooserDone=true}); return true;")
                def private_dialogs():
                    windows = json.loads(subprocess.check_output([niri, "msg", "-j", "windows"]))
                    result = []
                    for window in windows:
                        if window["id"] in before or not window.get("pid"):
                            continue
                        try:
                            environment_entries = Path(f"/proc/{window['pid']}/environ").read_bytes().split(b"\0")
                        except FileNotFoundError:
                            continue
                        if f"DBUS_SESSION_BUS_ADDRESS={original['DBUS_SESSION_BUS_ADDRESS']}".encode() in environment_entries:
                            result.append(window)
                    return result
                try:
                    dialogs = wait_for("isolated file chooser", private_dialogs)
                except AssertionError:
                    windows = json.loads(subprocess.check_output([niri, "msg", "-j", "windows"]))
                    print("File chooser diagnostic:",
                          [{key: w.get(key) for key in ("id", "pid", "app_id")} for w in windows if w["id"] not in before], flush=True)
                    print((root / "driver.log").read_text(), flush=True)
                    raise
                print("OBSERVED: isolated file chooser app IDs", sorted({w["app_id"] for w in dialogs}), flush=True)
                capture("08-file-chooser-parent.png")
                for dialog in dialogs:
                    # Guard again immediately before closing only a new private-bus dialog.
                    assert dialog["id"] in {w["id"] for w in private_dialogs()}
                    subprocess.check_call([niri, "msg", "action", "close-window", "--id", str(dialog["id"])], stdout=subprocess.DEVNULL)
                wait_for("file chooser cancellation", lambda: script("return window.__chooserDone"))

            if audio_server:
                audio_server.start("first")
                music = root / "music"
                music.mkdir()
                track = music / "Silent fixture.mp3"
                subprocess.check_call(["ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=stereo", "-t", "30", "-codec:a", "libmp3lame", str(track)])
                with sqlite3.connect(dbpath) as db:
                    db.execute("INSERT INTO music_sources(id,kind,path,created_at) VALUES(?,?,?,?)",
                               ("audio-folder", "folder", str(music), 1))
                assert len(invoke("refresh_music_library")["queue"]) == 1
                start()
                playing = invoke("audio_toggle")
                assert playing["playback"]["playing"], playing["playback"]
                time.sleep(3.5)
                assert invoke("app_snapshot")["playback"]["playing"], "healthy output stalled before removal"
                audio_server.stop()
                lost = wait_for("real output loss", lambda: s if (s := invoke("app_snapshot"))["playback"]["health"] == "waiting_for_output_device" else None)
                assert lost["session"]["state"] == "working"
                assert not lost["playback"]["playing"]
                audio_server.start("replacement")
                recovered_audio = invoke("audio_retry_output")
                assert recovered_audio["playback"]["health"] == "ready" and recovered_audio["playback"]["playing"], recovered_audio["playback"]
                assert recovered_audio["session"]["state"] == "working"
                time.sleep(3.5)
                assert invoke("app_snapshot")["playback"]["playing"], "replacement output stalled"
                invoke("audio_toggle")
                track.unlink()
                removed = wait_for("paused track removal", lambda: s if not (s := invoke("app_snapshot"))["queue"] else None)
                assert removed["playback"]["currentIndex"] is None
                assert removed["playback"]["health"] == "missing_file"
                assert not invoke("audio_toggle")["playback"]["playing"]
                invoke("session_end")
                finished("ended_early")
                print("PASS: real private PipeWire loss/replacement/retry, focus unaffected, paused-file removal", flush=True)
            invoke("save_setting", {"key": "theme", "value": "mist"})
            capture("03-idle-mist.png")
            start()
            if os.environ.get("DEEPIFY_PERFORMANCE_VALIDATION") == "1":
                measure("working, current queue")
            capture("04-working.png")
            paused = invoke("session_pause")
            assert paused["session"]["restrictionsActive"]
            capture("05-paused.png")
            for command, args in [("rerun_setup", {}), ("add_whitelist", {"kind": "application", "value": "fixture"}),
                                  ("browser_forget_pairing", {})]:
                assert "conflict" in invoke(command, args, reject=True).lower()
            time.sleep(1.1)
            assert invoke("app_snapshot")["session"]["focusedSeconds"] == paused["session"]["focusedSeconds"]
            invoke("session_end")
            finished("ended_early")
            capture("06-summary.png")
            print("PASS: real desktop start/pause/guards/early-end/persisted cleanup", flush=True)
            start(2)
            finished("completed")
            wait_for("completion notification", lambda: "PHASE5_NOTIFICATION\n" in bus_lines)
            print("PASS: real timer completion and notification delivery on private bus", flush=True)
            for event in ("sleep", "shutdown"):
                start()
                bus.stdin.write(event + "\n")
                bus.stdin.flush()
                finished("interrupted")
                print(f"PASS: injected login1 {event} through actual desktop handler", flush=True)
            start()
            peer.close()
            peer = None
            failed = wait_for("browser disconnect", lambda: (s if s["summary"] else None) if (s := invoke("app_snapshot")) else None)
            assert failed["summary"]["reason"] == "extension_or_app_crash"
            assert "browser" in failed["session"]["latestNotice"]
            capture("07-failure.png")
            assert dnd() == prior_dnd
            peer = BrowserPeer(runtime / "deepify/browser-v1.sock", token)
            time.sleep(0.15)
            invoke("repair_integrations")
            finished("extension_or_app_crash")
            print("PASS: desktop disconnect interruption, visible failure, cleanup retry", flush=True)
            if os.environ.get("DEEPIFY_FAILURE_VALIDATION") == "1":
                start()
                monitors = []
                for pid in descendants(app_pid()):
                    try:
                        arguments = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
                    except FileNotFoundError:
                        continue
                    if arguments[-4:] == [b"msg", b"-j", b"event-stream", b""]:
                        monitors.append(pid)
                assert len(monitors) == 1, monitors
                os.kill(monitors[0], signal.SIGTERM)
                assert "Niri" in finished("extension_or_app_crash")["session"]["latestNotice"]
                invoke("repair_integrations")
                print("PASS: actual desktop Niri stream loss, cleanup, visible failure and repair", flush=True)
                start()
                dnd(False)
                assert "Noctalia" in finished("extension_or_app_crash")["session"]["latestNotice"]
                invoke("repair_integrations")
                print("PASS: actual desktop Noctalia DND loss, cleanup, visible failure and repair", flush=True)
                start()
                peer.respond_heartbeat = False
                assert "browser" in finished("extension_or_app_crash")["session"]["latestNotice"]
                peer.respond_heartbeat = True
                invoke("repair_integrations")
                print("PASS: actual desktop browser heartbeat timeout, cleanup and repair", flush=True)
            start()
            pid = app_pid()
            os.kill(pid, signal.SIGTERM)
            wait_for("SIGTERM cleanup", lambda: not Path(f"/proc/{pid}").exists())
            assert dnd() == prior_dnd
            with sqlite3.connect(dbpath) as db:
                assert db.execute("SELECT finish_reason FROM sessions ORDER BY rowid DESC LIMIT 1").fetchone()[0] == "interrupted"
            end_driver_session()
            launch()
            assert invoke("app_snapshot")["settings"]["theme"] == "mist"
            print("PASS: active SIGTERM cleanup and theme persistence on relaunch", flush=True)
            start()
            pid = app_pid()
            os.kill(pid, signal.SIGKILL)
            wait_for("hard kill", lambda: not Path(f"/proc/{pid}").exists())
            end_driver_session()
            # A vanished removable folder must not break startup/recovery.
            with sqlite3.connect(dbpath) as db:
                db.execute("INSERT INTO music_sources(id,kind,path,created_at) VALUES(?,?,?,?)",
                           ("missing-folder", "folder", str(root / "missing-music"), 1))
            launch()
            recovered = finished("extension_or_app_crash")
            assert recovered["settings"]["theme"] == "mist"
            assert invoke("refresh_music_library")["queue"] == []
            print("PASS: hard-kill restart recovery, no session resume, missing folder startup", flush=True)
            if os.environ.get("DEEPIFY_PERFORMANCE_VALIDATION") == "1":
                directory = root / "large-library"
                directory.mkdir()
                for index in range(1000):
                    # Metadata/queue workload only; these are not decoder fixtures.
                    (directory / f"Fixture {index:04}.mp3").touch()
                with sqlite3.connect(dbpath) as db:
                    db.execute("INSERT INTO music_sources(id,kind,path,created_at) VALUES(?,?,?,?)",
                               ("large-library", "folder", str(directory), 1))
                began = time.monotonic()
                assert len(invoke("refresh_music_library")["queue"]) == 1000
                print(f"MEASURED: 1000-file metadata scan/reconciliation {(time.monotonic()-began)*1000:.0f} ms", flush=True)
                start()
                measure("working, 1000-track queue")
                invoke("session_end")
                finished("ended_early")
        except Exception:
            print("Disposable desktop diagnostic:\n" + "\n".join((root / "driver.log").read_text().splitlines()[-24:]), flush=True)
            raise
        finally:
            end_driver_session()
            driver.terminate()
            driver.wait(timeout=5)
            driver_log.close()
            bus.terminate()
            bus.wait(timeout=5)
            if audio_server:
                audio_server.close()
            dnd(prior_dnd)


if __name__ == "__main__":
    main()
