import Base
import ./check.bend as C
import ../src/core/i18n.bend as I
import ../src/core/msgs.bend as MSG
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/onboard.bend as OB
import ../src/core/plane.bend as PL
import ../src/core/sched.bend as S
import ../src/core/fold.bend as F
import ../src/core/botui.bend as BU
import ../src/core/title.bend as TTL
import ../src/core/stamp.bend as STP
import ../src/core/cron.bend as CR
import ../src/core/tz.bend as TZ

# Spec 4.1: the shared core strings. English is byte-identical to the old
# literals (copied here); Japanese is the catalog's.

def ja() -> U.Ui:
  U.Lang.apply(U.Ui.scratch_set(U.Ui.init(0n), "@lang.preview", "1"), "ja", "ja-JP")

def label(p: U.PItem) -> String:
  match p:
    case U.PItem{l, _, _}: l

def labels(ps: List<&2, U.PItem>) -> String:
  match ps:
    case Nil{}: ""
    case p <> t: label(p) ++ "|" ++ labels(t)

def first(ps: List<&2, U.PItem>) -> String:
  match ps:
    case Nil{}: "-"
    case p <> _: label(p)

def mlabel(m: U.MItem) -> String:
  match m:
    case U.MItem{l, _, _, _}: l

def mlabels(ms: List<&2, U.MItem>) -> String:
  match ms:
    case Nil{}: ""
    case m <> t: mlabel(m) ++ "|" ++ mlabels(t)

def state() -> M.State:
  M.State.replay([
    M.ProjectCreated{"p1-1", "Project", "/p", 1n},
    M.ThreadCreated{"t1-1", "p1-1", "First", M.Local{}, "", "", "claude", "", 2n}
  ], M.State.empty())

def nf(k: String, v: String) -> Map<&2, String>:
  Map.set(&2, String, Map.new(&2, String), k, v)

def blabel(b: U.TBtn) -> String:
  match b:
    case U.TBtn{l, _, _, _}: l

def blabels(bs: List<&2, U.TBtn>) -> String:
  match bs:
    case Nil{}: ""
    case b <> t: blabel(b) ++ "|" ++ blabels(t)

def qlabels(bs: List<&2, U.QBtn>) -> String:
  match bs:
    case Nil{}: ""
    case U.QBtn{l, _, _} <> t: l ++ "|" ++ qlabels(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("title en new", U.Title.shown(I.En{}, "New thread"), "New thread")
    C.Check.str("title ja new", U.Title.shown(I.Ja{}, "New thread"), "新しいスレッド")
    C.Check.str("title other", U.Title.shown(I.Ja{}, "x"), "x")
    C.Check.str("title module", TTL.Title.shown(I.Ja{}, "New thread") ++ "|" ++ TTL.Title.shown(I.En{}, "New thread"), "新しいスレッド|New thread")
    C.Check.str("a project named New thread keeps its name", U.Label.title(state(), "p1-1", "New thread"), "New thread")
    C.Check.str("palette en first", first(U.Palette.cmds(U.Ui.init(0n))), "New thread")
    C.Check.str("palette ja first", first(U.Palette.cmds(ja())), "新しいスレッド")
    C.Check.str("palette en actions", labels(U.Palette.actions(I.En{}, "p")),
      "New thread|Diff: what this thread changed|Terminal|Attach a file|Commit & push|Open pull request|Revert thread|Settle thread|Pin thread|Archive thread|Unarchive thread|Delete thread|Snooze 1 hour|Snooze until tomorrow 9:00|Snooze 1 week|Unsnooze|Search threads|Find file|Viewer: board, schematic, 3D, Mech, files, diff, browser|Open in KiCad|Open in KiCad 10 (exported copy)|Settings|Update Backplane|")
    C.Check.str("palette ja filter", first(U.Palette.filter(U.Palette.actions(I.Ja{}, "p"), "ターミナル")), "ターミナル")
    C.Check.str("row menu en", mlabels(U.Menu.thread(I.En{}, state(), 10n, "t1-1", False{})),
      "Pin|Rename|Settle|Snooze 1 hour|Snooze until tomorrow|Snooze 1 week|Archive|Delete|Copy link|")
    C.Check.str("row menu ja", mlabels(U.Menu.thread(I.Ja{}, state(), 10n, "t1-1", False{})),
      "ピン留め|名前を変更|完了|1時間スヌーズ|明日までスヌーズ|1週間スヌーズ|アーカイブ|削除|リンクをコピー|")
    C.Check.str("row menu armed en", mlabels(U.Menu.thread(I.En{}, state(), 10n, "t1-1", True{})),
      "Pin|Rename|Settle|Snooze 1 hour|Snooze until tomorrow|Snooze 1 week|Archive|Click again to delete|Copy link|")
    C.Check.str("project menu en", mlabels(U.Menu.project(I.En{}, "p1-1", False{}, True{})), "New thread|Colour and icon|Remove project|Copy link|")
    C.Check.str("project menu ja", mlabels(U.Menu.project(I.Ja{}, "p1-1", True{}, True{})), "新しいスレッド|色とアイコン|もう一度クリックで削除|リンクをコピー|")
    C.Check.str("file menu en", mlabels(U.Menu.file("a")), "Open|Show in folder|Copy path|")
    C.Check.str("update note en", U.Update.line(I.En{}, Map.new(&2, String), "update 1.2.3"), "update 1.2.3")
    C.Check.str("update note ja", U.Update.line(I.Ja{}, Map.new(&2, String), "update 1.2.3"), "更新可能：1.2.3")
    C.Check.str("update current en", U.Update.line(I.En{}, Map.new(&2, String), ""), "Up to date, checked hourly.")
    C.Check.str("update current ja", U.Update.line(I.Ja{}, Map.new(&2, String), ""), "最新です。1時間ごとに確認します。")
    C.Check.str("update progress stays", U.Update.line(I.Ja{}, Map.new(&2, String), "downloading"), "downloading")
    C.Check.str("update waits en", U.Update.waits.line(I.En{}, 1n, Map.new(&2, String), ""), "Update waits for 1 thread to finish their work.")
    C.Check.str("update waits en many", U.Update.waits.line(I.En{}, 3n, Map.new(&2, String), ""), "Update waits for 3 threads to finish their work.")
    C.Check.str("update waits ja", U.Update.waits.line(I.Ja{}, 3n, Map.new(&2, String), ""), "スレッド3件の作業が終わるまで、更新を待っています。")
    C.Check.str("tz source en", U.Tzs.source(I.En{}, "phone") ++ "|" ++ U.Tzs.source(I.En{}, "") , "from your phone|nothing reported yet")
    C.Check.str("tz source ja", U.Tzs.source(I.Ja{}, "machine"), "このマシン")
    C.Check.str("tailscale en", U.Tail.label(I.En{}, nf("tailscale", "unavailable")) ++ "|" ++ U.Tail.note(I.En{}, nf("tailscale", "looking")), "Tailscale: not connected|Looking for Tailscale.")
    C.Check.str("tailscale ja", U.Tail.label(I.Ja{}, nf("tailscale", "unavailable")), "Tailscale：未接続")
    C.Check.str("kicad missing", U.KiCad.cli.label(Map.new(&2, String)) ++ "|" ++ U.KiCad.cli.label.loc(I.Ja{}, Map.new(&2, String)), "KiCad CLI: not found|KiCad CLI：見つかりません")
    C.Check.str("composer en", U.Follow.label(I.En{}, state(), "t1-1", True{}) ++ "|" ++ U.Follow.label(I.En{}, state(), "t1-1", False{}) ++ "|" ++ U.Follow.button(I.En{}, state(), "t1-1", False{}, True{}, ""), "Steer|Send|Stop")
    C.Check.str("composer ja", U.Follow.button(I.Ja{}, state(), "t1-1", False{}, True{}, "") ++ "|" ++ U.Follow.button(I.Ja{}, state(), "t1-1", False{}, False{}, "/btw x"), "停止|質問")
    C.Check.str("queue en", qlabels(U.Qu.buttons(False{}, True{}, "m")), "Resume|Send|Edit|Up|Remove|")
    C.Check.str("queue tag", U.Qu.tag(I.En{}, True{}, "m") ++ "|" ++ U.Qu.tag(I.En{}, False{}, "m") ++ "|" ++ U.Qu.tag(I.En{}, True{}, "handoff:x"), "Queued|Held|Context")
    C.Check.str("queue ja", U.Qu.tag(I.Ja{}, False{}, "m"), "保留中")
    C.Check.str("ask head en", U.Ask.head("approval", "Bash", "ls") ++ "|" ++ U.Ask.head("plan", "", "") ++ "|" ++ U.Ask.head("input", "", ""), "Allow Bash?|Proposed plan|The agent asks")
    C.Check.str("ask head ja", U.Ask.head.loc(I.Ja{}, "approval", "Bash", "ls"), "Bashを許可しますか？")
    C.Check.str("btw text", U.Btw.text(I.En{}, "") ++ "|" ++ U.Btw.text(I.Ja{}, "") ++ "|" ++ U.Btw.text(I.Ja{}, "x"), "Thinking…|考え中…|x")
    C.Check.str("notify en", U.Notify.title(I.En{}) ++ "|" ++ U.Notify.lead(I.En{}), "Notify me for|Phone and desktop alerts. The rest stays in the app as unread.")
    C.Check.str("notify ja title", U.Notify.title(I.Ja{}), "通知する内容")
    C.Check.str("machines en", U.Mate.title(I.En{}) ++ "|" ++ U.Mate.note.linked(I.En{}, 2n, False{}) ++ "|" ++ U.Mate.note.linked(I.En{}, 1n, False{}),
      "Your machines|Catching up: 2 projects still loading. The rest are ready.|Catching up: 1 project still loading. The rest are ready.")
    C.Check.str("machines ja", U.Mate.title(I.Ja{}), "自分のマシン")
    C.Check.str("api keys en", U.ApiKeys.form.note(I.En{}), "Agents find a key by its name, e.g. OPENAI_API_KEY. It is never shown again.")
    C.Check.str("teach en", blabels(U.Teach.btns(I.En{}, 0n)) ++ U.Teach.title(I.En{}, 1n, "Kit"), "Teach a task|Take over|Kit is watching and learning")
    C.Check.str("teach ja", blabels(U.Teach.btns(I.Ja{}, 3n)), "引き渡す|")
    C.Check.str("teach step en", U.Teach.step.k(I.En{}, "navigate", "x.com") ++ "|" ++ U.Teach.step.k(I.En{}, "type", "Name") ++ "|" ++ U.Teach.step.k(I.En{}, "other", "Name"), "Go to x.com|Type in Name|Name")
    C.Check.str("teach step ja", U.Teach.step.k(I.Ja{}, "click", "OK"), "OKをクリック")
    C.Check.str("tour title en", OB.Ob.title(I.En{}, 0n) ++ "|" ++ OB.Ob.title(I.En{}, 9n), "Welcome to Backplane|Your uberbot")
    C.Check.str("tour title ja", OB.Ob.title(I.Ja{}, 0n), "Backplaneへようこそ")
    C.Check.str("tour body en", OB.Ob.body(I.En{}, 2n, "7", 0n), "A short tour of what is where. Skip it any time; More, Settings shows it again.")
    C.Check.str("tour body ja", OB.Ob.body(I.Ja{}, 0n, "7", 2n), "スレッドは、プロジェクトで作業するエージェントとの会話です。Gitプロジェクトでは既定で別々のワークツリーを使い、Gitがない場合やローカルモードでは、スレッドはプロジェクトのフォルダを共有します。ツールの呼び出しは1行にまとめられ、委任した作業はサブエージェントの下に表示されます。")
    C.Check.str("tour days", OB.Ob.body(I.Ja{}, 0n, "9", 1n), MSG.T.tour.body.projects.desktop(I.Ja{}, "9"))
    C.Check.str("tour buttons", OB.Ob.forward(I.En{}, 9n) ++ OB.Ob.forward(I.Ja{}, 3n) ++ OB.Ob.counter(I.En{}, 2n) ++ OB.Ob.counter(I.Ja{}, 2n), "Done次へ3 of 103／10")
    C.Check.str("tour sound", Bool.show(OB.Ob.sound()), "True")
    C.Check.str("plane counts en", PL.Plane.counts.join(PL.Plane.counts.need(I.En{}, 3n), PL.Plane.counts.run(I.En{}, 2n)), "3 need you · 2 running")
    C.Check.str("plane counts ja", PL.Plane.counts.join(PL.Plane.counts.need(I.Ja{}, 3n), PL.Plane.counts.run(I.Ja{}, 0n)), "3件が要対応")
    C.Check.str("plane home", PL.Plane.home.label(I.En{}) ++ "|" ++ PL.Plane.home.label(I.Ja{}), "Make this the home|ここをホームにする")
    C.Check.str("sched rel en", S.Sched.rel.l(I.En{}, 100n, 0n) ++ "|" ++ S.Sched.rel.l(I.En{}, 100n, 50n) ++ "|" ++ S.Sched.rel.l(I.En{}, 100n, 11800n), "|due now|in 3h 15m")
    C.Check.str("sched rel same as old", S.Sched.rel.l(I.En{}, 100n, 11800n), S.Sched.rel(100n, 11800n))
    C.Check.str("sched rel ja", S.Sched.rel.l(I.Ja{}, 100n, 11800n) ++ "|" ++ S.Sched.rel.l(I.Ja{}, 100n, 50n), "3時間15分後|実行時刻です")
    C.Check.str("sched label", S.Sched.label(I.En{}, "reminder") ++ "|" ++ S.Sched.label(I.En{}, "once") ++ "|" ++ S.Sched.label(I.En{}, "routine") ++ "|" ++ S.Sched.label(I.Ja{}, "routine"), "Reminder|Once|Repeats|繰り返し")
    C.Check.str("fold en", F.Fold.label(I.En{}, [M.Act{"a", "t", "tool", "Bash", "x", 0n}]), "1 tool call · Bash")
    C.Check.str("fold ja", F.Fold.label(I.Ja{}, [M.Act{"a", "t", "tool", "Bash", "x", 0n}, M.Act{"b", "t", "tool", "Bash", "x", 0n}]), "ツール呼び出し2件 · Bash")
    C.Check.str("work ago", F.Work.ago.loc(I.En{}, 100n, 90n) ++ "|" ++ F.Work.ago.loc(I.En{}, 400n, 100n) ++ "|" ++ F.Work.ago.loc(I.Ja{}, 100n, 90n), "just now|5m ago|たった今")
    C.Check.str("bot tab", BU.BU.tab.label(I.En{}, "routines") ++ "|" ++ BU.BU.tab.label(I.Ja{}, "routines") ++ "|" ++ BU.BU.tab.label(I.En{}, "settings"), "Routines|ルーティン|Settings")
    C.Check.str("bot mood", BU.Mood.note("wait") ++ "|" ++ BU.Mood.note.loc(I.Ja{}, "wait") ++ "|" ++ BU.Mood.note("?"), "needs you|対応待ち|away")
    C.Check.str("loc from ui", I.I18n.tag(U.Lang.loc(ja())), "ja")
    C.Check.str("history labels en", U.Hs.label.shown(I.En{}, "Step 3") ++ "|" ++ U.Hs.label.shown(I.En{}, "Working copy") ++ "|" ++ U.Hs.label.shown(I.En{}, "Turn 2") ++ "|" ++ U.Hs.label.shown(I.En{}, "main"), "Step 3|Working copy|Turn 2|main")
    C.Check.str("history labels ja", U.Hs.label.shown(I.Ja{}, "Step 3") ++ "|" ++ U.Hs.label.shown(I.Ja{}, "Working copy") ++ "|" ++ U.Hs.label.shown(I.Ja{}, "Turn 2") ++ "|" ++ U.Hs.label.shown(I.Ja{}, "Branch point · main"), "変更3|作業コピー|ターン2|分岐点 · main")
    C.Check.str("history position en", U.Hs.bar.pos(I.En{}, True{}, 1n, 4n) ++ "|" ++ U.Hs.bar.pos(I.En{}, False{}, 2n, 4n), "Step 1 of 4 · before the agent's edits|Step 2 of 4")
    C.Check.str("history live note", U.Hs.live.said(0n) ++ "|" ++ U.Hs.live.said.loc(I.Ja{}, 3n), "The live file. Compare it with any commit, branch or tag.|現在のファイルです。目盛りはエージェントの変更です。再生すると順に表示されます。")
    C.Check.str("review tabs", U.Rp.label(I.En{}, "checks") ++ "|" ++ U.Rp.label(I.En{}, "release") ++ "|" ++ U.Rp.label(I.Ja{}, "comments"), "Checks|Release|コメント")
    C.Check.str("review note en", U.RpView.note(U.RpView{"comments", Nil{}, "", "", "", True{}}) ++ "|" ++ U.RpView.note(U.RpView{"bom", Nil{}, "", "", "", False{}}), "Reading the design…|Nothing yet.")
    C.Check.str("review note ja", U.RpView.note.loc(I.Ja{}, U.RpView{"bom", Nil{}, "", "", "", True{}}), "設計を読み込み中…")
    C.Check.str("antigravity en", U.Agv.gcp.note(I.En{}, "p", "") ++ "|" ++ U.Agv.gcp.note(I.En{}, "", "") ++ "|" ++ U.Agv.google.note(I.En{}, False{}, "default"), "Project p, location not set.|Not set.|default: not signed in.")
    C.Check.str("antigravity ja", U.Agv.google.note(I.Ja{}, True{}, "work"), "work：サインイン済みです。サインイン情報はエージェントが保持し、Backplaneからは見えません。")
    C.Check.str("btw none", U.Btw.text(I.En{}, U.Btw.answer("")) ++ "|" ++ U.Btw.text(I.Ja{}, U.Btw.answer("")), "No answer came back (Settings, Writing: the model it asks).|回答がありませんでした（質問先のモデルは設定の「文章生成」で選べます）。")
    C.Check.str("cron en same", S.Sched.cron(I.En{}, "*/15 * * * *") ++ "|" ++ S.Sched.cron(I.En{}, "30 9 * * *") ++ "|" ++ S.Sched.cron(I.En{}, "0 8 * * 1-5"), "every 15 minutes|daily at 09:30|weekdays at 08:00")
    C.Check.str("cron en as before", S.Sched.cron(I.En{}, "0 */2 * * *") ++ "|" ++ S.Sched.cron(I.En{}, "5 * * * *"), CR.Cron.describe("0 */2 * * *") ++ "|" ++ CR.Cron.describe("5 * * * *"))
    C.Check.str("cron ja", S.Sched.cron(I.Ja{}, "*/15 * * * *") ++ "|" ++ S.Sched.cron(I.Ja{}, "30 9 * * *"), "15分ごと|毎日09:30")
    C.Check.str("bad time flag", Bool.show(S.Sched.hint.bad(M.State.empty(), "soon-ish", 0n, 0n, True{})) ++ Bool.show(S.Sched.hint.bad(M.State.empty(), "", 0n, 0n, True{})) ++ Bool.show(S.Sched.hint.bad(M.State.empty(), "*/5 * * * *", 0n, 0n, True{})), "TrueFalseFalse")
    C.Check.str("finished en", STP.Stamp.done.k(I.En{}, True{}, "14:05", 100n, 400n) ++ "|" ++ STP.Stamp.done.k(I.En{}, False{}, "14:05", 0n, 0n), "Finished 14:05 · 5m 0s|Finished 14:05")
    C.Check.str("finished ja", STP.Stamp.done.k(I.Ja{}, False{}, "14:05", 0n, 0n), "14:05に完了")
    C.Check.str("state words", U.StateWord.label(I.En{}, "needs you") ++ "|" ++ U.StateWord.label(I.Ja{}, "running") ++ "|" ++ U.StateWord.label(I.Ja{}, "odd"), "needs you|実行中|odd")
    C.Check.str("picker hint ja", MSG.T.picker.hint.find(I.Ja{}), "プロジェクト、またはフォルダを追加する~/パス")
    C.Check.str("schedule words ja", S.Sched.when(I.Ja{}, "0 8 * * 1-5", 0n, TZ.Tz.utc0()), "平日08:00")
