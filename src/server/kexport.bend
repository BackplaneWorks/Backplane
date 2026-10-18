import Base
import ./kicad.bend as K
import ./proc.bend as P
import ../core/kexport.bend as X
import ../core/json.bend as J
import ../core/kihealth.bend as KH
import ../core/links.bend as LK
import ../core/codec.bend as C

# Deadlines return to the caller without signalling the child. A timed-out
# command keeps its .busy directory until it exits, so no other request
# can clear its output while it still writes it.
def KExport.run(work: IO(P.Ran)) -> IO(P.Ran):
  IO.bind(Maybe<&1, P.Ran>, P.Ran, IO.within(P.Ran, 600000, work), r => IO.pure(P.Ran, Maybe.default(&1, P.Ran, r, P.Ran{124, "KiCad export timed out after ten minutes. Its export folder stays locked until the command exits."})))

# All operands are arguments, never shell source. Only the fixed kicad10
# child can be removed. Refuse symlinked exports/base/marker/report/output,
# and physical project ancestry (including a project containing the home).
# Existing unmarked folders are NEVER adopted, even if empty.
def KExport.guard() -> String:
  "set -eu; umask 077; home=$1; base=$2; pro=$3; mode=$4; cli=$5; shift 5; [ \"$mode\" != refuse ] || { echo \"Export marker missing or unmanaged folder refused\"; exit 1; }; " ++
  "home=$(cd \"$home\" && pwd -P); project=$(cd \"$(dirname \"$pro\")\" && pwd -P); " ++
  "[ ! -L \"$home/exports\" ] || { echo 'Symlinked exports folder refused'; exit 1; }; mkdir -p \"$home/exports\"; " ++
  "case \"$base\" in \"$home/exports/\"*) leaf=${base#\"$home/exports/\"};; *) echo 'Export outside managed root refused'; exit 1;; esac; " ++
  "case \"$leaf\" in ''|*[!a-zA-Z0-9_-]*) echo 'Unsafe export name'; exit 1;; esac; " ++
  "case \"$base/\" in \"$project/\"*) echo 'Export must be outside the project'; exit 1;; esac; " ++
  "[ ! -L \"$base\" ] || { echo 'Symlinked export base refused'; exit 1; }; " ++
  "if mkdir \"$base\" 2>/dev/null; then printf '%s\\n' backplane-export >\"$base/.backplane-export\"; fi; " ++
  "[ -f \"$base/.backplane-export\" ] && [ ! -L \"$base/.backplane-export\" ] || { echo 'Unmarked export folder refused'; exit 1; }; " ++
  "[ ! -L \"$base/report.json\" ] && [ ! -L \"$base/kicad10\" ] || { echo 'Symlinked export content refused'; exit 1; }; " ++
  "mkdir \"$base/.busy\" 2>/dev/null || { echo 'Export already running in this folder'; exit 1; }; trap 'rmdir \"$base/.busy\"' EXIT; " ++
  "if [ \"$mode\" = export ]; then rm -rf -- \"$base/kicad10\"; else rm -f -- \"$base/report.json\"; fi; " ++
  "\"$cli\" project downgrade --target 10.0 --report-json \"$base/report.json\" -o \"$base/kicad10\" \"$pro\" \"$@\""

def KExport.clear_mode(+home: String, +pro: String, +mode: String, marker: Bool) -> String:
  match mode:
    case "export": Bool.pick(String, X.KX.may_clear_in(home, X.KX.base(X.KX.paths(home, pro)), marker), "export", "refuse")
    case _: mode

def KExport.command(+home: String, +pro: String, +cli: String, +mode: String, flags: List<&2, String>) -> IO(P.Ran):
  do IO<P.Ran>:
    marker : Bool <- K.Io.is_file(X.KX.base(X.KX.paths(home, pro)) ++ "/.backplane-export")
    KExport.run(P.Proc.run("sh", List.append(&2, String, ["-c", KExport.guard(), "sh", home, X.KX.base(X.KX.paths(home, pro)), pro, KExport.clear_mode(home, pro, mode, marker), cli], flags), "/"))

def KExport.result(+home: String, +pro: String, +report: J.Json, state: String, text: String) -> J.Json:
  J.Json.obj([J.KV{"export10", J.Str{state}}, J.KV{"path", J.Str{X.KX.output(X.KX.paths(home, pro))}}, J.KV{"project", J.Str{pro}}, J.KV{"report", report}, J.KV{"text", J.Str{text}}])

def KExport.finished(r: P.Ran, +home: String, +pro: String, report: J.Json) -> J.Json:
  match r:
    case P.Ran{0, _}: KExport.result(home, pro, report, "done", X.KX.success(X.KX.output(X.KX.paths(home, pro))))
    case P.Ran{_, out}: KExport.result(home, pro, report, "error", out)

def KExport.finish_report(r: P.Ran, +home: String, +pro: String, fallback: J.Json) -> IO(J.Json):
  do IO<J.Json>:
    text : String <- K.Io.text(X.KX.report_path(X.KX.paths(home, pro)))
    return KExport.finished(r, home, pro, Maybe.default(&2, J.Json, J.Json.parse(text), fallback))

def KExport.confirm(go: Bool, +home: String, +pro: String, cli: String, +report: J.Json, anyway: Bool) -> IO(J.Json):
  match go:
    case False{}: IO.pure(J.Json, KExport.result(home, pro, report, "confirm", "Review the changes before exporting."))
    case True{}:
      do IO<J.Json>:
        r : P.Ran <- KExport.command(home, pro, cli, "export", X.KX.flags(X.KX.report(report), anyway))
        KExport.finish_report(r, home, pro, report)

def KExport.valid(valid: Bool, +home: String, +pro: String, cli: String, +report: J.Json, +anyway: Bool, +expected: String, agent: Bool) -> IO(J.Json):
  match valid:
    case False{}: IO.pure(J.Json, KExport.result(home, pro, report, "error", "KiCad did not write a valid compatibility report."))
    case True{}: KExport.confirm(Bool.or(X.KX.empty(X.KX.report(report)), Bool.and(anyway, Bool.or(agent, String.eq(X.KX.signature(Maybe.default(&2, J.Json, J.Json.parse(expected), J.Null{})), X.KX.signature(report))))), home, pro, cli, report, anyway)

def KExport.scanned(r: P.Ran, +home: String, +pro: String, +cli: String, +report: J.Json, anyway: Bool, expected: String, agent: Bool) -> IO(J.Json):
  match r:
    case P.Ran{0, _}:
      KExport.valid(X.KX.valid(report), home, pro, cli, report, anyway, expected, agent)
    case P.Ran{_, out}: IO.pure(J.Json, KExport.result(home, pro, report, "error", out))

def KExport.project(ok: Bool, +home: String, +pro: String, +cli: String, anyway: Bool, expected: String, agent: Bool) -> IO(J.Json):
  match ok:
    case False{}: IO.pure(J.Json, KExport.result(home, pro, J.Null{}, "error", "Export needs an absolute .kicad_pro path outside the managed export folder."))
    case True{}:
      do IO<J.Json>:
        r : P.Ran <- KExport.command(home, pro, cli, "dry", ["--dry-run"])
        text : String <- K.Io.text(X.KX.report_path(X.KX.paths(home, pro)))
        KExport.scanned(r, home, pro, cli, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), anyway, expected, agent)

def KExport.ready(ok: Bool, +rt: K.Rt, +home: String, +pro: String, anyway: Bool, expected: String, agent: Bool) -> IO(J.Json):
  match ok:
    case False{}: IO.pure(J.Json, J.Json.obj([J.KV{"kicad", J.Str{K.Rt.status(rt)}}, J.KV{"export10", J.Str{"unavailable"}}, J.KV{"text", J.Str{"KiCad 10 export needs a healthy Backplane KiCad runtime with export-v10 capability."}}]))
    case True{}: KExport.project(Bool.and(Bool.and(String.starts_with(pro, "/"), String.ends_with(pro, ".kicad_pro")), Bool.not(String.starts_with(pro, home ++ "/exports/"))), home, pro, K.Rt.cli(rt), anyway, expected, agent)

def KExport.export(+rt: K.Rt, home: String, pro: String, anyway: Bool, expected: String, agent: Bool) -> IO(J.Json):
  KExport.ready(Bool.and(String.eq(K.Rt.status(rt), "ok"), Nat.is_lt(C.Codec.pos(KH.KH.strings(J.Json.elems(J.Json.get_obj(K.Rt.health(rt), "caps"))), "export-v10"), List.length(&2, String, KH.KH.strings(J.Json.elems(J.Json.get_obj(K.Rt.health(rt), "caps")))))), rt, home, pro, anyway, expected, agent)

# Probe stock KiCad separately from the configured Backplane runtime.
# Launch detached through a new session, as KiCad.open does; macOS uses
# the system's open -a. A missing stock 10 falls back to file.open's script.
def KExport.open_script() -> String:
  "set -eu; out=$1; pro=$2; gui=$(command -v kicad || true); " ++
  "for g in \"$gui\" /usr/bin/kicad /Applications/KiCad/KiCad.app/Contents/MacOS/kicad; do " ++
  "[ -n \"$g\" ] && [ -x \"$g\" ] || continue; c=$(dirname \"$g\")/kicad-cli; [ -x \"$c\" ] || continue; " ++
  "v=$(\"$c\" version 2>/dev/null) || continue; case \"$v\" in 10.0.[0-9]*) ;; *) continue;; esac; " ++
  "case \"$v\" in *bp.*) continue;; esac; " ++
  "if [ \"$(uname)\" = Darwin ]; then open -a /Applications/KiCad/KiCad.app \"$pro\"; " ++
  "else setsid -f \"$g\" \"$pro\" </dev/null >/dev/null 2>&1; fi; exit 0; done; " ++
  "set -- \"$out\"; " ++ LK.Link.file.cmd("open")

def KXFile.name(pro: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(pro, '/')), "project.kicad_pro")

def KExport.open(done: Bool, +result: J.Json, +pro: String) -> IO(J.Json):
  match done:
    case False{}: IO.pure(J.Json, result)
    case True{}:
      do IO<J.Json>:
        r : P.Ran <- KExport.run(P.Proc.run("sh", ["-c", KExport.open_script(), "sh", J.Json.get_str(result, "path", ""), J.Json.get_str(result, "path", "") ++ KXFile.name(pro)], "/"))
        return KH.KH.put.obj(result, "openError", J.Str{K.KiCad.open.said(r)})
