import Base
import ./i18n.bend as I
import ./msgs.bend as MSG

# The first-run tour, for every client (the window, the web, the phones):
# a short list of steps, each a title, a few sentences and the part of the
# screen it is about, and the moves between them. Pure: the text depends on
# the client's kind (`k`: 0 window, 1 web, 2 phone, where the controls
# differ) and on the hub's Scratch lifetime in days.
#
# The client keeps which step is on screen as text (client.bend's Tour.*):
# "" (the tour came up by itself, at the first step), digits (it came up by
# itself and the person moved on), or "m" and digits (the person asked for
# it again, from Settings). The hub keeps the one fact that outlives it,
# the setting "onboard.done". A tour that came up by itself closes the
# moment the hub says it is done, wherever that was decided; one asked for
# by hand stays until it is ended.
#
# `target` names what a client may point at: "" (nothing), "sidebar",
# "add" (the sidebar's + button), "head" (an open thread's buttons),
# "settings" (the sidebar's Settings button). Phones point at nothing.

def Ob.count() -> Nat:
  11n

def Ob.title(+l: I.Loc, i: Nat) -> String:
  match i:
    case 0n:
      MSG.T.tour.title.welcome(l)
    case 1n:
      MSG.T.tour.title.projects(l)
    case 2n:
      MSG.T.tour.title.threads(l)
    case 3n:
      MSG.T.tour.title.buttons(l)
    case 4n:
      MSG.T.tour.title.bots(l)
    case 5n:
      MSG.T.tour.title.agents(l)
    case 6n:
      MSG.T.tour.title.keys(l)
    case 7n:
      MSG.T.tour.title.machines(l)
    case 8n:
      MSG.T.tour.title.phones(l)
    case 9n:
      "Anonymous usage"
    case 10n:
      MSG.T.tour.title.uber(l)
    case _:
      ""

# what a desktop client may point at; phones point at nothing
def Ob.target.d(i: Nat) -> String:
  match i:
    case 0n:
      ""
    case 1n:
      "add"
    case 2n:
      "sidebar"
    case 3n:
      "head"
    case 4n:
      "sidebar"
    case 5n:
      "settings"
    case 6n:
      "settings"
    case 7n:
      "settings"
    case 8n:
      "settings"
    case 9n:
      "settings"
    case 10n:
      "sidebar"
    case _:
      ""

def Ob.target(+k: Nat, +i: Nat) -> String:
  match k:
    case 2n:
      ""
    case _:
      Ob.target.d(i)

def Ob.known(+g: String) -> Bool:
  Bool.or(Bool.or(String.is_empty(g), String.eq(g, "sidebar")), Bool.or(Bool.or(String.eq(g, "add"), String.eq(g, "head")), String.eq(g, "settings")))

def Ob.phone(+k: Nat) -> Bool:
  Nat.is_eq(k, 2n)

# the words of a step. Every claim here is checked against the code (see
# docs in AGENTS.md): Scratch's lifetime is the "scratch.days" setting;
# threads in a git project get their own worktree unless asked for the
# folder ("local", Hub.env.of); Settings shows whether each agent program is
# found, not whether it is signed in; the API keys' file is the hub user's,
# and agents are only told not to print it; Tailscale is looked for when
# Backplane starts and Settings says which of on, not connected or off, as the hub reports it
# (client.bend's Tail.*). The thread buttons are tools.bend's, named by
# their labels (test/onboard_test.bend ties them).
def Ob.body.s0.k(+l: I.Loc, +k: Nat) -> String:
  match k:
    case 2n:
      MSG.T.tour.body.welcome.phone(l)
    case 1n:
      MSG.T.tour.body.welcome.web(l)
    case _:
      MSG.T.tour.body.welcome.window(l)

def Ob.beta() -> String:
  "Backplane is in beta. If something breaks or looks wrong, please report it."

def Ob.body.s0(+l: I.Loc, k: Nat) -> String:
  Ob.body.s0.k(l, k) ++ " " ++ Ob.beta()

def Ob.body.share() -> String:
  "Backplane counts installs and how many people use it: a random install id, the version and OS, once a day. Nothing else: no projects, files, prompts, names or address. If you opt out, installs and updates are still counted, without the id. Change it any time in Settings."

def Ob.body.s1(+l: I.Loc, +k: Nat, +d: String) -> String:
  Bool.pick(String, Ob.phone(k), MSG.T.tour.body.projects.phone(l, d), MSG.T.tour.body.projects.desktop(l, d))

def Ob.body.s2(+l: I.Loc) -> String:
  MSG.T.tour.body.threads(l)

def Ob.body.s3(+l: I.Loc, +k: Nat) -> String:
  Bool.pick(String, Ob.phone(k), MSG.T.tour.body.buttons.phone(l), MSG.T.tour.body.buttons.desktop(l))

def Ob.body.s4(+l: I.Loc, +k: Nat) -> String:
  Bool.pick(String, Ob.phone(k), MSG.T.tour.body.bots.phone(l), MSG.T.tour.body.bots.desktop(l))

def Ob.body.s5(+l: I.Loc) -> String:
  MSG.T.tour.body.agents(l)

def Ob.body.s6(+l: I.Loc, +k: Nat) -> String:
  Bool.pick(String, Ob.phone(k), MSG.T.tour.body.keys.phone(l), MSG.T.tour.body.keys.desktop(l))

# How other machines are found and kept in sync (server.bend's Machines.*
# every two minutes: tailscale status, then GET /hello on each online
# machine of yours; client.bend's Mate.*: the hub with the lower address
# asks the other to link, which only your own devices may, and the two
# then mirror each other's projects and threads: far.bend). Settings' row
# is client.bend's Mate.title().
def Ob.body.s7(+l: I.Loc, +k: Nat) -> String:
  Bool.pick(String, Ob.phone(k), MSG.T.tour.body.machines.phone(l), MSG.T.tour.body.machines.desktop(l))

def Ob.body.s8(+l: I.Loc, +k: Nat) -> String:
  Bool.pick(String, Ob.phone(k), MSG.T.tour.body.phones.phone(l), MSG.T.tour.body.phones.desktop(l))

# The last step is the uberbot (core/uber.bend): the bot the person makes
# their main interface. Its words depend on the case client.bend's
# Tour.uber.case gives: "none" (no bots here), "pick" (bots, no uberbot;
# `name` is the one used most), "have" (an uberbot, `name`), or "join"
# (reserved: a linked machine has one, `name`; never produced yet).
def Ob.uber() -> Nat:
  10n

# The step before it asks about anonymous usage counts (core/telemetry.bend,
# docs/telemetry.md); client.bend's Share.* act on its choices.
def Ob.share() -> Nat:
  9n

def Ob.body.s9(+l: I.Loc, +k: Nat, +uc: String, +name: String) -> String:
  Bool.pick(String, String.eq(uc, "have"), Bool.pick(String, Ob.phone(k), MSG.T.tour.body.uber.have.phone(l, name), MSG.T.tour.body.uber.have.desktop(l, name)),
    Bool.pick(String, String.eq(uc, "join"), MSG.T.tour.body.uber.join(l, name),
      Bool.pick(String, String.eq(uc, "pick"), MSG.T.tour.body.uber.pick(l, name),
        MSG.T.tour.body.uber.none(l))))

def Ob.body(+l: I.Loc, +k: Nat, +d: String, i: Nat) -> String:
  match i:
    case 0n:
      Ob.body.s0(l, k)
    case 1n:
      Ob.body.s1(l, k, d)
    case 2n:
      Ob.body.s2(l)
    case 3n:
      Ob.body.s3(l, k)
    case 4n:
      Ob.body.s4(l, k)
    case 5n:
      Ob.body.s5(l)
    case 6n:
      Ob.body.s6(l, k)
    case 7n:
      Ob.body.s7(l, k)
    case 8n:
      Ob.body.s8(l, k)
    case 9n:
      Ob.body.share()
    case 10n:
      Ob.body.s9(l, k, "none", "")
    case _:
      ""

# the words of step i for a case of the uberbot step; other steps ignore it
def Ob.body.uber(+l: I.Loc, +k: Nat, +d: String, +uc: String, +name: String, +i: Nat) -> String:
  Bool.pick(String, Nat.is_eq(i, Ob.uber()), Ob.body.s9(l, k, uc, name), Ob.body(l, k, d, i))

# digits in a client's text (anything else, the "m", is skipped)
def Ob.num(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Ob.num(t, Bool.pick(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))), acc))

# asked for by hand: "m" first
def Ob.manual(+raw: String) -> Bool:
  String.starts_with(raw, "m")

# a step number kept in range; `count` stands for the tour being over
def Ob.clamp(+i: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(i, Ob.count()), i, Ob.count())

def Ob.at.auto(done: Bool, +raw: String) -> Nat:
  match done:
    case True{}:
      Ob.count()
    case False{}:
      Ob.clamp(Ob.num(raw, 0n))

def Ob.at.k(fresh: Bool, done: Bool, +raw: String) -> Nat:
  match fresh:
    case True{}:
      Bool.pick(Nat, done, Ob.count(), 0n)
    case False{}:
      Ob.at.auto(done, raw)

def Ob.at.up(ready: Bool, done: Bool, +raw: String) -> Nat:
  match ready:
    case False{}:
      Ob.count()
    case True{}:
      Ob.at.k(String.is_empty(raw), done, raw)

# The step on screen, `count` when none. One asked for by hand shows
# whatever the hub says (it works offline). One that comes up by itself
# waits until the client holds this connection's state (`ready`), starts at
# the first step and ends the moment the hub says it is done.
def Ob.at.m(manual: Bool, ready: Bool, done: Bool, +raw: String) -> Nat:
  match manual:
    case True{}:
      Ob.clamp(Ob.num(raw, 0n))
    case False{}:
      Ob.at.up(ready, done, raw)

def Ob.at(ready: Bool, done: Bool, +raw: String) -> Nat:
  Ob.at.m(Ob.manual(raw), ready, done, raw)

# is a step on screen
def Ob.live(i: Nat) -> Bool:
  Nat.is_lt(i, Ob.count())

def Ob.next(+i: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(i, Ob.count()), 1n+i, Ob.count())

def Ob.back(+i: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(i, Ob.count()), Nat.sub(i, 1n), Nat.sub(Ob.count(), 1n))

# Skip ends the tour from any step
def Ob.skip() -> Nat:
  Ob.count()

# the client's text for a step
def Ob.raw(manual: Bool, i: Nat) -> String:
  Bool.pick(String, manual, "m", "") ++ Nat.show(i)

# the last step's forward button reads Done
def Ob.last(i: Nat) -> Bool:
  Nat.is_eq(1n+i, Ob.count())

def Ob.forward(+l: I.Loc, i: Nat) -> String:
  Bool.pick(String, Ob.last(i), MSG.T.tour.done(l), MSG.T.tour.next(l))

# "3 of 9"
def Ob.counter(+l: I.Loc, i: Nat) -> String:
  MSG.T.tour.counter(l, Nat.show(1n+i), Nat.show(Ob.count()))

# The keys, one policy for every client: a browser's key name, or an X
# keysym (the window), to the tour's action ("" for any other key).
def Ob.dom(+key: String) -> String:
  Bool.pick(String, Bool.or(String.eq(key, "Enter"), String.eq(key, "ArrowRight")), "ob-next",
    Bool.pick(String, String.eq(key, "ArrowLeft"), "ob-back", Bool.pick(String, String.eq(key, "Escape"), "ob-skip", "")))

def Ob.sym(+code: U32) -> String:
  Bool.pick(String, Bool.or(Bool.or(U32.is_eq(code, 65293), U32.is_eq(code, 65421)), U32.is_eq(code, 65363)), "ob-next",
    Bool.pick(String, U32.is_eq(code, 65361), "ob-back", Bool.pick(String, U32.is_eq(code, 65307), "ob-skip", "")))

# Every step has a title, a known target and words for every kind of
# client (checked for the first n steps, counting down)
def Ob.sound.i(+l: I.Loc, +i: Nat) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(Ob.title(l, i))), Ob.known(Ob.target.d(i))),
    Bool.and(Bool.and(Bool.not(String.is_empty(Ob.body(l, 0n, "7", i))), Bool.not(String.is_empty(Ob.body(l, 1n, "7", i)))), Bool.not(String.is_empty(Ob.body(l, 2n, "7", i)))))

def Ob.sound.go(+l: I.Loc, +n: Nat) -> Bool:
  match n:
    case 0n:
      True{}
    case 1n+m:
      Bool.and(Ob.sound.i(l, m), Ob.sound.go(l, m))

# (for every locale)
def Ob.sound() -> Bool:
  Bool.and(Ob.sound.go(I.En{}, Ob.count()), Ob.sound.go(I.Ja{}, Ob.count()))

# the moves keep to the steps, for every number from 0 to n (past the end
# they clamp): next and back stay within 0..count, a step on screen has a
# next that goes forward and (past the first) a back that goes back
def Ob.nav.i(+i: Nat) -> Bool:
  Bool.and(Bool.and(Nat.is_le(Ob.next(i), Ob.count()), Nat.is_lt(Ob.back(i), Ob.count())),
    Bool.and(Bool.or(Bool.not(Ob.live(i)), Nat.is_gt(Ob.next(i), i)),
      Bool.or(Bool.or(Bool.not(Ob.live(i)), Nat.is_eq(i, 0n)), Nat.is_lt(Ob.back(i), i))))

def Ob.nav(+n: Nat) -> Bool:
  match n:
    case 0n:
      Ob.nav.i(0n)
    case 1n+m:
      Bool.and(Ob.nav.i(1n+m), Ob.nav(m))
