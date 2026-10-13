import Base
import ./check.bend as C
import ./trb_ui.bend as X
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/mobile/screen.bend as MS
import ../src/mobile/hubs.bend as HS

def titles(ss: List<&2, U.BSec>) -> String:
  match ss:
    case Nil{}:
      ""
    case U.BSec{t, _, _} <> r:
      t ++ ";" ++ titles(r)

def status(+u: U.Ui, +id: String) -> String:
  J.Json.get_str(J.Json.get_obj(X.params(u, "tb-tick", id), "item"), "status", "")

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = X.ui()
    +opened : U.Ui = X.act(u, "tb-open", "")
    +all : U.Ui = X.act(u, "tb-f", "all")
    +owner : U.Ui = X.act(u, "tb-who", "b1")
    C.Check.str("sections in status order", titles(U.BoardUi.sections(u)), "Doing;Blocked;To do;")
    C.Check.str("all includes done and dropped", titles(U.BoardUi.sections(all)), "Doing;Blocked;To do;Done;Dropped;")
    C.Check.str("open hides done", String.join(U.BoardUi.sections.ids(U.BoardUi.sections(U.BoardUi.demo())), ","), "doing,todo")
    C.Check.str("owner filter", String.join(U.BoardUi.sections.ids(U.BoardUi.sections(owner)), ","), "owned")
    C.Check.str("project filter", String.join(U.BoardUi.sections.ids(U.BoardUi.sections(X.act(u, "tb-proj", "p1"))), ","), "owned")
    C.Check.str("group filter", String.join(U.BoardUi.sections.ids(U.BoardUi.sections(X.act(u, "tb-group", "Rev C"))), ","), "owned")
    C.Check.str("tick todo", status(u, "todo"), "doing")
    C.Check.str("tick doing", status(u, "doing"), "done")
    C.Check.str("tick done", status(u, "done"), "todo")
    C.Check.str("arbitrary status", J.Json.get_str(J.Json.get_obj(X.params(u, "tb-status", "todo\tblocked"), "item"), "status", ""), "blocked")
    C.Check.str("board opens, clears main selections", U.Ui.scratch(opened, "@board") ++ U.Ui.sel(opened) ++ U.Ui.scratch(opened, "@bot") ++ U.Ui.scratch(opened, "@room"), "1")
    C.Check.str("phone board JSON", J.Json.get_str(MS.Mob.board(opened), "title", ""), "Board")
    C.Check.str("phone closed board", J.Json.show(MS.Mob.board(u)), "null")
    C.Check.str("detail fields populated", U.Bots.field(X.act(u, "tb-row", "owned"), "tbnotes"), "Check the drawing")
    C.Check.str("first delete sends nothing", X.method(u, "tb-del", "owned"), "")
    C.Check.str("second delete sends", X.method(X.act(u, "tb-del", "owned"), "tb-del", "owned"), "board.drop")
    +adding : U.Ui = U.Ui.scratch_set(U.Ui.scratch_set(owner, "@tb.group", "Rev C"), "@f.tbtitle", "Check it")
    C.Check.str("add owner filter", J.Json.get_str(J.Json.get_obj(X.params(adding, "tb-add", ""), "item"), "owner", ""), "b1")
    C.Check.str("add clears field", U.Bots.field(X.act(adding, "tb-add", ""), "tbtitle"), "")
    C.Check.str("overdue", Bool.pick(String, U.BoardUi.overdue("2026-10-12", 1791936000n), "yes", "no"), "yes")
    C.Check.str("due within two days", Bool.pick(String, U.BoardUi.soon("2026-10-15", 1791936000n), "yes", "no"), "yes")
    C.Check.str("qualified row id", J.Json.get_str(HS.Hubs.board.walk(J.Json.obj([J.KV{"id", J.Str{"owned"}}]), "desk:3799"), "id", ""), "desk:3799|owned")
    C.Check.str("qualified action keeps group literal", J.Json.get_str(HS.Hubs.board.walk(J.Json.obj([J.KV{"action", J.Str{"tb-group"}}, J.KV{"value", J.Str{"Rev C"}}]), "desk:3799"), "value", ""), "desk:3799|Rev C")
