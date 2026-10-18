import Base
import ./check.bend as C
import ../src/core/kihealth.bend as K
import ../src/core/prompt.bend as P
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/hub.bend as H
import ../src/core/codec.bend as CD

# The static model, including legacy/v2 identity and the client glue.
def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def key(n: Maybe<&2, K.KNote>) -> String:
  match n:
    case Some{K.KNote{k, _, _, _}}: k
    case None{}: "none"

def title(n: Maybe<&2, K.KNote>) -> String:
  match n:
    case Some{K.KNote{_, t, _, _}}: t
    case None{}: "none"

def body(n: Maybe<&2, K.KNote>) -> String:
  match n:
    case Some{K.KNote{_, _, b, _}}: b
    case None{}: "none"

def acts.go(as: List<&2, K.KAct>) -> String:
  match as:
    case Nil{}: ""
    case K.KAct{l, a, v} <> t: l ++ "/" ++ a ++ "/" ++ v ++ "|" ++ acts.go(t)

def acts(n: Maybe<&2, K.KNote>) -> String:
  match n:
    case Some{K.KNote{_, _, _, a}}: acts.go(a)
    case None{}: "none"

def note(+s: String, can: Bool) -> Maybe<&2, K.KNote>:
  K.KH.notice(s, "/c", "10.0.6", "  exec failed  \nsecond line", can)

def all(ss: List<&2, String>) -> IO(Unit):
  match ss:
    case Nil{}: IO.pure(Unit, Unit{})
    case +s <> t:
      do IO<Unit>:
        C.Check.str(s ++ " key", key(note(s, True{})), s ++ ":/c:10.0.6")
        C.Check.str(s ++ " ends in retry", yes(K.KH.retry(note(s, True{}))), "yes")
        C.Check.str(s ++ " no install when unsupported", yes(K.KH.has_install(note(s, False{}))), "no")
        C.Check.str(s ++ " install when supported", yes(K.KH.has_install(note(s, True{}))), "yes")
        C.Check.str(s ++ " download actions", acts(note(s, False{})), "Download/link/https://github.com/i2cjak/Backplane_KiCad/releases|Set path/kicad-path/|Retry/kicad-retry/|")
        C.Check.str(s ++ " prompt no IPC launch", yes(String.contains(P.Prompt.kicad.health(s, "/c", "10.0.6", "commit", "failed", True{}), "api-server")), "no")
        all(t)

def manifest(m: K.KManifest) -> String:
  match m:
    case K.KManifest{f, v, c, a, caps, up, kv}:
      yes(f) ++ "/" ++ v ++ "/" ++ c ++ "/" ++ Nat.show(a) ++ "/" ++ String.join(caps, ",") ++ "/" ++ up ++ "/" ++ kv

def info(u: U.Ui) -> Map<&2, String>:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, nf, _, _}: nf

def err(u: U.Ui) -> String:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, e, _, _, _, _}: e

def client(+v: String) -> U.Ui:
  U.Ui.recv(U.Ui.init(1n), J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj([
    J.KV{"kicad.status", J.Str{"stock"}}, J.KV{"kicad.cli", J.Str{"/c"}}, J.KV{"kicad.version", J.Str{v}}, J.KV{"kicad.can_install", J.Str{"1"}}])}]))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("missing beats run/fork/version", K.KH.status(False{}, True{}, True{}, "99.0.0", 99n), "missing")
    C.Check.str("broken beats stock/age", K.KH.status(True{}, False{}, False{}, "0", 0n), "broken")
    C.Check.str("stock beats age", K.KH.status(True{}, True{}, False{}, "0", 0n), "stock")
    C.Check.str("fork too old", K.KH.status(True{}, True{}, True{}, "10.0.5", 9n), "old")
    C.Check.str("10.0.6 is old now", K.KH.status(True{}, True{}, True{}, "10.0.6", 1n), "old")
    C.Check.str("right version, API 0", K.KH.status(True{}, True{}, True{}, "10.99.0-bp.1", 0n), "old")
    C.Check.str("fork minimum", K.KH.status(True{}, True{}, True{}, "10.99.0", 1n), "ok")
    C.Check.str("fork suffix", K.KH.status(True{}, True{}, True{}, "10.99.0-bp.1", 1n), "ok")
    C.Check.str("future nightly", K.KH.status(True{}, True{}, True{}, "10.99.0", 1n), "ok")
    C.Check.str("older major", K.KH.status(True{}, True{}, True{}, "9.99.99", 1n), "old")
    C.Check.str("no notice ok", key(note("ok", True{})), "none")
    C.Check.str("no notice checking", key(note("checking", True{})), "none")
    all(["missing", "stock", "old", "broken"])
    C.Check.str("missing title", title(note("missing", True{})), "KiCad isn't installed")
    C.Check.str("missing body", body(note("missing", True{})), "Backplane KiCad runs ERC, DRC, exports and agent edits. Install it, or point Backplane at a copy you have.")
    C.Check.str("stock title", title(note("stock", True{})), "This is KiCad 10.0.6, not Backplane KiCad")
    C.Check.str("stock body", body(note("stock", True{})), "Exports and checks work. Agents can't drive KiCad over IPC until Backplane KiCad is installed.")
    C.Check.str("old title", title(note("old", True{})), "Backplane KiCad 10.0.6 is too old")
    C.Check.str("old body", body(note("old", True{})), "Backplane needs 10.99.0 or newer (API 1).")
    C.Check.str("broken title", title(note("broken", True{})), "KiCad at /c doesn't run")
    C.Check.str("broken first line trimmed", body(note("broken", True{})), "exec failed")
    C.Check.str("broken output limited", Nat.show(String.length(K.KH.why(String.repeat("x", 200n)))), "160")
    C.Check.str("update action", acts(note("old", True{})), "Update/kicad-install/|Set path/kicad-path/|Retry/kicad-retry/|")
    C.Check.str("install action", acts(note("missing", True{})), "Install/kicad-install/|Set path/kicad-path/|Retry/kicad-retry/|")
    C.Check.str("version changes key", yes(String.eq(key(note("stock", True{})), key(K.KH.notice("stock", "/c", "10.0.7", "", True{})))), "no")
    +st : M.State = M.State.replay([M.SettingSet{"kicad.notice.hidden", "stock:/c:10.0.6", 1n}], M.State.empty())
    C.Check.str("dismissed same key", yes(K.KH.dismissed(st, "stock:/c:10.0.6")), "yes")
    C.Check.str("new key visible", yes(K.KH.dismissed(st, "stock:/c:10.0.7")), "no")
    C.Check.str("ok prompt names kicad_ipc", yes(String.contains(P.Prompt.kicad.health("ok", "/c", "10.99.0-bp.1", "rev", "", True{}), "`kicad_ipc`")), "yes")
    C.Check.str("ok prompt starts no server", yes(String.contains(P.Prompt.kicad.health("ok", "/c", "10.99.0-bp.1", "rev", "", True{}), "api-server")), "no")
    C.Check.str("ok prompt no pcbnew import", yes(String.contains(P.Prompt.kicad.health("ok", "/c", "10.99.0-bp.1", "rev", "", True{}), "import pcbnew")), "no")
    C.Check.str("ok prompt guide at the commit", yes(String.contains(P.Prompt.kicad.health("ok", "/c", "10.99.0-bp.1", "rev", "", True{}), "/blob/rev/BACKPLANE_IPC.md")), "yes")
    C.Check.str("off prompt names the fallback", yes(String.contains(P.Prompt.kicad.health("ok", "/c", "10.99.0-bp.1", "rev", "", False{}), "answer with the fallback")), "yes")
    C.Check.str("old prompt names the fallback", yes(String.contains(P.Prompt.kicad.health("old", "/c", "10.0.6", "rev", "", True{}), "answer with the fallback")), "yes")
    C.Check.str("checking prompt", yes(String.contains(P.Prompt.kicad.health("checking", "/c", "", "", "", True{}), "still being checked")), "yes")
    C.Check.str("no obsolete prompt", yes(String.contains(P.Prompt.kicad("/c", False{}, "10.0.6", "", True{}), "No matching Backplane")), "no")
    C.Check.str("v1 manifest", manifest(K.KH.manifest("{\"sourceRepository\":\"https://github.com/i2cjak/Backplane_KiCad\",\"version\":\"10.0.6\",\"sourceCommit\":\"abc\"}")), "yes/10.0.6/abc/0///10.0.6")
    C.Check.str("v2 manifest", manifest(K.KH.manifest("{\"sourceRepository\":\"Backplane_KiCad\",\"version\":\"10.99.0-bp.2\",\"sourceCommit\":\"abc\",\"kicadVersion\":\"10.99.0\",\"apiVersion\":1,\"upstreamCommit\":\"up\",\"capabilities\":[\"api-server\",\"headless-pcb\"]}")), "yes/10.99.0-bp.2/abc/1/api-server,headless-pcb/up/10.99.0")
    C.Check.str("bad manifest", manifest(K.KH.manifest("bad")), "no///0///")
    C.Check.str("new action appended", yes(String.eq(Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, U.Ui.actions(), 183n)), ""), "kicad-retry")), "yes")
    +shown : U.Ui = U.Ui.recv(client("10.0.6"), J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"ok", J.Flag{False{}}},
      J.KV{"result", J.Json.obj([J.KV{"kicad", J.Str{"stock"}}, J.KV{"thread", J.Str{"t1"}}])}]))
    C.Check.str("open reply shows thread notice", key(U.KN.thread(info(shown), U.Ui.state(shown), "t1")), "stock:/c:10.0.6")
    C.Check.str("health reply has no error toast", err(shown), "")
    +hidden : U.Ui = U.Act.ui(U.Ui.act(shown, "setting", "kicad.notice.hidden=stock:/c:10.0.6"))
    C.Check.str("normal setting dismisses thread notice", key(U.KN.thread(info(hidden), U.Ui.state(hidden), "t1")), "none")
    C.Check.str("settings always shows truth", key(U.KN.of(info(hidden), U.Ui.state(hidden))), "stock:/c:10.0.6")
    C.Check.str("new version shows notice again", key(U.KN.visible(info(client("10.0.7")), U.Ui.state(hidden))), "stock:/c:10.0.7")
    C.Check.str("retry method appended", Nat.show(CD.Codec.pos(H.Hub.methods(), "kicad.retry")), "39")
