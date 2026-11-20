#!/usr/bin/env python3
"""Single shared manifest for rail-shots.sh, rail_shots_driver.py, and rail_shots_post.py."""
from __future__ import annotations

KINDS = ["full", "board-only", "schematic-only", "mech-only", "no-design"]

# (slug, rail_control_id, settle_kind, expected_open)
PAGES = [
    ("idle", None, "", ""),
    ("board", "rail:Board", "board", "board"),
    ("schematic", "rail:Schematic", "schematic", "schematic"),
    ("3d", "rail:3D", "3d", "3d"),
    ("gerbers", "rail:Gerbers", "gerbers", "board"),
    ("split", "rail:Split", "split", "split"),
    ("mech", "rail:Mech", "mech", "mech"),
    ("split-after-mech", "rail:Split", "split", "split"),
    ("files", "rail:Files", "files", "files"),
    ("diff", "rail:Diff", "diff", "diff"),
    ("review", "rail:Review", "review", "review"),
    ("browser", "rail:Browser", "browser", "browser"),
]

# Non-schematic pages from which Task 3 verifies 1-step return to Schematic on `full`.
TASK3_SOURCES = [
    ("board", "rail:Board", "board"),
    ("3d", "rail:3D", "3d"),
    ("gerbers", "rail:Gerbers", "gerbers"),
    ("split", "rail:Split", "split"),
    ("mech", "rail:Mech", "mech"),
    ("files", "rail:Files", "files"),
    ("diff", "rail:Diff", "diff"),
    ("review", "rail:Review", "review"),
    ("browser", "rail:Browser", "browser"),
]

HAS_RESOURCE = {
    "full": {"board", "schematic", "3d", "gerbers", "split", "mech", "split-after-mech", "files", "diff", "review", "browser"},
    "board-only": {"board", "3d", "files", "diff", "review", "browser"},
    "schematic-only": {"schematic", "files", "diff", "review", "browser"},
    "mech-only": {"mech", "files", "diff", "review", "browser"},
    "no-design": {"files", "diff", "review", "browser"},
}

# Pages where design-only controls (Open in KiCad, Edit) are expected to be absent.
NON_DESIGN_PAGES = {"files", "diff", "review", "browser"}

# Pages where Follow agent is allowed to appear.
FOLLOW_PAGES = {"board", "schematic", "split", "split-after-mech"}

# Extra shots captured on `full` when `--embed` is enabled (Tasks 7 and 8).
EMBED_SHOTS = [
    "task8-edit",
    "task8-view",
    "task7-sim",
    "task7-sim-run",
]

# Viewer rail entries exercised for Rule B1 (partial coverage: 10 viewer pages; omits Simulation, Terminal, Commands).
# (id, label, rail_control_id, settle_kind, bare_key, palette_query)
B1_ENTRIES = [
    ("board", "Board", "rail:Board", "board", "b", "Board"),
    ("schematic", "Schematic", "rail:Schematic", "schematic", "s", "Schematic"),
    ("3d", "3D", "rail:3D", "3d", "3", "3D"),
    ("gerbers", "Gerbers", "rail:Gerbers", "gerbers", "g", "Gerbers"),
    ("split", "Split", "rail:Split", "split", "v", "Split"),
    ("mech", "Mech", "rail:Mech", "mech", "m", "Mech"),
    ("files", "Files", "rail:Files", "files", "f", "Files"),
    ("diff", "Diff", "rail:Diff", "diff", "d", "Diff"),
    ("review", "Review", "rail:Review", "review", "r", "Review"),
    ("browser", "Browser", "rail:Browser", "browser", "w", "Browser"),
]

B1_OMITTED_ENTRIES = ["simulation", "terminal", "commands"]


def contact_shots() -> list[str]:
    """Ordered shot slugs shown in each kind's contact sheet."""
    out: list[str] = []
    for slug, _, _, _ in PAGES:
        if slug != "idle":
            out.append(f"{slug}-300ms")
        out.append(slug)
    return out


def validate_kinds(kinds: list[str]) -> list[str]:
    """Validate requested project kinds strictly against KINDS (exact names, no path separators or '..')."""
    if not kinds:
        raise ValueError("at least one project kind is required")
    allowed = set(KINDS)
    seen: set[str] = set()
    out: list[str] = []
    for k in kinds:
        if not isinstance(k, str) or "/" in k or "\\" in k or ".." in k or k not in allowed:
            raise ValueError(f"invalid project kind {k!r} (allowed: {', '.join(KINDS)})")
        if k not in seen:
            seen.add(k)
            out.append(k)
    return out


def expected_shots(kind: str, embed: bool = False, schema: int = 3) -> list[str]:
    """Every shot slug that must be captured for `kind` under evidence `schema`."""
    validate_kinds([kind])
    shots = list(contact_shots())
    if kind == "full":
        if schema >= 3:
            shots.extend([
                "task1-back",
                "task2-back",
                "task5-back",
                "task6-back",
                "task9-follow-off",
            ])
        shots.extend([
            "task4-gerbers-vs-board",
            "task5-split-probe",
            "task9-follow-on",
            "task10-closed",
        ])
        for src_slug, _, _ in TASK3_SOURCES:
            shots.append(f"task3-from-{src_slug}")
        if embed:
            shots.extend(EMBED_SHOTS)
    return shots


if __name__ == "__main__":
    import sys
    if len(sys.argv) >= 2 and sys.argv[1] == "validate":
        try:
            valid = validate_kinds(sys.argv[2:])
        except ValueError as e:
            print(f"rail-shots: {e}", file=sys.stderr)
            raise SystemExit(2)
        print("\n".join(valid))
        raise SystemExit(0)
    print("\n".join(KINDS))
