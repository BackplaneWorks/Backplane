#!/usr/bin/env python3
"""KiStack's KiCad skills, tracked in Backplane's git and shipped with every build.

  kistack.py verify            deploy/kistack matches deploy/kistack.lock
  kistack.py build DEST        DEST: the copy a build ships (from deploy/kistack)
  kistack.py check DIR         DIR is a shipped copy of exactly the pinned commit
  kistack.py bump REV          take KiStack's commit REV into deploy/kistack and the lock

deploy/kistack holds KiStack's skills/ and LICENSE at the commit
deploy/kistack.lock names, with the sha256 of the skills tree. A build copies
it beside the hub (scripts/build-kistack.sh, scripts/build-helpers.sh,
scripts/service-release.py) with a plugin.json of ours and a REVISION, and
nothing is fetched: not at build time, not by the hub. Every packaged service
folder is checked (scripts/check-helpers.py, dev.sh stage, release-classic.sh),
so a build without KiStack, or with another commit, fails. Release packages
had none from 0.12.0 on (the release producer never ran build-kistack.sh), and
the hub then fetched the newest commit at every start, unreviewed; now a new
KiStack reaches people only through a commit here (`bump`), reviewed like any
other change.

The tree hash: each file under skills/ as "<sha256>  <path>" (path relative to
the copy, e.g. skills/pcb/SKILL.md), sorted bytewise, one per line, hashed
again. GitHub's tarballs are not byte-stable; the tree is.
"""
import hashlib
import io
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import sys
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
SOURCE = "deploy/kistack"
LOCK = "deploy/kistack.lock"
REPO = "BackplaneWorks/kistack"
# Backplane's own plugin manifest:
# never the repository's, whose hooks or .mcp.json would run in every Claude
MANIFEST = '{"name":"kistack","description":"KiStack KiCad skills by American Embedded"}\n'
# what a shipped copy holds, and nothing else
SHIPPED = {".claude-plugin", "LICENSE", "REVISION", "skills"}
MAX_BYTES, MAX_UNPACKED, MAX_ENTRIES = 32 << 20, 128 << 20, 5000


def require(ok, message):
    if not ok:
        raise ValueError(message)


def hexsha(text, n=64):
    return len(text) == n and all(c in "0123456789abcdef" for c in text)


def lock(root=None):
    """The pinned commit and skills tree hash: {"rev": ..., "skills": ...}."""
    path = Path(root or ROOT) / LOCK
    found = {}
    for line in path.read_text().splitlines():
        parts = line.split()
        if len(parts) == 2 and parts[0] in ("rev", "skills"):
            found[parts[0]] = parts[1]
    require(hexsha(found.get("rev", ""), 40), LOCK + " names no commit (rev)")
    require(hexsha(found.get("skills", "")), LOCK + " names no skills tree hash (skills)")
    return found


def files(root):
    """The regular files under ROOT/skills, as posix paths relative to ROOT;
    a link or anything but files and folders is refused."""
    root = Path(root)
    skills = root / "skills"
    require(skills.is_dir() and not skills.is_symlink(), "No skills/ in " + str(root))
    out = []
    for dirpath, dirnames, filenames in os.walk(skills):
        for name in dirnames + filenames:
            p = Path(dirpath) / name
            mode = os.lstat(p).st_mode
            require(stat.S_ISDIR(mode) or stat.S_ISREG(mode), "Not a plain file or folder in KiStack: " + str(p.relative_to(root)))
            if stat.S_ISREG(mode):
                out.append(p.relative_to(root).as_posix())
    require(out, "No files under " + str(skills))
    return sorted(out, key=lambda s: s.encode())


def tree(root):
    """The skills tree hash of the copy at ROOT."""
    root = Path(root)
    text = "".join(hashlib.sha256((root / f).read_bytes()).hexdigest() + "  " + f + "\n" for f in files(root))
    return hashlib.sha256(text.encode()).hexdigest()


def verify(root=None):
    """deploy/kistack is exactly the pinned skills tree (and its LICENSE)."""
    root = Path(root or ROOT)
    pin = lock(root)
    src = root / SOURCE
    require(src.is_dir(), SOURCE + " is missing: KiStack ships from there")
    extra = sorted(p.name for p in src.iterdir() if p.name not in ("skills", "LICENSE"))
    require(not extra, SOURCE + " holds more than skills/ and LICENSE: " + ", ".join(extra))
    require((src / "LICENSE").is_file(), SOURCE + "/LICENSE is missing")
    got = tree(src)
    require(got == pin["skills"], SOURCE + "/skills hashes to " + got + ", not " + pin["skills"] + " (" + LOCK
            + "); change KiStack only with scripts/kistack.py bump")
    require(any(PurePosixPath(f).name == "SKILL.md" for f in files(src)), "No skills/*/SKILL.md in " + SOURCE)
    return pin


def check(dest, root=None):
    """DEST is a shipped copy of exactly the pinned KiStack: our manifest, the
    pinned REVISION, the pinned skills tree, nothing else. Answers the lock."""
    dest = Path(dest)
    pin = lock(root)
    require(dest.is_dir() and not dest.is_symlink(), "No KiStack at " + str(dest) + " (this build would ship without it)")
    names = {p.name for p in dest.iterdir()}
    require(names <= SHIPPED, "KiStack at " + str(dest) + " holds more than it should: " + ", ".join(sorted(names - SHIPPED)))
    manifest = dest / ".claude-plugin/plugin.json"
    require(manifest.is_file() and not manifest.is_symlink() and manifest.read_text() == MANIFEST,
            "KiStack at " + str(dest) + " lacks Backplane's own plugin.json")
    require(sorted(p.name for p in (dest / ".claude-plugin").iterdir()) == ["plugin.json"],
            "KiStack at " + str(dest) + " carries more than plugin.json in .claude-plugin")
    revision = dest / "REVISION"
    have = revision.read_text().strip() if revision.is_file() else ""
    require(have == pin["rev"], "KiStack at " + str(dest) + " is " + (have or "of no commit") + ", not the pinned " + pin["rev"])
    got = tree(dest)
    require(got == pin["skills"], "KiStack at " + str(dest) + " has skills hashing to " + got + ", not the pinned " + pin["skills"])
    return pin


def build(dest, root=None):
    """The copy a build ships, into DEST (replaced whole, by rename)."""
    root = Path(root or ROOT)
    pin = verify(root)
    dest = Path(dest).absolute()
    new = dest.with_name(dest.name + ".new")
    shutil.rmtree(new, ignore_errors=True)
    (new / ".claude-plugin").mkdir(parents=True)
    shutil.copytree(root / SOURCE / "skills", new / "skills")
    shutil.copyfile(root / SOURCE / "LICENSE", new / "LICENSE")
    (new / ".claude-plugin/plugin.json").write_text(MANIFEST)
    (new / "REVISION").write_text(pin["rev"] + "\n")
    check(new, root)
    if dest.is_dir() and not dest.is_symlink():
        shutil.rmtree(dest)
    elif dest.exists() or dest.is_symlink():
        dest.unlink()
    new.rename(dest)
    return pin, len(files(dest))


def kept(name, top):
    """An archive member's path inside the repository when it is one we take
    (skills/** and LICENSE), else None."""
    p = PurePosixPath(name)
    if not p.parts or p.parts[0] != top or len(p.parts) < 2:
        return None
    inside = PurePosixPath(*p.parts[1:])
    if inside.parts[0] == "skills" or str(inside) == "LICENSE":
        return inside
    return None


def unpack(data, into):
    """KiStack's skills/ and LICENSE from GitHub's tarball bytes into INTO;
    links, special files, paths out of the tree and oversize archives refused."""
    require(len(data) <= MAX_BYTES, "The archive is over " + str(MAX_BYTES) + " bytes")
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tar:
        members = tar.getmembers()
        require(len(members) <= MAX_ENTRIES, "The archive has over " + str(MAX_ENTRIES) + " entries")
        tops = {PurePosixPath(m.name).parts[0] for m in members if m.name and m.name != "pax_global_header"}
        require(len(tops) == 1, "The archive is not one folder")
        top = tops.pop()
        total = 0
        for m in members:
            inside = kept(m.name, top)
            if inside is None:
                continue
            require(".." not in inside.parts and not inside.is_absolute(), "A path out of the tree: " + m.name)
            require(m.isfile() or m.isdir(), "A link or special file among the skills: " + m.name)
            out = Path(into) / inside
            if m.isdir():
                out.mkdir(parents=True, exist_ok=True)
                continue
            total += m.size
            require(total <= MAX_UNPACKED, "The archive unpacks to over " + str(MAX_UNPACKED) + " bytes")
            out.parent.mkdir(parents=True, exist_ok=True)
            with tar.extractfile(m) as src:
                out.write_bytes(src.read())
            out.chmod(0o755 if m.mode & 0o111 else 0o644)
    require((Path(into) / "skills").is_dir(), "No skills/ in the archive")


def lock_text(rev, skills):
    return ("# KiStack (github.com/" + REPO + "), tracked in Backplane's git: its skills/ and\n"
            "# LICENSE at this commit live in deploy/kistack, and every build ships exactly\n"
            "# that beside the hub (scripts/kistack.py; the hub fetches nothing). The\n"
            "# sha256 of the skills/ tree (each file's sha256 and path, sorted, hashed\n"
            "# again) is checked at every build. Change all three only with\n"
            "#   python3 scripts/kistack.py bump <commit>\n"
            "rev " + rev + "\n"
            "skills " + skills + "\n")


def bump(rev, root=None, data=None):
    """KiStack's commit REV into deploy/kistack and the lock (DATA: its
    tarball's bytes, else downloaded from GitHub). Review the diff."""
    root = Path(root or ROOT)
    require(hexsha(rev, 40), "A full 40-hex commit, please (not a branch: that would not pin anything)")
    if data is None:
        url = "https://codeload.github.com/" + REPO + "/tar.gz/" + rev
        with urllib.request.urlopen(url, timeout=120) as r:
            data = r.read(MAX_BYTES + 1)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp) / "kistack"
        unpack(data, work)
        require(any(PurePosixPath(f).name == "SKILL.md" for f in files(work)), "No skills/*/SKILL.md at " + rev)
        skills = tree(work)
        src = root / SOURCE
        old = src.with_name(src.name + ".old")
        shutil.rmtree(old, ignore_errors=True)
        if src.exists():
            src.rename(old)
        shutil.copytree(work, src)
        shutil.rmtree(old, ignore_errors=True)
    (root / LOCK).write_text(lock_text(rev, skills))
    verify(root)
    return skills


def main(argv):
    if len(argv) < 2 or argv[1] not in ("verify", "build", "check", "bump"):
        print(__doc__.strip().split("\n\n")[0], file=sys.stderr)
        return 2
    op = argv[1]
    if op == "verify" and len(argv) == 2:
        pin = verify()
        print("kistack ok: " + SOURCE + " is " + pin["rev"])
    elif op == "build" and len(argv) == 3:
        pin, n = build(argv[2])
        print(argv[2] + ": KiStack " + pin["rev"] + " (" + str(n) + " files)")
    elif op == "check" and len(argv) == 3:
        pin = check(argv[2])
        print("kistack ok: " + argv[2] + " is " + pin["rev"])
    elif op == "bump" and len(argv) == 3:
        skills = bump(argv[2])
        print("KiStack " + argv[2] + " in " + SOURCE + " (skills " + skills + "); review `git diff " + SOURCE + "` and commit it with " + LOCK)
    else:
        print(__doc__.strip().split("\n\n")[0], file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv))
    except (ValueError, OSError, tarfile.TarError) as e:
        print("kistack: " + str(e), file=sys.stderr)
        sys.exit(1)
