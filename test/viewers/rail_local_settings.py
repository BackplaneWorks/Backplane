#!/usr/bin/env python3
"""The client's private settings for one rail-shots run.

`RAIL_THEME` (exactly `dark` or `light`) and `RAIL_TEXT_PCT` (exactly `100` or `125`) are optional; only a knob that is set
appears in the file (an unset theme stays unset and follows the hub). Anything else exits 2 with a message. The file is
ALWAYS written and the client is ALWAYS pointed at it (`BACKPLANE_GPUI_LOCAL`), knobs or not, so an inherited settings path
of the person's own can never be read or saved by a test run.

  rail_local_settings.py --check            validate the environment only
  rail_local_settings.py OUT.json           validate and write OUT.json
"""

from __future__ import annotations

import json
import os
import sys

THEMES = ("dark", "light")
TEXT_PCTS = ("100", "125")


def settings_from_env(env: dict[str, str]) -> dict:
    out: dict = {}
    theme = env.get("RAIL_THEME", "")
    pct = env.get("RAIL_TEXT_PCT", "")
    if theme:
        if theme not in THEMES:
            raise ValueError(f"RAIL_THEME must be exactly dark or light (got {theme!r})")
        out["theme"] = theme
    if pct:
        if pct not in TEXT_PCTS:
            raise ValueError(f"RAIL_TEXT_PCT must be exactly 100 or 125 (got {pct!r})")
        out["text_pct"] = int(pct)
    return out


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: rail_local_settings.py --check | OUT.json", file=sys.stderr)
        return 2
    try:
        settings = settings_from_env(dict(os.environ))
    except ValueError as e:
        print(f"rail-shots: {e}", file=sys.stderr)
        return 2
    if argv[1] != "--check":
        with open(argv[1], "w", encoding="utf-8") as f:
            json.dump(settings, f)
            f.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
