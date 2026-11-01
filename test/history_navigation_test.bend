import Base
import ./check.bend as C
import ./hist_test.bend as T
import ../src/core/client.bend as U
import ../src/core/hist.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def resolved(+ref: String) -> J.Json:
  J.Json.obj([J.KV{"thread", J.Str{"t"}}, J.KV{"side", J.Str{"a"}}, J.KV{"ref", J.Str{ref}},
    J.KV{"commit", J.Str{"c9"}}, J.KV{"git", J.Str{"/r/.git"}}, J.KV{"top", J.Str{"/r"}}])

def other() -> M.Change:
  M.ActivityLogged{"t", "other-step", "step", "kicad", H.Hist.record("other\nc4\n/r/.git\n/r\n" ++ T.z("M") ++ T.z("other.kicad_pcb"), "a2"), 10n}

def multi() -> U.Ui:
  U.Ui.view_put(T.act(U.Ui.adopt(U.Ui.init(0n), M.State.replay([other()], T.state())), "select", "t"), "board", "/r/b.kicad_pcb")

def main() -> IO(Unit):
  +played = T.act(T.base(), "hist-play", "")
  +picked = T.act(played, "hist-go", "step:c2")
  +sheet = U.Ui.view_put(T.act(T.sheet(), "hist-first", ""), "schematic", H.Hist.git("/r/.git", "c1", "/r", "child.kicad_sch"))
  +cmp = T.act(T.base(), "cmp-pick", "a\tc1\tStep 1\t/r/.git\t/r")
  +pending = T.act(T.base(), "rv-ref", "a\tv1")
  +board_return = U.Ui.view_put(T.act(pending, "board-pick", "/r/other.kicad_pcb"), "board", "/r/b.kicad_pcb")
  +tab_return = U.Ui.view_put(T.act(pending, "view-kind", "schematic"), "board", "/r/b.kicad_pcb")
  +sheet_return = U.Ui.view_put(U.Sch.put(False{}, pending, "/r/child.kicad_sch"), "board", "/r/b.kicad_pcb")
  +files_return = U.Ui.view_put(T.act(pending, "fm", ""), "board", "/r/b.kicad_pcb")
  +thread_return = T.act(T.act(pending, "select", "other"), "select", "t")
  +unresolved = U.Ui.info_set(U.Ui.view_put(T.base(), "board", ""), "view.design:t", "")
  do IO<Unit>:
    C.Check.str("a loaded source message can be revealed", T.yes(U.Hs.jump.available(T.base(), "a1")), "yes")
    C.Check.str("an off-page label is not a scroll target", T.yes(U.Hs.jump.available(U.Ui.adopt.raw(T.base(), H.Hist.window(T.state(), "t", M.State.empty())), "a1")), "no")
    C.Check.str("an absent source has no jump", T.yes(U.Hs.jump.available(T.base(), "")), "no")
    C.Check.str("a cancelled ref stays cancelled after returning to its board", T.path(U.Rv.reply.k(resolved("v1"), board_return)), "/r/b.kicad_pcb")
    C.Check.str("a tab change cancels a ref even after returning", T.path(U.Rv.reply.k(resolved("v1"), tab_return)), "/r/b.kicad_pcb")
    C.Check.str("a sheet change cancels a ref even after returning", T.path(U.Rv.reply.k(resolved("v1"), sheet_return)), "/r/b.kicad_pcb")
    C.Check.str("Files cancels a ref even after returning", T.path(U.Rv.reply.k(resolved("v1"), files_return)), "/r/b.kicad_pcb")
    C.Check.str("a thread change cancels a ref even after returning", T.path(U.Rv.reply.k(resolved("v1"), thread_return)), "/r/b.kicad_pcb")
    C.Check.str("a new version picker cancels the old typed ref", U.Ui.scratch(U.Rv.reply.k(resolved("v1"), T.act(pending, "cmp", "a")), "@cmp.menu"), "a")
    C.Check.str("picking a board pauses playback immediately", T.yes(U.Hs.playing(T.act(played, "board-pick", "/r/b.kicad_pcb"))), "no")
    C.Check.str("picking a board exits its old comparison", T.yes(U.Hs.cmp(T.act(T.act(cmp, "rv-as", "new"), "board-pick", "/r/b.kicad_pcb"))), "no")
    C.Check.str("another board's edits stay out of this board's history", T.Show.ids(U.Hs.track(multi())), "step:c1 step:c2 step:c4 ")
    C.Check.str("last stays on the selected board", T.path(T.act(multi(), "hist-last", "")), "git\t/r/.git\tc4\t2\t/r/b.kicad_pcb")
    C.Check.str("a board with no recorded edits has no track", T.Show.ids(U.Hs.track(U.Ui.view_put(multi(), "board", "/r/new.kicad_pcb"))), "")
    C.Check.str("an unknown file retains the kind's track", T.Show.ids(H.Hist.track.file(H.Hist.steps(T.state(), "t"), 0n, "")), "step:c1 step:c2 step:c4 ")
    C.Check.str("an unresolved default never scrubs into an arbitrary board", T.path(T.act(unresolved, "hist-first", "")) ++ "|" ++ T.Show.ids(U.Hs.track(unresolved)), "|")
    C.Check.str("a child sheet keeps the selected snapshot on the track", T.Show.ids(U.Hs.track(sheet)), "step:c1 ")
    C.Check.str("a child sheet in a snapshot is never labelled Live", Nat.show(U.Hs.at(sheet, U.Hs.track(sheet))), "1")
    C.Check.str("highlighting keeps an unchanged child sheet", H.Hist.family(T.path(T.act(sheet, "hist-diff", ""))), "/r/child.kicad_sch")
    C.Check.str("Live returns to the sheet on screen", T.path(T.act(sheet, "hist-live", "")), "/r/child.kicad_sch")
    C.Check.str("reload preserves a snapshot's file", U.Viewer.live(T.path(sheet)), "/r/child.kicad_sch")
    C.Check.str("reload preserves a comparison's file", U.Viewer.live(T.path(cmp)), "/r/b.kicad_pcb")
    C.Check.str("scrubbing pauses playback", T.yes(U.Hs.playing(picked)), "no")
    C.Check.str("a pending tick cannot replace a chosen snapshot", T.path(T.act(picked, "hist-tick", "t\t1")), T.path(picked))
    C.Check.str("manual Next pauses playback", T.yes(U.Hs.playing(T.act(played, "hist-next", ""))), "no")
    C.Check.str("manual First pauses playback", T.yes(U.Hs.playing(T.act(played, "hist-first", ""))), "no")
    C.Check.str("manual Last pauses playback", T.yes(U.Hs.playing(T.act(played, "hist-last", ""))), "no")
    C.Check.str("the compare picker pauses playback", T.yes(U.Hs.playing(T.act(played, "cmp", ""))), "no")
    C.Check.str("Live closes a comparison's version menu", U.Ui.scratch(T.act(T.act(cmp, "cmp-side", "a"), "hist-live", ""), "@cmp.menu"), "")
    C.Check.str("another board never restores the previous board's comparison", U.Rv.back(U.Ui.view_put(cmp, "board", "/r/other.kicad_pcb"), "board"), "")
    C.Check.str("another board is live even if the previous comparison showed New", T.yes(U.Hs.cmp(U.Ui.view_put(T.act(cmp, "rv-as", "new"), "board", "/r/other.kicad_pcb"))), "no")
    C.Check.str("Review can still return to the same comparison", U.Rv.back(U.Ui.view_put(cmp, "review", "bom"), "board"), T.path(cmp))
    C.Check.str("Highlight edits from Live opens the last snapshot", H.Hist.family(T.path(T.act(T.base(), "hist-diff", ""))) ++ "|" ++ Nat.show(U.Hs.at(T.act(T.base(), "hist-diff", ""), U.Hs.track(T.act(T.base(), "hist-diff", "")))), "/r/b.kicad_pcb|3")
    C.Check.str("Highlight edits works again after returning to Live", T.yes(U.Hs.diff(T.act(T.act(T.act(T.base(), "hist-diff", ""), "hist-live", ""), "hist-diff", ""))) ++ "|" ++ Nat.show(U.Hs.at(T.act(T.act(T.act(T.base(), "hist-diff", ""), "hist-live", ""), "hist-diff", ""), U.Hs.track(T.base()))), "yes|3")
    C.Check.str("switching threads closes the version picker", U.Ui.scratch(T.act(T.act(played, "cmp", ""), "select", "other"), "@cmp.menu"), "")
    C.Check.str("switching threads pauses before a pending tick arrives", U.Ui.info(T.act(played, "select", "other"), "hist.play:t"), "")
    C.Check.str("switching tabs closes the version picker", U.Ui.scratch(T.act(T.act(played, "cmp", ""), "view-kind", "schematic"), "@cmp.menu"), "")
    C.Check.str("switching tabs pauses playback", T.yes(U.Hs.playing(T.act(played, "view-kind", "schematic"))), "no")
    C.Check.str("a current ref lookup applies", T.path(U.Rv.reply.k(resolved("v1"), pending)), "diff\ngit\t/r/.git\tc9\t2\t/r/b.kicad_pcb\n/r/b.kicad_pcb")
    C.Check.str("a late ref lookup leaves another board alone", T.path(U.Rv.reply.k(resolved("v1"), U.Ui.view_put(pending, "board", "/r/other.kicad_pcb"))), "/r/other.kicad_pcb")
    C.Check.str("an older lookup cannot replace a newer ref", T.path(U.Rv.reply.k(resolved("v1"), T.act(pending, "rv-ref", "a\tv2"))), "/r/b.kicad_pcb")
    C.Check.str("returning to Live cancels a ref lookup", T.path(U.Rv.reply.k(resolved("v1"), T.act(pending, "hist-live", ""))), "/r/b.kicad_pcb")
    C.Check.str("choosing a snapshot cancels a ref lookup", T.path(U.Rv.reply.k(resolved("v1"), T.act(pending, "hist-first", ""))), "git\t/r/.git\tc1\t2\t/r/b.kicad_pcb")
