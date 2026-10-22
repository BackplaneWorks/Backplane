#!/usr/bin/env python3
"""Write a private compile-only Google desktop client from explicit inputs."""
import json
import os
from pathlib import Path
import re
import sys


def write_header(path: Path) -> None:
    values = {name: "" for name in ("ID", "SECRET")}
    if os.environ.get("BACKPLANE_GOOGLE_BAKE", "1") != "0":
        values = {name: os.environ.get("BACKPLANE_GOOGLE_CLIENT_" + name, "") for name in values}
        if bool(values["ID"]) != bool(values["SECRET"]):
            raise ValueError("Google client requires both ID and SECRET inputs")
        if any(value and not re.fullmatch(r"[A-Za-z0-9._-]+", value) for value in values.values()):
            raise ValueError("Google client inputs contain unsupported characters")
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.fchmod(fd, 0o600)
    with os.fdopen(fd, "w") as stream:
        stream.write("/* Generated locally for compilation only. */\n")
        if path.suffix == ".js":
            # the JS build's prelude (scripts/build-js.sh), read by the JS
            # twin of the effect; the same values the C header defines
            baked = {"BACKPLANE_GOOGLE_CLIENT_" + name: value for name, value in values.items()}
            stream.write("globalThis.BACKPLANE_BAKED = Object.assign(globalThis.BACKPLANE_BAKED || {}, "
                         + json.dumps(baked, sort_keys=True) + ");\n")
            return
        for name, value in values.items():
            stream.write(f'#define BP_GOOGLE_CLIENT_{name} "{value}"\n')


if __name__ == "__main__":
    try:
        write_header(Path(sys.argv[1]))
    except (ValueError, OSError, IndexError) as exc:
        sys.exit(f"google-header: {exc}")
