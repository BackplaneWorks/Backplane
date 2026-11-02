import Base
import ../../src/server/git.bend as GT
import ../../src/core/pcb-usage.bend as PC
import ../../src/core/model.bend as M
import ../../src/core/telemetry.bend as T
import ../../src/core/telemetry-usage.bend as U
import ../../src/server/telemetry.bend as TI
import ../../src/server/argv.bend as A

# Only synthetic data, the actual committed-event projection, and the actual
# signed worker. No provider, real hub home, event log or production endpoint.
def Probe.work(+n: Nat, +at: Nat, s: M.State) -> M.State:
  match n:
    case 0n: s
    case 1n+k:
      +id = "synthetic-" ++ Nat.show(n)
      Probe.work(k, at, M.State.replay([
        M.ThreadCreated{id, "private-project", "private-title", M.Local{}, "", "/private/path", "private-provider", "private-model", at},
        M.MessagePosted{id, id ++ "u", M.User{}, "private-prompt", at}, M.MessagePosted{id, id ++ "a", M.Assistant{}, "private-reply", at},
        M.MailPosted{id, "private-conversation", "private-sender", "private-recipient", "note", "private-ref", "fyi", "private-subject", 999n, at},
        M.SettingSet{PC.Pcb.key("step:" ++ id), PC.Pcb.record(at, 1n, 0n), at},
        M.SettingSet{PC.Pcb.key("order:" ++ id), PC.Pcb.record(at, 0n, 1n), at},
        M.BotWoke{"private-bot", 0n, at}, M.TurnUsage{id, "private-provider", "private-model", id, 100n, 20n, 30n, 40n, 10n, 999n, "irwotc", "private-basis", at}], s))

def Probe.state(+at: Nat, n: Nat, share: String, consent: String, reset: String) -> M.State:
  Probe.work(n, at, M.State.replay([M.SettingSet{"telemetry.usage.enabled", "1", at}, M.SettingSet{"telemetry.asked", "1", at},
    M.SettingSet{"telemetry.usage.asked", consent, at}, M.SettingSet{"telemetry.share", share, at}, M.SettingSet{"telemetry.reset", reset, at}], M.State.empty()))

def Probe.start(on: Bool, +cfg: List<&2, TI.TmCfg>, +home: String, +st: M.State, +now: Nat, +permit: Bool) -> IO(Unit):
  match on:
    case False{}: IO.pure(Unit, Unit{})
    case True{}:
      do IO<Unit>:
        id : String <- TI.Tio.job.usage(cfg, home, TI.Tio.view(st, "0.12.0", now, cfg), U.Usage.rows(now, st), now,
          u => IO.pure(Bool, Bool.and(permit, T.Tm.allowed(st))), u => IO.pure(Bool, Bool.and(permit, M.Metrics.allowed(st))))
        return Unit{}

def Probe.line(+out: String, at: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, String.split(out, '\n'), at)), "")

def Probe.captured(+cwd: String, +out: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(out), IO.print("0"),
    IO.bind(Nat, Unit, GT.Git.step.created(cwd, Probe.line(out, 0n), Probe.line(out, 1n), Probe.line(out, 2n)), n => IO.print(Nat.show(n))))

def Probe.run(xs: List<String>) -> IO(Unit):
  match xs:
    case mode <> +cwd <> reg <> Nil{}:
      Bool.pick(IO(Unit), String.eq(mode, "--pcb-capture"),
        IO.bind(String, Unit, GT.Git.step.capture(cwd, "synthetic-pcb-thread", reg), out => Probe.captured(cwd, out)), IO.die(Unit, 2, "expected test-only pcb capture mode"))
    case mode <> +cwd <> commit <> prev <> gitdir <> Nil{}:
      Bool.pick(IO(Unit), String.eq(mode, "--pcb-created"),
        IO.bind(Nat, Unit, GT.Git.step.created(cwd, commit, prev, gitdir), n => IO.print(Nat.show(n))), IO.die(Unit, 2, "expected test-only pcb mode"))
    case +home <> +now <> count <> share <> consent <> reset <> permit <> Nil{}:
      do IO<Unit>:
        +cfg : List<&2, TI.TmCfg> <- TI.Tio.config(False{})
        Probe.start(TI.Tio.can(cfg, T.Tm.num(now)), cfg, home, Probe.state(T.Tm.num(now), T.Tm.num(count), share, consent, reset), T.Tm.num(now), String.eq(permit, "yes"))
    case _: IO.die(Unit, 2, "expected temporary home, time, count, share, consent, reset, permit")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, A.Argv.args(), xs => TI.Tio.main(xs, Probe.run))
