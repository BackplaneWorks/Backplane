import Base
import ./check.bend as C
import ./fixtures/i18n_webphone.bend as F
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/tools.bend as TO
import ../src/web/dom.bend as D
import ../src/web/app.bend as WA
import ../src/web/view.bend as W
import ../src/web/bots.bend as B
import ../src/mobile/screen.bend as S
import ../src/mobile/notify.bend as N
import ../src/mobile/hubs.bend as H

# Test the actual render model, including its memo expansion, and phone JSON.
# English expectations below are copied from the pre-i18n literals.
def words.go(fuel: Nat, ns: List<&2, D.Node>, acc: String) -> String:
  match fuel ns:
    case 0n _: acc
    case 1n+_ Nil{}: acc
    case 1n+k D.Txt{t} <> rest: words.go(k, rest, acc ++ t ++ "|")
    case 1n+k D.El{_, _, +as, ks} <> rest:
      words.go(k, List.append(&2, D.Node, ks, rest), acc ++ W.View.attr(as, "title") ++ "|" ++ W.View.attr(as, "placeholder") ++ "|")

def words(n: D.Node) -> String:
  words.go(10000n, W.View.expand([n]), "")

def seen.k(hit: Bool, value: String) -> String:
  match hit:
    case True{}: value
    case False{}: "<missing>"

def seen(+s: String, +literal: String) -> String:
  seen.k(String.contains(s, literal), literal)

def web(tag: String) -> String:
  words(W.View.page(F.Fixture.thread(tag)))

def settings(tag: String, page: String) -> String:
  words(W.View.page(F.Fixture.settings.page(tag, page)))

def menu(tag: String) -> String:
  words(W.View.page(F.Fixture.menu(tag)))

def bot(tag: String) -> String:
  words(W.View.page(F.Fixture.bot(tag)))

def screen(tag: String) -> J.Json:
  S.Mob.screen(F.Fixture.thread(tag))

def thread(tag: String) -> J.Json:
  J.Json.get_obj(screen(tag), "thread")

def first(js: List<&2, J.Json>) -> J.Json:
  Maybe.default(&2, J.Json, List.head(&2, J.Json, js), J.Null{})

def project(tag: String) -> J.Json:
  first(J.Json.elems(J.Json.get_obj(screen(tag), "projects")))

def phone.settings(tag: String, page: String) -> String:
  J.Json.show(J.Json.get_obj(S.Mob.screen(F.Fixture.settings.page(tag, page)), "settings"))

def phone.plane(tag: String) -> String:
  J.Json.show(J.Json.get_obj(H.Hubs.screen(H.Hubs{"c", 200n, "test:3820", [H.Hub{"test:3820", U.Ui.fold([M.SettingSet{"machine.id", "test-machine", 109n}, M.UberSet{"b43", "u43", 110n}], F.Fixture.ui(tag))}]}), "plane"))

# each phone row as "<id>=<run>" (an open drawer's rows name their run)
def phone.runs(js: List<&2, J.Json>) -> List<&2, String>:
  match js:
    case Nil{}: Nil{}
    case +j <> t: (J.Json.get_str(j, "id", "?") ++ "=" ++ J.Json.get_str(j, "run", "")) <> phone.runs(t)

# Default is a wire key, even when the labels around it are Japanese.
def model.about() -> J.Json:
  J.Json.obj([J.KV{"labels", J.Json.obj([J.KV{"default", J.Json.str("Provider default") }])},
    J.KV{"efforts", J.Json.obj([J.KV{"default", J.Json.arr([J.Json.str("high")])}])}])

def model.picker() -> J.Json:
  S.Mob.picker.j.loc(I.Ja{}, J.Null{}, model.about(), "claude", "", "", "")

def main() -> IO(Unit):
  +en = web("en")
  +ja = web("ja")
  +se = settings("en", "agents")
  +sj = settings("ja", "agents")
  +ve = settings("en", "voice")
  +vj = settings("ja", "voice")
  do IO<Unit>:
    C.Check.str("web sidebar en", seen(en, "Add project"), "Add project")
    C.Check.str("web sidebar ja", seen(ja, "プロジェクトを追加"), "プロジェクトを追加")
    C.Check.str("web thread en", seen(en, "New thread"), "New thread")
    C.Check.str("web thread ja", seen(ja, "新しいスレッド"), "新しいスレッド")
    C.Check.str("web composer en", seen(en, "Ask anything: @bot, >thread, %project, #room, $skill, /btw"), "Ask anything: @bot, >thread, %project, #room, $skill, /btw")
    C.Check.str("web composer ja", seen(ja, "何でも質問：@bot、>thread、%project、#room、$skill、/btw"), "何でも質問：@bot、>thread、%project、#room、$skill、/btw")
    C.Check.str("web Settings en", seen(se, "Provider for new threads"), "Provider for new threads")
    C.Check.str("web Settings ja", seen(sj, "新しいスレッドのプロバイダー"), "新しいスレッドのプロバイダー")
    C.Check.str("web voice en", seen(ve, "OpenAI API key"), "OpenAI API key")
    C.Check.str("web voice ja", seen(vj, "OpenAI APIキー"), "OpenAI APIキー")
    C.Check.str("web menus en", seen(menu("en"), "Copy link"), "Copy link")
    C.Check.str("web menus ja", seen(menu("ja"), "リンクをコピー"), "リンクをコピー")
    C.Check.str("web bot en", seen(bot("en"), "Routines"), "Routines")
    C.Check.str("web bot ja", seen(bot("ja"), "ルーティン"), "ルーティン")
    C.Check.str("phone home en", J.Json.get_str(project("en"), "snoozedShelf", "?"), "Snoozed")
    C.Check.str("phone home ja", J.Json.get_str(project("ja"), "snoozedShelf", "?"), "スヌーズ中")
    C.Check.str("phone search en", J.Json.get_str(J.Json.get_obj(screen("en"), "search"), "hint", "?"), "Search")
    C.Check.str("phone search ja", J.Json.get_str(J.Json.get_obj(screen("ja"), "search"), "hint", "?"), "検索")
    C.Check.str("phone thread en", J.Json.get_str(thread("en"), "send", "?"), "Send")
    C.Check.str("phone thread ja", J.Json.get_str(thread("ja"), "send", "?"), "送信")
    C.Check.str("phone title ja", J.Json.get_str(thread("ja"), "title", "?"), "新しいスレッド")
    C.Check.str("phone Settings en", seen(phone.settings("en", "threads"), "Settle after 3 days"), "Settle after 3 days")
    C.Check.str("phone Settings ja", seen(phone.settings("ja", "threads"), "3日後に完了へ移動"), "3日後に完了へ移動")
    C.Check.str("phone menu sheet en", seen(J.Json.show(J.Json.get_obj(S.Mob.screen(F.Fixture.menu("en")), "rowMenu")), "Copy link"), "Copy link")
    C.Check.str("phone menu sheet ja", seen(J.Json.show(J.Json.get_obj(S.Mob.screen(F.Fixture.menu("ja")), "rowMenu")), "リンクをコピー"), "リンクをコピー")
    C.Check.str("phone notification en", N.Note.body.loc(I.En{}, "fail", ""), "Failed")
    C.Check.str("phone notification ja", N.Note.body.loc(I.Ja{}, "fail", ""), "失敗")
    C.Check.str("phone notification content", N.Note.body.loc(I.Ja{}, "done", "Finished in user content"), "Finished in user content")
    C.Check.str("phone island en", N.Note.headline.loc(I.En{}, 1200n, ""), "1200 agents working")
    C.Check.str("phone island ja", N.Note.headline.loc(I.Ja{}, 1200n, ""), "エージェント1200件が作業中")
    C.Check.str("phone no hubs en", H.Hubs.empty.loc(I.En{}, 0n, 0n, False{}), "Pair with a hub to start.")
    C.Check.str("phone no hubs ja", H.Hubs.empty.loc(I.Ja{}, 0n, 0n, False{}), "ハブとペアリングしてください。")
    C.Check.str("option English stored spelling", W.VLabel.option(I.En{}, "ui.theme", "dark"), "dark")
    C.Check.str("option translated label", W.VLabel.option(I.Ja{}, "ui.theme", "dark"), "ダーク")
    C.Check.str("model called dark stays intact", W.VLabel.option(I.Ja{}, "agent.model", "dark"), "dark")
    C.Check.str("large counts retain English digits", S.Mob.set.days.label.loc(I.En{}, "1200"), "Settle after 1200 days")
    C.Check.str("theme value stays protocol", J.Json.get_str(screen("ja"), "theme", "?"), "")
    C.Check.str("phone plane en", seen(phone.plane("en"), "Make this the home"), "Make this the home")
    C.Check.str("phone plane ja", seen(phone.plane("ja"), "ここをホームにする"), "ここをホームにする")
    C.Check.str("compact idle en", J.Json.get_str(J.Json.get_obj(screen("en"), "island"), "headline", "?"), "All agents idle")
    C.Check.str("compact idle ja", J.Json.get_str(J.Json.get_obj(screen("ja"), "island"), "headline", "?"), "すべてのエージェントが待機中")
    C.Check.str("subagent badge en", W.View.sub.badge(I.En{}, 1200n), "1200 agents")
    C.Check.str("subagent badge ja", W.View.sub.badge(I.Ja{}, 1200n), "エージェント1200件")
    C.Check.str("subagent idle panel en", W.View.sub.panel(I.En{}, Nil{}), "0 done")
    C.Check.str("subagent idle panel ja", W.View.sub.panel(I.Ja{}, Nil{}), "完了0件")
    C.Check.str("phone pin tooltip ja", J.Json.get_str(first(J.Json.elems(J.Json.get_obj(thread("ja"), "tools"))), "tip", "?"), "一覧の先頭に固定")
    C.Check.str("phone default model wire key", S.Mob.model_name.loc(I.Ja{}, model.about(), ""), "Provider default")
    C.Check.str("phone effort default wire key", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(model.picker(), "efforts")))), "2")
    C.Check.str("phone effort translated", J.Json.get_str(first(J.Json.elems(J.Json.get_obj(model.picker(), "efforts"))), "label", "?"), "自動")
    C.Check.str("phone effort value unchanged", J.Json.get_str(first(J.Json.elems(J.Json.get_obj(model.picker(), "efforts"))), "value", "?"), "")
    C.Check.str("phone model chip ja", J.Json.get_str(J.Json.get_obj(thread("ja"), "picker"), "label", "?"), "claude · 既定")
    C.Check.str("model id high unchanged", W.VLabel.model(I.Ja{}, "codex", "high"), "codex · high")
    C.Check.str("provider chip unchanged", W.VLabel.chip(I.Ja{}, "@codex"), "codex")
    C.Check.str("effort chip en", W.VLabel.chip(I.En{}, "high"), "high")
    C.Check.str("effort chip ja", W.VLabel.chip(I.Ja{}, "high"), "高")
    C.Check.str("3d progress en", WA.solid_waiting("en", 14n), "Preparing the model on the hub · 14 s")
    C.Check.str("3d progress ja", WA.solid_waiting("ja", 14n), "ハブでモデルを準備中 · 14秒")
    C.Check.str("3d cached hint boundary", seen(WA.solid_waiting("en", 15n), "after that it is cached"), "after that it is cached")
    C.Check.str("3d stats en", WA.solid_stats("en", "1.2 MB", "12k", "0.7", "0.4", True{}), "1.2 MB · 12k triangles · loaded in 0.7 s · built in 0.4 s")
    C.Check.str("3d stats ja", WA.solid_stats("ja", "1.2 MB", "12k", "0.7", "0.4", True{}), "1.2 MB · 三角形12k個 · 読み込み0.7秒 · 描画準備0.4秒")
    C.Check.str("3d stats immediate", WA.solid_stats("en", "1.2 MB", "12k", "0.5", "0.4", False{}), "1.2 MB · 12k triangles · built in 0.4 s")
    C.Check.str("2d stats en", WA.plot_stats("en", "357 KB", "2.1", True{}), "357 KB · loaded in 2.1 s")
    C.Check.str("2d stats ja", WA.plot_stats("ja", "357 KB", "2.1", True{}), "357 KB · 読み込み2.1秒")
    C.Check.str("2d stats immediate", WA.plot_stats("en", "357 KB", "0.3", False{}), "357 KB")
    C.Check.str("web bot header model en", seen(bot("en"), "claude · default"), "claude · default")
    C.Check.str("web bot header no English default", seen(bot("ja"), "claude · default"), "<missing>")
    C.Check.str("web upload confirmation en", seen(words(D.Dom.div("", W.View.ask.btns.loc(I.En{}, "send", "a", "", ""))), "Not now"), "Not now")
    C.Check.str("web upload confirmation ja", seen(words(D.Dom.div("", W.View.ask.btns.loc(I.Ja{}, "send", "a", "", ""))), "今はしない"), "今はしない")
    C.Check.str("phone upload confirmation ja", J.Json.get_str(first(S.Mob.ask.btns.loc(I.Ja{}, 2n, "a", "", "")), "label", "?"), "送信")
    C.Check.str("phone upload action preserved", J.Json.get_str(first(S.Mob.ask.btns.loc(I.Ja{}, 2n, "a", "", "")), "value", "?"), "a|accept")
    C.Check.str("phone minimized order route preserved", J.Json.get_str(first(S.Mob.menu.loc(I.Ja{}, "t", False{}, [TO.Tool{"shopping-bag", "Buy it", "", "fab-show", "t", False{}, False{}, False{}}])), "action", "?"), "fab-show")
    C.Check.str("ordering toggle localized", W.VLabel.option(I.Ja{}, "order.enabled", "off"), "オフ")
    C.Check.str("phone live drawer en", J.Json.show(first(S.Mob.entries(I.En{}, [M.Act{"a", "t", "tool", "Bash", "x", 0n}], M.State.empty(), "t", Map.new(&2, String), Map.new(&2, String), True{}))),
      "{\"id\":\"fold:a\",\"kind\":\"fold\",\"text\":\"1 tool call · running: Bash\",\"open\":false,\"live\":true,\"value\":\"1a\"}")
    C.Check.str("phone live drawer ja, shut", J.Json.get_str(first(S.Mob.entries(I.Ja{}, [M.Act{"a", "t", "tool", "Bash", "x", 0n}], M.State.empty(), "t", Map.new(&2, String), Map.new(&2, String), True{})), "text", "?") ++ " " ++ Nat.show(List.length(&2, J.Json, S.Mob.entries(I.Ja{}, [M.Act{"a", "t", "tool", "Bash", "x", 0n}], M.State.empty(), "t", Map.new(&2, String), Map.new(&2, String), True{}))),
      "ツール呼び出し1件 · 実行中: Bash 1")
    C.Check.str("phone opened drawer: its rows name their run", String.join(phone.runs(S.Mob.entries(I.En{}, [M.Act{"a", "t", "tool", "Bash", "x", 0n}, M.Act{"b", "t", "tool", "Read", "y", 1n}, M.Msg{"m", "t", M.Assistant{}, "done", 2n}], M.State.empty(), "t", Map.set(&2, String, Map.new(&2, String), "fold:a", "1"), Map.new(&2, String), False{})), ","),
      "fold:a=,a=a,b=a,m=")
