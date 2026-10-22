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
