import Base
import ./model.bend as M
import ./json.bend as J

# Runtime health and the one notice every client renders. This module
# starts no processes and contains no notification or alert decisions.
type KAct is Data:
  KAct{label: String, action: String, value: String}

type KNote is Data:
  KNote{key: String, title: String, body: String, acts: List<&2, KAct>}

type KManifest is Data:
  KManifest{fork: Bool, version: String, commit: String, api: Nat, caps: List<&2, String>, upstream: String, kversion: String}

def KH.min_version() -> String:
  "10.99.0"

def KH.min_api() -> Nat:
  1n

# Parse only the numeric core, allowing the fork's -bp.N suffix. No
# Nat.read: symbolic laws over the early precedence cases stay cheap.
def KH.num(s: String, +n: Nat) -> Nat:
  match s:
    case SCon{Chr{48}, t}: KH.num(t, Nat.mul(n, 10n))
    case SCon{Chr{49}, t}: KH.num(t, 1n+Nat.mul(n, 10n))
    case SCon{Chr{50}, t}: KH.num(t, 2n+Nat.mul(n, 10n))
    case SCon{Chr{51}, t}: KH.num(t, 3n+Nat.mul(n, 10n))
    case SCon{Chr{52}, t}: KH.num(t, 4n+Nat.mul(n, 10n))
    case SCon{Chr{53}, t}: KH.num(t, 5n+Nat.mul(n, 10n))
    case SCon{Chr{54}, t}: KH.num(t, 6n+Nat.mul(n, 10n))
    case SCon{Chr{55}, t}: KH.num(t, 7n+Nat.mul(n, 10n))
    case SCon{Chr{56}, t}: KH.num(t, 8n+Nat.mul(n, 10n))
    case SCon{Chr{57}, t}: KH.num(t, 9n+Nat.mul(n, 10n))
    case _: n

def KH.part(xs: List<&2, String>, n: Nat) -> Nat:
  match xs n:
    case x <> _ 0n: KH.num(x, 0n)
    case _ <> t 1n+k: KH.part(t, k)
    case Nil{} _: 0n

def KH.version.ge(+v: String, +min: String) -> Bool:
  +a = String.split(v, '.')
  +b = String.split(min, '.')
  +am = KH.part(a, 0n)
  +bm = KH.part(b, 0n)
  +an = KH.part(a, 1n)
  +bn = KH.part(b, 1n)
  Bool.or(Nat.is_gt(am, bm), Bool.and(Nat.is_eq(am, bm),
    Bool.or(Nat.is_gt(an, bn), Bool.and(Nat.is_eq(an, bn), Nat.is_ge(KH.part(a, 2n), KH.part(b, 2n))))))

def KH.status.age(ok: Bool) -> String:
  match ok:
    case True{}: "ok"
    case False{}: "old"

def KH.status(found: Bool, runs: Bool, fork: Bool, +version: String, api: Nat) -> String:
  match found runs fork:
    case False{} _ _: "missing"
    case True{} False{} _: "broken"
    case True{} True{} False{}: "stock"
    case True{} True{} True{}:
      KH.status.age(Bool.and(KH.version.ge(version, KH.min_version()), Nat.is_ge(api, KH.min_api())))

def KH.why(+text: String) -> String:
  String.take(String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(text)), "")), 160n)

def KH.install(can: Bool, +status: String) -> KAct:
  match can:
    case True{}:
      KAct{Bool.pick(String, String.eq(status, "old"), "Update", "Install"), "kicad-install", ""}
    case False{}:
      KAct{"Download", "link", "https://github.com/i2cjak/Backplane_KiCad/releases"}

def KH.acts(can: Bool, status: String) -> List<&2, KAct>:
  [KH.install(can, status), KAct{"Set path", "kicad-path", ""}, KAct{"Retry", "kicad-retry", ""}]

def KH.note(+status: String, +cli: String, +version: String, title: String, body: String, can: Bool) -> Maybe<&2, KNote>:
  Some{KNote{status ++ ":" ++ cli ++ ":" ++ version, title, body, KH.acts(can, status)}}

def KH.notice(+status: String, +cli: String, +version: String, +why: String, can_install: Bool) -> Maybe<&2, KNote>:
  match status:
    case "missing":
      KH.note(status, cli, version, "KiCad isn't installed", "Backplane KiCad runs ERC, DRC, exports and agent edits. Install it, or point Backplane at a copy you have.", can_install)
    case "stock":
      KH.note(status, cli, version, "This is KiCad " ++ version ++ ", not Backplane KiCad", "Exports and checks work. Agents can't drive KiCad over IPC until Backplane KiCad is installed.", can_install)
    case "old":
      KH.note(status, cli, version, "Backplane KiCad " ++ version ++ " is too old", "Backplane needs " ++ KH.min_version() ++ " or newer (API " ++ Nat.show(KH.min_api()) ++ ").", can_install)
    case "broken":
      KH.note(status, cli, version, "KiCad at " ++ cli ++ " doesn't run", KH.why(why), can_install)
    case _: None{}

# A project's headless server stopped for good (core/kserve.bend): which
# one and why; Retry forgets its failures and the next request starts it.
# name: the project's name, project: what keys the notice
def KH.ipc.notice(+project: String, +name: String, +why: String) -> Maybe<&2, KNote>:
  Bool.pick(Maybe<&2, KNote>, String.is_empty(project), None{},
    Some{KNote{"ipc-failed:" ++ project, "Headless KiCad stopped for " ++ name, why, [KAct{"Retry", "kicad-retry", ""}, KAct{"Settings", "kicad-path", ""}]}})

def KH.dismissed(+st: M.State, +key: String) -> Bool:
  String.eq(M.Setting.get(st, "kicad.notice.hidden", ""), key)

def KH.retry(n: Maybe<&2, KNote>) -> Bool:
  match n:
    case Some{KNote{_, _, _, _ <> _ <> KAct{_, "kicad-retry", _} <> Nil{}}}: True{}
    case _: False{}

def KH.has_install(n: Maybe<&2, KNote>) -> Bool:
  match n:
    case Some{KNote{_, _, _, KAct{_, "kicad-install", _} <> _}}: True{}
    case _: False{}

def KH.strings(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case J.Str{s} <> t: s <> KH.strings(t)
    case _ <> t: KH.strings(t)

def KH.manifest.obj(+o: J.Json) -> KManifest:
  KManifest{String.contains(J.Json.get_str(o, "sourceRepository", ""), "Backplane_KiCad"),
    J.Json.get_str(o, "version", J.Json.get_str(o, "kicadVersion", "")), J.Json.get_str(o, "sourceCommit", ""),
    J.Json.get_nat(o, "apiVersion", 0n), KH.strings(J.Json.elems(J.Json.get_obj(o, "capabilities"))),
    J.Json.get_str(o, "upstreamCommit", ""), J.Json.get_str(o, "kicadVersion", J.Json.get_str(o, "version", ""))}

def KH.manifest.fork(m: KManifest) -> Bool:
  match m:
    case KManifest{f, _, _, _, _, _, _}: f

def KH.manifest(text: String) -> KManifest:
  KH.manifest.obj(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))

def KH.put.keep(same: Bool, k: String, v: J.Json, rest: J.Json) -> J.Json:
  match same:
    case True{}: rest
    case False{}: J.Field{k, v, rest}

def KH.put.fields(fs: J.Json, +key: String) -> J.Json:
  match fs:
    case J.Field{+k, v, t}: KH.put.keep(String.eq(k, key), k, v, KH.put.fields(t, key))
    case _: J.End{}

def KH.put.obj(o: J.Json, +key: String, value: J.Json) -> J.Json:
  match o:
    case J.Obj{fs}: J.Obj{J.Field{key, value, KH.put.fields(fs, key)}}
    case _: J.Json.obj([J.KV{key, value}])
