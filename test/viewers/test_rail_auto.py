#!/usr/bin/env python3
"""Decision-5 gates (rail_auto.py): scheduling, privacy, every gate's pass, and the counterexample that must not pass."""

from __future__ import annotations

import copy
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rail_auto as A  # noqa: E402
import rail_shots_driver as D  # noqa: E402
import rail_shots_manifest as M  # noqa: E402
import rail_shots_post as P  # noqa: E402

KEY = "editor.kicad.abc"


def frames(kind, shown=True):
    return [{"kind": kind, "shown": shown}]


def cap(runtime, pid=4242, shown_kind="PCB"):
    sess = {"state": "ready", "active": shown_kind, "pid": pid, "frames": frames(shown_kind)}
    if runtime is not None:
        sess["runtime"] = runtime
    return {"ok": True, "inspect_ok": True, "obstructed": False, "hub_kicad_sessions": {"ok": True, "inventory_complete": True, "sessions": {KEY: sess}}}


def pane(shown, keys, pid=4242, kind="PCB", painted=500, state="ready", open_="board"):
    return {"ok": True, "in_gpui_tree": True, "shown": shown, "keys": keys, "pid": pid, "state": state, "open": open_,
            "frames": [{"xid": shown, "kind": kind, "painted": painted}] if shown else []}


GONE = {"ok": False, "error": "not editing"}


def log(seq, **obj):
    return {"seq": seq + 0.5, "mark": "", "obj": obj}


MARKS = ["auto:start", "auto:a:pre", "auto:a:attach", "auto:a:mid", "auto:a:painted", "auto:a:done", "auto:b", "auto:b:done", "auto:c:board", "auto:f", "auto:f:done",
         "auto:c:schematic", "auto:c:back", "auto:c:done", "auto:d:view-leave", "auto:d:view-back", "auto:d:view-done", "auto:d:leave", "auto:d:back", "auto:d:done",
         "auto:e:close", "auto:e:refused", "auto:e:refused-done", "auto:e:edit", "auto:e:done", "auto:f:old"]

STATE = {"pid": 4242, "generation": 3, "session": "live", "holder": "7", "shown_frames": ["PCB"]}


def good():
    marks = {m: i * 10 for i, m in enumerate(MARKS, start=1)}
    comp = [{"seq": marks["auto:start"] + 1, "text": "x"}, {"seq": marks["auto:a:pre"] + 2, "text": "xqpre"},
            {"seq": marks["auto:a:mid"] + 2, "text": "xqpreqmid"}, {"seq": marks["auto:a:painted"] + 2, "text": "xqpreqmidqpost"}]
    logs = [log(marks["auto:b"] + 1, kicad_auto_to_deliberate="attach"),
            log(marks["auto:e:close"] + 1, kicad_closed_by_person="p"), log(marks["auto:e:refused"] + 1, kicad_attach="refused", why="closed by the person")]
    tags = {
        "auto-a-report": pane(11, False), "auto-a-painted": pane(11, False), "auto-b-report": pane(11, True),
        "auto-c-board-report": pane(11, True), "auto-c-board-painted": pane(11, True),
        "auto-c-sch-report": pane(12, True, kind="SCH"), "auto-c-sch-painted": pane(12, True, kind="SCH"),
        "auto-c-back-report": pane(11, True), "auto-c-back-painted": pane(11, True),
        "auto-d-view-left-report": GONE, "auto-d-view-back-report": pane(11, False), "auto-d-view-back-painted": pane(11, False),
        "auto-d-left-report": {**GONE, "open": "files"}, "auto-d-back-report": pane(11, False), "auto-d-back-painted": pane(11, False),
        "auto-e-refused-report": GONE, "auto-e-edit-report": pane(21, True, pid=5555), "auto-e-edit-painted": pane(21, True, pid=5555),
    }
    # the pane's own `open` is `files` in the d2 left report even when it is detached
    live = {"session": "live", "holder": "7", "generation": 3}
    parked = {"session": "parked", "generation": 3}  # the hub omits `holder` when no window holds the pane
    sch = lambda r: cap(r, shown_kind="SCH")  # noqa: E731
    caps = {
        "auto-a-frame": cap(live), "auto-b-frame": cap(live), "auto-c-board": cap(live), "auto-f-after": cap(live), "auto-c-schematic": sch(live), "auto-c-back": cap(live),
        "auto-d-view-left": cap(parked), "auto-d-view-back": cap(live), "auto-d-left": cap(parked), "auto-d-back": cap(live),
        "auto-e-refused": cap({"session": "closedByPerson", "generation": 3}), "auto-e-edit": cap(live, pid=5555),
    }
    press = lambda i: [{"id": i, "found": True, "seq": 1}]  # noqa: E731
    parsed = {"mark_seqs": marks, "kicad_tags": tags, "gpui_logs": logs, "composer_reads": comp,
              "presses": {"auto:d:view-leave": press("ui:View"), "auto:d:leave": press("rail:Files")},
              "rail_events": [(marks["auto:a:painted"] + 3, {"lit": ["Board"]})]}
    probe = {"ok": True, "before": dict(STATE), "after": dict(STATE), "stale": True, "control_ok": True, "control_stale": False, "generation": 3}
    actions = [
        {"action": "type-pre", "ok": True}, {"action": "type-mid", "ok": True, "kicad_state_before": "starting", "kicad_state_after": "starting"}, {"action": "type-post", "ok": True},
        {"action": "stale-future", "sent_generation": 10, **probe}, {"action": "stale-old", "sent_generation": 3, **probe}, {"action": "close", "ok": True},
    ]
    return parsed, caps, actions


def run(parsed, caps, actions):
    res = A.evaluate_auto_gates(Path("."), parsed, caps, actions)
    return {r["id"].split("auto-")[1]: r for r in res}


def mutate(gid, fn):
    parsed, caps, actions = good()
    fn(parsed, caps, actions)
    return run(parsed, caps, actions)[gid]


class AutoGateTests(unittest.TestCase):
    # ---------------------------------------------------------------- scheduling and privacy
    def test_scheduling_is_last_embed_only_full_only_and_valid_shot_names(self) -> None:
        steps = D.build_script("full", 300, 15000, embed=True)
        shots = [s["shot"] for s in steps if "shot" in s]
        for name in ("auto-act-kill", "auto-act-type-pre", "auto-act-type-mid", "auto-act-type-post", "auto-act-stale-future", "auto-act-stale-old", "auto-act-close", "auto-a-frame", "auto-e-edit"):
            self.assertIn(name, shots)
        self.assertTrue(all(D.SHOT_NAME_RE.match(n) for n in shots))
        self.assertEqual(len(shots), len(set(shots)))
        # AFTER every existing task and check: nothing but the auto steps follows the first auto step
        first = next(i for i, s in enumerate(steps) if s == {"mark": "auto:start"})
        self.assertGreater(first, 100)
        self.assertEqual(steps[first:][-1].get("tag"), "auto-end-idle")
        self.assertFalse([s for s in steps[:first] if str(s.get("mark", "")).startswith("auto:") or str(s.get("shot", "")).startswith("auto-")])
        self.assertTrue([s for s in steps[:first] if s.get("mark") == "press:task4-dock-split"], "the docking steps come before the auto gates")
        self.assertFalse([s for s in D.build_script("full", 300, 15000, embed=False) if str(s.get("mark", "")).startswith("auto:")])
        self.assertFalse([s for s in D.build_script("schematic-only", 300, 15000, embed=True) if str(s.get("mark", "")).startswith("auto:")])
        self.assertTrue(all(v.islower() and v.isalpha() for v in A.MARKERS.values()))
        attach = [s["kicad"]["why"] for s in steps if isinstance(s.get("kicad"), dict) and "attach" in s["kicad"]]
        self.assertTrue({"auto", "deliberate", "edit"} <= set(attach))
        # "pre" is typed before the attach step, "mid" after it
        auto = steps[first:]
        idx = {s.get("shot"): i for i, s in enumerate(auto) if "shot" in s}
        att = next(i for i, s in enumerate(auto) if s.get("kicad", {}).get("tag") == "auto-a-attach")
        self.assertLess(idx["auto-act-type-pre"], att)
        self.assertGreater(idx["auto-act-type-mid"], att)

    def test_action_only_shots_save_no_screenshot_and_every_auto_canvas_is_private(self) -> None:
        for n in ("auto-act-type-pre", "auto-act-stale-old", "auto-act-close", "auto-act-kill"):
            self.assertTrue(A.is_action_only_shot(n))
        self.assertFalse([n for n in A.AUTO_SHOTS if A.is_action_only_shot(n)])
        self.assertFalse([n for n in M.expected_shots("full", embed=True) if A.is_action_only_shot(n)])
        for n in A.AUTO_SHOTS:
            self.assertIn(n, M.expected_shots("full", embed=True))
            self.assertIn(n, P.EMBED_CANVAS_SHOTS)
            reasons = P.shot_privacy_reasons({"inspect_ok": True, "obstructed": False}, {}, shot_name=n, embed=True)
            self.assertTrue([r for r in reasons if "KiCad canvas" in r], (n, reasons))
        # the driver's watcher branch for them writes no PNG: it never reaches the `import` call
        src = (Path(D.__file__)).read_text(encoding="utf-8")
        branch = src[src.index("if AUTO.is_action_only_shot(name):"):src.index('out_png = kind_out / f"{name}.png"')]
        self.assertIn("continue", branch)
        self.assertNotIn("import", branch.replace("# an action-only shot saves NO screenshot", ""))

    def test_no_auto_marks_no_check_records(self) -> None:
        self.assertEqual(A.evaluate_auto_gates(Path("."), {"mark_seqs": {}}, {}, []), [])

    def test_private_shots_do_not_block_the_gates_which_read_metrics_not_pixels(self) -> None:
        parsed, caps, actions = good()
        r = A.evaluate_auto_gates(Path("."), parsed, caps, actions, cleared_shots=set())
        self.assertEqual({x["id"].split("auto-")[1]: x["outcome"] for x in r}["b-deliberate-promotes"], "passed")

    def test_every_gate_passes_on_good_evidence_except_the_unobservables(self) -> None:
        r = run(*good())
        self.assertEqual({k: v["outcome"] for k, v in r.items()},
                         {"a-frame-no-keyboard": "passed", "b-deliberate-promotes": "passed", "c-frame-switch": "unavailable", "d-keep-warm": "passed",
                          "d2-leave-page-detaches": "passed", "e-person-closed-hold": "passed", "f-stale-generation": "passed", "g-always-viewer": "unavailable"})
        self.assertIn("geometry push", r["c-frame-switch"]["note"])
        self.assertIn("reconcile", r["g-always-viewer"]["note"])

    # ---------------------------------------------------------------- a
    def test_a_counterexamples(self) -> None:
        a = lambda fn: mutate("a-frame-no-keyboard", fn)  # noqa: E731
        r = a(lambda p, c, x: p["composer_reads"].__setitem__(2, {"seq": p["composer_reads"][2]["seq"], "text": "xqpre"}) or p["composer_reads"].__setitem__(3, {"seq": p["composer_reads"][3]["seq"], "text": "xqprequost"}))
        self.assertEqual(r["outcome"], "failed")
        self.assertIn("during", r["note"])
        # KiCad has the keyboard at ANY pane report in the interval (not only the last)
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-report", pane(11, True)))["outcome"], "failed")
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-painted", pane(11, True)))["outcome"], "failed")
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"]["auto-a-painted"].pop("keys"))["outcome"], "failed")  # absent is not false
        # the client says the keyboard was taken, or KiCad's container saw keys (the positive observable)
        self.assertEqual(a(lambda p, c, x: p["gpui_logs"].append(log(p["mark_seqs"]["auto:a:mid"] + 1, kicad_auto_keys_taken="x")))["outcome"], "failed")
        self.assertEqual(a(lambda p, c, x: p["gpui_logs"].append(log(p["mark_seqs"]["auto:a:mid"] + 1, kicad_key="q")))["outcome"], "failed")
        self.assertIn("composer received all", good_note(a, lambda p, c, x: None))
        # no holder
        self.assertEqual(a(lambda p, c, x: c.__setitem__("auto-a-frame", cap({"session": "live", "generation": 3})))["outcome"], "failed")
        # painted PCB frame required (attach that never painted)
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-painted", pane(11, False, painted=3)))["outcome"], "failed")
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-painted", pane(11, False, state="failed")))["outcome"], "failed")
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-painted", pane(11, False, kind="SCH")))["outcome"], "failed")
        # rail, frame
        self.assertEqual(a(lambda p, c, x: p.__setitem__("rail_events", [(p["mark_seqs"]["auto:a:painted"] + 3, {"lit": ["Schematic"]})]))["outcome"], "failed")
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-report", GONE))["outcome"], "failed")
        # the start is too fast to type inside it (or not observed): the mid part is unavailable, not passed
        for state in ("ready", None, "stopped"):
            r = a(lambda p, c, x, s=state: x.__setitem__(1, {"action": "type-mid", "ok": True, "kicad_state_before": s}))
            self.assertEqual(r["outcome"], "unavailable", state)
            self.assertIn("not proven", r["note"])
        # `pre` typed AFTER the attach request is not "before"
        self.assertEqual(a(lambda p, c, x: p["composer_reads"].__setitem__(1, {"seq": p["mark_seqs"]["auto:a:attach"] + 1, "text": "xqpre"}))["outcome"], "unavailable")
        # unavailable, never passed: typing not delivered, no hub runtime (or an incomplete one), `{}` report, no rail snapshot, missing read
        self.assertEqual(a(lambda p, c, x: x.__setitem__(0, {"action": "type-pre", "ok": False, "error": "xinput"}))["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: c.__setitem__("auto-a-frame", cap(None)))["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: c.__setitem__("auto-a-frame", cap({"session": "live", "holder": "7"})))["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-report", {}))["outcome"], "failed")  # `{}` is a dict with no frame: not mounted
        self.assertEqual(a(lambda p, c, x: p.__setitem__("rail_events", []))["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: p["composer_reads"].pop())["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: p.__setitem__("rail_events", [(1, {"lit": ["Board"]})]))["outcome"], "unavailable")

    # ---------------------------------------------------------------- b, c
    def test_b_and_c_counterexamples(self) -> None:
        b = lambda fn: mutate("b-deliberate-promotes", fn)  # noqa: E731
        self.assertEqual(b(lambda p, c, x: p["kicad_tags"].__setitem__("auto-b-report", pane(11, False)))["outcome"], "failed")
        self.assertEqual(b(lambda p, c, x: p["gpui_logs"].remove(p["gpui_logs"][0]))["outcome"], "failed")
        self.assertEqual(b(lambda p, c, x: p["kicad_tags"].__setitem__("auto-b-report", {}))["outcome"], "unavailable")
        c = lambda fn: mutate("c-frame-switch", fn)  # noqa: E731
        # a second KiCad process; the hub's pid is not the pane's pid; two frames that are the same frame; a step that never painted
        self.assertEqual(c(lambda p, k, x: k.__setitem__("auto-c-schematic", cap({"session": "live", "generation": 3}, pid=9999, shown_kind="SCH")))["outcome"], "failed")
        self.assertEqual(c(lambda p, k, x: k.__setitem__("auto-c-back", cap({"session": "live", "generation": 3}, pid=1111)))["outcome"], "failed")
        self.assertEqual(c(lambda p, k, x: p["kicad_tags"].__setitem__("auto-c-sch-report", pane(11, True, kind="SCH")))["outcome"], "failed")
        for tag, kind in (("auto-c-board-painted", "PCB"), ("auto-c-sch-painted", "SCH"), ("auto-c-back-painted", "PCB")):
            r = c(lambda p, k, x, t=tag, kd=kind: p["kicad_tags"].__setitem__(t, pane(30, True, kind=kd, painted=2)))
            self.assertEqual(r["outcome"], "failed", tag)
            self.assertIn("no painted", r["note"])
        # the partial claim says exactly what is and is not proven
        note = run(*good())["c-frame-switch"]["note"]
        self.assertIn("pid equals the pane's pid", note)
        self.assertIn("Unproven", note)

    # ---------------------------------------------------------------- d, d2
    def test_d_counterexamples(self) -> None:
        d = lambda fn: mutate("d-keep-warm", fn)  # noqa: E731
        self.assertEqual(d(lambda p, c, x: c.__setitem__("auto-d-view-left", cap({"session": "live", "holder": "7", "generation": 3})))["outcome"], "failed")
        self.assertEqual(d(lambda p, c, x: c.__setitem__("auto-d-view-left", cap({"session": "failed", "generation": 3})))["outcome"], "failed")
        self.assertEqual(d(lambda p, c, x: p["kicad_tags"].__setitem__("auto-d-view-back-report", pane(11, False, pid=1)))["outcome"], "failed")
        self.assertEqual(d(lambda p, c, x: p["gpui_logs"].append(log(p["mark_seqs"]["auto:d:view-back"] + 1, kicad_info="d", state="starting")))["outcome"], "failed")
        self.assertEqual(d(lambda p, c, x: p["kicad_tags"].__setitem__("auto-d-view-back-painted", pane(11, False, painted=1)))["outcome"], "failed")
        # still in the tree: mounted (shown) or in the tree with shown=0: neither is "detached"
        self.assertEqual(d(lambda p, c, x: p["kicad_tags"].__setitem__("auto-d-view-left-report", pane(11, False)))["outcome"], "failed")
        r = d(lambda p, c, x: p["kicad_tags"].__setitem__("auto-d-view-left-report", pane(0, False)))
        self.assertEqual(r["outcome"], "failed")
        self.assertIn("not detached", r["note"])
        # absence of `holder` counts only inside a COMPLETE runtime block (session AND generation)
        self.assertEqual(d(lambda p, c, x: c.__setitem__("auto-d-view-left", cap({"session": "parked"})))["outcome"], "unavailable")
        self.assertEqual(d(lambda p, c, x: c.__setitem__("auto-d-view-left", cap(None)))["outcome"], "unavailable")
        # `ui:View` must have been delivered
        self.assertEqual(d(lambda p, c, x: p["presses"].__setitem__("auto:d:view-leave", [{"id": "ui:View", "found": False, "seq": 1}]))["outcome"], "unavailable")
        self.assertEqual(d(lambda p, c, x: p["presses"].pop("auto:d:view-leave"))["outcome"], "unavailable")

    def test_d2_is_unavailable_and_cannot_pass_without_files_observed_open(self) -> None:
        d2 = lambda fn: mutate("d2-leave-page-detaches", fn)  # noqa: E731
        # a pane still in the tree over Files (shown, or shown=0): observed, the reconcile's job: unavailable
        for shown in (11, 0):
            r = d2(lambda p, c, x, s=shown: p["kicad_tags"].__setitem__("auto-d-left-report", {**pane(s, False), "open": "files"}))
            self.assertEqual(r["outcome"], "unavailable")
            self.assertIn("reconcile", r["note"])
        # a detached-looking pane with Files NOT observed open can never pass (the same predicates would otherwise pass)
        for opened in (None, "board", ""):
            r = d2(lambda p, c, x, o=opened: p["kicad_tags"].__setitem__("auto-d-left-report", {**GONE, "open": o}))
            self.assertEqual(r["outcome"], "unavailable", opened)
            self.assertIn("Files was not observed open", r["note"])
        self.assertEqual(d2(lambda p, c, x: p["kicad_tags"].__setitem__("auto-d-left-report", GONE))["outcome"], "unavailable")
        # the rail press must have been delivered
        self.assertEqual(d2(lambda p, c, x: p["presses"].__setitem__("auto:d:leave", [{"id": "rail:Files", "found": False}]))["outcome"], "unavailable")
        # with Files observed open and a really detached pane the predicates may fail (held / not warm) ...
        self.assertEqual(d2(lambda p, c, x: c.__setitem__("auto-d-left", cap({"session": "live", "holder": "7", "generation": 3})))["outcome"], "failed")
        # ... and an incomplete runtime block never passes by the missing `holder` key
        self.assertEqual(d2(lambda p, c, x: c.__setitem__("auto-d-left", cap({"session": "parked"})))["outcome"], "unavailable")

    # ---------------------------------------------------------------- e
    def test_e_counterexamples(self) -> None:
        e = lambda fn: mutate("e-person-closed-hold", fn)  # noqa: E731
        self.assertEqual(e(lambda p, c, x: p["gpui_logs"].pop())["outcome"], "failed")
        self.assertEqual(e(lambda p, c, x: p["gpui_logs"].append(log(p["mark_seqs"]["auto:e:refused"] + 2, kicad_rpc_sent="editor.kicad.attach")))["outcome"], "failed")
        self.assertEqual(e(lambda p, c, x: p["kicad_tags"].__setitem__("auto-e-edit-report", pane(21, True, pid=4242)))["outcome"], "failed")
        self.assertEqual(e(lambda p, c, x: p["kicad_tags"].__setitem__("auto-e-refused-report", pane(11, False)))["outcome"], "failed")
        self.assertEqual(e(lambda p, c, x: p["kicad_tags"].__setitem__("auto-e-edit-painted", pane(21, True, pid=5555, painted=1)))["outcome"], "failed")
        self.assertEqual(e(lambda p, c, x: x.__setitem__(5, {"action": "close", "ok": False, "error": "no socket"}))["outcome"], "unavailable")
        # both interval boundaries and both pids are required
        for m in ("auto:e:close", "auto:e:refused", "auto:e:refused-done", "auto:e:edit", "auto:e:done"):
            self.assertEqual(e(lambda p, c, x, mm=m: p["mark_seqs"].pop(mm))["outcome"], "unavailable", m)
        self.assertEqual(e(lambda p, c, x: p["kicad_tags"]["auto-d-back-report"].pop("pid"))["outcome"], "unavailable")
        self.assertEqual(e(lambda p, c, x: p["kicad_tags"]["auto-e-edit-report"].pop("pid"))["outcome"], "unavailable")

    # ---------------------------------------------------------------- f
    def test_f_counterexamples(self) -> None:
        f = lambda fn: mutate("f-stale-generation", fn)  # noqa: E731
        fut, old = 3, 4
        self.assertEqual(f(lambda p, c, x: x[fut].update(stale=False))["outcome"], "failed")
        self.assertEqual(f(lambda p, c, x: x[old].update(stale=False))["outcome"], "failed")
        self.assertEqual(f(lambda p, c, x: x[fut].update(control_stale=True))["outcome"], "failed")
        # any part of the state moving fails; any part missing is unavailable, never "equal"
        for key, val in (("pid", 9), ("generation", 4), ("session", "parked"), ("holder", None), ("shown_frames", ["SCH"])):
            self.assertEqual(f(lambda p, c, x, k=key, v=val: x[fut]["after"].__setitem__(k, v))["outcome"], "failed", key)
        self.assertEqual(f(lambda p, c, x: x[fut].update(after=None))["outcome"], "unavailable")
        self.assertEqual(f(lambda p, c, x: x[fut].update(before=None, after=None))["outcome"], "unavailable")
        # a control that errored without `stale` is not "the running generation is fine"
        self.assertEqual(f(lambda p, c, x: x[fut].update(control_ok=False, control_error="boom", control_stale=False))["outcome"], "unavailable")
        # only the future probe measured (no restart, so no old generation): unavailable, naming it
        r = f(lambda p, c, x: x.__setitem__(old, {"action": "stale-old", "ok": False, "error": "no previous generation exists"}))
        self.assertEqual(r["outcome"], "unavailable")
        self.assertIn("old: not run", r["note"])
        self.assertIn("generation+7", r["evidence"]["note"])
        # nothing run at all
        self.assertEqual(f(lambda p, c, x: x[:] and [x.pop(3), x.pop(3)])["outcome"], "unavailable")

    def test_holder_is_this_client_turns_on_with_the_cid_lines_and_never_compares_x_window(self) -> None:
        def a_with(clients, logs_extra, holder="7", report_cid=None):
            parsed, caps, actions = good()
            parsed["hub_clients"] = clients
            parsed["gpui_logs"] += logs_extra
            parsed["kicad_tags"]["auto-a-report"] = {**pane(11, False), **({"cid": report_cid} if report_cid else {})}
            caps["auto-a-frame"] = cap({"session": "live", "holder": holder, "generation": 3})
            return run(parsed, caps, actions)["a-frame-no-keyboard"]
        conn = [log(1, hub_connected={"cid": "gpui-1-2", "generation": 1})]
        # no lines: inferred (the pane's own X-window `holder` is never compared)
        r = a_with({}, [])
        self.assertEqual(r["outcome"], "passed")
        self.assertIn("inferred", r["evidence"]["holder_is_this_client"])
        # cid -> N -> runtime.holder == N: observed
        r = a_with({"gpui-1-2": 7}, conn)
        self.assertEqual(r["outcome"], "passed")
        self.assertIn("observed", r["evidence"]["holder_is_this_client"])
        # another window holds the pane: failed
        self.assertEqual(a_with({"gpui-1-2": 8}, conn)["outcome"], "failed")
        # the report's cid must agree; a reconnect (later hub_connected) is the one that counts
        self.assertEqual(a_with({"gpui-1-2": 7}, conn, report_cid="gpui-9-9")["outcome"], "failed")
        re = conn + [log(2, hub_connected={"cid": "gpui-1-3", "generation": 2})]
        self.assertEqual(a_with({"gpui-1-2": 7, "gpui-1-3": 9}, re)["outcome"], "failed")
        self.assertEqual(a_with({"gpui-1-2": 7, "gpui-1-3": 7}, re)["outcome"], "passed")

    def test_hub_log_client_lines_are_parsed(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            lg = Path(td) / "hub.log"
            lg.write_text("noise\nkicad embed: connection 5 is client gpui-10-20\nkicad embed: connection 8 is client gpui-10-20\nkicad embed: connection 9 is client other\n", encoding="utf-8")
            self.assertEqual(P.parse_hub_clients(lg), {"gpui-10-20": 8, "other": 9})
            self.assertEqual(P.parse_hub_clients(Path(td) / "none.log"), {})

    def test_c_second_half_reads_kicad_place_and_turns_on_by_itself(self) -> None:
        def c_with(sch, back):
            parsed, caps, actions = good()
            m = parsed["mark_seqs"]
            for evs, mk in ((sch, "auto:c:schematic"), (back, "auto:c:back")):
                for e in evs:
                    parsed["gpui_logs"].append(log(m[mk] + 1, kicad_place=[1, 2, 3, 4], visible=True, **e))
            return run(parsed, caps, actions)["c-frame-switch"]
        # no kicad_place lines at all: still unavailable, naming it
        r = c_with([], [])
        self.assertEqual(r["outcome"], "unavailable")
        self.assertIn("no `kicad_place` log lines", r["note"])
        sw = {"changed": False, "frame_changed": True, "why": "layout", "frame": "SCH"}
        self.assertEqual(c_with([sw], [{**sw, "frame": "PCB"}])["outcome"], "passed")
        # one switch observed and the other not: still unavailable (not earned)
        self.assertEqual(c_with([sw], [])["outcome"], "unavailable")
        # a new geometry pushed on a switch, or no frame_changed placement: failed
        self.assertEqual(c_with([{**sw, "changed": True}], [sw])["outcome"], "failed")
        self.assertEqual(c_with([{**sw, "frame_changed": False}], [sw])["outcome"], "failed")

    def test_state_of_requires_every_part(self) -> None:
        good_sess = {"pid": 5, "runtime": {"session": "live", "generation": 2}, "frames": [{"kind": "PCB", "shown": True}]}
        self.assertEqual(A.state_of(good_sess), {"pid": 5, "generation": 2, "session": "live", "holder": None, "shown_frames": ["PCB"]})
        for broken in ({**good_sess, "pid": 0}, {**good_sess, "frames": None}, {**good_sess, "runtime": {"session": "live"}}, {**good_sess, "runtime": None}, {k: v for k, v in good_sess.items() if k != "pid"}):
            self.assertIsNone(A.state_of(broken))

    def test_parser_collects_composer_reads_and_kicad_logs_in_order(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            lg = Path(td) / "c.log"
            lg.write_text("\n".join([
                'GPUI_METRIC {"event":"mark","name":"auto:a:pre"}',
                'GPUI_METRIC {"event":"read","what":"composer","text":"abc","supported":true}',
                'GPUI_LOG {"kicad_attach":"board","why":"Automatic","generation":null}',
                'GPUI_LOG {"unrelated":1}',
                'GPUI_METRIC {"event":"mark","name":"auto:a:done"}',
            ]) + "\n", encoding="utf-8")
            p = P.parse_client_log(lg)
            self.assertEqual([r["text"] for r in p["composer_reads"]], ["abc"])
            self.assertEqual(len(p["gpui_logs"]), 1)
            self.assertTrue(p["mark_seqs"]["auto:a:pre"] < p["gpui_logs"][0]["seq"] < p["mark_seqs"]["auto:a:done"])


def good_note(a, fn):
    return a(fn)["evidence"]["kicad_side"]


if __name__ == "__main__":
    unittest.main()
