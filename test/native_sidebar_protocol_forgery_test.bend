import Base
import ./check.bend as C
import ./native_sidebar_projection_test.bend as F
import ./native_sidebar_fixture_dump.bend as D
import ./native_sidebar_page_session_test.bend as B
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/presentation.bend as P
import ../src/server/thin-view.bend as V
import ../src/server/thin-actor.bend as A

def request() -> J.Json:
  +p = B.request()
  +session = J.Json.get_obj(p, "session")
  V.Obj.put(p, "session", V.Obj.put(V.Obj.put(V.Obj.put(session,
    "drafts", J.Json.obj([J.KV{"t1", J.Str{"wrong-caller-draft"}}, J.KV{"@caller-only", J.Str{"injected-draft"}}])),
    "info", J.Json.obj([J.KV{"fold:known", J.Str{"wrong-caller-info"}}, J.KV{"fold:caller-only", J.Str{"injected-info"}}])),
    "outbox", J.Json.obj([J.KV{"ordinary-read", J.Str{"wrong-caller-rpc"}}, J.KV{"caller-only", J.Str{"injected-rpc"}}])))

def reply() -> A.Rendered:
  D.answer(U.Ui.info_set(B.polluted(F.open()), "fold:known", "server-info"), request())

def session() -> J.Json:
  J.Json.get_obj(A.Rendered.result(reply()), "session")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("captured wrong existing draft cannot replace actor value", J.Json.get_str(J.Json.get_obj(session(), "drafts"), "t1", "missing"), "Unsent 日本語 IME")
    C.Check.str("captured wrong existing info cannot replace actor value", J.Json.get_str(J.Json.get_obj(session(), "info"), "fold:known", "missing"), "server-info")
    C.Check.str("captured wrong existing outbox cannot replace actor value", J.Json.get_str(J.Json.get_obj(session(), "outbox"), "ordinary-read", "missing"), "fab.look")
    C.Check.str("caller-only draft never copied to wire", Bool.show(Maybe.is_some(&2, J.Json, J.Json.get(J.Json.get_obj(session(), "drafts"), "@caller-only"))), "False")
    C.Check.str("caller-only info never copied to wire", Bool.show(Maybe.is_some(&2, J.Json, J.Json.get(J.Json.get_obj(session(), "info"), "fold:caller-only"))), "False")
    C.Check.str("caller-only outbox never copied to wire", Bool.show(Maybe.is_some(&2, J.Json, J.Json.get(J.Json.get_obj(session(), "outbox"), "caller-only"))), "False")
    C.Check.str("caller-only data never expands private actor state", Bool.show(String.contains(J.Json.show(P.Session.pack(A.Actor.ui(D.actor(reply())))), "injected-")), "False")
    C.Check.str("forged protocol values never gain commands", J.Json.show(J.Json.get_obj(A.Rendered.result(reply()), "cmds")), "[]")
