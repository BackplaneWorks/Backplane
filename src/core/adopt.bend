import Base
import ./json.bend as J
import ./cmds.bend as CM

# Agents kept through a restart (pure)
# ====================================
# A restart in place (a deploy or an update: the hub execs its new image)
# used to leave each Claude process running its turn on a dead stdout
# while the new image resumed the same session: two processes on one
# thread. Now the hub keeps each one it started in a record (the server's
# effects/adopt.js writes <home>/agents.pids: thread, pid, start identity,
# a descriptor on its socket), and before the exec writes a handoff
# (<home>/agents.handoff: each Claude agent's generation, key and command
# ledger) and keeps those descriptors open across it. The new image reads
# both and decides here, for each recorded process:
#
# - Adopt: it was handed off (its descriptor came through the exec), it
#   still runs as the same process (same start identity), adoption is
#   allowed, and no earlier entry adopted its thread: the hub reads and
#   writes it again as its own, and recovery leaves its thread alone.
# - Stop: the same process still runs but cannot be adopted (a crash
#   restart, adoption off, a second entry for the thread): it is signalled,
#   that pid only, before anything starts an agent for the thread.
# - Leave: it is gone, or the pid now names another process (the start
#   identity differs), or the pid is 0 or 1: nothing is signalled.
#
# So no thread ever has two agent processes (laws adopt_*).

type AEntry is Data:
  AEntry{thread: String, pid: Nat, ident: String, fd: Nat, handed: Bool, gen: String, key: String, cmds: CM.Cmds}

type AVerdict is Data:
  VAdopt{e: AEntry}
  VStop{e: AEntry}
  VLeave{e: AEntry}

def AEntry.thread(e: AEntry) -> String:
  match e:
    case AEntry{t, _, _, _, _, _, _, _}:
      t

def AEntry.pid(e: AEntry) -> Nat:
  match e:
    case AEntry{_, p, _, _, _, _, _, _}:
      p

def AEntry.ident(e: AEntry) -> String:
  match e:
    case AEntry{_, _, i, _, _, _, _, _}:
      i

# Reading
# -------

def Adopt.digits(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Adopt.digits(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))))

def Adopt.num.k(ok: Bool, +s: String) -> Nat:
  match ok:
    case True{}:
      Adopt.digits(s, 0n)
    case False{}:
      0n

def Adopt.isdigits(s: String, ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{Chr{+c}, t}:
      Adopt.isdigits(t, Bool.and(ok, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))))

# a field of digits, else 0
def Adopt.num(+s: String) -> Nat:
  Adopt.num.k(Bool.and(Bool.not(String.is_empty(s)), Adopt.isdigits(s, True{})), s)

def Adopt.field(xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

# the handoff's line for a thread and pid (a JSON object), or null
def Adopt.hand.find(hs: List<&2, J.Json>, +thread: String, +pid: Nat) -> J.Json:
  match hs:
    case Nil{}:
      J.Null{}
    case +h <> t:
      Bool.pick(J.Json, Bool.and(String.eq(J.Json.get_str(h, "thread", ""), thread), Nat.is_eq(J.Json.get_nat(h, "pid", 0n), pid)), h, Adopt.hand.find(t, thread, pid))

def Adopt.has.k(o: J.Json) -> Bool:
  match o:
    case J.Null{}:
      False{}
    case _:
      True{}

def Adopt.entry.k(+thread: String, +pid: Nat, +ident: String, +fd: Nat, +h: J.Json) -> AEntry:
  AEntry{thread, pid, ident, fd, Adopt.has.k(h), J.Json.get_str(h, "gen", ""), J.Json.get_str(h, "key", ""), CM.Cmds.read(J.Json.get_obj(h, "cmds"))}

def Adopt.entry(+fs: List<&2, String>, +hs: List<&2, J.Json>) -> AEntry:
  Adopt.entry.k(Adopt.field(fs, 0n), Adopt.num(Adopt.field(fs, 1n)), Adopt.field(fs, 2n), Adopt.num(Adopt.field(fs, 3n)),
    Adopt.hand.find(hs, Adopt.field(fs, 0n), Adopt.num(Adopt.field(fs, 1n))))

def Adopt.lines.go(ls: List<&2, String>, +hs: List<&2, J.Json>) -> List<&2, AEntry>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      List.append(&2, AEntry, Bool.pick(List<&2, AEntry>, String.is_empty(String.trim(l)), Nil{}, [Adopt.entry(String.split(l, '\t'), hs)]), Adopt.lines.go(t, hs))

def Adopt.hands(ls: List<&2, String>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      List.append(&2, J.Json, Bool.pick(List<&2, J.Json>, String.is_empty(String.trim(l)), Nil{}, [Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{})]), Adopt.hands(t))

# the record (thread TAB pid TAB start identity TAB descriptor, a line each)
# joined with the handoff (a JSON object a line)
def Adopt.entries(+record: String, +handoff: String) -> List<&2, AEntry>:
  Adopt.lines.go(String.split(record, '\n'), Adopt.hands(String.split(handoff, '\n')))

# Writing the handoff
# -------------------

def Adopt.hand.line(+thread: String, +pid: Nat, +gen: String, +key: String, c: CM.Cmds) -> String:
  J.Json.show(J.Json.obj([J.KV{"thread", J.Str{thread}}, J.KV{"pid", J.Json.nat(pid)}, J.KV{"gen", J.Str{gen}}, J.KV{"key", J.Str{key}}, J.KV{"cmds", CM.Cmds.json(c)}]))

# Deciding
# --------

def Adopt.in(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +y <> t:
      Bool.or(String.eq(x, y), Adopt.in(t, x))

def Adopt.one.k(same: Bool, can: Bool, e: AEntry) -> AVerdict:
  match same can:
    case False{} _:
      VLeave{e}
    case True{} True{}:
      VAdopt{e}
    case True{} False{}:
      VStop{e}

# one entry: now is its pid's start identity as read now ("" when gone),
# taken the threads adopted before it
def Adopt.one(e: AEntry, +now: String, +allow: Bool, +taken: List<&2, String>) -> AVerdict:
  match e:
    case AEntry{+t, +p, +i, +fd, +hd, +g, +k, c}:
      Adopt.one.k(Bool.and(Nat.is_gt(p, 1n), Bool.and(Bool.not(String.is_empty(i)), String.eq(now, i))),
        Bool.and(Bool.and(hd, allow), Bool.and(Nat.is_gt(fd, 2n), Bool.not(Adopt.in(taken, t)))), AEntry{t, p, i, fd, hd, g, k, c})

def Adopt.took(v: AVerdict, taken: List<&2, String>) -> List<&2, String>:
  match v:
    case VAdopt{e}:
      AEntry.thread(e) <> taken
    case _:
      taken

def Adopt.go(es: List<&2, AEntry>, nows: List<&2, String>, +allow: Bool, +taken: List<&2, String>, acc: List<&2, AVerdict>) -> List<&2, AVerdict>:
  match es nows:
    case +e <> t +n <> nt:
      Adopt.go(t, nt, allow, Adopt.took(Adopt.one(e, n, allow, taken), taken), Adopt.one(e, n, allow, taken) <> acc)
    case _ _:
      List.reverse(&2, AVerdict, acc)

# every entry's verdict, in order (nows: each pid's start identity now)
def Adopt.decide(es: List<&2, AEntry>, nows: List<&2, String>, +allow: Bool) -> List<&2, AVerdict>:
  Adopt.go(es, nows, allow, Nil{}, Nil{})

# the threads adopted
def Adopt.kept(vs: List<&2, AVerdict>) -> List<&2, String>:
  match vs:
    case Nil{}:
      Nil{}
    case VAdopt{e} <> t:
      AEntry.thread(e) <> Adopt.kept(t)
    case _ <> t:
      Adopt.kept(t)

# the pids signalled
def Adopt.stopped(vs: List<&2, AVerdict>) -> List<&2, Nat>:
  match vs:
    case Nil{}:
      Nil{}
    case VStop{e} <> t:
      AEntry.pid(e) <> Adopt.stopped(t)
    case _ <> t:
      Adopt.stopped(t)

# whether a list of names has one twice
def Adopt.dup(xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> +t:
      Bool.or(Adopt.in(t, x), Adopt.dup(t))

# Examples the laws read
# ----------------------

def Adopt.demo.record() -> String:
  "t1\t42\tb:100\t9\nt1\t43\tb:200\t10\nt2\t50\tb:300\t11\nt3\t0\t\t12\nt4\t60\tb:400\t13\n"

def Adopt.demo.handoff() -> String:
  Adopt.hand.line("t1", 42n, "g1", "k1", CM.Cmds.demo.kept()) ++ "\n" ++ Adopt.hand.line("t1", 43n, "g2", "k2", CM.Cmds.new()) ++ "\n" ++ Adopt.hand.line("t2", 50n, "g3", "k3", CM.Cmds.new()) ++ "\n"

# t1 twice (both live), t2 now another process, t3 pid 0, t4 live but
# not handed off (a crash restart)
def Adopt.demo.nows() -> List<&2, String>:
  ["b:100", "b:200", "b:999", "", "b:400"]

def Adopt.demo(+allow: Bool) -> List<&2, AVerdict>:
  Adopt.decide(Adopt.entries(Adopt.demo.record(), Adopt.demo.handoff()), Adopt.demo.nows(), allow)
