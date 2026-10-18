import Base
import ./check.bend as C
import ../src/core/mailbody.bend as MB
import ../src/core/mail.bend as ML
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# Fixtures shared by the real filesystem tests. Delimiters are intentional.
def fixture(id: String, +conv: String, text: String, +at: Nat) -> MB.Body:
  MB.Body{ML.Conv{conv, "subject", "pair", ["bot:a", "thread:b"], "p", ["tag"], 0n, at, 1n}, ML.Mail{id, conv, "bot:a", "thread:b", "thread", "", "act", "header", 0n, at}, text}

def encoded(b: Maybe<&2, MB.Body>) -> String:
  match b:
    case None{}: "none"
    case Some{b}: MB.Body.line(b)

def long(n: Nat, +s: String) -> String:
  match n:
    case 0n: s
    case 1n+k: long(k, String.reverse.go(String.reverse(s), s))

def newlines(s: String, n: Nat) -> Nat:
  match s:
    case SNil{}: n
    case SCon{Chr{+c}, t}: newlines(t, Nat.add(n, Bool.pick(Nat, U32.is_eq(c, 10), 1n, 0n)))

def replaced(fs: J.Json, +key: String, +v: J.Json) -> J.Json:
  match fs:
    case J.Field{+k, old, t}: J.Field{k, Bool.pick(J.Json, String.eq(k, key), v, old), replaced(t, key, v)}
    case _: J.End{}

def field(j: J.Json, key: String, v: J.Json) -> J.Json:
  match j:
    case J.Obj{fs}: J.Obj{replaced(fs, key, v)}
    case _: J.Null{}

def rejection(name: String, j: J.Json) -> IO(Unit):
  C.Check.str(name, Bool.show(Maybe.is_some(&2, MB.Body, MB.Body.decode(j))), "False")

def badnum(+fieldname: String, +raw: String) -> IO(Unit):
  rejection(fieldname ++ raw, field(MB.Body.encode(fixture("m", "c", "", 0n)), "mail", field(MB.Body.mail.encode(MB.Body.mail(fixture("m", "c", "", 0n))), fieldname, J.Num{raw})))

def recovered_ids(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.MailPosted{id, _, _, _, _, _, _, _, _, _} <> t: id ++ recovered_ids(t)
    case _ <> t: recovered_ids(t)

# The generic test helper compares with Base.String.eq, which itself has
# non-tail recursion. Compare long protocol outputs without that stack cost.
def check_long(name: String, got: String, want: String) -> IO(Unit):
  C.Check.str(name, Bool.show(MB.Body.same(got, want)), "True")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("indexed row parser roundtrip", Bool.show(Maybe.is_some(&2, MB.Body, MB.Body.parse_line(MB.Body.line(fixture("row", "conv", "雪😀\n\"{}", 0n))))), "True")
    C.Check.str("indexed row needs terminator", Bool.show(Maybe.is_some(&2, MB.Body, MB.Body.parse_line(J.Json.show(MB.Body.encode(fixture("row", "conv", "", 0n)))))), "False")
    C.Check.str("indexed row rejects interior newline", Bool.show(Maybe.is_some(&2, MB.Body, MB.Body.parse_line("{\n}\n"))), "False")
    +text : String = long(15n, "雪😀\n\"{\"text\":true}\\")
    +b : MB.Body = fixture("m", "c", text, 0n)
    check_long("unicode quotes newline JSON and long exact roundtrip", encoded(MB.Body.parse(MB.Body.line(b))), MB.Body.line(b))
    check_long("indexed long row exact roundtrip", encoded(MB.Body.parse_line(MB.Body.line(b))), MB.Body.line(b))
    C.Check.str("one literal newline terminator", Nat.show(newlines(MB.Body.line(b), 0n)), "1")
    +arrays : MB.Body = MB.Body{ML.Conv{"c", "", "", ["a|b", "c\nd", "雪"], "", ["x,y", "z\nq"], 0n, 0n, 0n}, ML.Mail{"m", "c", "", "", "", "", "fyi", "", 0n, 0n}, ""}
    C.Check.str("array delimiters preserved", encoded(MB.Body.decode(MB.Body.encode(arrays))), MB.Body.line(arrays))
    rejection("unknown version", field(MB.Body.encode(b), "v", J.Num{"2"}))
    rejection("version string", field(MB.Body.encode(b), "v", J.Str{"1"}))
    rejection("text type", field(MB.Body.encode(b), "text", J.Null{}))
    rejection("missing text", J.Json.obj([J.KV{"v", J.Num{"1"}}, J.KV{"conv", MB.Body.conv.encode(MB.Body.conv(b))}, J.KV{"mail", MB.Body.mail.encode(MB.Body.mail(b))}]))
    rejection("empty conv id", MB.Body.encode(fixture("m", "", "", 0n)))
    rejection("empty mail id", MB.Body.encode(fixture("", "c", "", 0n)))
    rejection("mismatched conv", field(MB.Body.encode(b), "mail", field(MB.Body.mail.encode(MB.Body.mail(b)), "conv", J.Str{"other"})))
    rejection("invalid want", field(MB.Body.encode(b), "mail", field(MB.Body.mail.encode(MB.Body.mail(b)), "want", J.Str{"maybe"})))
    rejection("invalid array member", field(MB.Body.encode(b), "conv", field(MB.Body.conv.encode(MB.Body.conv(b)), "parts", J.Json.arr([J.Num{"1"}]))))
    rejection("delimiter string not array", field(MB.Body.encode(b), "conv", field(MB.Body.conv.encode(MB.Body.conv(b)), "tags", J.Str{"a|b"})))
    badnum("size", "-1")
    badnum("at", "1.5")
    badnum("size", "1e3")
    badnum("at", "01")
    rejection("negative conv count", field(MB.Body.encode(b), "conv", field(MB.Body.conv.encode(MB.Body.conv(b)), "n", J.Num{"-1"})))
    rejection("fraction conv time", field(MB.Body.encode(b), "conv", field(MB.Body.conv.encode(MB.Body.conv(b)), "made", J.Num{"0.5"})))
    +extra : J.Json = J.Json.obj([J.KV{"v", J.Num{"1"}}, J.KV{"conv", MB.Body.conv.encode(MB.Body.conv(b))}, J.KV{"mail", MB.Body.mail.encode(MB.Body.mail(b))}, J.KV{"text", J.Str{text}}, J.KV{"future", J.Flag{True{}}}])
    check_long("extra field tolerated", encoded(MB.Body.decode(extra)), MB.Body.line(b))
    C.Check.str("hostile path one segment", MB.Body.path("/tmp/h", "../a:b|%雪;$(x)"), "/tmp/h/mail/..%2Fa%3Ab%7C%25%E9%9B%AA%3B%24%28x%29.jsonl")
    C.Check.str("percent not double interpreted", MB.Body.path("H", "%2f/.."), "H/mail/%252f%2F...jsonl")
    C.Check.str("recovery same id", recovered_ids(MB.Body.recover(b, ML.Box.empty())), "m")
    C.Check.str("seen recovery empty", Nat.show(List.length(&2, M.Change, MB.Body.recover(b, ML.Box.mail(ML.Box.empty(), MB.Body.mail(b))))), "0")
    C.Check.str("existing conv preserves metadata", Nat.show(List.length(&2, M.Change, MB.Body.recover(b, ML.Box.conv_set(ML.Box.empty(), "c", "new", "room", ["bot:z"], "q", ["changed"], 99n)))), "1")
