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
    C.Check.str("the last line of a child's output", S.Boot.ran.last("store-rebuild progress 1 2 3\\nstore built lines=5 applied_off=9\\n"), "store built lines=5 applied_off=9")
