#!/usr/bin/env python3
"""Single shared manifest for rail-shots.sh, rail_shots_driver.py, and rail_shots_post.py."""
from __future__ import annotations

KINDS = ["full", "schematic-only", "mech-only", "no-design"]

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


def contact_shots() -> list[str]:
    """Ordered shot slugs shown in each kind's contact sheet."""
    out: list[str] = []
    for slug, _, _, _ in PAGES:
        if slug != "idle":
            out.append(f"{slug}-300ms")
        out.append(slug)
    return out


def expected_shots(kind: str, embed: bool = False) -> list[str]:
    """Every shot slug that must be captured for `kind`."""
    shots = list(contact_shots())
    if kind == "full":
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
