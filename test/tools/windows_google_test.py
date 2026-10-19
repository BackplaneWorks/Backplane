"""Compile-input privacy and refusal checks; no compiler, login or network."""
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="windows google header ") as temp:
    header = Path(temp) / "private header.h"
    def run(inputs, ok=True):
        env = {k: v for k, v in os.environ.items() if not k.startswith("BACKPLANE_GOOGLE_")}
        env.update(inputs)
        header.unlink(missing_ok=True)
        result = subprocess.run([sys.executable, str(root / "scripts/google-header.py"), str(header)],
                                env=env, capture_output=True, text=True)
        assert (result.returncode == 0) == ok
        assert not result.stdout
        assert "fixture-client" not in result.stderr and "fixture-secret" not in result.stderr
        if not ok:
            assert not header.exists()
            return ""
        assert stat.S_IMODE(header.stat().st_mode) == 0o600
        return header.read_text()

    assert '#define BP_GOOGLE_CLIENT_ID ""' in run({})
    pair = {"BACKPLANE_GOOGLE_CLIENT_ID": "fixture-client.apps.googleusercontent.com",
            "BACKPLANE_GOOGLE_CLIENT_SECRET": "fixture-secret"}
    baked = run(pair)
    assert '"fixture-client.apps.googleusercontent.com"' in baked and '"fixture-secret"' in baked
    assert "fixture-client" not in run(dict(pair, BACKPLANE_GOOGLE_BAKE="0"))
    run({"BACKPLANE_GOOGLE_CLIENT_ID": pair["BACKPLANE_GOOGLE_CLIENT_ID"]}, False)
    run({"BACKPLANE_GOOGLE_CLIENT_SECRET": pair["BACKPLANE_GOOGLE_CLIENT_SECRET"]}, False)
    run(dict(pair, BACKPLANE_GOOGLE_CLIENT_SECRET='fixture-secret"\n#error leaked'), False)
    # Existing public mode must be tightened before any credential bytes are written.
    header.write_text("old")
    header.chmod(0o644)
    env = dict(os.environ, **pair)
    env.pop("BACKPLANE_GOOGLE_BAKE", None)
    subprocess.run([sys.executable, str(root / "scripts/google-header.py"), str(header)], env=env, check=True)
    assert stat.S_IMODE(header.stat().st_mode) == 0o600
print("WINDOWS_GOOGLE_COMPILE_INPUT_PRIVACY_AND_REFUSAL_PASS_NO_COMPILER")
