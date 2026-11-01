import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/slash.bend as S
import ../src/core/client.bend as U
import ../src/app/nui.bend as N

# Slash commands in the composer: `/` as a lone word offers the commands
# (Claude's own only in a Claude thread) then the skills; Esc shuts the
# menu until the draft changes; Tab/Enter completes.

def yes(b: Bool) -> String:
  match b:
    case True{}:
      "yes"
    case False{}:
      "no"

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def join(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      x ++ " " ++ join(t)

def names(rs: List<&2, U.SkillRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.SkillRow{n, _, _} <> t:
      n ++ " " ++ names(t)

def about(rs: List<&2, U.SkillRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.SkillRow{_, a, _} <> t:
      a ++ "|" ++ about(t)

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "board", "/tmp/board", 100n},
   M.ThreadCreated{"c", "p1", "Claude one", M.Local{}, "", "", "claude", "", 900000n},
   M.ThreadCreated{"x", "p1", "Codex one", M.Local{}, "", "", "codex", "", 900100n}]

def skills() -> String:
  "[{\"name\":\"pcb-review\",\"description\":\"Review\"},{\"name\":\"parts\",\"description\":\"Find parts\"}]"

# a UI on thread t with skills seeded for it
def start(+t: String) -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(ui(U.Ui.select(U.Ui.fold(log(), U.Ui.init(1000000n)), t)), "skills", skills()), "skills.for", t)

def focus.in(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Focus{f} <> _:
      f
    case _ <> t:
      focus.in(t)

def focus(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      focus.in(cs)

def typed(u: U.Ui, d: String) -> U.Ui:
  ui(U.Ui.draft(u, d))

def lit(rs: List<&2, U.SkillRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.SkillRow{n, _, True{}} <> _:
      n
    case U.SkillRow{_, _, False{}} <> t:
      lit(t)

# the window: a key pressed in the focused composer
def stepped(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n2, _, _}:
      n2

def press(+n: N.Nui, +code: U32) -> N.Nui:
  stepped(N.Nui.key(n, code, 0, 0))

def win(u: U.Ui) -> N.Nui:
  N.Nui.with_focus(N.Nui.init(u, 1200, 800), "composer")

# what the web draws: the same rows, marked and shut as the window's
def web(u: U.Ui) -> List<&2, U.SkillRow>:
  match u:
    case U.Ui{+st, +sel, +dr, _, _, _, _, _, _, _, _, _, _, _, +nf, _, _}:
      U.Skills.shown(st, sel, dr, nf)

def main() -> IO(Unit):
  +c = start("c")
  +x = start("x")
  +c1 = typed(c, "/")
  +c2 = typed(c, "/co")
  +c3 = typed(c, "/give x")
  +c4 = typed(c, "hi /gi")
  +closed = ui(U.Ui.act(c1, "skill-close", ""))
  +reopen = typed(closed, "/c")
  +back = typed(typed(closed, "/x"), "/")
  +again = typed(closed, "/")
  +picked = ui(U.Ui.act(typed(c, "/gi"), "skill", ""))
  +clicked = ui(U.Ui.act(c1, "skill", "/context"))
  +down = ui(U.Ui.act(c2, "skill-move", "down"))
  +wd = press(win(c2), N.Key.down())
  +we = press(win(c2), N.Key.escape())
  +d1 = typed(c, "$p")
  +dclosed = ui(U.Ui.act(d1, "skill-close", ""))
  do IO<Unit>:
    C.Check.str("typing /gi", yes(S.Slash.typing("/gi")), "yes")
    C.Check.str("typing /", yes(S.Slash.typing("/")), "yes")
    C.Check.str("not after a space", yes(S.Slash.typing("/give >a")), "no")
    C.Check.str("not mid line", yes(S.Slash.typing("hi /gi")), "no")
    C.Check.str("not empty", yes(S.Slash.typing("")), "no")
    C.Check.str("cmds claude", join(S.Slash.names(S.Slash.cmds(True{}, ""))), "/btw /give /secret /compact /context ")
    C.Check.str("cmds other", join(S.Slash.names(S.Slash.cmds(False{}, ""))), "/btw /give /secret ")
    C.Check.str("cmds prefix", join(S.Slash.names(S.Slash.cmds(True{}, "c"))), "/compact /context ")
    C.Check.str("cmds none", join(S.Slash.names(S.Slash.cmds(True{}, "zz"))), "")
    C.Check.str("claude / offers all", names(U.Ui.skills(c1)), "/btw /give /secret /compact /context $pcb-review $parts ")
    C.Check.str("rows carry about", about(U.Ui.skills(typed(c, "/gi"))), "Hand this conversation to a >thread, %project or @bot|")
    C.Check.str("/co", names(U.Ui.skills(c2)), "/compact /context ")
    C.Check.str("/give x offers nothing", names(U.Ui.skills(c3)), "")
    C.Check.str("midline offers nothing", names(U.Ui.skills(c4)), "")
    C.Check.str("skills follow by prefix", names(U.Ui.skills(typed(c, "/p"))), "$pcb-review $parts ")
    C.Check.str("codex gets its own", names(U.Ui.skills(typed(x, "/"))), "/btw /give /secret $pcb-review $parts ")
    C.Check.str("esc shuts", names(U.Ui.skills(closed)), "")
    C.Check.str("edit reopens", names(U.Ui.skills(reopen)), "/compact /context ")
    C.Check.str("tab completes", U.Ui.draft_text(picked), "/give ")
    C.Check.str("click completes", U.Ui.draft_text(clicked), "/context ")
    C.Check.str("dollar still offers", names(U.Ui.skills(d1)), "$pcb-review $parts ")
    C.Check.str("esc shuts dollar", names(U.Ui.skills(dclosed)), "")
    C.Check.str("dollar narrows", names(U.Ui.skills(typed(c, "run $pa"))), "$parts ")
    C.Check.str("first row lit", lit(U.Ui.skills(c2)), "/compact")
    C.Check.str("down moves the light", lit(U.Ui.skills(down)), "/context")
    C.Check.str("enter takes the lit row", U.Ui.draft_text(ui(U.Ui.act(down, "skill", ""))), "/context ")
    C.Check.str("web lit like the window", lit(web(down)), "/context")
    C.Check.str("web shut by esc", names(web(closed)), "")
    C.Check.str("window down moves the light", lit(U.Ui.skills(N.Nui.ui(wd))), "/context")
    C.Check.str("window esc shuts", names(U.Ui.skills(N.Nui.ui(we))), "")
    C.Check.str("window esc keeps the composer", N.Nui.focus_of(we), "composer")
    C.Check.str("esc keeps the keyboard", focus(U.Ui.act(c1, "skill-close", "")), "composer")
    C.Check.str("a completion keeps the keyboard", focus(U.Ui.act(typed(c, "/gi"), "skill", "")), "composer")
    C.Check.str("shut, then the same text again later, opens", names(U.Ui.skills(back)), "/btw /give /secret /compact /context $pcb-review $parts ")
    C.Check.str("the same text sent again stays shut", names(U.Ui.skills(again)), "")
