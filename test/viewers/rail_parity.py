#!/usr/bin/env python3
"""Per-check parity between two rail-shots runs (`steps.json`): every scorecard task, every B1 entry x transition, every extra check.

  rail_parity.py OLD/steps.json NEW/steps.json [--md OUT.md]

Prints "0 differences of N checks" or a table of every difference (a check present in only one run counts). New gates under `full/auto-`
are listed separately as new checks (they have no baseline) and are not differences. Exit status 0 when there are no differences, 1 otherwise.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

NEW_GATE_PREFIX = "full/auto-"


def flatten(steps: dict) -> tuple[dict[str, str], dict[str, str]]:
    """`(baseline-comparable checks, new gates)` as `{check id: value}` strings."""
    checks: dict[str, str] = {}
    new: dict[str, str] = {}
    for t in steps.get("scorecard_tasks") or []:
        tid = t.get("task")
        checks[f"task {tid}: outcome"] = str(t.get("outcome"))
        checks[f"task {tid}: found"] = str((t.get("parts") or {}).get("found"))
        checks[f"task {tid}: one_step_back"] = str((t.get("parts") or {}).get("one_step_back"))
    for kind, kv in sorted((steps.get("kinds") or {}).items()):
        entries = ((kv.get("b1_checks") or {}).get("entries")) or {}
        for entry, rec in sorted(entries.items()):
            checks[f"B1 {kind}/{entry}: status"] = str(rec.get("status"))
            checks[f"B1 {kind}/{entry}: passed"] = str(rec.get("passed"))
            for transition, val in sorted((rec.get("checks") or {}).items()):
                checks[f"B1 {kind}/{entry}: {transition}"] = str(val)
    for c in steps.get("extra_checks") or []:
        cid = str(c.get("id"))
        (new if cid.startswith(NEW_GATE_PREFIX) else checks)[f"extra {cid}"] = str(c.get("outcome"))
    return checks, new


def compare(old: dict, new: dict) -> dict:
    a, new_gates_a = flatten(old)
    b, new_gates_b = flatten(new)
    ids = sorted(set(a) | set(b))
    diffs = [(i, a.get(i, "(absent)"), b.get(i, "(absent)")) for i in ids if a.get(i) != b.get(i)]
    return {"total": len(ids), "differences": diffs, "new_gates": new_gates_b, "baseline_gates": new_gates_a}


def render(result: dict) -> str:
    n, diffs = result["total"], result["differences"]
    lines = [f"{len(diffs)} differences of {n} checks" if diffs else f"0 differences of {n} checks"]
    if diffs:
        lines += ["", "| check | baseline | now |", "|---|---|---|"] + [f"| {i} | {x} | {y} |" for i, x, y in diffs]
    if result["new_gates"]:
        lines += ["", "New checks (no baseline, not differences):", "", "| gate | outcome |", "|---|---|"] + [f"| {k} | {v} |" for k, v in sorted(result["new_gates"].items())]
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    args = [a for a in argv[1:] if not a.startswith("--")]
    md = argv[argv.index("--md") + 1] if "--md" in argv else None
    if md and md in args:
        args.remove(md)
    if len(args) != 2:
        print("usage: rail_parity.py OLD/steps.json NEW/steps.json [--md OUT.md]", file=sys.stderr)
        return 2
    old, new = (json.loads(Path(p).read_text(encoding="utf-8")) for p in args)
    result = compare(old, new)
    text = render(result)
    print(text, end="")
    if md:
        Path(md).write_text(text, encoding="utf-8")
    return 0 if not result["differences"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
