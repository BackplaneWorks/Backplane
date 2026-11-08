#!/usr/bin/env python3
"""boat-prune.py ROOT LISTFILE: delete everything under ROOT that is not named in LISTFILE (NUL-separated paths relative to ROOT), then empty directories.
Runs ON a Boat build box after the worktree was copied there (scripts/lib-boat.sh): the copy then holds exactly the files the caller's worktree has."""
import os, sys
root, listfile = os.path.abspath(sys.argv[1]), sys.argv[2]
keep = set(p for p in open(listfile, "rb").read().decode("utf-8", "surrogateescape").split("\0") if p)
dirs = set()
for p in keep:
    d = os.path.dirname(p)
    while d and d not in dirs:
        dirs.add(d); d = os.path.dirname(d)
gone = 0
for cur, ds, fs in os.walk(root, topdown=True):
    rel = os.path.relpath(cur, root); rel = "" if rel == "." else rel
    if rel == ".git":
        ds[:] = []; continue
    for f in fs:
        r = os.path.join(rel, f)
        if r not in keep:
            os.unlink(os.path.join(cur, f)); gone += 1
for cur, ds, fs in os.walk(root, topdown=False):
    rel = os.path.relpath(cur, root)
    if rel not in (".", ".git") and not rel.startswith(".git" + os.sep) and rel not in dirs:
        try: os.rmdir(cur)
        except OSError: pass
if gone: print("boat-prune: removed %d files not in the worktree" % gone)
