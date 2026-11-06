import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/web/dom.bend as D
import ../src/web/view.bend as WV

# A Diff pressed again draws the thread's diff kept from before (its own
# last answer, or the hub's: "diff.kept:t") while git runs again, its git
# actions off until the answer; with nothing kept it says "Loading…"; a
# folder kept as no repository says so; only that thread's diff is kept.

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def json(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def on(u: U.Ui, +id: String) -> U.Ui:
  ui(U.Ui.act(u, "select", id))

def patch() -> String:
  "diff --git a/x.txt b/x.txt\n--- a/x.txt\n+++ b/x.txt\n@@ -1 +1 @@\n-a\n+b\n"

def reply(+diff: String, +nogit: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"7"}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", J.Json.obj([J.KV{"diff", J.Str{diff}}, J.KV{"thread", J.Str{"a"}}, J.KV{"nogit", J.Str{nogit}}])}])

def texts(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.Txt{t} <> rest: t ++ " " ++ texts(rest)
    case D.El{_, _, _, ks} <> rest: texts(ks) ++ texts(rest)

def offs.attrs(as: List<&2, D.Attr>) -> Nat:
  match as:
    case Nil{}: 0n
    case D.At{n, _} <> rest: Nat.add(Bool.pick(Nat, String.eq(n, "disabled"), 1n, 0n), offs.attrs(rest))
    case D.On{_, _} <> rest: offs.attrs(rest)

def offs(ns: List<&2, D.Node>) -> Nat:
  match ns:
    case Nil{}: 0n
    case D.Txt{_} <> rest: offs(rest)
    case D.El{_, _, as, ks} <> rest: Nat.add(offs.attrs(as), Nat.add(offs(ks), offs(rest)))

def drawn(+u: U.Ui) -> List<&2, D.Node>:
  WV.View.diff.at(I.En{}, U.Ui.view_get(u, "diff"), U.Ui.view_get(u, "diff.none"), U.Ui.view_get(u, "diff.wait"), U.Ui.view_get(u, "diff.kept"))

def main() -> IO(Unit):
  +a = on(U.Ui.init(0n), "a")
  +fresh = ui(U.Ui.act(a, "diff", ""))
  +answered = U.Ui.recv(fresh, reply(patch(), ""))
  +again = ui(U.Ui.act(answered, "diff", ""))
  +hub = ui(U.Ui.act(U.Ui.info_set(a, "diff.kept:a", J.Json.show(J.Json.obj([J.KV{"diff", J.Str{patch()}}]))), "diff", ""))
  +nogit = ui(U.Ui.act(U.Ui.recv(fresh, reply("", "/p")), "diff", ""))
  do IO<Unit>:
    C.Check.str("nothing kept: the pressed Diff says Loading", Bool.show(String.contains(texts(drawn(fresh)), "Loading")), "True")
    C.Check.str("an answer keeps the thread's diff", Bool.show(String.contains(U.Ui.info(answered, "diff.kept:a"), "x.txt")), "True")
    C.Check.str("pressed again, it still asks git", U.Ui.view_get(again, "diff.wait"), "1")
    C.Check.str("pressed again, the kept diff is drawn at once", Bool.show(String.contains(texts(drawn(again)), "x.txt")) ++ "|" ++ Bool.show(String.contains(texts(drawn(again)), "Loading")), "True|False")
    C.Check.str("its git actions are off until git answers", Nat.show(offs(drawn(again))), "3")
    C.Check.str("git's answer turns them on", Nat.show(offs(drawn(U.Ui.recv(again, reply(patch(), ""))))), "0")
    C.Check.str("the hub's kept diff is drawn at the press", Bool.show(String.contains(texts(drawn(hub)), "x.txt")), "True")
    C.Check.str("another thread is given no kept diff", U.Ui.view_get(on(answered, "b"), "diff.kept"), "")
    C.Check.str("a folder kept as no repository says so at the press", Bool.show(String.contains(texts(drawn(nogit)), "git init")), "True")
