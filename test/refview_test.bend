import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/refview.bend as V
import ../src/core/ref.bend as R
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/far.bend as F

def state() -> M.State:
  M.State.replay([
    M.ProjectCreated{"p1-1", "Project", "/p", 1n},
    M.ThreadCreated{"t1-1", "p1-1", "First", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadCreated{"t1-2", "p1-1", "Second", M.Local{}, "", "", "claude", "", 3n},
    M.BotSet{"b1-1", "Ada", "t1-1", 0n, "", 4n},
    M.RoomSet{"r1-1", "Design", "", 5n},
    M.RoutineSet{"o1-1", "b1-1", "Review", "* * * * *", "go", True{}, 6n},
    M.SettingSet{"board.b1-1-1", "{\"title\":\"Build\",\"status\":\"doing\"}", 7n},
    M.WorkSet{"t1-1", "watch", "w-t1-1-9\twatching\t\tCompile\twatch\trun", 8n}
  ], M.State.empty())

def line(m: Maybe<&2, V.Card>) -> String:
  match m:
    case None{}: "missing"
    case Some{c}: V.Card.line(c)

def cmds(a: U.Act) -> String:
  match a:
    case U.Act{_, U.Copy{s} <> _}: s
    case _: ""

def menu.item(m: U.MItem) -> String:
  match m:
    case U.MItem{_, a, v, _}: a ++ " " ++ v

def menu.last(xs: List<&2, U.MItem>) -> String:
  menu.item(Maybe.default(&2, U.MItem, List.last(&2, U.MItem, xs), U.MItem{"", "", "", ""}))

def fresh(+st: M.State, +ch: M.Change, +r: R.Ref) -> Bool:
  +uri = R.Ref.uri(r)
  +cs = V.Cards.result.cards(V.Cards.get(V.Cards.empty(), st, uri))
  +new = M.State.apply(st, ch)
  String.eq(line(V.Cards.result.value(V.Cards.get(V.Cards.drop(cs, V.Cards.touched(ch)), new, uri))), line(V.Card.of(new, r)))

def note(u: U.Ui) -> String:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, text, _, _, _}: text

def ui.k(st: M.State, u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{_, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def ui(st: M.State) -> U.Ui:
  ui.k(st, U.Ui.init(10n))

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, U.Send{text} <> _}: text
    case _: ""

def jumped.cmds(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Jump{id} <> _: id
    case _ <> t: jumped.cmds(t)

def jumped(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}: jumped.cmds(cs)

def cache.ids(cs: V.Cards) -> Nat:
  match cs:
    case V.Cards{_, rev}: List.length(&2, String, Map.keys(&2, Map<&2, Nat>, rev))

def cache.entries(cs: V.Cards) -> Nat:
  match cs:
    case V.Cards{map, _}: List.length(&2, String, Map.keys(&2, V.CardEntry, map))

def main() -> IO(Unit):
  +st = state()
  +remote = F.Fr.mark(M.State.replay([M.ProjectCreated{"peer~p", "Far project", "/far", 1n}, M.ThreadCreated{"peer~t", "peer~p", "Far thread", M.Local{}, "", "", "claude", "", 2n}], st), "peer", "Laptop", False{})
  +u = U.Act.ui(U.Ui.act(ui(st), "select", "t1-1"))
  +go = U.Act.ui(U.Ui.act(u, "ref-go", R.Ref.uri(R.Ref.thread("t1-2"))))
  +back = U.Act.ui(U.Ui.act(go, "ref-back", ""))
  +fwd = U.Act.ui(U.Ui.act(back, "ref-fwd", ""))
  +cs = V.Cards.result.cards(V.Cards.get(V.Cards.empty(), st, R.Ref.uri(R.Ref.thread("t1-1"))))
  +two = V.Cards.result.cards(V.Cards.get(cs, st, "backplane://thread/t1-2"))
  +invalid = V.Cards.drop(two, ["p1-1"])
  +one = V.Cards.result.cards(V.Cards.get(invalid, st, "backplane://thread/t1-1"))
  do IO<Unit>:
    C.Check.str("remote thread", line(V.Card.of(remote, R.Ref.thread("peer~t"))), "Far thread · Far project · idle @Laptop")
    C.Check.str("machine", line(V.Card.of(remote, R.Ref.machine("peer"))), "Laptop")
    C.Check.str("missing machine", line(V.Card.of(st, R.Ref.machine("unknown"))), "missing")
    C.Check.str("thread", line(V.Card.of(st, R.Ref.thread("t1-1"))), "First · Project · idle")
    C.Check.str("fragment", line(V.Card.of(st, R.Ref.msg("t1-1", "absent"))), "First · message · Project · idle")
    C.Check.str("missing", line(V.Card.of(st, R.Ref.thread("absent"))), "missing")
    C.Check.str("bot", line(V.Card.of(st, R.Ref.bot("b1-1"))), "Ada · bot · idle")
    C.Check.str("room", line(V.Card.of(st, R.Ref.room("r1-1"))), "Design · room")
    C.Check.str("project", line(V.Card.of(st, R.Ref.project("p1-1"))), "Project · project")
    C.Check.str("routine", line(V.Card.of(st, R.Ref.routine("o1-1"))), "Review · Ada · on")
    C.Check.str("task", line(V.Card.of(st, R.Ref.task("b1-1-1"))), "Build · doing")
    C.Check.str("work", line(V.Card.of(st, R.Ref.work("w-t1-1-9"))), "Compile · watching")
    C.Check.str("PR", line(V.Card.of(st, R.Ref.pr("i2cjak/Backplane", "128"))), "PR #128 · i2cjak/Backplane")
    C.Check.str("project PR", line(V.Card.of(st, R.Ref.pr("p1-1", "128"))), "PR #128 · Project")
    C.Check.str("file", line(V.Card.of(st, R.Ref.file("p1-1", ["main.kicad_pcb"]))), "main.kicad_pcb · Project · board")
    C.Check.str("missing file project", line(V.Card.of(st, R.Ref.file("absent", ["x.pdf"]))), "missing")
    C.Check.str("person", line(V.Card.of(st, R.Ref.person())), "You")
    C.Check.str("mailroom", line(V.Card.of(st, R.Ref.mailroom("b1-1"))), "Ada mailroom")
    C.Check.str("charter", line(V.Card.of(st, R.Ref.charter("b1-1"))), "Ada charter")
    C.Check.str("conv placeholder", line(V.Card.of(st, R.Ref.conv("c"))), "conv · c")
    C.Check.str("mail placeholder", line(V.Card.of(st, R.Ref.mail("c", "m"))), "mail · m")
    C.Check.str("decision placeholder", line(V.Card.of(st, R.Ref.decision("b", "d"))), "decision · d")
    C.Check.str("bare exists", line(V.Card.bare(st, R.Ref.thread("t1-1"))), "First · Project · idle")
    C.Check.str("bare missing", line(V.Card.bare(st, R.Ref.thread("no"))), "missing")
    C.Check.str("bare PR refused", line(V.Card.bare(st, R.Ref.pr("r", "1"))), "missing")
    C.Check.str("peek", line(V.Cards.peek(cs, R.Ref.uri(R.Ref.thread("t1-1")))), "First · Project · idle")
    C.Check.str("drop unrelated", line(V.Cards.peek(V.Cards.drop(cs, ["other"]), R.Ref.uri(R.Ref.thread("t1-1")))), "First · Project · idle")
    C.Check.str("drop dependency", line(V.Cards.peek(V.Cards.drop(cs, ["p1-1"]), R.Ref.uri(R.Ref.thread("t1-1")))), "missing")
    C.Check.str("retitle fresh", Bool.show(fresh(st, M.ThreadRenamed{"t1-1", "New", 11n}, R.Ref.thread("t1-1"))), "True")
    C.Check.str("settle fresh", Bool.show(fresh(st, M.ThreadSettled{"t1-1", True{}, 11n}, R.Ref.thread("t1-1"))), "True")
    C.Check.str("start fresh", Bool.show(fresh(st, M.TurnChanged{"t1-1", M.Running{}, 11n}, R.Ref.thread("t1-1"))), "True")
    C.Check.str("end fresh", Bool.show(fresh(st, M.TurnChanged{"t1-1", M.Completed{}, 11n}, R.Ref.thread("t1-1"))), "True")
    C.Check.str("delete fresh", Bool.show(fresh(st, M.ThreadDeleted{"t1-1", 11n}, R.Ref.thread("t1-1"))), "True")
    C.Check.str("project rename fresh", Bool.show(fresh(st, M.ProjectCreated{"p1-1", "Renamed", "/p", 11n}, R.Ref.thread("t1-1"))), "True")
    C.Check.str("ref go", U.Ui.sel(go), "t1-2")
    C.Check.str("ref back", U.Ui.sel(back), "t1-1")
    C.Check.str("ref fwd", U.Ui.sel(fwd), "t1-2")
    C.Check.str("invalid ref stays", U.Ui.sel(U.Act.ui(U.Ui.act(u, "ref-go", "javascript:x"))), "t1-1")
    C.Check.str("copy", cmds(U.Ui.act(u, "ref-copy", "backplane://person")), "backplane://person")
    C.Check.str("copy web", cmds(U.Ui.act(u, "ref-copy-web", "backplane://person")), "http://127.0.0.1:3787/#/person")
    C.Check.str("thread menu", menu.last(U.Menu.thread(I.En{}, st, 10n, "t1-1", False{})), "ref-copy backplane://thread/t1-1")
    C.Check.str("project menu", menu.last(U.Menu.project(I.En{}, "p1-1", False{}, True{})), "ref-copy backplane://project/p1-1")
    C.Check.str("bot menu", menu.last(U.Menu.bot(I.En{}, st, "b1-1")), "ref-copy backplane://bot/b1-1")
    C.Check.str("file menu", menu.last(U.Menu.file.here(u, "main.kicad_pcb")), "ref-copy backplane://file/p1-1/main.kicad_pcb")
    C.Check.str("room menu", menu.last(U.Menu.room(I.En{}, "r1-1")), "ref-copy backplane://room/r1-1")
    C.Check.str("message menu", menu.last(U.Menu.msg(I.En{}, "t1-1", "m")), "ref-copy backplane://thread/t1-1#m")
    C.Check.str("empty title", line(V.Card.of(M.State.apply(st, M.ThreadRenamed{"t1-1", "", 11n}), R.Ref.thread("t1-1"))), "New thread · Project · idle")
    C.Check.str("working", line(V.Card.of(M.State.apply(st, M.TurnChanged{"t1-1", M.Running{}, 11n}), R.Ref.thread("t1-1"))), "First · Project · working")
    C.Check.str("waiting", line(V.Card.of(M.State.apply(st, M.PendingChanged{"t1-1", 1n, 11n}), R.Ref.thread("t1-1"))), "First · Project · waiting")
    C.Check.str("archived", line(V.Card.of(M.State.apply(st, M.ThreadArchived{"t1-1", True{}, 11n}), R.Ref.thread("t1-1"))), "First · Project · archived")
    C.Check.str("auto settled", line(V.Card.of(M.State.apply(st, M.ThreadSettled{"t1-1", False{}, 11n}), R.Ref.thread("t1-1"))), "First · Project · settled")
    C.Check.str("routine off", line(V.Card.of(M.State.apply(st, M.RoutineSet{"o1-1", "b1-1", "Review", "* * * * *", "go", False{}, 11n}), R.Ref.routine("o1-1"))), "Review · Ada · off")
    C.Check.str("missing bot", line(V.Card.of(st, R.Ref.bot("missing"))), "missing")
    C.Check.str("missing room", line(V.Card.of(st, R.Ref.room("missing"))), "missing")
    C.Check.str("missing routine", line(V.Card.of(st, R.Ref.routine("missing"))), "missing")
    C.Check.str("missing task", line(V.Card.of(st, R.Ref.task("missing"))), "missing")
    C.Check.str("missing work", line(V.Card.of(st, R.Ref.work("w-t1-1-100"))), "missing")
    C.Check.str("missing charter", line(V.Card.of(st, R.Ref.charter("missing"))), "missing")
    C.Check.str("bare bot", line(V.Card.bare(st, R.Ref.bot("b1-1"))), "Ada · bot · idle")
    C.Check.str("bare room", line(V.Card.bare(st, R.Ref.room("r1-1"))), "Design · room")
    C.Check.str("bare project", line(V.Card.bare(st, R.Ref.project("p1-1"))), "Project · project")
    C.Check.str("bare routine", line(V.Card.bare(st, R.Ref.routine("o1-1"))), "Review · Ada · on")
    C.Check.str("bare task", line(V.Card.bare(st, R.Ref.task("b1-1-1"))), "Build · doing")
    C.Check.str("trail current", V.Trail.cur(U.Trail.read(go)), "backplane://thread/t1-2")
    C.Check.str("trail back current", V.Trail.cur(U.Trail.read(back)), "backplane://thread/t1-1")
    C.Check.str("trail pure go back", V.Trail.cur(V.Trail.back(V.Trail.go(V.Trail{Nil{}, "a", Nil{}}, "b"))), "a")
    C.Check.str("trail pure back fwd", V.Trail.cur(V.Trail.fwd(V.Trail.back(V.Trail{["a"], "b", Nil{}}))), "b")
    C.Check.str("trail clears forward", Bool.show(V.Trail.can_fwd(V.Trail.go(V.Trail{Nil{}, "a", ["b"]}, "c"))), "False")
    C.Check.str("trail same keeps forward", Bool.show(V.Trail.can_fwd(V.Trail.go(V.Trail{Nil{}, "a", ["b"]}, "a"))), "True")
    C.Check.str("ref bot", U.Ui.scratch(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://bot/b1-1")), "@bot"), "b1-1")
    C.Check.str("ref room", U.Ui.scratch(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://room/r1-1")), "@room"), "r1-1")
    C.Check.str("ref routine", U.Ui.scratch(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://routine/o1-1")), "@btab"), "routines")
    C.Check.str("ref file", U.Ui.view_get(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://file/p1-1/main.kicad_pcb")), "view.path"), "/p/main.kicad_pcb")
    C.Check.str("ref file kind", U.Ui.view_get(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://file/p1-1/main.kicad_pcb")), "view.kind"), "board")
    C.Check.str("ref work", U.Ui.sel(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://work/w-t1-1-9"))), "t1-1")
    C.Check.str("ref unresolved stays", U.Ui.sel(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://thread/missing"))), "t1-1")
    C.Check.str("copy web base", cmds(U.Ui.act(U.Ui.info_set(u, "hub.base", "https://box/"), "ref-copy-web", "backplane://thread/t1-1#m")), "https://box/#/thread/t1-1#m")
    C.Check.str("absolute file menu", menu.last(U.Menu.file.here(u, "/p/main.kicad_pcb")), "ref-copy backplane://file/p1-1/main.kicad_pcb")
    C.Check.str("schematic kind", line(V.Card.of(st, R.Ref.file("p1-1", ["x.kicad_sch"]))), "x.kicad_sch · Project · schematic")
    C.Check.str("model kind", line(V.Card.of(st, R.Ref.file("p1-1", ["x.STEP"]))), "x.STEP · Project · 3d")
    C.Check.str("image kind", line(V.Card.of(st, R.Ref.file("p1-1", ["x.png"]))), "x.png · Project · image")
    C.Check.str("PDF kind", line(V.Card.of(st, R.Ref.file("p1-1", ["x.pdf"]))), "x.pdf · Project · pdf")
    C.Check.str("sig separates fields", Bool.show(String.eq(V.Card.sig(V.Card{"u", "k", "i", "a/b", "c", "", ""}), V.Card.sig(V.Card{"u", "k", "i", "a", "b/c", "", ""}))), "False")
    C.Check.str("sig includes icon", Bool.show(String.eq(V.Card.sig(V.Card{"u", "k", "i", "a", "b", "", ""}), V.Card.sig(V.Card{"u", "k", "j", "a", "b", "", ""}))), "False")
    C.Check.str("fragment jump command", jumped(U.Ui.act(u, "ref-go", "backplane://thread/t1-2#m42")), "m42")
    C.Check.str("task without thread note", note(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://task/b1-1-1"))), "No view for this task yet")
    C.Check.str("PR with project note", note(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://pr/p1-1/128"))), "PR link needs a repo")
    C.Check.str("PR opener command", Bool.show(String.contains(sent(U.Ui.act(u, "ref-go", "backplane://pr/i2cjak%2FBackplane/128")), "https://github.com/i2cjak/Backplane/pull/128")), "True")
    C.Check.str("mailroom opens bot", U.Ui.scratch(U.Act.ui(U.Ui.act(u, "ref-go", "backplane://mailroom/b1-1")), "@bot"), "b1-1")
    C.Check.str("bot follows thread turn", line(V.Cards.result.value(V.Cards.get(V.Cards.drop(V.Cards.result.cards(V.Cards.get(V.Cards.empty(), st, "backplane://bot/b1-1")), ["t1-1"]), M.State.apply(st, M.TurnChanged{"t1-1", M.Running{}, 11n}), "backplane://bot/b1-1"))), "Ada · bot · working")
    C.Check.str("routine follows bot name", line(V.Cards.result.value(V.Cards.get(V.Cards.drop(V.Cards.result.cards(V.Cards.get(V.Cards.empty(), st, "backplane://routine/o1-1")), ["b1-1"]), M.State.apply(st, M.BotSet{"b1-1", "Grace", "t1-1", 0n, "", 11n}), "backplane://routine/o1-1"))), "Review · Grace · on")
    C.Check.str("cached miss becomes present", line(V.Cards.result.value(V.Cards.get(V.Cards.drop(V.Cards.result.cards(V.Cards.get(V.Cards.empty(), st, "backplane://thread/new")), ["new"]), M.State.apply(st, M.ThreadCreated{"new", "p1-1", "Created", M.Local{}, "", "", "claude", "", 11n}), "backplane://thread/new"))), "Created · Project · idle")
    C.Check.str("message wakes cached auto settle", Bool.show(fresh(M.State.apply(st, M.ThreadSettled{"t1-1", False{}, 10n}), M.MessagePosted{"t1-1", "new", M.User{}, "Hello", 11n}, R.Ref.thread("t1-1"))), "True")
    C.Check.str("pin refreshes cached shelf", Bool.show(fresh(M.State.apply(st, M.ThreadSettled{"t1-1", False{}, 10n}), M.ThreadPinned{"t1-1", True{}, 11n}, R.Ref.thread("t1-1"))), "True")
    C.Check.str("refilling one URI cannot revive another", line(V.Cards.peek(one, "backplane://thread/t1-2")), "missing")
    C.Check.str("refilled URI is current", line(V.Cards.peek(one, "backplane://thread/t1-1")), "First · Project · idle")
    C.Check.str("other URI refills at current generation", line(V.Cards.result.value(V.Cards.get(one, st, "backplane://thread/t1-2"))), "Second · Project · idle")
    C.Check.str("unreferenced drops do not grow the index", Nat.show(cache.ids(V.Cards.drop(cs, ["uncached"]))), "2")
    C.Check.str("repeated drops stay invalid", line(V.Cards.peek(V.Cards.drop(V.Cards.drop(cs, ["p1-1"]), ["p1-1"]), "backplane://thread/t1-1")), "missing")
    C.Check.str("empty URI miss is cached", Nat.show(cache.entries(V.Cards.result.cards(V.Cards.get(V.Cards.empty(), st, "")))), "1")
    C.Check.str("file menu strips line annotation", menu.last(U.Menu.file.here(u, "/p/main.kicad_pcb:12")), "ref-copy backplane://file/p1-1/main.kicad_pcb")
