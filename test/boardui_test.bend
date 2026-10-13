import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/tbdata.bend as TT
import ../src/mobile/screen.bend as P

# The Board (client.bend's BoardUi): the sections, the filters, the tick
# cycle, adding, the detail, and the screen the phones draw.

def item(+id: String, +title: String, +status: String, +owner: String, +project: String, +due: String, +group: String, +at: Nat) -> M.Change:
  M.SettingSet{"board." ++ id, TT.Tb.value(TT.TbItem{id, title, status, owner, project, due, group, ["https://example.com/a", "/tmp/notes.md"], "a note", at, at, ""}), at}

# 1970-01-03 is day 200000n div 86400n = 2: b2 is overdue, b8 due within two days
def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "Power board", "/tmp/power", 1n},
    M.ThreadCreated{"t-miso", "", "Miso", M.Local{}, "", "", "claude", "", 1n}, M.BotSet{"b-miso", "Miso", "t-miso", 0n, "", 1n},
    item("b1", "Fix it", "doing", "", "", "", "Errands", 10n), item("b2", "Peach Pass", "todo", "", "", "1970-01-02", "Errands", 11n),
    item("b3", "Call the vendor", "todo", "b-miso", "p1", "", "Work", 12n), item("b4", "Wait for parts", "blocked", "", "p1", "", "Work", 13n),
    item("b5", "Floss", "done", "", "", "", "Errands", 14n), item("b6", "Old idea", "dropped", "", "", "", "", 15n),
    item("b7", "Draft the layout", "doing", "b-miso", "p1", "", "Work", 16n), item("b8", "Pay the bill", "todo", "", "", "1970-01-04", "Errands", 17n)], M.State.empty())

def ui() -> U.Ui:
  U.Ui.far.st(U.Ui.init(200000n), state())

def ui_of(+a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sends(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{s} <> t:
      s ++ sends(t)
    case _ <> t:
      sends(t)

def sent(+a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sends(cs)

def go(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui_of(U.Ui.act(u, a, v))

def say(+u: U.Ui, +field: String, +text: String) -> U.Ui:
  go(u, "bfield", field ++ "\u{1f}" ++ text)

def rows(+rs: List<&2, U.BRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.BRow{+id, _, _, _, _, _, _, _, _, _} <> r:
      id ++ " " ++ rows(r)

def secs(+ss: List<&2, U.BSec>) -> String:
  match ss:
    case Nil{}:
      ""
    case U.BSec{+t, +n, +rs} <> r:
      t ++ " " ++ Nat.show(n) ++ ": " ++ rows(rs) ++ "| " ++ secs(r)

def titles(+ss: List<&2, U.BSec>) -> String:
  match ss:
    case Nil{}:
      ""
    case U.BSec{+t, _, _} <> r:
      t ++ "," ++ titles(r)

def chips(+cs: List<&2, U.BChip>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.BChip{+l, _, _, +on} <> r:
      l ++ Bool.pick(String, on, "*", "") ++ "," ++ chips(r)

def keep(+r: U.BRow, +id: String, +rest: List<&2, U.BRow>) -> List<&2, U.BRow>:
  match r:
    case U.BRow{+i, +a, +b, +c, +d, +e, +f, +g, +h, +j}:
      Bool.pick(List<&2, U.BRow>, String.eq(i, id), U.BRow{i, a, b, c, d, e, f, g, h, j} <> rest, rest)

def found(+rs: List<&2, U.BRow>, +id: String) -> List<&2, U.BRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      keep(r, id, found(t, id))

def find(+ss: List<&2, U.BSec>, +id: String) -> List<&2, U.BRow>:
  match ss:
    case Nil{}:
      Nil{}
    case U.BSec{_, _, +rs} <> r:
      List.append(&2, U.BRow, found(rs, id), find(r, id))

def row(+u: U.Ui, +id: String) -> U.BRow:
  Maybe.default(&2, U.BRow, List.head(&2, U.BRow, find(U.BoardUi.sections(u), id)), U.BRow{"?", "", "", "", False{}, False{}, False{}, "", Nil{}, False{}})

def sub.of(+r: U.BRow) -> String:
  match r:
    case U.BRow{_, _, +s, _, _, _, _, _, _, _}:
      s

def sub(+u: U.Ui, +id: String) -> String:
  sub.of(row(u, id))

def flags.of(+r: U.BRow) -> String:
  match r:
    case U.BRow{_, _, _, _, +soon, +over, +open, _, _, _}:
      Bool.show(soon) ++ "/" ++ Bool.show(over) ++ "/" ++ Bool.show(open)

def flags(+u: U.Ui, +id: String) -> String:
  flags.of(row(u, id))

def main() -> IO(Unit):
  do IO<Unit>:
    # the sections: doing, blocked, to do, and with "all" done and dropped
    C.Check.str("sections: open", secs(U.BoardUi.sections(ui())), "Doing 2: b1 b7 | Blocked 1: b4 | To do 3: b2 b8 b3 | ")
    C.Check.str("sections: all", titles(U.BoardUi.sections(go(ui(), "tb-f", "all"))), "Doing,Blocked,To do,Done,Dropped,")
    C.Check.str("sections: open hides done and dropped", titles(U.BoardUi.sections(go(go(ui(), "tb-f", "all"), "tb-f", "open"))), "Doing,Blocked,To do,")
    C.Check.str("sections: the count", Nat.show(U.BoardUi.count(ui())), "6")
    C.Check.str("sections: the sidebar's entry", U.BoardUi.entry(ui()), "Board · 6 open")
    C.Check.str("sections: none", Nat.show(List.length(&2, U.BSec, U.BoardUi.sections(U.Ui.init(1n)))), "0")
    # a row's words
    C.Check.str("row: sub", sub(ui(), "b2"), "you · due Jan 2 · Errands")
    C.Check.str("row: a bot's, in a project", sub(ui(), "b3"), "Miso · Power board · Work")
    C.Check.str("row: overdue", flags(ui(), "b2"), "False/True/False")
    C.Check.str("row: due soon", flags(ui(), "b8"), "True/False/False")
    C.Check.str("row: no date", flags(ui(), "b1"), "False/False/False")
    C.Check.str("row: done is never overdue", flags(go(ui(), "tb-f", "all"), "b5"), "False/False/False")
    # filters
    C.Check.str("chips", chips(U.BoardUi.chips(ui())), "Open*,All,You,Miso,Power board,Errands,Work,")
    C.Check.str("chips: all", chips(U.BoardUi.chips(go(ui(), "tb-f", "all"))), "Open,All*,You,Miso,Power board,Errands,Work,")
    C.Check.str("filter: yours", secs(U.BoardUi.sections(go(ui(), "tb-who", "person"))), "Doing 1: b1 | Blocked 1: b4 | To do 2: b2 b8 | ")
    C.Check.str("filter: a bot's", secs(U.BoardUi.sections(go(ui(), "tb-who", "b-miso"))), "Doing 1: b7 | To do 1: b3 | ")
    C.Check.str("filter: a project", secs(U.BoardUi.sections(go(ui(), "tb-proj", "p1"))), "Doing 1: b7 | Blocked 1: b4 | To do 1: b3 | ")
    C.Check.str("filter: a group", secs(U.BoardUi.sections(go(ui(), "tb-group", "Errands"))), "Doing 1: b1 | To do 2: b2 b8 | ")
    C.Check.str("filter: two together", secs(U.BoardUi.sections(go(go(ui(), "tb-group", "Work"), "tb-who", "b-miso"))), "Doing 1: b7 | To do 1: b3 | ")
    C.Check.str("filter: the chip on", chips(U.BoardUi.chips(go(ui(), "tb-group", "Work"))), "Open*,All,You,Miso,Power board,Errands,Work*,")
    C.Check.str("filter: again turns it off", secs(U.BoardUi.sections(go(go(ui(), "tb-group", "Work"), "tb-group", "Work"))), secs(U.BoardUi.sections(ui())))
    # the tick cycle
    C.Check.str("tick: to do, doing", sent(U.Ui.act(ui(), "tb-tick", "b2")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"id\":\"b2\",\"status\":\"doing\"}}}")
    C.Check.str("tick: doing, done", sent(U.Ui.act(ui(), "tb-tick", "b1")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"id\":\"b1\",\"status\":\"done\"}}}")
    C.Check.str("tick: done, to do", sent(U.Ui.act(ui(), "tb-tick", "b5")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"id\":\"b5\",\"status\":\"todo\"}}}")
    C.Check.str("tick: blocked goes on", sent(U.Ui.act(ui(), "tb-tick", "b4")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"id\":\"b4\",\"status\":\"doing\"}}}")
    C.Check.str("tick: unknown", sent(U.Ui.act(ui(), "tb-tick", "zz")), "")
    C.Check.str("status: any", sent(U.Ui.act(ui(), "tb-status", "b2\tblocked")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"id\":\"b2\",\"status\":\"blocked\"}}}")
    C.Check.str("status: drop", sent(U.Ui.act(ui(), "tb-status", "b2\tdropped")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"id\":\"b2\",\"status\":\"dropped\"}}}")
    # adding takes what the filters name
    C.Check.str("add: a title", sent(U.Ui.act(say(ui(), "tbtitle", " Buy milk "), "tb-add", "")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"title\":\"Buy milk\"}}}")
    C.Check.str("add: clears the field", U.Ui.scratch(go(say(ui(), "tbtitle", "Buy milk"), "tb-add", ""), "@f.tbtitle"), "")
    C.Check.str("add: nothing to add", sent(U.Ui.act(say(ui(), "tbtitle", "  "), "tb-add", "")), "")
    C.Check.str("add: under the filters", sent(U.Ui.act(say(go(go(go(ui(), "tb-who", "b-miso"), "tb-proj", "p1"), "tb-group", "Work"), "tbtitle", "Buy milk"), "tb-add", "")),
      "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"title\":\"Buy milk\",\"owner\":\"b-miso\",\"project\":\"p1\",\"group\":\"Work\"}}}")
    C.Check.str("add: yours", sent(U.Ui.act(say(go(ui(), "tb-who", "person"), "tbtitle", "x"), "tb-add", "")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"title\":\"x\",\"owner\":\"person\"}}}")
    # a row's detail
    +r : U.Ui = go(ui(), "tb-row", "b2")
    C.Check.str("detail: opens", flags(r, "b2"), "False/True/True")
    C.Check.str("detail: the title to edit", U.Ui.scratch(r, "@f.tbedit"), "Peach Pass")
    C.Check.str("detail: the notes to edit", U.Ui.scratch(r, "@f.tbnotes"), "a note")
    C.Check.str("detail: shuts", flags(go(r, "tb-row", "b2"), "b2"), "False/True/False")
    C.Check.str("detail: another row replaces it", flags(go(r, "tb-row", "b1"), "b2"), "False/True/False")
    C.Check.str("notes: saved", sent(U.Ui.act(say(r, "tbnotes", " call first "), "tb-notes", "b2")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"id\":\"b2\",\"notes\":\"call first\"}}}")
    C.Check.str("title: renamed", sent(U.Ui.act(say(r, "tbedit", "Peach Pass renewal"), "tb-title", "b2")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"id\":\"b2\",\"title\":\"Peach Pass renewal\"}}}")
    C.Check.str("title: Enter renames the open row", sent(U.Ui.act(say(r, "tbedit", "Renewal"), "tb-rename", "")), "{\"id\":0,\"m\":\"board.set\",\"p\":{\"item\":{\"id\":\"b2\",\"title\":\"Renewal\"}}}")
    C.Check.str("title: not empty", sent(U.Ui.act(say(r, "tbedit", " "), "tb-title", "b2")), "")
    # Delete asks twice
    +d1 : U.Act = U.Ui.act(r, "tb-del", "b2")
    C.Check.str("delete: the first click", sent(d1), "")
    C.Check.str("delete: armed", Bool.show(Bool.and(String.eq(flags(ui_of(d1), "b2"), "False/True/True"), String.contains(J.Json.show(P.Mob.board(go(ui_of(d1), "tb-open", ""))), "Delete?"))), "True")
    C.Check.str("delete: the second click", sent(U.Ui.act(ui_of(d1), "tb-del", "b2")), "{\"id\":0,\"m\":\"board.drop\",\"p\":{\"id\":\"b2\"}}")
    # opening: the board replaces a bot's view
    +b : U.Ui = go(U.Ui.scratch_set(ui(), "@bot", "b-miso"), "tb-open", "")
    C.Check.str("open: on", Bool.show(U.BoardUi.is(b)), "True")
    C.Check.str("open: the bot's view is shut", U.Ui.scratch(b, "@bot"), "")
    C.Check.str("open: a plane row opens its item", flags(go(ui(), "tb-open", "b2"), "b2"), "False/True/True")
    C.Check.str("close", Bool.show(U.BoardUi.is(go(b, "tb-close", ""))), "False")
    C.Check.str("a bot opened shuts it", Bool.show(U.BoardUi.is(go(b, "bot", "b-miso"))), "False")
    C.Check.str("a thread selected shuts it", Bool.show(U.BoardUi.is(go(b, "select", "t-miso"))), "False")
    # the phones' screen
    C.Check.str("phone: no board closed", J.Json.show(P.Mob.board(ui())), "null")
    +j : String = J.Json.show(P.Mob.board(b))
    C.Check.str("phone: sections", Bool.show(String.contains(j, "\"sections\":[{\"title\":\"Doing\",\"count\":2,\"rows\":[{\"id\":\"b1\",\"title\":\"Fix it\",\"sub\":\"you · Errands\",\"status\":\"doing\"")), "True")
    C.Check.str("phone: filters", Bool.show(String.contains(j, "\"filters\":[{\"label\":\"Open\",\"action\":\"tb-f\",\"value\":\"open\",\"on\":true}")), "True")
    C.Check.str("phone: the add field", Bool.show(String.contains(j, "\"add\":{\"name\":\"tbtitle\",\"hint\":\"Add an item\"")), "True")
    C.Check.str("phone: links open like links in messages", Bool.show(String.contains(J.Json.show(P.Mob.board(go(b, "tb-row", "b2"))), "{\"label\":\"https://example.com/a\",\"action\":\"open-url\",\"value\":\"https://example.com/a\"},{\"label\":\"/tmp/notes.md\",\"action\":\"file-open\",\"value\":\"/tmp/notes.md\"}")), "True")
    C.Check.str("phone: an open row has its buttons", Bool.show(String.contains(J.Json.show(P.Mob.board(go(b, "tb-row", "b2"))), "\"buttons\":[{\"label\":\"Blocked\",\"action\":\"tb-status\",\"value\":\"b2\\t")), "True")
