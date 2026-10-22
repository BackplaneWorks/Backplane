import Base
import ./host.bend as H
import ../core/utf8.bend as U
import ../core/copynet.bend as CN

# Running commands to completion. Loops that wait on the outside world count
# down a Nat fuel (Bend requires every loop to end).

type Chunks is Data:
  Chunks{rev: List<&2, List<&2, U32>>}

def Chunks.bytes(c: Chunks) -> List<&2, U32>:
  match c:
    case Chunks{rev}:
      List.concat(&2, U32, List.reverse(&2, List<&2, U32>, rev))

def Proc.drain.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, acc: Chunks,
    k: Socket -> Chunks -> IO(Socket & List<&2, U32>)) -> IO(Socket & List<&2, U32>):
  (s, res) = r
  match res acc:
    case Done{Nil{}} _:
      IO.pure(Socket & List<&2, U32>, (s, Chunks.bytes(acc)))
    case Done{bs} Chunks{rev}:
      k(s, Chunks{bs <> rev})
    case Fail{_} _:
      IO.pure(Socket & List<&2, U32>, (s, Chunks.bytes(acc)))

# every byte until the peer closes its side
def Proc.drain(fuel: Nat, s: Socket, acc: Chunks) -> IO(Socket & List<&2, U32>):
  match fuel:
    case 0n:
      IO.pure(Socket & List<&2, U32>, (s, Chunks.bytes(acc)))
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Socket & List<&2, U32>,
        H.Sock.recv_bytes(s, 65536),
        r => Proc.drain.got(r, acc, s2 => a2 => Proc.drain(f, s2, a2)))

# the result of a finished command
type Ran is Data:
  Ran{code: U32, out: String}

def Proc.finish(pid: U32, r: Socket & List<&2, U32>) -> IO(Ran):
  (s, bs) = r
  do IO<Ran>:
    Socket.close(s)
    code : U32 <- H.Proc.wait(pid)
    return Ran{code, U.Utf8.decode(bs)}

def Proc.collect(started: U32 & Socket) -> IO(Ran):
  (pid, s) = started
  do IO<Ran>:
    s2 : Socket <- H.Sock.shutdown(s)
    r : Socket & List<&2, U32> <- Proc.drain(1000000n, s2, Chunks{Nil{}})
    Proc.finish(pid, r)

def Proc.run.fail(e: U32 & String) -> IO(Ran):
  (code, msg) = e
  IO.pure(Ran, Ran{127, msg})

def Proc.run.k(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(Ran):
  match r:
    case Fail{e}:
      Proc.run.fail(e)
    case Done{started}:
      Proc.collect(started)

# Every command the hub runs for itself, and any git under it, takes git
# settings that win over a project's own .git/config: no fsmonitor command,
# no hooks, no pager (a folder from a zip could ship a .git, or a
# .backplane/history.git, whose config runs commands on the hub's git).
# git.bend's Git.safe.sh also blanks the filters a repository defines
# itself, and the hub's diffs take --no-ext-diff --no-textconv (an empty
# diff.external is a command too). Agents' own git is theirs: their
# processes start elsewhere (H.Proc.spawn).
def Proc.git.env() -> List<&2, String>:
  ["GIT_CONFIG_COUNT=3", "GIT_CONFIG_KEY_0=core.fsmonitor", "GIT_CONFIG_VALUE_0=false", "GIT_CONFIG_KEY_1=core.hooksPath", "GIT_CONFIG_VALUE_1=/dev/null",
   "GIT_CONFIG_KEY_2=core.pager", "GIT_CONFIG_VALUE_2=cat", "GIT_PAGER=cat"]

# whether this hub is on a copied log (core/copy.bend)
def Proc.copy() -> IO(Bool):
  IO.bind(Result<&1, &1, U32 & String, String>, Bool, IO.get_env("BACKPLANE_COPY"), r => IO.pure(Bool, String.eq(Result.default(&1, &1, U32 & String, String, r, ""), "1")))

def Proc.run.go(refused: Bool, +cmd: String, args: List<&2, String>, cwd: String) -> IO(Ran):
  match refused:
    case True{}:
      IO.pure(Ran, Ran{126, CN.Copy.refusal(cmd)})
    case False{}:
      IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, Ran, H.Proc.spawn("env", List.append(&2, String, Proc.git.env(), cmd <> args), cwd, 1), Proc.run.k)

# run cmd with args in cwd; stdout and stderr together, stdin closed (a
# hub on a copied log runs no network command: CN.Copy.refused)
def Proc.run(+cmd: String, +args: List<&2, String>, +cwd: String) -> IO(Ran):
  IO.bind(Bool, Ran, Proc.copy(), copy => Proc.run.go(CN.Copy.refused(copy, cmd, args), cmd, args, cwd))
