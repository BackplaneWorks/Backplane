import Base
import ./check.bend as C
import ../src/server/thin-body.bend as B
import ../src/server/db.bend as D
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L
import ../src/core/model.bend as M

def request(cursor: String) -> J.Json:
  J.Json.obj([J.KV{"target", J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"a"}}])},
    J.KV{"entryId", J.Str{"m"}}, J.KV{"bodyVersion", J.Str{"7"}}, J.KV{"cursor", J.Str{cursor}}])

def get(read: B.Read) -> J.Json:
  match read:
    case B.Read{_, message}: message

def code(read: B.Read) -> String:
  match read:
    case B.Read{code, _}: code

def put(+p: J.Json, key: String, value: J.Json) -> J.Json:
  J.Json.obj([J.KV{key, value}, J.KV{"target", J.Json.get_obj(p, "target")}, J.KV{"entryId", J.Str{J.Json.get_str(p, "entryId", "")}},
    J.KV{"bodyVersion", J.Str{J.Json.get_str(p, "bodyVersion", "")}}, J.KV{"cursor", J.Str{J.Json.get_str(p, "cursor", "")}}])

def main() -> IO(Unit):
  do IO<Unit>:
    +opened : String <- D.Db.open(":memory:")
    +h : String = String.drop(opened, 3n)
    +schema : String <- D.Db.exec(h, "CREATE TABLE entry(seq INTEGER,ord INTEGER,id TEXT,thread TEXT,src TEXT,kind TEXT,role TEXT,akind TEXT,text TEXT,at INTEGER)")
    C.Check.str("body fixture uses actual SQLite", schema, "ok")
    +prefix : String = String.repeat("x", 8191n)
    +ending : String = "😀日本\u{0}END"
    +inserted : String <- D.Db.run.t(h, "INSERT INTO entry VALUES(7,7,'m','a','','msg','assistant','',?,9)", [D.DbText{prefix ++ ending}])
    C.Check.str("large UTF8 and NUL body inserted", String.take(inserted, 2n), "ok")
    +first : B.Read <- B.Body.read(h, "a", request(""), "epoch", ["7"])
    C.Check.str("issued initial body read succeeds", code(first), "")
    C.Check.str("UTF8 scalar never split at byte8192", J.Json.get_str(get(first), "text", ""), prefix)
    C.Check.str("prefix is explicitly incomplete", Bool.show(J.Json.get_bool(get(first), "complete", True{})), "False")
    +second : B.Read <- B.Body.read(h, "a", request(J.Json.get_str(get(first), "cursor", "")), "epoch", ["7"])
    C.Check.str("next chunk preserves Unicode and NUL without a gap", J.Json.get_str(get(second), "text", ""), ending)
    C.Check.str("final byte range marks completion", Bool.show(J.Json.get_bool(get(second), "complete", False{})), "True")
    +previous : B.Read <- B.Body.read(h, "a", request(J.Json.get_str(get(second), "previous", "")), "epoch", ["7"])
    C.Check.str("reverse range ends exactly before retained next chunk", J.Json.get_str(get(previous), "text", ""), prefix)
    +missing : B.Read <- B.Body.read(h, "a", request(""), "epoch", Nil{})
    C.Check.str("initial caller cannot invent page membership", code(missing), "NOT_FOUND")
    +foreign : B.Read <- B.Body.read(h, "b", request(""), "epoch", ["7"])
    C.Check.str("body ownership rejects another selected thread", code(foreign), "NOT_FOUND")
    +stale : B.Read <- B.Body.read(h, "a", request(J.Json.get_str(get(first), "cursor", "")), "other-epoch", ["7"])
    C.Check.str("old epoch cannot continue body", code(stale), "STALE_CURSOR")
    +closed : String <- D.Db.close(h)
    C.Check.str("SQLite fixture closed", closed, "ok")
    +source : String = String.repeat("line 日本語 😀\n", 1200n)
    +stream : L.Stream = L.Stream{1n, String.length(source), T.Blocks.of(source), False{}, 9n}
    +part : J.Json = B.Body.live.part(stream, "a", "epoch", J.Json.get_obj(request(""), "target"))
    +liveP : J.Json = J.Json.obj([J.KV{"target", J.Json.get_obj(request(""), "target")}, J.KV{"entryId", J.Str{L.Stream.id("a")}}, J.KV{"bodyVersion", J.Str{"9"}}, J.KV{"cursor", J.Str{J.Json.get_str(part, "cursor", "")}}])
    +live : B.Read = B.Body.live.read(stream, "a", liveP, "epoch")
    C.Check.str("large live text has a bounded real body page", Bool.show(Nat.is_le(String.length(J.Json.get_str(get(live), "text", "")), 8192n)), "True")
    C.Check.str("new live version rejects old continuation", code(B.Body.live.read(L.Stream{1n, String.length(source), T.Blocks.of(source), False{}, 10n}, "a", liveP, "epoch")), "STALE_CURSOR")
