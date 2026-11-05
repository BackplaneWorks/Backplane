import Base
import ./check.bend as C
import ../src/server/farview.bend as F
import ../src/core/fview.bend as FV
import ../src/core/range.bend as R
import ../src/core/utf8.bend as U

# The far-view stream handoff: metadata only (the root, the file in it and
# the hub's home: the file is opened by descriptor when it is sent),
# unchanged binary HTTP framing. This does not exercise sockets or authenticate a peer.
def marked(bs: List<&2, U32>) -> String:
  U.Utf8.decode(List.drop(&2, U32, bs, 1n))

def main() -> IO(Unit):
  do IO<Unit>:
    +file : List<&2, U32> <- F.Fvs.get.read(209715200n, "/private/a\tb", "d/c.glb", "/home/h")
    C.Check.str("far stream response is a metadata marker", U32.show(Maybe.default(&2, U32, List.head(&2, U32, file), 255)), "0")
    C.Check.str("far stream marker keeps the byte count", Nat.show(F.Stream.size(marked(file))), "209715200")
    C.Check.str("far stream marker keeps the root, tabs and all", F.Stream.root(marked(file)), "/private/a\tb")
    C.Check.str("far stream marker names the file in the root", F.Stream.rel(marked(file)), "d/c.glb")
    C.Check.str("far stream marker names the hub's home", F.Stream.home(marked(file)), "/home/h")
    C.Check.str("far stream ceiling fits send_path without narrowing", Nat.show(U32.to_nat(U32.from_nat(FV.Fv.cap()))), Nat.show(FV.Fv.cap()))
    C.Check.str("far stream head retains the complete bounded length", Bool.show(String.contains(R.Rng.head(R.Rng.whole(FV.Fv.cap()), FV.Fv.cap(), "application/octet-stream", "no-store"), "Content-Length: 4194304000\r\n")), "True")
    +empty : List<&2, U32> <- F.Fvs.get.read(0n, "/private", "empty", "/home/h")
    C.Check.str("empty far file remains a complete HTTP response", Bool.show(String.starts_with(U.Utf8.decode(empty), "HTTP/1.1 200 OK\r\n")), "True")
    +over : List<&2, U32> <- F.Fvs.get.read(Nat.add(FV.Fv.cap(), 1n), "/private", "over", "/home/h")
    C.Check.str("oversized far file is refused before streaming", Bool.show(String.starts_with(U.Utf8.decode(over), "HTTP/1.1 413 Payload Too Large\r\n")), "True")
    +largest : List<&2, U32> <- F.Fvs.get.read(FV.Fv.cap(), "/private", "max.glb", "/home/h")
    C.Check.str("largest far file carries metadata rather than bytes", Bool.show(Nat.is_lt(List.length(&2, U32, largest), 128n)), "True")
