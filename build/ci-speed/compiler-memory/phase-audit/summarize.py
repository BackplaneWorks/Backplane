#!/usr/bin/env python3
"""Summarize saved V21 journals; never execute a compiler or start a job."""

import argparse
import collections
import gzip
import hashlib
import json
from pathlib import Path


CASES = {
    "hub": ("j20261103204946-2911179", 626),
    "web": ("j20261103204946-2911270", 411),
    "mobile": ("j20261103204947-2911542", 164),
}


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def source_name(name):
    return name.split("/source/", 1)[-1]


def intervals(marks):
    """Only a single book can use unqualified event range IDs unambiguously."""
    starts = [m for m in marks if m["phase"] == "load:start"]
    if len(starts) != 1:
        return {
            "available": False,
            "reason": "Multiple book invocations share range IDs; no book ID in events.",
        }
    pending = {}
    groups = []
    modules = {}
    module_rows = []
    for mark in marks:
        kind = mark["phase"]
        key = (mark.get("from"), mark.get("to"))
        if kind == "events:check:start":
            assert key not in pending, "overlapping unqualified event ranges"
            pending[key] = {"start": mark}
        elif kind == "events:validated":
            assert key in pending and "validated" not in pending[key]
            pending[key]["validated"] = mark
        elif kind == "events:check:end":
            group = pending.pop(key)
            start, validated = group["start"], group["validated"]
            assert start["names"] == validated["names"] == mark["names"]
            kernel = validated["seconds"] - start["seconds"]
            after = mark["seconds"] - validated["seconds"]
            assert kernel >= 0 and after >= 0
            groups.append({
                "from": key[0], "to": key[1],
                "first_name": start["names"][0], "last_name": start["names"][-1],
                "kernel_seconds": kernel, "after_check_seconds": after,
                "validated_rss_bytes": validated["rss_bytes"],
                "end_rss_bytes": mark["rss_bytes"],
            })
        elif kind == "module:check:start":
            assert mark["file"] not in modules
            modules[mark["file"]] = mark
        elif kind == "module:check:end":
            start = modules.pop(mark["file"])
            module_rows.append({
                "file": source_name(mark["file"]),
                "seconds": mark["seconds"] - start["seconds"],
                "added_definitions": mark["added"],
                "end_rss_bytes": mark["rss_bytes"],
                "cumulative_stored_bytes": mark["stored_bytes"],
            })
    return {
        "available": True,
        "completed_groups": len(groups),
        "kernel_seconds": sum(g["kernel_seconds"] for g in groups),
        "after_check_seconds": sum(g["after_check_seconds"] for g in groups),
        "after_check_includes": "spill, cache work, collection and scheduler yields",
        "top_kernel_groups": sorted(groups, key=lambda g: g["kernel_seconds"], reverse=True)[:6],
        "top_after_check_groups": sorted(groups, key=lambda g: g["after_check_seconds"], reverse=True)[:6],
        "top_modules": sorted(module_rows, key=lambda m: m["seconds"], reverse=True)[:8],
        "unfinished_groups": [{
            "from": key[0], "to": key[1],
            "start_seconds": group["start"]["seconds"],
            "validated": "validated" in group,
        } for key, group in pending.items()],
    }


def summarize(jobs_root, phase, job, credits):
    directory = jobs_root / job / "artifacts/ci-packet/emission-v21" / phase
    result_file = directory / "RESULT.json"
    result = json.loads(result_file.read_text())
    assert result["source_commit"] == "33ce69a522ec6de20a7b219923a65a4dce92dc37"
    assert result["source_tree"] == "aa65231bed347542f827abd5ad56292e6f43e4ec"
    assert result["compiler_scope_cap"]["memory_max_bytes"] == 3699998720
    assert result["compiler_scope_cap"]["swap_max"] == 0
    assert result["combined_hard_cap_bytes"] == 4236870912
    profile = directory / "profile.jsonl"
    if not profile.exists():
        profile = directory / "profile.jsonl.gz"
    opener = gzip.open if profile.suffix == ".gz" else open
    with opener(profile, "rt") as source:
        marks = [json.loads(line) for line in source]
    assert all(a["seconds"] <= b["seconds"] for a, b in zip(marks, marks[1:]))
    loads = [{"file": source_name(m["file"]), "seconds": m["seconds"]}
             for m in marks if m["phase"] == "load:start"]
    analyses, analysis_start = [], None
    for mark in marks:
        if mark["phase"] == "analysis:start":
            assert analysis_start is None
            analysis_start = mark
        elif mark["phase"] == "analysis:end":
            assert analysis_start is not None
            analyses.append({"start_seconds": analysis_start["seconds"],
                             "end_seconds": mark["seconds"],
                             "seconds": mark["seconds"] - analysis_start["seconds"],
                             "complete": True})
            analysis_start = None
    if analysis_start is not None:
        analyses.append({"start_seconds": analysis_start["seconds"], "complete": False,
                         "seconds_until_runner_termination": result["seconds"] - analysis_start["seconds"]})
    return {
        "job": job, "credit_seconds": credits, "cost_usd": credits / 100000,
        "inputs": {"result_sha256": digest(result_file),
                   "profile_name": profile.name, "profile_sha256": digest(profile)},
        "scope_seconds": result["seconds"], "raw_exit": result["raw_exit"],
        "timed_out": result["timed_out"],
        "unit_oom_kill": "Result=oom-kill" in result["compiler_unit_result"],
        "final_gnu_peak_rss_kib": result.get("peak_rss_kib"),
        "checkpoint_peak_rss_kib": max(m["max_rss_kib"] for m in marks),
        "load_invocations": loads,
        "counts": dict(collections.Counter(m["phase"] for m in marks)),
        "group_intervals": intervals(marks), "analysis_intervals": analyses,
        "last_memory_sample": result["last_memory_sample"],
        "full_build_qualified": False,
        "output_scope": "source check plus retained bridge reuse" if phase == "mobile"
                        else "no qualified final output",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jobs-root", type=Path,
                        default=Path.home() / ".local/state/backplane/boat/jobs")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = {"schema": 1, "kind": "offline saved V21 phase audit",
              "new_compiler_jobs": 0, "new_boat_cost_usd": 0,
              "limits": ["Completed disjoint group intervals only; partial groups excluded from sums.",
                         "RSS is process-wide, not memory owned exclusively by a phase or file.",
                         "Post-check intervals include storage, GC, cache work and yields, not just I/O.",
                         "Web has overlapping loader invocations with unqualified event range IDs.",
                         "Final GNU RSS is missing on killed/stopped jobs; checkpoint peaks are lower bounds.",
                         "No individual law/proof-time claim; no full proof suite executed."],
              "cases": {phase: summarize(args.jobs_root, phase, *case)
                        for phase, case in CASES.items()}}
    args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
