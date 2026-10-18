import Base
import ./json.bend as J
import ./kihealth.bend as KH

# The headless KiCad servers the hub runs for its agents: one `kicad-cli
# api-server` per project (at most two at once), spoken to over KiCad's IPC
# API by the backplane-kipc helper (tools/kipc). This is the pure policy:
# where a server's socket lives, its command line, reading its ready line,
# the version check, the restart schedule, which server to retire for a
# third project, which commands a headless server cannot answer, and the
# words shown to the person and the agent. The IO is src/server/kserve.bend.

# one project's server. state: "starting", "ready", "failed" or "stopped".
# fails: the times (seconds) of its recent crashes. why: what went wrong.
type KSrv is Data:
  KSrv{project: String, sock: String, state: String, pid: U32, started: Nat, last: Nat, fails: List<&2, Nat>, why: String, version: String}

# a project's server that has been asked for, as one of the least-recently
# used ones for eviction
type Best is Data:
  Best{project: String, at: Nat, has: Bool}

# Lists of words
# --------------

def KS.has(xs: List<&2, String>, +want: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(String.eq(x, want), KS.has(t, want))

def KS.nth(xs: List<&2, String>, n: Nat) -> String:
  match xs n:
    case x <> _ 0n: x
    case _ <> t 1n+k: KS.nth(t, k)
    case Nil{} _: ""

def KS.last(+p: String, +sep: Char) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(p, sep)), p)

# Where a server lives
# --------------------

def KS.hex(w: Nat, +n: Nat) -> String:
  match w:
    case 0n:
      ""
    case 1n+k:
      KS.hex(k, Nat.div(n, 16n)) ++ SCon{J.Json.hex(U32.from_nat(Nat.mod(n, 16n))), SNil{}}

def KS.hash(s: String, +m: Nat, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      KS.hash(t, m, Nat.mod(Nat.add(Nat.mul(acc, m), U32.to_nat(c)), Nat.mul(65536n, 65536n)))

# a stable name for a project's folder: 16 hex digits of its absolute path
def KS.digest(+project: String) -> String:
  KS.hex(8n, KS.hash(project, 31n, 7n)) ++ KS.hex(8n, KS.hash(project, 131n, 3n))

def KS.root(+runtime_dir: String, +home: String) -> String:
  Bool.pick(String, String.is_empty(runtime_dir), home ++ "/run/kicad/", runtime_dir ++ "/backplane/kicad/")

# the project's own directory (mode 0700): its socket and lock file are
# the server's alone
def KS.dir(+runtime_dir: String, +home: String, +project: String) -> String:
  KS.root(runtime_dir, home) ++ KS.digest(project)

# Starting one
# ------------

# the runtime's API schema for the IPC helper (CONTRACT B3), only when
# the runtime says it ships one (capability "api-descriptor"; bp.2 still
# reports apiVersion 1, so capabilities decide, not the API level). ""
# makes the helper fall back to its built-in Ping/GetVersion schema.
def KS.desc.k(has: Bool, +root: String) -> String:
  match has:
    case True{}: root ++ "/share/backplane/api.desc"
    case False{}: ""

def KS.desc(+root: String, caps: List<&2, String>) -> String:
  KS.desc.k(KS.has(caps, "api-descriptor"), root)

# --idle-exit is safe only from API 2 on: bp.2 (API 1) closes a server
# with unsaved changes when it goes idle (CONTRACT "B1 amended"), so
# below API 2 the capability is dropped and the hub never passes it
def KS.drop.k(same: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match same:
    case True{}: rest
    case False{}: x <> rest

def KS.drop(xs: List<&2, String>, +gone: String) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +x <> t: KS.drop.k(String.eq(x, gone), x, KS.drop(t, gone))

def KS.caps.safe.k(new: Bool, xs: List<&2, String>) -> List<&2, String>:
  match new:
    case True{}: xs
    case False{}: KS.drop(xs, "idle-exit")

def KS.caps.safe(xs: List<&2, String>, api: Nat) -> List<&2, String>:
  KS.caps.safe.k(Nat.is_ge(api, 2n), xs)

def KS.idle.args(has: Bool) -> List<&2, String>:
  match has:
    case True{}: ["--idle-exit", "600"]
    case False{}: Nil{}

# the command line: the server exits by itself when the hub is gone
# (--parent-pid), and after ten idle minutes when the runtime can
def KS.argv(+cli: String, +pro: String, +dir: String, hub_pid: U32, caps: List<&2, String>) -> List<&2, String>:
  List.concat(&2, String, [[cli, "api-server", pro, "--socket", dir ++ "/api.sock", "--parent-pid", U32.show(hub_pid)], KS.idle.args(KS.has(caps, "idle-exit"))])

def KS.ready.prefix() -> String:
  "KiCad API server listening at "

def KS.ready.path(+p: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, String.is_empty(p), None{}, Some{p})

def KS.ready.url(+p: String) -> String:
  Bool.pick(String, String.starts_with(p, "ipc://"), String.drop(p, 6n), p)

# the socket a server printed when it started listening (the real path
# may be api-<pid>.sock when api.sock was taken)
def KS.ready(+line: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, String.starts_with(line, KS.ready.prefix()),
    KS.ready.path(KS.ready.url(String.trim(String.drop(line, String.length(KS.ready.prefix()))))), None{})

# What a build's "-bp.N" is: the manifest's version ends in it
def KS.suffix.k(+l: String, +m: String) -> String:
  Bool.pick(String, String.starts_with(l, "bp."), "-" ++ l, m)

def KS.suffix(+m: String) -> String:
  KS.suffix.k(KS.last(m, '-'), m)

# the running server is the build the manifest names: its full version
# (which may carry a git-describe part) ends with the manifest's -bp.N
def KS.version_ok(+full: String, +manifest: String) -> Bool:
  String.ends_with(full, KS.suffix(manifest))

# Crashes
# -------

def KS.window() -> Nat:
  300n

def KS.keep.k(hit: Bool, f: Nat, rest: List<&2, Nat>) -> List<&2, Nat>:
  match hit:
    case True{}: f <> rest
    case False{}: rest

# the failures of the last 300 s
def KS.keep(fs: List<&2, Nat>, +now: Nat) -> List<&2, Nat>:
  match fs:
    case Nil{}: Nil{}
    case +f <> t: KS.keep.k(Nat.is_lt(Nat.sub(now, f), KS.window()), f, KS.keep(t, now))

def KS.recent(fs: List<&2, Nat>, +now: Nat) -> Nat:
  match fs:
    case Nil{}: 0n
    case +f <> t: Nat.add(Bool.pick(Nat, Nat.is_lt(Nat.sub(now, f), KS.window()), 1n, 0n), KS.recent(t, now))

def KS.delay(+n: Nat) -> Maybe<&2, Nat>:
  Bool.pick(Maybe<&2, Nat>, Nat.is_le(n, 1n), Some{1n},
    Bool.pick(Maybe<&2, Nat>, Nat.is_eq(n, 2n), Some{5n},
      Bool.pick(Maybe<&2, Nat>, Nat.is_eq(n, 3n), Some{30n}, None{})))

# seconds to wait before starting again after the failure just recorded in
# fails: 1, 5, 30 for the first, second and third within 300 s; None after
# the fourth (the server is "failed" and the person is told)
def KS.retry(fails: List<&2, Nat>, +now: Nat) -> Maybe<&2, Nat>:
  KS.delay(KS.recent(fails, now))

def KS.fails.add.k(clean: Bool, fs: List<&2, Nat>, +now: Nat) -> List<&2, Nat>:
  match clean:
    case True{}: fs
    case False{}: now <> KS.keep(fs, now)

# a failure is recorded unless the server left cleanly (code 0: idle-exit
# or Shutdown); a clean exit is never a failure
def KS.fails.add(code: U32, fs: List<&2, Nat>, +now: Nat) -> List<&2, Nat>:
  KS.fails.add.k(U32.is_eq(code, 0), fs, now)

# The records
# -----------

def KS.project(s: KSrv) -> String:
  match s:
    case KSrv{p, _, _, _, _, _, _, _, _}: p

def KS.sock(s: KSrv) -> String:
  match s:
    case KSrv{_, k, _, _, _, _, _, _, _}: k

def KS.state(s: KSrv) -> String:
  match s:
    case KSrv{_, _, st, _, _, _, _, _, _}: st

def KS.started.of(s: KSrv) -> Nat:
  match s:
    case KSrv{_, _, _, _, sa, _, _, _, _}: sa

def KS.last.of(s: KSrv) -> Nat:
  match s:
    case KSrv{_, _, _, _, _, la, _, _, _}: la

def KS.fails.of(s: KSrv) -> List<&2, Nat>:
  match s:
    case KSrv{_, _, _, _, _, _, fl, _, _}: fl

def KS.why.of(s: KSrv) -> String:
  match s:
    case KSrv{_, _, _, _, _, _, _, w, _}: w

def KS.version.of(s: KSrv) -> String:
  match s:
    case KSrv{_, _, _, _, _, _, _, _, v}: v

# a server asked for just now
def KS.new(+project: String, +now: Nat) -> KSrv:
  KSrv{project, "", "starting", 0, now, now, Nil{}, "", ""}

# its state changed (and the reason, "" when there is none)
def KS.set(s: KSrv, +state: String, +why: String) -> KSrv:
  match s:
    case KSrv{p, k, _, pid, st, la, fl, _, v}: KSrv{p, k, state, pid, st, la, fl, why, v}

# asked for again at this time (what eviction orders by)
# a server that refused Shutdown because a document has unsaved changes
# (CONTRACT "B1 amended"): not evictable until it is asked for again
def KS.held() -> String:
  "unsaved changes"

def KS.touch.why(+w: String) -> String:
  Bool.pick(String, String.eq(w, KS.held()), "", w)

def KS.touch(s: KSrv, +now: Nat) -> KSrv:
  match s:
    case KSrv{p, k, st, pid, sa, _, fl, +w, v}: KSrv{p, k, st, pid, sa, now, fl, KS.touch.why(w), v}

# it refused Shutdown: keep it, never force it
def KS.hold(s: KSrv) -> KSrv:
  match s:
    case KSrv{p, k, st, pid, sa, la, fl, _, v}: KSrv{p, k, st, pid, sa, la, fl, KS.held(), v}

# it answered Ping and GetVersion
def KS.up(s: KSrv, +sock: String, +version: String, +now: Nat) -> KSrv:
  match s:
    case KSrv{p, _, _, pid, sa, _, fl, _, _}: KSrv{p, sock, "ready", pid, sa, now, fl, "", version}

# started again (after a backoff, or asked for after it stopped): no socket
# yet; the failures stay on record
def KS.starting(s: KSrv, +now: Nat) -> KSrv:
  match s:
    case KSrv{p, _, _, pid, _, _, fl, _, v}: KSrv{p, "", "starting", pid, now, now, fl, "", v}

def KS.listening(s: KSrv, +sock: String) -> KSrv:
  match s:
    case KSrv{p, _, st, pid, sa, la, fl, w, v}: KSrv{p, sock, st, pid, sa, la, fl, w, v}

# the failures forgotten (the person pressed Retry), and the server left
# stopped so the next request starts it
def KS.cleared(s: KSrv) -> KSrv:
  match s:
    case KSrv{p, k, _, pid, sa, la, _, _, v}: KSrv{p, k, "stopped", pid, sa, la, Nil{}, "", v}

def KS.exited.state(clean: Bool, r: Maybe<&2, Nat>) -> String:
  match clean r:
    case True{} _: "stopped"
    case False{} Some{_}: "starting"
    case False{} None{}: "failed"

def KS.exited.why(+state: String, +tail: String) -> String:
  match state:
    case "failed": "stopped four times in five minutes: " ++ tail
    case "starting": "restarting after a crash: " ++ tail
    case _: ""

def KS.exited.k3(p: String, k: String, pid: U32, sa: Nat, la: Nat, v: String, fs: List<&2, Nat>, +state: String, +tail: String) -> KSrv:
  KSrv{p, k, state, pid, sa, la, fs, KS.exited.why(state, tail), v}

def KS.exited.k2(p: String, k: String, pid: U32, sa: Nat, la: Nat, v: String, +fs: List<&2, Nat>, clean: Bool, +tail: String, +now: Nat) -> KSrv:
  KS.exited.k3(p, k, pid, sa, la, v, fs, KS.exited.state(clean, KS.retry(fs, now)), tail)

def KS.exited.k(s: KSrv, fs: List<&2, Nat>, clean: Bool, +tail: String, +now: Nat) -> KSrv:
  match s:
    case KSrv{p, k, _, pid, sa, la, _, _, v}:
      KS.exited.k2(p, k, pid, sa, la, v, fs, clean, tail, now)

# the process ended with this code: stopped on a clean exit, else the crash
# is recorded and it waits to start again ("starting") or gives up ("failed")
def KS.exited(+s: KSrv, +code: U32, +now: Nat, +tail: String) -> KSrv:
  KS.exited.k(s, KS.fails.add(code, KS.fails.of(s), now), U32.is_eq(code, 0), tail, now)

# Room for two
# ------------

def KS.active(+st: String) -> Bool:
  Bool.or(String.eq(st, "starting"), String.eq(st, "ready"))

def KS.count(xs: List<&2, KSrv>) -> Nat:
  match xs:
    case Nil{}: 0n
    case s <> t: Nat.add(Bool.pick(Nat, KS.active(KS.state(s)), 1n, 0n), KS.count(t))

def KS.best.project(b: Best) -> String:
  match b:
    case Best{p, _, _}: p

def KS.best.at(b: Best) -> Nat:
  match b:
    case Best{_, a, _}: a

def KS.best.has(b: Best) -> Bool:
  match b:
    case Best{_, _, h}: h

def KS.step.k(take: Bool, +project: String, +last: Nat, b: Best) -> Best:
  match take:
    case True{}: Best{project, last, True{}}
    case False{}: b

def KS.better(has: Bool, +at: Nat, +last: Nat) -> Bool:
  Bool.or(Bool.not(has), Nat.is_lt(last, at))

def KS.step(+s: KSrv, +b: Best) -> Best:
  KS.step.k(Bool.and(Bool.and(String.eq(KS.state(s), "ready"), Bool.not(String.eq(KS.why.of(s), KS.held()))), KS.better(KS.best.has(b), KS.best.at(b), KS.last.of(s))), KS.project(s), KS.last.of(s), b)

def KS.lru(xs: List<&2, KSrv>, b: Best) -> Best:
  match xs:
    case Nil{}: b
    case h <> t: KS.lru(t, KS.step(h, b))

def KS.evict.k(+n: Nat, b: Best) -> String:
  Bool.pick(String, Nat.is_ge(n, 2n), KS.best.project(b), "")

# with two servers starting or ready and a third wanted: the project of the
# least recently used ready one ("" when there is none to retire)
def KS.evict(+srvs: List<&2, KSrv>) -> String:
  KS.evict.k(KS.count(srvs), KS.lru(srvs, Best{"", 0n, False{}}))

def KS.state.of.k(hit: Bool, st: String, rest: String) -> String:
  match hit:
    case True{}: st
    case False{}: rest

# a project's state ("" when the hub has no record of it)
def KS.state.of(xs: List<&2, KSrv>, +project: String) -> String:
  match xs:
    case Nil{}: ""
    case +s <> t: KS.state.of.k(String.eq(KS.project(s), project), KS.state(s), KS.state.of(t, project))

def KS.plan.room(room: Bool, +victim: String, can: Bool) -> String:
  match room:
    case True{}: "start"
    case False{}: Bool.pick(String, Bool.and(can, Bool.not(String.is_empty(victim))), "evict:" ++ victim, "busy")

def KS.plan.free(+srvs: List<&2, KSrv>, caps: List<&2, String>) -> String:
  KS.plan.room(Nat.is_lt(KS.count(srvs), 2n), KS.evict(srvs), KS.has(caps, "shutdown"))

def KS.plan.k(+st: String, srvs: List<&2, KSrv>, caps: List<&2, String>) -> String:
  match st:
    case "ready": "ready"
    case "starting": "wait"
    case "failed": "failed"
    case _: KS.plan.free(srvs, caps)

# what a request for a project's server needs: "ready" (use it), "wait"
# (it is starting), "failed", "start", "evict:<project>" (retire that one,
# then start) or "busy" (two others run and none can be retired)
def KS.plan(+srvs: List<&2, KSrv>, +project: String, caps: List<&2, String>) -> String:
  KS.plan.k(KS.state.of(srvs, project), srvs, caps)

# Commands a headless server cannot answer
# ----------------------------------------

def KS.refused() -> List<&2, String>:
  ["GetSelection", "AddToSelection", "RemoveFromSelection", "ClearSelection", "SaveSelectionToString", "SyncSelection", "FocusOnItems",
   "GetVisibleLayers", "SetVisibleLayers", "GetActiveLayer", "SetActiveLayer", "GetBoardEditorAppearanceSettings", "SetBoardEditorAppearanceSettings",
   "HighlightNets", "RunAction", "RevertDocument", "OpenLibraryItem", "InteractiveMoveItems"]

# selection, view state and the other editor commands upstream's headless
# api-server answers with AS_UNIMPLEMENTED: not worth a round trip
def KS.headless_refused(+ty: String) -> Bool:
  KS.has(KS.refused(), KS.last(ty, '.'))

def KS.refusal() -> String:
  "not available in headless KiCad: pass explicit item IDs (get them with GetItems)"

def KS.busy() -> String:
  "IPC busy: 2 KiCad servers already run for other projects"

# Words
# -----

# why IPC cannot be used from the runtime's health ("" when it can)
def KS.why.health(+status: String, +version: String, +why: String) -> String:
  match status:
    case "missing": "KiCad is not installed"
    case "stock": "this is stock KiCad " ++ version ++ ", not Backplane KiCad"
    case "old": "Backplane KiCad is too old (" ++ version ++ ", needs " ++ KH.KH.min_version() ++ ")"
    case "broken": "KiCad doesn't run: " ++ why
    case "checking": "Backplane is still checking KiCad"
    case _: ""

# why IPC is not used right now ("" when it is): the setting is off, else
# the runtime's health
def KS.gate(+status: String, +version: String, +why: String, +setting: String) -> String:
  Bool.pick(String, String.eq(setting, "off"), "IPC is turned off in Settings", KS.why.health(status, version, why))

# an error that names the way round: edit the files
def KS.unavailable(+why: String, +cli: String) -> String:
  "KiCad IPC unavailable: " ++ why ++ ". Fallback: edit the file directly or use `" ++ Bool.pick(String, String.is_empty(cli), "kicad-cli", cli) ++ " pcb …`; re-read the file after."

def KS.activity(+why: String) -> String:
  "KiCad: fell back to files: " ++ why

def KS.clean(s: String) -> String:
  match s:
    case SNil{}: SNil{}
    case SCon{Chr{+c}, t}: SCon{Chr{Bool.pick(U32, Bool.or(U32.is_eq(c, 10), Bool.or(U32.is_eq(c, 13), U32.is_eq(c, 124))), 32, c)}, KS.clean(t)}

# "project|state|why" (no pids)
def KS.line(s: KSrv) -> String:
  match s:
    case KSrv{p, _, st, _, _, _, _, w, _}: p ++ "|" ++ st ++ "|" ++ KS.clean(String.take(w, 160n))

def KS.lines.go(xs: List<&2, KSrv>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case s <> t: KS.line(s) <> KS.lines.go(t)

# the info message's "kicad.ipc.servers": one line per server
def KS.lines(xs: List<&2, KSrv>) -> String:
  String.join(KS.lines.go(xs), "\n")

def KS.name.k(+n: String) -> String:
  Bool.pick(String, String.ends_with(n, ".kicad_pro"), String.take(n, Nat.sub(String.length(n), 10n)), n)

# a project's name: its .kicad_pro without the folder and extension
def KS.name(+p: String) -> String:
  KS.name.k(KS.last(p, '/'))

def KS.note.why(+w: String) -> String:
  Bool.pick(String, String.is_empty(w), "", ": " ++ w)

def KS.note.line(+p: String, +st: String, +w: String) -> String:
  Bool.pick(String, String.is_empty(p), "", "KiCad server: " ++ st ++ " for " ++ KS.name(p) ++ KS.note.why(w))

def KS.note.parts(+ps: List<&2, String>) -> String:
  KS.note.line(KS.nth(ps, 0n), KS.nth(ps, 1n), KS.nth(ps, 2n))

def KS.note.go(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}: Nil{}
    case +l <> t: KS.note.parts(String.split(l, '|')) <> KS.note.go(t)

# what Settings says under the KiCad IPC row ("" when no server has run)
def KS.note(+text: String) -> String:
  String.take(String.join(KS.note.go(String.lines(text)), " · "), 200n)

def KS.failed.pick(hit: Bool, +l: String, rest: String) -> String:
  match hit:
    case True{}: l
    case False{}: rest

def KS.failed.go(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}: ""
    case +l <> t: KS.failed.pick(String.eq(KS.nth(String.split(l, '|'), 1n), "failed"), l, KS.failed.go(t))

# the first failed server's line ("" when none failed)
def KS.failed.line(+text: String) -> String:
  KS.failed.go(String.lines(text))

def KS.failed.project(+text: String) -> String:
  KS.nth(String.split(KS.failed.line(text), '|'), 0n)

def KS.failed.why(+text: String) -> String:
  KS.nth(String.split(KS.failed.line(text), '|'), 2n)
