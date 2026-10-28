import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/board.bend as B
import ../src/core/hist.bend as H
import ../src/core/look.bend as LO
import ../src/core/fold.bend as F
import ../src/core/client.bend as U
import ../src/core/json.bend as J

def item(+k: String, +sig: U32, +layer: U32) -> B.Item:
  B.Item{k, sig, layer, B.PSeg{B.Pt{0.0, 0.0}, B.Pt{1.0, 1.0}, 0.2}, k}

def Show.items(xs: List<&2, B.Item>) -> String:
  match xs:
    case Nil{}:
      ""
    case B.Item{+k, _, +l, _, _} <> t:
      k ++ "@" ++ U32.show(l) ++ " " ++ Show.items(t)

def Show.strs(xs: List<&2, String>) -> String:
  String.join(xs, ",")

def z(+s: String) -> String:
  s ++ SCon{Chr{0}, SNil{}}

# what the capture script prints for a first step and a later one
def out1() -> String:
  "c1\n\n/r/.git\n/r\n" ++ z("A") ++ z("b.kicad_pcb") ++ z("A") ++ z("sub/s.kicad_sch")

def out2() -> String:
  "c2\nc1\n/r/.git\n/r\n" ++ z("M") ++ z("b.kicad_pcb")

def out3() -> String:
  "c3\nc2\n/r/.git\n/r\n" ++ z("M") ++ z("sub/s.kicad_sch") ++ z("D") ++ z("old.kicad_pcb")

def step(+id: String, +rec: String, +at: Nat) -> M.Entry:
  M.Act{id, "t", "step", "kicad", rec, at}

# a turn: the message, a tool call, its step, words, another call, its
# step, the reply, then a step the turn's end took
def es() -> List<&2, M.Entry>:
  [M.Msg{"u", "t", M.User{}, "route it", 1n},
   step("step:c1", H.Hist.record(out1(), "u"), 2n),
   M.Msg{"m1", "t", M.Assistant{}, "I'll place the parts first.\nThen route.", 3n},
   M.Act{"a1", "t", "tool", "Bash", "python place.py", 4n},
   step("step:c2", H.Hist.record(out2(), "a1"), 5n),
   M.Act{"a2", "t", "tool", "Edit", "sub/s.kicad_sch", 6n},
   step("step:c3", H.Hist.record(out3(), "a2"), 7n),
   M.Msg{"m2", "t", M.Assistant{}, "Done.", 8n},
   step("step:c4", H.Hist.record("c4\nc3\n/r/.git\n/r\n" ++ z("M") ++ z("b.kicad_pcb"), ""), 9n)]

def Rep.changes(xs: List<&2, M.Entry>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      Nil{}
    case M.Msg{+id, +th, +r, +tx, +at} <> t:
      M.MessagePosted{th, id, r, tx, at} <> Rep.changes(t)
    case M.Act{+id, +th, +tone, +k, +sm, +at} <> t:
      M.ActivityLogged{th, id, tone, k, sm, at} <> Rep.changes(t)

def state() -> M.State:
  M.State.replay(Rep.changes(es()), M.State.empty())

def Show.ids(ss: List<&2, H.Step>) -> String:
  match ss:
    case Nil{}:
      ""
    case +s <> t:
      H.Step.id(s) ++ " " ++ Show.ids(t)

def yes(b: Bool) -> String:
  match b:
    case True{}:
      "yes"
    case False{}:
      "no"

def Show.ids.e(xs: List<&2, M.Entry>) -> String:
  match xs:
    case Nil{}:
      ""
    case +e <> +t:
      Bool.pick(String, List.is_empty(&2, M.Entry, t), M.Idx.id(e), M.Idx.id(e) ++ "," ++ Show.ids.e(t))

def newest() -> List<&2, M.Entry>:
  List.reverse(&2, M.Entry, es())

def nth(+i: Nat) -> H.Step:
  Maybe.default(&2, H.Step, H.Hist.nth(H.Hist.steps(state(), "t"), i), H.Step{"", "", "", "", "", "", Nil{}, 0n})

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def Show.cmds(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Later{+ms, +a, +v} <> t:
      "later " ++ Nat.show(ms) ++ " " ++ a ++ " " ++ v ++ ";" ++ Show.cmds(t)
    case U.Jump{+id} <> t:
      "jump " ++ id ++ ";" ++ Show.cmds(t)
    case U.Send{+x} <> t:
      "send;" ++ Show.cmds(t)
    case _ <> t:
      "other;" ++ Show.cmds(t)

def cmds(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      Show.cmds(cs)

def act(u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

# thread t selected, its viewer open on the board /r/b.kicad_pcb
def base() -> U.Ui:
  U.Ui.view_put(act(U.Ui.adopt(U.Ui.init(0n), state()), "select", "t"), "board", "/r/b.kicad_pcb")

# the same, on a sheet of its schematic (its file and its place)
def sheet() -> U.Ui:
  U.Ui.scratch_set(U.Ui.view_put(base(), "schematic", "/r/sub/s.kicad_sch"), "@sch:t", "1\t/sub/")

# a page loaded again with a thread's viewer as it was kept, the thread
# selected (app.bend's presentation_select)
def reload(+text: String) -> U.Ui:
  U.Ui.reselect("", U.Ui.select.nav(U.Viewer.restore(U.Ui.adopt(U.Ui.init(0n), state()), "t", text), "t"))

def path(+u: U.Ui) -> String:
  U.Ui.view_get(u, "view.path")

def Bar.show(b: U.HBar) -> String:
  match b:
    case U.HBar{+on, +n, +at, +title, +entry, +label, +said, +pl, +df, +cm, +a, +bb, +menu}:
      Bool.show(on) ++ "|" ++ Nat.show(n) ++ "|" ++ Nat.show(at) ++ "|" ++ title ++ "|" ++ entry ++ "|" ++ label ++ "|" ++ said ++ "|" ++ Bool.show(pl) ++ Bool.show(df) ++ Bool.show(cm) ++ "|" ++ a ++ "|" ++ bb ++ "|" ++ menu

def bar(+u: U.Ui) -> String:
  Bar.show(U.Hs.bar(u))

def main() -> IO(Unit):
  +u = base()
  +back = act(u, "hist-prev", "")
  +first = act(u, "hist-first", "")
  +second = act(first, "hist-next", "")
  +played = U.Ui.act(u, "hist-play", "")
  +diffed = act(second, "hist-diff", "")
  +live = act(second, "hist-live", "")
  +cmp = act(act(u, "cmp", ""), "cmp-pick", "a\tc1\tStep 1\t/r/.git\t/r")
  do IO<Unit>:
    C.Check.str("bar live", bar(u), "True|3|0|Live · 3 steps||||FalseFalseFalse||Working copy|")
    C.Check.str("a viewer kept comes back after a reload", path(reload(U.Viewer.keep(sheet()))) ++ "|"
      ++ U.Ui.view_get(reload(U.Viewer.keep(sheet())), "view.kind") ++ "|"
      ++ yes(U.Ui.panel(reload(U.Viewer.keep(sheet())))) ++ "|"
      ++ U.Ui.scratch(U.Viewer.restore(U.Ui.adopt(U.Ui.init(0n), state()), "t", U.Viewer.keep(sheet())), "@sch:t"), "/r/sub/s.kicad_sch|schematic|yes|1	/sub/")
    C.Check.str("a step on show comes back as the live file", path(reload(U.Viewer.keep(first))), "")
    C.Check.str("a shut viewer stays shut", yes(U.Ui.panel(reload(U.Viewer.keep(act(sheet(), "panel", ""))))), "no")
    C.Check.str("a record that does not parse changes nothing", U.Viewer.keep(reload("{oops")), "{\"open\":\"\",\"kind\":\"\",\"path\":\"\",\"sheet\":\"\",\"split\":\"\"}")
    C.Check.str("back from live: the last step", path(back), "git\t/r/.git\tc4\t2\t/r/b.kicad_pcb")
    C.Check.str("bar at the last", bar(back), "True|3|3|Step 3 of 3|m2|Done.|Done.|FalseFalseFalse|||")
    C.Check.str("first", path(first), "git\t/r/.git\tc1\t2\t/r/b.kicad_pcb")
    C.Check.str("first says it is before the edits", bar(first), "True|3|1|Step 1 of 3 · before the agent's edits|u|route it||FalseFalseFalse|||")
    C.Check.str("next", bar(second), "True|3|2|Step 2 of 3|a1|Bash  python place.py|I'll place the parts first.|FalseFalseFalse|||")
    C.Check.str("next past the last is live", path(act(back, "hist-next", "")), "/r/b.kicad_pcb")
    C.Check.str("live comes back", path(live) ++ "|" ++ U.Ui.view_get(live, "hist.at"), "/r/b.kicad_pcb|")
    C.Check.str("diff", path(diffed), "diff\ngit\t/r/.git\tc1\t2\t/r/b.kicad_pcb\ngit\t/r/.git\tc2\t2\t/r/b.kicad_pcb")
    C.Check.str("diff stays on", path(act(diffed, "hist-next", "")), "diff\ngit\t/r/.git\tc3\t2\t/r/b.kicad_pcb\ngit\t/r/.git\tc4\t2\t/r/b.kicad_pcb")
    C.Check.str("play starts at the first", path(ui(played)) ++ "|" ++ cmds(played), "git\t/r/.git\tc1\t2\t/r/b.kicad_pcb|later 1400 hist-tick t\t1;")
    C.Check.str("tick steps on", path(act(ui(played), "hist-tick", "t\t1")), "git\t/r/.git\tc2\t2\t/r/b.kicad_pcb")
    C.Check.str("tick schedules the next", cmds(U.Ui.act(ui(played), "hist-tick", "t\t1")), "later 1400 hist-tick t\t1;")
    C.Check.str("a stale tick does nothing", path(act(ui(played), "hist-tick", "t\t0")), "git\t/r/.git\tc1\t2\t/r/b.kicad_pcb")
    C.Check.str("paused ticks do nothing", path(act(act(ui(played), "hist-play", ""), "hist-tick", "t\t1")), "git\t/r/.git\tc1\t2\t/r/b.kicad_pcb")
    C.Check.str("playing ends at the last", cmds(U.Ui.act(act(act(ui(played), "hist-tick", "t\t1"), "hist-tick", "t\t1"), "hist-tick", "t\t1")) ++ Bool.show(U.Hs.playing(act(act(act(ui(played), "hist-tick", "t\t1"), "hist-tick", "t\t1"), "hist-tick", "t\t1"))), "False")
    C.Check.str("jump", cmds(U.Ui.act(second, "hist-jump", "")) ++ U.Ui.info(ui(U.Ui.act(second, "hist-jump", "")), "fold:a1"), "jump a1;1")
    C.Check.str("compare", path(cmp), "diff\ngit\t/r/.git\tc1\t2\t/r/b.kicad_pcb\n/r/b.kicad_pcb")
    C.Check.str("compare bar", bar(cmp), "True|3|0|Comparing||||FalseFalseTrue|Step 1|Working copy|")
    C.Check.str("compare, the new side first", path(act(u, "cmp-pick", "b\tc1\tStep 1\t/r/.git\t/r")) ++ "|" ++ bar(act(u, "cmp-pick", "b\tc1\tStep 1\t/r/.git\t/r")),
      "diff\n/r/b.kicad_pcb\ngit\t/r/.git\tc1\t2\t/r/b.kicad_pcb|True|3|0|Comparing||||FalseFalseTrue|Working copy|Step 1|")
    C.Check.str("another file starts a comparison over", U.Ui.view_get(act(U.Ui.view_put(cmp, "board", "/r/other.kicad_pcb"), "cmp-pick", "a\tc2\tStep 2\t/r/.git\t/r"), "cmp.bl"), "Working copy")
    C.Check.str("a tick for a thread not on show stops it", U.Ui.info(act(act(ui(played), "select", "other"), "hist-tick", "t\t1"), "hist.play:t"), "")
    C.Check.str("compare closes", path(act(cmp, "cmp-close", "")), "/r/b.kicad_pcb")
    C.Check.str("compare asks the hub", cmds(U.Ui.act(u, "cmp", "")), "send;")
    C.Check.str("show a step from the thread", path(act(U.Ui.view_put(u, "schematic", ""), "hist-show", "step:c3")), "git\t/r/.git\tc3\t2\t/r/sub/s.kicad_sch")
    C.Check.str("a schematic's track", bar(act(U.Ui.view_put(u, "schematic", ""), "hist-show", "step:c3")), "True|2|2|Step 2 of 2|a2|Edit  sub/s.kicad_sch|I'll place the parts first.|FalseFalseFalse|||")
    C.Check.str("not a board: no bar", bar(U.Ui.view_put(u, "3d", "")), "False|0|0|||||FalseFalseFalse|||")
    C.Check.str("record empty", H.Hist.record("", "x"), "")
    C.Check.str("record: a path with a newline", H.Hist.record("c\n\n/g\n/t\n" ++ z("A") ++ z("a\nb.kicad_pcb"), ""), "{\"e\":\"\",\"c\":\"c\",\"p\":\"\",\"g\":\"/g\",\"t\":\"/t\",\"f\":[\"A\\ta\\nb.kicad_pcb\"]}")
    C.Check.str("record", H.Hist.record(out2(), "a1"), "{\"e\":\"a1\",\"c\":\"c2\",\"p\":\"c1\",\"g\":\"/r/.git\",\"t\":\"/r\",\"f\":[\"M\\tb.kicad_pcb\"]}")
    C.Check.str("record: when it was taken", H.Hist.record.at(out2(), "a1", "start"), "{\"e\":\"a1\",\"c\":\"c2\",\"p\":\"c1\",\"g\":\"/r/.git\",\"t\":\"/r\",\"f\":[\"M\\tb.kicad_pcb\"],\"w\":\"start\"}")
    C.Check.str("the design before the edits: a first step at a turn's start", yes(H.Step.base(step("s", H.Hist.record.at(out1(), "u", "start"), 2n))), "yes")
    C.Check.str("a first step after a tool call is a change", yes(H.Step.base(step("s", H.Hist.record.at(out1(), "a1", ""), 2n))), "no")
    C.Check.str("a later step at a turn's start is a change", yes(H.Step.base(step("s", H.Hist.record.at(out2(), "u", "start"), 2n))), "no")
    C.Check.str("an older record's first step is the design before", yes(H.Step.base(step("s", H.Hist.record(out1(), "u"), 2n))), "yes")
    C.Check.str("an older record's later step is a change", yes(H.Step.base(step("s", H.Hist.record(out2(), "a1"), 2n))), "no")
    C.Check.str("the timeline leaves the design before out", Show.ids.e(H.Hist.seen(es())), "u,m1,a1,step:c2,a2,step:c3,m2,step:c4")
    C.Check.str("changes: the steps less the design before", Nat.show(H.Hist.changes(M.State.steps(state(), "t"))), "3")
    C.Check.str("steps", Show.ids(H.Hist.steps(state(), "t")), "step:c1 step:c2 step:c3 step:c4 ")
    C.Check.str("board track", Show.ids(H.Hist.track(H.Hist.steps(state(), "t"), 0n)), "step:c1 step:c2 step:c4 ")
    C.Check.str("schematic track (a deletion shows nothing)", Show.ids(H.Hist.track(H.Hist.steps(state(), "t"), 1n)), "step:c1 step:c3 ")
    C.Check.str("git spec", H.Hist.git("/r/.git", "c2", "/r", "sub/s.kicad_sch"), "git\t/r/.git\tc2\t2\t/r/sub/s.kicad_sch")
    C.Check.str("git obj", H.Hist.git.obj(H.Hist.git("/r/.git", "c2", "/r", "sub/s.kicad_sch")), "c2:sub/s.kicad_sch")
    C.Check.str("git obj folds ..", H.Hist.git.obj("git\t/r/.git\tc2\t2\t/r/sub/../x/./y.kicad_sch"), "c2:x/y.kicad_sch")
    C.Check.str("family of a version", H.Hist.family(H.Hist.git("/g", "c", "/top/dir", "a.kicad_pcb")), "/top/dir/a.kicad_pcb")
    C.Check.str("family of a comparison", H.Hist.family(H.Hist.diff(H.Hist.git("/g", "c", "/top", "a.kicad_pcb"), "/top/a.kicad_pcb")), "/top/a.kicad_pcb")
    C.Check.str("family of a live file", H.Hist.family("/top/a.kicad_pcb"), "/top/a.kicad_pcb")
    C.Check.str("key family", H.Hist.key.family("0|/top|" ++ H.Hist.git("/g", "c", "/top", "a.kicad_pcb")), "0|/top|/top/a.kicad_pcb")
    C.Check.str("diff sides", H.Hist.old(H.Hist.diff("A", "B")) ++ "," ++ H.Hist.new(H.Hist.diff("A", "B")), "A,B")
    C.Check.str("diff with no old", H.Hist.old(H.Hist.diff("", "B")) ++ "," ++ H.Hist.new(H.Hist.diff("", "B")), ",B")
    C.Check.str("kinds", Nat.show(H.Hist.kind("/a")) ++ Nat.show(H.Hist.kind(H.Hist.git("g", "c", "/t", "a"))) ++ Nat.show(H.Hist.kind(H.Hist.diff("a", "b"))), "012")
    C.Check.str("rebase", H.Hist.rebase(H.Hist.git("/g", "c", "/t", "sub/a.kicad_sch"), "/t/root.kicad_sch"), "git\t/g\tc\t2\t/t/root.kicad_sch")
    C.Check.str("rebase live", H.Hist.rebase("/t/x.kicad_sch", "/t/root.kicad_sch"), "/t/root.kicad_sch")
    C.Check.str("step spec", H.Step.spec(nth(2n), 0n, "", False{}), "git\t/r/.git\tc2\t2\t/r/b.kicad_pcb")
    C.Check.str("step diff spec", H.Step.spec(nth(2n), 0n, "", True{}), "diff\ngit\t/r/.git\tc1\t2\t/r/b.kicad_pcb\ngit\t/r/.git\tc2\t2\t/r/b.kicad_pcb")
    C.Check.str("first step diff has no old", H.Hist.old(H.Step.spec(nth(1n), 0n, "", True{})), "")
    C.Check.str("step file picks the one on show", H.Step.file(nth(1n), 1n, "/r/sub/s.kicad_sch"), "sub/s.kicad_sch")
    C.Check.str("index", Nat.show(H.Hist.index(H.Hist.steps(state(), "t"), "step:c3")) ++ Nat.show(H.Hist.index(H.Hist.steps(state(), "t"), "")) ++ Nat.show(H.Hist.index(H.Hist.steps(state(), "t"), "nope")), "300")
    C.Check.str("back from live is the last", Nat.show(H.Hist.back(4n, 0n)), "4")
    C.Check.str("back stops at the first", Nat.show(H.Hist.back(4n, 1n)), "1")
    C.Check.str("forward from the last is live", Nat.show(H.Hist.fwd(4n, 4n)), "0")
    C.Check.str("forward", Nat.show(H.Hist.fwd(4n, 2n)), "3")
    C.Check.str("anchor named", H.Hist.anchor(newest(), nth(2n)), "a1")
    C.Check.str("anchor at a turn's end", H.Hist.anchor(newest(), nth(4n)), "m2")
    C.Check.str("said before a tool call", H.Hist.said(newest(), "a1"), "I'll place the parts first.\nThen route.")
    C.Check.str("said: the reply itself", H.Hist.said(newest(), "m2"), "Done.")
    C.Check.str("said: nothing before the first words", H.Hist.said(newest(), "u"), "")
    C.Check.str("label", H.Hist.label(newest(), "a1"), "Bash  python place.py")
    C.Check.str("mark", Show.items(H.Hist.mark([item("a", 1, 3), item("b", 2, 3), item("c", 3, 5)], [item("b", 2, 3), item("c", 9, 5), item("d", 4, 1)])),
      "a@26 b@3 c~was@26 c@27 d@28 ")
    C.Check.str("mark same", Nat.show(H.Mark.count(H.Hist.mark([item("a", 1, 3), item("b", 2, 3)], [item("b", 2, 3), item("a", 1, 3)]))), "0")
    C.Check.str("mark from nothing", Show.items(H.Hist.mark(Nil{}, [item("a", 1, 3)])), "a@28 ")
    C.Check.str("mark to nothing", Show.items(H.Hist.mark([item("a", 1, 3)], Nil{})), "a@26 ")
    C.Check.str("tally", Nat.show(H.Mark.tally(H.Hist.mark([item("a", 1, 3)], [item("b", 1, 3)]), 28)), "1")
    C.Check.str("look: marks", U32.show(LO.Look.alpha.of(LO.Look.of(U32.or(0, LO.Look.diff()), 28))), "235")
    C.Check.str("look: the rest dimmed", U32.show(LO.Look.alpha.of(LO.Look.of(U32.or(0, LO.Look.diff()), 3))), U32.show((LO.Look.alpha.of(LO.Look.of(0, 3)) / 3 : U32)))
    C.Check.str("look: plain kinds unchanged", U32.show(LO.Look.alpha.of(LO.Look.of(0, 3))), "200")
    C.Check.str("names with marks", Show.strs(List.drop(&2, String, LO.Look.names(U32.or(1, LO.Look.diff()), ["x"]), 26n)), "Removed,Changed,Added")
    C.Check.str("fold label with steps", F.Fold.label(I.En{}, [M.Act{"a", "t", "tool", "Bash", "x", 0n}, step("s", "", 1n)]), "1 tool call · Bash · 1 design change")
    C.Check.str("fold label steps only", F.Fold.label(I.En{}, [step("s", "", 1n), step("s2", "", 1n)]), "2 design changes")
    C.Check.str("reveal: every key of its run", Show.strs(F.Fold.reveal(es(), "a1")), "fold:a1,fold:step:c2,fold:a2,fold:step:c3")
    C.Check.str("refs", Nat.show(List.length(&2, H.Ref, H.Hist.refs("head\tHEAD\tabc\nbranch\tmain\tdef\n\nbad line\n"))), "2")
