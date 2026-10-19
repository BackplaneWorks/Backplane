import Base
import ../check.bend as C
import ../../src/server/proc.bend as P
import ../../src/server/host.bend as H
import ../../src/server/modelcache.bend as M

# Real publication effects in a temporary folder, without a CAD install.
def result(r: P.Ran) -> String:
  match r:
    case P.Ran{c, text}: U32.show(c) ++ ":" ++ text

def check(name: String, +got: String, +want: String) -> IO(Bool):
  do IO<Bool>:
    IO.print(C.Check.pick(String.eq(got, want), name, got, want))
    return String.eq(got, want)

def shell(cwd: String, name: String, script: String) -> IO(Bool):
  IO.bind(P.Ran, Bool, P.Proc.run("sh", ["-c", "set -eu\n" ++ script], cwd), r => check(name, result(r), "0:"))

def all(bs: List<&2, Bool>) -> Bool:
  match bs:
    case Nil{}: True{}
    case b <> t: Bool.and(b, all(t))

def finish(ok: Bool) -> IO(Unit):
  match ok:
    case True{}: IO.print("all ok")
    case False{}: IO.die(Unit, 1, "model_cache_test failed")

def Cases.failed(+root: String) -> IO(Bool):
  do IO<Bool>:
    a : Bool <- shell(root, "failed converter fixture initializes", "printf truncated > failed.part")
    r : P.Ran <- M.ModelCache.publish(P.Ran{23, "converter failed"}, root ++ "/failed.part", root ++ "/failed.glb")
    b : Bool <- check("failed converter keeps its status and diagnostic", result(r), "23:converter failed")
    c : Bool <- shell(root, "failed converter cannot publish a cache entry", "test ! -e failed.glb\ntest ! -e failed.part")
    d : Bool <- shell(root, "previous cache fixture initializes", "printf previous > previous.glb\nprintf truncated > previous.part")
    again : P.Ran <- M.ModelCache.publish(P.Ran{23, "converter failed"}, root ++ "/previous.part", root ++ "/previous.glb")
    e : Bool <- check("failed replacement keeps converter status", result(again), "23:converter failed")
    f : Bool <- shell(root, "failed replacement preserves previous complete cache", "test \"$(cat previous.glb)\" = previous\ntest ! -e previous.part")
    return all([a, b, c, d, e, f])

def Cases.success(+root: String) -> IO(Bool):
  do IO<Bool>:
    a : Bool <- shell(root, "successful converter fixture initializes", "printf complete > ready.part")
    r : P.Ran <- M.ModelCache.publish(P.Ran{0, "converter note"}, root ++ "/ready.part", root ++ "/ready.glb")
    b : Bool <- check("successful publication preserves converter diagnostic", result(r), "0:converter note")
    c : Bool <- shell(root, "successful cache contains complete staged bytes", "test \"$(cat ready.glb)\" = complete\ntest ! -e ready.part")
    d : Bool <- shell(root, "replacement fixture initializes", "printf newer > ready.part")
    again : P.Ran <- M.ModelCache.publish(P.Ran{0, ""}, root ++ "/ready.part", root ++ "/ready.glb")
    e : Bool <- check("successful replacement succeeds", result(again), "0:")
    f : Bool <- shell(root, "successful replacement atomically changes cache", "test \"$(cat ready.glb)\" = newer\ntest ! -e ready.part")
    return all([a, b, c, d, e, f])

def Cases.rename(+root: String) -> IO(Bool):
  do IO<Bool>:
    a : Bool <- shell(root, "blocked cache fixture initializes", "mkdir blocked.glb\nprintf keep > blocked.glb/keep\nprintf complete > blocked.part")
    r : P.Ran <- M.ModelCache.publish(P.Ran{0, ""}, root ++ "/blocked.part", root ++ "/blocked.glb")
    b : Bool <- check("rename failure becomes a failed export", String.take(result(r), 1n), "1")
    c : Bool <- shell(root, "rename failure preserves destination", "test \"$(cat blocked.glb/keep)\" = keep")
    missing : P.Ran <- M.ModelCache.publish(P.Ran{0, ""}, root ++ "/missing.part", root ++ "/missing.glb")
    d : Bool <- check("missing converter output becomes a failed export", String.take(result(missing), 1n), "1")
    e : Bool <- shell(root, "missing output cannot claim a cache file", "test ! -e missing.glb")
    f : Bool <- shell(root, "empty converter output fixture initializes", "printf '' > empty.part")
    empty : P.Ran <- M.ModelCache.publish(P.Ran{0, ""}, root ++ "/empty.part", root ++ "/empty.glb")
    g : Bool <- check("empty converter output becomes a failed export", String.take(result(empty), 1n), "1")
    h : Bool <- shell(root, "empty converter output cannot poison the cache", "test ! -e empty.glb\ntest ! -e empty.part")
    return all([a, b, c, d, e, f, g, h])

def run(+root: String) -> IO(Unit):
  do IO<Unit>:
    H.Env.put("HOME", root)
    H.Env.put("BACKPLANE_HOME", root ++ "/home")
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    linked : Bool <- check("host effect linkage frame", Nat.show(List.length(&2, U32, frame)), "3")
    a : Bool <- Cases.failed(root)
    b : Bool <- Cases.success(root)
    c : Bool <- Cases.rename(root)
    removed : P.Ran <- P.Proc.run("rm", ["-rf", root], "/tmp")
    finish(all([linked, a, b, c]))

def created(r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{0, root}: run(String.trim(root))
    case P.Ran{_, error}: IO.die(Unit, 1, "cannot create test directory: " ++ error)

def main() -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("mktemp", ["-d", "/tmp/bp-model-cache.XXXXXX"], "/tmp"), created)
