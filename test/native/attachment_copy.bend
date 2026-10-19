import Base
import ../../src/server/argv.bend as A
import ../../src/server/host.bend as H
import ../../src/server/server.bend as S

# Exercise the same file/stat/read/write/reply path as attach.file without
# starting a hub. Only synthetic paths supplied by the test are touched.
def report(m: Maybe<&1, S.Msg>) -> IO(Unit):
  match m:
    case Some{S.Push{_, text}}:
      IO.print(text)
    case _:
      IO.die(Unit, 1, "missing attachment reply")

def copy(src: String, home: String) -> IO(Unit):
  do IO<Unit>:
    +inbox : Chan(S.Msg) <- Chan.new(S.Msg, 4)
    S.Media.file(inbox, 1, "1", "t", src, "copy.bin", home)
    IO.bind(Maybe<&1, S.Msg>, Unit, Chan.recv(S.Msg, inbox), report)

def args(xs: List<String>) -> IO(Unit):
  match xs:
    case src <> home <> Nil{}:
      copy(src, home)
    case _:
      IO.die(Unit, 2, "usage: attachment-copy SOURCE HOME")

def linked(ok: Bool) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(List<String>, Unit, A.Argv.args(), args)
    case False{}:
      IO.die(Unit, 1, "host linkage failed")

def main() -> IO(Unit):
  IO.bind(List<&2, U32>, Unit, H.Json.cbor_frame("null", "", ""), frame => linked(Nat.is_eq(List.length(&2, U32, frame), 3n)))
