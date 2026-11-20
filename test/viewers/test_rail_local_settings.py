#!/usr/bin/env python3
"""The client's private settings for rail-shots runs: knobs, validation and isolation."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import rail_local_settings as L  # noqa: E402


def run_cli(args: list[str], env_extra: dict[str, str]) -> subprocess.CompletedProcess:
    env = {k: v for k, v in os.environ.items() if not k.startswith("RAIL_") and k != "BACKPLANE_GPUI_LOCAL"}
    env.update(env_extra)
    return subprocess.run([sys.executable, str(HERE / "rail_local_settings.py"), *args], env=env, capture_output=True, text=True)


class LocalSettingsTests(unittest.TestCase):
    def test_knobs_unset_theme_only_percentage_only(self) -> None:
        self.assertEqual(L.settings_from_env({}), {})  # nothing set: follows the hub, default size
        self.assertEqual(L.settings_from_env({"RAIL_THEME": "light"}), {"theme": "light"})
        # percentage alone must NOT force a theme
        self.assertEqual(L.settings_from_env({"RAIL_TEXT_PCT": "125"}), {"text_pct": 125})
        self.assertEqual(L.settings_from_env({"RAIL_THEME": "dark", "RAIL_TEXT_PCT": "100"}), {"theme": "dark", "text_pct": 100})

    def test_invalid_knobs_exit_nonzero_with_a_message(self) -> None:
        for env in ({"RAIL_TEXT_PCT": "099"}, {"RAIL_TEXT_PCT": "999"}, {"RAIL_TEXT_PCT": "110"}, {"RAIL_TEXT_PCT": "125 "}, {"RAIL_THEME": "hub"}, {"RAIL_THEME": "Dark"}):
            r = run_cli(["--check"], env)
            self.assertEqual(r.returncode, 2, env)
            self.assertIn("RAIL_", r.stderr)
        # and through the runner itself, before anything else happens (fake positional args)
        r = subprocess.run(["bash", str(HERE / "rail-shots.sh"), "a", "b", "c", "d"], env={**os.environ, "RAIL_TEXT_PCT": "099"}, capture_output=True, text=True)
        self.assertEqual(r.returncode, 2)
        self.assertIn("RAIL_TEXT_PCT must be exactly 100 or 125", r.stderr)

    def test_the_file_is_always_written_and_an_inherited_settings_path_is_never_touched(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            person = Path(td) / "person-settings.json"
            person.write_text('{"theme":"light","text_pct":150}\n', encoding="utf-8")
            before = person.read_text(encoding="utf-8")
            out = Path(td) / "run-settings.json"
            r = run_cli([str(out)], {"BACKPLANE_GPUI_LOCAL": str(person)})
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertEqual(json.loads(out.read_text(encoding="utf-8")), {})  # knobs unset: still written, empty
            self.assertEqual(person.read_text(encoding="utf-8"), before)
        # the runner points the client at its own file on every run, not only when a knob is set
        sh = (HERE / "rail-shots.sh").read_text(encoding="utf-8")
        launch = [ln for ln in sh.splitlines() if 'BACKPLANE_GPUI_LOCAL="$RUN/local-settings.json"' in ln]
        self.assertEqual(len(launch), 1)
        self.assertNotIn("RAIL_THEME", launch[0])
        self.assertNotIn("RAIL_TEXT_PCT", launch[0])
        self.assertIn('rail_local_settings.py" "$RUN/local-settings.json"', sh)


if __name__ == "__main__":
    unittest.main()
