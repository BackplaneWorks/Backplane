import Base
import ./check.bend as C
import ../src/server/term.bend as T
import ../src/core/vt.bend as V
import ../src/core/utf8.bend as U8
import ../src/core/emb.bend as E

def screen(es: List<&2, T.ETerm>) -> String:
  match es:
    case T.ETerm{_, _, v, _, _, _} <> _: E.Emb.snap.text(v)
    case _: ""

def saved() -> List<&2, T.ETerm>:
  [T.ETerm{"t", "", V.Vt.feed(V.Vt.new(20, 5), U8.Utf8.encode("PRESERVED")), True{}, "running", 42}]

def main() -> IO(Unit):
  do IO<Unit>:
    +es : List<&2, T.ETerm> = saved()
    C.Check.str("same shell keeps screen on reopen", Bool.show(String.contains(screen(T.Et.shell.open(T.Et.find(es, "t"), es, "t", 20, 5, 42)), "PRESERVED")), "True")
    C.Check.str("new shell starts blank", Bool.show(String.contains(screen(T.Et.shell.open(T.Et.find(es, "t"), es, "t", 20, 5, 43)), "PRESERVED")), "False")
    C.Check.str("current process output is retained", Bool.show(String.contains(screen(T.Et.shell.fed(T.Et.find(es, "t"), es, "t", 42, U8.Utf8.encode("_NEW"))), "PRESERVED_NEW")), "True")
    C.Check.str("retired process cannot alter saved screen", Bool.show(String.contains(screen(T.Et.shell.fed(T.Et.find(es, "t"), es, "t", 41, U8.Utf8.encode("_OLD"))), "_OLD")), "False")
    C.Check.str("resize keeps screen contents", Bool.show(String.contains(screen(T.Et.shell.resize(T.Et.find(es, "t"), es, "t", 30, 6)), "PRESERVED")), "True")
    C.Check.str("another shell stays separate", Bool.show(String.contains(screen(T.Et.shell.fed(T.Et.find(es, "other"), es, "other", 42, U8.Utf8.encode("WRONG"))), "WRONG")), "False")
