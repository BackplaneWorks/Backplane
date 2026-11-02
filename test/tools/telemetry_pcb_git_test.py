"""Exercise the production Git creation classifier on disposable local commits."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", type=Path, required=True)
    args = parser.parse_args()
    worker = args.worker.resolve()
    env = {**os.environ, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
           "GIT_AUTHOR_NAME": "Test", "GIT_AUTHOR_EMAIL": "test@localhost",
           "GIT_COMMITTER_NAME": "Test", "GIT_COMMITTER_EMAIL": "test@localhost",
           "BACKPLANE_TELEMETRY": "off", "BEND_NO_TELEMETRY": "1"}
    with tempfile.TemporaryDirectory(prefix="bp-pcb-counts-") as temp:
        root = Path(temp)
        def git(*argv):
            return subprocess.check_output(["git", *argv], cwd=root, env=env, text=True).strip()
        def commit():
            git("add", "-A")
            git("-c", "commit.gpgsign=false", "commit", "-qm", "fixture")
            return git("rev-parse", "HEAD")
        def check(name, now, before, expected):
            value = subprocess.check_output([str(worker), "--pcb-created", str(root), now, before, git("rev-parse", "--absolute-git-dir")],
                                            env=env, text=True, timeout=60).strip()
            if value != str(expected):
                raise AssertionError(f"{name}: got {value}, expected {expected}")
            print("ok " + name, flush=True)
        git("init", "-q")
        (root / "README").write_text("fixture\n")
        baseline = commit()
        (root / "private.kicad_pro").write_text("{}\n")
        (root / "private.kicad_pcb").write_text("(kicad_pcb (version 20260324))\n")
        created = commit()
        check("project and PCB created together count once", created, baseline, 1)
        (root / "private.kicad_pcb").write_text("(kicad_pcb (version 20260324) (generator pcbnew))\n")
        edited = commit()
        check("editing existing board counts zero", edited, created, 0)
        (root / "private.kicad_pro").rename(root / "renamed.kicad_pro")
        (root / "private.kicad_pcb").rename(root / "renamed.kicad_pcb")
        renamed = commit()
        check("renaming project and board counts zero", renamed, edited, 0)
        (root / "second.kicad_pro").write_text("{\"meta\":{}}\n")
        project = commit()
        check("new KiCad project without board counts once", project, renamed, 1)
        (root / "second.kicad_pcb").write_text("(kicad_pcb (version 20260324))\n")
        counterpart = commit()
        check("later PCB counterpart does not double count", counterpart, project, 0)
        (root / "second.kicad_pcb").unlink()
        deleted = commit()
        check("deleting a board counts zero", deleted, counterpart, 0)
        (root / "third\nprivate.kicad_pcb").write_text("(kicad_pcb)\n")
        newline = commit()
        check("NUL framing handles newline in local path", newline, deleted, 1)

        # The actual capture creates a trusted shadow repository for non-git
        # projects; creation lookup must use that capture's explicit git dir.
        registry = root / "history.trusted"
        def capture(name, expected):
            value = subprocess.check_output([str(worker), "--pcb-capture", str(shadow), str(registry)],
                                           env=env, text=True, timeout=60).strip()
            if value != str(expected):
                raise AssertionError(f"{name}: got {value}, expected {expected}")
            print("ok " + name, flush=True)
        # Put it outside the git fixture root to exercise true shadow mode.
        with tempfile.TemporaryDirectory(prefix="bp-pcb-shadow-") as other:
            outside = Path(other) / "project"
            outside.mkdir()
            (outside / "one.kicad_pro").write_text("{}\n")
            (outside / "one.kicad_pcb").write_text("(kicad_pcb)\n")
            shadow = outside
            capture("non-git capture uses trusted shadow repository", 1)
            capture("unchanged shadow capture counts zero", 0)
            (shadow / "two.kicad_pcb").write_text("(kicad_pcb (version 20260324))\n")
            capture("new board in non-git project counts once", 1)
            (shadow / "two.kicad_pcb").rename(shadow / "renamed.kicad_pcb")
            capture("shadow board rename counts zero", 0)


if __name__ == "__main__":
    main()
