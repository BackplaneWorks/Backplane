#!/usr/bin/env python3
r"""Gates for decision 5: Board and Schematic open straight into embedded KiCad where it is installed and local.

`--embed` only, `full` kind only, every gate "observed or unavailable": a gate that cannot see what it claims is `unavailable`,
never a pass. Uses only the client's existing script step `{"kicad":{"attach":...,"why":"auto"|"deliberate"|"edit"}}`, real X keys
(the watcher's `auto-act-*` shots), the hub's own inventory and the client's `GPUI_LOG` lines. No KiCad pane code is touched.

Gates (check ids `full/auto-*`):
  a  an Automatic attach shows KiCad's frame in the pane and does NOT take the keyboard (marker typed before, during the cold
     start and after the frame appears: every key lands in the composer)
  b  a Deliberate attach after it promotes and takes the keyboard
  c  Board <-> Schematic under Automatic is a frame switch (one KiCad process); the geometry-push count is `unavailable`
  d  View detaches with keep-warm and the attach again is no cold start; d2 the same by leaving the page with the rail (reconcile's job)
  e  person-closed hold: after KiCad's own Close an Automatic attach is refused; Edit clears the hold
  f  stale generation: an attach with an old generation is answered `stale:true` and changes nothing
  g  setting "Always viewer": `unavailable` until the reconcile exists

Observables new to this slice are only taken from existing places: pane report (`ok`, `in_gpui_tree`, `shown`, `keys`, `holder`,
`pid`, `sock`), hub `hub.status` inventory (`runtime.{session,holder,generation}`, `pid`), `GPUI_LOG` (`kicad_info`, `kicad_attach`,
`kicad_rpc_sent`, `kicad_auto_keys_taken`, `kicad_keys_flip`, `kicad_auto_to_deliberate`, `kicad_closed_by_person`, `kicad_stale`).
"""

from __future__ import annotations

from pathlib import Path

# What the watcher types with real X keys at each `auto-act-type-<tag>` shot (lower-case letters only: no shift, no KiCad hotkey
# that opens a dialog is ever typed into KiCad by a passing run).
MARKERS = {"pre": "qpre", "mid": "qmid", "post": "qpost"}

GATES = ("a", "b", "c", "d", "e", "f", "g")

# The shots the gates read (each is captured, privacy-checked and carries the hub's session inventory); the `auto-act-*` shots only
# release the watcher's action and are not evidence.
AUTO_SHOTS = (
    "auto-a-frame", "auto-b-frame", "auto-c-board", "auto-f-after", "auto-c-schematic", "auto-c-back",
    "auto-d-view-left", "auto-d-view-back", "auto-d-left", "auto-d-back", "auto-e-refused", "auto-e-edit",
)


def is_action_only_shot(name: str) -> bool:
    """`auto-act-*` shots only release the watcher's action: no screenshot is saved for them (nothing to review, nothing to leak)."""
    return name.startswith("auto-act-")


def build_auto_steps(settle_timeout_ms: int = 15000) -> list[dict]:
    """The scripted half of the gates, scheduled AFTER every existing task and check so earlier measurements are untouched. Marks, the
    existing attach step, reads, reports and shots; the `auto-act-*` shots make the watcher act BEFORE the script goes on."""
    tmo = min(settle_timeout_ms, 8000)
    painted = 90000
    return [
        # a clean viewer, then KiCad is killed (a crash is not the person's close) so the first Automatic attach is a cold start
        {"keys": "ctrl-l"},
        {"wait": 60},
        {"keys": "escape"},
        {"wait": 60},
        {"keys": "escape"},
        {"wait": 100},
        {"click": "ui:View"},
        {"wait": 1500},
        {"mark": "auto:start"},
        {"read": "composer"},
        {"shot": "auto-act-kill", "settle": "", "timeout_ms": tmo},
        {"wait": 5000},
        # a: "pre" is typed BEFORE the attach; "mid" right after the attach request while KiCad starts; "post" once the frame is up
        {"mark": "auto:a:pre"},
        {"shot": "auto-act-type-pre", "settle": "", "timeout_ms": tmo},
        {"read": "composer"},
        {"mark": "auto:a:attach"},
        {"press": "rail:Board"},
        {"kicad": {"attach": "board", "why": "auto", "tag": "auto-a-attach"}},
        {"mark": "auto:a:mid"},
        {"shot": "auto-act-type-mid", "settle": "", "timeout_ms": tmo},
        {"read": "composer"},
        {"kicad": {"wait_painted": painted, "kind": "PCB", "tag": "auto-a-painted"}},
        {"mark": "auto:a:painted"},
        {"shot": "auto-act-type-post", "settle": "", "timeout_ms": tmo},
        {"read": "composer"},
        {"kicad": {"report": True, "tag": "auto-a-report"}},
        {"read": "rail"},
        {"wait": 200},
        {"mark": "auto:a:done"},
        {"shot": "auto-a-frame", "settle": "", "timeout_ms": tmo},
        # b: a Deliberate attach promotes the automatic pane and takes the keyboard
        {"mark": "auto:b"},
        {"kicad": {"attach": "board", "why": "deliberate", "tag": "auto-b-attach"}},
        {"wait": 3000},
        {"kicad": {"report": True, "tag": "auto-b-report"}},
        {"mark": "auto:b:done"},
        {"shot": "auto-b-frame", "settle": "", "timeout_ms": tmo},
        # c (+ f, a generation that does not exist yet): back to the viewer, then Automatic Board -> Schematic -> Board
        {"click": "ui:View"},
        {"wait": 2500},
        {"mark": "auto:c:board"},
        {"kicad": {"attach": "board", "why": "auto", "tag": "auto-c-board-attach"}},
        {"kicad": {"wait_painted": painted, "kind": "PCB", "tag": "auto-c-board-painted"}},
        {"kicad": {"report": True, "tag": "auto-c-board-report"}},
        {"shot": "auto-c-board", "settle": "", "timeout_ms": tmo},
        {"mark": "auto:f"},
        {"shot": "auto-act-stale-future", "settle": "", "timeout_ms": tmo},
        {"wait": 500},
        {"mark": "auto:f:done"},
        {"shot": "auto-f-after", "settle": "", "timeout_ms": tmo},
        {"mark": "auto:c:schematic"},
        {"kicad": {"attach": "schematic", "why": "auto", "tag": "auto-c-sch-attach"}},
        {"kicad": {"wait_painted": painted, "kind": "SCH", "tag": "auto-c-sch-painted"}},
        {"kicad": {"report": True, "tag": "auto-c-sch-report"}},
        {"shot": "auto-c-schematic", "settle": "", "timeout_ms": tmo},
        {"mark": "auto:c:back"},
        {"kicad": {"attach": "board", "why": "auto", "tag": "auto-c-back-attach"}},
        {"kicad": {"wait_painted": painted, "kind": "PCB", "tag": "auto-c-back-painted"}},
        {"kicad": {"report": True, "tag": "auto-c-back-report"}},
        {"mark": "auto:c:done"},
        {"shot": "auto-c-back", "settle": "", "timeout_ms": tmo},
        # d: View detaches the automatic pane (KiCad kept warm); the attach again is no cold start
        {"mark": "auto:d:view-leave"},
        {"click": "ui:View"},
        {"wait": 2500},
        {"kicad": {"report": True, "tag": "auto-d-view-left-report"}},
        {"shot": "auto-d-view-left", "settle": "", "timeout_ms": tmo},
        {"mark": "auto:d:view-back"},
        {"kicad": {"attach": "board", "why": "auto", "tag": "auto-d-view-back-attach"}},
        {"kicad": {"wait_painted": 30000, "kind": "PCB", "tag": "auto-d-view-back-painted"}},
        {"kicad": {"report": True, "tag": "auto-d-view-back-report"}},
        {"mark": "auto:d:view-done"},
        {"shot": "auto-d-view-back", "settle": "", "timeout_ms": tmo},
        # d2: leaving to another page by the rail (the reconcile's job to detach)
        {"mark": "auto:d:leave"},
        {"press": "rail:Files"},
        {"wait": 2500},
        {"kicad": {"report": True, "tag": "auto-d-left-report"}},
        {"shot": "auto-d-left", "settle": "", "timeout_ms": tmo},
        {"mark": "auto:d:back"},
        {"press": "rail:Board"},
        {"kicad": {"attach": "board", "why": "auto", "tag": "auto-d-back-attach"}},
        {"kicad": {"wait_painted": 30000, "kind": "PCB", "tag": "auto-d-back-painted"}},
        {"kicad": {"report": True, "tag": "auto-d-back-report"}},
        {"mark": "auto:d:done"},
        {"shot": "auto-d-back", "settle": "", "timeout_ms": tmo},
        # e: KiCad's own Close (the watcher closes it on its socket), an Automatic attach, then Edit
        {"mark": "auto:e:close"},
        {"shot": "auto-act-close", "settle": "", "timeout_ms": tmo},
        {"wait": 8000},
        {"mark": "auto:e:refused"},
        {"kicad": {"attach": "board", "why": "auto", "tag": "auto-e-auto-attach"}},
        {"wait": 3000},
        {"read": "pane"},
        {"kicad": {"report": True, "tag": "auto-e-refused-report"}},
        {"mark": "auto:e:refused-done"},
        {"shot": "auto-e-refused", "settle": "", "timeout_ms": tmo},
        {"mark": "auto:e:edit"},
        {"kicad": {"attach": "board", "why": "edit", "tag": "auto-e-edit-attach"}},
        {"wait": 3000},
        {"kicad": {"wait_painted": painted, "kind": "PCB", "tag": "auto-e-edit-painted"}},
        {"kicad": {"report": True, "tag": "auto-e-edit-report"}},
        {"mark": "auto:e:done"},
        {"shot": "auto-e-edit", "settle": "", "timeout_ms": tmo},
        # f: an OLD generation, which exists now that Edit started a new KiCad
        {"mark": "auto:f:old"},
        {"shot": "auto-act-stale-old", "settle": "", "timeout_ms": tmo},
        {"wait": 500},
        # back to a clean viewer
        {"click": "ui:View"},
        {"wait": 1500},
        {"keys": "ctrl-l"},
        {"wait": 60},
        {"keys": "escape"},
        {"wait": 60},
        {"keys": "escape"},
        {"wait": 60},
        {"wait_settled": "", "tag": "auto-end-idle", "timeout_ms": min(tmo, 3000)},
    ]


# ---------------------------------------------------------------- evaluation helpers

def _rec(gate_id: str, name: str, outcome: str, note: str = "", evidence: dict | None = None) -> dict:
    return {"id": f"full/auto-{gate_id}", "kind": "full", "name": name, "outcome": outcome, "note": note, "evidence": evidence or {}}


def _is_int(v: object) -> bool:
    return isinstance(v, int) and not isinstance(v, bool)


def _sessions(capture: dict | None) -> tuple[dict | None, str]:
    """The hub's session inventory at a shot: `{key: session}` only when complete, else None and why."""
    hk = (capture or {}).get("hub_kicad_sessions")
    if not isinstance(hk, dict) or not (hk.get("ok") and hk.get("inventory_complete")):
        return None, "no complete hub session inventory at this shot"
    return dict(hk.get("sessions") or {}), ""


def _only_session(capture: dict | None) -> tuple[dict | None, str]:
    sess, why = _sessions(capture)
    if sess is None:
        return None, why
    if len(sess) != 1:
        return None, f"expected exactly one KiCad session in the hub inventory, got {len(sess)}"
    return next(iter(sess.values())), ""


def _runtime(session: dict | None) -> dict | None:
    """The runtime block only when the hub really sent a COMPLETE one (`session` and `generation`): the hub omits `holder` when no
    window holds the pane, which counts as absence only inside a complete block."""
    rt = (session or {}).get("runtime")
    if isinstance(rt, dict) and isinstance(rt.get("session"), str) and _is_int(rt.get("generation")):
        return rt
    return None


def state_of(session: dict | None) -> dict | None:
    """A complete, comparable picture of a KiCad session (pid, runtime generation/session/holder, shown frames); None if any part is missing."""
    rt = _runtime(session)
    frames = (session or {}).get("frames")
    pid = (session or {}).get("pid")
    if rt is None or not isinstance(frames, list) or not _is_int(pid) or pid <= 0:
        return None
    return {"pid": pid, "generation": rt["generation"], "session": rt["session"], "holder": rt.get("holder"),
            "shown_frames": sorted(str(f.get("kind")) for f in frames if isinstance(f, dict) and f.get("shown") is True)}


def _report(kicad_tags: dict, tag: str) -> dict | None:
    r = kicad_tags.get(tag)
    return r if isinstance(r, dict) else None


def _attached(report: dict | None) -> bool | None:
    """True: mounted and shown. False: detached (the pane says it is not editing, or is not in the tree). None: no real report, or in the
    tree with no frame shown (neither mounted nor detached)."""
    if not isinstance(report, dict):
        return None
    if report.get("ok") is False and report.get("error") == "not editing":
        return False
    if report.get("ok") is True and isinstance(report.get("in_gpui_tree"), bool) and "shown" in report:
        if not report["in_gpui_tree"]:
            return False
        return True if report.get("shown") else None
    return None


def _painted_ok(report: dict | None, kind: str) -> bool:
    """What `wait_painted` calls success: ready, a frame shown, of this kind, painted."""
    if not isinstance(report, dict) or report.get("ok") is not True or report.get("state") != "ready":
        return False
    shown = report.get("shown")
    if not _is_int(shown) or shown <= 0:
        return False
    for f in report.get("frames") or []:
        if isinstance(f, dict) and f.get("xid") == shown and f.get("kind") == kind and _is_int(f.get("painted")) and f["painted"] >= 15:
            return True
    return False


def _logs_between(logs: list[dict], first_seq: int | None, last_seq: int | None, key: str) -> list[dict]:
    if first_seq is None or last_seq is None:
        return []
    return [lg for lg in logs if first_seq <= lg["seq"] <= last_seq and key in lg["obj"]]


def _composer_reads_in(reads: list[dict], lo: int | None, hi: int | None) -> list[dict]:
    if lo is None or hi is None:
        return []
    return [r for r in reads if lo <= r["seq"] <= hi and isinstance(r.get("text"), str)]


def _shot_ok(caps: dict, name: str) -> tuple[bool, str]:
    """The shot was taken, inspected and unobstructed. Gates read metrics and the hub inventory, never pixels, so the per-shot privacy
    clearance (which decides what may be shared) is not a precondition."""
    cap = caps.get(name) or {}
    if not (cap.get("ok") and cap.get("inspect_ok") is True):
        return False, f"capture {name} missing or not inspected"
    if cap.get("obstructed"):
        return False, f"capture {name} obstructed by a native dialog"
    return True, ""


def _action(actions: list[dict], action: str) -> list[dict]:
    return [a for a in actions if a.get("action") == action]


def _delivered(presses: dict, mark: str, press_id: str) -> bool:
    return any(p.get("id") == press_id and p.get("found") is True for p in (presses.get(mark) or []))


def _pid(report: dict | None) -> int | None:
    p = (report or {}).get("pid")
    return p if _is_int(p) and p > 0 else None


def _rail_lit_between(parsed: dict, lo: int | None, hi: int | None) -> list[str] | None:
    """The last rail snapshot drawn after `lo` and not after `hi` (None: no snapshot in that window)."""
    if lo is None or hi is None:
        return None
    last = None
    for s, rail in (parsed.get("rail_events") or []):
        if lo < s <= hi and isinstance(rail, dict) and isinstance(rail.get("lit"), list):
            last = list(rail["lit"])
    return last


# ---------------------------------------------------------------- the gates

def evaluate_auto_gates(
    outdir: Path,
    parsed_full: dict,
    caps_full: dict[str, dict],
    actions: list[dict],
    cleared_shots: set[str] | None = None,
) -> list[dict]:
    """Every gate as a check record; `unavailable` whenever the run did not observe what the gate claims. `cleared_shots` is accepted
    for symmetry with the other evaluators and deliberately not read (see `_shot_ok`)."""
    marks = parsed_full.get("mark_seqs") or {}
    if not any(m.startswith("auto:") for m in marks):
        return []  # the auto steps were not scheduled in this run (not --embed): no check records at all
    kt = parsed_full.get("kicad_tags") or {}
    logs: list[dict] = parsed_full.get("gpui_logs") or []
    comp: list[dict] = parsed_full.get("composer_reads") or []
    presses = parsed_full.get("presses") or {}
    return [
        _gate_a(marks, kt, logs, comp, caps_full, actions, parsed_full),
        _gate_b(marks, kt, logs, caps_full),
        _gate_c(marks, kt, logs, caps_full),
        _gate_d(marks, kt, logs, caps_full, presses),
        _gate_d2(marks, kt, logs, caps_full, presses),
        _gate_e(marks, kt, logs, caps_full, actions),
        _gate_f(actions),
        _rec("g-always-viewer", "With the setting \"Always viewer\" no Automatic attach happens", "unavailable",
             "needs the rail reconcile: the client issues no Automatic attach from `viewer.mode` yet (the hub's AUTO_PLATFORMS is empty) "
             "and the setting is not in the client; nothing to observe until then"),
    ]


KEY_LOGS = ("kicad_auto_keys_taken", "kicad_auto_to_deliberate", "kicad_key", "kicad_key_dispatch")


def _gate_a(marks, kt, logs, comp, caps, actions, parsed) -> dict:
    gid = "a-frame-no-keyboard"
    name = "Automatic attach shows KiCad in the pane and never takes the keyboard (marker typed before, during the start, after)"
    typed = {t: _action(actions, f"type-{t}") for t in MARKERS}
    for t, acts in typed.items():
        if not acts or acts[-1].get("ok") is not True:
            return _rec(gid, name, "unavailable", f"real X typing of the `{t}` marker was not delivered ({acts[-1].get('error') if acts else 'no action recorded'})")
    ok, why = _shot_ok(caps, "auto-a-frame")
    if not ok:
        return _rec(gid, name, "unavailable", why)
    sess, why = _only_session(caps.get("auto-a-frame"))
    if sess is None:
        return _rec(gid, name, "unavailable", why)
    rt = _runtime(sess)
    rep = _report(kt, "auto-a-report")
    painted = _report(kt, "auto-a-painted")
    if rt is None or not isinstance(rep, dict) or not isinstance(painted, dict):
        return _rec(gid, name, "unavailable", "no complete hub runtime block, or no real pane report / paint report to observe the frame")
    reads = _composer_reads_in(comp, marks.get("auto:start"), marks.get("auto:a:done"))
    if len(reads) < 4 or marks.get("auto:a:attach") is None:
        return _rec(gid, name, "unavailable", f"expected 4 composer reads (start, pre, mid, post) and the attach mark, got {len(reads)} reads")
    start, r_pre, r_mid, r_post = (r["text"] for r in reads[:4])
    fails: list[str] = []
    # "pre" was typed BEFORE the attach request
    if not reads[1]["seq"] < marks["auto:a:attach"]:
        return _rec(gid, name, "unavailable", "the `pre` composer read is not before the attach request")
    exp = [start + MARKERS["pre"], start + MARKERS["pre"] + MARKERS["mid"], start + MARKERS["pre"] + MARKERS["mid"] + MARKERS["post"]]
    for label, got, want in (("before", r_pre, exp[0]), ("during", r_mid, exp[1]), ("after", r_post, exp[2])):
        if got != want:
            fails.append(f"composer after typing {label} the attach is {got!r}, expected {want!r} (keys went elsewhere)")
    if not _painted_ok(painted, "PCB"):
        fails.append(f"the Automatic attach did not end in a painted PCB frame (paint report state={painted.get('state')!r})")
    if _attached(rep) is not True:
        fails.append("KiCad's frame is not mounted and shown in the pane")
    for tag, r in (("auto-a-painted", painted), ("auto-a-report", rep)):
        if r.get("keys") is not False:
            fails.append(f"the pane report {tag} says keys={r.get('keys')!r}: KiCad has the keyboard")
    if not rt.get("holder"):
        fails.append("hub runtime.holder is absent: the pane is not held by a client")
    lit = _rail_lit_between(parsed, marks.get("auto:a:painted"), marks.get("auto:a:done"))
    if lit is None:
        return _rec(gid, name, "unavailable", "no fresh rail snapshot after the frame appeared")
    if "Board" not in lit:
        fails.append(f"rail does not light Board (lit {lit})")
    window = (marks.get("auto:a:attach"), marks.get("auto:a:done"))
    taken = [list(lg["obj"])[0] for key in ("kicad_auto_keys_taken", "kicad_auto_to_deliberate") for lg in _logs_between(logs, *window, key)]
    taken += ["kicad_keys_flip"] * len([lg for lg in _logs_between(logs, *window, "kicad_keys_flip") if (lg["obj"]["kicad_keys_flip"] or {}).get("to") is True])
    kicad_keys = [lg["obj"] for key in ("kicad_key", "kicad_key_dispatch") for lg in _logs_between(logs, *window, key)]
    if taken:
        fails.append(f"the client says KiCad took the keyboard ({taken})")
    if kicad_keys:
        fails.append(f"KiCad's container saw key events ({kicad_keys[:2]})")
    mid = typed["mid"][-1]
    mid_state = mid.get("kicad_state_before")
    ev = {"composer": {"start": start, "pre": r_pre, "mid": r_mid, "post": r_post}, "report": {k: rep.get(k) for k in ("ok", "in_gpui_tree", "shown", "keys", "pid", "state")},
          "hub_runtime": rt, "rail_lit": lit, "mid_typed_while_kicad_state": mid_state, "mid_typing_after_state": mid.get("kicad_state_after"),
          "kicad_side": "composer received all; KiCad-side: no kicad_key / kicad_key_dispatch log in the interval (those fire for keys KiCad's container receives)",
          "holder_is_this_client": "inferred: the single GPUI client of the run is the only possible holder (the client's own connection id has no observable)"}
    if fails:
        return _rec(gid, name, "failed", "; ".join(fails), ev)
    if mid_state != "starting":
        return _rec(gid, name, "unavailable", f"composer received all of the marker and KiCad took none, but `mid` began when KiCad's state was {mid_state!r}, not `starting`: the typing during the start is not proven", ev)
    return _rec(gid, name, "passed", "", ev)


def _gate_b(marks, kt, logs, caps) -> dict:
    gid, name = "b-deliberate-promotes", "A Deliberate attach after an Automatic one promotes the pane and takes the keyboard"
    rep = _report(kt, "auto-b-report")
    att = _attached(rep)
    if not isinstance(rep, dict) or rep.get("ok") is not True:
        return _rec(gid, name, "unavailable", "no real pane report after the deliberate attach")
    ok, why = _shot_ok(caps, "auto-b-frame")
    if not ok:
        return _rec(gid, name, "unavailable", why)
    promoted = _logs_between(logs, marks.get("auto:b"), marks.get("auto:b:done"), "kicad_auto_to_deliberate")
    ev = {"keys": rep.get("keys"), "attached": att, "promoted_log": [p["obj"] for p in promoted]}
    fails = []
    if att is not True:
        fails.append("the pane is not mounted/shown after the deliberate attach")
    if rep.get("keys") is not True:
        fails.append(f"the pane reports keys={rep.get('keys')!r}: KiCad does not have the keyboard")
    if not promoted:
        fails.append("no `kicad_auto_to_deliberate` between the attach and its report")
    return _rec(gid, name, "failed" if fails else "passed", "; ".join(fails), ev)


def _gate_c(marks, kt, logs, caps) -> dict:
    gid, name = "c-frame-switch", "Board <-> Schematic under Automatic is a frame switch on one KiCad process"
    steps = (("auto-c-board", "auto-c-board-report", "auto-c-board-painted", "PCB"),
             ("auto-c-schematic", "auto-c-sch-report", "auto-c-sch-painted", "SCH"),
             ("auto-c-back", "auto-c-back-report", "auto-c-back-painted", "PCB"))
    reps, paints, hub_pids = [], [], []
    for shot, rtag, ptag, _kind in steps:
        r, pt = _report(kt, rtag), _report(kt, ptag)
        if r is None or pt is None or _attached(r) is None:
            return _rec(gid, name, "unavailable", f"pane or paint report missing or not real ({rtag}/{ptag})")
        ok, why = _shot_ok(caps, shot)
        if not ok:
            return _rec(gid, name, "unavailable", why)
        s, why = _only_session(caps.get(shot))
        if s is None:
            return _rec(gid, name, "unavailable", why)
        reps.append(r)
        paints.append(pt)
        hub_pids.append(s.get("pid"))
    pids = [_pid(r) for r in reps]
    ev = {"pane_pids": pids, "hub_pids": hub_pids, "frames": [r.get("shown") for r in reps],
          "geometry_pushes": "unavailable: the client has no per-switch geometry-push observable on Linux (`kicad_host_push` is macOS-only)"}
    fails = []
    for (shot, _r, _p, kind), pt in zip(steps, paints):
        if not _painted_ok(pt, kind):
            fails.append(f"{shot}: no painted {kind} frame at the end of its attach (paint report state={pt.get('state')!r})")
    if any(p is None for p in pids) or len(set(pids)) != 1:
        fails.append(f"pane pids differ or are missing: {pids}")
    if any(h != p for h, p in zip(hub_pids, pids)):
        fails.append(f"hub session pid does not equal the pane's pid at each step: hub {hub_pids}, pane {pids}")
    if any(_attached(r) is not True for r in reps):
        fails.append("a frame was not mounted and shown in the pane after an attach")
    if not (reps[0].get("shown") and reps[1].get("shown") and reps[0].get("shown") != reps[1].get("shown") and reps[0].get("shown") == reps[2].get("shown")):
        fails.append(f"Board and Schematic are not two different frames of the one KiCad (shown {ev['frames']})")
    sent = _logs_between(logs, marks.get("auto:c:schematic"), marks.get("auto:c:done"), "kicad_rpc_sent")
    ev["rpcs_sent_during_switches"] = [s["obj"]["kicad_rpc_sent"] for s in sent]
    if fails:
        return _rec(gid, name, "failed", "; ".join(fails), ev)
    return _rec(gid, name, "unavailable",
                "measured: each switch ended in a painted PCB/SCH frame and the hub's pid equals the pane's pid every time (one process, two frames). "
                "Unproven: one geometry push per switch (no client observable on Linux), so the gate as a whole is not earned", ev)


def _keep_warm_fails(left: dict, rt: dict) -> list[str]:
    fails = []
    if _attached(left) is not False:
        fails.append("the pane is not detached after leaving (mounted, or in the tree with no frame shown)")
    if rt.get("holder"):
        fails.append(f"hub runtime.holder is still {rt.get('holder')!r} after leaving")
    if rt.get("session") not in ("live", "parked"):
        fails.append(f"KiCad session is {rt.get('session')!r} after leaving, not live/parked (not kept warm)")
    return fails


def _return_fails(before: dict, back: dict, painted: dict | None, s_back: dict, starting: list) -> list[str]:
    fails = []
    if _attached(back) is not True:
        fails.append("coming back did not mount and show the frame")
    if not _painted_ok(painted, "PCB"):
        fails.append("coming back did not end in a painted PCB frame")
    if _pid(before) is None or _pid(before) != _pid(back) or s_back.get("pid") != _pid(back):
        fails.append("a different KiCad process after coming back (a cold start)")
    if starting:
        fails.append("KiCad went through `starting` on the way back (a cold start)")
    return fails


def _gate_d(marks, kt, logs, caps, presses) -> dict:
    gid, name = "d-keep-warm", "Leaving by View detaches the pane with KiCad kept warm; coming back re-attaches without a cold start"
    if not _delivered(presses, "auto:d:view-leave", "ui:View"):
        return _rec(gid, name, "unavailable", "the `ui:View` click was not delivered (no executed action) under auto:d:view-leave")
    left, back, before = _report(kt, "auto-d-view-left-report"), _report(kt, "auto-d-view-back-report"), _report(kt, "auto-c-back-report")
    painted = _report(kt, "auto-d-view-back-painted")
    if not isinstance(left, dict) or not isinstance(back, dict) or not isinstance(before, dict) or not isinstance(painted, dict):
        return _rec(gid, name, "unavailable", "a pane report is missing or not real")
    for shot in ("auto-d-view-left", "auto-d-view-back"):
        ok, why = _shot_ok(caps, shot)
        if not ok:
            return _rec(gid, name, "unavailable", why)
    s_left, why = _only_session(caps.get("auto-d-view-left"))
    s_back, why2 = _only_session(caps.get("auto-d-view-back"))
    if s_left is None or s_back is None:
        return _rec(gid, name, "unavailable", why or why2)
    rt = _runtime(s_left)
    if rt is None:
        return _rec(gid, name, "unavailable", "the hub reported no complete runtime block (session and generation) while the pane was away")
    starting = [lg for lg in _logs_between(logs, marks.get("auto:d:view-back"), marks.get("auto:d:view-done"), "kicad_info") if lg["obj"].get("state") == "starting"]
    ev = {"left_state": _attached(left), "runtime_when_left": rt, "pid_before": _pid(before), "pid_after": _pid(back), "hub_pid_after": s_back.get("pid"), "starting_infos_on_return": len(starting)}
    fails = _keep_warm_fails(left, rt) + _return_fails(before, back, painted, s_back, starting)
    return _rec(gid, name, "failed" if fails else "passed", "; ".join(fails), ev)


def _gate_d2(marks, kt, logs, caps, presses) -> dict:
    """Leaving to another page by the rail: detaching an Automatic pane there is the rail reconcile's job. Never a pass unless Files was
    observed open, `rail:Files` was delivered and the pane really is detached."""
    gid, name = "d2-leave-page-detaches", "Leaving the design page by the rail detaches the pane (keep-warm) and coming back is no cold start"
    if not _delivered(presses, "auto:d:leave", "rail:Files"):
        return _rec(gid, name, "unavailable", "the `rail:Files` press was not delivered (no executed action) under auto:d:leave")
    left, back, before = _report(kt, "auto-d-left-report"), _report(kt, "auto-d-back-report"), _report(kt, "auto-d-view-back-report")
    painted = _report(kt, "auto-d-back-painted")
    if not isinstance(left, dict) or not isinstance(back, dict) or not isinstance(before, dict) or not isinstance(painted, dict):
        return _rec(gid, name, "unavailable", "a pane report is missing or not real")
    if left.get("open") != "files":
        return _rec(gid, name, "unavailable", f"Files was not observed open after the rail press (pane report open={left.get('open')!r}): nothing was left", {"left_open": left.get("open")})
    for shot in ("auto-d-left", "auto-d-back"):
        ok, why = _shot_ok(caps, shot)
        if not ok:
            return _rec(gid, name, "unavailable", why)
    s_left, why = _only_session(caps.get("auto-d-left"))
    s_back, why2 = _only_session(caps.get("auto-d-back"))
    if s_left is None or s_back is None:
        return _rec(gid, name, "unavailable", why or why2)
    rt = _runtime(s_left)
    if rt is None:
        return _rec(gid, name, "unavailable", "the hub reported no complete runtime block while the page was left")
    ev = {"left_open": left.get("open"), "left_state": _attached(left), "runtime_when_left": rt, "pid_before": _pid(before), "pid_after": _pid(back)}
    if _attached(left) is not False:
        return _rec(gid, name, "unavailable", "observed: the pane stayed in the tree over the page the rail went to; detaching on a page change is the reconcile's job, not built yet", ev)
    starting = [lg for lg in _logs_between(logs, marks.get("auto:d:back"), marks.get("auto:d:done"), "kicad_info") if lg["obj"].get("state") == "starting"]
    fails = _keep_warm_fails(left, rt) + _return_fails(before, back, painted, s_back, starting)
    return _rec(gid, name, "failed" if fails else "passed", "; ".join(fails), ev)


def _gate_e(marks, kt, logs, caps, actions) -> dict:
    gid, name = "e-person-closed-hold", "After KiCad's own Close an Automatic attach is refused (viewer shows); Edit clears the hold"
    close = _action(actions, "close")
    if not close or close[-1].get("ok") is not True:
        return _rec(gid, name, "unavailable", f"KiCad's own Close was not delivered ({close[-1].get('error') if close else 'no action recorded'})")
    needed = ("auto:e:close", "auto:e:refused", "auto:e:refused-done", "auto:e:edit", "auto:e:done")
    missing = [m for m in needed if marks.get(m) is None]
    if missing:
        return _rec(gid, name, "unavailable", f"interval boundaries missing: {missing}")
    for shot in ("auto-e-refused", "auto-e-edit"):
        ok, why = _shot_ok(caps, shot)
        if not ok:
            return _rec(gid, name, "unavailable", why)
    refused, edit, before = _report(kt, "auto-e-refused-report"), _report(kt, "auto-e-edit-report"), _report(kt, "auto-d-back-report")
    painted = _report(kt, "auto-e-edit-painted")
    if not isinstance(refused, dict) or not isinstance(edit, dict) or not isinstance(painted, dict):
        return _rec(gid, name, "unavailable", "a pane report is missing or not real")
    if _pid(before) is None or _pid(edit) is None:
        return _rec(gid, name, "unavailable", f"the KiCad pid is missing before ({_pid(before)}) or after Edit ({_pid(edit)}): a new KiCad cannot be shown")
    closed = _logs_between(logs, marks["auto:e:close"], marks["auto:e:refused"], "kicad_closed_by_person")
    refusals = [lg for lg in _logs_between(logs, marks["auto:e:refused"], marks["auto:e:refused-done"], "kicad_attach") if lg["obj"].get("kicad_attach") == "refused"]
    sent = [lg for lg in _logs_between(logs, marks["auto:e:refused"], marks["auto:e:refused-done"], "kicad_rpc_sent") if lg["obj"].get("kicad_rpc_sent") == "editor.kicad.attach"]
    s_ref, _why = _sessions(caps.get("auto-e-refused"))
    ev = {"closed_by_person_logged": bool(closed), "refusals": [r["obj"] for r in refusals], "attach_rpcs_after_refusal": len(sent),
          "pid_before": _pid(before), "pid_after_edit": _pid(edit), "hub_sessions_after_close": [(s.get("runtime") or {}).get("session") for s in (s_ref or {}).values()]}
    fails = []
    if not closed:
        fails.append("the pane never reported `kicad_closed_by_person` after KiCad's own Close")
    if not any(r["obj"].get("why") == "closed by the person" for r in refusals):
        fails.append("the Automatic attach after the Close was not refused as 'closed by the person'")
    if sent:
        fails.append("an `editor.kicad.attach` was sent after the refusal")
    if _attached(refused) is not False:
        fails.append("the pane is mounted (or in the tree) after a refused Automatic attach (the viewer page should show)")
    if _attached(edit) is not True or not _painted_ok(painted, "PCB"):
        fails.append("Edit did not open KiCad again with a painted PCB frame")
    if _pid(edit) == _pid(before):
        fails.append("Edit did not start a new KiCad (same pid)")
    return _rec(gid, name, "failed" if fails else "passed", "; ".join(fails), ev)


def _gate_f(actions) -> dict:
    gid, name = "f-stale-generation", "An attach carrying a stale generation gets stale:true and changes nothing"
    fut = _action(actions, "stale-future")
    old = _action(actions, "stale-old")
    results: dict = {}
    fails: list[str] = []
    for label, acts in (("future", fut), ("old", old)):
        if not acts or acts[-1].get("ok") is not True:
            results[label] = f"not run: {acts[-1].get('error') if acts else 'no action recorded'}"
            continue
        a = acts[-1]
        before, after = a.get("before"), a.get("after")
        if not isinstance(before, dict) or not isinstance(after, dict):
            results[label] = "unavailable: the state before or after the attach was incomplete (pid, generation, session, holder, shown frames); missing is not equal"
            continue
        if a.get("control_ok") is not True:
            results[label] = f"unavailable: the control (the running generation) did not succeed (reply error {a.get('control_error')!r})"
            continue
        r = {"sent_generation": a.get("sent_generation"), "running_generation": a.get("generation"), "stale": a.get("stale"), "control_stale": a.get("control_stale"), "before": before, "after": after}
        results[label] = r
        if a.get("stale") is not True:
            fails.append(f"{label}: the attach was not answered stale (stale={a.get('stale')!r})")
        if a.get("control_stale") is True:
            fails.append(f"{label}: the control (the running generation) was answered stale too")
        if before != after:
            fails.append(f"{label}: the stale attach changed the state {before} -> {after}")
    measured = [k for k, v in results.items() if isinstance(v, dict)]
    ev = {"probes": results, "note": "`future` is generation+7 (a generation that does not exist yet); `old` is the previous KiCad's generation after Edit restarted it"}
    if fails:
        return _rec(gid, name, "failed", "; ".join(fails), ev)
    if "future" not in measured or "old" not in measured:
        missing = {k: v for k, v in results.items() if not isinstance(v, dict)}
        return _rec(gid, name, "unavailable", "both the future-generation and the old-generation probes must be measured; not measured: " + "; ".join(f"{k}: {v}" for k, v in missing.items()), ev)
    return _rec(gid, name, "passed", "", ev)
