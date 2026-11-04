import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/web/phone.bend as PHN

# The phone arrangement's own state (web phone.bend): only a page that says
# it is a phone's gets it; the open tab is this client's scratch (Now by
# default, an unknown name is Now); Settings is the shared "settings" flag,
# which the other tabs shut.

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

def main() -> IO(Unit):
  +desk = U.Ui.init(1000000n)
  +phone = U.Ui.info_set(U.Ui.init(1000000n), "local.workspace.form", "phone")
  +bots = ui(PHN.Phone.act(phone, "ph-tab", "bots"))
  +odd = ui(PHN.Phone.act(phone, "ph-tab", "nonsense"))
  +set = ui(PHN.Phone.act(phone, "ph-tab", "settings"))
  +again = ui(PHN.Phone.act(set, "ph-tab", "settings"))
  +proj = ui(PHN.Phone.act(set, "ph-tab", "projects"))
  +sheet = ui(PHN.Phone.act(phone, "ph-sheet", "rename"))
  do IO<Unit>:
    C.Check.str("a phone's page", yes(PHN.Phone.on(phone)), "yes")
    C.Check.str("any other page", yes(PHN.Phone.on(desk)), "no")
    C.Check.str("Now by default", PHN.Phone.tab(phone), "now")
    C.Check.str("Bots", PHN.Phone.tab(bots), "bots")
    C.Check.str("an unknown tab is Now", PHN.Phone.tab(odd), "now")
    C.Check.str("Settings is the flag", yes(U.Ui.flag(set, "settings")), "yes")
    C.Check.str("Settings again stays open", yes(U.Ui.flag(again, "settings")), "yes")
    C.Check.str("Projects shuts Settings", yes(U.Ui.flag(proj, "settings")), "no")
    C.Check.str("Projects", PHN.Phone.tab(proj), "projects")
    C.Check.str("the tab is scratch only", U.Ui.info(bots, "@ph.tab"), "")
    C.Check.str("a sheet", U.Ui.scratch(sheet, "@ph.sheet"), "rename")
    C.Check.str("ph- actions", yes(PHN.Phone.handles("ph-tab")), "yes")
    C.Check.str("others pass by", yes(PHN.Phone.handles("select")), "no")
