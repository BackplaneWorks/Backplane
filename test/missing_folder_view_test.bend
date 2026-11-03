import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/i18n.bend as I
import ../src/core/roots.bend as R
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as WP

# A project whose folder is gone (core/roots.bend, view.bend's View.gone.*):
# its threads say so above the composer, with what to do, and its sidebar
# head is marked; a project whose folder is there shows nothing new.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "here", "/work/here", 1n},
   M.ProjectCreated{"p2", "gone", "/work/gone", 2n},
   M.ThreadCreated{"t1", "p1", "One", M.Local{}, "", "/work/here", "claude", "", 3n},
   M.ThreadCreated{"t2", "p2", "Two", M.Local{}, "", "/work/gone", "claude", "", 4n},
   M.SettingSet{R.Roots.key(), " p2 ", 5n}]

def st() -> M.State:
  M.State.replay(log(), M.State.empty())

def html(ns: List<&2, D.Node>) -> String:
  J.Json.show(WP.Node.write(D.El{"div", "test", Nil{}, ns}))

def gone(+t: String, +armed: String) -> String:
  html(V.View.gone(I.En{}, st(), t, Map.set(&2, String, Map.new(&2, String), "@rm", armed)))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a thread of the gone project says so", Bool.show(String.contains(gone("t2", ""), "This project's folder is missing")), "True")
    C.Check.str("it names the path", Bool.show(String.contains(gone("t2", ""), "/work/gone")), "True")
    C.Check.str("it offers Remove project", Bool.show(String.contains(gone("t2", ""), "proj-remove")) ++ Bool.show(String.contains(gone("t2", ""), "Remove project")), "TrueTrue")
    C.Check.str("it asks where the folder is now", Bool.show(String.contains(gone("t2", ""), "ws-proj-move")) ++ Bool.show(String.contains(gone("t2", ""), "Use this folder")), "TrueTrue")
    C.Check.str("a second click removes (armed)", Bool.show(String.contains(gone("t2", "p2"), "Click again to remove")), "True")
    C.Check.str("a thread of a project that is there: nothing", gone("t1", ""), html(Nil{}))
    C.Check.str("the sidebar chip only when missing", html(V.View.gone.chip(True{})) ++ "|" ++ html(V.View.gone.chip(False{})), html([D.Dom.span("missing-note", "folder missing")]) ++ "|" ++ html(Nil{}))
