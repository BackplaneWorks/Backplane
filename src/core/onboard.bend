import Base

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
  10n

def Ob.title(i: Nat) -> String:
  match i:
    case 0n:
      "Welcome to Backplane"
    case 1n:
      "Projects"
    case 2n:
      "Threads"
    case 3n:
      "The thread's buttons"
    case 4n:
      "Bots"
    case 5n:
      "Your agents"
    case 6n:
      "Keys and voice"
    case 7n:
      "Other machines"
    case 8n:
      "Phones and browsers"
    case 9n:
      "Your uberbot"
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
def Ob.body.s0(+k: Nat) -> String:
  match k:
    case 2n:
      "A short tour of what is where. Skip it any time; More, Settings shows it again."
    case 1n:
      "A short tour of what is where. Enter goes on (on a focused button it presses that button), Esc skips it, and Settings shows it again."
    case _:
      "A short tour of what is where. Enter goes on, Esc skips it, and Settings shows it again."

def Ob.body.s1(+k: Nat, +d: String) -> String:
  "A project is a folder, usually a KiCad project. Add one with " ++ Bool.pick(String, Ob.phone(k), "the folder-plus button on the project list", "+ at the top of the sidebar")
    ++ ". Scratch holds chats with no project. It is temporary: a thread left alone for " ++ d ++ " days is deleted with its folder, unless pinned."

def Ob.body.s2() -> String:
  "A thread is a conversation with an agent working on a project. Git projects use separate worktrees by default; without git, or in local mode, threads share the project folder. Runs of tool calls fold into one row, and delegated work shows under Subagents."

def Ob.body.s3(+k: Nat) -> String:
  Bool.pick(String, Ob.phone(k), "In a thread's toolbar menu, Settle takes it out of the active list. Snooze hides it until later. Fork starts a new thread from it. The viewer shows the board, schematic, 3D and files. Pin, archive and delete are there too.",
    "Settle takes a thread out of the active list. Snooze hides it until later. Fork starts a new thread from this one. Viewer opens the board, schematic, 3D and files beside it. Pin, archive and delete are there too.")

def Ob.body.s4(+k: Nat) -> String:
  "A bot is an agent that stays between chats, with a name, a cat and a memory. Add one under Bots in the " ++ Bool.pick(String, Ob.phone(k), "list", "sidebar")
    ++ ", then mention it in any message: @scout look at the BOM."

def Ob.body.s5() -> String:
  "Backplane runs the Claude Code, Codex, Grok, Muse Code or oh-my-pi command-line program: install one and sign in to it on the machine that runs this hub. Settings shows whether each is found, not whether you are signed in. Google's Antigravity agent installs and signs in from Settings. Choose the provider and model in a thread's composer, or in Settings for new threads."

def Ob.body.s6(+k: Nat) -> String:
  "API keys for other services are kept in a file on the hub's machine that agents running there could read. They are told not to print them; nothing enforces it. "
    ++ Bool.pick(String, Ob.phone(k), "Dictation's OpenAI key is set in Settings on a computer or the web page.", "Dictation needs an OpenAI key, set under Voice in Settings.")

# How other machines are found and kept in sync (server.bend's Machines.*
# every two minutes: tailscale status, then GET /hello on each online
# machine of yours; client.bend's Mate.*: the hub with the lower address
# asks the other to link, which only your own devices may, and the two
# then mirror each other's projects and threads: far.bend). Settings' row
# is client.bend's Mate.title().
def Ob.body.s7(+k: Nat) -> String:
  Bool.pick(String, Ob.phone(k),
    "Each home folder runs one hub. Hubs on your computers find each other over Tailscale and sync each other's projects and threads in the background, so any one of them shows them all. A project still loading is faded and says syncing. In a message, >desktop: names a thread on the machine called desktop. The Hubs button lists the hubs this phone is paired with; this phone needs Tailscale on the same tailnet.",
    "Each home folder runs one hub. With Tailscale signed in when Backplane starts, the hub listens on your tailnet and every two minutes asks your other online machines whether Backplane runs there. Tailscale vouches that they are yours, so the hubs link by themselves and sync each other's projects and threads into this sidebar in the background: no switching. A project still loading is faded and says syncing; the rest work meanwhile. In a message, >desktop: names a thread on the machine called desktop. Settings, Your machines shows each one and turns sync off for any of them. Settings also says Tailscale: on, not connected (sign in, then restart Backplane) or off (started with --no-tailscale).")

def Ob.body.s8(+k: Nat) -> String:
  Bool.pick(String, Ob.phone(k),
    "On the hub's computer open Settings, Pairing link, QR code. Here, Hubs, Scan QR code pairs it. This phone needs Tailscale on the same tailnet. A browser can open a hub's web address too.",
    "To pair a phone, install Tailscale on it and join the same tailnet. Then open Settings, Pairing link, QR code and scan it with the Backplane app or the phone's camera. Settings shows the link when Backplane has one. A browser on a tailnet device can open this hub's web address too.")

# The last step is the uberbot (core/uber.bend): the bot the person makes
# their main interface. Its words depend on the case client.bend's
# Tour.uber.case gives: "none" (no bots here), "pick" (bots, no uberbot;
# `name` is the one used most), "have" (an uberbot, `name`), or "join"
# (reserved: a linked machine has one, `name`; never produced yet).
def Ob.uber() -> Nat:
  9n

def Ob.body.s9.lead() -> String:
  "Your uberbot is the cat you talk to first: it sees every project, thread, bot and machine, and its Status tab shows what runs and what waits on you."

def Ob.body.s9(+k: Nat, +uc: String, +name: String) -> String:
  Bool.pick(String, String.eq(uc, "have"), name ++ " is your uberbot. Click it at the top of the " ++ Bool.pick(String, Ob.phone(k), "list", "sidebar") ++ " for the Status of everything.",
    Bool.pick(String, String.eq(uc, "join"), "Another of your machines has an uberbot, " ++ name ++ ".",
      Bool.pick(String, String.eq(uc, "pick"), Ob.body.s9.lead() ++ " Make one of your bots the uberbot: " ++ name ++ " is the one you use most. You can change it later.",
        Ob.body.s9.lead() ++ " Create it now; you can rename it later.")))

def Ob.body(+k: Nat, +d: String, i: Nat) -> String:
  match i:
    case 0n:
      Ob.body.s0(k)
    case 1n:
      Ob.body.s1(k, d)
    case 2n:
      Ob.body.s2()
    case 3n:
      Ob.body.s3(k)
    case 4n:
      Ob.body.s4(k)
    case 5n:
      Ob.body.s5()
    case 6n:
      Ob.body.s6(k)
    case 7n:
      Ob.body.s7(k)
    case 8n:
      Ob.body.s8(k)
    case 9n:
      Ob.body.s9(k, "none", "")
    case _:
      ""

# the words of step i for a case of the uberbot step; other steps ignore it
def Ob.body.uber(+k: Nat, +d: String, +uc: String, +name: String, +i: Nat) -> String:
  Bool.pick(String, Nat.is_eq(i, Ob.uber()), Ob.body.s9(k, uc, name), Ob.body(k, d, i))

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

def Ob.forward(i: Nat) -> String:
  Bool.pick(String, Ob.last(i), "Done", "Next")

# "3 of 9"
def Ob.counter(i: Nat) -> String:
  Nat.show(1n+i) ++ " of " ++ Nat.show(Ob.count())

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
def Ob.sound.i(+i: Nat) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(Ob.title(i))), Ob.known(Ob.target.d(i))),
    Bool.and(Bool.and(Bool.not(String.is_empty(Ob.body(0n, "7", i))), Bool.not(String.is_empty(Ob.body(1n, "7", i)))), Bool.not(String.is_empty(Ob.body(2n, "7", i)))))

def Ob.sound.go(+n: Nat) -> Bool:
  match n:
    case 0n:
      True{}
    case 1n+m:
      Bool.and(Ob.sound.i(m), Ob.sound.go(m))

def Ob.sound() -> Bool:
  Ob.sound.go(Ob.count())

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
