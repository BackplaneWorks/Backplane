import Base
import ../server/server.bend as S
import ../server/host.bend as H
import ../server/kicad.bend as K
import ../server/proc.bend as P
import ../server/modelcache.bend as CP
import ../core/board.bend as B
import ../core/pcb3d.bend as P3

# Where a board's 3D models are on this machine, and their GLBs: a model
# path's ${VARS} filled in (KIPRJMOD, KiCad's model library), STEP taken in
# place of VRML as kicad-cli's --subst-models does, models embedded in the
# board written out once, and each file converted by backplane-step2glb
# into a cache keyed by its path, size and time.

def Parts.step2glb() -> IO(String):
  do IO<String>:
    +d : String <- S.Sys.exe_dir.io()
    ok : Bool <- K.Io.is_file(d ++ "/backplane-step2glb")
    return Bool.pick(String, ok, d ++ "/backplane-step2glb", "backplane-step2glb")

# where converted models are kept
def Parts.cache() -> IO(String):
  do IO<String>:
    +home : String <- S.Env.get("HOME", "/tmp")
    xdg : String <- S.Env.get("XDG_CACHE_HOME", home ++ "/.cache")
    return xdg ++ "/backplane-bend/models"

def Parts.lib.k(ok: Bool, +c: String, k: IO(String)) -> IO(String):
  match ok:
    case True{}:
      IO.pure(String, c)
    case False{}:
      k

# KiCad's 3D model library: the first of the usual places that exists
def Parts.lib.go(cs: List<&2, String>) -> IO(String):
  match cs:
    case Nil{}:
      IO.pure(String, "/usr/share/kicad/3dmodels")
    case +c <> t:
      IO.bind(Result<&1, &1, U32 & String, U32 & U32 & U32>, String, H.FS.stat(c), r => Parts.lib.k(Result.is_done(&1, &1, U32 & String, U32 & U32 & U32, r), c, Parts.lib.go(t)))

def Parts.var(+name: String, +lib: String) -> IO(P3.KV):
  IO.bind(String, P3.KV, S.Env.get(name, lib), v => IO.pure(P3.KV, P3.KV{name, v}))

# the path variables a model path may use
def Parts.vars(+dir: String) -> IO(List<&2, P3.KV>):
  do IO<List<&2, P3.KV>>:
    +lib : String <- Parts.lib.go(["/usr/share/kicad/3dmodels", "/usr/local/share/kicad/3dmodels", "/opt/homebrew/share/kicad/3dmodels",
      "/Applications/KiCad/KiCad.app/Contents/SharedSupport/3dmodels"])
    a : P3.KV <- Parts.var("KICAD10_3DMODEL_DIR", lib)
    b : P3.KV <- Parts.var("KICAD9_3DMODEL_DIR", lib)
    c : P3.KV <- Parts.var("KICAD8_3DMODEL_DIR", lib)
    d : P3.KV <- Parts.var("KICAD7_3DMODEL_DIR", lib)
    e : P3.KV <- Parts.var("KICAD6_3DMODEL_DIR", lib)
    f : P3.KV <- Parts.var("KISYS3DMOD", lib)
    return [P3.KV{"KIPRJMOD", dir}, a, b, c, d, e, f]

def Parts.stem(+p: String) -> String:
  String.take(p, Nat.sub(String.length(p), 4n))

# where a model may be, best first: STEP in place of VRML (as kicad-cli's
# --subst-models does), relative paths from the board's folder
def Parts.cands(+p: String, +dir: String) -> List<&2, String>:
  +abs = Bool.pick(String, String.starts_with(p, "/"), p, dir ++ "/" ++ p)
  Bool.pick(List<&2, String>, Bool.or(String.ends_with(p, ".wrl"), String.ends_with(p, ".WRL")),
    [Parts.stem(abs) ++ ".step", Parts.stem(abs) ++ ".stp", Parts.stem(abs) ++ ".STEP", Parts.stem(abs) ++ ".STP"],
    [abs])

def Parts.first.k(ok: Bool, +c: String, k: IO(String)) -> IO(String):
  match ok:
    case True{}:
      IO.pure(String, c)
    case False{}:
      k

# the first candidate that is a file ("" for none)
def Parts.first(cs: List<&2, String>) -> IO(String):
  match cs:
    case Nil{}:
      IO.pure(String, "")
    case +c <> t:
      IO.bind(Bool, String, K.Io.is_file(c), ok => Parts.first.k(ok, c, Parts.first(t)))

def Str.hash(s: String, +h: U32) -> U32:
  match s:
    case SNil{}:
      h
    case SCon{Chr{+c}, t}:
      Str.hash(t, (U32.xor(h, c) * 16777619 : U32))

# the base64 of the board's last (file (name NAME) ... (data |...|)) (the
# board's own list, after the footprints' references), through base64
# and zstd into a STEP file
def Parts.embed.awk() -> String:
  "index($0, \"(name \\\"\" n \"\\\")\") { f = 1; b = \"\"; next } " ++
  "/\\(name \"/ { f = 0 } " ++
  "f && !d && /\\(data \\|/ { d = 1; sub(/.*\\(data \\|/, \"\") } " ++
  "d { if (index($0, \"|\")) { sub(/\\|.*/, \"\"); b = b $0; d = 0; f = 0; last = b; next } b = b $0 } " ++
  "END { printf \"%s\", last }"

def Parts.embed.run(have: Bool, +board: String, +name: String, +out: String) -> IO(String):
  match have:
    case True{}:
      IO.pure(String, out)
    case False{}:
      do IO<String>:
        r : P.Ran <- P.Proc.run("sh", ["-c", "awk -v n=\"$3\" \"$4\" \"$1\" | tr -d ' \\t\\r\\n' | base64 -d | zstd -dqf -o \"$2.part\" && mv \"$2.part\" \"$2\"",
          "sh", board, out, name, Parts.embed.awk()], "/")
        ok : Bool <- K.Io.is_file(out)
        return Bool.pick(String, ok, out, "")

# a model inside the board file, written out as STEP once (kept by its
# name and checksum, so it is not looked for in the board again)
def Parts.embed(+board: String, +name: String, +sum: String, +cache: String) -> IO(String):
  do IO<String>:
    +out : String = cache ++ "/e" ++ U32.show(Str.hash(name ++ "#" ++ Bool.pick(String, String.is_empty(sum), board, sum), 2166136261)) ++ ".step"
    have : Bool <- K.Io.is_file(out)
    Parts.embed.run(have, board, name, out)

def Parts.found.k(none: Bool, +found: String, +board: String, +name: String, +sum: String, +cache: String) -> IO(String):
  match none:
    case True{}:
      Parts.embed(board, name, sum, cache)
    case False{}:
      IO.pure(String, found)

def Parts.found(+f: String, +embed: Bool, +board: String, +name: String, +sum: String, +cache: String) -> IO(String):
  Parts.found.k(Bool.and(embed, String.is_empty(f)), f, board, name, sum, cache)

def Parts.hash_at(s: String, +i: Nat, +at: Nat, seen: Bool) -> Nat:
  match s seen:
    case SNil{} True{}:
      at
    case SNil{} False{}:
      i
    case SCon{Chr{+c}, t} _:
      Parts.hash_at(t, Nat.add(i, 1n), Bool.pick(Nat, U32.is_eq(c, 35), i, at), Bool.or(seen, U32.is_eq(c, 35)))

# "kicad-embed://NAME#SUM" split at the last "#"
def Parts.cut(+s: String) -> String & String:
  +i = Parts.hash_at(s, 0n, 0n, False{})
  (String.take(s, i), String.drop(s, Nat.add(i, 1n)))

def Parts.file.k(ns: String & String, +board: String, +dir: String, +cache: String) -> IO(String):
  (+name, +sum) = ns
  IO.bind(String, String, Parts.first([dir ++ "/" ++ name]), f => Parts.found(f, True{}, board, name, sum, cache))

def Parts.file.e(embed: Bool, +p: String, +board: String, +dir: String, +cache: String) -> IO(String):
  match embed:
    case True{}:
      Parts.file.k(Parts.cut(String.drop(p, 14n)), board, dir, cache)
    case False{}:
      Parts.first(Parts.cands(p, dir))

# the model file a board names, as a path on this machine ("" if
# missing); an embedded one is its copy in the project, else extracted
def Parts.file(+raw: String, +board: String, +dir: String, +vars: List<&2, P3.KV>, +cache: String) -> IO(String):
  +p = P3.Env.sub(raw, vars)
  Parts.file.e(String.starts_with(p, "kicad-embed://"), p, board, dir, cache)

def Parts.size.m(sm: U32 & U32) -> String:
  (sz, mt) = sm
  U32.show(sz) ++ "." ++ U32.show(mt)

def Parts.size.k(v: U32 & U32 & U32) -> String:
  (_, sm) = v
  Parts.size.m(sm)

def Parts.size(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> String:
  match r:
    case Done{v}:
      Parts.size.k(v)
    case Fail{_}:
      "0"

def Parts.conv.done(r: P.Ran, out: String) -> String:
  match r:
    case P.Ran{0, _}: out
    case P.Ran{_, _}: ""

def Parts.conv(have: Bool, +src: String, +out: String, +step: String) -> IO(String):
  match have:
    case True{}:
      IO.pure(String, out)
    case False{}:
      do IO<String>:
        published : P.Ran <- CP.ModelCache.convert(out, from => P.Proc.run(step, [src, from], "/"))
        return Parts.conv.done(published, out)

# where the model's GLB is kept (a .glb is its own)
def Parts.out(+src: String, +cache: String) -> IO(String):
  do IO<String>:
    st : Result<&1, &1, U32 & String, U32 & U32 & U32> <- H.FS.stat(src)
    return Bool.pick(String, String.ends_with(src, ".glb"), src, cache ++ "/" ++ U32.show(Str.hash("step1|" ++ src ++ "|" ++ Parts.size(st), 2166136261)) ++ ".glb")

# the model's GLB when it is ready to read, else ""
def Parts.cached(+src: String, +cache: String) -> IO(String):
  do IO<String>:
    +out : String <- Parts.out(src, cache)
    have : Bool <- K.Io.is_file(out)
    return Bool.pick(String, Bool.and(have, Bool.not(String.is_empty(src))), out, "")

# the model's GLB: from the cache, else converted into it
def Parts.glb(+src: String, +cache: String, +step: String) -> IO(String):
  do IO<String>:
    +out : String <- Parts.out(src, cache)
    have : Bool <- K.Io.is_file(out)
    Parts.conv(have, src, out, step)

def Parts.dir(+path: String) -> String:
  String.join(List.reverse(&2, String, List.drop(&2, String, List.reverse(&2, String, String.split(path, '/')), 1n)), "/")
