import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "fixture", "/fixture", 1n}, M.ThreadCreated{"parent", "p", "parent", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

def call(wt: String) -> MC.Res:
  MC.Mcp.delegate.go(base(), 5n, "parent", "p", M.Local{}, "", wt, "claude", "", M.Supervised{}, M.DefaultMode{}, "read only", "child")

def prefix.outs(os: List<&2, H.Out>, +child: String) -> String:
  match os:
    case H.Git{0, "", "checkpoint0", +a, +cwd, "", ""} <> H.Git{0, "", "step", +b, +snapcwd, "", "start"} <> H.AgentStart{+c, +run, _, _, _, _, _} <> _:
      Bool.show(Bool.and(String.eq(a, child), Bool.and(String.eq(b, child), Bool.and(String.eq(c, child), Bool.and(String.eq(cwd, snapcwd), String.eq(snapcwd, run)))))) ++ ":" ++ cwd
    case _:
      "missing baseline before agent"

def prefix(r: MC.Res) -> String:
  match r:
    case MC.Res{_, +body, H.Step{_, os}}:
      prefix.outs(os, J.Json.get_str(body, "childThreadId", ""))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("delegation snapshots its initial project design before running", prefix(call("")), "True:/fixture")
    C.Check.str("delegation snapshots its selected worktree before running", prefix(call("/fixture-wt")), "True:/fixture-wt")
