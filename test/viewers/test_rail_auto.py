#!/usr/bin/env python3
"""Decision-5 gates (rail_auto.py): scheduling, every gate's pass, and the counterexample that must not pass."""

from __future__ import annotations

import copy
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rail_auto as A  # noqa: E402
import rail_shots_driver as D  # noqa: E402
import rail_shots_post as P  # noqa: E402

SHOTS = ["auto-a-frame", "auto-b-frame", "auto-c-board", "auto-f-after", "auto-c-schematic", "auto-c-back", "auto-d-left", "auto-d-back", "auto-e-refused", "auto-e-edit"]
KEY = "editor.kicad.abc"


def cap(runtime: dict | None, pid: int = 4242, extra: dict | None = None) -> dict:
    sess = {"state": "ready", "active": "PCB", "pid": pid, "frames": []}
    if runtime is not None:
        sess["runtime"] = runtime
    sess.update(extra or {})
    return {"ok": True, "inspect_ok": True, "obstructed": False, "hub_kicad_sessions": {"ok": True, "inventory_complete": True, "sessions": {KEY: sess}}}


def pane(shown, keys, holder="7", pid=4242):
    return {"ok": True, "in_gpui_tree": True, "shown": shown, "keys": keys, "holder": holder, "pid": pid, "state": "ready"}


GONE = {"ok": False, "error": "not editing"}


def log(seq, **obj):
    return {"seq": seq + 0.5, "mark": "", "obj": obj}


def good():
    marks = {m: i * 10 for i, m in enumerate(
        ["auto:start", "auto:a:pre", "auto:a:attach", "auto:a:mid", "auto:a:painted", "auto:a:done", "auto:b", "auto:b:done", "auto:c:board", "auto:f", "auto:f:done",
         "auto:c:schematic", "auto:c:back", "auto:c:done", "auto:d:leave", "auto:d:back", "auto:d:done", "auto:e:close", "auto:e:refused", "auto:e:refused-done", "auto:e:edit", "auto:e:done"], start=1)}
    comp = [{"seq": 11, "text": "x"}, {"seq": 22, "text": "xqpre"}, {"seq": 33, "text": "xqpreqmid"}, {"seq": 45, "text": "xqpreqmidqpost"}]
    logs = [log(marks["auto:a:attach"] + 1, kicad_info="d", state="starting"), log(marks["auto:b"] + 1, kicad_auto_to_deliberate="attach"),
            log(marks["auto:e:close"] + 1, kicad_closed_by_person="p"), log(marks["auto:e:refused"] + 1, kicad_attach="refused", why="closed by the person")]
    tags = {
        "auto-a-report": pane(11, False), "auto-b-report": pane(11, True),
        "auto-c-board-report": pane(11, True), "auto-c-sch-report": pane(12, True), "auto-c-back-report": pane(11, True),
        "auto-d-left-report": GONE, "auto-d-back-report": pane(11, False),
        "auto-e-refused-report": GONE, "auto-e-edit-report": pane(21, True, pid=5555),
    }
    live = {"session": "live", "holder": "7", "generation": 3}
    parked = {"session": "parked", "generation": 3}  # the hub omits `holder` when no window holds the pane
    caps = {
        "auto-a-frame": cap(live), "auto-b-frame": cap(live), "auto-c-board": cap(live), "auto-f-after": cap(live), "auto-c-schematic": cap(live), "auto-c-back": cap(live),
        "auto-d-left": cap(parked), "auto-d-back": cap(live), "auto-e-refused": cap({"session": "closedByPerson", "generation": 3}), "auto-e-edit": cap(live, pid=5555),
    }
    parsed = {"mark_seqs": marks, "kicad_tags": tags, "gpui_logs": logs, "composer_reads": comp, "presses": {}, "rail_events": [(marks["auto:a:painted"] + 3, {"lit": ["Board"]})]}
    actions = [
        {"action": "type-pre", "ok": True}, {"action": "type-mid", "ok": True}, {"action": "type-post", "ok": True},
        {"action": "stale", "ok": True, "generation": 3, "sent_generation": 10, "stale": True, "current_stale": False}, {"action": "close", "ok": True},
    ]
    return parsed, caps, actions


def run(parsed, caps, actions, cleared=None):
    res = A.evaluate_auto_gates(Path("."), parsed, caps, actions, cleared_shots=cleared)
    return {r["id"].split("auto-")[1]: r for r in res}


class AutoGateTests(unittest.TestCase):
    def test_scheduling_embed_only_full_only_and_valid_shot_names(self) -> None:
        steps = D.build_script("full", 300, 15000, embed=True)
        shots = [s["shot"] for s in steps if "shot" in s]
        for name in ("auto-act-type-pre", "auto-act-type-mid", "auto-act-type-post", "auto-act-stale", "auto-act-close", "auto-a-frame", "auto-e-edit"):
            self.assertIn(name, shots)
        self.assertTrue(all(D.SHOT_NAME_RE.match(n) for n in shots))
        self.assertEqual(len(shots), len(set(shots)))
        self.assertEqual(steps[0], {"select": 0})
        self.assertEqual(steps[1]["shot"], "idle")
        self.assertEqual(steps[2], {"mark": "auto:start"})  # while KiCad is still cold: before any page opens
        self.assertFalse([s for s in D.build_script("full", 300, 15000, embed=False) if str(s.get("mark", "")).startswith("auto:")])
        self.assertFalse([s for s in D.build_script("schematic-only", 300, 15000, embed=True) if str(s.get("mark", "")).startswith("auto:")])
        # only the existing attach step is used; markers are lower-case letters (no shifted key)
        self.assertTrue(all(v.islower() and v.isalpha() for v in A.MARKERS.values()))
        attach = [s["kicad"]["why"] for s in steps if isinstance(s.get("kicad"), dict) and "attach" in s["kicad"]]
        self.assertEqual(set(attach), {"auto", "deliberate", "edit"})

    def test_no_auto_marks_no_check_records(self) -> None:
        self.assertEqual(A.evaluate_auto_gates(Path("."), {"mark_seqs": {}}, {}, []), [])

    def test_every_gate_passes_on_good_evidence_except_the_two_unobservables(self) -> None:
        r = run(*good())
        self.assertEqual({k: v["outcome"] for k, v in r.items()},
                         {"a-frame-no-keyboard": "passed", "b-deliberate-promotes": "passed", "c-frame-switch": "unavailable", "d-keep-warm": "passed",
                          "e-person-closed-hold": "passed", "f-stale-generation": "passed", "g-always-viewer": "unavailable"})
        self.assertIn("geometry push", r["c-frame-switch"]["note"])
        self.assertIn("reconcile", r["g-always-viewer"]["note"])

    def test_a_counterexamples(self) -> None:
        def a(mut):
            parsed, caps, actions = good()
            mut(parsed, caps, actions)
            return run(parsed, caps, actions)["a-frame-no-keyboard"]
        # a key went to KiCad: the composer misses the middle marker
        r = a(lambda p, c, x: p["composer_reads"].__setitem__(2, {"seq": 33, "text": "xqpre"}) or p["composer_reads"].__setitem__(3, {"seq": 45, "text": "xqprequost"}))
        self.assertEqual(r["outcome"], "failed")
        self.assertIn("during", r["note"])
        # KiCad has the keyboard
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-report", pane(11, True)))["outcome"], "failed")
        # the client itself says the keyboard was taken
        self.assertEqual(a(lambda p, c, x: p["gpui_logs"].append(log(p["mark_seqs"]["auto:a:mid"] + 1, kicad_auto_keys_taken="x")))["outcome"], "failed")
        # no holder / someone else's holder
        self.assertEqual(a(lambda p, c, x: c.__setitem__("auto-a-frame", cap({"session": "live", "generation": 3})))["outcome"], "failed")
        self.assertEqual(a(lambda p, c, x: c.__setitem__("auto-a-frame", cap({"session": "live", "holder": "99", "generation": 3})))["outcome"], "failed")
        # the rail does not light Board
        self.assertEqual(a(lambda p, c, x: p.__setitem__("rail_events", [(p["mark_seqs"]["auto:a:painted"] + 3, {"lit": ["Schematic"]})]))["outcome"], "failed")
        # frame not mounted
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-report", GONE))["outcome"], "failed")
        # unavailable, never passed: typing not delivered, no cold start observed, no hub runtime, `{}` report, no rail snapshot
        self.assertEqual(a(lambda p, c, x: x.__setitem__(1, {"action": "type-mid", "ok": False, "error": "xinput"}))["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: p["gpui_logs"].pop(0))["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: c.__setitem__("auto-a-frame", cap(None)))["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: p["kicad_tags"].__setitem__("auto-a-report", {}))["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: p.__setitem__("rail_events", []))["outcome"], "unavailable")
        self.assertEqual(a(lambda p, c, x: p["composer_reads"].pop())["outcome"], "unavailable")
        # a stale rail snapshot from before the frame is not evidence
        self.assertEqual(a(lambda p, c, x: p.__setitem__("rail_events", [(1, {"lit": ["Board"]})]))["outcome"], "unavailable")

    def test_b_c_d_e_f_counterexamples(self) -> None:
        def g(gid, mut):
            parsed, caps, actions = good()
            mut(parsed, caps, actions)
            return run(parsed, caps, actions)[gid]
        # b: the deliberate attach did not take the keyboard / no promotion
        self.assertEqual(g("b-deliberate-promotes", lambda p, c, x: p["kicad_tags"].__setitem__("auto-b-report", pane(11, False)))["outcome"], "failed")
        self.assertEqual(g("b-deliberate-promotes", lambda p, c, x: p["gpui_logs"].remove(p["gpui_logs"][1]))["outcome"], "failed")
        self.assertEqual(g("b-deliberate-promotes", lambda p, c, x: p["kicad_tags"].__setitem__("auto-b-report", {}))["outcome"], "unavailable")
        # c: a second KiCad process, or two frames that are the same frame
        self.assertEqual(g("c-frame-switch", lambda p, c, x: c.__setitem__("auto-c-schematic", cap({"session": "live", "generation": 3}, pid=9999)))["outcome"], "failed")
        self.assertEqual(g("c-frame-switch", lambda p, c, x: p["kicad_tags"].__setitem__("auto-c-sch-report", pane(11, True)))["outcome"], "failed")
        # d: still held after leaving / a cold start on return / not kept warm
        self.assertEqual(g("d-keep-warm", lambda p, c, x: c.__setitem__("auto-d-left", cap({"session": "live", "holder": "7", "generation": 3})))["outcome"], "failed")
        self.assertEqual(g("d-keep-warm", lambda p, c, x: c.__setitem__("auto-d-left", cap({"session": "failed", "generation": 3})))["outcome"], "failed")
        self.assertEqual(g("d-keep-warm", lambda p, c, x: p["kicad_tags"].__setitem__("auto-d-back-report", pane(11, False, pid=1)))["outcome"], "failed")
        self.assertEqual(g("d-keep-warm", lambda p, c, x: p["gpui_logs"].append(log(p["mark_seqs"]["auto:d:back"] + 1, kicad_info="d", state="starting")))["outcome"], "failed")
        self.assertEqual(g("d-keep-warm", lambda p, c, x: p["kicad_tags"].__setitem__("auto-d-left-report", pane(11, False)))["outcome"], "failed")
        # a missing runtime block is not "holder absent"
        self.assertEqual(g("d-keep-warm", lambda p, c, x: c.__setitem__("auto-d-left", cap(None)))["outcome"], "unavailable")
        # e: not refused / attach sent / Edit did not start a new KiCad / close not delivered
        self.assertEqual(g("e-person-closed-hold", lambda p, c, x: p["gpui_logs"].pop())["outcome"], "failed")
        self.assertEqual(g("e-person-closed-hold", lambda p, c, x: p["gpui_logs"].append(log(p["mark_seqs"]["auto:e:refused"] + 2, kicad_rpc_sent="editor.kicad.attach")))["outcome"], "failed")
        self.assertEqual(g("e-person-closed-hold", lambda p, c, x: p["kicad_tags"].__setitem__("auto-e-edit-report", pane(21, True, pid=4242)))["outcome"], "failed")
        self.assertEqual(g("e-person-closed-hold", lambda p, c, x: p["kicad_tags"].__setitem__("auto-e-refused-report", pane(11, False)))["outcome"], "failed")
        self.assertEqual(g("e-person-closed-hold", lambda p, c, x: x.__setitem__(4, {"action": "close", "ok": False, "error": "no socket"}))["outcome"], "unavailable")
        # f: not stale / control stale / state changed / not delivered / no control reply
        self.assertEqual(g("f-stale-generation", lambda p, c, x: x[3].update(stale=False))["outcome"], "failed")
        self.assertEqual(g("f-stale-generation", lambda p, c, x: x[3].update(current_stale=True))["outcome"], "failed")
        self.assertEqual(g("f-stale-generation", lambda p, c, x: c.__setitem__("auto-f-after", cap({"session": "live", "holder": "7", "generation": 4})))["outcome"], "failed")
        self.assertEqual(g("f-stale-generation", lambda p, c, x: x.__setitem__(3, {"action": "stale", "ok": False, "error": "x"}))["outcome"], "unavailable")
        self.assertEqual(g("f-stale-generation", lambda p, c, x: x[3].update(current_stale=None))["outcome"], "unavailable")

    def test_privacy_clearance_and_obstruction_are_consulted(self) -> None:
        parsed, caps, actions = good()
        self.assertEqual(run(parsed, caps, actions, cleared=set(SHOTS) - {"auto-a-frame"})["a-frame-no-keyboard"]["outcome"], "unavailable")
        caps2 = copy.deepcopy(caps)
        caps2["auto-b-frame"]["obstructed"] = True
        self.assertEqual(run(parsed, caps2, actions)["b-deliberate-promotes"]["outcome"], "unavailable")

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


if __name__ == "__main__":
    unittest.main()
