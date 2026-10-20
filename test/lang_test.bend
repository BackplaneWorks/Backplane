import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/core/prompt.bend as P
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/mobile/hubs.bend as H
import ../src/mobile/screen.bend as S
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/app/layout.bend as L
import ../src/app/nui.bend as N
import ./fixtures/lang_prompt_record.bend as REC
import ../src/server/server.bend as HUB
import ../src/server/kicad.bend as K
import ../src/core/bothub.bend as BH
import ../src/core/bot.bend as BT

# A recorded assembled prompt: off/unknown must not add even a newline.
def recorded() -> String:
  "system\n\n<project>test</project>\n\n# You are Kit, a Backplane bot\n"

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def preview() -> U.Ui:
  U.Ui.scratch_set(U.Ui.init(0n), "@lang.preview", "1")

def ja() -> U.Ui:
  U.Lang.apply(preview(), "auto", "ja-JP")

def sux() -> U.Ui:
  U.Lang.apply(U.Ui.init(0n), "sux", "en")

def offered(xs: List<&2, String>) -> String:
  String.join(xs, ",")

def switched() -> U.Ui:
  ui(U.Ui.machine(U.Ui.scratch_set(U.Lang.apply(preview(), "ja", "ja-JP"), "@menu", "open"), "other"))

def phones() -> H.Hubs:
  H.Hubs.local(H.Hubs.local(H.Hubs.local(H.Hubs.set(H.Hubs.init(0n, "c"), ["one", "two"]), "lang-preview", "1"), "lang", "auto"), "lang-os", "ja-JP")

def phone(+hs: H.Hubs, k: String) -> String:
  U.Ui.scratch(H.Hubs.ui(hs, k), "@lang.tag")

def hitrows(hs: List<&2, N.Hit>) -> String:
  match hs:
    case Nil{}: ""
    case N.Hit{_, _, _, _, a, v} <> t: a ++ "=" ++ v ++ ";" ++ hitrows(t)

def hits(out: L.Out) -> String:
  match out:
    case L.Out{_, hs}: hitrows(hs)

# Traverse the DOM with explicit fuel, avoiding mutually recursive walks.
def dom.todo(fuel: Nat, ns: List<&2, D.Node>, acc: String) -> String:
  match fuel ns:
    case _ Nil{}: acc
    case 0n _: acc
    case 1n+k D.Txt{s} <> t: dom.todo(k, t, acc ++ s ++ ";")
    case 1n+k D.El{_, _, _, kids} <> t: dom.todo(k, List.append(&2, D.Node, kids, t), acc)

def contains(+s: String, word: String) -> String:
  Bool.pick(String, String.contains(s, word), "yes", "no")

def lang.buttons(+u: U.Ui) -> String:
  J.Json.show(S.Mob.lang(u))

def agent_state(v: String) -> M.State:
  M.State.apply(M.State.empty(), M.SettingSet{"agent.lang", v, 0n})

def with_state(u: U.Ui, st: M.State) -> U.Ui:
  match u:
    case U.Ui{_, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def assembled(v: String) -> String:
  HUB.Hub.prompt(REC.cfg(), agent_state(v), "t", "/tmp/lang-project", K.Files{"", "", "", ""}, "")

def bot(v: String) -> M.State:
  M.State.apply(agent_state(v), M.BotSet{"b", "Kit", "t", 0n, "", 0n})

def window(+u: U.Ui) -> String:
  hits(L.Lay.settings(L.Ty.none(), U.Ui.state(u), N.Nui.init(u, 1400, 10000), Map.new(&2, String), "test", "", 264, 1400, 10000, 0))

def web(+u: U.Ui) -> String:
  dom.todo(3000n, [V.View.settings(u, U.Ui.state(u), Map.new(&2, String), "test", "")], "")

def sends(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{_} <> t: "rpc" ++ sends(t)
    case _ <> t: sends(t)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}: sends(cs)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("default", U.Lang.choice(U.Ui.init(0n)), "auto")
    C.Check.str("unknown normalizes", U.Lang.choice(U.Lang.apply(preview(), "garbage", "ja-JP")), "auto")
    C.Check.str("auto ja preview", U.Ui.scratch(ja(), "@lang.tag"), "ja")
    C.Check.str("gate", U.Lang.resolve("auto", "ja-JP", False{}), "en")
    C.Check.str("override", U.Lang.resolve("en", "ja-JP", True{}), "en")
    C.Check.str("unsupported OS", U.Lang.resolve("auto", "fr_FR", True{}), "en")
    C.Check.str("auto cuneiform OS", U.Lang.resolve("auto", "sux-Xsux", False{}), "sux-Xsux")
    C.Check.str("cuneiform override", U.Lang.resolve("sux", "en", False{}), "sux-Xsux")
    C.Check.str("cuneiform canonical override", U.Lang.resolve("sux-Xsux", "en", False{}), "sux-Xsux")
    C.Check.str("cuneiform choice", U.Lang.choice(sux()), "sux")
    C.Check.str("cuneiform selection", U.Ui.scratch(sux(), "@lang.tag"), "sux-Xsux")
    C.Check.str("live OS change", U.Ui.scratch(ui(U.Ui.act(U.Lang.apply(preview(), "auto", "en"), "lang-os", "ja-JP")), "@lang.tag"), "ja")
    C.Check.str("immediate choice", U.Ui.scratch(ui(U.Ui.act(ja(), "lang", "en")), "@lang.tag"), "en")
    C.Check.str("offered gated", offered(U.Lang.offered(U.Ui.init(0n))), "auto,en,sux")
    C.Check.str("offered preview", offered(U.Lang.offered(ja())), "auto,en,ja,sux")
    C.Check.str("English own name", U.Lang.label(I.Ja{}, "en"), "English")
    C.Check.str("Japanese own name", U.Lang.label(I.En{}, "ja"), "日本語")
    C.Check.str("Sumerian own name", U.Lang.label(I.En{}, "sux"), "𒅴𒂠")
    C.Check.str("switch choice", U.Ui.scratch(switched(), "@lang"), "ja")
    C.Check.str("switch os", U.Ui.scratch(switched(), "@lang.os"), "ja-JP")
    C.Check.str("switch resolved", U.Ui.scratch(switched(), "@lang.tag"), "ja")
    C.Check.str("switch preview", U.Ui.scratch(switched(), "@lang.preview"), "1")
    C.Check.str("switch drops other scratch", U.Ui.scratch(switched(), "@menu"), "")
    C.Check.str("reset keeps language", U.Ui.scratch(U.Ui.reset(ja(), J.Json.obj([])), "@lang.tag"), "ja")
    C.Check.str("hub one", phone(phones(), "one"), "ja")
    C.Check.str("hub two", phone(phones(), "two"), "ja")
    C.Check.str("later hub", phone(H.Hubs.set(phones(), ["one", "two", "three"]), "three"), "ja")
    C.Check.str("hub auto cuneiform", phone(H.Hubs.local(phones(), "lang-os", "sux-Xsux"), "one"), "sux-Xsux")
    C.Check.str("window Settings has offered chips", contains(window(ja()), "lang=ja;"), "yes")
    C.Check.str("window Settings offers cuneiform", contains(window(sux()), "lang=sux;"), "yes")
    C.Check.str("web Settings has localized row", contains(web(ja()), "エージェントも選択した言語で応答"), "yes")
    C.Check.str("loc reads only resolved tag", I.I18n.tag(U.Lang.loc(U.Ui.scratch_set(ja(), "@lang.os", "en"))), "ja")
    C.Check.str("loc resolves cuneiform", I.I18n.tag(U.Lang.loc(sux())), "sux-Xsux")
    C.Check.str("language action sends no RPC", sent(U.Ui.act(ja(), "lang", "en")), "")
    C.Check.str("OS action sends no RPC", sent(U.Ui.act(ja(), "lang-os", "en")), "")
    C.Check.str("window language chips", contains(hits(L.Set.lang(L.Ty.none(), ja(), 300, 720, 100)), "lang=ja;"), "yes")
    C.Check.str("cuneiform language chips", contains(hits(L.Set.lang(L.Ty.none(), sux(), 300, 720, 100)), "lang=sux;"), "yes")
    C.Check.str("window agent toggle", contains(hits(L.Set.lang(L.Ty.none(), ja(), 300, 720, 100)), "setting=agent.lang=ja;"), "yes")
    C.Check.str("agent language follows", M.Setting.get(U.Ui.state(ui(U.Ui.act(U.Ui.scratch_set(U.Lang.apply(with_state(preview(), agent_state("en")), "en", "ja-JP"), "@lang.preview", "1"), "lang", "ja"))), "agent.lang", "?"), "ja")
    C.Check.str("agent language off stays off", M.Setting.get(U.Ui.state(ui(U.Ui.act(U.Lang.apply(with_state(preview(), agent_state("off")), "en", "ja-JP"), "lang", "ja"))), "agent.lang", "?"), "off")
    C.Check.str("agent language follows cuneiform", M.Setting.get(U.Ui.state(ui(U.Ui.act(U.Lang.apply(with_state(preview(), agent_state("off")), "en", "en"), "lang", "sux"))), "agent.lang", "?"), "sux-Xsux")
    C.Check.str("window agent off chip", contains(hits(L.Set.lang(L.Ty.none(), ja(), 300, 720, 100)), "setting=agent.lang=off;"), "yes")
    C.Check.str("window gate", contains(hits(L.Set.lang(L.Ty.none(), U.Ui.init(0n), 300, 720, 100)), "lang=ja;"), "no")
    C.Check.str("web localized row", contains(dom.todo(100n, V.View.lang(ja()), ""), "言語"), "yes")
    C.Check.str("web offered chips", contains(dom.todo(100n, V.View.lang(ja()), ""), "日本語"), "yes")
    C.Check.str("web cuneiform language name", contains(dom.todo(100n, V.View.lang(sux()), ""), "𒅴𒂠"), "yes")
    C.Check.str("phone localized row", contains(lang.buttons(ja()), "エージェントも選択した言語で応答"), "yes")
    C.Check.str("phone offered chips", contains(lang.buttons(ja()), "日本語"), "yes")
    C.Check.str("phone merged screen", contains(J.Json.show(H.Hubs.screen(phones())), "エージェントも選択した言語で応答"), "yes")
    C.Check.str("assembled off recorded", assembled("off"), REC.recorded())
    C.Check.str("assembled default recorded", assembled("x"), REC.recorded())
    C.Check.str("assembled en once", assembled("en"), REC.recorded() ++ "\n" ++ P.Prompt.lang("en"))
    C.Check.str("assembled ja once", assembled("ja"), REC.recorded() ++ "\n" ++ P.Prompt.lang("ja"))
    C.Check.str("assembled cuneiform once", assembled("sux-Xsux"), REC.recorded() ++ "\n" ++ P.Prompt.lang("sux-Xsux"))
    C.Check.str("standalone prompt language", BH.Bots.prompt(agent_state("ja"), "t", "/tmp/lang-test"), "\n" ++ P.Prompt.lang("ja"))
    C.Check.str("bot off byte identical", BH.Bots.prompt(bot("off"), "t", "/tmp/lang-test"), BH.Bots.prompt.k(Some{BT.World.bot(M.State.bots(bot("off")), "b")}, bot("off"), "/tmp/lang-test"))
    C.Check.str("bot ja once", BH.Bots.prompt(bot("ja"), "t", "/tmp/lang-test"), BH.Bots.prompt(bot("off"), "t", "/tmp/lang-test") ++ "\n" ++ P.Prompt.lang("ja"))
    C.Check.str("bot cuneiform once", BH.Bots.prompt(bot("sux-Xsux"), "t", "/tmp/lang-test"), BH.Bots.prompt(bot("off"), "t", "/tmp/lang-test") ++ "\n" ++ P.Prompt.lang("sux-Xsux"))
    C.Check.str("prompt off", P.Prompt.lang("off"), "")
    C.Check.str("prompt unknown", P.Prompt.lang("x"), "")
    C.Check.str("prompt ja", P.Prompt.lang("ja"), "Reply in Japanese (日本語) unless the person writes in another language or asks otherwise. Keep code, commands, paths and identifiers as they are.")
    C.Check.str("prompt en", P.Prompt.lang("en"), "Reply in English unless the person writes in another language or asks otherwise. Keep code, commands, paths and identifiers as they are.")
    C.Check.str("prompt cuneiform", P.Prompt.lang("sux-Xsux"), "Reply in Sumerian using Unicode Sumero-Akkadian cuneiform unless the person writes in another language or asks otherwise. Keep code, commands, paths and identifiers as they are.")
    C.Check.str("recorded off byte identical", P.Prompt.lang.add(recorded(), "off"), recorded())
    C.Check.str("recorded unknown byte identical", P.Prompt.lang.add(recorded(), "x"), recorded())
