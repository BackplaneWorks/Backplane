import Base
import ./hub.bend as B
import ./copynet.bend as CN

# A hub on a copy of a real log (BACKPLANE_COPY=1): for benches and boot
# tests on the person's own data. It acts on nothing outside its home: no
# watched work is launched, re-launched or signalled (every watch reads as
# gone), no deploy, update or restart, no pushes, no far pushes or launch
# reports to the owner's other machines, no git that writes, no webhook or
# bot replies, no network commands at all; agents only as stand-in blocks.
# A copied log names real work, real folders and real machines: a hub on
# one once re-ran a work_run that polled the live hub, and another waited
# to move a folder of the live coordination state.

def Copy.on(+v: String) -> Bool:
  String.eq(v, "1")

# git jobs that only read (a client's diff, status, file lists, refs...)
def Copy.git.reads(+job: String) -> Bool:
  CN.Copy.has(["diff", "status", "name", "files", "kicad", "refs", "dir", "mech", "pdf"], job)

# bot effects inside the home (scratch folders, the zone rule)
def Copy.fx.home(+kind: String) -> Bool:
  CN.Copy.has(["scratch", "scratch-gone", "zone", "home"], kind)

# a client that asked is told (it would wait for an answer that never comes)
def Copy.git.no(server: Bool, +client: U32, +id: String) -> List<&2, B.Out>:
  match server:
    case True{}:
      Nil{}
    case False{}:
      [B.Reply.err(client, id, "a hub on a copied log (BACKPLANE_COPY=1) runs no git that writes")]

def Copy.git.k(reads: Bool, o: B.Out, +client: U32, +id: String) -> List<&2, B.Out>:
  match reads:
    case True{}:
      [o]
    case False{}:
      Copy.git.no(U32.is_eq(client, 0), client, id)

def Copy.fx.k(home: Bool, o: B.Out) -> List<&2, B.Out>:
  match home:
    case True{}:
      [o]
    case False{}:
      Nil{}

# what a copy hub does of one out: nothing for what acts outside
def Copy.out(o: B.Out) -> List<&2, B.Out>:
  match o:
    case B.RunStart{_, _, _}: Nil{}
    case B.CharterWrite{_, _}: Nil{}
    case B.LockGuard{_, _, _}: Nil{}
    case B.Upgrade{}: Nil{}
    case B.UpgradeForce{}: Nil{}
    case B.Restart{}: Nil{}
    case B.KiCadInstall{}: Nil{}
    case B.KiCadRetry{}: Nil{}
    case B.TurnEnded{_, _}: Nil{}
    case B.IslandSync{}: Nil{}
    case B.TermRun{_, _, _, _, _, _, _}: Nil{}
    case B.BotReply{_, _, _, _}: Nil{}
    case B.WatchStop{_, _, _, _, _}: Nil{}
    case B.LaunchFar{}: Nil{}
    case B.Alerted{_}: Nil{}
    case B.WakePoll{_, _}: Nil{}
    case B.DeployProbe{}: Nil{}
    case B.DeployGo{_}: Nil{}
    case B.ShipRun{_}: Nil{}
    case B.ShipNext{_}: Nil{}
    case B.Teach{_, _, _, _}: Nil{}
    case B.Git{+client, +id, +job, th, cwd, a, b}: Copy.git.k(Copy.git.reads(job), B.Git{client, id, job, th, cwd, a, b}, client, id)
    case B.BotFx{+kind, a, b, c}: Copy.fx.k(Copy.fx.home(kind), B.BotFx{kind, a, b, c})
    case _: [o]

def Copy.drop(os: List<&2, B.Out>) -> List<&2, B.Out>:
  match os:
    case Nil{}:
      Nil{}
    case o <> t:
      List.append(&2, B.Out, Copy.out(o), Copy.drop(t))

# the outs a hub runs: all of them, or a copy's
def Copy.outs(copy: Bool, os: List<&2, B.Out>) -> List<&2, B.Out>:
  match copy:
    case False{}:
      os
    case True{}:
      Copy.drop(os)

def Copy.note() -> String:
  "BACKPLANE_COPY=1: a copy of a log; no watched work, deploys, pushes, far pushes, network commands or git that writes; agents are stand-in blocks"
