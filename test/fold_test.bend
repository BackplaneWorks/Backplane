import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/fold.bend as F
import ../src/core/client.bend as U
import ../src/web/view.bend as V
import ../src/web/dom.bend as D

def tool(+id: String, +kind: String) -> M.Entry:
  M.Act{id, "t", "tool", kind, "x", 0n}

def msg(+id: String) -> M.Entry:
  M.Msg{id, "t", M.Assistant{}, "hi", 0n}

def err(+id: String) -> M.Entry:
  M.Act{id, "t", "error", "exit", "boom", 0n}

def user(+id: String, +at: Nat) -> M.Entry:
  M.Msg{id, "t", M.User{}, "do it", at}

def said(+id: String, +at: Nat) -> M.Entry:
  M.Msg{id, "t", M.Assistant{}, "done", at}

def ran(+id: String, +kind: String, +at: Nat) -> M.Entry:
  M.Act{id, "t", "tool", kind, "x", at}

# a turn: a message, two tool calls, a reply on the way, one more call and
# the last reply 133 s after the message
def turn() -> List<&2, M.Entry>:
  [user("u", 100n), ran("a", "shell", 110n), ran("b", "edit", 120n), said("m1", 150n), ran("c", "read", 160n), said("m2", 233n)]

def at(+now: Nat) -> Map<&2, String>:
  Map.set(&2, String, Map.new(&2, String), "clock", Nat.show(now))

def Show.idl(es: List<&2, M.Entry>) -> List<&2, String>:
  match es:
    case Nil{}:
      Nil{}
    case e <> t:
      F.Fold.id(e) <> Show.idl(t)

# groups as text: m for a single entry, [ids] for a run
def Show.ids(es: List<&2, M.Entry>) -> String:
  String.join(Show.idl(es), ",")

def Show.gs(gs: List<&2, F.Grp>) -> String:
  match gs:
    case Nil{}:
      ""
    case F.GOne{e} <> t:
      F.Fold.id(e) ++ " " ++ Show.gs(t)
    case F.GRun{xs} <> t:
      "[" ++ Show.ids(xs) ++ "] " ++ Show.gs(t)

def Show.role(r: F.Role) -> String:
  match r:
    case F.RShow{}:
      "s"
    case F.RHead{id, label, open, live}:
      "h(" ++ id ++ "|" ++ label ++ "|" ++ Bool.show(open) ++ Bool.pick(String, live, "|live", "") ++ ")"
    case F.RIn{open}:
      "i(" ++ Bool.show(open) ++ ")"
    case F.RWork{id, label, open, inner}:
      "w(" ++ id ++ "|" ++ label ++ "|" ++ Bool.show(open) ++ "|" ++ Show.role(inner) ++ ")"

def Show.wrole(w: F.WRole) -> String:
  match w:
    case F.WPlain{}:
      "p"
    case F.WHead{id, _, open}:
      "h(" ++ id ++ "|" ++ Bool.show(open) ++ ")"
    case F.WIn{open}:
      "i(" ++ Bool.show(open) ++ ")"

def Show.wroles(ws: List<&2, F.WRole>) -> String:
  match ws:
    case Nil{}:
      ""
    case w <> t:
      Show.wrole(w) ++ " " ++ Show.wroles(t)

def Show.roles(rs: List<&2, F.Role>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      Show.role(r) ++ " " ++ Show.roles(t)

def es1() -> List<&2, M.Entry>:
  [msg("m1"), tool("a", "shell"), tool("b", "edit"), tool("c", "shell"), err("e"), tool("d", "read"), msg("m2"), tool("f", "shell")]

def none() -> Map<&2, String>:
  Map.new(&2, String)

# the web's nodes as tags and text (of the attributes, class and data-value)
def Web.attrs(xs: List<&2, D.Attr>) -> String:
  match xs:
    case Nil{}:
      ""
    case D.At{"class", v} <> t:
      "." ++ v ++ Web.attrs(t)
    case D.At{"data-value", v} <> t:
      "=" ++ v ++ Web.attrs(t)
    case D.At{"data-run", v} <> t:
      "~" ++ v ++ Web.attrs(t)
    case _ <> t:
      Web.attrs(t)

def Web.shows(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}:
      ""
    case D.Txt{s} <> t:
      s ++ Web.shows(t)
    case D.El{"svg", _, _, _} <> t:
      "^" ++ Web.shows(t)
    case D.El{+tag, _, xs, ks} <> t:
      "<" ++ tag ++ Web.attrs(xs) ++ ">" ++ Web.shows(ks) ++ "</" ++ tag ++ ">" ++ Web.shows(t)

def after(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

# Runs across a page's edges
# --------------------------

def blank(+id: String) -> M.Entry:
  M.Msg{id, "t", M.Assistant{}, " \n ", 0n}

def calls(n: Nat, +i: Nat) -> List<&2, M.Entry>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      tool("c" ++ Nat.show(i), "Bash") <> calls(k, 1n+i)

# thirteen calls in a row between two messages, oldest first
def thread13() -> List<&2, M.Entry>:
  List.append(&2, M.Entry, [user("u", 0n), said("m1", 1n)], List.append(&2, M.Entry, calls(13n, 1n), [said("m2", 2n)]))

# the same with a blank message after every call
def blanks(es: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      e <> blank("b" ++ F.Fold.id(e)) <> blanks(t)

def posted(es: List<&2, M.Entry>, st: M.State) -> M.State:
  match es:
    case Nil{}:
      st
    case +e <> t:
      posted(t, M.State.post(st, e))

# each call shown as a page of its own (as the hub-drawn page's windows and
# the rows it keeps from earlier pictures did): the roles each page gets
def pages(ps: List<&2, M.Entry>, +newest: List<&2, M.Entry>, +running: Bool) -> String:
  match ps:
    case Nil{}:
      ""
    case +e <> t:
      Show.roles(F.Fold.roles.ctx(I.En{}, F.Fold.context(Nil{}, newest, Nil{}, [e]), [e], none(), running)) ++ pages(t, newest, running)

def heads13(n: Nat) -> String:
  match n:
    case 0n:
      ""
    case 1n+k:
      "h(c1|13 tool calls · Bash|False) " ++ heads13(k)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("groups", Show.gs(F.Fold.groups(es1())), "m1 [a,b,c] e [d] m2 [f] ")
    C.Check.str("flat keeps all", Show.ids(F.Fold.flat(F.Fold.groups(es1()))), Show.ids(es1()))
    C.Check.str("count", Nat.show(F.Fold.count(F.Fold.groups(es1()))), "8")
    C.Check.str("runs only tools", Bool.show(F.Fold.all(F.Fold.runs(F.Fold.groups(es1())))), "True")
    C.Check.str("empty", Show.gs(F.Fold.groups(Nil{})), "")
    C.Check.str("13 calls in a row: one drawer", Show.roles(F.Fold.roles(calls(13n, 1n), none(), False{})),
      "h(c1|13 tool calls · Bash|False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) ")
    C.Check.str("13 one-call pages: the same drawer on each", pages(calls(13n, 1n), List.reverse(&2, M.Entry, thread13()), False{}), heads13(13n))
    C.Check.str("a page cut mid-run while the turn runs: the whole run, live",
      Show.roles(F.Fold.roles.ctx(I.En{}, F.Fold.context(Nil{}, List.reverse(&2, M.Entry, calls(13n, 1n)), Nil{}, [tool("c12", "Bash"), tool("c13", "Bash")]), [tool("c12", "Bash"), tool("c13", "Bash")], none(), True{})),
      "h(c1|13 tool calls · running: Bash|False|live) i(False) ")
    C.Check.str("opened from any page: the run's own key", Show.roles(F.Fold.roles.ctx(I.En{}, F.Fold.context(Nil{}, List.reverse(&2, M.Entry, thread13()), Nil{}, [tool("c5", "Bash")]), [tool("c5", "Bash")], Map.set(&2, String, none(), "fold:c1", "1"), False{})),
      "h(c1|13 tool calls · Bash|True) ")
    C.Check.str("a page from a message: the context stops at it", Show.roles(F.Fold.roles.ctx(I.En{}, F.Fold.context(Nil{}, List.reverse(&2, M.Entry, thread13()), Nil{}, [said("m2", 2n)]), [said("m2", 2n)], none(), False{})), "s ")
    C.Check.str("no context: as Fold.roles", Show.roles(F.Fold.roles.ctx(I.En{}, F.Around.none(), es1(), none(), False{})), Show.roles(F.Fold.roles(es1(), none(), False{})))
    C.Check.str("blank messages between calls: one drawer", Show.roles(F.Fold.roles(F.Fold.shown(blanks(calls(13n, 1n))), none(), False{})),
      "h(c1|13 tool calls · Bash|False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) i(False) ")
    C.Check.str("blank messages kept, hidden (the window)", Show.roles(F.Fold.roles.all(I.En{}, F.Around.none(), [tool("a", "x"), blank("z"), tool("b", "x"), said("m", 0n)], none(), False{})),
      "h(a|2 tool calls · x|False) i(False) i(False) s ")
    C.Check.str("text between calls splits them", Show.roles(F.Fold.roles(F.Fold.shown([tool("a", "x"), blank("z"), said("m", 0n), tool("b", "x")]), none(), False{})),
      "h(a|1 tool call · x|False) s h(b|1 tool call · x|False) ")
    C.Check.str("web: a thread's page from mid-run, one drawer", Web.shows(V.View.expand(V.View.entries(I.En{}, [tool("c7", "Bash"), tool("c8", "Bash")], posted(thread13(), M.State.empty()), "t", none(), False{}))),
      "<button.fold=1c1>^<span>13 tool calls · Bash</span></button>")
    C.Check.str("web: the hub's window, its outside calls filed beside it", Web.shows(V.View.expand(V.View.entries(I.En{}, [tool("c7", "Bash")],
        posted(F.Around.files(F.Around.after.key("t"), calls(6n, 8n)), posted(F.Around.files(F.Around.before.key("t"), calls(6n, 1n)), posted([tool("c7", "Bash")], M.State.empty()))), "t", none(), False{}))),
      "<button.fold=1c1>^<span>13 tool calls · Bash</span></button>")
    C.Check.str("web: blank messages show nothing and split nothing", Web.shows(V.View.expand(V.View.entries(I.En{}, blanks([tool("a", "shell"), tool("b", "shell")]), M.State.empty(), "t", none(), False{}))),
      "<button.fold=1a>^<span>2 tool calls · shell</span></button>")
    C.Check.str("label", F.Fold.label(I.En{}, [tool("a", "shell"), tool("b", "edit"), tool("c", "shell"), tool("d", "read")]), "4 tool calls · shell, edit, read")
    C.Check.str("label one", F.Fold.label(I.En{}, [tool("a", "shell")]), "1 tool call · shell")
    C.Check.str("label many kinds", F.Fold.label(I.En{}, [tool("a", "a"), tool("b", "b"), tool("c", "c"), tool("d", "d")]), "4 tool calls · a, b, c, …")
    C.Check.str("roles idle", Show.roles(F.Fold.roles(es1(), none(), False{})),
      "s h(a|3 tool calls · shell, edit|False) i(False) i(False) s h(d|1 tool call · read|False) s h(f|1 tool call · shell|False) ")
    C.Check.str("roles running: the last run shut too, saying what runs", Show.roles(F.Fold.roles(es1(), none(), True{})),
      "s h(a|3 tool calls · shell, edit|False) i(False) i(False) s h(d|1 tool call · read|False) s h(f|1 tool call · running: shell|False|live) ")
    C.Check.str("roles running: a live run of three says its latest", Show.roles(F.Fold.roles([msg("m"), tool("a", "Read"), tool("b", "Edit"), tool("c", "Bash")], none(), True{})),
      "s h(a|3 tool calls · running: Bash|False|live) i(False) i(False) ")
    C.Check.str("roles running: the person opened the live run", Show.roles(F.Fold.roles([tool("a", "Read"), tool("b", "Bash")], Map.set(&2, String, none(), "fold:a", "1"), True{})),
      "h(a|2 tool calls · running: Bash|True|live) i(True) ")
    C.Check.str("roles running: a run of design changes alone has no hint", Show.roles(F.Fold.roles([M.Act{"s", "t", "step", "", "x", 0n}], none(), True{})),
      "h(s|1 design change|False|live) ")
    C.Check.str("ja live hint", Show.roles(F.Fold.roles.loc(I.Ja{}, [tool("a", "Bash")], none(), True{})), "h(a|ツール呼び出し1件 · 実行中: Bash|False|live) ")
    C.Check.str("no stored choice: shut, live or not", Bool.show(F.Fold.run.open("", True{})) ++ Bool.show(F.Fold.run.open("", False{})), "FalseFalse")
    C.Check.str("roles running: not at the bottom", Show.roles(F.Fold.roles([tool("a", "x"), msg("m")], none(), True{})), "h(a|1 tool call · x|False) s ")
    C.Check.str("user opened", Show.roles(F.Fold.roles([tool("a", "x"), tool("b", "x")], Map.set(&2, String, none(), "fold:a", "1"), False{})), "h(a|2 tool calls · x|True) i(True) ")
    C.Check.str("user shut the live one", Show.roles(F.Fold.roles([tool("a", "x")], Map.set(&2, String, none(), "fold:a", "0"), True{})), "h(a|1 tool call · running: x|False|live) ")
    C.Check.str("ask", F.Fold.ask(True{}, "a") ++ " " ++ F.Fold.ask(False{}, "a"), "0a 1a")
    C.Check.str("took", M.Time.took(45n) ++ " " ++ M.Time.took(133n) ++ " " ++ M.Time.took(3900n), "45s 2m 13s 1h 5m")
    C.Check.str("turn done: its messages show, only tool runs fold", Show.roles(F.Fold.roles(turn(), at(533n), False{})),
      "s h(a|2 tool calls · shell, edit|False) i(False) s h(c|1 tool call · read|False) s ")
    C.Check.str("turn running: the last run shut", Show.roles(F.Fold.roles([user("u", 0n), said("m", 1n), ran("a", "x", 2n)], at(533n), True{})),
      "s s h(a|1 tool call · running: x|False|live) ")
    C.Check.str("turn running: its work shut until opened", Show.wroles(F.Work.turn(True{}, user("u", 0n), [ran("a", "x", 1n), said("m", 2n)], at(533n), 533n)), "p h(work:u|False) i(False) ")
    C.Check.str("turn with no reply: nothing folds", Show.roles(F.Fold.roles([user("u", 0n), ran("a", "x", 1n), err("e")], none(), False{})),
      "s h(a|1 tool call · x|False) s ")
    C.Check.str("turn answered at once: nothing folds", Show.roles(F.Fold.roles([user("u", 0n), said("m", 1n)], none(), False{})), "s s ")
    C.Check.str("a page from mid-turn: nothing folds", Show.roles(F.Fold.roles([ran("a", "x", 1n), said("m", 2n)], none(), False{})), "h(a|1 tool call · x|False) s ")
    C.Check.str("clock ticks into the info map", U.Ui.info(U.Ui.tick(U.Ui.init(0n), 42n), "clock"), "42")
    C.Check.str("click opens", U.Ui.info(after(U.Ui.act(U.Ui.init(0n), "fold", "1a")), "fold:a"), "1")
    C.Check.str("click shuts", U.Ui.info(after(U.Ui.act(after(U.Ui.act(U.Ui.init(0n), "fold", "1a")), "fold", "0a")), "fold:a"), "0")
    C.Check.str("web folded", Web.shows(V.View.expand(V.View.entries(I.En{}, [tool("a", "shell"), tool("b", "edit"), err("e")], M.State.empty(), "t", none(), False{}))),
      "<button.fold=1a>^<span>2 tool calls · shell, edit</span></button><div.act error><span.kind>exit</span><span.summary>boom</span></div>")
    C.Check.str("web live shut, saying what runs", Web.shows(V.View.expand(V.View.entries(I.En{}, [msg("m"), tool("a", "shell")], M.State.empty(), "t", none(), True{}))),
      "<div.msg assistant><div.body><p>hi</p></div><span.stamp>00:00</span><button.copy=hi>Copy</button></div><button.fold live=1a>^<span>1 tool call · running: shell</span></button>")
    C.Check.str("web opened: its rows hang in the drawer, marked with its run", Web.shows(V.View.expand(V.View.entries(I.En{}, [tool("a", "shell"), tool("b", "edit"), msg("m")], M.State.empty(), "t", Map.set(&2, String, none(), "fold:a", "1"), False{}))),
      "<button.fold open=0a>^<span>2 tool calls · shell, edit</span></button><div.act tool~a><span.kind>shell</span><span.summary>x</span></div><div.act tool~a><span.kind>edit</span><span.summary>x</span></div><div.msg assistant><div.body><p>hi</p></div><span.stamp>Finished 00:00</span><button.copy=hi>Copy</button></div>")
    C.Check.str("web opened live: open and pulsing", Web.shows(V.View.expand(V.View.entries(I.En{}, [tool("a", "shell")], M.State.empty(), "t", Map.set(&2, String, none(), "fold:a", "1"), True{}))),
      "<button.fold open live=0a>^<span>1 tool call · running: shell</span></button><div.act tool~a><span.kind>shell</span><span.summary>x</span></div>")
