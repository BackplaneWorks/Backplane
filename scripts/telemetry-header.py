#!/usr/bin/env python3
"""Write compile-only telemetry defaults; never put credentials in emitted C."""
import json
import os
from pathlib import Path
import re
import sys


def write_header(path: Path) -> None:
    values = {name: "" for name in ("URL", "KID", "KEY", "CHANNEL")}
    if os.environ.get("BACKPLANE_TELEMETRY_BAKE") == "1":
        source = os.environ.get("BACKPLANE_VERSION", "").removeprefix("v")
        stamp = os.environ.get("BACKPLANE_VERSION_STAMP", "").removeprefix("v")
        if source and stamp and source != stamp:
            raise ValueError("source and stamped package versions disagree")
        version = os.environ.get("BACKPLANE_VERSION_STAMP") or os.environ.get("BACKPLANE_VERSION", "")
        version = version.removeprefix("v")
        if not re.fullmatch(r"\d{1,4}\.\d{1,4}\.\d{1,4}(?:-nightly\.[0-9A-Za-z.]{1,24})?", version):
            raise ValueError("telemetry baking requires a stable or nightly package version")
        channel = "nightly" if "-nightly." in version else "release"
        supplied = os.environ.get("BACKPLANE_TELEMETRY_CHANNEL", channel)
        if supplied != channel:
            raise ValueError("telemetry channel disagrees with the package version")
        values = {name: os.environ.get("BACKPLANE_TELEMETRY_" + name, "") for name in values}
        values["CHANNEL"] = channel
        # Missing configuration is an intentional build kill switch.
        if all(values[name] for name in ("URL", "KID", "KEY")):
            if values["URL"] != "https://backplane.works/api/v1/events":
                raise ValueError("release telemetry endpoint must be the agreed HTTPS ingest endpoint")
            if not re.fullmatch(r"rel-[a-z0-9-]{1,28}", values["KID"]):
                raise ValueError("release telemetry needs a release key id")
            if not re.fullmatch(r"[0-9a-f]{64}", values["KEY"]):
                raise ValueError("telemetry key must be 32 bytes of lowercase hex")
        else:
            values = {name: "" for name in values}
    path.parent.mkdir(parents=True, exist_ok=True)
    # Create privately, including when the surrounding build directory is public.
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.fchmod(fd, 0o600)
    with os.fdopen(fd, "w") as stream:
        stream.write("/* Generated locally for compilation only. */\n")
        if path.suffix == ".js":
            # the JS build's prelude (scripts/build-js.sh), read by the JS
            # twin of the effect; the same values the C header defines
            baked = {"BACKPLANE_TELEMETRY_" + name: value for name, value in values.items()}
            stream.write("globalThis.BACKPLANE_BAKED = Object.assign(globalThis.BACKPLANE_BAKED || {}, "
                         + json.dumps(baked, sort_keys=True) + ");\n")
            return
        for name, value in values.items():
            stream.write(f'#define BP_TELEMETRY_{name} "{value}"\n')


if __name__ == "__main__":
    try:
        write_header(Path(sys.argv[1]))
    except (ValueError, OSError, IndexError) as exc:
        # Validation messages contain no configuration values.
        sys.exit(f"telemetry-header: {exc}")
