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

# KiStack's skills come with the app: scripts/build-kistack.sh puts the
# pinned commit's skills/ (checked against deploy/kistack.lock), its LICENSE
# and a plugin.json of ours in <exe dir>/kistack, which every Claude thread
# loads with --plugin-dir. The hub never fetches KiStack (it took the
# repository's newest commit at each start, its own plugin manifest too, so
# a hook or an .mcp.json there would have run in every Claude session).
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

def KiStack.dir(+exe: String) -> String:
  KiStack.dir.k(String.ends_with(exe, KiStack.desktop()), exe)

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
