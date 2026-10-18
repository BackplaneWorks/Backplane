import Base
import ./check.bend as C
import ../src/core/i18n.bend as I
import ../src/core/msgs.bend as MSG
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/app/layout.bend as L
import ../src/app/nui.bend as N
import ../src/app/bots.bend as BV
import ../src/app/forms.bend as F
import ../src/app/viewer.bend as V
import ../src/core/winnote.bend as NT
import ../src/core/tools.bend as TO
import ../src/gfx/text.bend as X
import ../src/gfx/raster.bend as R
import ../src/gfx/draw.bend as D

# The uncached test face records every drawn code point as a glyph request.
# This exercises the actual layout's texts, without fonts or an X server.
def face.of(f: X.Face) -> X.Face:
  match f:
    case X.Face{_, _, px, asc, desc, lh, miss, name, tab}:
      X.Face{1, True{}, px, asc, desc, lh, miss, name, tab}

def ty(+l: I.Loc) -> L.Ty:
  +f = face.of(X.Text.bitmap())
  L.Ty{Map.new(&2, X.TG), f, f, f, f, f, l}

def ja() -> U.Ui:
  U.Lang.apply(U.Ui.scratch_set(U.Ui.init(100n), "@lang.preview", "1"), "ja", "ja-JP")

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Board", "/tmp/board", 1n},
    M.ThreadCreated{"t", "p", "A person's title", M.Local{}, "", "", "missing-provider", "", 2n},
    M.MessagePosted{"t", "m", M.User{}, "Leave this message alone", 3n}], M.State.empty())

def ui.of(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{_, _, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      U.Ui{state(), "t", dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def text.ops(os: List<&2, R.Op>, acc: String) -> String:
  match os:
    case Nil{}: acc
    case R.Op{4294967295, _, _, _, cp, 0, _} <> t:
      text.ops(t, SCon{Chr{cp}, acc})
    case _ <> t: text.ops(t, acc)

def text(o: L.Out) -> String:
  match o:
    case L.Out{os, _}: text.ops(os, "")

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}: hs

def has(+s: String, v: String) -> String:
  Bool.show(String.contains(s, v))

def captions(hs: List<&2, N.Hit>) -> String:
  match hs:
    case Nil{}: ""
    case N.Hit{_, _, _, _, caption, _} <> t: caption ++ "|" ++ captions(t)

def right.go(hs: List<&2, N.Hit>, +a: String, +v: String) -> U32:
  match hs:
    case Nil{}: 0
    case N.Hit{_, _, x1, _, +action, +value} <> t:
      Bool.pick(U32, Bool.and(String.eq(a, action), String.eq(v, value)), x1, right.go(t, a, v))

def busy(v: V.Vw) -> String:
  match v:
    case V.Vw{_, _, _, _, _, _, _, _, _, _, V.Vx{_, _, _, _, _, b, _, _, _, _, _}}:
      b

def heads() -> TO.Head:
  TO.Head{True{}, True{}, True{}, True{}, True{}, True{}, True{}, True{}, True{}}

def captions.tools(ts: List<&2, TO.Tool>) -> String:
  match ts:
    case Nil{}: ""
    case TO.Tool{_, l, _, _, _, _, _, _} <> t: l ++ "|" ++ captions.tools(t)

def settings() -> L.Out:
  L.Lay.settings(ty(I.Ja{}), state(), N.Nui.init(ui.of(ja()), 1400, 10000), Map.new(&2, String), "test", "", 264, 1400, 10000, 0)

def thread() -> L.Out:
  +font = ty(I.Ja{})
  +n = N.Nui.init(ui.of(ja()), 1400, 900)
  L.Lay.thread(font, L.Lay.memo(font, n, L.TL.memo.none()), n, 264, 1400)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("settings en", MSG.T.settings.restart(I.En{}), "Continue after a restart")
    C.Check.str("settings ja", MSG.T.settings.restart(I.Ja{}), "再起動後に続行")
    C.Check.str("sidebar en", MSG.T.sidebar.snoozed(I.En{}), "Snoozed")
    C.Check.str("sidebar ja", MSG.T.sidebar.snoozed(I.Ja{}), "スヌーズ中")
    C.Check.str("thread chrome en", MSG.T.thread.subagents(I.En{}), "Subagents")
    C.Check.str("thread chrome ja", MSG.T.thread.subagents(I.Ja{}), "サブエージェント")
    C.Check.str("composer en", MSG.T.composer.placeholder(I.En{}), "Ask anything: @bot, >thread, %project, #room, $skill, /btw")
    C.Check.str("composer ja", MSG.T.composer.placeholder(I.Ja{}), "何でも質問：@bot、>thread、%project、#room、$skill、/btw")
    C.Check.str("menu en", text(L.Lay.button(ty(I.En{}), D.Clip.all(), 0, 0, MSG.T.common.cancel(I.En{}), 0, "cancel", "")), "Cancel")
    C.Check.str("menu ja", text(L.Lay.button(ty(I.Ja{}), D.Clip.all(), 0, 0, MSG.T.common.cancel(I.Ja{}), 0, "cancel", "")), MSG.T.common.cancel(I.Ja{}))
    C.Check.str("approval menu en", captions(L.Ask.choices.loc(I.En{}, False{}, "approval", "a", "", "")), "Allow|Allow for session|Deny|")
    C.Check.str("approval menu ja", captions(L.Ask.choices.loc(I.Ja{}, False{}, "approval", "a", "", "")), MSG.T.approval.allow(I.Ja{}) ++ "|" ++ MSG.T.approval.session(I.Ja{}) ++ "|" ++ MSG.T.approval.deny(I.Ja{}) ++ "|")
    C.Check.str("bots en", BV.Bv.onoff(True{}), "On")
    C.Check.str("bots ja", BV.Bv.onoff.loc(I.Ja{}, True{}), MSG.T.settings.lang.on(I.Ja{}))
    C.Check.str("forms en", text(F.Fm.field(ty(I.En{}), F.Fm.fs(N.Nui.init(U.Ui.init(0n), 800, 600)), D.Clip.all(), 0, 0, 400, "name", "save", "", MSG.T.bots.room.name(I.En{}))), "Room name")
    C.Check.str("forms ja", text(F.Fm.field(ty(I.Ja{}), F.Fm.fs(N.Nui.init(ja(), 800, 600)), D.Clip.all(), 0, 0, 400, "name", "save", "", MSG.T.bots.room.name(I.Ja{}))), MSG.T.bots.room.name(I.Ja{}))
    C.Check.str("viewer en", L.Lay.notice(I.En{}, NT.Notice.make("noboard", "")), "No board in this project yet")
    C.Check.str("viewer ja", L.Lay.notice(I.Ja{}, NT.Notice.make("noboard", "")), MSG.T.viewer.empty.board(I.Ja{}))
    C.Check.str("viewer content retained", L.Lay.notice(I.Ja{}, "a tool's own diagnostic"), "a tool's own diagnostic")
    C.Check.str("viewer error en", L.Lay.notice(I.En{}, NT.Notice.make("export", "diagnostic")), "3D export failed: diagnostic")
    C.Check.str("viewer error ja", L.Lay.notice(I.Ja{}, NT.Notice.make("export", "diagnostic")), MSG.T.viewer.export.failed(I.Ja{}, "diagnostic"))
    C.Check.str("asked en", L.Lay.notice(I.En{}, busy(N.Vw.asked(N.Nui.vw(N.Nui.init(ja(), 800, 600)), "k"))), "Loading…")
    C.Check.str("asked ja", L.Lay.notice(I.Ja{}, busy(N.Vw.asked(N.Nui.vw(N.Nui.init(ja(), 800, 600)), "k"))), "読み込み中…")
    C.Check.str("link reason en", L.Lay.notice(I.En{}, NT.Notice.make("nohost", "box.ts.net")), "unreachable: cannot resolve or reach box.ts.net")
    C.Check.str("link reason ja", L.Lay.notice(I.Ja{}, NT.Notice.make("nohost", "box.ts.net")), MSG.T.remote.unreachable(I.Ja{}, "box.ts.net"))
    C.Check.str("remote refusal en", L.Lay.notice(I.En{}, NT.Notice.make("refused", "500 oops")), "that machine did not take it: 500 oops")
    C.Check.str("remote away en", L.Lay.notice(I.En{}, NT.Notice.make("away", "")), "that machine is away")
    C.Check.str("footer update en", L.Lay.note.shown(I.En{}, "update 1.2.3"), "update 1.2.3")
    C.Check.str("footer update ja", L.Lay.note.shown(I.Ja{}, "update 1.2.3"), MSG.T.sidebar.update(I.Ja{}, "1.2.3"))
    C.Check.str("footer hub words kept", L.Lay.note.shown(I.Ja{}, "installing"), "installing")
    C.Check.str("header en", captions.tools(TO.Tools.head(heads(), "", "", "", "")), "Stop|Pinned|Settled|Snoozed|Archived|Delete thread|Cancel|Fork thread|Bring back|Terminal|Viewer|")
    C.Check.str("header en wrapper", captions.tools(TO.Tools.head.loc(I.En{}, heads(), "", "", "", "")), captions.tools(TO.Tools.head(heads(), "", "", "", "")))
    C.Check.str("header ja", captions.tools(TO.Tools.head.loc(I.Ja{}, heads(), "", "", "", "")), MSG.T.composer.stop(I.Ja{}) ++ "|" ++ MSG.T.tools.pinned(I.Ja{}) ++ "|" ++ MSG.T.notify.settled(I.Ja{}) ++ "|" ++ MSG.T.sidebar.snoozed(I.Ja{}) ++ "|" ++ MSG.T.sidebar.archived(I.Ja{}) ++ "|" ++ MSG.T.palette.delete(I.Ja{}) ++ "|" ++ MSG.T.common.cancel(I.Ja{}) ++ "|" ++ MSG.T.tools.forkthread(I.Ja{}) ++ "|" ++ MSG.T.menu.bringback(I.Ja{}) ++ "|" ++ MSG.T.palette.terminal(I.Ja{}) ++ "|" ++ MSG.T.thread.viewer(I.Ja{}) ++ "|")
    C.Check.str("inspector field en", L.Info.key(I.En{}, "footprint"), "footprint")
    C.Check.str("inspector field ja", L.Info.key(I.Ja{}, "footprint"), "フットプリント")
    C.Check.str("inspector unknown field kept", L.Info.key(I.Ja{}, "uuid"), "uuid")
    C.Check.str("inspector pad en", L.Info.title(I.En{}, "pad 3"), "pad 3")
    C.Check.str("inspector pad ja", L.Info.title(I.Ja{}, "pad 3"), "パッド3")
    C.Check.str("inspector kind ja", L.Info.title(I.Ja{}, "global label"), "グローバルラベル")
    C.Check.str("settings pair width en", U32.show(L.Set.pairw(ty(I.En{}), MSG.T.settings.theme.light(I.En{}), MSG.T.settings.theme.dark(I.En{}), 150)), "150")
    C.Check.str("files range en", MSG.T.viewer.files.range(I.En{}, "1", "20", "1234"), "1–20 of 1234")
    C.Check.str("bots routine en", MSG.T.routine.run(I.En{}), "Run now")
    C.Check.str("bots routine ja", MSG.T.routine.run(I.Ja{}), "今すぐ実行")
    C.Check.str("bots hook en", MSG.T.hooks.status(I.En{}, "h1", "called", "3", I.I18n.ago(I.En{}, 400n, 100n)), "POST /hook/h1 · 3 calls, last 5m ago")
    C.Check.str("bots hook ja", MSG.T.hooks.status(I.Ja{}, "h1", "called", "3", I.I18n.ago(I.Ja{}, 400n, 100n)), "POST /hook/h1・3回、最終：5分前")
    C.Check.str("bots memory en", MSG.T.bots.memory.count(I.En{}, "1234"), "1234 items. The ones about you and your preferences go into every turn.")
    C.Check.str("picker en default", L.Picker.name(""), "default")
    C.Check.str("picker ja default", L.Picker.name.loc(I.Ja{}, ""), MSG.T.common.default(I.Ja{}))
    C.Check.str("picker model retained", L.Picker.name.loc(I.Ja{}, "claude-opus-5"), "claude-opus-5")
    C.Check.str("pane en", L.Pane.range(3n, 5n, 1234n), " · 4–8 of 1234")
    C.Check.str("pane ja", L.Pane.range.loc(I.Ja{}, 3n, 5n, 1234n), MSG.T.pane.range(I.Ja{}, "4", "8", "1234"))
    C.Check.str("ungrouped count en", MSG.T.settings.settle.days(I.En{}, "1234", "other"), "1234 days")
    C.Check.str("ungrouped count ja", MSG.T.settings.settle.days(I.Ja{}, "1234", "other"), "1234日")
    C.Check.str("voice en", L.Vx.head("live", "mic"), "Listening · mic")
    C.Check.str("voice ja", L.Vx.head.loc(I.Ja{}, "live", "mic"), MSG.T.voice.listening(I.Ja{}, "named", "mic"))
    C.Check.str("provider en", L.Set.cli("", "codex"), "codex is not installed")
    C.Check.str("provider ja", L.Set.cli.loc(I.Ja{}, "", "codex"), MSG.T.settings.provider.missing(I.Ja{}, "codex"))
    C.Check.str("settings draws Japanese", has(text(settings()), MSG.T.settings.voice(I.Ja{})), "True")
    C.Check.str("settings draws thread card", has(text(settings()), MSG.T.settings.settle(I.Ja{})), "True")
    C.Check.str("settings hit retained", Bool.show(U32.is_gt(right.go(hits(settings()), "setting", "restart.continue=off"), 0)), "True")
    C.Check.str("right aligned Japanese chip", U32.show(right.go(hits(L.Set.carry(ty(I.Ja{}), True{}, 300, 700, 100)), "setting", "restart.continue=off")), "984")
    C.Check.str("IPC chips right aligned", U32.show(right.go(hits(L.Set.ipc(ty(I.Ja{}), "on", 300, 700, 100)), "setting", "kicad.ipc=off")), "984")
    C.Check.str("thread title retained", has(text(thread()), "A person's title"), "True")
    C.Check.str("thread draws Japanese composer", has(text(thread()), "何でも質問"), "True")
    C.Check.str("thread retains message", has(text(thread()), "Leave this message alone"), "True")
    C.Check.str("thread header translated", has(text(thread()), MSG.T.thread.viewer(I.Ja{})), "True")
    C.Check.str("thread send hit", Bool.show(U32.is_gt(right.go(hits(thread()), "send", ""), 0)), "True")
