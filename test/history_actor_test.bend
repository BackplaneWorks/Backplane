import Base
import ./check.bend as C
import ./hist_test.bend as T
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V

def state() -> M.State:
  M.State.replay([M.ThreadCreated{"t", "", "History", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"other", "", "Other", M.Local{}, "", "", "codex", "", 1n}], T.state())

def old() -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.info_set(U.Ui.info_set(U.Ui.view_put(U.Ui.select.nav(U.Ui.adopt(U.Ui.init(10n), state()), "t"), "board", "/r/b.kicad_pcb"),
    "hist.at:t", "step:c2"), "hist.play:t", "7"), "@cmp.menu", "a"), "@cmp.resolve:t:a", "pending")

def request(+th: String) -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", th)}, J.KV{"cid", J.Str{"B-history"}}])

def refreshed(+th: String, view: String) -> U.Ui:
  A.Actor.ui(A.Actor.refresh([A.Actor.make(old(), "view", V.Target.json("thread", "t"))], True{}, state(), J.Null{}, J.Null{}, 10n, request(th), view))

def main() -> IO(Unit):
  +moved = refreshed("other", "view")
  +same = refreshed("t", "view")
  +replaced = refreshed("t", "replacement")
  do IO<Unit>:
    C.Check.str("thin navigation selects the destination", U.Ui.sel(moved), "other")
    C.Check.str("thin navigation closes the old version picker", U.Ui.scratch(moved, "@cmp.menu"), "")
    C.Check.str("thin navigation cancels the old ref marker", U.Ui.scratch(moved, "@cmp.resolve:t:a"), "")
    C.Check.str("thin navigation pauses the departed history", U.Ui.info(moved, "hist.play:t"), "")
    C.Check.str("thin navigation keeps the chosen snapshot for return", U.Ui.info(moved, "hist.at:t"), "step:c2")
    C.Check.str("ordinary refresh keeps the current picker", U.Ui.scratch(same, "@cmp.menu"), "a")
    C.Check.str("ordinary refresh keeps playback", U.Ui.info(same, "hist.play:t"), "7")
    C.Check.str("a replacement view closes its old picker", U.Ui.scratch(replaced, "@cmp.menu"), "")
    C.Check.str("a replacement view pauses its old playback", U.Ui.info(replaced, "hist.play:t"), "")
