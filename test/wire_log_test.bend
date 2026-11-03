import Base
import ./check.bend as C
import ../src/server/db.bend as D
import ../src/server/wire-log.bend as W

# Real file-backed SQLite, independent foreign-effect closures, and public
# queued wire rows. No child hub or live home is involved.
def dir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}: IO.pure(String, String.trim(out))
    case Fail{(_, e)}: IO.die(String, 1, "wire_log_test: mktemp failed: " ++ e)

def main() -> IO(Unit):
  do IO<Unit>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- dir.of(made)
    opened : String <- D.Db.open(dir ++ "/store.db")
    +h : String = String.drop(opened, 3n)
    ddl : String <- D.Db.exec(h, "CREATE TABLE line(seq INTEGER PRIMARY KEY, off INTEGER, len INTEGER, wire TEXT); INSERT INTO line VALUES(100,0,0,'public-100')")
    C.Check.str("file database initialized", ddl, "ok")
    +disk : W.WirePage <- W.WireLog.replay(h, dir, 103n, 99n, ["public-103", "public-102", "public-101"])
    C.Check.str("durable prefix before queued suffix", String.join(W.WirePage.lines(disk), ","), "public-100")
    C.Check.str("durable prefix did not request absent queued rows", Bool.show(W.WirePage.ok(disk)), "True")
    queued : W.WirePage <- W.WireLog.replay(h, dir, 103n, 100n, ["public-103", "public-102", "public-101"])
    C.Check.str("first queued row follows prefix", String.join(W.WirePage.lines(queued), ","), "public-101,public-102,public-103")
    resumed : W.WirePage <- W.WireLog.replay(h, dir, 103n, 101n, ["public-103", "public-102", "public-101"])
    C.Check.str("queued resume preserves cursor", String.join(W.WirePage.lines(resumed), ","), "public-102,public-103")
    appended : String <- D.Db.exec(h, "INSERT INTO line VALUES(101,0,0,'public-101'),(102,0,0,'public-102'),(103,0,0,'public-103')")
    C.Check.str("queued suffix persisted", appended, "ok")
    flushed : W.WirePage <- W.WireLog.replay(h, dir, 103n, 100n, Nil{})
    C.Check.str("post-flush SQL replay equals pending replay", String.join(W.WirePage.lines(flushed), ","), "public-101,public-102,public-103")
    closed : String <- D.Db.close(h)
    C.Check.str("writer closes after independent page readers", closed, "ok")
    removed : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", "--", dir], "", 4096, 10000)
    return Unit{}
