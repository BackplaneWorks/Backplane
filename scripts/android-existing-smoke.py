#!/usr/bin/env python3
"""Launch an already built APK on a disposable emulator; never build or sign."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

PACKAGE = "dev.backplane.mobile"
BASE = Path("build/android-existing")
INPUT = BASE / "input"
OUT = BASE / "evidence"


def bound_apk(root, source, version, code):
    if not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ValueError("Expected full source commit")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) or not code.isdecimal():
        raise ValueError("Invalid expected version")
    receipt = json.loads((root / "receipt.json").read_text())
    expected = {"status": "ARTIFACTS_BUILT_PENDING_TARGET_ACCEPTANCE",
                "sourceCommit": source, "version": version, "channel": "release",
                "package": PACKAGE, "version_name": version, "version_code": int(code),
                "debuggable": False}
    for key, value in expected.items():
        if receipt.get(key) != value:
            raise ValueError(f"Producer mismatch: {key}")
    artifact = receipt["artifacts"]["apk"]
    apk = root / artifact["path"]
    if not apk.is_file() or apk.is_symlink() or apk.resolve().parent != root.resolve():
        raise ValueError("APK must be a regular direct input member")
    if hashlib.sha256(apk.read_bytes()).hexdigest() != artifact["sha256"]:
        raise ValueError("APK differs from producer")
    return apk, receipt


def adb(*args, timeout=25, check=True):
    return subprocess.run(["adb", "-e", *args], capture_output=True, text=True,
                          timeout=timeout, check=check).stdout


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    apk, receipt = bound_apk(INPUT, os.environ["EXPECTED_SOURCE"],
                            os.environ["EXPECTED_VERSION"], os.environ["EXPECTED_CODE"])
    binding = {"sourceCommit": receipt["sourceCommit"], "channel": receipt["channel"],
               "version": receipt["version"], "versionCode": receipt["version_code"],
               "apkSha256": receipt["artifacts"]["apk"]["sha256"],
               "producerRun": os.environ["PRODUCER_RUN"], "buildRepeated": False,
               "published": False}
    (OUT / "input-binding.json").write_text(json.dumps(binding, indent=2) + "\n")
    if sys.argv[1:] == ["--preflight"]:
        return
    if sys.argv[1:]:
        raise ValueError("Unknown argument")
    result = {**binding, "status": "FAIL", "installed": False}
    try:
        adb("wait-for-device", timeout=60)
        if adb("shell", "getprop", "ro.kernel.qemu").strip() != "1":
            raise ValueError("Refusing a non-emulator device")
        adb("install", "--no-streaming", str(apk), timeout=90)
        result["installed"] = True
        package = adb("shell", "dumpsys", "package", PACKAGE)
        (OUT / "package.txt").write_text(package)
        if not re.search(r"versionCode=" + str(receipt["version_code"]) + r"\b", package):
            raise ValueError("Installed version code mismatch")
        if "versionName=" + receipt["version"] not in package:
            raise ValueError("Installed version name mismatch")
        adb("logcat", "-c")
        launch = adb("shell", "am", "start", "-W", "-n", PACKAGE + "/.MainActivity")
        (OUT / "launch.txt").write_text(launch)
        if not re.search(r"Status:\s+ok\b", launch):
            raise ValueError("Activity failed to launch")
        deadline = time.monotonic() + 30
        connected_screen = False
        while time.monotonic() < deadline:
            adb("shell", "uiautomator", "dump", "/sdcard/backplane-smoke.xml")
            adb("pull", "/sdcard/backplane-smoke.xml", str(OUT / "screen.xml"))
            tree = ET.parse(OUT / "screen.xml")
            words = " ".join(n.get("text", "") for n in tree.iter("node"))
            if "Connect" in words:
                connected_screen = True
                break
            time.sleep(1)
        if not connected_screen:
            raise ValueError("First-run Connect screen did not paint")
        pid = adb("shell", "pidof", PACKAGE).strip()
        if not re.fullmatch(r"\d+(?: \d+)*", pid):
            raise ValueError("App exited")
        activity = adb("shell", "dumpsys", "activity", "activities")
        (OUT / "activity.txt").write_text(activity)
        if not any(PACKAGE + "/" in line and "ResumedActivity" in line
                   for line in activity.splitlines()):
            raise ValueError("App is not foreground")
        log = adb("logcat", "-d", "--pid=" + pid.split()[0])
        (OUT / "logcat.txt").write_text(log)
        if "FATAL EXCEPTION" in log or "Fatal signal" in log:
            raise ValueError("App crashed")
        adb("shell", "screencap", "-p", "/sdcard/backplane-smoke.png")
        adb("pull", "/sdcard/backplane-smoke.png", str(OUT / "screen.png"))
        result.update(status="PASS", connectScreenPainted=True, foreground=True,
                      pid=pid, noHubOrLogin=True)
    except Exception as error:
        result["error"] = str(error)
        raise
    finally:
        if result["installed"]:
            try:
                adb("shell", "am", "force-stop", PACKAGE)
                result["uninstallOutput"] = adb("uninstall", PACKAGE)
                result["ownedAppStopped"] = True
            except Exception as error:
                result["cleanupError"] = str(error)
                result["status"] = "FAIL"
        (OUT / "RESULT.json").write_text(json.dumps(result, indent=2) + "\n")
    if result["status"] != "PASS":
        raise RuntimeError("Smoke cleanup failed")


if __name__ == "__main__":
    main()
