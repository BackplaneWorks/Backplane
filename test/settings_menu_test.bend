import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/web/settings-menu.bend as SM

# The desktop and web Settings menu (web/settings-menu.bend): every page the
# menus list (client.bend's Settings.menu.pages) has a card in one of its
# groups, with its own note and icon. (Devices had none: the pairing screen
# said "Settings → Devices" and there was no Devices to open.)

def has(xs: List<&2, String>, +p: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +x <> t: Bool.or(String.eq(x, p), has(t, p))

def in.groups(+p: String) -> Bool:
  Bool.or(Bool.or(has(SM.Menu.group.pages(0n), p), has(SM.Menu.group.pages(1n), p)), Bool.or(has(SM.Menu.group.pages(2n), p), has(SM.Menu.group.pages(3n), p)))

# the pages the menus list that no group shows
def missing(ps: List<&2, String>) -> String:
  match ps:
    case Nil{}: ""
    case +p <> t: Bool.pick(String, in.groups(p), "", p ++ " ") ++ missing(t)

# pages without a note of their own, or with the fallback icon
def bare(ps: List<&2, String>) -> String:
  match ps:
    case Nil{}: ""
    case +p <> t: Bool.pick(String, Bool.or(String.is_empty(SM.Menu.note(I.En{}, p)), String.eq(SM.Menu.icon(p), "settings")), p ++ " ", "") ++ bare(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("every listed page is in a group", missing(U.Settings.menu.pages()), "")
    C.Check.str("every listed page has a note and an icon", bare(U.Settings.menu.pages()), "")
    C.Check.str("devices sits with the machine's pages", Bool.show(has(SM.Menu.group.pages(3n), "devices")), "True")
