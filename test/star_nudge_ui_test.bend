import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/star-nudge.bend as ST
import ../src/mobile/screen.bend as S
import ../src/mobile/hubs.bend as H
import ../src/mobile/thin.bend as T
import ../src/core/i18n.bend as I
import ../src/web/view.bend as V
import ../src/web/presentation.bend as P
import ../src/web/dom.bend as D

# Use the actual shared ledger and targeted offer, not a renderer-only flag.
def eligible() -> U.Ui:
  +st = M.State.replay([M.ProjectCreated{"p", "Project", "/fixture", 1n}, M.ThreadCreated{"t", "p", "Thread", M.Local{}, "", "", "codex", "", 2n},
    M.SettingSet{"onboard.done", "1", 3n}, M.SettingSet{"onboard.uber", "1", 3n},
    M.SettingSet{ST.Star.key(), J.Json.show(ST.Star.json(["a", "b", "c", "d", "offer"], 10n, "offer")), 10n}], M.State.empty())
  U.Ui.info_set(U.Ui.select.k(U.Ui.scratch_set(U.Ui.online(U.Ui.adopt.raw(U.Ui.init(20n), st), True{}), "@sync", "1"), "t"), "star.nudge.offer", "offer")

def web(+u: U.Ui) -> String:
  J.Json.show(P.Node.write(V.View.page(u)))

def mobile(u: U.Ui) -> J.Json:
  J.Json.get_obj(S.Mob.screen(u), "starNudge")

def web.has(u: U.Ui) -> String:
  Bool.show(String.contains(web(u), "star-nudge"))

def mobile.has(u: U.Ui) -> String:
  Bool.show(S.Mob.up(mobile(u)))

def decided(+status: String) -> U.Ui:
  U.Ui.adopt.raw(eligible(), M.State.setting(U.Ui.state(eligible()), ST.Star.key(), "{\"status\":\"" ++ status ++ "\",\"token\":\"offer\"}"))

def touring() -> U.Ui:
  U.Ui.adopt.raw(eligible(), M.State.setting(U.Ui.state(eligible()), "onboard.done", ""))

def merged(+focus: String, bOffer: Bool) -> J.Json:
  H.Hubs.screen.k(I.En{}, [H.Shown{"a", "Hub A", S.Mob.screen(eligible())},
    H.Shown{"b", "Hub B", S.Mob.screen(Bool.pick(U.Ui, bOffer, eligible(), U.Ui.info_set(eligible(), "star.nudge.offer", "")))}], focus, Nil{}, False{})

def main() -> IO(Unit):
  +page = web(eligible())
  +phone = mobile(eligible())
  +setup = U.Act.ui(U.Ui.act(eligible(), "provider-setup-open", ""))
  do IO<Unit>:
    C.Check.str("eligible offer appears on full web page", web.has(eligible()), "True")
    C.Check.str("eligible offer appears on phone snapshot", mobile.has(eligible()), "True")
    C.Check.str("web friendly body from shared API", Bool.show(String.contains(page, U.Star.body())), "True")
    C.Check.str("web status role is nonmodal", Bool.show(String.contains(page, "\"value\":\"status\"")), "True")
    C.Check.str("web close has accessible label", Bool.show(String.contains(page, "Dismiss star suggestion")), "True")
    C.Check.str("web contains explicit star action", Bool.show(String.contains(page, "\"action\":\"star-open\"")), "True")
    C.Check.str("web contains explicit dismiss action", Bool.show(String.contains(page, "\"action\":\"star-dismiss\"")), "True")
    C.Check.str("web star link uses official URL", Bool.show(String.contains(page, "\"value\":\"" ++ U.Star.url() ++ "\"")), "True")
    C.Check.str("web action carries offered token", Bool.show(String.contains(page, "\"name\":\"data-value\",\"value\":\"offer\"")), "True")
    C.Check.str("mobile body comes from shared API", J.Json.get_str(phone, "text", ""), U.Star.body())
    C.Check.str("mobile URL comes from shared API", J.Json.get_str(phone, "url", ""), U.Star.url())
    C.Check.str("mobile carries offered token", J.Json.get_str(phone, "token", ""), "offer")
    C.Check.str("web absent without targeted offer", web.has(U.Ui.info_set(eligible(), "star.nudge.offer", "")), "False")
    C.Check.str("mobile absent without targeted offer", mobile.has(U.Ui.info_set(eligible(), "star.nudge.offer", "")), "False")
    C.Check.str("web absent for wrong offer token", web.has(U.Ui.info_set(eligible(), "star.nudge.offer", "other")), "False")
    C.Check.str("mobile absent for wrong offer token", mobile.has(U.Ui.info_set(eligible(), "star.nudge.offer", "other")), "False")
    C.Check.str("web defers for tour", web.has(touring()), "False")
    C.Check.str("mobile defers for tour", mobile.has(touring()), "False")
    C.Check.str("web defers for provider setup", web.has(setup), "False")
    C.Check.str("mobile defers for provider setup", mobile.has(setup), "False")
    C.Check.str("web defers for settings", web.has(U.Ui.flag_on(eligible(), "settings")), "False")
    C.Check.str("mobile defers for settings", mobile.has(U.Ui.flag_on(eligible(), "settings")), "False")
    C.Check.str("web defers for row menu", web.has(U.Ui.scratch_set(eligible(), "@menu", "r:t")), "False")
    C.Check.str("mobile defers for row menu", mobile.has(U.Ui.scratch_set(eligible(), "@menu", "r:t")), "False")
    C.Check.str("web defers for folder picker", web.has(U.Ui.flag_on(eligible(), "proj")), "False")
    C.Check.str("mobile defers for folder picker", mobile.has(U.Ui.flag_on(eligible(), "proj")), "False")
    C.Check.str("web returns after sheet closes", web.has(U.Ui.flag_off(U.Ui.flag_on(eligible(), "settings"), "settings")), "True")
    C.Check.str("mobile returns after sheet closes", mobile.has(U.Ui.flag_off(U.Ui.flag_on(eligible(), "settings"), "settings")), "True")
    C.Check.str("dismiss removes web banner immediately", web.has(U.Act.ui(U.Ui.act(eligible(), "star-dismiss", "offer"))), "False")
    C.Check.str("open removes phone banner immediately", mobile.has(U.Act.ui(U.Ui.act(eligible(), "star-open", "offer"))), "False")
    C.Check.str("persisted dismiss hides stale web offer", web.has(decided("dismissed")), "False")
    C.Check.str("persisted open hides stale phone offer", mobile.has(decided("opened")), "False")
    C.Check.str("ineligible helper creates no DOM", Nat.show(List.length(&2, D.Node, V.View.star.k(False{}, "offer"))), "0")
    C.Check.str("merged phone retains focused hub offer", J.Json.get_str(J.Json.get_obj(merged("a", False{}), "starNudge"), "token", ""), "offer")
    C.Check.str("background hub offer stays off unfocused phone", J.Json.show(J.Json.get_obj(merged("b", False{}), "starNudge")), "null")
    C.Check.str("switching focus reveals that hub's own offer", J.Json.get_str(J.Json.get_obj(merged("b", True{}), "starNudge"), "token", ""), "offer")
    C.Check.str("thin phone projection retains suggestion", J.Json.get_str(J.Json.get_obj(T.Partial.screen(S.Mob.screen(eligible()), J.Json.obj(Nil{})), "starNudge"), "token", ""), "offer")
