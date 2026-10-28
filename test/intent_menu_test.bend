import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/mobile/screen.bend as MB
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as WP
import ../src/core/order.bend as OR

# A project's intent is no longer on show above every thread: its menu's
# Intent… opens it (client.bend's Intent.*), on the web over the page
# (view.bend's View.intent), on phones as the thread's Goal row.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p", "Lamp", "/lamp", 1n},
    M.ThreadCreated{"current", "p", "Current thread", M.Local{}, "", "/lamp", "codex", "", 1n},
    M.ThreadCreated{"other", "p", "Other thread", M.Local{}, "", "/lamp", "codex", "", 1n},
    M.ProjectCreated{"q", "Amp", "/amp", 1n},
    M.ThreadCreated{"amp", "q", "Amp thread", M.Local{}, "", "/amp", "codex", "", 1n}]

def base() -> U.Ui:
  U.Ui.select.k(U.Ui.fold(log(), U.Ui.init(2n)), "current")

def t_ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def opened(+u: U.Ui, +pid: String) -> U.Ui:
  t_ui(U.Ui.act(u, "proj-intent", pid))

# what an action sends the hub and asks the page to do, in order
def cmds(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{t} <> rest: "send " ++ t ++ ";" ++ cmds(rest)
    case U.Focus{id} <> rest: "focus " ++ id ++ ";" ++ cmds(rest)
    case U.Later{_, a, v} <> rest: "later " ++ a ++ " " ++ v ++ ";" ++ cmds(rest)
    case _ <> rest: "other;" ++ cmds(rest)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}: cmds(cs)

# the hub's card for the selected thread
def card(+u: U.Ui, +intent: String) -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(u, "fab.line:" ++ U.Ui.sel(u), "A lamp"), "fab.intent:" ++ U.Ui.sel(u), intent)

def items(xs: List<&2, U.MItem>) -> String:
  match xs:
    case Nil{}: ""
    case U.MItem{l, a, v, _} <> t: l ++ "=" ++ a ++ ":" ++ v ++ ";" ++ items(t)

def nodes(ns: List<&2, D.Node>) -> String:
  Nat.show(List.length(&2, D.Node, ns))

def text(ns: List<&2, D.Node>) -> String:
  J.Json.show(WP.Node.write(D.El{"div", "test", Nil{}, ns}))

# the composer holding Edit's intent, changed to a goal (the rid moved on
# past the look Intent… asked for)
def edited() -> U.Ui:
  t_ui(U.Ui.act(t_ui(U.Ui.act(card(opened(base(), "p"), ""), "fab-edit", "current")), "draft", "/intent\n# Goal\nA lamp"))

def saving() -> U.Ui:
  t_ui(U.Ui.act(edited(), "send", ""))

# the Save's request id
def sid() -> String:
  U.Ui.scratch(saving(), "@intent.saving")

# the hub's card in answer to request id
def answered(+u: U.Ui, +id: String) -> U.Ui:
  U.Ui.recv(u, Maybe.default(&2, J.Json, J.Json.parse("{\"t\":\"reply\",\"id\":" ++ id ++ ",\"ok\":true,\"result\":{\"thread\":\"current\",\"goal\":\"A lamp\",\"intent\":\"# Goal\\nA lamp\"}}"), J.Null{}))

def queue(+u: U.Ui) -> String:
  J.Json.show(J.Json.get_obj(J.Json.get_obj(MB.Mob.screen(u), "thread"), "queue"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the project menu offers Intent…", Bool.show(String.starts_with(items(U.Menu.project(I.En{}, "p", False{}, True{})), "New thread=new-thread:p;Intent…=proj-intent:p;Colour and icon=look:p;Remove project=proj-remove:p;")), "True")
    C.Check.str("phones' project menu offers it too", Bool.show(String.starts_with(items(U.Menu.project(I.En{}, "p", False{}, False{})), "New thread=new-thread:p;Intent…=proj-intent:p;Remove project=proj-remove:p;")), "True")
    C.Check.str("nothing on show at first", Bool.show(U.Intent.on(base())), "False")
    C.Check.str("Intent… on its own project: on show", Bool.show(U.Intent.on(opened(base(), "p"))), "True")
    C.Check.str("Intent… on its own project: the same thread", U.Ui.sel(opened(base(), "p")), "current")
    C.Check.str("Intent… asks the hub for it, once", U.Ui.scratch(opened(base(), "p"), "@fab.at"), U.Fab.at(opened(base(), "p")))
    C.Check.str("Intent… on another project opens its thread", U.Ui.sel(opened(base(), "q")), "amp")
    C.Check.str("Intent… on another project: on show there", Bool.show(U.Intent.on(opened(base(), "q"))), "True")
    C.Check.str("another project: asked for with the move, focused, nothing later", sent(U.Ui.act(base(), "proj-intent", "q")),
      "send {\"id\":0,\"m\":\"fab.look\",\"p\":{\"thread\":\"amp\"}};focus intent;")
    C.Check.str("another project: asked once", U.Ui.scratch(opened(base(), "q"), "@fab.at"), U.Fab.at(opened(base(), "q")))
    C.Check.str("Intent… from Settings leaves Settings", Bool.show(U.Ui.flag(opened(U.Ui.flag_on(base(), "settings"), "q"), "settings")), "False")
    C.Check.str("Intent… shuts the menu", U.Ui.scratch(opened(t_ui(U.Ui.act(base(), "proj-menu", "q")), "q"), "@menu"), "")
    C.Check.str("Intent… on its own project shuts the menu", U.Ui.scratch(opened(t_ui(U.Ui.act(base(), "proj-menu", "p")), "p"), "@menu"), "")
    C.Check.str("Intent… brings the thread's own viewer", Bool.show(U.Ui.panel(opened(U.Ui.view_put(base(), "board", ""), "q"))), "False")
    C.Check.str("Intent… on a project with no thread: nothing", Bool.show(U.Intent.on(opened(base(), "nothing"))), "False")
    C.Check.str("Close puts it away", Bool.show(U.Intent.on(t_ui(U.Ui.act(opened(base(), "p"), "intent-close", "")))), "False")
    C.Check.str("another thread puts it away", Bool.show(U.Intent.on(t_ui(U.Ui.act(opened(base(), "p"), "select", "other")))), "False")
    C.Check.str("loading until the hub answers", Bool.show(U.Intent.got(opened(base(), "p"))), "False")
    C.Check.str("its text once it answers", U.Intent.text(card(opened(base(), "p"), "# Goal\nA lamp")), "# Goal\nA lamp")
    C.Check.str("Edit: the intent goes in the composer", U.Ui.draft_text(t_ui(U.Ui.act(card(opened(base(), "p"), "# Goal\nA lamp"), "fab-edit", "current"))), "/intent\n# Goal\nA lamp")
    C.Check.str("Edit: the intent is put away", Bool.show(U.Intent.on(t_ui(U.Ui.act(card(opened(base(), "p"), "# Goal\nA lamp"), "fab-edit", "current")))), "False")
    C.Check.str("Edit with none yet: the template", U.Ui.draft_text(t_ui(U.Ui.act(card(opened(base(), "p"), ""), "fab-edit", "current"))), "/intent\n# Goal\n\n# Parts\n\n# Notes\n")
    C.Check.str("web: no dialog until asked", nodes(V.View.intent(I.En{}, card(base(), "# Goal\nA lamp"))), "0")
    C.Check.str("web: the dialog and its shade", nodes(V.View.intent(I.En{}, card(opened(base(), "p"), "# Goal\nA lamp"))), "2")
    C.Check.str("web: the card is out of the timeline", Bool.show(String.contains(text([V.View.thread(I.En{}, card(base(), "# Goal\nA lamp"), U.Ui.thread(U.Ui.state(base()), "current"))]), "fab-card")), "False")
    C.Check.str("web: it says loading first", Bool.show(String.contains(text(V.View.intent(I.En{}, opened(base(), "p"))), "Loading the intent")), "True")
    C.Check.str("web: none yet says so", Bool.show(String.contains(text(V.View.intent(I.En{}, card(opened(base(), "p"), ""))), "No intent yet")), "True")
    C.Check.str("web: the project's name in its head", Bool.show(String.contains(text(V.View.intent(I.En{}, card(opened(base(), "p"), "x"))), "Intent · Lamp")), "True")
    C.Check.str("phone: no Goal row until asked", Bool.show(String.contains(queue(card(base(), "x")), "fab:current")), "False")
    C.Check.str("phone: the Goal row when asked", Bool.show(String.contains(queue(card(opened(base(), "p"), "x")), "fab:current")), "True")
    C.Check.str("phone: its × puts it away", Bool.show(String.contains(queue(card(opened(base(), "p"), "x")), "intent-close")), "True")
    # what the dialog shows: the intent less its empty headings
    C.Check.str("shown: a goal only, its empty Parts and Notes go", OR.Intent.shown("# Goal\nA lamp\n\n# Parts\n\n# Notes\n"), "# Goal\nA lamp")
    C.Check.str("shown: the bare template is nothing", OR.Intent.shown("# Goal\n\n# Parts\n\n# Notes\n"), "")
    C.Check.str("shown: a section kept for its subsection's text", OR.Intent.shown("# Goal\nA\n\n# Parts\n## Lamp\n- bulb\n\n# Notes"), "# Goal\nA\n\n# Parts\n## Lamp\n- bulb")
    C.Check.str("shown: an empty subsection goes, its section with it", OR.Intent.shown("# Parts\n## Empty\n# Notes\nx"), "# Notes\nx")
    C.Check.str("shown: text with no headings stays", OR.Intent.shown("A lamp\n#hashtag"), "A lamp\n#hashtag")
    C.Check.str("the dialog's text has no empty headings", U.Intent.text(card(opened(base(), "p"), "# Goal\nA lamp\n\n# Parts\n\n# Notes\n")), "# Goal\nA lamp")
    C.Check.str("web: no empty Parts heading", Bool.show(String.contains(text(V.View.intent(I.En{}, card(opened(base(), "p"), "# Goal\nA lamp\n\n# Parts\n\n# Notes\n"))), "Parts")), "False")
    C.Check.str("web: a saved template says none yet", Bool.show(String.contains(text(V.View.intent(I.En{}, card(opened(base(), "p"), "# Goal\n\n# Parts\n\n# Notes\n"))), "No intent yet")), "True")
    C.Check.str("Edit keeps the file's empty headings to fill in", U.Ui.draft_text(t_ui(U.Ui.act(card(opened(base(), "p"), "# Goal\nA lamp\n\n# Parts\n"), "fab-edit", "current"))), "/intent\n# Goal\nA lamp\n\n# Parts\n")
    # Enter while an intent is edited is a new line; only Save or Ctrl+Enter saves
    C.Check.str("Enter: a popup's row", U.Draft.enter(True{}, "/intent\n# Goal\n"), "skill")
    C.Check.str("Enter: an intent's own (a new line)", U.Draft.enter(False{}, "/intent\n# Goal\n"), "")
    C.Check.str("Enter: anything else sends", U.Draft.enter(False{}, "hello"), "send")
    # Save: the draft goes, the hub is asked, and a while later the note goes
    C.Check.str("Save sends the intent, then asks for the note's end", sent(U.Ui.act(edited(), "send", "")),
      "other;send {\"id\":" ++ sid() ++ ",\"m\":\"fab.intent\",\"p\":{\"thread\":\"current\",\"text\":\"# Goal\\nA lamp\"}};later intent-saved-off " ++ sid() ++ ";")
    C.Check.str("Ctrl+Enter saves too", Bool.show(String.contains(sent(U.Ui.act(edited(), "send-alt", "")), "fab.intent")), "True")
    C.Check.str("Save empties the composer", U.Ui.draft_text(saving()), "")
    C.Check.str("Save waits for its answer", Bool.show(String.is_empty(sid())), "False")
    C.Check.str("no note before the hub answers", Bool.show(U.Intent.saved(saving())), "False")
    C.Check.str("the hub's answer to the Save: Intent saved", Bool.show(U.Intent.saved(answered(saving(), sid()))), "True")
    C.Check.str("another answer is not it", Bool.show(U.Intent.saved(answered(saving(), "999"))), "False")
    C.Check.str("web: the note", Bool.show(String.contains(text(V.View.intent(I.En{}, answered(saving(), sid()))), "Intent saved")), "True")
    C.Check.str("web: no note before", Bool.show(String.contains(text(V.View.intent(I.En{}, saving())), "Intent saved")), "False")
    C.Check.str("its time up, it goes", Bool.show(U.Intent.saved(t_ui(U.Ui.act(answered(saving(), sid()), "intent-saved-off", sid())))), "False")
    C.Check.str("an older Save's time leaves a newer one's", Bool.show(U.Intent.saved(t_ui(U.Ui.act(answered(saving(), sid()), "intent-saved-off", "999")))), "True")
    C.Check.str("a click puts it away", Bool.show(U.Intent.saved(t_ui(U.Ui.act(answered(saving(), sid()), "intent-saved-off", "")))), "False")
