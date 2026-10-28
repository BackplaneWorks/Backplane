import Base
import ./check.bend as C
import ../src/mobile/app.bend as A
import ../src/mobile/thin.bend as T
import ../src/web/app.bend as W
import ../src/core/json.bend as J

def obj(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def check(+name: String, +yes: Bool) -> IO(Unit):
  C.Check.str(name, Bool.show(yes), "True")

def main() -> IO(Unit):
  do IO<Unit>:
    +session : String = "{\"sel\":\"thread\",\"rid\":1,\"drafts\":{},\"route\":{\"kind\":\"bot-tab\",\"bot\":\"bot\",\"tab\":\"mail\"}}"
    +drafted : J.Json = A.session_draft(session, "日本語🙂")
    C.Check.str("draft adapter retains typed bot route", J.Json.get_str(J.Json.get_obj(drafted, "route"), "tab", ""), "mail")
    C.Check.str("draft adapter retains actual text", J.Json.get_str(J.Json.get_obj(drafted, "drafts"), "thread", ""), "日本語🙂")
    +chosen : J.Json = A.thin_choice(session, "select", "hub|other")
    check("new explicit selection removes old bot route", Bool.not(Maybe.is_some(&2, J.Json, J.Json.get(chosen, "route"))))
    C.Check.str("selected target uses actual raw ID", J.Json.get_str(A.thin_target(J.Json.show(chosen)), "id", ""), "other")
    +empty : J.Json = A.thin_partial(obj("{\"thread\":null}"), obj("{\"partial\":[]}"))
    C.Check.str("no phantom native thread on recent page", J.Json.show(J.Json.get_obj(empty, "thread")), "null")
    +partial : J.Json = A.thin_partial(obj("{\"thread\":{\"entries\":[{\"id\":\"one\",\"text\":\"prefix\"}]}}"), obj("{\"partial\":[{\"id\":\"one\",\"version\":\"3\",\"cursor\":\"opaque\",\"hasMore\":true,\"complete\":false}]}"))
    +row : J.Json = Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(partial, "thread"), "entries"))), J.Null{})
    C.Check.str("native row carries exact cursor", J.Json.get_str(J.Json.get_obj(row, "partial"), "cursor", ""), "opaque")
    check("native row never claims bounded prefix complete", Bool.not(J.Json.get_bool(J.Json.get_obj(row, "partial"), "complete", True{})))
    +said : J.Json = A.thin_partial(obj("{\"thread\":{\"entries\":[{\"id\":\"a\",\"kind\":\"assistant\",\"text\":\"x\"},{\"id\":\"u\",\"kind\":\"user\",\"text\":\"y\"},{\"id\":\"t\",\"kind\":\"act\",\"text\":\"z\"}]}}"), obj("{\"partial\":[{\"id\":\"a\",\"version\":\"1\",\"cursor\":\"c\",\"hasMore\":true,\"complete\":false},{\"id\":\"u\",\"version\":\"1\",\"cursor\":\"c\",\"hasMore\":true,\"complete\":false},{\"id\":\"t\",\"version\":\"1\",\"cursor\":\"c\",\"hasMore\":true,\"complete\":false}]}"))
    +rows : List<&2, J.Json> = J.Json.elems(J.Json.get_obj(J.Json.get_obj(said, "thread"), "entries"))
    C.Check.str("no read-rest on messages: only the activity row keeps a partial", J.Json.show(J.Json.arr(rows)), "[{\"id\":\"a\",\"kind\":\"assistant\",\"text\":\"x\"},{\"id\":\"u\",\"kind\":\"user\",\"text\":\"y\"},{\"partial\":{\"id\":\"t\",\"version\":\"1\",\"cursor\":\"c\",\"hasMore\":true,\"complete\":false},\"id\":\"t\",\"kind\":\"act\",\"text\":\"z\"}]")
    +context : J.Json = obj("{\"kind\":\"mail-draft\",\"owner\":\"owner\",\"origin\":\"epoch\",\"conv\":\"conv\",\"recipient\":\"room:r\"}")
    +local : String = W.mail_snapshot(W.mail_local_field(W.mail_local_context(W.init(0n, "cid"), context), "newer 日本語🙂"))
    C.Check.str("normal draft snapshot captures local field", J.Json.get_str(obj(local), "draft", ""), "newer 日本語🙂")
    C.Check.str("local key binds owning recipient context", W.mail_identity(W.mail_local_context(W.init(0n, "cid"), context)), "@wm:5:epoch5:ownerconv:")
    C.Check.str("local restore extracts text without command effects", W.mail_local_text(W.mail_restore(W.mail_local_context(W.init(0n, "cid"), context), "{\"draft\":\"restored\"}")), "restored")
    C.Check.str("untyped context clears local persistence identity", W.mail_identity(W.mail_local_context(W.mail_local_context(W.init(0n, "cid"), context), obj("{}"))), "")
    +search : J.Json = A.thin_search("{\"hub\":\"hub\"}", "{\"query\":\"word\",\"rows\":[{\"thread\":\"t\",\"id\":\"m7\",\"text\":\"found\"}],\"hasMore\":false}")
    C.Check.str("bounded search uses native Find mode", J.Json.get_str(J.Json.get_obj(search, "find"), "mode", ""), "search")
    +found : J.Json = Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(search, "find"), "rows"))), J.Null{})
    C.Check.str("a search row opens its thread", J.Json.get_str(found, "value", ""), "hub|t")
    C.Check.str("a search row names the message found", J.Json.get_str(found, "entry", ""), "m7")
    +snipped : J.Json = A.thin_search("{\"hub\":\"hub\"}", "{\"query\":\"word\",\"rows\":[{\"thread\":\"t\",\"id\":\"m7\",\"text\":\"found\",\"title\":\"Power\",\"snip\":[\"a\\n\",\"word\",\" b\"]}],\"hasMore\":false}")
    +snrow : J.Json = Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(snipped, "find"), "rows"))), J.Null{})
    C.Check.str("a result from any thread names its thread, the words around the match on a line", J.Json.get_str(snrow, "label", ""), "Power · a word b")
