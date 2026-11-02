import Base
import ./check.bend as C
import ../src/core/json.bend as J

def show(m: Maybe<&2, J.Json>) -> String:
  match m:
    case None{}:
      "<bad>"
    case Some{j}:
      J.Json.show(j)

def rt(s: String) -> String:
  show(J.Json.parse(s))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("roundtrip", rt("{\"a\": [1, -2.5e3, true, false, null], \"b\": {\"c\": \"x\\ny\\u0041\\\"\"}, \"d\": []}"), "{\"a\":[1,-2.5e3,true,false,null],\"b\":{\"c\":\"x\\nyA\\\"\"},\"d\":[]}")
    C.Check.str("unclosed", rt("[1, 2"), "<bad>")
    C.Check.str("escapes", rt("  \"tab\\tq\"  "), "\"tab\\tq\"")
    C.Check.str("get", J.Json.get_str(J.Json.obj([J.KV{"k", J.Json.str("v")}]), "k", "?"), "v")
    C.Check.str("nested get", J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse("{\"m\":{\"t\":\"x\"}}"), J.Null{}), "m"), "t", "?"), "x")
    C.Check.str("ctl", J.Json.show(J.Json.str(SCon{Chr{1}, "z"})), "\"\\u0001z\"")
    C.Check.str("surrogate pair", rt("\"a\\ud83d\\udc69b\""), "\"a\u{1F469}b\"")
    C.Check.str("emoji zwj", rt("\"\\ud83d\\udc69\\ud83c\\udffd\\u200d\\ud83d\\udcbb\""), "\"\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB}\"")
    C.Check.str("lone high", rt("\"a\\ud83db\""), "\"a\u{FFFD}b\"")
    C.Check.str("lone high at end", rt("\"a\\ud83d\""), "\"a\u{FFFD}\"")
    C.Check.str("lone low", rt("\"\\udc69x\""), "\"\u{FFFD}x\"")
    C.Check.str("high then escape", rt("\"\\ud83d\\n\""), "\"\u{FFFD}\\n\"")
    C.Check.str("high then high", rt("\"\\ud83d\\ud83d\\udc69\""), "\"\u{FFFD}\u{1F469}\"")
    C.Check.str("high then bmp", rt("\"\\ud83d\\u0041\""), "\"\u{FFFD}A\"")
    C.Check.str("high unclosed", rt("\"\\ud83d"), "<bad>")
