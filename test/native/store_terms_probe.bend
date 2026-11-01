import Base
import ../../src/server/argv.bend as AV
import ../../src/server/storeterms.bend as STT
import ../../src/server/db.bend as D
import ../../src/core/storedb.bend as SDB

# `store_terms_probe HOME`: what the hub does for a store whose search terms
# are behind (storeterms.bend): the child's part (term_next made from the
# entries), the hub's swap, and the child again (term_old cleared), all on
# one handle. Run: bend test/native/store_terms_probe.bend HOME
# (test/tools/store_terms_e2e.ts)

def probe.swap(built: Bool, +h: String, +out: String) -> IO(String):
  match built:
    case True{}:
      STT.Terms.swap(h, STT.Terms.num(String.drop(out, 6n)))
    case False{}:
      IO.pure(String, "not built: " ++ out)

def probe.h(+h: String) -> IO(Unit):
  do IO<Unit>:
    have : String <- STT.Terms.meta(h, "terms")
    +out : String <- STT.Terms.go(SDB.Sdb.terms.due(have), h)
    +sw : String <- probe.swap(String.starts_with(out, "built "), h, out)
    cl : String <- STT.Terms.clear(h, "term_old")
    c : String <- D.Db.close(h)
    IO.print("probe: " ++ out ++ " | swap " ++ sw ++ " | clear " ++ cl)

def probe.o(+o: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.starts_with(o, "ok "), probe.h(String.drop(o, 3n)), IO.print(o))

def probe.k(xs: List<String>) -> IO(Unit):
  match xs:
    case home <> _:
      IO.bind(String, Unit, D.Db.open(home ++ "/store.db"), o => probe.o(o))
    case Nil{}:
      IO.print("usage: store_terms_probe HOME")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, AV.Argv.args(), probe.k)
