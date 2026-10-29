import Base
import ./host.bend as H
import ./proc.bend as P
import ../core/json.bend as J
import ../core/utf8.bend as U
import ../core/kihealth.bend as KH
import ../core/port.bend as PORT
import ../core/kicad_install.bend as KI
import ./kicad_library.bend as KL

# KiCad on this machine: which kicad-cli to use (the Backplane KiCad fork,
# with IPC, when present), a project's canonical files, and the KiStack
# skills. The agents' instructions are built from these, so every session
# is told the same exact paths.

# the CLI, whether it is the Backplane fork, its version and source commit
type Rt is Data:
  Rt{cli: String, fork: Bool, version: String, commit: String, health: J.Json}

# a project's canonical design files ("" when it has none)
type Files is Data:
  Files{pro: String, pcb: String, sch: String, all: String}

# why a KiCad export cannot run: no kicad-cli anywhere (Rt.first)
def KiCad.missing() -> String:
  "KiCad is not installed on this machine (no kicad-cli): get it from kicad.org/download or its package manager"

# Convert filesystem arguments at the native tool boundary. Other arguments
# (flags, commands and model text) retain their bytes; POSIX is unchanged.
def KiCad.native_args(args: List<&2, String>) -> IO(List<&2, String>):
  match args:
    case Nil{}: IO.pure(List<&2, String>, Nil{})
    case h <> t:
      do IO<List<&2, String>>:
        head : String <- H.Sys.native_path(h)
        tail : List<&2, String> <- KiCad.native_args(t)
        return head <> tail

def KiCad.version() -> String:
  "10.99.0-bp.4"

def Io.bytes.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Io.bytes.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, U32>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>,
    File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), Io.bytes.done)

def Io.bytes.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, U32>, File.size(f), Io.bytes.sized)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

# Files
# -----

# a whole file's bytes ([] when it cannot be read)
def Io.bytes(path: String) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), Io.bytes.opened)

def Io.text(path: String) -> IO(String):
  IO.bind(List<&2, U32>, String, Io.bytes(path), bs => IO.pure(String, U.Utf8.decode(bs)))

def Io.is_file.kind(v: U32 & U32 & U32) -> Bool:
  (k, _) = v
  U32.is_eq(k, 0)

def Io.is_file.k(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> Bool:
  match r:
    case Done{v}:
      Io.is_file.kind(v)
    case Fail{_}:
      False{}

def Io.is_file(path: String) -> IO(Bool):
  do IO<Bool>:
    r : Result<&1, &1, U32 & String, U32 & U32 & U32> <- H.FS.stat(path)
    return Io.is_file.k(r)

def Io.names.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Io.names.copy(t)

def Io.names.k(r: Result<&1, &1, U32 & String, List<String>>) -> List<&2, String>:
  match r:
    case Done{xs}:
      Io.names.copy(xs)
    case Fail{_}:
      Nil{}

def Io.names(path: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    r : Result<&1, &1, U32 & String, List<String>> <- H.FS.list(path)
    return List.sort(~String, ~String.is_le, Io.names.k(r))

# Runtime identity is static; the version command is run only by the
# background health job after the hub is listening.
def Rt.cli(rt: Rt) -> String:
  match rt:
    case Rt{c, _, _, _, _}: c

def Rt.health(rt: Rt) -> J.Json:
  match rt:
    case Rt{_, _, _, _, h}: h

def Rt.fork(rt: Rt) -> Bool:
  match rt:
    case Rt{_, f, _, _, _}: f

def Rt.status(+rt: Rt) -> String:
  J.Json.get_str(Rt.health(rt), "status", "checking")

def Rt.set(rt: Rt, +key: String, value: J.Json) -> Rt:
  match rt:
    case Rt{c, f, v, co, h}:
      Rt{c, f, v, co, KH.KH.put.obj(h, key, value)}

def Rt.caps.json(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: J.Str{x} <> Rt.caps.json(t)

def Rt.manifest.of(m: KH.KManifest, +cli: String) -> Rt:
  match m:
    case KH.KManifest{f, v, c, a, caps, up, kv}:
      Rt{cli, f, v, c, J.Json.obj([J.KV{"api", J.Num{Nat.show(a)}}, J.KV{"caps", J.Json.arr(Rt.caps.json(caps))},
        J.KV{"upstreamCommit", J.Str{up}}, J.KV{"kicadVersion", J.Str{kv}}, J.KV{"status", J.Str{"checking"}}])}

def Rt.manifest(+cli: String, text: String) -> Rt:
  Rt.manifest.of(KH.KH.manifest(text), cli)

def Rt.empty() -> Rt:
  Rt.manifest("", "")

# A CLI path or runtime root. Missing explicit roots stay explicit.
def Rt.candidate(+p: String) -> String:
  Bool.pick(String, String.ends_with(p, ".app"), p ++ "/Contents/MacOS/kicad-cli",
    Bool.pick(String, Bool.or(String.ends_with(p, "kicad-cli"), String.ends_with(p, "kicad-cli.exe")), p, p ++ "/bin/kicad-cli"))

def Rt.root.stem(+cli: String) -> String:
  Bool.pick(String, String.ends_with(cli, "/Contents/MacOS/kicad-cli"),
    String.take(cli, Nat.sub(String.length(cli), String.length("/Contents/MacOS/kicad-cli"))),
    Bool.pick(String, String.ends_with(cli, "/bin/kicad-cli"), String.take(cli, Nat.sub(String.length(cli), String.length("/bin/kicad-cli"))),
      String.take(cli, Nat.sub(String.length(cli), 1n+String.length(Maybe.default(&2, String, List.last(&2, String, String.split(cli, '/')), cli))))))


# Windows runtime metadata is beside bin/, including when the CLI has .exe.
def Rt.root.normal(+cli: String) -> String:
  Rt.root.stem(Bool.pick(String, String.ends_with(cli, ".exe"), String.take(cli, Nat.sub(String.length(cli), 4n)), cli))

def Rt.root(cli: String) -> String:
  Rt.root.normal(String.join(String.split(cli, '\\'), "/"))

def KiCad.prepare.found(found: Bool, cli: String) -> IO(String):
  match found:
    case True{}: KL.Library.prepare(Rt.root(cli))
    case False{}: IO.pure(String, "")

def KiCad.prepare(+cli: String) -> IO(String):
  IO.bind(Bool, String, Io.is_file(cli), found => KiCad.prepare.found(found, cli))

def KiCad.agent_env() -> IO(List<&2, String>):
  KL.Library.child_env()

def KiCad.run.prepared(fine: Bool, why: String, cli: String, args: List<&2, String>) -> IO(P.Ran):
  match fine:
    case False{}: IO.pure(P.Ran, P.Ran{126, why})
    case True{}:
      do IO<P.Ran>:
        native : List<&2, String> <- KiCad.native_args(args)
        P.Proc.run(cli, native, "/")

def KiCad.run.k(empty: Bool, +cli: String, args: List<&2, String>) -> IO(P.Ran):
  match empty:
    case True{}: IO.pure(P.Ran, P.Ran{127, KiCad.missing()})
    case False{}:
      IO.bind(String, P.Ran, KiCad.prepare(cli), +why => KiCad.run.prepared(String.is_empty(why), why, cli, args))

# CLI exports in the hub and plot/review children get the real process env.
def KiCad.run(+cli: String, args: List<&2, String>) -> IO(P.Ran):
  KiCad.run.k(String.is_empty(cli), cli, args)

def Rt.env(name: String) -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env(name)
    return Result.default(&1, &1, U32 & String, String, r, "")

type KCandidate is Data:
  KCandidate{path: String, source: String}

def Rt.paths(ps: List<&2, String>) -> List<&2, KCandidate>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t:
      KCandidate{Bool.pick(String, String.is_empty(p), "./kicad-cli", p ++ "/kicad-cli"), "path"} <> Rt.paths(t)

# Keep older managed installs discoverable so the health notice offers
# Update. The pinned install stays first; the rest are newest-first.
def Rt.installed.keep.name(+name: String) -> Bool:
  Bool.and(Bool.not(String.eq(name, KiCad.version())),
    Nat.is_gt(KH.KH.part(String.split(name, '.'), 0n), 0n))

def Rt.installed.keep(name: String) -> Bool:
  Rt.installed.keep.name(name)

def Rt.installed.before(a: String, b: String) -> Bool:
  KH.KH.version.ge(a, b)

def Rt.installed.paths(ns: List<&2, String>, +data: String) -> List<&2, KCandidate>:
  match ns:
    case Nil{}: Nil{}
    case n <> t:
      KCandidate{data ++ "/kicad/" ++ n ++ "/bin/kicad-cli", "installed"} <> Rt.installed.paths(t, data)

def Rt.installed(ns: List<&2, String>, data: String) -> List<&2, KCandidate>:
  Rt.installed.paths(List.sort(~String, ~Rt.installed.before, List.filter(~String, ~Rt.installed.keep, ns)), data)

def Rt.candidates(+setting: String, +root: String, +explicit: String, +exe: String, +data: String, +user: String, +path: String, +platform: String, older: List<&2, KCandidate>) -> List<&2, KCandidate>:
  List.concat(&2, KCandidate, [
    Bool.pick(List<&2, KCandidate>, String.is_empty(setting), Nil{}, [KCandidate{setting, "setting"}]),
    Bool.pick(List<&2, KCandidate>, String.is_empty(root), Nil{}, [KCandidate{root, "env"}]),
    Bool.pick(List<&2, KCandidate>, String.is_empty(explicit), Nil{}, [KCandidate{explicit, "env"}]),
    [KCandidate{exe ++ "/kicad/bin/kicad-cli", "bundled"}, KCandidate{data ++ "/kicad/" ++ KI.Install.installed(KI.Install.platform(platform)), "installed"}],
    older,
    Bool.pick(List<&2, KCandidate>, String.starts_with(platform, "Darwin"), [KCandidate{"/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli", "app"}, KCandidate{user ++ "/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli", "app"}], Nil{}),
    Rt.paths(String.split(path, ':')), [KCandidate{"/usr/bin/kicad-cli", "system"}, KCandidate{"/usr/local/bin/kicad-cli", "system"}]])

def Rt.platform(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}: String.trim(out)
    case _: ""

def Rt.explicit.k(file: Bool, +p: String) -> String:
  Bool.pick(String, Bool.or(file, String.is_empty(p)), p, Rt.candidate(p))

def Rt.explicit(+p: String) -> IO(String):
  IO.bind(Bool, String, Io.is_file(p), file => IO.pure(String, Rt.explicit.k(file, p)))

def Rt.list(+setting: String, +explicit: String, +exe: String, +data: String) -> IO(List<&2, KCandidate>):
  do IO<List<&2, KCandidate>>:
    root0 : String <- Rt.env("BACKPLANE_KICAD")
    root : String <- Rt.explicit(root0)
    setting2 : String <- Rt.explicit(setting)
    explicit2 : String <- Rt.explicit(explicit)
    user : String <- Rt.env("HOME")
    path : String <- Rt.env("PATH")
    os : P.Ran <- P.Proc.run("uname", ["-sm"], "/")
    installed : List<&2, String> <- Io.names(data ++ "/kicad")
    return Rt.candidates(setting2, root, explicit2, exe, data, user, path, Rt.platform(os), Rt.installed(installed, data))

def Rt.stamp.k(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> String:
  match r:
    case Done{v}:
      (kind, sz, mt) = v
      U32.show(kind) ++ ":" ++ U32.show(sz) ++ ":" ++ U32.show(mt)
    case Fail{_}: "absent"

def Rt.stamp(path: String) -> IO(String):
  IO.bind(Result<&1, &1, U32 & String, U32 & U32 & U32>, String, H.FS.stat(path), r => IO.pure(String, Rt.stamp.k(r)))

# Watch the selected candidate and every candidate that could override it.
# Paths after it (for example a login shell's extra PATH entries) cannot
# change discovery, so they must not invalidate an otherwise valid cache.
def Rt.watch.next(selected: Bool, cs: List<&2, KCandidate>) -> List<&2, KCandidate>:
  match selected:
    case True{}: Nil{}
    case False{}: cs

def Rt.watch(cs: List<&2, KCandidate>, +cli: String) -> List<&2, KCandidate>:
  match cs:
    case Nil{}: Nil{}
    case KCandidate{+p, source} <> t:
      KCandidate{p, source} <> Rt.watch.next(String.eq(p, cli), Rt.watch(t, cli))

# Include manifest and parent-directory stamps: installing or replacing
# a runtime (including one ahead of the chosen CLI) invalidates the cache.
def Rt.stamps(cs: List<&2, KCandidate>) -> IO(String):
  match cs:
    case Nil{}: IO.pure(String, "")
    case KCandidate{+p, _} <> t:
      do IO<String>:
        a : String <- Rt.stamp(p)
        b : String <- Rt.stamp(Rt.root(p))
        c : String <- Rt.stamp(Rt.root(p) ++ "/manifest.json")
        d : String <- Rt.stamp(Rt.root(p) ++ "/Contents/Resources/manifest.json")
        rest : String <- Rt.stamps(t)
        return p ++ "=" ++ a ++ ":" ++ b ++ ":" ++ c ++ ":" ++ d ++ "\n" ++ rest

def Rt.read.fallback(+cli: String, +source: String, +text: String) -> IO(Rt):
  do IO<Rt>:
    alt : String <- Io.text(Rt.root(cli) ++ "/Contents/Resources/manifest.json")
    return Rt.set(Rt.manifest(cli, Bool.pick(String, String.is_empty(String.trim(text)), alt, text)), "source", J.Str{source})

def Rt.read(+cli: String, +source: String) -> IO(Rt):
  IO.bind(String, Rt, Io.text(Rt.root(cli) ++ "/manifest.json"), text => Rt.read.fallback(cli, source, text))

def Rt.choose.k(found: Bool, explicit: Bool, +p: String, +source: String, rest: IO(Rt)) -> IO(Rt):
  match found explicit:
    case True{} _: Rt.read(p, source)
    case False{} True{}: IO.pure(Rt, Rt.set(Rt.manifest(p, ""), "source", J.Str{source}))
    case False{} False{}: rest

def Rt.choose(cs: List<&2, KCandidate>) -> IO(Rt):
  match cs:
    case Nil{}: IO.pure(Rt, Rt.empty())
    case KCandidate{+p, +source} <> t:
      IO.bind(Bool, Rt, Io.is_file(p), f => Rt.choose.k(f, Bool.or(String.eq(source, "setting"), String.eq(source, "env")), p, source, Rt.choose(t)))

def Rt.discover(+setting: String, +explicit: String, +exe: String, +data: String) -> IO(Rt):
  IO.bind(List<&2, KCandidate>, Rt, Rt.list(setting, explicit, exe, data), Rt.choose)

# Compatibility callers use the last checked override from this hub's
# cache, plus the same environment/bundle/PATH candidates as the hub.
def Rt.resolve(+explicit: String, +exe: String, +data: String) -> IO(Rt):
  do IO<Rt>:
    text : String <- Io.text(data ++ "/kicad-health.json")
    +o : J.Json = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
    Rt.discover(J.Json.get_str(o, "setting", ""), explicit, exe, data)

def Rt.json(rt: Rt) -> J.Json:
  match rt:
    case Rt{c, f, v, co, h}:
      KH.KH.put.obj(KH.KH.put.obj(KH.KH.put.obj(KH.KH.put.obj(h, "cli", J.Str{c}), "fork", J.Flag{f}), "version", J.Str{v}), "commit", J.Str{co})

# A cache written by an older app must satisfy this app's minimum too.
def Rt.cache.status(+o: J.Json) -> String:
  KH.KH.status(Bool.not(String.eq(J.Json.get_str(o, "status", "checking"), "missing")),
    J.Json.get_bool(o, "runs", False{}), J.Json.get_bool(o, "fork", False{}),
    J.Json.get_str(o, "version", ""), J.Json.get_nat(o, "api", 0n))

def Rt.cached(+o: J.Json) -> Rt:
  Rt{J.Json.get_str(o, "cli", ""), J.Json.get_bool(o, "fork", False{}), J.Json.get_str(o, "version", ""), J.Json.get_str(o, "commit", ""), KH.KH.put.obj(o, "status", J.Str{Rt.cache.status(o)})}

def Rt.boot.k(valid: Bool, o: J.Json, +stamp: String, cs: List<&2, KCandidate>) -> IO(Rt):
  match valid:
    case True{}: IO.pure(Rt, Rt.cached(o))
    case False{}: IO.bind(Rt, Rt, Rt.choose(cs), rt => IO.pure(Rt, Rt.set(rt, "stamp", J.Str{stamp})))

def Rt.boot(+home: String, +setting: String, +exe: String) -> IO(Rt):
  do IO<Rt>:
    explicit : String <- Rt.env("BACKPLANE_KICAD_CLI")
    +cs : List<&2, KCandidate> <- Rt.list(setting, explicit, exe, home)
    text : String <- Io.text(home ++ "/kicad-health.json")
    +o : J.Json = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
    +stamp : String <- Rt.stamps(Rt.watch(cs, J.Json.get_str(o, "cli", "")))
    Rt.boot.k(Bool.and(String.eq(J.Json.get_str(o, "stamp", "?"), stamp), String.eq(J.Json.get_str(o, "setting", "?"), setting)), o, stamp, cs)

def Rt.checked(rt: Rt, +found: Bool, r: P.Ran) -> Rt:
  match rt r:
    case Rt{+cli, +fork, +version, commit, +h} P.Ran{+code, +out}:
      +v = Bool.pick(String, fork, version, Bool.pick(String, U32.is_eq(code, 0), KH.KH.why(out), ""))
      +runs = U32.is_eq(code, 0)
      +status = KH.KH.status(found, runs, fork, v, J.Json.get_nat(h, "api", 0n))
      +why = Bool.pick(String, found, Bool.pick(String, runs, "", Bool.pick(String, String.is_empty(KH.KH.why(out)), "kicad-cli version failed or timed out", KH.KH.why(out))), "no kicad-cli at " ++ cli)
      Rt{cli, fork, v, commit, KH.KH.put.obj(KH.KH.put.obj(KH.KH.put.obj(h, "runs", J.Flag{runs}), "status", J.Str{status}), "why", J.Str{why})}

def Rt.version(+rt: Rt) -> IO(Rt):
  IO.bind(P.Ran, Rt, P.Proc.run("sh", PORT.Port.timeout(["10", Rt.cli(rt), "version"]), "/"), r => IO.pure(Rt, Rt.checked(rt, True{}, r)))

# what `api-server --describe` printed, when it is the fork's manifest
# (contract: the dynamic identity for runtimes without a manifest.json)
def Rt.described.k(fork: Bool, +rt: Rt, +out: String) -> Rt:
  match fork:
    case True{}: Rt.set(Rt.set(Rt.manifest(Rt.cli(rt), out), "source", J.Str{J.Json.get_str(Rt.health(rt), "source", "")}), "described", J.Flag{True{}})
    case False{}: rt

def Rt.described(+rt: Rt, r: P.Ran) -> Rt:
  match r:
    case P.Ran{0, +out}: Rt.described.k(KH.KH.manifest.fork(KH.KH.manifest(out)), rt, out)
    case P.Ran{_, _}: rt

def Rt.describe.k(fork: Bool, +rt: Rt) -> IO(Rt):
  match fork:
    case True{}: IO.pure(Rt, rt)
    case False{}: IO.bind(P.Ran, Rt, P.Proc.run("sh", PORT.Port.timeout(["10", Rt.cli(rt), "api-server", "--describe"]), "/"), r => IO.pure(Rt, Rt.described(rt, r)))

# no manifest.json: ask the CLI itself (stock KiCad lacks the flag and
# fails, so it stays "not the fork")
def Rt.describe(+rt: Rt) -> IO(Rt):
  Rt.describe.k(Rt.fork(rt), rt)

def Rt.run.k(found: Bool, +rt: Rt) -> IO(Rt):
  match found:
    case True{}:
      IO.bind(Rt, Rt, Rt.describe(rt), r => Rt.version(r))
    case False{}: IO.pure(Rt, Rt.checked(rt, False{}, P.Ran{127, ""}))

def Rt.run(+rt: Rt) -> IO(Rt):
  IO.bind(Bool, Rt, Io.is_file(Rt.cli(rt)), found => Rt.run.k(found, rt))

def Rt.save(+home: String, +rt: Rt) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", "umask 077; printf '%s' \"$2\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"", "sh", home ++ "/kicad-health.json", J.Json.show(Rt.json(rt))], "/"), _ => IO.pure(Unit, Unit{}))

def Rt.refresh.k(check: Bool, +home: String, +setting: String, +stamp: String, +cs: List<&2, KCandidate>, can: Bool, +last: Rt) -> IO(Rt):
  match check:
    case False{}: IO.pure(Rt, Rt.set(last, "can_install", J.Flag{can}))
    case True{}:
      do IO<Rt>:
        rt : Rt <- Rt.choose(cs)
        +rt2 : Rt <- Rt.run(rt)
        chosen_stamp : String <- Rt.stamps(Rt.watch(cs, Rt.cli(rt2)))
        return Rt.set(Rt.set(Rt.set(rt2, "setting", J.Str{setting}), "stamp", J.Str{chosen_stamp}), "can_install", J.Flag{can})

def Rt.library.cleared(had_error: Bool, +rt: Rt) -> Rt:
  match had_error:
    case True{}: Rt.set(Rt.set(Rt.set(rt, "status", J.Str{Rt.cache.status(Rt.json(rt))}), "why", J.Str{""}), "library_error", J.Str{""})
    case False{}: Rt.set(rt, "library_error", J.Str{""})

def Rt.library.result(fine: Bool, +why: String, +rt: Rt) -> Rt:
  match fine:
    case True{}: Rt.library.cleared(Bool.not(String.is_empty(J.Json.get_str(Rt.health(rt), "library_error", ""))), rt)
    case False{}: Rt.set(Rt.set(Rt.set(rt, "library_error", J.Str{why}), "why", J.Str{why}), "status", J.Str{"failed"})

def Rt.library.found(found: Bool, +rt: Rt) -> IO(Rt):
  match found:
    case False{}: IO.pure(Rt, rt)
    case True{}: IO.bind(String, Rt, KL.Library.prepare(Rt.root(Rt.cli(rt))), +why => IO.pure(Rt, Rt.library.result(String.is_empty(why), why, rt)))

# Background selection, never ordinary boot: activate before health is sent
# and before new agent children inherit the selected KiCad environment.
def Rt.library(+rt: Rt) -> IO(Rt):
  IO.bind(Bool, Rt, Io.is_file(Rt.cli(rt)), found => Rt.library.found(found, rt))

def Rt.refresh(+home: String, +setting: String, +exe: String, force: Bool, +last: Rt) -> IO(Rt):
  do IO<Rt>:
    explicit : String <- Rt.env("BACKPLANE_KICAD_CLI")
    +cs : List<&2, KCandidate> <- Rt.list(setting, explicit, exe, home)
    +stamp : String <- Rt.stamps(Rt.watch(cs, Rt.cli(last)))
    os : P.Ran <- P.Proc.run("uname", ["-sm"], "/")
    rt : Rt <- Rt.refresh.k(Bool.or(force, Bool.or(String.eq(Rt.status(last), "checking"), Bool.not(String.eq(J.Json.get_str(Rt.health(last), "stamp", ""), stamp)))), home, setting, stamp, cs, KI.Install.available(KI.Install.platform(Rt.platform(os))), last)
    Rt.library(rt)

# Project files
# -------------

def Files.pick.hit(ns: List<&2, String>, +ext: String) -> Bool:
  match ns:
    case n <> _:
      String.ends_with(n, ext)
    case Nil{}:
      False{}

def Files.pick.go(ns: List<&2, String>, hit: Bool, +ext: String) -> String:
  match ns hit:
    case n <> _ True{}:
      n
    case _ <> +t False{}:
      Files.pick.go(t, Files.pick.hit(t, ext), ext)
    case Nil{} _:
      ""

# the first name with the extension ("" when none)
def Files.pick(+ns: List<&2, String>, +ext: String) -> String:
  Files.pick.go(ns, Files.pick.hit(ns, ext), ext)

def Files.stem(+name: String, +ext: String) -> String:
  String.take(name, Nat.sub(String.length(name), String.length(ext)))

def Files.abs(+root: String, +rel: String) -> String:
  Bool.pick(String, String.is_empty(rel), "", Bool.pick(String, String.starts_with(rel, "/"), rel, root ++ "/" ++ rel))

def Files.named.k(found: Bool, +exact: String, +first: String) -> String:
  Bool.pick(String, found, exact, first)

def Files.in(ns: List<&2, String>, +want: String) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case n <> t:
      Bool.or(String.eq(n, want), Files.in(t, want))

# prefer the file named like the project, else the first of its kind
def Files.named(+ns: List<&2, String>, +stem: String, +ext: String) -> String:
  Files.named.k(Files.in(ns, stem ++ ext), stem ++ ext, Files.pick(ns, ext))

# a path's folder ("" for a name at the root) and its last part
def Files.last(+p: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(p, '/')), p)

def Files.dir(+p: String) -> String:
  String.take(p, Nat.sub(Nat.sub(String.length(p), String.length(Files.last(p))), Bool.pick(Nat, String.contains(p, "/"), 1n, 0n)))

def Files.in.dir(+dir: String, +name: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(dir), String.is_empty(name)), name, dir ++ "/" ++ name)

# which .kicad_pro is the project's (a path from the root): the one
# .backplane.json names ("project"), else one in the root, else the first
# found below it (Files.found: shallowest first)
def Files.pro(+named: String, +top: String, +found: String) -> String:
  Bool.pick(String, String.is_empty(named), Bool.pick(String, String.is_empty(top), Maybe.default(&2, String, List.head(&2, String, String.lines(found)), ""), top), named)

# .backplane.json may name "project" (a .kicad_pro), "pcb" and "schematic"
# (all project-relative); the board and schematic are otherwise found
# beside the .kicad_pro (ns: the names in its folder)
def Files.of(+root: String, +pro: String, +ns: List<&2, String>, +cfg: J.Json, +all: String) -> Files:
  +stem = Files.stem(Files.last(pro), ".kicad_pro")
  Files{Files.abs(root, pro),
    Files.abs(root, Bool.pick(String, String.is_empty(J.Json.get_str(cfg, "pcb", "")), Files.in.dir(Files.dir(pro), Files.named(ns, stem, ".kicad_pcb")), J.Json.get_str(cfg, "pcb", ""))),
    Files.abs(root, Bool.pick(String, String.is_empty(J.Json.get_str(cfg, "schematic", "")), Files.in.dir(Files.dir(pro), Files.named(ns, stem, ".kicad_sch")), J.Json.get_str(cfg, "schematic", ""))),
    all}

# every .kicad_pro under the root, four folders deep, as paths from it,
# shallowest first, folders of copies (archive, build, old, out...: the
# board picker's, order.bend's Boards.copies) after the rest (hidden
# folders, KiCad's backups and node_modules left
# out; a worktree under .backplane is not the project's)
# (the answer is kept for 30 s per folder in the user's cache: a start asks
# every time, and the walk is the slow part)
def Files.found.sh() -> String:
  "b=\"${XDG_CACHE_HOME:-$HOME/.cache}/backplane-bend/found\"; c=\"$b/$(printf %s \"$1\" | cksum | cut -d' ' -f1)\"; now=$(date +%s); if [ -f \"$c\" ]; then t=$(head -n 1 \"$c\"); case $t in ''|*[!0-9]*) ;; *) if [ $((now - t)) -lt 30 ]; then tail -n +2 \"$c\"; exit 0; fi;; esac; fi; cd \"$1\" 2>/dev/null || exit 0; out=$(find . -mindepth 1 -maxdepth 4 \\( -type d \\( -name '.*' -o -name node_modules -o -name '*-backups' \\) -prune \\) -o -name '*.kicad_pro' -print 2>/dev/null | sed 's|^\\./||' | awk -F/ '{c = (tolower($0) ~ /(^|\\/)((archive|build|output|export)[^\\/]*|old|out|tmp|temp|staging|scratch|renders)\\//) ? 1 : 0; print c \"\\t\" NF \"\\t\" $0}' | LC_ALL=C sort -t \"$(printf '\\t')\" -k1,1n -k2,2n -k3 | cut -f3- | head -n 50); mkdir -p \"$b\" 2>/dev/null && { printf '%s\\n%s\\n' \"$now\" \"$out\" > \"$c.tmp\" 2>/dev/null && mv \"$c.tmp\" \"$c\"; }; [ -z \"$out\" ] || printf '%s\\n' \"$out\""

def Files.found.out(r: P.Ran) -> String:
  match r:
    case P.Ran{0, +out}:
      String.trim(out)
    case _:
      ""

def Files.found(+root: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", Files.found.sh(), "sh", root], "/"), r => IO.pure(String, Files.found.out(r)))

def Files.read.at(+root: String, +pro: String, +cfg: J.Json, +all: String) -> IO(Files):
  IO.bind(List<&2, String>, Files, Io.names(Bool.pick(String, String.is_empty(Files.dir(pro)), root, Files.abs(root, Files.dir(pro)))), ns => IO.pure(Files, Files.of(root, pro, ns, cfg, all)))

def Files.read.cfg(+root: String, +cfg: J.Json) -> IO(Files):
  do IO<Files>:
    +ns : List<&2, String> <- Io.names(root)
    +all : String <- Files.found(root)
    Files.read.at(root, Files.pro(J.Json.get_str(cfg, "project", ""), Files.pick(ns, ".kicad_pro"), all), cfg, all)

def Files.read(+root: String) -> IO(Files):
  IO.bind(String, Files, Io.text(root ++ "/.backplane.json"), text => Files.read.cfg(root, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})))

# KiStack
# -------

# KiStack's KiCad skills (github.com/BackplaneWorks/kistack, formerly
# American-Embedded/KiStack, which GitHub redirects there) come two
# ways, and every Claude thread loads the one in use with --plugin-dir (the
# other agents get its skills in their <skills> block, server.bend's
# Skills.scan):
# - synced: at every start, once the hub serves, its default branch's newest
#   commit is fetched in the background into <home>/kistack (KiStack.sync;
#   server.bend's KSync.*). It wins whenever it is complete (KiStack.pick).
# - bundled: scripts/build-kistack.sh puts the pinned commit (checked
#   against deploy/kistack.lock) in <exe dir>/kistack (KiStack.dir), the
#   offline fallback.
# Either way only skills/** and the LICENSE are taken (KiStack.kept), with a
# plugin.json of ours (KiStack.manifest): never the repository's own
# manifest, hooks, .mcp.json, commands or agents, which would run in every
# Claude session (an older hub took its whole newest commit: f31414b9d).
# The desktop app's own hub runs from <prefix>/desktop/runtime/resources/
# backplane-service in a Linux install (scripts/install-release.sh), which
# puts KiStack in <prefix>/kistack, beside the classic hub: that one is its.
def KiStack.desktop() -> String:
  "/desktop/runtime/resources/backplane-service"

def KiStack.dir.k(desk: Bool, +exe: String) -> String:
  match desk:
    case True{}:
      String.take(exe, Nat.sub(String.length(exe), String.length(KiStack.desktop()))) ++ "/kistack"
    case False{}:
      exe ++ "/kistack"

# the bundled copy
def KiStack.dir(+exe: String) -> String:
  KiStack.dir.k(String.ends_with(exe, KiStack.desktop()), exe)

# the synced copy, in the hub's home
def KiStack.home(+home: String) -> String:
  home ++ "/kistack"

# where a sync stages the archive and the new copy: in the home too, so
# the copy moves into place by a rename
def KiStack.stage(+home: String) -> String:
  home ++ "/kistack.sync"

# Backplane's own plugin manifest (scripts/build-kistack.sh writes the same)
def KiStack.manifest() -> String:
  "{\"name\":\"kistack\",\"description\":\"KiStack KiCad skills by American Embedded\"}"

# the copy in use: the synced one when it is complete, else the bundled one
def KiStack.pick(synced: Bool, +home: String, +bundled: String) -> String:
  match synced:
    case True{}:
      KiStack.home(home)
    case False{}:
      bundled

# a Claude thread's --plugin-dir/--add-dir: the synced copy when it is
# complete, else what the boot found (the bundled copy, or none)
def KiStack.dirs(synced: Bool, +home: String, boot: List<&2, String>) -> List<&2, String>:
  match synced:
    case True{}:
      [KiStack.home(home)]
    case False{}:
      boot

# what a sync keeps of the repository (a path inside it): skills/** and its
# LICENSE, nothing else
def KiStack.kept(+path: String) -> Bool:
  Bool.or(String.starts_with(path, "skills/"), String.eq(path, "LICENSE"))

# Commits
# -------

def KiStack.hex.k(+c: U32) -> Bool:
  Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 102)))

def KiStack.hex(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(KiStack.hex.k(c), KiStack.hex(t))

def KiStack.sha.k(forty: Bool, +s: String) -> String:
  match forty:
    case True{}:
      Bool.pick(String, KiStack.hex(s), s, "")
    case False{}:
      ""

def KiStack.first(+text: String) -> String:
  String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(text)), ""))

def KiStack.rev.of(+line: String) -> String:
  KiStack.sha.k(Nat.is_eq(String.length(line), 40n), line)

# the commit a REVISION file (or GitHub's answer) names: its first line when
# that is 40 lowercase hex digits, else "" (a copy from a local source says
# "local", never the newest commit)
def KiStack.rev(+text: String) -> String:
  KiStack.rev.of(KiStack.first(text))

# the installed commit a sync compares with: only a complete copy's
def KiStack.have(ok: Bool, +text: String) -> String:
  match ok:
    case True{}:
      KiStack.rev(text)
    case False{}:
      ""

# a script's last non-empty line
def KiStack.last(ls: List<&2, String>, +acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      KiStack.last(t, Bool.pick(String, String.is_empty(String.trim(l)), acc, String.trim(l)))

def KiStack.got.ok(local: Bool, +r: String) -> String:
  match local:
    case True{}:
      r
    case False{}:
      KiStack.rev(r)

def KiStack.got.k(got: Bool, +r: String) -> String:
  match got:
    case True{}:
      KiStack.got.ok(String.eq(r, "local"), r)
    case False{}:
      ""

def KiStack.got.w(+w: String) -> String:
  KiStack.got.k(String.starts_with(w, "got "), String.drop(w, 4n))

# what the fetch got ("got <commit>" or "got local"); "" when it stopped
# ("up to date at <commit>", "skip: why", "fail: why", or nothing when it
# was timed out)
def KiStack.got(+out: String) -> String:
  KiStack.got.w(KiStack.last(String.lines(out), ""))

# The archive
# -----------
# A tar listing's entries all sit in one top folder (GitHub's
# "kistack-<commit>/"); the members extracted are <top>/skills and, when
# there is one, <top>/LICENSE (each with what is under it, KiStack.kept).

def KiStack.part(+l: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(l, '/')), "")

def KiStack.top.step(+acc: String, +l: String) -> String:
  Bool.pick(String, Bool.or(Bool.not(String.is_empty(acc)), Bool.or(String.is_empty(l), String.eq(l, "pax_global_header"))), acc, KiStack.part(l))

# the top folder: the first part of the first entry
def KiStack.top(ls: List<&2, String>, +acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      KiStack.top(t, KiStack.top.step(acc, l))

# an entry's path inside the repository ("" when it is not under top)
def KiStack.inside(+top: String, +name: String) -> String:
  Bool.pick(String, String.starts_with(name, top ++ "/"), String.drop(name, Nat.add(String.length(top), 1n)), "")

def KiStack.in.k(+p: String, +root: String) -> Bool:
  Bool.and(KiStack.kept(p), Bool.or(String.eq(p, root), String.starts_with(p, root ++ "/")))

# whether an entry is kept and under root ("skills" or "LICENSE")
def KiStack.in(+top: String, +root: String, +name: String) -> Bool:
  KiStack.in.k(KiStack.inside(top, name), root)

def KiStack.any(ls: List<&2, String>, +top: String, +root: String, acc: Bool) -> Bool:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      KiStack.any(t, top, root, Bool.or(acc, KiStack.in(top, root, l)))

def KiStack.lic(has: Bool, +top: String) -> List<&2, String>:
  match has:
    case True{}:
      [top ++ "/LICENSE"]
    case False{}:
      Nil{}

def KiStack.members.k(skills: Bool, lic: Bool, +top: String) -> List<&2, String>:
  match skills:
    case True{}:
      (top ++ "/skills") <> KiStack.lic(lic, top)
    case False{}:
      Nil{}

def KiStack.members.of(+ls: List<&2, String>, +top: String) -> List<&2, String>:
  KiStack.members.k(Bool.and(Bool.not(String.is_empty(top)), KiStack.any(ls, top, "skills", False{})), KiStack.any(ls, top, "LICENSE", False{}), top)

# the members to extract, from the archive's listing (tar -tzf): none when
# it has no skills/ (the install then stops)
def KiStack.members(+listing: String) -> List<&2, String>:
  KiStack.members.of(String.lines(listing), KiStack.top(String.lines(listing), ""))

# The sync
# --------
# Two scripts, POSIX sh and nothing GNU (macOS runs them as is), each under
# a time limit. On Windows the fetch says so and stops: the bundled copy is
# used there. Whatever fails, the copy in place stays.

# why a hub does not sync ("" when it does): turned off; a hub on a copied
# log (core/copy.bend: no network); a test hub (stand-in agents,
# BACKPLANE_AGENTS) with no local source, unless BACKPLANE_KISTACK_SYNC=1
def KiStack.off.k(no: Bool, yes: Bool, +agents: String, +src: String) -> String:
  match no:
    case True{}:
      "BACKPLANE_KISTACK_SYNC=0"
    case False{}:
      Bool.pick(String, Bool.or(yes, Bool.or(String.is_empty(agents), Bool.not(String.is_empty(src)))), "",
        "stand-in agents (BACKPLANE_AGENTS) and no BACKPLANE_KISTACK_SOURCE")

def KiStack.off(+sync: String, copy: Bool, +agents: String, +src: String) -> String:
  match copy:
    case True{}:
      "a hub on a copied log (BACKPLANE_COPY=1) reaches no network"
    case False{}:
      KiStack.off.k(String.eq(sync, "0"), String.eq(sync, "1"), agents, src)

# The fetch: `get.sh DEST STAGE SOURCE HAVE MAX UNPACKED`. A swap cut short
# is undone first (DEST.old back). With no SOURCE it asks GitHub for the
# default branch's newest commit, stops ("up to date") when that is HAVE,
# else downloads its archive; SOURCE (BACKPLANE_KISTACK_SOURCE, for
# tests) is a local .tar.gz like GitHub's or a folder, packed the same way.
# The archive is bounded (MAX bytes, UNPACKED bytes unpacked, 5000
# entries) and listed into STAGE/list. Its last line: "got <commit>" or
# "got local", "up to date at <commit>", "skip: why", "fail: why".
def KiStack.get.sh() -> String:
  "d=$1 st=$2 src=$3 have=$4 max=$5 unz=$6\n"
    ++ "case \"$(uname -s 2>/dev/null)\" in CYGWIN*|MINGW*|MSYS*|Windows*) echo 'skip: not on Windows yet (the bundled copy is used)'; exit 0;; esac\n"
    ++ "[ -d \"$d\" ] || [ ! -f \"$d.old/.claude-plugin/plugin.json\" ] || mv \"$d.old\" \"$d\"\n"
    ++ "fail() { echo \"fail: $*\"; rm -rf \"$st\"; exit 1; }\n"
    ++ "rm -rf \"$st\" && mkdir -p \"$st\" || fail \"cannot write $st\"\n"
    ++ "z=$st/k.tgz\n"
    ++ "if [ -n \"$src\" ]; then\n"
    ++ "  rev=local\n"
    ++ "  if [ -d \"$src\" ]; then tar -czf \"$z\" --exclude .git -C \"$(dirname \"$src\")\" \"$(basename \"$src\")\" 2>/dev/null || fail \"cannot pack $src\"\n"
    ++ "  elif [ -f \"$src\" ]; then cp \"$src\" \"$z\" || fail \"cannot read $src\"\n"
    ++ "  else fail \"no KiStack at $src\"; fi\n"
    ++ "else\n"
    ++ "  rev=$(curl -fsSL --proto =https --proto-redir =https --connect-timeout 15 --max-time 30 -H 'Accept: application/vnd.github.sha' https://api.github.com/repos/BackplaneWorks/kistack/commits/HEAD 2>\"$st/err\") || fail \"cannot ask GitHub for the newest commit ($(head -n 1 \"$st/err\"))\"\n"
    ++ "  case $rev in ''|*[!0-9a-f]*) fail 'GitHub named no commit';; esac\n"
    ++ "  [ ${#rev} = 40 ] || fail 'GitHub named no commit'\n"
    ++ "  if [ \"$rev\" = \"$have\" ]; then rm -rf \"$st\"; echo \"up to date at $rev\"; exit 0; fi\n"
    ++ "  curl -fsSL --proto =https --proto-redir =https --connect-timeout 15 --max-time 120 --max-filesize \"$max\" -o \"$z\" \"https://codeload.github.com/BackplaneWorks/kistack/tar.gz/$rev\" 2>\"$st/err\" || fail \"the download failed ($(head -n 1 \"$st/err\"))\"\n"
    ++ "fi\n"
    ++ "[ \"$(wc -c < \"$z\" | tr -d ' ')\" -le \"$max\" ] || fail \"the archive is over $max bytes\"\n"
    ++ "[ \"$(gzip -dc < \"$z\" 2>/dev/null | head -c \"$((unz + 1))\" | wc -c | tr -d ' ')\" -le \"$unz\" ] || fail \"the archive unpacks to over $unz bytes\"\n"
    ++ "tar -tzf \"$z\" > \"$st/list\" 2>/dev/null || fail 'the archive does not list'\n"
    ++ "[ \"$(wc -l < \"$st/list\" | tr -d ' ')\" -le 5000 ] || fail 'the archive has over 5000 entries'\n"
    ++ "echo \"got $rev\"\n"

# The install: `put.sh DEST STAGE COMMIT MANIFEST MEMBERS...` extracts only
# the members (KiStack.members; none stops it), refuses a link or anything
# but files and folders among them, needs a skills/*/SKILL.md, writes our
# manifest and REVISION beside them in STAGE/new, and only then swaps:
# DEST to DEST.old, STAGE/new to DEST (the old copy back if that fails),
# DEST.old gone. Its last line: "ok <files>" or "fail: why".
def KiStack.put.sh() -> String:
  "d=$1 st=$2 rev=$3 man=$4; shift 4\n"
    ++ "fail() { echo \"fail: $*\"; rm -rf \"$st\"; exit 1; }\n"
    ++ "[ $# -gt 0 ] || fail 'the archive has no skills/'\n"
    ++ "x=$st/x n=$st/new\n"
    ++ "rm -rf \"$x\" \"$n\" && mkdir -p \"$x\" \"$n/.claude-plugin\" || fail \"cannot write $st\"\n"
    ++ "tar -xzf \"$st/k.tgz\" -C \"$x\" --strip-components=1 \"$@\" 2>\"$st/err\" || fail \"the archive did not unpack ($(head -n 1 \"$st/err\"))\"\n"
    ++ "[ -z \"$(find \"$x\" ! -type f ! -type d | head -n 1)\" ] || fail 'a link or special file among the skills: refused'\n"
    ++ "[ -n \"$(find \"$x/skills\" -name SKILL.md -type f | head -n 1)\" ] || fail 'no skills/*/SKILL.md'\n"
    ++ "mv \"$x/skills\" \"$n/skills\" || fail 'cannot stage skills/'\n"
    ++ "if [ -f \"$x/LICENSE\" ]; then mv \"$x/LICENSE\" \"$n/LICENSE\" || fail 'cannot stage the LICENSE'; fi\n"
    ++ "printf '%s\\n' \"$man\" > \"$n/.claude-plugin/plugin.json\" && printf '%s\\n' \"$rev\" > \"$n/REVISION\" || fail 'cannot write the manifest'\n"
    ++ "c=$(find \"$n/skills\" -type f | wc -l | tr -d ' ')\n"
    ++ "rm -rf \"$d.old\"\n"
    ++ "if [ -d \"$d\" ]; then mv \"$d\" \"$d.old\" || fail 'cannot move the old copy aside'; fi\n"
    ++ "if ! mv \"$n\" \"$d\"; then [ -d \"$d.old\" ] && mv \"$d.old\" \"$d\"; fail 'cannot move the new copy into place'; fi\n"
    ++ "rm -rf \"$d.old\" \"$st\"\n"
    ++ "echo \"ok $c\"\n"

# the archive's limits: 32 MB, 128 MB unpacked
def KiStack.get.args(+home: String, +src: String, +have: String) -> List<&2, String>:
  PORT.Port.timeout(["300", "sh", "-c", KiStack.get.sh(), "sh", KiStack.home(home), KiStack.stage(home), src, have, "33554432", "134217728"])

def KiStack.put.args(+home: String, +rev: String, ms: List<&2, String>) -> List<&2, String>:
  PORT.Port.timeout(List.append(&2, String, ["120", "sh", "-c", KiStack.put.sh(), "sh", KiStack.home(home), KiStack.stage(home), rev, KiStack.manifest()], ms))

def KiStack.said.k(none: Bool, +w: String) -> String:
  "kistack sync: " ++ Bool.pick(String, none, "stopped (timed out)", w)

# the log line for a sync that stopped before installing (or failed in it)
def KiStack.said(+w: String) -> String:
  KiStack.said.k(String.is_empty(w), w)

def KiStack.done.k(ok: Bool, +w: String, +rev: String, +d: String) -> String:
  match ok:
    case True{}:
      "kistack sync: " ++ rev ++ " in " ++ d ++ " (" ++ String.drop(w, 3n) ++ " files)"
    case False{}:
      KiStack.said(w)

def KiStack.done.w(+w: String, +rev: String, +d: String) -> String:
  KiStack.done.k(String.starts_with(w, "ok "), w, rev, d)

# the install's log line
def KiStack.done(+out: String, +rev: String, +d: String) -> String:
  KiStack.done.w(KiStack.last(String.lines(out), ""), rev, d)

def KiStack.out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, o}:
      o

def Io.is_dir.kind(v: U32 & U32 & U32) -> Bool:
  (k, _) = v
  U32.is_eq(k, 1)

def Io.is_dir.k(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> Bool:
  match r:
    case Done{v}:
      Io.is_dir.kind(v)
    case Fail{_}:
      False{}

def Io.is_dir(path: String) -> IO(Bool):
  do IO<Bool>:
    r : Result<&1, &1, U32 & String, U32 & U32 & U32> <- H.FS.stat(path)
    return Io.is_dir.k(r)

def KiStack.ready.k(none: Bool, +home: String) -> IO(Bool):
  match none:
    case True{}:
      IO.pure(Bool, False{})
    case False{}:
      do IO<Bool>:
        m : Bool <- Io.is_file(KiStack.home(home) ++ "/.claude-plugin/plugin.json")
        s : Bool <- Io.is_dir(KiStack.home(home) ++ "/skills")
        return Bool.and(m, s)

# whether the synced copy is complete (our manifest and skills/)
def KiStack.ready(+home: String) -> IO(Bool):
  KiStack.ready.k(String.is_empty(home), home)

# the copy in use now (a turn's skills, the composer's `$` list)
def KiStack.live(+home: String, +exe: String) -> IO(String):
  IO.bind(Bool, String, KiStack.ready(home), ok => IO.pure(String, KiStack.pick(ok, home, KiStack.dir(exe))))

def KiStack.put(none: Bool, +home: String, +rev: String, +out: String) -> IO(String):
  match none:
    case True{}:
      IO.pure(String, KiStack.said(KiStack.last(String.lines(out), "")))
    case False{}:
      do IO<String>:
        ls : String <- Io.text(KiStack.stage(home) ++ "/list")
        r : P.Ran <- P.Proc.run("sh", KiStack.put.args(home, rev, KiStack.members(ls)), "/")
        return KiStack.done(KiStack.out(r), rev, KiStack.home(home))

# KiStack's newest commit (or BACKPLANE_KISTACK_SOURCE's copy) into
# <home>/kistack; the one log line that says how it went
def KiStack.sync(+home: String, +src: String) -> IO(String):
  do IO<String>:
    ok : Bool <- KiStack.ready(home)
    rv : String <- Io.text(KiStack.home(home) ++ "/REVISION")
    r : P.Ran <- P.Proc.run("sh", KiStack.get.args(home, src, KiStack.have(ok, rv)), "/")
    +out : String = KiStack.out(r)
    KiStack.put(String.is_empty(KiStack.got(out)), home, KiStack.got(out), out)

def KiCad.install.k(r: P.Ran) -> String:
  match r:
    case P.Ran{0, _}:
      ""
    case P.Ran{_, out}:
      "KiCad install failed: " ++ String.trim(out)

# The fork, installed on request
# ------------------------------

# The backend and can_install use the same catalog. Unsupported hosts fail
# before directory creation, download, or extraction. Stage before promotion.
def KiCad.install.on(p: Maybe<&2, KI.Package>, data: String) -> IO(String):
  match p:
    case None{}:
      IO.pure(String, "No published Backplane KiCad package for this hub OS and architecture; set a compatible runtime path")
    case Some{KI.Package{platform, version, release, asset, sha, format, cli}}:
      do IO<String>:
        r : P.Ran <- P.Proc.run("sh", ["-c", KI.Install.script(), "sh", data ++ "/kicad", platform, version, release, asset, sha, format, cli], "/")
        return KiCad.install.k(r)

def KiCad.install(+data: String) -> IO(String):
  do IO<String>:
    os : P.Ran <- P.Proc.run("uname", ["-sm"], "/")
    KiCad.install.on(KI.Install.platform(Rt.platform(os)), data)

# Opening a project
# -----------------

def KiCad.open.said(r: P.Ran) -> String:
  match r:
    case P.Ran{0, _}:
      ""
    case P.Ran{_, out}:
      String.trim(out)

def KiCad.open.script() -> String:
  "set -eu\nr=$1\nif [ -n \"$2\" ]; then set -- kicad \"$2\"; elif [ -n \"$3\" ]; then set -- pcbnew \"$3\"; elif [ -n \"$4\" ]; then set -- eeschema \"$4\"; else echo 'no KiCad project here'; exit 1; fi\ng=$r/bin/$1\n[ -x \"$g\" ] || g=$g.exe\n[ -x \"$g\" ] || { echo \"Backplane KiCad has no $1\"; exit 1; }\nif command -v setsid >/dev/null 2>&1; then\n  setsid -f \"$g\" \"$2\" </dev/null >/dev/null 2>&1\nelse\n  perl -MPOSIX -e 'defined(my $pid=fork) or exit 127; exit 0 if $pid; POSIX::setsid(); exec {$ARGV[0]} @ARGV or exit 127' \"$g\" \"$2\" </dev/null >/dev/null 2>&1\nfi\n"

def KiCad.open.ready(fork: Bool, +cli: String, +fs: Files) -> IO(String):
  match fork fs:
    case True{} Files{+pro, +pcb, +sch, _}:
      do IO<String>:
        native : List<&2, String> <- KiCad.native_args([Rt.root(cli), pro, pcb, sch])
        r : P.Ran <- P.Proc.run("sh", List.append(&2, String, ["-c", KiCad.open.script(), "sh"], native), "/")
        return KiCad.open.said(r)
    case False{} _:
      IO.pure(String, "Install Backplane KiCad (Settings) to open projects in it")

def KiCad.open.prepared(fine: Bool, why: String, fork: Bool, cli: String, fs: Files) -> IO(String):
  match fine:
    case True{}: KiCad.open.ready(fork, cli, fs)
    case False{}: IO.pure(String, why)

def KiCad.open.k(fork: Bool, +cli: String, fs: Files) -> IO(String):
  match fork:
    case True{}: IO.bind(String, String, KiCad.prepare(cli), +why => KiCad.open.prepared(String.is_empty(why), why, True{}, cli, fs))
    case False{}: KiCad.open.ready(False{}, cli, fs)

# open a project's .kicad_pro in the bundled KiCad (its board or schematic
# editor when there is no .kicad_pro), detached; answers "" or an error.
# Only the Backplane fork: its launcher brings the official libraries
def KiCad.open(rt: Rt, +fs: Files) -> IO(String):
  match rt:
    case Rt{+cli, fork, _, _, _}:
      KiCad.open.k(fork, cli, fs)
