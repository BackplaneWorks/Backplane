import Base
import ./check.bend as C
import ../src/core/ref.bend as R

def Show(m: Maybe<&2, R.Ref>) -> String:
  match m:
    case None{}: "None"
    case Some{R.Ref{k, xs, f}}: k ++ ":" ++ String.join(xs, "|") ++ "#" ++ f

def Round(rs: List<&2, R.Ref>) -> IO(Unit):
  match rs:
    case Nil{}: IO.pure(Unit, Unit{})
    case +r <> t:
      do IO<Unit>:
        C.Check.str("uri " ++ R.Ref.path(r), Show(R.Ref.parse(R.Ref.uri(r))), Show(Some{r}))
        C.Check.str("web " ++ R.Ref.path(r), Show(R.Ref.parse(R.Ref.web("https://h:3787/", r))), Show(Some{r}))
        C.Check.str("addr " ++ R.Ref.path(r), Show(R.Ref.of_addr(R.Ref.addr(r))), Show(Some{r}))
        Round(t)

def main() -> IO(Unit):
  do IO<Unit>:
    Round([R.Ref.thread("p1~t17-2"), R.Ref.bot("b1-2"), R.Ref.room("r1-2"), R.Ref.person(), R.Ref.project("p1-2"), R.Ref.routine("o1-2"), R.Ref.task("b1-2-3"), R.Ref.pr("p1-2", "42"), R.Ref.work("w1"), R.Ref.file("p1-2", ["dir name", "é.txt"]), R.Ref.machine("p1"), R.Ref.conv("pair:bot:b1|thread:t2"), R.Ref.mail("pair:bot:b1|thread:t2", "m5"), R.Ref.mailroom("*"), R.Ref.decision("b1", "d1"), R.Ref.charter("b1")])
    C.Check.str("pct bytes", R.Ref.pct(":|/ é%"), "%3A%7C%2F%20%C3%A9%25")
    C.Check.str("pct safe", R.Ref.pct("AZaz09-._~"), "AZaz09-._~")
    C.Check.str("utf8 all widths", R.Ref.unpct(R.Ref.pct("é€😀")), "é€😀")
    C.Check.str("decoded slash", Show(R.Ref.parse("backplane://file/p1/a%2Fb")), "file:p1|a/b#")
    C.Check.str("malformed percent", R.Ref.unpct("%zz % %A %2z %%41"), "%zz % %A %2z %A")
    C.Check.str("lower hex", R.Ref.unpct("%c3%a9"), "é")
    C.Check.str("frag", Show(R.Ref.parse(R.Ref.uri(R.Ref.msg("p1~t17-2", "m:é")))), "thread:p1~t17-2#m:é")
    C.Check.str("legacy", Show(R.Ref.parse("backplane://open?thread=p1~t17-2")), "thread:p1~t17-2#")
    C.Check.str("legacy encoded", Show(R.Ref.parse("backplane://open?thread=t%3A1")), "thread:t:1#")
    C.Check.str("web no slash", Show(R.Ref.parse("http://h:3787#/thread/t1-2")), "thread:t1-2#")
    C.Check.str("trailing slash", Show(R.Ref.parse("backplane://thread/t1-2/")), "thread:t1-2#")
    C.Check.str("remote", R.Ref.remote(R.Ref.thread("p1~t17-2")), "p1")
    C.Check.str("local", R.Ref.remote(R.Ref.bot("b1-2")), "")
    C.Check.str("bare thread", Show(R.Ref.bare("t1-2")), "thread:t1-2#")
    C.Check.str("bare bot", Show(R.Ref.bare("b1-2")), "bot:b1-2#")
    C.Check.str("bare task", Show(R.Ref.bare("b1-2-3")), "task:b1-2-3#")
    C.Check.str("bare project", Show(R.Ref.bare("p1-2")), "project:p1-2#")
    C.Check.str("bare room", Show(R.Ref.bare("r1-2")), "room:r1-2#")
    C.Check.str("bare routine", Show(R.Ref.bare("o1-2")), "routine:o1-2#")
    C.Check.str("bare remote", Show(R.Ref.bare("p1~t17-2")), "thread:p1~t17-2#")
    C.Check.str("bare missing group", Show(R.Ref.bare("t12")), "None")
    C.Check.str("bare word", Show(R.Ref.bare("the")), "None")
    C.Check.str("bare incomplete", Show(R.Ref.bare("t1-")), "None")
    C.Check.str("bare unknown", Show(R.Ref.bare("x1-2")), "None")
    C.Check.str("bare extra", Show(R.Ref.bare("t1-2-3")), "None")
    C.Check.str("bare empty link", Show(R.Ref.bare("~t1-2")), "None")
    C.Check.str("bare extra link", Show(R.Ref.bare("p1~p2~t1-2")), "None")
    C.Check.str("unknown", Show(R.Ref.parse("backplane://nope/x")), "None")
    C.Check.str("script", Show(R.Ref.parse("javascript:thread/t1-2")), "None")
    C.Check.str("missing id", Show(R.Ref.parse("backplane://thread/")), "None")
    C.Check.str("empty id", Show(R.Ref.parse("backplane://mail//m1")), "None")
    C.Check.str("extra id", Show(R.Ref.parse("backplane://thread/t1/extra")), "None")
    C.Check.str("person id", Show(R.Ref.parse("backplane://person/x")), "None")
    C.Check.str("nonthread frag", Show(R.Ref.parse("backplane://bot/b1#m1")), "None")
    C.Check.str("web path forbidden", Show(R.Ref.parse("http://h/path/#/thread/t1")), "None")
    C.Check.str("web empty host", Show(R.Ref.parse("http:///#/thread/t1")), "None")
    C.Check.str("addr drops frag", R.Ref.addr(R.Ref.msg("t1-2", "m1")), "thread:t1-2")
    C.Check.str("all kinds", String.join(R.Ref.kinds(), " "), "thread bot room person project routine task pr work file machine conv mail mailroom decision charter")
    C.Check.str("known", Bool.show(R.Ref.known("mailroom")), "True")
    C.Check.str("unknown kind", Bool.show(R.Ref.known("nope")), "False")
    C.Check.str("conv is not remote", R.Ref.remote(R.Ref.conv("pair:bot:p1~b1|thread:t2")), "")
    C.Check.str("bare link is a word", Show(R.Ref.bare("a b~t1-2")), "None")
