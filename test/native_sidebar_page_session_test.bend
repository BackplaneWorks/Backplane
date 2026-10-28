import Base
import ./check.bend as C
import ./native_sidebar_projection_test.bend as F
import ./native_sidebar_fixture_dump.bend as D
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/presentation.bend as P
import ../src/server/thin-view.bend as V
import ../src/server/thin-actor.bend as A

def polluted.raw(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, ver, rid, want, _, note, nf, ob, stores}:
      U.Ui{st, sel, dr, live, op, on, now, ad, pa, ver, rid, want, "unrelated ordinary read failure", note, nf, Map.set(&2, String, ob, "ordinary-read", "fab.look"), stores}

def polluted(u: U.Ui) -> U.Ui:
  polluted.raw(U.Ui.scratch_set(u, "@fab.at", "ordinary-read-internal-slot"))

def request() -> J.Json:
  +p = D.request(F.open())
  +session = V.Obj.put(V.Obj.put(V.Obj.put(J.Json.get_obj(p, "session"), "drafts", J.Json.obj([J.KV{"t1", J.Str{"Unsent 日本語 IME"}}])), "info", J.Json.obj(Nil{})), "outbox", J.Json.obj(Nil{}))
  V.Obj.put(V.Obj.put(V.Obj.put(p, "session", session), "action", J.Str{"native-sidebar-page"}), "value", J.Str{"h|" ++ J.Json.get_str(J.Json.get_obj(J.Json.get_obj(F.project(F.open()), "settledPage"), "next"), "value", "")})

def reply() -> A.Rendered:
  D.answer(polluted(F.open()), request())

def shown_session() -> J.Json:
  J.Json.get_obj(A.Rendered.result(reply()), "session")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("readonly page never publishes unrelated pending outbox", J.Json.show(J.Json.get_obj(shown_session(), "outbox")), "{}")
    C.Check.str("readonly page never introduces unrelated private fab scratch", Bool.show(Maybe.is_some(&2, J.Json, J.Json.get(J.Json.get_obj(shown_session(), "drafts"), "@fab.at"))), "False")
    C.Check.str("readonly page retains pending outbox privately", J.Json.get_str(J.Json.get_obj(P.Session.pack(A.Actor.ui(D.actor(reply()))), "outbox"), "ordinary-read", "missing"), "fab.look")
    C.Check.str("readonly page retains internal fab scratch privately", U.Ui.scratch(A.Actor.ui(D.actor(reply())), "@fab.at"), "ordinary-read-internal-slot")
    C.Check.str("readonly page retains exact current draft", J.Json.get_str(J.Json.get_obj(shown_session(), "drafts"), "t1", "missing"), "Unsent 日本語 IME")
    C.Check.str("readonly page preserves selected destination", J.Json.get_str(shown_session(), "sel", "missing"), "t1")
    C.Check.str("readonly page has no commands", J.Json.show(J.Json.get_obj(A.Rendered.result(reply()), "cmds")), "[]")
