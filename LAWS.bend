import Base
import ./src/core/auth.bend as AUTH
import ./src/core/http.bend as HTTPAUTH
import ./src/core/pcb2d.bend as PCB2 # PCB viewer text
import ./src/app/side_rows.bend as SR
import ./src/app/side_row_types.bend as SRT
import ./src/server/botnet.bend as MPLOG # mail perf runtime count
# ---- native phone i18n imports ----
import ./src/mobile/app.bend as NATIVEI18N
# ---- end native phone i18n imports ----
import ./src/core/fallback.bend as FALL
import ./src/server/server.bend as MPCREDIT # mail perf
# ---- i18n web phone imports ----
import ./src/web/view.bend as WPH
# ---- end i18n web phone imports ----
import ./src/core/schematic.bend as SC # kicad fork viewers
import ./src/core/charter.bend as CH # mail 2c
import ./src/core/mailrun.bend as MRUN # mail 2c
import ./src/app/mailroom.bend as MRUI # mail ui
import ./src/server/mailstore.bend as MIX # mail index
import ./src/app/mm.bend as MM # window memos
import ./src/core/kexport.bend as KEX
import ./src/core/i18n.bend as I18
import ./test/fixtures/i18n_fallback.bend as I18F
import ./src/core/usage.bend as UZ
import ./src/core/refview.bend as RC # links (cards, trail)
import ./src/core/order.bend as ORD # ordering (project card, buy threads)
import ./src/core/mail.bend as ML # mail 1
import ./src/core/mailbody.bend as MB # mail 2a
import ./src/core/mailview.bend as MVW # mail view
import ./src/core/ref.bend as REF
# ---- i18n cjk ----
import ./src/core/cjk.bend as CJK
# ---- end i18n cjk ----
# ---- i18n ime ----
import ./src/core/ime.bend as IMEL
import ./src/app/win.bend as WINL
# ---- end i18n ime ----
import ./src/core/kihealth.bend as KH
import ./src/core/kserve.bend as KSL
import ./src/core/tz.bend as TZ
import ./src/core/when.bend as WH
import ./src/core/sched.bend as SCH
import ./src/core/exe.bend as EX
import ./src/core/locks.bend as LCK
import ./src/core/wake.bend as WAK
import ./src/core/bothub.bend as BHS
import ./src/core/bot.bend as BOT
import ./src/core/far.bend as FR
import ./src/core/fview.bend as FVW
import ./src/core/rview.bend as RVW
import ./src/core/botui.bend as BU
import ./src/core/model.bend as M
import ./src/app/viewer.bend as V
import ./src/app/theme.bend as TH
import ./src/core/semver.bend as SV
import ./src/core/clipath.bend as CPA
import ./src/app/nui.bend as N
import ./src/app/bots.bend as BV
import ./src/app/cats.bend as VCAT
import ./src/core/sexpr.bend as KSEX # kicad fork footprint placement
import ./src/core/board.bend as B
import ./src/core/hub.bend as H
import ./src/core/deploy.bend as DP
import ./src/core/ship.bend as SHP # deploy main
import ./src/core/cmds.bend as CM
import ./src/core/errtail.bend as ERT
import ./src/core/keyring.bend as KRG
import ./src/core/ride.bend as RD
import ./src/core/adopt.bend as AD
import ./src/core/utf8.bend as UT
import ./src/core/refs.bend as RF
import ./src/core/todo.bend as TD
import ./src/core/apikeys.bend as AK
import ./src/core/onboard.bend as OBD
import ./src/core/fm.bend as FMG
import ./src/core/acp.bend as AC
import ./src/core/msp.bend as MSP
import ./src/core/mcp.bend as MC
import ./src/core/fmcp.bend as FMC
import ./src/core/handoff.bend as HD
import ./src/core/agent.bend as AG
import ./src/core/tailnet.bend as TN
import ./src/core/fuzzy.bend as FZ
import ./src/mobile/notify.bend as Nt
import ./src/core/notice.bend as NO
import ./src/core/codec.bend as C
import ./src/core/json.bend as J
import ./src/core/attach.bend as AT
import ./src/gfx/tiles.bend as T
import ./src/gfx/raster.bend as R
import ./src/gfx/draw.bend as D
import ./src/app/layout.bend as L
import ./src/core/eq.bend as Q
import ./src/core/uber.bend as UBR
import ./src/core/ubersync.bend as UBS
import ./src/core/ubop.bend as UBO
import ./src/core/plane.bend as PLN
import ./src/core/svg.bend as SVG
import ./src/core/icons.bend as ICO
import ./src/core/tools.bend as TOOL
import ./src/core/lightbox.bend as LBX
import ./src/core/links.bend as LK
import ./src/web/dom.bend as DM
import ./src/core/client.bend as U
import ./src/mobile/screen.bend as MSC
import ./src/core/gh.bend as GH
import ./src/gfx/text.bend as X
import ./src/core/fold.bend as FO
import ./src/core/subs.bend as SB
import ./src/core/http.bend as WS
import ./src/core/plot.bend as PL
import ./src/core/look.bend as LO
import ./src/core/hist.bend as HS
import ./src/mobile/view.bend as MV
import ./src/core/pcb3d.bend as P3
import ./src/gfx/mesh.bend as MS
import ./src/gfx/face.bend as FA
import ./src/core/glb.bend as G
import ./src/core/menu.bend as MN
import ./src/app/win.bend as W
import ./src/core/badge.bend as BG

import ./src/mobile/picker.bend as MP
import ./src/mobile/hubs.bend as Hb
import ./src/core/hier.bend as HI

import ./src/core/boot.bend as BT
import ./src/core/store.bend as ST
import ./src/server/logread.bend as LOGREAD
import ./src/core/sign.bend as SG
import ./src/core/ring.bend as RG
import ./src/core/cron.bend as CR
import ./src/core/space.bend as SPC
import ./src/core/google.bend as GO
import ./src/core/cat.bend as CAT
import ./src/core/perf.bend as PF
import ./src/core/prompt.bend as PM
import ./src/core/mech.bend as MCH
import ./src/core/png.bend as PNG
import ./src/core/voice.bend as VO
import ./src/core/once.bend as ON
import ./src/core/desk.bend as DK
import ./src/core/emb.bend as EMB
import ./src/core/outbox.bend as OB
import ./src/core/live.bend as LV
import ./src/core/rpcout.bend as RPO
import ./src/core/farlive.bend as FLV
import ./src/core/qr.bend as QR
import ./src/core/layers.bend as LY
import ./src/core/bom.bend as BOM
import ./src/core/cad.bend as CAD
import ./src/core/checks.bend as CHK
import ./src/core/release.bend as REL
import ./src/core/review.bend as RV
import ./src/core/slash.bend as SL
import ./src/core/agy.bend as AGY
import ./src/core/told.bend as TLD
import ./src/core/rules.bend as RLS
import ./src/core/tboard.bend as TBD
import ./src/core/task.bend as TSK
import ./src/core/teach.bend as TCH
import ./src/core/cef.bend as CEF # browser-cef
import ./src/mobile/bots.bend as MBT
import ./src/core/cli.bend as CLI # one-hub
import ./src/core/onehub.bend as OHB # one-hub
import ./src/core/standin.bend as SIN # one-hub

# Backplane's rules, stated over the real code. `bend PROOF.bend` proves each
# one. Humans own this file: an agent may add laws, never weaken or delete
# one without the user's explicit consent.

# Equality
# --------

# a string always equals itself (viewer keys and ids rely on it)
law string_eq_refl:
  for +s: String
  {String.eq(s, s) == True{} : Bool}

# Settling
# --------

# a thread settles once the window has fully passed since its last activity
law settle_at_window:
  for +last: Nat
  for +w: Nat
  {M.Settle.stale(Nat.add(last, w), last, w) == True{} : Bool}

# a thread touched just now never settles, for any non-empty window
law settle_fresh:
  for +now: Nat
  for +w: Nat
  {M.Settle.stale(now, now, 1n+w) == False{} : Bool}

# once stale, a thread stays stale as time moves forward
law settle_monotone:
  for +now: Nat
  for +last: Nat
  for +w: Nat
  for +d: Nat
  for h: {M.Settle.stale(now, last, w) == True{} : Bool}
  {M.Settle.stale(Nat.add(now, d), last, w) == True{} : Bool}

# a running turn never auto-settles
law settle_never_running:
  for +now: Nat
  for +w: Nat
  for pending: Nat
  for archived: Bool
  for o: M.Override
  {M.Settle.auto.k(M.Thread.busy(M.Running{}, pending), archived, o, M.Settle.stale(now, now, w)) == False{} : Bool}

# a thread waiting on the user (approval, question) never auto-settles
law settle_never_pending:
  for t: M.TurnState
  for +p: Nat
  for archived: Bool
  for o: M.Override
  for stale: Bool
  {M.Settle.auto.k(M.Thread.busy(t, 1n+p), archived, o, stale) == False{} : Bool}

# a thread the user brought back stays back: no auto-settle overrides them
law settle_respects_unsettle:
  for busy: Bool
  for archived: Bool
  for stale: Bool
  {M.Settle.auto.k(busy, archived, M.ForcedActive{}, stale) == False{} : Bool}

# Event sourcing
# --------------

# folding one more event equals replaying the longer log: the live read
# model and a cold replay of the database can never disagree
law replay_snoc:
  for es: List<&2, M.Change>
  for s: M.State
  for e: M.Change
  {M.State.replay(List.append(&2, M.Change, es, [e]), s) == M.State.apply(M.State.replay(es, s), e) : M.State}

# Sidebar
# -------

# every thread lands on exactly one shelf: none lost, none duplicated
law sidebar_split_count:
  for ts: List<&2, M.Thread>
  {M.Shelves.count(M.Sidebar.split(ts)) == List.length(&2, M.Thread, ts) : Nat}

# every thread lands on exactly one shelf with snoozes too: none lost,
# none duplicated
law sidebar_split_at_count:
  for ts: List<&2, M.Thread>
  for +navs: Map<&2, M.Nav>
  for +now: Nat
  {M.Shelves.count(M.Sidebar.split.at(ts, navs, now)) == List.length(&2, M.Thread, ts) : Nat}

# Snooze
# ------

# a snoozed thread never auto-settles while its snooze lasts
law settle_never_snoozed:
  for +now: Nat
  for +w: Nat
  for t: M.Thread
  for +until: Nat
  for h: {Nat.is_ge(now, until) == False{} : Bool}
  {M.Settle.auto.at(now, w, t, until) == False{} : Bool}

# a snooze ends by the clock alone: once its time has come the thread is
# awake, with no event needed
law snooze_wakes:
  for +now: Nat
  for +until: Nat
  for h: {Nat.is_ge(now, until) == True{} : Bool}
  {M.Snooze.on(now, until) == False{} : Bool}

# Manual order
# ------------

# past any place on a shelf there is a later one
law key_after:
  for s: List<&2, Bool>
  {M.Key.lt(s, M.Key.after(s)) == True{} : Bool}

# the key made between two places sorts after the first...
law key_mid_above:
  for a: List<&2, Bool>
  for b: List<&2, Bool>
  for h: {M.Key.lt(a, b) == True{} : Bool}
  {M.Key.lt(a, M.Key.mid(a, b)) == True{} : Bool}

# ...and before the second: a dragged thread lands strictly between its
# neighbours, and they keep their keys
law key_mid_below:
  for a: List<&2, Bool>
  for b: List<&2, Bool>
  for h: {M.Key.lt(a, b) == True{} : Bool}
  {M.Key.lt(M.Key.mid(a, b), b) == True{} : Bool}

# a project dragged into place sits above every project never dragged
law project_keyed_first:
  for x: Bool
  for xs: List<&2, Bool>
  for a: U.SRow
  for b: U.SRow
  {U.PSlot.before(U.PSlot{x <> xs, a}, U.PSlot{Nil{}, b}) == True{} : Bool}

# and one never dragged sits below every one that was
law project_unkeyed_below:
  for a: U.SRow
  for y: Bool
  for ys: List<&2, Bool>
  for b: U.SRow
  {U.PSlot.before(U.PSlot{Nil{}, a}, U.PSlot{y <> ys, b}) == False{} : Bool}

# Sidebar status
# --------------

# an approval waiting outranks everything on a thread's dot
law status_approval_first:
  for i: Bool
  for t: M.TurnState
  for q: Bool
  {M.Status.of(True{}, i, t, q) == M.StApproval{} : M.Status}

# then a question or plan waiting, even while a turn runs
law status_input_before_work:
  for t: M.TurnState
  for q: Bool
  {M.Status.of(False{}, True{}, t, q) == M.StInput{} : M.Status}

# then a running turn, even with messages queued
law status_working_before_queued:
  for q: Bool
  {M.Status.of(False{}, False{}, M.Running{}, q) == M.StWorking{} : M.Status}

# a failed turn shows as failed even with messages queued
law status_failed_before_queued:
  for q: Bool
  {M.Status.of(False{}, False{}, M.Failed{}, q) == M.StFailed{} : M.Status}

# nothing on a thread outranks an approval
law status_approval_top:
  for a: Bool
  for i: Bool
  for t: M.TurnState
  for q: Bool
  {Nat.is_ge(M.Status.rank(M.StApproval{}), M.Status.rank(M.Status.of(a, i, t, q))) == True{} : Bool}

# Thread phase
# ------------

# an approval waiting outranks everything on a thread's phase line
law phase_approval_first:
  for i: Bool
  for n: Nat
  for t: M.TurnState
  for q: Bool
  {M.Phase.of(True{}, i, n, t, q) == M.PhApproval{} : M.Phase}

# then a question or plan waiting, even while a turn or tasks run
law phase_input_before_work:
  for n: Nat
  for t: M.TurnState
  for q: Bool
  {M.Phase.of(False{}, True{}, n, t, q) == M.PhInput{} : M.Phase}

# a running turn is working, whatever its child tasks do
law phase_working_before_tasks:
  for n: Nat
  for q: Bool
  {M.Phase.of(False{}, False{}, n, M.Running{}, q) == M.PhWorking{} : M.Phase}

# a finished turn with child tasks still running is monitoring them, not complete
law phase_monitoring_before_complete:
  for n: Nat
  for q: Bool
  {M.Phase.of(False{}, False{}, 1n+n, M.Completed{}, q) == M.PhMonitoring{1n+n} : M.Phase}

# a finished turn with nothing left running or queued is complete
law phase_complete:
  {M.Phase.of(False{}, False{}, 0n, M.Completed{}, False{}) == M.PhComplete{} : M.Phase}

# a failed turn shows as failed even with messages queued
law phase_failed_before_queued:
  {M.Phase.of(False{}, False{}, 0n, M.Failed{}, True{}) == M.PhFailed{} : M.Phase}

# only a thread with nothing yet has no phase line
law phase_silent_only_idle:
  for a: Bool
  for i: Bool
  for n: Nat
  for t: M.TurnState
  for q: Bool
  {String.is_empty(M.Phase.label(M.Phase.of(a, i, n, t, q))) == String.eq(M.Phase.name(M.Phase.of(a, i, n, t, q)), "idle") : Bool}

# Board viewer
# ------------

# re-reading an unchanged board changes nothing: every item keeps its birth
# time, so nothing re-fades or flickers (the viewer's no-jitter promise)
law scene_merge_self:
  for ss: List<&2, V.Seen>
  for +now: Nat
  {V.Scene.update(ss, V.Scene.items(ss), now) == ss : List<&2, V.Seen>}

# a new item starts invisible
law fade_start:
  for +born: Nat
  {V.Fade.alpha(born, born) == 0 : U32}

# and is fully drawn once its fade time has passed, at any later time
law fade_done:
  for +born: Nat
  for +k: Nat
  {V.Fade.alpha(Nat.add(Nat.add(born, V.Fade.ms()), k), born) == 255 : U32}

# Layers the user turns on and off
# --------------------------------

# with nothing flipped every layer looks exactly as by default
law look_flip_none:
  for +kind: U32
  for +layer: U32
  {V.Look.on(kind, layer, 0) == V.Look.of(kind, layer) : U32 & U32}

# flipping a shown layer hides it (F.Cu)...
law look_flip_hides:
  {V.Look.shows(0, 12, V.Look.toggle(0, 12)) == False{} : Bool}

# ...and flipping a hidden one shows it (F.CrtYd)
law look_flip_shows:
  {V.Look.shows(0, 17, V.Look.toggle(0, 17)) == True{} : Bool}

# the viewer's light ground shows and hides the same layers as the dark
# one (F.Cu, B.Cu and a hidden courtyard), and neither ground's bit is a
# layer's flip
law look_light_same_alpha:
  {[V.Look.alpha(0, 12, V.Look.light.bit()), V.Look.alpha(0, 5, V.Look.light.bit()), V.Look.alpha(0, 17, V.Look.light.bit()), V.Look.alpha(1, 14, V.Look.light.bit())]
    == [V.Look.alpha(0, 12, 0), V.Look.alpha(0, 5, 0), V.Look.alpha(0, 17, 0), V.Look.alpha(1, 14, 0)] : List<&2, U32>}

law look_grounds_not_layers:
  {V.Look.shows(0, 12, U32.or(V.Look.light.bit(), V.Look.ui.bit())) == True{} : Bool}

# the dark theme draws every colour as it is
law theme_dark_as_is:
  for +c: U32
  {TH.Th.of(False{}, c) == c : U32}

# flipping a layer twice gives it back
law look_flip_twice:
  {V.Look.toggle(V.Look.toggle(4096, 12), 12) == 4096 : U32}

# an item on a hidden layer draws nothing, wherever it is and however new
law ops_hidden_draws_nothing:
  for +p: B.Prim
  for +l: U32
  for +v: V.View
  for +c: D.Clip
  for +now: Nat
  for +born: Nat
  for +kind: U32
  for +flip: U32
  for +rest: List<&2, R.Op>
  for h: {V.Look.shows(kind, l, flip) == False{} : Bool}
  {V.Ops.gate(True{}, p, l, v, c, now, born, kind, flip, rest) == rest : List<&2, R.Op>}

# and a click never picks it: the best pick so far stays
law pick_hidden_skipped:
  for hit: Bool
  for +area: F32
  for +key: String
  for bk: String
  for +ba: F32
  {V.Pick.better((hit, area), False{}, key, (bk, ba)) == (bk, ba) : String & F32}

# Updates
# -------

# a version is equal to itself
law semver_cmp_refl:
  for +v: SV.Ver
  {SV.Ver.cmp(v, v) == EQ{} : Cmp}

# the updater never offers the version already running
law update_never_same:
  for +v: SV.Ver
  {SV.Ver.newer(v, v) == False{} : Bool}

# a packaged install never updates itself: BACKPLANE_NO_UPDATE set, or
# running as an AppImage
law update_off_managed:
  for +c: Char
  for +t: String
  for +a: String
  {SV.Chan.managed(SCon{c, t}, a) == True{} : Bool}

law update_appimage_managed:
  for +o: String
  for +c: Char
  for +t: String
  {SV.Chan.managed(o, SCon{c, t}) == True{} : Bool}

# a prerelease (a nightly) sorts before the release of its version
law semver_pre_before_release:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +n: Nat
  {SV.Ver.newer(SV.Ver{a, b, c, False{}, 0n, 0n}, SV.Ver{a, b, c, True{}, d, n}) == True{} : Bool}

# a later nightly build of the same day is newer
law semver_nightly_later:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +n: Nat
  {SV.Ver.newer(SV.Ver{a, b, c, True{}, d, 1n+n}, SV.Ver{a, b, c, True{}, d, n}) == True{} : Bool}

# a nightly from a later day is newer, whatever its build number
law semver_nightly_next_day:
  for +a: Nat
  for +b: Nat
  for +c: Nat
  for +d: Nat
  for +n: Nat
  for +m: Nat
  {SV.Ver.newer(SV.Ver{a, b, c, True{}, 1n+d, m}, SV.Ver{a, b, c, True{}, d, n}) == True{} : Bool}

# Attachments
# -----------

# a stored attachment's name uses only letters, digits, '.', '_' and '-',
# so no name can reach outside its thread's directory
law attach_clean_ok:
  for s: String
  {AT.Att.all_ok(AT.Att.clean(s)) == True{} : Bool}

# a dropped uri-list line that is not a file: URI (http:, a '#' comment,
# anything not starting with 'f') attaches nothing
law drop_non_file_ignored:
  for c: Char
  for s: String
  for rest: List<&2, String>
  for h: {Char.is_eq(c, 'f') == False{} : Bool}
  {AT.Att.uri.one(SCon{c, s}, rest) == rest : List<&2, String>}

# a file manager's drop: file: URIs (any host) come back as their decoded
# paths, in order; comments and other schemes are dropped
law drop_uri_list_example:
  {AT.Att.uri_list("# nautilus\r\nfile:///home/me/My%20Board%C3%A9.png\r\nhttps://x.org/a.png\r\nfile://localhost/tmp/b%25.txt\r\n") == ["/home/me/My Boardé.png", "/tmp/b%.txt"] : List<&2, String>}

# Models working together
# -----------------------

# a finished, failed or cancelled task never changes state again
law task_terminal_absorbing:
  for cur: M.TaskState
  for want: M.TaskState
  for h: {M.TaskState.terminal(cur) == True{} : Bool}
  {M.TaskState.next(cur, want) == cur : M.TaskState}

# a running task never goes back to the queue
law task_never_requeued:
  {M.TaskState.next(M.TaskRunning{}, M.TaskQueued{}) == M.TaskRunning{} : M.TaskState}

# once delivered to its parent, a task's result is never delivered again
law task_delivered_once:
  for t: M.Task
  {M.Task.pending(M.Task.delivered_set(t)) == False{} : Bool}

# delegation stops at the depth limit: no chain of models spawning models
# grows without end
law delegation_bounded:
  for +k: Nat
  {M.Task.may_delegate(Nat.add(M.Task.max_depth(), k)) == False{} : Bool}

# Settings
# --------

# a setting never set reads as its default (settings are events, so a
# fresh or replayed store agrees on every unset key)
law setting_default:
  for +k: String
  for +d: String
  {M.Setting.get(M.State.empty(), k, d) == d : String}

# Viewer sources
# --------------

# giving the window back its own viewer changes nothing
law nui_with_own_vw:
  for n: N.Nui
  {N.Nui.with_vw(n, N.Nui.vw(n)) == n : N.Nui}

# a scene read for a source the viewer no longer shows changes nothing:
# switching views or projects can never show a stale file
law stale_board_ignored:
  for n: N.Nui
  for +key: String
  for items: List<&2, B.Item>
  for +now: Nat
  for h: {N.Vw.fresh_for(N.Nui.vw(n), key) == False{} : Bool}
  {N.Nui.board(n, key, items, now) == n : N.Nui}

# Delivery
# --------

# a client that reconnects with the first changes already folded, and
# folds only what it missed, holds exactly what a cold replay gives
law resume_is_replay:
  for a: List<&2, M.Change>
  for b: List<&2, M.Change>
  for s: M.State
  {M.State.replay(List.append(&2, M.Change, a, b), s) == M.State.replay(b, M.State.replay(a, s)) : M.State}

# a message for a running thread is delivered into the turn at once; no
# message ever waits (Hub.deliver)
law post_running_delivers:
  for +s: M.State
  for +now: Nat
  for +th: String
  for +msg: String
  for role: M.Role
  for +text: String
  for h1: {H.Hub.running(H.Hub.thread(s, th)) == True{} : Bool}
  for h2: {M.Worktree.pending(H.Hub.thread(s, th), now) == False{} : Bool}
  {H.Hub.post(s, now, th, msg, role, text) == H.Hub.deliver.r(True{}, s, now, th, msg, role, text) : H.Step}

# whatever the thread runs, delivery holds nothing back
law deliver_never_waits:
  for run: Bool
  for +s: M.State
  for +now: Nat
  for +th: String
  for +msg: String
  for role: M.Role
  for +text: String
  {H.Deliver.held(H.Queue.changes(H.Hub.deliver(run, False{}, s, now, th, msg, role, text))) == False{} : Bool}

law deliver_held_go:
  for b: Bool
  for +th: String
  for +now: Nat
  {H.Deliver.held(List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, b, [], [M.QueueTaken{th, now}]), [M.TurnChanged{th, M.Running{}, now}])) == False{} : Bool}

law deliver_held_steer:
  for b1: Bool
  for b2: Bool
  for +th: String
  for +id: String
  for +now: Nat
  {H.Deliver.held(List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, b1, [], [M.QueueTaken{th, now}]), List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, b2, [], [M.ActivityLogged{th, id, "info", "steer", "Steered: the turn starts again with this message", now}]), [M.TurnChanged{th, M.Running{}, now}]))) == False{} : Bool}

law deliver_held_steer_k:
  for c: Bool
  for +s: M.State
  for +now: Nat
  for +th: String
  for +msg: String
  for role: M.Role
  for +text: String
  {H.Deliver.held(H.Queue.changes(H.Hub.deliver.steer(c, s, now, th, msg, role, text))) == False{} : Bool}

# Claude takes it on its input, mid-turn
law deliver_claude_running:
  {H.Queue.changes(H.Hub.post(H.Demo.state("claude", M.Running{}), 5n, "t1", "m1", M.User{}, "hi")) == [M.MessagePosted{"t1", "m1", M.User{}, "hi", 5n}, M.TurnChanged{"t1", M.Running{}, 5n}] : List<&2, M.Change>}

# a message the server already stored (resent after a dropped link) is not
# stored again, and starts nothing
law resend_stored_once:
  for +s: M.State
  for +now: Nat
  for +client: U32
  for +id: String
  for +th: String
  for +text: String
  for +msg: String
  for h1: {String.is_empty(msg) == False{} : Bool}
  for h2: {H.Hub.has_msg(H.Hub.entries(s), msg) == True{} : Bool}
  {H.Queue.changes(H.Hub.turn(s, now, client, id, th, text, msg)) == [] : List<&2, M.Change>}

# an unnamed message is never taken for one already stored: only a named
# one is looked up
law has_msg_unnamed_skips:
  for s: M.State
  for msg: String
  {H.Hub.dup(False{}, s, msg) == False{} : Bool}

# a thread's last reply is its newest assistant message, read from its own
# entries; what it sent another thread is not its reply
law mcp_last_newest:
  {MC.Mcp.last(M.State.mine(M.State.replay([M.MessagePosted{"t1", "m1", M.Assistant{}, "first", 3n}, M.MessagePosted{"t1", "m2", M.Assistant{}, "second", 4n}, M.MessagePosted{"t2", M.Link.id("t1", "x"), M.Assistant{}, "to t2", 5n}], H.Demo.state("claude", M.Completed{})), "t1"), "t1") == "second" : String}

# a step that ends no turn finishes no task: it comes back as it was
law task_finish_not_ended_same:
  for +s: M.State
  for +now: Nat
  for +thread: String
  for +cs: List<&2, M.Change>
  for os: List<&2, H.Out>
  for h: {H.Task.ended(cs) == H.NotEnded{} : H.Ended}
  {H.Task.finish(s, now, thread, H.Step{cs, os}) == H.Step{cs, os} : H.Step}

# a message from another thread is delivered into the target's running
# turn like any other
law inbox_running_delivers:
  for +s: M.State
  for +now: Nat
  for +from: String
  for +to: String
  for role: M.Role
  for +text: String
  for +msg: String
  for h1: {H.Hub.running(H.Hub.thread(s, to)) == True{} : Bool}
  for h2: {M.Worktree.pending(H.Hub.thread(s, to), now) == False{} : Bool}
  {H.Hub.inbox.id(s, now, from, to, role, text, msg) == H.Hub.deliver.r(True{}, s, now, to, msg, role, text) : H.Step}
# Worktrees
# ---------

# a thread in the project folder never waits for a worktree
law worktree_pending_local:
  for +wt: String
  for +c: Nat
  for +now: Nat
  {M.Worktree.pending.k(M.Local{}, wt, c, now) == False{} : Bool}

# no thread waits for its worktree past the wait: its messages then run in
# the project folder
law worktree_pending_ends:
  for e: M.EnvMode
  for +wt: String
  for +c: Nat
  for +now: Nat
  for h: {Nat.is_lt(now, Nat.add(c, M.Worktree.wait())) == False{} : Bool}
  {M.Worktree.pending.k(e, wt, c, now) == False{} : Bool}

# a message to a thread whose worktree is still being made starts nothing:
# it waits in the thread's queue
law worktree_waits:
  for +s: M.State
  for +now: Nat
  for +th: String
  for +msg: String
  for role: M.Role
  for +text: String
  for h: {M.Worktree.pending(H.Hub.thread(s, th), now) == True{} : Bool}
  {H.Step.outs_of(H.Hub.post(s, now, th, msg, role, text)) == [] : List<&2, H.Out>}

# only "local" asks for the project folder; every other mode (the default
# "auto" included) asks for a worktree
law worktree_by_default:
  {H.Hub.env.of("auto") == M.Worktree{} : M.EnvMode}

# a message to a thread whose worktree was removed makes it again from the
# kept branch, and does nothing else: the turn waits for the worktree and
# never runs in the project folder
law worktree_revives:
  for +s: M.State
  for +now: Nat
  for +th: String
  for +msg: String
  for role: M.Role
  for +text: String
  for h1: {H.Hub.running(H.Hub.thread(s, th)) == False{} : Bool}
  for h2: {M.Worktree.gone(H.Hub.thread(s, th)) == True{} : Bool}
  for h3: {M.Worktree.pending(H.Hub.thread(s, th), now) == False{} : Bool}
  {H.Step.outs_of(H.Hub.post(s, now, th, msg, role, text)) == [H.Wt.revive(s, th)] : List<&2, H.Out>}

# a worktree is never removed while its thread works or waits on the user
law worktree_kept_while_busy:
  for sh: M.Shelf
  for +ours: Bool
  for merged: Bool
  for aged: Bool
  {M.Worktree.drop.k(True{}, sh, ours, merged, aged) == False{} : Bool}

# a worktree is never removed while its thread is out (active, pinned,
# snoozed): only settled or hidden threads lose theirs
law worktree_kept_out:
  for busy: Bool
  for sh: M.Shelf
  for +ours: Bool
  for merged: Bool
  for aged: Bool
  for h: {M.Worktree.parked(sh) == False{} : Bool}
  {M.Worktree.drop.k(busy, sh, ours, merged, aged) == False{} : Bool}

# only a directory Backplane made (under .backplane/worktrees) is removed
law worktree_kept_unless_ours:
  for busy: Bool
  for sh: M.Shelf
  for +wt: String
  for merged: Bool
  for aged: Bool
  for h: {M.Worktree.ours(wt) == False{} : Bool}
  {M.Worktree.drop.k(busy, sh, M.Worktree.ours(wt), merged, aged) == False{} : Bool}

# a settled thread that was not merged keeps its worktree for the window
law worktree_kept_fresh:
  for +ours: Bool
  {M.Worktree.drop.k(False{}, M.Settled{}, ours, False{}, False{}) == False{} : Bool}

# a merged (so settled) idle thread's worktree goes at once
law worktree_dropped_on_merge:
  for aged: Bool
  {M.Worktree.drop.k(False{}, M.Settled{}, True{}, True{}, aged) == True{} : Bool}

# a settled idle thread's worktree goes once the window has passed
law worktree_dropped_when_aged:
  for merged: Bool
  {M.Worktree.drop.k(False{}, M.Settled{}, True{}, merged, True{}) == True{} : Bool}

# so does a hidden (archived or deleted) thread's
law worktree_dropped_when_hidden:
  for merged: Bool
  {M.Worktree.drop.k(False{}, M.Hidden{}, True{}, merged, True{}) == True{} : Bool}

# Merges
# ------

# a merged thread settles, pinned or brought back before or not
law merge_settles:
  for t: M.Thread
  for +at: Nat
  for h: {M.Thread.hidden(t) == False{} : Bool}
  {M.Shelf.of(M.Thread.settled(True{}, at, t)) == M.Settled{} : M.Shelf}

# a recorded merge is never due again, so it settles a thread once
law merge_once:
  for sh: M.Shelf
  for +branch: String
  for +url: String
  for h: {String.is_empty(url) == False{} : Bool}
  {M.Merge.due.k(sh, branch, url) == False{} : Bool}

# a thread without a branch is never merged
law merge_needs_branch:
  for sh: M.Shelf
  for +merged: String
  {M.Merge.due.k(sh, "", merged) == False{} : Bool}

# GitHub accounts
# ---------------

# the account a repository worked with before is tried first
law gh_pick_saved:
  for owner_in: Bool
  for +saved: String
  for +owner: String
  for +active: String
  {GH.Gh.pick(True{}, saved, owner_in, owner, active) == saved : String}

# else the account named like the remote's owner
law gh_pick_owner:
  for +saved: String
  for +owner: String
  for +active: String
  {GH.Gh.pick(False{}, saved, True{}, owner, active) == owner : String}

# else gh's active account: Backplane never switches it
law gh_pick_active:
  for +saved: String
  for +owner: String
  for +active: String
  {GH.Gh.pick(False{}, saved, False{}, owner, active) == active : String}

# the first choice is tried first
law gh_order_first:
  for +first: String
  for us: List<&2, String>
  {List.head(&2, String, GH.Gh.order.k(first, us)) == Some{first} : Maybe<&2, String>}

# and never again: the rest leaves it out
law gh_without_drops:
  for us: List<&2, String>
  for +u: String
  {GH.Gh.has(GH.Gh.without(us, u), u) == False{} : Bool}

# Phone alerts
# ------------

# a phone alerts only for a turn it saw running: connecting, or a thread
# that never ran, raises nothing
law alert_needs_running:
  for t: M.TurnState
  {Nt.Note.kind(False{}, t) == "" : String}

# a turn still running never alerts
law alert_never_running:
  for b: Bool
  {Nt.Note.kind(b, M.Running{}) == "" : String}

# a stop the user asked for is not news
law alert_not_on_stop:
  for b: Bool
  {Nt.Note.kind(b, M.Interrupted{}) == "" : String}

# connecting (no turn seen running yet) raises no alerts, however many
# threads and finished turns the log holds
law alert_none_on_connect:
  for ts: List<&2, M.Thread>
  for +st: M.State
  for +es: List<&2, M.Entry>
  for +desk: String
  {Nt.Note.alerts(ts, [], st, es, desk) == [] : List<&2, J.Json>}

# One item, one alert (core/notice.bend). Notice.decide(failed, bot,
# asking, posted, first, msg): 0 no alert, 1 an alert, 2 a failure.

# a failed turn always alerts, as a failure
law notice_fail_always:
  for bot: Bool
  for asking: Bool
  for posted: Bool
  for first: Bool
  for msg: Bool
  {NO.Notice.decide(True{}, bot, asking, posted, first, msg) == 2n : Nat}

# a thread that is not a bot's alerts when it finishes: work the person started
law notice_thread_kept:
  for asking: Bool
  for posted: Bool
  for first: Bool
  for msg: Bool
  {NO.Notice.decide(False{}, False{}, asking, posted, first, msg) == 1n : Nat}

# a bot waiting on the person alerts, whatever else its turn did
law notice_ask_kept:
  for posted: Bool
  for first: Bool
  for msg: Bool
  {NO.Notice.decide(False{}, True{}, True{}, posted, first, msg) == 1n : Nat}

# a room exchange alerts once: a bot post after the exchange's first is quiet
law notice_room_once:
  for msg: Bool
  {NO.Notice.decide(False{}, True{}, False{}, True{}, False{}, msg) == 0n : Nat}

# a bot woken by a room or bot message that posted nothing is quiet
law notice_chatter_quiet:
  for first: Bool
  {NO.Notice.decide(False{}, True{}, False{}, False{}, first, True{}) == 0n : Nat}

# an earlier bot post in the same room (not the person's, not this turn's)
# means the exchange already alerted
law notice_earlier_bot_quiet:
  {NO.Vd.room(True{}, False{}, False{}, True{}) == NO.VNo{} : NO.Vd}

# a person's post starts a new exchange
law notice_person_opens:
  for own: Bool
  for bot: Bool
  {NO.Vd.room(True{}, True{}, own, bot) == NO.VYes{} : NO.Vd}

# past the window, older posts no longer hold an alert back
law notice_window_opens:
  for +room: String
  for +name: String
  for +start: Nat
  for local: Bool
  for p: BOT.Post
  {NO.Vd.post(True{}, room, name, start, local, p) == NO.VYes{} : NO.Vd}

# a phone paired with several hubs names every thread and project by its
# hub ("key|id"), but "nothing selected" stays nothing, on any hub
law hubs_no_id:
  for +k: String
  {Hb.Hubs.id(k, "") == "" : String}

# the screen's id walk knows every name it treats apart: the hash that
# turns keys away first never turns one of them away
law q_kind_names:
  {Hb.Q.own(Hb.QN{"value", 2n} <> Hb.QN{"id", 2n} <> Hb.Q.names()) == True{} : Bool}

# a phone builds no screen for streamed text from a hub not in focus...
law hubs_delta_elsewhere:
  for +hs: Hb.Hubs
  for +key: String
  for +o: J.Json
  for h: {String.eq(key, Hb.Hubs.cur(hs)) == False{} : Bool}
  {Hb.Hubs.shows(hs, key, o) == False{} : Bool}

# ...nor for a thread other than the one open on the hub in focus
law hubs_delta_other_thread:
  for +hs: Hb.Hubs
  for +th: String
  for +text: String
  for h: {String.eq(th, U.Ui.sel(Hb.Hubs.ui(hs, Hb.Hubs.cur(hs)))) == False{} : Bool}
  {Hb.Hubs.shows(hs, Hb.Hubs.cur(hs), J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{th}}, J.KV{"text", J.Str{text}}])) == False{} : Bool}

# the phone redraws for any message from any paired hub (the merged
# sidebar and the island show them all); streamed text only as Hubs.shows
# says: not from a hub out of focus, nor for another thread
law hubs_redraw_changes_elsewhere:
  for +hs: Hb.Hubs
  for +key: String
  for +items: List<&2, J.Json>
  {Hb.Hubs.redraw(hs, key, J.Json.obj([J.KV{"t", J.Str{"changes"}}, J.KV{"items", J.Json.arr(items)}])) == True{} : Bool}

law hubs_redraw_delta_elsewhere:
  for +hs: Hb.Hubs
  for +key: String
  for +th: String
  for +text: String
  for h: {String.eq(key, Hb.Hubs.cur(hs)) == False{} : Bool}
  {Hb.Hubs.redraw(hs, key, J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{th}}, J.KV{"text", J.Str{text}}])) == False{} : Bool}

law hubs_redraw_delta_other_thread:
  for +hs: Hb.Hubs
  for +th: String
  for +text: String
  for h: {String.eq(th, U.Ui.sel(Hb.Hubs.ui(hs, Hb.Hubs.cur(hs)))) == False{} : Bool}
  {Hb.Hubs.redraw(hs, Hb.Hubs.cur(hs), J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{th}}, J.KV{"text", J.Str{text}}])) == False{} : Bool}

# streamed text raises no alert (and nothing is compared for it)
law alerts_none_for_delta:
  for +u: U.Ui
  for +th: String
  for +text: String
  {Hb.Did.alerts(Hb.One.got(u, J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{th}}, J.KV{"text", J.Str{text}}]))) == [] : List<&2, J.Json>}

# the island read from each thread's own entries says what the walk over
# the whole log says, a message the thread sent elsewhere included
law island_doing_mine_same:
  {Nt.Note.island.st(U.Ui.threads(Hb.Island.demo()), Hb.Island.demo()) == Nt.Note.island(U.Ui.threads(Hb.Island.demo()), U.Ui.entries(Hb.Island.demo())) : J.Json}

law island_doing_mine_text:
  {J.Json.show(Nt.Note.island.st(U.Ui.threads(Hb.Island.demo()), Hb.Island.demo())) == "{\"running\":1,\"headline\":\"x\",\"lines\":[{\"thread\":\"t1\",\"title\":\"x\",\"doing\":\"tool ran ls\"}]}" : String}

# a hub out of focus builds no thread for the merged screen
law hubs_screen_unfocused_thread_null:
  {J.Json.get_obj(MSC.Mob.screen.f(False{}, U.Ui.init(0n)), "thread") == J.Null{} : J.Json}

# a phone paired with no hub sends nothing, however time passes
law hubs_unpaired_quiet:
  for +cid: String
  for now: Nat
  for cur: String
  for +t: Nat
  {Hb.Hubs.cmds(Hb.Hubs.tick(Hb.Hubs{cid, now, cur, []}, t)) == [] : List<&2, Hb.HCmd>}

# with no thread running the island is empty, so the Live Activity (iOS)
# ends and the live service (Android) stops
law island_idle_empty:
  for ts: List<&2, M.Thread>
  for +es: List<&2, M.Entry>
  for h: {Nt.Note.busy(ts) == False{} : Bool}
  {Nt.Note.lines(ts, es) == [] : List<&2, J.Json>}

# Phone pushes
# ------------

# a push token never goes to clients: what they get of a registration is
# the same whatever the token (and the rest of the registration) was
law device_token_never_sent:
  for tok: String
  for p: String
  for k: String
  for th: String
  for e: String
  for b: String
  for +at: Nat
  {C.Change.wire(M.DeviceRegistered{tok, p, k, th, e, b, at}) == C.Change.wire(M.DeviceRegistered{"", "", "", "", "", "", at}) : J.Json}

# nor when a token is dropped
law device_drop_never_sent:
  for tok: String
  for +at: Nat
  {C.Change.wire(M.DeviceDropped{tok, at}) == C.Change.wire(M.DeviceDropped{"", at}) : J.Json}

# a registration without a token (what clients fold) registers nothing
law device_blank_ignored:
  for s: M.State
  for p: String
  for k: String
  for th: String
  for e: String
  for b: String
  for at: Nat
  {M.State.devices(M.State.apply(s, M.DeviceRegistered{"", p, k, th, e, b, at})) == M.State.devices(s) : Map<&2, M.Device>}

# registering the same token twice leaves one device: the second
# registration replaces the first
law device_register_once:
  for +tok: String
  for h: {String.is_empty(tok) == False{} : Bool}
  for p: String
  for k: String
  for th: String
  for e: String
  for b: String
  for at: Nat
  for p2: String
  for k2: String
  for th2: String
  for e2: String
  for b2: String
  for at2: Nat
  {Map.size(&2, M.Device, M.State.devices(M.State.apply(M.State.apply(M.State.empty(), M.DeviceRegistered{tok, p, k, th, e, b, at}), M.DeviceRegistered{tok, p2, k2, th2, e2, b2, at2}))) == 1n : Nat}

# Pickers
# -------

# an empty query matches every candidate (a picker shows everything
# before you type)
law fuzzy_empty_matches:
  for s: String
  {Nat.is_gt(FZ.Fuzzy.score(s, ""), 0n) == True{} : Bool}

# the sidebar's project search: an empty query keeps every project, in
# order, and a shut search shows the sidebar as it is
law find_empty_keeps:
  for rs: List<&2, U.SRow>
  {U.Find.rows(rs, "") == rs : List<&2, U.SRow>}

law find_shut_shows_all:
  for st: M.State
  for now: Nat
  for dr: Map<&2, String>
  {U.Side.shown(st, now, U.Find.q.k(False{}, dr)) == U.Side.rows(st, now) : List<&2, U.SRow>}

# the sidebar's search field is always there now: its query holds whether
# or not the phones' old toggle ("psearch") is on
law find_always_on:
  for op: Map<&2, Bool>
  for dr: Map<&2, String>
  {U.Find.q(op, dr) == U.Str.get(dr, "@f.pq") : String}

# the active view puts what waits on the person first: nothing ranks over
# an approval, and a question or a finished turn sorts above work in
# progress, pinned or not
law urg_approval_top:
  for p: M.Phase
  {Nat.is_ge(U.Urg.rank(M.PhApproval{}), U.Urg.rank(p)) == True{} : Bool}

law urg_input_over_working:
  for a: Bool
  for b: Bool
  {Nat.is_gt(U.Urg.key(M.PhInput{}, a), U.Urg.key(M.PhWorking{}, b)) == True{} : Bool}

law urg_done_over_working:
  for a: Bool
  for b: Bool
  {Nat.is_gt(U.Urg.key(M.PhComplete{}, a), U.Urg.key(M.PhWorking{}, b)) == True{} : Bool}

# within a phase, a pinned thread comes first
law urg_pin_within:
  for p: M.Phase
  {Nat.is_gt(U.Urg.key(p, True{}), U.Urg.key(p, False{})) == True{} : Bool}

# the active view with its settled fold shut shows only the live rows
law active_fold_shut:
  for l: List<&2, U.ARow>
  for f: List<&2, U.ARow>
  {U.Active.folded(False{}, U.AParts{l, f}) == l : List<&2, U.ARow>}

# a settled thread's menu leads with bringing it back, and ends with a
# Delete that asks first (menu_delete_asks in test/sidebar_test.bend)
# every thread's menu can rename it (its second item, whatever its shelf)
law menu_renames_settled:
  for p: Bool
  for +id: String
  for armed: Bool
  {List.get(&2, U.MItem, U.Menu.thread.k(M.Settled{}, p, id, armed), 1n) == Some{U.MItem{"Rename", "row-rename", id, ""}} : Maybe<&2, U.MItem>}

law menu_renames_open:
  for +id: String
  for armed: Bool
  {List.get(&2, U.MItem, U.Menu.thread.k(M.Active{}, False{}, id, armed), 1n) == Some{U.MItem{"Rename", "row-rename", id, ""}} : Maybe<&2, U.MItem>}

# a rename to a blank title changes nothing
# a project brought back has its threads: adding its folder starts none
law project_back_no_thread:
  for +s: M.State
  for +now: Nat
  for +client: U32
  for +id: String
  for +pid: String
  for +made: M.Change
  {H.Hub.project_add.new(True{}, s, now, client, id, pid, made) == H.Step.reply(client, id, [made]) : H.Step}

law rename_blank_nothing:
  for +th: String
  for +now: Nat
  {H.Rename.changes(th, "", now) == [] : List<&2, M.Change>}

law menu_settled_brings_back:
  for p: Bool
  for id: String
  for armed: Bool
  {List.head(&2, U.MItem, U.Menu.thread.k(M.Settled{}, p, id, armed)) == Some{U.MItem{"Bring back", "row-unsettle", id, ""}} : Maybe<&2, U.MItem>}

# a caret that just moved (or a field just focused) is shown
law caret_shows_when_moved:
  {N.Blink.shown(0n) == True{} : Bool}

# a hub that does not know its owner lets no tailnet device in without the
# pairing token
law tailnet_no_owner_no_trust:
  for login: String
  {TN.Tailnet.trusts("", login) == False{} : Bool}

# the pairing link carries the token after "#", as the phones and the
# web page read it
law tailnet_link_token:
  {TN.Tailnet.link("http://100.64.0.1:3787", "ab12") == "http://100.64.0.1:3787/#token=ab12" : String}

# Drawing only what changed
# ------------------------
# A frame is kept as tiles and the timeline as per-entry parts; the next
# frame reuses exactly what is equal and draws the rest.

# a tile looked up in any drawn tiles, wherever the lookup lands, is
# exactly the tile drawn from scratch: an old image is taken only when it
# drew the same local ops over the same base
law seek_made:
  for k: Nat
  for +e: Nat
  for +old: List<&2, R.Op>
  for +b0: Image
  for +mx: U32
  for +my: U32
  for +mn: U32
  for +x: U32
  for +y: U32
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +l: List<&2, R.Op>
  for +base: Image
  {T.Tiles.seek(k, T.Tiles.make(k, e, old, b0, mx, my, mn), x, y, rx, ry, n, e, l, base) == T.Tile.draw(e, l, base) : Image}

# drawing a frame over the last one's tiles gives exactly the tiles drawn
# from scratch: a kept tile is never stale, whatever changed (ops, base),
# and neither is one taken from where a scroll moved it (any past tiles,
# any shift)
law tiles_again_exact:
  for k: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +old: List<&2, R.Op>
  for +base0: Image
  for +base1: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +pk: Nat
  for +old2: List<&2, R.Op>
  for +base2: Image
  for +prx: U32
  for +pry: U32
  for +pn: U32
  for +qn: U32
  for +dy: U32
  {T.Tiles.again(k, e, ops, base1, T.Tiles.make(k, e, old, base0, rx, ry, n), T.Past{pk, T.Tiles.make(pk, e, old2, base2, prx, pry, pn), qn, dy}, rx, ry, n) == T.Tiles.make(k, e, ops, base1, rx, ry, n) : T.Tiles}

# a tile whose ops and base are unchanged is kept as it is, whatever image
# it holds: it is not drawn again
law tile_keep:
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +l: List<&2, R.Op>
  for +base: Image
  for +img: Image
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  {T.Tile.again(e, ops, base, T.Tile{ops, l, base, img}, p, rx, ry, n) == T.Tile{ops, l, base, img} : T.Tile}

# the same for a whole frame as the window draws it, even when the window
# was resized in between, for any scroll hint
law frame_again_exact:
  for +w0: U32
  for +h0: U32
  for +w: U32
  for +h: U32
  for +ops: List<&2, R.Op>
  for +old: List<&2, R.Op>
  for +base0: Image
  for +base: Image
  for +dy: U32
  {T.Frame.again(w, h, ops, base, T.FSome{w0, h0, T.Frame.make(w0, h0, old, base0)}, dy) == T.Frame.make(w, h, ops, base) : T.Tiles}

# a tile's image taken from what the parallel jobs drew is the tile drawn
# here, wherever its path leads in whatever jobs ran
law drawn_get_run:
  for t: T.Jobs
  for path: List<&2, Bool>
  for +e: Nat
  for +l: List<&2, R.Op>
  for +base: Image
  {T.Drawn.get(T.Jobs.run(t, e), path, e, l, base) == T.Tile.draw(e, l, base) : Image}

# a plan filled from any jobs drawn in parallel is exactly the tiles
# Tiles.again gives, over any last tiles, with any scroll
law plan_fill_again:
  for k: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +base: Image
  for prev: T.Tiles
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +off: Nat
  for +t: T.Jobs
  {T.Plan.fill(T.Tiles.plan(k, e, ops, base, prev, p, rx, ry, n), e, off, T.Jobs.run(t, e)) == T.Tiles.again(k, e, ops, base, prev, p, rx, ry, n) : T.Tiles}

# the window's frame drawn in parallel is exactly Frame.again's (and so,
# by frame_again_exact, the frame drawn from scratch)
law frame_par_exact:
  for +w: U32
  for +h: U32
  for +ops: List<&2, R.Op>
  for +base: Image
  for prev: T.Frames
  for +dy: U32
  {T.Frame.par(w, h, ops, base, prev, dy) == T.Frame.again(w, h, ops, base, prev, dy) : T.Tiles}

# the timeline's memo brought up to date is exactly the memo made from
# scratch, for any earlier entries
law tl_again_exact:
  for es: List<&2, M.Entry>
  for +ty: L.Ty
  for +old: List<&2, M.Entry>
  for +px: U32
  for +sel: String
  {L.TL.memo.again(es, ty, px, sel, L.TL.memo.make(ty, old, px, sel)) == L.TL.memo.make(ty, es, px, sel) : List<&2, L.TLE>}

# an entry equal to the one its memo item was made for keeps the item:
# its markdown is not laid out again
law tl_keep:
  for +ty: L.Ty
  for +e: M.Entry
  for +px: U32
  for +sel: String
  for +p: L.TLPart
  {L.TL.memo.one(ty, e, px, sel, L.TLE{e, p}) == L.TLE{e, p} : L.TLE}

# the same for the whole memo, whatever width and thread it was made for
law tl_memo_exact:
  for +ty: L.Ty
  for +es: List<&2, M.Entry>
  for +old: List<&2, M.Entry>
  for +px0: U32
  for +sel0: String
  for +px: U32
  for +sel: String
  {L.TL.memo.of(ty, es, px, sel, L.TLMemo{px0, sel0, L.TL.memo.make(ty, old, px0, sel0), L.TL.live.none()}) == L.TLMemo{px, sel, L.TL.memo.make(ty, es, px, sel), L.TL.live.none()} : L.TLMemo}

# a memo lined up with the entries (an item dropped in front, or entries
# made in front: TL.align.of's guess, whatever it is) is still exactly the
# memo made for some entries, so bringing it up to date stays exact
law tl_align:
  for c: L.TLAlign
  for +es: List<&2, M.Entry>
  for +ty: L.Ty
  for old: List<&2, M.Entry>
  for +px: U32
  for +sel: String
  {L.TL.memo.apply(c, es, L.TL.memo.make(ty, old, px, sel), ty, px, sel) == L.TL.memo.make(ty, L.TL.memo.apply.es(c, es, old), px, sel) : List<&2, L.TLE>}

# the timeline drawn from a memo is the timeline drawn from scratch
law tl_lines_exact:
  for +ty: L.Ty
  for +es: List<&2, M.Entry>
  for +old: List<&2, M.Entry>
  for +px0: U32
  for +sel0: String
  for +st: M.State
  for +sel: String
  for +px: U32
  for +pics: Map<&2, String>
  {L.TL.memo.lines(ty, L.TLMemo{px0, sel0, L.TL.memo.make(ty, old, px0, sel0), L.TL.live.none()}, es, st, sel, px, pics) == L.TL.entries(ty, es, st, sel, px, pics) : List<&2, L.TLine>}
# Links
# -----

# only web addresses open: a link never runs script or reads a file
law link_no_script:
  for s: String
  {LK.Link.is_web("javascript:" ++ s) == False{} : Bool}

law link_no_file:
  for s: String
  {LK.Link.is_web("file:" ++ s) == False{} : Bool}

# a path is a file's link, never a web page's: it goes to the hub
law link_path_not_web:
  for s: String
  {LK.Link.is_web("/" ++ s) == False{} : Bool}

# an absolute path is opened as written, whatever the thread's folder
law link_file_abs_kept:
  for cwd: String
  for s: String
  {LK.Link.file.abs(cwd, "/" ++ s) == LK.Link.file.path("/" ++ s) : String}

# a file link's menu leads with Open, then Show in folder
law menu_file_open_first:
  for p: String
  {List.take(&2, U.MItem, U.Menu.file(p), 2n) == [U.MItem{"Open", "file-open", p, ""}, U.MItem{"Show in folder", "file-reveal", p, ""}] : List<&2, U.MItem>}

# a line with no links is drawn as it is, one run of text
law link_plain_line:
  for s: String
  {LK.Link.cut(s, Nil{}) == [LK.PText{s}] : List<&2, LK.Piece>}

# Image lightbox
# --------------

# the lightbox never zooms out past the fitted picture
law lightbox_zoom_min:
  for +z: Nat
  {Nat.is_ge(LBX.Lb.clamp(z), LBX.Lb.zmin()) == True{} : Bool}

# nor in past its most
law lightbox_zoom_max:
  for +z: Nat
  {Nat.is_le(LBX.Lb.clamp(z), LBX.Lb.zmax()) == True{} : Bool}

# a zoomed picture is never scrolled past its far edge: it keeps covering
# the window
law lightbox_scroll_max:
  for +a: Nat
  for +m: Nat
  for +d: Nat
  for +v: Nat
  {Nat.is_le(LBX.Lb.scroll(a, m, d, v), Nat.mul(Nat.sub(d, v), LBX.Lb.sub())) == True{} : Bool}

# a zoom ease starts where the zoom was
law lightbox_ease_start:
  for +z0: Nat
  for +z1: Nat
  for +t0: Nat
  {LBX.Lb.ease.at(z0, z1, t0, t0) == z0 : Nat}

# and lands on its target once its time has passed, at any later time
law lightbox_ease_done:
  for +z0: Nat
  for +z1: Nat
  for +t0: Nat
  for +k: Nat
  {LBX.Lb.ease.at(z0, z1, t0, Nat.add(Nat.add(t0, LBX.Lb.ms()), k)) == z1 : Nat}

# after which it asks for no more frames (no repaint loop)
law lightbox_idle:
  for +t0: Nat
  for +k: Nat
  {LBX.Lb.easing(t0, Nat.add(Nat.add(t0, LBX.Lb.ms()), k)) == False{} : Bool}

# a picture just opened is still: nothing animates
law lightbox_opens_still:
  for +now: Nat
  {LBX.Lb.busy(LBX.Lb.open(), now) == False{} : Bool}

# Icons
# -----

# closepath takes the pen back to where its subpath began
law svg_close_returns:
  for p: SVG.Pen
  {SVG.Pen.cur(SVG.Pen.close(p)) == SVG.Pen.start(p) : SVG.Pt}

# a moveto starts a new subpath where it lands
law svg_move_starts:
  for p: SVG.Pen
  for +x: F32
  for +y: F32
  {SVG.Pen.start(SVG.Pen.move(p, x, y)) == SVG.Pt{x, y} : SVG.Pt}

# an absolute coordinate ignores the pen
law svg_absolute_ignores_pen:
  for +p: F32
  for +v: F32
  {SVG.Svg.at(False{}, p, v) == v : F32}

# extra pairs after a moveto are linetos
law svg_implicit_lineto:
  {SVG.Svg.kinds(SVG.Svg.parse("m1 2 3 4 5 6z")) == [77, 76, 76, 90] : List<&2, U32>}

# compact numbers split where SVG says: "1-2.5.5e1" is 1, -2.5, .5e1
law svg_numbers_split:
  {List.length(&2, SVG.Tok, SVG.Svg.lex("M1-2.5.5e1")) == 4n : Nat}

# every icon the UIs draw is in the set
law icons_used_exist:
  {ICO.Icon.missing(ICO.Icon.used()) == Nil{} : List<&2, String>}

# every path of every icon parses and begins with a moveto
law icons_all_draw:
  {ICO.Icon.bad(ICO.Icon.all()) == Nil{} : List<&2, String>}

# every button the thread header can show has an icon in the set
law tools_icons_exist:
  {TOOL.Tools.missing() == Nil{} : List<&2, String>}

# and a label and a sentence saying what it does beside its icon
law tools_labelled:
  {TOOL.Tools.labelled(TOOL.Tools.every()) == True{} : Bool}

# the hub's own view when an agent starts in a KiCad project
# it opens the panel of a thread whose viewer has shown nothing yet
law viewer_auto_opens:
  {U.Ui.panel(U.Ui.view(U.Act.ui(U.Ui.select(U.Ui.init(0n), "t1")), H.Auto.view.json("t1", "/p/b.kicad_pcb", ""))) == True{} : Bool}

# and never one the person shut by hand
law viewer_auto_hand_shut:
  {U.Ui.panel(U.Ui.view(U.Act.ui(U.Ui.toggle_panel(U.Act.ui(U.Ui.toggle_panel(U.Act.ui(U.Ui.select(U.Ui.init(0n), "t1")))))), H.Auto.view.json("t1", "/p/b.kicad_pcb", ""))) == False{} : Bool}

# a project with no board or schematic sends nothing
law auto_view_none:
  {H.Auto.view("t1", "", "") == "" : String}

# a running turn's Stop comes first, before the thread's own buttons
law tools_stop_first:
  {List.head(&2, String, TOOL.Tools.icons(TOOL.Tools.head(TOOL.Head{True{}, False{}, False{}, False{}, False{}, False{}, False{}, False{}, False{}}, "", "", "", ""))) == Some{"stop"} : Maybe<&2, String>}

# Hotkeys
# -------

# alt+1..9 picks a project everywhere, the terminal included
law hotkey_alt_digit:
  {N.Keys.name(49, 4, True{}) == "project" : String}

# ctrl+1..9 picks a thread of this project
law hotkey_ctrl_digit:
  {N.Keys.name(57, 2, False{}) == "jump" : String}

# pressing alt shows the project numbers, whatever was shown before
law hint_alt_starts:
  for +h: String
  for +mods: U32
  {N.Hint.down(h, 65513, mods) == "alt" : String}

# a key pressed with the hint's modifier up ends the hint (its release was
# missed), so numbers never stick
law hint_missed_release_ends:
  {N.Hint.down("ctrl", 97, 0) == "" : String}

# Editing and threads
# -------------------

# a space typed at the end of the composer stays on its line, so the caret
# moves past it at once (not only once the next character comes)
law wrap_keeps_last_space:
  {X.Text.wrap(Map.new(&2, X.TG), X.Text.bitmap(), "hello ", 400) == ["hello "] : List<&2, String>}

# ctrl+a selects the whole text: the caret at its end, the other end at 0
law edit_select_all:
  {N.Ed.key(97, 0, 2, "abc", 1n, 1n) == N.Ed{"abc", 3n, 0n, False{}, False{}, "", 0n} : N.Ed}

# typing over a selection replaces it
law edit_type_replaces:
  {N.Ed.key(120, 120, 0, "hello world", 6n, 11n) == N.Ed{"hello x", 7n, 7n, False{}, True{}, "", 1n} : N.Ed}

# a first Delete only asks: nothing reaches the hub until it is confirmed
law delete_asks_first:
  for u: U.Ui
  {U.Act.cmds(U.Ui.delete.k(False{}, u)) == Nil{} : List<&2, U.Cmd>}

# deleting from a row (a swipe on the phone) asks first too
law row_delete_asks_first:
  for u: U.Ui
  for +id: String
  {U.Act.cmds(U.Ui.row_delete.k(False{}, u, id)) == Nil{} : List<&2, U.Cmd>}

# a right-click on a row the user settled brings it back
law row_toggle_unsettles:
  for t: M.Thread
  for +at: Nat
  for h: {M.Thread.hidden(t) == False{} : Bool}
  {U.Ui.row_toggle.m(M.Shelf.of(M.Thread.settled(True{}, at, t))) == "thread.unsettle" : String}

# and on a row the user brought back it settles it again
law row_toggle_settles:
  for +at: Nat
  for t: M.Thread
  {U.Ui.row_toggle.m(M.Shelf.of(M.Thread.unsettled(at, t))) == "thread.settle" : String}

# an answer to anything but the project picker's newest request leaves the
# client as it was (a late listing never replaces a newer one)
law picker_drops_stale:
  for u: U.Ui
  for +o: J.Json
  {U.Proj.reply.k(0n, u, o) == u : U.Ui}

# the timeline a frame draws, from the memo the window brings up to date
# before it (Lay.memo), is the timeline drawn from scratch
law tl_frame_exact:
  for +ty: L.Ty
  for +es: List<&2, M.Entry>
  for +old: List<&2, M.Entry>
  for +px0: U32
  for +sel0: String
  for +st: M.State
  for +sel: String
  for +px: U32
  for +pics: Map<&2, String>
  {L.TL.memo.shown(ty, L.TL.memo.of(ty, es, px, sel, L.TLMemo{px0, sel0, L.TL.memo.make(ty, old, px0, sel0), L.TL.live.none()}), st, sel, px, pics) == L.TL.entries(ty, es, st, sel, px, pics) : List<&2, L.TLine>}

# Tool calls folded
# -----------------

# folding a thread's tool calls into runs loses no entry and keeps the order
law fold_keeps_entries:
  for es: List<&2, M.Entry>
  {FO.Fold.flat(FO.Fold.groups(es)) == es : List<&2, M.Entry>}

# so the groups hold exactly as many entries as the thread
law fold_count:
  for es: List<&2, M.Entry>
  {FO.Fold.count(FO.Fold.groups(es)) == List.length(&2, M.Entry, es) : Nat}

# a folded run only ever holds tool calls
law fold_runs_only_tools:
  for es: List<&2, M.Entry>
  {FO.Fold.all(FO.Fold.runs(FO.Fold.groups(es))) == True{} : Bool}

# errors and messages are never folded
law fold_never_error:
  for +i: String
  for +t: String
  for +k: String
  for +s: String
  for +a: Nat
  {FO.Fold.tool(M.Act{i, t, "error", k, s, a}) == False{} : Bool}

law fold_never_message:
  for +i: String
  for +t: String
  for +r: M.Role
  for +x: String
  for +a: Nat
  {FO.Fold.tool(M.Msg{i, t, r, x, a}) == False{} : Bool}

# a run the user opened stays open and one they shut stays shut, live or
# not; one nobody touched is open only while live (the last thing in a
# thread whose turn runs), so it shuts once the turn ends
law fold_user_opened:
  for +live: Bool
  {FO.Fold.open("1", live) == True{} : Bool}

law fold_user_shut:
  for +live: Bool
  {FO.Fold.open("0", live) == False{} : Bool}

law fold_default_live:
  for +live: Bool
  {FO.Fold.open("", live) == live : Bool}

# a message is never folded away: it shows as itself wherever it sits
law fold_roles_message_shown:
  for +i: String
  for +th: String
  for +r: M.Role
  for +x: String
  for +a: Nat
  for +t: List<&2, M.Entry>
  for +info: Map<&2, String>
  for +running: Bool
  {List.head(&2, FO.Role, FO.Fold.roles(M.Msg{i, th, r, x, a} <> t, info, running)) == Some{FO.RShow{}} : Maybe<&2, FO.Role>}

# a turn's work folds under its message: the message itself is never in
# it, and while the turn runs its work stays open until shut by hand
law work_message_shown:
  for live: Bool
  for +u: M.Entry
  for +b: List<&2, M.Entry>
  for +info: Map<&2, String>
  for +now: Nat
  {List.head(&2, FO.WRole, FO.Work.turn(live, u, b, info, now)) == Some{FO.WPlain{}} : Maybe<&2, FO.WRole>}

law work_live_open:
  for +u: M.Entry
  for +e: M.Entry
  for +t: List<&2, M.Entry>
  for +now: Nat
  {FO.Work.turn(True{}, u, e <> t, Map.new(&2, String), now)
    == FO.WPlain{} <> FO.WHead{FO.Work.id(u), "Working · started " ++ FO.Work.ago(M.Time.ago(now, FO.Work.at(u))), True{}} <> FO.Work.ins(t, True{}) : List<&2, FO.WRole>}

# "/btw <question>" asks beside the turn: only the word itself starts one,
# and the hub answers it with a job alone, logging nothing, so the thread
# never holds it
law btw_command:
  {U.Btw.is("/btw what changed?") == True{} : Bool}

law btw_not_a_word:
  {U.Btw.is("/btwx") == False{} : Bool}

law btw_question:
  {U.Btw.q("/btw  what changed? ") == "what changed?" : String}

law btw_never_logged:
  for +s: M.State
  for +c: U32
  for +id: String
  for +th: String
  for +q: String
  {H.Hub.btw(s, c, id, th, q) == H.Step{[], [H.Git{c, id, "btw", th, "", q, HD.Hand.btw(H.Hub.entries(s), th, HD.Hand.title(H.Hub.thread(s, th)), q)}]} : H.Step}

# without the Backplane KiCad fork there is no IPC to turn on: Settings
# says so and offers no switch
law kicad_ipc_needs_fork:
  {U.KiCad.ipc.ready(Map.new(&2, String)) == False{} : Bool}

# without a kicad-cli on the hub's machine, Settings says KiCad is missing
law kicad_missing_said:
  {U.KiCad.cli.label(Map.new(&2, String)) == "KiCad CLI: not found" : String}

# the agents are told when KiCad is not installed
law kicad_missing_prompt:
  {String.starts_with(PM.Prompt.kicad("", False{}, "", "", False{}), "<kicad_runtime>KiCad is not installed") == True{} : Bool}

# every Claude thread loads each KiStack directory as a plugin, so the
# skills are its own besides being listed in its prompt
law claude_kistack_plugin:
  {PM.Claude.plugins(["/k/rev"]) == ["--plugin-dir", "/k/rev"] : List<&2, String>}

# the KiStack skills are loaded or listed, never spelled out in the prompt
law runtime_no_kistack_block:
  {String.contains(PM.Prompt.runtime("Backplane"), "<kistack_skills>") == False{} : Bool}

# a part's render from a view goes to renders/ beside it, named by view
law mech_render_path:
  {MCH.Mech.render("mech/bracket.step", "iso") == "mech/renders/bracket.iso.png" : String}

# a part lists the renders there are of it, and nothing else is a part
law mech_part_renders:
  {List.length(&2, MCH.MFile, MCH.Mech.part.shots.of(MCH.Mech.parts("1 9 mech/a.step\n2 9 mech/renders/a.iso.png\n3 9 mech/renders/a.top.png\n4 9 mech/notes.png"))) == 2n : Nat}

# a render's image key carries its checksum, and names its file
law mech_key_path:
  {MCH.Mech.key.path("v:123:/p/mech/renders/a.iso.png") == "/p/mech/renders/a.iso.png" : String}

# Mech opens on the assembly when there is one, else the first part
law mech_default_assembly:
  {MCH.Mech.part.path(Maybe.default(&2, MCH.MPart, MCH.Mech.default(MCH.Mech.parts("1 9 mech/bracket.step\n2 9 mech/case-assembly.step\n3 9 mech/lid.step")), MCH.MPart{"", "", Nil{}})) == "mech/case-assembly.step" : String}

law mech_default_first:
  {MCH.Mech.part.path(Maybe.default(&2, MCH.MPart, MCH.Mech.default(MCH.Mech.parts("1 9 mech/bracket.step\n3 9 mech/lid.step")), MCH.MPart{"", "", Nil{}})) == "mech/bracket.step" : String}

# agents are pointed at FreeCAD's GUI beside its console build (the only
# one that writes a STEP's colours), on Linux and in the macOS app
law mech_gui_linux:
  {PM.Prompt.mech.gui("/usr/bin/freecadcmd") == "/usr/bin/freecad" : String}

law mech_gui_mac:
  {PM.Prompt.mech.gui("/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd") == "/Applications/FreeCAD.app/Contents/MacOS/FreeCAD" : String}

# PNG chunks carry the standard CRC-32 (IEND's is AE426082)
law png_crc_iend:
  {PNG.Png.crc(PNG.Png.ascii("IEND")) == 2923585666 : U32}

# without FreeCAD on the hub's machine, Settings and the agents say so
law freecad_missing_said:
  {U.FreeCAD.label(Map.new(&2, String)) == "FreeCAD: not found" : String}

law freecad_missing_prompt:
  {String.starts_with(PM.Prompt.mech.freecad(True{}, ""), "FreeCAD is not installed") == True{} : Bool}

# how long a turn took, as its row says it
law took_seconds:
  {M.Time.took(45n) == "45s" : String}

law took_minutes:
  {M.Time.took(133n) == "2m 13s" : String}

law took_hours:
  {M.Time.took(3900n) == "1h 5m" : String}

# every frame the hub sends is binary (opcode 2, final): CBOR, never text
law hub_frames_cbor:
  for +t: String
  {List.head(&2, U32, WS.Ws.cbor(t)) == Some{130} : Maybe<&2, U32>}

# a plot for a phone goes as a binary frame too, never text
law hub_plot_frames_binary:
  for s: String
  {List.head(&2, U32, WS.Ws.bin(s)) == Some{130} : Maybe<&2, U32>}

# The phones' board viewer
# ------------------------
# The hub sends a phone a board or schematic as chunks of geometry, and
# each later version only as a delta against what that phone holds.

# what a phone rebuilds from a delta against the chunks it holds is
# exactly the new plot, whatever it held before
law plot_delta_exact:
  for ns: List<&2, PL.Chunk>
  for +old: List<&2, PL.Chunk>
  {PL.Plot.apply(PL.Plot.delta(ns, old), old) == ns : List<&2, PL.Chunk>}

# re-reading an unchanged board sends no geometry: every chunk goes as the
# index of one the phone holds (and so nothing fades in again)
law plot_unchanged_sends_nothing:
  for +cs: List<&2, PL.Chunk>
  {PL.Plot.fresh(PL.Plot.delta(cs, cs)) == 0n : Nat}

# an item on a layer the viewers hide is never sent
law plot_hidden_not_sent:
  for +key: String
  for +sig: U32
  for +layer: U32
  for +p: B.Prim
  for +info: String
  for +kind: U32
  for +acc: List<&2, PL.Piece>
  for h: {PL.Piece.alpha(kind, layer) == 0 : U32}
  {PL.Piece.put(B.Item{key, sig, layer, p, info}, kind, acc) == acc : List<&2, PL.Piece>}

# the courtyards are such a layer on a board
law plot_courtyard_hidden:
  {PL.Piece.alpha(0, 17) == 0 : U32}

# a schematic's coloured box goes to the phones in its own colour, as the
# window draws it (a tint's code is its fill chunk's width), and a tint
# once packed packs again the same
law plot_tint_own_colour:
  {PL.Chunk.tint(True{}, 1, PL.Rank.of(1, 0, B.Tint.code(12386304, 16), 0)) == (12386304, 16) : U32 & U32}

law tint_code_again:
  {B.Tint.code(B.Tint.color(1523716), B.Tint.alpha(1523716)) == 1523716 : U32}

# numbers survive the wire exactly: small and large, and negatives as
# two's complement
law plot_numbers_exact:
  {PL.Dec.nums(PL.Enc.text([0, 1, 31, 32, 1024, 123456789, 4294967295])) == [0, 1, 31, 32, 1024, 123456789, 4294967295] : List<&2, U32>}

# a closed viewer wants no source, in any project
law view_closed_wants_nothing:
  for +root: String
  {MV.View.key("", root) == "" : String}

# a viewer the hub is already showing its source asks the hub nothing
law view_asks_once:
  for +u: U.Ui
  for +cmds: List<&2, U.Cmd>
  for h: {String.eq(MV.View.want(u), U.Ui.scratch(u, "@view.at")) == True{} : Bool}
  {MV.View.sync.u(u, cmds) == U.Act{u, cmds} : U.Act}

# a plot goes to a phone as one binary frame, as every hub frame, whatever
# its bytes (the hub never parses it again)
law plot_frames_binary:
  for +bs: List<&2, U32>
  {List.head(&2, U32, WS.Ws.encode(2, bs)) == Some{130} : Maybe<&2, U32>}

# Inspecting on a phone
# ---------------------

# a tap on nothing asks the hub nothing (and closes any card)
law pick_nothing_there:
  for +u: U.Ui
  {U.Act.cmds(MV.View.pick(u, "[]")) == Nil{} : List<&2, U.Cmd>}

# with no pick waiting, an answer changes nothing: a late one never shows
# a stale card
law pick_late_ignored:
  for +u: U.Ui
  for +o: J.Json
  for h: {String.is_empty(U.Ui.scratch(u, "@pick.rid")) == True{} : Bool}
  {MV.View.picked(u, o) == u : U.Ui}

# a pad wins over the zone it sits in, whichever the phone found first
law pick_pad_over_zone:
  {MV.Pick.best([MV.Cand{"zone", 900, 12}, MV.Cand{"pad 1", 4, 12}], None{}) == Some{MV.Cand{"pad 1", 4, 12}} : Maybe<&2, MV.Cand>}

law pick_pad_over_zone_after:
  {MV.Pick.best([MV.Cand{"pad 1", 4, 12}, MV.Cand{"zone", 900, 12}], None{}) == Some{MV.Cand{"pad 1", 4, 12}} : Maybe<&2, MV.Cand>}

# of two the same size, the one drawn on top wins
law pick_top_on_tie:
  {MV.Pick.best([MV.Cand{"F", 4, 12}, MV.Cand{"B", 4, 5}], None{}) == Some{MV.Cand{"F", 4, 12}} : Maybe<&2, MV.Cand>}

# The 3D view
# -----------

# a 3D view lays the project's own board plot on the board's faces
law view_3d_draws_board:
  for +root: String
  {MV.View.layers("3d", root) == MV.View.key("board", root) : String}

law view_3d_key:
  {MV.View.key("3d", "/p") == "2|/p|" : String}

# a sheet is part of a schematic's source and of nothing else's, and the
# 3D model without its parts is the hub's kind 3
law view_sheet_key:
  {MV.View.key.at("schematic", "/p", "/p/amp.kicad_sch", False{}) == "1|/p|/p/amp.kicad_sch" : String}

law view_board_no_sheet:
  {MV.View.key.at("board", "/p", "/p/amp.kicad_sch", False{}) == "0|/p|" : String}

# a board's version (the design history) is its source
law view_board_version_key:
  {MV.View.key.at("board", "/p", "git\tg\tc\t2\t/p/a.kicad_pcb", False{}) == "0|/p|git\tg\tc\t2\t/p/a.kicad_pcb" : String}

law view_bare_key:
  {MV.View.key.at("3d", "/p", "", True{}) == "3|/p|" : String}

# a layer the user turned off on the phone is never what a tap picks
law pick_off_skipped:
  {MV.Pick.shown([MV.Cand{"pad", 4, 12}, MV.Cand{"zone", 900, 5}], V.Look.toggle(0, 12)) == [MV.Cand{"zone", 900, 5}] : List<&2, MV.Cand>}

# 3D boards
# ---------

# a model path's ${VARS} come from the board's variables
law pcb3d_path_vars:
  {P3.Env.sub("${KICAD10_3DMODEL_DIR}/R.3dshapes/R_0402.step", [P3.KV{"KIPRJMOD", "/b"}, P3.KV{"KICAD10_3DMODEL_DIR", "/lib"}])
    == "/lib/R.3dshapes/R_0402.step" : String}

# a variable the board does not know stays as written (the file is then
# not found, rather than found in the wrong place)
law pcb3d_path_unknown_kept:
  {P3.Env.sub("${NOPE}/x.step", [P3.KV{"KIPRJMOD", "/b"}]) == "${NOPE}/x.step" : String}

# triangles there from the start never fade
law mesh_first_never_fades:
  for +now: Nat
  {MS.Fade.alpha(0n, now) == 255 : U32}

# parts that arrive with the board (within 350 ms) do not fade in
law scene3_prompt_parts_no_fade:
  {FA.Scene3.fading(FA.Scene3.add(FA.Scene3.board(FA.FNone{}, 5000n), [G.Tri3{0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 255}], 5100n), 5101n)
    == False{} : Bool}

# parts that arrive late fade in
law scene3_late_parts_fade:
  {FA.Scene3.fading(FA.Scene3.add(FA.Scene3.board(FA.FNone{}, 5000n), [G.Tri3{0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 255}], 7000n), 7001n)
    == True{} : Bool}

# and the fade ends
law scene3_fade_ends:
  {FA.Scene3.fading(FA.Scene3.add(FA.Scene3.board(FA.FNone{}, 5000n), [G.Tri3{0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 255}], 7000n), 7450n)
    == False{} : Bool}

# parts from an older read of the board are dropped
law scene3_stale_parts_dropped:
  {MS.Model.size(FA.Scene3.model(FA.Scene3.add_if(FA.Scene3.board(FA.FNone{}, 5000n), 4000n, [G.Tri3{0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 255}], 7000n)))
    == 0n : Nat}

# a camera fitted about a point turns about that point, whatever it shows
law cam_fit_turns_about:
  for b: MS.Box3
  for +px: F32
  for +py: F32
  for +pz: F32
  for c: MS.Cam
  {MS.Cam.pivot(MS.Cam.of_box.at(b, px, py, pz, c)) == (px, py, pz) : F32 & F32 & F32}

# a board turns about the centre of its bounding rectangle, halfway
# through its thickness, wherever its parts are
law scene3_board_pivot:
  for +x0: F32
  for +y0: F32
  for +x1: F32
  for +y1: F32
  for +thick: F32
  for m: MS.Model
  for c: MS.Cam
  {MS.Cam.pivot(FA.Face3.refit.q(True{}, x0, y0, x1, y1, thick, m, c))
    == (F32.mul(0.5, F32.add(x0, x1)), F32.mul(0.5, thick), F32.mul(0.5, F32.add(y0, y1))) : F32 & F32 & F32}
# The model picker
# ----------------
# Its pieces come from core/menu.bend for whatever models and effort
# levels the CLIs report, so a new model can never make two collide.

# model rows stack: each starts where the one above it ends
law menu_rows_apart:
  for n: Nat
  for +x: Nat
  for +y: Nat
  for +w: Nat
  {MN.Boxes.apart(MN.Menu.rows(n, x, y, w)) == True{} : Bool}

# effort chips never overlap, however many and however wide, in a menu of
# any width: each is right of the one before it or on a line below
law menu_chips_apart:
  for ws: List<&2, Nat>
  for +left: Nat
  for +right: Nat
  for +x: Nat
  for +y: Nat
  for +h: Nat
  for +gap: Nat
  {MN.Boxes.apart(MN.Flow.place(ws, left, right, x, y, h, gap)) == True{} : Bool}

# and all of them end above the first model row
law menu_chips_above_rows:
  for ws: List<&2, Nat>
  for +left: Nat
  for +right: Nat
  for +x: Nat
  for +y: Nat
  for +h: Nat
  for +gap: Nat
  {MN.Boxes.above(MN.Flow.place(ws, left, right, x, y, h, gap), MN.Flow.under(ws, left, right, x, y, h, gap)) == True{} : Bool}

# the rows shown take no more height than there is (the menu never covers
# the thread's header; the rest scroll)
law menu_rows_fit:
  for +n: Nat
  for +room: Nat
  {Nat.is_ge(room, MN.Menu.span(n, room)) == True{} : Bool}

# scrolled, the rows shown never run past the last model
law menu_scroll_in_list:
  for +off: Nat
  for +n: Nat
  for +shown: Nat
  for h: {Nat.is_ge(n, shown) == True{} : Bool}
  {Nat.is_ge(n, Nat.add(shown, MN.Menu.off(off, n, shown))) == True{} : Bool}

# the menu is never wider than the room it opens in
law menu_width_fits:
  for +want: Nat
  for +room: Nat
  {Nat.is_ge(room, MN.Menu.width(want, room)) == True{} : Bool}

# a model's label starts past the name column (names are clipped to it)
law menu_label_clear:
  for +name_x: Nat
  for +widest: Nat
  for +w: Nat
  {Nat.is_ge(MN.Menu.label_x(name_x, MN.Menu.col(widest, w)), Nat.add(name_x, MN.Menu.col(widest, w))) == True{} : Bool}

# every press in the window first closes a menu it lands outside of
law menu_press_first:
  for +n: N.Nui
  for +x: U32
  for +y: U32
  {N.Nui.event(n, W.WButton{x, y, 1, True{}}) == N.Nui.press(N.Nui.in_viewer(n, x), N.Menu.press(N.Hit.find(N.Nui.hits(n), x, y), n), x, y) : N.Step}

# a right-click too
law menu_right_press_first:
  for +n: N.Nui
  for +x: U32
  for +y: U32
  {N.Nui.event(n, W.WButton{x, y, 3, True{}}) == N.Row.menu(N.Hit.find(N.Nui.hits(n), x, y), N.Menu.press(N.Hit.find(N.Nui.hits(n), x, y), n), x, y) : N.Step}

# a press on nothing is outside
law menu_press_nothing:
  {N.Menu.inside(None{}) == False{} : Bool}

# so is a press on anything that is not the menu's own
law menu_press_elsewhere:
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +a: String
  for +v: String
  for h: {N.Menu.own(a, v) == False{} : Bool}
  {N.Menu.inside(Some{N.Hit{x0, y0, x1, y1, a, v}}) == False{} : Bool}

# the picker's own pieces keep it open: any model row, effort chip, or its box
law menu_press_rows_inside:
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +v: String
  {N.Menu.inside(Some{N.Hit{x0, y0, x1, y1, "model", v}}) == True{} : Bool}

law menu_press_box_inside:
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +v: String
  {N.Menu.inside(Some{N.Hit{x0, y0, x1, y1, "menu", v}}) == True{} : Bool}

law menu_press_chips_inside:
  for +x0: U32
  for +y0: U32
  for +x1: U32
  for +y1: U32
  for +v: String
  {N.Menu.inside(Some{N.Hit{x0, y0, x1, y1, "effort", v}}) == True{} : Bool}

# Project badges
# --------------

# every icon a project may take is in the set (and so draws: icons_all_draw)
law badge_icons_exist:
  {ICO.Icon.missing(BG.Badge.icons()) == Nil{} : List<&2, String>}

# a project with no colour set gets one of the tints, whatever its id
law badge_auto_tint:
  for +k: Nat
  {BG.Badge.has(BG.Badge.colors(), BG.Badge.nth(k)) == True{} : Bool}

# a colour that is not a tint falls back to the project's own
law badge_unknown_color:
  for +id: String
  {BG.Badge.color.of("mauve", id) == BG.Badge.auto(id) : String}

# a set tint wins
law badge_set_color:
  for +id: String
  {BG.Badge.color.of("teal", id) == "teal" : String}

# no icon set, or one not in the set, is the folder
law badge_default_icon:
  {BG.Badge.icon.of("") == "folder" : String}

law badge_unknown_icon:
  {BG.Badge.icon.of("rocket-ship") == "folder" : String}

# The project picker on a phone
# ------------------------------

# a closed picker draws nothing
law mpick_closed_draws_nothing:
  for +u: U.Ui
  for h: {U.Ui.flag(u, "proj") == False{} : Bool}
  {MP.MPick.json(u) == J.Null{} : J.Json}

# typing within the folder already asked for only filters: the hub is
# asked nothing
law mpick_filter_asks_nothing:
  for +u: U.Ui
  for +text: String
  for h: {String.eq(MP.MPick.dir(text), U.Ui.scratch(u, "@mpick.asked")) == True{} : Bool}
  {MP.MPick.type(u, text) == U.Act{U.Ui.scratch_set(u, "@mpick", text), Nil{}} : U.Act}

# it opens at home, and a folder under home shows as ~
law mpick_opens_home:
  {MP.MPick.dir("~/") == "~/" : String}

law mpick_tilde:
  {MP.MPick.field("/home/h", "/home/h/docs") == "~/docs/" : String}

# Thread titles
# -------------

# a written title never replaces a title the user gave the thread meanwhile
law title_user_rename_wins:
  for +current: String
  for +placeholder: String
  for +answer: String
  for +thread: String
  for +now: Nat
  for h: {String.eq(current, placeholder) == False{} : Bool}
  {H.Title.changes(current, placeholder, answer, thread, now) == [] : List<&2, M.Change>}

# while the first message's placeholder stands, the cleaned answer is the title
law title_applies_when_placeholder:
  for +current: String
  for +placeholder: String
  for +answer: String
  for +thread: String
  for +now: Nat
  for same: {String.eq(current, placeholder) == True{} : Bool}
  for set: {String.is_empty(placeholder) == False{} : Bool}
  for some: {String.is_empty(AG.Title.clean(answer)) == False{} : Bool}
  {H.Title.changes(current, placeholder, answer, thread, now) == [M.ThreadRenamed{thread, AG.Title.clean(answer), now}] : List<&2, M.Change>}

# an empty answer (the writer failed) changes nothing
law title_empty_keeps:
  for +current: String
  for +placeholder: String
  for +thread: String
  for +now: Nat
  {H.Title.changes(current, placeholder, "", thread, now) == [] : List<&2, M.Change>}

# no text generation settings: Claude's haiku writes
law text_model_default:
  {AG.Text.of(M.State.empty()) == AG.Tx{"claude", "haiku"} : AG.Tx}

# an unknown provider is claude; an empty model is the provider's light one
law text_model_unknown_provider:
  {AG.Text.pick("gemini", "") == AG.Tx{"claude", "haiku"} : AG.Tx}

law text_model_light:
  {AG.Text.pick("codex", " ") == AG.Tx{"codex", "gpt-6-luna"} : AG.Tx}

# a text.model left over from another provider gives way to the chosen
# provider's light model
law text_model_mismatch:
  for +provider: String
  for +model: String
  for h: {AG.Text.fits(AG.Text.norm(String.trim(provider)), String.trim(model)) == False{} : Bool}
  {AG.Text.pick(provider, model) == AG.Tx{AG.Text.norm(String.trim(provider)), AG.Text.light(AG.Text.norm(String.trim(provider)))} : AG.Tx}

law text_model_mismatch_codex:
  {AG.Text.pick("codex", "haiku") == AG.Tx{"codex", "gpt-6-luna"} : AG.Tx}

law text_model_mismatch_claude:
  {AG.Text.pick("claude", "gpt-6-luna") == AG.Tx{"claude", "haiku"} : AG.Tx}

# a model the provider owns, or one no provider is known to own, is kept
law text_model_kept:
  {AG.Text.pick("claude", "claude-sonnet-5") == AG.Tx{"claude", "claude-sonnet-5"} : AG.Tx}

# an agent's viewer_show opens the viewer only from the thread on screen:
# another thread's request waits in that thread's view
law view_other_stays_shut:
  for u: U.Ui
  for +th: String
  for +o: J.Json
  for p: {U.Ui.panel(u) == False{} : Bool}
  for h: {String.eq(th, U.Ui.sel(u)) == False{} : Bool}
  {U.Ui.panel(U.Ui.view.k(u, th, o)) == False{} : Bool}

# no two threads share a browser page: a thread no bot owns drives the
# page named by the thread
law browser_page_own:
  for +th: String
  {BOT.World.page(BOT.World.empty(), th) == th : String}

# Hierarchical schematics
# -----------------------
# The window reads the sheet tree from the root schematic; these hold for
# every tree it builds.

# a sheet file written as an absolute path is used as written
law hier_resolve_absolute:
  for +parent: String
  for +file: String
  for h: {String.starts_with(file, "/") == True{} : Bool}
  {HI.Hier.resolve(parent, file) == file : String}

# any other is beside its parent
law hier_resolve_beside:
  {HI.Hier.resolve("/p/io/io.kicad_sch", "adc.kicad_sch") == "/p/io/adc.kicad_sch" : String}

# a sheet that includes an ancestor is listed once, marked, and never
# expanded, so a recursive design still gives a finite tree
law hier_loop_listed_once:
  {HI.Hier.rows([HI.Kids{"/p/a", [HI.Sheet{"b", "/p/b"}]}, HI.Kids{"/p/b", [HI.Sheet{"a", "/p/a"}]}], "/p/a")
    == [HI.Row{0n, "a", "/p/a", False{}}, HI.Row{1n, "b", "/p/b", False{}}, HI.Row{2n, "a", "/p/a", True{}}] : List<&2, HI.Row>}

# going up from the root stays at the root, in any tree
law hier_root_is_top:
  for +rs: List<&2, HI.Row>
  {HI.Hier.parent(rs, 0n) == 0n : Nat}

# opening a sheet and going up returns to where it was opened
law hier_up_after_open:
  {HI.Hier.parent([HI.Row{0n, "top", "/p/top", False{}}, HI.Row{1n, "power", "/p/power", False{}}, HI.Row{1n, "io", "/p/io", False{}}, HI.Row{2n, "adc", "/p/adc", False{}}],
      Maybe.default(&2, Nat, HI.Hier.child([HI.Row{0n, "top", "/p/top", False{}}, HI.Row{1n, "power", "/p/power", False{}}, HI.Row{1n, "io", "/p/io", False{}}, HI.Row{2n, "adc", "/p/adc", False{}}], 2n, "adc"), 0n))
    == 2n : Nat}

# Drafts kept
# -----------

# every change to a thread's draft asks the host to write it down at once
law draft_kept:
  for u: U.Ui
  for +text: String
  {U.Act.cmds(U.Ui.draft.set(u, text)) == U.Draft.keep(U.Ui.sel(u), text) : List<&2, U.Cmd>}

# a thread's id is kept, word for word
law draft_keep_thread:
  for +text: String
  {U.Draft.keep("t1790175755-9", text) == [U.Keep{"t1790175755-9", text}] : List<&2, U.Cmd>}

# no thread, a scratch key or anything path-like is never written
law draft_keep_no_thread:
  for +text: String
  {U.Draft.keep("", text) == Nil{} : List<&2, U.Cmd>}

law draft_keep_no_scratch:
  for +text: String
  {U.Draft.keep("@del", text) == Nil{} : List<&2, U.Cmd>}

law draft_keep_no_path:
  for +text: String
  {U.Draft.keep("../x", text) == Nil{} : List<&2, U.Cmd>}

# a change the client cannot read still counts toward what it holds, so a
# reconnect asks only for what came after it; known ones count once each
law ui_seq_counts_unknown:
  {U.Ui.seq(U.Ui.recv.k(1n, U.Ui.init(0n), U.Seq.demo("changes", [C.Change.encode(M.SettingSet{"a", "1", 1n}), J.Json.obj([J.KV{"$", J.Str{"NoSuch"}}]), C.Change.encode(M.SettingSet{"b", "2", 2n})]))) == 3n : Nat}

law ui_seq_known_unchanged:
  {U.Ui.seq(U.Ui.recv.k(1n, U.Ui.init(0n), U.Seq.demo("changes", [C.Change.encode(M.SettingSet{"a", "1", 1n}), C.Change.encode(M.SettingSet{"b", "2", 2n})]))) == 2n : Nat}

law ui_seq_log_counts_unknown:
  {U.Ui.seq(U.Ui.recv.k(0n, U.Ui.init(0n), U.Seq.demo("log", [J.Json.obj([J.KV{"$", J.Str{"NoSuch"}}]), C.Change.encode(M.SettingSet{"b", "2", 2n})]))) == 2n : Nat}

# a reconnect to the same hub keeps each thread's viewer as the person
# left it; a log from another hub keeps none of it
law reset_keeps_viewer:
  {U.Ui.info(U.Ui.reset(U.Ui.info_set(U.Ui.reset(U.Ui.init(0n), U.Reset.demo("O")), "view.kind:t1", "board"), U.Reset.demo("O")), "view.kind:t1") == "board" : String}

law reset_keeps_hand:
  {U.Ui.info(U.Ui.reset(U.Ui.info_set(U.Ui.reset(U.Ui.init(0n), U.Reset.demo("O")), "panel.hand:t1", "1"), U.Reset.demo("O")), "panel.hand:t1") == "1" : String}

law reset_new_origin_drops_viewer:
  {U.Ui.info(U.Ui.reset(U.Ui.info_set(U.Ui.reset(U.Ui.init(0n), U.Reset.demo("O")), "view.kind:t1", "board"), U.Reset.demo("P")), "view.kind:t1") == "" : String}

# the project card survives a reconnect to the same hub, and is asked for
# again after any reconnect (the scratch's last ask is forgotten)
law reset_keeps_card:
  {U.Ui.info(U.Ui.reset(U.Ui.info_set(U.Ui.reset(U.Ui.init(0n), U.Reset.demo("O")), "fab.line:t1", "A lamp"), U.Reset.demo("O")), "fab.line:t1") == "A lamp" : String}

law reset_asks_card_again:
  {U.Ui.scratch(U.Ui.reset(U.Ui.scratch_set(U.Ui.reset(U.Ui.init(0n), U.Reset.demo("O")), "@fab.at", "on#t1#idle"), U.Reset.demo("O")), "@fab.at") == "" : String}

law reset_drops_results:
  {U.Ui.info(U.Ui.reset(U.Ui.info_set(U.Ui.reset(U.Ui.init(0n), U.Reset.demo("O")), "fm.list:t1", "x"), U.Reset.demo("O")), "fm.list:t1") == "" : String}

# Kept requests replay in numeric request order, with their original bytes.
# Empty entries from older saved clients are not requests.
law client_outbox_numeric_order:
  {RPO.RpcOut.values(Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "100", "fourth"), "9", "first"), "99", "third"), "10", "second")) == ["first", "second", "third", "fourth"] : List<&2, String>}

law client_outbox_legacy_empty:
  {RPO.RpcOut.values(Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "9", "first"), "10", ""), "100", "second"), "unknown", "")) == ["first", "second"] : List<&2, String>}

law client_outbox_payload_exact:
  for +request: String
  {RPO.RpcOut.values(Map.set(&2, String, Map.new(&2, String), "9", request)) == Bool.pick(List<&2, String>, String.is_empty(request), [], [request]) : List<&2, String>}

# a page's lines are those after since, oldest first; the walk stops at
# the first line at or below it
law far_feed_after_stops:
  {FR.Fr.feed.after([FR.Line{5n, "e"}, FR.Line{4n, "d"}, FR.Line{3n, "c"}, FR.Line{2n, "b"}, FR.Line{9n, "x"}], 3n, True{}, []) == [FR.Line{4n, "d"}, FR.Line{5n, "e"}] : List<&2, FR.Line>}

# a draft read back after a restart fills an empty composer and never
# replaces what is typed there
law draft_restore_fills:
  for +saved: String
  {U.Draft.pick("", saved) == saved : String}

law draft_restore_keeps_typing:
  for +c: Char
  for +t: String
  for +saved: String
  {U.Draft.pick(SCon{c, t}, saved) == SCon{c, t} : String}

# Restarts, stops and waiting asks
# --------------------------------

# a stop reaches only a running turn
law interrupt_idle_nothing:
  for s: M.State
  for now: Nat
  for +client: U32
  for +id: String
  for thread: String
  {H.Hub.interrupt.k(False{}, s, now, client, id, thread) == H.Step{[], [H.Reply.ok(client, id)]} : H.Step}

# Claude takes the interrupt on its input; a Codex or Grok turn's process
# is stopped
law interrupt_claude_input:
  for +th: String
  for +r: String
  {H.Stop.outs("claude", th, r) == [H.AgentSend{th, H.Claude.interrupt(r)}] : List<&2, H.Out>}

law interrupt_codex_stops:
  for +th: String
  for r: String
  {H.Stop.outs("codex", th, r) == [H.AgentStop{th}] : List<&2, H.Out>}

law interrupt_grok_stops:
  for +th: String
  for r: String
  {H.Stop.outs("grok", th, r) == [H.AgentStop{th}] : List<&2, H.Out>}

law interrupt_muse_stops:
  for +th: String
  for r: String
  {H.Stop.outs("muse", th, r) == [H.AgentStop{th}] : List<&2, H.Out>}

# a Muse thread's turn is a Muse process (one per turn, over MSP)
law muse_run_turn:
  for +th: String
  for +cwd: String
  for +m: String
  for +s: String
  for +r: M.RuntimeMode
  for i: M.InteractionMode
  for +t: String
  for u: String
  {H.Hub.run.k(3n, th, cwd, "muse", m, s, r, i, t, u) == [H.MuseTurn{th, cwd, m, s, r, t}] : List<&2, H.Out>}

# Muse's approval modes: supervised asks for anything its policy does not
# allow; full access allows everything (no approval reaches the person)
law msp_mode_supervised_asks:
  {MSP.Msp.mode.k(0n) == "promptUnmatched" : String}

law msp_mode_full_allows:
  {MSP.Msp.mode.k(3n) == "allowAll" : String}

# a retired process leaves nothing waiting: archive ends a running turn
# (an idle thread gets no turn change), Stop, archive and an exit close
# the asks that wait on it, a plan card stays, a delegated child's task
# ends, and the parked requests get the empty answer
law archive_running_stops:
  {H.Hub.running(H.Hub.thread(H.Retire.after(H.Rec.demo(M.Running{}), H.Retire.request(7n, H.Rec.demo(M.Running{}))), "t1")) == False{} : Bool}

law archive_idle_no_turn:
  {H.Task.ended(H.Queue.changes(H.Retire.request(7n, H.Rec.demo(M.Completed{})))) == H.NotEnded{} : H.Ended}

law archive_closes_approval:
  {H.Pend.open(M.Asks.all(H.Retire.after(H.Rec.demo(M.Running{}), H.Retire.request(7n, H.Rec.demo(M.Running{})))), "t1") == 0n : Nat}

law delete_running_stops:
  {H.Hub.running(H.Hub.thread(H.Retire.after(H.Rec.demo(M.Running{}), H.Retire.request(12n, H.Rec.demo(M.Running{}))), "t1")) == False{} : Bool}

law interrupt_closes_approval:
  {H.Pend.open(M.Asks.all(H.Retire.after(H.Rec.demo(M.Running{}), H.Hub.interrupt(H.Rec.demo(M.Running{}), 9n, 0, "r", "t1"))), "t1") == 0n : Nat}

law interrupt_keeps_plan:
  {H.Pend.open(M.Asks.all(H.Retire.after(H.Retire.demo(), H.Hub.interrupt(H.Retire.demo(), 9n, 0, "r", "t1"))), "t1") == 1n : Nat}

law interrupt_ends_child_task:
  {H.Retire.task(H.Retire.after(H.Retire.demo(), H.Hub.interrupt(H.Retire.demo(), 9n, 0, "r", "t1"))) == True{} : Bool}

law gone_closes_approval:
  {H.Pend.open(M.Asks.all(H.Retire.after(H.Rec.demo(M.Running{}), H.Hub.agent_gone(H.Rec.demo(M.Running{}), 9n, "t1", 1))), "t1") == 0n : Nat}

# Stop, archive and delete of a running turn tell the server, so the
# phones' Live Activity drops it (the hub pushes the island); an idle
# thread tells nothing
law retire_stop_tells:
  {H.Ended.stopped(H.Step.outs_of(H.Hub.interrupt(H.Rec.demo(M.Running{}), 9n, 0, "r", "t1")), "t1") == True{} : Bool}

law archive_running_tells:
  {H.Ended.stopped(H.Step.outs_of(H.Retire.request(7n, H.Rec.demo(M.Running{}))), "t1") == True{} : Bool}

law delete_running_tells:
  {H.Ended.stopped(H.Step.outs_of(H.Retire.request(12n, H.Rec.demo(M.Running{}))), "t1") == True{} : Bool}

law archive_idle_tells_nothing:
  {H.Ended.stopped(H.Step.outs_of(H.Retire.request(7n, H.Rec.demo(M.Completed{}))), "t1") == False{} : Bool}

# a turn that starts (or changes any way the step does not report as
# ended), an archive or a delete pushes the phones' island; a step that
# reports the end pushes it with the alert, not twice; nothing else does
law island_sync_on_start:
  {H.Island.outs([M.TurnChanged{"t1", M.Running{}, 1n}], []) == [H.IslandSync{}] : List<&2, H.Out>}

law island_sync_on_archive:
  {H.Island.outs([M.ThreadArchived{"t1", True{}, 1n}], []) == [H.IslandSync{}] : List<&2, H.Out>}

law island_sync_on_delete:
  {H.Island.outs([M.ThreadDeleted{"t1", 1n}], []) == [H.IslandSync{}] : List<&2, H.Out>}

law island_sync_not_twice:
  {H.Island.outs([M.TurnChanged{"t1", M.Interrupted{}, 1n}], [H.TurnEnded{"t1", "stopped"}]) == [H.TurnEnded{"t1", "stopped"}] : List<&2, H.Out>}

law island_sync_quiet:
  {H.Island.outs([M.ThreadRenamed{"t1", "x", 1n}], []) == [] : List<&2, H.Out>}

law retire_answers_empty:
  {H.Retire.answers(M.Asks.all(H.Rec.demo(M.Running{})), "t1") == [H.AskAnswered{"a1", ""}] : List<&2, H.Out>}

# Claude's process outlives a Stop: its late result leaves the turn
# stopped and its late text streams nowhere; a running turn's result
# completes it
law claude_result_after_stop_quiet:
  {H.Hub.running(H.Hub.thread(M.State.replay(H.Queue.changes(H.Hub.agent_line(H.Demo.state("claude", M.Interrupted{}), 9n, "t1", "{\"type\":\"result\",\"is_error\":false}")), H.Demo.state("claude", M.Interrupted{})), "t1")) == False{} : Bool}

law claude_result_after_stop_interrupted:
  {M.Thread.turn(H.Hub.thread(M.State.replay(H.Queue.changes(H.Hub.agent_line(H.Demo.state("claude", M.Interrupted{}), 9n, "t1", "{\"type\":\"result\",\"is_error\":false}")), H.Demo.state("claude", M.Interrupted{})), "t1")) == M.Interrupted{} : M.TurnState}

law claude_result_running_completes:
  {M.Thread.turn(H.Hub.thread(M.State.replay(H.Queue.changes(H.Hub.agent_line(H.Demo.state("claude", M.Running{}), 9n, "t1", "{\"type\":\"result\",\"is_error\":false}")), H.Demo.state("claude", M.Running{})), "t1")) == M.Completed{} : M.TurnState}

law claude_delta_after_stop_quiet:
  {H.Step.outs_of(H.Hub.agent_line(H.Demo.state("claude", M.Interrupted{}), 9n, "t1", "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"late\"}}}")) == [] : List<&2, H.Out>}

law claude_delta_running_streams:
  {List.length(&2, H.Out, H.Step.outs_of(H.Hub.agent_line(H.Demo.state("claude", M.Running{}), 9n, "t1", "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"live\"}}}"))) == 1n : Nat}

# what a stopped turn's process still says changes nothing
law stopped_turn_quiet:
  for s: M.State
  for now: Nat
  for thread: String
  for line: String
  {H.Hub.agent_line.c(False{}, s, now, thread, line) == H.Step{[], []} : H.Step}

# after a restart a turn left running is interrupted, and an open ask is
# closed, so nothing waits on what died with the old process
law recover_interrupts_running:
  {H.Hub.running(H.Hub.thread(H.Rec.after(H.Rec.demo(M.Running{}), 10n), "t1")) == False{} : Bool}

law recover_closes_asks:
  {H.Pend.open(M.Asks.all(H.Rec.after(H.Rec.demo(M.Running{}), 10n)), "t1") == 0n : Nat}

# a thread that was not running is left as it was
law recover_leaves_idle:
  {H.Hub.running(H.Hub.thread(H.Rec.after(H.Rec.demo(M.Completed{}), 10n), "t1")) == False{} : Bool}

law recover_empty_nothing:
  for now: Nat
  {H.Hub.recover(M.State.empty(), now) == H.Step{[], []} : H.Step}

# with "restart.continue" on (the default) a turn the restart cut short
# runs again; an idle thread stays idle; with it off nothing carries on;
# a thread whose newest entry is already the continue message stays
# stopped, so a turn that takes the hub down never loops
law carry_continues:
  {H.Hub.running(H.Hub.thread(H.Carry.after(H.Rec.demo(M.Running{}), 10n), "t1")) == True{} : Bool}

law carry_leaves_idle:
  {H.Hub.running(H.Hub.thread(H.Carry.after(H.Rec.demo(M.Completed{}), 10n), "t1")) == False{} : Bool}

law carry_off_nothing:
  {H.Hub.running(H.Hub.thread(H.Carry.after(H.Carry.demo([M.SettingSet{"restart.continue", "off", 4n}]), 10n), "t1")) == False{} : Bool}

law carry_once:
  {H.Hub.running(H.Hub.thread(H.Carry.after(H.Carry.demo([M.MessagePosted{"t1", "c1", M.User{}, M.Carry.text(), 4n}]), 10n), "t1")) == False{} : Bool}

# the restart note after the continue message is no progress either, so a
# second restart before the agent wrote anything leaves it stopped; an
# archived thread never carries on
law carry_twice_once:
  {H.Hub.running(H.Hub.thread(H.Carry.after(H.Carry.after(H.Rec.demo(M.Running{}), 10n), 20n), "t1")) == False{} : Bool}

law carry_skips_archived:
  {H.Hub.running(H.Hub.thread(H.Carry.after(H.Carry.demo([M.ThreadArchived{"t1", True{}, 4n}]), 10n), "t1")) == False{} : Bool}

# an open ask makes its thread pending (so it never auto-settles); the
# answer clears it
law ask_makes_pending:
  {H.Hub.pending(H.Hub.thread(M.State.replay(H.Pend.fix(M.State.replay(H.Rec.demo.base(), M.State.empty()), [M.AskOpened{"a1", "t1", "approval", "Bash", "ls", "", 3n}]), M.State.replay(H.Rec.demo.base(), M.State.empty())), "t1")) == 1n : Nat}

law answer_clears_pending:
  {H.Hub.pending(H.Hub.thread(M.State.replay(H.Pend.fix(H.Rec.demo(M.Completed{}), [M.AskClosed{"a1", "allow", 4n}]), H.Rec.demo(M.Completed{})), "t1")) == 0n : Nat}

# a parent whose delegated task still works does not settle
law settle_waits_for_children:
  {H.Kids.busy([M.Task{"k1", "t1", "t2", 1n, M.TaskRunning{}, "", False{}, "codex", ""}], "t1") == True{} : Bool}

law settle_children_done:
  {H.Kids.busy([M.Task{"k1", "t1", "t2", 1n, M.TaskDone{}, "", False{}, "codex", ""}], "t1") == False{} : Bool}

# a turn process's tag names its thread and process; only the process the
# hub runs for the thread is heard, and an untagged line always is
law tag_thread:
  {H.Tag.thread(H.Tag.of("t1790175755-9", 4242)) == "t1790175755-9" : String}

law tag_pid:
  {H.Tag.pid(H.Tag.of("t1790175755-9", 4242)) == Some{4242} : Maybe<&2, U32>}

law tag_untagged:
  {H.Tag.pid("t1790175755-9") == None{} : Maybe<&2, U32>}

law tag_old_process_unheard:
  {H.Tag.heard(Some{41}, Some{42}) == False{} : Bool}

law tag_untagged_heard:
  for r: Maybe<&2, U32>
  {H.Tag.heard(None{}, r) == True{} : Bool}

# Follow-ups
# ----------

# every message steers a running turn: no setting, provider or ctrl+Enter
# makes one wait
law follow_default_claude:
  {U.Follow.mode("", "claude", False{}) == "steer" : String}

law follow_default_codex:
  {U.Follow.mode("", "codex", False{}) == "steer" : String}

law follow_setting_queue:
  for +p: String
  {U.Follow.mode("queue", p, False{}) == "steer" : String}

law follow_setting_steer:
  for +p: String
  {U.Follow.mode("steer", p, False{}) == "steer" : String}

law follow_alt_same:
  for +s: String
  for +p: String
  for a: Bool
  {U.Follow.mode(s, p, a) == "steer" : String}

# the send button is Stop only while a turn runs with nothing typed: an
# idle thread or a typed message always sends, and Stop interrupts
law follow_stop_idle:
  for b: Bool
  {U.Follow.stop.k(False{}, b) == False{} : Bool}

law follow_stop_typed:
  for r: Bool
  {U.Follow.stop.k(r, False{}) == False{} : Bool}

law follow_stop_running:
  {U.Follow.stop.k(True{}, True{}) == True{} : Bool}

law follow_stop_interrupts:
  {U.Follow.act(True{}) == "interrupt" : String}

# an older client's "queue" mode delivers at once like any message
law queue_mode_delivers:
  for +s: M.State
  for +now: Nat
  for +th: String
  for +msg: String
  for role: M.Role
  for +text: String
  {H.Hub.post.mode("queue", s, now, th, msg, role, text) == H.Hub.post(s, now, th, msg, role, text) : H.Step}

# steering Claude sends into its running process; a Codex or Grok turn is
# stopped and starts again with the message
law steer_claude_in_place:
  for +th: String
  {H.Steer.stop("claude", th) == [] : List<&2, H.Out>}

law steer_codex_restarts:
  {List.head(&2, H.Out, H.Step.outs_of(H.Hub.post.mode("steer", H.Demo.state("codex", M.Running{}), 5n, "t1", "m1", M.User{}, "hi"))) == Some{H.AgentStop{"t1"}} : Maybe<&2, H.Out>}

# the queue, message by message: joined as it always was, edited in
# place, removed, moved
law queue_joined:
  {M.Queue.texts([M.QItem{"a", "x"}, M.QItem{"b", "y"}]) == "x\n\ny" : String}

law queue_edit_in_place:
  {M.Queue.edit([M.QItem{"a", "x"}, M.QItem{"b", "y"}], "b", "z") == [M.QItem{"a", "x"}, M.QItem{"b", "z"}] : List<&2, M.QItem>}

law queue_remove_one:
  {M.Queue.drop([M.QItem{"a", "x"}, M.QItem{"b", "y"}], "a") == [M.QItem{"b", "y"}] : List<&2, M.QItem>}

law queue_move_first:
  {M.Queue.move([M.QItem{"a", "x"}, M.QItem{"b", "y"}, M.QItem{"c", "z"}], "c", 0n) == [M.QItem{"c", "z"}, M.QItem{"a", "x"}, M.QItem{"b", "y"}] : List<&2, M.QItem>}

law queue_move_unknown:
  {M.Queue.move([M.QItem{"a", "x"}], "q", 0n) == [M.QItem{"a", "x"}] : List<&2, M.QItem>}

# steering one queued message leaves the others waiting, in order
law queue_steer_one_leaves_rest:
  {M.Queue.items(M.State.replay([M.TurnQueued{"t1", "a", "x", 1n}, M.TurnQueued{"t1", "b", "y", 1n}, M.TurnQueued{"t1", "c", "z", 1n}, M.QueueTakenOne{"t1", "b", 2n}], M.State.empty()), "t1")
    == [M.QItem{"a", "x"}, M.QItem{"c", "z"}] : List<&2, M.QItem>}

# a row's first button steers while a turn runs; with none running the
# first row can resume the held queue
law queue_row_running:
  for +m: String
  {List.head(&2, U.QBtn, U.Qu.buttons(True{}, True{}, m)) == Some{U.QBtn{"Steer", "q-steer", m}} : Maybe<&2, U.QBtn>}

law queue_row_held:
  for +m: String
  {List.head(&2, U.QBtn, U.Qu.buttons(False{}, True{}, m)) == Some{U.QBtn{"Resume", "q-resume", ""}} : Maybe<&2, U.QBtn>}

# Handed-over context and forks
# -----------------------------

# the newest entries that fit the budget are kept, oldest first; what does
# not fit is counted as left out
law hand_newest_fit:
  {HD.Hand.go([M.Msg{"3", "t1", M.User{}, "c", 3n}, M.Msg{"2", "t1", M.User{}, "b", 2n}, M.Msg{"1", "t1", M.User{}, "a", 1n}], HD.Sel{17n, [], False{}, 0n})
    == HD.Sel{1n, ["User: b", "User: c"], True{}, 1n} : HD.Sel}

# once one entry is left out, every older one is too: the kept history
# has no holes
law hand_no_holes:
  for fits: Bool
  for +line: String
  for +left: Nat
  for ls: List<&2, String>
  for +om: Nat
  {HD.Hand.put(True{}, fits, line, left, ls, om) == HD.Sel{left, ls, True{}, 1n+om} : HD.Sel}

# a long item is cut and says by how much
law hand_cut_says:
  {HD.Hand.cut("abcdef", 2n) == "ab ...[4 more characters]" : String}

# context waiting alone never starts a turn: it goes with a message
law handoff_never_runs_alone:
  {M.Queue.real([M.QItem{"handoff:t1", "ctx"}]) == False{} : Bool}

law handoff_with_message_runs:
  {M.Queue.real([M.QItem{"handoff:t1", "ctx"}, M.QItem{"m1", "hi"}]) == True{} : Bool}

# and it goes first, ahead of the user's words, which are never cut
law handoff_goes_first:
  {M.Queue.texts([M.QItem{"handoff:t1", "ctx"}, M.QItem{"m1", "hi"}]) == "ctx\n\nhi" : String}

# a fork remembers its source and waits with the source's context queued
law fork_records_source:
  {M.Fork.source(M.State.replay(H.Queue.changes(H.Hub.fork(H.Demo.state("claude", M.Completed{}), 5n, 0, "1", "t1")), H.Demo.state("claude", M.Completed{})), "t5-3") == "t1" : String}

law fork_waits_with_context:
  {M.Queue.waiting(M.State.replay(H.Queue.changes(H.Hub.fork(H.Demo.state("claude", M.Completed{}), 5n, 0, "1", "t1")), H.Demo.state("claude", M.Completed{})), "t5-3") == False{} : Bool}

# only a fork can be brought back
law back_needs_fork:
  {H.Queue.changes(H.Hub.back(H.Demo.state("claude", M.Completed{}), 5n, 0, "1", "t1")) == [] : List<&2, M.Change>}

# Switching providers
# -------------------

# a running turn is never switched, and switching to the same provider
# changes nothing
law switch_between_turns:
  {H.Queue.changes(H.Hub.switch(H.Demo.state("claude", M.Running{}), 5n, 0, "1", "t1", "codex")) == [] : List<&2, M.Change>}

law switch_same_nothing:
  {H.Queue.changes(H.Hub.switch(H.Demo.state("claude", M.Completed{}), 5n, 0, "1", "t1", "claude")) == [] : List<&2, M.Change>}

# each provider keeps its own session: a new one starts fresh, and coming
# back resumes the one it had
law switch_new_session:
  {M.Switch.sess(M.State.thread.get(M.State.replay([M.SessionSet{"t1", "S1", 3n}, M.ProviderSet{"t1", "codex", "", 4n}], H.Demo.state("claude", M.Completed{})), "t1")) == "" : String}

law switch_keeps_sessions:
  {M.Switch.sess(M.State.thread.get(M.State.replay([M.ProviderSet{"t1", "claude", "", 6n}], H.Demo.switched()), "t1")) == "S1" : String}

law switch_keeps_sessions_both:
  {M.Switch.sess(M.State.thread.get(M.State.replay([M.ProviderSet{"t1", "claude", "", 6n}, M.ProviderSet{"t1", "codex", "", 7n}], H.Demo.switched()), "t1")) == "S2" : String}

# a provider new to a thread is handed it all; one coming back that
# missed nothing is handed nothing
law switch_hands_new_provider:
  {List.length(&2, M.QItem, M.Queue.items(M.State.replay(H.Queue.changes(H.Hub.switch(H.Demo.switched(), 6n, 0, "1", "t1", "grok")), H.Demo.switched()), "t1")) == 1n : Nat}

law switch_back_missed_nothing:
  {List.length(&2, M.QItem, M.Queue.items(M.State.replay(H.Queue.changes(H.Hub.switch(H.Demo.switched(), 6n, 0, "1", "t1", "claude")), H.Demo.switched()), "t1")) == 0n : Nat}

# The orchestrator's thread tools
# -------------------------------

# thread_launch makes an ordinary thread in the caller's project
law mcp_launch_in_project:
  {List.head(&2, M.Change, MC.Res.changes(MC.Mcp.launch(H.Demo.state("claude", M.Completed{}), 5n, "t1", J.Json.obj([J.KV{"prompt", J.Str{"hi"}}]))))
    == Some{M.ThreadCreated{"t5-3", "p1", "hi", M.Worktree{}, "", "", "claude", "", 5n}} : Maybe<&2, M.Change>}

# a thread of another project is out of reach
law mcp_other_project_refused:
  for run: Bool
  for s: M.State
  for now: Nat
  for +t: String
  {MC.Res.good(MC.Mcp.interrupt.k(False{}, run, s, now, t)) == False{} : Bool}

# thread_interrupt stops only a running turn
law mcp_interrupt_idle_nothing:
  for s: M.State
  for now: Nat
  for +t: String
  {MC.Res.changes(MC.Mcp.interrupt.k(True{}, False{}, s, now, t)) == [] : List<&2, M.Change>}

# a thread is done once no turn runs and nothing waits to run
law mcp_done_idle:
  {MC.Mcp.done(H.Demo.state("claude", M.Completed{}), "t1") == True{} : Bool}

law mcp_not_done_running:
  {MC.Mcp.done(H.Demo.state("codex", M.Running{}), "t1") == False{} : Bool}

# a wait lasts between one second and ten minutes
law wait_at_least_a_second:
  {MC.Wait.clamp(0n) == 1n : Nat}

law wait_at_most_ten_minutes:
  {MC.Wait.clamp(9000000n) == 600n : Nat}
# Start-up
# --------
# The window opens before the providers are probed; until they answer,
# the hub serves the model list it found last time.

# the cached list reads back as it was written
law models_cache_json:
  for +ms: String
  for +mt: String
  {BT.Cache.json(BT.Cache.of(ms, mt)) == BT.Cache.one(ms) : String}

# and so does the prompt's text after it
law models_cache_text:
  for +ms: String
  for +mt: String
  {BT.Cache.text(BT.Cache.of(ms, mt)) == mt : String}

# a probe that finds the list clients already have tells no one
law models_quiet:
  for +ms: String
  {BT.Models.news(ms, ms) == False{} : Bool}

# the log is read in one pass, each line parsed once: the state it gives
# is exactly the replay of the log's changes
law store_state_is_replay:
  for ls: List<&2, String>
  for +a: ST.Loaded
  {ST.Loaded.st(ST.Store.load(ls, a)) == M.State.replay(ST.Store.changes(ls), ST.Loaded.st(a)) : M.State}

# and a log read in two parts is the log read whole
law store_load_append:
  for xs: List<&2, String>
  for +ys: List<&2, String>
  for +a: ST.Loaded
  {ST.Store.load(List.append(&2, String, xs, ys), a) == ST.Store.load(ys, ST.Store.load(xs, a)) : ST.Loaded}

# Frames
# ------

# a link's id names the thread it came from
law link_from_id:
  for +from: String
  for +n: String
  {M.Link.from(M.Link.id(M.Link.bare(from), n)) == M.Link.bare(from) : String}

# and any id not starting with 't' is the user's own, told at its first
# character (the timeline asks this of every entry, every frame)
law link_from_plain:
  for +c: Char
  for t: String
  for e: {Char.is_eq(c, Chr{116}) == False{} : Bool}
  {M.Link.from(SCon{c, t}) == "" : String}

# going back to a thread finds the memo it left: only what changed in it
# is laid out again (any memo is a sound start, tl_memo_exact)
law tl_memos_hit:
  for +px: U32
  for +sel: String
  for +its: List<&2, L.TLE>
  for +lv: L.LiveMemo
  for ms: List<&2, L.TLMemo>
  {L.TL.memos.pick(L.TL.memos.keep(L.TLMemo{px, sel, its, lv}, ms), sel) == L.TLMemo{px, sel, its, lv} : L.TLMemo}

# and the memos kept stay few
law tl_memos_bounded:
  for +m: L.TLMemo
  for ms: List<&2, L.TLMemo>
  {Nat.is_le(List.length(&2, L.TLMemo, L.TL.memos.keep(m, ms)), L.TL.memos.max()) == True{} : Bool}

# The hub's own window takes the read model itself, not the log as text
# (nothing to build, parse or replay at start-up). It never holds a
# device or its token, as the wire never carries them
law window_state_private:
  for s: M.State
  {M.State.devices(M.State.private(s)) == Map.new(&2, M.Device) : Map<&2, M.Device>}

# and it counts the same changes the hub does, so a reconnect asks for
# exactly what it missed
law window_state_seq:
  for s: M.State
  {M.State.seq(M.State.private(s)) == M.State.seq(s) : Nat}

# the model handed over is the one the window holds
law window_adopt:
  for u: U.Ui
  for st: M.State
  {U.Ui.state(U.Ui.adopt(u, st)) == st : M.State}

# Todo lists
# ----------

# TodoWrite sets the whole list; a step being done reads as its active form
law todo_write_whole_list:
  {TD.Todo.write(J.Json.obj([J.KV{"todos", J.Json.arr([J.Json.obj([J.KV{"content", J.Str{"Run tests"}}, J.KV{"status", J.Str{"in_progress"}}, J.KV{"activeForm", J.Str{"Running tests"}}])])}]))
    == [TD.Todo{"1", "Run tests", "Running tests", "in_progress"}] : List<&2, TD.Todo>}

law todo_line_active_form:
  {TD.Todo.line(TD.Todo{"1", "Run tests", "Running tests", "in_progress"}) == "[>] Running tests" : String}

# TaskCreate adds a step under its call's id until the result names it;
# TaskUpdate changes it, or drops it when deleted
law todo_task_created:
  {TD.Todo.named(TD.Todo.create([], "u1", J.Json.obj([J.KV{"subject", J.Str{"a"}}])), "u1", "3") == [TD.Todo{"3", "a", "", "pending"}] : List<&2, TD.Todo>}

law todo_task_result_number:
  {TD.Todo.result("Task #12 created successfully: a") == "12" : String}

law todo_task_update:
  {TD.Todo.update([TD.Todo{"3", "a", "", "pending"}], J.Json.obj([J.KV{"taskId", J.Str{"3"}}, J.KV{"status", J.Str{"completed"}}])) == [TD.Todo{"3", "a", "", "completed"}] : List<&2, TD.Todo>}

law todo_task_delete:
  {TD.Todo.update([TD.Todo{"3", "a", "", "pending"}], J.Json.obj([J.KV{"taskId", J.Str{"3"}}, J.KV{"status", J.Str{"deleted"}}])) == [] : List<&2, TD.Todo>}

# a Codex item's completed flag, and an ACP plan's status, become the step's status
law todo_codex_completed:
  {TD.Todo.codex(J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"text", J.Str{"a"}}, J.KV{"completed", J.Flag{True{}}}]), J.Json.obj([J.KV{"text", J.Str{"b"}}, J.KV{"completed", J.Flag{False{}}}])])}]))
    == [TD.Todo{"1", "a", "", "completed"}, TD.Todo{"2", "b", "", "pending"}] : List<&2, TD.Todo>}

law todo_acp_plan:
  {TD.Todo.codex(J.Json.obj([J.KV{"items", J.Json.arr(AC.Acp.plan([J.Json.obj([J.KV{"content", J.Str{"x"}}, J.KV{"priority", J.Str{"high"}}, J.KV{"status", J.Str{"in_progress"}}])]))}]))
    == [TD.Todo{"1", "x", "", "in_progress"}] : List<&2, TD.Todo>}

# the list shows while the turn runs or a step is left, and is gone once
# all is done and the turn has ended
law todo_shown_while_running:
  {TD.Todo.shown([TD.Todo{"1", "a", "", "completed"}], True{}) == True{} : Bool}

law todo_shown_while_left:
  for run: Bool
  {TD.Todo.shown([TD.Todo{"1", "a", "", "pending"}], run) == True{} : Bool}

law todo_hidden_when_done:
  {TD.Todo.shown([TD.Todo{"1", "a", "", "completed"}], False{}) == False{} : Bool}

# a long list shows the steps around the first one not done, and a line
# each for the done steps before them and the steps after them
law todo_rows_around_open:
  {TD.Todo.rows([TD.Todo{"1", "a", "", "completed"}, TD.Todo{"2", "b", "", "completed"}, TD.Todo{"3", "c", "", "completed"}, TD.Todo{"4", "d", "", "pending"}, TD.Todo{"5", "e", "", "pending"}, TD.Todo{"6", "f", "", "pending"}, TD.Todo{"7", "g", "", "pending"}, TD.Todo{"8", "h", "", "pending"}, TD.Todo{"9", "i", "", "pending"}, TD.Todo{"10", "j", "", "pending"}, TD.Todo{"11", "k", "", "pending"}, TD.Todo{"12", "l", "", "pending"}])
    == [TD.TRow{"… 2 done", "completed"}, TD.TRow{"[x] c", "completed"}, TD.TRow{"[ ] d", "pending"}, TD.TRow{"[ ] e", "pending"}, TD.TRow{"[ ] f", "pending"}, TD.TRow{"[ ] g", "pending"}, TD.TRow{"[ ] h", "pending"}, TD.TRow{"… 4 more", "pending"}] : List<&2, TD.TRow>}

# near the end the card shows the last steps, with nothing after them
law todo_rows_last:
  {List.length(&2, TD.TRow, TD.Todo.rows([TD.Todo{"1", "a", "", "completed"}, TD.Todo{"2", "b", "", "completed"}, TD.Todo{"3", "c", "", "completed"}, TD.Todo{"4", "d", "", "completed"}, TD.Todo{"5", "e", "", "completed"}, TD.Todo{"6", "f", "", "completed"}, TD.Todo{"7", "g", "", "completed"}, TD.Todo{"8", "h", "", "completed"}, TD.Todo{"9", "i", "", "completed"}, TD.Todo{"10", "j", "", "in_progress"}, TD.Todo{"11", "k", "", "pending"}, TD.Todo{"12", "l", "", "pending"}])) == 7n : Nat}

# a list that fits shows every step
law todo_rows_short:
  {TD.Todo.rows([TD.Todo{"1", "a", "", "completed"}, TD.Todo{"2", "b", "", "pending"}]) == [TD.TRow{"[x] a", "completed"}, TD.TRow{"[ ] b", "pending"}] : List<&2, TD.TRow>}

# an assistant message with a todo tool sets the thread's list, once
law todo_write_sets_list:
  {List.length(&2, M.Change, H.Claude.todos(M.State.empty(), "t1", 5n, [J.Json.obj([J.KV{"type", J.Str{"tool_use"}}, J.KV{"name", J.Str{"TodoWrite"}}, J.KV{"input", J.Json.obj([J.KV{"todos", J.Json.arr([])}])}])])) == 1n : Nat}

# Context usage
# -------------

# a reading keeps the part it does not know (0) from the last one
law usage_keeps_window:
  {M.Usage.merge("10/200", 5n, 0n) == "5/200" : String}

law usage_keeps_used:
  {M.Usage.merge("10/200", 0n, 400n) == "10/400" : String}

# the chip says the share of the window, tokens alone without a window,
# and nothing before the first reading
law usage_text_share:
  {U.Usage.text(50n, 200n) == " · ctx 25% of 200" : String}

law usage_text_tokens:
  {U.Usage.text(5000n, 0n) == " · ctx 5k" : String}

law usage_nothing_yet:
  for +w: Nat
  {U.Usage.text(0n, w) == "" : String}

# a reading goes to the log only when it changes
law usage_same_nothing:
  {H.Usage.used.k(M.State.replay([M.UsageSet{"t1", 7n, 0n, 1n}], M.State.empty()), "t1", 2n, 7n) == [] : List<&2, M.Change>}

law usage_change_logged:
  {H.Usage.used.k(M.State.replay([M.UsageSet{"t1", 7n, 0n, 1n}], M.State.empty()), "t1", 2n, 8n) == [M.UsageSet{"t1", 8n, 0n, 2n}] : List<&2, M.Change>}

# The 3D viewer's controls (SolidWorks)
# -------------------------------------

# a drag rotates; with ctrl it pans, with shift zooms, with alt rolls
law view_drag_rotates:
  {N.Drag.mode(0) == 0 : U32}

law view_drag_ctrl_pans:
  {N.Drag.mode(2) == 1 : U32}

law view_drag_shift_zooms:
  {N.Drag.mode(1) == 2 : U32}

law view_drag_alt_rolls:
  {N.Drag.mode(4) == 3 : U32}

# 1-7 are the standard views (5 top, 7 isometric); ctrl+digit stays the
# thread switch
law view_key_top:
  {N.Key3.act(53, 0) == 5 : U32}

law view_key_iso:
  {N.Key3.act(55, 0) == 7 : U32}

law view_key_ctrl_digit_free:
  {N.Key3.act(49, 2) == 0 : U32}

# arrows turn 15 degrees, 90 with shift; alt rolls, ctrl pans
law view_key_arrow_turns:
  {N.Key3.act(65361, 0) == 10 : U32}

law view_key_shift_arrow_quarter:
  {N.Key3.act(65361, 1) == 14 : U32}

law view_key_alt_arrow_rolls:
  {N.Key3.act(65363, 4) == 19 : U32}

law view_key_ctrl_arrow_pans:
  {N.Key3.act(65364, 2) == 23 : U32}

# z zooms out, shift+z in; other letters do nothing
law view_key_z_out:
  {N.Key3.act(122, 0) == 8 : U32}

law view_key_shift_z_in:
  {N.Key3.act(90, 1) == 9 : U32}

law view_key_other_none:
  {N.Key3.act(97, 0) == 0 : U32}
# Signed requests
# ---------------

# a webhook or peer request outside the clock window is refused, whatever
# its signature
law sign_stale_rejected:
  for secret: String
  for +now: Nat
  for +ts: Nat
  for body: String
  for sig: String
  for h: {SG.Sign.fresh(now, ts) == False{} : Bool}
  {SG.Sign.check(secret, now, ts, body, sig) == False{} : Bool}

# the window is five minutes
law sign_window_300:
  {SG.Sign.window() == 300n : Nat}

# a key just accepted is in the replay cache, so it is refused next time
law seen_add_has:
  for s: SG.Seen
  for +k: String
  for +now: Nat
  {SG.Seen.has(SG.Seen.add(s, k, now), k) == True{} : Bool}

# an invite reads back as what made it, colons in its url and all
law invite_roundtrip:
  {SG.Invite.parse(SG.Invite.make("http://host:3787", "p1", "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"))
    == Some{SG.Inv{"http://host:3787", "p1", "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"}} : Maybe<&2, SG.Inv>}
# Routine schedules
# -----------------

# a routine's next time is after the time asked about, or 0 (never)
law cron_next_after:
  for +c: CR.Cron
  for +after: Nat
  for +off: Nat
  for +east: Bool
  {CR.Cron.ok(after, CR.Cron.next(c, after, off, east)) == True{} : Bool}

# once a routine fires at now (last := now) it is not due again at now
law cron_due_fires_once:
  for +c: CR.Cron
  for +now: Nat
  for +off: Nat
  for +east: Bool
  {CR.Cron.due(c, now, now, off, east) == False{} : Bool}
# Bot spaces
# ----------

# a parsed space never holds more than 200 blocks, whatever a bot writes
law space_caps:
  for +j: J.Json
  {Nat.is_le(SPC.Space.count(SPC.Space.parse(j)), 200n) == True{} : Bool}

# a progress bar never runs past full
law space_progress_max:
  for +p: Nat
  {Nat.is_le(SPC.Space.clamp(p), 1000n) == True{} : Bool}

# what a space's button or input sends the bot is always marked as coming
# from the space
law space_message_prefix:
  for +action: String
  for +value: String
  {String.starts_with(SPC.Space.message(action, value), "[space] ") == True{} : Bool}

# a bot's page is always served behind the head whose policy runs no
# script and loads nothing from the network, whatever the bot wrote
law space_page_locked:
  for html: String
  {String.starts_with(SPC.Space.page(html), SPC.Space.page.head()) == True{} : Bool}

# only a "space:" link in a bot's page sends the bot anything
law space_link_only_ours:
  for +u: String
  for h: {String.starts_with(u, "space:") == False{} : Bool}
  {SPC.Space.link(u) == Nil{} : List<&2, String>}
# Google (bots' Gmail and Calendar)
# ---------------------------------

# the sign-in asks for access at all (openid, email, Gmail, Calendar)
law google_scopes_nonempty:
  {List.is_empty(&2, String, GO.Google.scopes()) == False{} : Bool}

# base64url undoes itself on bytes that need both '-' and '_'
law google_b64url_roundtrip_example:
  {GO.Gb64.decode(GO.Gb64.encode_url([251, 255, 0, 104])) == [251, 255, 0, 104] : List<&2, U32>}

# a pasted redirect gives its code and state, decoded
law google_code_of_example:
  {GO.Google.code_of("http://127.0.0.1:3787/oauth/google?state=st8&code=4%2F0Ab&scope=email") == Some{GO.CodeState{"4/0Ab", "st8", False{}}} : Maybe<&2, GO.CodeState>}

# a redirect without a code (the user said no) is no code
law google_denied_no_code:
  {GO.Google.code_of("http://127.0.0.1:3787/oauth/google?error=access_denied&state=st8") == None{} : Maybe<&2, GO.CodeState>}

# a sign-in's state names the hub that started it
law google_state_home_example:
  {GO.Google.state.home(GO.Google.state("n0Ab-_", "http://box.tail1.ts.net:3787")) == "http://box.tail1.ts.net:3787" : String}

# Google's redirect for another hub's sign-in goes on to that hub, once
law google_hop_other:
  {GO.Google.hop("state=" ++ GO.Google.state("n0", "http://box.tail1.ts.net:3787") ++ "&code=4%2F0Ab", "n9")
    == "http://box.tail1.ts.net:3787/oauth/google?state=" ++ GO.Google.state("n0", "http://box.tail1.ts.net:3787") ++ "&code=4%2F0Ab&hop=1" : String}

# this hub's own sign-in finishes here
law google_hop_mine:
  {GO.Google.hop("state=" ++ GO.Google.state("n0", "http://box.tail1.ts.net:3787") ++ "&code=c", GO.Google.state("n0", "http://box.tail1.ts.net:3787")) == "" : String}

# a browser already sent on is never sent again (no loop)
law google_hop_once:
  {GO.Google.hop("state=" ++ GO.Google.state("n0", "http://box.tail1.ts.net:3787") ++ "&code=c&hop=1", "") == "" : String}

# only to a machine on a tailnet
law google_hop_tailnet_only:
  {GO.Google.hop("state=" ++ GO.Google.state("n0", "https://evil.example.com") ++ "&code=c", "") == "" : String}

law google_hop_no_path:
  {GO.Google.hop("state=" ++ GO.Google.state("n0", "https://evil.example.com/x.ts.net") ++ "&code=c", "") == "" : String}

# a phone's own loopback redirect begins a sign-in (RFC 8252 7.3)
law google_loopback_phone:
  {GO.Google.redirect.pick("http://127.0.0.1:49152/oauth/google", "http://127.0.0.1:3787/oauth/google") == "http://127.0.0.1:49152/oauth/google" : String}

# none asked: the hub's own
law google_loopback_none:
  {GO.Google.redirect.pick("", "http://127.0.0.1:3787/oauth/google") == "http://127.0.0.1:3787/oauth/google" : String}

# only loopback: Google's code never goes to another host
law google_loopback_not_host:
  {GO.Google.loopback("http://evil.example.com:80/oauth/google") == False{} : Bool}

law google_loopback_not_userinfo:
  {GO.Google.loopback("http://127.0.0.1:1@evil.example.com/oauth/google") == False{} : Bool}

law google_loopback_not_path:
  {GO.Google.loopback("http://127.0.0.1:80/x/oauth/google") == False{} : Bool}

law google_loopback_no_port:
  {GO.Google.loopback("http://127.0.0.1:/oauth/google") == False{} : Bool}

# a To, Cc or Subject cannot start a header of its own
law google_header_no_newline_example:
  {GO.Mail.clean("a@x.org\r\nBcc: e@x.org") == "a@x.org  Bcc: e@x.org" : String}

# sending mail and creating events ask the user first
law google_send_asks:
  {GO.Google.asks("gmail_send") == True{} : Bool}

law google_create_asks:
  {GO.Google.asks("calendar_create") == True{} : Bool}

# an access token is never used in its last minute
law google_expiring_not_fresh:
  for +now: Nat
  {GO.Google.fresh(now, now) == False{} : Bool}

# a client of the user's own is always the one a sign-in uses; the shared
# client only stands in when there is none
law google_own_client_wins:
  for +mine: GO.Client
  for sh: GO.Client
  for h: {GO.Client.has(mine) == True{} : Bool}
  {GO.Google.client(mine, sh) == mine : GO.Client}

# the shared client asks for Calendar and who signed in, never Gmail
law google_shared_scopes_all:
  {GO.Google.scopes_for(False{}) == GO.Google.scopes() : List<&2, String>}

# a link through the shared client runs every tool, Gmail's too
law google_shared_covers_all:
  for +tool: String
  {GO.Google.covers(True{}, tool) == True{} : Bool}

# the user's own client runs every tool
law google_own_covers_all:
  for +tool: String
  {GO.Google.covers(False{}, tool) == True{} : Bool}

# a tool that names no account uses the first one signed in
law google_accts_pick_first:
  for +a: GO.Acct
  for +t: List<&2, GO.Acct>
  {GO.Accts.pick(a <> t, "") == Some{a} : Maybe<&2, GO.Acct>}

# signing in to an address already there replaces it where it stands
law google_accts_put_replaces:
  for +a: GO.Acct
  for +t: List<&2, GO.Acct>
  for +b: GO.Acct
  for h: {String.eq(GO.Acct.email(a), GO.Acct.email(b)) == True{} : Bool}
  {GO.Accts.put(a <> t, b) == b <> t : List<&2, GO.Acct>}

# a new address goes last
law google_accts_put_new:
  for +b: GO.Acct
  {GO.Accts.put(Nil{}, b) == [b] : List<&2, GO.Acct>}

# signing one account out keeps none with its address
law google_accts_drop_named:
  for +a: GO.Acct
  for +t: List<&2, GO.Acct>
  for +e: String
  for h: {String.eq(GO.Acct.email(a), e) == True{} : Bool}
  {GO.Accts.drop(a <> t, e) == GO.Accts.drop(t, e) : List<&2, GO.Acct>}

# Bots
# ----

# the step behind bot_hop_bounded: a message goes out only within the bound
law bot_hop_step:
  for ok: Bool
  for +n: Nat
  for h: {Nat.is_le(n, BOT.Hop.max()) == ok : Bool}
  {BOT.Hop.bound(BOT.Hop.send.k(ok, n)) == True{} : Bool}

# a message between bots never goes out deeper than the bound, so two bots
# cannot keep each other talking forever
law bot_hop_bounded:
  for +h: Nat
  {BOT.Hop.bound(BOT.Hop.next(h)) == True{} : Bool}

# a busy bot hears a message: its turn keeps its hop (the message waits for
# the next turn), so another bot's answer never deepens its own
law bot_hop_busy_keeps:
  for +c: BOT.Hops
  for +h: Nat
  {BOT.Hops.now(BOT.Hop.woke(True{}, c, h)) == BOT.Hops.now(c) : Nat}

# a bot at rest hears a message at hop h: its turn runs at h
law bot_hop_idle_starts:
  for +c: BOT.Hops
  for +h: Nat
  {BOT.Hops.now(BOT.Hop.woke(False{}, c, h)) == h : Nat}

# a message that waits alone is answered by the next turn at its hop, so
# each message of a back-and-forth is answered one hop deeper
law bot_hop_queue_takes:
  for +n: Nat
  for +w: Nat
  for +h: Nat
  {BOT.Hops.now(BOT.Hop.taken(BOT.Hop.woke(True{}, BOT.Hops{n, w, False{}}, h))) == h : Nat}

# the step behind bot_hop_fanout: any number of messages reaching a busy
# bot leave its turn's hop as it was
law bot_hop_wakes_keep:
  for hs: List<&2, Nat>
  for +c: BOT.Hops
  {BOT.Hops.now(BOT.Hop.wakes(hs, c)) == BOT.Hops.now(c) : Nat}

# a request fanned out to N bots lets each reply once: a bot woken by a post
# at hop h answers at h+1, however many of the others' answers reach it
# while it works
law bot_hop_fanout:
  for hs: List<&2, Nat>
  for +c: BOT.Hops
  for +h: Nat
  {BOT.Hop.next(BOT.Hops.now(BOT.Hop.wakes(hs, BOT.Hop.woke(False{}, c, h)))) == BOT.Hop.next(h) : Maybe<&2, Nat>}

# the step behind bot_loop_bounded: one message of a back-and-forth keeps
# the talk within the bound
law bot_loop_up:
  for ok: Bool
  for b: Bool
  for +h: Nat
  for q: {Nat.is_le(1n+h, BOT.Hop.max()) == ok : Bool}
  for e: {Nat.is_le(h, BOT.Hop.max()) == True{} : Bool}
  {Nat.is_le(BOT.Hop.up.k(BOT.Hop.send.k(ok, 1n+h), b, h), BOT.Hop.max()) == True{} : Bool}

# a bot-only loop stops within the limit: however long two bots answer each
# other, busy or at rest, the talk never goes past the bound
law bot_loop_bounded:
  for bs: List<&2, Bool>
  for +h: Nat
  for e: {Nat.is_le(h, BOT.Hop.max()) == True{} : Bool}
  {Nat.is_le(BOT.Hop.climb(bs, h), BOT.Hop.max()) == True{} : Bool}

# ten answers back and forth from a person's message: the talk stops at the
# bound after six
law bot_loop_stops:
  {BOT.Hop.climb([True{}, False{}, True{}, True{}, False{}, False{}, True{}, False{}, True{}, True{}], 0n) == BOT.Hop.max() : Nat}

# a held post (past the bound) wakes no bot, here or on the machine it
# reaches; it is kept in its room for the person
law bot_held_wakes_none:
  for st: H.Step
  {BHS.Bots.hop.gate(BOT.Hop.live(BOT.Hop.held()), st) == H.Step.none() : H.Step}

# a linked machine never takes over a live room here with no member on it
law bot_room_no_takeover:
  for +i: String
  for +n: String
  for +c: Nat
  for +s: M.State
  for +pid: String
  {BHS.Bots.room.open(BOT.Room{i, n, Nil{}, 1n+c, False{}}, s, pid) == False{} : Bool}

# nor opens a room the person deleted
law bot_room_deleted_stays:
  for +i: String
  for +n: String
  for +ms: List<&2, String>
  for +c: Nat
  for +s: M.State
  for +pid: String
  {BHS.Bots.room.open(BOT.Room{i, n, ms, 1n+c, True{}}, s, pid) == False{} : Bool}

# each room is listed once: the rooms here (the window's sidebar, the web,
# each hub's list on a phone) never name one room id twice
law rooms_listed_once:
  for +u: U.Ui
  {BU.BU.once.distinct(BU.RRow, BU.BU.rooms.keyed(u)) == True{} : Bool}

# nor does a phone's one list over every paired hub, where a room shared
# by two machines is reported by both
law hubs_rooms_listed_once:
  for ss: List<&2, Hb.Shown>
  for +many: Bool
  {BU.BU.once.distinct(J.Json, Hb.Hubs.rooms.keyed(ss, many)) == True{} : Bool}

# one room reported twice is listed once; two rooms are two, whatever
# their names
law rooms_same_id_once:
  {BU.BU.once.of(String, [BU.RK{"r1", "den"}, BU.RK{"r1", "den"}]) == [BU.RK{"r1", "den"}] : List<&2, BU.RK<String>>}

law rooms_same_name_both:
  {BU.BU.once.of(String, [BU.RK{"r1", "den"}, BU.RK{"r2", "den"}]) == [BU.RK{"r1", "den"}, BU.RK{"r2", "den"}] : List<&2, BU.RK<String>>}

# a cat waits for you (ears up) whenever its thread needs an approval
law bot_mood_wait:
  for talked: Bool
  for idle: Bool
  {BU.Mood.of(M.StApproval{}, talked, idle) == "wait" : String}

# ... or an answer
law bot_mood_input:
  for talked: Bool
  for idle: Bool
  {BU.Mood.of(M.StInput{}, talked, idle) == "wait" : String}

# a sleeping cat holds still: a window of resting bots never repaints
law bot_mood_sleep_still:
  {BU.Mood.moving("sleep") == False{} : Bool}

# a bot at rest for a quarter hour is asleep
law bot_mood_rest:
  for talked: Bool
  {BU.Mood.of(M.StReady{}, talked, True{}) == "sleep" : String}
# Cats
# ----

# a sleeping, failed or unreachable cat holds still in the window: it never
# asks for frames
law cat_sleep_still:
  {CAT.Cat.moving("sleep") == False{} : Bool}

law cat_error_still:
  {CAT.Cat.moving("error") == False{} : Bool}

law cat_away_still:
  {CAT.Cat.moving("away") == False{} : Bool}

# an unreachable cat carries no animation at all, on any client
law cat_away_holds_still:
  for +look: Nat
  {CAT.Cat.anims.count(CAT.Cat.rig(look, "away")) == 0n : Nat}

# each mood has its own pose (none falls back to idle but idle)
law cat_moods_known:
  {CAT.Cat.mood.all(CAT.Cat.moods()) == [0, 1, 2, 3, 4, 5, 6, 7] : List<&2, U32>}

# every seed has a look: its index is one of the looks
law cat_look_total:
  for +s: Nat
  {Nat.is_lt(CAT.Cat.look.index(s), CAT.Cat.looks()) == True{} : Bool}

# a Pebble ring's webhook that is not fresh is never accepted, whatever it
# carries or claims to be signed with
law index_stale_rejected:
  for secret: String
  for +now: Nat
  for +ts: Nat
  for delivery: String
  for trigger: String
  for test: Bool
  for body: String
  for sig: String
  for h: {SG.Sign.fresh(now, ts) == False{} : Bool}
  {SG.Sign.index(secret, now, ts, delivery, trigger, test, body, sig) == False{} : Bool}

# a webhook with no bearer token lets no bearer in, whatever is sent
law bearer_needs_token:
  for +given: String
  {SG.Sign.bearer("", given) == False{} : Bool}

# a form under a bearer token reaches the bot as JSON: its words, and a
# note in place of the recording (a fuller example, with recordedAt and
# client, is in test/bots_test.bend: normalizing it here takes too long)
law ring_form_json:
  {RG.Ring.form("--b\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\nhi\r\n--b\r\nContent-Disposition: form-data; name=\"audio\"\r\n\r\nxx\r\n--b--\r\n", "2")
    == "{\"source\":\"form-data\",\"transcription\":\"hi\",\"audio\":\"omitted, 2 bytes\"}" : String}

# a phone reaches the hub's webhooks at the HTTPS address `tailscale
# serve` gives the hub's own port, not another service's
law tailnet_serve_https:
  {TN.Tailnet.serve("{\"Web\":{\"a.ts.net:443\":{\"Handlers\":{\"/\":{\"Proxy\":\"http://127.0.0.1:3773\"}}},\"a.ts.net:8443\":{\"Handlers\":{\"/\":{\"Proxy\":\"http://127.0.0.1:3787\"}}}}}", "3787")
    == "https://a.ts.net:8443" : String}

# the step behind bot_spawn_bounded
law bot_spawn_step:
  for ok: Bool
  for +n: Nat
  for h: {Nat.is_le(n, BHS.Spawn.max()) == ok : Bool}
  {BHS.Spawn.bound(BOT.Hop.send.k(ok, n)) == True{} : Bool}

# a sub-bot is never made deeper than the bound below a person's bot, so
# bots cannot make bots without end
law bot_spawn_bounded:
  for +d: Nat
  {BHS.Spawn.bound(BHS.Spawn.next(d)) == True{} : Bool}

# the step behind bot_request_in_room
law bot_request_keep:
  for dm: Bool
  for r: BOT.Room
  for +i: String
  for +n: String
  for +ms: List<&2, String>
  for +now: Nat
  for h: {String.starts_with(i, "dm:") == False{} : Bool}
  for e: {BOT.Room.is_dm(r) == dm : Bool}
  {BHS.Req.ok(Some{BHS.Req.target(BHS.Req.keep.k(dm, r), i, n, ms, now)}) == True{} : Bool}

# a request from one bot to another is posted in a room, never a direct
# message, so the person can follow it: a room found is used only when it
# is not a dm: one, and a new one's id (bot_request_fresh_id) is not either
law bot_request_in_room:
  for m: Maybe<&2, BOT.Room>
  for +i: String
  for +n: String
  for +ms: List<&2, String>
  for +now: Nat
  for h: {String.starts_with(i, "dm:") == False{} : Bool}
  {BHS.Req.ok(Some{BHS.Req.target(BHS.Req.keep(m), i, n, ms, now)}) == True{} : Bool}

# a new request room's id is never a direct message's
law bot_request_fresh_id:
  for +now: Nat
  for +seq: Nat
  {String.starts_with(H.Hub.id("r", now, seq), "dm:") == False{} : Bool}

# a bot that names no project may not delegate: its work would run where
# the person cannot see it
law bot_delegate_needs_project:
  for +s: M.State
  for +now: Nat
  for +t: String
  for +proj: String
  for +p: J.Json
  {MC.Res.good(MC.Mcp.delegate.b(True{}, True{}, s, now, t, proj, p)) == False{} : Bool}

# a bot's delegated child is never hidden under the bot: it shows in the
# sidebar of the project it runs in
law kids_bot_child_shown:
  for m: Map<&2, Bool>
  for +c: String
  {U.Kids.add(True{}, m, c) == m : Map<&2, Bool>}

# a cat at rest (ready) holds still in the window: resting bots never keep
# the window drawing frames
law bot_mood_idle_still:
  {BU.Mood.moving("idle") == False{} : Bool}
# Performance numbers
# -------------------

# an error is logged whatever BACKPLANE_LOG says
law perf_errors_always_logged:
  for +th: Nat
  {PF.Log.on(0n, th) == True{} : Bool}

# debug lines stay quiet unless asked for
law perf_debug_quiet_by_default:
  {PF.Log.on(3n, PF.Log.level("")) == False{} : Bool}

# a gauge's window is its latest reading, whatever came before
law perf_gauge_window_is_reading:
  for +name: String
  for +n: Nat
  for +sum: Nat
  for +max: Nat
  for +last: Nat
  for +bs: List<&2, Nat>
  for +old: PF.Stat
  {PF.Stat.since(PF.Stat{name, 1, n, sum, max, last, bs}, old) == PF.Stat{name, 1, n, sum, max, last, bs} : PF.Stat}

# a row with no values has no percentiles
law perf_pct_empty:
  for +name: String
  for +per: Nat
  {PF.Stat.pct(PF.Stat.none(name), per) == 0n : Nat}

# a percentile is the top of its bucket, never above the max
law perf_pct_bucket_top:
  {PF.Stat.pct(PF.Stat{"x", 0, 4n, 10000n, 6000n, 1000n, [0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 2n, 1n, 0n, 1n]}, 500n) == 1023n : Nat}

law perf_pct_capped_by_max:
  {PF.Stat.pct(PF.Stat{"x", 0, 4n, 10000n, 6000n, 1000n, [0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 2n, 1n, 0n, 1n]}, 990n) == 6000n : Nat}

# the overlay's first frame has no window yet to compare against
law perf_hud_first_frame:
  for +cur: String
  for +at: Nat
  for +now: Nat
  {PF.Hud.text(cur, "", at, now) == "measuring (ctrl+shift+h hides this)" : String}

# the overlay off lays nothing out (no cost in a frame)
law perf_hud_off_draws_nothing:
  for +ty: L.Ty
  for +text: String
  for +w: U32
  {L.Lay.perf.if(False{}, ty, text, w) == L.Out.none() : L.Out}

# Dictation
# ---------

# a term already in the dictionary adds nothing
law voice_add_idem:
  for +ds: List<&2, String>
  for +term: String
  for h: {VO.Voice.dict.has(ds, term) == True{} : Bool}
  {VO.Voice.dict.add(ds, term) == ds : List<&2, String>}

# a term that is not blank, once added, is in the dictionary
law voice_add_has:
  for +ds: List<&2, String>
  for +term: String
  for h: {String.is_empty(String.trim(term)) == False{} : Bool}
  {VO.Voice.dict.has(VO.Voice.dict.add(ds, term), term) == True{} : Bool}

# replacing a word with itself, or an empty word, leaves the text as it is
law voice_fix_same:
  for +t: String
  for +w: String
  {VO.Voice.fix(t, w, w) == t : String}

law voice_fix_empty:
  for +t: String
  for +r: String
  {VO.Voice.fix(t, "", r) == t : String}

# never more than five words to ask about, nor more keywords than the
# transcriber takes
law voice_unsure_bounded:
  for +ds: List<&2, String>
  for +ig: List<&2, String>
  for +text: String
  {Nat.is_le(List.length(&2, VO.Unsure, VO.Voice.unsure(ds, ig, text)), VO.Voice.max_unsure()) == True{} : Bool}

law voice_keywords_bounded:
  for +ds: List<&2, String>
  {Nat.is_le(List.length(&2, String, VO.Voice.keywords(ds)), VO.Voice.max_keywords()) == True{} : Bool}

# a segment heard as nothing leaves the draft as it is
law voice_put_blank:
  for firm: Bool
  for +d: String
  for +at: Nat
  {VO.Ins.text(VO.Voice.put(firm, d, at, "")) == d : String}

# Each bot once
# -------------
# Every list of bots keys each bot by what it stands for (a bot here by its
# id, a bot elsewhere by its name, any case, at its machine's link id) and
# keeps the first of each key (src/core/once.bend).

# a list kept once holds no key twice
law once_distinct:
  for -A: Data
  for xs: List<&2, ON.Ky<A>>
  {ON.Once.distinct(ON.Once.keys(A, ON.Once.of(A, xs))) == True{} : Bool}

# and when no key repeats it keeps everything, in order: two things with
# different keys are never merged
law once_keeps:
  for -A: Data
  for +xs: List<&2, ON.Ky<A>>
  for hx: {ON.Once.distinct(ON.Once.keys(A, xs)) == True{} : Bool}
  {ON.Once.of(A, xs) == xs : List<&2, ON.Ky<A>>}

# each bot appears once in the bot list: this machine's bots and those the
# linked machines report (bots.remote), however often one is reported
law bot_rows_once:
  for +u: U.Ui
  {ON.Once.distinct(ON.Once.keys(BU.BRow, BU.BU.rows.keyed(u))) == True{} : Bool}

# the list shown is exactly that keyed list
law bot_rows_shown:
  for +u: U.Ui
  {BU.BU.rows(u) == ON.Once.vals(BU.BRow, BU.BU.rows.keyed(u)) : List<&2, BU.BRow>}

# distinct bots all stay listed
law bot_rows_keep:
  for +u: U.Ui
  for hx: {ON.Once.distinct(ON.Once.keys(BU.BRow, BU.BU.rows.all(u))) == True{} : Bool}
  {BU.BU.rows.keyed(u) == BU.BU.rows.all(u) : List<&2, ON.Ky<BU.BRow>>}

# the room picker lists each bot once too, here or elsewhere, however a
# member names it
law bot_picks_once:
  for +u: U.Ui
  {ON.Once.distinct(ON.Once.keys(BU.Pick, BU.BU.picks.keyed(u))) == True{} : Bool}

law bot_picks_shown:
  for +u: U.Ui
  {BU.BU.picks(u) == ON.Once.vals(BU.Pick, BU.BU.picks.keyed(u)) : List<&2, BU.Pick>}

# a phone paired with several hubs lists each bot once, though linked hubs
# each report the other's bots
law hubs_bots_once:
  for +ss: List<&2, Hb.Shown>
  for +hs: List<&2, Hb.Hub>
  for +many: Bool
  {ON.Once.distinct(ON.Once.keys(J.Json, Hb.Hubs.bots.keyed(ss, hs, many))) == True{} : Bool}

law hubs_bots_shown:
  for +ss: List<&2, Hb.Shown>
  for +hs: List<&2, Hb.Hub>
  for +many: Bool
  {Hb.Hubs.bots(ss, hs, many) == ON.Once.vals(J.Json, Hb.Hubs.bots.keyed(ss, hs, many)) : List<&2, J.Json>}

# a bot's answer goes back only to a person who wrote from a linked machine:
# never to a bot there, never for a room post
law reply_peer_person:
  {BHS.Reply.peer("[message from person@desktop]\nhey") == "desktop" : String}

law reply_peer_bot:
  {BHS.Reply.peer("[message from Bub@desktop]\nhey") == "" : String}

law reply_peer_room:
  {BHS.Reply.peer("[message from person@desktop in #staff]\nhey") == "" : String}

# the answer sent back is the turn's newest, and the turn is the one the
# thread's newest user message started (another thread's entries are skipped)
law reply_said_newest:
  {BHS.Said.of([M.Msg{"3", "t", M.Assistant{}, "second", 3n}, M.Msg{"x", "u", M.Assistant{}, "other", 3n},
    M.Msg{"2", "t", M.Assistant{}, "first", 2n}, M.Msg{"1", "t", M.User{}, "[message from person@desktop]\nhey", 1n},
    M.Msg{"0", "t", M.Assistant{}, "old", 0n}], "t")
    == BHS.Said{"second", "[message from person@desktop]\nhey"} : BHS.Said}

# an answer from a bot elsewhere to the person here wakes no one
law reply_person_wakes_none:
  for +s: M.State
  for +now: Nat
  for +from: String
  for +text: String
  {H.Step.outs_of(BHS.Bots.incoming.person(s, now, from, text)) == Nil{} : List<&2, H.Out>}

# a bot elsewhere opens on the owner's machine it lives on: the peer's
# MagicDNS name is the machine the switcher lists by its address
law far_machine_magicdns:
  {U.Far.machine(J.Json.obj([J.KV{"self", J.Json.obj([J.KV{"name", J.Str{"desk"}}, J.KV{"url", J.Str{"http://100.64.0.1:3787"}}, J.KV{"domain", J.Str{"tail1.ts.net"}}])},
    J.KV{"peers", J.Json.arr([J.Json.obj([J.KV{"name", J.Str{"Box"}}, J.KV{"url", J.Str{"http://100.64.0.2:3787"}}])])}]), "lab", "http://box.tail1.ts.net:3787") == Some{"http://100.64.0.2:3787"} : Maybe<&2, String>}

# one on this machine ("" is the home hub)
law far_machine_self:
  {U.Far.machine(J.Json.obj([J.KV{"self", J.Json.obj([J.KV{"name", J.Str{"desk"}}, J.KV{"url", J.Str{"http://100.64.0.1:3787"}}, J.KV{"domain", J.Str{"tail1.ts.net"}}])},
    J.KV{"peers", J.Json.arr([J.Json.obj([J.KV{"name", J.Str{"Box"}}, J.KV{"url", J.Str{"http://100.64.0.2:3787"}}])])}]), "desk", "http://100.64.0.1:3787") == Some{""} : Maybe<&2, String>}

# another hub on the same machine but another port is not that machine's
law far_machine_other_port:
  {U.Far.machine(J.Json.obj([J.KV{"self", J.Json.obj([J.KV{"name", J.Str{"desk"}}, J.KV{"url", J.Str{"http://100.64.0.1:3787"}}, J.KV{"domain", J.Str{"tail1.ts.net"}}])},
    J.KV{"peers", J.Json.arr([J.Json.obj([J.KV{"name", J.Str{"Box"}}, J.KV{"url", J.Str{"http://100.64.0.2:3787"}}])])}]), "box", "http://box.tail1.ts.net:4000") == None{} : Maybe<&2, String>}

# someone else's machine is not the owner's, even with the same name as
# one of them: its bots stay a conversation
law far_machine_stranger:
  {U.Far.machine(J.Json.obj([J.KV{"self", J.Json.obj([J.KV{"name", J.Str{"desk"}}, J.KV{"url", J.Str{"http://100.64.0.1:3787"}}, J.KV{"domain", J.Str{"tail1.ts.net"}}])},
    J.KV{"peers", J.Json.arr([J.Json.obj([J.KV{"name", J.Str{"Box"}}, J.KV{"url", J.Str{"http://100.64.0.2:3787"}}])])}]), "box", "http://box.tail9.ts.net:3787") == None{} : Maybe<&2, String>}

# the tailnet's domain, from this machine's MagicDNS name
law tailnet_domain:
  {TN.Tailnet.domain("{\"Self\":{\"DNSName\":\"desk.tail1.ts.net.\"}}") == "tail1.ts.net" : String}

# A bot's new project waits for the person
# ----------------------------------------

# with no approval for the session on record, project_create always asks
law bot_newproj_asks:
  for +th: String
  {BHS.Bots.newproj.asks(Nil{}, th) == True{} : Bool}

# a yes to one call is not a yes to the next: only "allow for the session" is
law bot_newproj_once:
  {BHS.Bots.newproj.asks([M.Ask{"q1", "t1", "approval", "project_create", "", "", False{}, "accept"}], "t1") == True{} : Bool}

# declining, or no answer, makes nothing
law bot_newproj_decline:
  {BHS.Bots.newproj.yes("decline") == False{} : Bool}

law bot_newproj_no_answer:
  {BHS.Bots.newproj.yes("") == False{} : Bool}

# References in a message
# -----------------------

# a word with no sigil is offered nothing
law refs_plain_offers_nothing:
  for +s: M.State
  for +sel: String
  for +now: Nat
  {RF.Refs.offer(s, sel, now, "hello") == [] : List<&2, RF.Cand>}

law refs_plain_not_typing:
  {RF.Refs.typing("hello") == False{} : Bool}

# a bot on a linked machine is a reference as it is typed
law refs_far_typing:
  {RF.Refs.typing("@kit@box") == True{} : Bool}

# a message that names nothing reaches its agent as it was sent
law refs_agent_plain:
  for +s: M.State
  for +sel: String
  {RF.Refs.agent(s, sel, "hello") == "hello" : String}

# a bot is never woken by a mention in its own thread
law refs_own_bot_never:
  for named: Bool
  for +id: String
  for +name: String
  {RF.Refs.b.one(named, True{}, id, name) == [] : List<&2, RF.Hit>}

# a message sent again (a dropped connection) wakes no bot twice
law bot_mention_once:
  for +s: M.State
  for +now: Nat
  for +p: J.Json
  {BHS.Bots.mentions.k(True{}, s, now, p) == H.Step.none() : H.Step}

# a summary never made is due; one not due is neither asked for nor stamped
law sum_never_made_due:
  for +now: Nat
  {RF.Sum.due.k("", now) == True{} : Bool}

law sum_not_due_asks_nothing:
  for +s: M.State
  for +id: String
  for +kind: String
  {H.Sum.out(False{}, s, id, kind) == [] : List<&2, H.Out>}

law sum_not_due_stamps_nothing:
  for +id: String
  for +now: Nat
  {H.Sum.at(False{}, id, now) == [] : List<&2, M.Change>}

# Subagents
# ---------

# a subagent is listed once when it starts, says what it does as it goes,
# and leaves the list when it ends; progress for one not listed changes nothing
law bg_start_progress_end:
  {M.Bg.show(M.Bg.drop(M.Bg.set(M.Bg.add(M.Bg.lines("a\tx"), "b", "y"), "b", "z"), "a")) == "b\tz" : String}

law bg_added_once:
  {M.Bg.show(M.Bg.add(M.Bg.lines("a\tx"), "a", "y")) == "a\tx" : String}

law bg_progress_unlisted:
  {M.Bg.show(M.Bg.set(M.Bg.lines("a\tx"), "b", "z")) == "a\tx" : String}

law bg_doing:
  {M.Bg.doing("a\tRunning tests") == "Running tests" : String}

# a thread whose turn is over and whose subagents are still at work shows
# as waiting on them (its own colour)
law status_waits_on_subagents:
  {M.Status.wait(True{}, M.Status.of(False{}, False{}, M.Completed{}, False{})) == M.StWaiting{} : M.Status}

# a thread starting a turn (its message waits only for its worktree) is
# working, never queued
law status_starting_works:
  {M.Status.of(False{}, False{}, M.Completed{}, True{}) == M.StWorking{} : M.Status}

# a running turn, a question and an approval still come first
law status_work_before_waiting:
  for q: Bool
  {M.Status.wait(True{}, M.Status.of(False{}, False{}, M.Running{}, q)) == M.StWorking{} : M.Status}

law status_input_before_waiting:
  for t: M.TurnState
  for q: Bool
  {M.Status.wait(True{}, M.Status.of(False{}, True{}, t, q)) == M.StInput{} : M.Status}

# with none at work the dot is as before
law status_no_subagents_same:
  for s: M.Status
  {M.Status.wait(False{}, s) == s : M.Status}

# Many agents at once
# -------------------
# The window draws a thread's streamed text only while that thread is
# open: another thread's delta asks for no frame, the open thread's does,
# and every other message still does (U.Ui.shows).

law shows_other_delta:
  for u: U.Ui
  for +th: String
  for +text: String
  for h: {String.eq(th, U.Ui.sel(u)) == False{} : Bool}
  {U.Ui.shows(u, J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{th}}, J.KV{"text", J.Str{text}}])) == False{} : Bool}

law shows_own_delta:
  for u: U.Ui
  for +text: String
  {U.Ui.shows(u, J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{U.Ui.sel(u)}}, J.KV{"text", J.Str{text}}])) == True{} : Bool}

law shows_changes:
  for u: U.Ui
  for +items: J.Json
  {U.Ui.shows(u, J.Json.obj([J.KV{"t", J.Str{"changes"}}, J.KV{"items", items}])) == True{} : Bool}

# Each thread's entries are filed as they are posted (M.Idx), so a frame
# reads the open thread's page and the sidebar's links without walking
# every thread's log. What is filed is what the walk finds: a thread's
# page, the messages it sent included, and the links.
law idx_page_own:
  {L.TL.newest(M.State.mine(M.Idx.demo(), "t2"), "t2", 0n, L.NW{60n, []}) == L.TL.newest(U.Ui.entries(M.Idx.demo()), "t2", 0n, L.NW{60n, []}) : List<&2, M.Entry>}

law idx_page_sent:
  {L.TL.newest(M.State.mine(M.Idx.demo(), "t1"), "t1", 0n, L.NW{60n, []}) == L.TL.newest(U.Ui.entries(M.Idx.demo()), "t1", 0n, L.NW{60n, []}) : List<&2, M.Entry>}

law idx_sent_filed:
  {List.length(&2, M.Entry, M.State.mine(M.Idx.demo(), "t1")) == 3n : Nat}

law idx_page_short:
  {L.TL.newest(M.State.mine(M.Idx.demo(), "t2"), "t2", 0n, L.NW{2n, []}) == L.TL.newest(U.Ui.entries(M.Idx.demo()), "t2", 0n, L.NW{2n, []}) : List<&2, M.Entry>}

law idx_links:
  {U.Links.recent(M.Idx.demo(), 10n) == U.Links.recent.go(U.Ui.entries(M.Idx.demo()), 10n, True{}) : List<&2, U.Link2>}

law idx_links_found:
  {List.length(&2, U.Link2, U.Links.recent(M.Idx.demo(), 10n)) == 1n : Nat}

# Settings
# --------

# choosing a thread, a bot or a room while Settings is open leaves it
law select_leaves_settings:
  {U.Ui.flag(U.Act.ui(U.Ui.select(U.Ui.flag_on(U.Ui.init(0n), "settings"), "t1")), "settings") == False{} : Bool}

law room_leaves_settings:
  {U.Ui.flag(U.Act.ui(U.Bots.room.select(U.Ui.flag_on(U.Ui.init(0n), "settings"), "r1")), "settings") == False{} : Bool}

# a room's posts are lines the tiles slide with, so a wheel notch reuses
# what the last frame drew
law hits_chat_line_slides:
  for +x0: U32
  {N.Hits.kind(False{}, x0, "chat-line") == True{} : Bool}

# a room scrolls one tile a notch; Settings and bot panes keep 48 px
law chat_notch_tile:
  {N.Set.scroll.by("0", "1000", 1, N.Nui.notch()) == "64" : String}

law set_scroll_48:
  {N.Set.scroll("0", "1000", 1) == "48" : String}

# a cat that holds still is posed once (at clock 0), not on every tick
# another cat moves; a moving one follows the clock
law bots_still_cat_posed_once:
  for +c: Nat
  {BV.Bv.warm.at("idle", c) == 0n : Nat}

law bots_away_cat_posed_once:
  for +c: Nat
  {BV.Bv.warm.at("away", c) == 0n : Nat}

law bots_moving_cat_follows_clock:
  for +c: Nat
  {BV.Bv.warm.at("work", c) == c : Nat}

# input holds the cats still for a whole Bv.calm, so their frames never
# compete with keys or the wheel
law bots_cats_calm_on_input:
  for +ms: Nat
  {BV.Bv.calm.after(ms, ms) == BV.Bv.calm() : Nat}

# Threads elsewhere (far.bend)
# ----------------------------
# A thing on a linked machine is named by the link and its own id there,
# so it never takes the place of anything here, and a request about it
# goes back to that machine in that machine's ids.

law far_id_parts:
  {FR.Fr.link(FR.Fr.id("p1", "t1")) ++ " " ++ FR.Fr.local(FR.Fr.id("p1", "t1")) == "p1 t1" : String}

# an id here names nothing elsewhere and is its own local id
law far_id_here:
  {FR.Fr.link("t1790715007-34164") ++ "|" ++ FR.Fr.local("t1790715007-34164") == "|t1790715007-34164" : String}

# a linked machine's message lands in its thread as named here, never in
# a thread of this machine
law far_change_names_link:
  for +l: String
  for +th: String
  for +msg: String
  for role: M.Role
  for +text: String
  for +at: Nat
  {FR.Fr.change(l, M.MessagePosted{th, msg, role, text, at}) == Some{M.MessagePosted{FR.Fr.id(l, th), FR.Fr.sent(l, msg), role, text, at}} : Maybe<&2, M.Change>}

# a message id names its sender (M.Link.id) as the mirror names it: a
# thread of the mirroring machine's own is its own again, one of the
# owner's goes through the link; an id with no sender is left as it is
law far_sent_names_sender:
  {FR.Fr.sent("p1", M.Link.id("p1~ta", "m5")) ++ " " ++ FR.Fr.sent("p1", M.Link.id("tb", "m6")) ++ " " ++ FR.Fr.sent("p1", "m1790715007-3") == "t:ta:m5 t:p1~tb:m6 m1790715007-3" : String}

law far_change_bot_named:
  for +l: String
  for +id: String
  for +name: String
  for +th: String
  for +look: Nat
  for +persona: String
  for +at: Nat
  {FR.Fr.change(l, M.BotSet{id, name, th, look, persona, at}) == Some{M.BotSet{FR.Fr.id(l, id), name, FR.Fr.id(l, th), look, persona, at}} : Maybe<&2, M.Change>}

# settings, links, devices, webhooks and rooms stay on their machine: never
# shared, never folded in elsewhere
law far_settings_stay_home:
  for +s: M.State
  for +k: String
  for +v: String
  for +at: Nat
  {Bool.or(FR.Fr.shared(s, M.SettingSet{k, v, at}), FR.Fr.is.some(FR.Fr.change("p1", M.SettingSet{k, v, at}))) == False{} : Bool}

law far_peers_stay_home:
  for +s: M.State
  for +i: String
  for +n: String
  for +u: String
  for r: Bool
  for +at: Nat
  {Bool.or(FR.Fr.shared(s, M.PeerSet{i, n, u, r, at}), FR.Fr.is.some(FR.Fr.change("p1", M.PeerSet{i, n, u, r, at}))) == False{} : Bool}

law far_hooks_stay_home:
  for +s: M.State
  for +i: String
  for +b: String
  for +n: String
  for r: Bool
  for +at: Nat
  {Bool.or(FR.Fr.shared(s, M.HookSet{i, b, n, r, at}), FR.Fr.is.some(FR.Fr.change("p1", M.HookSet{i, b, n, r, at}))) == False{} : Bool}

law far_devices_stay_home:
  for +s: M.State
  for +tk: String
  for +pl: String
  for +k: String
  for +th: String
  for +env: String
  for +bu: String
  for +at: Nat
  {Bool.or(FR.Fr.shared(s, M.DeviceRegistered{tk, pl, k, th, env, bu, at}), FR.Fr.is.some(FR.Fr.change("p1", M.DeviceRegistered{tk, pl, k, th, env, bu, at}))) == False{} : Bool}

law far_rooms_stay_home:
  for +s: M.State
  for +i: String
  for +r: String
  for +f: String
  for +t: String
  for +h: Nat
  for +at: Nat
  {Bool.or(FR.Fr.shared(s, M.RoomPosted{i, r, f, t, h, at}), FR.Fr.is.some(FR.Fr.change("p1", M.RoomPosted{i, r, f, t, h, at}))) == False{} : Bool}

# a batch past a gap takes nothing (the rest is asked for), whatever it holds
law far_gap_takes_nothing:
  for xs: List<&2, FR.Item>
  {FR.Fr.take(3n, 5n, 9n, xs) == FR.Got{3n, Nil{}, True{}} : FR.Got}

# a batch already had (late, or sent twice) takes nothing and moves nothing back
law far_old_takes_nothing:
  for a: J.Json
  for b: J.Json
  {FR.Fr.take(9n, 3n, 6n, [FR.Item{4n, a}, FR.Item{6n, b}]) == FR.Got{9n, Nil{}, False{}} : FR.Got}

# of a batch that overlaps what is had, only what is new is taken
law far_overlap_takes_new:
  for a: J.Json
  for b: J.Json
  for c: J.Json
  {FR.Fr.take(5n, 3n, 8n, [FR.Item{4n, a}, FR.Item{6n, b}, FR.Item{8n, c}]) == FR.Got{8n, [FR.Item{6n, b}, FR.Item{8n, c}], False{}} : FR.Got}

# and the same batch again then takes nothing
law far_batch_again:
  for a: J.Json
  for b: J.Json
  {FR.Fr.take(8n, 5n, 8n, [FR.Item{6n, a}, FR.Item{8n, b}]) == FR.Got{8n, Nil{}, False{}} : FR.Got}

# a request about a thread here is never sent elsewhere; one about a
# thread elsewhere goes to its link
law far_route_here_stays:
  {FR.Fr.route(J.Json.obj([J.KV{"m", J.Str{"turn.start"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"text", J.Str{"a~b"}}])}])) == None{} : Maybe<&2, FR.Route>}

law far_route_goes_home:
  {FR.Route.link.of(FR.Fr.route(J.Json.obj([J.KV{"m", J.Str{"turn.interrupt"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"p1~t1"}}])}]))) == "p1" : String}

# the owning hub takes from a linked machine only what Fr.methods lists
law far_allowed_only_methods:
  for +s: M.State
  for +p: J.Json
  {FR.Fr.allowed(s, J.Json.obj([J.KV{"m", J.Str{"setting.set"}}, J.KV{"p", p}])) == False{} : Bool}

law far_no_delete:
  for +s: M.State
  for +p: J.Json
  {FR.Fr.allowed(s, J.Json.obj([J.KV{"m", J.Str{"thread.delete"}}, J.KV{"p", p}])) == False{} : Bool}

# where a thing lives is shown beside its name, only for a thing elsewhere
law far_where_here:
  for +n: String
  {FR.Fr.where("t1", n) == "" : String}

law far_where_far:
  {FR.Fr.where("p1~t1", "box") == " · box" : String}

# a bot mirrored from a linked machine has only the tabs its machine
# shares; a bot here keeps them all
law far_bot_tabs:
  {BU.BU.tabs.of("p1~b1") == ["chat", "space", "memory", "routines"] : List<&2, String>}

law far_bot_tabs_here:
  {BU.BU.tabs.of("b1") == BU.BU.tabs() : List<&2, String>}

# Remote bots in the same view
# ----------------------------
# A row opens what it stands for: a bot with a thread here (this
# machine's, or one mirrored from a linked machine) as a bot; a bot only
# reported by a linked machine as the conversation with it
law far_row_act_mirror:
  for +i: String
  for +n: String
  for +lk: Nat
  for +m: String
  for +no: String
  for +se: Bool
  for +pe: String
  for +l: String
  {BU.BU.act(BU.BRow{i, n, "p1~bt1", lk, m, no, se, True{}, pe, l}) == "bot" : String}

law far_row_act_reported:
  for +i: String
  for +n: String
  for +lk: Nat
  for +m: String
  for +no: String
  for +se: Bool
  for +pe: String
  for +l: String
  {BU.BU.act(BU.BRow{i, n, "", lk, m, no, se, True{}, pe, l}) == "remote" : String}

# a bot elsewhere is named with its machine; one here by its name alone
law bot_row_title_far:
  for +i: String
  for +th: String
  for +lk: Nat
  for +m: String
  for +no: String
  for +se: Bool
  for +l: String
  {BU.BU.row.title(BU.BRow{i, "Kit", th, lk, m, no, se, True{}, "box", l}) == "Kit · box" : String}

law bot_row_title_here:
  for +i: String
  for +n: String
  for +th: String
  for +lk: Nat
  for +m: String
  for +no: String
  for +se: Bool
  for +pe: String
  for +l: String
  {BU.BU.row.title(BU.BRow{i, n, th, lk, m, no, se, False{}, pe, l}) == n : String}

# a bot only reported by a linked machine shows in a bot's view with one
# tab, its chat, and nothing that acts on its machine
law bot_head_remote:
  for +u: U.Ui
  for +id: String
  for +t: String
  for +lk: Nat
  for +m: String
  {BU.BU.head.m(True{}, u, id, t, lk, m) == BU.Head{t, lk, m, BU.Mood.note(m), ["chat"], "chat", False{}, False{}} : BU.Head}

law bot_head_remote_view:
  for +u: U.Ui
  for +id: String
  {BU.Head.tabs(BU.BU.head(u, BU.VRemote{id})) == ["chat"] : List<&2, String>}

# a mirrored bot's view has the tabs its machine shares, no viewer (its
# files are on its machine), and its memory and routines are read here
law far_bot_head_tabs:
  for +u: U.Ui
  for +t: String
  for +lk: Nat
  for +m: String
  {BU.Head.tabs(BU.BU.head.m(False{}, u, "p1~b1", t, lk, m)) == ["chat", "space", "memory", "routines"] : List<&2, String>}

law far_bot_head_stays_home:
  for +u: U.Ui
  for +t: String
  for +lk: Nat
  for +m: String
  {Bool.or(BU.Head.viewer(BU.BU.head.m(False{}, u, "p1~b1", t, lk, m)), BU.Head.owns(BU.BU.head.m(False{}, u, "p1~b1", t, lk, m))) == False{} : Bool}

law bot_head_here_owns:
  for +u: U.Ui
  for +t: String
  for +lk: Nat
  for +m: String
  {Bool.and(BU.Head.viewer(BU.BU.head.m(False{}, u, "b1", t, lk, m)), BU.Head.owns(BU.BU.head.m(False{}, u, "b1", t, lk, m))) == True{} : Bool}

# on a phone paired with a bot's own hub and a hub that mirrors it, the
# mirrored copy adds nothing: the bot shows once, as its own hub's
law hubs_bots_mirror_dropped:
  for +far: Bool
  for +want: Bool
  for +hs: List<&2, Hb.Hub>
  for +from: String
  for +r: J.Json
  for rest: List<&2, ON.Ky<J.Json>>
  {Hb.Hubs.bot.keep.k(True{}, far, want, hs, from, r, rest) == rest : List<&2, ON.Ky<J.Json>>}
# Text fitting: early exit preserves the original recurrence.

law fit_count_guard:
  for +s: String
  for +fits: Bool
  for +cache: Map<&2, X.TG>
  for +f: X.Face
  for +max: U32
  for +used: U32
  {X.Fit.count.go(s, fits, cache, f, max, used) == X.Fit.count.k(fits, X.Fit.count(s, cache, f, max, used)) : Nat}

law fit_count_step:
  for +cp: U32
  for +s: String
  for +cache: Map<&2, X.TG>
  for +f: X.Face
  for +max: U32
  for +used: U32
  {X.Fit.count(SCon{Chr{cp}, s}, cache, f, max, used) ==
    X.Fit.count.k(U32.is_le((used + X.TG.adv(X.Text.get(cache, f, cp)) : U32), max),
      X.Fit.count(s, cache, f, max, (used + X.TG.adv(X.Text.get(cache, f, cp)) : U32))) : Nat}

law fit_chunk_progress:
  for n: Nat
  exs k: Nat
  {X.Fit.atleast(n) == 1n+k : Nat}

law fit_wrap_unicode:
  {X.Text.wrap(Map.new(&2, X.TG), X.Text.bitmap(), "日本語 😀🚀🙂\nabcde  x\n", 24) ==
    ["日本語", "😀🚀🙂", "abc", "de ", "x", ""] : List<&2, String>}
# Per-peer remote bot directories and bounded refreshes

law remote_dir_failed_peer:
  for +xs: List<&2, J.Json>
  for +p: String
  {BHS.Dir.merge(None{}, xs, p) == BHS.Dir.away(xs, p) : List<&2, J.Json>}

law remote_dir_away_seen:
  for +p: String
  for +n: String
  for +look: Nat
  for +seen: Nat
  {J.Json.get_obj(BHS.Dir.away.one(BHS.Dir.item(p, "machine", look, n, "work", seen)), "seen") == J.Json.nat(seen) : J.Json}

law remote_dir_arrival_order:
  {BHS.Dir.merge(Some{[BHS.Dir.item("a", "alpha", 0n, "miso", "ready", 1n)]},
      [BHS.Dir.item("b", "beta", 0n, "nori", "ready", 1n)], "a") ==
    BHS.Dir.merge(Some{[BHS.Dir.item("b", "beta", 0n, "nori", "ready", 1n)]},
      [BHS.Dir.item("a", "alpha", 0n, "miso", "ready", 1n)], "b") : List<&2, J.Json>}

law remote_dir_other_unchanged:
  for +o: J.Json
  {BHS.Dir.changed(False{}, o) == o : J.Json}

law remote_dir_obsolete_ignored:
  for +xs: List<&2, J.Json>
  for +p: String
  for +r: Maybe<&2, List<&2, J.Json>>
  {BHS.Dir.accept(False{}, r, xs, p) == xs : List<&2, J.Json>}

law remote_dir_known_not_queued:
  for +a: List<&2, String>
  for +q: List<&2, String>
  for +p: String
  {BHS.DirWork.add.k(True{}, a, q, p) == BHS.DirWork{a, q} : BHS.DirWork}

law remote_dir_four_slots:
  {BHS.DirPlan.launch(BHS.DirWork.plan(BHS.DirWork{[], ["a", "b", "c", "d", "e"]})) == ["a", "b", "c", "d"] : List<&2, String>}

law remote_dir_freed_slot:
  {BHS.DirPlan.launch(BHS.DirWork.plan(BHS.DirWork.finish(BHS.DirWork{["d", "c", "b", "a"], ["e"]}, "b"))) == ["e"] : List<&2, String>}

# A pending selection cannot consume another hub's read model.
law want_other_hub:
  for +u: U.Ui
  for +generation: Bool
  for +installed: Bool
  {U.Bots.wanted.k(U.Want.accept(False{}, generation, installed), u) == u : U.Ui}

law want_old_generation:
  for +u: U.Ui
  for +hub: Bool
  for +installed: Bool
  {U.Bots.wanted.k(U.Want.accept(hub, False{}, installed), u) == u : U.Ui}

law want_before_install:
  for +u: U.Ui
  for +hub: Bool
  for +generation: Bool
  {U.Bots.wanted.k(U.Want.accept(hub, generation, False{}), u) == u : U.Ui}

law machine_clears_selection:
  for +u: U.Ui
  for +hub: String
  {U.Ui.sel(U.Act.ui(U.Ui.machine(u, hub))) == "" : String}

law timeline_hidden_settings:
  for +selected: Bool
  for +thread: Bool
  {U.Timeline.visible(True{}, selected, thread) == False{} : Bool}

law timeline_hidden_pane:
  for +settings: Bool
  for +selected: Bool
  {U.Timeline.visible(settings, selected, False{}) == False{} : Bool}

# Cache walks return at most one home and one linked snapshot.
law hub_cache_home_short:
  for +vs: List<&2, U.HubView>
  {List.drop(&2, U.HubView, U.HubCache.home(vs), 1n) == Nil{} : List<&2, U.HubView>}

law hub_cache_remote_short:
  for +vs: List<&2, U.HubView>
  {List.drop(&2, U.HubView, U.HubCache.remote(vs), 1n) == Nil{} : List<&2, U.HubView>}

law hub_cache_bounded:
  for +v: U.HubView
  for +vs: List<&2, U.HubView>
  {List.drop(&2, U.HubView, U.HubCache.put(v, vs), 2n) == Nil{} : List<&2, U.HubView>}

# Owner-shared projects and ordinary threads use the far projection too.
law far_project_named:
  for +l: String
  for +id: String
  for title: String
  for root: String
  for at: Nat
  {FR.Fr.change(l, M.ProjectCreated{id, title, root, at}) == Some{M.ProjectCreated{FR.Fr.id(l, id), title, root, at}} : Maybe<&2, M.Change>}

law far_project_shared:
  for s: M.State
  {FR.Fr.shared(s, M.ProjectCreated{"p1", "Board", "/board", 1n}) == True{} : Bool}

law far_project_thread_shared:
  {FR.Fr.thread.shared(M.State.replay([M.ProjectCreated{"p", "Board", "/board", 1n},
    M.ThreadCreated{"t", "p", "Layout", M.Local{}, "", "", "claude", "", 2n}], M.State.empty()), "t") == True{} : Bool}

law far_label_here:
  for +s: M.State
  for +id: String
  for title: String
  {FR.Fr.title.k(False{}, s, id, title) == title : String}

law far_label_away:
  {FR.Fr.title(FR.Fr.mark(M.State.empty(), "p1", "desk", True{}), "p1~t", "Layout") == "Layout · desk (away)" : String}

law far_cursor_keeps_marker:
  {FR.Fr.title(FR.Fr.head.set(FR.Fr.mark(M.State.empty(), "p1", "desk", True{}), "p1", 5n), "p1~t", "Layout") == "Layout · desk (away)" : String}

law far_routine_routes_home:
  {FR.Fr.params.local(J.Field{"routine", J.Str{"p1~r1"}, J.End{}}, "p1") == J.Field{"routine", J.Str{"r1"}, J.End{}} : J.Json}

law hubs_catalog_mirror_dropped:
  for r: J.Json
  for rest: List<&2, J.Json>
  {Hb.Hubs.part.k(Bool.not(True{}), r, rest) == rest : List<&2, J.Json>}

law hubs_sidebar_view_everywhere:
  {Hb.Hubs.every.op(Hb.Do{"setting", "sidebar.view=active"}) == True{} : Bool}

# History reset and restart races remain isolated to their owner.
law far_epoch_legacy:
  for +old: String
  {FR.Fr.epoch.changed(old, "") == False{} : Bool}

law far_epoch_same:
  {FR.Fr.epoch.changed("history1", "history1") == False{} : Bool}

law far_epoch_reset:
  {FR.Fr.epoch.changed("history1", "history2") == True{} : Bool}

law far_reset_keeps_sequence:
  for s: M.State
  for +l: String
  {M.State.seq(FR.Fr.forget(s, l)) == M.State.seq(s) : Nat}

law far_loaded_live_wins:
  {FR.In.head(FR.In.loaded(FR.In.put(FR.In.new("p1", "desk"), 0n, 9n, Nil{}),
    FR.In.new("p1", "desk"))) == 9n : Nat}

law far_reply_preserves_error:
  {FR.Fr.reply(J.Json.obj([J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{"declined"}}]), "7") ==
    "{\"id\":7,\"ok\":false,\"error\":\"declined\"}" : String}

law far_draft_kept:
  for text: String
  {U.Draft.keep("p1~t1", text) == [U.Keep{"p1~t1", text}] : List<&2, U.Cmd>}

law hubs_fold_null:
  for +older: Bool
  for +hs: List<&2, Hb.Hub>
  for +from: String
  {Hb.Hubs.fold.json(J.Null{}, older, hs, from) == J.Null{} : J.Json}

law hubs_origin_here:
  for p: J.Json
  for name: String
  {Hb.Hubs.origin.project(False{}, p, name) == p : J.Json}

law hubs_origin_elsewhere:
  {J.Json.get_str(Hb.Hubs.origin.project(True{}, J.Json.obj([J.KV{"title", J.Str{"Board"}}]), "desk"), "machine", "") == "desk" : String}

law far_arrival_no_choice:
  for u: U.Ui
  {U.Far.arrived.k(False{}, u) == u : U.Ui}

law far_arrival_not_shared:
  for u: U.Ui
  {U.Far.arrived.m(None{}, u) == u : U.Ui}

law far_arrival_thread_selected:
  for u: U.Ui
  for id: String
  for name: String
  for +thread: String
  {U.Ui.sel(U.Far.arrived.m(Some{BOT.Bot{id, name, thread, 1n, "", 0n, False{}}}, u)) == thread : String}

law far_arrival_focus_unchanged:
  for n: N.Nui
  {N.Nui.unremote.k(False{}, n) == n : N.Nui}

law hubs_origin_label_plain:
  for v: J.Json
  for name: String
  for online: Bool
  {Hb.Hubs.origin.label(False{}, v, name, online) == v : J.Json}

law hubs_origin_title_here:
  for title: String
  for name: String
  {Hb.Hubs.origin.title(True{}, title, name, True{}) == title ++ " · " ++ name : String}

law hubs_origin_title_away:
  for title: String
  for name: String
  {Hb.Hubs.origin.title(True{}, title, name, False{}) == title ++ " · " ++ name ++ " (away)" : String}
# a subagent started by an Agent call keeps the call and what it was asked
# while its progress changes what it does
law bg_start_keeps_call:
  {M.Bg.show(M.Bg.set(M.Bg.start(Nil{}, "a", "x", "c1", "List files", "subagent"), "a", "Running ls")) == "a\tRunning ls\tc1\tList files\tsubagent" : String}

law bg_call_title:
  {M.Bg.call("a\tx\tc1\tList files") ++ "|" ++ M.Bg.title("a\tx\tc1\tList files") == "c1|List files" : String}

# a subagent at work shows what it does now and its steps from its log,
# open until the person shuts it
law subs_agent_at_work:
  {SB.Subs.rows(M.State.replay([M.SettingSet{"bg.t1", "a\tRunning ls\tc1\tList files\tsubagent", 1n},
      M.ActivityLogged{"@sub:t1", "c1/start:subagent", "c1", "start:subagent", "List files", 1n}, M.ActivityLogged{"@sub:t1", "c1/u1", "c1", "Bash", "ls\n-la", 2n}], M.State.empty()),
    Map.new(&2, String), "t1", 3n)
    == [SB.SubRow{"c1", True{}, "List files", "subagent", "running", "Running ls", True{}, True{}, 1n, [SB.SubStep{"Bash", "ls"}]}] : List<&2, SB.SubRow>}

# a finished one stays listed behind the shut "N done" toggle (changed
# by ui-panel-paste-lag spec 1: finished rows fold under one toggle row)
law subs_agent_done_shut:
  {SB.Subs.rows(M.State.replay([M.ActivityLogged{"@sub:t1", "c1/start:subagent", "c1", "start:subagent", "List files", 1n},
      M.ActivityLogged{"@sub:t1", "c1/u1", "c1", "Bash", "ls", 2n}, M.ActivityLogged{"@sub:t1", "c1/end", "c1", "end", "completed", 3n}], M.State.empty()),
    Map.new(&2, String), "t1", 3n)
    == [SB.SubRow{"subs.done:t1", True{}, "1 done", "", "", "", False{}, False{}, 1n, []}] : List<&2, SB.SubRow>}


# a background shell is listed by its kind, and is not an agent in the
# sidebar's count
law subs_shell_named:
  {SB.Subs.rows(M.State.replay([M.SettingSet{"bg.t1", "b\tRunning\tc2\tscripts/check.sh\tshell", 1n}], M.State.empty()), Map.new(&2, String), "t1", 3n)
    == [SB.SubRow{"c2", True{}, "shell: scripts/check.sh", "shell", "running", "Running", True{}, True{}, 0n, []}] : List<&2, SB.SubRow>}

law subs_shell_not_agent:
  {SB.Subs.count(M.State.replay([M.SettingSet{"bg.t1", "b\tRunning\tc2\tscripts/check.sh\tshell\na\tx\tc1\tList files\tsubagent", 1n}], M.State.empty()), "t1") == 1n : Nat}

law subs_badge:
  {SB.Subs.badge(0n) ++ "|" ++ SB.Subs.badge(1n) ++ "|" ++ SB.Subs.badge(3n) == "|1 agent|3 agents" : String}

# Desks (core/desk.bend)
# ----------------------

# a client that never said desk.look (a phone) watches nothing
law desk_phone_never:
  for thread: String
  for now: Nat
  {DK.Look.sees(DK.Look.none(), thread, now) == False{} : Bool}

# a desk without the focus watches nothing
law desk_unfocused_never:
  for d: Bool
  for th: String
  for at: Nat
  for thread: String
  for now: Nat
  {DK.Look.sees(DK.DeskView{d, th, False{}, at}, thread, now) == False{} : Bool}

# a focused desk watches its thread while in use, and stops once a window
# passes with no word from it
law desk_look_fresh:
  {DK.Look.sees(DK.DeskView{True{}, "t", True{}, 100n}, "t", 279n) == True{} : Bool}

law desk_look_expires:
  {DK.Look.sees(DK.DeskView{True{}, "t", True{}, 100n}, "t", 280n) == False{} : Bool}

# a thread a desk watches raises no push alert
law desk_seen_holds:
  for key: String
  {DK.Desk.hold(True{}, key) == "" : String}

# nor a desktop notification
law desk_seen_nobody:
  for xs: List<&2, DK.At>
  {DK.Desk.to.k(True{}, xs) == None{} : Maybe<&2, U32>}

# with no desk connected there is no desktop notification
law desk_none_nobody:
  {DK.Desk.lead(Nil{}) == None{} : Maybe<&2, U32>}

# nor the phone's own alert
law alert_watched_quiet:
  for kind: String
  for id: String
  for title: String
  for st: M.State
  for es: List<&2, M.Entry>
  for rest: List<&2, J.Json>
  {Nt.Note.alert.w(True{}, kind, id, title, st, es, rest) == rest : List<&2, J.Json>}

# a desk that never said its look says it at once
law desk_due_first:
  for last_at: String
  for thread: String
  for focused: Bool
  for now: Nat
  {DK.Desk.due("", last_at, thread, focused, now) == True{} : Bool}
# Design history (core/hist.bend)
# --------------------------------

# a version compared with itself marks nothing: every item stays on its
# layer, dimmed
law hist_mark_self:
  for xs: List<&2, B.Item>
  {HS.Mark.go(xs, xs, HS.Mark.heads(xs, xs)) == HS.Mark.dims(xs) : List<&2, B.Item>}

# a file in a commit and a comparison ending on it are of the live file,
# so moving between them keeps the viewer's scene and view
law hist_version_family:
  {HS.Hist.family(HS.Hist.git("/r/.git", "c1", "/r", "hw/a.kicad_pcb")) == "/r/hw/a.kicad_pcb" : String}

law hist_compare_family:
  {HS.Hist.family(HS.Hist.diff(HS.Hist.git("/r/.git", "c1", "/r", "a.kicad_pcb"), "/r/a.kicad_pcb")) == "/r/a.kicad_pcb" : String}

# git show names the file from the top, a sheet's ".." folded
law hist_obj_folds:
  {HS.Hist.git.obj(HS.Hist.git("/r/.git", "c1", "/r", "hw/sub/../a.kicad_sch")) == "c1:hw/a.kicad_sch" : String}

# a sheet in a commit resolves beside its parent, in the same commit
law hist_sheet_same_commit:
  {HI.Hier.resolve(HS.Hist.git("/g", "c", "/t", "hw/root.kicad_sch"), "kid.kicad_sch") == HS.Hist.git("/g", "c", "/t", "hw/kid.kicad_sch") : String}

# the arrows: forward from the last step is the live file, back from the
# live file the last step
law hist_fwd_last_live:
  for +n: Nat
  {HS.Hist.fwd(n, n) == 0n : Nat}

law hist_back_live_last:
  for +n: Nat
  {HS.Hist.back(n, 0n) == n : Nat}

# no step is named: the live file
law hist_index_live:
  for ss: List<&2, HS.Step>
  {HS.Hist.index(ss, "") == 0n : Nat}

# a capture that changed nothing logs nothing
law hist_record_nothing:
  for +e: String
  {HS.Hist.record("", e) == "" : String}

# a board's or schematic's own look is untouched by comparisons; a
# comparison's marks each have a colour of their own
law look_plain_board:
  for +layer: U32
  {LO.Look.of(0, layer) == LO.Look.plain(0, layer) : U32 & U32}

law look_plain_sch:
  for +layer: U32
  {LO.Look.of(1, layer) == LO.Look.plain(1, layer) : U32 & U32}

law look_marks_distinct:
  {Bool.or(U32.is_eq(LO.Look.mark.color(False{}, 26), LO.Look.mark.color(False{}, 27)), Bool.or(U32.is_eq(LO.Look.mark.color(False{}, 27), LO.Look.mark.color(False{}, 28)), U32.is_eq(LO.Look.mark.color(False{}, 26), LO.Look.mark.color(False{}, 28)))) == False{} : Bool}

# the layer list reaches a comparison's marks, whatever the kind's own count
law look_names_marks:
  {List.drop(&2, String, LO.Look.names(U32.or(0, LO.Look.diff()), B.Layer.names()), 26n) == ["Removed", "Changed", "Added"] : List<&2, String>}

# Scratch threads (M.Scratch): a pinned one is kept, whatever else holds
law scratch_pinned_kept:
  for +now: Nat
  for +w: Nat
  for +id: String
  for +ti: String
  for +c: Nat
  for +a: Nat
  for tu: M.TurnState
  for +pe: Nat
  for ar: Bool
  for o: M.Override
  for +sa: Nat
  for +ua: Nat
  for env: M.EnvMode
  for +br: String
  for +wt: String
  for +pr: String
  for +mo: String
  for rt: M.RuntimeMode
  for im: M.InteractionMode
  for +se: String
  for de: Bool
  {M.Scratch.expired(now, w, M.Thread{id, "scratch", ti, c, a, tu, pe, ar, o, sa, ua, True{}, env, br, wt, pr, mo, rt, im, se, de}) == False{} : Bool}

# a scratch thread whose turn runs is never taken away under it
law scratch_running_kept:
  for +now: Nat
  for +w: Nat
  for +id: String
  for +ti: String
  for +c: Nat
  for +a: Nat
  for +pe: Nat
  for ar: Bool
  for o: M.Override
  for +sa: Nat
  for +ua: Nat
  for pin: Bool
  for env: M.EnvMode
  for +br: String
  for +wt: String
  for +pr: String
  for +mo: String
  for rt: M.RuntimeMode
  for im: M.InteractionMode
  for +se: String
  for de: Bool
  {M.Scratch.expired(now, w, M.Thread{id, "scratch", ti, c, a, M.Running{}, pe, ar, o, sa, ua, pin, env, br, wt, pr, mo, rt, im, se, de}) == False{} : Bool}

# only Scratch's threads ever expire
law scratch_project_kept:
  for +now: Nat
  for +w: Nat
  for +id: String
  for +ti: String
  for +c: Nat
  for +a: Nat
  for tu: M.TurnState
  for +pe: Nat
  for ar: Bool
  for o: M.Override
  for +sa: Nat
  for +ua: Nat
  for pin: Bool
  for env: M.EnvMode
  for +br: String
  for +wt: String
  for +pr: String
  for +mo: String
  for rt: M.RuntimeMode
  for im: M.InteractionMode
  for +se: String
  for de: Bool
  {M.Scratch.expired(now, w, M.Thread{id, "p1", ti, c, a, tu, pe, ar, o, sa, ua, pin, env, br, wt, pr, mo, rt, im, se, de}) == False{} : Bool}

# an idle scratch thread goes once its window has passed, not before
law scratch_expires:
  {M.Scratch.expired(1000n, 100n, M.Thread{"t", "scratch", "x", 0n, 900n, M.Idle{}, 0n, False{}, M.NoOverride{}, 0n, 0n, False{}, M.Local{}, "", "", "", "", M.FullAccess{}, M.DefaultMode{}, "", False{}}) == True{} : Bool}

law scratch_not_yet:
  {M.Scratch.expired(999n, 100n, M.Thread{"t", "scratch", "x", 0n, 900n, M.Idle{}, 0n, False{}, M.NoOverride{}, 0n, 0n, False{}, M.Local{}, "", "", "", "", M.FullAccess{}, M.DefaultMode{}, "", False{}}) == False{} : Bool}

# the window in days: seven unless set, at most ninety
law scratch_days_default:
  {M.Scratch.days.k(0n) == 7n : Nat}

law scratch_days_most:
  {M.Scratch.days.k(91n) == 90n : Nat}

# the Scratch project is made once, and a removed one stays removed
law scratch_made:
  {M.Scratch.ensure(M.State.empty(), "/h/scratch", 1n) == [M.ProjectCreated{"scratch", "Scratch", "/h/scratch", 1n}] : List<&2, M.Change>}

law scratch_removed_stays:
  {M.Scratch.ensure(M.State.replay([M.ProjectCreated{"scratch", "Scratch", "/h/scratch", 1n}, M.ProjectRemoved{"scratch", 2n}], M.State.empty()), "/h/scratch", 3n) == [] : List<&2, M.Change>}

# Scratch never folds away in the sidebar, even with nothing in it
law scratch_always_live:
  for +now: Nat
  {U.SRow.group(now, U.SRow{0n, M.Project{"scratch", "Scratch", "/h/scratch", 0n, False{}}, M.Shelves{[], [], [], [], []}}) == 0n : Nat}

# "/give" is a command only as its own word, and never a side question
law give_command:
  {U.Give.is("/give >notes keep this") == True{} : Bool}

law give_not_a_word:
  {U.Give.is("/giver") == False{} : Bool}

law give_not_btw:
  {U.Btw.is("/give >notes") == False{} : Bool}

# a give that names nothing changes nothing and says why
law give_nothing_refused:
  for +c: U32
  for +id: String
  for os: List<&2, H.Out>
  {BHS.Give.done(H.Step{[], os}, c, id) == H.Step{[], H.Reply.err(c, id, "name a thread (>), a project (%) or a bot (@) to give this thread to") <> os} : H.Step}

# API keys (core/apikeys.bend): a name becomes an environment variable, a
# value is single-quoted for a shell, setting a name replaces its line and
# keeps the rest, and agents get nothing when there are no keys
law keys_name_env:
  {AK.Keys.name(" openai key ") == "OPENAI_KEY" : String}

law keys_name_digit:
  {AK.Keys.name("2captcha") == "_2CAPTCHA" : String}

law keys_quote_single:
  {AK.Keys.line("a", "it's") == "A='it'\\''s'" : String}

law keys_set_replaces:
  {AK.Keys.set("A='1'\nB='2'\n", "a", "3") == "B='2'\nA='3'\n" : String}

law keys_drop_keeps_others:
  {AK.Keys.drop("# mine\nA='1'\nB='2'\n", "A") == "# mine\nB='2'\n" : String}

law keys_names_of_lines:
  {AK.Keys.names("# note\nA='x=y'\n\nB='2'\n") == ["A", "B"] : List<&2, String>}

law keys_prompt_none:
  {AK.Keys.prompt("/h/secrets/api.env", "") == "" : String}

# Markdown
# a separator row makes the row above it the table's head, and only when
# that row is the table's first
law md_head_first:
  for cs: List<&2, String>
  {DM.Md.headed([DM.RBody{cs}]) == [DM.RHead{cs}] : List<&2, DM.Row>}

law md_head_only_first:
  for cs: List<&2, String>
  for r: DM.Row
  for rs: List<&2, DM.Row>
  {DM.Md.headed(DM.RBody{cs} <> r <> rs) == DM.RBody{cs} <> r <> rs : List<&2, DM.Row>}

# a fence opens a code block whatever follows it on the line
law md_fence_any:
  for s: String
  {DM.Md.line("```" ++ s) == DM.LFence{} : DM.Line}

# The viewer's Files tab (core/fm.bend): what a file opens as, any case;
# paths from the project's root that never leave it
law fm_kind_board:
  {FMG.Fm.kind("hw/main.kicad_pcb") == "board" : String}

law fm_kind_schematic:
  {FMG.Fm.kind("main.kicad_sch") == "schematic" : String}

law fm_kind_model:
  {FMG.Fm.kind("mech/case.STEP") == "3d" : String}

law fm_kind_pdf:
  {FMG.Fm.kind("docs/Datasheet.PDF") == "pdf" : String}

law fm_kind_image:
  {FMG.Fm.kind("render.png") == "image" : String}

law fm_kind_other:
  {FMG.Fm.kind("notes.txt") == "" : String}

law fm_safe_up:
  {FMG.Fm.safe("lib/../../etc") == False{} : Bool}

law fm_safe_abs:
  {FMG.Fm.safe("/etc") == False{} : Bool}

law fm_safe_down:
  {FMG.Fm.safe("hw/main") == True{} : Bool}

law fm_parent_top:
  {FMG.Fm.parent("hw") == "" : String}

law fm_parent_down:
  {FMG.Fm.parent("hw/main") == "hw" : String}

law fm_join_root:
  {FMG.Fm.join("", "hw") == "hw" : String}

# Terminals in the chat (core/emb.bend)
# -------------------------------------

# a terminal entry's summary gives back its title, command and size
law emb_summary_round:
  {EMB.Emb.title(EMB.Emb.summary("Pick", 100, 30, "py pick.py")) ++ "|" ++ EMB.Emb.cmd(EMB.Emb.summary("Pick", 100, 30, "py pick.py")) == "Pick|py pick.py" : String}

law emb_size_round:
  {(EMB.Emb.wide(EMB.Emb.summary("t", 100, 30, "c")) + EMB.Emb.tall(EMB.Emb.summary("t", 100, 30, "c")) : U32) == 130 : U32}

# a pinned terminal stays live whatever comes after it
law emb_pinned_live:
  for touched: Nat
  for last: Nat
  for now: Nat
  {EMB.Emb.live(True{}, touched, last, now) == True{} : Bool}

# a message the person sends after it ends it
law emb_message_ends:
  {EMB.Emb.live(False{}, 10n, 11n, 12n) == False{} : Bool}

law emb_fresh_live:
  {EMB.Emb.live(False{}, 10n, 9n, 12n) == True{} : Bool}

# so do 30 minutes untouched
law emb_idle_ends:
  {EMB.Emb.live(False{}, 10n, 0n, 1810n) == False{} : Bool}

law emb_note_none:
  {EMB.Emb.note(Nil{}) == "" : String}

law emb_focus_round:
  {EMB.Emb.focused(EMB.Emb.focus(EMB.Emb.key("t1", "x5"))) == "x:t1:x5" : String}
# Claude's command ledger (core/cmds.bend): a turn ends when its own
# command's lifecycle does, a result while another command is outstanding
# never ends it (the live repro: a stale notification turn's result while
# the user's message waited), a stopped command's late error is quiet, and
# a turn Claude starts by itself shows as running
law cmds_foreign_result_quiet:
  {CM.Cmds.verdict(CM.Cmds.one("u1", 1n, False{}), True{}, CM.Cmds.o(CM.Cmds.l.result())) == 1n : Nat}

law cmds_foreign_result_keeps_running:
  {H.Hub.running(H.Hub.thread(M.State.replay(H.Queue.changes(H.Hub.agent_line.v(1n, H.Demo.state("claude", M.Running{}), 9n, "t1", CM.Cmds.l.result(), "")), H.Demo.state("claude", M.Running{})), "t1")) == True{} : Bool}

law cmds_result_waits_lc:
  {CM.Cmds.verdict(CM.Cmds.one("u1", 2n, False{}), True{}, CM.Cmds.o(CM.Cmds.l.result())) == 1n : Nat}

law cmds_lc_completed_ends:
  {CM.Cmds.verdict(CM.Cmds.see(CM.Cmds.one("u1", 2n, False{}), CM.Cmds.o(CM.Cmds.l.result())), True{}, CM.Cmds.o(CM.Cmds.l.lc("u1", "completed"))) == 3n : Nat}

law cmds_lc_end_completes:
  {M.Thread.turn(H.Hub.thread(M.State.replay(H.Queue.changes(H.Hub.agent_line.v(3n, H.Demo.state("claude", M.Running{}), 9n, "t1", CM.Cmds.l.lc("u1", "completed"), "ok")), H.Demo.state("claude", M.Running{})), "t1")) == M.Completed{} : M.TurnState}

law cmds_pair_stays_running:
  {CM.Cmds.verdict(CM.Cmds.see(CM.Cmds{[CM.Cmd{"u3", 2n, False{}, ""}, CM.Cmd{"u4", 1n, False{}, ""}], 1n, False{}}, CM.Cmds.o(CM.Cmds.l.result())), True{}, CM.Cmds.o(CM.Cmds.l.lc("u3", "completed"))) == 1n : Nat}

law cmds_tomb_error_quiet:
  {CM.Cmds.verdict(CM.Cmds.stop(CM.Cmds.one("u1", 2n, False{}), []), False{}, CM.Cmds.o(CM.Cmds.l.failed())) == 1n : Nat}

law cmds_tomb_cancel_quiet:
  {CM.Cmds.verdict(CM.Cmds.see(CM.Cmds.stop(CM.Cmds.one("u1", 2n, False{}), []), CM.Cmds.o(CM.Cmds.l.failed())), False{}, CM.Cmds.o(CM.Cmds.l.lc("u1", "cancelled"))) == 1n : Nat}

# after a Stop, the next message's own lifecycle ends its turn, and the
# stopped one's error never fails it
law cmds_stop_then_new_error_quiet:
  {CM.Cmds.verdict(CM.Cmds.sent(CM.Cmds.accept(CM.Cmds.stop(CM.Cmds.one("u1", 2n, False{}), []), "u2"), "u2"), True{}, CM.Cmds.o(CM.Cmds.l.failed())) == 1n : Nat}

law cmds_stop_then_new:
  {CM.Cmds.verdict(CM.Cmds.see(CM.Cmds.see(CM.Cmds.see(CM.Cmds.see(CM.Cmds.sent(CM.Cmds.accept(CM.Cmds.stop(CM.Cmds.one("u1", 2n, False{}), []), "u2"), "u2"),
      CM.Cmds.o(CM.Cmds.l.failed())), CM.Cmds.o(CM.Cmds.l.lc("u1", "cancelled"))), CM.Cmds.o(CM.Cmds.l.lc("u2", "started"))), CM.Cmds.o(CM.Cmds.l.result())),
    True{}, CM.Cmds.o(CM.Cmds.l.lc("u2", "completed"))) == 3n : Nat}

law cmds_wake_start:
  {CM.Cmds.verdict(CM.Cmds{[], 1n, False{}}, False{}, CM.Cmds.o(CM.Cmds.l.note())) == 6n : Nat}

law cmds_wake_runs:
  {H.Hub.running(H.Hub.thread(M.State.replay(H.Queue.changes(H.Hub.agent_line.v(6n, H.Demo.state("claude", M.Completed{}), 9n, "t1", CM.Cmds.l.note(), "")), H.Demo.state("claude", M.Completed{})), "t1")) == True{} : Bool}

law cmds_wake_result_ends:
  {CM.Cmds.verdict(CM.Cmds{[], 1n, True{}}, True{}, CM.Cmds.o(CM.Cmds.l.result())) == 3n : Nat}

law cmds_wake_with_queued_user:
  {CM.Cmds.verdict(CM.Cmds.one("u1", 1n, True{}), True{}, CM.Cmds.o(CM.Cmds.l.result())) == 1n : Nat}

law cmds_tomb_started_resumes:
  {CM.Cmds.verdict(CM.Cmds.one("u1", 4n, False{}), False{}, CM.Cmds.o(CM.Cmds.l.lc("u1", "started"))) == 6n : Nat}

law cmds_tomb_started_says:
  {H.Wake.text(True{}, H.Demo.state("claude", M.Interrupted{}), "t1", CM.Cmds.o(CM.Cmds.l.lc("u1", "started"))) == "Resumed after Stop" : String}

# a message still being prepared (its images read) counts as outstanding
law cmds_prep_counts:
  {CM.Cmds.verdict(CM.Cmds.one("u1", 0n, True{}), True{}, CM.Cmds.o(CM.Cmds.l.result())) == 1n : Nat}

# lifecycles from the first lifecycle line, or from an init new enough;
# an older CLI keeps the old rule (verdict 0)
law cmds_mode_version:
  {CM.Cmds.mode(CM.Cmds.see(CM.Cmds.new(), CM.Cmds.o(CM.Cmds.l.init("2.1.286")))) == 1n : Nat}

law cmds_mode_old:
  {CM.Cmds.mode(CM.Cmds.see(CM.Cmds.new(), CM.Cmds.o(CM.Cmds.l.init("2.0.0")))) == 2n : Nat}

law cmds_mode_old_legacy:
  {CM.Cmds.verdict(CM.Cmds.see(CM.Cmds.new(), CM.Cmds.o(CM.Cmds.l.init("2.0.0"))), True{}, CM.Cmds.o(CM.Cmds.l.result())) == 0n : Nat}

law cmds_mode_lc:
  {CM.Cmds.mode(CM.Cmds.see(CM.Cmds.new(), CM.Cmds.o(CM.Cmds.l.lc("u1", "queued")))) == 1n : Nat}

# an old CLI answers no command: its ledger does not grow. A command is
# forgotten once written (a ledger that knows it is legacy), and the ones
# written before it knew go when it learns; one still being prepared stays
law cmds_legacy_retires_sent:
  {List.length(&2, CM.Cmd, CM.Cmds.cs(CM.Cmds.sent(CM.Cmds.accept(CM.Cmds.see(CM.Cmds.new(), CM.Cmds.o(CM.Cmds.l.init("2.0.0"))), "u1"), "u1"))) == 0n : Nat}

law cmds_legacy_learned_retires:
  {List.length(&2, CM.Cmd, CM.Cmds.cs(CM.Cmds.see(CM.Cmds.sent(CM.Cmds.accept(CM.Cmds.new(), "u1"), "u1"), CM.Cmds.o(CM.Cmds.l.init("2.0.0"))))) == 0n : Nat}

law cmds_legacy_prep_stays:
  {List.length(&2, CM.Cmd, CM.Cmds.cs(CM.Cmds.see(CM.Cmds.accept(CM.Cmds.new(), "u1"), CM.Cmds.o(CM.Cmds.l.init("2.0.0"))))) == 1n : Nat}

law cmds_lifecycle_keeps_sent:
  {List.length(&2, CM.Cmd, CM.Cmds.cs(CM.Cmds.sent(CM.Cmds.accept(CM.Cmds.see(CM.Cmds.new(), CM.Cmds.o(CM.Cmds.l.init("2.1.286"))), "u1"), "u1"))) == 1n : Nat}

law cmds_legacy_same:
  for s: M.State
  for now: Nat
  for thread: String
  for line: String
  for text: String
  {H.Hub.agent_line.v(0n, s, now, thread, line, text) == H.Hub.agent_line(s, now, thread, line) : H.Step}

# a wake is told once: a notification while the wake runs starts nothing
law wake_act_once:
  {CM.Cmds.verdict(CM.Cmds{[], 1n, True{}}, True{}, CM.Cmds.o(CM.Cmds.l.note())) == 2n : Nat}

# Background work (model.bend's Bg ended rows, hub.bend's Bg.sync and
# Bg.settle, the WorkSet change): a finished task stays as work until its
# outcome reaches the agent (its notification, or the next turn's end);
# Claude's open list ends what it no longer lists and adds nothing; the
# rows travel to the owner's other machines as WorkSet, never as a setting
law bg_sync_ends_absent:
  {M.Bg.show(M.Bg.sync(M.Bg.lines("a\tx\nb\ty"), ["b"])) == M.Bg.line("a", M.Bg.ended.mark() ++ "done") ++ "\nb\ty" : String}

law bg_sync_never_adds:
  for ls: List<&2, String>
  for +open: List<&2, String>
  {List.length(&2, String, M.Bg.sync(ls, open)) == List.length(&2, String, ls) : Nat}

law bg_updated_ends_not_drops:
  {M.Bg.show(M.Bg.of(M.State.replay(H.Bg.system(H.Bg.demo(), "t1", 2n, CM.Cmds.o("{\"type\":\"system\",\"subtype\":\"task_updated\",\"task_id\":\"a\",\"patch\":{\"status\":\"completed\"}}")), H.Bg.demo()), "t1"))
    == M.Bg.line("a", M.Bg.ended.mark() ++ "completed") ++ "\tc1\tsleep 8\tshell" : String}

law bg_notification_drops:
  {M.Bg.live(M.Bg.of(M.State.replay(H.Bg.system(H.Bg.demo(), "t1", 2n, CM.Cmds.o("{\"type\":\"system\",\"subtype\":\"task_notification\",\"task_id\":\"a\",\"status\":\"completed\"}")), H.Bg.demo()), "t1")) == 0n : Nat}

# Claude's empty open list ends the row, but its outcome is still owed
law bg_empty_snapshot_keeps_pending:
  {Nat.show(M.Bg.live(M.Bg.of(M.State.replay(H.Bg.system(H.Bg.demo(), "t1", 2n, CM.Cmds.o("{\"type\":\"system\",\"subtype\":\"background_tasks_changed\",\"tasks\":[]}")), H.Bg.demo()), "t1")))
    ++ "/" ++ Nat.show(M.Bg.running(M.Bg.of(M.State.replay(H.Bg.system(H.Bg.demo(), "t1", 2n, CM.Cmds.o("{\"type\":\"system\",\"subtype\":\"background_tasks_changed\",\"tasks\":[]}")), H.Bg.demo()), "t1"))) == "1/0" : String}

law bg_turn_end_settles_ended:
  {M.Bg.live(M.Bg.of(M.State.replay(H.Bg.settle(M.State.replay([M.WorkSet{"t1", "bg", M.Bg.line("a", M.Bg.ended.mark() ++ "done") ++ "\nb\ty", 1n}], M.State.empty()), "t1", 2n),
    M.State.replay([M.WorkSet{"t1", "bg", M.Bg.line("a", M.Bg.ended.mark() ++ "done") ++ "\nb\ty", 1n}], M.State.empty())), "t1")) == 1n : Nat}

law workset_fold_same_as_setting:
  for s: M.State
  for +th: String
  for +kind: String
  for rows: String
  for at: Nat
  {M.State.apply(s, M.WorkSet{th, kind, rows, at}) == M.State.apply(s, M.SettingSet{kind ++ "." ++ th, rows, at}) : M.State}

# (stored with every field; the decode back is test/claude_cmds_test.bend's:
# normalizing Change.decode's 57 arms is too slow for a law)
law store_workset_encoded:
  {J.Json.get_str(C.Change.encode(M.WorkSet{"t1", "bg", "a\tx", 7n}), "$", "") ++ "|" ++ J.Json.get_str(C.Change.encode(M.WorkSet{"t1", "bg", "a\tx", 7n}), "thread", "")
    ++ "|" ++ J.Json.get_str(C.Change.encode(M.WorkSet{"t1", "bg", "a\tx", 7n}), "kind", "") ++ "|" ++ J.Json.get_str(C.Change.encode(M.WorkSet{"t1", "bg", "a\tx", 7n}), "rows", "") == "WorkSet|t1|bg|a\tx" : String}

law far_workset_shared:
  {FR.Fr.shared(H.Bg.demo(), M.WorkSet{"t1", "bg", "a\tx", 7n}) == True{} : Bool}

law far_workset_mapped:
  for +l: String
  for +th: String
  for +kind: String
  for +rows: String
  for +at: Nat
  {FR.Fr.change(l, M.WorkSet{th, kind, rows, at}) == Some{M.WorkSet{FR.Fr.id(l, th), kind, rows, at}} : Maybe<&2, M.Change>}

# a link's history starting over takes its threads' work with it
law far_forget_drops_work:
  {M.Setting.get(FR.Fr.forget(M.State.replay([M.WorkSet{"p1~t1", "bg", "a\tx", 1n}, M.WorkSet{"t2", "bg", "b\ty", 1n}], M.State.empty()), "p1"), "bg.p1~t1", "")
    ++ "|" ++ M.Setting.get(FR.Fr.forget(M.State.replay([M.WorkSet{"p1~t1", "bg", "a\tx", 1n}, M.WorkSet{"t2", "bg", "b\ty", 1n}], M.State.empty()), "p1"), "bg.t2", "") == "|b\ty" : String}

# a task's progress lines pass at most once in Prog.gap() seconds; the rest
# are held (the newest of each task) and land on the server's beat once the
# gap is over, so the newest value always lands
law bg_progress_coalesced:
  {Nat.show(List.length(&2, String, H.PStep.flush(H.Prog.line(100n, [], 102n, H.Prog.l1(), H.Prog.o(H.Prog.l1()))))) ++ "/" ++ Nat.show(List.length(&2, String, H.PStep.pend(H.Prog.line(100n, [], 102n, H.Prog.l1(), H.Prog.o(H.Prog.l1())))))
    ++ "/" ++ Nat.show(List.length(&2, String, H.PStep.flush(H.Prog.line(100n, [], 105n, H.Prog.l1(), H.Prog.o(H.Prog.l1()))))) == "0/1/1" : String}

law bg_progress_one_held_per_task:
  {List.length(&2, String, H.PStep.pend(H.Prog.line(100n, H.PStep.pend(H.Prog.line(100n, [], 101n, H.Prog.l1(), H.Prog.o(H.Prog.l1()))), 102n, H.Prog.l2(), H.Prog.o(H.Prog.l2())))) == 1n : Nat}

law bg_progress_beat_waits:
  {List.length(&2, String, H.PStep.flush(H.Prog.beat(100n, [H.Prog.l1()], 103n))) == 0n : Nat}

law bg_progress_latest_lands:
  {M.Bg.show(M.Bg.of(M.State.replay(H.Queue.changes(H.Prog.flush(H.Bg.demo(), "t1", 3n, H.PStep.flush(H.Prog.beat(100n, H.PStep.pend(H.Prog.line(100n, H.PStep.pend(H.Prog.line(100n, [], 101n, H.Prog.l1(), H.Prog.o(H.Prog.l1()))), 102n, H.Prog.l2(), H.Prog.o(H.Prog.l2()))), 105n)))), H.Bg.demo()), "t1"))
    == M.Bg.show(M.Bg.of(M.State.replay(H.Bg.system(H.Bg.demo(), "t1", 3n, H.Prog.o(H.Prog.l2())), H.Bg.demo()), "t1")) : String}

law bg_progress_is_own_line:
  {H.Prog.is(H.Prog.o(H.Prog.l1())) == True{} : Bool}

# the link's projection removed by cutting the index gives what rebuilding
# it from the kept entries gives (Fr.forget.spec)
law far_forget_equiv:
  {FR.Fr.forget(FR.Fr.demo.idx(), "p1") == FR.Fr.forget.spec(FR.Fr.demo.idx(), "p1") : M.State}

law far_forget_equiv_none:
  {FR.Fr.forget(FR.Fr.demo.idx(), "p9") == FR.Fr.forget.spec(FR.Fr.demo.idx(), "p9") : M.State}

law prompt_background_work:
  {Bool.and(String.contains(PM.Prompt.background(), "wakes you"), Bool.and(String.contains(PM.Prompt.background(), "poll"), String.contains(PM.Prompt.background(), "restart"))) == True{} : Bool}

# One readiness check (model.bend's Work.*): a thread is done when no turn
# runs, nothing is queued and no work is out (unfinished child tasks, its
# background rows, ended ones included, and watched work); alerts,
# thread_wait, settling, scratch expiry, worktree removal, the phone's
# island, delegated tasks and bot replies all ask it
law work_busy_counts_ended_rows:
  {M.Work.busy(M.State.replay([M.WorkSet{"t1", "bg", M.Bg.line("a", M.Bg.ended.mark() ++ "done"), 1n}], M.State.empty()), "t1") == 1n : Nat}

law work_ready_idle:
  {M.Work.ready(H.Answer.demo(False{}), "t1") == True{} : Bool}

law work_not_ready_busy:
  {M.Work.ready(H.Answer.demo(True{}), "t1") == False{} : Bool}

law notice_decide_w_idle_same:
  for f: Bool
  for b: Bool
  for a: Bool
  for p: Bool
  for fi: Bool
  for m: Bool
  {NO.Notice.decide.w(0n, f, b, a, p, fi, m) == NO.Notice.decide(f, b, a, p, fi, m) : Nat}

law notice_busy_quiet:
  for n: Nat
  for b: Bool
  for p: Bool
  for fi: Bool
  for m: Bool
  {NO.Notice.decide.w(1n+n, False{}, b, False{}, p, fi, m) == 0n : Nat}

law notice_busy_fail_alerts:
  for busy: Nat
  for b: Bool
  for a: Bool
  for p: Bool
  for fi: Bool
  for m: Bool
  {NO.Notice.decide.w(busy, True{}, b, a, p, fi, m) == 2n : Nat}

law notice_busy_ask_kept:
  for busy: Nat
  for p: Bool
  for fi: Bool
  for m: Bool
  {NO.Notice.decide.w(busy, False{}, True{}, True{}, p, fi, m) == 1n : Nat}

law wait_busy_not_done:
  {MC.Mcp.done.w(H.Answer.demo(True{}), "t1") == False{} : Bool}

law wait_idle_done:
  {MC.Mcp.done.w(H.Answer.demo(False{}), "t1") == True{} : Bool}

law settle_waits_for_work:
  {H.Hub.settle.go([H.Hub.thread(H.Answer.demo(True{}), "t1")], M.State.navs(H.Answer.demo(True{})), M.Work.kids.map(H.Answer.demo(True{})), H.Answer.demo(True{}), 9n, 2n) == [] : List<&2, M.Change>}

law settle_without_work:
  {H.Hub.settle.go([H.Hub.thread(H.Answer.demo(False{}), "t1")], M.State.navs(H.Answer.demo(False{})), M.Work.kids.map(H.Answer.demo(False{})), H.Answer.demo(False{}), 9n, 2n) == [M.ThreadSettled{"t1", False{}, 9n}] : List<&2, M.Change>}

law scratch_busy_kept:
  for +now: Nat
  for +w: Nat
  for t: M.Thread
  for n: Nat
  {M.Scratch.expired.w(now, w, t, 1n+n) == False{} : Bool}

law worktree_busy_kept:
  for +s: M.State
  for +t: M.Thread
  for +now: Nat
  for +window: Nat
  for n: Nat
  {M.Worktree.drop.w(s, t, now, window, 1n+n) == False{} : Bool}

law island_busy_shows:
  {J.Json.show(J.Json.arr(Nt.Note.lines.w([H.Hub.thread(H.Answer.demo(True{}), "t1")], M.Work.kids.map(H.Answer.demo(True{})), H.Answer.demo(True{}))))
    == "[{\"thread\":\"t1\",\"title\":\"x\",\"doing\":\"waiting on 1\"}]" : String}

# an archived thread is never on the phones' island, running or not
law island_gone_hidden:
  {Nt.Note.lines.w(U.Ui.threads(M.State.replay([M.ThreadArchived{"t1", True{}, 3n}], H.Rec.demo(M.Running{}))), M.Work.kids.map(H.Rec.demo(M.Running{})),
    M.State.replay([M.ThreadArchived{"t1", True{}, 3n}], H.Rec.demo(M.Running{}))) == [] : List<&2, J.Json>}

# a hub the phone no longer reaches adds nothing to the island once it has
# been quiet Note.fresh() seconds (its saved turns are stale); one that
# answers always counts
law island_unheard_empty:
  for ts: List<&2, M.Thread>
  for +st: M.State
  {Nt.Note.island.h(False{}, ts, st) == Nt.Note.island.k([]) : J.Json}

law island_heard_online:
  for +now: Nat
  for +heard: Nat
  for +fresh: Nat
  {Nt.Note.heard.in(True{}, now, heard, fresh) == True{} : Bool}

law island_unheard_late:
  {Nt.Note.heard.in(False{}, 20n, 5n, 10n) == False{} : Bool}

law island_unheard_recent:
  {Nt.Note.heard.in(False{}, 12n, 5n, 10n) == True{} : Bool}

# a Live Activity counts every thread but lists only the first few, so it
# fits the lock screen and a push (Notice.island.max)
law island_lines_capped:
  {J.Json.show(Nt.Note.island.k.named([J.Json.nat(1n), J.Json.nat(2n), J.Json.nat(3n), J.Json.nat(4n), J.Json.nat(5n)], ""))
    == "{\"running\":5,\"headline\":\"5 agents working\",\"lines\":[1,2,3]}" : String}

# a phone's upload ends under the thread it started in, though the phone
# named it with its hub ("<hub>|<id>"): the reply's thread is read as the
# hub's own id, so the spinner clears and the path joins that thread
# (test/attach_test.bend runs it through the client)
law upload_thread_local:
  {U.Att.local("h:1|t1") == "t1" : String}

law upload_thread_plain:
  {U.Att.local("t1") == "t1" : String}

law task_waits_for_child_work:
  {H.Task.finish.w(H.Task.demo(True{}), 9n, "t1", H.Step{[M.TurnChanged{"t1", M.Completed{}, 9n}], []}) == H.Step{[M.TurnChanged{"t1", M.Completed{}, 9n}], []} : H.Step}

law task_ends_when_ready:
  {H.Task.marked(H.Queue.changes(H.Task.finish.w(H.Task.demo(False{}), 9n, "t1", H.Step{[M.TurnChanged{"t1", M.Completed{}, 9n}], []})), "k1") == True{} : Bool}

law task_ended_last_change:
  {H.Task.ended.last([M.TurnChanged{"t1", M.Completed{}, 9n}, M.QueueTaken{"t1", 9n}, M.TurnChanged{"t1", M.Running{}, 9n}], "t1") == H.NotEnded{} : H.Ended}

law task_failed_ends_even_busy:
  {H.Task.marked(H.Queue.changes(H.Task.finish.w(H.Task.demo(True{}), 9n, "t1", H.Step{[M.TurnChanged{"t1", M.Failed{}, 9n}], []})), "k1") == True{} : Bool}

law bot_reply_captured_once:
  {H.Answer.out(True{}, H.Answer.demo(False{}), "t1") == [H.BotReply{"t1", "desk", "hi", "t1:" ++ Nat.show(H.Hub.seq(H.Answer.demo(False{})))}] : List<&2, H.Out>}

law bot_reply_waits_for_work:
  {H.Answer.out(True{}, H.Answer.demo(True{}), "t1") == [] : List<&2, H.Out>}

law phase_failed_before_monitoring:
  for +n: Nat
  for q: Bool
  {M.Phase.of(False{}, False{}, 1n+n, M.Failed{}, q) == M.PhFailed{} : M.Phase}

law status_wait_f_keeps_failed:
  for any: Bool
  {M.Status.wait.f(any, M.StFailed{}) == M.StFailed{} : M.Status}

# Durable steps: a step is one write (ST.Store.text); a delivery's
# TaskDelivered comes after the parent's message, whose id is the task's,
# so a restart finishes a torn delivery once (Task.repair); a torn last
# line is left as one malformed line
law commit_text_joins:
  {ST.Store.text(["a", "b"]) == "a\nb\n" : String}

law task_finish_delivered_last:
  {List.head(&2, M.Change, List.reverse(&2, M.Change, H.Queue.changes(H.Task.finish(H.Task.demo(False{}), 9n, "t1", H.Step{[M.TurnChanged{"t1", M.Completed{}, 9n}], []}))))
    == Some{M.TaskDelivered{"k1", 9n}} : Maybe<&2, M.Change>}

law task_repair_delivers:
  {H.Repair.msgs(H.Queue.changes(H.Task.repair(H.Repair.demo(0n), 9n))) == 1n : Nat}

law task_repair_delivers_once:
  {H.Task.repair(H.Repair.again(H.Repair.demo(0n), 9n), 10n) == H.Step{[], []} : H.Step}

law task_repair_activates_idle:
  {H.Queue.changes(H.Task.repair(H.Repair.demo(1n), 9n)) == [M.TurnChanged{"t0", M.Running{}, 9n}, M.TaskDelivered{"k1", 9n}] : List<&2, M.Change>}

law task_repair_running_marks_only:
  {H.Task.repair(H.Repair.demo(2n), 9n) == H.Step{[M.TaskDelivered{"k1", 9n}], []} : H.Step}

law task_repair_quiet_when_done:
  {H.Task.repair(H.Repair.demo(3n), 9n) == H.Step{[], []} : H.Step}

# (the lines after it read as before: test/native/store_torn_test.bend)
law store_torn_tail_resyncs:
  for +rest: List<&2, String>
  {ST.Store.changes(ST.Store.torn.demo() <> rest) == ST.Store.changes(rest) : List<&2, M.Change>}

law store_torn_tail_newline:
  {ST.Store.tail_fix([123, 34]) ++ "|" ++ ST.Store.tail_fix([123, 10]) ++ "|" ++ ST.Store.tail_fix([]) == "\n||" : String}

law store_fail_names_repair:
  {ST.Store.fail.msg("disk full", True{}) ++ "|" ++ ST.Store.fail.msg("disk full", False{}) == "disk full|disk full; the log's tail could not be repaired" : String}

# An existing history must not be replayed as empty after an IO failure.
law log_read_missing_only:
  {[LOGREAD.LogRead.missing(2), LOGREAD.LogRead.missing(0), LOGREAD.LogRead.missing(5), LOGREAD.LogRead.missing(13)] == [True{}, False{}, False{}, False{}] : List<&2, Bool>}

law log_read_complete_only:
  {[LOGREAD.LogRead.complete(0, 0n), LOGREAD.LogRead.complete(3, 3n), LOGREAD.LogRead.complete(3, 2n), LOGREAD.LogRead.complete(3, 4n)] == [True{}, True{}, False{}, False{}] : List<&2, Bool>}

law log_read_native_capacity:
  {[LOGREAD.LogRead.supported(0), LOGREAD.LogRead.supported(2147483647), LOGREAD.LogRead.supported(2147483648), LOGREAD.LogRead.supported(4294967295)] == [True{}, True{}, False{}, False{}] : List<&2, Bool>}

# torn before the task's state: the child's turn ended, so its task ends
law task_repair_finishes_ended_child:
  {H.Task.marked(H.Queue.changes(H.Task.repair(M.State.replay([M.TurnChanged{"t1", M.Completed{}, 5n}], H.Task.demo(False{})), 9n)), "k1") == True{} : Bool}

# a child still at work keeps its task
law task_repair_leaves_working_child:
  {H.Task.repair(H.Task.demo(False{}), 9n) == H.Step{[], []} : H.Step}

# Lost work is told, once (hub.bend's Bg.lost, Rec.lost, Hub.carry,
# Rec.release): a restart, the agent's exit or its replacement tells the
# agent what it lost and clears the rows in the same step; with
# restart.continue off it is logged and the rows stay, outcome unknown; a
# restart starts Rec.burst() threads at most, the rest are released later;
# a carried child's task stays open for the child to finish
law recover_lost_told:
  {H.Hub.running(H.Hub.thread(H.Carry.after(H.Lost.demo(True{}, False{}), 10n), "t1")) == True{} : Bool}

law recover_lost_clears:
  {M.Bg.live(M.Bg.of(H.Carry.after(H.Lost.demo(True{}, False{}), 10n), "t1")) == 0n : Nat}

law recover_no_work_quiet:
  {H.Hub.recover(H.Answer.demo(False{}), 9n) == H.Step{[], []} : H.Step}

law recover_lost_once:
  {H.Lost.told(M.State.mine(H.Carry.after(H.Carry.after(H.Lost.demo(True{}, False{}), 10n), 20n), "t1")) == 1n : Nat}

law lost_off_logs_and_keeps:
  {Nat.show(M.Bg.live(M.Bg.of(H.Rec.after(H.Lost.demo(False{}, False{}), 10n), "t1"))) ++ "/" ++ M.Bg.status(Maybe.default(&2, String, List.head(&2, String, M.Bg.of(H.Rec.after(H.Lost.demo(False{}, False{}), 10n), "t1")), ""))
    ++ "/" ++ Bool.show(H.Hub.running(H.Hub.thread(H.Carry.after(H.Lost.demo(False{}, False{}), 10n), "t1"))) == "1/unknown/False" : String}

law gone_idle_told:
  {H.Hub.running(H.Hub.thread(M.State.replay(H.Queue.changes(H.Hub.agent_gone(H.Lost.demo(True{}, False{}), 9n, "t1", 0)), H.Lost.demo(True{}, False{})), "t1")) == True{} : Bool}

# an agent gone mid-turn: its lost work is told at once, in a turn of its own
law gone_running_told:
  {H.Hub.running(H.Hub.thread(M.State.replay(H.Queue.changes(H.Hub.agent_gone(H.Lost.demo(True{}, True{}), 9n, "t1", 0)), H.Lost.demo(True{}, True{})), "t1")) == True{} : Bool}

law replaced_told_once:
  {H.Bg.lost(M.State.replay(H.Queue.changes(H.Bg.lost(H.Lost.demo(True{}, True{}), 9n, "t1", "replaced")), H.Lost.demo(True{}, True{})), 10n, "t1", "replaced") == H.Step{[], []} : H.Step}

law replaced_keeps_new_rows:
  {M.Bg.live(M.Bg.of(M.State.replay([M.WorkSet{"t1", "bg", "b2\tnew\tc2\tnew\tshell", 11n}], M.State.replay(H.Queue.changes(H.Bg.lost(H.Lost.demo(True{}, True{}), 9n, "t1", "replaced")), H.Lost.demo(True{}, True{}))), "t1")) == 1n : Nat}

law carry_keeps_child_task:
  {H.Task.marked(H.Queue.changes(H.Hub.recover(H.Task.demo(False{}), 9n)), "k1") == False{} : Bool}

law carry_off_fails_child_task:
  {H.Task.marked(H.Queue.changes(H.Hub.recover(M.State.replay([M.SettingSet{"restart.continue", "off", 2n}], H.Task.demo(False{})), 9n)), "k1") == True{} : Bool}

law carry_child_result_once:
  {H.Task.marked(H.Queue.changes(H.Task.finish.w(H.Carry.after(H.Task.demo(False{}), 9n), 30n, "t1", H.Step{[M.TurnChanged{"t1", M.Completed{}, 30n}], []})), "k1") == True{} : Bool}

law rec_burst_bounded:
  {H.Lost.running(H.Carry.after(H.Lost.many(), 10n)) == 4n : Nat}

law rec_release_bounded:
  {H.Lost.running(M.State.replay(H.Queue.changes(H.Rec.release(H.Carry.after(H.Lost.many(), 10n), 11n)), H.Carry.after(H.Lost.many(), 10n))) == 6n : Nat}

law rec_release_carry_marker_on:
  {Nat.show(H.Lost.running(H.Lost.released(True{}))) ++ "/" ++ Nat.show(List.length(&2, String, H.Resume.ids(H.Lost.released(True{})))) == "1/0" : String}

law rec_release_carry_marker_off:
  {Nat.show(H.Lost.running(H.Lost.released(False{}))) ++ "/" ++ Nat.show(List.length(&2, String, H.Resume.ids(H.Lost.released(False{})))) == "0/0" : String}

# a message an older log held goes with the note's turn: nothing stays held
law rec_release_takes_held:
  {H.Lost.taken(H.Queue.changes(H.Rec.release(H.Lost.marked(), 9n))) == True{} : Bool}

law lost_text_not_carry_text:
  for +ls: List<&2, String>
  {String.eq(H.Lost.text(ls, "restart"), M.Carry.text()) == False{} : Bool}

law update_wait_busy:
  for +now: Nat
  for +c: U32
  for +id: String
  {H.Lost.upgrades(H.Step.outs_of(H.Update.ask(True{}, now, c, id))) == 0n : Nat}

law update_wait_idle:
  for +now: Nat
  for +c: U32
  for +id: String
  {H.Lost.upgrades(H.Step.outs_of(H.Update.ask(False{}, now, c, id))) == 1n : Nat}

law update_wait_goes_when_done:
  {H.Lost.upgrades(H.Step.outs_of(H.Update.due(M.State.replay([M.SettingSet{"update.wait", "5", 5n}], H.Answer.demo(False{})), 9n))) == 1n : Nat}

law update_wait_holds_while_busy:
  {H.Lost.upgrades(H.Step.outs_of(H.Update.due(M.State.replay([M.SettingSet{"update.wait", "5", 5n}], H.Answer.demo(True{})), 9n))) == 0n : Nat}

# An install that finished asks the hub whether to restart (Update.ready):
# the busy check is made then, not when the update was asked; work that
# started during the download holds the restart (update.restart) until it
# is done or the wait is up, and a forced update goes at once.
law update_ready_holds_busy:
  for +now: Nat
  {H.Lost.restarts(H.Step.outs_of(H.Update.ready(H.Answer.demo(True{}), False{}, now))) == 0n : Nat}

law update_ready_idle_restarts:
  for +now: Nat
  {H.Lost.restarts(H.Step.outs_of(H.Update.ready(H.Answer.demo(False{}), False{}, now))) == 1n : Nat}

law update_ready_forced_restarts:
  for +now: Nat
  {H.Lost.restarts(H.Step.outs_of(H.Update.ready(H.Answer.demo(True{}), True{}, now))) == 1n : Nat}

law update_held_goes_when_done:
  {H.Lost.restarts(H.Step.outs_of(H.Update.held(M.State.replay([M.SettingSet{"update.restart", "5", 5n}], H.Answer.demo(False{})), 9n))) == 1n : Nat}

law update_held_waits_while_busy:
  {H.Lost.restarts(H.Step.outs_of(H.Update.held(M.State.replay([M.SettingSet{"update.restart", "5", 5n}], H.Answer.demo(True{})), 9n))) == 0n : Nat}

law update_wait_late_forces:
  {H.Lost.forced(H.Step.outs_of(H.Update.due(M.State.replay([M.SettingSet{"update.wait", "5", 5n}], H.Answer.demo(True{})), 1805n))) == 1n : Nat}

law update_wait_early_not_forced:
  {H.Lost.forced(H.Step.outs_of(H.Update.due(M.State.replay([M.SettingSet{"update.wait", "5", 5n}], H.Answer.demo(False{})), 9n))) == 0n : Nat}

# Spawn generations (hub.bend's Gen.*): only the generation a thread waits
# for adopts its pid or reports its failure; a stale process's MCP call may
# read, never change; a message prepared for an earlier process, or
# stopped, is not written; pid 0 and 1 are never signalled
law kill_never_low_pid:
  {Bool.show(H.Proc.kill.ok(0)) ++ Bool.show(H.Proc.kill.ok(1)) ++ Bool.show(H.Proc.kill.ok(4242)) == "FalseFalseTrue" : String}

law spawn_gen_accepts_current:
  {H.Gen.accepts(H.Gen.key("1790-7"), "1790-7") == True{} : Bool}

law spawn_gen_drops_stale:
  {Bool.or(H.Gen.accepts(H.Gen.key("1790-7"), "1789-3"), H.Gen.accepts(H.Gen.turn("1790-7"), "1790-7")) == False{} : Bool}

# a thread stopped while its process came up has no placeholder left
law stale_spawn_fail_quiet:
  for +gen: String
  {H.Gen.accepts("", gen) == False{} : Bool}

law mcp_stale_gen_refused:
  {H.Gen.refused("g1", "g2", CM.Cmds.o("{\"method\":\"tools/call\",\"params\":{\"name\":\"thread_send\"}}")) == True{} : Bool}

law mcp_stale_gen_reads:
  {Bool.or(H.Gen.refused("g1", "g2", CM.Cmds.o("{\"method\":\"tools/call\",\"params\":{\"name\":\"thread_read\"}}")),
    Bool.or(H.Gen.refused("g2", "g2", CM.Cmds.o("{\"method\":\"tools/call\",\"params\":{\"name\":\"thread_send\"}}")),
      H.Gen.refused("", "g2", CM.Cmds.o("{\"method\":\"tools/call\",\"params\":{\"name\":\"thread_send\"}}")))) == False{} : Bool}

law prep_stale_dropped:
  {Bool.show(H.Gen.write("g1", "g2", 1n)) ++ Bool.show(H.Gen.write("g1", "g1", 4n)) ++ Bool.show(H.Gen.write("g1", "g1", 1n)) == "FalseFalseTrue" : String}

law gen_mcp_path:
  {H.Gen.thread(H.Gen.mcp("t17-3", "1790-7")) ++ "|" ++ H.Gen.tag(H.Gen.mcp("t17-3", "1790-7")) ++ "|" ++ H.Gen.tag("t17-3") == "t17-3|1790-7|" : String}

# Silence and exit (hub.bend's Quiet.*, Exit.*; utf8.bend's Lines.rest): a
# quiet running turn is noted once, never while it waits on work; a stale
# generation's exit changes nothing; the last line of a stream with no
# newline is still read
law quiet_once:
  {Bool.show(H.Quiet.due(True{}, 0n, 700n, False{}, 0n)) ++ Bool.show(H.Quiet.due(True{}, 0n, 700n, True{}, 0n)) ++ Bool.show(H.Quiet.due(True{}, 200n, 700n, False{}, 0n)) == "TrueFalseFalse" : String}

law quiet_never_when_busy:
  for +heard: Nat
  for +now: Nat
  for +n: Nat
  {H.Quiet.due(True{}, heard, now, False{}, 1n+n) == False{} : Bool}

law quiet_never_idle:
  for +heard: Nat
  for +now: Nat
  for said: Bool
  for +busy: Nat
  {H.Quiet.due(False{}, heard, now, said, busy) == False{} : Bool}

law exit_stale_gen_quiet:
  {Bool.show(H.Exit.current("g1", "g2")) ++ Bool.show(H.Exit.current("", "")) ++ Bool.show(H.Exit.current("g1", "g1")) == "FalseFalseTrue" : String}

law final_partial_line_flushed:
  {UT.Lines.rest(UT.Lines.after(UT.Lines.feed(UT.Lines.new(), UT.Utf8.encode("{\"a\":1}\n{\"type\":\"result\"}")))) == "{\"type\":\"result\"}" : String}

# Watched work (hub.bend's Watch.*, mcp.bend's work_* tools): the row is
# written before anything is launched; its end is told once, waking an
# idle thread or waiting behind a running turn; an outcome that cannot be
# known is never success; a cancel shows until confirmed; unwatching says
# the work may still run; a restart watches open work again, a mirror
# never does
law watch_intent_before_launch:
  {Bool.and(String.contains(M.Setting.get(H.Watch.again(H.Answer.demo(False{}), H.Watch.start(H.Answer.demo(False{}), 9n, "t1", "run", "make", "build", 0n)), "watch.t1", ""), "\tlaunching"),
    Nat.is_eq(H.Watch.awaits(H.Step.outs_of(H.Watch.start(H.Answer.demo(False{}), 9n, "t1", "run", "make", "build", 0n))), 1n)) == True{} : Bool}

law watch_end_once:
  {H.Watch.end(H.Watch.again(H.Watch.demo(False{}), H.Watch.end(H.Watch.demo(False{}), 9n, "t1", "w-1", "exit 0", "")), 10n, "t1", "w-1", "exit 0", "") == H.Step{[], []} : H.Step}

law watch_end_wakes_idle:
  {H.Hub.running(H.Hub.thread(H.Watch.again(H.Watch.demo(False{}), H.Watch.end(H.Watch.demo(False{}), 9n, "t1", "w-1", "exit 4", "")), "t1")) == True{} : Bool}

law watch_end_delivers_running:
  {M.Queue.waiting(H.Watch.again(H.Watch.demo(True{}), H.Watch.end(H.Watch.demo(True{}), 9n, "t1", "w-1", "exit 4", "")), "t1") == False{} : Bool}

law watch_timeout_unknown_not_success:
  {M.Bg.status(M.Setting.get(H.Watch.again(H.Watch.demo(False{}), H.Watch.end(H.Watch.demo(False{}), 9n, "t1", "w-1", "timeout", "")), "watch.t1", "")) == "unknown" : String}

law watch_pid_identity_unknown:
  {String.contains(H.Watch.word("gone", False{}), "outcome unknown") == True{} : Bool}

law watch_cancel_keeps_until_confirmed:
  {M.Bg.doing(M.Setting.get(H.Watch.again(H.Watch.demo(False{}), H.Watch.cancel(H.Watch.demo(False{}), 9n, "t1", "w-1")), "watch.t1", "")) == "cancelling" : String}

law watch_cancel_confirmed:
  {H.Watch.told(M.State.mine(H.Watch.again(H.Watch.again(H.Watch.demo(False{}), H.Watch.cancel(H.Watch.demo(False{}), 9n, "t1", "w-1")),
      H.Watch.end(H.Watch.again(H.Watch.demo(False{}), H.Watch.cancel(H.Watch.demo(False{}), 9n, "t1", "w-1")), 10n, "t1", "w-1", "gone", "")), "t1"), "ended: cancelled") == True{} : Bool}

# an archived or deleted thread is not woken by its watched work's end, and
# retiring it stops its watchers
law watch_end_retired_silent:
  {Bool.or(H.Watch.told(M.State.mine(H.Watch.again(H.Watch.demo.archived(), H.Watch.end(H.Watch.demo.archived(), 9n, "t1", "w-1", "exit 4", "")), "t1"), "ended"),
    H.Hub.running(H.Hub.thread(H.Watch.again(H.Watch.demo.archived(), H.Watch.end(H.Watch.demo.archived(), 9n, "t1", "w-1", "exit 4", "")), "t1"))) == False{} : Bool}

law watch_retire_unwatches:
  {H.Watch.retire(H.Watch.demo(False{}), 9n, "t1") == H.Step{[H.Watch.put("t1", 9n, [])], [H.WatchStop{"t1", "w-1", "unwatch", "make", "-"}]} : H.Step}

# a watched command or path is stored whole (newlines, tabs, backslashes) and
# read back as it was run; a row written before keeps reading as written
law watch_target_roundtrip:
  {H.Watch.target(H.Watch.line("w-1", "watching", "n", "run", "echo A\necho B\tc\\n\r\\", "-", 0n)) == "echo A\necho B\tc\\n\r\\" : String}

law watch_target_one_line:
  {Bool.or(String.contains(H.Watch.line("w-1", "watching", "n", "run", "a\nb\tc", "-", 0n), "a\nb"), String.contains(H.Watch.line("w-1", "watching", "n", "run", "a\nb\tc", "-", 0n), "b\tc")) == False{} : Bool}

law watch_target_old_row_as_written:
  {H.Watch.target("w-1\twatching\t\tn\twatch\trun\tsed s/a\\nb/c/ x\t-\t0") == "sed s/a\\nb/c/ x" : String}

law watch_unwatch_says_may_run:
  {String.contains(H.Watch.unwatch.text(), "may still run") == True{} : Bool}

law recover_rearms_watches:
  {H.Watch.awaits(H.Step.outs_of(H.Hub.recover(H.Watch.demo(False{}), 9n))) == 1n : Nat}

law mirror_never_awaits:
  {H.Watch.rearm(H.Watch.demo.at(False{}, "p1~t1")) == [] : List<&2, H.Out>}

law prompt_background_work_b:
  {Bool.and(String.contains(PM.Prompt.background(), "work_run"), Bool.and(String.contains(PM.Prompt.background(), "work_watch"), String.contains(PM.Prompt.background(), "Never poll"))) == True{} : Bool}

# Transport (core/outbox.bend, server.bend's Wire.*/Hub.boot): every frame
# for a client is counted and one too far behind is dropped, a writer's
# report from an older connection changes nothing, a log goes out in
# chunks that cover it in order, a client still taking one raises no
# alerts and ends with the same read model as from one message, pongs are
# rate-bounded, and a message encoded once for all sockets is the frame
# each writer would have made
law credit_drop_over_margin:
  {Bool.show(OB.Cr.over(OB.Cr.offer(OB.Cr{1, 768n, 0n, 0n, 0n, False{}, False{}, 0n, 0n, 0n, False{}, []}, 0n))) ++ Bool.show(OB.Cr.over(OB.Cr{1, 768n, 0n, 0n, 0n, False{}, False{}, 0n, 0n, 0n, False{}, []}))
    ++ Bool.show(OB.Cr.over(OB.Cr.offer(OB.Cr{1, 900n, 200n, 0n, 0n, False{}, False{}, 0n, 0n, 0n, False{}, []}, 0n))) ++ Bool.show(OB.Cr.over(OB.Cr.offer(OB.Cr.new(1, False{}, False{}, 0n), Nat.add(OB.Cr.cap(), 1n)))) == "TrueFalseFalseTrue" : String}

# a live client far behind takes the log's changes as a join does, in
# chunks, instead of being dropped for a burst
law credit_behind_catches:
  {Bool.show(OB.Cr.behind(OB.Cr{1, 97n, 0n, 0n, 0n, False{}, False{}, 0n, 0n, 0n, False{}, []})) ++ Bool.show(OB.Cr.behind(OB.Cr{1, 96n, 0n, 0n, 0n, False{}, False{}, 0n, 0n, 0n, False{}, []}))
    ++ Bool.show(OB.Cr.boot(OB.Cr.catch(OB.Cr.new(1, False{}, False{}, 0n), 50n, 2n))) ++ Nat.show(OB.Cr.at(OB.Cr.catch(OB.Cr.new(1, False{}, False{}, 0n), 50n, 2n))) == "TrueFalseTrue50" : String}

law credit_stale_gen_quiet:
  for +n: Nat
  for +b: Nat
  {OB.Cr.drained(OB.Cr.new(2, False{}, True{}, 5n), 1, n, b) == OB.Cr.new(2, False{}, True{}, 5n) : OB.Cr}

law credit_drained_counts:
  {OB.Cr.drained(OB.Cr{2, 40n, 0n, 900n, 0n, False{}, False{}, 0n, 0n, 0n, False{}, []}, 2, 32n, 800n) == OB.Cr{2, 40n, 32n, 900n, 800n, False{}, False{}, 0n, 0n, 0n, False{}, []} : OB.Cr}

law credit_drained_never_back:
  {OB.Cr.drained(OB.Cr{2, 40n, 32n, 900n, 800n, False{}, False{}, 0n, 0n, 0n, False{}, []}, 2, 3n, 9n) == OB.Cr{2, 40n, 32n, 900n, 800n, False{}, False{}, 0n, 0n, 0n, False{}, []} : OB.Cr}

law boot_room_after_drain:
  {Bool.show(OB.Cr.room(OB.Cr{2, 80n, 64n, 2000000n, 1900000n, False{}, True{}, 0n, 0n, 0n, False{}, []})) ++ Bool.show(OB.Cr.room(OB.Cr{2, 80n, 0n, 0n, 0n, False{}, True{}, 0n, 0n, 0n, False{}, []})) == "TrueFalse" : String}

# a log goes oldest first from where the client was, chunk after chunk
law outbox_fifo:
  {OB.Join.from(["d", "c", "b", "a"], 4n, 1n) == ["b", "c", "d"] : List<&2, String>}

law join_window_same:
  {OB.Join.window(["e", "d", "c", "b", "a"], 5n, 1n) == OB.Join.from(["e", "d", "c", "b", "a"], 5n, 1n) : List<&2, String>}

law join_chunks_cover:
  {OB.Join.chunks.at(10n, ["a", "bb", "ccc", "d", "ee"], 2n, 4n) == [["a", "bb"], ["ccc", "d"], ["ee"]] : List<&2, List<&2, String>>}

law join_chunks_flat:
  {OB.Join.flat(OB.Join.chunks.at(10n, ["a", "bb", "ccc", "d", "ee"], 2n, 4n)) == ["a", "bb", "ccc", "d", "ee"] : List<&2, String>}

law join_oversized_alone:
  {OB.Join.chunks.at(10n, ["toolong", "a", "b"], 5n, 3n) == [["toolong"], ["a", "b"]] : List<&2, List<&2, String>>}

# a join frame's lines go out in byte-bounded frames: a small chunk is
# the frame as offered; a big one is split, the later frames "boot": true
# and the very last "boot": false when the offered frame ended the join
law items_one_frame:
  {OB.Items.texts("H[", ["a", "b"], "]", True{}) == ["H[a,b]"] : List<&2, String>}

law items_split_by_bytes:
  {OB.Items.texts.at("H[", ["aaa", "bb", "cccc"], "]", True{}, 5n, 4n) == ["H[aaa]", "{\"t\":\"changes\",\"boot\":true,\"items\":[bb]}", "{\"t\":\"changes\",\"boot\":false,\"items\":[cccc]}"] : List<&2, String>}

law items_split_keeps_boot:
  {OB.Items.texts.at("H[", ["aaa", "bb", "cccc"], "]", False{}, 5n, 4n) == ["H[aaa]", "{\"t\":\"changes\",\"boot\":true,\"items\":[bb]}", "{\"t\":\"changes\",\"boot\":true,\"items\":[cccc]}"] : List<&2, String>}

# a log frame the writer splits says "more": true (it was the join's last as
# offered), so the frames after it stay inside the bootstrap; one that is
# not split stays as offered
law items_split_marks_more:
  {OB.Items.texts.s("L", ",i[", ",m,i[", ["aaa", "bb", "cccc"], "]", True{}, 5n, 4n) == ["L,m,i[aaa]", "{\"t\":\"changes\",\"boot\":true,\"items\":[bb]}", "{\"t\":\"changes\",\"boot\":false,\"items\":[cccc]}"] : List<&2, String>}

law items_whole_not_marked:
  {OB.Items.texts.s("L", ",i[", ",m,i[", ["a", "b"], "]", True{}, 5n, 4n) == ["L,i[a,b]"] : List<&2, String>}

# a failed log write is cut back to the length before it; while that cannot
# be done the hold keeps the length and the reason
law store_hold_len:
  {ST.Store.hold.len(ST.Store.hold("42", "no space")) == "42" : String}

law store_hold_err:
  {ST.Store.hold.err(ST.Store.hold("42", "no space")) == "no space" : String}

law store_trunc_args:
  {ST.Store.trunc.args("42", "/h/events.jsonl") == ["-s", "42", "/h/events.jsonl"] : List<&2, String>}

law ui_chunked_join_equals_whole:
  {U.Ui.state(U.Ui.recv(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", True{}, True{}, [C.Change.encode(M.SettingSet{"a", "1", 1n})])),
      U.Boot.demo("changes", False{}, True{}, [C.Change.encode(M.SettingSet{"b", "2", 2n})])), U.Boot.demo("changes", False{}, False{}, [])))
    == U.Ui.state(U.Ui.recv(U.Ui.init(0n), U.Seq.demo("log", [C.Change.encode(M.SettingSet{"a", "1", 1n}), C.Change.encode(M.SettingSet{"b", "2", 2n})]))) : M.State}

law boot_on_until_last:
  {Bool.show(U.Boot.on(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", True{}, True{}, [])))) ++ Bool.show(U.Boot.on(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", True{}, True{}, [])), U.Boot.demo("changes", False{}, False{}, []))))
    ++ Bool.show(U.Boot.on(U.Ui.recv(U.Ui.init(0n), U.Seq.demo("log", [])))) == "TrueFalseFalse" : String}

# a turn's end folded while the log still arrives (and with its last
# chunk) is history: no alert; the same frame on a live client alerts
law boot_no_alerts:
  {List.append(&2, J.Json, Hb.Did.alerts(Hb.One.got(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", True{}, True{}, [C.Change.encode(M.ThreadCreated{"t1", "p1", "hi", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.TurnChanged{"t1", M.Running{}, 2n})])),
      U.Boot.demo("changes", False{}, True{}, [C.Change.encode(M.TurnChanged{"t1", M.Completed{}, 3n})]))),
    Hb.Did.alerts(Hb.One.got(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", True{}, True{}, [C.Change.encode(M.ThreadCreated{"t1", "p1", "hi", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.TurnChanged{"t1", M.Running{}, 2n})])),
      U.Boot.demo("changes", False{}, False{}, [C.Change.encode(M.TurnChanged{"t1", M.Completed{}, 3n})])))) == [] : List<&2, J.Json>}

law boot_done_alerts:
  {List.length(&2, J.Json, Hb.Did.alerts(Hb.One.got(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", False{}, False{}, [C.Change.encode(M.ThreadCreated{"t1", "p1", "hi", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.TurnChanged{"t1", M.Running{}, 2n})])),
      U.Seq.demo("changes", [C.Change.encode(M.TurnChanged{"t1", M.Completed{}, 3n})])))) == 1n : Nat}

# a live client's catch-up (changes with "boot": true, no log before it)
# is history too: no alert from it
law boot_catchup_no_alerts:
  {List.length(&2, J.Json, Hb.Did.alerts(Hb.One.got(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", False{}, False{}, [C.Change.encode(M.ThreadCreated{"t1", "p1", "hi", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.TurnChanged{"t1", M.Running{}, 2n})])), U.Boot.demo("changes", False{}, True{}, [])),
      U.Boot.demo("changes", False{}, True{}, [C.Change.encode(M.TurnChanged{"t1", M.Completed{}, 3n})])))) == 0n : Nat}

law pong_rate_bounded:
  {Bool.show(OB.Pong.due(1000n, 1999n)) ++ Bool.show(OB.Pong.due(1000n, 2000n)) ++ Bool.show(OB.Pong.due(0n, 999n)) == "FalseTrueFalse" : String}

law wire_all_bin_eq:
  for t: String
  {WS.Ws.encode(2, WS.Ws.cbor.bytes(t)) == WS.Ws.cbor(t) : List<&2, U32>}

# Live text (core/live.bend): a reconnect's snapshot replaces the live
# text (never appended to it), taking it twice is taking it once, a thread
# not in it shows none, a posted reply or an ended turn clears the hub's
# buffer, what streamed is what a joining client shows, and an older
# snapshot than one already taken changes nothing
law live_snapshot_replaces:
  {U.Ui.live.of(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{"t1"}}, J.KV{"text", J.Str{"abc"}}])), LV.Live.demo("t1", "abcdef", False{}, 0n)), "t1") == "abcdef" : String}

law live_snapshot_idempotent:
  {U.Ui.live.of(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), LV.Live.demo("t1", "abcdef", False{}, 0n)), LV.Live.demo("t1", "abcdef", False{}, 0n)), "t1") == "abcdef" : String}

law live_snapshot_clears_absent:
  {U.Ui.live.of(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{"t2"}}, J.KV{"text", J.Str{"x"}}])), LV.Live.demo("t1", "abc", False{}, 0n)), "t2") == "" : String}

law live_end_clears:
  {LV.Live.msg(LV.Live.ends([M.MessagePosted{"t1", "m:0", M.Assistant{}, "ab", 3n}], LV.Live.add(LV.Live.add(Nil{}, "t1", "a"), "t1", "b"))) == "{\"t\":\"live\",\"items\":[]}" : String}

law live_turn_end_clears:
  {LV.Live.msg(LV.Live.ends([M.TurnChanged{"t1", M.Interrupted{}, 3n}], LV.Live.add(Nil{}, "t1", "a"))) == "{\"t\":\"live\",\"items\":[]}" : String}

law live_joined_exact:
  {U.Ui.live.of(U.Ui.recv(U.Ui.init(0n), Maybe.default(&2, J.Json, J.Json.parse(LV.Live.msg(LV.Live.add(LV.Live.add(LV.Live.add(Nil{}, "t1", "ab"), "t2", "zz"), "t1", "cd"))), J.Null{})), "t1") == "abcd" : String}

law live_old_gen_quiet:
  {U.Ui.live.of(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), LV.Live.demo("t1", "new", False{}, 2n)), LV.Live.demo("t1", "old", False{}, 1n)), "t1") == "new" : String}

law live_trunc_marked:
  {U.Ui.live.of(U.Ui.recv(U.Ui.init(0n), LV.Live.demo("t1", "tail", True{}, 0n)), "t1") == "…tail" : String}

# Creating a thread (client.bend's Made.*): only the hub's answer to this
# client's own request selects the thread it made; another client's new
# thread, a replayed log or an older request's answer selects nothing
law want_new_ignores_other_threads:
  {U.Ui.sel(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1")), U.Made.made("changes", "t9"))) == "" : String}

law made_selects_own:
  {U.Ui.sel(U.Ui.recv(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1")), U.Made.made("changes", "t9")), U.Made.demo("0", "t9"))) == "t9" : String}

law made_selects_own_later:
  {U.Ui.sel(U.Ui.recv(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1")), U.Made.demo("0", "t9")), U.Made.made("changes", "t9"))) == "t9" : String}

law replay_cannot_select:
  {U.Ui.sel(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1")), U.Made.made("log", "t9"))) == "" : String}

law made_stale_quiet:
  {U.Ui.sel(U.Ui.recv(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1")), "p1")), U.Made.made("changes", "t9")), U.Made.demo("0", "t9"))) == "" : String}

law made_fork_selects_own:
  {U.Ui.sel(U.Ui.recv(U.Ui.recv(U.Act.ui(U.Ui.fork(U.Ui.init(0n))), U.Made.made("changes", "t9")), U.Made.demo("0", "t9"))) == "t9" : String}

# New Thread is waited for at once (the phones open its page then), and
# the hub's answer ends the wait whether it made the thread or failed
law made_waiting_at_once:
  {U.Made.waiting(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1"))) == True{} : Bool}

law made_waiting_ends_made:
  {U.Made.waiting(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1")), U.Made.demo("0", "t9"))) == False{} : Bool}

law made_waiting_ends_failed:
  {U.Made.waiting(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1")), J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{False{}}}]))) == False{} : Bool}

# Create anywhere: a request to make, fork, pin, archive or delete a thread
# in a project elsewhere, or to add or browse a folder on another machine,
# goes to the machine that owns it with its ids made local; the owning hub
# takes it only for what it shares; the answer names the new thread by its
# id here, and the client making it waits for the mirror (laws far_create_*,
# made_far_*)
law far_create_routes:
  {FR.Route.link.of(FR.Fr.route(J.Json.obj([J.KV{"id", J.Num{"7"}}, J.KV{"m", J.Str{"thread.create"}}, J.KV{"p", J.Json.obj([J.KV{"project", J.Str{"p1~p"}}, J.KV{"title", J.Str{"New thread"}}])}]))) == "p1" : String}

law far_fork_routes:
  {FR.Route.link.of(FR.Fr.route(J.Json.obj([J.KV{"id", J.Num{"7"}}, J.KV{"m", J.Str{"thread.fork"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"p1~pt"}}])}]))) == "p1" : String}

law far_create_ids_local:
  {FR.Fr.params.local(FR.Fr.obj.fields(J.Json.obj([J.KV{"project", J.Str{"p1~p"}}, J.KV{"title", J.Str{"x~y"}}])), "p1") == FR.Fr.obj.fields(J.Json.obj([J.KV{"project", J.Str{"p"}}, J.KV{"title", J.Str{"x~y"}}])) : J.Json}

law far_machine_routes:
  {FR.Fr.params.link(FR.Fr.obj.fields(J.Json.obj([J.KV{"on", J.Str{"p1"}}, J.KV{"path", J.Str{"/x"}}]))) == "p1" : String}

law far_machine_none_here:
  {FR.Fr.params.link(FR.Fr.obj.fields(J.Json.obj([J.KV{"on", J.Str{""}}, J.KV{"path", J.Str{"/x"}}]))) == "" : String}

law far_create_in_shared_project:
  {FR.Fr.allowed(M.State.apply(M.State.empty(), M.ProjectCreated{"p", "b", "/b", 1n}),
    J.Json.obj([J.KV{"m", J.Str{"thread.create"}}, J.KV{"p", J.Json.obj([J.KV{"project", J.Str{"p"}}])}])) == True{} : Bool}

law far_create_not_in_unshared_project:
  {FR.Fr.allowed(M.State.apply(M.State.empty(), M.ProjectCreated{"p", "b", "/b", 1n}),
    J.Json.obj([J.KV{"m", J.Str{"thread.create"}}, J.KV{"p", J.Json.obj([J.KV{"project", J.Str{"q"}}])}])) == False{} : Bool}

law far_create_not_in_a_mirror:
  {FR.Fr.allowed(M.State.apply(M.State.empty(), M.ProjectCreated{"p1~p", "b", "/b", 1n}),
    J.Json.obj([J.KV{"m", J.Str{"thread.create"}}, J.KV{"p", J.Json.obj([J.KV{"project", J.Str{"p1~p"}}])}])) == False{} : Bool}

law far_pin_only_shared_thread:
  {FR.Fr.allowed(M.State.empty(), J.Json.obj([J.KV{"m", J.Str{"thread.pin"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"t"}}])}])) == False{} : Bool}

law far_settings_not_remote:
  {FR.Fr.allowed(M.State.empty(), J.Json.obj([J.KV{"m", J.Str{"setting.set"}}, J.KV{"p", J.Json.obj([J.KV{"key", J.Str{"k"}}])}])) == False{} : Bool}

law far_reply_names_thread:
  {FR.Fr.reply.on(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"thread", J.Str{"t5"}}]), "9", "p1") ==
    "{\"t\":\"reply\",\"id\":9,\"ok\":true,\"thread\":\"p1~t5\"}" : String}

law far_pin_shared:
  {FR.Fr.shared(M.State.apply(M.State.apply(M.State.empty(), M.ProjectCreated{"p", "b", "/b", 1n}), M.ThreadCreated{"t", "p", "x", M.Local{}, "", "", "claude", "", 2n}), M.ThreadPinned{"t", True{}, 3n}) == True{} : Bool}

law made_far_waits_for_mirror:
  {U.Made.waiting(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1~p")), U.Made.demo("0", "p1~t9"))) == True{} : Bool}

law made_far_selects_when_mirrored:
  {U.Ui.sel(U.Ui.recv(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1~p")), U.Made.demo("0", "p1~t9")), U.Made.made("changes", "p1~t9"))) == "p1~t9" : String}

law made_far_done_when_mirrored:
  {U.Made.waiting(U.Ui.recv(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1~p")), U.Made.demo("0", "p1~t9")), U.Made.made("changes", "p1~t9"))) == False{} : Bool}

law made_far_failed_not_waiting:
  {U.Made.waiting(U.Ui.recv(U.Act.ui(U.Ui.new_thread(U.Ui.init(0n), "p1~p")), J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{False{}}}]))) == False{} : Bool}

law proj_machine_cycle_ends_here:
  {U.Proj.cycle(["", "p1", "p2"], "p2", "") == "" : String}

law proj_machine_cycle_next:
  {U.Proj.cycle(["", "p1", "p2"], "p1", "") == "p2" : String}

law proj_on_request_names_machine:
  {U.Proj.on.kv("p1", [J.KV{"path", J.Str{"/x"}}]) == [J.KV{"on", J.Str{"p1"}}, J.KV{"path", J.Str{"/x"}}] : List<&2, J.KV>}

law proj_here_request_unchanged:
  for p: List<&2, J.KV>
  {U.Proj.on.kv("", p) == p : List<&2, J.KV>}

# The sidebar's index (client.bend's Side.ix, gathered once a frame) gives
# what walking the state gives: rows, phases, subagent counts, the active
# view, and work for the settle tick; phones compare only the threads a
# frame touches and raise the same alerts
law side_rows_ix_eq:
  {U.Side.rows.ix(U.Side.demo(), U.Side.ix(U.Side.demo()), 100n) == U.Side.rows(U.Side.demo(), 100n) : List<&2, U.SRow>}

law side_phase_ix_eq:
  {U.Side.phases.ix(U.Ui.threads(U.Side.demo()), U.Side.ix(U.Side.demo()), U.Side.demo()) == U.Side.phases(U.Ui.threads(U.Side.demo()), U.Side.demo()) : List<&2, M.Phase>}

law subs_count_ix_eq:
  {SB.Subs.counts.ix(U.Ui.threads(U.Side.demo()), U.Side.ix(U.Side.demo()), U.Side.demo()) == SB.Subs.counts(U.Ui.threads(U.Side.demo()), U.Side.demo()) : List<&2, Nat>}

law active_rows_ix_eq:
  {U.Active.rows.ix(U.Side.demo(), U.Side.ix(U.Side.demo()), 100n, "") == U.Active.rows(U.Side.demo(), 100n, "") : List<&2, U.ARow>}

law settle_ix_eq:
  {U.Side.busies.ix(U.Ui.threads(U.Side.demo()), M.Work.kids.map(U.Side.demo()), U.Side.demo()) == U.Side.busies(U.Ui.threads(U.Side.demo()), U.Side.demo()) : List<&2, Nat>}

law alerts_ids_same:
  {Hb.Did.alerts(Hb.One.got(U.Ui.recv(U.Ui.init(0n), U.Seq.demo("log", [C.Change.encode(M.ThreadCreated{"t1", "p1", "a", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.ThreadCreated{"t2", "p1", "b", M.Local{}, "", "", "claude", "", 1n}),
      C.Change.encode(M.ThreadCreated{"t3", "p1", "c", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.TurnChanged{"t1", M.Running{}, 2n}), C.Change.encode(M.TurnChanged{"t2", M.Running{}, 2n}), C.Change.encode(M.TurnChanged{"t3", M.Running{}, 2n})])),
      U.Seq.demo("changes", [C.Change.encode(M.TurnChanged{"t1", M.Completed{}, 3n}), C.Change.encode(M.TurnChanged{"t3", M.Failed{}, 3n})])))
    == Hb.Did.alerts(Hb.One.got.all(U.Ui.recv(U.Ui.init(0n), U.Seq.demo("log", [C.Change.encode(M.ThreadCreated{"t1", "p1", "a", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.ThreadCreated{"t2", "p1", "b", M.Local{}, "", "", "claude", "", 1n}),
      C.Change.encode(M.ThreadCreated{"t3", "p1", "c", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.TurnChanged{"t1", M.Running{}, 2n}), C.Change.encode(M.TurnChanged{"t2", M.Running{}, 2n}), C.Change.encode(M.TurnChanged{"t3", M.Running{}, 2n})])),
      U.Seq.demo("changes", [C.Change.encode(M.TurnChanged{"t1", M.Completed{}, 3n}), C.Change.encode(M.TurnChanged{"t3", M.Failed{}, 3n})]))) : List<&2, J.Json>}

law alerts_ids_raise:
  {List.length(&2, J.Json, Hb.Did.alerts(Hb.One.got(U.Ui.recv(U.Ui.init(0n), U.Seq.demo("log", [C.Change.encode(M.ThreadCreated{"t1", "p1", "a", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.ThreadCreated{"t2", "p1", "b", M.Local{}, "", "", "claude", "", 1n}),
      C.Change.encode(M.ThreadCreated{"t3", "p1", "c", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.TurnChanged{"t1", M.Running{}, 2n}), C.Change.encode(M.TurnChanged{"t2", M.Running{}, 2n}), C.Change.encode(M.TurnChanged{"t3", M.Running{}, 2n})])),
      U.Seq.demo("changes", [C.Change.encode(M.TurnChanged{"t1", M.Completed{}, 3n}), C.Change.encode(M.TurnChanged{"t3", M.Failed{}, 3n})])))) == 2n : Nat}

# a push that failed gives its items back, ahead of what waited, and the
# link holds before it goes again; one through drops them
law far_fail_keeps_items:
  for a: J.Json
  for b: J.Json
  {FR.Out.waiting(FR.Out.done(FR.Out.sent(FR.Out.add(FR.Out.new("l"), 0n, 2n, [FR.Item{1n, a}, FR.Item{2n, b}])), False{}, 2n)) == [FR.Item{1n, a}, FR.Item{2n, b}] : List<&2, FR.Item>}

law far_ack_removes_batch:
  for a: J.Json
  {FR.Out.flight(FR.Out.done(FR.Out.sent(FR.Out.add(FR.Out.new("l"), 0n, 1n, [FR.Item{1n, a}])), True{}, 1n)) == Nil{} : List<&2, FR.Item>}

law far_ack_keeps_waiting:
  for a: J.Json
  for b: J.Json
  {FR.Out.waiting(FR.Out.done(FR.Out.add(FR.Out.sent(FR.Out.add(FR.Out.new("l"), 0n, 1n, [FR.Item{1n, a}])), 1n, 2n, [FR.Item{2n, b}]), True{}, 1n)) == [FR.Item{2n, b}] : List<&2, FR.Item>}

# what came while the batch was out follows it again, in order, and the
# batch is sent from where it began
law far_retry_order:
  for a: J.Json
  for b: J.Json
  {FR.Out.waiting(FR.Out.done(FR.Out.add(FR.Out.sent(FR.Out.add(FR.Out.new("l"), 4n, 5n, [FR.Item{5n, a}])), 5n, 6n, [FR.Item{6n, b}]), False{}, 5n)) == [FR.Item{5n, a}, FR.Item{6n, b}] : List<&2, FR.Item>}

law far_retry_begins:
  for a: J.Json
  for b: J.Json
  {FR.Out.begins(FR.Out.done(FR.Out.add(FR.Out.sent(FR.Out.add(FR.Out.new("l"), 4n, 5n, [FR.Item{5n, a}])), 5n, 6n, [FR.Item{6n, b}]), False{}, 5n)) == 4n : Nat}

law far_hold_waits:
  for a: J.Json
  {FR.Out.due(FR.Out.done(FR.Out.sent(FR.Out.add(FR.Out.new("l"), 0n, 1n, [FR.Item{1n, a}])), False{}, 1n)) == False{} : Bool}

law far_release_resends:
  for a: J.Json
  {FR.Out.due(FR.Out.release(FR.Out.done(FR.Out.sent(FR.Out.add(FR.Out.new("l"), 0n, 1n, [FR.Item{1n, a}])), False{}, 1n))) == True{} : Bool}

law far_backoff:
  {[FR.Out.delay(1n), FR.Out.delay(2n), FR.Out.delay(3n), FR.Out.delay(6n), FR.Out.delay(9n)] == [2n, 4n, 8n, 60n, 60n] : List<&2, Nat>}

# lines the log holds that this build cannot decode give the feed a history
# of its own, so mirrors pull it afresh
law far_epoch_on_unknown_lines:
  {[FR.Fr.history.body("x", 3n, 3n), FR.Fr.history.body("x", 5n, 3n)] == ["x|view:2", "x|unknown:2|view:2"] : List<&2, String>}

# the hub's heartbeat draws nothing and changes nothing; a link silent for
# longer than the limit is dead, one heard within it is not
law hb_shows_nothing:
  for u: U.Ui
  {U.Ui.shows(u, J.Json.obj([J.KV{"t", J.Str{"hb"}}])) == False{} : Bool}

law hubs_redraw_hb:
  for +hs: Hb.Hubs
  for +key: String
  {Hb.Hubs.redraw(hs, key, J.Json.obj([J.KV{"t", J.Str{"hb"}}])) == False{} : Bool}

law hb_ignored_by_fold:
  {U.Ui.state(U.Ui.recv(U.Ui.init(0n), J.Json.obj([J.KV{"t", J.Str{"hb"}}]))) == U.Ui.state(U.Ui.init(0n)) : M.State}

law alive_due:
  {[DK.Alive.due(1000n, 41000n), DK.Alive.due(1000n, 41001n), DK.Alive.due(5n, 5n), DK.Alive.due(9n, 3n)] == [False{}, True{}, False{}, False{}] : List<&2, Bool>}

# the CLI numbers each line's blocks from 0, so two text lines of one
# message get ids of their own from the line's uuid; a line with none, and
# every id already stored, keep "msg:n"
law claude_text_ids_distinct:
  for b: J.Json
  {[H.Claude.block.k(0n, "t", "m", 0n, 1n, 0n, "aaaaaaaa-1111", b), H.Claude.block.k(0n, "t", "m", 0n, 1n, 0n, "bbbbbbbb-2222", b), H.Claude.block.k(0n, "t", "m", 0n, 1n, 0n, "", b)]
    == [[M.MessagePosted{"t", "m:0:aaaaaaaa", M.Assistant{}, J.Json.get_str(b, "text", ""), 1n}], [M.MessagePosted{"t", "m:0:bbbbbbbb", M.Assistant{}, J.Json.get_str(b, "text", ""), 1n}], [M.MessagePosted{"t", "m:0", M.Assistant{}, J.Json.get_str(b, "text", ""), 1n}]] : List<&2, List<&2, M.Change>>}

# a bot request sent again (the client's outbox after a dropped link, the
# same `msg`) is answered ok and does nothing a second time
law bots_say_once:
  for live: Bool
  for +s: M.State
  for +now: Nat
  for +client: U32
  for +id: String
  for +bot: String
  for +text: String
  for +msg: String
  {BHS.Bots.say.once(True{}, msg, live, s, now, client, id, bot, text) == H.Step.outs([H.Reply.ok(client, id)]) : H.Step}

law bots_post_once:
  for +mid: String
  for m: Maybe<&2, BOT.Room>
  for +s: M.State
  for +now: Nat
  for +client: U32
  for +id: String
  for +text: String
  {BHS.Bots.post.once(True{}, mid, m, s, now, client, id, text) == H.Step.outs([H.Reply.ok(client, id)]) : H.Step}

law bots_tell_once:
  for +mid: String
  for remote: Bool
  for +s: M.State
  for +now: Nat
  for +client: U32
  for +id: String
  for +to: String
  for +text: String
  {BHS.Bots.tell.once(True{}, mid, remote, s, now, client, id, to, text) == H.Step.outs([H.Reply.ok(client, id)]) : H.Step}
# An agent's process ended by SIGKILL (137) is told as most likely out of
# memory; any other end says its exit code (Exit.why gives ""). Backplane
# picks the thread back up either way.
law exit_why_killed:
  {H.Exit.why(137) ++ "|" ++ H.Exit.why(3) == "agent was killed (signal 9, likely out of memory)|" : String}

# A streamed reply's settled prefix (client.bend's Live.head: the text up
# to the last blank line outside a fenced block) renders, and lays out,
# exactly as it does alone, so a window keeps it and lays out the rest.
law live_split_fence_safe:
  {U.Live.at("a\n\n```\nx\n\ny\n") == 3n : Nat}

law live_split_fence_closed:
  {U.Live.at("a\n\n```\nx\n\ny\n```\nz\n\nw") == 19n : Nat}

law live_split_none:
  {U.Live.at("one paragraph\nstill one") == 0n : Nat}

law live_head_rest:
  {U.Live.head("a\n\nb\n\nc") ++ U.Live.rest("a\n\nb\n\nc") == "a\n\nb\n\nc" : String}

law live_prefix_exact:
  {List.append(&2, DM.Node, DM.Md.render(U.Live.head("# T\n\nsome *words*\nand more\n\n- a\n- b\n\n1. x\n2. y\n\n| p | q |\n|---|---|\n| 1 | 2 |\n\n> quoted\n\n```\ncode\n\nmore code\n```\n\nlast\nline")),
    DM.Md.render(U.Live.rest("# T\n\nsome *words*\nand more\n\n- a\n- b\n\n1. x\n2. y\n\n| p | q |\n|---|---|\n| 1 | 2 |\n\n> quoted\n\n```\ncode\n\nmore code\n```\n\nlast\nline"))) == DM.Md.render("# T\n\nsome *words*\nand more\n\n- a\n- b\n\n1. x\n2. y\n\n| p | q |\n|---|---|\n| 1 | 2 |\n\n> quoted\n\n```\ncode\n\nmore code\n```\n\nlast\nline") : List<&2, DM.Node>}

law live_prefix_exact_open_fence:
  {List.append(&2, DM.Node, DM.Md.render(U.Live.head("para\n\n```\nopen\n\nstill open")), DM.Md.render(U.Live.rest("para\n\n```\nopen\n\nstill open"))) == DM.Md.render("para\n\n```\nopen\n\nstill open") : List<&2, DM.Node>}

# QR codes for pairing (core/qr.bend): level M's format bits for masks 0
# and 1, version 7's information bits, as the standard tabulates them
law qr_format_m:
  {QR.Qr.format(0) == 21522 : U32}

law qr_format_m1:
  {QR.Qr.format(1) == 20773 : U32}

law qr_version7_bits:
  {QR.Qr.vbits(7) == 31892 : U32}

# the smallest version that holds the text; none past version 40
law qr_version_fits:
  {QR.Qr.version(84) == 5 : U32}

law qr_version_grows:
  {QR.Qr.version(85) == 6 : U32}

law qr_version_none:
  {QR.Qr.version(2332) == 0 : U32}

# GF(256) under QR's polynomial: 2^8 wraps to 29
law qr_gmul_wraps:
  {QR.Qr.gmul(128, 2) == 29 : U32}

# the code a phone's camera opens the app with; "#" never ends the link
law qr_link_escapes:
  {QR.Qr.link("http://h:3787/#token=ab") == "backplane://pair?url=http://h:3787/%23token=ab" : String}

law qr_link_none:
  {QR.Qr.link("") == "" : String}

# no pairing link, no code
law pair_qr_none:
  {U.Pair.qr.ready(Map.new(&2, String)) == False{} : Bool}
# The first-run tour (core/onboard.bend). Every step has a title, a known
# target and words for the window, the web and the phones.
law onboard_steps_sound:
  {OBD.Ob.sound() == True{} : Bool}

# next and back stay within the steps for every number from 0 to 14 (past
# the end they clamp), a step on screen goes forward by next and (past the
# first) back by back
law onboard_nav_every_step:
  {OBD.Ob.nav(14n) == True{} : Bool}

law onboard_back_floor:
  {OBD.Ob.back(0n) == 0n : Nat}

law onboard_next_step:
  {OBD.Ob.next(2n) == 3n : Nat}

law onboard_back_step:
  {OBD.Ob.back(3n) == 2n : Nat}

# next from the last step ends it, and the end stays the end. Stated for
# whatever the step count is: the last step is `count - 1`, not a number
law onboard_next_ends:
  {OBD.Ob.live(OBD.Ob.next(Nat.sub(OBD.Ob.count(), 1n))) == False{} : Bool}

law onboard_end_stays:
  {OBD.Ob.next(OBD.Ob.count()) == OBD.Ob.count() : Nat}

# skip ends it from anywhere
law onboard_skip_ends:
  {OBD.Ob.live(OBD.Ob.skip()) == False{} : Bool}

# a tour that came up by itself waits for this connection's state
law onboard_waits:
  for done: Bool
  {OBD.Ob.at(False{}, done, "") == OBD.Ob.count() : Nat}

# a first run shows the first step; once done it never shows by itself
law onboard_fresh_first:
  {OBD.Ob.at(True{}, False{}, "") == 0n : Nat}

law onboard_done_quiet:
  {OBD.Ob.at(True{}, True{}, "") == OBD.Ob.count() : Nat}

# one that came up by itself and was moved on keeps its step, and closes
# when the hub says it is done (from another client too)
law onboard_auto_moved:
  {OBD.Ob.at(True{}, False{}, OBD.Ob.raw(False{}, 4n)) == 4n : Nat}

law onboard_done_closes_auto:
  {OBD.Ob.at(True{}, True{}, OBD.Ob.raw(False{}, 5n)) == OBD.Ob.count() : Nat}

# one asked for by hand shows whatever the hub says, offline too
law onboard_hand_stays:
  {OBD.Ob.at(True{}, True{}, OBD.Ob.raw(True{}, 5n)) == 5n : Nat}

law onboard_hand_offline:
  {OBD.Ob.at(False{}, False{}, OBD.Ob.raw(True{}, 2n)) == 2n : Nat}

law onboard_wild_ends:
  {OBD.Ob.at(True{}, False{}, "99") == OBD.Ob.count() : Nat}

# phones point at nothing
law onboard_phone_no_target:
  {OBD.Ob.target(2n, 3n) == "" : String}

# one policy for the keys, in the window and the web
law onboard_keys:
  {OBD.Ob.dom("Enter") ++ OBD.Ob.dom("ArrowLeft") ++ OBD.Ob.dom("Escape") ++ OBD.Ob.dom("a") == "ob-nextob-backob-skip" : String}

law onboard_syms:
  {OBD.Ob.sym(65293) ++ OBD.Ob.sym(65361) ++ OBD.Ob.sym(65307) ++ OBD.Ob.sym(97) == "ob-nextob-backob-skip" : String}

# the client: a state not yet current shows nothing; skipping sets the
# hub's setting; a done from elsewhere closes a tour that was moved on;
# the Settings button shows it again after it was done
def OnbUi.log() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}], M.State.empty())

# a tour finished with every step answered (onboard.uber too)
def OnbUi.done() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.SettingSet{"onboard.done", "1", 2n}, M.SettingSet{"onboard.uber", "1", 2n}], M.State.empty())

def OnbUi.synced(st: M.State) -> U.Ui:
  U.Ui.scratch_set(U.Ui.adopt.raw(U.Ui.init(1n), st), "@sync", "1")

def OnbUi.after(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

law onboard_unsynced_hidden:
  {U.Tour.at(U.Ui.adopt.raw(U.Ui.init(1n), OnbUi.log())) == OBD.Ob.count() : Nat}

law onboard_synced_shows:
  {U.Tour.at(OnbUi.synced(OnbUi.log())) == 0n : Nat}

law onboard_finish_sets_done:
  {U.Tour.done(U.Ui.state(OnbUi.after(U.Ui.act(OnbUi.synced(OnbUi.log()), "ob-skip", "")))) == True{} : Bool}

law onboard_elsewhere_closes:
  {U.Tour.at(U.Ui.adopt.raw(OnbUi.after(U.Ui.act(OnbUi.synced(OnbUi.log()), "ob-next", "")), OnbUi.done())) == OBD.Ob.count() : Nat}

law onboard_reopen:
  {U.Tour.at(OnbUi.after(U.Ui.act(OnbUi.synced(OnbUi.done()), "ob-open", ""))) == 0n : Nat}

# State put back from a cache (a page's saved log, a phone's saved state) is
# not this connection's: only a live log shows the tour
def OnbUi.live() -> U.Ui:
  U.Ui.recv.k(0n, U.Ui.init(1n), U.Seq.demo("log", [C.Change.encode(M.ProjectCreated{"p1", "p", "/p", 1n})]))

def OnbUi.phone() -> Hb.Hubs:
  Hb.Hubs{"c", 1n, "h", [Hb.Hub{"h", OnbUi.live()}]}

law onboard_live_log_shows:
  {U.Tour.at(OnbUi.live()) == 0n : Nat}

# a long log comes in chunks: the tour waits for the last, which may hold
# the "onboard.done" that keeps it shut
def OnbUi.chunk() -> U.Ui:
  U.Ui.recv.b(0n, U.Ui.init(1n), U.Boot.demo("log", True{}, False{}, [C.Change.encode(M.ProjectCreated{"p1", "p", "/p", 1n})]))

law onboard_join_waits:
  {U.Tour.at(OnbUi.chunk()) == OBD.Ob.count() : Nat}

law onboard_join_shows:
  {U.Tour.at(U.Ui.recv.b(1n, OnbUi.chunk(), U.Boot.demo("changes", False{}, False{}, Nil{}))) == 0n : Nat}

law onboard_cache_unsynced:
  {U.Tour.at(U.Ui.unsync(OnbUi.live())) == OBD.Ob.count() : Nat}

law onboard_phone_cache_unsynced:
  {U.Tour.at(Hb.Hubs.ui(Hb.Hubs.stale(OnbUi.phone()), "h")) == OBD.Ob.count() : Nat}

# the keyboard goes back to, and the head is pointed at, only what is on
# screen: a thread selected under a room, a remote chat, a bot's view or
# Settings is not
def OnbUi.thread() -> U.Ui:
  U.Bots.sel.set(OnbUi.live(), "t1")

def OnbUi.step(u: U.Ui, s: String) -> U.Ui:
  U.Ui.scratch_set(u, "@tour", s)

law onboard_focus_thread:
  {U.Tour.focus(OnbUi.thread()) == "composer" : String}

law onboard_focus_none:
  {U.Tour.focus(OnbUi.live()) == "" : String}

law onboard_focus_remote:
  {U.Tour.focus(U.Bots.views(OnbUi.thread(), "", "", "x")) == "remote-send" : String}

law onboard_focus_room:
  {U.Tour.focus.k(False{}, True{}, "", True{}) == "room-post" : String}

law onboard_focus_settings:
  {U.Tour.focus(U.Ui.flag_on(OnbUi.thread(), "settings")) == "" : String}

law onboard_head_thread:
  {U.Tour.target(0n, OnbUi.step(OnbUi.thread(), "m3")) == "head" : String}

law onboard_head_remote:
  {U.Tour.target(0n, OnbUi.step(U.Bots.views(OnbUi.thread(), "", "", "x"), "m3")) == "" : String}

law onboard_head_settings:
  {U.Tour.target(0n, OnbUi.step(U.Ui.flag_on(OnbUi.thread(), "settings"), "m3")) == "" : String}

# keys pressed while the tour is up stay quarantined until let go, however
# the tour closes, until their release
def OnbN.n() -> N.Nui:
  N.Nui.init(U.Ui.init(1n), 800, 600)

law held_set_has:
  {N.Held.has(N.Nui.ui(N.Held.set(OnbN.n(), 36)), 36) == True{} : Bool}

law held_other_key:
  {N.Held.has(N.Nui.ui(N.Held.set(OnbN.n(), 36)), 38) == False{} : Bool}

law held_let_frees:
  {N.Held.has(N.Nui.ui(N.Held.let(N.Held.set(OnbN.n(), 36), 36)), 36) == False{} : Bool}

# with nothing in quarantine every key press and release goes through as it
# always did
law held_empty_passes_press:
  {N.Nui.event.t(False{}, OnbN.n(), W.WKey{97, 97, 0, True{}, 38}) == N.Nui.event.go(False{}, OnbN.n(), W.WKey{97, 97, 0, True{}, 38}) : N.Step}

law held_empty_passes_release:
  {N.Nui.event.t(False{}, OnbN.n(), W.WKey{97, 0, 0, False{}, 38}) == N.Nui.event.go(False{}, N.Held.let(OnbN.n(), 38), W.WKey{97, 0, 0, False{}, 38}) : N.Step}

law held_none_at_first:
  {N.Held.has(N.Nui.ui(OnbN.n()), 36) == False{} : Bool}

law held_swallowed_without_tour:
  {N.Nui.event.t(False{}, N.Held.set(OnbN.n(), 36), W.WKey{65293, 0, 0, True{}, 36}) == N.Step{N.Held.set(OnbN.n(), 36), Nil{}, False{}} : N.Step}

# the wheel over the tour's card scrolls two lines a notch within what
# overflows
law tour_scroll_down:
  {N.Tour.scroll(0n, 5n, False{}) == 2n : Nat}

law tour_scroll_down_stops:
  {N.Tour.scroll(4n, 5n, False{}) == 5n : Nat}

law tour_scroll_up:
  {N.Tour.scroll(3n, 5n, True{}) == 1n : Nat}

law tour_scroll_up_stops:
  {N.Tour.scroll(1n, 5n, True{}) == 0n : Nat}

law tour_scroll_none:
  {N.Tour.scroll(0n, 0n, False{}) == 0n : Nat}

# Pairing the owner's machines (client.bend's Mate.*): the hub with the
# lower address asks a machine of the owner's it is not linked to; never
# one already linked, never one sync is off with, never from the higher
law mate_lower_asks:
  {U.Mate.due(True{}, U.Mate.demo("http://100.1.0.1:3787"), Nil{}, M.State.empty()) == [U.MRow{"desk", "http://100.2.0.1:3787", True{}}] : List<&2, U.MRow>}

law mate_higher_waits:
  {U.Mate.due(True{}, U.Mate.demo("http://100.3.0.1:3787"), Nil{}, M.State.empty()) == Nil{} : List<&2, U.MRow>}

law mate_linked_never_asks:
  {U.Mate.due(True{}, U.Mate.demo("http://100.1.0.1:3787"), [BOT.Peer{"p1", "desk", "http://desk.t.ts.net:3787", 0n, False{}}], M.State.empty()) == Nil{} : List<&2, U.MRow>}

law mate_off_never_asks:
  {U.Mate.due(True{}, U.Mate.demo("http://100.1.0.1:3787"), Nil{}, M.State.setting(M.State.empty(), "sync.desk", "off")) == Nil{} : List<&2, U.MRow>}

law mate_no_self_never_asks:
  {U.Mate.due(True{}, U.Mate.demo(""), Nil{}, M.State.empty()) == Nil{} : List<&2, U.MRow>}

# a hub without its own tailnet pairs only with machines BACKPLANE_PEERS
# names, never with the owner's tailnet machines it lists
law mate_unnamed_without_tailnet:
  {U.Mate.due(False{}, U.Mate.demo("http://100.1.0.1:3787"), Nil{}, M.State.empty()) == Nil{} : List<&2, U.MRow>}

# the machine asked answers an invite, unless the asker is none of the
# machines it found, sync with it is off there or a live link reaches it
law mate_answer_stranger:
  {U.Mate.answer(U.Mate.demo("http://100.3.0.1:3787"), Nil{}, M.State.empty(), "desk", "http://127.0.0.1:9999") == 3n : Nat}

law mate_answer_invites:
  {U.Mate.answer(U.Mate.demo("http://100.3.0.1:3787"), Nil{}, M.State.empty(), "desk", "http://desk.t.ts.net:3787") == 0n : Nat}

law mate_answer_off:
  {U.Mate.answer(U.Mate.demo("http://100.3.0.1:3787"), Nil{}, M.State.setting(M.State.empty(), "sync.desk", "off"), "desk", "http://desk.t.ts.net:3787") == 1n : Nat}

law mate_answer_linked:
  {U.Mate.answer(U.Mate.demo("http://100.3.0.1:3787"), [BOT.Peer{"p1", "desk", "http://desk.t.ts.net:3787", 0n, False{}}], M.State.empty(), "desk", "http://desk.t.ts.net:3787") == 2n : Nat}

# a machine sync is off with lists nothing of its own in the sidebar
law mate_off_hidden:
  {U.Mate.hidden(FR.Fr.mark(M.State.setting(M.State.empty(), "sync.desk", "off"), "L", "desk", False{}), "L~p1") == True{} : Bool}

law mate_on_shown:
  {U.Mate.hidden(FR.Fr.mark(M.State.empty(), "L", "desk", False{}), "L~p1") == False{} : Bool}

# A catalog lists a link's projects at once; each is catching up until the
# link's head reaches its last number, and a project here never is
law far_catalog_stands_in:
  {U.Side.pids(U.Side.sorted(FR.Fr.cat.demo(0n), 0n)) == ["L~p1"] : List<&2, String>}

law far_catalog_syncing:
  {FR.Fr.syncing(FR.Fr.cat.demo(4n), "L~p1") == True{} : Bool}

law far_catalog_caught_up:
  {FR.Fr.syncing(FR.Fr.cat.demo(5n), "L~p1") == False{} : Bool}

law far_catalog_here_never:
  {FR.Fr.syncing(FR.Fr.cat.demo(0n), "p1") == False{} : Bool}

law far_catalog_titled:
  {U.Label.title(FR.Fr.cat.demo(4n), "L~p1", "Board") == "Board · desk (syncing)" : String}

# a change's project: its own, or its thread's
law far_scope_thread:
  {FR.Fr.scope(M.State.apply(M.State.empty(), M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n}), M.MessagePosted{"t1", "m1", M.User{}, "hi", 2n}) == "p1" : String}

law far_scope_project:
  {FR.Fr.scope(M.State.empty(), M.ProjectCreated{"p2", "x", "/x", 1n}) == "p2" : String}

# a thread on another of the owner's machines is `>machine:slug`; a word
# ending in ':' still names a thread here (`>fix: it fails`)
law refs_machine_word:
  {RF.Refs.x.word("desk:fix-perf") == True{} : Bool}

law refs_colon_after_stays_here:
  {RF.Refs.x.word("fix:") == False{} : Bool}

# two hubs on one machine compare their loopback addresses, so exactly
# one of them asks: 4001 asks 4002, and 4003 does not ask 4002
law mate_loopback_lower_asks:
  {U.Mate.due(False{}, U.Mate.demo.lo("4001"), Nil{}, M.State.empty()) == [U.MRow{"127.0.0.1", "http://127.0.0.1:4002", True{}}] : List<&2, U.MRow>}

law mate_loopback_higher_waits:
  {U.Mate.due(False{}, U.Mate.demo.lo("4003"), Nil{}, M.State.empty()) == Nil{} : List<&2, U.MRow>}

# Viewers for threads elsewhere (core/fview.bend): the owning hub answers
# only reads about a thread it shares, a path never leaves the thread's
# folder (and never reaches a hidden file), and what is kept here from
# another machine lives under <home>/farfs/<link>, whatever the link or the
# folder says.

law fview_file_up:
  {FVW.Fv.file.ok("lib/../../etc/passwd") == False{} : Bool}

law fview_file_abs:
  {FVW.Fv.file.ok("/etc/passwd") == False{} : Bool}

law fview_file_hidden:
  {FVW.Fv.file.ok(".git/config") == False{} : Bool}

law fview_file_hidden_deep:
  {FVW.Fv.file.ok("hw/.env") == False{} : Bool}

law fview_file_pick:
  {FVW.Fv.file.ok(".backplane.json") == True{} : Bool}

law fview_file_ok:
  {FVW.Fv.file.ok("hw/board.kicad_pcb") == True{} : Bool}

law fview_file_empty:
  {FVW.Fv.file.ok("") == False{} : Bool}

law fview_dir_root:
  {FVW.Fv.dir.ok("") == True{} : Bool}

law fview_dir_up:
  {FVW.Fv.dir.ok("../x") == False{} : Bool}

law fview_reads_disjoint:
  {FVW.Fv.disjoint(FR.Fr.methods(), FVW.Fv.reads()) == True{} : Bool}

law fview_no_open:
  {FVW.Fv.read("file.open") == False{} : Bool}

law fview_no_pdf:
  {FVW.Fv.read("pdf.page") == False{} : Bool}

law fview_no_fs:
  {FVW.Fv.read("fs.list") == False{} : Bool}

law fview_allowed_not_write:
  for +s: M.State
  for +p: J.Json
  {FVW.Fv.allowed(s, J.Json.obj([J.KV{"m", J.Str{"turn.start"}}, J.KV{"p", p}])) == False{} : Bool}

law fview_allowed_path:
  for +s: M.State
  {FVW.Fv.allowed(s, J.Json.obj([J.KV{"m", J.Str{"view.get"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"path", J.Str{"../../etc/passwd"}}])}])) == False{} : Bool}

law fview_allowed_want:
  for +s: M.State
  {FVW.Fv.allowed(s, J.Json.obj([J.KV{"m", J.Str{"view.ver"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"want", J.Str{"a.kicad_pcb\n.ssh/id_rsa"}}])}])) == False{} : Bool}

law fview_allowed_dir:
  for +s: M.State
  {FVW.Fv.allowed(s, J.Json.obj([J.KV{"m", J.Str{"files.dir"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"path", J.Str{"/etc"}}])}])) == False{} : Bool}

law fview_cache_under:
  {FVW.Fv.cache("/h", "beta", "/home/u/p") == "/h/farfs/beta/home/u/p" : String}

law fview_cache_dotdot:
  {FVW.Fv.cache("/h", "beta", "/home/../etc") == "" : String}

law fview_cache_link_slash:
  {FVW.Fv.cache("/h", "a/b", "/p") == "" : String}

law fview_cache_link_dotdot:
  {FVW.Fv.cache("/h", "..", "/p") == "" : String}

law fview_cache_relative:
  {FVW.Fv.cache("/h", "beta", "p") == "" : String}

law fview_cache_of_file:
  {FVW.Fv.cache.of("/h", "b", "/p", "/p/a.kicad_pcb") == "/h/farfs/b/p/a.kicad_pcb" : String}

law fview_cache_of_outside:
  {FVW.Fv.cache.of("/h", "b", "/p", "/q/a") == "" : String}

law fview_cache_of_prefix:
  {FVW.Fv.cache.of("/h", "b", "/p", "/pq/a") == "" : String}

law fview_fix_thread:
  {J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(FVW.Fv.fix("/h", "b", "{\"t\":\"reply\",\"id\":0,\"ok\":true,\"result\":{\"thread\":\"t1\",\"fmRoot\":\"/p\"}}")), J.Null{}), "result"), "thread", "") == "b~t1" : String}

law fview_fix_root:
  {J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(FVW.Fv.fix("/h", "b", "{\"t\":\"reply\",\"id\":0,\"ok\":true,\"result\":{\"thread\":\"t1\",\"fmRoot\":\"/p\"}}")), J.Null{}), "result"), "fmRoot", "") == "/h/farfs/b/p" : String}

law fview_fix_again:
  {FVW.Fv.fix("/h", "b", FVW.Fv.fix("/h", "b", "{\"t\":\"reply\",\"id\":0,\"ok\":true,\"result\":{\"thread\":\"t1\",\"root\":\"/p\"}}")) == FVW.Fv.fix("/h", "b", "{\"t\":\"reply\",\"id\":0,\"ok\":true,\"result\":{\"thread\":\"t1\",\"root\":\"/p\"}}") : String}

law fview_fix_leaves_error:
  {FVW.Fv.fix("/h", "b", "not json") == "not json" : String}
# Agents across machines (core/fmcp.bend)
# ----------------------------------------
# An agent's thread tool naming another machine is a signed request to that
# machine's hub, which runs it only for what it shares; a machine that is
# not linked here is never sent anything, and only the listed tools are
# taken from the owner's machines.

# a tool call as the hub hears it
def Fx.call(+tool: String, +a: J.Json) -> J.Json:
  J.Json.obj([J.KV{"method", J.Str{"tools/call"}}, J.KV{"params", J.Json.obj([J.KV{"name", J.Str{tool}}, J.KV{"arguments", a}])}])

def Fx.ask(+tool: String) -> J.Json:
  J.Json.obj([J.KV{"m", J.Str{"mcp.call"}}, J.KV{"p", J.Json.obj([J.KV{"tool", J.Str{tool}}])}])

law fmc_taken_tool:
  {FMC.Fm.allowed(Fx.ask("thread_send")) == True{} : Bool}

law fmc_only_listed_tools:
  {FMC.Fm.allowed(Fx.ask("work_run")) == False{} : Bool}

law fmc_no_terminal_or_delegate:
  {FMC.Fm.allowed(Fx.ask("terminal_open")) == False{} : Bool}

law fmc_permit_keeps_client_rules:
  for +s: M.State
  for +p: J.Json
  {FMC.Fm.permit(s, J.Json.obj([J.KV{"m", J.Str{"thread.delete"}}, J.KV{"p", p}])) == False{} : Bool}

# a thread this machine does not share is neither written to nor changed
law fmc_unshared_refused:
  {MC.Res.good(FMC.Fm.call(M.State.empty(), 1000n, "thread_send", "p1~t9", J.Json.obj([J.KV{"threadId", J.Str{"t1"}}, J.KV{"text", J.Str{"hi"}}]))) == False{} : Bool}

law fmc_unshared_changes_nothing:
  {List.is_empty(&2, M.Change, MC.Res.changes(FMC.Fm.call(M.State.empty(), 1000n, "thread_send", "p1~t9", J.Json.obj([J.KV{"threadId", J.Str{"t1"}}, J.KV{"text", J.Str{"hi"}}])))) == True{} : Bool}

law fmc_unshared_interrupt_refused:
  {MC.Res.good(FMC.Fm.call(M.State.empty(), 1000n, "thread_interrupt", "p1~t9", J.Json.obj([J.KV{"threadId", J.Str{"t1"}}]))) == False{} : Bool}

# where a call goes
law fmc_plan_here_stays:
  {FMC.Fm.plan.name(FMC.Fm.plan.o(Fx.call("thread_send", J.Json.obj([J.KV{"threadId", J.Str{"t1"}}])), M.State.empty(), Nil{}, "t0")) == "here" : String}

law fmc_plan_wait_polls_here:
  {FMC.Fm.plan.name(FMC.Fm.plan.o(Fx.call("thread_wait", J.Json.obj([J.KV{"threadId", J.Str{"p1~t1"}}])), M.State.empty(), Nil{}, "t0")) == "here" : String}

law fmc_plan_unlinked_never_sent:
  {FMC.Fm.plan.name(FMC.Fm.plan.o(Fx.call("thread_send", J.Json.obj([J.KV{"threadId", J.Str{"p1~t1"}}])), M.State.empty(), Nil{}, "t0")) == "answer" : String}

law fmc_plan_unlinked_machine_never_sent:
  {FMC.Fm.plan.name(FMC.Fm.plan.o(Fx.call("thread_launch", J.Json.obj([J.KV{"machine", J.Str{"beta"}}])), M.State.empty(), Nil{}, "t0")) == "answer" : String}

law fmc_plan_machine_list_answered:
  {FMC.Fm.plan.name(FMC.Fm.plan.o(Fx.call("machine_list", J.Json.obj(Nil{})), M.State.empty(), Nil{}, "t0")) == "answer" : String}

law fmc_plan_other_tools_here:
  {FMC.Fm.plan.name(FMC.Fm.plan.o(Fx.call("work_run", J.Json.obj([J.KV{"command", J.Str{"ls"}}])), M.State.empty(), Nil{}, "t0")) == "here" : String}

# ids: the machine's own going out, its link coming back
law fmc_args_made_local:
  {J.Json.show(J.Obj{FMC.Fm.args.local(FR.Fr.obj.fields(J.Json.obj([J.KV{"threadId", J.Str{"p1~t1"}}, J.KV{"text", J.Str{"a~b"}}])), "p1")}) == "{\"threadId\":\"t1\",\"text\":\"a~b\"}" : String}

law fmc_args_other_link_kept:
  {J.Json.show(J.Obj{FMC.Fm.args.local(FR.Fr.obj.fields(J.Json.obj([J.KV{"threadId", J.Str{"p2~t1"}}])), "p1")}) == "{\"threadId\":\"p2~t1\"}" : String}

law fmc_reply_ids_named_by_link:
  {J.Json.show(FMC.Fm.far.json(J.Json.obj([J.KV{"threadId", J.Str{"t1"}}, J.KV{"title", J.Str{"t1"}}, J.KV{"threads", J.Json.arr([J.Json.obj([J.KV{"id", J.Str{"t2"}}, J.KV{"project", J.Str{""}}])])}]), "p1")) == "{\"threadId\":\"p1~t1\",\"title\":\"t1\",\"threads\":[{\"id\":\"p1~t2\",\"project\":\"\"}]}" : String}

# /give sends nothing to a machine that is not linked: each word naming
# one is an error the person hears
law fmc_give_unlinked_nothing:
  {FMC.Fm.give.names(FMC.Fm.give(M.State.empty(), Nil{}, J.Json.obj([J.KV{"m", J.Str{"bots.give"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"text", J.Str{"/give >beta:fix and %beta:board"}}])}]), 5n)) == "answer answer " : String}

# the hub's own /give never reads a machine's word as one of its own
law fmc_give_local_drops_machines:
  {FMC.Fm.give.text("/give >beta:fix %mine %beta:board now") == "/give %mine now" : String}
# Live text of threads elsewhere (core/farlive.bend): ephemeral, ordered by
# length and generation, never a change

law flive_delta_appends:
  {FLV.FL.text(FLV.FL.get(FLV.Took.bs(FLV.FL.take(FLV.Took.bs(FLV.FL.take(Nil{}, "b~t", "ab", 5n, 0n, False{}, False{})), "b~t", "cd", 5n, 2n, False{}, False{})), "b~t")) == "abcd" : String}

law flive_gap_asks:
  {FLV.Took.v(FLV.FL.take(FLV.Took.bs(FLV.FL.take(Nil{}, "b~t", "ab", 5n, 0n, False{}, False{})), "b~t", "zz", 5n, 9n, False{}, False{})) == 2n : Nat}

law flive_gap_changes_nothing:
  {FLV.FL.text(FLV.FL.get(FLV.Took.bs(FLV.FL.take(FLV.Took.bs(FLV.FL.take(Nil{}, "b~t", "ab", 5n, 0n, False{}, False{})), "b~t", "zz", 5n, 9n, False{}, False{})), "b~t")) == "ab" : String}

law flive_end_blocks_old:
  {FLV.Took.v(FLV.FL.take(FLV.FL.ends([FLV.End{"b~t", 9n}], FLV.Took.bs(FLV.FL.take(Nil{}, "b~t", "ab", 5n, 0n, False{}, False{}))), "b~t", "ab", 5n, 0n, False{}, False{})) == 1n : Nat}

law flive_end_no_resurrect:
  {FLV.FL.text(FLV.FL.get(FLV.Took.bs(FLV.FL.take(FLV.FL.ends([FLV.End{"b~t", 9n}], FLV.Took.bs(FLV.FL.take(Nil{}, "b~t", "ab", 5n, 0n, False{}, False{}))), "b~t", "ab", 5n, 0n, False{}, False{})), "b~t")) == "" : String}

law flive_next_gen_taken:
  {FLV.FL.text(FLV.FL.get(FLV.Took.bs(FLV.FL.take(FLV.FL.ends([FLV.End{"b~t", 9n}], FLV.Took.bs(FLV.FL.take(Nil{}, "b~t", "ab", 5n, 0n, False{}, False{}))), "b~t", "cd", 9n, 0n, False{}, False{})), "b~t")) == "cd" : String}

# the durable end came first (a restart, a reordering): a delta of the
# stream it ended, arriving after, never shows
law flive_end_before_any:
  {FLV.Took.v(FLV.FL.take(FLV.FL.ends([FLV.End{"b~t", 9n}], Nil{}), "b~t", "ab", 5n, 0n, False{}, False{})) == 1n : Nat}

# an end of an older stream leaves a newer one that came first standing
law flive_end_keeps_newer:
  {FLV.FL.text(FLV.FL.get(FLV.FL.ends([FLV.End{"b~t", 9n}], FLV.Took.bs(FLV.FL.take(Nil{}, "b~t", "cd", 10n, 0n, False{}, False{}))), "b~t")) == "cd" : String}

law flive_snap_replaces:
  {FLV.FL.text(FLV.FL.get(FLV.Took.bs(FLV.FL.take(FLV.Took.bs(FLV.FL.take(Nil{}, "b~t", "ab", 5n, 0n, False{}, False{})), "b~t", "abcdef", 5n, 0n, False{}, True{})), "b~t")) == "abcdef" : String}

law flive_snap_older_stale:
  {FLV.Took.v(FLV.FL.take(FLV.FL.ends([FLV.End{"b~t", 9n}], FLV.Took.bs(FLV.FL.take(Nil{}, "b~t", "ab", 5n, 0n, False{}, False{}))), "b~t", "ab", 5n, 0n, False{}, True{})) == 1n : Nat}

law flive_ended_ids:
  {FLV.FL.ended.ids(FLV.FL.ended([FR.Item{4n, C.Change.encode(M.MessagePosted{"t", "m:1", M.Assistant{}, "hi", 3n})}, FR.Item{5n, C.Change.encode(M.TurnChanged{"u", M.Completed{}, 4n})}, FR.Item{6n, C.Change.encode(M.TurnChanged{"v", M.Running{}, 5n})}], "b")) == ["b~t", "b~u"] : List<&2, String>}

law flive_user_message_keeps:
  {FLV.FL.ended.ids(FLV.FL.ended([FR.Item{4n, C.Change.encode(M.MessagePosted{"t", "m:1", M.User{}, "hi", 3n})}], "b")) == Nil{} : List<&2, String>}

law flive_body_no_items:
  {List.length(&2, FR.Item, FR.Fr.items.of(J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(FLV.FL.body("a", 0n, [FLV.LI{"t", 5n, 0n, "ab", 2n, False{}}])), J.Null{}), "items")))) == 0n : Nat}

law flive_body_has_live:
  {List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(FLV.FL.body("a", 0n, [FLV.LI{"t", 5n, 0n, "ab", 2n, False{}}])), J.Null{}), "live"))) == 1n : Nat}

law flive_queue_joins:
  {FLV.LQ.pend(FLV.LQ.add(FLV.LQ.add(FLV.LQ.new("b"), 10n, "t", 5n, 0n, "ab"), 10n, "t", 5n, 2n, "cd")) == [FLV.LI{"t", 5n, 0n, "abcd", 4n, False{}}] : List<&2, FLV.LI>}

law flive_queue_gap_owes_whole:
  {FLV.LQ.pend(FLV.LQ.add(FLV.LQ.add(FLV.LQ.new("b"), 10n, "t", 5n, 0n, "ab"), 10n, "t", 5n, 7n, "cd")) == [FLV.LI{"t", 5n, 0n, "", 0n, True{}}] : List<&2, FLV.LI>}

law flive_queue_whole_absorbs:
  {FLV.LQ.pend(FLV.LQ.add(FLV.LQ.need(FLV.LQ.new("b"), "t", 0n), 10n, "t", 5n, 0n, "ab")) == [FLV.LI{"t", 0n, 0n, "", 0n, True{}}] : List<&2, FLV.LI>}

law flive_queue_threads_apart:
  {List.length(&2, FLV.LI, FLV.LQ.pend(FLV.LQ.add(FLV.LQ.add(FLV.LQ.new("b"), 10n, "t", 5n, 0n, "ab"), 10n, "u", 5n, 0n, "cd"))) == 2n : Nat}

law flive_one_request_out:
  {FLV.LQ.due(FLV.LQ.add(FLV.LQ.start(FLV.LQ.add(FLV.LQ.new("b"), 10n, "t", 5n, 0n, "ab")), 10n, "t", 5n, 2n, "cd")) == False{} : Bool}

law flive_due_after_answer:
  {FLV.LQ.due(FLV.LQ.done(FLV.LQ.add(FLV.LQ.start(FLV.LQ.add(FLV.LQ.new("b"), 10n, "t", 5n, 0n, "ab")), 10n, "t", 5n, 2n, "cd"), True{}, 10n)) == True{} : Bool}

law flive_quiet_drops:
  {FLV.LQ.pend(FLV.LQ.add(FLV.LQ.done(FLV.LQ.start(FLV.LQ.add(FLV.LQ.new("b"), 10n, "t", 5n, 0n, "ab")), False{}, 100n), 105n, "t", 5n, 2n, "cd")) == Nil{} : List<&2, FLV.LI>}

law flive_quiet_ends:
  {FLV.LQ.pend(FLV.LQ.add(FLV.LQ.done(FLV.LQ.start(FLV.LQ.add(FLV.LQ.new("b"), 10n, "t", 5n, 0n, "ab")), False{}, 100n), 108n, "t", 5n, 2n, "cd")) == [FLV.LI{"t", 5n, 2n, "cd", 2n, False{}}] : List<&2, FLV.LI>}

law flive_whole_from_buffer:
  {FLV.FL.fill([FLV.LI{"t", 0n, 0n, "", 0n, True{}}], LV.Live.add(LV.Live.add(Nil{}, "t", "ab"), "t", "cd")) == [FLV.LI{"t", 0n, 0n, "abcd", 4n, True{}}] : List<&2, FLV.LI>}

# a delta of a stream that ended before its request went is not sent
law flive_delta_ended_dropped:
  {FLV.FL.fill([FLV.LI{"t", 0n, 0n, "ab", 2n, False{}}], LV.Live.end(LV.Live.add(Nil{}, "t", "ab"), "t")) == Nil{} : List<&2, FLV.LI>}

law flive_whole_ended_dropped:
  {FLV.FL.fill([FLV.LI{"t", 0n, 0n, "", 0n, True{}}], LV.Live.end(LV.Live.add(Nil{}, "t", "ab"), "t")) == Nil{} : List<&2, FLV.LI>}

law flive_link_gone:
  {LV.Live.msg(FLV.FL.drop.link(LV.Live.add(LV.Live.add(Nil{}, "b~t", "ab"), "t", "cd"), "b~")) == LV.Live.msg(LV.Live.add(Nil{}, "t", "cd")) : String}

law flive_far_message_ends_live:
  {U.Ui.live.of(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{"b~t"}}, J.KV{"text", J.Str{"hel"}}])), Maybe.default(&2, J.Json, J.Json.parse(FR.Fr.msg("b", "Beta", False{}, 3n, 4n, [FR.Item{4n, C.Change.encode(M.MessagePosted{"t", "m:1", M.Assistant{}, "hello", 3n})}])), J.Null{})), "b~t") == "" : String}

law flive_far_activity_keeps_live:
  {U.Ui.live.of(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{"b~t"}}, J.KV{"text", J.Str{"hel"}}])), Maybe.default(&2, J.Json, J.Json.parse(FR.Fr.msg("b", "Beta", False{}, 3n, 4n, [FR.Item{4n, C.Change.encode(M.TodosSet{"t", "[]", 3n})}])), J.Null{})), "b~t") == "hel" : String}

law flive_far_delta_shows:
  {U.Ui.live.of(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{"b~t"}}, J.KV{"text", J.Str{"hel"}}])), J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{"b~t"}}, J.KV{"text", J.Str{"lo"}}])), "b~t") == "hello" : String}

# Alerts of threads elsewhere: one key, one island line

law notice_far_key_same:
  {NO.Notice.key(NO.Notice.of(M.State.empty(), "b~t", False{}, False{})) == "turn-t" : String}

law notice_far_fail_key_same:
  {NO.Notice.kind(NO.Notice.of(M.State.empty(), "b~t", True{}, False{})) ++ " " ++ NO.Notice.key(NO.Notice.of(M.State.empty(), "b~t", True{}, False{})) == "fail turn-t" : String}

law notice_local_key_kept:
  {NO.Notice.key(NO.Notice.of(M.State.empty(), "t", False{}, False{})) == "turn-t" : String}

law island_far_once:
  {List.length(&2, J.Json, Hb.Hubs.lines.once([J.Json.obj([J.KV{"thread", J.Str{"b~t"}}]), J.Json.obj([J.KV{"thread", J.Str{"t"}}])])) == 1n : Nat}

law island_far_owner_wins:
  {J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, Hb.Hubs.lines.once([J.Json.obj([J.KV{"thread", J.Str{"b~t"}}]), J.Json.obj([J.KV{"thread", J.Str{"t"}}])])), J.Null{}), "thread", "") == "t" : String}

law island_far_alone_kept:
  {List.length(&2, J.Json, Hb.Hubs.lines.once([J.Json.obj([J.KV{"thread", J.Str{"b~t"}}]), J.Json.obj([J.KV{"thread", J.Str{"u"}}])])) == 2n : Nat}

# A window on another machine's hub (core/rview.bend): the hub answers a
# client's viewer reads only for a thread it has a folder for and a path the
# viewers may read; what the window keeps of a hub's files lives under its
# own folder for that hub, whatever the path says.

law rview_at_under:
  {RVW.Rview.at("/c/remote/hb", "/home/u/p/a.kicad_pcb") == "/c/remote/hb/home/u/p/a.kicad_pcb" : String}

law rview_at_climbs:
  {RVW.Rview.at("/c/remote/hb", "/home/u/../../../etc/passwd") == "" : String}

law rview_at_relative:
  {RVW.Rview.at("/c/remote/hb", "git\tx") == "git\tx" : String}

law rview_back_at:
  {RVW.Rview.back("/c/remote/hb", RVW.Rview.at("/c/remote/hb", "/home/u/p/a.kicad_pcb")) == "/home/u/p/a.kicad_pcb" : String}

law rview_back_outside:
  {RVW.Rview.back("/c/remote/hb", "/etc/passwd") == "" : String}

law rview_want_hidden:
  {RVW.Rview.want("/c/remote/hb", "/c/remote/hb/p", "/c/remote/hb/p/.env") == "" : String}

law rview_want_file:
  {RVW.Rview.want("/c/remote/hb", "/c/remote/hb/p", "/c/remote/hb/p/hw/a.step") == "hw/a.step" : String}

law rview_ask_get_up:
  {RVW.Rview.ask.ok("/view/get", "/p", "", "", "../a") == False{} : Bool}

law rview_ask_get_abs:
  {RVW.Rview.ask.ok("/view/get", "/p", "", "", "/etc/passwd") == False{} : Bool}

law rview_ask_get_hidden:
  {RVW.Rview.ask.ok("/view/get", "/p", "", "", ".ssh/id_rsa") == False{} : Bool}

law rview_ask_get_ok:
  {RVW.Rview.ask.ok("/view/get", "/p", "", "", "hw/a.kicad_pcb") == True{} : Bool}

law rview_ask_ver_hidden:
  {RVW.Rview.ask.ok("/view/ver", "/p", "", "a.kicad_pcb\n.env", "") == False{} : Bool}

law rview_ask_no_folder:
  {RVW.Rview.ask.ok("/view/get", "", "", "", "a.kicad_pcb") == False{} : Bool}

law rview_ask_other:
  {RVW.Rview.ask.ok("/view/other", "/p", "", "", "a.kicad_pcb") == False{} : Bool}

law rview_git_option:
  {RVW.Rview.ask.ok("/view/git", "/p", "", "", "git\t/p/.git\t--output=/tmp/x\t2\t/p/a.kicad_pcb") == False{} : Bool}

law rview_git_dir:
  {RVW.Rview.ask.ok("/view/git", "/p", "", "", "git\t/etc\tabc123\t2\t/p/a.kicad_pcb") == False{} : Bool}

law rview_git_climbs:
  {RVW.Rview.ask.ok("/view/git", "/p", "", "", "git\t/p/.git\tabc123\t2\t/p/../../etc/passwd") == False{} : Bool}

law rview_git_ok:
  {RVW.Rview.ask.ok("/view/git", "/p", "", "", "git\t/p/.git\tabc123\t2\t/p/a.kicad_pcb") == True{} : Bool}

# a version is read only from the thread's own repository, and never a
# hidden file of it
law rview_git_other_repo:
  {RVW.Rview.ask.ok("/view/git", "/p", "", "", "git\t/private/.git\tabc123\t8\t/private/a.kicad_pcb") == False{} : Bool}

law rview_git_hidden:
  {RVW.Rview.ask.ok("/view/git", "/p", "", "", "git\t/p/.git\tabc123\t2\t/p/.env") == False{} : Bool}

law rview_root_unknown:
  {RVW.Rview.root.k(False{}, False{}, "/h", "", "/p", "") == "" : String}

law rview_root_far_cache:
  {RVW.Rview.root.k(False{}, True{}, "/h", "beta", "", "/h/farfs/beta/p") == "/h/farfs/beta/p" : String}

law rview_root_far_other_link:
  {RVW.Rview.root.k(False{}, True{}, "/h", "beta", "", "/h/farfs/gamma/p") == "" : String}

law rview_root_far_climbs:
  {RVW.Rview.root.k(False{}, True{}, "/h", "beta", "", "/h/farfs/beta/../../etc") == "" : String}

law rview_root_own_ignores_claim:
  {RVW.Rview.root.k(True{}, False{}, "/h", "", "/p", "/etc") == "/p" : String}

law rview_enc_unreserved:
  {RVW.Rview.enc("hw/a-b_c.kicad_pcb") == "hw/a-b_c.kicad_pcb" : String}

law rview_enc_breaks:
  {RVW.Rview.enc("a\nb&c=d") == "a%0Ab%26c%3Dd" : String}

law rview_spec_local:
  {RVW.Rview.spec("", "/c", "t", "g") == "" : String}

# The layer list and its focus (core/layers.bend)
# ------------------------------------------------
# A board's layers listed in KiCad's order: copper front to back, pads,
# vias, holes, then the technical layers front/back paired
law lyr_board_order_kicad:
  {LY.Lyr.order(0) == [12, 11, 10, 9, 8, 7, 6, 5, 13, 23, 24, 15, 2, 16, 3, 14, 4, 19, 20, 22, 21, 17, 1, 18, 0] : List<&2, U32>}

# with no focus every layer looks as it does
law lyr_plain_as_is:
  {LY.Lyr.shade(0, LY.Lyr.plain(), 16, (7, 200)) == (7, 200) : U32 & U32}

# dimmed: the other layers at a quarter
law lyr_dim_quarter:
  {LY.Lyr.shade(0, LY.Focus{12, 1}, 16, (7, 200)) == (7, 50) : U32 & U32}

# only: the other layers hidden
law lyr_only_hides:
  {LY.Lyr.shade(0, LY.Focus{12, 2}, 16, (7, 200)) == (7, 0) : U32 & U32}

# the active layer always shows, even one hidden by default
law lyr_active_shows:
  {LY.Lyr.shade(0, LY.Focus{15, 2}, 15, (7, 0)) == (7, 200) : U32 & U32}

# a copper layer keeps its vias, and every layer the board's outline
law lyr_copper_keeps_vias:
  {LY.Lyr.shade(0, LY.Focus{5, 2}, 23, (7, 255)) == (7, 255) : U32 & U32}

law lyr_outline_stays:
  {LY.Lyr.shade(0, LY.Focus{16, 2}, 22, (7, 255)) == (7, 255) : U32 & U32}

# a click on the active layer again puts every layer back
law lyr_pick_again_all:
  {LY.Lyr.pick(LY.Focus{16, 1}, 16) == LY.Focus{16, 0} : LY.Focus}

# ctrl+H goes around: dimmed, hidden, as they are
law lyr_cycle_round:
  {LY.Lyr.cycle(0, LY.Lyr.cycle(0, LY.Lyr.cycle(0, LY.Focus{12, 0}))) == LY.Focus{12, 0} : LY.Focus}

# the window's kind word carries the focus and gives the kind back
law lyr_kind_round:
  {LY.Lyr.kind.focus(LY.Lyr.kind.with(9, LY.Focus{16, 2})) == LY.Focus{16, 2} : LY.Focus}

law lyr_kind_base:
  {LY.Lyr.kind.base(LY.Lyr.kind.with(9, LY.Focus{16, 2})) == 9 : U32}

# the masks the web and phones draw from agree: only hides every layer
# not kept and never the active one
law lyr_masks_only:
  {LY.Masks.off(LY.Lyr.masks(0, 4096, LY.Focus{12, 2})) == 37736447 : U32}

# Design review: comparing (core/hist.bend, client.bend's Rv.*)
# ---------------------------------------------------------------

# a comparison on no layers in particular is the plain comparison, and
# marks every layer
law rv_diff_on_all:
  for a: String
  for b: String
  {HS.Hist.diff.on(a, b, 0) == HS.Hist.diff(a, b) : String}

law rv_mark_on_all:
  for olds: List<&2, B.Item>
  for news: List<&2, B.Item>
  {HS.Hist.mark.on(0, olds, news) == HS.Hist.mark(olds, news) : List<&2, B.Item>}

# a sheet picked outside a comparison is the sheet; inside one, that sheet
# compared in both versions
law rv_sheet_plain:
  {U.Rv.sheet("/t/root.kicad_sch", "/t/kid.kicad_sch") == "/t/kid.kicad_sch" : String}

law rv_sheet_compared:
  {U.Rv.sheet(HS.Hist.diff(HS.Hist.git("/g", "c", "/t", "root.kicad_sch"), "/t/root.kicad_sch"), "/t/kid.kicad_sch") == HS.Hist.diff(HS.Hist.git("/g", "c", "/t", "kid.kicad_sch"), "/t/kid.kicad_sch") : String}

# with no layer standing out, a comparison is about every layer
law rv_bits_plain:
  for kind: U32
  {U.Rv.bits(kind, LY.Lyr.plain()) == 0 : U32}
# Design review (bom)

# references sort naturally: C2 before C10, and R10 after R2
law bom_nat_c2_c10:
  {BOM.Bom.before("C2", "C10") == True{} : Bool}

law bom_nat_r10_r2:
  {BOM.Bom.before("R10", "R2") == False{} : Bool}

# a BOM compared with itself has no changes
law bom_diff_self:
  {BOM.Bom.diff(BOM.Bom.sample(), BOM.Bom.sample()) == Nil{} : List<&2, BOM.Chg>}

# every BOM line's quantity is the number of its references
law bom_group_qty:
  {BOM.Bom.qty.ok(BOM.Bom.group(BOM.Bom.sample())) == True{} : Bool}

# R1 and R2 are alike: one line "R1, R2" with quantity 2
law bom_group_csv:
  {BOM.Bom.csv(BOM.Bom.group(BOM.Bom.sample())) == "Reference,Qty,Value,Footprint,MPN,Manufacturer,DNP\nC1,1,1u,0402,,,\n\"R1, R2\",2,10k,0603,,,\n" : String}

# a cell with a comma is quoted, a quote is doubled, a plain one is left
law bom_csv_quote:
  {BOM.Bom.q("a,b") == "\"a,b\"" : String}

law bom_csv_quote_inner:
  {BOM.Bom.q("5\" x") == "\"5\"\" x\"" : String}

law bom_csv_plain:
  {BOM.Bom.q("10k") == "10k" : String}

# the diff summary line
law bom_diff_line:
  {BOM.Bom.line(3n, 1n, 2n) == "3 added, 1 removed, 2 changed" : String}

law bom_diff_md_none:
  {BOM.Bom.diff.md(BOM.Bom.sample(), BOM.Bom.sample()) == "No changes" : String}

# a pipe in a cell is escaped
law bom_md_pipe:
  {BOM.Bom.c("a|b") == "a/b" : String}
# Design review (checks, cad, release)
# -------------------------------------

# The CAD layer knows a KiCad board by its extension
law cad_of_kicad_board:
  {CAD.Cad.of("a/b.kicad_pcb") == CAD.Hit{"kicad", "board", True{}} : CAD.Hit}

law cad_of_kicad_schematic:
  {CAD.Cad.of("x.kicad_sch") == CAD.Hit{"kicad", "schematic", True{}} : CAD.Hit}

law cad_of_case_ignored:
  {CAD.Cad.of("X.KICAD_PRO") == CAD.Hit{"kicad", "project", True{}} : CAD.Hit}

# a tool Backplane cannot run is recognised but marked not drawn
law cad_of_altium_not_drawn:
  {CAD.Cad.of("Main.PcbDoc") == CAD.Hit{"altium", "board", False{}} : CAD.Hit}

law cad_of_unknown:
  {CAD.Cad.of("readme.md") == CAD.Hit{"", "", False{}} : CAD.Hit}

# a template's placeholders are replaced by their values
law cad_fill_puts_vars_in:
  {CAD.Cad.fill(["{cli}", "{out}/x.csv"], [CAD.Var{"cli", "k"}, CAD.Var{"out", "/o"}]) == ["k", "/o/x.csv"] : List<&2, String>}

# a brace that names no variable passes through (KiCad's ${QUANTITY})
law cad_fill_keeps_other_braces:
  {CAD.Cad.fill(["${QUANTITY}"], [CAD.Var{"cli", "k"}]) == ["${QUANTITY}"] : List<&2, String>}

# a single quote inside a word is written '\'' so sh reads it back
law cad_shell_quotes_a_quote:
  {CAD.Cad.quote("a'b") == "'a'\\''b'" : String}

law cad_shell_words:
  {CAD.Cad.shell(["a", "b c"]) == "'a' 'b c'" : String}

# ticking a box and ticking it again gives the file back, byte for byte
law checklist_toggle_twice:
  {CHK.Checklist.toggle(CHK.Checklist.toggle("# A\n- [ ] x\n* [x] y\n", 1n), 1n) == "# A\n- [ ] x\n* [x] y\n" : String}

law checklist_toggle_flips_one:
  {CHK.Checklist.toggle("# A\n- [ ] x\n* [x] y\n", 1n) == "# A\n- [ ] x\n* [ ] y\n" : String}

# a box that is not there changes nothing
law checklist_toggle_past_end:
  {CHK.Checklist.toggle("- [ ] x\n", 5n) == "- [ ] x\n" : String}

# with no findings the gate passes
law checks_gate_no_findings:
  {CHK.Checks.gate(Nil{}, CHK.Checks.gate.default()) == "" : String}

law checks_gate_no_findings_strict:
  {CHK.Checks.gate(Nil{}, CHK.Gate{"warning", "warning", True{}}) == "" : String}

law checks_md_empty:
  {CHK.Checks.md(Nil{}, Nil{}, Nil{}) == "**Design checks:** No findings\n" : String}

# an ERC report's violation becomes one finding of its severity
law checks_erc_error_counted:
  {CHK.Checks.count(CHK.Checks.erc("{\"sheets\":[{\"path\":\"/\",\"violations\":[{\"type\":\"t\",\"severity\":\"error\",\"description\":\"d\",\"items\":[]}]}]}"), "erc", "error") == 1n : Nat}

# the next tag counts on from the newest one
law release_next_dotted:
  {REL.Release.next(["v1.1", "v1.2"]) == "v1.3" : String}

law release_next_letter:
  {REL.Release.next(["rev-A"]) == "rev-B" : String}

law release_next_plain:
  {REL.Release.next(["r7"]) == "r8" : String}

law release_next_first:
  {REL.Release.next(Nil{}) == "v1" : String}

law release_tag_valid:
  {REL.Release.valid("v1.2") == True{} : Bool}

# git refuses these as tag names
law release_tag_no_space:
  {REL.Release.valid("a b") == False{} : Bool}

law release_tag_no_dotdot:
  {REL.Release.valid("a..b") == False{} : Bool}

law release_tag_no_dash_lead:
  {REL.Release.valid("-x") == False{} : Bool}

law release_tag_no_lock_end:
  {REL.Release.valid("x.lock") == False{} : Bool}

law release_tag_not_empty:
  {REL.Release.valid("") == False{} : Bool}

# a release's outputs live in the repo's .backplane/releases/<tag>
law release_dir_in_repo:
  {REL.Release.dir("/r", "v1") == "/r/.backplane/releases/v1" : String}

law release_gh_command:
  {REL.Release.gh("v1", "n.md", ["f"]) == ["gh", "release", "create", "v1", "--title", "v1", "--notes-file", "n.md", "--verify-tag", "f"] : List<&2, String>}
# Design review (review.bend)

# no ops, no comments, and nothing to tell an agent
law review_fold_empty:
  {RV.Review.fold([]) == [] : List<&2, RV.Comment>}

law review_prompt_none:
  {RV.Review.prompt([]) == "" : String}

# an op on an id nobody added changes nothing
law review_unknown_ignored:
  {RV.Review.fold([RV.Resolve{"zz", "p"}, RV.Edit{"zz", "x"}, RV.Drop{"zz"}]) == [] : List<&2, RV.Comment>}

# add, then resolve: not open any more
law review_resolve:
  {RV.Review.count.open(RV.Review.fold([RV.Add{RV.Review.mk("a", "")}, RV.Resolve{"a", "p"}])) == 0n : Nat}

# resolve, then reopen: open again
law review_reopen:
  {RV.Review.count.open(RV.Review.fold([RV.Add{RV.Review.mk("a", "")}, RV.Resolve{"a", "p"}, RV.Reopen{"a", "p"}])) == 1n : Nat}

# a resolved comment is told to no agent
law review_prompt_resolved:
  {RV.Review.prompt(RV.Review.fold([RV.Add{RV.Review.mk("a", "")}, RV.Resolve{"a", "p"}])) == "" : String}

# drop removes the comment and the replies to it
law review_drop:
  {RV.Review.fold([RV.Add{RV.Review.mk("a", "")}, RV.Add{RV.Review.mk("b", "a")}, RV.Drop{"a"}]) == [] : List<&2, RV.Comment>}

# a reply joins its root and is no pin of its own
law review_reply_no_pin:
  {List.length(&2, RV.Pin, RV.Review.pins(RV.Review.fold([RV.Add{RV.Review.mk("a", "")}, RV.Add{RV.Review.mk("b", "a")}]), "f", "")) == 1n : Nat}

law review_reply_counted:
  {RV.Review.pin.count(RV.Rv.pin(RV.Review.fold([RV.Add{RV.Review.mk("a", "")}, RV.Add{RV.Review.mk("b", "a")}]), RV.Review.mk("a", ""))) == 1n : Nat}

# a reply to nothing is dropped
law review_orphan_ignored:
  {RV.Review.fold([RV.Add{RV.Review.mk("b", "a")}]) == [] : List<&2, RV.Comment>}

# an op (but an add, whose numbers print as floats) reads back the same
law review_roundtrip_resolve:
  {RV.Review.op.of("resolve", RV.Review.op.resolve("c1", "claude"), 3n) == Some{RV.Resolve{"c1", "claude"}} : Maybe<&2, RV.Op>}

law review_roundtrip_edit:
  {RV.Review.op.of("edit", RV.Review.op.edit("c1", "a b"), 3n) == Some{RV.Edit{"c1", "a b"}} : Maybe<&2, RV.Op>}

law review_roundtrip_drop:
  {RV.Review.op.of("drop", RV.Review.op.drop("c1"), 3n) == Some{RV.Drop{"c1"}} : Maybe<&2, RV.Op>}

# an Act of another tone is no review op
law review_other_tone:
  {RV.Review.of(M.Act{"x", "t", "tool", "add", "{}", 1n}) == None{} : Maybe<&2, RV.Op>}

# a pin moved only when its item is among the changed
law review_moved:
  {RV.Review.moved(RV.Review.mk("a", ""), ["i"]) == True{} : Bool}

law review_not_moved:
  {RV.Review.moved(RV.Review.mk("a", ""), ["j"]) == False{} : Bool}
# Slash commands in the composer (slash.bend)
law slash_typing_word:
  {SL.Slash.typing("/gi") == True{} : Bool}

law slash_typing_alone:
  {SL.Slash.typing("/") == True{} : Bool}

law slash_not_after_space:
  {SL.Slash.typing("/give >a") == False{} : Bool}

law slash_not_midline:
  {SL.Slash.typing("hi /gi") == False{} : Bool}

law slash_claude_prefix:
  {SL.Slash.names(SL.Slash.cmds(True{}, "c")) == ["/compact", "/context"] : List<&2, String>}

law slash_others_own_only:
  {SL.Slash.names(SL.Slash.cmds(False{}, "")) == ["/btw", "/give"] : List<&2, String>}

law slash_none_unknown:
  {SL.Slash.names(SL.Slash.cmds(True{}, "zz")) == [] : List<&2, String>}

# --- uberbot: uber_*, plane_* (thread t1791834693) ---

# a bot on another machine is never made the uberbot, and nothing is logged
law uber_promote_far_refused:
  for +st: M.State
  {UBR.Promo.log(UBR.Uber.promote(st, "p1~b9", "u1-a", 5n)) == Nil{} : List<&2, M.Change>}

# a demotion leaves no uberbot
law uber_demote_clears:
  {UBR.Uber.bot(M.State.replay([M.UberSet{"b1", "u1-a", 5n}, M.UberSet{"", "", 6n}], UBR.Uber.demo())) == "" : String}

# a promotion of a live bot of this machine makes it the uberbot
law uber_promote_sets:
  {UBR.Uber.bot(M.State.replay([M.UberSet{"b2", "u1-a", 5n}], UBR.Uber.demo())) == "b2" : String}

# the uberbot already promoted is not promoted again
law uber_promote_again_nothing:
  {UBR.Promo.log(UBR.Uber.promote(M.State.replay([M.UberSet{"b2", "u1-a", 5n}], UBR.Uber.demo()), "b2", "u2-b", 6n)) == Nil{} : List<&2, M.Change>}

# each thread and bot row of the status plane is there once
law plane_each_once:
  for +u: U.Ui
  for +ix: U.SIx
  for +now: Nat
  {ON.Once.distinct(ON.Once.keys(PLN.PRow, PLN.Plane.all.keyed(u, ix, now))) == True{} : Bool}

# and in the sections, no thread in two of them
law plane_sections_once:
  {ON.Once.distinct(PLN.Plane.threads.ids(PLN.Plane.of(PLN.Plane.demo(), 200000n))) == True{} : Bool}

# what waits on the person comes in urgency order
law plane_need_ranked:
  {PLN.Plane.ranked(PLN.Plane.need(PLN.Plane.of(PLN.Plane.demo(), 200000n))) == True{} : Bool}

# a click on a thread's row selects that thread
law plane_hit_opens:
  for +t: M.Thread
  for +at: Nat
  for +pt: String
  for +u: U.Ui
  for +ix: U.SIx
  for +st: M.State
  {PLN.Plane.hit(PLN.Plane.thread.t(t, at, pt, u, ix, st)) == "select " ++ M.Thread.id(t) : String}

# --- uberbot onboarding: onboard_uber_* ---

# the tour's last step, shown for a state; its choices as "action value;"
def OnbUber.step(st: M.State) -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.adopt.raw(U.Ui.init(1n), st), "@sync", "1"), "@tour", "9")

def OnbUber.acts(cs: List<&2, U.TChoice>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.TChoice{_, a, v} <> t:
      a ++ " " ++ v ++ ";" ++ OnbUber.acts(t)

# Kit said once, Pip and Zed three times (Pip older), none an uberbot
def OnbUber.bots() -> M.State:
  M.State.replay([M.ThreadCreated{"t1", "", "Kit", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Kit", "t1", 7n, "", 100n},
    M.ThreadCreated{"t2", "", "Pip", M.Local{}, "", "", "claude", "", 101n}, M.BotSet{"b2", "Pip", "t2", 8n, "", 101n},
    M.ThreadCreated{"t3", "", "Zed", M.Local{}, "", "", "claude", "", 102n}, M.BotSet{"b3", "Zed", "t3", 9n, "", 102n},
    M.MessagePosted{"t1", "m1", M.User{}, "a", 110n},
    M.MessagePosted{"t2", "m2", M.User{}, "a", 111n}, M.MessagePosted{"t2", "m3", M.User{}, "b", 112n}, M.MessagePosted{"t2", "m4", M.User{}, "c", 113n},
    M.MessagePosted{"t3", "m5", M.User{}, "a", 114n}, M.MessagePosted{"t3", "m6", M.User{}, "b", 115n}, M.MessagePosted{"t3", "m7", M.User{}, "c", 116n}], M.State.empty())

def OnbUber.first(cs: List<&2, U.TChoice>) -> String:
  OnbUber.acts(List.take(&2, U.TChoice, cs, 1n))

# with no bots here the only choice makes the uberbot
law onboard_uber_none_creates:
  {OnbUber.acts(U.Tour.choices(OnbUber.step(OnbUi.log()))) == "uber-new ;" : String}

# with bots and no uberbot the first choice promotes the one used most
# (ties: the oldest)
law onboard_uber_pick_most_used:
  {OnbUber.first(U.Tour.choices(OnbUber.step(OnbUber.bots()))) == "uber-promote b2;" : String}

# with an uberbot the step offers nothing to choose
law onboard_uber_have_quiet:
  {List.is_empty(&2, U.TChoice, U.Tour.choices(OnbUber.step(M.State.replay([M.UberSet{"b1", "u1-a", 200n}], OnbUber.bots())))) == True{} : Bool}

# an install that finished the tour before the uberbot step, with no
# uberbot: the step comes up alone, once its state has arrived ...
def OnbUber.old() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.SettingSet{"onboard.done", "1", 2n}], M.State.empty())

law onboard_uber_existing_shows:
  {U.Tour.at(OnbUi.synced(OnbUber.old())) == OBD.Ob.uber() : Nat}

# ... never from a cached state
law onboard_uber_existing_cached:
  {U.Tour.at(U.Ui.adopt.raw(U.Ui.init(1n), OnbUber.old())) == OBD.Ob.count() : Nat}

# once answered it does not come up by itself
law onboard_uber_answered_quiet:
  {U.Tour.at(OnbUi.synced(M.State.replay([M.SettingSet{"onboard.uber", "1", 3n}], OnbUber.old()))) == OBD.Ob.count() : Nat}

# nor with an uberbot
law onboard_uber_have_no_ask:
  {U.Tour.at(OnbUi.synced(M.State.replay([M.UberSet{"b1", "u1-a", 200n}], M.State.replay([M.SettingSet{"onboard.done", "1", 2n}], OnbUber.bots())))) == OBD.Ob.count() : Nat}

# Skip answers it, and a step asked alone has no Back
law onboard_uber_skip_answers:
  {U.Tour.answered(U.Ui.state(OnbUi.after(U.Ui.act(OnbUi.synced(OnbUber.old()), "ob-skip", "")))) == True{} : Bool}

law onboard_uber_single_no_back:
  {U.Tour.back(OnbUi.synced(OnbUber.old())) == False{} : Bool}

# the phone's status plane lists each row once, whichever hubs reported it
law hubs_plane_once:
  for +rs: List<&2, J.Json>
  {ON.Once.distinct(ON.Once.keys(J.Json, Hb.Hubs.pl.keyed(rs))) == True{} : Bool}
# --- uberbot sync: uber_merge_*, uber_*_term, uber_fire_*, uber_slot_once, uber_derived_no_ops (SPEC-2) ---

# two machines' ops on one key, in either order, leave the same registers
law uber_merge_comm:
  {UBS.Ub.demo.fold([UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.mem("color", "fact", "blue", "")), UBS.Ub.demo.op("mB", 1n, 1001n, UBO.Ub.b.mem("color", "fact", "red", ""))]) == UBS.Ub.demo.fold([UBS.Ub.demo.op("mB", 1n, 1001n, UBO.Ub.b.mem("color", "fact", "red", "")), UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.mem("color", "fact", "blue", ""))]) : String}

# an op taken twice is taken once
law uber_merge_idem:
  {UBS.Ub.demo.fold([UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.mem("color", "fact", "blue", "")), UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.mem("color", "fact", "blue", ""))]) == UBS.Ub.demo.fold([UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.mem("color", "fact", "blue", ""))]) : String}

# a forget is a tombstone in the memory's register: a later one beats the save it follows, in either order
law uber_forget_wins_older:
  {M.Setting.get(M.State.replay([UBS.Ub.demo.op("mB", 1n, 1001n, UBO.Ub.b.forget("color")), UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.mem("color", "fact", "blue", ""))], UBS.Ub.demo.a()), "uber.r.u1-x.mem:color", "-") == UBO.Ub.b.forget("color") : String}

# and an older forget does not beat a later save
law uber_forget_loses_newer:
  {M.Setting.get(M.State.replay([UBS.Ub.demo.op("mA", 1n, 1002n, UBO.Ub.b.mem("color", "fact", "blue", "")), UBS.Ub.demo.op("mB", 1n, 1001n, UBO.Ub.b.forget("color"))], UBS.Ub.demo.a()), "uber.r.u1-x.mem:color", "-") == UBO.Ub.b.mem("color", "fact", "blue", "") : String}

# two machines taking the same term leave one holder, the same in either order
law uber_one_holder_per_term:
  {UBS.Ub.holder(M.State.replay([UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.lease("mA", 2n)), UBS.Ub.demo.op("mB", 1n, 1001n, UBO.Ub.b.lease("mB", 2n))], UBS.Ub.demo.a())) ++ UBS.Ub.holder(M.State.replay([UBS.Ub.demo.op("mB", 1n, 1001n, UBO.Ub.b.lease("mB", 2n)), UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.lease("mA", 2n))], UBS.Ub.demo.a())) == "mBmB" : String}

# a higher term holds against a later stamp
law uber_higher_term_wins:
  {UBS.Ub.holder(M.State.replay([UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.lease("mA", 3n)), UBS.Ub.demo.op("mB", 1n, 2000n, UBO.Ub.b.lease("mB", 2n))], UBS.Ub.demo.a())) ++ UBS.Ub.holder(M.State.replay([UBS.Ub.demo.op("mB", 1n, 2000n, UBO.Ub.b.lease("mB", 2n)), UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.lease("mA", 3n))], UBS.Ub.demo.a())) == "mAmA" : String}

# an unpinned routine runs where the conversation is held and nowhere else
law uber_fire_holder_only:
  for +mine: Bool
  for +holder: Bool
  {UBS.Ub.fires.k(False{}, mine, holder, True{}) == holder : Bool}

# a pinned routine runs on its machine, whoever holds the conversation
law uber_pinned_stays:
  for +mine: Bool
  for +holder: Bool
  {UBS.Ub.fires.k(True{}, mine, holder, True{}) == mine : Bool}

# a slot already run never runs again
law uber_slot_once:
  for +pinned: Bool
  for +mine: Bool
  for +holder: Bool
  {UBS.Ub.fires.k(pinned, mine, holder, False{}) == False{} : Bool}

# the same on a machine's own state: A holds and fires; B does not; neither fires a slot that ran
law uber_fire_state:
  {UBS.Ub.fires(M.State.replay([UBS.Ub.demo.op("mA", 1n, 900n, UBO.Ub.b.lease("mA", 1n))], UBS.Ub.demo.a()), "mA:r1", 5000n) == True{} : Bool}

law uber_fire_state_other:
  {UBS.Ub.fires(M.State.replay([UBS.Ub.demo.op("mA", 1n, 900n, UBO.Ub.b.lease("mA", 1n))], UBS.Ub.demo.b()), "mA:r1", 5000n) == False{} : Bool}

law uber_slot_once_state:
  {UBS.Ub.fires(M.State.replay([UBS.Ub.demo.op("mA", 1n, 900n, UBO.Ub.b.lease("mA", 1n)), UBS.Ub.demo.op("mA", 2n, 5001n, UBO.Ub.b.fired("mA:r1", 5000n))], UBS.Ub.demo.a()), "mA:r1", 5000n) == False{} : Bool}

law uber_pinned_state:
  {UBS.Ub.fires(M.State.replay([UBS.Ub.demo.op("mA", 1n, 900n, UBO.Ub.b.lease("mA", 1n)), UBS.Ub.demo.op("mA", 2n, 901n, UBO.Ub.b.routine("mA:r1", "R", "daily 09:00", "p", True{}, "mB"))], UBS.Ub.demo.b()), "mA:r1", 5000n) == True{} : Bool}

# for any kind of op, what is derived is not an op
law uber_derive_n_no_ops:
  for n: Nat
  for +b: String
  for +me: String
  for +o: J.Json
  for +at: Nat
  {UBS.Ub.noops(UBS.Ub.derive.n(n, b, me, o, at)) == True{} : Bool}

# what an op does to the replica is never an op
law uber_derived_no_ops:
  for +win: Bool
  for +st: M.State
  for +o: J.Json
  for +at: Nat
  {UBS.Ub.noops(UBS.Ub.derive.k(win, st, o, at)) == True{} : Bool}

# an op a later one superseded is kept as a nop, and the register holds the later one
law uber_superseded_nop:
  {M.Setting.get(M.State.replay([UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.mem("color", "fact", "blue", "")), UBS.Ub.demo.op("mB", 1n, 1001n, UBO.Ub.b.mem("color", "fact", "red", ""))], UBS.Ub.demo.a()), "uber.op.u1-x.mA.1", "-") == UBO.Ub.line.nop("u1-x", "mA", 1n, UBO.Ub.stamp.make(1000n, 0n, "mA")) : String}

law uber_superseded_keeps:
  {M.Setting.get(M.State.replay([UBS.Ub.demo.op("mA", 1n, 1000n, UBO.Ub.b.mem("color", "fact", "blue", "")), UBS.Ub.demo.op("mB", 1n, 1001n, UBO.Ub.b.mem("color", "fact", "red", ""))], UBS.Ub.demo.a()), "uber.r.u1-x.mem:color", "-") == UBO.Ub.b.mem("color", "fact", "red", "") : String}

# an uberbot promoted before ops existed is seeded once: its first ops are logged, and none again after they are folded
law uber_seed_once:
  {UBS.Ub.seed.n(UBS.Ub.demo.a(), 3000n) == "3,0" : String}

# --- uberbot inc 3: plane_tally_*, plane_text_*, uber_home_* ---

# a thread's rows from the index (one pass over the settings) are the rows its own lookups find
law side_work_ix_eq:
  {U.Side.phases.ix(U.Ui.threads(U.Side.demo.work()), U.Side.ix(U.Side.demo.work()), U.Side.demo.work()) == U.Side.phases(U.Ui.threads(U.Side.demo.work()), U.Side.demo.work()) : List<&2, M.Phase>}

# the count the phones' list makes without building rows is the plane's count
law plane_tally_counts:
  {PLN.Plane.tally.same(PLN.Plane.demo(), 200000n) == True{} : Bool}

# also when threads are need or run by a pending tool, a queued message, background rows or a child task
law plane_tally_counts_rich:
  {PLN.Plane.tally.same(PLN.Plane.demo.rich(), 200000n) == True{} : Bool}

# plane_read: every need row comes before every run row in the text
law plane_text_need_first:
  {PLN.Plane.text.need_first(PLN.Plane.of(PLN.Plane.demo(), 200000n), 200000n) == True{} : Bool}

law plane_text_need_first_rich:
  {PLN.Plane.text.need_first(PLN.Plane.of(PLN.Plane.demo.rich(), 200000n), 200000n) == True{} : Bool}

# plane_read names no machine: this hub reads its own; one it names (and none is linked) is never sent
law fmc_plan_plane_read_here:
  {FMC.Fm.plan.name(FMC.Fm.plan.o(Fx.call("plane_read", J.Json.obj(Nil{})), M.State.empty(), Nil{}, "t0")) == "here" : String}

law fmc_plan_plane_read_unlinked_never_sent:
  {FMC.Fm.plan.name(FMC.Fm.plan.o(Fx.call("plane_read", J.Json.obj([J.KV{"machine", J.Str{"beta"}}])), M.State.empty(), Nil{}, "t0")) == "answer" : String}

# an owner takes plane_read from its machines
law fmc_plane_read_allowed:
  {FMC.Fm.allowed(Fx.ask("plane_read")) == True{} : Bool}

# "Make this the home": this machine takes the lease one term on
law uber_home_takes_term:
  {UBS.Ub.home.after(UBS.Ub.demo.away(), 2000n) == "mA,3" : String}

# and does nothing where it holds it already, where no lease was ever taken, or with no uberbot
law uber_home_holder_nothing:
  {Nat.add(Nat.add(UBS.Ub.home.count(UBS.Ub.demo.a(), 2000n), UBS.Ub.home.count(M.State.replay([UBS.Ub.demo.op("mA", 1n, 1001n, UBO.Ub.b.lease("mA", 2n))], UBS.Ub.demo.a()), 2000n)), UBS.Ub.home.count(M.State.empty(), 2000n)) == 0n : Nat}

# the Status strip offers "Make this the home" where the uberbot is here and its conversation is not
law plane_home_button:
  {PLN.Plane.home.button(PLN.Plane.home(U.Ui.far.st(U.Ui.init(2000n), UBS.Ub.demo.away()))) == True{} : Bool}

# and not where this machine holds it, or has no uberbot
law plane_home_button_held:
  {Bool.or(PLN.Plane.home.button(PLN.Plane.home(U.Ui.far.st(U.Ui.init(2000n), UBS.Ub.demo.a()))), PLN.Plane.home.button(PLN.Plane.home(U.Ui.far.st(U.Ui.init(2000n), M.State.empty())))) == False{} : Bool}

# the phones' list plane has the rows of the plane that need the person or run, in the same order
law plane_list_same:
  {PLN.Plane.list.same(PLN.Plane.demo(), 200000n) == True{} : Bool}

law plane_list_same_rich:
  {PLN.Plane.list.same(PLN.Plane.demo.rich(), 200000n) == True{} : Bool}

# Launched threads report back (hub.bend's Launch.tell, Lr.*)
# ----------------------------------------------------------

# a launched thread's turn end reaches its launcher once
law launch_reports_turn_end:
  {H.Lr.demo.tell(H.Lr.demo(True{}), H.Lr.demo.end()) == 1n : Nat}

# an ask it opens reaches the launcher at once, before any turn end
law launch_ask_at_once:
  {H.Lr.demo.tell(H.Lr.demo(True{}), [M.AskOpened{"q1", "t1", "question", "AskUserQuestion", "which board?", "", 9n}]) == 1n : Nat}

# a failure too
law launch_fail_told:
  {H.Lr.demo.tell(H.Lr.demo(True{}), [M.TurnChanged{"t1", M.Failed{}, 9n}]) == 1n : Nat}

# a step that ends no turn and opens no ask tells nothing (one report per end)
law launch_once_per_end:
  {H.Lr.demo.tell(H.Lr.demo(True{}), [M.MessagePosted{"t1", "a2", M.Assistant{}, "more", 9n}]) == 0n : Nat}

# reports turned off: the step is left as it was
law launch_muted_quiet:
  {H.Launch.tell(H.Lr.demo(False{}), 9n, "t1", H.Step{H.Lr.demo.end(), []}) == H.Step{H.Lr.demo.end(), []} : H.Step}

# a turn that left work out of hand reports nothing yet (the one that
# finishes the work does)
law launch_waits_for_work:
  {H.Lr.demo.tell(H.Lr.demo.k(True{}, False{}, False{}, [M.WorkSet{"t1", "bg", "a\tx", 2n}]), H.Lr.demo.end()) == 0n : Nat}

# a thread that already wrote to its launcher this turn is not echoed
law launch_said_quiet:
  {H.Lr.demo.tell(H.Lr.demo.k(True{}, False{}, False{}, [M.MessagePosted{"t0", M.Link.id("t1", "m5"), M.User{}, "here it is", 2n}]), H.Lr.demo.end()) == 0n : Nat}

# a launcher stopping its own thread is not told it stopped it
law launch_own_stop_quiet:
  {H.Lr.count(H.Queue.changes(H.Retire.stop.f(H.Lr.demo(True{}), 9n, "t1", "t0", [])), "t0") == 0n : Nat}

# the person stopping it is told
law launch_stop_told:
  {H.Lr.count(H.Queue.changes(H.Retire.stop(H.Lr.demo(True{}), 9n, "t1", [])), "t0") == 1n : Nat}

# reports to a busy launcher reach it at once, one per end, in order
law launch_delivered_in_order:
  {H.Lr.demo.twice() == 2n : Nat}

# back and forth with the launcher Lr.loops() times without the person:
# the reports stop
law launch_loop_bounded:
  {H.Lr.demo.loop.on() == False{} : Bool}

# a report from another machine posts once however often it is sent
law launch_far_once:
  {FMC.Fm.report.twice() == 1n : Nat}

# the outbox keeps its order, and a line taken leaves it alone
law launch_far_order:
  {String.join(H.Lr.far.keep(["a\t1", "b\t2", "c\t3"], "b"), "|") == "a\t1|c\t3" : String}

# a launcher lists each thread once
law launch_list_once:
  {M.Launch.add(M.Launch.add("a", "b"), "b") == "a\nb" : String}

# a launched thread links back to its launcher
law launched_up_launcher:
  {U.Tasks.up(H.Lr.demo(True{}), "t1") == "t0" : String}

# and its launcher lists it while it runs
law subs_launched_live_kept:
  {List.length(&2, SB.SubRow, SB.Subs.launched(H.Lr.demo(True{}), "t0")) == 1n : Nat}

# a bot turn woken by a report is the launched thread's news, already
# alerted: it is chatter
law notice_report_quiet:
  {NO.Turn.msg(NO.Turn{False{}, "[Backplane] Launched thread \"work\" (t1) completed.", 0n}) == True{} : Bool}

# a thread a bot launched may thread_send the bot's thread, which is in no
# project (and the bot may send to it)
law launch_send_launcher:
  {H.Lr.may(M.State.replay([M.ThreadLaunched{"t1", "tb", True{}, 1n}], M.State.empty()), "t1", "tb") == True{} : Bool}
# Google Antigravity (core/agy.bend)
# ----------------------------------

law interrupt_antigravity_stops:
  for +th: String
  for r: String
  {H.Stop.outs("antigravity", th, r) == [H.AgentStop{th}] : List<&2, H.Out>}

# an Antigravity thread's turn is one run of Google's agent (over ACP)
law antigravity_run_turn:
  for +th: String
  for +cwd: String
  for +m: String
  for +s: String
  for +r: M.RuntimeMode
  for i: M.InteractionMode
  for +t: String
  for u: String
  {H.Hub.run.k(4n, th, cwd, "antigravity", m, s, r, i, t, u) == [H.AgyTurn{th, cwd, m, s, r, t}] : List<&2, H.Out>}

# its permission modes: supervised asks, full access allows everything
law agy_mode_supervised_asks:
  {AGY.Agy.mode.k(0n) == "default" : String}

law agy_mode_full_allows:
  {AGY.Agy.mode.k(3n) == "yolo" : String}

# an API key is the default sign-in, and a Google account carries Google's warning
law agy_default_api_key:
  {AGY.Agy.method("") == "gemini-api-key" : String}

law agy_google_warns:
  {AGY.Agy.note.k(3n) == AGY.Agy.warn() : String}

# no address is ever fetched for a sign-in that did not ask
law agy_callback_needs_signin:
  for b: String
  {AGY.Agy.callback.ok("", b) == False{} : Bool}

# ios-snappy: nav
# The bot row's nav, qualified by its hub, is the same qualified thread
# sel the opened screen carries. This example has an arbitrary t-prefixed
# thread id; the general law below covers all untagged nonempty threads.
# Room and reported bot keys stay local.
law mnav_bot_sel:
  for +hub: String
  for +id: String
  for +tail: String
  {Hb.Hubs.nav.bot(hub, BU.BRow{id, "", "t" ++ tail, 0n, "idle", "", False{}, False{}, "", ""}) == Hb.Hubs.id(hub, "t" ++ tail) : String}

law mnav_room_local:
  for +hub: String
  for +id: String
  {Hb.Hubs.nav.qual(hub, "@room:" ++ id) == "@room:" ++ id : String}

law mnav_remote_local:
  for +hub: String
  for +id: String
  {Hb.Hubs.nav.qual(hub, "@remote:" ++ id) == "@remote:" ++ id : String}

# Every valid bot thread qualifies exactly like the opened screen's sel.
# The reserved room/remote prefixes name other destination kinds.
law mnav_bot_thread:
  for +hub: String
  for +id: String
  for +thread: String
  for some: {String.is_empty(thread) == False{} : Bool}
  for own: {Hb.Hubs.nav.tag(thread) == False{} : Bool}
  {Hb.Hubs.nav.bot(hub, BU.BRow{id, "", thread, 0n, "idle", "", False{}, False{}, "", ""}) == Hb.Hubs.id(hub, thread) : String}
# ---- notif-policy ----

# a bot turn woken by a room message that posts, a routine's turn, a
# launched thread's end that reports to its launcher, a delegated task's
# note, an intermediate end with background work out and a stop write no
# record that alerts
law policy_no_alert_unless:
  {NO.Policy.demo.quiet() == 0n : Nat}

# the person's message, a completed ready turn: one done; the turn a report wakes afterwards alerts nothing
law policy_settle_once:
  {NO.Policy.demo.once() == "done -" : String}

# a turn that ends while a reporting launched thread still works alerts nothing; the turn its report wakes alerts once
law policy_settle_after_launch:
  {NO.Policy.demo.launch() == "- done" : String}

# a failure after the person's message alerts; a failed routine turn after it settled does not
law policy_fail_person_only:
  {NO.Policy.demo.fail.p() == "fail -" : String}

# an ask opened for the person records an ask (needs on), or no kind (needs off)
law policy_ask_alerts:
  {NO.Policy.demo.ask.k(NO.Policy.demo.s(NO.Policy.demo.ask())) == "ask _" : String}

# and the step tells the server once; with needs off it tells no one
law policy_ask_alerted_out:
  {H.Alerts.demo.asks(False{}) == 1n : Nat}

law policy_ask_off_silent:
  {H.Alerts.demo.asks(True{}) == 0n : Nat}

# a merge found records merged and tells the server
law policy_merge_alerts:
  {H.Alerts.demo.merge() == "merged 1" : String}

# a turn end that settles records done and tells the server
law policy_end_alerted:
  {H.Alerts.demo.end() == "done 1" : String}

# with settled off the completed case records no kind but moves the memory; on again, the old episode does not alert
law policy_setting_off:
  {NO.Policy.demo.off() == "_ m1 -" : String}

# with notify.all on the older rule alerts a room exchange's turn again
law policy_all_legacy:
  {NO.Policy.demo.all() == "done room-r1" : String}

# a message with a link id or a routine's text is not the person's message
law policy_link_not_person:
  {NO.Policy.demo.link() == False{} : Bool}

# the person's own text, or typed from another machine, is
law policy_person_direct:
  {Bool.and(NO.Policy.direct("m1", "do it"), NO.Policy.direct("m2", "[message from person@desk]\nhi")) == True{} : Bool}

# notify_person refuses an unknown reason and a reminder outside a routine
law notify_refuses_chatter:
  {BHS.Notify.demo.chatter() == "refused refused refused" : String}

# a reminder or digest goes through only in a routine's turn
law notify_reminder_routine_only:
  {BHS.Notify.demo.routine() == "note note refused" : String}

# needs_you alerts once in a turn
law notify_needs_you_once:
  {BHS.Notify.demo.once() == "note refused" : String}

# done tells the person once and the turn's end then alerts nothing
law notify_done_settles:
  {BHS.Notify.demo.done() == "note refused -" : String}

# with the setting off the call succeeds and records no kind
law notify_setting_off:
  {BHS.Notify.demo.off() == "_" : String}

# a redraw or a resize is not the person using the window: an open,
# focused window stops holding phone alerts once they stop using it
law desk_redraw_not_input:
  {N.Nui.input([W.WExpose{}, W.WSize{800, 600}, W.WDragLeave{}]) == False{} : Bool}

law desk_key_input:
  for c: U32
  for t: U32
  for m: U32
  for d: Bool
  for k: U32
  for rest: List<&2, W.WinEv>
  {N.Nui.input(W.WKey{c, t, m, d, k} <> rest) == True{} : Bool}

# a phone alerts when a thread's record changed and has a kind
law policy_phone_raises:
  {List.length(&2, J.Json, Nt.Note.ralerts(U.Ui.threads(Nt.Note.demo.s(Nt.Note.demo.rec("done"))), [Nt.RSeen{"t1", ""}], Nt.Note.demo.s(Nt.Note.demo.rec("done")), "")) == 1n : Nat}

# the same record again alerts no one
law policy_phone_same_none:
  {List.length(&2, J.Json, Nt.Note.ralerts(U.Ui.threads(Nt.Note.demo.s(Nt.Note.demo.rec("done"))), [Nt.RSeen{"t1", Nt.Note.demo.rec("done")}], Nt.Note.demo.s(Nt.Note.demo.rec("done")), "")) == 0n : Nat}

# a record with no kind alerts no one
law policy_phone_empty_none:
  {List.length(&2, J.Json, Nt.Note.ralerts(U.Ui.threads(Nt.Note.demo.s(Nt.Note.demo.rec(""))), [Nt.RSeen{"t1", ""}], Nt.Note.demo.s(Nt.Note.demo.rec("")), "")) == 0n : Nat}

# a thread the frame did not touch alerts no one
law policy_phone_unseen_none:
  {List.length(&2, J.Json, Nt.Note.ralerts(U.Ui.threads(Nt.Note.demo.s(Nt.Note.demo.rec("done"))), [], Nt.Note.demo.s(Nt.Note.demo.rec("done")), "")) == 0n : Nat}

# a desk watching the thread holds a done
law policy_phone_watched_held:
  {List.length(&2, J.Json, Nt.Note.ralerts(U.Ui.threads(Nt.Note.demo.s(Nt.Note.demo.rec("done"))), [Nt.RSeen{"t1", ""}], Nt.Note.demo.s(Nt.Note.demo.rec("done")), "t1")) == 0n : Nat}

# but not a note
law policy_phone_note_not_held:
  {List.length(&2, J.Json, Nt.Note.ralerts(U.Ui.threads(Nt.Note.demo.s(Nt.Note.demo.rec("note"))), [Nt.RSeen{"t1", ""}], Nt.Note.demo.s(Nt.Note.demo.rec("note")), "t1")) == 1n : Nat}

# a frame that carries only a record names its thread
law policy_phone_ids_record:
  {Hb.One.pol.ids([J.Json.obj([J.KV{"key", J.Str{"alert.t1"}}, J.KV{"value", J.Str{"x"}}])], []) == ["t1"] : List<&2, String>}

# through a hub's frame: the record raises one alert
law policy_phone_frame_raises:
  {List.length(&2, J.Json, Hb.Did.alerts(Hb.One.pol(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", False{}, False{}, [C.Change.encode(M.ThreadCreated{"t1", "p1", "hi", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.TurnChanged{"t1", M.Running{}, 2n})])),
      U.Seq.demo("changes", [C.Change.encode(M.TurnChanged{"t1", M.Completed{}, 3n}), C.Change.encode(M.SettingSet{"alert.t1", Nt.Note.demo.rec("done"), 3n})])))) == 1n : Nat}

# and a turn's end alone raises none
law policy_phone_turn_end_quiet:
  {Hb.Did.alerts(Hb.One.pol(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", False{}, False{}, [C.Change.encode(M.ThreadCreated{"t1", "p1", "hi", M.Local{}, "", "", "claude", "", 1n}), C.Change.encode(M.TurnChanged{"t1", M.Running{}, 2n})])),
      U.Seq.demo("changes", [C.Change.encode(M.TurnChanged{"t1", M.Completed{}, 3n})]))) == [] : List<&2, J.Json>}

# a log still arriving is history: no alert
law policy_phone_boot_quiet:
  {Hb.Did.alerts(Hb.One.pol(U.Ui.recv(U.Ui.init(0n), U.Boot.demo("log", True{}, True{}, [C.Change.encode(M.ThreadCreated{"t1", "p1", "hi", M.Local{}, "", "", "claude", "", 1n})])),
      U.Boot.demo("changes", False{}, True{}, [C.Change.encode(M.SettingSet{"alert.t1", Nt.Note.demo.rec("done"), 3n})]))) == [] : List<&2, J.Json>}

# a reminder that came due records a reminder (the setting on, the
# default), or no kind (off)
law policy_reminder_alerts:
  {NO.Policy.demo.reminder() == "reminder _" : String}

# The notification Settings show the policy defaults even before any setting is saved.
law notify_rows_defaults:
  {U.Notify.rows(M.State.empty()) == [
    U.NotifyRow{"Needs me", "Approvals, questions and anything an agent is blocked on until you act.", "on", "notify.needs"},
    U.NotifyRow{"Settled", "Work you gave a thread is finished, failed, or its branch merged. Once per piece of work.", "on", "notify.settled"},
    U.NotifyRow{"Reminders", "Reminders and digests you scheduled.", "on", "notify.reminders"},
    U.NotifyRow{"Everything", "Also every turn end and room exchange, as before.", "off", "notify.all"}] : List<&2, U.NotifyRow>}
# The compact island counts roots but names only the latest direct send.
# The earlier island laws retain their legacy helpers (including waiting
# on work and listing three); production hub/phone content uses compact.
law island_compact_one_line:
  {List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(NO.Island.compact.demo.json(NO.Island.compact.demo.five()), "lines"))) == 1n : Nat}

law island_no_children:
  {NO.Island.compact.count(NO.Island.compact.demo.of(NO.Island.compact.demo.children())) == 0n : Nat}

law island_line_last_sent:
  {NO.Island.compact.id(NO.Island.compact.demo.of(NO.Island.compact.demo.last())) == "t2" : String}

# ---- end notif-policy ----
# harness-coord: CLI path
# the merged PATH begins with the hub's own, unchanged
law clipath_keeps_current:
  {CPA.CliPath.merge("/usr/bin:/bin", "/home/h/.nvm/versions/node/v24.13.1/bin:/bin", ["/usr/local/bin"]) == "/usr/bin:/bin:/home/h/.nvm/versions/node/v24.13.1/bin:/usr/local/bin" : String}

# entries the login PATH and the extras repeat appear once
law clipath_no_dupes:
  {CPA.CliPath.merge("/a", "/b:/a:/b", ["/c", "/b", "/c"]) == "/a:/b:/c" : String}

# merging again changes nothing
law clipath_idempotent:
  {CPA.CliPath.merge(CPA.CliPath.merge("/usr/bin", "/l:/usr/bin", ["/x"]), "/l:/usr/bin", ["/x"]) == CPA.CliPath.merge("/usr/bin", "/l:/usr/bin", ["/x"]) : String}

# nvm's newest node comes first, by version and not by text
law clipath_nvm_newest_first:
  {CPA.CliPath.candidates("/h", ["/h/.nvm/versions/node/v9.11.2/bin", "/h/.nvm/versions/node/v24.13.1/bin", "/h/.nvm/versions/node/v18.0.0/bin"]) == ["/h/.nvm/versions/node/v24.13.1/bin", "/h/.nvm/versions/node/v18.0.0/bin", "/h/.nvm/versions/node/v9.11.2/bin"] : List<&2, String>}

# harness-coord: wakes
law wake_once_per_event:
  {H.Wake.demo.twice() == 0n : Nat}

law wake_once_drops:
  {H.Wake.demo.dropped() == 0n : Nat}

law wake_two_events_one_message:
  {H.Wake.demo.count(H.Wake.demo(False{}, ["end", "done"])) == 1n : Nat}

law wake_not_self:
  {WAK.Wake.error("tb", "thread", "tb", ["end"]) == "a subscriber cannot wake itself" : String}

law wake_bot_thread_delivered:
  {H.Wake.demo.count(H.Wake.demo(True{}, ["end"])) == 1n : Nat}

law wake_pr_merged:
  {H.Lr.count(H.Queue.changes(H.Wake.pr(H.Wake.demo.pr(), 9n, "owner/repo#7", H.Wake.demo.merged())), "tb") == 1n : Nat}

law wake_pr_repeat_quiet:
  {H.Wake.demo.pr.twice() == 0n : Nat}

law codec_wake_roundtrip:
  {C.Change.decode(C.Change.encode(M.WakeSet{"w1", "tb", "thread", "t1", ["end", "done"], True{}, "note", 4n})) == Some{M.WakeSet{"w1", "tb", "thread", "t1", ["end", "done"], True{}, "note", 4n}} : Maybe<&2, M.Change>}

law codec_wake_dropped_roundtrip:
  {C.Change.decode(C.Change.encode(M.WakeDropped{"w1", 9n})) == Some{M.WakeDropped{"w1", 9n}} : Maybe<&2, M.Change>}

law wake_message_one_paragraph:
  {WAK.Wake.flat("one\ntwo\r\tthree", "") == "one two  three" : String}

law wake_event_independent_of_step:
  {H.Wake.demo.partial() == 0n : Nat}
# ---- told-rules-board ----
# "Already told" history (told.bend), standing rules (rules.bend) and the
# task board (tboard.bend); all three are settings (model.bend).

# an empty value removes a ledger key, so dropped records leave nothing behind
law told_setting_empty_removes:
  {List.length(&2, String, M.Setting.keys(M.State.replay([M.SettingSet{"told.x", "v", 1n}, M.SettingSet{"told.x", "", 2n}], M.State.empty()))) == 0n : Nat}

# a mark stores its key normalized
law told_mark_key_norm:
  {TLD.Told.key(TLD.Told.demo.first(TLD.Told.demo.marked())) == "lunch:-kane-" : String}

# a mark is found by its key
law told_check_finds_marked:
  {List.length(&2, TLD.Told, TLD.Told.check.d(10n, TLD.Told.demo.marked(), 1010n, "Lunch: Kane!!", "", 0n)) == 1n : Nat}

# and by the words of the same thing said differently
law told_check_text_overlap:
  {List.length(&2, TLD.Told, TLD.Told.check.d(10n, TLD.Told.demo.marked(), 1010n, "", "lunch with Kane in ATL on 10/28", 0n)) == 1n : Nat}

# a record 31 days old is pruned, one 29 days old is kept
law told_prune_old:
  {Nat.add(Nat.mul(List.length(&2, M.Change, TLD.Told.prune.at(30n, 2000n, TLD.Told.demo.marked(), 1032n)), 10n), List.length(&2, M.Change, TLD.Told.prune.at(30n, 2000n, TLD.Told.demo.marked(), 1029n))) == 10n : Nat}

# a bot woken by a room message that posted nothing tells the person nothing, so nothing is marked
law told_quiet_end_marks_nothing:
  {List.length(&2, M.Change, TLD.Told.alert(TLD.Told.demo.chatter(), 2000n, "tb", "completed")) == 0n : Nat}

# with no rule, no block
law rules_block_empty_when_none:
  {RLS.Rules.block(RLS.Rules.demo.base(), "t-miso") == "" : String}

# a rule for a bot is in that bot's thread only, a project's in that project's threads only
law rules_block_scoped:
  {Bool.and(Bool.and(String.contains(RLS.Rules.block(RLS.Rules.demo.scoped(), "t-miso"), "Miso only"), String.contains(RLS.Rules.block(RLS.Rules.demo.scoped(), "t-pb"), "Power only")),
    Bool.not(Bool.or(Bool.or(String.contains(RLS.Rules.block(RLS.Rules.demo.scoped(), "t-tofu"), "Miso only"), String.contains(RLS.Rules.block(RLS.Rules.demo.scoped(), "t-other"), "Power only")),
      Bool.or(String.contains(RLS.Rules.block(RLS.Rules.demo.scoped(), "t-miso"), "Power only"), String.contains(RLS.Rules.block(RLS.Rules.demo.scoped(), "t-pb"), "Miso only"))))) == True{} : Bool}

# an enforced rule refuses the call
law rules_refuse_enforced:
  {String.contains(RLS.Rules.gate(RLS.Rules.demo.enforced(), "t-miso", RLS.Rules.demo.call("bot_send")), "Refused by a standing rule") == True{} : Bool}

# the person's word in their latest message unlocks it; a delivered message never does
law rules_word_unlocks:
  {Bool.and(String.is_empty(RLS.Rules.gate(RLS.Rules.demo.say(RLS.Rules.demo.enforced(), "t-miso", "sudo buy it"), "t-miso", RLS.Rules.demo.call("bot_send"))),
    Bool.not(String.is_empty(RLS.Rules.gate(RLS.Rules.demo.say(RLS.Rules.demo.enforced(), "t-miso", "[message from tofu] sudo"), "t-miso", RLS.Rules.demo.call("bot_send"))))) == True{} : Bool}

# a rule on preview_click matches the MCP name
law rules_mcp_prefix:
  {RLS.Rules.tool.match("preview_click", "mcp__backplane__preview_click") == True{} : Bool}

# a rule that is off is neither in the block nor enforced
law rules_off_ignored:
  {Bool.and(String.is_empty(RLS.Rules.block(RLS.Rules.demo.off(), "t-miso")), String.is_empty(RLS.Rules.gate(RLS.Rules.demo.off(), "t-miso", RLS.Rules.demo.call("bot_send")))) == True{} : Bool}

# open items sort before done ones
law tb_order_open_first:
  {TBD.Tb.id(Maybe.default(&2, TBD.TbItem, List.head(&2, TBD.TbItem, TBD.Tb.order([TBD.TbItem{"done", "a", "done", "", "", "", "", Nil{}, "", 1n, 1n, ""}, TBD.TbItem{"todo", "b", "todo", "", "", "", "", Nil{}, "", 2n, 2n, ""}])), TBD.Tb.blank("?", 0n))) == "todo" : String}

# checkbox items become items, headings groups, other lines are skipped
law tb_import_checkbox:
  {String.join([TBD.Tb.demo.summary(TBD.Tb.import(M.State.empty(), 1000n, "person", "## Errands\n- [ ] Peach Pass\n- [x] floss\n- plain", "", "")),
    TBD.Tb.demo.lines(TBD.Tb.demo.import("## Errands\n- [ ] Peach Pass\n- [x] floss\n- plain", M.State.empty()))], " | ") == "2 0 - plain | todo Errands Peach Pass;done Errands floss" : String}

# the same list imported twice adds nothing the second time
law tb_import_idempotent:
  {TBD.Tb.demo.summary(TBD.Tb.import(TBD.Tb.demo.import("## Errands\n- [ ] Peach Pass\n- [x] floss", M.State.empty()), 2000n, "person", "## Errands\n- [ ] Peach Pass\n- [x] floss", "", "")) == "0 2 " : String}

# an update that gives only the status keeps the rest
law tb_update_merges:
  {TBD.Tb.demo.merged(TBD.Tb.demo.updated()) == "Fix it|a note|doing" : String}

# a due date is YYYY-MM-DD
law tb_due_shape:
  {Bool.and(TBD.Tb.date("2026-10-28"), Bool.not(TBD.Tb.date("10/28"))) == True{} : Bool}
# a report from a launched thread to a bot is bot-to-bot: nothing is told
law told_report_quiet:
  {List.length(&2, M.Change, TLD.Told.alert(TLD.Told.demo.report(), 2000n, "tb", "completed")) == 0n : Nat}

# a launch report never unlocks a rule, even when it says the word
law rules_report_never_unlocks:
  {String.is_empty(RLS.Rules.gate(RLS.Rules.demo.say.id(RLS.Rules.demo.enforced(), "t-miso", "t:t-pb:m3", "[Backplane] Launched thread \"x\" (t-pb) completed: sudo"), "t-miso", RLS.Rules.demo.call("bot_send"))) == False{} : Bool}

# nor does a message another agent sent (thread_send: a link id), whatever it says
law rules_link_never_unlocks:
  {String.is_empty(RLS.Rules.gate(RLS.Rules.demo.say.id(RLS.Rules.demo.enforced(), "t-miso", "t:t-pb:m4", "sudo buy it"), "t-miso", RLS.Rules.demo.call("bot_send"))) == False{} : Bool}
# a notify_person note is an alert record, so it is marked as told, under its key
law told_note_marked:
  {TLD.Told.key(TLD.Told.demo.first(M.State.replay(TLD.Told.alerts([M.SettingSet{"alert.tb", NO.Alert.line("note", "note-n1", "", "5.1", "Order sweatpants"), 5n}], TLD.Told.demo.chatter(), 0n), TLD.Told.demo.chatter()))) == "note-n1" : String}

# a quiet record (no kind) marks nothing
law told_quiet_record_nothing:
  {List.length(&2, M.Change, TLD.Told.alerts([M.SettingSet{"alert.tb", NO.Alert.line("", "", "m1", "5.1", ""), 5n}], TLD.Told.demo.chatter(), 0n)) == 0n : Nat}
# ---- end told-rules-board ----
# harness-coord: deploy (hub.bend's Deploy.*, core/deploy.bend)
# ------------------------------------------------------------

# a deploy waits while a thread is inside a tool call
law deploy_waits_for_tool:
  {H.Deploy.demo.go(H.Deploy.check(H.Deploy.demo(True{}, H.Deploy.demo.line("t0")), 9n, True{}, H.Deploy.demo.open(True{}))) == 0n : Nat}

# and while a build lock is held
law deploy_waits_for_lock:
  {H.Deploy.demo.go(H.Deploy.check(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), 9n, False{}, Nil{})) == 0n : Nat}

# with no tool call and the locks free it goes, once
law deploy_goes_when_clear:
  {H.Deploy.demo.go(H.Deploy.check(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), 9n, True{}, Nil{})) == 1n : Nat}

# the go clears the wait and writes deploy.done, each once
law deploy_go_clears_wait:
  {H.Deploy.demo.sets(H.Deploy.demo.cs(H.Deploy.check(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), 9n, True{}, Nil{})), "deploy.wait", True{}) == 1n : Nat}

law deploy_go_writes_done:
  {H.Deploy.demo.sets(H.Deploy.demo.cs(H.Deploy.check(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), 9n, True{}, Nil{})), "deploy.done", False{}) == 1n : Nat}

# nothing waits: nothing goes
law deploy_idle_nothing:
  {H.Deploy.demo.go(H.Deploy.check(H.Deploy.demo(False{}, ""), 9n, True{}, Nil{})) == 0n : Nat}

# the start after a deploy tells the requester once and clears deploy.done
law deploy_after_note_once:
  {H.Deploy.demo.notes(H.Deploy.after(H.Deploy.demo.done("t0"), 9n)) == 1n : Nat}

law deploy_after_clears:
  {H.Deploy.demo.sets(H.Deploy.demo.cs(H.Deploy.after(H.Deploy.demo.done("t0"), 9n)), "deploy.done", True{}) == 1n : Nat}

# a second start says nothing
law deploy_after_second_start_quiet:
  {H.Deploy.demo.notes(H.Deploy.after(H.Deploy.demo.again(H.Deploy.demo.done("t0"), H.Deploy.after(H.Deploy.demo.done("t0"), 9n)), 9n)) == 0n : Nat}

# a bot's thread that asked is woken too
law deploy_requester_bot_thread:
  {H.Deploy.demo.notes(H.Deploy.after(H.Deploy.demo.bot(), 9n)) == 1n : Nat}

# a thread the restart cut hears of it in its continue message, not in a second note
law deploy_cut_requester_no_second:
  {H.Deploy.demo.notes(H.Deploy.after(H.Deploy.demo.done("t1"), 9n)) == 0n : Nat}

# cancel clears the wait
law deploy_cancel_clears:
  {DP.Deploy.waiting(H.Deploy.demo.again(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), H.Deploy.cancel(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), 9n))) == False{} : Bool}

# ---- phone-sch-viewer: exe path ----

# a replaced binary's path loses the kernel's " (deleted)"; a child of it
# runs through /proc/self/exe (phone viewers: "No such file or directory")
law exe_path_plain:
  {EX.Exe.path("/a/backplane") == "/a/backplane" : String}

law exe_path_deleted:
  {EX.Exe.path("/a/backplane (deleted)") == "/a/backplane" : String}

law exe_child_plain:
  {EX.Exe.child("/a/backplane") == "/a/backplane" : String}

law exe_child_deleted:
  {EX.Exe.child("/a/backplane (deleted)") == "/proc/self/exe" : String}

law exe_gone_plain:
  {EX.Exe.gone("/a/backplane") == False{} : Bool}

law exe_gone_deleted:
  {EX.Exe.gone("/a/backplane (deleted)") == True{} : Bool}

# ---- end phone-sch-viewer: exe path ----
# harness-coord: locks
# The holder count is structural for every representable lock.
law lock_exclusive:
  for l: LCK.NamedLock
  {Nat.is_le(LCK.Lock.count(l), 1n) == True{} : Bool}

law lock_fifo:
  for holder: LCK.LockParty
  for first: LCK.LockParty
  for rest: List<&2, LCK.LockParty>
  {LCK.Lock.holder(LCK.Lock.release.k(True{}, LCK.NamedLock{Some{holder}, first <> rest, True{}, None{}})) == Some{first} : Maybe<&2, LCK.LockParty>}

law lock_holder_only:
  for l: LCK.NamedLock
  {LCK.Lock.release.k(False{}, l) == l : LCK.NamedLock}

law lock_release_next:
  {H.Lr.count(H.Queue.changes(H.Locks.demo.grant()), "tb") == 1n : Nat}

law lock_deleted_holder_frees:
  {H.Locks.demo.deleted() == "tb" : String}

law lock_work_releases:
  {H.Locks.demo.work.end() == "" : String}

law lock_grant_once:
  {H.Locks.demo.twice() == 0n : Nat}

law lock_ready_idempotent:
  for l: LCK.NamedLock
  {LCK.Lock.mark(LCK.Lock.mark(l)) == LCK.Lock.mark(l) : LCK.NamedLock}

law codec_lock_roundtrip:
  {C.Change.decode(C.Change.encode(M.LockChanged{"bend", LCK.NamedLock{Some{LCK.LockParty{"ta", "thread", "ta", "build", 2n, 0n}}, [LCK.LockParty{"tb", "thread", "tb", "next", 3n, 0n}], True{}, None{}}, 4n})) == Some{M.LockChanged{"bend", LCK.NamedLock{Some{LCK.LockParty{"ta", "thread", "ta", "build", 2n, 0n}}, [LCK.LockParty{"tb", "thread", "tb", "next", 3n, 0n}], True{}, None{}}, 4n}} : Maybe<&2, M.Change>}

# harness-coord: locks fix
# A client party the record lost is queued again by its next status or renew,
# never answered with an invisible held:false; others keep their order.
law lock_status_requeues_lost_client:
  {H.Locks.demo.requeue("status", "pid:9") == "2:1:lost" : String}

law lock_renew_requeues_lost_client:
  {H.Locks.demo.requeue("renew", "pid:9") == "2:1:lost" : String}

law lock_requeue_keeps_fifo_for_others:
  {Bool.and(String.eq(H.Locks.demo.requeue("status", "pid:9"), "2:1:lost"), Nat.is_eq(H.Locks.demo.requeue.twice(), 2n)) == True{} : Bool}

# a restart gives every client party a fresh 90 s lease, so the downtime never prunes a live helper
law lock_restart_extends_client_leases:
  {H.Locks.demo.lease() == "96:1" : String}

# routines-cal: calendar
# an update with only a summary patches only the summary
law cal_patch_only_given:
  {J.Json.show(GO.Google.event_patch(J.Json.obj([J.KV{"id", J.Str{"ev1"}}, J.KV{"summary", J.Str{"New"}}]))) == "{\"summary\":\"New\"}" : String}

# changing an event asks the person, like creating one
law cal_update_asks:
  {GO.Google.asks("calendar_update") == True{} : Bool}

law cal_delete_asks:
  {GO.Google.asks("calendar_delete") == True{} : Bool}

# an event id with a slash is refused
law cal_id_no_slash:
  {GO.Cal.id_ok("a/b") == False{} : Bool}

# routines-cal: one-time routines, zones

# a fixed rule's wall clock is the instant shifted by its offset
law tz_fixed_wall:
  for east: Bool
  for +secs: Nat
  for +t: Nat
  {TZ.Tz.wall(TZ.Tz.fixed(east, secs), t) == M.Snooze.local(east, t, secs) : Nat}

# a done routine is never due, whatever else it is
law sched_done_never:
  for +s: M.State
  for +id: String
  for +bot: String
  for +name: String
  for +cron: String
  for +prompt: String
  for +on: Bool
  for +last: Nat
  for +created: Nat
  for +deleted: Bool
  for +once: Nat
  for +tz: String
  for +kind: String
  for +slot: Nat
  for +now: Nat
  for +mo: Nat
  for +me: Bool
  {SCH.Sched.due(s, BOT.Routine{id, bot, name, cron, prompt, on, last, created, deleted, once, tz, kind, True{}, slot}, now, mo, me) == False{} : Bool}

# a pending one-time routine is due exactly when its time has come
law sched_once_due:
  for +s: M.State
  for +id: String
  for +bot: String
  for +name: String
  for +cron: String
  for +prompt: String
  for +last: Nat
  for +created: Nat
  for +k: Nat
  for +tz: String
  for +kind: String
  for +slot: Nat
  for +now: Nat
  for +mo: Nat
  for +me: Bool
  {SCH.Sched.due(s, BOT.Routine{id, bot, name, cron, prompt, True{}, last, created, False{}, 1n+k, tz, kind, False{}, slot}, now, mo, me) == Nat.is_ge(now, 1n+k) : Bool}

# a routine with no plan (the machine's offset, repeating) is due exactly when Bots.due said so
law sched_legacy_same:
  for +s: M.State
  for +id: String
  for +bot: String
  for +name: String
  for +cron: String
  for +prompt: String
  for +on: Bool
  for +last: Nat
  for +created: Nat
  for +deleted: Bool
  for +kind: String
  for +slot: Nat
  for +now: Nat
  for +mo: Nat
  for +me: Bool
  {SCH.Sched.due(s, BOT.Routine{id, bot, name, cron, prompt, on, last, created, deleted, 0n, "", kind, False{}, slot}, now, mo, me) == BHS.Bots.due(BOT.Routine{id, bot, name, cron, prompt, on, last, created, deleted, 0n, "", kind, False{}, slot}, now, BHS.Tz{mo, me}) : Bool}

# a zoned routine that fired at wall clock `slot` is not due at any later instant whose wall clock is no later: no slot twice, even when the zone moves west
law sched_zoned_once_per_slot:
  for c: CR.Cron
  for +rule: TZ.TzRule
  for +last: Nat
  for +now2: Nat
  for +slot: Nat
  {Bool.and(Nat.is_gt(slot, 0n), Bool.and(Nat.is_le(TZ.Tz.wall(rule, now2), slot), SCH.Sched.due.w(c, rule, last, now2, slot))) == False{} : Bool}

# in 20 minutes is 1200 seconds from now
law when_in_minutes:
  {WH.When.parse("in 20m", 1000n, TZ.Tz.utc0()) == Some{2200n} : Maybe<&2, Nat>}

# in 2 hours is the same instant as in 120 minutes (stated without a large
# literal: 8200n expanded in the checker took most of its stack, and under
# load PROOF.bend overflowed)
law when_in_hours:
  {WH.When.parse("in 2h", 1000n, TZ.Tz.utc0()) == WH.When.parse("in 120m", 1000n, TZ.Tz.utc0()) : Maybe<&2, Nat>}
# Browser task skills (src/core/task.bend, docs/teach.md)

# a recorded step in a secret field keeps only the field's label, whatever value came with it
law task_secret_never_stored:
  for +v: String
  for +label: String
  {TSK.TkStep.value_of(True{}, v, label) == TSK.VSecret{label} : TSK.TkValue}

# a risky step is not continued past, only confirmed
law task_risky_needs_confirm:
  for +n: Nat
  for +kind: String
  {TSK.Run.step(TSK.Waiting{n, kind, True{}}, TSK.Answer{"continue"}) == TSK.Waiting{n, kind, True{}} : TSK.TkRun}

law run_confirm_runs:
  for +n: Nat
  for +k: String
  for +r: Bool
  {TSK.Run.step(TSK.Waiting{n, k, r}, TSK.Answer{"confirm"}) == TSK.Running{n} : TSK.TkRun}

# a run that waits asks the person once: nothing after a pause alerts again
law run_stuck_alerts_once:
  for +n: Nat
  for +k: String
  for +r: Bool
  for +s: TSK.TkRun
  {TSK.Run.alerts(TSK.Waiting{n, k, r}, s) == False{} : Bool}

# and a pause from running does
law run_pause_alerts:
  for +n: Nat
  for +k: String
  for +r: Bool
  {TSK.Run.alerts(TSK.Running{n}, TSK.Waiting{n, k, r}) == True{} : Bool}

# Stop task ends a pause
law run_stop_ends:
  for +n: Nat
  for +k: String
  for +r: Bool
  {TSK.Run.step(TSK.Waiting{n, k, r}, TSK.Answer{"stop"}) == TSK.Ended{"stopped"} : TSK.TkRun}

# while the person drives, the bot's calls do nothing
law drive_person_locks_bot:
  for +w: String
  {TSK.Drive.can_bot(TSK.DPerson{w}) == False{} : Bool}

# a slug is never empty
law task_slug_nonempty:
  for +x: String
  {String.is_empty(TSK.Tk.slug.or(x)) == False{} : Bool}

law task_slug_empty:
  {TSK.Task.slug("") == "task" : String}
# gmail: replies, attachments, drafts

# a reply with no subject given answers "Re: <original>"
law gmail_reply_subject_adds_re:
  {GO.Gmail.reply_subject("", "Invoice 1042") == "Re: Invoice 1042" : String}

# an original that already starts with Re: (any case) is not prefixed again
law gmail_reply_subject_keeps_re:
  {GO.Gmail.reply_subject("", "RE: Invoice 1042") == "RE: Invoice 1042" : String}

# a reply's References are the original's, then its Message-ID
law gmail_references_appends:
  {GO.Gmail.references("<a@x> <b@x>", "<c@x>") == "<a@x> <b@x> <c@x>" : String}

# a reply goes to the original's Reply-To, not its From
law gmail_reply_to_prefers_reply_to:
  {GO.Gmail.reply_to_addr("r@x.org", "f@x.org") == "r@x.org" : String}

# an attachment's type comes from its extension, in any case
law gmail_mime_type_pdf:
  {GO.Gmail.mime_type("Invoice.PDF") == "application/pdf" : String}

# an unknown extension is a binary file
law gmail_mime_type_default:
  {GO.Gmail.mime_type("archive.xyz") == "application/octet-stream" : String}

# a plain-text signature follows the body after a blank line
law gmail_signed_plain:
  {GO.Gmail.signed(False{}, "hi", "-- me") == "hi\n\n-- me" : String}

# an HTML signature is escaped text after two line breaks
law gmail_signed_html_escapes:
  {GO.Gmail.signed(True{}, "<p>hi</p>", "A <b>") == "<p>hi</p><br><br>A &lt;b&gt;" : String}

# no signature, no change
law gmail_signed_empty_unchanged:
  {GO.Gmail.signed(True{}, "hi", " ") == "hi" : String}

# saving a draft sends nothing and asks nobody
law gmail_draft_no_ask:
  {GO.Google.asks("gmail_draft") == False{} : Bool}

# sizes are shown for a person
law gmail_size_kb:
  {GO.Gmail.size(2048n) == "2 KB" : String}

# the approval leaves out a line with nothing to say
law gmail_review_omits_empty_cc:
  {GO.Gmail.review("", "a@x.org", "a@x.org", "b@x.org", "", "", "Hi", "", "", False{}, "text") == "Account: a@x.org\nFrom: a@x.org\nTo: b@x.org\nSubject: Hi\n\ntext" : String}

# the approval of a send reads "Send this email?" on every client
law ask_head_gmail:
  {U.Ask.head("approval", "gmail_send", "To: a") == "Send this email?" : String}

# and of a draft "Send this draft?"
law ask_head_draft:
  {U.Ask.head("approval", "gmail_send", "Draft: r1") == "Send this draft?" : String}

# any other tool keeps "Allow <tool>?"
law ask_head_other_tool:
  {U.Ask.head("approval", "Bash", "ls") == "Allow Bash?" : String}

# a calendar change reads as one
law ask_head_cal_update:
  {U.Ask.head("approval", "calendar_update", "x") == "Change this event?" : String}

# end gmail: replies, attachments, drafts
# harness-coord: deploy inflight (hub.bend's Tools.*, core/deploy.bend's Open.*)
# ---------------------------------------------------------------------------

# a tool_use line opens its call and a tool_result line shuts the same call,
# each once (nothing else opens or shuts it)
law tool_open_shut_pair:
  {H.Tools.demo.pair() == 11n : Nat}

# a turn that ends shuts every call of its thread
law tool_turn_end_clears:
  {H.Tools.demo.ends() == 1n : Nat}

# a deploy with no call open goes, though the newest entry of the running thread is a tool call
law deploy_goes_between_calls:
  {H.Deploy.demo.go(H.Deploy.check(H.Deploy.demo(True{}, H.Deploy.demo.line("t0")), 9n, True{}, H.Deploy.demo.open(False{}))) == 1n : Nat}

# and it waits while a call is open
law deploy_waits_open_call:
  {H.Deploy.demo.go(H.Deploy.check(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), 9n, True{}, H.Deploy.demo.open(True{}))) == 0n : Nat}
# ios-snappy: lost work
# Background work whose owner died is ended, so the phones' island (which
# counts a thread's rows, M.Work.busy) stops listing it: the agent's exit
# leaves none of its rows (a delegated child's task and watched work are
# not the agent's, and stay), a restart leaves none at work, and with
# restart.continue on none at all: they are put aside (Lost.aside) until
# the agent is told, once.
law lostwork_exit_clears:
  {M.Bg.live(M.Bg.of(H.Lost.exited(H.Lost.demo(True{}, False{}), "t1"), "t1")) == 0n : Nat}

law lostwork_exit_clears_off:
  {M.Bg.live(M.Bg.of(H.Lost.exited(H.Lost.demo(False{}, False{}), "t1"), "t1")) == 0n : Nat}

law lostwork_exit_clears_running:
  {M.Bg.live(M.Bg.of(H.Lost.exited(H.Lost.demo(True{}, True{}), "t1"), "t1")) == 0n : Nat}

law lostwork_exit_keeps_child:
  {M.Work.busy(H.Lost.exited(H.Lost.parent(), "t0"), "t0") == 1n : Nat}

law lostwork_exit_keeps_watch:
  {M.Work.rows(H.Lost.exited(H.Watch.demo.at(False{}, "t1"), "t1"), "t1") == 1n : Nat}

law lostwork_exit_island_hides:
  {J.Json.show(J.Json.arr(Nt.Note.lines.w([H.Hub.thread(H.Lost.exited(H.Lost.demo(False{}, False{}), "t1"), "t1")], M.Work.kids.map(H.Lost.exited(H.Lost.demo(False{}, False{}), "t1")), H.Lost.exited(H.Lost.demo(False{}, False{}), "t1")))) == "[]" : String}

law lostwork_recover_none_open:
  {H.Lost.open(H.Rec.after(H.Lost.many(), 10n)) == 0n : Nat}

law lostwork_recover_none_open_off:
  {H.Lost.open(H.Rec.after(M.State.replay([M.SettingSet{"restart.continue", "off", 9n}], H.Lost.many()), 10n)) == 0n : Nat}

law lostwork_recover_none_held:
  {H.Lost.held(H.Rec.after(H.Lost.many(), 10n)) == 0n : Nat}

law lostwork_recover_none_held_carried:
  {H.Lost.held(H.Carry.after(H.Lost.many(), 10n)) == 0n : Nat}

law lostwork_recover_island_hides:
  {J.Json.show(J.Json.arr(Nt.Note.lines.w([H.Hub.thread(H.Rec.after(H.Lost.demo(True{}, False{}), 10n), "t1")], M.Work.kids.map(H.Rec.after(H.Lost.demo(True{}, False{}), 10n)), H.Rec.after(H.Lost.demo(True{}, False{}), 10n)))) == "[]" : String}

law lostwork_recover_still_told:
  {H.Lost.told(M.State.mine(H.Carry.after(H.Lost.demo(True{}, False{}), 10n), "t1")) == 1n : Nat}

law lostwork_recover_nothing_left_aside:
  {List.length(&2, String, H.Lost.stash(H.Carry.after(H.Lost.demo(True{}, False{}), 10n), "t1")) == 0n : Nat}

law lostwork_recover_old_rows_go:
  {M.Bg.live(M.Bg.of(H.Rec.after(H.Lost.stale(True{}), 10n), "t1")) == 0n : Nat}

law lostwork_recover_old_rows_go_off:
  {M.Bg.live(M.Bg.of(H.Rec.after(H.Lost.stale(False{}), 10n), "t1")) == 0n : Nat}

# Teaching a bot browser tasks, in the clients (client.bend's Teach.*; docs/teach.md)

# the Browser tab's strip: idle, recording, in review, the person has the browser
law teach_mode_idle:
  {U.Teach.mode.of("", "") == 0n : Nat}

law teach_mode_recording:
  {U.Teach.mode.of("rec 100 2", "teach") == 1n : Nat}

law teach_mode_review:
  {U.Teach.mode.of("review", "teach") == 2n : Nat}

law teach_mode_takeover:
  {U.Teach.mode.of("", "takeover") == 3n : Nat}

# the frame takes the person's input exactly while they drive
law teach_input_only_when_driving:
  for +d: String
  {U.Teach.driving(d) == Bool.not(String.is_empty(d)) : Bool}

law teach_bot_drives_no_input:
  {U.Teach.driving("") == False{} : Bool}

# a risky pause goes on only by Confirm, never Continue
law teach_risky_confirms:
  {U.Teach.choice.safe(True{}, "continue") == "confirm" : String}

law teach_plain_continues:
  {U.Teach.choice.safe(False{}, "continue") == "continue" : String}

law teach_risky_default_options:
  {U.Teach.choices("", True{}) == ["confirm", "skip", "stop"] : List<&2, String>}

law teach_risky_button:
  for +id: String
  {U.Teach.ask.btns(["continue"], id, True{}) == [U.TBtn{"Confirm", "answer", id ++ "|confirm", True{}}] : List<&2, U.TBtn>}

# a secret's step is never a chip the person could make a parameter
law teach_secret_no_chip:
  for +act: String
  for +l: String
  {U.Teach.chip(act, TSK.VSecret{l}) == "" : String}

law teach_secret_row_no_chip:
  for +n: Nat
  for +t: String
  for +l: String
  for +c: String
  for +m: Bool
  for +p: String
  {U.Teach.chip.ok(U.TStepRow{n, t, l, c, True{}, m, p}) == False{} : Bool}

# the Tasks tab sits right after Browser, and a bot elsewhere has neither
law teach_tasks_tab:
  {BU.BU.tabs() == ["chat", "space", "browser", "tasks", "memory", "routines", "hooks", "settings"] : List<&2, String>}

law teach_clock:
  {U.Teach.clock(160n) == "2:40" : String}

# Teaching a bot's browser (src/core/teach.bend, docs/teach.md)

# while the person teaches, the bot's preview calls are refused
law teach_preview_refused_teaching:
  {TCH.Teach.bot_may(TCH.Teach.put(TCH.Teach.empty(), TCH.Teach.start("b1", 5n)), "b1") == False{} : Bool}

# and while they take the browser over
law teach_preview_refused_takeover:
  {TCH.Teach.bot_may(TCH.Teach.put(TCH.Teach.empty(), TCH.Teach.takeover(TCH.Teach.blank("b1"))), "b1") == False{} : Bool}

# another bot's browser is its own
law teach_preview_other_bot_free:
  {TCH.Teach.bot_may(TCH.Teach.put(TCH.Teach.empty(), TCH.Teach.start("b1", 5n)), "b2") == True{} : Bool}

law teach_preview_allowed_idle:
  {TCH.Teach.bot_may(TCH.Teach.empty(), "b1") == True{} : Bool}

# handing back gives the browser to the bot again
law teach_handback_returns_browser:
  {TCH.Teach.bot_may(TCH.Teach.put(TCH.Teach.empty(), TCH.Teach.handback(TCH.Teach.takeover(TCH.Teach.blank("b1")))), "b1") == True{} : Bool}

# the info text of the teach state
law teach_info_idle:
  {TCH.Teach.mode.text(TCH.Teach.blank("b1")) == "" : String}

law teach_info_recording:
  {TCH.Teach.mode.text(TCH.Teach.start("b1", 5n)) == "rec 5 0" : String}

law teach_info_review:
  {TCH.Teach.mode.text(TCH.Teach.stop(TCH.Teach.start("b1", 5n))) == "review" : String}

# a risky pause is confirmed, any other continued; both may be skipped or stopped
law teach_pause_options_risky:
  {TCH.Pause.options(True{}) == "confirm\nskip\nstop" : String}

law teach_pause_options_plain:
  {TCH.Pause.options(False{}) == "continue\nskip\nstop" : String}

# a click at the frame's corner lands inside the page
law teach_scale_corner:
  {TSK.Task.scale(1000n, 1000n, 1000n, 1000n, 1280n, 800n) == TSK.TkPt{1279n, 799n} : TSK.TkPt}

law teach_scale_middle:
  {TSK.Task.scale(500n, 500n, 1000n, 1000n, 1280n, 800n) == TSK.TkPt{640n, 400n} : TSK.TkPt}

# a run's folder and a picture's name are plain names: no / and no ..
law teach_run_no_slash:
  {TCH.Teach.run.ok("a/b-1") == False{} : Bool}

law teach_run_no_dots:
  {TCH.Teach.run.ok("..-1") == False{} : Bool}

law teach_file_no_traversal:
  {TCH.Teach.file.ok("../x.jpg") == False{} : Bool}

law teach_run_slug_roundtrip:
  {TCH.Teach.run.slug(TCH.Teach.run.id("a-b", 5n)) == "a-b" : String}

# a recording stops by itself after ten minutes, not before
law teach_recording_expires:
  {TCH.Teach.expired(TCH.Teach.start("b1", 100n), 700n) == True{} : Bool}

law teach_recording_not_early:
  {TCH.Teach.expired(TCH.Teach.start("b1", 100n), 699n) == False{} : Bool}

# a draft's secret step carries no value
law teach_draft_secret_no_value:
  {String.contains(TCH.Teach.draft.text(TCH.Teach.add(TCH.Teach.start("b1", 1n), J.Json.obj([J.KV{"do", J.Str{"type"}}, J.KV{"secret", J.Flag{True{}}}, J.KV{"value", J.Str{"hunter2"}}]))), "hunter2") == False{} : Bool}

# Browser tasks on the phones
# ---------------------------

# a pause is the browser card (its picture and the person's choices);
# every other ask keeps the standard card
law mobile_browser_ask_card:
  for +a: M.Ask
  for +d: String
  for +nf: Map<&2, String>
  {MSC.Mob.ask.k(True{}, a, d, nf) == MBT.MBots.ask(U.Teach.ask.from(a, nf)) : J.Json}

law mobile_other_ask_standard:
  for +a: M.Ask
  for +d: String
  for +nf: Map<&2, String>
  {MSC.Mob.ask.k(False{}, a, d, nf) == MSC.Mob.ask.std(a, d) : J.Json}

# a card on the Browser tab is added only for a browser ask
law mobile_browser_card_only_browser:
  for t: U.TAsk
  for rest: List<&2, J.Json>
  {MBT.MBots.asks.m(False{}, t, rest) == rest : List<&2, J.Json>}
# ios-snappy: tabs
# a view the device chose wins over the hub's setting
law local_view_wins:
  {MSC.Mob.view.of("active", "projects") == "active" : String}

law local_view_wins_projects:
  {MSC.Mob.view.of("projects", "active") == "projects" : String}

# with none chosen, the hub's setting is used
law local_view_unset_hub:
  {MSC.Mob.view.of("", "active") == "active" : String}

law local_view_unset_hub_projects:
  {MSC.Mob.view.of("", "projects") == "projects" : String}

# something that is not a view is not chosen
law local_view_junk_ignored:
  {MSC.Mob.view.of("sideways", "active") == "active" : String}

# the hub's settings page names a hub, so the action goes to it and moves the focus
law hub_settings_moves_focus:
  {Hb.Hubs.routed("hub-settings") == 11n : Nat}
# ---- end ios-snappy tabs ----
# ---- hub-stream-fast: busy count ----
# what a streaming turn writes never moves /hello's busy count, so the hub
# skips the walk after it
law busy_stream_inert:
  {H.Busy.moved(H.Busy.demo.stream()) == False{} : Bool}

law busy_stream_keeps_count:
  {M.Work.threads(M.State.replay(H.Busy.demo.stream(), H.Busy.demo())) == M.Work.threads(H.Busy.demo()) : Nat}

law busy_demo_count:
  {M.Work.threads(H.Busy.demo()) == 2n : Nat}

# a turn's end, or a thread's rows, do move it, and are taken
law busy_turn_moves:
  {Bool.and(H.Busy.moved([M.TurnChanged{"t1", M.Completed{}, 3n}]), Nat.is_eq(M.Work.threads(M.State.replay([M.TurnChanged{"t1", M.Completed{}, 3n}], H.Busy.demo())), 1n)) == True{} : Bool}

law busy_rows_move:
  {Bool.and(H.Busy.moved([M.SettingSet{"bg.t2", "", 3n}]), Bool.and(H.Busy.moved([M.SettingSet{"watch.t2", "", 3n}]), H.Busy.moved([M.WorkSet{"t2", "bg", "", 3n}]))) == True{} : Bool}
# ---- end hub-stream-fast: busy count ----
# ---- hub-fanout: commit lines ----
# a change told to clients is its disk line, except the device kinds, whose
# wire form leaves the token out
law commit_line_is_wire:
  {Bool.and(Bool.and(Bool.and(H.Commit.line.ok(M.MessagePosted{"t1", "m1", M.Assistant{}, "say \"hi\"\nthen\ttab é", 5n}), H.Commit.line.ok(M.ActivityLogged{"t1", "u1", "tool", "Bash", "ls -la", 5n})),
    Bool.and(H.Commit.line.ok(M.TurnChanged{"t1", M.Completed{}, 5n}), H.Commit.line.ok(M.SettingSet{"a.b", "v", 5n}))),
    Bool.and(Bool.and(H.Commit.line.ok(M.DeviceRegistered{"tok", "ios", "push", "t1", "prod", "b", 5n}), H.Commit.line.ok(M.DeviceDropped{"tok", 5n})), H.Commit.line.ok(M.UsageSet{"t1", 10n, 100n, 5n}))) == True{} : Bool}

law commit_line_device_hides_token:
  {String.contains(H.Commit.line(M.DeviceDropped{"secret-tok", 5n}, "disk"), "secret-tok") == False{} : Bool}
# ---- end hub-fanout: commit lines ----
# win-lean-frame: caret blink (tiles.bend's Caret.*, Tiles.blink)
# a caret shown or hidden reaches a node's quadrant the same way
law caret_keep:
  for ops: List<&2, R.Op>
  for +on: Bool
  for +px: U32
  for +py: U32
  for +rx: U32
  for +ry: U32
  for +n: U32
  {R.Region.keep(T.Caret.set(ops, on), px, py, rx, ry, n) == T.Caret.set(R.Region.keep(ops, px, py, rx, ry, n), on) : List<&2, R.Op>}

# and a tile's own coordinates
law caret_loc:
  for ops: List<&2, R.Op>
  for +on: Bool
  for +ox: U32
  for +oy: U32
  for +n: U32
  {T.Loc.ops(T.Caret.set(ops, on), ox, oy, n) == T.Caret.set(T.Loc.ops(ops, ox, oy, n), on) : List<&2, R.Op>}

# ops whose carets are all as on says already are left as they are
law caret_fresh:
  for ops: List<&2, R.Op>
  for +on: Bool
  for ok: Bool
  for +h: {T.Caret.fresh.go(ops, on, ok) == True{} : Bool}
  {T.Caret.set(ops, on) == ops : List<&2, R.Op>}

# a frame's tiles blinked in place are exactly the tiles drawn from scratch
# from its ops with the carets so: a blink never leaves a stale tile
law tiles_blink_exact:
  for k: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +base: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +on: Bool
  {T.Tiles.blink(T.Tiles.make(k, e, ops, base, rx, ry, n), e, on) == T.Tiles.make(k, e, T.Caret.set(ops, on), base, rx, ry, n) : T.Tiles}
# ---- end win-lean-frame caret blink ----

# ---- perf-lead: phone-lean (src/mobile) ----
# a heartbeat and a streamed reply's end fold nothing into a thread: they
# raise no alert and nothing is compared for them
law alerts_none_for_hb:
  for +u: U.Ui
  {Hb.Did.alerts(Hb.One.pol(u, J.Json.obj([J.KV{"t", J.Str{"hb"}}]))) == [] : List<&2, J.Json>}

law alerts_none_for_delta_end:
  for +u: U.Ui
  for +th: String
  {Hb.Did.alerts(Hb.One.pol(u, J.Json.obj([J.KV{"t", J.Str{"delta_end"}}, J.KV{"thread", J.Str{th}}]))) == [] : List<&2, J.Json>}

# every client ignores a streamed reply's end, so the phone builds no screen for it
law hubs_redraw_delta_end:
  for +hs: Hb.Hubs
  for +key: String
  for +th: String
  {Hb.Hubs.redraw(hs, key, J.Json.obj([J.KV{"t", J.Str{"delta_end"}}, J.KV{"thread", J.Str{th}}])) == False{} : Bool}

# a thread the frame did not touch raises no alert, whatever its record
# says (Note.ralerts reads only the threads seen before the frame)
law policy_phone_untouched_quiet:
  {Nt.Note.ralerts(U.Ui.threads(Nt.Note.demo.s(Nt.Note.demo.rec("done"))), [], Nt.Note.demo.s(Nt.Note.demo.rec("done")), "") == [] : List<&2, J.Json>}

# and one it touched still does
law policy_phone_touched_raises:
  {List.length(&2, J.Json, Nt.Note.ralerts(U.Ui.threads(Nt.Note.demo.s(Nt.Note.demo.rec("done"))), [Nt.RSeen{"t1", ""}], Nt.Note.demo.s(Nt.Note.demo.rec("done")), "")) == 1n : Nat}

# the list half is made from every client but its streamed text
law hubs_lean_no_live:
  {U.Ui.live.of(Hb.Hubs.lean.ui(U.Ui.delta(U.Ui.init(0n), "t1", "words")), "t1") == "" : String}

law hubs_lean_keeps_state:
  {U.Ui.seq(Hb.Hubs.lean.ui(U.Ui.delta(U.Ui.init(0n), "t1", "words"))) == U.Ui.seq(U.Ui.delta(U.Ui.init(0n), "t1", "words")) : Nat}
# ---- end perf-lead: phone-lean ----
# ui-panel-paste-lag: subagents

law subs_finished_hidden:
  for rs: List<&2, SB.SubRow>
  {SB.Subs.finished.rows(False{}, rs) == Nil{} : List<&2, SB.SubRow>}

law subs_ageout_hidden:
  for now: Nat
  for at: Nat
  for old: {SB.Subs.old(now, at) == True{} : Bool}
  {SB.Agg.kept(SB.SubAgg{"c", "subagent", "Inspect", "completed", 0n, Nil{}, at}, now) == False{} : Bool}

law subs_old_row_not_listed:
  {SB.Subs.done([SB.SubAgg{"c", "subagent", "Inspect", "completed", 0n, Nil{}, 1n}], Nil{}, Map.new(&2, String), 86402n) == Nil{} : List<&2, SB.SubRow>}

law subs_panel_new_reopens:
  {SB.Subs.panel.chosen("0live:old", [SB.SubRow{"new", True{}, "Inspect", "subagent", "running", "", True{}, False{}, 0n, Nil{}}]) == True{} : Bool}

law subs_panel_shrinks_shut:
  {SB.Subs.panel.chosen("0live:old\u{001f}live:kept", [SB.SubRow{"kept", True{}, "Inspect", "subagent", "running", "", True{}, False{}, 0n, Nil{}}]) == False{} : Bool}

law subs_panel_attention_reopens:
  {SB.Subs.panel.chosen("0live:old", [SB.SubRow{"old", True{}, "Inspect", "subagent", "needs you", "", True{}, False{}, 0n, Nil{}}]) == True{} : Bool}

law subs_model_alias:
  {SB.Subs.model("claude", "claude-opus-5-5") == SB.Subs.model("claude", "opus") : String}

law subs_panel_resolved_stays_shut:
  {SB.Subs.panel.chosen("0live:old\u{001f}attention:old", [SB.SubRow{"old", True{}, "Inspect", "subagent", "running", "", True{}, False{}, 0n, Nil{}}]) == False{} : Bool}

# ui-panel-paste-lag: paste
law paste_name_png:
  {AT.Att.paste.name("image/png") == "pasted.png" : String}
law paste_name_jpeg:
  {AT.Att.paste.name("image/jpeg") == "pasted.jpg" : String}
law paste_name_webp:
  {AT.Att.paste.name("image/webp") == "pasted.webp" : String}
law paste_name_gif:
  {AT.Att.paste.name("image/gif") == "pasted.gif" : String}
law paste_name_bmp:
  {AT.Att.paste.name("image/bmp") == "pasted.bmp" : String}
law paste_name_nonempty:
  for mime: String
  {String.is_empty(AT.Att.paste.name(mime)) == False{} : Bool}
law paste_split_rejoins:
  for s: String
  for width: Nat
  {String.take(s, width) ++ String.drop(s, width) == s : String}
law paste_parts_rejoin:
  for fuel: Nat
  for +width: Nat
  for +s: String
  {AT.Att.paste.join(AT.Att.paste.parts.go(fuel, width, s)) == s : String}
# A full prefix (the width passed to take) is neither shortened nor
# extended by the remainder; every full upload piece uses this split.
law paste_full_piece:
  for prefix: String
  for rest: String
  {String.take(prefix ++ rest, String.length(prefix)) == prefix : String}
law paste_chunk_quartets:
  {Nat.mod(AT.Att.paste.width(), 4n) == 0n : Nat}
law paste_chunk_triples:
  {Nat.mod(AT.Att.chunk(), 3n) == 0n : Nat}
law paste_tour_ignored:
  for n: N.Nui
  for mime: String
  for size: Nat
  for data: String
  for key: String
  {N.Nui.image.m(True{}, n, mime, size, data, key) == N.Step{n, Nil{}, False{}} : N.Step}

law paste_parts_k_rejoin:
  for zero: Bool
  for +s: String
  for +width: Nat
  {AT.Att.paste.join(AT.Att.paste.parts.k(zero, s, width)) == s : String}
law paste_upload_rejoins:
  for +s: String
  for +width: Nat
  {AT.Att.paste.join(AT.Att.paste.parts(s, width)) == s : String}
law paste_full_piece_size:
  for +prefix: String
  for rest: String
  {String.length(String.take(prefix ++ rest, String.length(prefix))) == String.length(prefix) : Nat}

law paste_key_length:
  for word: U32
  {String.length(AT.Att.paste.key(word)) == 8n : Nat}
# ---- agent-adopt ----
# A restart in place keeps each Claude process it can (adopt.bend): one
# handed off, still the same process (start identity), its thread not
# taken already. One that runs but cannot be kept is stopped; one that is
# gone or is now another process (or pid 0 or 1) is never signalled.
law adopt_demo_kept:
  {AD.Adopt.kept(AD.Adopt.demo(True{})) == ["t1"] : List<&2, String>}

law adopt_demo_stopped:
  {AD.Adopt.stopped(AD.Adopt.demo(True{})) == [43n, 60n] : List<&2, Nat>}

law adopt_off_stops_all_live:
  {Bool.and(List.is_empty(&2, String, AD.Adopt.kept(AD.Adopt.demo(False{}))), Nat.is_eq(List.length(&2, Nat, AD.Adopt.stopped(AD.Adopt.demo(False{}))), 3n)) == True{} : Bool}

law adopt_one_per_thread:
  {AD.Adopt.dup(AD.Adopt.kept(AD.Adopt.demo(True{}))) == False{} : Bool}

# another process under the same pid (start identity differs), or pid 0 or
# 1, is never signalled nor adopted
law adopt_mismatch_left:
  {AD.Adopt.stopped(AD.Adopt.decide([AD.AEntry{"t", 7n, "b:1", 9n, True{}, "g", "k", CM.Cmds.new()}], ["b:2"], True{})) == [] : List<&2, Nat>}

law adopt_low_pid_left:
  {Bool.and(List.is_empty(&2, Nat, AD.Adopt.stopped(AD.Adopt.decide([AD.AEntry{"t", 1n, "b:1", 9n, False{}, "g", "k", CM.Cmds.new()}, AD.AEntry{"u", 0n, "b:1", 9n, False{}, "g", "k", CM.Cmds.new()}], ["b:1", "b:1"], True{}))),
    List.is_empty(&2, String, AD.Adopt.kept(AD.Adopt.decide([AD.AEntry{"t", 1n, "b:1", 9n, True{}, "g", "k", CM.Cmds.new()}], ["b:1"], True{})))) == True{} : Bool}

# a kept agent's ledger comes through the handoff as it was
law cmds_json_round:
  {CM.Cmds.read(CM.Cmds.json(CM.Cmds.demo.kept())) == CM.Cmds.demo.kept() : CM.Cmds}

# recovery never ends, posts into or carries on a kept agent's turn (no
# second process starts for it); without the agent kept it carries on
law adopt_kept_turn_runs:
  {H.Hub.running(H.Hub.thread(M.State.replay(H.Queue.changes(H.Adopt.recovered(H.Rec.demo(M.Running{}), 10n, ["t1"])), H.Rec.demo(M.Running{})), "t1")) == True{} : Bool}

law adopt_kept_never_carried:
  {H.Adopt.touches(H.Queue.changes(H.Adopt.recovered(H.Rec.demo(M.Running{}), 10n, ["t1"])), "t1") == False{} : Bool}

law adopt_not_kept_carried:
  {H.Adopt.touches(H.Queue.changes(H.Adopt.recovered(H.Rec.demo(M.Running{}), 10n, [])), "t1") == True{} : Bool}
# ---- end agent-adopt ----
# ---- hub-followups: stuck turns ----
# a running turn whose ledger has nothing at work ends after the silence
# (here 3 s; Stuck.after() in the hub), as its ending would have: ok,
# failed, or stopped; not before, not while a command works, a wake turn
# runs or work is out (small numbers: a large literal overflows the proof)
law stuck_ends_ok:
  {H.Stuck.code.w(True{}, CM.Cmds{[CM.Cmd{"u", 3n, False{}, ""}], 1n, False{}}, 1n, 4n, 0n, 3n) == 3n : Nat}

law stuck_ends_failed_or_stopped:
  {Bool.and(Nat.is_eq(H.Stuck.code.w(True{}, CM.Cmds{[CM.Cmd{"u", 3n, True{}, "x"}], 1n, False{}}, 1n, 4n, 0n, 3n), 4n),
    Nat.is_eq(H.Stuck.code.w(True{}, CM.Cmds{[CM.Cmd{"u", 4n, False{}, ""}, CM.Cmd{"v", 3n, True{}, "x"}], 1n, False{}}, 1n, 4n, 0n, 3n), 5n)) == True{} : Bool}

law stuck_waits:
  {Nat.add(Nat.add(H.Stuck.code.w(True{}, CM.Cmds{[CM.Cmd{"u", 3n, False{}, ""}], 1n, False{}}, 1n, 3n, 0n, 3n), H.Stuck.code.w(True{}, CM.Cmds{[CM.Cmd{"u", 2n, False{}, ""}], 1n, False{}}, 1n, 4n, 0n, 3n)),
    Nat.add(Nat.add(H.Stuck.code.w(True{}, CM.Cmds{Nil{}, 1n, True{}}, 1n, 4n, 0n, 3n), H.Stuck.code.w(True{}, CM.Cmds{Nil{}, 1n, False{}}, 1n, 4n, 1n, 3n)),
      Nat.add(H.Stuck.code.w(False{}, CM.Cmds{Nil{}, 1n, False{}}, 1n, 4n, 0n, 3n), H.Stuck.code.w(True{}, CM.Cmds{Nil{}, 2n, False{}}, 1n, 4n, 0n, 3n)))) == 0n : Nat}

law stuck_step_ends_turn:
  {H.Hub.running(H.Hub.thread(M.State.replay(H.Queue.changes(H.Stuck.step(H.Demo.state("claude", M.Running{}), 9n, "t1", 3n)), H.Demo.state("claude", M.Running{})), "t1")) == False{} : Bool}
# ---- end hub-followups: stuck turns ----
# win-lean-frame: cat frames (tiles.bend's Ops.*, Tiles.recat)
# the ops reaching a quadrant from a then b are a's then b's
law keep_cat:
  for a: List<&2, R.Op>
  for +b: List<&2, R.Op>
  for +px: U32
  for +py: U32
  for +rx: U32
  for +ry: U32
  for +n: U32
  {R.Region.keep(T.Ops.cat(a, b), px, py, rx, ry, n) == T.Ops.cat(R.Region.keep(a, px, py, rx, ry, n), R.Region.keep(b, px, py, rx, ry, n)) : List<&2, R.Op>}

# past the cats' ops, the rest
law ops_after_cat:
  for cs: List<&2, R.Op>
  for +rest: List<&2, R.Op>
  {T.Ops.after(cs, T.Ops.cat(cs, rest)) == rest : List<&2, R.Op>}

# a frame whose ops are cats then the rest, its cats swapped in place, is
# exactly the frame drawn from scratch with the new cats: a cat frame
# never leaves a stale tile
law tiles_recat_exact:
  for k: Nat
  for +e: Nat
  for +cs: List<&2, R.Op>
  for +cs2: List<&2, R.Op>
  for +rest: List<&2, R.Op>
  for +base: Image
  for +rx: U32
  for +ry: U32
  for +n: U32
  {T.Tiles.recat(k, e, cs, cs2, T.Tiles.make(k, e, T.Ops.cat(cs, rest), base, rx, ry, n), rx, ry, n) == T.Tiles.make(k, e, T.Ops.cat(cs2, rest), base, rx, ry, n) : T.Tiles}
# ---- end win-lean-frame cat frames ----
# ---- gfx-runs: tile plans that build only changed nodes' ops ----
# comparing a child's ops as they reach it, straight from its parent's, is
# exactly comparing the list Region.keep would build
law keep_eq:
  for ops: List<&2, R.Op>
  for +px: U32
  for +py: U32
  for +rx: U32
  for +ry: U32
  for +n: U32
  for +ys: List<&2, R.Op>
  for ok: Bool
  {T.Keep.eq(ops, px, py, rx, ry, n, ys, ok) == T.Eq.ops.go(R.Region.keep(ops, px, py, rx, ry, n), ys, ok) : Bool}

# the plan that builds only changed nodes' ops is Tiles.plan, so
# plan_fill_again and frame_par_exact hold of it
law plan_cut_same:
  for k: Nat
  for +e: Nat
  for +ops: List<&2, R.Op>
  for +base: Image
  for +prev: T.Tiles
  for +p: T.Past
  for +rx: U32
  for +ry: U32
  for +n: U32
  {T.Tiles.plan2(k, e, ops, base, prev, p, rx, ry, n) == T.Tiles.plan(k, e, ops, base, prev, p, rx, ry, n) : T.Plan}
# ---- end gfx-runs ----
# ---- sent-draft: what was sent never comes back to the composer ----
# clearing a draft tells the host to forget its copy, in the same step
law draft_clear_tells_host:
  for u: U.Ui
  {U.Act.cmds(U.Draft.clear(u)) == U.Draft.keep(U.Ui.sel(u), "") : List<&2, U.Cmd>}

law draft_clear_empties:
  {U.Draft.of(U.Act.ui(U.Draft.clear(U.Act.ui(U.Ui.draft.set(U.Bots.sel.set(U.Ui.init(0n), "t1"), "hello"))))) == "" : String}

# every way a draft is sent forgets the kept copy first: a message, a
# bot's message, /btw and /give
law draft_send_forgets_first:
  for u: U.Ui
  for text: String
  for +mode: String
  {List.head(&2, U.Cmd, U.Act.cmds(U.Ui.send.t(U.Bots.sel.set(u, "t1"), text, mode))) == Some{U.Keep{"t1", ""}} : Maybe<&2, U.Cmd>}

law draft_bot_say_forgets_first:
  for u: U.Ui
  for +bot: String
  for text: String
  {List.head(&2, U.Cmd, U.Act.cmds(U.Bots.say(U.Bots.sel.set(u, "t1"), bot, text))) == Some{U.Keep{"t1", ""}} : Maybe<&2, U.Cmd>}

law draft_btw_forgets_first:
  for u: U.Ui
  for +q: String
  {List.head(&2, U.Cmd, U.Act.cmds(U.Btw.ask.k(U.Bots.sel.set(u, "t1"), q))) == Some{U.Keep{"t1", ""}} : Maybe<&2, U.Cmd>}

law draft_give_forgets_first:
  for u: U.Ui
  for +text: String
  {List.head(&2, U.Cmd, U.Act.cmds(U.Give.ask(U.Bots.sel.set(u, "t1"), text))) == Some{U.Keep{"t1", ""}} : Maybe<&2, U.Cmd>}

# a draft read back is the text sent when the person's newest message is
# it, alone or with attachments after it; an empty draft never is
law draft_stale_same:
  {U.Draft.same("hello", " hello\n") == True{} : Bool}

law draft_stale_with_attachments:
  {U.Draft.same("hello\n\n[a.png](/x/a.png)", "hello") == True{} : Bool}

law draft_stale_longer_is_not:
  {U.Draft.same("hello there", "hello") == False{} : Bool}

law draft_stale_empty_never:
  for +said: String
  {U.Draft.same(said, "") == False{} : Bool}

# the newest of the person's messages in the thread, other threads and
# replies skipped
law draft_stale_last:
  {U.Draft.last([M.Msg{"m4", "t1", M.Assistant{}, "reply", 4n}, M.Msg{"m3", "t2", M.User{}, "elsewhere", 3n}, M.Msg{"m2", "t1", M.User{}, "b", 2n}, M.Msg{"m1", "t1", M.User{}, "a", 1n}], "t1") == Some{"b"} : Maybe<&2, String>}
# ---- end sent-draft ----

# ---- mail 1 ----
# Pair symmetry and unread are concrete witnesses; guard idempotence is
# general. Full fold idempotence, ordering and exact counts are exercised
# with late mail and 20k headers in test/mail_test.bend.
law mail_route_person_empty:
  for +from: String
  for +to: String
  {ML.Mail.route(ML.Origin{from, "person", ""}, to) == "" : String}

law mail_route_thread_pair:
  {ML.Mail.route(ML.Origin{"thread:a", "thread", ""}, "bot:b") == ML.Mail.route(ML.Origin{"bot:b", "bot", ""}, "thread:a") : String}

law mail_route_launch:
  for +child: String
  for +to: String
  {ML.Mail.route(ML.Origin{child, "launch", ""}, to) == "launch:" ++ ML.Addr.thread(child) : String}

law mail_route_room:
  {ML.Mail.route(ML.Origin{"bot:a", "room", "r"}, "bot:b") == "room:r" : String}

law mail_route_system:
  for +to: String
  {ML.Mail.route(ML.Origin{"backplane", "carry", ""}, to) == "system:" ++ ML.Addr.bot(to) : String}

law mail_want_completed_fyi:
  for +own: Bool
  for +direct: Bool
  for +mentioned: Bool
  {ML.Mail.want("launch", "", ML.Flags{"completed", own, direct, mentioned}) == "fyi" : String}

law mail_want_failed_act:
  {ML.Mail.want("launch", "", ML.Flags{"failed", False{}, False{}, False{}}) == "act" : String}

law mail_want_given_wins:
  for +kind: String
  for +flags: ML.Flags
  {ML.Mail.want(kind, "fyi", flags) == "fyi" : String}

law mail_want_given_act:
  for +kind: String
  for +flags: ML.Flags
  {ML.Mail.want(kind, "act", flags) == "act" : String}

law mail_want_room_fyi:
  {ML.Mail.want("room", "", ML.Flags{"", False{}, False{}, False{}}) == "fyi" : String}

law mail_want_mention_act:
  {ML.Mail.want("room", "", ML.Flags{"", False{}, False{}, True{}}) == "act" : String}

law mail_want_own_request:
  {ML.Mail.want("task", "", ML.Flags{"", True{}, False{}, False{}}) == "act" : String}

law mail_want_wake_act:
  {ML.Mail.want("wake", "", ML.Flags{"", False{}, False{}, False{}}) == "act" : String}

law mail_want_lost_act:
  {ML.Mail.want("lost", "", ML.Flags{"", False{}, False{}, False{}}) == "act" : String}

law mail_want_deploy_own:
  {ML.Mail.want("deploy", "", ML.Flags{"", True{}, False{}, False{}}) == "act" : String}

law mail_want_lock_own:
  {ML.Mail.want("lock", "", ML.Flags{"", True{}, False{}, False{}}) == "act" : String}

law mail_want_status_fyi:
  {ML.Mail.want("deploy", "", ML.Flags{"", False{}, False{}, False{}}) == "fyi" : String}

law mail_want_routine_act:
  {ML.Mail.want("routine", "", ML.Flags{"", False{}, False{}, False{}}) == "act" : String}

law mail_want_hook_act:
  {ML.Mail.want("hook", "", ML.Flags{"", False{}, False{}, False{}}) == "act" : String}

law mail_want_direct_act:
  {ML.Mail.want("thread", "", ML.Flags{"", False{}, True{}, False{}}) == "act" : String}

law mail_want_person_act:
  {ML.Mail.want("person", "", ML.Flags{"", False{}, True{}, False{}}) == "act" : String}

law mail_post_idempotent:
  for +b: ML.Box
  for +m: ML.Mail
  {ML.Box.mail_guard(True{}, b, m) == b : ML.Box}

law mail_unread_after_post:
  {ML.Box.unread(ML.Box.mail(ML.Box.empty(), ML.Mail{"m", "c", "person", "bot:a", "thread", "", "act", "hi", 2n, 1n}), "a") == 1n : Nat}

# Adding provenance never changes delivery effects.
law mail_origin_keeps_outputs:
  for +st: H.Step
  for thread: String
  for msg: String
  for origin: ML.Origin
  for at: Nat
  {H.Step.outs_of(H.Origin.step(st, thread, msg, origin, at)) == H.Step.outs_of(st) : List<&2, H.Out>}

# ---- end mail 1 ----
# ---- links (ref) ----

law ref_uri_parse_thread:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.msg("p1~t17-2", "m:é"))) == Some{REF.Ref.msg("p1~t17-2", "m:é")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_bot:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.bot("b1-2"))) == Some{REF.Ref.bot("b1-2")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_room:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.room("r1-2"))) == Some{REF.Ref.room("r1-2")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_person:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.person())) == Some{REF.Ref.person()} : Maybe<&2, REF.Ref>}

law ref_uri_parse_project:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.project("p1-2"))) == Some{REF.Ref.project("p1-2")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_routine:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.routine("o1-2"))) == Some{REF.Ref.routine("o1-2")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_task:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.task("b1-2-3"))) == Some{REF.Ref.task("b1-2-3")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_pr:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.pr("p1-2", "42"))) == Some{REF.Ref.pr("p1-2", "42")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_work:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.work("w1"))) == Some{REF.Ref.work("w1")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_file:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.file("p1-2", ["dir name", "a/b", "é.txt"]))) == Some{REF.Ref.file("p1-2", ["dir name", "a/b", "é.txt"])} : Maybe<&2, REF.Ref>}

law ref_uri_parse_machine:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.machine("p1"))) == Some{REF.Ref.machine("p1")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_conv:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.conv("pair:bot:b1|thread:t2"))) == Some{REF.Ref.conv("pair:bot:b1|thread:t2")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_mail:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.mail("pair:bot:b1|thread:t2", "m5"))) == Some{REF.Ref.mail("pair:bot:b1|thread:t2", "m5")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_mailroom:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.mailroom("*"))) == Some{REF.Ref.mailroom("*")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_decision:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.decision("b1", "d:é"))) == Some{REF.Ref.decision("b1", "d:é")} : Maybe<&2, REF.Ref>}

law ref_uri_parse_charter:
  {REF.Ref.parse(REF.Ref.uri(REF.Ref.charter("b1"))) == Some{REF.Ref.charter("b1")} : Maybe<&2, REF.Ref>}

law ref_web_parse:
  {REF.Ref.parse(REF.Ref.web("http://h:3787", REF.Ref.msg("p1~t17-2", "m:é"))) == Some{REF.Ref.msg("p1~t17-2", "m:é")} : Maybe<&2, REF.Ref>}

law ref_web_parse_slash:
  {REF.Ref.parse(REF.Ref.web("http://h:3787/", REF.Ref.msg("p1~t17-2", "m:é"))) == Some{REF.Ref.msg("p1~t17-2", "m:é")} : Maybe<&2, REF.Ref>}

law ref_addr_roundtrip_thread:
  {REF.Ref.of_addr(REF.Ref.addr(REF.Ref.thread("p1~t17-2"))) == Some{REF.Ref.thread("p1~t17-2")} : Maybe<&2, REF.Ref>}

law ref_addr_roundtrip_bot:
  {REF.Ref.of_addr(REF.Ref.addr(REF.Ref.bot("b1-2"))) == Some{REF.Ref.bot("b1-2")} : Maybe<&2, REF.Ref>}

law ref_addr_roundtrip_room:
  {REF.Ref.of_addr(REF.Ref.addr(REF.Ref.room("r1-2"))) == Some{REF.Ref.room("r1-2")} : Maybe<&2, REF.Ref>}

law ref_addr_roundtrip_person:
  {REF.Ref.of_addr(REF.Ref.addr(REF.Ref.person())) == Some{REF.Ref.person()} : Maybe<&2, REF.Ref>}

law ref_addr_roundtrip_mail:
  {REF.Ref.of_addr(REF.Ref.addr(REF.Ref.mail("pair:bot:b1|thread:t2", "m5"))) == Some{REF.Ref.mail("pair:bot:b1|thread:t2", "m5")} : Maybe<&2, REF.Ref>}

law ref_addr_roundtrip_conv:
  {REF.Ref.of_addr(REF.Ref.addr(REF.Ref.conv("pair:bot:b1|thread:t2"))) == Some{REF.Ref.conv("pair:bot:b1|thread:t2")} : Maybe<&2, REF.Ref>}

law ref_thread_addr:
  for id: String
  {REF.Ref.addr(REF.Ref.thread(id)) == "thread:" ++ id : String}

law ref_pct_safe:
  {REF.Ref.unpct(REF.Ref.pct("AZaz09-._~")) == "AZaz09-._~" : String}

law ref_pct_bytes:
  {REF.Ref.unpct(REF.Ref.pct(":|/ é%")) == ":|/ é%" : String}

law ref_pct_three:
  {REF.Ref.unpct(REF.Ref.pct("€")) == "€" : String}

law ref_pct_four:
  {REF.Ref.unpct(REF.Ref.pct("😀")) == "😀" : String}

law ref_pct_encoding:
  {REF.Ref.pct(":|/ é%") == "%3A%7C%2F%20%C3%A9%25" : String}

law ref_pct_malformed:
  {REF.Ref.unpct("%zz % %A %2z %%41") == "%zz % %A %2z %A" : String}

law ref_unknown_kind:
  {REF.Ref.parse("backplane://nope/x") == None{} : Maybe<&2, REF.Ref>}

law ref_no_script:
  for s: String
  {REF.Ref.parse("javascript:" ++ s) == None{} : Maybe<&2, REF.Ref>}

law ref_legacy_open:
  {REF.Ref.parse("backplane://open?thread=p1~t17-2") == Some{REF.Ref.thread("p1~t17-2")} : Maybe<&2, REF.Ref>}

law ref_bare_thread:
  {REF.Ref.bare("t1-2") == Some{REF.Ref.thread("t1-2")} : Maybe<&2, REF.Ref>}

law ref_bare_bot:
  {REF.Ref.bare("b1-2") == Some{REF.Ref.bot("b1-2")} : Maybe<&2, REF.Ref>}

law ref_bare_task:
  {REF.Ref.bare("b1-2-3") == Some{REF.Ref.task("b1-2-3")} : Maybe<&2, REF.Ref>}

law ref_bare_project:
  {REF.Ref.bare("p1-2") == Some{REF.Ref.project("p1-2")} : Maybe<&2, REF.Ref>}

law ref_bare_room:
  {REF.Ref.bare("r1-2") == Some{REF.Ref.room("r1-2")} : Maybe<&2, REF.Ref>}

law ref_bare_routine:
  {REF.Ref.bare("o1-2") == Some{REF.Ref.routine("o1-2")} : Maybe<&2, REF.Ref>}

law ref_bare_remote:
  {REF.Ref.bare("p1~t17-2") == Some{REF.Ref.thread("p1~t17-2")} : Maybe<&2, REF.Ref>}

law ref_bare_not_short:
  {REF.Ref.bare("t12") == None{} : Maybe<&2, REF.Ref>}

law ref_bare_not_word:
  {REF.Ref.bare("the") == None{} : Maybe<&2, REF.Ref>}

law ref_bare_not_incomplete:
  {REF.Ref.bare("t1-") == None{} : Maybe<&2, REF.Ref>}

law ref_bare_not_unknown:
  {REF.Ref.bare("x1-2") == None{} : Maybe<&2, REF.Ref>}

law link_ref_uri:
  {LK.Link.pieces("see backplane://thread/t1-2 now") == [LK.PText{"see "}, LK.PRef{"backplane://thread/t1-2", "backplane://thread/t1-2", False{}}, LK.PText{" now"}] : List<&2, LK.Piece>}

law link_ref_bare:
  {LK.Link.pieces("see t1-2 now") == [LK.PText{"see "}, LK.PRef{"t1-2", "backplane://thread/t1-2", True{}}, LK.PText{" now"}] : List<&2, LK.Piece>}

law link_ref_md:
  {LK.Link.pieces("[thread](backplane://thread/t1-2)") == [LK.PRef{"thread", "backplane://thread/t1-2", False{}}] : List<&2, LK.Piece>}

law link_ref_plain_unchanged:
  {LK.Link.pieces("a line with no refs") == [LK.PText{"a line with no refs"}] : List<&2, LK.Piece>}

# ---- i18n ----

law i18n_plural_ja_other:
  for n: Nat
  {I18.I18n.plural(I18.Ja{}, n) == I18.POther{} : I18.Plural}

law i18n_plural_en_one:
  {I18.I18n.plural(I18.En{}, 1n) == I18.POne{} : I18.Plural}

law i18n_plural_en_zero:
  {I18.I18n.plural(I18.En{}, 0n) == I18.POther{} : I18.Plural}

law i18n_plural_en_many:
  for k: Nat
  {I18.I18n.plural(I18.En{}, 1n+1n+k) == I18.POther{} : I18.Plural}

law i18n_resolve_override_en:
  for os: String
  {I18.I18n.resolve("en", os) == I18.En{} : I18.Loc}

law i18n_resolve_override_ja:
  for os: String
  {I18.I18n.resolve("ja", os) == I18.Ja{} : I18.Loc}

law i18n_resolve_auto:
  for os: String
  {I18.I18n.resolve("auto", os) == I18.I18n.of_tag(os) : I18.Loc}

law i18n_of_tag_posix:
  {I18.I18n.of_tag("ja_JP.UTF-8") == I18.Ja{} : I18.Loc}

law i18n_of_tag_c:
  {I18.I18n.of_tag("C") == I18.En{} : I18.Loc}

law i18n_of_tag_empty:
  {I18.I18n.of_tag("") == I18.En{} : I18.Loc}

law i18n_of_tag_bcp:
  {I18.I18n.of_tag("ja-JP") == I18.Ja{} : I18.Loc}

law i18n_ago_en_same:
  for +now: Nat
  for +at: Nat
  {I18.I18n.ago(I18.En{}, now, at) == M.Time.ago(now, at) : String}

law i18n_took_en_same:
  for +s: Nat
  {I18.I18n.took(I18.En{}, s) == M.Time.took(s) : String}


law i18n_fallback_fixture_save:
  {I18F.T.common.save(I18.Ja{}) == I18F.T.common.save(I18.En{}) : String}

law i18n_fallback_fixture_plural:
  for +n: Nat
  {I18F.T.files.count(I18.Ja{}, n) == I18F.T.files.count(I18.En{}, n) : String}
# ---- end i18n ----
# ---- usage-screen ----
# The model chip's context tail and Settings > Usage (src/core/usage.bend)

# a provider that never reports its context says so instead of nothing
law usage_tail_na:
  {UZ.Use.tail("codex", 0n, "") == " · ctx n/a" : String}

# Claude shows nothing until its first reading
law usage_tail_claude_waits:
  {UZ.Use.tail("claude", 0n, "") == "" : String}

# a reading's text is shown as it is, whatever the provider
law usage_tail_reading:
  for +p: String
  for +t: String
  {UZ.Use.tail(p, 5n, t) == t : String}

# a reading is what a session holds, said as held, with its share (small
# values: the large ones of a real session are in test/usage_test.bend)
law usage_note_held:
  {UZ.Use.held.note(50n, 200n) == "50 of 200 held (25%)" : String}

law usage_note_no_window:
  {UZ.Use.held.note(5n, 0n) == "5 held, window unknown" : String}

# without a window there is no share, whatever is held
law usage_pct_no_window:
  for +u: Nat
  {UZ.Use.pct(u, 0n) == 0n : Nat}

# Boat's CLI is asked at once when it never was, whenever that is
law usage_boat_due_never_read:
  for +now: Nat
  {UZ.Bj.due("", True{}, now) == True{} : Bool}

# credit-seconds are shown as exact dollars, never rounded
law usage_boat_usd_exact:
  {UZ.Bj.usd(158n) == "$0.00158" : String}
# ---- i18n cjk ----
law cjk_segs_lossless:
  {CJK.Cjk.join(CJK.Cjk.segs("日本語。")) == "日本語。" : String}

law cjk_segs_lossless_mixed:
  {CJK.Cjk.join(CJK.Cjk.segs("PRを開いた。")) == "PRを開いた。" : String}

law cjk_segs_lossless_spaces:
  {CJK.Cjk.join(CJK.Cjk.segs("  あ  い  ")) == "  あ  い  " : String}

law cjk_segs_lossless_empty:
  {CJK.Cjk.join(CJK.Cjk.segs("")) == "" : String}

law cjk_no_start_stop:
  {CJK.Cjk.can_break(12354, 12290) == False{} : Bool}

law cjk_no_end_quote:
  {CJK.Cjk.can_break(12300, 12354) == False{} : Bool}

law cjk_kana_break:
  {CJK.Cjk.can_break(12354, 12356) == True{} : Bool}

law cjk_ascii_no_break:
  {CJK.Cjk.can_break(65, 66) == False{} : Bool}

law cjk_terminal_cells:
  {L.Term.cells("A日本B", 0) == 6 : U32}

law cjk_terminal_combining_cells:
  {L.Term.cells("é日", 0) == 3 : U32}

# ---- end i18n cjk ----

# ---- usage-tokens ----
# Turn usage (model.bend's Tokens, hub.bend's Turn and Lim)

# a result line seen again is not counted twice
law tok_claude_seen_nothing:
  {H.Turn.claude(M.State.replay([M.TurnUsage{"t1", "claude", "m", "r1", 0n, 0n, 0n, 1n, 0n, 0n, "irwo", "", 1n}], M.State.empty()), "t1", 2n,
    J.Json.obj([J.KV{"uuid", J.Str{"r1"}}, J.KV{"usage", J.Json.obj([J.KV{"output_tokens", J.Num{"1"}}])}])) == [] : List<&2, M.Change>}

# a running total that rose gives the rise; one that went down started again
law tok_rise_rose:
  {H.Turn.rise(5n, 3n) == 2n : Nat}

law tok_rise_restarted:
  {H.Turn.rise(3n, 5n) == 3n : Nat}

# a Codex session that started again counts its totals as they are
law tok_codex_restart_whole:
  for +was: List<&2, String>
  for +i: Nat
  for +v: Nat
  {H.Turn.codex.d(False{}, was, i, v) == v : Nat}

# every turn adds one turn to a total, whatever it used
law tok_add_one_turn:
  {M.Tok.part(M.Tok.add("2/0/0/0/0/0/0/0", 1n, 0n, 0n, 1n, 0n, 0n, False{}), 0n) == 3n : Nat}

# a turn with no cost reported adds nothing to the turns with a cost
law tok_add_no_cost:
  {M.Tok.part(M.Tok.add("", 1n, 0n, 0n, 1n, 0n, 0n, False{}), 7n) == 0n : Nat}

# ---- usage-boat-cap ----
# without a shared cap in the status, the spend row says nothing about one
law usage_boat_no_cap_nothing:
  {UZ.Bj.cap.of(J.Json.obj([])) == "" : String}
# ---- agent failures: stderr ----
# an agent that ends before its turn did is reported with its exit code or
# signal and the end of its stderr, with anything secret-shaped dropped
law errtail_scrub_token:
  {ERT.Err.scrub("GET http://h/mcp/t1?token=ab12cd done") == "GET http://h/mcp/t1?token=… done" : String}

law errtail_scrub_words:
  {ERT.Err.scrub("auth sk-AB12cd and task-list ghp_x9 Bearer q.w") == "auth sk-… and task-list ghp_… Bearer …" : String}

law errtail_say_exit:
  {ERT.Exit.say("codex exited before the turn ended", "", 1, "Error: boom\n") == "codex exited before the turn ended (exit 1): Error: boom" : String}

law errtail_say_signal:
  {ERT.Exit.say("x", "", 143, "  ") == "x (signal 15)" : String}
# ---- end agent failures: stderr ----
# ---- agent failures: the login keyring ----
# a provider that reads credentials from the login keyring is never started
# while it is locked; unknown (no bus) or unlocked starts it; one that does
# not need it always starts; a failure while it is locked says so
law keyring_locked_never_starts:
  {KRG.Keyring.start("codex", 2n) == False{} : Bool}

law keyring_unknown_or_open_starts:
  {Bool.and(Bool.and(KRG.Keyring.start("codex", 0n), KRG.Keyring.start("codex", 1n)), KRG.Keyring.start("claude", 2n)) == True{} : Bool}

law keyring_answers:
  {Bool.and(Bool.and(Nat.is_eq(KRG.Keyring.of("b true\n"), 2n), Nat.is_eq(KRG.Keyring.of("b false"), 1n)), Nat.is_eq(KRG.Keyring.of(""), 0n)) == True{} : Bool}

law keyring_hint_only_locked:
  {Bool.and(String.contains(KRG.Keyring.say("x", 2n), "keyring is locked"), String.eq(KRG.Keyring.say("x", 1n), "x")) == True{} : Bool}
# ---- end agent failures: the login keyring ----
# ---- riding mail ----
# a message for a running Codex turn is posted at once (nothing waits in
# the UI) and rides the turn's next tool result: the turn is not stopped
law ride_running_no_restart:
  {H.Step.outs_of(H.Hub.post(H.Demo.state("codex", M.Running{}), 5n, "t1", "m1", M.User{}, "hi")) == [] : List<&2, H.Out>}

law ride_posted_at_once:
  {List.head(&2, M.Change, H.Queue.changes(H.Hub.post(H.Demo.state("codex", M.Running{}), 5n, "t1", "m1", M.User{}, "hi"))) == Some{M.MessagePosted{"t1", "m1", M.User{}, "hi", 5n}} : Maybe<&2, M.Change>}

# what waits is a setting in the log, so a restart finds it again
law ride_kept:
  {RD.Ride.input(H.Ride.of(H.Ride.demo(), "t1")) == "hi" : String}

# a tool result carries what waits, and the same step empties it, so it is
# delivered at most once: a result for a thread with nothing waiting is
# left as it was
law ride_carries_block:
  {String.contains(MC.McpOut.reply(BHS.Ride.mcp(MC.McpOut{"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}]}}", H.Step.none(), ""}, H.Ride.demo(), 6n, "t1")), "Incoming messages") == True{} : Bool}

law ride_taken_empties:
  {H.Ride.of(M.State.replay(H.Ride.taken(H.Ride.demo(), 6n, "t1"), H.Ride.demo()), "t1") == "" : String}

law ride_nothing_waiting_same:
  for o: MC.McpOut
  for +s: M.State
  for +now: Nat
  for +th: String
  for h: {H.Ride.of(s, th) == "" : String}
  {BHS.Ride.mcp(o, s, now, th) == o : MC.McpOut}

# never lost: a turn that ends (done, failed, stopped, or cut short by a
# restart) with mail waiting starts the next at once with it, emptying it
law ride_turn_end_starts:
  {List.last(&2, M.Change, H.Queue.changes(H.Ride.after(H.Ride.demo(), 6n, H.Step{[M.TurnChanged{"t1", M.Completed{}, 6n}], Nil{}}))) == Some{M.TurnChanged{"t1", M.Running{}, 6n}} : Maybe<&2, M.Change>}

law ride_interrupted_starts:
  {List.last(&2, M.Change, H.Queue.changes(H.Ride.after(H.Ride.demo(), 6n, H.Step{[M.TurnChanged{"t1", M.Interrupted{}, 6n}], Nil{}}))) == Some{M.TurnChanged{"t1", M.Running{}, 6n}} : Maybe<&2, M.Change>}

law ride_turn_end_empties:
  {H.Ride.of(M.State.replay(H.Queue.changes(H.Ride.after(H.Ride.demo(), 6n, H.Step{[M.TurnChanged{"t1", M.Failed{}, 6n}], Nil{}})), H.Ride.demo()), "t1") == "" : String}

# a step that ends no turn starts nothing
law ride_after_running_same:
  for +s: M.State
  for +now: Nat
  for +th: String
  for os: List<&2, H.Out>
  {H.Ride.after(s, now, H.Step{[M.TurnChanged{th, M.Running{}, now}], os}) == H.Step{[M.TurnChanged{th, M.Running{}, now}], os} : H.Step}

# delivery holds nothing back on the restart (steer) path either
law deliver_held_restart:
  for b1: Bool
  for b2: Bool
  for b3: Bool
  for +th: String
  for +id: String
  for +now: Nat
  for +v: String
  for +r: String
  {H.Deliver.held(List.append(&2, M.Change, List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, b1, [], [M.QueueTaken{th, now}]), Bool.pick(List<&2, M.Change>, b3, [], H.Ride.done(v, now, th, r))), List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, b2, [], [M.ActivityLogged{th, id, "info", "steer", "Steered: the turn starts again with this message", now}]), [M.TurnChanged{th, M.Running{}, now}]))) == False{} : Bool}
# ---- end riding mail ----
# ---- resume after a restart ----
# a per-turn process (Codex, Grok, Muse, Antigravity: recorded with no
# descriptor, "-1" read as 0, never handed off) that still runs after a
# restart is stopped, never adopted, whatever adoption allows, so its
# thread never has two processes on one session
law resume_turn_process_stopped:
  {AD.Adopt.stopped(AD.Adopt.decide([AD.AEntry{"t", 42n, "b:1", 0n, False{}, "", "", CM.Cmds.new()}], ["b:1"], True{})) == [42n] : List<&2, Nat>}

law resume_turn_process_not_kept:
  {AD.Adopt.kept(AD.Adopt.decide([AD.AEntry{"t", 42n, "b:1", 0n, False{}, "", "", CM.Cmds.new()}], ["b:1"], True{})) == [] : List<&2, String>}

# the cut turn is told that a tool call in flight lost its result, and the
# words are still the continue message (a turn that brings the hub down
# never loops: Carry.is)
law resume_carry_tells_cut:
  {Bool.and(H.Carry.is(M.Carry.text() ++ " " ++ H.Carry.cut()), String.contains(H.Carry.cut(), "its result was lost")) == True{} : Bool}
# ---- end resume after a restart ----
# ---- deploy waits for MCP calls ----
# a request on the MCP route (any provider's) is a tool call in flight: a
# deploy never restarts while one is open, goes once its answer is written,
# and the caller's own deploy_* call never holds it (no self-deadlock)
law deploy_mcp_call_waits:
  {H.Deploy.demo.go(H.Deploy.check(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), 9n, True{}, DP.Open.mcp(Nil{}, "t1", "c1", "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"thread_wait\",\"arguments\":{}}}"))) == 0n : Nat}

law deploy_mcp_answered_goes:
  {H.Deploy.demo.go(H.Deploy.check(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), 9n, True{}, DP.Open.mcp.shut(DP.Open.mcp(Nil{}, "t1", "c1", "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"thread_wait\",\"arguments\":{}}}"), "t1", "c1"))) == 1n : Nat}

law deploy_mcp_own_call_not_counted:
  {H.Deploy.demo.go(H.Deploy.check(H.Deploy.demo(False{}, H.Deploy.demo.line("t0")), 9n, True{}, DP.Open.mcp(Nil{}, "t0", "c1", "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"deploy_restart\",\"arguments\":{}}}"))) == 1n : Nat}

# a turn's end does not shut a request still waiting for its answer
law deploy_mcp_outlives_turn_end:
  {DP.Open.count(DP.Open.gone(DP.Open.mcp(DP.Open.add(Nil{}, "t1", "x"), "t1", "c1", "{\"method\":\"tools/call\",\"params\":{\"name\":\"thread_wait\"}}"), "t1")) == 1n : Nat}
# ---- end deploy waits for MCP calls ----
# ---- i18n ime ----

law ime_preedit_not_in_draft:
  {IMEL.Ime.shown(IMEL.Ime.none(), "ab", 1n) == "ab" : String}

law ime_shown_at_caret:
  {IMEL.Ime.shown(IMEL.Ime.draw(IMEL.Ime.none(), "o", 0n, 0n, "にほんご", "2222"), "abcd", 2n) == "abにほんごcd" : String}

law ime_range_replace_chars:
  {IMEL.Ime.text(IMEL.Ime.draw(IMEL.Ime.draw(IMEL.Ime.none(), "o", 0n, 0n, "にほんご", "2222"), "o", 1n, 2n, "ホン", "11")) == "にホンご" : String}

law ime_delete_range:
  {IMEL.Ime.text(IMEL.Ime.draw(IMEL.Ime.draw(IMEL.Ime.none(), "o", 0n, 0n, "にほんご", "2222"), "o", 1n, 2n, "", "")) == "にご" : String}

law ime_wide_counts_once:
  {IMEL.Ime.len(IMEL.Ime.draw(IMEL.Ime.none(), "o", 0n, 0n, "a😀日", "012")) == 3n : Nat}

law ime_clear_is_off:
  {IMEL.Ime.on(IMEL.Ime.clear(IMEL.Ime.draw(IMEL.Ime.none(), "o", 0n, 0n, "か", "2"))) == False{} : Bool}

law ime_focus_change_drops:
  {IMEL.Ime.on(IMEL.Ime.sync(IMEL.Ime.draw(IMEL.Ime.none(), "t:composer", 0n, 0n, "か", "2"), "t:other")) == False{} : Bool}

law ime_owner_keeps:
  {IMEL.Ime.on(IMEL.Ime.sync(IMEL.Ime.draw(IMEL.Ime.none(), "t:composer", 0n, 0n, "か", "2"), "t:composer")) == True{} : Bool}

law ime_other_owner_never_merges:
  {IMEL.Ime.text(IMEL.Ime.draw(IMEL.Ime.draw(IMEL.Ime.none(), "a", 0n, 0n, "か", "2"), "b", 1n, 0n, "き", "2")) == "き" : String}

law ime_unknown_event_not_quit:
  {WINL.Win.quit(WINL.Win.ev(99, 0, 0, 0, 0)) == False{} : Bool}

law ime_unknown_event_is_junk:
  {WINL.Win.junk(WINL.Win.ev(99, 0, 0, 0, 0)) == True{} : Bool}

law ime_quit_is_quit:
  {WINL.Win.quit(WINL.Win.ev(6, 0, 0, 0, 0)) == True{} : Bool}

law ime_commit_event_id:
  {WINL.Win.junk(WINL.Win.ev(14, 7, 0, 0, 0)) == False{} : Bool}

# ---- end i18n ime ----
# ---- mail 2a ----

# JSON escaping's no-interior-newline property is exercised on long,
# adversarial strings in mailbody_test; this law fixes framing exactly.
law body_line_terminated:
  for +b: MB.Body
  {MB.Body.line(b) == J.Json.show(MB.Body.encode(b)) ++ "\n" : String}

# A useful structural no-body property: changing text cannot change any
# metadata/header Change, for arbitrary conversation, header and texts.
law body_header_omits_text:
  for +c: ML.Conv
  for +m: ML.Mail
  for a: String
  for b: String
  {MB.Body.changes(MB.Body{c, m, a}) == MB.Body.changes(MB.Body{c, m, b}) : List<&2, M.Change>}

law recover_seen_empty:
  for +b: MB.Body
  for +box: ML.Box
  for seen: {ML.Box.has(box, ML.Mail.id(MB.Body.mail(b))) == True{} : Bool}
  {MB.Body.recover(b, box) == Nil{} : List<&2, M.Change>}

law recover_missing_header:
  for +c: ML.Conv
  for +m: ML.Mail
  for text: String
  {MB.Body.recover.new(True{}, MB.Body{c, m, text}) == [MB.Body.metadata(c), MB.Body.header(m)] : List<&2, M.Change>}

law body_path_encoded_segment:
  for +home: String
  for +conv: String
  {MB.Body.path(home, conv) == home ++ "/mail/" ++ REF.Ref.pct(conv) ++ ".jsonl" : String}
# ---- links (cards, trail) ----

def RefView.state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Project", "/p", 1n}, M.ThreadCreated{"t", "p", "Title", M.Local{}, "", "", "claude", "", 2n}], M.State.empty())

def RefView.cached() -> RC.Cards:
  RC.Cards.result.cards(RC.Cards.get(RC.Cards.empty(), RefView.state(), "backplane://thread/t"))

def RefView.fresh(+ch: M.Change) -> Maybe<&2, RC.Card>:
  RC.Cards.result.value(RC.Cards.get(RC.Cards.drop(RefView.cached(), RC.Cards.touched(ch)), M.State.apply(RefView.state(), ch), "backplane://thread/t"))

law cards_get_fresh:
  {RefView.fresh(M.ThreadRenamed{"t", "New", 3n}) == RC.Card.of(M.State.apply(RefView.state(), M.ThreadRenamed{"t", "New", 3n}), REF.Ref.thread("t")) : Maybe<&2, RC.Card>}

law cards_get_fresh_settle:
  {RefView.fresh(M.ThreadSettled{"t", True{}, 3n}) == RC.Card.of(M.State.apply(RefView.state(), M.ThreadSettled{"t", True{}, 3n}), REF.Ref.thread("t")) : Maybe<&2, RC.Card>}

law cards_get_fresh_start:
  {RefView.fresh(M.TurnChanged{"t", M.Running{}, 3n}) == RC.Card.of(M.State.apply(RefView.state(), M.TurnChanged{"t", M.Running{}, 3n}), REF.Ref.thread("t")) : Maybe<&2, RC.Card>}

law cards_get_fresh_end:
  {RefView.fresh(M.TurnChanged{"t", M.Completed{}, 3n}) == RC.Card.of(M.State.apply(RefView.state(), M.TurnChanged{"t", M.Completed{}, 3n}), REF.Ref.thread("t")) : Maybe<&2, RC.Card>}

law cards_get_fresh_delete:
  {RefView.fresh(M.ThreadDeleted{"t", 3n}) == None{} : Maybe<&2, RC.Card>}

law cards_get_fresh_project:
  {RefView.fresh(M.ProjectCreated{"p", "Renamed", "/p", 3n}) == RC.Card.of(M.State.apply(RefView.state(), M.ProjectCreated{"p", "Renamed", "/p", 3n}), REF.Ref.thread("t")) : Maybe<&2, RC.Card>}

law cards_peek_after_get:
  {RC.Cards.peek(RefView.cached(), "backplane://thread/t") == RC.Card.of(RefView.state(), REF.Ref.thread("t")) : Maybe<&2, RC.Card>}

law cards_drop_other_kept:
  {RC.Cards.peek(RC.Cards.drop(RefView.cached(), ["other"]), "backplane://thread/t") == RC.Cards.peek(RefView.cached(), "backplane://thread/t") : Maybe<&2, RC.Card>}

law trail_go_back:
  for +bs: List<&2, String>
  for +fs: List<&2, String>
  {RC.Trail.back(RC.Trail.go(RC.Trail{bs, "a", fs}, "b")) == RC.Trail{List.take(&2, String, bs, 49n), "a", ["b"]} : RC.Trail}

law trail_back_fwd:
  for +bs: List<&2, String>
  for +b: String
  for +c: String
  for +fs: List<&2, String>
  {RC.Trail.fwd(RC.Trail.back(RC.Trail{b <> bs, c, fs})) == RC.Trail{b <> bs, c, fs} : RC.Trail}

law trail_go_same:
  for +bs: List<&2, String>
  for +fs: List<&2, String>
  {RC.Trail.go(RC.Trail{bs, "a", fs}, "a") == RC.Trail{bs, "a", fs} : RC.Trail}

law trail_go_clears_fwd:
  for +bs: List<&2, String>
  for fs: List<&2, String>
  {RC.Trail.go(RC.Trail{bs, "a", fs}, "b") == RC.Trail{List.take(&2, String, "a" <> bs, 50n), "b", Nil{}} : RC.Trail}

def RefView.repeat(n: Nat) -> List<&2, String>:
  match n:
    case 0n: Nil{}
    case 1n+k: "x" <> RefView.repeat(k)

def RefView.back_count(t: RC.Trail) -> Nat:
  match t:
    case RC.Trail{bs, _, _}: List.length(&2, String, bs)

law trail_cap:
  {RefView.back_count(RC.Trail.go(RC.Trail{RefView.repeat(75n), "a", ["z"]}, "b")) == 50n : Nat}

law card_file_kind_shared:
  {RC.Card.of(RefView.state(), REF.Ref.file("p", ["x.kicad_pcb"])) == Some{RC.Card{"backplane://file/p/x.kicad_pcb", "file", "file", "x.kicad_pcb", "Project", "board", ""}} : Maybe<&2, RC.Card>}

def RefView.entries(es: List<&2, M.Entry>, st: M.State) -> M.State:
  match st:
    case M.State{ps, ts, _, n, tk, sg, qu, ak, dv, nv, bw}: M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw}

law card_thread_no_timeline:
  for es: List<&2, M.Entry>
  {RC.Card.of(RefView.entries(es, RefView.state()), REF.Ref.thread("t")) == RC.Card.of(RefView.state(), REF.Ref.thread("t")) : Maybe<&2, RC.Card>}

law card_github_pr_no_state:
  for st: M.State
  {RC.Card.of(st, REF.Ref.pr("i2cjak/Backplane", "128")) == Some{RC.Card{REF.Ref.uri(REF.Ref.pr("i2cjak/Backplane", "128")), "pr", "git-fork", "PR #128", "i2cjak/Backplane", "", ""}} : Maybe<&2, RC.Card>}

law card_person_no_state:
  for st: M.State
  {RC.Card.of(st, REF.Ref.person()) == Some{RC.Card{"backplane://person", "person", "circle-check", "You", "", "", ""}} : Maybe<&2, RC.Card>}

# a task card's status reads as the board's own
law card_task_status_agrees:
  {String.join([RC.Card.task.status(" Doing"), RC.Card.task.status("blocked"), RC.Card.task.status("todo"), RC.Card.task.status("DONE "), RC.Card.task.status("dropped"), RC.Card.task.status("later"), RC.Card.task.status("")], ",")
    == String.join([TBD.Tb.status.norm(" Doing"), TBD.Tb.status.norm("blocked"), TBD.Tb.status.norm("todo"), TBD.Tb.status.norm("DONE "), TBD.Tb.status.norm("dropped"), TBD.Tb.status.norm("later"), TBD.Tb.status.norm("")], ",") : String}

# Ordering (core/order.bend): Buy it shows only when there is something
# to order and the order server answered (or estimates are off); its
# words carry the price; an intent sent back from Edit is the text edited
law order_buy_needs_parts:
  {ORD.Card.buy(ORD.Card{"g", "i", "from $5.00", 0n, 0n, "", True{}}) == "" : String}

law order_buy_priced:
  {ORD.Card.buy(ORD.Card{"g", "", "from $5.00", 2n, 0n, "", True{}}) == "Buy it · from $5.00" : String}

law order_buy_unreachable_hidden:
  {ORD.Card.buy(ORD.Card{"g", "", "", 2n, 0n, "the order server is not reachable", True{}}) == "" : String}

law order_buy_estimates_off:
  {ORD.Card.buy(ORD.Card{"", "", "", 1n, 0n, "", False{}}) == "Buy it" : String}

# a project with a board but no fab files yet can still be bought (the buy
# thread writes them), unless the order server is out of reach
law order_buy_board_only:
  {ORD.Card.buy(ORD.Card{"", "", "", 0n, 1n, "", True{}}) == "Buy it" : String}

law order_buy_board_unreachable:
  {ORD.Card.buy(ORD.Card{"", "", "", 0n, 1n, "the order server is not reachable", True{}}) == "" : String}

law intent_edit_round_trip:
  {ORD.Intent.body(ORD.Intent.edit("# Goal\nA lamp")) == "# Goal\nA lamp" : String}

law intent_edit_is_intent:
  {ORD.Intent.is(ORD.Intent.edit("")) == True{} : Bool}

# only a buy thread's agent has the order tools
law order_tools_buy_only:
  {BHS.Bots.buy.tools(False{}) == [] : List<&2, J.Json>}

# the line beside Buy it above the message box names what there is to order,
# says "the board" for a board with no fab files yet, and is not there
# while the agent works
law order_ready_names:
  {ORD.Card.ready.k("LED board · Lamp shade", 0n) == "Ready to order · LED board · Lamp shade" : String}

law order_ready_board:
  {ORD.Card.ready.k("", 1n) == "Ready to order · the board" : String}

law order_ready_nothing:
  {ORD.Card.ready.k("", 0n) == "" : String}

law fab_ready_running_hidden:
  for line: String
  {U.Fab.ready.k("Buy it", line, True{}) == "" : String}

law fab_ready_needs_buy:
  for line: String
  {U.Fab.ready.k("", line, False{}) == "" : String}

# a manufacturing review is current only for the design it was recorded
# with (its fingerprint); a verdict is ok or issues, nothing else
law order_review_none:
  {ORD.Order.review.state("f1", J.Json.obj([])) == "none" : String}

law order_review_current:
  {ORD.Order.review.state("f1", ORD.Order.review.rec("f1", "ok", "fine", 5n)) == "current" : String}

law order_review_stale:
  {ORD.Order.review.state("f2", ORD.Order.review.rec("f1", "ok", "fine", 5n)) == "stale" : String}

law order_review_verdict_closed:
  {ORD.Order.review.verdict("looks great") == "issues" : String}

# the viewer's board picker (client.bend's Bd): a board is named by its
# folder, its schematic is beside it, and it is lit while the viewer shows
# a file in its folder
law bd_name_folder:
  {U.Bd.name("boards/led-board/led-board.kicad_pcb") == "led-board" : String}

law bd_name_root:
  {U.Bd.name("main.kicad_pcb") == "main" : String}

law bd_schematic_beside:
  {U.Bd.path("schematic", "boards/a/a.kicad_pcb") == "boards/a/a.kicad_sch" : String}

law bd_board_itself:
  {U.Bd.path("board", "boards/a/a.kicad_pcb") == "boards/a/a.kicad_pcb" : String}

law bd_on_sheet:
  {U.Bd.on("/p/boards/a/power.kicad_sch", "boards/a/a.kicad_pcb") == True{} : Bool}

law bd_off_other:
  {U.Bd.on("/p/boards/b/b.kicad_pcb", "boards/a/a.kicad_pcb") == False{} : Bool}

# another tab stays on the board picked (its schematic beside it); with
# none picked, the project's own
law bd_keep_schematic:
  {U.Bd.keep.k("boards/a/a.kicad_pcb", "schematic", "/p") == "/p/boards/a/a.kicad_sch" : String}

law bd_keep_none:
  for kind: String
  {U.Bd.keep.k("", kind, "/p") == "" : String}

# with none picked, the project's own board's chip is the one lit
law bd_shown_default:
  {U.Bd.shown.k("", "/p", "boards/a/a.kicad_pcb") == "/p/boards/a/a.kicad_pcb" : String}

law bd_shown_picked:
  for root: String
  for own: String
  {U.Bd.shown.k("/x.kicad_pcb", root, own) == "/x.kicad_pcb" : String}

# a project's second buy thread is numbered, the first is not
law buy_title_first:
  for t: String
  {H.Buy.title.k(t, 0n) == t : String}

law buy_title_second:
  {H.Buy.title.k("Buy: lamp", 1n) == "Buy: lamp · 2" : String}

# once something of the project is paid for, the Ready bar says so and its
# button reads Buy again (a quote alone changes nothing)
law order_ordered_paid:
  {ORD.Card.ready.or("Ready to order · Lamp", "paid") == "Ordered · paid, being placed" : String}

law order_ordered_quote_only:
  for line: String
  {ORD.Card.ready.or(line, "ready") == line : String}

law fab_buy_again:
  {U.Fab.ready.btn.k(True{}, "Buy it · from $5.00") == "Buy again" : String}

law fab_buy_again_off:
  {U.Fab.ready.btn.k(True{}, "") == "" : String}

# the Ready bar's ×: put away per project (setting fab.min.<project>), and
# then a Buy it in the header (or Ordered, once paid) brings it back
law fab_min_key:
  {U.Fab.min.key("p1") == "fab.min.p1" : String}

law fab_min_hides_bar:
  for line: String
  {U.Fab.ready.shown.k(True{}, line) == "" : String}

law fab_min_keeps_bar:
  for line: String
  {U.Fab.ready.shown.k(False{}, line) == line : String}

law fab_min_header:
  {U.Fab.mini.k(True{}, "Ready to order · Lamp") == True{} : Bool}

law fab_min_header_shown_bar:
  for line: String
  {U.Fab.mini.k(False{}, line) == False{} : Bool}

law fab_min_header_nothing:
  {U.Fab.mini.k(True{}, "") == False{} : Bool}

law fab_min_header_ordered:
  {U.Fab.mini.label(True{}) == "Ordered" : String}

# the order server: the setting when given, the default when it is empty
law buy_server_default:
  {H.Buy.server.k("") == ORD.Order.server() : String}

law buy_server_set:
  {H.Buy.server.k(" https://fab.example ") == "https://fab.example" : String}

# an order's progress alerts the person in plain words: shipped with its
# tracking, and nothing for an expired quote
law order_note_shipped:
  {ORD.Order.status.note("shipped", "1Z999") == "Shipped. Tracking: 1Z999" : String}

law order_note_expired:
  for t: String
  {ORD.Order.status.note("expired", t) == "" : String}

# a Stripe checkout that expired unpaid leaves the quote as it was: still
# payable, then expired (not an order the watcher follows for ever)
law order_state_cancelled:
  for q: String
  {ORD.Order.state.k("cancelled", q) == q : String}

law order_state_paid:
  {ORD.Order.state.k("paid", "ready") == "paid" : String}

# ordering is off until the person turns it on (order.enabled): no Buy it
# anywhere, no order tools for any agent, even in a buy thread
law fab_buy_off_hidden:
  for label: String
  {U.Fab.buy.k(True{}, label) == "" : String}

law order_off_no_tools:
  for buy: Bool
  {BHS.Bots.order.tools(False{}, buy) == [] : List<&2, J.Json>}

law order_off_by_default:
  {ORD.Order.enabled("") == False{} : Bool}

# a review's issues: their lines trimmed, blank ones and non-strings left out
law order_review_issues_kept:
  {ORD.Order.review.lines([J.Str{" Shade: walls 0.6 mm "}, J.Str{"  "}, J.Num{"1"}, J.Str{"Base: hole too small"}], 8n) == [J.Str{"Shade: walls 0.6 mm"}, J.Str{"Base: hole too small"}] : List<&2, J.Json>}

law order_review_issues_at_most:
  {List.length(&2, J.Json, ORD.Order.review.lines([J.Str{"a"}, J.Str{"b"}, J.Str{"c"}], 2n)) == 2n : Nat}

# an order's question parks nothing, so a Stop or a crash leaves it open
# (its answer starts the agent again); an order's approval still closes
law order_ask_survives_retire:
  {H.Retire.hit(M.Ask{"a3", "t1", "input", "order_ask", "Fix these?", "", True{}, ""}, "t1") == False{} : Bool}

law order_approval_retired:
  {H.Retire.hit(M.Ask{"a4", "t1", "approval", "order_quote", "Send?", "", True{}, ""}, "t1") == True{} : Bool}

# the review gate: a current, passed review goes on; none or stale asks for
# a review; current with issues shows them and asks whether to fix them
law order_gate_passed:
  {ORD.Order.gate.k("current", "ok") == 0n : Nat}

law order_gate_stale:
  for v: String
  {ORD.Order.gate.k("stale", v) == 1n : Nat}

law order_gate_none:
  for v: String
  {ORD.Order.gate.k("none", v) == 1n : Nat}

law order_gate_issues:
  {ORD.Order.gate.k("current", "issues") == 2n : Nat}

# the pilot: a paid test checkout is "pilot", said as such, and not watched
law order_pilot_paid:
  {ORD.Order.state.pilot(True{}, "paid") == "pilot" : String}

law order_pilot_off:
  for st: String
  {ORD.Order.state.pilot(False{}, st) == st : String}

law order_pilot_done:
  {ORD.Order.done("pilot") == True{} : Bool}

law order_pilot_said:
  {String.contains(ORD.Order.status.text("pilot", ""), "your card was not charged") == True{} : Bool}

law order_pilot_quiet:
  {ORD.Order.status.note("pilot", "") == "" : String}

# the issues as bullets with the review's link: the recorded ones, else the
# summary cut at its semicolons
law order_gate_lists_issues:
  {ORD.Order.gate.md("x", [J.Str{"Shade: walls 0.6 mm"}, J.Str{"Knob: bore too tight."}]) == "**The manufacturing review found what to fix before ordering:**\n\n- Shade: walls 0.6 mm\n- Knob: bore too tight\n\nDetails: `.backplane/review.md`" : String}

law order_gate_cuts_summary:
  {ORD.Order.gate.md("8 BOM lines lack part numbers; knob bore too tight.", []) == "**The manufacturing review found what to fix before ordering:**\n\n- 8 BOM lines lack part numbers\n- knob bore too tight\n\nDetails: `.backplane/review.md`" : String}

# an order's question alerts in its own words, without the tool's name
law order_ask_alert_plain:
  for q: String
  {NO.Alert.ask.say("order_ask", q) == q : String}

# a tool call with only a question (order_ask) is summed up by it, not by
# its name again (the phone's running line: "<tool> <summary>")
law tool_summary_question:
  {H.Claude.tool_summary("mcp__backplane__order_ask", J.Json.obj([J.KV{"question", J.Str{"Ship where?"}}])) == "Ship where?" : String}

# with ordering off, an agent asked to order points to the setting
law order_off_says_where:
  {String.contains(ORD.Order.prompt.project.k(False{}), "Settings → Ordering") == True{} : Bool}

# ---- i18n lang ----
law lang_default_auto:
  {U.Lang.resolve("", "en_US.UTF-8", False{}) == "en" : String}
law lang_default_unknown:
  {U.Lang.resolve("garbage", "en_US.UTF-8", False{}) == "en" : String}
law lang_gate:
  for os: String
  {U.Lang.resolve("ja", os, False{}) == "en" : String}
law lang_gate_preview:
  for os: String
  {U.Lang.resolve("ja", os, True{}) == "ja" : String}
law lang_auto_preview:
  {U.Lang.resolve("auto", "ja_JP.UTF-8", True{}) == "ja" : String}
law lang_override_en:
  for os: String
  for p: Bool
  {U.Lang.resolve("en", os, p) == "en" : String}
law prompt_lang_off:
  {PM.Prompt.lang("off") == "" : String}
law prompt_lang_unknown:
  {PM.Prompt.lang("x") == "" : String}
# ---- end i18n lang ----

# ---- mail view ----
# the rows built for the visible window never run past the list
law mv_visible_bounds:
  for +total: Nat
  for +scroll: Nat
  for +view: Nat
  {Nat.is_ge(total, Nat.add(MVW.Mv.count(total, scroll, view), MVW.Mv.first(total, scroll, view))) == True{} : Bool}

# merging full-text hits into local ones never repeats a conversation
# (a closed case: local and server both repeat an id, and share one)
law mv_merge_no_dup:
  {MVW.Mv.unique(MVW.Mv.merge_hits([MVW.Mv.stub("a", 9n), MVW.Mv.stub("b", 7n), MVW.Mv.stub("a", 9n)], [MVW.Mv.stub("b", 8n), MVW.Mv.stub("d", 6n), MVW.Mv.stub("a", 9n)])) == True{} : Bool}

# a newer page for the same slots leaves the ids of rows it did not change
law mv_rows_stable:
  {MVW.Mv.ids(MVW.Mv.rows(MVW.Mv.put_page(MVW.Mv.put_page(MVW.Mv.cache(), MVW.Mv.filter(), 0n, 3n, [MVW.Mv.stub("a", 9n), MVW.Mv.stub("b", 7n), MVW.Mv.stub("c", 5n)]), MVW.Mv.filter(), 0n, 3n, [MVW.Mv.stub("a", 9n), MVW.Mv.stub("b", 11n), MVW.Mv.stub("c", 5n)]), MVW.Mv.key(MVW.Mv.filter()), 0n, 3n)) == "a,b,c," : String}

# two chips that differ never share a cache key, even with a "|" in a name
law mv_filter_key_separates:
  {String.eq(MVW.Mv.key(MVW.Filter{"a|b", "", "c", ""}), MVW.Mv.key(MVW.Filter{"a", "|b", "c", ""})) == False{} : Bool}

# ---- end mail view ----

# ---- usage-baseline ----
# a turn is never charged with what its process or session did before the
# hub saw it start

# a Claude process already running when its thread was first seen: cost unknown
law tok_cost_unknown_without_start:
  {H.Turn.cost.known(True{}, False{}, "") == False{} : Bool}

# a turn that starts its process is charged from 0, whatever came before
law tok_cost_known_at_start:
  for +b: String
  {H.Turn.cost.known(True{}, True{}, b) == True{} : Bool}

# a Codex session the hub sees start counts from 0
law tok_codex_start_zero:
  {H.Turn.codex.start(False{}, "t", "s", 1n) == [M.SettingSet{"tok.basis.t", "x:s:0/0/0/0/0", 1n}] : List<&2, M.Change>}

# counting starts again only once
law tok_epoch_once:
  {H.Turn.epoch(M.State.replay([M.SettingSet{"tok.epoch", "1", 1n}], M.State.empty()), 2n) == [] : List<&2, M.Change>}

# totals of a count live under its epoch
law tok_pre_epoch:
  {M.Tok.pre.k("1") == "tok.1." : String}
# ---- mail index ----
# Position participates in validation, independently of the ID fingerprint.
law mail_index_wrong_position:
  {MIX.MI.row.ok(MIX.MIndexRow{0, 16, 0, 0, 65536, 0, 65536}, 65600) == False{} : Bool}

law mail_index_forward_chain_rejected:
  {MIX.MI.row.ok(MIX.MIndexRow{0, 16, 0, 65664, 65536, 0, 65600}, 65600) == False{} : Bool}

law mail_index_overflow_rejected:
  {MIX.MI.row.ok(MIX.MIndexRow{4294967290, 16, 0, 0, 65536, 0, 65536}, 65536) == False{} : Bool}

law mail_index_hash_empty:
  for h: U32
  {MIX.MI.hash("", h) == h : U32}
# ---- end mail index ----
# ---- scroll-stable: reading while a thread grows ----
# a view at its very end follows it
law hold_at_end_zero:
  {U.Hold.at_end(0n) == True{} : Bool}

# nothing new: no pill
law hold_label_nothing:
  {U.Hold.label(0n, False{}) == "" : String}

# the entry seen is the newest: nothing is counted as new
law hold_fresh_seen:
  for +e: M.Entry
  for +t: List<&2, M.Entry>
  for +n: Nat
  {U.Hold.fresh(e <> t, U.Hold.stop(e <> t, M.Idx.id(e), n), M.Idx.id(e), n) == n : Nat}

# a panel above the window's timeline moving its top from t0 to t1 moves
# what is read by as much (off + t1 - t0), so nothing read moves on screen
law hold_off_still:
  for +off: Nat
  for +t0: Nat
  for +t1: Nat
  for +over: Nat
  for h: {Nat.is_le(Nat.sub(Nat.add(off, t1), t0), over) == True{} : Bool}
  {U.Hold.off(off, t0, t1, over) == Nat.sub(Nat.add(off, t1), t0) : Nat}

# the window's timeline at its end (sc 0) never holds: it follows
law hold_end_follows:
  for +hold: String
  for +off: U32
  for +top: U32
  for +over: U32
  {L.TL.hold.off(hold, 0, off, top, over) == off : U32}
# ---- end scroll-stable ----

# Entry boundaries carry identity without moving any pixels.
law hold_boundary_zero:
  {L.TL.h(29) == 0 : U32}

# Correct for growth above the anchor as well as movement of the panel.
law hold_entry_moves:
  for +off: Nat
  for +y0: Nat
  for +y1: Nat
  for +t0: Nat
  for +t1: Nat
  for +over: Nat
  for h: {Nat.is_le(Nat.sub(Nat.add(Nat.add(Nat.sub(off, y0), y1), t1), t0), over) == True{} : Bool}
  {L.TL.hold.move(off, y0, y1, t0, t1, over) == Nat.sub(Nat.add(Nat.add(Nat.sub(off, y0), y1), t1), t0) : Nat}

# At the end there is no held page: the ordinary page limit applies.
law hold_page_free:
  for +es: List<&2, M.Entry>
  for +base: Nat
  {U.Hold.page(es, "", base) == base : Nat}

# ---- usage-claude-resume ----
# a Claude session the hub sees start is charged from 0; a resumed one keeps
# its baseline (result_index 0 does not mean 0)
law tok_claude_start_zero:
  {H.Turn.claude.start(False{}, "t", "s", 1n) == [M.SettingSet{"tok.basis.t", "c:0", 1n}] : List<&2, M.Change>}

law tok_claude_same_session_keeps:
  for +s: String
  {H.Turn.claude.start(True{}, "t", s, 1n) == [] : List<&2, M.Change>}

# the recount happens once
law tok_recount_once:
  {H.Turn.recount(M.State.replay([M.SettingSet{"tok.recount", "b", 1n}], M.State.empty()), 2n) == [] : List<&2, M.Change>}
# ---- usage-boat-chip ----
# a thread with no Boat sandbox shows no chip
law chip_none:
  for +now: Nat
  {UZ.Chip.text("", now) == "" : String}

# a running sandbox says its size
law chip_running:
  {UZ.Chip.text("running/large/1/0", 5n) == "Boat · large · running" : String}

# a warm sandbox past its idle time is gone: no chip, never "0m"
law chip_warm_gone:
  {UZ.Chip.text("warm/default/0/10", 20n) == "" : String}

# a warm sandbox is never stale (it goes quiet by right)
law chip_warm_not_stale:
  for +gen: Nat
  for +now: Nat
  {UZ.Chip.stale("warm", gen, now) == False{} : Bool}

# ---- deploy main ----
# deploy_main (core/ship.bend): a request something covers (the commit
# being built, staged or live already holds origin/main) never starts a build
law ship_covered_never_builds:
  for c: Char
  for cs: String
  for busy: Bool
  {SHP.Ship.start.k(SCon{c, cs}, busy) == False{} : Bool}

# at most one build at a time: while one is on its way none starts
law ship_busy_never_builds:
  for +cover: String
  {SHP.Ship.start.k(cover, True{}) == False{} : Bool}

law ship_building_is_busy:
  {SHP.Ship.busy(SHP.Ship.demo.building()) == True{} : Bool}

# requests during a build yield at most one further build: whatever came,
# the end leaves no second follow-up wanted
law ship_coalesce_at_most_one:
  for sh: SHP.Ship
  for +state: String
  for +commit: String
  {SHP.Ship.more(SHP.Ship.end(sh, state, commit)) == False{} : Bool}

# three requests (t2 twice, t3) during the build of c1: one follow-up, for
# t2 and t3
law ship_coalesce_three:
  {SHP.Ship.phase(SHP.Ship.end(SHP.Ship.demo.queued(), "staged", "c1")) == "asked" : String}

law ship_coalesce_askers:
  {SHP.Ship.askers(SHP.Ship.end(SHP.Ship.demo.queued(), "staged", "c1")) == ["t2", "t3"] : List<&2, String>}

# every asker since the last report is told once: one who asks twice is
# listed once, and after the report no one is left to tell again
law ship_told_each_once:
  {SHP.Ship.told(SHP.Ship.asked(SHP.Ship.asked(SHP.Ship.demo.building(), "t1", "building"), "t4", "building")) == ["t1", "t4"] : List<&2, String>}

law ship_told_cleared:
  {SHP.Ship.told(SHP.Ship.end(SHP.Ship.demo.building(), "failed", "c1")) == [] : List<&2, String>}

law ship_told_next_cleared:
  for sh: SHP.Ship
  for +state: String
  for +commit: String
  {SHP.Ship.next(SHP.Ship.end(sh, state, commit)) == [] : List<&2, String>}

# a request a covered build holds is told at its end, not queued again
law ship_covered_joins_build:
  {SHP.Ship.more(SHP.Ship.asked(SHP.Ship.demo.building(), "t2", "building")) == False{} : Bool}

# deploy.follow is off unless set "on"
law ship_follow_off_default:
  {SHP.Ship.follow.on("") == False{} : Bool}

# a build gone without an exit status is never success
law ship_gone_not_ok:
  {SHP.Ran.ok("STATUS gone\nVERSION \n") == False{} : Bool}
# ---- end deploy main ----
# ---- kicad-ipc ----
law kicad_status_missing:
  for +r: Bool
  for +f: Bool
  for +v: String
  for +a: Nat
  {KH.KH.status(False{}, r, f, v, a) == "missing" : String}

law kicad_status_broken:
  for +f: Bool
  for +v: String
  for +a: Nat
  {KH.KH.status(True{}, False{}, f, v, a) == "broken" : String}

law kicad_status_stock:
  for +v: String
  for +a: Nat
  {KH.KH.status(True{}, True{}, False{}, v, a) == "stock" : String}

law kicad_notice_ok_none:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.notice("ok", c, v, w, i) == None{} : Maybe<&2, KH.KNote>}

law kicad_notice_checking_none:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.notice("checking", c, v, w, i) == None{} : Maybe<&2, KH.KNote>}

law kicad_notice_has_retry_missing:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.retry(KH.KH.notice("missing", c, v, w, i)) == True{} : Bool}

law kicad_notice_install_iff_missing:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.has_install(KH.KH.notice("missing", c, v, w, i)) == i : Bool}

law kicad_notice_has_retry_stock:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.retry(KH.KH.notice("stock", c, v, w, i)) == True{} : Bool}

law kicad_notice_install_iff_stock:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.has_install(KH.KH.notice("stock", c, v, w, i)) == i : Bool}

law kicad_notice_has_retry_old:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.retry(KH.KH.notice("old", c, v, w, i)) == True{} : Bool}

law kicad_notice_install_iff_old:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.has_install(KH.KH.notice("old", c, v, w, i)) == i : Bool}

law kicad_notice_has_retry_broken:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.retry(KH.KH.notice("broken", c, v, w, i)) == True{} : Bool}

law kicad_notice_install_iff_broken:
  for +c: String
  for +v: String
  for +w: String
  for +i: Bool
  {KH.KH.has_install(KH.KH.notice("broken", c, v, w, i)) == i : Bool}

law prompt_kicad_ipc_only_ok_missing:
  for +i: Bool
  {String.contains(PM.Prompt.kicad.health("missing", "/c", "10.0.6", "rev", "failed", i), "api-server") == False{} : Bool}

law prompt_kicad_ipc_only_ok_stock:
  for +i: Bool
  {String.contains(PM.Prompt.kicad.health("stock", "/c", "10.0.6", "rev", "failed", i), "api-server") == False{} : Bool}

law prompt_kicad_ipc_only_ok_old:
  for +i: Bool
  {String.contains(PM.Prompt.kicad.health("old", "/c", "10.0.6", "rev", "failed", i), "api-server") == False{} : Bool}

law prompt_kicad_ipc_only_ok_broken:
  for +i: Bool
  {String.contains(PM.Prompt.kicad.health("broken", "/c", "10.0.6", "rev", "failed", i), "api-server") == False{} : Bool}

law prompt_kicad_ipc_only_ok_checking:
  for +i: Bool
  {String.contains(PM.Prompt.kicad.health("checking", "/c", "10.0.6", "rev", "failed", i), "api-server") == False{} : Bool}

# --- window memos (area b): mm_* ---
# the main area's memo is never used at or after its due time: by then the
# clock alone may have changed what it shows (mm.bend's Mm.due)
law mm_late_misses:
  for +key: String
  for +ms: List<&2, MM.MainM>
  for +now: Nat
  for h: {Nat.is_ge(now, MM.MainM.due(ms)) == True{} : Bool}
  {MM.Mm.use(key, ms, now) == False{} : Bool}
# ---- mail 2b ----
law mail_off_unchanged:
  for +st: H.Step
  for +s: M.State
  for +now: Nat
  for +o: ML.Origin
  for +to: String
  for +text: String
  for +msg: String
  for +off: {M.Setting.get(s, "mail.on", "off") == "off" : String}
  {H.Mail.tee(st, s, now, o, to, text, msg) == st : H.Step}

law mail_file_never_waits:
  for +s: M.State
  for +now: Nat
  for +o: ML.Origin
  for +to: String
  for +text: String
  for +msg: String
  {H.Mail.file.waits(H.Mail.file(s, now, o, to, text, msg)) == False{} : Bool}

law mail_tee_keeps_delivery:
  for +st: H.Step
  for +s: M.State
  for +now: Nat
  for +o: ML.Origin
  for +to: String
  for +text: String
  for +msg: String
  {H.Queue.changes(H.Mail.tee(st, s, now, o, to, text, msg)) == H.Queue.changes(st) : List<&2, M.Change>}

law mail_file_one_conv:
  for +s: M.State
  for +now: Nat
  for +o: ML.Origin
  for +to: String
  for +text: String
  for +msg: String
  {H.Mail.file.route(H.Mail.file(s, now, o, to, text, msg)) == ML.Mail.route(o, to) : String}

law mail_room_once:
  for +s: M.State
  for +now: Nat
  for +from: String
  for +room: String
  for +text: String
  for +msg: String
  for +parts: List<&2, String>
  for +subject: String
  {H.Mail.file.count(H.Mail.room.file(s, now, from, room, text, msg, parts, subject)) == 1n : Nat}
# ---- i18n core ui ----
law title_shown_ja_new:
  {U.Title.shown(I18.Ja{}, "New thread") == "新しいスレッド" : String}
law title_shown_en_new:
  {U.Title.shown(I18.En{}, "New thread") == "New thread" : String}
law title_shown_other:
  {U.Title.shown(I18.Ja{}, "x") == "x" : String}
# the localized memo has exactly the lines of a fresh localized layout
law tl_entries_locale:
  for +ty: L.Ty
  for +es: List<&2, M.Entry>
  for +st: M.State
  for +sel: String
  for +px: U32
  for +pics: Map<&2, String>
  {L.TL.entries.m.loc(ty, L.TL.memo.make(ty, es, px, sel), st, sel, px, pics) == L.TL.entries.loc(ty, es, st, sel, px, pics) : List<&2, L.TLine>}
# ---- end i18n core ui ----
# ---- kicad-ipc 2 ----
# every server is started with --parent-pid, so it ends with the hub
law kserve_argv_parent_pid_with_idle:
  {KSL.KS.has(KSL.KS.argv("c", "/p/a.kicad_pro", "/d", 0, ["idle-exit"]), "--parent-pid") == True{} : Bool}

law kserve_argv_parent_pid_without_idle:
  {KSL.KS.has(KSL.KS.argv("c", "/p/a.kicad_pro", "/d", 0, Nil{}), "--parent-pid") == True{} : Bool}

# the socket is inside the project's own directory
law kserve_argv_socket_in_dir:
  {String.starts_with(KSL.KS.nth(KSL.KS.argv("c", "/p/a.kicad_pro", "/d/x", 0, Nil{}), 4n), "/d/x/") == True{} : Bool}

law kserve_argv_idle_with_cap:
  {KSL.KS.has(KSL.KS.argv("c", "/p/a.kicad_pro", "/d", 0, ["idle-exit"]), "--idle-exit") == True{} : Bool}

# a runtime without idle-exit is not given the flag
law kserve_argv_no_idle_without_cap:
  {KSL.KS.has(KSL.KS.argv("c", "/p/a.kicad_pro", "/d", 0, Nil{}), "--idle-exit") == False{} : Bool}

# the socket comes from the ready line (it may be api-<pid>.sock)
law kserve_ready_line:
  {KSL.KS.ready("KiCad API server listening at /d/api-7.sock") == Some{"/d/api-7.sock"} : Maybe<&2, String>}

law kserve_ready_other_line:
  {KSL.KS.ready("loading the project") == None{} : Maybe<&2, String>}

law kserve_version_ok_exact:
  {KSL.KS.version_ok("10.99.0-bp.1", "10.99.0-bp.1") == True{} : Bool}

# a git-describe part before the suffix is fine
law kserve_version_ok_describe:
  {KSL.KS.version_ok("10.99.0-unknown-bp.1", "10.99.0-bp.1") == True{} : Bool}

law kserve_version_other_build:
  {KSL.KS.version_ok("10.99.0-bp.2", "10.99.0-bp.1") == False{} : Bool}

law kserve_version_longer_build:
  {KSL.KS.version_ok("10.99.0-bp.11", "10.99.0-bp.1") == False{} : Bool}

law kserve_version_not_fork:
  {KSL.KS.version_ok("10.99.0", "10.99.0-bp.1") == False{} : Bool}

# the restart schedule: 1, 5, 30 seconds, then give up
law kserve_retry_first:
  {KSL.KS.retry([100n], 110n) == Some{1n} : Maybe<&2, Nat>}

law kserve_retry_second:
  {KSL.KS.retry([101n, 100n], 110n) == Some{5n} : Maybe<&2, Nat>}

law kserve_retry_third:
  {KSL.KS.retry([102n, 101n, 100n], 110n) == Some{30n} : Maybe<&2, Nat>}

# never more than three restarts in 300 s
law kserve_retry_gives_up:
  {KSL.KS.retry([103n, 102n, 101n, 100n], 110n) == None{} : Maybe<&2, Nat>}

law kserve_retry_old_failures_forgotten:
  {KSL.KS.retry([990n, 102n, 101n, 100n], 1000n) == Some{1n} : Maybe<&2, Nat>}

# a clean exit (idle-exit, Shutdown) is never a failure
law kserve_clean_exit_not_a_failure:
  {KSL.KS.fails.add(0, [5n], 10n) == [5n] : List<&2, Nat>}

law kserve_crash_is_a_failure:
  {KSL.KS.fails.add(1, [5n], 10n) == [10n, 5n] : List<&2, Nat>}

# and it is stopped, not restarted
law kserve_clean_exit_stops:
  {KSL.KS.state(KSL.KS.exited(KSL.KS.up(KSL.KS.new("A", 1n), "/s/A", "v", 5n), 0, 20n, "")) == "stopped" : String}

law kserve_three_crashes_restart:
  {KSL.KS.state(KSL.KS.exited(KSL.KS.exited(KSL.KS.exited(KSL.KS.new("A", 1n), 1, 100n, "x"), 1, 110n, "x"), 1, 120n, "x")) == "starting" : String}

law kserve_fourth_crash_fails:
  {KSL.KS.state(KSL.KS.exited(KSL.KS.exited(KSL.KS.exited(KSL.KS.exited(KSL.KS.new("A", 1n), 1, 100n, "x"), 1, 110n, "x"), 1, 120n, "x"), 1, 130n, "x")) == "failed" : String}

# a third project retires the least recently used ready server
law kserve_evict_least_recent:
  {KSL.KS.evict([KSL.KS.up(KSL.KS.new("A", 1n), "/s/A", "v", 5n), KSL.KS.up(KSL.KS.new("B", 1n), "/s/B", "v", 3n)]) == "B" : String}

law kserve_no_evict_with_room:
  {KSL.KS.evict([KSL.KS.up(KSL.KS.new("A", 1n), "/s/A", "v", 5n)]) == "" : String}

# never more than two servers starting or ready
law kserve_third_project_evicts:
  {KSL.KS.plan([KSL.KS.up(KSL.KS.new("A", 1n), "/s/A", "v", 5n), KSL.KS.up(KSL.KS.new("B", 1n), "/s/B", "v", 3n)], "C", ["shutdown"]) == "evict:B" : String}

law kserve_third_project_busy_without_shutdown:
  {KSL.KS.plan([KSL.KS.up(KSL.KS.new("A", 1n), "/s/A", "v", 5n), KSL.KS.up(KSL.KS.new("B", 1n), "/s/B", "v", 3n)], "C", Nil{}) == "busy" : String}

law kserve_running_server_is_used:
  {KSL.KS.plan([KSL.KS.up(KSL.KS.new("A", 1n), "/s/A", "v", 5n), KSL.KS.up(KSL.KS.new("B", 1n), "/s/B", "v", 3n)], "A", Nil{}) == "ready" : String}

# headless KiCad has no editor state: these are answered without a round trip
law kserve_refused_selection:
  {KSL.KS.headless_refused("kiapi.common.commands.GetSelection") == True{} : Bool}

law kserve_refused_view:
  {KSL.KS.headless_refused("kiapi.board.commands.SetVisibleLayers") == True{} : Bool}

law kserve_refused_run_action:
  {KSL.KS.headless_refused("kiapi.common.commands.RunAction") == True{} : Bool}

law kserve_get_items_allowed:
  {KSL.KS.headless_refused("kiapi.board.commands.GetItems") == False{} : Bool}

law kserve_eighteen_refused:
  {List.length(&2, String, KSL.KS.refused()) == 18n : Nat}

law kserve_gate_off:
  {KSL.KS.gate("ok", "10.99.0-bp.1", "", "off") == "IPC is turned off in Settings" : String}

law kserve_gate_ok:
  {KSL.KS.gate("ok", "10.99.0-bp.1", "", "on") == "" : String}

# an unavailable IPC always names the way round
law kserve_unavailable_names_fallback:
  {String.contains(KSL.KS.unavailable("x", "c"), "Fallback: edit the file directly") == True{} : Bool}

# Backplane KiCad 10.99.0-bp.4 with API 2 is the least that serves IPC
law kicad_min_version:
  {KH.KH.min_version() == "10.99.0-bp.4" : String}

law kicad_old_runtime_is_old:
  {KH.KH.status(True{}, True{}, True{}, "10.0.6", 1n) == "old" : String}

law kicad_new_runtime_is_ok:
  {KH.KH.status(True{}, True{}, True{}, "10.99.0-bp.4", 2n) == "ok" : String}

# with IPC on, agents edit through the kicad_ipc tool
law prompt_kicad_ok_names_kicad_ipc:
  {String.contains(PM.Prompt.kicad.health("ok", "/c", "10.99.0-bp.1", "rev", "", True{}), "`kicad_ipc`") == True{} : Bool}

law prompt_kicad_ok_no_pcbnew_import:
  {String.contains(PM.Prompt.kicad.health("ok", "/c", "10.99.0-bp.1", "rev", "", True{}), "import pcbnew") == False{} : Bool}

law prompt_kicad_ok_starts_no_server:
  {String.contains(PM.Prompt.kicad.health("ok", "/c", "10.99.0-bp.1", "rev", "", True{}), "api-server") == False{} : Bool}

law prompt_kicad_off_names_fallback:
  {String.contains(PM.Prompt.kicad.health("ok", "/c", "10.99.0-bp.1", "rev", "", False{}), "answer with the fallback") == True{} : Bool}

law kserve_desc_needs_cap:
  for +root: String
  {KSL.KS.desc(root, Nil{}) == "" : String}

law kserve_desc_with_cap:
  for +root: String
  {KSL.KS.desc.k(True{}, root) == root ++ "/share/backplane/api.desc" : String}

law kserve_idle_needs_api2:
  for xs: List<&2, String>
  {KSL.KS.caps.safe(xs, 1n) == KSL.KS.drop(xs, "idle-exit") : List<&2, String>}

# ---- end kicad-ipc 2 ----
# ---- kicad-ipc 3 ----

law kexport_no_unmarked_clear:
  for +base: String
  {KEX.KX.may_clear(base, False{}) == False{} : Bool}

law kexport_no_unconsented_force:
  for +report: KEX.KReport
  {KEX.KX.flags(report, False{}) == Nil{} : List<&2, String>}

law kexport_block_force:
  {KEX.KX.flags(KEX.KReport{Nil{}, Nil{}, ["blocked"], Nil{}}, True{}) == ["--force", "--drop-approximations"] : List<&2, String>}

law kexport_lossy_force:
  {KEX.KX.flags(KEX.KReport{["changed"], ["removed"], Nil{}, Nil{}}, True{}) == ["--force"] : List<&2, String>}

law kexport_no_outside_clear:
  {KEX.KX.may_clear("/tmp/design", True{}) == False{} : Bool}

# A path not under this home's exports is never cleared, marker or not.
law kexport_no_foreign_base:
  for +base: String
  for +marker: Bool
  {KEX.KX.clear.allowed(False{}, base, marker) == False{} : Bool}

# ---- mail 2c ----
law foh_clean:
  for +cs: List<&2, M.Change>
  {MRUN.Run.posts(H.Mail.foh.cs(cs)) == 0n : Nat}

law fyi_never_spawns:
  for +live: String
  {MRUN.Run.plan("fyi", live) == MRUN.FileOnly{} : MRUN.MailRunPlan}

law act_one_run:
  for +live: String
  {MRUN.Run.actions(MRUN.Run.plan("act", live)) == 1n : Nat}

law run_fresh:
  for th: String
  for cwd: String
  for provider: String
  for model: String
  for session: String
  for runtime: M.RuntimeMode
  for interaction: M.InteractionMode
  {H.Run.fresh.out(H.AgentStart{th, cwd, provider, model, session, runtime, interaction}) == H.AgentStart{th, cwd, provider, model, "", runtime, interaction} : H.Out}

law run_one_live_per_conv:
  {ML.Box.live_n(ML.Box.run_set(ML.Box.run_set(ML.Box.empty(), "r1", "b", "c", "m1", 1n), "r2", "b", "c", "m2", 2n), "b") == 1n : Nat}

law escalate_only_path:
  for +cs: List<&2, M.Change>
  for os: List<&2, H.Out>
  {MRUN.Run.posts(H.Queue.changes(H.Mail.foh(H.Step{cs, os}))) == 0n : Nat}

law charter_gate_denies:
  for +text: String
  for +tool: String
  for +input: J.Json
  for risky: {CH.Charter.risky(tool, input) == True{} : Bool}
  for unlisted: {CH.Charter.allows(text, tool, input) == False{} : Bool}
  {CH.Charter.gate(text, tool, input) == False{} : Bool}

law charter_gate_allows_listed:
  for +text: String
  for +tool: String
  for +input: J.Json
  for listed: {CH.Charter.allows(text, tool, input) == True{} : Bool}
  {CH.Charter.gate(text, tool, input) == True{} : Bool}

law charter_risky_fixed:
  {Bool.and(CH.Charter.risky("gmail_send", J.Null{}), Bool.and(CH.Charter.risky("deploy_restart", J.Null{}), CH.Charter.risky("Bash", J.Json.obj([J.KV{"command", J.Str{"cd /tmp && git push origin main"}}])))) == True{} : Bool}
# ---- i18n window ----
law window_notice_english:
  {L.Lay.notice(I18.En{}, "\u{1}noboard\u{1f}") == "No board in this project yet" : String}
law window_notice_words_kept:
  {L.Lay.notice(I18.Ja{}, "disk full") == "disk full" : String}
law window_tools_english:
  for +h: TOOL.Head
  for +sa: String
  for +sv: String
  for +ta: String
  for +tv: String
  {TOOL.Tools.head(h, sa, sv, ta, tv) == TOOL.Tools.head.loc(I18.En{}, h, sa, sv, ta, tv) : List<&2, TOOL.Tool>}
law window_picker_default:
  {L.Picker.name.loc(I18.Ja{}, "") == "既定" : String}
law window_provider_missing:
  {L.Set.cli.loc(I18.En{}, "", "codex") == "codex is not installed" : String}
# ---- end i18n window ----

# --- browser-cef ---
# The embedded Chromium (core/cef.bend): the hub moves the helper to the CEF
# engine only for the setting "cef", with a window's embed known and no
# runtime failure; only this machine's window may embed; the shield is on
# exactly while the agent drives; a page is placed natively only when no
# overlay covers it, and only what changed is sent.

law cef_engine_needs_setting:
  for known: Bool
  for ok: Bool
  {CEF.Cef.engine("chrome", known, ok) == False{} : Bool}

law cef_engine_needs_embed:
  for ok: Bool
  {CEF.Cef.engine("cef", False{}, ok) == False{} : Bool}

law cef_engine_needs_runtime:
  for known: Bool
  {CEF.Cef.engine("cef", known, False{}) == False{} : Bool}

law cef_engine_on:
  {CEF.Cef.engine("cef", True{}, True{}) == True{} : Bool}

# only the window in this process or one linked over loopback embeds
law cef_embed_window_here:
  {CEF.Cef.from(0, False{}) == True{} : Bool}

law cef_embed_loopback_link:
  {CEF.Cef.from(5, True{}) == True{} : Bool}

law cef_embed_remote_refused:
  {CEF.Cef.from(2147483649, False{}) == False{} : Bool}

law cef_loopback_v4:
  {CEF.Cef.loopback("127.0.0.1") == True{} : Bool}

law cef_loopback_v6:
  {CEF.Cef.loopback("::1") == True{} : Bool}

law cef_tailnet_not_loopback:
  {CEF.Cef.loopback("100.64.0.7") == False{} : Bool}

# a window that leaves takes its embed with it, another's leaving does not
law cef_leave_clears_embed:
  {CEF.Cef.session.known(CEF.Cef.session.leave(CEF.Cef.session.embed_set(CEF.Cef.empty(), 7, 12, ":9"), 7)) == False{} : Bool}

law cef_leave_other_keeps_embed:
  {CEF.Cef.session.known(CEF.Cef.session.leave(CEF.Cef.session.embed_set(CEF.Cef.empty(), 7, 12, ":9"), 8)) == True{} : Bool}

law cef_embed_bad_xid_unknown:
  {CEF.Cef.known(0, ":9") == False{} : Bool}

# the window says its X window once per connection, after the log came
law cef_embed_once_per_join:
  {CEF.Cef.embed.due("12", "", "o1", "#1", "#1") == False{} : Bool}

law cef_embed_new_join:
  {CEF.Cef.embed.due("12", "", "o1", "#1", "#2") == True{} : Bool}

law cef_embed_not_remote:
  {CEF.Cef.embed.due("12", "http://x:3787", "o1", "", "#2") == False{} : Bool}

law cef_embed_needs_xid:
  {CEF.Cef.embed.due("", "", "o1", "", "#2") == False{} : Bool}

# the shield: on while the agent drives, off while the person holds the page
law cef_shield_agent_drives:
  {CEF.Cef.shield("") == True{} : Bool}

law cef_shield_takeover_off:
  {CEF.Cef.shield("takeover") == False{} : Bool}

law cef_shield_teach_off:
  {CEF.Cef.shield("teach") == False{} : Bool}

law cef_shield_focus_opposite:
  for +d: String
  {CEF.Cef.shield.focus(d) == Bool.not(CEF.Cef.shield(d)) : Bool}

# a click on the shield takes the page over only from the agent
law cef_shield_take_from_agent:
  {CEF.Cef.shield.take("") == True{} : Bool}

law cef_shield_take_not_from_person:
  {CEF.Cef.shield.take("takeover") == False{} : Bool}

# the shield is sent again only when it changed
law cef_shield_due_same:
  {CEF.Cef.shield.due("1", "") == False{} : Bool}

law cef_shield_due_first:
  {CEF.Cef.shield.due("", "") == True{} : Bool}

law cef_shield_due_person:
  {CEF.Cef.shield.due("1", "takeover") == True{} : Bool}

# a page is placed natively only while no overlay is over it
law cef_covered_none:
  {CEF.Cef.covered(False{}, False{}, False{}, False{}, False{}, False{}, False{}, False{}) == False{} : Bool}

law cef_covered_palette:
  {CEF.Cef.covered(True{}, False{}, False{}, False{}, False{}, False{}, False{}, False{}) == True{} : Bool}

law cef_covered_picker:
  {CEF.Cef.covered(False{}, True{}, False{}, False{}, False{}, False{}, False{}, False{}) == True{} : Bool}

law cef_covered_menu:
  {CEF.Cef.covered(False{}, False{}, True{}, False{}, False{}, False{}, False{}, False{}) == True{} : Bool}

law cef_covered_look:
  {CEF.Cef.covered(False{}, False{}, False{}, True{}, False{}, False{}, False{}, False{}) == True{} : Bool}

law cef_covered_models:
  {CEF.Cef.covered(False{}, False{}, False{}, False{}, True{}, False{}, False{}, False{}) == True{} : Bool}

law cef_covered_tour:
  {CEF.Cef.covered(False{}, False{}, False{}, False{}, False{}, True{}, False{}, False{}) == True{} : Bool}

law cef_covered_perf:
  {CEF.Cef.covered(False{}, False{}, False{}, False{}, False{}, False{}, True{}, False{}) == True{} : Bool}

law cef_covered_lightbox:
  {CEF.Cef.covered(False{}, False{}, False{}, False{}, False{}, False{}, False{}, True{}) == True{} : Bool}

law cef_shows_needs_live:
  for covered: Bool
  {CEF.Cef.shows(False{}, covered) == False{} : Bool}

law cef_shows_not_covered:
  {CEF.Cef.shows(True{}, True{}) == False{} : Bool}

# placement: nothing is sent while the frame leaves what was sent; a new
# rectangle is placed; none where one was hides
law cef_place_same_nothing:
  {CEF.Cef.place.step("t1 0 52 600 400", "t1 0 52 600 400") == 0n : Nat}

law cef_place_hidden_nothing:
  {CEF.Cef.place.step("", "") == 0n : Nat}

law cef_place_new_shows:
  {CEF.Cef.place.step("", "t1 0 52 600 400") == 1n : Nat}

law cef_place_moved_shows:
  {CEF.Cef.place.step("t1 0 52 600 400", "t1 0 52 700 400") == 1n : Nat}

law cef_place_gone_hides:
  {CEF.Cef.place.step("t1 0 52 600 400", "") == 2n : Nat}

# out of cef mode the window sends nothing: it forgets a place once
law cef_post_chrome_idle:
  {CEF.Cef.post.step(False{}, "", "") == 0n : Nat}

law cef_post_chrome_forgets:
  {CEF.Cef.post.step(False{}, "t1 0 52 600 400", "") == 3n : Nat}

law cef_post_native_places:
  {CEF.Cef.post.step(True{}, "", "t1 0 52 600 400") == 1n : Nat}

# a hit's corners become the pane's extent
law cef_pane_extent:
  {CEF.Cef.extent(832, 1400) == 568 : U32}

law cef_extent_never_wraps:
  {CEF.Cef.extent(900, 96) == 0 : U32}

# a window on another machine's hub never shows pages natively
law cef_native_local_only:
  {CEF.Cef.native("cef", "http://x:3787") == False{} : Bool}

law cef_native_local_cef:
  {CEF.Cef.native("cef", "") == True{} : Bool}

law cef_native_chrome:
  {CEF.Cef.native("chrome", "") == False{} : Bool}

# only the native local viewer stops the JPEG cast; others keep it
law cef_native_skips_cast:
  {CEF.Cef.cast(True{}, "cef", "") == False{} : Bool}

law cef_web_keeps_cast:
  {CEF.Cef.cast(False{}, "cef", "") == True{} : Bool}

law cef_remote_keeps_cast:
  {CEF.Cef.cast(True{}, "cef", "http://other") == True{} : Bool}

law cef_page_here_native:
  {CEF.Cef.page_local("t1") == True{} : Bool}

law cef_page_elsewhere_jpeg:
  {CEF.Cef.page_local("peer~t1") == False{} : Bool}

# a fallback to chrome still lets the person hand a held page back
law cef_fallback_keeps_handback:
  {CEF.Cef.strip_on("chrome", "takeover") == True{} : Bool}

law cef_chrome_idle_strip_off:
  {CEF.Cef.strip_on("chrome", "") == False{} : Bool}

# the settings row's note is only the reason Embedded cannot run
law cef_note_quiet:
  {CEF.Cef.note("") == "" : String}

# engine replies: only the newest request's counts
law cef_old_engine_reply_ignored:
  {CEF.Cef.current(CEF.Cef.session.request_set(CEF.Cef.session.request_set(CEF.Cef.empty(), "cef 1 :9"), "chrome"), "1") == False{} : Bool}

law cef_current_engine_reply_used:
  {CEF.Cef.current(CEF.Cef.session.request_set(CEF.Cef.session.request_set(CEF.Cef.empty(), "cef 1 :9"), "chrome"), "2") == True{} : Bool}

# the engine is compared as a short key: the same embed asks nothing again
law cef_want_key_chrome:
  for x: U32
  for +d: String
  {CEF.Cef.want.key(False{}, x, d) == "chrome" : String}

# the hub reconsiders the engine after requests, leaves, engine answers and
# helper events, never after an agent's streamed line
law cef_after_rpc:
  {CEF.Cef.after("rpc") == True{} : Bool}

law cef_after_helper:
  {CEF.Cef.after("browser_ev") == True{} : Bool}

law cef_not_after_agent_line:
  {CEF.Cef.after("agent_line") == False{} : Bool}

# a helper restart re-shows the page a few times at most
law cef_revive_first:
  {CEF.Cef.revive(True{}, True{}, 0n) == True{} : Bool}

law cef_revive_bounded:
  {CEF.Cef.revive(True{}, True{}, 3n) == False{} : Bool}

law cef_revive_not_chrome:
  for placed: Bool
  {CEF.Cef.revive(False{}, placed, 0n) == False{} : Bool}

# page ids: an unknown one is refused, a thread's or bot's accepted
law cef_unknown_refused:
  {CEF.Cef.owner("unknown", False{}, False{}) == False{} : Bool}

law cef_thread_owner:
  {CEF.Cef.owner("t1", True{}, False{}) == True{} : Bool}

law cef_empty_owner_refused:
  for t: Bool
  for b: Bool
  {CEF.Cef.owner("", t, b) == False{} : Bool}

# take over and hand back act on the page their value names (a thread's
# page: its Browser pane's strip); without one, on the bot whose tab is open
law cef_handback_names_page:
  {U.Teach.target(7n, "b1", "t9") == "t9" : String}

law cef_handback_default_bot:
  {U.Teach.target(7n, "b1", "") == "b1" : String}

law cef_takeover_names_page:
  {U.Teach.target(6n, "b1", "t9") == "t9" : String}

law cef_other_actions_keep_bot:
  {U.Teach.target(11n, "b1", "t9") == "b1" : String}
# --- end browser-cef ---

# --- browser-cef local window guard ---
law cef_web_origin_refused:
  for proxy: String
  {CEF.Cef.link(proxy, "http://localhost") == False{} : Bool}

law cef_native_link_allowed:
  {CEF.Cef.link("", "") == True{} : Bool}
# --- end browser-cef local window guard ---
# ---- mail perf ----

# Credit travels with the message, independently of its encoding and size.
law mail_credit_carried:
  for text: String
  for charge: Nat
  {MPCREDIT.Wire.charged(False{}, MPCREDIT.WText{text}, charge) == MPCREDIT.WCharged{text, charge} : MPCREDIT.Wire}

law mail_credit_native_kept:
  for w: MPCREDIT.Wire
  for charge: Nat
  {MPCREDIT.Wire.charged(True{}, w, charge) == w : MPCREDIT.Wire}
# ---- i18n web phone ----
law webphone_option_english:
  {WPH.VLabel.option(I18.En{}, "ui.theme", "dark") == "dark" : String}
law webphone_model_name_kept:
  {WPH.VLabel.option(I18.Ja{}, "agent.model", "dark") == "dark" : String}
law webphone_count_ungrouped:
  {WPH.View.sub.badge(I18.En{}, 1200n) == "1200 agents" : String}
law webphone_count_japanese:
  {WPH.View.sub.badge(I18.Ja{}, 1200n) == "エージェント1200件" : String}
law webphone_idle_japanese:
  {Nt.Note.compact.head(I18.Ja{}, 0n) == "すべてのエージェントが待機中" : String}
# ---- end i18n web phone ----
# ---- one-hub ----
# The command line is strict (core/cli.bend): an unknown flag never runs a
# hub, whatever follows it or comes before it; --help and --version never
# run one either, and a flag without its value is refused.
law cli_unknown_never_runs:
  for xs: List<&2, String>
  {CLI.Cli.runs(CLI.Cli.of("--bogus" <> xs)) == False{} : Bool}

law cli_unknown_after_flags_never_runs:
  for xs: List<&2, String>
  {CLI.Cli.runs(CLI.Cli.of("--home" <> "/h" <> "--no-tailscale" <> "--bogus" <> xs)) == False{} : Bool}

law cli_word_never_runs:
  for xs: List<&2, String>
  {CLI.Cli.runs(CLI.Cli.of("serve" <> xs)) == False{} : Bool}

law cli_version_never_runs:
  for xs: List<&2, String>
  {CLI.Cli.of("--version" <> xs) == CLI.CliVersion{} : CLI.Cli}

law cli_version_after_flags:
  for xs: List<&2, String>
  {CLI.Cli.of("--home" <> "/h" <> "--port" <> "3799" <> "--version" <> xs) == CLI.CliVersion{} : CLI.Cli}

law cli_help_never_runs:
  for xs: List<&2, String>
  {CLI.Cli.of("--help" <> xs) == CLI.CliHelp{} : CLI.Cli}

law cli_h_never_runs:
  for xs: List<&2, String>
  {CLI.Cli.of("-h" <> xs) == CLI.CliHelp{} : CLI.Cli}

law cli_missing_value:
  {CLI.Cli.of(["--home"]) == CLI.CliBad{"--home needs a value"} : CLI.Cli}

law cli_value_not_a_flag:
  for xs: List<&2, String>
  {CLI.Cli.runs(CLI.Cli.of("--home" <> "--version" <> xs)) == False{} : Bool}

law cli_test_hub_runs:
  {CLI.Cli.of(["--home", "/h", "--port", "3799", "--no-tailscale", "--foreground"]) == CLI.CliRun{} : CLI.Cli}

# One hub per home (core/onehub.bend): what the lock's answer means
law onehub_ours:
  {OHB.OneHub.got("ok") == OHB.HubOurs{} : OHB.HubGot}

law onehub_held:
  {OHB.OneHub.got("held 12 3799 flock") == OHB.HubHeld{"12", "3799"} : OHB.HubGot}

law onehub_says:
  {OHB.OneHub.says("/h", "12", "3799") == "a hub already serves /h (pid 12, port 3799)" : String}

# Stand-ins (core/standin.bend) stay ahead of the login shell's PATH and
# ~/.local/bin, where the real CLIs are
law standin_path_first:
  {SIN.Standin.path("/f", "/b", "/usr/bin") == "/f:/b:/usr/bin" : String}

law standin_merge_behind:
  {CPA.CliPath.merge(SIN.Standin.path("/f", "/b", "/usr/bin"), "/u/.local/bin:/usr/bin", ["/u/.local/bin"]) == "/f:/b:/usr/bin:/u/.local/bin" : String}
# ---- end one-hub ----
# ---- kicad-ipc 4 ----
# the pin is bp.4 (API 2); the -bp.N suffix orders numerically
law kicad4_bp1_old:
  {KH.KH.status(True{}, True{}, True{}, "10.99.0-bp.1", 2n) == "old" : String}

law kicad4_bp2_old:
  {KH.KH.status(True{}, True{}, True{}, "10.99.0-bp.2", 2n) == "old" : String}

law kicad4_bp3_old:
  {KH.KH.status(True{}, True{}, True{}, "10.99.0-bp.3", 2n) == "old" : String}

law kicad4_bp4_ok:
  {KH.KH.status(True{}, True{}, True{}, "10.99.0-bp.4", 2n) == "ok" : String}

law kicad4_bp10_ok:
  {KH.KH.status(True{}, True{}, True{}, "10.99.0-bp.10", 2n) == "ok" : String}

law kicad4_describe_part_reads_bp:
  {KH.KH.status(True{}, True{}, True{}, "10.99.0-unknown-bp.4", 2n) == "ok" : String}

law kicad4_no_suffix_is_bp0:
  {KH.KH.status(True{}, True{}, True{}, "10.99.0", 2n) == "old" : String}

law kicad4_bp3_needs_api2:
  {KH.KH.status(True{}, True{}, True{}, "10.99.0-bp.3", 1n) == "old" : String}

law kicad4_bp2_before_bp3:
  {KH.KH.version.ge("10.99.0-bp.2", "10.99.0-bp.3") == False{} : Bool}

law kicad4_bp10_after_bp3:
  {KH.KH.version.ge("10.99.0-bp.10", "10.99.0-bp.3") == True{} : Bool}

law kicad4_min_api:
  {KH.KH.min_api() == 2n : Nat}

law kicad4_pinned_version_is_min:
  {KH.KH.min_version() == "10.99.0-bp.4" : String}

law kserve4_launcher_never_real:
  {KSL.KS.launcher("/r/bin/kicad-cli.real") == "/r/bin/kicad-cli" : String}

law kserve4_launcher_kept:
  {KSL.KS.launcher("/r/bin/kicad-cli") == "/r/bin/kicad-cli" : String}

law kserve4_argv_launcher:
  {KSL.KS.nth(KSL.KS.argv("/r/bin/kicad-cli.real", "/p/a.kicad_pro", "/d", 0, Nil{}), 0n) == "/r/bin/kicad-cli" : String}

law kserve4_bare_footprint_refused:
  {KSL.KS.guard("kiapi.common.commands.UpdateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.board.types.FootprintInstance"}}])])}]), Nil{}) == KSL.KS.fp.refusal() : String}

law kserve4_refusal_names_full_instance:
  {String.contains(KSL.KS.fp.refusal(), "send the full FootprintInstance from GetItems with only the fields you change") == True{} : Bool}

law kserve4_full_footprint_passes:
  {KSL.KS.guard("kiapi.common.commands.UpdateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.board.types.FootprintInstance"}}, J.KV{"definition", J.Json.obj(Nil{})}])])}]), Nil{}) == "" : String}

law kserve4_footprint_cap_lifts:
  {KSL.KS.guard("kiapi.common.commands.UpdateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.board.types.FootprintInstance"}}])])}]), ["footprint-partial-update"]) == "" : String}

law kserve4_other_item_passes:
  {KSL.KS.guard("kiapi.common.commands.UpdateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.board.types.Track"}}])])}]), Nil{}) == "" : String}

law kserve4_other_command_passes:
  {KSL.KS.guard("kiapi.common.commands.CreateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.board.types.FootprintInstance"}}])])}]), Nil{}) == "" : String}

law kserve4_bare_symbol_refused:
  {KSL.KS.guard("kiapi.common.commands.CreateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance"}}])])}]), Nil{}) == KSL.KS.sym.refusal() : String}

law kserve4_symbol_advice:
  {String.contains(KSL.KS.sym.refusal(), "LoadAllLibraries") == True{} : Bool}

law kserve4_symbol_with_definition_passes:
  {KSL.KS.guard("kiapi.common.commands.CreateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance"}}, J.KV{"definition", J.Json.obj(Nil{})}])])}]), Nil{}) == "" : String}

law kserve4_symbol_cap_lifts:
  {KSL.KS.guard("kiapi.common.commands.CreateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance"}}])])}]), ["symbol-from-libid"]) == "" : String}

law kserve4_no_items_passes:
  {KSL.KS.guard("kiapi.common.commands.UpdateItems", J.Json.obj(Nil{}), Nil{}) == "" : String}

law prompt4_kicad_ok_names_the_rules:
  {String.contains(PM.Prompt.kicad.health("ok", "/c", "10.99.0-bp.3", "rev", "", True{}), "PlaceSymbolFromLibrary") == True{} : Bool}

law kserve4_nested_symbol_library_id_refused:
  {KSL.KS.guard("kiapi.common.commands.CreateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance"}}, J.KV{"definition", J.Json.obj([J.KV{"id", J.Json.obj(Nil{})}])}])])}]), Nil{}) == KSL.KS.sym.refusal() : String}

law kserve4_nested_symbol_cap_lifts:
  {KSL.KS.guard("kiapi.common.commands.CreateItems", J.Json.obj([J.KV{"items", J.Json.arr([J.Json.obj([J.KV{"@type", J.Str{"type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance"}}, J.KV{"definition", J.Json.obj([J.KV{"id", J.Json.obj(Nil{})}])}])])}]), ["symbol-from-libid"]) == "" : String}

# ---- end kicad-ipc 4 ----
# ---- kicad fork viewers ----
law fork_cache_legacy:
  for id: String
  {SC.Sch.cache.name("", id) == id : String}
law fork_cache_alias:
  for id: String
  {SC.Sch.cache.name("local", id) == "local" : String}
# ---- end kicad fork viewers ----
# ---- kicad fork footprint placement ----
law fork_footprint_legacy_at:
  for x: KSEX.Sx
  {B.Fp.at.k(True{}, x) == KSEX.Sx.get(x, "at") : KSEX.Sx}
# ---- end kicad fork footprint placement ----

# Primary orders already carry an exact subtree size; totals need no page.
law mail_plain_count:
  for +b: ML.Box
  for +bot: String
  {ML.Box.count(b, bot, "", "") == ML.Order.count(ML.Box.order_of(b, bot)) : Nat}

# All-mail includes a thread-only conversation and a person-only one.
law mail_global_primary:
  {ML.Box.slice(ML.Box.conv_set(ML.Box.conv_set(ML.Box.empty(), "a", "A", "pair", ["thread:x"], "", Nil{}, 1n), "b", "B", "pair", ["person"], "", Nil{}, 2n), "*", 0n, 2n)
    == [ML.Conv{"b", "B", "pair", ["person"], "", Nil{}, 2n, 2n, 0n}, ML.Conv{"a", "A", "pair", ["thread:x"], "", Nil{}, 1n, 1n, 0n}] : List<&2, ML.Conv>}

law mail_log_count_set:
  for c: MPCREDIT.Cfg
  for n: Nat
  {MPCREDIT.LogCount.n(MPCREDIT.LogCount.set(c, n)) == n : Nat}

law mail_log_count_committed:
  for +c: MPCREDIT.Cfg
  for +fresh: List<&2, String>
  {MPCREDIT.LogCount.n(MPCREDIT.LogCount.committed(c, fresh)) == Nat.add(MPCREDIT.LogCount.n(c), List.length(&2, String, fresh)) : Nat}

# ---- native phone i18n ----
law native_language_english:
  {NATIVEI18N.native_locale("en", "ja-JP", "1") == "en" : String}
law native_language_preview:
  {NATIVEI18N.native_locale("ja", "en-US", "1") == "ja" : String}
law native_language_system:
  {NATIVEI18N.native_locale("auto", "ja_JP.UTF-8", "1") == "ja" : String}
law native_language_fallback:
  {NATIVEI18N.native_locale("auto", "de-DE", "1") == "en" : String}
# ---- end native phone i18n ----
# ---- build-speed: one executable, exact entry selection ----
law exe_server_name:
  {EX.Exe.server(["backplane-serve"]) == True{} : Bool}

law exe_server_path:
  {EX.Exe.server(["/opt/backplane/backplane-serve", "--port", "3799"]) == True{} : Bool}

law exe_app_name:
  {EX.Exe.server(["backplane", "backplane-serve"]) == False{} : Bool}

law exe_server_exact:
  {EX.Exe.server(["/opt/other-backplane-serve"]) == False{} : Bool}
# ---- end build-speed ----

# ---- global provider fallback ----

# An empty suffix cannot launch any provider, regardless of state/catalog.
law fallback_empty_exhausted:
  for s: M.State
  for caps: J.Json
  for now: Nat
  {FALL.Chain.pick([], s, caps, now) == None{} : Maybe<&2, FALL.Target>}

law fallback_never_wraps:
  {FALL.Chain.show(FALL.Chain.suffix(FALL.Chain.parse("claude/sonnet,codex/gpt-6.1-sol"), "codex/gpt-6.1-sol")) == "" : String}

law fallback_auth_is_not_quota:
  {FALL.Error.classify("codex", "gpt-6.1-sol", J.Json.obj([J.KV{"type", J.Str{"turn.failed"}}, J.KV{"message", J.Str{"login keyring is locked"}}])) == None{} : Maybe<&2, FALL.Limit>}

law fallback_unknown_reset_held:
  {FALL.Limit.active.o(J.Json.obj([J.KV{"scope", J.Str{"claude"}}, J.KV{"reset", J.Json.nat(0n)}]), 1000n) == True{} : Bool}

law fallback_reset_boundary:
  {FALL.Limit.active.o(J.Json.obj([J.KV{"scope", J.Str{"claude"}}, J.KV{"reset", J.Json.nat(1000n)}]), 1000n) == False{} : Bool}

law fallback_no_recovery_while_locked:
  for s: M.State
  for now: Nat
  {H.Auth.recover(False{}, s, now) == H.Step.none() : H.Step}
# ---- end global provider fallback ----

# Sidebar row memos compare exact resolved inputs with the same typography.
law sidebar_row_key_sound:
  for a: SRT.SideRowK
  for b: SRT.SideRowK
  for +h: {SR.Key.same(a, b) == True{} : Bool}
  {a == b : SRT.SideRowK}

law sidebar_row_paint_exact:
  for +ty: L.Ty
  for +a: SRT.SideRowK
  for +b: SRT.SideRowK
  for h: {SR.Key.same(a, b) == True{} : Bool}
  {SR.Key.paint(ty, a) == SR.Key.paint(ty, b) : L.Out}
# Visible cat requests are additional to sidebar/header warm-up.
law visible_cats_empty_preserves_cache:
  for +c: Map<&2, VCAT.CatF>
  for +clock: Nat
  {BV.Spot.warm(Nil{}, c, clock) == c : Map<&2, VCAT.CatF>}

law visible_sleep_cat_posed_once:
  for +clock: Nat
  {BV.Bv.warm.at("sleep", clock) == 0n : Nat}
# ---- mail ui ----
law mailroom_tab_after_chat:
  {BU.BU.tabs.mail("b1") == ["chat", "mail", "space", "browser", "tasks", "memory", "routines", "hooks", "settings"] : List<&2, String>}

law mailroom_uber_default:
  for +u: U.Ui
  for +id: String
  {BU.BU.tab.dflt(u, id) == Bool.pick(String, UBR.Uber.is(U.Ui.state(u), id), "mail", "chat") : String}

law mailroom_run_collapsed_default:
  for +now: Nat
  {U.Mailv.run_open(U.Ui.init(now)) == False{} : Bool}

law mailroom_chip_filters:
  for +f: MVW.Filter
  for +bot: String
  {MVW.Filter.bot(MVW.Mv.with_bot(f, bot)) == bot : String}

law mailroom_chip_keeps_selection:
  for +u: U.Ui
  for +bot: String
  {U.Ui.sel(U.Mailv.chip(u, bot)) == U.Ui.sel(u) : String}

law mailroom_covers_plane:
  for +p: PLN.Plane
  {MRUI.Mr.all(p) == List.concat(&2, PLN.PRow, [PLN.Plane.need(p), PLN.Plane.run(p), PLN.Plane.locks(p), PLN.Plane.routines(p), PLN.Plane.rooms(p), PLN.Plane.idle(p), PLN.Plane.bots(p)]) : List<&2, PLN.PRow>}

# The body cache carries no independent sidebar or timeline state.
law mail_body_kept_output:
  for k: String
  for +o: L.Out
  for at: Nat
  for due: Nat
  {MM.BotM.out(MM.BotM.kept(MM.BotM{k, o, at, due, False{}})) == o : L.Out}

law mail_age_relative_deadline:
  {MM.BotM.age_due(160n, 101n) == 161n : Nat}

# Reusing the window's exact sidebar index preserves the complete plane.
law mail_plane_indexed_build:
  for +u: U.Ui
  {BV.Bv.plane.build.ix(True{}, u, U.Side.ix(U.Ui.state(u))) == BV.Bv.plane.build(True{}, u) : PLN.Plane}

law mail_plane_indexed_hidden:
  for +u: U.Ui
  for ix: U.SIx
  {BV.Bv.plane.build.ix(False{}, u, ix) == BV.Bv.plane.none() : PLN.Plane}
# Browser provenance is mandatory even when a connection is otherwise trusted.
law auth_foreign_origin_denied:
  for +trusted: Bool
  for +path: String
  for +token: String
  {AUTH.Auth.allowed(trusted, [HTTPAUTH.Header{"origin", "https://foreign.example"}, HTTPAUTH.Header{"host", "localhost:3787"}], path, token) == False{} : Bool}

law auth_empty_remote_token_denied:
  {AUTH.Auth.allowed(False{}, Nil{}, "/ws?token=", "") == False{} : Bool}

law auth_cookie_prefix_denied:
  {AUTH.Auth.allowed(False{}, [HTTPAUTH.Header{"cookie", "bp_token=secret-extra"}], "/ws", "secret") == False{} : Bool}
# ---- PCB viewer text ----
law pcb2_hidden_text_draws_nothing:
  for +at: B.Pt
  for +angle: F32
  for +x: KSEX.Sx
  for +text: String
  for +up: Bool
  for +key: String
  for +sig: U32
  for +ctx: String
  {PCB2.Text.visible.k(True{}, at, angle, x, text, up, key, sig, ctx) == Nil{} : List<&2, B.Item>}
