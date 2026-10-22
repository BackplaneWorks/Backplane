import Base
import ./check.bend as C
import ../src/server/server.bend as S

# What a boot does with the store it loaded (S3, 0.12.1): use it, build
# store.db (a child: --store-rebuild) while builds are left, then stop; the
# whole replay only behind the developer's escape (BACKPLANE_HOT_BOOT=0).
# Law boot_next_never_cold states the last for every input.

def show(n: S.BootNext) -> String:
  match n:
    case S.BootUse{}: "use"
    case S.BootBuild{}: "build"
    case S.BootStop{}: "stop"
    case S.BootCold{}: "cold"

def journal.status(r: Result<&1, &1, U32 & String, Unit>) -> String:
  match r:
    case Done{_}: "ok"
    case Fail{(code, why)}: "fail " ++ U32.show(code) ++ ":" ++ why

def journal.out(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> String:
  match r:
    case Done{(code, (out, _))}: U32.show(code) ++ ":" ++ out
    case Fail{(_, why)}: "fail " ++ why

def journal.dir(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(0, (out, _))}: IO.pure(String, String.trim(out))
    case _: IO.die(String, 1, "coldboot_test: mktemp failed")

# Exercise the same lock-owned boundary as Main.boot. A missing journal
# is created, existing bytes survive, and open errors are never success.
def journal.fixture() -> IO(Unit):
  do IO<Unit>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- journal.dir(made)
    +held : String <- S.Lock.take(dir, 3799)
    S.Lock.refuse(String.is_empty(held), held, dir)
    fresh : Result<&1, &1, U32 & String, Unit> <- S.Boot.journal(dir)
    C.Check.str("fresh home initializes journal", journal.status(fresh), "ok")
    empty : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("cat", [dir ++ "/events.jsonl"], "", 4096, 10000)
    C.Check.str("fresh journal exists and is empty", journal.out(empty), "0:")
    +text : String = "existing journal\nincluding a torn tail"
    seed : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("sh", ["-c", "printf '%s' \"$2\" > \"$1\"", "sh", dir ++ "/events.jsonl", text], "", 4096, 10000)
    C.Check.str("seed journal", journal.out(seed), "0:")
    again : Result<&1, &1, U32 & String, Unit> <- S.Boot.journal(dir)
    C.Check.str("existing journal opens", journal.status(again), "ok")
    kept : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("cat", [dir ++ "/events.jsonl"], "", 4096, 10000)
    C.Check.str("existing journal bytes unchanged", journal.out(kept), "0:" ++ text)
    missing : Result<&1, &1, U32 & String, Unit> <- S.Boot.journal(dir ++ "/missing-parent")
    C.Check.str("missing parent is an error", Bool.show(String.starts_with(journal.status(missing), "fail ")), "True")
    denied : Result<&1, &1, U32 & String, Unit> <- S.Boot.journal.opened(Fail{(13, "denied")})
    C.Check.str("open errors propagate unchanged", journal.status(denied), "fail 13:denied")
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a usable store is used", show(S.Boot.decide(False{}, True{}, 2n)), "use")
    C.Check.str("a usable store after a build is used", show(S.Boot.decide(False{}, True{}, 1n)), "use")
    C.Check.str("a usable store with no build left is used", show(S.Boot.decide(False{}, True{}, 0n)), "use")
    C.Check.str("no usable store: build", show(S.Boot.decide(False{}, False{}, 2n)), "build")
    C.Check.str("a failed build: build once more", show(S.Boot.decide(False{}, False{}, 1n)), "build")
    C.Check.str("two failed builds: stop", show(S.Boot.decide(False{}, False{}, 0n)), "stop")
    C.Check.str("the escape replays the whole log", show(S.Boot.decide(True{}, False{}, 0n)), "cold")
    C.Check.str("two builds at most", Nat.show(S.Boot.builds()), "2")
    C.Check.str("the child's JS heap is capped", String.join(S.Boot.child.env(), " "), "BUN_JSC_forceRAMSize=671088640 BUN_JSC_maxPerThreadStackUsage=536870912")
    C.Check.str("the last line of a child's output", S.Boot.ran.last("store-rebuild progress 1 2 3\nstore built lines=5 applied_off=9\n"), "store built lines=5 applied_off=9")
    journal.fixture()
