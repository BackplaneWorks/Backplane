import Base
import ./dom.bend as D
import ./icon.bend as W
import ../core/client.bend as U
import ../core/i18n.bend as I
import ../core/codec.bend as C
import ../core/msgs.bend as MSG

# Settings keeps its existing page IDs, actions and localized titles.
# This module only describes the category menu and its context labels.
def Menu.text(lc: I.Loc, en: String, ja: String) -> String:
  match lc:
    case I.En{}: en
    case I.Ja{}: ja

def Menu.icon.i(i: Nat) -> String:
  match i:
    case 0n: "sun"
    case 1n: "message-square"
    case 2n: "cpu"
    case 3n: "square-pen"
    case 4n: "circuit-board"
    case 5n: "radio"
    case 6n: "mic"
    case 7n: "terminal"
    case 8n: "zap"
    case 9n: "clock"
    case 10n: "layers"
    case 11n: "book"
    case 12n: "circle-check"
    case 13n: "box"
    case _: "settings"

def Menu.icon(page: String) -> String:
  Menu.icon.i(C.Codec.pos(U.Settings.pages(), page))

def Menu.note.i(i: Nat, +lc: I.Loc) -> String:
  match i:
    case 0n: Menu.text(lc, "Theme, text size, language and sidebar", "テーマ、文字サイズ、言語、サイドバー")
    case 1n: Menu.text(lc, "Notifications, settling and restart behavior", "通知、完了済みスレッド、再起動時の動作")
    case 2n: Menu.text(lc, "Default models, effort and fallback order", "既定のモデル、推論レベル、フォールバック順序")
    case 3n: Menu.text(lc, "Models for titles, summaries and writing", "タイトル、要約、文章作成に使うモデル")
    case 4n: Menu.text(lc, "KiCad, FreeCAD and viewer connections", "KiCad、FreeCAD、ビューアーの接続")
    case 5n: Menu.text(lc, "Pair devices, sync machines and updates", "デバイスのペアリング、同期、更新")
    case 6n: Menu.text(lc, "Microphone, dictation and vocabulary", "マイク、音声入力、用語辞書")
    case 7n: Menu.text(lc, "Manage provider API credentials", "プロバイダーのAPI認証情報を管理")
    case 8n: Menu.text(lc, "Install and sign in to Google's agent", "Googleのエージェントのインストールとログイン")
    case 9n: Menu.text(lc, "Local time for routines and reminders", "ルーチンとリマインダーのタイムゾーン")
    case 10n: Menu.text(lc, "Daily tokens, project usage and limits", "日別トークン、プロジェクト別の使用量、上限")
    case 11n: Menu.text(lc, "About Backplane and the guided tour", "Backplaneについて、使い方のツアー")
    case 12n: Menu.text(lc, "Control anonymous usage sharing", "匿名の使用状況の共有設定")
    case 13n: Menu.text(lc, "Manufacturing, quotes and price estimates", "製造、見積もり、概算価格")
    case _: ""

def Menu.note(+lc: I.Loc, page: String) -> String:
  Menu.note.i(C.Codec.pos(U.Settings.pages(), page), lc)

def Menu.card(+lc: I.Loc, +page: String, +hint: String) -> D.Node:
  D.El{"button", "settings-category-" ++ page, [D.At{"class", "settings-category"}, D.At{"data-value", page}, D.On{"click", "settings-page"},
    D.At{"aria-labelledby", "settings-label-" ++ page}, D.At{"aria-describedby", "settings-note-" ++ page}],
    [D.Dom.div("settings-category-icon", [W.WIcon.svg(Menu.icon(page), 22)]),
     D.Dom.div("settings-category-copy", [D.El{"span", "", [D.At{"class", "settings-category-title"}, D.At{"id", "settings-label-" ++ page}], [D.Txt{U.Settings.title(lc, page)}]},
       D.El{"span", "", [D.At{"class", "settings-category-note"}, D.At{"id", "settings-note-" ++ page}], [D.Txt{Bool.pick(String, String.is_empty(hint), Menu.note(lc, page), hint)}]}]),
     W.WIcon.svg("chevron-right", 16)]}

def Menu.cards(ps: List<&2, String>, +lc: I.Loc, +q: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}: Nil{}
    case +p <> tail: Menu.card(lc, p, U.Settings.hint(lc, p, q)) <> Menu.cards(tail, lc, q)

def Menu.group.title(i: Nat, +lc: I.Loc) -> String:
  match i:
    case 0n: Menu.text(lc, "Your workspace", "ワークスペース")
    case 1n: Menu.text(lc, "Agents", "エージェント")
    case 2n: Menu.text(lc, "Hardware", "ハードウェア")
    case _: Menu.text(lc, "Machine & data", "マシンとデータ")

def Menu.group.pages(i: Nat) -> List<&2, String>:
  match i:
    case 0n: ["appearance", "threads", "timezone"]
    case 1n: ["agents", "writing", "voice", "antigravity", "keys"]
    case 2n: ["cad", "ordering"]
    case _: ["network", "usage", "privacy", "about"]

def Menu.group(+i: Nat, +lc: I.Loc) -> D.Node:
  D.El{"section", "settings-group-" ++ Nat.show(i), [D.At{"class", "settings-group"}, D.At{"aria-labelledby", "settings-group-" ++ Nat.show(i)}],
    [D.El{"h3", "", [D.At{"id", "settings-group-" ++ Nat.show(i)}], [D.Txt{Menu.group.title(i, lc)}]},
     D.Dom.div("settings-grid", Menu.cards(Menu.group.pages(i), lc, ""))]}

def Menu.results(none: Bool, ps: List<&2, String>, +lc: I.Loc, +q: String) -> List<&2, D.Node>:
  match none:
    case True{}: [D.Dom.el("p", [D.At{"class", "set-about"}, D.At{"role", "status"}], [D.Txt{MSG.T.settings.search.none(lc)}])]
    case False{}: [D.Dom.div("settings-grid", Menu.cards(ps, lc, q))]

# the groups in one block, which a wide page flows into columns (style.css)
def Menu.body(searching: Bool, +lc: I.Loc, +q: String) -> List<&2, D.Node>:
  match searching:
    case False{}: [D.Dom.div("settings-groups", [Menu.group(0n, lc), Menu.group(1n, lc), Menu.group(2n, lc), Menu.group(3n, lc)])]
    case True{}:
      +ps = U.Settings.found(U.Settings.menu.pages(), lc, q)
      Menu.results(List.is_empty(&2, String, ps), ps, lc, q)

# beside an open page, on a wide page only (style.css): every category under
# its group's name, the open one marked; a click opens it as its card does
def Menu.side.item(+lc: I.Loc, +cur: String, +page: String) -> D.Node:
  D.El{"button", "settings-side-" ++ page, [D.At{"class", Bool.pick(String, String.eq(page, cur), "settings-side-item on", "settings-side-item")}, D.At{"data-value", page},
    D.On{"click", "settings-page"}, D.At{"aria-current", Bool.pick(String, String.eq(page, cur), "page", "false")}],
    [W.WIcon.svg(Menu.icon(page), 16), D.Dom.span("", U.Settings.title(lc, page))]}

def Menu.side.items(ps: List<&2, String>, +lc: I.Loc, +cur: String) -> List<&2, D.Node>:
  match ps:
    case Nil{}: Nil{}
    case +p <> tail: Menu.side.item(lc, cur, p) <> Menu.side.items(tail, lc, cur)

def Menu.side.group(+i: Nat, +lc: I.Loc, +cur: String) -> List<&2, D.Node>:
  D.Dom.span("settings-side-h", Menu.group.title(i, lc)) <> Menu.side.items(Menu.group.pages(i), lc, cur)

def Menu.side(+lc: I.Loc, +cur: String) -> D.Node:
  D.El{"nav", "settings-side", [D.At{"class", "settings-side"}, D.At{"aria-label", Menu.text(lc, "Categories", "カテゴリ")}],
    List.concat(&2, D.Node, [Menu.side.group(0n, lc, cur), Menu.side.group(1n, lc, cur), Menu.side.group(2n, lc, cur), Menu.side.group(3n, lc, cur)])}

def Menu.context(+lc: I.Loc, +page: String) -> List<&2, D.Node>:
  [D.Dom.div("settings-context", [W.WIcon.svg(Menu.icon(page), 20), D.Dom.el("p", Nil{}, [D.Txt{Menu.note(lc, page)}])])]

def Menu.root(+lc: I.Loc, +u: U.Ui) -> List<&2, D.Node>:
  +q = U.Settings.q(u)
  D.Dom.el("p", [D.At{"class", "settings-intro"}], [D.Txt{Menu.text(lc, "Choose a category, or search for a setting.", "カテゴリを選ぶか、設定を検索してください。") }]) <>
  D.El{"input", "settings-find", [D.At{"id", "settings-find"}, D.At{"class", "field set-find"}, D.At{"placeholder", MSG.T.settings.search(lc)}, D.At{"aria-label", MSG.T.settings.search(lc)}, D.At{"autocomplete", "off"},
    D.At{"spellcheck", "false"}, D.At{"value", U.Ui.scratch(u, "@f.setq")}, D.At{"data-bfield", "setq"}, D.On{"input", "bfield"}, D.At{"data-enter", "settings-page"}, D.At{"data-value", "?"}], Nil{}}
    <> Menu.body(Bool.not(String.is_empty(q)), lc, q)
