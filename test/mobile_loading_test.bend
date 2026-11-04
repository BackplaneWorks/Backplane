import Base
import ./check.bend as C
import ./mailroom_client_test.bend as MC
import ../src/core/presentation.bend as P
import ../src/core/desktop-browser.bend as DB
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/outbox.bend as O
import ../src/mobile/hubs.bend as H
import ../src/web/mail-actions.bend as MA

# Independent malformed/cross-epoch inputs and a live same-writer backlog.
def parsed(text: String) -> P.Resume:
  P.Session.cursor(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))

def cursor(r: P.Resume) -> String:
  match r:
    case P.Captured{}: "captured"
    case P.Invalid{}: "invalid"
    case P.Cursor{n, origin}: Nat.show(n) ++ ":" ++ origin

def credit() -> O.Cr:
  O.Cr{7, 11n, 3n, 4194304n, 2048n, False{}, True{}, 23n, 4n, 8n, True{}, ["old"]}

def counters(c: O.Cr) -> String:
  match c:
    case O.Cr{g, sent, acked, bytes, backed, _, _, _, _, _, _, _}:
      U32.show(g) ++ ":" ++ Nat.show(sent) ++ ":" ++ Nat.show(acked) ++ ":" ++ Nat.show(bytes) ++ ":" ++ Nat.show(backed)

def mail() -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.online(U.Ui.info_set(MC.ui(), "origin", "current-origin"), True{}), "@mr.conv", "c"), "@mr.to", "thread:new-choice"), "@f.mrreply", "newer 日本語 edit")

def payload() -> J.Json:
  J.Json.obj([J.KV{"recipient", J.Str{"thread:captured"}}, J.KV{"draft", J.Str{"captured 日本語"}}])

# A captured send remains first; sync may append normal list/read operations.
def first(cs: List<&2, U.Cmd>) -> J.Json:
  match cs:
    case U.Send{raw} <> _: Maybe.default(&2, J.Json, J.Json.parse(raw), J.Null{})
    case _: J.Null{}

def mail_sends(cs: List<&2, U.Cmd>) -> Nat:
  match cs:
    case Nil{}: 0n
    case U.Send{raw} <> tail:
      Nat.add(Bool.pick(Nat, String.eq(J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(raw), J.Null{}), "m", ""), "mail.send"), 1n, 0n), mail_sends(tail))
    case _ <> tail: mail_sends(tail)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("ordinary client does not opt in", P.Session.mode("", ""), "")
    C.Check.str("existing mobile opt in retained", P.Session.mode("1", ""), "mobile")
    C.Check.str("explicit frontend opt in", P.Session.mode("", "1"), "frontend")
    C.Check.str("ambiguous hints use explicit frontend contract", P.Session.mode("1", "1"), "frontend")
    C.Check.str("unknown hint is not opt in", P.Session.mode("true", "yes"), "")
    C.Check.str("old mobile history keeps captured cursor", cursor(parsed("{}")), "captured")
    C.Check.str("fresh explicit cursor exact", cursor(parsed("{\"since\":12025,\"origin\":\"epoch\"}")), "12025:epoch")
    C.Check.str("empty cold explicit cursor valid", cursor(parsed("{\"since\":0,\"origin\":\"\"}")), "0:")
    C.Check.str("cursor cannot come from string coercion", cursor(parsed("{\"since\":\"12\",\"origin\":\"e\"}")), "invalid")
    C.Check.str("half cursor refused", cursor(parsed("{\"since\":12}")), "invalid")
    C.Check.str("half origin refused", cursor(parsed("{\"origin\":\"e\"}")), "invalid")
    C.Check.str("negative cursor refused", cursor(parsed("{\"since\":-1,\"origin\":\"e\"}")), "invalid")
    C.Check.str("fractional cursor refused", cursor(parsed("{\"since\":1.5,\"origin\":\"e\"}")), "invalid")
    C.Check.str("exponent cursor refused", cursor(parsed("{\"since\":1e3,\"origin\":\"e\"}")), "invalid")
    C.Check.str("oversized cursor refused before arithmetic", cursor(parsed("{\"since\":99999999999999999999999999999999999,\"origin\":\"e\"}")), "invalid")
    C.Check.str("cursor beyond bounded domain refused", cursor(parsed("{\"since\":4294967296,\"origin\":\"e\"}")), "invalid")
    C.Check.str("largest supported cursor exact", cursor(parsed("{\"since\":4294967295,\"origin\":\"e\"}")), "4294967295:e")
    C.Check.str("ten digit decimal remains bounded before domain refusal", cursor(parsed("{\"since\":9999999999,\"origin\":\"e\"}")), "invalid")
    C.Check.str("empty raw numeric cursor refused", cursor(P.Session.cursor(J.Json.obj([J.KV{"since", J.Num{""}}, J.KV{"origin", J.Str{"e"}}]))), "invalid")
    C.Check.str("noncanonical leading zero cursor refused", cursor(P.Session.cursor(J.Json.obj([J.KV{"since", J.Num{"0012"}}, J.KV{"origin", J.Str{"e"}}]))), "invalid")
    C.Check.str("nonASCII raw numeric cursor refused before arithmetic", cursor(P.Session.cursor(J.Json.obj([J.KV{"since", J.Num{"１２"}}, J.KV{"origin", J.Str{"e"}}]))), "invalid")
    C.Check.str("native Nat maximum parses without overflow", Nat.show(P.Session.number(J.Json.obj([J.KV{"n", J.Num{"281474976710655"}}]), "n", 7n)), "281474976710655")
    C.Check.str("native Nat overflow falls back before multiply", Nat.show(P.Session.number(J.Json.obj([J.KV{"n", J.Num{"281474976710656"}}]), "n", 7n)), "7")
    C.Check.str("unsafe JS integer falls back before arithmetic", Nat.show(P.Session.number(J.Json.obj([J.KV{"n", J.Num{"9007199254740992"}}]), "n", 7n)), "7")
    C.Check.str("number strings cannot coerce into request counter", Nat.show(P.Session.number(J.Json.obj([J.KV{"n", J.Str{"123"}}]), "n", 7n)), "7")
    C.Check.str("negative raw number falls back before arithmetic", Nat.show(P.Session.number(J.Json.obj([J.KV{"n", J.Num{"-1"}}]), "n", 7n)), "7")
    C.Check.str("fractional raw number falls back before arithmetic", Nat.show(P.Session.number(J.Json.obj([J.KV{"n", J.Num{"1.5"}}]), "n", 7n)), "7")
    C.Check.str("malformed oversized retained counter keeps actual rid", Nat.show(U.Ui.rid(P.Session.adopt(mail(), J.Json.obj([J.KV{"rid", J.Num{"999999999999999999999999999999999"}}])))), Nat.show(U.Ui.rid(mail())))
    C.Check.str("frontend cannot silently borrow connect cursor", Bool.show(P.Session.resume_allowed("frontend", P.Captured{})), "False")
    C.Check.str("old mobile captured cursor remains valid", Bool.show(P.Session.resume_allowed("mobile", P.Captured{})), "True")
    C.Check.str("invalid resume never accepted", Bool.show(P.Session.resume_allowed("mobile", P.Invalid{})), "False")
    C.Check.str("old epoch resets to canonical beginning", Nat.show(P.Session.resume_cursor(12n, "old", "new", 20n)), "0")
    C.Check.str("future cursor resets to canonical beginning", Nat.show(P.Session.resume_cursor(21n, "same", "same", 20n)), "0")
    C.Check.str("exact committed head remains exact", Nat.show(P.Session.resume_cursor(20n, "same", "same", 20n)), "20")
    C.Check.str("valid missing suffix starts at applied cursor", Nat.show(P.Session.resume_cursor(12n, "same", "same", 20n)), "12")
    +continued : O.Cr = P.Session.history_credit(credit(), 99n, False{}, ["next"])
    C.Check.str("same writer generation sent acked byte accounting exact", counters(continued), "7:11:3:4194304:2048")
    C.Check.str("history installs exact committed progress", Nat.show(O.Cr.at(continued)), "99")
    C.Check.str("history installs exact ordered remainder", String.join(O.Cr.rest(continued), ","), "next")
    C.Check.str("history does not erase existing backpressure", Bool.show(O.Cr.room(continued)), Bool.show(O.Cr.room(credit())))
    C.Check.str("history does not hide excess credit", Bool.show(O.Cr.over(continued)), Bool.show(O.Cr.over(credit())))
    +sent : U.Act = H.One.mail.send(mail(), payload())
    +rpc : J.Json = first(U.Act.cmds(sent))
    C.Check.str("phone human send remains ordinary authenticated RPC", J.Json.get_str(rpc, "m", ""), "mail.send")
    C.Check.str("phone captures exact tap text", J.Json.get_str(J.Json.get_obj(rpc, "p"), "text", ""), "captured 日本語")
    C.Check.str("phone captures exact tap recipient", J.Json.get_str(J.Json.get_obj(rpc, "p"), "to", ""), "thread:captured")
    C.Check.str("phone send keeps newer unsent text", U.Ui.scratch(U.Act.ui(sent), "@f.mrreply"), "newer 日本語 edit")
    C.Check.str("phone send keeps newer recipient choice", U.Ui.scratch(U.Act.ui(sent), "@mr.to"), "thread:new-choice")
    C.Check.str("phone does not put mail send in durable retry outbox", Nat.show(mail_sends(U.Ui.outbox(U.Act.ui(sent)))), "0")
    C.Check.str("repeated captured send is not reissued", Nat.show(mail_sends(U.Act.cmds(H.One.mail.send(U.Act.ui(sent), payload())))), "0")
    +snapshot : J.Json = P.Session.pack(U.Act.ui(sent))
    C.Check.str("presentation excludes active ACK bookkeeping", J.Json.get_str(J.Json.get_obj(snapshot, "drafts"), MA.MA.key(U.Act.ui(sent)) ++ "state", "absent"), "absent")
    +forged : J.Json = DB.Desktop.put(DB.Desktop.put(DB.Desktop.put(P.Session.pack(U.Act.ui(sent)), "seq", J.Json.nat(999999n)), "info", DB.Desktop.put(J.Json.get_obj(snapshot, "info"), "origin", J.Str{"other"})), "drafts", DB.Desktop.put(DB.Desktop.put(J.Json.get_obj(snapshot, "drafts"), MA.MA.key(U.Act.ui(sent)) ++ "state", J.Str{"filed"}), "@f.mrreply", J.Str{"stale"}))
    +adopted : U.Ui = P.Session.adopt(U.Act.ui(sent), forged)
    C.Check.str("presentation cannot advance actual replay cursor", Nat.show(U.Ui.seq(adopted)), Nat.show(U.Ui.seq(U.Act.ui(sent))))
    C.Check.str("presentation cannot change actual origin", U.Ui.info(adopted, "origin"), "current-origin")
    C.Check.str("presentation cannot confirm uncommitted mail", MA.MA.get(adopted, "state"), "sending")
    C.Check.str("presentation cannot replace newer reply edit", U.Ui.scratch(adopted, "@f.mrreply"), "newer 日本語 edit")
    C.Check.str("disconnect leaves captured send unresolved", MA.MA.get(MA.MA.online(adopted, False{}), "state"), "unknown")
    C.Check.str("forged same-owner snapshot preserves original status namespace", MA.MA.key(adopted), MA.MA.key(U.Act.ui(sent)))
    +foreign : U.Ui = P.Session.adopt(U.Act.ui(sent), DB.Desktop.put(snapshot, "drafts", DB.Desktop.put(J.Json.get_obj(snapshot, "drafts"), "@bot", J.Str{"other-owner"})))
    C.Check.str("legitimate selection of another owner cannot borrow pending status", MA.MA.get(foreign, "state"), "")
    C.Check.str("owner switch retains original pending status in original scope", U.Ui.scratch(foreign, MA.MA.key(U.Act.ui(sent)) ++ "state"), "sending")
    C.Check.str("return to original owner after disconnect remains unresolved", MA.MA.get(MA.MA.online(P.Session.adopt(foreign, snapshot), False{}), "state"), "unknown")
    C.Check.str("command transport preserves Unicode and ordinary method bytes", J.Json.get_str(P.Session.command(U.Send{"exact 日本語"}), "text", ""), "exact 日本語")
    C.Check.str("command transport preserves typed copied text", J.Json.get_str(P.Session.command(U.Copy{"copy 日本語"}), "type", ""), "copy")
    C.Check.str("command transport retains all nine command variants", Nat.show(List.length(&2, J.Json, P.Session.commands([U.Send{"s"}, U.Copy{"c"}, U.FocusField{"f"}, U.ScrollEnd{}, U.Connect{"http://paired"}, U.Keep{"t", "d"}, U.Notify{"t", "title", "body", "k"}, U.Later{1n, "a", "v"}, U.Jump{"e"}]))), "9")
