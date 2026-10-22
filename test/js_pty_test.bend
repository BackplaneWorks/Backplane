import Base
import ../src/core/vt.bend as V
import ../src/core/utf8.bend as U
import ../src/server/host.bend as H
import ../src/server/pty.bend as P
import ../src/server/jsloop.bend as LP

# Real shells on a real pty under the JS runtime (pty.js, host.js's
# recv/send fallback and Proc.wait), read into the emulator. Prints ok /
# FAIL lines and exits non-zero on any failure.
#   bend test/js_pty_test.bend
# The same program also builds natively (pty.c):
#   bend test/js_pty_test.bend -o build/js_pty_test && build/js_pty_test

def runs.text(rs: List<&2, V.VRun>) -> String:
  match rs:
    case Nil{}:
      ""
    case V.VRun{t, _, _, _, _} <> r:
      t ++ runs.text(r)

# the first n rows' text, joined by "|"
def lines.text(n: Nat, ls: List<&2, V.VLine>) -> String:
  match n ls:
    case 0n _:
      ""
    case _ Nil{}:
      ""
    case 1n V.VLine{rs} <> _:
      runs.text(rs)
    case 1n+p V.VLine{rs} <> r:
      runs.text(rs) ++ "|" ++ lines.text(p, r)

def screen(+v: V.Vt) -> String:
  lines.text(24n, V.Vt.lines(v, 0n))

# whether a starts with p
def starts(a: String, p: String) -> Bool:
  match a p:
    case _ SNil{}:
      True{}
    case SNil{} SCon{_, _}:
      False{}
    case SCon{Chr{x}, xt} SCon{Chr{y}, yt}:
      Bool.and(U32.is_eq(x, y), starts(xt, yt))

def has.go(a: String, hit: Bool, +p: String) -> Bool:
  match a hit:
    case _ True{}:
      True{}
    case SNil{} False{}:
      False{}
    case SCon{_, +t} False{}:
      has.go(t, starts(t, p), p)

# whether p occurs in a
def has(+a: String, +p: String) -> Bool:
  has.go(a, starts(a, p), p)

def check.k(same: Bool, name: String, got: String, want: String) -> IO(U32):
  match same:
    case True{}:
      do IO<U32>:
        IO.print("ok " ++ name)
        return 0
    case False{}:
      do IO<U32>:
        IO.print("FAIL " ++ name ++ ": got " ++ got ++ " want " ++ want)
        return 1

def check(name: String, +got: String, +want: String) -> IO(U32):
  check.k(String.eq(got, want), name, got, want)

def check.any(name: String, +got: String, +a: String, +b: String) -> IO(U32):
  check.k(Bool.or(String.eq(got, a), String.eq(got, b)), name, got, a ++ " or " ++ b)

# the rows after the first n
def rows.from(n: Nat, ls: List<&2, V.VLine>) -> List<&2, V.VLine>:
  match n ls:
    case 0n _:
      ls
    case _ Nil{}:
      Nil{}
    case 1n+p _ <> r:
      rows.from(p, r)

def check.has(name: String, +got: String, +want: String) -> IO(U32):
  check.k(has(got, want), name, got, "... " ++ want ++ " ...")

# Reading
# -------

type Rd is Data:
  Rd{v: V.Vt, n: U32}

def Rd.fed(v: V.Vt, +n: U32, +bs: List<&2, U32>) -> Rd:
  +k = U32.from_nat(List.length(&2, U32, bs))
  Rd{V.Vt.feed(v, bs), (n + k : U32)}

def Rd.count(+n: U32, +bs: List<&2, U32>) -> U32:
  (n + U32.from_nat(List.length(&2, U32, bs)) : U32)

def Rd.more(enough: Bool, s: Socket, st: Rd, k: Socket -> Rd -> IO(Socket & Rd)) -> IO(Socket & Rd):
  match enough:
    case True{}:
      IO.pure(Socket & Rd, (s, st))
    case False{}:
      k(s, st)

def Rd.data(st: Rd, +bs: List<&2, U32>, s: Socket, +want: U32, k: Socket -> Rd -> IO(Socket & Rd)) -> IO(Socket & Rd):
  match st:
    case Rd{v, +n}:
      Rd.more(U32.is_ge(Rd.count(n, bs), want), s, Rd.fed(v, n, bs), k)

def Rd.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, st: Rd, +want: U32, k: Socket -> Rd -> IO(Socket & Rd)) -> IO(Socket & Rd):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      IO.pure(Socket & Rd, (s, st))
    case Done{bs}:
      Rd.data(st, bs, s, want, k)
    case Fail{_}:
      IO.pure(Socket & Rd, (s, st))

# read into the terminal until want bytes came, or the pty hung up
def Rd.go(fuel: Nat, s: Socket, st: Rd, +want: U32) -> IO(Socket & Rd):
  match fuel:
    case 0n:
      IO.pure(Socket & Rd, (s, st))
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Socket & Rd, H.Sock.recv_bytes(s, 65536),
        r => Rd.got(r, st, want, s2 => st2 => Rd.go(f, s2, st2, want)))

def Rd.vt(st: Rd) -> V.Vt:
  match st:
    case Rd{v, _}:
      v

def Rd.close(r: Socket & Rd) -> IO(V.Vt):
  (s, st) = r
  do IO<V.Vt>:
    Socket.close(s)
    return Rd.vt(st)

def Rd.keep(r: Socket & Rd) -> Socket & V.Vt:
  (s, st) = r
  (s, Rd.vt(st))

def Rd.all(s: Socket, cols: U32, rows: U32) -> IO(V.Vt):
  IO.bind(Socket & Rd, V.Vt, Rd.go(1000n, s, Rd{V.Vt.new(cols, rows), 0}, 4000000000), Rd.close)

def sent(r: Socket & Result<&1, &1, U32 & String, Unit>) -> Socket:
  (s, _) = r
  s

def send(s: Socket, +text: String) -> IO(Socket):
  IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Socket, H.Sock.send_bytes(s, U.Utf8.encode(text)), r => IO.pure(Socket, sent(r)))

def fail(name: String, e: U32 & String) -> IO(U32):
  (code, msg) = e
  do IO<U32>:
    IO.print("FAIL " ++ name ++ " spawn: " ++ U32.show(code) ++ " " ++ msg)
    return 1

# Cases
# -----

# an interactive sh: type a line, read its output, see its exit status
def case.shell.k(started: U32 & Socket) -> IO(U32):
  (+pid, s) = started
  do IO<U32>:
    s2 : Socket <- send(s, "echo X$((40+2))Y; exit 3\n")
    +v : V.Vt <- Rd.all(s2, 80, 24)
    code : U32 <- H.Proc.wait(pid)
    a : U32 <- check.has("pty shell output", screen(v), "X42Y")
    b : U32 <- check("pty shell exit status", U32.show(code), "3")
    return (a + b : U32)

def case.shell(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(U32):
  match r:
    case Done{started}:
      case.shell.k(started)
    case Fail{e}:
      fail("pty shell", e)

# the size given at start, then a resize the child sees
def case.size.more(r: Socket & V.Vt, +pid: U32) -> IO(U32):
  (s, +v) = r
  do IO<U32>:
    s2 : Socket <- P.Pty.resize(s, 100, 30)
    s3 : Socket <- send(s2, "go\n")
    +v2 : V.Vt <- IO.bind(Socket & Rd, V.Vt, Rd.go(1000n, s3, Rd{v, 0}, 4000000000), Rd.close)
    code : U32 <- H.Proc.wait(pid)
    a : U32 <- check.has("pty start size", screen(v2), "24 80")
    b : U32 <- check.has("pty resize stty size", screen(v2), "30 100")
    c : U32 <- check("pty resize exit", U32.show(code), "0")
    return (a + b + c : U32)

def case.size.k(started: U32 & Socket) -> IO(U32):
  (+pid, s) = started
  do IO<U32>:
    r1 : Socket & V.Vt <- IO.bind(Socket & Rd, Socket & V.Vt, Rd.go(1000n, s, Rd{V.Vt.new(100, 30), 0}, 6), r => IO.pure(Socket & V.Vt, Rd.keep(r)))
    a : U32 <- case.size.more(r1, pid)
    return a

def case.size(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(U32):
  match r:
    case Done{started}:
      case.size.k(started)
    case Fail{e}:
      fail("pty resize", e)

# TERM, cwd, stdin a tty, the pty is the controlling terminal, and none of
# our other descriptors leak in (ls sees 0-2 and its own directory: 3 on
# Linux, 3 and 4 on macOS)
def case.env.k(started: U32 & Socket) -> IO(U32):
  (+pid, s) = started
  do IO<U32>:
    +v : V.Vt <- Rd.all(s, 80, 24)
    code : U32 <- H.Proc.wait(pid)
    a : U32 <- check("pty env cwd ctty", lines.text(5n, V.Vt.lines(v, 0n)), "xterm-256color|truecolor||/tmp|tty ctty")
    b : U32 <- check.any("pty no fds leak", lines.text(1n, rows.from(5n, V.Vt.lines(v, 0n))), "0 1 2 3", "0 1 2 3 4")
    return (a + b : U32)

def case.env(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(U32):
  match r:
    case Done{started}:
      case.env.k(started)
    case Fail{e}:
      fail("pty env", e)

# closing the master hangs up the shell (SIGHUP: 129)
def case.close.more(r: Socket & V.Vt, +pid: U32) -> IO(U32):
  (s, +v) = r
  do IO<U32>:
    Socket.close(s)
    code : U32 <- H.Proc.wait(pid)
    a : U32 <- check.has("pty child up", screen(v), "up")
    b : U32 <- check("pty close kills child", U32.show(code), "129")
    return (a + b : U32)

def case.close.k(started: U32 & Socket) -> IO(U32):
  (+pid, s) = started
  do IO<U32>:
    r1 : Socket & V.Vt <- IO.bind(Socket & Rd, Socket & V.Vt, Rd.go(1000n, s, Rd{V.Vt.new(80, 24), 0}, 3), r => IO.pure(Socket & V.Vt, Rd.keep(r)))
    a : U32 <- case.close.more(r1, pid)
    return a

def case.close(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(U32):
  match r:
    case Done{started}:
      case.close.k(started)
    case Fail{e}:
      fail("pty close", e)

# a hang-up sent to the shell's process group (its pid negated, as
# term.bend's Term.kill sends it) ends the command it runs too
def case.hup.more(r: Socket & V.Vt, +pid: U32) -> IO(U32):
  (s, +v) = r
  do IO<U32>:
    H.Proc.kill(((4294967295 - pid : U32) + 1 : U32), 1)
    code : U32 <- H.Proc.wait(pid)
    Socket.close(s)
    a : U32 <- check("pty group hang-up", U32.show(code), "129")
    return a

def case.hup.k(started: U32 & Socket) -> IO(U32):
  (+pid, s) = started
  do IO<U32>:
    r1 : Socket & V.Vt <- IO.bind(Socket & Rd, Socket & V.Vt, Rd.go(1000n, s, Rd{V.Vt.new(80, 24), 0}, 3), r => IO.pure(Socket & V.Vt, Rd.keep(r)))
    a : U32 <- case.hup.more(r1, pid)
    return a

def case.hup(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(U32):
  match r:
    case Done{started}:
      case.hup.k(started)
    case Fail{e}:
      fail("pty group hang-up", e)

def case.missing.k(started: U32 & Socket) -> IO(U32):
  (pid, s) = started
  do IO<U32>:
    Socket.close(s)
    check("pty missing command fails", "spawned", "ENOENT")

def case.missing.code(e: U32 & String) -> IO(U32):
  (code, _) = e
  check("pty missing command fails", U32.show(code), "2")

def case.missing(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(U32):
  match r:
    case Done{started}:
      case.missing.k(started)
    case Fail{e}:
      case.missing.code(e)

def case.badcwd.k(started: U32 & Socket) -> IO(U32):
  (pid, s) = started
  do IO<U32>:
    Socket.close(s)
    check("pty bad cwd fails", "spawned", "ENOENT")

def case.badcwd.code(e: U32 & String) -> IO(U32):
  (code, _) = e
  check("pty bad cwd fails", U32.show(code), "2")

def case.badcwd(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(U32):
  match r:
    case Done{started}:
      case.badcwd.k(started)
    case Fail{e}:
      case.badcwd.code(e)

# a big write to a program that does not read yet: whether the send parks
# until it reads (libc) or the terminal holds the rest (Bun), the loop never
# spins on it (other work runs on time) and the program reads every byte
def full.tick(c: Chan(Nat)) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(300)
    t : Nat <- IO.now()
    r : Result<&1, &1, Nat, Unit> <- Chan.send(Nat, c, t)
    return Unit{}

def full.sent(res: Result<&1, &1, U32 & String, Unit>) -> Bool:
  match res:
    case Done{_}:
      True{}
    case Fail{_}:
      False{}

def full.at(m: Maybe<&1, Nat>) -> Nat:
  match m:
    case Some{t}:
      t
    case None{}:
      4000000000n

def case.full.done(s: Socket, ok: Bool, +t0: Nat, c: Chan(Nat), +pid: U32) -> IO(U32):
  do IO<U32>:
    m : Maybe<&1, Nat> <- Chan.recv(Nat, c)
    +tick : Nat = full.at(m)
    +v : V.Vt <- Rd.all(s, 80, 24)
    code : U32 <- H.Proc.wait(pid)
    a : U32 <- check("pty full send", Bool.show(ok), "True")
    d : U32 <- check("pty full: other work ran on time meanwhile", Bool.show(Nat.is_lt(Nat.sub(tick, t0), 700n)), "True")
    e : U32 <- check.has("pty full: the program read every byte", screen(v), "20000")
    f : U32 <- check("pty full exit", U32.show(code), "0")
    return (a + d + e + f : U32)

def case.full.more(r: Socket & Result<&1, &1, U32 & String, Unit>, +t0: Nat, c: Chan(Nat), +pid: U32) -> IO(U32):
  (s, res) = r
  case.full.done(s, full.sent(res), t0, c, pid)

def case.full.k(started: U32 & Socket) -> IO(U32):
  (+pid, s) = started
  do IO<U32>:
    +c : Chan(Nat) <- Chan.new(Nat, 1)
    IO.spawn(Unit, full.tick(c))
    +t0 : Nat <- IO.now()
    r : Socket & Result<&1, &1, U32 & String, Unit> <- H.Sock.send_bytes(s, List.replicate(U32, 20000n, 97))
    case.full.more(r, t0, c, pid)

def case.full(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(U32):
  match r:
    case Done{started}:
      case.full.k(started)
    case Fail{e}:
      fail("pty full", e)

# the shell starts with a soft stack limit of 8 MiB at most (the hub's may
# be far larger), or the hub's own when that is less
def case.stack.k(started: U32 & Socket) -> IO(U32):
  (+pid, s) = started
  do IO<U32>:
    +v : V.Vt <- Rd.all(s, 80, 24)
    code : U32 <- H.Proc.wait(pid)
    a : U32 <- check.has("pty stack limit", screen(v), "stack-ok")
    return a

def case.stack(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(U32):
  match r:
    case Done{started}:
      case.stack.k(started)
    case Fail{e}:
      fail("pty stack", e)

def done.k(ok: Bool, +n: U32) -> IO(Unit):
  match ok:
    case True{}:
      IO.print("js pty: all ok")
    case False{}:
      IO.die(Unit, 1, "js pty: " ++ U32.show(n) ++ " failed")

def done(+n: U32) -> IO(Unit):
  done.k(U32.is_eq(n, 0), n)

def main() -> IO(Unit):
  do IO<Unit>:
    # the runtime's loop (loop.js) is part of the program, as in the hub:
    # on Windows nothing spawns without it
    rt : String <- LP.Rt.loop()
    a : U32 <- IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, U32, P.Pty.spawn("sh", [], "", 80, 24), case.shell)
    b : U32 <- IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, U32, P.Pty.spawn("sh", ["-c", "stty size; read x; stty size"], "", 80, 24), case.size)
    c : U32 <- IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, U32, P.Pty.spawn("sh", ["-c", "echo $TERM; echo $COLORTERM; echo $LINES$COLUMNS; pwd; [ -t 0 ] && printf tty; : </dev/tty && echo ' ctty'; echo $(ls /dev/fd/)"], "/tmp", 80, 24), case.env)
    d : U32 <- IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, U32, P.Pty.spawn("sh", ["-c", "echo up; sleep 30; echo done"], "", 80, 24), case.close)
    e : U32 <- IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, U32, P.Pty.spawn("definitely-not-a-command", [], "", 80, 24), case.missing)
    f : U32 <- IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, U32, P.Pty.spawn("sh", ["-c", "true"], "/definitely/not/a/dir", 80, 24), case.badcwd)
    g : U32 <- IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, U32, P.Pty.spawn("sh", ["-c", "echo up; sleep 30; echo done"], "", 80, 24), case.hup)
    h : U32 <- IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, U32, P.Pty.spawn("sh", ["-c", "stty raw -echo; sleep 1; head -c 20000 | wc -c"], "", 80, 24), case.full)
    i : U32 <- IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, U32, P.Pty.spawn("sh", ["-c", "c=$(ulimit -s); p=$(awk '/Max stack size/{print $4}' /proc/$PPID/limits); if [ \"$p\" = unlimited ] || [ \"$p\" -gt 8388608 ]; then w=8192; else w=$((p/1024)); fi; [ \"$c\" = \"$w\" ] && echo stack-ok || echo \"child $c hub $p\""], "", 80, 24), case.stack)
    done((a + b + c + d + e + f + g + h + i : U32))
