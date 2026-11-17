#!/usr/bin/env python3
"""One KiCad candidate per build cycle: a new candidate supersedes the runs of older ones.

usage: scripts/kicad-candidate-guard.py --new fix/bp13.1-final2 [--sha <new SHA>] [--keep <run id> ...] [--apply]

The fork integrator runs this by hand when it dispatches a candidate. It never runs on its own.
It lists the fork's queued and running runs on candidate branches (--candidate, by default
^fix/bp<N>(.<M>)*-final<k>$) whose SHA is older than the new candidate's. The new SHA defaults to
the head of --new. For each run it compares the elapsed time with the median duration of that
workflow's last successful candidate or release runs. A run due to finish within --margin minutes
(default 30) is kept, because it can still prove what it built. The rest are superseded.

The default is a dry run that only prints. --apply cancels the superseded runs. It never touches:
runs of the new SHA (QA included), release/**, tags (backplane-kicad-v*), main, pull requests, any
non-candidate branch, or a run named with --keep.

The output is a Markdown table of what was cancelled (or would be) and what was kept, followed by
a one-line message. The integrator posts that line with the candidate's message
(docs/release-playbook.md, "Candidates").
"""
import argparse, datetime as dt, json, re, statistics, subprocess, sys

WORKFLOWS = ["backplane-macos.yml", "backplane-linux-release.yml", "backplane-windows.yml", "backplane-qa.yml"]
# Used while a workflow has fewer than 3 successful runs to take a median from (minutes, 0.16.0 cold).
FALLBACK_MIN = {"backplane-macos.yml": 90, "backplane-linux-release.yml": 95,
                "backplane-windows.yml": 150, "backplane-qa.yml": 120}


def gh(path):
    out = subprocess.run(["gh", "api", path], check=True, capture_output=True, text=True).stdout
    return json.loads(out)


def ts(s):
    return dt.datetime.fromisoformat(s.replace("Z", "+00:00"))


def median_minutes(repo, wf, ref_ok):
    runs = gh(f"repos/{repo}/actions/workflows/{wf}/runs?status=success&per_page=50")["workflow_runs"]
    mins = [(ts(r["updated_at"]) - ts(r["run_started_at"])).total_seconds() / 60
            for r in runs if r["event"] != "pull_request" and ref_ok(r["head_branch"] or "")][:20]
    return (statistics.median(mins), len(mins)) if len(mins) >= 3 else (FALLBACK_MIN.get(wf, 120), 0)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--repo", default="BackplaneWorks/Backplane_KiCad")
    ap.add_argument("--new", required=True, help="the new candidate's branch; its own runs are never cancelled")
    ap.add_argument("--sha", help="the new candidate's SHA (default: the head of --new)")
    ap.add_argument("--keep", type=int, nargs="*", default=[], help="run ids never to cancel")
    ap.add_argument("--candidate", default=r"^fix/bp\d+(\.\d+)*-final\d*$", help="regex for candidate branches")
    ap.add_argument("--margin", type=float, default=30, help="keep runs due to finish within this many minutes")
    ap.add_argument("--apply", action="store_true", help="cancel; without it, only report")
    a = ap.parse_args()
    cand = re.compile(a.candidate)
    line = lambda b: bool(cand.match(b)) or b.startswith("release/")
    new_sha = a.sha or gh(f"repos/{a.repo}/commits/{a.new}")["sha"]
    now = dt.datetime.now(dt.timezone.utc)
    cancelled, kept = [], []
    for wf in WORKFLOWS:
        try:
            med, n = median_minutes(a.repo, wf, line)
        except subprocess.CalledProcessError:
            continue  # the workflow doesn't exist on the fork (any more)
        for status in ("in_progress", "queued", "waiting", "pending"):
            for r in gh(f"repos/{a.repo}/actions/workflows/{wf}/runs?status={status}&per_page=100")["workflow_runs"]:
                b = r["head_branch"] or ""
                if (not cand.match(b) or r["event"] == "pull_request" or r["head_sha"].startswith(new_sha)
                        or r["head_repository"]["full_name"] != a.repo):
                    continue
                if r["id"] in a.keep:
                    kept.append(dict(id=r["id"], wf=wf, branch=b, sha=r["head_sha"][:10], elapsed=0.0, median=med,
                                     n=n, left=0.0, url=r["html_url"], why="--keep"))
                    continue
                started = r.get("run_started_at") or r["created_at"]
                elapsed = (now - ts(started)).total_seconds() / 60 if status == "in_progress" else 0.0
                left = med - elapsed
                row = dict(id=r["id"], wf=wf, branch=b, sha=r["head_sha"][:10], elapsed=elapsed, median=med,
                           n=n, left=left, url=r["html_url"], why="due within margin")
                if left <= a.margin:
                    kept.append(row)
                    continue
                if a.apply:
                    subprocess.run(["gh", "api", "-X", "POST", f"repos/{a.repo}/actions/runs/{r['id']}/cancel"],
                                   check=False, capture_output=True)
                cancelled.append(row)

    def table(rows):
        if not rows:
            return "none\n"
        s = "| Run | Workflow | Branch | SHA | Elapsed | Median | Left | Why |\n|---|---|---|---|---|---|---|---|\n"
        for r in rows:
            med = f"{r['median']:.0f} min" + ("" if r["n"] else " (fallback)")
            s += (f"| [{r['id']}]({r['url']}) | {r['wf'].removesuffix('.yml')} | {r['branch']} | {r['sha']} "
                  f"| {r['elapsed']:.0f} min | {med} | {max(r['left'], 0):.0f} min | {r.get('why', 'superseded')} |\n")
        return s

    verb = "Cancelled" if a.apply else "Would cancel (dry run, pass --apply)"
    for r in cancelled:
        r["why"] = "superseded"
    print(f"Candidate {a.new} @ {new_sha[:10]} supersedes older candidate SHAs. Runs due within {a.margin:.0f} min are kept.\n")
    print(f"**{verb}:**\n\n{table(cancelled)}\n**Kept:**\n\n{table(kept)}")
    ids = lambda rows: ", ".join(str(r["id"]) for r in rows) or "none"
    word = "cancelled" if a.apply else "would cancel"
    print(f"Message: candidate {a.new} @ {new_sha[:10]}: {word} {ids(cancelled)}; kept {ids(kept)} "
          f"(due within {a.margin:.0f} min, or --keep).")


if __name__ == "__main__":
    sys.exit(main())
