import Base

# One hub per home (pure). A hub holds an exclusive flock on
# <home>/hub.lock for as long as its process lives (effects/hublock.js):
# the kernel lets it go when the process dies, so a stale file never blocks
# anyone, and an exec (a restart in place) keeps it. The file says the
# holder's pid and port. HubLock.take answers "ok" (this process holds it),
# "held <pid> <port>" or "fail <why>". Laws onehub_*; test test/cli_test.bend.

type HubGot is Data:
  HubOurs{}
  HubHeld{pid: String, port: String}
  HubFail{why: String}

def OneHub.or(+w: String) -> String:
  Bool.pick(String, String.is_empty(w), "?", w)

def OneHub.word(+s: String, +i: Nat) -> String:
  OneHub.or(Maybe.default(&2, String, List.get(&2, String, String.split(String.trim(s), ' '), i), ""))

def OneHub.held(+rest: String) -> HubGot:
  HubHeld{OneHub.word(rest, 0n), OneHub.word(rest, 1n)}

def OneHub.fail(+r: String) -> HubGot:
  HubFail{Bool.pick(String, String.starts_with(r, "fail "), String.drop(r, 5n), r)}

def OneHub.got.k(held: Bool, +r: String) -> HubGot:
  match held:
    case True{}:
      OneHub.held(String.drop(r, 5n))
    case False{}:
      OneHub.fail(r)

def OneHub.got(+r: String) -> HubGot:
  Bool.pick(HubGot, String.eq(r, "ok"), HubOurs{}, OneHub.got.k(String.starts_with(r, "held "), r))

# what the file holds: "<pid> <port>"
def OneHub.line(+pid: String, +port: String) -> String:
  pid ++ " " ++ port

# what a second hub says before it ends
def OneHub.says(+home: String, +pid: String, +port: String) -> String:
  "a hub already serves " ++ home ++ " (pid " ++ pid ++ ", port " ++ port ++ ")"

# the holder as "<pid> <port>" ("" when this process holds it)
def OneHub.holder(g: HubGot) -> String:
  match g:
    case HubOurs{}:
      ""
    case HubHeld{+p, +n}:
      OneHub.line(p, n)
    case HubFail{_}:
      ""

# A holder as a probe or the lock file says it: "<pid> <port>" (digits),
# optionally followed by " flock". A shell's own noise (Cygwin bash's
# "could not find /tmp", a locale warning) is no holder: taken for one, it
# kept a Windows hub from starting on a fresh home (laws onehub_legacy_*)
def OneHub.digits.go(s: String, ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{Chr{+c}, t}:
      OneHub.digits.go(t, Bool.and(ok, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))))

def OneHub.digits(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), OneHub.digits.go(s, True{}))

def OneHub.pair.k(ws: List<&2, String>) -> String:
  match ws:
    case +p <> +n <> Nil{}:
      Bool.pick(String, Bool.and(OneHub.digits(p), OneHub.digits(n)), p ++ " " ++ n, "")
    case +p <> +n <> "flock" <> Nil{}:
      Bool.pick(String, Bool.and(OneHub.digits(p), OneHub.digits(n)), p ++ " " ++ n, "")
    case _:
      ""

# "<pid> <port>" when a line says a holder, else ""
def OneHub.pair(+line: String) -> String:
  OneHub.pair.k(String.split(String.trim(line), ' '))

def OneHub.legacy.pick(+p: String, +acc: String) -> String:
  Bool.pick(String, String.is_empty(p), acc, p)

def OneHub.legacy.go(ls: List<&2, String>, +acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      OneHub.legacy.go(t, OneHub.legacy.pick(OneHub.pair(l), acc))

# the holder a probe's output names: its last line that is "<pid> <port>",
# else "" (anything else the shell printed is noise, not a holder)
def OneHub.legacy(+out: String) -> String:
  OneHub.legacy.go(String.split(out, '\n'), "")
