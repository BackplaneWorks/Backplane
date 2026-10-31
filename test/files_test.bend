import Base
import ../src/mobile/view.bend as MV
import ../src/core/images.bend as IM
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/fm.bend as FM

# The viewer's Files tab (core/fm.bend, client.bend's Fm): a folder asked
# for, listed, its rows; files opened in the view their kind takes; a PDF
# asks for its page; sizes as people read them.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "board", "/w/board", 1n}, M.ThreadCreated{"t1", "p1", "Bring-up", M.Local{}, "", "", "claude", "", 2n}]

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def base() -> U.Ui:
  ui(U.Ui.act(U.Ui.fold(log(), U.Ui.init(10n)), "select", "t1"))

def crumbs(ps: List<&2, U.PItem>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{l, _, _} <> t:
      l <> crumbs(t)

def act(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

def sent.go(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> _:
      t
    case _ <> r:
      sent.go(r)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sent.go(cs)

def listed(+u: U.Ui, +dir: String, +entries: String) -> U.Ui:
  U.Ui.recv(u, J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"1"}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", J.Json.obj([J.KV{"fmEntries", J.Str{entries}}, J.KV{"fmDir", J.Str{dir}}, J.KV{"fmRoot", J.Str{"/w/board"}}, J.KV{"thread", J.Str{"t1"}}])}]))

def labels(rs: List<&2, U.FmRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case U.FmRow{l, a, _, d, k} <> t:
      (l ++ ":" ++ a ++ ":" ++ k ++ ":" ++ d) <> labels(t)

def failed() -> U.Ui:
  U.Fm.got.k(True{}, listed(base(), "gone", "f\t4\tnote.md"), J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"fmDir", J.Str{"gone"}}, J.KV{"fmEntries", J.Str{""}}, J.KV{"fmError", J.Str{"no such folder"}}]))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("restored Files asks for its root", sent(U.Fm.sync(U.Act{U.Ui.view_put(base(), "files", ""), Nil{}})), "{\"id\":1,\"m\":\"files.dir\",\"p\":{\"thread\":\"t1\",\"path\":\"\"}}")
    C.Check.str("a Files read is only requested once", sent(U.Fm.sync(U.Act{ui(U.Fm.sync(U.Act{U.Ui.view_put(base(), "files", ""), Nil{}})), Nil{}})), "")
    C.Check.str("an ordinary Files click is not requested twice", sent(U.Fm.sync(U.Act{act(base(), "fm", ""), Nil{}})), "")
    C.Check.str("a closed viewer does not request Files", sent(U.Fm.sync(U.Act{base(), Nil{}})), "")
    C.Check.str("phone opens README as text", MV.View.kind(ui(MV.View.fm.open(base(), "README.md"))), "text")
    C.Check.str("phone opens notes as text", MV.View.kind(ui(MV.View.fm.open(base(), "notes.txt"))), "text")
    C.Check.str("phone text stays in Files", Bool.show(MV.View.fm(ui(MV.View.fm.open(base(), "README.md")))) , "True")
    C.Check.str("phone text model names its kind", J.Json.get_str(MV.File.json(True{}, ui(MV.View.fm.open(base(), "README.md"))), "kind", ""), "text")
    C.Check.str("phone text model names its source", J.Json.get_str(MV.File.json(True{}, ui(MV.View.fm.open(base(), "README.md"))), "url", ""), IM.Img.url("/w/board/README.md"))
    C.Check.str("text preview endpoint accepts notes", Bool.show(IM.Img.viewable("/r/notes.txt")), "True")
    C.Check.str("Markdown is served as literal text", IM.Img.ctype("/r/README.MD"), "text/plain; charset=utf-8")
    C.Check.str("HTML is not a file preview", Bool.show(IM.Img.viewable("/r/note.html")), "False")
    C.Check.str("text notes open as text", FM.Fm.preview("notes.txt"), "text")
    C.Check.str("Markdown notes open as text", FM.Fm.preview("README.MD"), "text")
    C.Check.str("binary still has no preview", FM.Fm.preview("firmware.bin"), "")
    C.Check.str("failed refresh clears stale entries", U.Ui.info(failed(), "fm.list:t1"), "")
    C.Check.str("failed refresh explains missing folder", U.Ui.info(failed(), "fm.err:t1"), "no such folder")
    C.Check.str("successful refresh clears folder error", U.Ui.info(listed(failed(), "", ""), "fm.err:t1"), "")
    # (the first action on a thread also asks for its card, Fab.sync: one request id before it)
    C.Check.str("Files asks for the folder", sent(U.Ui.act(base(), "fm", "")), "{\"id\":1,\"m\":\"files.dir\",\"p\":{\"thread\":\"t1\",\"path\":\"\"}}")
    C.Check.str("the viewer shows Files", U.Ui.view_get(act(base(), "fm", ""), "view.kind"), "files")
    C.Check.str("not ready before the hub answers", Bool.show(U.Fm.ready(act(base(), "fm", ""))), "False")
    C.Check.str("answered empty root is ready", Bool.show(U.Fm.ready(listed(act(base(), "fm", ""), "", ""))), "True")
    C.Check.str("answered empty nested folder is ready", Bool.show(U.Fm.ready(listed(act(act(base(), "fm", ""), "fm-cd", "hw"), "hw", ""))), "True")
    C.Check.str("empty old root cannot satisfy another folder", Bool.show(U.Fm.ready(listed(act(act(base(), "fm", ""), "fm-cd", "hw"), "", ""))), "False")
    C.Check.str("invalidated root answer is not current", Bool.show(U.Fm.ready(U.Ui.info_drop(listed(act(base(), "fm", ""), "", ""), "fm.dir:t1"))), "False")
    C.Check.str("reconnect refreshes the empty root path", sent(U.Fm.again(True{}, U.Act{act(base(), "fm", ""), Nil{}})), "{\"id\":2,\"m\":\"files.dir\",\"p\":{\"thread\":\"t1\",\"path\":\"\"}}")
    C.Check.str("ordinary reply does not relist root", sent(U.Fm.again(False{}, U.Act{act(base(), "fm", ""), Nil{}})), "")
    C.Check.str("reconnect without a folder request does not invent one", sent(U.Fm.again(True{}, U.Act{base(), Nil{}})), "")
    C.Check.str("the rows, folders first", String.join(labels(U.Fm.rows(listed(act(base(), "fm", ""), "", "d\t0\thw\nf\t2048\tboard.kicad_pcb\nf\t5\tnotes.txt\n"))), "|"),
      "hw:fm-cd:folder:|board.kicad_pcb:fm-open:board:2 KB|notes.txt:fm-open:text:5 B")
    C.Check.str("a folder below", sent(U.Ui.act(act(base(), "fm", ""), "fm-cd", "hw/main")), "{\"id\":2,\"m\":\"files.dir\",\"p\":{\"thread\":\"t1\",\"path\":\"hw/main\"}}")
    C.Check.str("an old answer is not shown", Bool.show(U.Fm.ready(listed(act(act(base(), "fm", ""), "fm-cd", "hw"), "", "d\t0\thw\n"))), "False")
    C.Check.str("up from a folder", sent(U.Ui.act(act(act(base(), "fm", ""), "fm-cd", "hw/main"), "fm-up", "")), "{\"id\":3,\"m\":\"files.dir\",\"p\":{\"thread\":\"t1\",\"path\":\"hw\"}}")
    C.Check.str("the trail", String.join(crumbs(U.Fm.trail(act(act(base(), "fm", ""), "fm-cd", "hw/main"))), "|"), "Project|hw|main")
    C.Check.str("a board opens in the board viewer", U.Ui.view_get(act(base(), "fm-open", "hw/x.kicad_pcb"), "view.kind") ++ " " ++ U.Ui.view_get(act(base(), "fm-open", "hw/x.kicad_pcb"), "view.path"), "board /w/board/hw/x.kicad_pcb")
    C.Check.str("a model opens in 3D", U.Ui.view_get(act(base(), "fm-open", "mech/case.step"), "view.kind"), "3d")
    C.Check.str("a PDF asks for its first page", sent(U.Ui.act(base(), "fm-open", "docs/ds.pdf")), "{\"id\":1,\"m\":\"pdf.page\",\"p\":{\"path\":\"/w/board/docs/ds.pdf\",\"page\":\"1\"}}")
    C.Check.str("a text file opens its preview", U.Ui.view_get(act(base(), "fm-open", "notes.txt"), "view.kind"), "text")
    C.Check.str("sizes", String.join([FM.Fm.size(0n), FM.Fm.size(1500n), FM.Fm.size(3500000n), FM.Fm.size(2147483648n)], "|"), "0 B|1 KB|3.3 MB|2.0 GB")
