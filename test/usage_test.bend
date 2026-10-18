import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/usage.bend as UZ

# Settings > Usage from a replayed log and Boat fixtures: readings are
# listed fullest first, never added up; a Codex thread says ctx n/a;
# archived threads are left out; a reading's missing part (0) keeps the
# last one. Boat: the account from `boat limits --json` (the real shape,
# trimmed), Backplane's own jobs from the job status; a failed read keeps
# the last answer and says so; what Boat did not give shows "?".

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "demo", "/tmp/demo", 1n},
   M.ThreadCreated{"t1", "p1", "Small", M.Local{}, "", "", "claude", "claude-opus-5-5", 1n},
   M.ThreadCreated{"t2", "p1", "Big", M.Local{}, "", "", "claude", "claude-sonnet-5-5", 1n},
   M.ThreadCreated{"t3", "p1", "Codex", M.Local{}, "", "", "codex", "gpt-6.1-sol", 1n},
   M.ThreadCreated{"t4", "p1", "Old", M.Local{}, "", "", "claude", "", 1n},
   M.UsageSet{"t1", 0n, 1000000n, 2n},
   M.UsageSet{"t1", 50000n, 0n, 3n},
   M.UsageSet{"t2", 150000n, 200000n, 4n},
   M.UsageSet{"t4", 9000n, 0n, 5n},
   M.ThreadArchived{"t4", True{}, 6n}]

def limits() -> String:
  "{\"activeSandboxPlanKey\":\"box_20\",\"sandboxPlanDollars\":20,\"billingStatus\":\"active\",\"subscriptionStatus\":\"active\"," ++
  "\"subscriptionCurrentPeriodEnd\":\"2026-11-15T21:05:40.000Z\",\"subscriptionCancelAtPeriodEnd\":false,\"activeSandboxes\":3,\"maxActiveSandboxes\":100," ++
  "\"canStart\":true,\"startBlockedReason\":null,\"subscriptionQuotaSeconds\":2000000,\"subscriptionRemainingSeconds\":1999687,\"last24hUsageSeconds\":1155," ++
  "\"starts\":{\"minute\":{\"used\":1,\"remaining\":11,\"limit\":12},\"hour\":{\"used\":7,\"remaining\":53,\"limit\":60},\"day\":{\"used\":9,\"remaining\":191,\"limit\":200}}," ++
  "\"machineTypes\":[{\"type\":\"small\",\"available\":true},{\"type\":\"default\",\"available\":true},{\"type\":\"large\",\"available\":true},{\"type\":\"xlarge\",\"available\":false}]}"

def status() -> String:
  "{\"running\":1,\"orphans\":0,\"today\":{\"day\":\"2026-10-15\",\"cost_cs\":524,\"reserved_open_cs\":1500},\"month\":{\"month\":\"2026-10\",\"cost_cs\":746}," ++
  "\"jobs\":[{\"job\":\"j1\",\"state\":\"ended\",\"size\":\"default\",\"thread\":\"t1\",\"cost_cs\":158}," ++
  "{\"job\":\"j2\",\"state\":\"running\",\"size\":\"default\",\"thread\":\"t2\",\"machine\":\"desktop\",\"started_at\":1000,\"runtime_s\":42,\"reserved_cs\":1500}]}"

def show(rs: List<&2, UZ.URow>) -> String:
  match rs:
    case Nil{}:
      ""
    case +r <> t:
      UZ.URow.label(r) ++ " | " ++ UZ.URow.note(r) ++ Bool.pick(String, String.is_empty(UZ.URow.act(r)), "", " [" ++ UZ.URow.act(r) ++ "]") ++ "\n" ++ show(t)

def boat(+st: M.State, +cache: String, +stat: String, +now: Nat) -> String:
  show(UZ.Bj.rows(st, UZ.Bj.info(cache, stat), now))

def many.go(n: Nat) -> List<&2, String>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      "{\"state\":\"running\",\"size\":\"small\",\"started_at\":5,\"reserved_cs\":1}" <> many.go(k)

def many(+n: Nat) -> String:
  "{\"running\":" ++ Nat.show(n) ++ ",\"orphans\":0,\"today\":{\"cost_cs\":0,\"reserved_open_cs\":0},\"jobs\":[" ++ String.join(many.go(n), ",") ++ "]}"

def count.go(ls: List<&2, String>, +p: String) -> Nat:
  match ls:
    case Nil{}:
      0n
    case l <> t:
      Nat.add(Bool.pick(Nat, String.starts_with(l, p), 1n, 0n), count.go(t, p))

def count(+s: String, +p: String) -> Nat:
  count.go(String.split(s, '\n'), p)

def main() -> IO(Unit):
  do IO<Unit>:
    +st : M.State = M.State.replay(log(), M.State.empty())
    C.Check.str("rows", show(UZ.Use.rows(st, U.Ui.threads(st))),
      "Context now | 2 of 3 open threads have a reading. Only Claude reports one; others show ctx n/a.\n" ++
      "Big | claude-sonnet-5-5 · 150k of 200k held (75%)\n" ++
      "Small | claude-opus-5-5 · 50k of 1M held (5%)\n" ++
      "Tokens and cost | Not recorded yet. Readings above are held now, never summed.\n")
    C.Check.str("chip claude", U.Usage.tail(st, "t1"), " · ctx 5% of 1M")
    C.Check.str("chip codex", U.Usage.tail(st, "t3"), " · ctx n/a")
    C.Check.str("empty", show(UZ.Use.rows(M.State.empty(), Nil{})),
      "Context now | 0 of 0 open threads have a reading. Only Claude reports one; others show ctx n/a.\n" ++
      "Tokens and cost | Not recorded yet. Readings above are held now, never summed.\n")
    +c1 : String = UZ.Bj.cache.next("", limits(), 1000n)
    C.Check.str("boat fresh", boat(st, c1, status(), 1120n),
      "Boat plan | box_20 · $20/month · active · renews 2026-11-15 · read 2 min ago [usage-refresh]\n" ++
      "Boat sandboxes | 3 of 100 active on the account · Backplane's: 1 running\n" ++
      "Boat starts | Rolling: 1/12 this minute (11 left) · 7/60 this hour (53 left) · 9/200 today (191 left)\n" ++
      "Boat compute | 555.4 h of 555.5 h left this period · 19 min used in the last 24 h\n" ++
      "Boat sizes | small, default, large · not offered: xlarge\n" ++
      "Boat for Backplane | Today $0.00524 · this month $0.00746 · $0.01500 held for open jobs (worst case, not spent)\n" ++
      "Boat job running · default | Big on desktop · 120 s · reserved $0.01500 (worst case)\n")
    +c2 : String = UZ.Bj.cache.next(c1, "boat: command not found", 5000n)
    C.Check.str("boat failed keeps last", boat(st, c2, "", 5000n),
      "Boat plan | box_20 · $20/month · active · renews 2026-11-15 · Boat's CLI did not answer; last read 1 h ago [usage-refresh]\n" ++
      "Boat sandboxes | 3 of 100 active on the account · Backplane's: ?\n" ++
      "Boat starts | Rolling: 1/12 this minute (11 left) · 7/60 this hour (53 left) · 9/200 today (191 left)\n" ++
      "Boat compute | 555.4 h of 555.5 h left this period · 19 min used in the last 24 h\n" ++
      "Boat sizes | small, default, large · not offered: xlarge\n" ++
      "Boat for Backplane | No job status yet.\n")
    C.Check.str("boat never read", boat(st, "", "", 5000n),
      "Boat plan | Not read yet [usage-refresh]\nBoat for Backplane | No job status yet.\n")
    C.Check.str("boat never answered", boat(st, UZ.Bj.cache.next("", "", 7n), "", 9n),
      "Boat plan | Not read yet · Boat's CLI did not answer [usage-refresh]\nBoat for Backplane | No job status yet.\n")
    C.Check.str("boat unknown is ?", boat(st, UZ.Bj.cache.next("", "{\"maxActiveSandboxes\":100}", 1n), "", 1n),
      "Boat plan | ? · $?/month · ? · read just now [usage-refresh]\n" ++
      "Boat sandboxes | ? of 100 active on the account · Backplane's: ?\n" ++
      "Boat starts | Rolling: ?/? this minute (? left) · ?/? this hour (? left) · ?/? today (? left)\n" ++
      "Boat compute | ? of ? left this period · ? used in the last 24 h\n" ++
      "Boat sizes | ?\n" ++
      "Boat for Backplane | No job status yet.\n")
    C.Check.str("due: refresh waits 10 s", Bool.show(UZ.Bj.due(c1, True{}, 1005n)), "False")
    C.Check.str("due: refresh after 10 s", Bool.show(UZ.Bj.due(c1, True{}, 1010n)), "True")
    C.Check.str("due: open waits 5 min", Bool.show(UZ.Bj.due(c1, False{}, 1299n)), "False")
    C.Check.str("due: open after 5 min", Bool.show(UZ.Bj.due(c1, False{}, 1300n)), "True")
    C.Check.str("due: no cache", Bool.show(UZ.Bj.due("", False{}, 5n)), "True")
    C.Check.str("usd", UZ.Bj.usd(158n), "$0.00158")
    +m : String = boat(st, "", many(8n), 9n)
    C.Check.str("boat jobs capped", Bool.show(String.contains(m, "More Boat jobs | 2 more not listed")), "True")
    C.Check.str("boat jobs listed", Nat.show(count(m, "Boat job running")), "6")
    C.Check.str("boat runtime from start", Bool.show(String.contains(m, "started by hand · 4 s")), "True")
    +w : String = boat(st, "", "{\"running\":1,\"orphans\":1,\"ambiguous\":1,\"today\":{\"cost_cs\":0,\"reserved_open_cs\":0},\"jobs\":[" ++
      "{\"state\":\"ended\",\"size\":\"small\",\"cost_cs\":1}," ++
      "{\"state\":\"running\",\"size\":\"small\",\"started_at\":5,\"reserved_cs\":1}," ++
      "{\"state\":\"ambiguous\",\"size\":\"default\",\"thread\":\"t1\",\"why\":\"create answered unclearly\"}," ++
      "{\"state\":\"orphan\",\"size\":\"default\",\"started_at\":1,\"reserved_cs\":2}]}", 9n)
    C.Check.str("boat warnings first, never capped", w,
      "Boat plan | Not read yet [usage-refresh]\n" ++
      "Boat for Backplane | Today $0.00000 · $0.00000 held for open jobs (worst case, not spent)\n" ++
      "Boat warning | 1 Boat create unclear: check boat list\n" ++
      "Boat create unclear · default | Small · create answered unclearly · check boat list\n" ++
      "Boat sandbox NOT deleted · default | started by hand · ? s · reserved $0.00002 (worst case)\n" ++
      "Boat job running · small | started by hand · 4 s · reserved $0.00001 (worst case)\n")
    +w8 : String = boat(st, "", "{\"running\":8,\"orphans\":0,\"ambiguous\":1,\"today\":{\"cost_cs\":0,\"reserved_open_cs\":0},\"jobs\":[" ++
      String.join(many.go(8n), ",") ++ ",{\"state\":\"ambiguous\",\"size\":\"small\",\"why\":\"timeout\"}]}", 9n)
    C.Check.str("boat unclear shown past the cap", Nat.show(count(w8, "Boat create unclear")), "1")
    +wb : String = boat(st, "", "{\"running\":0,\"orphans\":0,\"ambiguous\":2,\"unresolved\":1,\"today\":{\"cost_cs\":0,\"reserved_open_cs\":0},\"jobs\":[" ++
      "{\"state\":\"unresolved\",\"size\":\"small\",\"why\":\"look-alike up\"},{\"state\":\"ambiguous\",\"size\":\"small\",\"why\":\"slot held\"}," ++
      "{\"state\":\"reconciled\",\"size\":\"small\",\"cost_cs\":3}]}", 9n)
    C.Check.str("boat creates blocked, loudly", wb,
      "Boat plan | Not read yet [usage-refresh]\n" ++
      "Boat for Backplane | Today $0.00000 · $0.00000 held for open jobs (worst case, not spent)\n" ++
      "Boat creates blocked | Unclear create unresolved, check boat list\n" ++
      "Boat warning | 1 Boat create unclear: check boat list\n" ++
      "Boat create UNRESOLVED, creates blocked · small | started by hand · look-alike up · check boat list\n" ++
      "Boat create unclear · small | started by hand · slot held · check boat list\n")
    C.Check.str("boat blocked only: no second line for the same job", boat(st, "", "{\"running\":0,\"ambiguous\":1,\"unresolved\":1,\"today\":{},\"jobs\":[]}", 9n),
      "Boat plan | Not read yet [usage-refresh]\nBoat for Backplane | Today ? · ? held for open jobs (worst case, not spent)\nBoat creates blocked | Unclear create unresolved, check boat list\n")
    C.Check.str("boat daily cap and what is left", boat(st, "", "{\"running\":0,\"budget\":{\"usd\":10,\"cs\":1000000,\"source\":\"file\"},\"today\":{\"cost_cs\":45400,\"reserved_open_cs\":3300},\"jobs\":[]}", 9n),
      "Boat plan | Not read yet [usage-refresh]\nBoat for Backplane | Today $0.45400 · daily cap $10.00000 (file) · $9.51300 left · $0.03300 held for open jobs (worst case, not spent)\n")
    C.Check.str("boat cap spent: none left, never below 0", boat(st, "", "{\"running\":0,\"budget\":{\"cs\":50000,\"source\":\"default\"},\"today\":{\"cost_cs\":48742,\"reserved_open_cs\":3000},\"jobs\":[]}", 9n),
      "Boat plan | Not read yet [usage-refresh]\nBoat for Backplane | Today $0.48742 · daily cap $0.50000 (default) · $0.00000 left · $0.03000 held for open jobs (worst case, not spent)\n")
