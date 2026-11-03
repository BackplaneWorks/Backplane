import Base
import ./check.bend as C
import ./mailfile_test.bend as F
import ./mailrun_test.bend as R
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/json.bend as J
import ../src/core/mail.bend as ML
import ../src/core/mailbody.bend as MB
import ../src/core/mcp.bend as MC
import ../src/core/auth.bend as A
import ../src/core/http.bend as HTTP
import ../src/core/codec.bend as CD

def reply(os: List<&2, H.Out>) -> J.Json:
  match os:
    case Nil{}: J.Null{}
    case H.Send{_, text} <> _: Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
    case _ <> t: reply(t)

def body(os: List<&2, H.Out>) -> MB.Body:
  Maybe.default(&2, MB.Body, F.first(os), R.body("fyi", "missing"))

def b(+label: String, value: Bool, expected: Bool) -> IO(Unit):
  C.Check.str(label, Bool.show(value), Bool.show(expected))

def n(+label: String, value: Nat, expected: Nat) -> IO(Unit):
  C.Check.str(label, Nat.show(value), Nat.show(expected))

def params(+to: String) -> J.Json:
  J.Json.obj([J.KV{"bot", J.Str{"bb"}}, J.KV{"to", J.Str{to}}, J.KV{"conv", J.Str{"quiet-public"}}, J.KV{"subject", J.Str{"Public quiet reply"}}, J.KV{"text", J.Str{"hé🙂 public reply"}}, J.KV{"want", J.Str{"act"}}, J.KV{"status", J.Str{"open"}}, J.KV{"tags", J.Json.arr([J.Str{"public"}])}, J.KV{"n", J.Num{"50"}}])

def rpc(+s: M.State, +method: String, +p: J.Json) -> H.Step:
  H.Hub.request(s, 10n, 7, J.Json.obj([J.KV{"id", J.Num{"31"}}, J.KV{"m", J.Str{method}}, J.KV{"p", p}]))

def access(+method: String) -> IO(Unit):
  do IO<Unit>:
    +st : H.Step = rpc(F.disabled(), method, params("bot:bb"))
    b(method ++ " OFF human reply ok", J.Json.get_bool(reply(F.outs(st)), "ok", False{}), True{})
    C.Check.str(method ++ " request id retained", J.Json.as_raw(J.Json.get(reply(F.outs(st)), "id"), ""), "31")

# Keep these above their users in the generated file.
def read_effect(os: List<&2, H.Out>) -> Bool:
  match os:
    case Nil{}: False{}
    case H.MailRead{_, _, "quiet-public", "", 50n} <> _: True{}
    case _ <> t: read_effect(t)

def mcp_off(+i: Nat) -> IO(Unit):
  b("OFF MCP " ++ Nat.show(i) ++ " remains denied", MC.Res.good(MC.Mcp.mail.call(i, F.disabled(), 10n, "a", params("bot:bb"))), False{})

def main() -> IO(Unit):
  do IO<Unit>:
    access("mail.list")
    access("mail.find")
    access("mail.mark")
    access("mail.tag")
    access("mail.send")
    +rd : H.Step = rpc(F.disabled(), "mail.read", params("bot:bb"))
    C.Check.str("OFF read effect unchanged", J.Json.show(reply(F.outs(rd))), "null")
    b("OFF read uses existing bounded body effect", read_effect(F.outs(rd)), True{})
    n("OFF read marks only selected bot", List.length(&2, M.Change, H.Queue.changes(rd)), 1n)
    +send : H.Step = rpc(F.disabled(), "mail.send", params("bot:bb"))
    +sent : MB.Body = body(F.outs(send))
    n("OFF bot reply exactly one body append", F.appends(F.outs(send)), 1n)
    n("OFF bot reply no direct FOH post", F.posts(H.Queue.changes(send)), 0n)
    n("OFF bot reply no agent-start output", List.length(&2, H.Out, H.Step.outs_of(H.Mail.route(F.disabled(), 10n, sent))), 0n)
    n("OFF bot reply no RunSet/change", List.length(&2, M.Change, H.Queue.changes(H.Mail.route(F.disabled(), 10n, sent))), 0n)
    C.Check.str("OFF reply exact full Unicode body", MB.Body.text(sent), "hé🙂 public reply")
    C.Check.str("OFF reply person origin", ML.Mail.from(MB.Body.mail(sent)), "person")
    C.Check.str("OFF reply explicit bot target", ML.Mail.to(MB.Body.mail(sent)), "bot:bb")
    C.Check.str("OFF reply opaque conversation", ML.Mail.conv(MB.Body.mail(sent)), "quiet-public")
    +committed : M.State = M.State.replay(F.pending(F.outs(send)), F.disabled())
    +replayed : M.State = M.State.replay(F.pending(F.outs(send)), committed)
    n("same committed header folds once", ML.Conv.n(ML.Box.conv(M.State.mail(replayed), "quiet-public")), 1n)
    +listing : H.Step = rpc(committed, "mail.list", J.Json.obj([J.KV{"bot", J.Str{"bb"}}, J.KV{"n", J.Num{"50"}}]))
    n("OFF list sees committed reply", J.Json.get_nat(J.Json.get_obj(reply(F.outs(listing)), "result"), "total", 0n), 1n)
    +worker : H.Step = rpc(F.disabled(), "mail.send", params("thread:a"))
    n("explicit worker send still delivers ordinary post", F.posts(H.Queue.changes(worker)), 1n)
    n("explicit worker send appends once", F.appends(F.outs(worker)), 1n)
    n("foreign target cannot append", F.appends(F.outs(rpc(F.disabled(), "mail.send", params("thread:foreign")))), 0n)
    n("foreign target cannot post", F.posts(H.Queue.changes(rpc(F.disabled(), "mail.send", params("thread:foreign")))), 0n)
    C.Check.str("ON bot send changes compatible", F.changes(H.Queue.changes(H.Mail.ui.send(F.enabled(), 10n, params("bot:bb")))), F.changes(H.Queue.changes(H.Mail.send(F.enabled(), 10n, "person", params("bot:bb")))))
    C.Check.str("ON bot send body compatible", MB.Body.line(body(H.Step.outs_of(H.Mail.ui.send(F.enabled(), 10n, params("bot:bb"))))), MB.Body.line(body(H.Step.outs_of(H.Mail.send(F.enabled(), 10n, "person", params("bot:bb"))))))
    C.Check.str("ON worker changes compatible", F.changes(H.Queue.changes(H.Mail.ui.send(F.enabled(), 10n, params("thread:a")))), F.changes(H.Queue.changes(H.Mail.send(F.enabled(), 10n, "person", params("thread:a")))))
    mcp_off(34n)
    mcp_off(35n)
    mcp_off(36n)
    mcp_off(37n)
    mcp_off(38n)
    mcp_off(39n)
    mcp_off(40n)
    b("unpaired remote socket denied", A.Auth.allowed(Nil{}, "/ws", "public-fixture-key"), False{})
    b("paired remote socket allowed", A.Auth.allowed(Nil{}, "/ws?token=public-fixture-key", "public-fixture-key"), True{})
    b("foreign origin remains denied", A.Auth.allowed([HTTP.Header{"origin", "https://foreign.invalid"}, HTTP.Header{"host", "127.0.0.1:3787"}], "/ws?token=public-fixture-key", "public-fixture-key"), False{})
    n("OFF tee stays inert", F.appends(F.outs(H.Mail.tee(H.Step.none(), F.disabled(), 10n, ML.Origin{"thread:a", "thread", ""}, "bot:bb", "status", "quiet-event"))), 0n)
    n("OFF due does not wake snoozed mail", List.length(&2, M.Change, H.Mail.due(F.disabled(), 10n)), 0n)
