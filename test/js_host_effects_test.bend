import Base
import ./check.bend as C
import ../src/server/host.bend as H
import ../src/server/proc.bend as P
import ../src/server/adoptio.bend as A
import ../src/server/hublock.bend as L
import ../src/server/lockio.bend as G
import ../src/server/castio.bend as K
import ../src/server/perf.bend as F
import ../src/core/exe.bend as EX
import ../src/core/utf8.bend as U
import ../src/server/jsloop.bend as LP

# The server's host effects on the target the test runs on (JS through
# bun under `bend test/js_host_effects_test.bend`, or native): children
# (status, output, cwd, stderr modes, errtail, exited, kill, the
# descriptors and signals a child starts with), Env.put, Net.listen,
# Sock.peer and Sock.dup, a big send to a slow reader that parks (other
# computations run meanwhile), Guard, HubLock, Cast, Perf, Adopt.
#
# Running this program again (Exe.child) and Sys.exec with the hub lock and
# an adopted agent kept through it need the program to be its own file:
#   bend test/js_host_effects_test.bend -o build/js_host_effects.js
#   bun build/js_host_effects.js
#   bun build --compile build/js_host_effects.js --outfile build/js_host_effects
#   build/js_host_effects
# (in-process under `bend X.bend` the running binary is bend itself, so
# those checks report themselves skipped there).
# Everything lives in a fresh directory under /tmp.

def ran.show(r: P.Ran) -> String:
  match r:
    case P.Ran{c, o}:
      U32.show(c) ++ ":" ++ String.trim(o)

def ran.out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, o}:
      String.trim(o)

def sh(script: String) -> IO(P.Ran):
  P.Proc.run("sh", ["-c", script], "")

def sh.out(script: String) -> IO(String):
  IO.bind(P.Ran, String, sh(script), r => IO.pure(String, ran.out(r)))

def made(r: Result<&1, &1, U32 & String, Unit>, +dir: String) -> IO(String):
  match r:
    case Done{_}:
      IO.pure(String, dir)
    case Fail{(_, e)}:
      IO.die(String, 1, "js_host_effects_test cannot make " ++ dir ++ ": " ++ e)

def scratch(t: U32, n: U32) -> IO(String):
  +dir = "/tmp/bp-js-host-" ++ U32.show(t) ++ "-" ++ U32.show(n)
  IO.bind(Result<&1, &1, U32 & String, Unit>, String, H.FS.mkdirs(dir), r => made(r, dir))

def rand(r: Result<&1, &1, U32 & String, U32>) -> U32:
  match r:
    case Done{n}:
      n
    case Fail{_}:
      7

# a started child's status once it ends (its socket closed first)
def spawned.code(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(String):
  match r:
    case Fail{(c, _)}:
      IO.pure(String, "fail " ++ U32.show(c))
    case Done{(pid, s)}:
      do IO<String>:
        Socket.close(s)
        code : U32 <- H.Proc.wait(pid)
        return "started " ++ U32.show(code)

# output and status, then what errtail read back
def spawned.tail(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(String):
  match r:
    case Fail{(c, _)}:
      IO.pure(String, "fail " ++ U32.show(c))
    case Done{(+pid, s)}:
      do IO<String>:
        got : P.Ran <- P.Proc.collect((pid, s))
        tail : String <- H.Proc.errtail(pid)
        return ran.show(got) ++ "|" ++ String.trim(tail)

def spawned.ran(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(String):
  match r:
    case Fail{(c, _)}:
      IO.pure(String, "fail " ++ U32.show(c))
    case Done{started}:
      IO.bind(P.Ran, String, P.Proc.collect(started), x => IO.pure(String, ran.show(x)))

# exited before and after the reap, and the reap's code
def spawned.exited(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(String):
  match r:
    case Fail{(c, _)}:
      IO.pure(String, "fail " ++ U32.show(c))
    case Done{(+pid, s)}:
      do IO<String>:
        IO.sleep(300)
        a : U32 <- H.Proc.exited(pid)
        w : U32 <- H.Proc.wait(pid)
        b : U32 <- H.Proc.exited(pid)
        Socket.close(s)
        return U32.show(a) ++ " " ++ U32.show(w) ++ " " ++ U32.show(b)

# running, then killed (SIGTERM): 128 + 15
def spawned.killed(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(String):
  match r:
    case Fail{(c, _)}:
      IO.pure(String, "fail " ++ U32.show(c))
    case Done{(+pid, s)}:
      do IO<String>:
        a : U32 <- H.Proc.exited(pid)
        H.Proc.kill(pid, 15)
        w : U32 <- H.Proc.wait(pid)
        Socket.close(s)
        return U32.show(a) ++ " " ++ U32.show(w)

def procs(+dir: String) -> IO(Unit):
  do IO<Unit>:
    r1 : P.Ran <- sh("echo hi; exit 3")
    C.Check.str("run status and output", ran.show(r1), "3:hi")
    r2 : P.Ran <- P.Proc.run("pwd", [], dir)
    C.Check.str("run in cwd", ran.show(r2), "0:" ++ dir)
    s3 : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("/nonexistent/bp-js-x", [], "", 0)
    c3 : String <- spawned.code(s3)
    C.Check.str("spawn of a missing program fails ENOENT", c3, "fail 2")
    s4 : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("sh", ["-c", "echo out; echo oops >&2; exit 4"], "", 3)
    c4 : String <- spawned.tail(s4)
    C.Check.str("err 3 to errtail", c4, "4:out|oops")
    s5 : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("sh", ["-c", "echo a; echo b >&2"], "", 0)
    c5 : String <- spawned.ran(s5)
    C.Check.str("err 0 discards stderr", c5, "0:a")
    s6 : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("sh", ["-c", "echo a; sleep 0.05; echo b >&2"], "", 1)
    c6 : String <- spawned.ran(s6)
    C.Check.str("err 1 joins stderr to stdout", c6, "0:a\nb")
    s7 : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("sh", ["-c", "exit 7"], "", 0)
    c7 : String <- spawned.exited(s7)
    C.Check.str("exited unreaped, wait, then reaped", c7, "7 7 255")
    s8 : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("sleep", ["5"], "", 0)
    c8 : String <- spawned.killed(s8)
    C.Check.str("running, then killed by SIGTERM", c8, "4294967295 143")
    r9 : P.Ran <- sh("echo x | cat; read v; echo \"[$v]\"")
    C.Check.str("stdin closed by Proc.run", ran.show(r9), "0:x\n[]")
    H.Env.put("BP_JS_HOST_T", "v1")
    r10 : P.Ran <- sh("echo $BP_JS_HOST_T")
    C.Check.str("Env.put reaches children", ran.show(r10), "0:v1")
    return Unit{}

# a big string: s doubled n times
def big(n: Nat, +s: String) -> String:
  match n:
    case 0n:
      s
    case 1n+k:
      big(k, s ++ s)

# a computation that runs while the send is parked: the time it woke
def ticker(+ch: Chan(Nat)) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(100)
    t : Nat <- IO.now()
    ok : Result<&1, &1, Nat, Unit> <- Chan.send(Nat, ch, t)
    return Unit{}

def recv.nat(m: Maybe<&1, Nat>) -> Nat:
  match m:
    case Some{n}:
      n
    case None{}:
      0n

def sent.ok(r: Result<&1, &1, U32 & String, Unit>) -> String:
  match r:
    case Done{_}:
      "sent"
    case Fail{(c, _)}:
      "fail " ++ U32.show(c)

def bytes.text(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(String):
  (s, got) = r
  match got:
    case Done{bs}:
      do IO<String>:
        Socket.close(s)
        return String.trim(U.Utf8.decode(bs))
    case Fail{_}:
      do IO<String>:
        Socket.close(s)
        return "fail"

# 2 MiB to `sleep 0.5; wc -c` over the child's socketpair: the send waits
# for the reader, parked, so the ticker runs before it ends
def slow.sent(r: Socket & Result<&1, &1, U32 & String, Unit>, +t0: Nat, +n: Nat, +pid: U32, +ch: Chan(Nat)) -> IO(Unit):
  (s2, res) = r
  do IO<Unit>:
    +t1 : Nat <- IO.now()
    C.Check.str("big send to a slow reader", sent.ok(res), "sent")
    s3 : Socket <- H.Sock.shutdown(s2)
    got : Socket & List<&2, U32> <- P.Proc.drain(1000000n, s3, P.Chunks{Nil{}})
    done : P.Ran <- P.Proc.finish(pid, got)
    C.Check.str("slow reader got every byte", ran.show(done), "0:" ++ Nat.show(n))
    m : Maybe<&1, Nat> <- Chan.recv(Nat, ch)
    +tick : Nat = recv.nat(m)
    C.Check.str("loop ran while the send waited", Bool.show(Bool.and(Nat.is_lt(0n, tick), Nat.is_lt(tick, t1))), "True")
    C.Check.str("send waited for the reader", Bool.show(Nat.is_ge(Nat.sub(t1, t0), 300n)), "True")
    return Unit{}

def slow.peer(r: Socket & String, +pid: U32, +ch: Chan(Nat)) -> IO(Unit):
  (s, peer) = r
  do IO<Unit>:
    +payload : String = big(21n, "x")
    C.Check.str("socketpair has no peer", peer, "")
    +t0 : Nat <- IO.now()
    r2 : Socket & Result<&1, &1, U32 & String, Unit> <- H.Sock.send_text(s, payload)
    slow.sent(r2, t0, String.length(payload), pid, ch)

def slow.go(started: U32 & Socket, +ch: Chan(Nat)) -> IO(Unit):
  (pid, s) = started
  IO.bind(Socket & String, Unit, H.Sock.peer(s), p => slow.peer(p, pid, ch))

def slow.k(r: Result<&1, &1, U32 & String, U32 & Socket>, +ch: Chan(Nat)) -> IO(Unit):
  match r:
    case Fail{(c, _)}:
      C.Check.str("slow reader spawned", U32.show(c), "started")
    case Done{started}:
      slow.go(started, ch)

def slow() -> IO(Unit):
  do IO<Unit>:
    +ch : Chan(Nat) <- Chan.new(Nat, 1)
    IO.spawn(Unit, ticker(ch))
    r : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("sh", ["-c", "sleep 0.5; wc -c | tr -d ' '"], "", 0)
    slow.k(r, ch)

# a send parked on a full socketpair whose peer hangs up (the child closes
# its end after 0.3 s and lives on): it fails at once, as poll() reports the
# hang-up (select() never puts such a socket in the write set). Within 80 ms
# on the Bun loop; the sync loop's retry takes up to a second.
def hup.limit.k(bun: Bool) -> Nat:
  match bun:
    case True{}:
      380n
    case False{}:
      1500n

def hup.limit(rt: String) -> Nat:
  hup.limit.k(String.eq(rt, "bun"))

def hup.sent(r: Socket & Result<&1, &1, U32 & String, Unit>, +t0: Nat, +pid: U32, +rt: String) -> IO(Unit):
  (s, res) = r
  do IO<Unit>:
    +t1 : Nat <- IO.now()
    C.Check.str("a parked send fails when the peer hangs up", sent.ok(res), "fail 32")
    C.Check.str("it fails within " ++ Nat.show(Nat.sub(hup.limit(rt), 300n)) ++ " ms of the hang-up", Bool.show(Nat.is_lt(Nat.sub(t1, t0), hup.limit(rt))), "True")
    Socket.close(s)
    H.Proc.kill(pid, 9)
    c : U32 <- H.Proc.wait(pid)
    return Unit{}

def hup.go(started: U32 & Socket, +rt: String) -> IO(Unit):
  (pid, s) = started
  do IO<Unit>:
    +t0 : Nat <- IO.now()
    r : Socket & Result<&1, &1, U32 & String, Unit> <- H.Sock.send_text(s, big(21n, "x"))
    hup.sent(r, t0, pid, rt)

def hup.k(r: Result<&1, &1, U32 & String, U32 & Socket>, +rt: String) -> IO(Unit):
  match r:
    case Fail{(c, _)}:
      C.Check.str("hang-up child spawned", U32.show(c), "started")
    case Done{started}:
      hup.go(started, rt)

def hup() -> IO(Unit):
  do IO<Unit>:
    +rt : String <- LP.Rt.loop()
    r : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("sh", ["-c", "sleep 0.3; exec 0<&- 1>&-; sleep 5"], "", 0)
    hup.k(r, rt)

# a send parked on a full socket while another computation cuts the
# connection through its raw descriptor (Sock.raw + Fd.shut: how the hub
# drops a client whose writer waits): the send fails, at once, and the
# program runs on
def cut.later(+fd: U32) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(300)
    H.Fd.shut(fd)

def cut.sent(r: Socket & Result<&1, &1, U32 & String, Unit>, +t0: Nat, +pid: U32) -> IO(Unit):
  (s2, res) = r
  do IO<Unit>:
    +t1 : Nat <- IO.now()
    C.Check.str("a parked send cut by Fd.shut fails", Bool.show(String.starts_with(sent.ok(res), "fail")), "True")
    C.Check.str("the cut send ended at the cut", Bool.show(Bool.and(Nat.is_ge(Nat.sub(t1, t0), 200n), Nat.is_lt(Nat.sub(t1, t0), 3000n))), "True")
    Socket.close(s2)
    H.Proc.kill(pid, 15)
    code : U32 <- H.Proc.wait(pid)
    return Unit{}

def cut.raw(r: Socket & U32, +pid: U32) -> IO(Unit):
  (s, +fd) = r
  do IO<Unit>:
    IO.spawn(Unit, cut.later(fd))
    +t0 : Nat <- IO.now()
    r2 : Socket & Result<&1, &1, U32 & String, Unit> <- H.Sock.send_text(s, big(21n, "x"))
    cut.sent(r2, t0, pid)

def cut.k(r: Result<&1, &1, U32 & String, U32 & Socket>) -> IO(Unit):
  match r:
    case Fail{(c, _)}:
      C.Check.str("non-reader spawned", U32.show(c), "started")
    case Done{(+pid, s)}:
      IO.bind(Socket & U32, Unit, H.Sock.raw(s), raw => cut.raw(raw, pid))

def cut() -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, Unit, H.Proc.spawn("sleep", ["10"], "", 0), cut.k)

def dup.show(r: Result<&1, &1, U32 & String, Socket>) -> IO(String):
  match r:
    case Done{d}:
      do IO<String>:
        Socket.close(d)
        return "dup"
    case Fail{(c, _)}:
      IO.pure(String, "fail " ++ U32.show(c))

def net.recv(r: Socket & Result<&1, &1, U32 & String, Unit>, c: Socket) -> IO(Unit):
  (s3, res) = r
  do IO<Unit>:
    C.Check.str("send over the listener's socket", sent.ok(res), "sent")
    back : Socket & Result<&1, &1, U32 & String, List<&2, U32>> <- H.Sock.recv_bytes(c, 64)
    txt : String <- bytes.text(back)
    C.Check.str("client reads it", txt, "hello")
    Socket.close(s3)

def net.dup(r: Socket & Result<&1, &1, U32 & String, Socket>, c: Socket) -> IO(Unit):
  (s2, d) = r
  do IO<Unit>:
    ds : String <- dup.show(d)
    C.Check.str("Sock.dup on TCP", ds, "dup")
    r3 : Socket & Result<&1, &1, U32 & String, Unit> <- H.Sock.send_text(s2, "hello")
    net.recv(r3, c)

def net.peer(r: Socket & String, c: Socket) -> IO(Unit):
  (s1, peer) = r
  do IO<Unit>:
    C.Check.str("Sock.peer on loopback", peer, "127.0.0.1")
    r2 : Socket & Result<&1, &1, U32 & String, Socket> <- H.Sock.dup(s1)
    net.dup(r2, c)

# the accepted socket: its peer, a dup, a send the client reads
def net.accepted(r: Listener & Result<&1, &1, U32 & String, Socket>, c: Socket) -> IO(Unit):
  (lst, got) = r
  match got:
    case Fail{(e, _)}:
      do IO<Unit>:
        Socket.close(c)
        Listener.close(lst)
        C.Check.str("accept", U32.show(e), "ok")
    case Done{srv}:
      do IO<Unit>:
        Listener.close(lst)
        p : Socket & String <- H.Sock.peer(srv)
        net.peer(p, c)

def net.connected(lst: Listener, r: Result<&1, &1, U32 & String, Socket>) -> IO(Unit):
  match r:
    case Fail{(e, _)}:
      do IO<Unit>:
        Listener.close(lst)
        C.Check.str("connect", U32.show(e), "ok")
    case Done{c}:
      IO.bind(Listener & Result<&1, &1, U32 & String, Socket>, Unit, TCP.accept(lst), a => net.accepted(a, c))

def net.listened(r: Result<&1, &1, U32 & String, Listener>, +port: U32) -> IO(Unit):
  match r:
    case Fail{(e, msg)}:
      C.Check.str("Net.listen", U32.show(e) ++ " " ++ msg, "ok")
    case Done{lst}:
      do IO<Unit>:
        IO.print("ok Net.listen on 127.0.0.1")
        c : Result<&1, &1, U32 & String, Socket> <- TCP.connect("127.0.0.1", port)
        net.connected(lst, c)

def net(+port: U32) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, Listener>, Unit, H.Net.listen("127.0.0.1", port), r => net.listened(r, port))

def locks(+dir: String) -> IO(Unit):
  do IO<Unit>:
    +hub : String = dir ++ "/hub.lock"
    l1 : String <- L.HubLock.take(hub, "3999")
    C.Check.str("HubLock.take", l1, "ok")
    l2 : String <- L.HubLock.held(hub)
    C.Check.str("HubLock.held by this process", l2, "")
    l3 : String <- sh.out("flock -n " ++ hub ++ " true; echo $?")
    C.Check.str("hub lock held against others", l3, "1")
    l4 : String <- sh.out("cat " ++ hub)
    C.Check.str("hub lock file names the holder", Bool.show(String.ends_with(l4, " 3999 flock")), "True")
    H.Env.put("BACKPLANE_LOCK_DIR", dir)
    g1 : Bool <- G.Guard.take("jst", "h1", dir, "cat >/dev/null; sleep 0.2; echo done > \"$1/guard-$2\"")
    C.Check.str("Guard.take", Bool.show(g1), "True")
    g2 : Bool <- G.Guard.take("jst", "h2", dir, "cat >/dev/null")
    C.Check.str("Guard.take by another holder", Bool.show(g2), "False")
    g3 : String <- sh.out("flock -n " ++ dir ++ "/bp-jst.lock true; echo $?")
    C.Check.str("guard holds the flock", g3, "1")
    # what a child of the spawned shell inherits from it, as ls lists its
    # own descriptors (less the one it reads the listing through): listing
    # the shell's from outside raced its own pipes and saved descriptors
    fds : String <- sh.out("ls -l /proc/self/fd | awk '$NF !~ /^\\/proc\\/[0-9]+\\/fd$/ && NF > 3 {printf \"%s \", $(NF-2)}'")
    C.Check.str("a child gets only 0-2 (no lock, socket or Bun descriptor)", fds, "0 1 2")
    sp : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("cat", ["/proc/self/status"], "", 0)
    +sig : String <- spawned.ran(sp)
    C.Check.str("a child starts with no signal ignored or blocked", Bool.show(Bool.and(String.contains(sig, "SigIgn:\t0000000000000000"), String.contains(sig, "SigBlk:\t0000000000000000"))), "True")
    # (the hub may run with a large stack limit; a child starts with 8 MiB
    # at most, or the hub's own when that is less)
    stk : String <- sh.out("c=$(ulimit -s); p=$(awk '/Max stack size/{print $4}' /proc/$PPID/limits); if [ \"$p\" = unlimited ] || [ \"$p\" -gt 8388608 ]; then w=8192; else w=$((p/1024)); fi; [ \"$c\" = \"$w\" ] && echo stack-ok || echo \"child $c hub $p\"")
    C.Check.str("a child starts with a soft stack of 8 MiB at most", stk, "stack-ok")
    G.Guard.drop("jst", "h1")
    g4 : String <- sh.out("cat " ++ dir ++ "/guard-h1")
    C.Check.str("Guard.drop waits for the cleanup", g4, "done")
    g5 : String <- sh.out("flock -n " ++ dir ++ "/bp-jst.lock true; echo $?")
    C.Check.str("Guard.drop lets the flock go", g5, "0")
    return Unit{}

def ids(xs: List<&2, U32>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      U32.show(x) ++ "," ++ ids(t)

def cast() -> IO(Unit):
  do IO<Unit>:
    a : Bool <- K.Cast.seen(1, "p")
    b : Bool <- K.Cast.seen(1, "p")
    c : Bool <- K.Cast.seen(2, "p")
    C.Check.str("Cast.seen lease due once a second", Bool.show(a) ++ Bool.show(b) ++ Bool.show(c), "TrueFalseFalse")
    to : List<&2, U32> <- K.Cast.to("p")
    C.Check.str("Cast.to the page's clients", ids(to), "1,2,")
    none : List<&2, U32> <- K.Cast.to("q")
    C.Check.str("Cast.to another page", ids(none), "")
    return Unit{}

def perf(+dir: String) -> IO(Unit):
  do IO<Unit>:
    H.Env.put("BACKPLANE_LOG_FILE", dir ++ "/log")
    F.Perf.emit("js host effects log line")
    +t : String <- F.Perf.take()
    C.Check.str("Perf.take reads peak and cpu", Bool.show(Bool.and(String.contains(t, "proc.peak_kb\t1\t1\t"), String.contains(t, "proc.cpu_ms\t1\t1\t"))), "True")
    l : String <- sh.out("cat " ++ dir ++ "/log")
    C.Check.str("BACKPLANE_LOG_FILE gets the log", l, "js host effects log line")
    return Unit{}

def adopt.go(started: U32 & Socket, +dir: String) -> IO(Unit):
  (+pid, s) = started
  do IO<Unit>:
    A.Adopt.record(dir ++ "/adopt")
    s2 : Socket <- A.Adopt.keep("t1", pid, s)
    +id : String <- A.Adopt.ident(pid)
    boot : String <- sh.out("cat /proc/sys/kernel/random/boot_id")
    C.Check.str("Adopt.ident is boot id and start time", Bool.show(String.starts_with(id, boot ++ ":")), "True")
    rec : String <- sh.out("cut -f1-3 " ++ dir ++ "/adopt")
    C.Check.str("Adopt.keep writes the record", rec, "t1\t" ++ U32.show(pid) ++ "\t" ++ id)
    no : Bool <- A.Adopt.stop(pid, "x:1")
    C.Check.str("Adopt.stop refuses another identity", Bool.show(no), "False")
    yes : Bool <- A.Adopt.stop(pid, id)
    C.Check.str("Adopt.stop signals the recorded process", Bool.show(yes), "True")
    w : U32 <- H.Proc.wait(pid)
    C.Check.str("stopped by SIGTERM", U32.show(w), "143")
    A.Adopt.drop("t1", pid)
    rec2 : String <- sh.out("cat " ++ dir ++ "/adopt")
    C.Check.str("Adopt.drop empties the record", rec2, "")
    Socket.close(s2)

def adopt.k(r: Result<&1, &1, U32 & String, U32 & Socket>, +dir: String) -> IO(Unit):
  match r:
    case Fail{(c, _)}:
      C.Check.str("cat spawned", U32.show(c), "started")
    case Done{started}:
      adopt.go(started, dir)

def adopt(+dir: String) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, Unit, H.Proc.spawn("cat", [], "", 0), r => adopt.k(r, dir))

# Running this program again
# --------------------------

def copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def arg(xs: List<&2, String>, n: Nat) -> String:
  match xs n:
    case Nil{} _:
      ""
    case x <> t 0n:
      x
    case x <> t 1n+k:
      arg(t, k)

# --exec DIR: take the hub lock, keep an agent (cat), exec into --after
def exec.go(started: U32 & Socket, +dir: String) -> IO(Unit):
  (+pid, s) = started
  do IO<Unit>:
    l : String <- L.HubLock.take(dir ++ "/x.lock", "1")
    A.Adopt.record(dir ++ "/adopt")
    s2 : Socket <- A.Adopt.keep("t", pid, s)
    raw : String <- H.Sys.exe_path()
    r : Result<&1, &1, U32 & String, Unit> <- H.Sys.exec(EX.Exe.path(raw), ["--after", dir])
    Socket.close(s2)
    IO.print("exec failed " ++ l)

def exec.k(r: Result<&1, &1, U32 & String, U32 & Socket>, +dir: String) -> IO(Unit):
  match r:
    case Fail{_}:
      IO.print("exec: no cat")
    case Done{started}:
      exec.go(started, dir)

def exec.mode(+dir: String) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, Unit, H.Proc.spawn("cat", [], "", 0), r => exec.k(r, dir))

def after.sent(r: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(String):
  (s2, w) = r
  IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, String, H.Sock.recv_bytes(s2, 64), bytes.text)

def after.sock(r: Result<&1, &1, U32 & String, Socket>) -> IO(String):
  match r:
    case Fail{(c, _)}:
      IO.pure(String, "take " ++ U32.show(c))
    case Done{s}:
      IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, String, H.Sock.send_text(s, "ping\n"), after.sent)

# --after DIR: the image the exec started
def after.mode(+dir: String) -> IO(Unit):
  do IO<Unit>:
    l : String <- L.HubLock.take(dir ++ "/x.lock", "2")
    fd : String <- sh.out("cut -f4 " ++ dir ++ "/adopt")
    +pid : U32 = Maybe.default(&2, U32, U32.read(fd), 0)
    pidt : String <- sh.out("cut -f2 " ++ dir ++ "/adopt")
    +cat : U32 = Maybe.default(&2, U32, U32.read(pidt), 0)
    s : Result<&1, &1, U32 & String, Socket> <- A.Adopt.take(pid)
    echo : String <- after.sock(s)
    fds : String <- sh.out("ls -l /proc/self/fd | awk '$NF !~ /^\\/proc\\/[0-9]+\\/fd$/ && NF > 3 {printf \"%s \", $(NF-2)}'")
    w : U32 <- H.Proc.wait(cat)
    IO.print("after lock=" ++ l ++ " agent=" ++ echo ++ " fds=" ++ fds ++ " child=" ++ Bool.show(Bool.not(U32.is_eq(w, 255))))

def self.k(+exe: String, js: Bool, +dir: String) -> IO(Unit):
  match js:
    case False{}:
      IO.print("ok self-run and exec checks skipped (running inside " ++ exe ++ ")")
    case True{}:
      do IO<Unit>:
        r1 : P.Ran <- P.Proc.run(EX.Exe.child(exe), ["--child"], "")
        C.Check.str("Exe.child runs this program again", ran.show(r1), "0:child-ok")
        r2 : P.Ran <- P.Proc.run(EX.Exe.child(exe), ["--exec", dir], "")
        C.Check.str("Sys.exec keeps the hub lock and the agent", ran.show(r2), "0:after lock=ok agent=ping fds=0 1 2 child=True")
        held : String <- L.HubLock.held(dir ++ "/x.lock")
        C.Check.str("the lock goes when the exec'd image ends", held, "")
        return Unit{}

def self.at(+exe: String, +dir: String) -> IO(Unit):
  self.k(exe, Bool.not(String.ends_with(exe, "/bend")), dir)

def self(+dir: String) -> IO(Unit):
  IO.bind(String, Unit, H.Sys.exe_path(), exe => self.at(exe, dir))

def tests() -> IO(Unit):
  do IO<Unit>:
    t : U32 <- H.Clock.epoch()
    n : Result<&1, &1, U32 & String, U32> <- IO.random_u32()
    +r : U32 = rand(n)
    +dir : String <- scratch(t, r)
    procs(dir)
    slow()
    hup()
    cut()
    net((30000 + (r % 20000 : U32) : U32))
    locks(dir)
    cast()
    perf(dir)
    adopt(dir)
    self(dir)
    out : String <- sh.out("rm -rf " ++ dir ++ " && echo gone")
    C.Check.str("scratch removed", out, "gone")

def main.go(c: Bool, e: Bool, a: Bool, +dir: String) -> IO(Unit):
  match c e a:
    case True{} _ _:
      IO.print("child-ok")
    case _ True{} _:
      exec.mode(dir)
    case _ _ True{}:
      after.mode(dir)
    case _ _ _:
      tests()

def main.k(+mode: String, +dir: String) -> IO(Unit):
  main.go(String.eq(mode, "--child"), String.eq(mode, "--exec"), String.eq(mode, "--after"), dir)

def main() -> IO(Unit):
  do IO<Unit>:
    rt : String <- LP.Rt.loop()
    raw : List<String> <- IO.args()
    +args : List<&2, String> = copy(raw)
    main.k(arg(args, 1n), arg(args, 2n))
