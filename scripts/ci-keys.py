#!/usr/bin/env python3
"""Keys for the results local CI keeps (bp-coord's local-ci.sh, scripts/focus.sh).

A key names one gate's verdict on exactly these inputs: the bytes of every
file the gate reads from the checkout, the runner it ran on (its bend, bun,
clang, python, environment and stack: scripts/ci-runner-id.sh, whose output
is the RUNNER text given here) and the scripts that judge it. A pass is
reused only for a byte-identical key; anything this tool cannot read or
does not understand fails closed (exit 1, nothing printed), so nothing is
reused.

  ci-keys.py tests RUNNER [TEST...]   "<key> <test>" for each test (default:
                                      every test scripts/test.sh runs)
  ci-keys.py check RUNNER             "<key> <file>" for each tracked .bend
                                      file check.sh type-checks
  ci-keys.py proof RUNNER             "<key> PROOF.bend"
  ci-keys.py js RUNNER                "<key> js": the whole checkout
  ci-keys.py inputs TEST|FILE...      the files a test's (or a file's check)
                                      key covers, one per line
  ci-keys.py affected BASE [HEAD]     what the diff BASE..HEAD (HEAD default:
                                      the working tree) can change: lines
                                      "test <t>", "check <f>", "proof", "js"

What a test's key covers (scripts/test.sh runs it):
  - its import closure (scripts/bend-closure.py's rules: .bend imports, the
    effect .c/.js files they import and the local headers those include);
  - its driver test/tools/<name>.mjs when there is one, and the local files
    that driver imports (relative import/require, transitively);
  - every file under test/fixtures/ (tests read fixtures at run time);
  - bunfig.toml, package.json and the like at the root, when there are any;
  - the files it declares: lines "# test-inputs: PATH..." in the test (or
    "// test-inputs: PATH..." in its driver). PATH is a file, a folder
    ending in "/" (every checkout file under it) or "closure:PROG.bend"
    (that program's closure, for a test that builds or runs it);
  - scripts/test.sh, scripts/test-shard.py, this file and
    scripts/ci-runner-id.sh;
  - the runner text.
A file's type-check key: its closure, scripts/check.sh, check-manifest.py,
bendok.sh, i18n-gen.py, this file, the runner. The proof's: PROOF.bend's
closure, check.sh, bendok.sh, this file, the runner. The js step's: every
checkout file (git ls-files -co --exclude-standard), the runner.
"""
import hashlib
import os
import re
import subprocess
import sys

VERSION = "ci-keys-v1"
BEND = re.compile(r'^\s*import\s+(\S+)(?:\s+as\s+[A-Za-z_][A-Za-z0-9_]*)?\s*(?:#.*)?$')
INCL = re.compile(r'^\s*#\s*include\s+"([^"]+)"')
JSIMP = re.compile(r'''(?:\bfrom\s*|\bimport\s*\(?\s*|\brequire\s*\(\s*)['"](\.{1,2}/[^'"]+)['"]''')
DECL = re.compile(r'^\s*(?:#|//)\s*test-inputs:\s*(.*)$')
RED = "test/cache_identity_before_test.bend"  # test.sh skips it


class Fail(Exception):
    pass


def rel(p):
    return os.path.relpath(os.path.normpath(p))


def checkout_files():
    out = subprocess.run(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
                         capture_output=True, check=False)
    if out.returncode != 0:
        raise Fail("git ls-files failed")
    fs = [f.decode("utf-8", "surrogateescape") for f in out.stdout.split(b"\0") if f]
    # the gates' own work folders exist only while they run
    return sorted(f for f in fs if not f.startswith((".bend-check.", ".bend-test.")) and os.path.isfile(f))


_hash = {}


def sha(f):
    if f not in _hash:
        try:
            with open(f, "rb") as fh:
                _hash[f] = hashlib.sha256(fh.read()).hexdigest()
        except OSError as e:
            raise Fail("cannot read %s: %s" % (f, e))
    return _hash[f]


_text = {}


def text(f):
    if f not in _text:
        try:
            _text[f] = open(f, encoding="utf-8", errors="surrogateescape").read()
        except OSError as e:
            raise Fail("cannot read %s: %s" % (f, e))
    return _text[f]


_deps = {}


def deps(f):
    """direct local reads of one file, as bend-closure.py reads them"""
    if f in _deps:
        return _deps[f]
    if f.startswith("..") or os.path.isabs(f):
        raise Fail("outside the checkout: %s" % f)
    t, here, out = text(f), os.path.dirname(f), []
    if f.endswith(".bend"):
        for line in t.split("\n"):
            if not re.match(r"^\s*import\b", line):
                continue
            m = BEND.match(line)
            if not m:
                raise Fail("%s: an import it does not understand: %r" % (f, line))
            name = m.group(1)
            if name.startswith('"') and name.endswith('"'):
                name = name[1:-1]
            if name.startswith("./") or name.startswith("../"):
                out.append(rel(os.path.join(here, name)))
            elif not re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", name):
                raise Fail("%s: an import it does not understand: %r" % (f, line))
    elif f.endswith((".c", ".h")):
        for line in t.split("\n"):
            m = INCL.match(line)
            if m:
                p = rel(os.path.join(here, m.group(1)))
                if os.path.exists(p):
                    out.append(p)
    _deps[f] = out
    return out


def js_deps(f):
    here = os.path.dirname(f)
    out = []
    for m in JSIMP.finditer(text(f)):
        p = rel(os.path.join(here, m.group(1)))
        if not os.path.isfile(p):
            for ext in (".mjs", ".js", ".ts", "/index.js"):
                if os.path.isfile(p + ext):
                    p = p + ext
                    break
            else:
                # a path built at run time (an emitted module beside it) is
                # not a checkout file; one that names a checkout file is
                continue
        out.append(p)
    return out


def closure(roots, js=False):
    seen, todo = set(), [rel(r) for r in roots]
    while todo:
        f = todo.pop()
        if f in seen:
            continue
        if not os.path.isfile(f):
            raise Fail("missing: %s" % f)
        seen.add(f)
        if f.endswith((".bend", ".c", ".h")):
            todo.extend(deps(f))
        elif js and f.endswith((".mjs", ".js", ".ts")):
            todo.extend(js_deps(f))
    return seen


def under(folder, files):
    folder = folder.rstrip("/") + "/"
    got = [f for f in files if f.startswith(folder)]
    if not got:
        raise Fail("declared folder has no files: %s" % folder)
    return got


def declared(f, files):
    out = set()
    for line in text(f).split("\n"):
        m = DECL.match(line)
        if not m:
            continue
        for w in m.group(1).split():
            if w.startswith("closure:"):
                out |= closure([w[len("closure:"):]])
            elif w.endswith("/"):
                out |= set(under(w, files))
            elif os.path.isfile(w):
                out.add(rel(w))
            else:
                raise Fail("%s declares %s, which is not a file" % (f, w))
    return out


def driver(t):
    return "test/tools/%s.mjs" % os.path.basename(t)[: -len("_test.bend")]


def all_tests():
    import glob
    return sorted(t for t in glob.glob("test/*_test.bend") if t != RED)


ROOT_CONFIG = ["bunfig.toml", "package.json", "tsconfig.json", "jsconfig.json", ".npmrc", "bend.toml"]
TEST_GATE = ["scripts/test.sh", "scripts/test-shard.py", "scripts/ci-keys.py", "scripts/ci-runner-id.sh"]
CHECK_GATE = ["scripts/check.sh", "scripts/check-manifest.py", "scripts/bendok.sh", "scripts/i18n-gen.py",
              "scripts/ci-keys.py", "scripts/ci-runner-id.sh"]
PROOF_GATE = ["scripts/check.sh", "scripts/bendok.sh", "scripts/ci-keys.py", "scripts/ci-runner-id.sh"]


def test_inputs(t, files):
    s = closure([t])
    d = driver(t)
    if os.path.isfile(d):
        s |= closure([d], js=True)
        s |= declared(d, files)
    s |= set(under("test/fixtures/", files))
    s |= declared(t, files)
    # config bun and bend look for at the root (bun probes bunfig.toml): when
    # one appears or goes, the set of inputs changes, and so the key
    s |= {f for f in ROOT_CONFIG if os.path.isfile(f)}
    s |= set(TEST_GATE)
    return s


def check_inputs(f):
    return closure([f]) | set(CHECK_GATE)


def proof_inputs():
    return closure(["PROOF.bend"]) | set(PROOF_GATE)


def key(tag, name, runner, inputs):
    h = hashlib.sha256()
    h.update(("%s\n%s %s\nrunner %s\n" % (VERSION, tag, name, hashlib.sha256(runner.encode()).hexdigest())).encode())
    for f in sorted(inputs):
        h.update(("%s %s\n" % (sha(f), f)).encode("utf-8", "surrogateescape"))
    return h.hexdigest()[:48]


def check_files(files):
    return [f for f in files if f.endswith(".bend") and f != "PROOF.bend"]


def changed(base, head):
    args = ["git", "diff", "--name-only", "-z", base] + ([head] if head else [])
    out = subprocess.run(args, capture_output=True, check=False)
    if out.returncode != 0:
        raise Fail("git diff failed")
    fs = {f.decode("utf-8", "surrogateescape") for f in out.stdout.split(b"\0") if f}
    if not head:
        # untracked, not ignored files are part of the working tree too
        u = subprocess.run(["git", "ls-files", "-z", "--others", "--exclude-standard"], capture_output=True, check=False)
        if u.returncode != 0:
            raise Fail("git ls-files failed")
        fs |= {f.decode("utf-8", "surrogateescape") for f in u.stdout.split(b"\0") if f}
    return fs


def main(argv):
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    op, rest = argv[1], argv[2:]
    root = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    if root.returncode != 0:
        raise Fail("not in a git checkout")
    os.chdir(root.stdout.strip())
    files = checkout_files()
    lines = []
    if op in ("tests", "check", "proof", "js"):
        if not rest:
            raise Fail("no runner text")
        runner = rest[0]
        if not runner.strip():
            raise Fail("empty runner text")
        if op == "tests":
            ts = rest[1:] or all_tests()
            for t in ts:
                lines.append("%s %s" % (key("test", t, runner, test_inputs(t, files)), t))
        elif op == "check":
            for f in check_files(files):
                lines.append("%s %s" % (key("check", f, runner, check_inputs(f)), f))
        elif op == "proof":
            lines.append("%s PROOF.bend" % key("proof", "PROOF.bend", runner, proof_inputs()))
        else:
            lines.append("%s js" % key("js", "js", runner, set(files)))
    elif op == "inputs":
        out = set()
        for t in rest:
            out |= test_inputs(t, files) if t.endswith("_test.bend") and t.startswith("test/") else check_inputs(t)
        lines = sorted(out)
    elif op == "affected":
        if not rest:
            raise Fail("affected BASE [HEAD]")
        ch = changed(rest[0], rest[1] if len(rest) > 1 else None)
        # a deleted file still changes whatever read it: compare by name
        for t in all_tests():
            if ch & test_inputs(t, files):
                lines.append("test %s" % t)
        for f in check_files(files):
            if ch & check_inputs(f):
                lines.append("check %s" % f)
        if ch & proof_inputs():
            lines.append("proof")
        if ch:
            lines.append("js")
    else:
        print(__doc__, file=sys.stderr)
        return 2
    sys.stdout.write("".join(l + "\n" for l in lines))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv))
    except Fail as e:
        sys.stderr.write("ci-keys: %s\n" % e)
        sys.exit(1)
