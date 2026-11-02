import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/core/model.bend as M
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/mobile/screen.bend as MB

def state(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def status(+installed: Bool) -> J.Json:
  J.Json.obj([J.KV{"runtime", J.Flag{installed}}, J.KV{"vad", J.Flag{installed}}, J.KV{"supported", J.Flag{True{}}}, J.KV{"models", J.Json.arr([
    J.Json.obj([J.KV{"id", J.Str{"parakeet-v2"}}, J.KV{"label", J.Str{"Parakeet v2"}}, J.KV{"size", J.Num{"482468385"}}, J.KV{"installed", J.Flag{installed}}])])}])

def init() -> U.Ui:
  U.Voice.reply(J.Json.obj([J.KV{"local", status(False{})}]), U.Ui.init(0n))

def busy() -> U.Ui:
  state(U.Voice.local(init(), "install:parakeet-v2"))

def progress() -> J.Json:
  J.Json.obj([J.KV{"ev", J.Str{"local-progress"}}, J.KV{"what", J.Str{"Parakeet v2"}}, J.KV{"done", J.Num{"241000000"}}, J.KV{"total", J.Num{"482000000"}}])

def text(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.Txt{s} <> t: s ++ text(t)
    case D.El{_, _, _, kids} <> t: text(kids) ++ text(t)

def main() -> IO(Unit):
  do IO<Unit>:
    +b : U.Ui = busy()
    +p : U.Ui = U.Voice.recv(b, progress())
    +race : U.Ui = U.Voice.reply(J.Json.obj([J.KV{"local", status(False{})}]), p)
    +failed : U.Ui = U.Voice.recv(p, J.Json.obj([J.KV{"ev", J.Str{"local-result"}}, J.KV{"local", status(False{})}, J.KV{"error", J.Str{"network offline"}}]))
    +done : U.Ui = U.Voice.recv(p, J.Json.obj([J.KV{"ev", J.Str{"local-result"}}, J.KV{"local", status(True{})}]))
    C.Check.str("start is immediate", U.Vlocal.note(I.En{}, b), "Starting download… You can leave Settings while it runs.")
    C.Check.str("progress before result", U.Vlocal.note(I.En{}, p), "Downloading Parakeet v2 · 241 / 482 MB (50%)")
    C.Check.str("status reply cannot clear download", U.Ui.scratch(race, "@voice.busy"), "install:parakeet-v2")
    C.Check.str("duplicate download ignored", U.Ui.scratch(state(U.Voice.local(p, "install:parakeet-v3")), "@voice.busy"), "install:parakeet-v2")
    C.Check.str("failure clears busy", U.Ui.scratch(failed, "@voice.busy"), "")
    C.Check.str("failure is visible", U.Vlocal.note(I.En{}, failed), "Download failed: network offline")
    C.Check.str("status reply cannot erase failure", U.Vlocal.note(I.En{}, U.Voice.reply(J.Json.obj([J.KV{"local", status(False{})}]), failed)), "Download failed: network offline")
    C.Check.str("retry clears failure", U.Vlocal.error(state(U.Voice.local(failed, "install:parakeet-v2"))), "")
    C.Check.str("completion clears busy", U.Ui.scratch(done, "@voice.busy"), "")
    C.Check.str("completion shows ready", U.Vlocal.note(I.En{}, done), "Parakeet v2 is ready.")
    C.Check.str("missing metadata is not zero MB", U.Vlocal.note(I.En{}, U.Ui.init(0n)), "Checking the voice helper on this machine…")
    C.Check.str("unpack stage", U.Vlocal.progress.k("unpack", I.En{}, J.Json.obj([J.KV{"what", J.Str{"Parakeet v2"}}])), "Verifying and unpacking Parakeet v2…")
    C.Check.str("unknown total", U.Vlocal.progress.k("download", I.En{}, J.Json.obj([J.KV{"what", J.Str{"runtime"}}, J.KV{"done", J.Num{"5000000"}}])), "Downloading runtime · 5 MB received")
    C.Check.str("progress does not stop microphone", U.Ui.scratch(U.Voice.recv(U.Ui.scratch_set(b, "@voice", "live"), progress()), "@voice"), "live")
    C.Check.str("ready needs runtime and VAD", Bool.show(U.Vlocal.ready(J.Null{}, J.Json.obj([J.KV{"installed", J.Flag{True{}}}]))), "False")
    C.Check.str("info kind", U.Settings.row.kind(False{}), "info")
    C.Check.str("control kind", U.Settings.row.kind(True{}), "control")
    C.Check.str("web empty has next step", Bool.show(String.contains(text(V.View.agy.rows(init(), U.Vlearned.rows(I.En{}, U.Ui.state.of(init())))), "No fixes learned yet.")), "True")
    C.Check.str("phone model and progress", Bool.show(String.contains(J.Json.show(J.Json.arr(MB.Mob.agy.rows(U.Vlocal.rows(I.En{}, p)))), "241 / 482 MB (50%)")), "True")
    C.Check.str("phone empty has next step", Bool.show(String.contains(J.Json.show(J.Json.arr(MB.Mob.agy.rows(U.Vlearned.rows(I.En{}, U.Ui.state.of(init()))))), "No fixes learned yet.")), "True")
