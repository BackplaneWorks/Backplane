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
  d  leaving the page detaches with keep-warm; coming back re-attaches without a cold start
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


def build_auto_steps(settle_timeout_ms: int = 15000) -> list[dict]:
    """The scripted half of the gates: marks, the existing attach step, reads, reports and shots (the `auto-act-*` shots make the
    watcher act BEFORE the script goes on)."""
    tmo = min(settle_timeout_ms, 8000)
    painted = 90000
    return [
        {"mark": "auto:start"},
        {"keys": "ctrl-l"},
        {"wait": 400},
        {"read": "composer"},
        # a: Automatic attach, typing before / during the cold start / after the frame appears
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
        # c (+ f): back to the viewer, then Automatic Board -> Schematic -> Board on the one running KiCad
        {"click": "ui:View"},
        {"wait": 2500},
        {"mark": "auto:c:board"},
        {"kicad": {"attach": "board", "why": "auto", "tag": "auto-c-board-attach"}},
        {"kicad": {"wait_painted": painted, "kind": "PCB", "tag": "auto-c-board-painted"}},
        {"kicad": {"report": True, "tag": "auto-c-board-report"}},
        {"shot": "auto-c-board", "settle": "", "timeout_ms": tmo},
        {"mark": "auto:f"},
        {"shot": "auto-act-stale", "settle": "", "timeout_ms": tmo},
        {"wait": 1500},
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
        # d: leave the page (keep-warm), come back
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
        {"kicad": {"wait_painted": painted, "kind": "PCB", "tag": "auto-e-edit-painted"}},
        {"kicad": {"report": True, "tag": "auto-e-edit-report"}},
        {"mark": "auto:e:done"},
        {"shot": "auto-e-edit", "settle": "", "timeout_ms": tmo},
        # back to a clean viewer for the rest of the run
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
    rt = (session or {}).get("runtime")
    return rt if isinstance(rt, dict) and "session" in rt else None


def _report(kicad_tags: dict, tag: str) -> dict | None:
    r = kicad_tags.get(tag)
    return r if isinstance(r, dict) else None


def _attached(report: dict | None) -> bool | None:
    """True: the pane reports a mounted, shown frame; False: the pane says it is not editing; None: not a real report."""
    if not isinstance(report, dict):
        return None
    if report.get("ok") is False and report.get("error") == "not editing":
        return False
    if report.get("ok") is True and isinstance(report.get("in_gpui_tree"), bool) and "shown" in report:
        return bool(report["in_gpui_tree"]) and bool(report.get("shown"))
    return None


def _logs_between(logs: list[dict], first_seq: int | None, last_seq: int | None, key: str) -> list[dict]:
    if first_seq is None:
        return []
    hi = last_seq if last_seq is not None else 10**12
    return [lg for lg in logs if first_seq <= lg["seq"] <= hi and key in lg["obj"]]


def _composer_reads_in(reads: list[dict], lo: int | None, hi: int | None) -> list[dict]:
    if lo is None:
        return []
    return [r for r in reads if lo <= r["seq"] and (hi is None or r["seq"] <= hi)]


def _shot_ok(caps: dict, name: str, cleared: set[str] | None) -> tuple[bool, str]:
    cap = caps.get(name) or {}
    if cleared is not None and name not in cleared:
        return False, f"shot {name} not in the run's per-shot privacy clearance"
    if not (cap.get("ok") and cap.get("inspect_ok") is True):
        return False, f"capture {name} missing or not inspected"
    if cap.get("obstructed"):
        return False, f"capture {name} obstructed by a native dialog"
    return True, ""


def _action(actions: list[dict], action: str) -> list[dict]:
    return [a for a in actions if a.get("action") == action]


# ---------------------------------------------------------------- the gates

def evaluate_auto_gates(
    outdir: Path,
    parsed_full: dict,
    caps_full: dict[str, dict],
    actions: list[dict],
    cleared_shots: set[str] | None = None,
) -> list[dict]:
    """Every gate as a check record; `unavailable` whenever the run did not observe what the gate claims."""
    marks = parsed_full.get("mark_seqs") or {}
    if not any(m.startswith("auto:") for m in marks):
        return []  # the auto steps were not scheduled in this run (not --embed): no check records at all
    kt = parsed_full.get("kicad_tags") or {}
    logs: list[dict] = parsed_full.get("gpui_logs") or []
    comp: list[dict] = parsed_full.get("composer_reads") or []
    presses = parsed_full.get("presses") or {}
    out: list[dict] = []

    # ---- a
    name = "Automatic attach shows KiCad in the pane and never takes the keyboard (marker typed before, during, after)"
    out.append(_gate_a(name, marks, kt, logs, comp, caps_full, actions, parsed_full, cleared_shots))
    # ---- b
    out.append(_gate_b(marks, kt, logs, caps_full, cleared_shots))
    # ---- c
    out.append(_gate_c(marks, kt, logs, caps_full, cleared_shots))
    # ---- d
    out.append(_gate_d(marks, kt, logs, caps_full, cleared_shots))
    # ---- e
    out.append(_gate_e(marks, kt, logs, caps_full, cleared_shots, actions))
    # ---- f
    out.append(_gate_f(actions, caps_full, cleared_shots))
    # ---- g
    out.append(_rec("g-always-viewer", "With the setting \"Always viewer\" no Automatic attach happens", "unavailable",
                    "needs the rail reconcile: the client issues no Automatic attach from `viewer.mode` yet (the hub's AUTO_PLATFORMS is empty) "
                    "and the setting is not in the client; nothing to observe until then"))
    return out


def _gate_a(name, marks, kt, logs, comp, caps, actions, parsed, cleared) -> dict:
    gid = "a-frame-no-keyboard"
    typed = {t: _action(actions, f"type-{t}") for t in MARKERS}
    for t, acts in typed.items():
        if not acts or acts[-1].get("ok") is not True:
            return _rec(gid, name, "unavailable", f"real X typing of the `{t}` marker was not delivered ({acts[-1].get('error') if acts else 'no action recorded'})")
    shot_ok, why = _shot_ok(caps, "auto-a-frame", cleared)
    if not shot_ok:
        return _rec(gid, name, "unavailable", why)
    sess, why = _only_session(caps.get("auto-a-frame"))
    if sess is None:
        return _rec(gid, name, "unavailable", why)
    rt = _runtime(sess)
    rep = _report(kt, "auto-a-report")
    att = _attached(rep)
    if rt is None or att is None:
        return _rec(gid, name, "unavailable", "no hub runtime block or no real pane report to observe the frame and the holder")
    # composer: start text, then each marker appended in order
    reads = _composer_reads_in(comp, marks.get("auto:start"), marks.get("auto:a:done"))
    if len(reads) < 4:
        return _rec(gid, name, "unavailable", f"expected 4 composer reads (start, pre, mid, post), got {len(reads)}")
    start, r_pre, r_mid, r_post = (r["text"] for r in reads[:4])
    exp_pre, exp_mid, exp_post = start + MARKERS["pre"], start + MARKERS["pre"] + MARKERS["mid"], start + MARKERS["pre"] + MARKERS["mid"] + MARKERS["post"]
    fails: list[str] = []
    for label, got, exp in (("before", r_pre, exp_pre), ("during", r_mid, exp_mid), ("after", r_post, exp_post)):
        if got != exp:
            fails.append(f"composer after typing {label} the attach is {got!r}, expected {exp!r} (keys went elsewhere)")
    # frame in the pane, keyboard not taken
    if att is not True:
        fails.append("KiCad's frame is not mounted and shown in the pane")
    if rep.get("keys") is not False:
        fails.append(f"the pane reports keys={rep.get('keys')!r}: KiCad has the keyboard")
    holder = rt.get("holder")
    if not holder:
        fails.append("hub runtime.holder is absent: the pane is not held by a client")
    elif str(rep.get("holder", "")) != str(holder):
        fails.append(f"hub runtime.holder {holder!r} is not the pane's own holder {rep.get('holder')!r}")
    # rail lights Board after the scripted rail read
    lit = _rail_lit_between(parsed, marks.get("auto:a:painted"), marks.get("auto:a:done"))
    if lit is None:
        return _rec(gid, name, "unavailable", "no fresh rail snapshot after the frame appeared")
    if "Board" not in lit:
        fails.append(f"rail does not light Board (lit {lit})")
    taken = [lg for key in ("kicad_auto_keys_taken", "kicad_auto_to_deliberate") for lg in _logs_between(logs, marks.get("auto:a:attach"), marks.get("auto:a:done"), key)]
    flips = [lg for lg in _logs_between(logs, marks.get("auto:a:attach"), marks.get("auto:a:done"), "kicad_keys_flip") if (lg["obj"]["kicad_keys_flip"] or {}).get("to") is True]
    if taken or flips:
        fails.append(f"the client says KiCad took the keyboard ({[list(x['obj'])[0] for x in taken + flips]})")
    # the cold start was really typed through
    starting = [lg for lg in _logs_between(logs, marks.get("auto:a:attach"), marks.get("auto:a:painted"), "kicad_info") if lg["obj"].get("state") == "starting"]
    ev = {"composer": {"start": start, "pre": r_pre, "mid": r_mid, "post": r_post}, "report": {k: rep.get(k) for k in ("ok", "in_gpui_tree", "shown", "keys", "holder", "pid", "state")},
          "hub_runtime": rt, "rail_lit": lit, "cold_start_observed": bool(starting)}
    if fails:
        return _rec(gid, name, "failed", "; ".join(fails), ev)
    if not starting:
        return _rec(gid, name, "unavailable", "the keyboard stayed with the composer, but no KiCad `starting` state was observed between the attach and the frame: the 'during the cold start' typing is not proven", ev)
    return _rec(gid, name, "passed", "", ev)


def _rail_lit_between(parsed: dict, lo: int | None, hi: int | None) -> list[str] | None:
    """The last rail snapshot drawn after `lo` and not after `hi` (None: no snapshot in that window)."""
    if lo is None or hi is None:
        return None
    last = None
    for s, rail in (parsed.get("rail_events") or []):
        if lo < s <= hi and isinstance(rail, dict) and isinstance(rail.get("lit"), list):
            last = list(rail["lit"])
    return last


def _gate_b(marks, kt, logs, caps, cleared) -> dict:
    gid, name = "b-deliberate-promotes", "A Deliberate attach after an Automatic one promotes the pane and takes the keyboard"
    rep = _report(kt, "auto-b-report")
    att = _attached(rep)
    if att is None:
        return _rec(gid, name, "unavailable", "no real pane report after the deliberate attach")
    ok, why = _shot_ok(caps, "auto-b-frame", cleared)
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


def _pid(report: dict | None) -> int | None:
    p = (report or {}).get("pid")
    return p if isinstance(p, int) and not isinstance(p, bool) and p > 0 else None


def _gate_c(marks, kt, logs, caps, cleared) -> dict:
    gid, name = "c-frame-switch", "Board <-> Schematic under Automatic is a frame switch on one KiCad process"
    reps = [_report(kt, t) for t in ("auto-c-board-report", "auto-c-sch-report", "auto-c-back-report")]
    if any(_attached(r) is None for r in reps):
        return _rec(gid, name, "unavailable", "a pane report is missing or not real")
    for shot in ("auto-c-board", "auto-c-schematic", "auto-c-back"):
        ok, why = _shot_ok(caps, shot, cleared)
        if not ok:
            return _rec(gid, name, "unavailable", why)
    pids = [_pid(r) for r in reps]
    sess_pids = []
    for shot in ("auto-c-board", "auto-c-schematic", "auto-c-back"):
        s, why = _only_session(caps.get(shot))
        if s is None:
            return _rec(gid, name, "unavailable", why)
        sess_pids.append(s.get("pid"))
    ev = {"pane_pids": pids, "hub_pids": sess_pids, "frames": [r.get("shown") for r in reps],
          "geometry_pushes": "unavailable: the client has no per-switch geometry-push observable on Linux (`kicad_host_push` is macOS-only)"}
    fails = []
    if any(p is None for p in pids) or len(set(pids)) != 1:
        fails.append(f"pane pids differ or are missing: {pids}")
    if any(p is None for p in sess_pids) or len(set(sess_pids)) != 1:
        fails.append(f"hub session pid changed or is missing: {sess_pids}")
    if any(_attached(r) is not True for r in reps):
        fails.append("a frame was not mounted and shown in the pane after an attach")
    if not (reps[0].get("shown") and reps[1].get("shown") and reps[0].get("shown") != reps[1].get("shown") and reps[0].get("shown") == reps[2].get("shown")):
        fails.append(f"Board and Schematic are not two different frames of the one KiCad (shown {ev['frames']})")
    # a switch sends no second attach for the process; count what the hub was asked
    sent = _logs_between(logs, marks.get("auto:c:schematic"), marks.get("auto:c:done"), "kicad_rpc_sent")
    ev["rpcs_sent_during_switches"] = [s["obj"]["kicad_rpc_sent"] for s in sent]
    if fails:
        return _rec(gid, name, "failed", "; ".join(fails), ev)
    return _rec(gid, name, "unavailable", "one KiCad process and two frames observed; the 'one geometry push per switch' half has no client observable (listed in the report)", ev)


def _gate_d(marks, kt, logs, caps, cleared) -> dict:
    gid, name = "d-keep-warm", "Leaving the page detaches with keep-warm; coming back re-attaches without a cold start"
    left, back, before = _report(kt, "auto-d-left-report"), _report(kt, "auto-d-back-report"), _report(kt, "auto-c-back-report")
    if _attached(left) is None or _attached(back) is None or _attached(before) is None:
        return _rec(gid, name, "unavailable", "a pane report is missing or not real")
    for shot in ("auto-d-left", "auto-d-back"):
        ok, why = _shot_ok(caps, shot, cleared)
        if not ok:
            return _rec(gid, name, "unavailable", why)
    s_left, why = _only_session(caps.get("auto-d-left"))
    if s_left is None:
        return _rec(gid, name, "unavailable", why)
    rt = _runtime(s_left)
    if rt is None:
        return _rec(gid, name, "unavailable", "the hub reported no runtime block while the page was left")
    s_back, why = _only_session(caps.get("auto-d-back"))
    if s_back is None:
        return _rec(gid, name, "unavailable", why)
    starting = [lg for lg in _logs_between(logs, marks.get("auto:d:back"), marks.get("auto:d:done"), "kicad_info") if lg["obj"].get("state") == "starting"]
    ev = {"left_attached": _attached(left), "runtime_when_left": rt, "pid_before": _pid(before), "pid_after": _pid(back), "hub_pid_after": s_back.get("pid"), "starting_infos_on_return": len(starting)}
    fails = []
    if _attached(left) is not False:
        fails.append("the pane is still mounted after leaving the page")
    if rt.get("holder"):
        fails.append(f"hub runtime.holder is still {rt.get('holder')!r} after leaving")
    if rt.get("session") not in ("live", "parked"):
        fails.append(f"KiCad session is {rt.get('session')!r} after leaving, not live/parked (not kept warm)")
    if _attached(back) is not True:
        fails.append("coming back did not mount and show the frame")
    if _pid(before) is None or _pid(before) != _pid(back) or s_back.get("pid") != _pid(back):
        fails.append("a different KiCad process after coming back (a cold start)")
    if starting:
        fails.append("KiCad went through `starting` on the way back (a cold start)")
    return _rec(gid, name, "failed" if fails else "passed", "; ".join(fails), ev)


def _gate_e(marks, kt, logs, caps, cleared, actions) -> dict:
    gid, name = "e-person-closed-hold", "After KiCad's own Close an Automatic attach is refused (viewer shows); Edit clears the hold"
    close = _action(actions, "close")
    if not close or close[-1].get("ok") is not True:
        return _rec(gid, name, "unavailable", f"KiCad's own Close was not delivered ({close[-1].get('error') if close else 'no action recorded'})")
    for shot in ("auto-e-refused", "auto-e-edit"):
        ok, why = _shot_ok(caps, shot, cleared)
        if not ok:
            return _rec(gid, name, "unavailable", why)
    refused = _report(kt, "auto-e-refused-report")
    edit = _report(kt, "auto-e-edit-report")
    before = _report(kt, "auto-d-back-report")
    if _attached(refused) is None or _attached(edit) is None:
        return _rec(gid, name, "unavailable", "a pane report is missing or not real")
    closed = _logs_between(logs, marks.get("auto:e:close"), marks.get("auto:e:refused"), "kicad_closed_by_person")
    refusals = [lg for lg in _logs_between(logs, marks.get("auto:e:refused"), marks.get("auto:e:refused-done"), "kicad_attach") if lg["obj"].get("kicad_attach") == "refused"]
    sent = [lg for lg in _logs_between(logs, marks.get("auto:e:refused"), marks.get("auto:e:refused-done"), "kicad_rpc_sent") if lg["obj"].get("kicad_rpc_sent") == "editor.kicad.attach"]
    s_ref, _why = _sessions(caps.get("auto-e-refused"))
    sessions = list((s_ref or {}).values())
    ev = {"closed_by_person_logged": bool(closed), "refusals": [r["obj"] for r in refusals], "attach_rpcs_after_refusal": len(sent),
          "pid_before": _pid(before), "pid_after_edit": _pid(edit), "hub_sessions_after_close": [(s.get("runtime") or {}).get("session") for s in sessions]}
    fails = []
    if not closed:
        fails.append("the pane never reported `kicad_closed_by_person` after KiCad's own Close")
    if not any(r["obj"].get("why") == "closed by the person" for r in refusals):
        fails.append("the Automatic attach after the Close was not refused as 'closed by the person'")
    if sent:
        fails.append("an `editor.kicad.attach` was sent after the refusal")
    if _attached(refused) is not False:
        fails.append("the pane is mounted after a refused Automatic attach (the viewer page should show)")
    if _attached(edit) is not True:
        fails.append("Edit did not open KiCad again")
    if _pid(edit) is None or _pid(edit) == _pid(before):
        fails.append("Edit did not start a new KiCad (same or missing pid)")
    return _rec(gid, name, "failed" if fails else "passed", "; ".join(fails), ev)


def _gate_f(actions, caps, cleared) -> dict:
    gid, name = "f-stale-generation", "An attach carrying an old generation gets stale:true and changes nothing"
    acts = _action(actions, "stale")
    if not acts or acts[-1].get("ok") is not True:
        return _rec(gid, name, "unavailable", f"the stale attach was not sent ({acts[-1].get('error') if acts else 'no action recorded'})")
    a = acts[-1]
    for shot in ("auto-c-board", "auto-f-after"):
        ok, why = _shot_ok(caps, shot, cleared)
        if not ok:
            return _rec(gid, name, "unavailable", why)
    s0, why = _only_session(caps.get("auto-c-board"))
    s1, why1 = _only_session(caps.get("auto-f-after"))
    if s0 is None or s1 is None:
        return _rec(gid, name, "unavailable", why or why1)
    r0, r1 = _runtime(s0), _runtime(s1)
    if r0 is None or r1 is None or not isinstance(a.get("generation"), int):
        return _rec(gid, name, "unavailable", "no hub runtime generation to compare")
    ev = {"generation": a.get("generation"), "sent_generation": a.get("sent_generation"), "stale_reply": a.get("stale"), "current_reply_stale": a.get("current_stale"),
          "runtime_before": r0, "runtime_after": r1, "pid_before": s0.get("pid"), "pid_after": s1.get("pid")}
    fails = []
    if a.get("stale") is not True:
        fails.append(f"the old-generation attach was not answered stale (reply stale={a.get('stale')!r})")
    if a.get("current_stale") is True:
        fails.append("the control (the running generation) was answered stale too")
    if a.get("current_stale") is None:
        return _rec(gid, name, "unavailable", "the control attach (the running generation) got no reply", ev)
    if r0.get("generation") != r1.get("generation") or s0.get("pid") != s1.get("pid") or r0.get("session") != r1.get("session"):
        fails.append("the stale attach changed the runtime (generation, session or pid moved)")
    return _rec(gid, name, "failed" if fails else "passed", "; ".join(fails), ev)
