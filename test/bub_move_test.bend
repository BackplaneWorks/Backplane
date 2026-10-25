import Base
import ./check.bend as C
import ../src/core/bub-move.bend as H
import ../src/core/json.bend as J
import ../src/core/cat.bend as CT
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/ubersync.bend as US
import ../src/core/ubop.bend as UO
import ../src/mobile/screen.bend as MS
import ../src/mobile/hubs.bend as MH
import ../src/mobile/thin.bend as MT
import ../src/web/view.bend as WV
import ../src/web/presentation.bend as P

def lease(+uid: String, +holder: String, +term: String, +stamp: String, +name: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"uid", J.Str{uid}}, J.KV{"holder", J.Str{holder}}, J.KV{"term", J.Str{term}},
    J.KV{"stamp", J.Str{stamp}}, J.KV{"name", J.Str{name}}, J.KV{"look", J.Json.nat(0n)}]))

def move(before: String, after: String) -> String:
  J.Json.show(H.Hop.text(before, after))

def sample() -> String:
  move(lease("bub", "desktop", "1", "0000000001.000000.desktop", "desktop"), lease("bub", "boat", "2", "0000000002.000000.boat", "the Boat"))

def actual_named(name: String) -> U.Ui:
  U.Ui.fold([M.ThreadCreated{"t", "", "Bub", M.Local{}, "", "", "claude", "", 1n}, M.BotSet{"b", name, "t", 7n, "", 1n},
    M.SettingSet{"machine.id", "desktop", 1n}, M.UberSet{"b", "uid-bub", 1n},
    M.UberOp{"uid-bub", "desktop", 1n, "0000000002.000000.desktop", UO.Ub.b.lease("desktop", 1n), 2n}], U.Ui.init(2n))

def actual() -> U.Ui:
  actual_named("Bub")

def changed(+uid: String, +holder: String, +term: Nat, +stamp: String, +next_uid: String, +next_holder: String, +next_term: Nat, +next_stamp: String) -> String:
  Bool.show(H.Hop.changed(uid, holder, term, stamp, next_uid, next_holder, next_term, next_stamp))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("new committed holder and term", changed("u", "desk", 1n, "001", "u", "boat", 2n, "002"), "True")
    C.Check.str("same-term winning stamp can move holder", changed("u", "desk", 2n, "001", "u", "boat", 2n, "002"), "True")
    C.Check.str("new term wins even with earlier stamp", changed("u", "desk", 1n, "009", "u", "boat", 2n, "001"), "True")
    C.Check.str("older term cannot announce despite stamp", changed("u", "desk", 2n, "001", "u", "boat", 1n, "009"), "False")
    C.Check.str("older same-term stamp cannot announce", changed("u", "desk", 2n, "009", "u", "boat", 2n, "001"), "False")
    C.Check.str("equal register does not move", changed("u", "desk", 2n, "002", "u", "boat", 2n, "002"), "False")
    C.Check.str("same holder renewal is quiet", changed("u", "desk", 1n, "001", "u", "desk", 2n, "002"), "False")
    C.Check.str("new uberbot is a baseline", changed("u", "desk", 1n, "001", "v", "boat", 2n, "002"), "False")
    C.Check.str("empty uid cannot announce", changed("", "desk", 1n, "001", "", "boat", 2n, "002"), "False")
    C.Check.str("empty previous holder is a baseline", changed("u", "", 1n, "001", "u", "boat", 2n, "002"), "False")
    C.Check.str("empty next holder cannot announce", changed("u", "desk", 1n, "001", "u", "", 2n, "002"), "False")
    C.Check.str("transfer cat has no SMIL", Bool.show(String.contains(H.Hop.cat(0n), "animate")), "False")
    C.Check.str("transfer rig has no part animations", Nat.show(CT.Cat.anims.count(H.Hop.rig(0n))), "0")
    C.Check.str("snapshot reads committed holder", J.Json.get_str(H.Hop.snapshot(actual()), "holder", ""), "desktop")
    C.Check.str("snapshot reads winning stamp", J.Json.get_str(H.Hop.snapshot(actual()), "stamp", ""), "0000000002.000000.desktop")
    C.Check.str("snapshot uses Bub look", Nat.show(J.Json.get_nat(H.Hop.snapshot(actual()), "look", 0n)), "7")
    C.Check.str("snapshot keeps machine and bot names separate", J.Json.get_str(H.Hop.snapshot(actual_named("Tub")), "botName", ""), "Tub")
    C.Check.str("title uses current uberbot name", H.Hop.title("Tub"), "Tub’s moving")
    C.Check.str("title trims uberbot name", H.Hop.title(" Tub "), "Tub’s moving")
    C.Check.str("blank name falls back to Bub", H.Hop.title("  "), "Bub’s moving")
    C.Check.str("older snapshot falls back to Bub", J.Json.get_str(H.Hop.text(lease("bub", "desktop", "1", "001", "desktop"), lease("bub", "boat", "2", "002", "the Boat")), "title", ""), "Bub’s moving")
    C.Check.str("move title comes from destination bot snapshot", J.Json.get_str(H.Hop.result(True{}, J.Null{}, J.Json.obj([J.KV{"botName", J.Str{"Tub"}}])), "title", ""), "Tub’s moving")
    C.Check.str("web projects snapshot in every page", Bool.show(String.contains(J.Json.show(P.Node.write(WV.View.page(actual()))), "data-bub-home")), "True")
    C.Check.str("phone projects snapshot without Status tab", J.Json.get_str(J.Json.get_obj(MS.Mob.screen(actual()), "bubHome"), "holder", ""), "desktop")
    C.Check.str("thin phone keeps lease snapshot", J.Json.get_str(J.Json.get_obj(MT.Partial.screen(MS.Mob.screen(actual()), J.Json.obj(Nil{})), "bubHome"), "holder", ""), "desktop")
    C.Check.str("paired phone hubs retain both lease snapshots", Nat.show(List.length(&2, J.Json, MH.Hubs.bub.homes([MH.Shown{"desktop-link", "desktop", MS.Mob.screen(actual())}, MH.Shown{"boat-link", "Boat", MS.Mob.screen(actual())}]))), "2")
