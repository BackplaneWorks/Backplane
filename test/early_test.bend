import Base
import ./check.bend as C
import ../src/core/early.bend as E
import ../src/core/utf8.bend as U

# The hub's answer while it starts (core/early.bend): the note from
# store.build's text, the body, the whole 503

def head(bs: List<&2, U32>) -> String:
  U.Utf8.decode(bs)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("missing", E.Early.note(""), "starting")
    C.Check.str("0%", E.Early.note("0 1000\n"), "building store 0%")
    C.Check.str("50%", E.Early.note("500 1000\n"), "building store 50%")
    C.Check.str("100%", E.Early.note("1000 1000"), "building store 100%")
    C.Check.str("past the end", E.Early.note("1200 1000\n"), "building store 100%")
    C.Check.str("floor", E.Early.note("1 3"), "building store 33%")
    C.Check.str("big", E.Early.note("2500000000 10000000000\n"), "building store 25%")
    C.Check.str("no total", E.Early.note("0 0\n"), "building store 0%")
    C.Check.str("garbage", E.Early.note("half done\n"), "starting")
    C.Check.str("one word", E.Early.note("12\n"), "starting")
    C.Check.str("three words", E.Early.note("1 2 3\n"), "starting")
    C.Check.str("negative", E.Early.note("-1 10"), "starting")
    C.Check.str("body", E.Early.body("0.12.1", "250 1000\n"), "{\"status\":\"starting\",\"note\":\"building store 25%\",\"app\":\"backplane\",\"version\":\"0.12.1\"}")
    C.Check.str("body starting", E.Early.body("0.12.1", ""), "{\"status\":\"starting\",\"note\":\"starting\",\"app\":\"backplane\",\"version\":\"0.12.1\"}")
    C.Check.str("response", head(E.Early.response("1.2.3", "")), "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: 75\r\nCache-Control: no-store\r\nConnection: close\r\nRetry-After: 2\r\n\r\n{\"status\":\"starting\",\"note\":\"starting\",\"app\":\"backplane\",\"version\":\"1.2.3\"}")
    C.Check.str("path", E.Early.build.path("/h"), "/h/store.build")
