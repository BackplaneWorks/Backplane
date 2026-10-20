import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/core/msgs.bend as MSG
import ../src/web/view.bend as W
import ../src/web/dom.bend as D
import ../src/app/layout.bend as L
import ../src/gfx/text.bend as X
import ../src/gfx/raster.bend as R
import ../src/mobile/view.bend as MV

# A render can fail after its last model disappeared. Exercise the real
# web/native output and phone screen data, including that empty listing.
def diagnostic() -> String:
  "could not write fixture.iso.png"

def listing() -> String:
  "1 9 mech/fixture.glb"

def act(+u: U.Ui, +action: String, +value: String) -> U.Ui:
  U.Act.ui(U.Ui.act(u, action, value))

def base() -> U.Ui:
  act(U.Ui.fold([M.ProjectCreated{"p", "Fixture", "/p", 1n},
    M.ThreadCreated{"a", "p", "First", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadCreated{"b", "p", "Second", M.Local{}, "", "", "claude", "", 3n}], U.Ui.init(10n)), "select", "a")

def listed(u: U.Ui, +files: String) -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(u, "mech:a", files), "mech.root:a", "/p")

def failed(u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(u, "@mech.err:a", diagnostic())

def quiet(u: U.Ui) -> U.Ui:
  U.Ui.scratch_set(u, "@mech.err:a", "")

def words.go(fuel: Nat, ns: List<&2, D.Node>, acc: String) -> String:
  match fuel ns:
    case 0n _:
      acc
    case 1n+_ Nil{}:
      acc
    case 1n+k D.Txt{s} <> t:
      words.go(k, t, acc ++ s ++ "\n")
    case 1n+k D.El{_, _, _, kids} <> t:
      words.go(k, List.append(&2, D.Node, kids, t), acc)

def web(+u: U.Ui) -> String:
  words.go(1000n, W.View.mech(I.En{}, u), "")

# An uncached face records the code points the actual window draws as
# glyph requests, so these checks need neither fonts nor an X server.
def face(f: X.Face) -> X.Face:
  match f:
    case X.Face{_, _, px, asc, desc, lh, miss, name, tab}:
      X.Face{1, True{}, px, asc, desc, lh, miss, name, tab}

def ty() -> L.Ty:
  +f = face(X.Text.bitmap())
  L.Ty{Map.new(&2, X.TG), f, f, f, f, f, I.En{}}

def drawn.go(os: List<&2, R.Op>, acc: String) -> String:
  match os:
    case Nil{}:
      acc
    case R.Op{4294967295, _, _, _, cp, 0, _} <> t:
      drawn.go(t, SCon{Chr{cp}, acc})
    case _ <> t:
      drawn.go(t, acc)

def drawn(o: L.Out) -> String:
  match o:
    case L.Out{os, _}:
      drawn.go(os, "")

def native(+u: U.Ui) -> String:
  drawn(L.Lay.mech.if(True{}, ty(), u, 0, 1400, 900))

def has(+text: String, +part: String) -> String:
  Bool.show(String.contains(text, part))

def phone(+u: U.Ui) -> J.Json:
  MV.Mech.json(I.En{}, True{}, u)

def reply(+u: U.Ui, +thread: String, +files: String, +err: String) -> U.Ui:
  U.Ui.recv(u, J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"1"}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", J.Json.obj([J.KV{"thread", J.Str{thread}}, J.KV{"root", J.Str{"/p"}}, J.KV{"mech", J.Str{files}},
      J.KV{"rendered", J.Str{"mech/fixture.glb"}}, J.KV{"error", J.Str{err}}])}]))

def main() -> IO(Unit):
  +loading = failed(base())
  +empty = listed(loading, "")
  +parts = listed(loading, listing())
  +message = MSG.T.viewer.render.failed(I.En{}, diagnostic())
  +prefix = MSG.T.viewer.render.failed(I.En{}, "")
  +asked = act(base(), "mech-render", "mech/fixture.glb")
  +answer = reply(asked, "a", "", diagnostic())
  +retry = act(answer, "mech-render", "mech/fixture.glb")
  +recovered = reply(retry, "a", listing(), "")
  +both = U.Ui.scratch_set(asked, "@mech.busy:b", "mech/fixture.glb")
  +other = reply(both, "b", "", diagnostic())
  +selected = act(other, "select", "b")
  do IO<Unit>:
    C.Check.str("fixture still loading", Nat.show(U.Mpage.state(loading)), "0")
    C.Check.str("fixture has no parts", Nat.show(U.Mpage.state(empty)), "1")
    C.Check.str("fixture has a model", Nat.show(U.Mpage.state(parts)), "2")
    C.Check.str("web shows failure while loading", has(web(loading), message), "True")
    C.Check.str("web shows failure without parts", has(web(empty), message), "True")
    C.Check.str("web shows failure with parts", has(web(parts), message), "True")
    C.Check.str("web omits blank failure while loading", has(web(quiet(loading)), prefix), "False")
    C.Check.str("web omits blank failure without parts", has(web(quiet(empty)), prefix), "False")
    C.Check.str("web omits blank failure with parts", has(web(quiet(parts)), prefix), "False")
    C.Check.str("window draws failure while loading", has(native(loading), message), "True")
    C.Check.str("window draws failure without parts", has(native(empty), message), "True")
    C.Check.str("window draws failure with parts", has(native(parts), message), "True")
    C.Check.str("window omits blank failure while loading", has(native(quiet(loading)), prefix), "False")
    C.Check.str("window omits blank failure without parts", has(native(quiet(empty)), prefix), "False")
    C.Check.str("window omits blank failure with parts", has(native(quiet(parts)), prefix), "False")
    C.Check.str("phone retains failure without parts", J.Json.get_str(phone(empty), "err", ""), diagnostic())
    C.Check.str("phone still reports an empty parts list", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(phone(empty), "parts")))), "0")
    C.Check.str("render request is busy", U.Mpage.busy(asked), "mech/fixture.glb")
    C.Check.str("empty failed reply stops rendering", U.Mpage.busy(answer), "")
    C.Check.str("empty failed reply retains the diagnostic", U.Mpage.err(answer), diagnostic())
    C.Check.str("empty failed reply is visible on the web", has(web(answer), message), "True")
    C.Check.str("empty failed reply is visible in the window", has(native(answer), message), "True")
    C.Check.str("retry clears the previous error", U.Mpage.err(retry), "")
    C.Check.str("successful retry stops rendering", U.Mpage.busy(recovered), "")
    C.Check.str("successful retry leaves no diagnostic", U.Mpage.err(recovered), "")
    C.Check.str("successful retry removes visible failure", has(web(recovered), prefix), "False")
    C.Check.str("another thread's reply keeps this request busy", U.Mpage.busy(other), "mech/fixture.glb")
    C.Check.str("another thread's reply does not leak its error", U.Mpage.err(other), "")
    C.Check.str("switching threads shows its diagnostic", U.Mpage.err(selected), diagnostic())
    C.Check.str("the replied-to thread is no longer busy", U.Mpage.busy(selected), "")
