#!/usr/bin/env python3
"""Generate and verify Bend check manifests for scripts/check.sh.

A manifest is a generated .bend file at the repo root that imports every
ordinary .bend file exactly once under a synthetic unique alias (M000,
M001, ...). Bend checks every def of every imported file when it checks an
importer (verified against bend 2.0.25), so checking a few manifests
type-checks the whole tree while loading each file's source once per
manifest instead of once per importing file.

Usage:
  check-manifest.py generate OUTDIR SHARDS FILE...
  check-manifest.py verify OUTDIR FILE...

generate partitions FILE... into SHARDS manifests, balancing the added
transitive-source cost of each file (files that share imports are grouped
together), writes OUTDIR/manifest-<i>.bend, then verifies coverage.
verify re-reads the manifests in OUTDIR and fails unless every listed
file is imported exactly once across them and nothing else is imported.
Both modes print one "manifest-<i>.bend: N files" line per shard.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path.cwd()
IMPORT_RE = re.compile(r"^\s*import\s+(\./|\.\./)(\S+\.bend)\s")


def deps(path):
    out = []
    try:
        text = pathlib.Path(path).read_text(errors="ignore")
    except OSError:
        return out
    for line in text.splitlines():
        m = IMPORT_RE.match(line)
        if m:
            resolved = (pathlib.Path(path).parent / (m.group(1) + m.group(2)))
            try:
                out.append(str(resolved.resolve().relative_to(ROOT.resolve())))
            except ValueError:
                pass
    return out


def closures(files):
    # A selected file may reach several levels of unselected imports.
    # Discover and cost every reached file, including cycles, before scoring
    # the selection; only the requested roots become manifest imports.
    graph, lines = {}, {}
    pending = list(files)
    while pending:
        f = pending.pop()
        if f in graph:
            continue
        graph[f] = deps(f)
        pending.extend(graph[f])
        try:
            lines[f] = len(pathlib.Path(f).read_text(errors="ignore").splitlines())
        except OSError:
            lines[f] = 0

    def close(f):
        seen, stack = set(), [f]
        while stack:
            x = stack.pop()
            if x in seen:
                continue
            seen.add(x)
            stack.extend(graph.get(x, []))
        return seen

    return {f: close(f) for f in files}, lines


def partition(files, shards):
    clos, lines = closures(files)
    order = sorted(files, key=lambda f: (-sum(lines[x] for x in clos[f]), f))
    groups = [[] for _ in range(shards)]
    unions = [set() for _ in range(shards)]
    for f in order:
        best, best_key = 0, None
        for i in range(shards):
            added = sum(lines[x] for x in clos[f] - unions[i])
            # Balance each shard's total imported source; among shards
            # with a similar total, prefer the one this file overlaps
            # most (smallest added cost).
            key = (sum(lines[x] for x in unions[i]) + added, added, i)
            if best_key is None or key < best_key:
                best, best_key = i, key
        groups[best].append(f)
        unions[best] |= clos[f]
    return groups


def write_manifests(outdir, groups):
    import os

    out = pathlib.Path(outdir)
    out.mkdir(parents=True, exist_ok=True)
    n = 0
    for i, group in enumerate(groups):
        rows = ["import Base"]
        for f in sorted(group):
            rel = os.path.relpath(ROOT / f, out)
            if not rel.startswith("."):
                rel = "./" + rel
            rows.append(f"import {rel} as M{n:03d}")
            n += 1
        (out / f"manifest-{i}.bend").write_text("\n".join(rows) + "\n")


def manifest_imports(outdir):
    found = []
    for mf in sorted(pathlib.Path(outdir).glob("manifest-*.bend")):
        for line in mf.read_text(errors="ignore").splitlines():
            m = IMPORT_RE.match(line)
            if m:
                resolved = (mf.parent / (m.group(1) + m.group(2)))
                found.append(str(resolved.resolve().relative_to(ROOT.resolve())))
    return found


def verify(outdir, files):
    found = manifest_imports(outdir)
    want = sorted(files)
    if sorted(found) != want:
        missing = sorted(set(want) - set(found))
        extra = sorted(set(found) - set(want))
        dupes = sorted({f for f in found if found.count(f) > 1})
        for label, items in (("not covered", missing), ("unexpected", extra), ("duplicated", dupes)):
            for f in items:
                print(f"check-manifest: {label}: {f}", file=sys.stderr)
        return False
    for mf in sorted(pathlib.Path(outdir).glob("manifest-*.bend")):
        count = sum(1 for line in mf.read_text().splitlines() if IMPORT_RE.match(line))
        print(f"{mf.name}: {count} files")
    return True


def main(argv):
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    mode, outdir = argv[1], argv[2]
    if mode == "generate":
        shards, files = int(argv[3]), argv[4:]
        if shards < 1 or not files:
            print("check-manifest: need SHARDS >= 1 and at least one file", file=sys.stderr)
            return 2
        write_manifests(outdir, partition(files, shards))
        return 0 if verify(outdir, files) else 1
    if mode == "verify":
        return 0 if verify(outdir, argv[3:]) else 1
    print(f"check-manifest: unknown mode {mode}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
