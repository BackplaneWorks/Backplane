import Base
import ../core/exe.bend as EX
import ./host.bend as H
import ./kicad.bend as K
import ../core/mech.bend as MC
import ./proc.bend as PR
import ../core/board.bend as B
import ../core/schematic.bend as SC
import ../core/plot.bend as P
import ../core/glb.bend as G
import ../core/sexpr.bend as SX
import ../core/utf8.bend as U
import ../core/eq.bend as Q
import ../core/json.bend as J
import ../core/hier.bend as HI
import ../core/hist.bend as HS
import ../core/look.bend as LO
import ./git.bend as GT

# The phones' viewer. One actor owns every board, schematic or 3D model a
# phone watches and what each phone holds of it. It polls the files (every
# 150 ms, and only while one is watched), parses a changed one off the
# actor (in parallel, core/plot.bend), and sends each phone watching it the
# delta against what that phone holds: an unchanged file sends nothing.
# Messages leave through `out`; the server hands them to the hub, which
# writes them to the phone's socket.

type PMsg is Data:
  # show a phone the board (kind 0), schematic (1) or 3D model (2: the
  # board's body and parts, with the board's plot for its layers) of a
  # project, or the file at path when set
  PWatch{client: U32, kind: U32, root: String, path: String}
  PStop{client: U32}
  PPoll{}
  # a file read: its size and mtime then, and its plot (None while it is
  # half written, or when it failed: err says why)
  PGot{key: String, sz: U32, mt: U32, plot: Maybe<&2, P.Plot>, err: String}
  # a phone asks what a piece it tapped is (reply id)
  PInfo{client: U32, id: String, key: String, at: Nat, idx: Nat}

# a message for a phone: JSON text (the hub makes it CBOR), or a frame's
# CBOR bytes as they are (a plot: never parsed again)
type POut is Data:
  POut{client: U32, text: String}
  PBin{client: U32, bytes: List<&2, U32>}

# a watched file: the size and mtime last read, a countdown to one more
# read after a change (a second save within the same second), whether a
# read is under way, and its latest plot
type Src is Data:
  Src{key: String, kind: U32, path: String, sz: U32, mt: U32, re: Nat, busy: Bool, plot: Maybe<&2, P.Plot>}

# a phone's view: a source it watches, the chunks it holds, and the 3D
# model's bytes it holds (a 3D view is two: the board's plot and the model)
type Sub is Data:
  Sub{client: U32, key: String, sent: List<&2, P.Chunk>, mesh: String, fresh: Bool}

type PSt is Data:
  PSt{srcs: List<&2, Src>, subs: List<&2, Sub>, polling: Bool}

# a source drawn as a 3D model: 2 with its parts, 3 its board alone, 4 as
# 2 (the web's key from before; every board model has its mask and silk)
def Pv.solid.kind(+kind: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(kind, 2), U32.is_eq(kind, 3)), U32.is_eq(kind, 4))

def Pv.fuel() -> Nat:
  U32.to_nat(4294967295)

def Pv.say(+out: Chan(POut), +client: U32, text: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, POut, Unit> <- Chan.send(POut, out, POut{client, text})
    return Unit{}

def Pv.bin(+out: Chan(POut), +client: U32, bs: List<&2, U32>) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, POut, Unit> <- Chan.send(POut, out, PBin{client, bs})
    return Unit{}

# Sources
# -------

def Src.key(+kind: U32, +root: String, +path: String) -> String:
  U32.show(kind) ++ "|" ++ root ++ "|" ++ path

def Src.is(s: Src, +key: String) -> Bool:
  match s:
    case Src{k, _, _, _, _, _, _, _}:
      String.eq(k, key)

def Src.has(ss: List<&2, Src>, +key: String) -> Bool:
  match ss:
    case Nil{}:
      False{}
    case +s <> t:
      Bool.or(Src.is(s, key), Src.has(t, key))

def Src.plot.k(hit: Bool, s: Src, rest: Maybe<&2, P.Plot>) -> Maybe<&2, P.Plot>:
  match hit s:
    case True{} Src{_, _, _, _, _, _, _, p}:
      p
    case False{} _:
      rest

# the latest plot of a source, when it has one
def Src.plot(ss: List<&2, Src>, +key: String) -> Maybe<&2, P.Plot>:
  match ss:
    case Nil{}:
      None{}
    case +s <> t:
      Src.plot.k(Src.is(s, key), s, Src.plot(t, key))

def Src.add(has: Bool, +key: String, +kind: U32, +path: String, ss: List<&2, Src>) -> List<&2, Src>:
  match has:
    case True{}:
      ss
    case False{}:
      Src{key, kind, path, 0, 0, 0n, False{}, None{}} <> ss

def Sub.watched(ss: List<&2, Sub>, +key: String) -> Bool:
  match ss:
    case Nil{}:
      False{}
    case Sub{_, +k, _, _, _} <> t:
      Bool.or(String.eq(k, key), Sub.watched(t, key))

def Src.keep(keep: Bool, s: Src, rest: List<&2, Src>) -> List<&2, Src>:
  match keep:
    case True{}:
      s <> rest
    case False{}:
      rest

def Src.key.of(s: Src) -> String:
  match s:
    case Src{k, _, _, _, _, _, _, _}:
      k

# the sources some phone still watches
def Src.gc(ss: List<&2, Src>, +subs: List<&2, Sub>) -> List<&2, Src>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      Src.keep(Sub.watched(subs, Src.key.of(s)), s, Src.gc(t, subs))

def Sub.keep(drop: Bool, s: Sub, rest: List<&2, Sub>) -> List<&2, Sub>:
  match drop:
    case True{}:
      rest
    case False{}:
      s <> rest

def Sub.client(s: Sub) -> U32:
  match s:
    case Sub{c, _, _, _, _}:
      c

# every view but the client's
def Sub.drop(ss: List<&2, Sub>, +client: U32) -> List<&2, Sub>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      Sub.keep(U32.is_eq(Sub.client(s), client), s, Sub.drop(t, client))

# the chunks a phone holds, whatever source they came from (a phone keeps
# the last plot it was sent; a 3D model's view holds no plot chunks): a new view's first plot is a delta against
# them, so moving between versions of a file sends and fades in only what
# changed (law plot_delta_exact holds against any held chunks). A new
# view sends its first plot even when it changes nothing (fresh).
def Sub.plot(+key: String) -> Bool:
  Bool.or(String.starts_with(key, "0|"), String.starts_with(key, "1|"))

def Sub.carry(ss: List<&2, Sub>, +client: U32) -> List<&2, P.Chunk>:
  match ss:
    case Nil{}:
      Nil{}
    case Sub{+c, +k, +cs, _, _} <> t:
      Bool.pick(List<&2, P.Chunk>, Bool.and(U32.is_eq(c, client), Sub.plot(k)), cs, Sub.carry(t, client))

def Sub.clear.k(hit: Bool, s: Sub) -> Sub:
  match hit s:
    case True{} Sub{c, k, _, m, f}:
      Sub{c, k, Nil{}, m, f}
    case False{} _:
      s

# Sending
# -------

def Pv.send.k(quiet: Bool, +out: Chan(POut), +client: U32, +key: String, +pl: P.Plot, rs: List<&2, P.Ref>) -> IO(Unit):
  match quiet:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      Pv.bin(out, client, P.Plot.msg.fam(key, HS.Hist.key.family(key), P.Plot.box(pl), P.Plot.thick(pl), P.Plot.edge(pl), rs))

# a phone's view brought up to a plot: the delta goes out unless it would
# change nothing; the phone now holds the plot's chunks
def Pv.send.plot(s: Sub, +pl: P.Plot, +out: Chan(POut)) -> IO(Sub):
  match s:
    case Sub{+client, +key, +sent, mesh, fresh}:
      do IO<Sub>:
        +rs : List<&2, P.Ref> = P.Plot.delta(P.Plot.chunks(pl), sent)
        Pv.send.k(Bool.and(Bool.not(fresh), P.Plot.quiet(rs, 0n, List.length(&2, P.Chunk, sent))), out, client, key, pl, rs)
        return Sub{client, key, P.Plot.chunks(pl), mesh, False{}}

def Pv.solid.rev(so: P.Solid) -> String:
  match so:
    case P.Solid{_, _, d}:
      d

def Pv.solid.n(so: P.Solid) -> U32:
  match so:
    case P.Solid{n, _, _}:
      n

def Pv.send.solid.k(same: Bool, +out: Chan(POut), +client: U32, +key: String, so: P.Solid) -> IO(Unit):
  match same:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      Pv.bin(out, client, P.Plot.msg.solid(key, so))

# a phone's view brought up to a 3D model: sent whole, unless it holds
# exactly this one
def Pv.send.solid(s: Sub, +so: P.Solid, +out: Chan(POut)) -> IO(Sub):
  match s:
    case Sub{+client, +key, sent, +mesh, fresh}:
      do IO<Sub>:
        Pv.send.solid.k(Q.Eq.str(Pv.solid.rev(so), mesh), out, client, key, so)
        return Sub{client, key, sent, Pv.solid.rev(so), fresh}

def Pv.send.at(solid: Bool, s: Sub, +pl: P.Plot, +out: Chan(POut)) -> IO(Sub):
  match solid:
    case True{}:
      Pv.send.solid(s, P.Plot.solid(pl), out)
    case False{}:
      Pv.send.plot(s, pl, out)

def Pv.send(s: Sub, +pl: P.Plot, +out: Chan(POut)) -> IO(Sub):
  Pv.send.at(U32.is_ne(Pv.solid.n(P.Plot.solid(pl)), 0), s, pl, out)

def Pv.send.if(hit: Bool, s: Sub, m: Maybe<&2, P.Plot>, +out: Chan(POut)) -> IO(Sub):
  match hit m:
    case True{} Some{+pl}:
      Pv.send(s, pl, out)
    case _ _:
      IO.pure(Sub, s)

def Sub.is(s: Sub, +key: String) -> Bool:
  match s:
    case Sub{_, k, _, _, _}:
      String.eq(k, key)

def Sub.clear.go(ss: List<&2, Sub>, +key: String) -> List<&2, Sub>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      Sub.clear.k(Sub.is(s, key), s) <> Sub.clear.go(t, key)

# a phone told there is nothing for key holds nothing
def Sub.clear(ss: List<&2, Sub>, +key: String, bad: Bool) -> List<&2, Sub>:
  match bad:
    case True{}:
      Sub.clear.go(ss, key)
    case False{}:
      ss

def Pv.fail.one(hit: Bool, +s: Sub, +key: String, +err: String, +out: Chan(POut)) -> IO(Unit):
  match hit:
    case True{}:
      Pv.say(out, Sub.client(s), P.Plot.none(key, err))
    case False{}:
      IO.pure(Unit, Unit{})

# every phone watching key told why there is nothing to show
def Pv.fail(ss: List<&2, Sub>, +key: String, +err: String, +out: Chan(POut)) -> IO(Unit):
  match ss:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +s <> t:
      IO.bind(Unit, Unit, Pv.fail.one(Sub.is(s, key), s, key, err, out), u => Pv.fail(t, key, err, out))

# every phone watching key, brought up to its plot
def Pv.fan(ss: List<&2, Sub>, +key: String, +m: Maybe<&2, P.Plot>, +out: Chan(POut)) -> IO(List<&2, Sub>):
  match ss:
    case Nil{}:
      IO.pure(List<&2, Sub>, Nil{})
    case +s <> t:
      do IO<List<&2, Sub>>:
        s2 : Sub <- Pv.send.if(Sub.is(s, key), s, m, out)
        t2 : List<&2, Sub> <- Pv.fan(t, key, m, out)
        return s2 <> t2

# Reading
# -------

def Pv.parse.k(sch: Bool, bs: List<&2, U32>) -> Maybe<&2, List<&2, B.Item>>:
  match sch:
    case True{}:
      SC.Sch.items(bs)
    case False{}:
      B.Board.items(bs)

def Pv.plot(m: Maybe<&2, List<&2, B.Item>>, +kind: U32) -> Maybe<&2, P.Plot>:
  match m:
    case Some{its}:
      Some{P.Plot.of(its, kind)}
    case None{}:
      None{}

def Pv.stamp.k(v: U32 & U32 & U32) -> U32 & U32:
  (_, sm) = v
  sm

def Pv.stamp(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> U32 & U32:
  match r:
    case Done{v}:
      Pv.stamp.k(v)
    case Fail{_}:
      (0, 0)

# The child
# ---------
# `backplane --plot KIND FILE` reads one file and prints its plot as lines
# (Line.plot), exiting 2 while the file is half written. The hub runs one
# per read: each parse starts on a fresh heap (the runtime slows as a long
# process allocates and frees big structures), its memory goes back when
# it exits, and a pathological file cannot stall the hub.

def Pv.write.done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def Pv.write.opened(r: Result<&1, &1, U32 & String, File>, text: String) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write(f, text), Pv.write.done)
    case Fail{_}:
      IO.pure(Unit, Unit{})

# a file written whole, then put in place (a reader never sees half of it)
def Pv.write(+path: String, text: String) -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, File> <- File.open(path ++ ".new", "w")
    Pv.write.opened(r, text)
    mv : Result<&1, &1, U32 & String, Unit> <- H.FS.rename(path ++ ".new", path)
    return Unit{}

def Pv.dir(+path: String) -> String:
  String.reverse(String.drop(String.reverse(path), Nat.add(String.length(Maybe.default(&2, String, List.last(&2, String, String.split(path, '/')), "")), 1n)))

def Pv.child.k(m: Maybe<&2, P.Plot>, +thick: U32, +infos: String) -> IO(Unit):
  match m:
    case Some{+pl}:
      do IO<Unit>:
        mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Pv.dir(infos))
        Bool.pick(IO(Unit), String.is_empty(infos), IO.pure(Unit, Unit{}), Pv.write(infos, P.Infos.file(P.Plot.chunks(pl))))
        IO.print(P.Line.plot(P.Plot.bare(P.Plot.with_thick(pl, thick))))
    case None{}:
      IO.die(Unit, 2, "")

def Pv.thick.seg(ss: List<&2, String>) -> String:
  match ss:
    case Nil{}:
      ""
    case +x <> t:
      Bool.pick(String, String.starts_with(x, "thickness "), x, Pv.thick.seg(t))

# the board's thickness in micrometres: the first (thickness ..) of the
# file, (general)'s, near its start (1.6 mm when there is none)
def Pv.thick.of(+seg: String) -> U32:
  Bool.pick(U32, String.is_empty(seg), 1600,
    P.Num.um(SX.Sx.f(SX.Sx.parse(U.Utf8.encode("(" ++ Maybe.default(&2, String, List.head(&2, String, String.split(seg, ')')), "") ++ ")")), 0n)))

def Pv.thick(bs: List<&2, U32>) -> U32:
  Pv.thick.of(Pv.thick.seg(String.split(U.Utf8.decode(List.take(&2, U32, bs, 8192n)), '(')))

def Pv.home() -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("HOME")
    return Result.default(&1, &1, U32 & String, String, r, "/tmp")

def Pv.cli.k(rt: K.Rt) -> String:
  match rt:
    case K.Rt{cli, _, _, _, _}:
      cli

# the kicad-cli the hub uses (K.Rt.resolve)
def Pv.cli() -> IO(String):
  do IO<String>:
    +home : String <- Pv.home()
    e : Result<&1, &1, U32 & String, String> <- IO.get_env("BACKPLANE_KICAD_CLI")
    exe : String <- H.Sys.exe_dir()
    +data : String <- K.Rt.env("BACKPLANE_HOME")
    rt : K.Rt <- K.Rt.resolve(Result.default(&1, &1, U32 & String, String, e, ""), exe, Bool.pick(String, String.is_empty(data), home ++ "/.backplane-bend", data))
    return Pv.cli.k(rt)

def Pv.step2glb() -> IO(String):
  do IO<String>:
    +d : String <- H.Sys.exe_dir()
    ok : Bool <- K.Io.is_file(d ++ "/backplane-step2glb")
    return Bool.pick(String, ok, d ++ "/backplane-step2glb", "backplane-step2glb")

def Pv.is_step(+p: String) -> Bool:
  Bool.or(String.ends_with(p, ".step"), Bool.or(String.ends_with(p, ".stp"), Bool.or(String.ends_with(p, ".STEP"), String.ends_with(p, ".STP"))))

# a 3D model as a GLB: a .glb as it is; a STEP through step2glb; a board's
# body and parts through kicad-cli (its copper, mask and silk the phone
# draws from the board's plot), or its body alone (kind 3: parts off)
# (a board's model is its body with its mask, silk and pads, and its parts
# unless bare; tracks and zones are left out: copper drawn over the mask
# only blots it)
def Pv.glb.flags(+bare: Bool, +full: Bool, +out: String, +path: String) -> List<&2, String>:
  List.append(&2, String, ["pcb", "export", "glb"], List.append(&2, String, Bool.pick(List<&2, String>, bare, ["--no-components"], ["--subst-models"]),
    ["--include-pads", "--include-soldermask", "--include-silkscreen", "-f", "-o", out, path]))

# a STEP's GLB is written beside and moved in whole: a reader never meets
# half of one
def Pv.step.ran.to(r: PR.Ran, +from: String, +out: String) -> IO(PR.Ran):
  IO.bind(Result<&1, &1, U32 & String, Unit>, PR.Ran, H.FS.rename(from, out), m => IO.pure(PR.Ran, r))

def Pv.step.ran(r: PR.Ran, +out: String) -> IO(PR.Ran):
  Pv.step.ran.to(r, out ++ ".part", out)

# a board's GLB is written beside and moved in whole only when KiCad
# succeeded, so a half-written export is never taken for a cached one
def Pv.board.ran.k(ok: Bool, r: PR.Ran, +out: String) -> IO(PR.Ran):
  match ok:
    case True{}:
      Pv.step.ran.to(r, out ++ ".part.glb", out)
    case False{}:
      IO.pure(PR.Ran, r)

def Pv.board.ran(r: PR.Ran, +out: String) -> IO(PR.Ran):
  match r:
    case PR.Ran{+c, +log}:
      Pv.board.ran.k(U32.is_eq(c, 0), PR.Ran{c, log}, out)

def Pv.glb.run(glb: Bool, step: Bool, +bare: Bool, +full: Bool, +path: String, +out: String) -> IO(PR.Ran):
  match glb step:
    case True{} _:
      IO.pure(PR.Ran, PR.Ran{0, ""})
    case False{} True{}:
      IO.bind(String, PR.Ran, Pv.step2glb(), x => IO.bind(PR.Ran, PR.Ran, PR.Proc.run(x, [path, out ++ ".part"], "/"), r => Pv.step.ran(r, out)))
    case False{} False{}:
      IO.bind(String, PR.Ran, Pv.cli(), c => IO.bind(PR.Ran, PR.Ran, K.KiCad.run(c, Pv.glb.flags(bare, full, out ++ ".part.glb", path)), r => Pv.board.ran(r, out)))

# how many parts kicad-cli found no 3D model for ("Could not add 3D model
# for R1."): the phone says so, else a bare board looks like a bug
def Pv.missing(ls: List<&2, String>, +n: Nat) -> Nat:
  match ls:
    case Nil{}:
      n
    case +l <> t:
      Pv.missing(t, Bool.pick(Nat, String.starts_with(l, "Could not add 3D model"), 1n+n, n))

def Pv.missing.note(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "",
    Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " part has", " parts have") ++ " no 3D model on the hub's machine (KiCad's 3D library is not installed there, or the paths are wrong)")

# a part from STEP stands Z up, as the window and renders show it
def Pv.solid.up(step: Bool, ts: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  match step:
    case True{}:
      MC.Mech.upright(MC.Mech.metres(ts))
    case False{}:
      ts

def Pv.solid.done(r: PR.Ran, +glb: String, +note: String, +step: Bool) -> IO(Unit):
  match r:
    case PR.Ran{0, +log}:
      do IO<Unit>:
        Pv.write(note, Pv.missing.note(Pv.missing(String.lines(log), 0n)))
        bs : List<&2, U32> <- K.Io.bytes(glb)
        IO.print(P.Line.plot(P.Plot.of.solid(P.Solid.of(P.Solid.thin(Pv.solid.up(step, G.Mesh.tris(G.Glb.parse(bs))))))))
    case PR.Ran{_, out}:
      IO.die(Unit, 3, "3D export failed: " ++ String.take(String.trim(out), 160n))

# the models cache the window and `backplane --render` keep (parts3.bend's
# Parts.out: the path, size and time name a STEP's GLB), so a part already
# rendered or seen opens at once, and a STEP is converted once per version
def Pv.cache() -> IO(String):
  do IO<String>:
    +home : String <- Pv.home()
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("XDG_CACHE_HOME")
    +dir : String = Result.default(&1, &1, U32 & String, String, r, home ++ "/.cache") ++ "/backplane-bend/models"
    mk : PR.Ran <- PR.Proc.run("mkdir", ["-p", dir], "/")
    return dir

def Pv.ver.m(sm: U32 & U32) -> String:
  (sz, mt) = sm
  U32.show(sz) ++ "." ++ U32.show(mt)

def Pv.ver.k(v: U32 & U32 & U32) -> String:
  (_, sm) = v
  Pv.ver.m(sm)

def Pv.ver(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> String:
  match r:
    case Done{v}:
      Pv.ver.k(v)
    case Fail{_}:
      "0"

def Pv.step.out(+path: String) -> IO(String):
  do IO<String>:
    +dir : String <- Pv.cache()
    st : Result<&1, &1, U32 & String, U32 & U32 & U32> <- H.FS.stat(path)
    return dir ++ "/" ++ U32.show(P.Key.hash(path ++ "|" ++ Pv.ver(st), 2166136261)) ++ ".glb"

# a board's model, cached per version of the board file (its size and
# time) and per export (with its parts or bare): KiCad's export takes half a
# minute on a full board, so it runs once per change, not on every view
def Pv.board.out(+path: String, +bare: Bool) -> IO(String):
  do IO<String>:
    +dir : String <- Pv.cache()
    st : Result<&1, &1, U32 & String, U32 & U32 & U32> <- H.FS.stat(path)
    return dir ++ "/board-" ++ U32.show(P.Key.hash(Bool.pick(String, bare, "bare2|", "parts2|") ++ path ++ "|" ++ Pv.ver(st), 2166136261)) ++ ".glb"

# where a model's GLB comes from: a .glb is its own; a STEP's and a board's
# are cached per version
def Pv.solid.out(isglb: Bool, step: Bool, +bare: Bool, +full: Bool, +path: String) -> IO(String):
  match isglb step:
    case True{} _:
      IO.pure(String, path)
    case False{} True{}:
      Pv.step.out(path)
    case False{} False{}:
      Pv.board.out(path, bare)

def Pv.child.solid(+bare: Bool, +full: Bool, +path: String, +note: String) -> IO(Unit):
  do IO<Unit>:
    +isglb : Bool = String.ends_with(path, ".glb")
    +out : String <- Pv.solid.out(isglb, Pv.is_step(path), bare, full, path)
    have : Bool <- K.Io.is_file(out)
    r : PR.Ran <- Pv.glb.run(Bool.or(isglb, have), Pv.is_step(path), bare, full, path, out)
    Pv.solid.done(r, out, note, Pv.is_step(path))

def Pv.unread() -> String:
  "This version of the file cannot be read."

# a file in a commit that cannot be read never will be: say so (exit 3)
# rather than wait as for a live file half written (exit 2)
def Pv.child.v(m: Maybe<&2, P.Plot>, fixed: Bool, +thick: U32, +infos: String) -> IO(Unit):
  match m fixed:
    case None{} True{}:
      IO.die(Unit, 3, Pv.unread())
    case _ _:
      Pv.child.k(m, thick, infos)

# a version's items: none at all when it cannot be read (a file in a
# commit that lacks it reads as no bytes)
def Pv.version(fixed: Bool, +kind: U32, bs: List<&2, U32>) -> Maybe<&2, List<&2, B.Item>>:
  match fixed bs:
    case True{} Nil{}:
      None{}
    case _ +b:
      Pv.parse.k(U32.is_eq(kind, 1), b)

def Pv.child.one(+kind: U32, +path: String, +infos: String) -> IO(Unit):
  do IO<Unit>:
    +bs : List<&2, U32> <- GT.Git.version(path)
    Pv.child.v(Pv.plot(Pv.version(Bool.not(Nat.is_eq(HS.Hist.kind(path), 0n)), kind, bs), kind), Bool.not(Nat.is_eq(HS.Hist.kind(path), 0n)), Bool.pick(U32, U32.is_eq(kind, 0), Pv.thick(bs), 0), infos)

# one side of a comparison: a file that is not there (made or removed by
# the change) has no items
def Pv.side(+kind: U32, bs: List<&2, U32>) -> Maybe<&2, List<&2, B.Item>>:
  match bs:
    case Nil{}:
      Some{Nil{}}
    case +b <> t:
      Pv.parse.k(U32.is_eq(kind, 1), b <> t)

def Pv.mark(+bits: U32, a: Maybe<&2, List<&2, B.Item>>, b: Maybe<&2, List<&2, B.Item>>) -> Maybe<&2, List<&2, B.Item>>:
  match a b:
    case Some{+oa} Some{+nb}:
      Some{HS.Hist.mark.on(bits, oa, nb)}
    case _ _:
      None{}

# a comparison (core/hist.bend): both versions parsed, the new one's
# items marked against the old one's (an old one that cannot be read has
# none: the file is new), in the comparison's look (kind | Look.diff)
def Pv.child.diff(+kind: U32, +path: String, +infos: String) -> IO(Unit):
  do IO<Unit>:
    +a : List<&2, U32> <- GT.Git.version(HS.Hist.old(path))
    +b : List<&2, U32> <- GT.Git.version(HS.Hist.new(path))
    Pv.child.k(Pv.plot(Pv.mark(HS.Hist.bits(path), Pv.side(kind, a), Pv.side(kind, b)), U32.or(kind, LO.Look.diff())), 0, infos)

def Pv.child.plot(+kind: U32, +path: String, +infos: String) -> IO(Unit):
  Bool.pick(IO(Unit), Nat.is_eq(HS.Hist.kind(path), 2n), Pv.child.diff(kind, path, infos), Pv.child.one(kind, path, infos))

def Pv.child(+kind: U32, +path: String, +infos: String) -> IO(Unit):
  Bool.pick(IO(Unit), Pv.solid.kind(kind), Pv.child.solid(U32.is_eq(kind, 3), U32.is_eq(kind, 4), path, infos), Pv.child.plot(kind, path, infos))

# `backplane --plot-info FILE SIG IDX`: one piece's info from an info file
def Pv.child.info(+file: String, +sig: U32, +idx: Nat) -> IO(Unit):
  do IO<Unit>:
    t : String <- K.Io.text(file)
    IO.print(P.Infos.find(t, sig, idx))

# A schematic's sheets
# ------------------
# The files the root reaches, read one by one (a sheet that includes an
# ancestor is listed but never read twice), as hier.bend's rows. The window
# walks them itself; the hub's phones get them from a --plot-tree child
# (Pv.tree), so no schematic is parsed in the hub.

def Tree.next(todo: List<&2, String>, +tab: List<&2, HI.Kids>, k: List<&2, HI.Kids> -> List<&2, String> -> IO(List<&2, HI.Kids>)) -> IO(List<&2, HI.Kids>):
  match todo:
    case Nil{}:
      IO.pure(List<&2, HI.Kids>, tab)
    case +p <> rest:
      do IO<List<&2, HI.Kids>>:
        bs : List<&2, U32> <- GT.Git.version(p)
        +kd : HI.Kids = HI.Hier.kids(p, bs)
        k(kd <> tab, HI.Hier.todo(HI.Hier.kids.sheets(kd), kd <> tab, rest))

def Tree.load(fuel: Nat, +todo: List<&2, String>, +tab: List<&2, HI.Kids>) -> IO(List<&2, HI.Kids>):
  match fuel:
    case 0n:
      IO.pure(List<&2, HI.Kids>, tab)
    case 1n+f:
      Tree.next(todo, tab, t2 => d2 => Tree.load(f, d2, t2))

# the root sheet: the project's schematic (in the same commit when the
# file shown is a version, core/hist.bend), else the file shown
def Tree.root(f: K.Files, +path: String) -> String:
  match f:
    case K.Files{_, _, +sch, _}:
      Bool.pick(String, String.is_empty(sch), path, HS.Hist.rebase(path, sch))

# `backplane --plot-tree ROOT`: the rows, as Hier.show writes them
def Pv.child.tree(+root: String) -> IO(Unit):
  do IO<Unit>:
    tab : List<&2, HI.Kids> <- Tree.load(64n, [root], Nil{})
    IO.print(HI.Hier.show(HI.Hier.rows(tab, root)))

# a phone watching a project's schematic hears its sheets ("" rows for a
# schematic of one sheet, or one the child could not read)
def Pv.tree.say(r: PR.Ran, +client: U32, +root: String, +out: Chan(POut)) -> IO(Unit):
  match r:
    case PR.Ran{0, +text}:
      Pv.say(out, client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"sheets"}}, J.KV{"root", J.Str{root}}, J.KV{"rows", J.Str{String.trim_end(text)}}])))
    case _:
      IO.pure(Unit, Unit{})

def Pv.tree.k(+file: String, +client: U32, +root: String, +out: Chan(POut)) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(file), IO.pure(Unit, Unit{}),
    IO.bind(String, Unit, H.Sys.exe_path(), exe => IO.bind(PR.Ran, Unit, PR.Proc.run(EX.Exe.child(exe), ["--plot-tree", file], "/"), r => Pv.tree.say(r, client, root, out))))

def Pv.tree(+client: U32, +root: String, +path: String, +out: Chan(POut)) -> IO(Unit):
  IO.bind(K.Files, Unit, K.Files.read(root), f => Pv.tree.k(Tree.root(f, Bool.pick(String, Nat.is_eq(HS.Hist.kind(path), 1n), path, "")), client, root, out))

def Pv.args.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Pv.args.copy(t)

def Pv.args.back(xs: List<&2, String>) -> List<String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Pv.args.back(t)

def Pv.main.k(xs: List<&2, String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  match xs:
    case "--plot" <> k <> f <> i <> _:
      Pv.child(Maybe.default(&2, U32, U32.read(k), 0), f, i)
    case "--plot" <> k <> f <> _:
      Pv.child(Maybe.default(&2, U32, U32.read(k), 0), f, "")
    case "--plot-tree" <> f <> _:
      Pv.child.tree(f)
    case "--plot-info" <> f <> sg <> ix <> _:
      Pv.child.info(f, Maybe.default(&2, U32, U32.read(sg), 0), Maybe.default(&2, Nat, Nat.read(ix), 0n))
    case +a:
      go(Pv.args.back(a))

# the program's arguments: a --plot child's, or the program's own (go)
def Pv.main(xs: List<String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  Pv.main.k(Pv.args.copy(xs), go)

def Pv.got.ran(r: PR.Ran, +kind: U32) -> Maybe<&2, P.Plot>:
  match r:
    case PR.Ran{0, out}:
      Some{P.Scan.plot(out)}
    case _:
      None{}

# why a read failed ("" when it did not, or the file is half written)
def Pv.got.err(r: PR.Ran) -> String:
  match r:
    case PR.Ran{0, _}:
      ""
    case PR.Ran{2, _}:
      ""
    case PR.Ran{_, out}:
      String.take(String.trim(out), 200n)

def Pv.read.k(sm: U32 & U32, +r: PR.Ran, +key: String, +kind: U32, +q: Chan(PMsg)) -> IO(Unit):
  (sz, mt) = sm
  do IO<Unit>:
    ok : Result<&1, &1, PMsg, Unit> <- Chan.send(PMsg, q, PGot{key, sz, mt, Pv.got.ran(r, kind), Pv.got.err(r)})
    return Unit{}

# read and plot a file in a child process (the stamp is taken first, so a
# save during the read is seen by the next poll)
# where the info tables of a source's newest plot are kept
def Pv.infos(+dir: String, +key: String) -> String:
  dir ++ "/" ++ U32.show(P.Key.hash(key, 2166136261)) ++ ".info"

# a 3D model's note (Pv.missing.note, in its source's info file) to the
# phones that watch it
def Pv.note.fan(ss: List<&2, Sub>, +key: String, +text: String, +out: Chan(POut)) -> IO(Unit):
  match ss:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +s <> t:
      do IO<Unit>:
        Bool.pick(IO(Unit), Sub.is(s, key), Pv.say(out, Sub.client(s), J.Json.show(J.Json.obj([J.KV{"t", J.Str{"plotnote"}}, J.KV{"key", J.Str{key}}, J.KV{"text", J.Str{text}}]))), IO.pure(Unit, Unit{}))
        Pv.note.fan(t, key, text, out)

def Pv.note(solid: Bool, +subs: List<&2, Sub>, +key: String, +dir: String, +out: Chan(POut)) -> IO(Unit):
  match solid:
    case True{}:
      IO.bind(String, Unit, K.Io.text(Pv.infos(dir, key)), t => Pv.note.fan(subs, key, String.trim(t), out))
    case False{}:
      IO.pure(Unit, Unit{})

# A version's stamp, as the poll compares it: a live file's size and
# mtime; a file in a commit never changes (1, 1); a comparison its sides'
# folded together, so an edit to either live side reads it again

def Pv.live(+path: String) -> Bool:
  Bool.and(Nat.is_eq(HS.Hist.kind(path), 0n), Bool.not(String.is_empty(path)))

# nothing to watch: no side is a live file
def Pv.stamp.fixed(+path: String) -> Bool:
  Bool.pick(Bool, Nat.is_eq(HS.Hist.kind(path), 2n), Bool.not(Bool.or(Pv.live(HS.Hist.old(path)), Pv.live(HS.Hist.new(path)))), Bool.not(Nat.is_eq(HS.Hist.kind(path), 0n)))

def Pv.stamp.one.k(live: Bool, +path: String) -> IO(U32 & U32):
  match live:
    case True{}:
      IO.bind(Result<&1, &1, U32 & String, U32 & U32 & U32>, U32 & U32, H.FS.stat(path), r => IO.pure(U32 & U32, Pv.stamp(r)))
    case False{}:
      IO.pure(U32 & U32, (1, 1))

def Pv.stamp.one(+path: String) -> IO(U32 & U32):
  Pv.stamp.one.k(Pv.live(path), path)

def Pv.stamp.fold(a: U32 & U32, b: U32 & U32) -> U32 & U32:
  (sa, ma) = a
  (sb, mb) = b
  ((sa + (sb * 31 : U32) : U32), U32.xor(ma, (mb * 2654435761 : U32)))

def Pv.stamp.two(+path: String) -> IO(U32 & U32):
  do IO<U32 & U32>:
    a : U32 & U32 <- Pv.stamp.one(HS.Hist.old(path))
    b : U32 & U32 <- Pv.stamp.one(HS.Hist.new(path))
    return Pv.stamp.fold(a, b)

def Pv.stamp.of.k(cmp: Bool, +path: String) -> IO(U32 & U32):
  match cmp:
    case True{}:
      Pv.stamp.two(path)
    case False{}:
      Pv.stamp.one.k(Nat.is_eq(HS.Hist.kind(path), 0n), path)

# a version's size and mtime (Pv.stamp)
def Pv.stamp.of(+path: String) -> IO(U32 & U32):
  Pv.stamp.of.k(Nat.is_eq(HS.Hist.kind(path), 2n), path)

def Pv.read(+key: String, +kind: U32, +path: String, +q: Chan(PMsg), +dir: String) -> IO(Unit):
  do IO<Unit>:
    st : U32 & U32 <- Pv.stamp.of(path)
    exe : String <- H.Sys.exe_path()
    r : PR.Ran <- PR.Proc.run(EX.Exe.child(exe), ["--plot", U32.show(kind), path, Pv.infos(dir, key)], "/")
    Pv.read.k(st, r, key, kind, q)

# Polling
# -------

def Pv.poll.go(changed: Bool, re: Nat, s: Src, +q: Chan(PMsg), +dir: String) -> IO(Src):
  match changed re s:
    case True{} _ Src{+k, +kd, +p, sz, mt, _, _, pl}:
      do IO<Src>:
        IO.spawn(Unit, Pv.read(k, kd, p, q, dir))
        return Src{k, kd, p, sz, mt, Bool.pick(Nat, Pv.solid.kind(kd), 0n, 7n), True{}, pl}
    case False{} 1n Src{+k, +kd, +p, sz, mt, _, _, pl}:
      do IO<Src>:
        IO.spawn(Unit, Pv.read(k, kd, p, q, dir))
        return Src{k, kd, p, sz, mt, 0n, True{}, pl}
    case False{} 1n+r Src{k, kd, p, sz, mt, _, b, pl}:
      IO.pure(Src, Src{k, kd, p, sz, mt, r, b, pl})
    case False{} 0n _:
      IO.pure(Src, s)

def Pv.poll.st(sm: U32 & U32, s: Src, +q: Chan(PMsg), +dir: String) -> IO(Src):
  (+s2, +m2) = sm
  match s:
    case Src{k, kd, p, +sz, +mt, +re, b, pl}:
      Pv.poll.go(Bool.or(U32.is_ne(s2, sz), U32.is_ne(m2, mt)), re, Src{k, kd, p, sz, mt, re, b, pl}, q, dir)

# one source's turn: a changed size or mtime reads it again (not while a
# read is under way)
def Pv.poll.one(busy: Bool, s: Src, +q: Chan(PMsg), +dir: String) -> IO(Src):
  match busy s:
    case True{} _:
      IO.pure(Src, s)
    case False{} Src{k, kd, +p, sz, mt, re, b, pl}:
      do IO<Src>:
        st : U32 & U32 <- Pv.stamp.of(p)
        Pv.poll.st(st, Src{k, kd, p, sz, mt, re, b, pl}, q, dir)

def Src.busy(s: Src) -> Bool:
  match s:
    case Src{_, _, _, _, _, _, b, _}:
      b

def Pv.poll(ss: List<&2, Src>, +q: Chan(PMsg), +dir: String) -> IO(List<&2, Src>):
  match ss:
    case Nil{}:
      IO.pure(List<&2, Src>, Nil{})
    case +s <> t:
      do IO<List<&2, Src>>:
        s2 : Src <- Pv.poll.one(Src.busy(s), s, q, dir)
        t2 : List<&2, Src> <- Pv.poll(t, q, dir)
        return s2 <> t2

def Pv.later(+q: Chan(PMsg)) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(150)
    ok : Result<&1, &1, PMsg, Unit> <- Chan.send(PMsg, q, PPoll{})
    return Unit{}

# the next poll, while anything is watched
def Pv.next(any: Bool, +q: Chan(PMsg)) -> IO(Bool):
  match any:
    case True{}:
      do IO<Bool>:
        IO.spawn(Unit, Pv.later(q))
        return True{}
    case False{}:
      IO.pure(Bool, False{})

# Messages
# --------

def Pv.got.src(hit: Bool, s: Src, +sz: U32, +mt: U32, +m: Maybe<&2, P.Plot>) -> Src:
  match hit s m:
    case True{} Src{k, kd, p, _, _, re, _, _} Some{pl}:
      Src{k, kd, p, sz, mt, re, False{}, Some{pl}}
    case True{} Src{k, kd, p, _, _, re, _, pl} None{}:
      Src{k, kd, p, sz, mt, re, False{}, pl}
    case False{} _ _:
      s

def Pv.got.srcs(ss: List<&2, Src>, +key: String, +sz: U32, +mt: U32, +m: Maybe<&2, P.Plot>) -> List<&2, Src>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      Pv.got.src(Src.is(s, key), s, sz, mt, m) <> Pv.got.srcs(t, key, sz, mt, m)

def Pv.start(polling: Bool, +q: Chan(PMsg)) -> IO(Bool):
  match polling:
    case True{}:
      IO.pure(Bool, True{})
    case False{}:
      do IO<Bool>:
        ok : Result<&1, &1, PMsg, Unit> <- Chan.send(PMsg, q, PPoll{})
        return True{}

def Pv.none(+kind: U32) -> String:
  Bool.pick(String, U32.is_eq(kind, 1), "No schematic in this project yet", "No board in this project yet")

def Pv.file.k(f: K.Files, +kind: U32) -> String:
  match f:
    case K.Files{_, pcb, sch, _}:
      Bool.pick(String, U32.is_eq(kind, 1), sch, pcb)

# the file a phone asked for: the explicit path, else the project's
def Pv.file(+kind: U32, +root: String, +path: String) -> IO(String):
  Bool.pick(IO(String), String.is_empty(path), IO.bind(K.Files, String, K.Files.read(root), f => IO.pure(String, Pv.file.k(f, kind))), IO.pure(String, path))

# a phone's view of a source that has a plot: all of it goes out now
def Pv.watch.now(m: Maybe<&2, P.Plot>, s: Sub, +out: Chan(POut)) -> IO(Sub):
  match m:
    case Some{+pl}:
      Pv.send(s, pl, out)
    case None{}:
      IO.pure(Sub, s)

def Pv.subs(keep: Bool, subs: List<&2, Sub>, +client: U32) -> List<&2, Sub>:
  match keep:
    case True{}:
      subs
    case False{}:
      Sub.drop(subs, client)

# a phone now watches key (its other views dropped unless keep: a 3D view
# watches the model and the board's plot)
def Pv.watch.k(none: Bool, keep: Bool, st: PSt, +client: U32, +kind: U32, +key: String, +file: String, +q: Chan(PMsg), +out: Chan(POut)) -> IO(PSt):
  match none st:
    case True{} PSt{ss, +subs, polling}:
      do IO<PSt>:
        Pv.say(out, client, P.Plot.none(key, Pv.none(kind)))
        +subs2 : List<&2, Sub> = Pv.subs(keep, subs, client)
        return PSt{Src.gc(ss, subs2), subs2, polling}
    case False{} PSt{+ss, +subs, polling}:
      do IO<PSt>:
        +ss2 : List<&2, Src> = Src.add(Src.has(ss, key), key, kind, file, ss)
        sub : Sub <- Pv.watch.now(Src.plot(ss2, key), Sub{client, key, Sub.carry(subs, client), SNil{}, True{}}, out)
        +subs2 : List<&2, Sub> = sub <> Pv.subs(keep, subs, client)
        p2 : Bool <- Pv.start(polling, q)
        return PSt{Src.gc(ss2, subs2), subs2, p2}

# a 3D view also watches the board's plot (its copper, mask and silk)
def Pv.watch.board(board: Bool, st: PSt, +client: U32, +root: String, +path: String, +file: String, +q: Chan(PMsg), +out: Chan(POut)) -> IO(PSt):
  match board:
    case True{}:
      Pv.watch.k(False{}, True{}, st, client, 0, Src.key(0, root, path), file, q, out)
    case False{}:
      IO.pure(PSt, st)

def Sub.sent.of(s: Sub) -> List<&2, P.Chunk>:
  match s:
    case Sub{_, _, cs, _, _}:
      cs

def Sub.mine(+s: Sub, +client: U32, +key: String) -> Bool:
  Bool.and(U32.is_eq(Sub.client(s), client), Sub.is(s, key))

# the chunks a phone holds of key
def Sub.held(ss: List<&2, Sub>, +client: U32, +key: String) -> List<&2, P.Chunk>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> t:
      Bool.pick(List<&2, P.Chunk>, Sub.mine(s, client, key), Sub.sent.of(s), Sub.held(t, client, key))

def Pv.reply(+id: String, +info: String) -> String:
  "{\"t\":\"reply\",\"id\":" ++ id ++ ",\"ok\":true,\"result\":{\"info\":" ++ J.Json.show(J.Str{info}) ++ "}}"

def Pv.info.done(r: PR.Ran, +client: U32, +id: String, +out: Chan(POut)) -> IO(Unit):
  match r:
    case PR.Ran{0, text}:
      Pv.say(out, client, Pv.reply(id, String.trim_end(text)))
    case _:
      Pv.say(out, client, Pv.reply(id, ""))

# the answer to a phone's kicad.info: the tapped chunk (of those it holds)
# by its sig, looked up in its source's info file by a child process
def Pv.info(+client: U32, +id: String, +file: String, +sig: U32, +idx: Nat, +out: Chan(POut)) -> IO(Unit):
  do IO<Unit>:
    exe : String <- H.Sys.exe_path()
    r : PR.Ran <- PR.Proc.run(EX.Exe.child(exe), ["--plot-info", file, U32.show(sig), Nat.show(idx)], "/")
    Pv.info.done(r, client, id, out)

def Pv.handle(st: PSt, m: PMsg, +q: Chan(PMsg), +out: Chan(POut), +dir: String) -> IO(PSt):
  match st m:
    case _ PWatch{+client, +kind, +root, +path}:
      do IO<PSt>:
        +file : String <- Pv.file(kind, root, path)
        st2 : PSt <- Pv.watch.k(String.is_empty(file), False{}, st, client, kind, Src.key(kind, root, path), file, q, out)
        Bool.pick(IO(Unit), U32.is_eq(kind, 1), IO.spawn(Unit, Pv.tree(client, root, path, out)), IO.pure(Unit, Unit{}))
        Pv.watch.board(Bool.and(Pv.solid.kind(kind), String.ends_with(file, ".kicad_pcb")), st2, client, root, path, file, q, out)
    case PSt{ss, +subs, polling} PStop{+client}:
      do IO<PSt>:
        +subs2 : List<&2, Sub> = Sub.drop(subs, client)
        return PSt{Src.gc(ss, subs2), subs2, polling}
    case PSt{ss, subs, _} PPoll{}:
      do IO<PSt>:
        +ss2 : List<&2, Src> <- Pv.poll(ss, q, dir)
        p2 : Bool <- Pv.next(Bool.not(List.is_empty(&2, Src, ss2)), q)
        return PSt{ss2, subs, p2}
    case PSt{ss, +subs, polling} PInfo{+client, +id, +key, at, +idx}:
      do IO<PSt>:
        IO.spawn(Unit, Pv.info(client, id, Pv.infos(dir, key), P.Chunk.sig(P.Plot.at(at, Sub.held(subs, client, key))), idx, out))
        return PSt{ss, subs, polling}
    case PSt{ss, +subs, polling} PGot{+key, +sz, +mt, +pl, +err}:
      do IO<PSt>:
        Bool.pick(IO(Unit), String.is_empty(err), IO.pure(Unit, Unit{}), Pv.fail(subs, key, err, out))
        +subs2 : List<&2, Sub> <- Pv.fan(Sub.clear(subs, key, Bool.not(String.is_empty(err))), key, pl, out)
        Pv.note(Bool.and(String.is_empty(err), Bool.or(String.starts_with(key, "2|"), String.starts_with(key, "3|"))), subs2, key, dir, out)
        return PSt{Pv.got.srcs(ss, key, sz, mt, pl), subs2, polling}

def Pv.got(m: Maybe<&1, PMsg>, st: PSt, +q: Chan(PMsg), +out: Chan(POut), +dir: String, k: PSt -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{msg}:
      IO.bind(PSt, Unit, Pv.handle(st, msg, q, out, dir), k)
    case None{}:
      IO.pure(Unit, Unit{})

# the actor; dir keeps the info tables (the hub's home/plot)
def Pv.loop(fuel: Nat, +q: Chan(PMsg), st: PSt, +out: Chan(POut), +dir: String) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, PMsg>, Unit, Chan.recv(PMsg, q), m => Pv.got(m, st, q, out, dir, st2 => Pv.loop(f, q, st2, out, dir)))

def Pv.empty() -> PSt:
  PSt{Nil{}, Nil{}, False{}}
