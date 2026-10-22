import Base
import ./telemetry.bend as TI
import ./telemetry-ui.bend as TUI
import ../core/telemetry.bend as TLM
import ../core/charter.bend as CH
import ../core/mailrun.bend as RUN
import ../core/maildisposition.bend as MD
import ../core/kihealth.bend as KH
import ../core/kserve.bend as KSC
import ./kserve.bend as KSV
import ../core/cef.bend as CF
import ../core/exe.bend as EX
import ./lockio.bend as LG
import ../core/locks.bend as LCK
import ./host.bend as H
import ./proc.bend as P
import ../core/json.bend as J
import ../core/utf8.bend as U
import ../core/http.bend as W
import ../core/auth.bend as AUTH
import ../core/model.bend as M
import ../core/codec.bend as C
import ../core/hub.bend as B
import ../core/deploy.bend as DP
import ../core/ship.bend as SHP
import ../core/prompt.bend as Q
import ../core/mcp.bend as MC
import ./update.bend as UP
import ../core/version.bend as VR
import ./kicad.bend as K
import ./kexport.bend as KEX
import ../core/kexport.bend as KXP
import ./browser.bend as BR
import ./desktop-transport.bend as DT
import ../core/desktop-browser.bend as DB
import ../core/acp.bend as AC
import ../core/msp.bend as MS
import ../core/agy.bend as AGY
import ./agyio.bend as AGX
import ./mailstore.bend as MST
import ../core/mailbody.bend as MB
import ../core/mailroom_history.bend as RH
import ../core/mail.bend as ML
import ../core/cbor.bend as CB
import ../mobile/session.bend as MSE
import ./thin-view.bend as TV
import ./thin-actor.bend as TA
import ../core/thin-pace.bend as TPACE
import ./thin-mail.bend as TMAIL
import ../core/thin.bend as THIN
import ../core/thin-live.bend as TLIVE
import ../core/thin-wire.bend as TVW
import ../core/presentation.bend as PRESENT
import ../web/session.bend as WEBFRONT
import ./push.bend as PU
import ./park.bend as PK
import ./git.bend as GT
import ./gh.bend as GH
import ./wake.bend as WJ
import ../core/wake.bend as WK
import ./term.bend as TM
import ./plot.bend as PV
import ../core/agent.bend as AG
import ../core/port.bend as PT
import ../core/clipath.bend as CPA
import ../core/tailnet.bend as TN
import ../core/images.bend as IM
import ../core/links.bend as LK
import ../core/attach.bend as ATT
import ../core/attach.bend as AT
import ../core/sha1.bend as SH
import ../core/boot.bend as BT
import ../core/store.bend as ST
import ./storedb.bend as SO
import ./hotboot.bend as HB
import ./botnet.bend as BN
import ../core/far.bend as FR
import ../core/fview.bend as FV
import ../core/rview.bend as RVW
import ./farview.bend as FVS
import ../core/xref.bend as XR
import ../core/fmcp.bend as FM
import ../core/client.bend as UC
import ../core/bothub.bend as BH
import ../core/bot.bend as BO
import ../core/space.bend as SPC
import ../core/refs.bend as RF
import ../core/sign.bend as SG
import ./google.bend as GS
import ../core/google.bend as GG
import ../core/ring.bend as RG
import ./voice.bend as VO
import ./apikeys.bend as AKS
import ./perf.bend as PS
import ./freecad.bend as FC
import ../core/perf.bend as PF
import ../core/semver.bend as SV
import ../core/desk.bend as DK
import ../core/notice.bend as NO
import ../core/hist.bend as HI
import ../core/emb.bend as EM
import ./review.bend as RVS
import ./hublock.bend as HL
import ../core/cli.bend as CL
import ../core/onehub.bend as OH
import ../core/standin.bend as SI
import ../core/cmds.bend as CM
import ../core/errtail.bend as ER
import ../core/keyring.bend as KR
import ../core/adopt.bend as AD
import ./adoptio.bend as AIO
import ./castio.bend as CST
import ../core/outbox.bend as OB
import ../core/live.bend as LV
import ../core/farlive.bend as FLV
import ../core/qr.bend as QR
import ../core/order.bend as OR
import ./orderio.bend as ODI
import ../core/uber.bend as UB
import ../core/ubersync.bend as UBS
import ../core/ubop.bend as UBO
import ../core/sched.bend as SCH
import ../core/tz.bend as TZN
import ../core/rules.bend as RLS
import ../core/task.bend as TKS
import ../core/teach.bend as TE
import ../core/usage.bend as UZ
import ./thinstore.bend as TSTO
import ../core/copy.bend as CP # BACKPLANE_COPY
import ../core/copynet.bend as CN

# The server. One hub computation owns the read model and the event log;
# every connection and agent process is its own computation and talks to
# the hub over channels. The hub runs the pure decider (core/hub.bend) and
# carries out the effects it answers.

# what a connection's writer sends
type Wire is Data:
  WText{text: String}
  # Socket-only text with its original offered credit. Native/link readers
  # keep WText; compression and batching never redefine this charge.
  WCharged{text: String, charge: Nat}
  WPong{payload: List<&2, U32>}
  # a message already encoded (a board's plot: never parsed again)
  WBin{cbor: List<&2, U32>}
  WClose{}
  # the hub's own window: the read model itself, with the log message's
  # head (no items), in place of the log as text
  WState{st: M.State, head: String}
  # a message made of a log's lines: head, the lines joined by ",", tail;
  # put together by the writer, off the hub (a join's chunk)
  WItems{head: String, plain: String, split: String, items: List<&2, String>, tail: String, ends: Bool}
  # a message as its JSON tree with its charge (a thin view's reply): the
  # writer encodes the tree, never printing and parsing it again
  WJson{json: J.Json, charge: Nat}

type Msg is Data:
  Desktop{client: U32, kind: String, text: String, out: Chan(String), epoch: String, lease: String}
  MailRouted{body: MB.Body}
  RunReady{thread: String, head: String, bodies: List<&2, MB.Body>}
  # the uberbot's fyi digest: the bodies of the conversations asked for
  DigestGot{bot: String, ids: List<&2, String>, bodies: List<&2, MB.Body>}
  # n: how many probes since the lock was asked for (the guard is tried
  # each 100 ms; who holds it outside is asked every LockJob.every())
  LockProbe{name: String, holder: String, n: Nat}
  # who holds a lock's flock outside the hub, as Guard.who printed it
  LockSaw{name: String, holder: String, out: String}
  LockHttp{op: String, body: String, reply: Chan(String)}
  Announce{text: String}
  Mcp{thread: String, body: String, reply: Chan(String)}
  # a request on the MCP route arrived, its answer was written (the open
  # calls a deploy waits for: core/deploy.bend's Open.mcp)
  McpOpen{thread: String, call: String, body: String}
  McpShut{thread: String, call: String}
  # gen: the connection's generation (a socket's id; the window's client 0
  # joins again under a new one); sock: a raw descriptor on its socket (0
  # for none), shut at once when it falls too far behind (core/outbox.bend)
  Join{client: U32, out: Chan(Wire), since: Nat, origin: String, gen: U32, sock: U32, legacy: Bool}
  # the window in this process: it takes the read model, not the log
  MobileJoin{client: U32, out: Chan(Wire), since: Nat, origin: String, gen: U32, sock: U32, legacy: Bool, mode: String}
  MobileTimeout{client: U32, gen: U32}
  ThinRead{client: U32, gen: U32, view: String, target: J.Json, text: String}
  JoinHere{client: U32, out: Chan(Wire), gen: U32}
  # a client's writer has put n frames (bytes bytes) out in all
  Drained{client: U32, gen: U32, n: Nat, bytes: Nat}
  # the next chunk of a joining client's log (OB.Cr: boot)
  JoinMore{client: U32, gen: U32}
  # a client pinged (its reader passes on at most one a second)
  Ping{client: U32, payload: List<&2, U32>}
  Leave{client: U32}
  Rpc{client: U32, text: String}
  AgentLine{thread: String, line: String}
  # why is the exit in words (B.Exit.why), made off the hub
  AgentGone{thread: String, code: U32, pid: U32, why: String}
  Tick{}
  TelemetryDone{gen: Nat, view: TLM.TmView, id: String}
  TelemetryPermit{gen: Nat, view: TLM.TmView, reply: Chan(Bool)}
  # a Codex or Grok turn's KiCad files, read off the hub: the panel opens
  # (Hub.auto) when they arrive
  AutoView{thread: String, fs: K.Files}
  # a Claude process started (its pid) or could not, by generation: the
  # hub registered the agent first, with a placeholder (pid 0)
  ClaudePid{thread: String, pid: U32, gen: String}
  ClaudeFail{thread: String, gen: String, msg: String}
  # every 15 s: a heartbeat for every client (Alive.due)
  Heart{}
  # thin views are owed a view.changed now due (core/thin-pace.bend)
  ThinFlush{}
  Push{client: U32, text: String}
  # a message for one client, already CBOR
  PushBin{client: U32, cbor: List<&2, U32>}
  TermOut{thread: String, data: List<&2, U32>}
  # a chat terminal's snapshot for a client (TM.Et.all(): every client)
  TermTo{client: U32, key: String, cols: U32, rows: U32, data: List<&2, U32>}
  # a line for a thread's agent built off the hub (a message with images)
  AgentWrite{thread: String, line: String, gen: String}
  # time to look for a release on the update channel
  UpdateCheck{}
  # an update is installed: the hub decides, as it is now, whether to restart
  # (force: the person asked, or the wait ran out)
  UpdateReady{force: Bool}
  # a waiting deploy's look at the build locks (free: neither is held),
  # and a failed swap of its staged package (core/deploy.bend, hub.bend's Deploy.*)
  DeployLocks{free: Bool}
  DeployFailed{err: String}
  # deploy_main (Ship*): origin/main read for a call (out: the look's
  # lines), for a follow-up or deploy.follow; a build's job started (out:
  # "PID <pid> <ident>"), ended or gone (out: its STATUS, VERSION, STAGE, log tail)
  ShipAsked{thread: String, body: String, reply: Chan(String), out: String}
  ShipNewest{why: String, out: String}
  ShipPid{commit: String, out: String}
  ShipDone{commit: String, out: String}
  KProbe{force: Bool}
  KFound{setting: String, rt: K.Rt}
  # the headless KiCad servers' lines changed (core/kserve.bend's KS.lines)
  KiServers{text: String}
  ModelsFound{models: String, text: String}
  # opencode's models read again (OcJob.run): Oc.models.sh's lines
  OcFound{text: String}
  # a project's own checks wait for the person's OK (RvJob.reply): the hub
  # asks in the thread (Hub.checks.ask)
  ChecksAsk{thread: String, hash: String, text: String}
  MachinesFound{machines: String}
  # the tailnet listener is up: the pairing link clients show in Settings
  LinkFound{link: String}
  # a Codex or Grok turn's process started: the one the hub now hears (and
  # stops) for the thread; its lines come tagged (B.Tag)
  TurnPid{thread: String, pid: U32, input: Chan(String), gen: String}
  # a turn's process exited (tagged)
  TurnDone{tag: String}
  # the hub just started from its log (B.Hub.recover)
  Recover{}
  # restart in place: the hub hands its agents off (adopt.bend), then execs
  Reexec{path: String, args: List<&2, String>}
  # changes a job off the hub made (a bot's home folder, a linked machine)
  Commit{changes: List<&2, M.Change>}
  # an order's progress: posted in its buy thread, and a note alert to the
  # person's devices when there is one (Hub.order.note)
  OrderNote{thread: String, id: String, text: String, note: String}
  # what an order tool learned once the person acted (the files sent and
  # the quote posted, or not sent): told to the buy thread's agent, which
  # starts its turn (order_quote and order_ask never wait on the person)
  OrderWake{thread: String, text: String}
  # the review gate's verdict (core/order.bend's Order.gate): what the hub
  # posts and asks in the buy thread
  OrderGate{thread: String, kind: Nat, md: String}
  # the uberbot's sync (core/ubersync.bend): a step touched its log or
  # promoted a bot ("kick": sync now; "init": log a new uberbot's first
  # ops, then sync); another machine's answer to our uber.sync request
  UberDo{what: String}
  UberAnswer{link: String, code: Nat, body: String}
  # the minute tick for bots: routines and the linked machines' bots, with
  # this machine's UTC offset as `date +%z` prints it
  BotTick{tz: String}
  # the bots on linked machines (a JSON array)
  BotsFound{peer: String, url: String, result: Maybe<&2, List<&2, J.Json>>}
  # one peer, or every peer when empty; the hub coalesces active fetches
  BotsRefresh{peer: String}
  # a request for bots' routes (a webhook, a linked machine, a bot's
  # screen, Google's sign-in redirect); the reply is the whole response
  # (`ct` its content type; `raw` its bytes, kept only for a form with a
  # recording, which is not text)
  BotHttp{kind: String, arg: String, peer: String, ts: String, sig: String, gh: String, delivery: String, body: String, ct: String, raw: List<&2, U32>, reply: Chan(List<&2, U32>)}
  # the same, its signature checked off the hub: `key` is its replay key
  # ("" when it is not genuine); `me` is this machine's name
  BotVerified{kind: String, arg: String, key: String, body: String, me: String, reply: Chan(List<&2, U32>)}
  # a browser page has a new frame: {"t":"shot",...} for every client
  Shot{text: String}
  # a frame only the clients showing its page get (its "all" is false)
  ShotTo{text: String, page: String}
  # the person's request about a bot's browser or what it is taught (the
  # hub keeps the driver and the draft: core/teach.bend); a helper event
  # about a page (a recorder step); the person's answer to a pause handed
  # the browser back; a task run asks the person (task_pause)
  TeachReq{client: U32, id: String, m: String, p: String}
  BrowserEv{line: String}
  CefDone{request: String, live: String, err: String}
  # a websocket from this machine (loopback, not proxied): it may embed
  Near{client: U32}
  TeachBack{bot: String}
  TaskPaused{thread: String, bot: String, run: String, slug: String, n: Nat, kind: String, question: String, risky: Bool, rid: String, reply: Chan(String)}
  # a bot's new project, approved by the person, its folder made and
  # resolved: the hub adds it and answers the tool call (`id` as JSON)
  NewProject{thread: String, root: String, name: String, id: String, reply: Chan(String)}
  # a calendar_update or calendar_delete, its event fetched off the hub:
  # the arguments as they will run and the words the person approves
  # (`id` as JSON); `ask` is false when a session approval covers it
  CalAsk{thread: String, tool: String, args: String, detail: String, id: String, reply: Chan(String), ask: Bool}
  # a bot's gmail_send prepared off the hub (the message written, the
  # whole email as `detail`): the hub asks the person, then sends `plan`
  GmailAsk{thread: String, tool: String, detail: String, plan: String, id: String, reply: Chan(String)}
  # a phone's upload (POST /attach) asks where a thread's attachments go
  AttDir{thread: String, reply: Chan(String)}
  # threads elsewhere (far.bend): this hub's feed made from its log; a
  # push ended; a page pulled (code 0: no answer); a request sent on for
  # a client answered; a mirror read back from disk; the linked machines
  # that answered the minute's directory (one id a line)
  FarFeed{lines: FR.Feed, epoch: String}
  FarPushed{link: String, ok: Bool, to: Nat}
  # a live-text request's answer (core/farlive.bend): the peer's code and body
  FarLiveAck{link: String, code: Nat, body: String}
  # a failed push's backoff ended
  FarResend{link: String}
  FarPulled{link: String, since: Nat, code: Nat, body: String}
  FarReplied{client: U32, id: String, link: String, code: Nat, body: String}
  FarLoaded{link: String, name: String, text: String}
  FarHere{answered: String}
  # a pull to try again (the other hub was still making its feed)
  FarRetry{link: String}
  # a linked machine's request, taken (Fr.allowed) and ready to run here
  # once its folder is resolved (Far.fs.job): answered on `reply`
  FarDo{text: String, reply: Chan(List<&2, U32>)}
  # a bot's answer to a person elsewhere, captured at its turn's end
  # (hub.bend's BotReply): sent and logged now
  Replied{thread: String, to: String, text: String}
  # a report to a launcher on another machine was taken there (hub.bend's
  # Lr.far.*): its line leaves the outbox
  LaunchSent{key: String}
  # time to start the threads a restart left marked (B.Rec.release): after
  # each turn's end and each minute
  Release{}
  # a Codex or Grok turn's process could not start: its failure line, heard
  # only from the thread's current generation (B.Gen.*)
  SpawnFailed{thread: String, gen: String, line: String}
  # an agent process exited (its watcher saw it; the reader ends within 2 s)
  AgentExited{thread: String, gen: String, code: U32}
  # time to note running turns that went quiet (each minute)
  Silence{}
  # every few seconds: progress lines held back land (Hub.beat)
  Beat{}
  KeyringSeen{state: Nat}
  # watched work (core's Watch.*): a work_run launched; what proves the
  # work is the same; its end
  WatchLaunched{thread: String, id: String}
  WatchIdent{thread: String, id: String, ident: String}
  WatchEnded{thread: String, id: String, status: String, detail: String}
  WakeSeen{kind: String, target: String, result: String}
  # a message from the hub's own inbox (agents' lines, jobs, its own
  # posts), passed on by Bulk.fwd behind a token (Bulk.*)
  Bulk{m: Msg}
  # Boat's status.json, read again because it changed: the threads' Boat
  # chips (core/usage.bend's Chip)
  BoatThreads{status: String}
  # the thin index and the mail cache, built off the hub after it
  # listened (Thin.warm)
  ThinWarm{index: THIN.Index, mail: TMAIL.Cache}

# remote access: also listen on the tailnet, serve HTTPS through
# `tailscale serve`, and the token remote clients must present
type Net is Data:
  Net{tailnet: Bool, https: Bool, token: String, models: String, mtext: String, rt: K.Rt, br: Maybe<&2, Chan(BR.BMsg)>, pk: Maybe<&2, Chan(PK.PMsg)>, tm: Maybe<&2, Chan(TM.TMsg)>, machines: String, pv: Maybe<&2, Chan(PV.PMsg)>, bn: BN.Bn, managed: Bool, link: String, freecad: String, ks: Maybe<&2, Chan(KSV.KMsg)>}

# a thin request that came while the index was still being built
# (THIN.Lazy): answered, in order, when it arrives (Hub.thin.warmed)
type ThinWait is Data:
  ThinWait{client: U32, o: J.Json, now: Nat}

type ThinCache is Data:
  # store: a store.db handle (server/thinstore.bend; "": none). With one,
  # index holds the recent threads alone and pages, message reads and
  # search are queries (Thin.store.open)
  ThinCache{index: THIN.Lazy, epoch: String, revision: Nat, streams: Map<&2, TLIVE.Stream>, mail: TMAIL.Lazy, waiting: List<&2, ThinWait>, notes: TPACE.Notes, store: String}

type Cfg is Data:
  Cfg{home: String, web: String, host: String, port: U32, prompt: String, dirs: List<&2, String>, version: String, net: Net, telemetry: List<&2, TI.TmCfg>, views: List<&2, ThinCache>}

def ThinCache.first(xs: List<&2, ThinCache>) -> ThinCache:
  Maybe.default(&2, ThinCache, List.head(&2, ThinCache, xs), ThinCache{THIN.Built{THIN.Index.empty()}, "0", 0n, Map.new(&2, TLIVE.Stream), TMAIL.Built{TMAIL.Cache.empty()}, Nil{}, TPACE.Notes.empty(), ""})

def Cfg.thin.cache(cfg: Cfg) -> ThinCache:
  match cfg:
    case Cfg{_, _, _, _, _, _, _, _, _, views}: ThinCache.first(views)

def ThinCache.lazy(c: ThinCache) -> THIN.Lazy:
  match c:
    case ThinCache{i, _, _, _, _, _, _, _}: i

def ThinCache.epoch(c: ThinCache) -> String:
  match c:
    case ThinCache{_, e, _, _, _, _, _, _}: e

def ThinCache.revision(c: ThinCache) -> Nat:
  match c:
    case ThinCache{_, _, n, _, _, _, _, _}: n

def ThinCache.streams(cache: ThinCache) -> Map<&2, TLIVE.Stream>:
  match cache:
    case ThinCache{_, _, _, streams, _, _, _, _}: streams

def ThinCache.mail(cache: ThinCache) -> TMAIL.Lazy:
  match cache:
    case ThinCache{_, _, _, _, mail, _, _, _}: mail

def ThinCache.notes(cache: ThinCache) -> TPACE.Notes:
  match cache:
    case ThinCache{_, _, _, _, _, _, notes, _}: notes

def Cfg.thin.notes(cfg: Cfg) -> TPACE.Notes:
  ThinCache.notes(Cfg.thin.cache(cfg))

def ThinCache.waiting(cache: ThinCache) -> List<&2, ThinWait>:
  match cache:
    case ThinCache{_, _, _, _, _, ws, _, _}: ws

def ThinCache.store(cache: ThinCache) -> String:
  match cache:
    case ThinCache{_, _, _, _, _, _, _, h}: h

def Cfg.thin.store(cfg: Cfg) -> String:
  ThinCache.store(Cfg.thin.cache(cfg))

def Cfg.thin.mail.lazy(cfg: Cfg) -> TMAIL.Lazy:
  ThinCache.mail(Cfg.thin.cache(cfg))

# the cache itself: like Cfg.thin, read only once Hub.thin.rpc found it built
def Cfg.thin.mail(cfg: Cfg) -> TMAIL.Cache:
  TMAIL.Lazy.force(Cfg.thin.mail.lazy(cfg))

def Cfg.thin.streams(cfg: Cfg) -> Map<&2, TLIVE.Stream>:
  ThinCache.streams(Cfg.thin.cache(cfg))

def Cfg.thin.lazy(cfg: Cfg) -> THIN.Lazy:
  ThinCache.lazy(Cfg.thin.cache(cfg))

# the index itself: Hub.thin.rpc answers only once it is built, so this
# never builds it on the hub's time
def Cfg.thin(cfg: Cfg) -> THIN.Index:
  THIN.Lazy.force(Cfg.thin.lazy(cfg))

def Cfg.thin.waiting(cfg: Cfg) -> List<&2, ThinWait>:
  ThinCache.waiting(Cfg.thin.cache(cfg))

def Cfg.thin.epoch(cfg: Cfg) -> String:
  ThinCache.epoch(Cfg.thin.cache(cfg))

def Cfg.thin.revision(cfg: Cfg) -> Nat:
  ThinCache.revision(Cfg.thin.cache(cfg))

def Cfg.with_thin_cache(cfg: Cfg, cache: ThinCache) -> Cfg:
  match cfg:
    case Cfg{h, w, ho, p, pr, ds, v, nt, tc, _}: Cfg{h, w, ho, p, pr, ds, v, nt, tc, [cache]}

def Cfg.thin.next(+n: Nat) -> Nat:
  Nat.add(n, Bool.pick(Nat, Nat.is_lt(n, PRESENT.Session.nat.max()), 1n, 0n))

def Cfg.with_thin(+cfg: Cfg, index: THIN.Lazy) -> Cfg:
  Cfg.with_thin_cache(cfg, ThinCache{index, Cfg.thin.epoch(cfg), Cfg.thin.next(Cfg.thin.revision(cfg)), Cfg.thin.streams(cfg), Cfg.thin.mail.lazy(cfg), Cfg.thin.waiting(cfg), TPACE.Notes.touch(Cfg.thin.notes(cfg), "*"), Cfg.thin.store(cfg)})

def Cfg.thin.bump(+cfg: Cfg) -> Cfg:
  Cfg.with_thin(cfg, Cfg.thin.lazy(cfg))

# the index built away from the hub arrived (THIN.Lazy.warm); the waiting
# requests are taken out, to be answered now (Hub.thin.warmed)
def Cfg.thin.warm(+cfg: Cfg, i: THIN.Index, m: TMAIL.Cache) -> Cfg:
  Cfg.with_thin_cache(cfg, ThinCache{THIN.Lazy.warm(i, Cfg.thin.lazy(cfg)), Cfg.thin.epoch(cfg), Cfg.thin.revision(cfg), Cfg.thin.streams(cfg), TMAIL.Lazy.warm(m, Cfg.thin.mail.lazy(cfg)), Nil{}, Cfg.thin.notes(cfg), Cfg.thin.store(cfg)})

def Cfg.thin.wait(+cfg: Cfg, w: ThinWait) -> Cfg:
  Cfg.with_thin_cache(cfg, ThinCache{Cfg.thin.lazy(cfg), Cfg.thin.epoch(cfg), Cfg.thin.revision(cfg), Cfg.thin.streams(cfg), Cfg.thin.mail.lazy(cfg), w <> Cfg.thin.waiting(cfg), Cfg.thin.notes(cfg), Cfg.thin.store(cfg)})

def Cfg.thin.first_epoch(changes: List<&2, M.Change>, old: String) -> String:
  match changes old:
    case c <> _ "": Nat.show(J.Json.get_nat(C.Change.encode(c), "at", 0n))
    case _ _: old

# a commit reaches the index: all of it, or (with a store, which has the
# rows) only the recent threads
def Cfg.thin.lazy.committed(memory: Bool, +changes: List<&2, M.Change>, l: THIN.Lazy) -> THIN.Lazy:
  match memory:
    case True{}: THIN.Lazy.changes(changes, l)
    case False{}: THIN.Built{THIN.Index.changes.recent(changes, THIN.Lazy.force(l))}

# The thin views on a store (S1's store.db, kept by the commit path): the
# recent threads read from it once, and from then on pages, message reads
# and search are queries; nothing of history stays in the hub
def Cfg.thin.stored(+cfg: Cfg, +h: String, recent: THIN.Index) -> Cfg:
  Cfg.with_thin_cache(cfg, ThinCache{THIN.Built{recent}, Cfg.thin.epoch(cfg), Cfg.thin.next(Cfg.thin.revision(cfg)), Cfg.thin.streams(cfg), Cfg.thin.mail.lazy(cfg), Cfg.thin.waiting(cfg), Cfg.thin.notes(cfg), h})

def Thin.store.open(+cfg: Cfg, +st: M.State, +h: String) -> IO(Cfg):
  do IO<Cfg>:
    r : THIN.Index <- TSTO.Ts.recent(h, st)
    return Cfg.thin.stored(cfg, h, r)

def Cfg.thin.committed(+cfg: Cfg, +changes: List<&2, M.Change>, st: M.State) -> Cfg:
  Cfg.with_thin_cache(cfg, ThinCache{Cfg.thin.lazy.committed(String.is_empty(Cfg.thin.store(cfg)), changes, Cfg.thin.lazy(cfg)), Cfg.thin.first_epoch(changes, Cfg.thin.epoch(cfg)), Cfg.thin.next(Cfg.thin.revision(cfg)), Cfg.thin.streams(cfg), TMAIL.Lazy.committed(changes, st, Cfg.thin.mail.lazy(cfg)), Cfg.thin.waiting(cfg), TPACE.Notes.changes(Cfg.thin.notes(cfg), changes), Cfg.thin.store(cfg)})

# ---- mail perf: local wire-log length ----
# Valid unknown JSON remains a wire line without changing State.seq.
# Count the actual replay wire once, then only actual successful fresh lines.
def LogCount.n(c: Cfg) -> Nat:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, _, _, _, _, _, _, bn, _, _, _, _}, +tcfg, +views}: BN.Bn.wire_n(bn)

def LogCount.set(c: Cfg, n: Nat) -> Cfg:
  match c:
    case Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, br, pk, tm, mc, pv, bn, mg, lk, fc, ks}, +tcfg, +views}:
      Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, br, pk, tm, mc, pv, BN.Bn.with_wire_n(bn, n), mg, lk, fc, ks}, tcfg, views}

def LogCount.init(c: Cfg, +ls: SO.Lg) -> Cfg:
  LogCount.set(c, SO.Lg.n(ls))

def LogCount.committed(+c: Cfg, fresh: List<&2, String>) -> Cfg:
  LogCount.set(c, Nat.add(LogCount.n(c), List.length(&2, String, fresh)))

type ThinContext is Data:
  ThinContext{hub: String, cid: String, target: J.Json, view: String, request_revision: Nat, anchor: Nat, actors: List<&2, TA.Actor>, last_control: Nat}

type MobileStart is Data:
  MobileStart{since: Nat, origin: String, mode: String}
  ThinStart{mode: String, context: List<&2, ThinContext>}

type Client is Data:
  # look: what the client last said it shows (desk.look; core/desk.bend);
  # cr: what it was offered and has taken, and whether it still takes its
  # log (core/outbox.bend); sock: a raw descriptor on its socket (0: none);
  # legacy: let in without its pairing token (core/auth.bend), so never
  # told the pairing link (Hub.pair)
  Client{id: U32, out: Chan(Wire), look: DK.Look, cr: OB.Cr, sock: U32, legacy: Bool, mobile: List<&2, MobileStart>}

def Client.id(c: Client) -> U32:
  match c:
    case Client{id, _, _, _, _, _, _}: id

def Client.thin.mode(c: Client) -> String:
  match c:
    case Client{_, _, _, _, _, _, MobileStart{_, _, mode} <> _}: mode
    case Client{_, _, _, _, _, _, ThinStart{mode, _} <> _}: mode
    case _: ""

def Client.thin(+c: Client) -> Bool:
  String.starts_with(Client.thin.mode(c), "view-")

# what the hub keeps about a Claude process's work (a one-element list; a
# Codex or Grok turn's process has none): its command ledger
# (core/cmds.bend) and the messages held behind one whose images are still
# being read; live, heard, gen and quiet are for streamed text, silence
# and spawn generations
type ARun is Data:
  ARun{cmds: CM.Cmds, held: List<&2, CM.Held>, live: List<&2, String>, heard: Nat, gen: String, quiet: Bool, prog_at: Nat, prog_last: List<&2, String>}

type Agent is Data:
  Agent{thread: String, input: Chan(String), pid: U32, key: String, run: List<&2, ARun>}

def ARun.new() -> ARun:
  ARun{CM.Cmds.new(), Nil{}, Nil{}, 0n, "", False{}, 0n, Nil{}}

# a Claude process of generation gen (B.Gen.*)
def ARun.new.g(+gen: String) -> ARun:
  ARun{CM.Cmds.new(), Nil{}, Nil{}, 0n, gen, False{}, 0n, Nil{}}

# the same, started (heard last) at now
def ARun.new.h(+gen: String, +now: Nat) -> ARun:
  ARun{CM.Cmds.new(), Nil{}, Nil{}, now, gen, False{}, 0n, Nil{}}

def Proc.kill.k(ok: Bool, +pid: U32, +sig: U32) -> IO(Unit):
  match ok:
    case True{}:
      H.Proc.kill(pid, sig)
    case False{}:
      IO.pure(Unit, Unit{})

# a signal to a process; never to pid 0 or 1 (B.Proc.kill.ok): an agent
# not started yet is a placeholder with pid 0, and kill(0) would stop the
# hub's own process group
def Proc.kill(+pid: U32, +sig: U32) -> IO(Unit):
  Proc.kill.k(B.Proc.kill.ok(pid), pid, sig)

def Fuel.forever() -> Nat:
  U32.to_nat(4294967295)

def Log.info(+s: String) -> IO(Unit):
  PS.Log.info("hub", s)

def Now.nat() -> IO(Nat):
  do IO<Nat>:
    t : U32 <- H.Clock.epoch()
    return U32.to_nat(t)

# Files
# -----

def Files.read.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Files.read.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, U32>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>,
    File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), Files.read.done)

def Files.read.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, U32>, File.size(f), Files.read.sized)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

# a whole file's bytes, or [] when it cannot be read
def Files.read(path: String) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), Files.read.opened)

def Files.text(path: String) -> IO(String):
  do IO<String>:
    bs : List<&2, U32> <- Files.read(path)
    return U.Utf8.decode(bs)

# Event log
# ---------

# read in one pass by src/core/store.bend (ST.Store.load)
def Store.lines.k(r: U.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def Store.lines(bs: List<&2, U32>) -> List<&2, String>:
  Store.lines.k(U.Lines.feed(U.Lines.new(), bs))

def Store.path(+home: String) -> String:
  home ++ "/events.jsonl"

# Clients and agents
# ------------------

def Clients.hit(cs: List<&2, Client>, +id: U32) -> Bool:
  match cs:
    case Client{i, _, _, _, _, _, _} <> _:
      U32.is_eq(i, id)
    case Nil{}:
      False{}

def Clients.find(cs: List<&2, Client>, hit: Bool, +id: U32) -> Maybe<&2, Client>:
  match cs hit:
    case c <> _ True{}:
      Some{c}
    case _ <> +t False{}:
      Clients.find(t, Clients.hit(t, id), id)
    case _ _:
      None{}

def Clients.get(+cs: List<&2, Client>, +id: U32) -> Maybe<&2, Client>:
  Clients.find(cs, Clients.hit(cs, id), id)

def Clients.drop.k(same: Bool, c: Client, rest: List<&2, Client>) -> List<&2, Client>:
  match same:
    case True{}:
      rest
    case False{}:
      c <> rest

def Clients.drop(cs: List<&2, Client>, +id: U32) -> List<&2, Client>:
  match cs:
    case Nil{}:
      Nil{}
    case Client{+i, o, l, r, k, lgc, mb} <> t:
      Clients.drop.k(U32.is_eq(i, id), Client{i, o, l, r, k, lgc, mb}, Clients.drop(t, id))

def Clients.look.k(same: Bool, c: Client, +look: DK.Look) -> Client:
  match same c:
    case True{} Client{i, o, _, r, k, lgc, mb}:
      Client{i, o, look, r, k, lgc, mb}
    case False{} _:
      c

# a client's new look
def Clients.look(cs: List<&2, Client>, +id: U32, +look: DK.Look) -> List<&2, Client>:
  match cs:
    case Nil{}:
      Nil{}
    case Client{+i, o, l, r, k, lgc, mb} <> t:
      Clients.look.k(U32.is_eq(i, id), Client{i, o, l, r, k, lgc, mb}, look) <> Clients.look(t, id, look)

def Clients.looks(cs: List<&2, Client>) -> List<&2, DK.At>:
  match cs:
    case Nil{}:
      Nil{}
    case Client{i, _, l, _, _, _, _} <> t:
      DK.At{i, l} <> Clients.looks(t)

def Clients.cr.k(same: Bool, c: Client, +gen: U32, +n: Nat, +b: Nat) -> Client:
  match same c:
    case True{} Client{i, o, l, r, k, lgc, mb}:
      Client{i, o, l, OB.Cr.drained(r, gen, n, b), k, lgc, mb}
    case False{} _:
      c

# a writer's report on a client's credit (OB.Cr.drained)
def Clients.cr(cs: List<&2, Client>, +id: U32, +gen: U32, +n: Nat, +b: Nat) -> List<&2, Client>:
  match cs:
    case Nil{}:
      Nil{}
    case Client{+i, o, l, r, k, lgc, mb} <> t:
      Clients.cr.k(U32.is_eq(i, id), Client{i, o, l, r, k, lgc, mb}, gen, n, b) <> Clients.cr(t, id, gen, n, b)

def Clients.sockets(cs: List<&2, Client>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case Client{_, _, _, r, _, _, _} <> t:
      Nat.add(Bool.pick(Nat, OB.Cr.native(r), 0n, 1n), Clients.sockets(t))

# whether a broadcast is worth encoding once on the hub (two or more
# socket clients; a single one's writer encodes it, off the hub)
def Clients.socket(cs: List<&2, Client>) -> Bool:
  Nat.is_ge(Clients.sockets(cs), 2n)

def Clients.add(m: Maybe<&2, Client>, cs: List<&2, Client>) -> List<&2, Client>:
  match m:
    case Some{c}:
      c <> cs
    case None{}:
      cs

def Agents.hit(as: List<&2, Agent>, +th: String) -> Bool:
  match as:
    case Agent{t, _, _, _, _} <> _:
      String.eq(t, th)
    case Nil{}:
      False{}

def Agents.find(as: List<&2, Agent>, hit: Bool, +th: String) -> Maybe<&2, Agent>:
  match as hit:
    case a <> _ True{}:
      Some{a}
    case _ <> +t False{}:
      Agents.find(t, Agents.hit(t, th), th)
    case _ _:
      None{}

def Agents.get(+as: List<&2, Agent>, +th: String) -> Maybe<&2, Agent>:
  Agents.find(as, Agents.hit(as, th), th)

def Agents.drop.k(same: Bool, a: Agent, rest: List<&2, Agent>) -> List<&2, Agent>:
  match same:
    case True{}:
      rest
    case False{}:
      a <> rest

def Agents.drop(as: List<&2, Agent>, +th: String) -> List<&2, Agent>:
  match as:
    case Nil{}:
      Nil{}
    case Agent{+t, i, p, k, r} <> rest:
      Agents.drop.k(String.eq(t, th), Agent{t, i, p, k, r}, Agents.drop(rest, th))

# the threads with an agent
def Agents.threads(as: List<&2, Agent>) -> List<&2, String>:
  match as:
    case Nil{}:
      Nil{}
    case Agent{t, _, _, _, _} <> rest:
      t <> Agents.threads(rest)

def Agents.run.k(m: Maybe<&2, Agent>) -> Maybe<&2, ARun>:
  match m:
    case Some{Agent{_, _, _, _, r <> _}}:
      Some{r}
    case _:
      None{}

# a Claude process's run (None: none, or a Codex or Grok turn's)
def Agents.run(+as: List<&2, Agent>, +th: String) -> Maybe<&2, ARun>:
  Agents.run.k(Agents.get(as, th))

def Agents.gen.k(m: Maybe<&2, Agent>) -> String:
  match m:
    case Some{Agent{_, _, _, _, ARun{_, _, _, _, g, _, _, _} <> _}}:
      g
    case Some{Agent{_, _, _, +k, Nil{}}}:
      B.Gen.of(k)
    case None{}:
      ""

# the generation of the thread's agent ("" for none)
def Agents.gen(+as: List<&2, Agent>, +th: String) -> String:
  Agents.gen.k(Agents.get(as, th))

def Agents.with.k(same: Bool, a: Agent, +r: ARun) -> Agent:
  match same a:
    case True{} Agent{t, i, p, k, _}:
      Agent{t, i, p, k, [r]}
    case False{} _:
      a

# the thread's agent with its run replaced
def Agents.with(as: List<&2, Agent>, +th: String, +r: ARun) -> List<&2, Agent>:
  match as:
    case Nil{}:
      Nil{}
    case Agent{+t, i, p, k, rs} <> rest:
      Agents.with.k(String.eq(t, th), Agent{t, i, p, k, rs}, r) <> Agents.with(rest, th, r)

# Offering frames
# ---------------
# Every frame for a client goes through Wire.offer: it is counted, and a
# client too far behind (OB.Cr.over) is dropped at once (its socket shut
# through the raw descriptor, so its writer stops; its channel closed), so
# the channel never fills and no send of the hub's waits on a client.
# Socket clients get a message as CBOR (encoded once for all of them),
# the window in this process as text.

def Wire.cut(c: Client) -> IO(Unit):
  match c:
    case Client{i, o, _, _, k, _, _}:
      do IO<Unit>:
        PS.Perf.add("hub.client_drop", 1)
        Log.info("client " ++ U32.show(i) ++ " fell too far behind: dropped (it reconnects from its sequence)")
        H.Fd.shut(k)
        Chan.close(Wire, o)

def Wire.kept(over: Bool, c: Client) -> IO(Maybe<&2, Client>):
  match over:
    case True{}:
      IO.bind(Unit, Maybe<&2, Client>, Wire.cut(c), _ => IO.pure(Maybe<&2, Client>, None{}))
    case False{}:
      IO.pure(Maybe<&2, Client>, Some{c})

def Wire.charged(native: Bool, w: Wire, n: Nat) -> Wire:
  match native w:
    case False{} WText{t}: WCharged{t, n}
    case False{} WCharged{t, _}: WCharged{t, n}
    case False{} WJson{j, _}: WJson{j, n}
    case _ _: w

def Wire.offer.k(c: Client, w: Wire, +n: Nat) -> IO(Maybe<&2, Client>):
  match c:
    case Client{i, +o, l, +r, k, lgc, mb}:
      do IO<Maybe<&2, Client>>:
        +charge : Nat = Bool.pick(Nat, OB.Cr.native(r), 0n, n)
        ok : Result<&1, &1, Wire, Unit> <- Chan.send(Wire, o, Wire.charged(OB.Cr.native(r), w, charge))
        +r2 : OB.Cr = OB.Cr.offer(r, charge)
        Wire.kept(OB.Cr.over(r2), Client{i, o, l, r2, k, lgc, mb})

# one frame of n bytes for a client: kept, or None when it was dropped
def Wire.offer(c: Client, w: Wire, +n: Nat) -> IO(Maybe<&2, Client>):
  Wire.offer.k(c, w, n)

def Wire.native(c: Client) -> Bool:
  match c:
    case Client{_, _, _, r, _, _, _}:
      OB.Cr.native(r)

def Wire.offer.bs(c: Client, +bs: List<&2, U32>) -> IO(Maybe<&2, Client>):
  Wire.offer(c, WBin{bs}, List.length(&2, U32, bs))

# (a message for one client is encoded by its writer, off the hub)
def Wire.text.charge(+text: String) -> Nat:
  String.length(text)

def Wire.text.k(native: Bool, c: Client, +text: String) -> IO(Maybe<&2, Client>):
  match native:
    case True{}:
      Wire.offer(c, WText{text}, 0n)
    case False{}:
      Wire.offer(c, WText{text}, Wire.text.charge(text))

def Client.thin.reply_actors(xs: List<&2, TA.Actor>, web: Bool, key: String, text: String) -> List<&2, TA.Actor>:
  match xs:
    case Nil{}: Nil{}
    case actor <> _: [TA.Actor.reply(actor, web, key, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))]

def Client.thin.reply(c: Client, +text: String) -> Client:
  match c:
    case Client{id, out, look, cr, sock, legacy, ThinStart{+mode, ThinContext{+hub, cid, target, view, revision, anchor, actors, control} <> _} <> _}:
      Client{id, out, look, cr, sock, legacy, [ThinStart{mode, [ThinContext{hub, cid, target, view, revision, anchor,
        Client.thin.reply_actors(actors, String.eq(mode, "view-web"), hub, text), control}]}]}
    case _: c

# a message (JSON text) for one client
def Wire.text(+c: Client, +text: String) -> IO(Maybe<&2, Client>):
  Wire.text.k(Wire.native(c), Client.thin.reply(c, text), text)

def Wire.text.m(m: Maybe<&2, Client>, +text: String) -> IO(Maybe<&2, Client>):
  match m:
    case Some{c}:
      Wire.text(c, text)
    case None{}:
      IO.pure(Maybe<&2, Client>, None{})

# the same, skipped when empty
def Wire.text.if(empty: Bool, m: Maybe<&2, Client>, +text: String) -> IO(Maybe<&2, Client>):
  match empty:
    case True{}:
      IO.pure(Maybe<&2, Client>, m)
    case False{}:
      Wire.text.m(m, text)

def Client.is(c: Client, +id: U32) -> Bool:
  match c:
    case Client{i, _, _, _, _, _, _}:
      U32.is_eq(i, id)

def Wire.to.one(hit: Bool, text: Bool, c: Client, w: Wire, +n: Nat, +t: String) -> IO(Maybe<&2, Client>):
  match hit text:
    case True{} True{}:
      Wire.text(c, t)
    case True{} False{}:
      Wire.offer(c, w, n)
    case False{} _:
      IO.pure(Maybe<&2, Client>, Some{c})

def Wire.to.go(cs: List<&2, Client>, +id: U32, +w: Wire, +n: Nat, +text: Bool, +t: String) -> IO(List<&2, Client>):
  match cs:
    case Nil{}:
      IO.pure(List<&2, Client>, Nil{})
    case +c <> rest:
      do IO<List<&2, Client>>:
        m : Maybe<&2, Client> <- Wire.to.one(Client.is(c, id), text, c, w, n, t)
        r : List<&2, Client> <- Wire.to.go(rest, id, w, n, text, t)
        return Clients.add(m, r)

# a message (JSON text) for the client with this id
def Wire.to(+cs: List<&2, Client>, +id: U32, +text: String) -> IO(List<&2, Client>):
  Wire.to.go(cs, id, WClose{}, 0n, True{}, text)

# a frame as it is (a plot's CBOR, a pong) for the client with this id
# A thin control reply/notification already has validated correlation and
# cannot settle an ordinary request. Do not parse its rendered tree again
# through the ordinary reply actor hook.
def Wire.to.view(+cs: List<&2, Client>, +id: U32, +j: J.Json, +size: Nat) -> IO(List<&2, Client>):
  Wire.to.go(cs, id, WJson{j, size}, size, False{}, "")

def Wire.to.raw(+cs: List<&2, Client>, +id: U32, w: Wire, +n: Nat) -> IO(List<&2, Client>):
  Wire.to.go(cs, id, w, n, False{}, "")

def Wire.all.bin(once: Bool, c: Client, +text: String, +bs: List<&2, U32>, +n: Nat) -> IO(Maybe<&2, Client>):
  match once:
    case True{}:
      Wire.offer(c, WBin{bs}, n)
    case False{}:
      Wire.offer(c, WText{text}, String.length(text))

# (n = 0: the message was not encoded, as for a single socket client: its
# writer encodes it)
def Wire.all.as(native: Bool, c: Client, +text: String, +bs: List<&2, U32>, +n: Nat) -> IO(Maybe<&2, Client>):
  match native:
    case True{}:
      Wire.offer(c, WText{text}, 0n)
    case False{}:
      Wire.all.bin(Nat.is_gt(n, 0n), c, text, bs, n)

def Wire.booting(c: Client) -> Bool:
  match c:
    case Client{_, _, _, r, _, _, _}:
      OB.Cr.boot(r)

def Wire.behind(c: Client) -> Bool:
  match c:
    case Client{_, _, _, r, _, _, _}:
      OB.Cr.behind(r)

# how a message goes to every client: kind 0 to all; 1 log changes (a
# client taking its log gets them with the rest of it, and one too far
# behind goes back to taking it, from `at`, Cr.catch); 2 streamed text
# (none for a client taking its log: the live text comes with its end)
type All is Data:
  All{kind: Nat, log: Nat, fresh: Nat, fl: Nat, inbox: List<&2, Chan(Msg)>}

# where a client too far behind takes its log up again: the log's length
# before the changes. Counted only for such a client: counting the whole
# log (100k lines) for every step cost ~30 ms each (log: its length now)
def All.at(+log: Nat, +fresh: Nat) -> Nat:
  Nat.sub(log, fresh)

# A message the hub sends itself. The hub fiber is the inbox's only
# reader, so a send into a full inbox (4096 slots) would park it for good:
# the send runs on a fiber of its own and the hub goes on
def Self.post.go(+inbox: Chan(Msg), m: Msg) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, m)
    return Unit{}

def Self.post(+inbox: Chan(Msg), m: Msg) -> IO(Unit):
  IO.spawn(Unit, Self.post.go(inbox, m))

def Self.post.wait(+inbox: Chan(Msg), m: Msg, +ms: U32) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(ms)
    Self.post.go(inbox, m)

# the same a moment later: the next chunk of a join waits a couple of
# milliseconds, so the event loop polls the sockets between chunks and a
# request is read while a long log goes out
def Self.post.later(+inbox: Chan(Msg), m: Msg) -> IO(Unit):
  IO.spawn(Unit, Self.post.wait(inbox, m, 2))

def Wire.post(qs: List<&2, Chan(Msg)>, +m: Msg) -> IO(Unit):
  match qs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +q <> t:
      do IO<Unit>:
        Self.post(q, m)
        Wire.post(t, m)

def Wire.catch(c: Client, +a: All) -> IO(Maybe<&2, Client>):
  match c a:
    case Client{+i, o, l, +r, k, lgc, mb} All{_, +log, +fresh, +fl, +qs}:
      do IO<Maybe<&2, Client>>:
        Wire.post(qs, JoinMore{i, OB.Cr.gen(r)})
        return Some{Client{i, o, l, OB.Cr.wait.set(OB.Cr.catch(r, All.at(log, fresh), fl), True{}), k, lgc, mb}}

def Wire.all.k3(behind: Bool, +c: Client, +a: All, +text: String, +bs: List<&2, U32>, +n: Nat) -> IO(Maybe<&2, Client>):
  match behind:
    case True{}:
      Wire.catch(c, a)
    case False{}:
      Wire.all.as(Wire.native(c), c, text, bs, n)

def Wire.all.legacy.k2(kind: Nat, booting: Bool, +c: Client, +a: All, +text: String, +bs: List<&2, U32>, +n: Nat) -> IO(Maybe<&2, Client>):
  match kind booting:
    case 0n _:
      Wire.all.as(Wire.native(c), c, text, bs, n)
    case 3n _:
      Wire.all.as(Wire.native(c), c, text, bs, n)
    case _ True{}:
      IO.pure(Maybe<&2, Client>, Some{c})
    case 1n False{}:
      Wire.all.k3(Wire.behind(c), c, a, text, bs, n)
    case _ False{}:
      Wire.all.as(Wire.native(c), c, text, bs, n)

# a thin (view) client takes no broadcast but the heartbeat (kind 3): it
# hears its own view's changes, and the heartbeat tells it the link lives
# (without it an idle page took a healthy link for dead after Alive.limit)
def Wire.all.thin.k(kind: Nat, +c: Client, +text: String, +bs: List<&2, U32>, +n: Nat) -> IO(Maybe<&2, Client>):
  match kind:
    case 3n: Wire.all.as(Wire.native(c), c, text, bs, n)
    case _: IO.pure(Maybe<&2, Client>, Some{c})

def Wire.all.thin(thin: Bool, kind: Nat, booting: Bool, c: Client, a: All, text: String, bs: List<&2, U32>, n: Nat) -> IO(Maybe<&2, Client>):
  match thin:
    case True{}: Wire.all.thin.k(kind, c, text, bs, n)
    case False{}: Wire.all.legacy.k2(kind, booting, c, a, text, bs, n)

def Wire.all.k2(kind: Nat, booting: Bool, +c: Client, a: All, text: String, bs: List<&2, U32>, n: Nat) -> IO(Maybe<&2, Client>):
  Wire.all.thin(Client.thin(c), kind, booting, c, a, text, bs, n)

def All.kind(+a: All) -> Nat:
  match a:
    case All{k, _, _, _, _}:
      k

def Wire.all.go(cs: List<&2, Client>, +text: String, +bs: List<&2, U32>, +n: Nat, +a: All) -> IO(List<&2, Client>):
  match cs:
    case Nil{}:
      IO.pure(List<&2, Client>, Nil{})
    case +c <> t:
      do IO<List<&2, Client>>:
        m : Maybe<&2, Client> <- Wire.all.k2(All.kind(a), Wire.booting(c), c, a, text, bs, n)
        rest : List<&2, Client> <- Wire.all.go(t, text, bs, n, a)
        return Clients.add(m, rest)

def Wire.all.bs(cs: List<&2, Client>, +text: String, +bs: List<&2, U32>, +a: All) -> IO(List<&2, Client>):
  Wire.all.go(cs, text, bs, List.length(&2, U32, bs), a)

def Wire.all.k(sock: Bool, cs: List<&2, Client>, +text: String, +a: All) -> IO(List<&2, Client>):
  match sock:
    case True{}:
      Wire.all.bs(cs, text, W.Ws.cbor.bytes(text), a)
    case False{}:
      Wire.all.go(cs, text, Nil{}, 0n, a)

# a message for every client (encoded once); the clients still there
def Wire.all.of(+cs: List<&2, Client>, +text: String, +a: All) -> IO(List<&2, Client>):
  Wire.all.k(Clients.socket(cs), cs, text, a)

def Wire.all(+cs: List<&2, Client>, +text: String) -> IO(List<&2, Client>):
  Wire.all.of(cs, text, All{0n, 0n, 0n, 0n, Nil{}})

# the heartbeat: every client, thin (view) ones too (Wire.all.thin)
def Wire.beat(+cs: List<&2, Client>, +text: String) -> IO(List<&2, Client>):
  Wire.all.of(cs, text, All{3n, 0n, 0n, 0n, Nil{}})

def Client.legacy(c: Client) -> Bool:
  match c:
    case Client{_, _, _, _, _, lg, mb}:
      lg

# a message (the pairing link, Hub.pair_msg) for every client but those let in
# the legacy way; the clients still there
def Wire.pair(cs: List<&2, Client>, +text: String) -> IO(List<&2, Client>):
  match cs:
    case Nil{}:
      IO.pure(List<&2, Client>, Nil{})
    case +c <> t:
      do IO<List<&2, Client>>:
        m : Maybe<&2, Client> <- Wire.text.if(Client.legacy(c), Some{c}, text)
        rest : List<&2, Client> <- Wire.pair(t, text)
        return Clients.add(m, rest)

# streamed text: none for a client taking its log
def Wire.delta(+cs: List<&2, Client>, +text: String) -> IO(List<&2, Client>):
  Wire.all.of(cs, text, All{2n, 0n, 0n, 0n, Nil{}})

# log changes: every client but those taking their log (they get them with
# the rest of it); one too far behind goes back to taking it from the log's
# length before these changes (log: the log with them, fresh: how many),
# the far mirrors left out
def Wire.changes(+cs: List<&2, Client>, +text: String, +log: Nat, +fresh: Nat, +fl: Nat, +inbox: Chan(Msg)) -> IO(List<&2, Client>):
  Wire.all.of(cs, text, All{1n, log, fresh, fl, [inbox]})

# Agent processes
# ---------------

# Is the desktop's login keyring locked (core/keyring.bend)? Asked of the
# Secret Service, read only, 2 s at most; BACKPLANE_KEYRING_LOCKED=1/0
# answers for tests. No bus or no service: unknown (0), never an error.
def Keyring.asked(r: P.Ran) -> Nat:
  match r:
    case P.Ran{0, +out}:
      KR.Keyring.of(out)
    case _:
      0n

def Keyring.probe.k(+set: Nat) -> IO(Nat):
  Bool.pick(IO(Nat), Nat.is_lt(set, 3n), IO.pure(Nat, set),
    IO.bind(P.Ran, Nat, P.Proc.run("sh", PT.Port.timeout(["2", "busctl", "--user", "get-property", "org.freedesktop.secrets", "/org/freedesktop/secrets/collection/login", "org.freedesktop.Secret.Collection", "Locked"]), "/"),
      r => IO.pure(Nat, Keyring.asked(r))))

def Keyring.env.of(r: Result<&1, &1, U32 & String, String>) -> Nat:
  match r:
    case Done{+v}:
      KR.Keyring.env(v)
    case Fail{_}:
      3n

def Keyring.probe() -> IO(Nat):
  IO.bind(Result<&1, &1, U32 & String, String>, Nat, IO.get_env("BACKPLANE_KEYRING_LOCKED"), r => Keyring.probe.k(Keyring.env.of(r)))

# (only when it may matter: an agent that ended with an error)
def Keyring.probe.if(go: Bool) -> IO(Nat):
  match go:
    case True{}:
      Keyring.probe()
    case False{}:
      IO.pure(Nat, 0n)

# a provider that needs the keyring is only started while it is not
# locked (law keyring_locked_never_starts); start runs it, refuse fails the
# turn with the words; one that does not need it never asks
def Keyring.gate.k(go: Bool, start: IO(Unit), refuse: IO(Unit)) -> IO(Unit):
  match go:
    case True{}:
      start
    case False{}:
      refuse

def Keyring.gate.n(needs: Bool, +provider: String, start: IO(Unit), refuse: IO(Unit)) -> IO(Unit):
  match needs:
    case True{}:
      IO.bind(Nat, Unit, Keyring.probe(), k => Keyring.gate.k(KR.Keyring.start(provider, k), start, refuse))
    case False{}:
      start

def Keyring.gate(+provider: String, start: IO(Unit), refuse: IO(Unit)) -> IO(Unit):
  Keyring.gate.n(KR.Keyring.needs(provider), provider, start, refuse)

# lines from the agent's stdout go to the hub; EOF reports the exit
def Agent.lines(ls: List<&2, String>, +thread: String, +inbox: Chan(Msg)) -> IO(Unit):
  match ls:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case l <> t:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AgentLine{thread, l})
        Agent.lines(t, thread, inbox)

def Agent.end(s: Socket, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Socket.close(s)
    +code : U32 <- H.Proc.wait(pid)
    AIO.Adopt.drop(thread, pid)
    +tail : String <- H.Proc.errtail(pid)
    +k : Nat <- Keyring.probe.if(U32.is_ne(code, 0))
    +why : String = KR.Keyring.say(ER.Exit.say("agent exited", B.Exit.why(code), code, tail), k)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AgentGone{thread, code, pid, why})
    return Unit{}

def Agent.pause(streaming: Bool) -> IO(Unit):
  match streaming:
    case True{}:
      IO.sleep(B.Delta.pace())
    case False{}:
      IO.pure(Unit, Unit{})

# a read's lines, text deltas merged (B.Delta.merge), then a moment's wait
# when it ended mid-stream, so the next read takes what came meanwhile
def Agent.fed.k(+ls: List<&2, String>, s: Socket, carry: U.Lines, +thread: String, +inbox: Chan(Msg), k: Socket -> U.Lines -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    Agent.lines(B.Delta.merge(ls), thread, inbox)
    Agent.pause(B.Delta.streaming(ls))
    k(s, carry)

def Agent.fed(r: U.Lines & List<&2, String>, s: Socket, +thread: String, +inbox: Chan(Msg), k: Socket -> U.Lines -> IO(Unit)) -> IO(Unit):
  (carry, ls) = r
  Agent.fed.k(ls, s, carry, thread, inbox, k)

# (lines go out under the tag, B.Tag.of(thread, pid), so a replaced
# process is not heard; the exit names the plain thread and pid)
# the stream ended: a last line with no newline still counts (a result
# written just before the process died), then the exit
def Agent.last(carry: U.Lines, s: Socket, +pid: U32, +thread: String, +gen: String, +tag: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +rest : String = U.Lines.rest(carry)
    Agent.lines(Bool.pick(List<&2, String>, String.is_empty(String.trim(rest)), Nil{}, [rest]), tag, inbox)
    Agent.end(s, pid, thread, gen, inbox)

def Agent.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, carry: U.Lines, +pid: U32, +thread: String, +gen: String, +tag: String, +inbox: Chan(Msg),
    k: Socket -> U.Lines -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Agent.last(carry, s, pid, thread, gen, tag, inbox)
    case Done{bs}:
      Agent.fed(U.Lines.feed(carry, bs), s, tag, inbox, k)
    case Fail{_}:
      Agent.last(carry, s, pid, thread, gen, tag, inbox)

def Agent.reader(fuel: Nat, s: Socket, carry: U.Lines, +pid: U32, +thread: String, +gen: String, +tag: String, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Agent.end(s, pid, thread, gen, inbox)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => Agent.got(r, carry, pid, thread, gen, tag, inbox, s2 => c2 => Agent.reader(f, s2, c2, pid, thread, gen, tag, inbox)))

def Agent.wrote(r: Socket & Result<&1, &1, U32 & String, Unit>, k: Socket -> IO(Unit)) -> IO(Unit):
  (s, _) = r
  k(s)

def Agent.write(m: Maybe<&1, String>, s: Socket, k: Socket -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{line}:
      IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.send_text(s, line ++ "\n"), r => Agent.wrote(r, k))
    case None{}:
      Socket.close(s)

# lines from the hub go to the agent's stdin until the channel closes
def Agent.writer(fuel: Nat, s: Socket, +input: Chan(String)) -> IO(Unit):
  match fuel:
    case 0n:
      Socket.close(s)
    case 1n+f:
      IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, input),
        m => Agent.write(m, s, s2 => Agent.writer(f, s2, input)))

# Connections
# -----------

def Conn.sent(r: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(Socket):
  (s, _) = r
  IO.pure(Socket, s)

def Conn.send(s: Socket, bs: List<&2, U32>) -> IO(Socket):
  IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Socket, H.Sock.send_bytes(s, bs), Conn.sent)

def Conn.finish(s: Socket, bs: List<&2, U32>) -> IO(Unit):
  do IO<Unit>:
    s2 : Socket <- Conn.send(s, bs)
    Socket.close(s2)

# a message as a frame: always one binary CBOR frame (about 40% fewer
# bytes on the air than JSON text; every client speaks it). A phone's plot
# is CBOR already and goes as WBin, never through here
def Conn.frame.out(t: String) -> List<&2, U32>:
  W.Ws.cbor(t)

# A client's frames go out with a deadline each (Conn.deadline, ms): a
# peer that stops reading (a half-open link, a phone that no longer reads)
# fails the send, and then its channel is closed first, so the hub's sends
# to it end at once instead of waiting on a full queue, and the socket is
# closed (the effect shut it both ways: its reader ends and it leaves)
def Conn.deadline() -> U32:
  10000

# what was queued for a client whose send failed is taken out, so the
# closed channel's row is freed (a closed channel is freed only once it is
# empty) and its frames are not held for good
def Conn.drain.got(m: Maybe<&1, Wire>, k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{_}:
      k(Unit{})
    case None{}:
      IO.pure(Unit, Unit{})

def Conn.drain(fuel: Nat, +out: Chan(Wire)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, Wire>, Unit, Chan.recv(Wire, out), m => Conn.drain.got(m, u => Conn.drain(f, out)))

def Conn.sent.k(r: Socket & Result<&1, &1, U32 & String, Unit>, +out: Chan(Wire), k: Socket -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{_}:
      k(s)
    case Fail{_}:
      do IO<Unit>:
        Chan.close(Wire, out)
        Conn.drain(Fuel.forever(), out)
        Socket.close(s)

def Conn.out(s: Socket, bs: List<&2, U32>, +out: Chan(Wire), k: Socket -> IO(Unit)) -> IO(Unit):
  IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.send_until(s, bs, Conn.deadline()), r => Conn.sent.k(r, out, k))

def Conn.written(s: Socket, size: Nat, credit: Nat, k: Socket -> Nat -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    PS.Perf.add("net.write.bytes", U32.from_nat(size))
    k(s, credit)

# a frame out, then k with its size in bytes
def Conn.put(s: Socket, +bs: List<&2, U32>, +out: Chan(Wire), k: Socket -> Nat -> IO(Unit)) -> IO(Unit):
  +n = List.length(&2, U32, bs)
  Conn.out(s, bs, out, s2 => Conn.written(s2, n, n, k))

# a join chunk's frames (OB.Items.texts: each within the byte bound), put
# out one by one; to the writer's count they are one frame of no bytes, as
# the hub offered it (it charged none)
# the wire's dictionaries as host.js's send_cbor takes them
def Conn.keys() -> String:
  String.join(CB.Cbor.keys(), "\n")

def Conn.words() -> String:
  String.join(CB.Cbor.words(), "\n")

# a chunk's frame out: its CBOR made and sent in the host (H.Sock.send_cbor, the
# same bytes as W.Ws.cbor): Bend's encoder sent a long log's join at ~1.5 MB/s
def Conn.out.cbor(s: Socket, +t: String, +out: Chan(Wire), k: Socket -> IO(Unit)) -> IO(Unit):
  IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.send_cbor(s, t, Conn.keys(), Conn.words(), Conn.deadline()), r => Conn.sent.k(r, out, k))

# a join frame's lines out as OB.Items.texts.s splits them, each frame's
# CBOR: measured, joined, encoded and sent in the host (H.Sock.send_items, the same
# bytes, test/native/cbor_frame_test.bend)
def Conn.out.items(s: Socket, +hd: String, +pl: String, +sp: String, +xs: List<&2, String>, +tl: String, +ends: Bool, +out: Chan(Wire), k: Socket -> Nat -> IO(Unit)) -> IO(Unit):
  IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit,
    H.Sock.send_items(s, hd, pl, sp, xs, tl, Bool.pick(U32, ends, 1, 0), U32.from_nat(OB.Join.most()), U32.from_nat(OB.Join.bytes()), Conn.keys(), Conn.words(), Conn.deadline()),
    r => Conn.sent.k(r, out, s2 => k(s2, 0n)))

def Conn.write(m: Maybe<&1, Wire>, s: Socket, +out: Chan(Wire), k: Socket -> Nat -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{WText{t}}:
      Conn.put(s, Conn.frame.out(t), out, k)
    case Some{WCharged{t, charge}}:
      # Credit is in the units offered by Wire.text.k. CBOR compression
      # changes the wire size; refunding that size leaves phantom backlog
      # after enough RPCs, so catch-up and streamed deltas stop forever.
      # A failed/cancelled write never calls k: Conn.sent.k closes/drains
      # the connection and Leave retires its credit, as before.
      +bs = Conn.frame.out(t)
      Conn.out(s, bs, out, s2 => Conn.written(s2, List.length(&2, U32, bs), charge, k))
    case Some{WPong{p}}:
      Conn.put(s, W.Ws.encode(10, p), out, k)
    case Some{WBin{bs}}:
      Conn.put(s, W.Ws.encode(2, bs), out, k)
    case Some{WClose{}}:
      Conn.finish(s, W.Ws.encode(8, Nil{}))
    case Some{WState{_, _}}:
      k(s, 0n)
    case Some{WItems{hd, pl, sp, xs, tl, ends}}:
      Conn.out.items(s, hd, pl, sp, xs, tl, ends, out, k)
    case Some{WJson{j, charge}}:
      +bs = W.Ws.encode(2, CB.Cbor.encode(j))
      Conn.out(s, bs, out, s2 => Conn.written(s2, List.length(&2, U32, bs), charge, k))
    case None{}:
      Socket.close(s)

# what the writer has put out, told to the hub every 32 frames or 256 KB
# (OB.Wr.due), so it knows how far behind the client is
def Conn.told(due: Bool, +w: OB.Wr, +id: U32, +gen: U32, +inbox: Chan(Msg)) -> IO(OB.Wr):
  match due:
    case True{}:
      do IO<OB.Wr>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Drained{id, gen, OB.Wr.n(w), OB.Wr.b(w)})
        return OB.Wr.said(w)
    case False{}:
      IO.pure(OB.Wr, w)

def Conn.wrote(s: Socket, +n: Nat, w: OB.Wr, +id: U32, +gen: U32, +inbox: Chan(Msg), k: Socket -> OB.Wr -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    +w2 : OB.Wr = OB.Wr.put(w, n)
    w3 : OB.Wr <- Conn.told(OB.Wr.due(w2), w2, id, gen, inbox)
    k(s, w3)

# frames from the hub, until it closes our channel
def Conn.writer(fuel: Nat, s: Socket, +out: Chan(Wire), w: OB.Wr, +id: U32, +gen: U32, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Socket.close(s)
    case 1n+f:
      IO.bind(Maybe<&1, Wire>, Unit, Chan.recv(Wire, out),
        m => Conn.write(m, s, out, s2 => n => Conn.wrote(s2, n, w, id, gen, inbox, s3 => w2 => Conn.writer(f, s3, out, w2, id, gen, inbox))))

def Conn.leave(s: Socket, +id: U32, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Socket.close(s)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Leave{id})
    return Unit{}

# a request's CBOR as the JSON text the hub reads
def Conn.cbor.json(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{j}:
      J.Json.show(j)
    case None{}:
      ""

# Reading a client
# ----------------
# Frames are scanned without IO (Conn.scan): pings that are not due (at
# most one a second goes to the hub, which answers through the client's
# credit: OB.Pong.due), text frames and pongs are passed over in one
# pure walk, so a flood of them costs a pass over the bytes, not an
# effect each. The clock is read once per read from the socket.

type Scan is Data:
  # fr: the next frame that needs the hub (None: more bytes are needed,
  # rest is then the whole unread buffer); done: the walk stopped
  Scan{fr: Maybe<&2, W.Frame>, rest: List<&2, U32>, done: Bool}

def Conn.quiet.k(+op: U32, +last: Nat, +now: Nat) -> Bool:
  Bool.or(Bool.and(U32.is_eq(op, 9), Bool.not(OB.Pong.due(last, now))), Bool.or(U32.is_eq(op, 1), Bool.or(U32.is_eq(op, 10), U32.is_eq(op, 0))))

def Conn.quiet(fr: W.Frame, +last: Nat, +now: Nat) -> Bool:
  match fr:
    case W.Frame{_, +op, _}:
      Conn.quiet.k(op, last, now)

def Conn.step.q(quiet: Bool, fr: W.Frame, rest: List<&2, U32>) -> Scan:
  match quiet:
    case True{}:
      Scan{None{}, rest, False{}}
    case False{}:
      Scan{Some{fr}, rest, True{}}

def Conn.step(m: Maybe<&2, W.Got>, buf: List<&2, U32>, +last: Nat, +now: Nat) -> Scan:
  match m:
    case Some{W.Got{+fr, rest}}:
      Conn.step.q(Conn.quiet(fr, last, now), fr, rest)
    case None{}:
      Scan{None{}, buf, True{}}

def Conn.scan(fuel: Nat, st: Scan, +last: Nat, +now: Nat) -> Scan:
  match fuel st:
    case 0n Scan{m, r, d}:
      Scan{m, r, d}
    case 1n+f Scan{m, r, True{}}:
      Scan{m, r, True{}}
    case 1n+f Scan{_, +r, False{}}:
      Conn.scan(f, Conn.step(W.Ws.decode(r), r, last, now), last, now)

# a ping passed on to the hub
def Conn.ping(p: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +now: Nat) -> IO(Maybe<&2, Nat>):
  do IO<Maybe<&2, Nat>>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Ping{id, p})
    return Some{now}

# what a frame the hub must hear does: None ends the connection, else the
# last pong's time
def Conn.frame.k(bin: Bool, close: Bool, ping: Bool, p: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +last: Nat, +now: Nat) -> IO(Maybe<&2, Nat>):
  match bin close ping:
    case True{} _ _:
      do IO<Maybe<&2, Nat>>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Rpc{id, Conn.cbor.json(CB.Cbor.decode(p))})
        return Some{last}
    case False{} True{} _:
      IO.pure(Maybe<&2, Nat>, None{})
    case False{} False{} True{}:
      Conn.ping(p, id, inbox, now)
    case False{} False{} False{}:
      IO.pure(Maybe<&2, Nat>, Some{last})

# a frame from a client: a binary frame is a request (CBOR, read here as
# JSON text for the hub); close ends; a due ping pongs
def Conn.frame(fr: W.Frame, +id: U32, +inbox: Chan(Msg), +last: Nat, +now: Nat) -> IO(Maybe<&2, Nat>):
  match fr:
    case W.Frame{_, +op, p}:
      Conn.frame.k(U32.is_eq(op, 2), U32.is_eq(op, 8), U32.is_eq(op, 9), p, id, inbox, last, now)

# a's bytes then b's, in tail calls only: Base's List.append keeps a stack
# frame per element of a, and a socket's buffered bytes grow past what the
# JS stack holds (a 64 KB paste into a terminal is a ~200 KB WebSocket frame
# read in pieces: the hub died at the second piece)
def Bytes.rev(xs: List<&2, U32>, acc: List<&2, U32>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      Bytes.rev(t, x <> acc)

def Bytes.cat(a: List<&2, U32>, b: List<&2, U32>) -> List<&2, U32>:
  Bytes.rev(Bytes.rev(a, Nil{}), b)

def Conn.more(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, buf: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +last: Nat,
    k: Socket -> List<&2, U32> -> Nat -> Nat -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Conn.leave(s, id, inbox)
    case Done{bs}:
      IO.bind(Nat, Unit, IO.now(), now => k(s, Bytes.cat(buf, bs), last, now))
    case Fail{_}:
      Conn.leave(s, id, inbox)

def Conn.after(m: Maybe<&2, Nat>, s: Socket, rest: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +now: Nat, k: Socket -> List<&2, U32> -> Nat -> Nat -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{last}:
      k(s, rest, last, now)
    case None{}:
      Conn.leave(s, id, inbox)

def Conn.scanned(sc: Scan, s: Socket, +id: U32, +inbox: Chan(Msg), +last: Nat, +now: Nat,
    k: Socket -> List<&2, U32> -> Nat -> Nat -> IO(Unit)) -> IO(Unit):
  match sc:
    case Scan{Some{fr}, rest, _}:
      IO.bind(Maybe<&2, Nat>, Unit, Conn.frame(fr, id, inbox, last, now), go => Conn.after(go, s, rest, id, inbox, now, k))
    case Scan{None{}, rest, _}:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => Conn.more(r, rest, id, inbox, last, k))

# the frames in buf; more read when they run out
def Conn.reader(fuel: Nat, s: Socket, buf: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +last: Nat, +now: Nat) -> IO(Unit):
  match fuel:
    case 0n:
      Conn.leave(s, id, inbox)
    case 1n+f:
      Conn.scanned(Conn.scan(65536n, Scan{None{}, buf, False{}}, last, now), s, id, inbox, last, now,
        s2 => b2 => l2 => n2 => Conn.reader(f, s2, b2, id, inbox, l2, n2))

# A client let in the legacy way (core/auth.bend's Auth.legacy: no pairing
# token, trusted as this machine's or the owner's tailnet device) is told,
# after its join, to pair again (info "auth.legacy", which every client
# shows once), and it is logged. Gone with Auth.legacy.on().
def Conn.legacy(legacy: Bool, +id: U32, +inbox: Chan(Msg)) -> IO(Unit):
  match legacy:
    case True{}:
      do IO<Unit>:
        PS.Log.warn("auth", "client " ++ U32.show(id) ++ " joined without its pairing token (legacy; asked to pair again)")
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Push{id, "{\"t\":\"info\",\"info\":{\"auth.legacy\":\"1\"}}"})
        return Unit{}
    case False{}:
      IO.pure(Unit, Unit{})

def Conn.join.mobile(mobile: String, +id: U32, out: Chan(Wire), since: Nat, origin: String, fd: U32, legacy: Bool) -> Msg:
  match mobile:
    case "": Join{id, out, since, origin, id, fd, legacy}
    case _: MobileJoin{id, out, since, origin, id, fd, legacy, mobile}

def Conn.raw(r: Socket & U32, rd: Socket, rest: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +since: Nat, +origin: String, +legacy: Bool, +mobile: String) -> IO(Unit):
  (wr, +fd) = r
  do IO<Unit>:
    +out : Chan(Wire) <- Chan.new(Wire, 1024)
    IO.spawn(Unit, Conn.writer(Fuel.forever(), wr, out, OB.Wr.new(), id, id, inbox))
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Conn.join.mobile(mobile, id, out, since, origin, fd, legacy))
    Conn.legacy(legacy, id, inbox)
    Conn.reader(Fuel.forever(), rd, rest, id, inbox, 0n, 0n)

def Conn.ws.dup(r: Socket & Result<&1, &1, U32 & String, Socket>, rest: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +since: Nat, +origin: String, legacy: Bool, +mobile: String) -> IO(Unit):
  (rd, res) = r
  match res:
    case Done{wr}:
      IO.bind(Socket & U32, Unit, H.Sock.raw(wr), x => Conn.raw(x, rd, rest, id, inbox, since, origin, legacy, mobile))
    case Fail{_}:
      Socket.close(rd)

def Conn.ws(s: Socket, key: String, rest: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +since: Nat, +origin: String, legacy: Bool, +mobile: String) -> IO(Unit):
  do IO<Unit>:
    s2 : Socket <- Conn.send(s, W.Ws.accept(key))
    r : Socket & Result<&1, &1, U32 & String, Socket> <- H.Sock.dup(s2)
    Conn.ws.dup(r, rest, id, inbox, since, origin, legacy, mobile)

def Conn.safe(+p: String) -> Bool:
  Bool.and(Bool.not(String.contains(p, "..")), String.starts_with(p, "/"))

def Conn.file(+p: String) -> String:
  Bool.pick(String, String.eq(p, "/"), "/index.html", p)

# hashed bundle chunks never change under their name; the page and the
# service worker are checked every time
def Conn.cache(+path: String) -> String:
  Bool.pick(String, String.starts_with(path, "/chunk-"), "public, max-age=31536000, immutable", "no-cache")

# One authentication boundary for browser, native, upload and MCP requests.
def Conn.allowed(+trusted: Bool, +hs: List<&2, W.Header>, +path: String, +token: String) -> Bool:
  AUTH.Auth.allowed(trusted, hs, path, token)

# a websocket from this machine's loopback, not proxied by tailscale serve,
# is marked for the hub: only such a window (or the one in this process)
# may embed the browser (core/cef.bend's Cef.from)
def Conn.near.say(near: Bool, +id: U32, +inbox: Chan(Msg)) -> IO(Unit):
  match near:
    case True{}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Near{id})
        return Unit{}
    case False{}:
      IO.pure(Unit, Unit{})

def Conn.near.k(r: Socket & String, +key: String, rest: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +since: Nat, +origin: String, legacy: Bool, +mobile: String) -> IO(Unit):
  (s, +ip) = r
  do IO<Unit>:
    Conn.near.say(CF.Cef.loopback(ip), id, inbox)
    Conn.ws(s, key, rest, id, inbox, since, origin, legacy, mobile)

def Conn.near(plain: Bool, s: Socket, +key: String, rest: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +since: Nat, +origin: String, legacy: Bool, +mobile: String) -> IO(Unit):
  match plain:
    case True{}:
      IO.bind(Socket & String, Unit, H.Sock.peer(s), r => Conn.near.k(r, key, rest, id, inbox, since, origin, legacy, mobile))
    case False{}:
      Conn.ws(s, key, rest, id, inbox, since, origin, legacy, mobile)

def Conn.gate(ok: Bool, plain: Bool, s: Socket, +key: String, rest: List<&2, U32>, +id: U32, +inbox: Chan(Msg), +since: Nat, +origin: String, legacy: Bool, +mobile: String) -> IO(Unit):
  match ok:
    case True{}:
      Conn.near(plain, s, key, rest, id, inbox, since, origin, legacy, mobile)
    case False{}:
      Conn.finish(s, W.Http.text("401 Unauthorized", "this Backplane needs its pairing token"))

def Conn.is_ws(req: W.Request) -> Bool:
  match req:
    case W.Request{_, p, +hs}:
      Bool.and(String.eq(W.Http.route(p), "/ws"), String.eq(String.to_lower(W.Http.header_of(hs, "upgrade")), "websocket"))

def Conn.is_mcp(req: W.Request) -> Bool:
  match req:
    case W.Request{m, p, _}:
      Bool.and(String.eq(m, "POST"), String.starts_with(W.Http.route(p), "/mcp/"))

def Mcp.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, buf: List<&2, U32>, k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Socket.close(s)
    case Done{bs}:
      k(s, Bytes.cat(buf, bs))
    case Fail{_}:
      Socket.close(s)

def Mcp.answer.k(empty: Bool, s: Socket, text: String) -> IO(Unit):
  match empty:
    case True{}:
      Conn.finish(s, W.Http.response("202 Accepted", "application/json", Nil{}, ""))
    case False{}:
      Conn.finish(s, W.Http.response("200 OK", "application/json", U.Utf8.encode(text), ""))

def Mcp.answer(m: Maybe<&1, String>, s: Socket) -> IO(Unit):
  match m:
    case Some{+text}:
      Mcp.answer.k(String.is_empty(text), s, text)
    case None{}:
      Conn.finish(s, W.Http.text("503 Service Unavailable", "the hub is gone"))

# the call counts as in flight (for a safe deploy) from its arrival until
# its answer is written
def Mcp.ask.raw(s: Socket, +body: String, +thread: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +t0 : U32 <- PS.Perf.us()
    +call : String = U32.show(t0) ++ "." ++ Nat.show(String.length(body))
    ok0 : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, McpOpen{thread, call, body})
    +reply : Chan(String) <- Chan.new(String, 1)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Mcp{thread, body, reply})
    m : Maybe<&1, String> <- Chan.recv(String, reply)
    Mcp.answer(m, s)
    ok2 : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, McpShut{thread, call})
    return Unit{}

def Mcp.ask(s: Socket, +body: String, +thread: String, +inbox: Chan(Msg)) -> IO(Unit):
  Mcp.ask.raw(s, body, thread, inbox)

def Mcp.body.k(full: Bool, s: Socket, +buf: List<&2, U32>, +thread: String, +inbox: Chan(Msg), k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  match full:
    case True{}:
      Mcp.ask(s, U.Utf8.decode(buf), thread, inbox)
    case False{}:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536), r => Mcp.got(r, buf, k))

# read until the body is complete
def Mcp.body(fuel: Nat, s: Socket, +buf: List<&2, U32>, +len: Nat, +thread: String, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Conn.finish(s, W.Http.text("413 Payload Too Large", "too large"))
    case 1n+f:
      Mcp.body.k(Nat.is_ge(List.length(&2, U32, buf), len), s, buf, thread, inbox, s2 => b2 => Mcp.body(f, s2, b2, len, thread, inbox))

# MCP over HTTP
# -------------
# One POST per JSON-RPC message; the reply is JSON (or 202 for a
# notification). The hub answers on a channel made for this request.

# an MCP call with neither the token nor its key (an agent the hub started
# before keys, or another program on this machine): let in the legacy way
# and logged; goes with the legacy way (core/auth.bend)
def Mcp.legacy(legacy: Bool, +thread: String) -> IO(Unit):
  match legacy:
    case True{}:
      PS.Log.warn("auth", "an MCP call for " ++ thread ++ " came without its key (legacy)")
    case False{}:
      IO.pure(Unit, Unit{})

def Mcp.gate(ok: Bool, legacy: Bool, s: Socket, +thread: String, +len: Nat, rest: List<&2, U32>, +inbox: Chan(Msg)) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(Unit, Unit, Mcp.legacy(legacy, thread), _ => Mcp.body(64n, s, rest, len, thread, inbox))
    case False{}:
      Conn.finish(s, W.Http.text("401 Unauthorized", "this Backplane needs its pairing token"))

def Conn.head.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, buf: List<&2, U32>, k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Socket.close(s)
    case Done{bs}:
      k(s, Bytes.cat(buf, bs))
    case Fail{_}:
      Socket.close(s)

# The hub
# -------

type Hub is Type:
  Hub{state: M.State, log: File, lines: SO.Lg, clients: List<&2, Client>, agents: List<&2, Agent>, inbox: Chan(Msg), cfg: Cfg, note: String}

def Hub.changes_msg(ls: List<&2, String>) -> String:
  "{\"t\":\"changes\",\"items\":[" ++ String.join(ls, ",") ++ "]}"

def Info.kicad(rt: K.Rt) -> List<&2, J.KV>:
  match rt:
    case K.Rt{cli, +fork, +v, c, +h}:
      [J.KV{"kicad.cli", J.Str{cli}}, J.KV{"kicad.fork", J.Str{Bool.pick(String, fork, v, "")}}, J.KV{"kicad.commit", J.Str{c}},
       J.KV{"kicad.status", J.Str{J.Json.get_str(h, "status", "checking")}}, J.KV{"kicad.source", J.Str{J.Json.get_str(h, "source", "")}},
       J.KV{"kicad.version", J.Str{v}}, J.KV{"kicad.api", J.Str{Nat.show(J.Json.get_nat(h, "api", 0n))}},
       J.KV{"kicad.caps", J.Str{String.join(KH.KH.strings(J.Json.elems(J.Json.get_obj(h, "caps"))), ",")}},
       J.KV{"kicad.why", J.Str{J.Json.get_str(h, "why", "")}}, J.KV{"kicad.min", J.Str{KH.KH.min_version()}},
       J.KV{"kicad.can_install", J.Str{Bool.pick(String, J.Json.get_bool(h, "can_install", False{}), "1", "")}},
       J.KV{"kicad.ipc.servers", J.Str{J.Json.get_str(h, "ipc", "")}}]

# the pairing link as a QR code's rows (core/qr.bend), for Settings
def Info.qr.k(none: Bool, +lk: String) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      QR.Qr.text(QR.Qr.link(lk))

# the pairing link's QR code; none for no link
def Info.qr(+lk: String) -> String:
  Info.qr.k(String.is_empty(lk), lk)

# the tailnet side as clients are told it ("tailscale"): off (the flag),
# checking (no answer yet), unavailable, or reachable
def Info.tailscale(tn: Bool, +st: String) -> String:
  Bool.pick(String, tn, Bool.pick(String, String.is_empty(st), "checking", st), "off")

# the bots' browsers: who drives, what is being taught (core/teach.bend)
def Info.tch(n: Net) -> List<&2, J.KV>:
  match n:
    case Net{_, _, _, _, _, _, _, _, _, _, _, +bnx, _, _, _, _}:
      List.append(&2, J.KV, TE.Teach.info(BN.Bn.tch(bnx)), List.append(&2, J.KV, CF.Cef.session.info(BN.Bn.cef(bnx)), CF.Cef.pages.info(CF.Cef.session.pages(BN.Bn.cef(bnx)))))

def Info.net(n: Net) -> List<&2, J.KV>:
  match n:
    case Net{+tn, hs, _, ms, _, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, +lk, fc, ksx}:
      List.append(&2, J.KV, [J.KV{"models", J.Str{ms}}, J.KV{"machines", J.Str{mc}}, J.KV{"bots.remote", J.Str{BN.Bn.dir(bnx)}}, J.KV{"tailnet", J.Str{Bool.pick(String, tn, Bool.pick(String, hs, "https", "on"), "off")}},
        J.KV{"update", J.Str{Bool.pick(String, mgd, "package", "releases")}}, J.KV{"tailscale", J.Str{Info.tailscale(tn, TN.Tailnet.report.state(lk))}}, J.KV{"freecad", J.Str{fc}}], Info.kicad(rt))

# what this machine runs, for the settings screen and the model picker
def Hub.info(c: Cfg) -> J.Json:
  match c:
    case Cfg{+home, _, host, port, _, _, _, +net, +tcfg, +views}:
      J.Json.obj(List.append(&2, J.KV, [J.KV{"telemetry.channel", J.Str{TI.Tio.channel(tcfg)}}, J.KV{"telemetry.id", J.Str{TI.Tio.id(tcfg)}}, J.KV{"telemetry.live", J.Str{Bool.pick(String, TI.Tio.can(tcfg, 0n), "1", "")}}, J.KV{"os", J.Str{TI.Tio.platform(tcfg, False{})}}, J.KV{"arch", J.Str{TI.Tio.platform(tcfg, True{})}}, J.KV{"home", J.Str{home}}, J.KV{"listen", J.Str{host ++ ":" ++ U32.show(port)}}], List.append(&2, J.KV, Info.net(net), Info.tch(net))))

# the pairing link (the tailnet address and the token) and its QR code: not
# in the info every client hears, but told only to clients that came with
# the token (Hub.join.k, Wire.pair), never to one let in the legacy way
# the pairing message for a link report (its QR code is made here: ~2 s on
# the JS target, so Cfg.with_link keeps it in Bn and joins reuse it)
def Hub.pair_msg.of(+lk: String) -> String:
  "{\"t\":\"info\",\"info\":" ++ J.Json.show(J.Json.obj([J.KV{"pair", J.Str{TN.Tailnet.report.link(lk)}}, J.KV{"pair.qr", J.Str{Info.qr(TN.Tailnet.report.link(lk))}}])) ++ "}"

def Hub.pair_msg.k(hit: Bool, +kept: String, +lk: String) -> String:
  match hit:
    case True{}:
      String.drop(kept, 1n+String.length(lk))
    case False{}:
      Hub.pair_msg.of(lk)

# the message kept for this link, else made (law pair_msg_kept)
def Hub.pair_msg.from(+kept: String, +lk: String) -> String:
  Hub.pair_msg.k(String.starts_with(kept, lk ++ "\n"), kept, lk)

def Hub.pair_msg(c: Cfg) -> String:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, _, _, _, _, _, _, bnx, _, +lk, _, _}, _, +views}:
      Hub.pair_msg.from(BN.Bn.pair(bnx), lk)

# A link change is handled after it has been stored, through the inbox,
# so both sides of an invite refresh immediately (revokes prune rows).
def Bot.dir.changed(c: M.Change, +inbox: Chan(Msg)) -> IO(Unit):
  match c:
    case M.PeerSet{peer, _, _, _, _}:
      Self.post(inbox, BotsRefresh{peer})
    case _:
      IO.pure(Unit, Unit{})

# One write per step
# -----------------
# A step's changes go to the log in one write (ST.Store.text), and only once
# it is written are they folded in, announced and their effects run. A
# write that fails folds nothing of the step and runs none of its effects;
# the hub logs it, counts it (hub.write_fail) and tells every client, and
# keeps serving (the window lives in this process).

# what a step's write did: the new lines for clients (newest first), or
# why it failed
type Wrote is Data:
  Wrote{fresh: List<&2, String>, ok: Bool, err: String}

def Hub.lines.disk(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      J.Json.show(C.Change.encode(c)) <> Hub.lines.disk(t)

# the written changes folded in: the state, the commit's store statements
# (SO.Cm: the log's count and the lines' facts and records) and the new
# lines for clients (newest first)
type Folded is Data:
  Folded{st: M.State, cm: SO.Cm, fresh: List<&2, String>}

def Hub.fold.k(r: M.State & SO.Cm, +w: String, fr: List<&2, String>) -> Folded:
  (st, cm) = r
  Folded{st, cm, w <> fr}

# (ds: the changes' disk lines, Hub.lines.disk: each change is shown once
# per commit and its line reused for clients, B.Commit.line; the state
# after each is the store's step's, SO.Sto.step)
def Hub.fold(cs: List<&2, M.Change>, ds: List<&2, String>, f: Folded) -> Folded:
  match cs ds f:
    case +c <> t +d <> dt Folded{+st, cm, fr}:
      +w = B.Commit.line(c, d)
      Hub.fold(t, dt, Hub.fold.k(SO.Sto.step(st, c, d, w, cm), w, fr))
    case _ _ _:
      f

def Bot.dir.changed.all(cs: List<&2, M.Change>, +inbox: Chan(Msg)) -> IO(Unit):
  match cs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case c <> t:
      do IO<Unit>:
        Bot.dir.changed(c, inbox)
        Bot.dir.changed.all(t, inbox)

def Hub.commit.k(fd: Folded, +cs: List<&2, M.Change>, f: File, cl: List<&2, Client>, as: List<&2, Agent>, inbox: Chan(Msg), cfg: Cfg, note: String) -> IO(Hub & Wrote):
  match fd:
    case Folded{+st, cm, +fr}:
      do IO<Hub & Wrote>:
        ls : SO.Lg <- SO.Sto.commit(cm)
        return (Hub{st, f, ls, cl, as, inbox, LogCount.committed(Cfg.thin.committed(cfg, cs, st), fr), note}, Wrote{fr, True{}, ""})

def Hub.err.msg(e: U32 & String) -> String:
  (_, msg) = e
  msg

def Hub.commit.r(r: Hub & Wrote, fresh: List<&2, String>) -> Hub & List<&2, String>:
  (h, w) = r
  match w:
    case Wrote{fr, _, _}:
      (h, List.append(&2, String, fr, fresh))

# a generation's name: when it started and the log's length then
def Hub.gen(+st: M.State, +now: Nat) -> String:
  Nat.show(now) ++ "-" ++ Nat.show(M.State.seq(st))

# An agent's watcher: each second it asks whether the process exited (not
# reaping it: the reader's Agent.end does). When it has, the hub hears so
# and, if the reader has not reached end of stream 2 s later (a child of
# the agent keeps its output open), the reading side is shut so it does;
# its last line is read and the exit handled once. It ends with the
# process, so none outlives its generation.
def Agent.watch.end(+code: U32, s: Socket, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AgentExited{thread, gen, code})
    IO.sleep(2000)
    s2 : Socket <- H.Sock.shut_read(s)
    Socket.close(s2)

def Agent.watch.k(running: Bool, +code: U32, s: Socket, +thread: String, +gen: String, +inbox: Chan(Msg), k: Socket -> IO(Unit)) -> IO(Unit):
  match running:
    case True{}:
      do IO<Unit>:
        IO.sleep(1000)
        k(s)
    case False{}:
      Agent.watch.end(code, s, thread, gen, inbox)

def Agent.watch.got(+code: U32, s: Socket, +thread: String, +gen: String, +inbox: Chan(Msg), k: Socket -> IO(Unit)) -> IO(Unit):
  Agent.watch.k(U32.is_eq(code, 4294967295), code, s, thread, gen, inbox, k)

def Agent.watch(fuel: Nat, s: Socket, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Socket.close(s)
    case 1n+f:
      IO.bind(U32, Unit, H.Proc.exited(pid), code => Agent.watch.got(code, s, thread, gen, inbox, s2 => Agent.watch(f, s2, pid, thread, gen, inbox)))

def Agent.watch.start(r: Socket & Result<&1, &1, U32 & String, Socket>, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Socket):
  (rd, res) = r
  match res:
    case Done{dup}:
      do IO<Socket>:
        IO.spawn(Unit, Agent.watch(Fuel.forever(), dup, pid, thread, gen, inbox))
        return rd
    case Fail{_}:
      IO.pure(Socket, rd)

def Hub.spawned.dup(r: Socket & Result<&1, &1, U32 & String, Socket>, +pid: U32, h: Hub, +thread: String, +key: String, +gen: String) -> IO(Hub):
  (rd0, res) = r
  match res h:
    case Done{wr} Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        +input : Chan(String) <- Chan.new(String, 256)
        +now : Nat <- Now.nat()
        IO.spawn(Unit, Agent.writer(Fuel.forever(), wr, input))
        rd : Socket <- IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Socket, H.Sock.dup(rd0), d => Agent.watch.start(d, pid, thread, gen, inbox))
        IO.spawn(Unit, Agent.reader(Fuel.forever(), rd, U.Lines.new(), pid, thread, gen, B.Tag.of(thread, pid), inbox))
        return Hub{st, f, ls, cl, Agent{thread, input, pid, key, [ARun.new.h(gen, now)]} <> as, inbox, cfg, note}
    # the process started but its socket could not be shared: it is
    # stopped and goes through the usual exit (Agent.end, AgentGone), so
    # its turn fails, its asks close and its task ends
    case Fail{_} Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        +input : Chan(String) <- Chan.new(String, 1)
        Chan.close(String, input)
        Proc.kill(pid, 15)
        IO.spawn(Unit, Agent.end(rd0, pid, thread, gen, inbox))
        return Hub{st, f, ls, cl, Agent{thread, input, pid, key, [ARun.new.g(gen)]} <> as, inbox, cfg, note}

def Hub.spawned.ok(started: U32 & Socket, h: Hub, +thread: String, +key: String, +gen: String) -> IO(Hub):
  (pid, s) = started
  do IO<Hub>:
    r : Socket & Result<&1, &1, U32 & String, Socket> <- H.Sock.dup(s)
    Hub.spawned.dup(r, pid, h, thread, key, gen)

def Hub.agent_send(m: Maybe<&2, Agent>, line: String) -> IO(Unit):
  match m:
    case Some{Agent{_, input, _, _, _}}:
      do IO<Unit>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, input, line)
        return Unit{}
    case None{}:
      IO.pure(Unit, Unit{})

# retiring an agent: its input closes and the process is signalled
def Hub.agent_stop(m: Maybe<&2, Agent>) -> IO(Unit):
  match m:
    case Some{Agent{_, input, pid, _, _}}:
      do IO<Unit>:
        Chan.close(String, input)
        Proc.kill(pid, 15)
    case None{}:
      IO.pure(Unit, Unit{})

def Hub.state(h: Hub) -> M.State & Hub:
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      (st, Hub{st, f, ls, cl, as, inbox, cfg, note})

# /hello's busy count follows every committed step, not the minute's tick
def Hub.busy.set(h: Hub) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        PS.Perf.set("hub.busy", U32.from_nat(M.Work.threads(st)))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# the busy count taken again only when the step's changes can move it
# (hub.bend's Busy.moved): a walk over every thread is no per-line cost
def Hub.busy.if(moved: Bool, h: Hub) -> IO(Hub):
  match moved:
    case True{}:
      Hub.busy.set(h)
    case False{}:
      IO.pure(Hub, h)

def Cfg.version(c: Cfg) -> String:
  match c:
    case Cfg{_, _, _, _, _, _, v, nt, +tcfg, +views}:
      v

# which log this is: the time of its first change (a client resumes only
# into the log it came from)
def Hub.origin(+ls: SO.Lg) -> String:
  SO.Lg.origin(ls)

# the log message with no items (the window in this process: it reads the
# state itself)
def Hub.log_msg.k(+ls: SO.Lg, +cfg: Cfg) -> String:
  "{\"t\":\"log\",\"since\":0,\"origin\":\"" ++ Hub.origin(ls) ++ "\",\"version\":" ++ J.Json.show(J.Str{Cfg.version(cfg)}) ++ ",\"info\":" ++ J.Json.show(Hub.info(cfg))
    ++ ",\"items\":[]}"

# the first frame of a join: the log message with the first chunk of what
# the client lacks; "more": the rest follows a chunk at a time
# (core/outbox.bend's Join.*), the last one with "boot": false
def Hub.log_msg.c(+origin: String, +cfg: Cfg, +since: Nat, xs: List<&2, String>, more: Bool) -> String:
  "{\"t\":\"log\",\"since\":" ++ Nat.show(since) ++ ",\"origin\":\"" ++ origin ++ "\",\"version\":" ++ J.Json.show(J.Str{Cfg.version(cfg)}) ++ ",\"info\":" ++ J.Json.show(Hub.info(cfg))
    ++ Bool.pick(String, more, ",\"more\":true", "") ++ ",\"items\":[" ++ String.join(xs, ",") ++ "]}"

# a later chunk of a join; the last says "boot": false
def Hub.boot_msg(xs: List<&2, String>, last: Bool) -> String:
  "{\"t\":\"changes\",\"boot\":" ++ Bool.pick(String, last, "false", "true") ++ ",\"items\":[" ++ String.join(xs, ",") ++ "]}"

# the same, put together by the writer
def Hub.boot_items(xs: List<&2, String>) -> Wire:
  WItems{"{\"t\":\"changes\",\"boot\":true,\"items\":[", "", "", xs, "]}", False{}}

# a join's first frame, put together by the writer
def Hub.log_items(+origin: String, +cfg: Cfg, +since: Nat, xs: List<&2, String>, +more: Bool) -> Wire:
  WItems{"{\"t\":\"log\",\"since\":" ++ Nat.show(since) ++ ",\"origin\":\"" ++ origin ++ "\",\"version\":" ++ J.Json.show(J.Str{Cfg.version(cfg)}) ++ ",\"info\":" ++ J.Json.show(Hub.info(cfg))
    , Bool.pick(String, more, ",\"more\":true,\"items\":[", ",\"items\":["), ",\"more\":true,\"items\":[", xs, "]}", Bool.not(more)}

# Desks
# -----
# which threads a desk watches (core/desk.bend): a client joining hears
# them, and every client hears them again whenever they change

def Desk.join(+text: String) -> String:
  Bool.pick(String, String.is_empty(text), "", DK.Desk.info(text))

def Desk.say.k(same: Bool, +cl: List<&2, Client>, +text: String) -> IO(List<&2, Client>):
  match same:
    case True{}:
      IO.pure(List<&2, Client>, cl)
    case False{}:
      Wire.all(cl, DK.Desk.info(text))

def Desk.say(+before: String, +cl: List<&2, Client>, +now: Nat) -> IO(List<&2, Client>):
  Desk.say.k(String.eq(before, DK.Desk.text(Clients.looks(cl), now)), cl, DK.Desk.text(Clients.looks(cl), now))

# a desk said what it shows
def Hub.desk(h: Hub, +client: U32, +thread: String, +focused: Bool) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        +cl2 : List<&2, Client> = Clients.look(cl, client, DK.DeskView{True{}, thread, focused, now})
        cl3 : List<&2, Client> <- Desk.say(DK.Desk.text(Clients.looks(cl), now), cl2, now)
        return Hub{st, f, ls, cl3, as, inbox, cfg, note}

def Hub.leave.close(m: Maybe<&2, Client>) -> IO(Unit):
  match m:
    case Some{Client{_, c, _, _, k, _, _}}:
      do IO<Unit>:
        Chan.close(Wire, c)
        H.Fd.close(k)
    case None{}:
      IO.pure(Unit, Unit{})

def Hub.push(h: Hub, +client: U32, +text: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.to(cl, client, text)
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

def Hub.push.bin(h: Hub, +client: U32, +bs: List<&2, U32>) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.to.raw(cl, client, WBin{bs}, List.length(&2, U32, bs))
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

# a ping answered through the client's credit
def Hub.pong(h: Hub, +client: U32, +p: List<&2, U32>) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.to.raw(cl, client, WPong{p}, List.length(&2, U32, p))
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

def Hub.note(h: Hub, +text: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, _}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.all(cl, text)
        return Hub{st, f, ls, cl2, as, inbox, cfg, text}

def Hub.close(h: Hub) -> IO(Unit):
  match h:
    case Hub{_, f, _, _, _, _, _, _}:
      File.close(f)

# the hub's inbox, handed to k beside the hub
def Hub.inbox(h: Hub, k: Chan(Msg) -> Hub -> IO(Unit)) -> IO(Unit):
  match h:
    case Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      k(inbox, Hub{st, f, ls, cl, as, inbox, cfg, note})

# Codex
# -----

def Codex.sandbox(r: M.RuntimeMode) -> List<&2, String>:
  match r:
    case M.FullAccess{}:
      ["--dangerously-bypass-approvals-and-sandbox"]
    case M.Supervised{}:
      ["-s", "read-only"]
    case _:
      ["-s", "workspace-write"]

def Codex.opt.k(empty: Bool, flag: String, v: String) -> List<&2, String>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      [flag, v]

def Codex.opt(flag: String, +v: String) -> List<&2, String>:
  Codex.opt.k(String.is_empty(v), flag, v)

def Codex.head.k(fresh: Bool, +session: String) -> List<&2, String>:
  match fresh:
    case True{}:
      ["exec", "--json", "--skip-git-repo-check"]
    case False{}:
      ["exec", "resume", session, "--json", "--skip-git-repo-check"]

def Cfg.prompt(c: Cfg) -> String:
  match c:
    case Cfg{_, _, _, _, pr, _, _, _, +tcfg, +views}:
      pr

def Codex.failed(e: U32 & String, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  (_, msg) = e
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, SpawnFailed{thread, gen, "{\"type\":\"turn.failed\",\"message\":" ++ J.Json.show(J.Str{"could not start codex: " ++ msg}) ++ "}"})
    return Unit{}

# whether a Codex event line ends the turn
def Codex.ends(+l: String) -> Bool:
  Bool.or(String.contains(l, "\"type\":\"turn.completed\""), String.contains(l, "\"type\":\"turn.failed\""))

def Codex.lines(ls: List<&2, String>, +ended: Bool, +thread: String, +inbox: Chan(Msg)) -> IO(Bool):
  match ls:
    case Nil{}:
      IO.pure(Bool, ended)
    case +l <> t:
      do IO<Bool>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AgentLine{thread, l})
        Codex.lines(t, Bool.or(ended, Codex.ends(l)), thread, inbox)

def Codex.fed(r: U.Lines & List<&2, String>, s: Socket, +ended: Bool, +thread: String, +inbox: Chan(Msg), k: Socket -> U.Lines -> Bool -> IO(Unit)) -> IO(Unit):
  (carry, ls) = r
  IO.bind(Bool, Unit, Codex.lines(ls, ended, thread, inbox), e2 => k(s, carry, e2))

# Folders for the project picker
# ------------------------------
# The path is always an argument ($1), never part of a script.

# $d from $1: "" and ~ are home, ~/x and relative paths are under home
def FsJob.path.sh() -> String:
  "d=\"$1\"; case \"$d\" in ''|'~') d=\"$HOME\";; '~/'*) d=\"$HOME/${d#??}\";; /*) ;; *) d=\"$HOME/$d\";; esac; "

# a folder that exists and can be opened, printed resolved (symlinks too)
def FsJob.dir.sh() -> String:
  "[ -e \"$d\" ] || { echo \"no such folder: $d\"; exit 2; }; [ -d \"$d\" ] || { echo \"not a folder: $d\"; exit 2; }; [ -r \"$d\" ] && [ -x \"$d\" ] && d=$(cd -P -- \"$d\" && pwd -P) || { echo \"cannot open: $d\"; exit 2; }; "

def FsJob.list.sh() -> String:
  "exec 2>&1; set -e; " ++ FsJob.path.sh() ++ FsJob.dir.sh() ++ "echo \"$d\"; find \"$d\" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sed 's|^.*/||' | LC_ALL=C sort | head -n 5000"

# a new folder (and any missing parents); an existing one is refused
def FsJob.mkdir.sh() -> String:
  "exec 2>&1; set -e; " ++ FsJob.path.sh() ++ "if [ -e \"$d\" ] || [ -L \"$d\" ]; then echo \"already exists: $d\"; exit 3; fi; mkdir -p -- \"$d\"; " ++ FsJob.dir.sh() ++ "echo \"$d\""

# the folder a project would have
def FsJob.resolve.sh() -> String:
  "exec 2>&1; set -e; " ++ FsJob.path.sh() ++ FsJob.dir.sh() ++ "echo \"$d\""

# folders under home holding .backplane or .backplane.json (Backplane
# projects), five levels down at most; other hidden folders, node_modules
# and other file systems are skipped, and it gives up after five seconds
def FsJob.find.sh() -> String:
  "exec 2>/dev/null; " ++ PT.Port.tmo() ++ "echo \"$HOME\"; tmo 5 find \"$HOME\" -xdev -mindepth 1 -maxdepth 5 \\( -type d \\( -name node_modules -o \\( -name '.*' ! -name .backplane \\) \\) -prune \\) -o \\( -type d -name .backplane -print -prune \\) -o \\( -name .backplane.json -print \\) | sed 's|/[^/]*$||' | grep -vxF \"$HOME\" | LC_ALL=C sort -u | head -n 500; true"

def FsJob.strs(ls: List<&2, String>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      J.Str{l} <> FsJob.strs(t)

def FsJob.listed.k(ls: List<&2, String>) -> J.Json:
  match ls:
    case d <> t:
      J.Json.obj([J.KV{"dir", J.Str{d}}, J.KV{"entries", J.Json.arr(FsJob.strs(t))}])
    case Nil{}:
      J.Json.obj([J.KV{"error", J.Str{"not a folder"}}])

def FsJob.listed(r: P.Ran) -> J.Json:
  match r:
    case P.Ran{0, out}:
      FsJob.listed.k(String.lines(String.trim(out)))
    case P.Ran{_, out}:
      J.Json.obj([J.KV{"error", J.Str{String.trim(out)}}])

def FsJob.found.k(ls: List<&2, String>) -> J.Json:
  match ls:
    case h <> t:
      J.Json.obj([J.KV{"home", J.Str{h}}, J.KV{"found", J.Json.arr(FsJob.strs(t))}])
    case Nil{}:
      J.Json.obj([J.KV{"found", J.Json.arr(Nil{})}])

def FsJob.found(r: P.Ran) -> J.Json:
  match r:
    case P.Ran{_, out}:
      FsJob.found.k(String.lines(String.trim(out)))

def FsJob.ok(+r: J.Json) -> Bool:
  String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(r, "error"), J.Null{})), "null")

# Git jobs
# --------
# The hub asks; git runs here off the hub's thread; results go back as a
# reply to the asking client, and as thread notes/events through the hub.

def GitJob.say(+inbox: Chan(Msg), +m: String, p: List<&2, J.KV>) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Rpc{0, J.Json.show(J.Json.obj([J.KV{"id", J.Num{"0"}}, J.KV{"m", J.Str{m}}, J.KV{"p", J.Json.obj(p)}]))})
    return Unit{}

def GitJob.note(+inbox: Chan(Msg), +thread: String, +tone: String, +summary: String) -> IO(Unit):
  GitJob.say(inbox, "thread.note", [J.KV{"thread", J.Str{thread}}, J.KV{"tone", J.Str{tone}}, J.KV{"kind", J.Str{"git"}}, J.KV{"summary", J.Str{summary}}])

def GitJob.reply(+inbox: Chan(Msg), +client: U32, +id: String, +ok: Bool, r: J.Json) -> IO(Unit):
  do IO<Unit>:
    sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Push{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{ok}}, J.KV{"result", r}]))})
    return Unit{}

def FsJob.answer(+inbox: Chan(Msg), +client: U32, +id: String, +r: J.Json) -> IO(Unit):
  GitJob.reply(inbox, client, id, FsJob.ok(r), r)

def FsJob.list(+inbox: Chan(Msg), +client: U32, +id: String, +path: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", FsJob.list.sh(), "sh", path], "/"), r => FsJob.answer(inbox, client, id, FsJob.listed(r)))

def FsJob.last(ls: List<&2, String>) -> String:
  Maybe.default(&2, String, List.last(&2, String, ls), "")

def FsJob.made(r: P.Ran, +inbox: Chan(Msg), +client: U32, +id: String) -> IO(Unit):
  match r:
    case P.Ran{0, out}:
      FsJob.list(inbox, client, id, FsJob.last(String.lines(String.trim(out))))
    case P.Ran{_, out}:
      FsJob.answer(inbox, client, id, J.Json.obj([J.KV{"error", J.Str{String.trim(out)}}]))

# a resolved folder becomes a project: the hub gets project.create as if
# the asking client sent it, so the answer (or "already a project") is its
def FsJob.create(r: P.Ran, +inbox: Chan(Msg), +client: U32, +id: String) -> IO(Unit):
  match r:
    case P.Ran{0, out}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Rpc{client, J.Json.show(J.Json.obj([J.KV{"id", J.Num{id}}, J.KV{"m", J.Str{"project.create"}},
          J.KV{"p", J.Json.obj([J.KV{"path", J.Str{FsJob.last(String.lines(String.trim(out)))}}])}]))})
        return Unit{}
    case P.Ran{_, out}:
      FsJob.answer(inbox, client, id, J.Json.obj([J.KV{"error", J.Str{String.trim(out)}}]))

def FsJob.sh(+sh: String, +path: String) -> IO(P.Ran):
  P.Proc.run("sh", ["-c", sh, "sh", path], "/")

# list, mkdir (then list it), add (resolve, then create), new (mkdir,
# then create), find
def FsJob.run.k(i: Nat, +inbox: Chan(Msg), +client: U32, +id: String, +path: String) -> IO(Unit):
  match i:
    case 1n:
      IO.bind(P.Ran, Unit, FsJob.sh(FsJob.mkdir.sh(), path), r => FsJob.made(r, inbox, client, id))
    case 2n:
      IO.bind(P.Ran, Unit, FsJob.sh(FsJob.resolve.sh(), path), r => FsJob.create(r, inbox, client, id))
    case 3n:
      IO.bind(P.Ran, Unit, FsJob.sh(FsJob.mkdir.sh(), path), r => FsJob.create(r, inbox, client, id))
    case 4n:
      IO.bind(P.Ran, Unit, FsJob.sh(FsJob.find.sh(), path), r => FsJob.answer(inbox, client, id, FsJob.found(r)))
    case _:
      FsJob.list(inbox, client, id, path)

def FsJob.run(+op: String, +inbox: Chan(Msg), +client: U32, +id: String, +path: String) -> IO(Unit):
  FsJob.run.k(C.Codec.pos(["list", "mkdir", "add", "new", "find"], op), inbox, client, id, path)

def GitJob.nonblank(ls: List<&2, String>, +n: Nat) -> Nat:
  match ls:
    case Nil{}:
      n
    case l <> t:
      GitJob.nonblank(t, Bool.pick(Nat, String.is_empty(String.trim(l)), n, 1n+n))

def GitJob.count.k(r: P.Ran) -> Nat:
  match r:
    case P.Ran{0, out}:
      GitJob.nonblank(String.lines(out), 0n)
    case _:
      0n

# how many checkpoints a thread has (its next turn number)
def GitJob.count(+cwd: String, +thread: String) -> IO(Nat):
  IO.bind(P.Ran, Nat, P.Proc.run("git", ["for-each-ref", "--format=%(refname)", "refs/backplane/checkpoints/" ++ GT.Git.b64url(thread) ++ "/turn/"], cwd),
    r => IO.pure(Nat, GitJob.count.k(r)))

def GitJob.cp0.k(n: Nat, +cwd: String, +thread: String) -> IO(Unit):
  match n:
    case 0n:
      IO.bind(String, Unit, GT.Git.checkpoint.capture(cwd, thread, 0n), r => IO.pure(Unit, Unit{}))
    case _:
      IO.pure(Unit, Unit{})

# A new thread's worktree (Git.thread_wt, in <repo>/.backplane/worktrees).
# When there is no repository to branch, or git fails, the thread works in
# the project folder (thread.local); only a worktree asked for by name
# ("worktree" rather than the default "auto") reports that as an error.

def WtJob.skip(+inbox: Chan(Msg), +thread: String, +tone: String, +why: String) -> IO(Unit):
  do IO<Unit>:
    GitJob.note(inbox, thread, tone, "working in the project folder: " ++ why)
    GitJob.say(inbox, "thread.local", [J.KV{"thread", J.Str{thread}}])

def WtJob.made(+inbox: Chan(Msg), +thread: String, +branch: String, +path: String, carried: Bool) -> IO(Unit):
  do IO<Unit>:
    GitJob.say(inbox, "thread.worktree", [J.KV{"thread", J.Str{thread}}, J.KV{"branch", J.Str{branch}}, J.KV{"path", J.Str{path}}])
    GitJob.note(inbox, thread, "info", "worktree " ++ path ++ Bool.pick(String, carried, " (with the folder's uncommitted work)", ""))

def WtJob.done(+r: String, +inbox: Chan(Msg), +thread: String, +mode: String, +branch: String, +carried: Bool) -> IO(Unit):
  Bool.pick(IO(Unit), GT.Git.skipped(r), WtJob.skip(inbox, thread, Bool.pick(String, String.eq(mode, "worktree"), "error", "info"), String.drop(r, 6n)),
    Bool.pick(IO(Unit), GT.Git.failed(r), WtJob.skip(inbox, thread, "error", String.trim(r)),
      WtJob.made(inbox, thread, branch, r, carried)))

def WtJob.go(+hw: Bool, +inbox: Chan(Msg), +thread: String, +root: String, +mode: String) -> IO(Unit):
  IO.bind(String, Unit, GT.Git.thread_wt(root, hw, GT.Git.branch.temp(B.Wt.seed(thread))),
    r => WtJob.done(r, inbox, thread, mode, GT.Git.branch.sanitize(GT.Git.branch.temp(B.Wt.seed(thread))), hw))

def WtJob.restore.k(n: Nat, +inbox: Chan(Msg), +thread: String, +path: String) -> IO(Unit):
  match n:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+last:
      IO.bind(String, Unit, GT.Git.checkpoint.revert(path, thread, last), r => IO.pure(Unit, Unit{}))

# a worktree made again gets back what it held when it was removed (the
# checkpoint taken then), before anything that waited for it runs
def WtJob.again.k(+r: String, +inbox: Chan(Msg), +thread: String, +branch: String) -> IO(Unit):
  Bool.pick(IO(Unit), Bool.or(GT.Git.failed(r), GT.Git.skipped(r)), WtJob.done(r, inbox, thread, "worktree", branch, False{}),
    IO.bind(Nat, Unit, GitJob.count(r, thread), n => IO.bind(Unit, Unit, WtJob.restore.k(n, inbox, thread, r), _ => WtJob.done(r, inbox, thread, "worktree", branch, False{}))))

# a fork's worktree: a new branch like any thread's, then the source
# thread's last checkpoint restored into it, so the fork starts from the
# files as the source left them
def WtJob.fork.k(+r: String, +inbox: Chan(Msg), +thread: String, +src: String, +branch: String) -> IO(Unit):
  Bool.pick(IO(Unit), Bool.or(GT.Git.failed(r), GT.Git.skipped(r)), WtJob.done(r, inbox, thread, "fork", branch, False{}),
    IO.bind(Nat, Unit, GitJob.count(r, src), n => IO.bind(Unit, Unit, WtJob.restore.k(n, inbox, src, r), _ => WtJob.done(r, inbox, thread, "fork", branch, False{}))))

def WtJob.fork(+inbox: Chan(Msg), +thread: String, +root: String, +src: String) -> IO(Unit):
  IO.bind(String, Unit, GT.Git.thread_wt(root, False{}, GT.Git.branch.temp(B.Wt.seed(thread))),
    r => WtJob.fork.k(r, inbox, thread, src, GT.Git.branch.sanitize(GT.Git.branch.temp(B.Wt.seed(thread)))))

def WtJob.run(+inbox: Chan(Msg), +thread: String, +root: String, +mode: String, +branch: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.eq(mode, "again"),
    IO.bind(String, Unit, GT.Git.thread_wt.again(root, branch), r => WtJob.again.k(r, inbox, thread, branch)),
    Bool.pick(IO(Unit), String.eq(mode, "fork"), WtJob.fork(inbox, thread, root, branch),
      IO.bind(List<&2, String>, Unit, K.Io.names(root), ns => WtJob.go(B.Wt.hardware(ns), inbox, thread, root, mode))))

# Removing a thread's worktree (M.Worktree.drop): its state is kept first as
# a checkpoint (a hidden ref, so nothing is lost; the branch stays too),
# then the worktree goes. Only a directory under .backplane/worktrees is
# ever removed. A worktree already gone is reported gone.

def RmJob.done(+r: String, +inbox: Chan(Msg), +thread: String, +branch: String) -> IO(Unit):
  Bool.pick(IO(Unit), GT.Git.failed(r), Log.info("worktree not removed: " ++ r),
    do IO<Unit>:
      GitJob.say(inbox, "thread.unrooted", [J.KV{"thread", J.Str{thread}}])
      GitJob.note(inbox, thread, "info", "worktree removed; branch " ++ branch ++ " and its last state are kept"))

def RmJob.top(+top: String, +inbox: Chan(Msg), +thread: String, +root: String, +branch: String) -> IO(Unit):
  Bool.pick(IO(Unit), Bool.not(M.Worktree.ours(top ++ "/")), Log.info("worktree not ours, kept: " ++ top),
    IO.bind(String, Unit, GT.Git.worktree.remove_force(root, top), r => RmJob.done(r, inbox, thread, branch)))

def RmJob.saved(+cp: String, +inbox: Chan(Msg), +thread: String, +top: String, +root: String, +branch: String) -> IO(Unit):
  Bool.pick(IO(Unit), GT.Git.failed(cp), Log.info("worktree kept, no checkpoint: " ++ cp),
    RmJob.top(top, inbox, thread, root, branch))

# whether the folder is a worktree of ours is asked before its last state is
# saved: a folder that is not (it is kept) took a new checkpoint ref each
# tick, for every such thread, forever
def RmJob.ours(+top: String, +inbox: Chan(Msg), +thread: String, +wt: String, +root: String, +branch: String) -> IO(Unit):
  Bool.pick(IO(Unit), Bool.not(M.Worktree.ours(top ++ "/")), PS.Log.debug("hub", "worktree not ours, kept: " ++ top),
    IO.bind(Nat, Unit, GitJob.count(wt, thread), n => IO.bind(String, Unit, GT.Git.checkpoint.capture(wt, thread, n), cp => RmJob.saved(cp, inbox, thread, top, root, branch))))

def RmJob.there(r: P.Ran, +inbox: Chan(Msg), +thread: String, +wt: String, +root: String, +branch: String) -> IO(Unit):
  match r:
    case P.Ran{0, _}:
      IO.bind(String, Unit, GT.Git.root(wt), top => RmJob.ours(top, inbox, thread, wt, root, branch))
    case _:
      GitJob.say(inbox, "thread.unrooted", [J.KV{"thread", J.Str{thread}}])

# a folder with no .git in it is no worktree (gone, or only an empty folder
# left behind inside another checkout): the thread is unrooted as when the
# folder is gone, so it is not asked about again each tick
def RmJob.run(+inbox: Chan(Msg), +thread: String, +wt: String, +root: String, +branch: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("test", ["-e", wt ++ "/.git"], "/"), r => RmJob.there(r, inbox, thread, wt, root, branch))

# Merges: which of a project's due branches gh reports merged (one call per
# project per tick; nothing happens without gh, auth or a GitHub remote)

def MergeJob.tell(hs: List<&2, J.KV>, +inbox: Chan(Msg)) -> IO(Unit):
  match hs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case J.KV{th, u} <> t:
      IO.bind(Unit, Unit, GitJob.say(inbox, "thread.merged", [J.KV{"thread", J.Str{th}}, J.KV{"url", u}]), _ => MergeJob.tell(t, inbox))

# the commits a due thread's worktree made itself (its HEAD reflog's commits,
# cherry-picks and rebased picks; never checkouts, so a branch it only
# stacked on or checked out to review is not its), so a pull request pushed
# from a branch of another name is still found
def MergeJob.reflog.k(none: Bool, +wt: String) -> IO(P.Ran):
  match none:
    case True{}:
      IO.pure(P.Ran, P.Ran{1, ""})
    case False{}:
      P.Proc.run("git", ["-C", wt, "log", "-g", "-n", "500", "-E", "--grep-reflog=^commit", "--grep-reflog=^cherry-pick", "--grep-reflog=^rebase \\((pick|finish)\\)", "--format=%H", "HEAD"], "/")

def MergeJob.reflog(parts: List<&2, String>) -> IO(P.Ran):
  match parts:
    case _ <> _ <> +wt <> Nil{}:
      MergeJob.reflog.k(String.is_empty(wt), wt)
    case _:
      IO.pure(P.Ran, P.Ran{1, ""})

def MergeJob.commits(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      String.join(String.lines(out), " ")
    case _:
      ""

def MergeJob.more(ls: List<&2, String>, +inbox: Chan(Msg), +out: String, +acc: String) -> IO(Unit):
  match ls:
    case Nil{}:
      MergeJob.tell(B.Merge.hits(out, acc), inbox)
    case +l <> t:
      IO.bind(P.Ran, Unit, MergeJob.reflog(String.split(l, '\t')), r => MergeJob.more(t, inbox, out, acc ++ l ++ "\t" ++ MergeJob.commits(r) ++ "\n"))

def MergeJob.got(r: P.Ran, +inbox: Chan(Msg), +lines: String) -> IO(Unit):
  match r:
    case P.Ran{0, out}:
      MergeJob.more(String.lines(lines), inbox, out, "")
    case _:
      IO.pure(Unit, Unit{})

def MergeJob.run(+inbox: Chan(Msg), +root: String, +lines: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, GH.GhRun.run(root, "sh", PT.Port.timeout(["30", "gh", "pr", "list", "--state", "merged", "--limit", "100", "--json", "headRefName,headRefOid,url"]), PT.Port.timeout(["30", "gh", "pr", "list", "--state", "merged", "--limit", "100", "--json", "headRefName,headRefOid,url"])),
    r => MergeJob.got(r, inbox, lines))

def GitJob.text.k(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +what: String, +r: String) -> IO(Unit):
  do IO<Unit>:
    GitJob.reply(inbox, client, id, Bool.not(GT.Git.failed(r)), J.Json.obj([J.KV{"text", J.Str{r}}, J.KV{"thread", J.Str{thread}}]))
    GitJob.note(inbox, thread, Bool.pick(String, GT.Git.failed(r), "error", "info"), what ++ Bool.pick(String, String.is_empty(r), "", ": " ++ String.trim(r)))

def GitJob.diff.k(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +r: String) -> IO(Unit):
  GitJob.reply(inbox, client, id, Bool.not(GT.Git.failed(r)), J.Json.obj([J.KV{"diff", J.Str{r}}, J.KV{"thread", J.Str{thread}}]))

def GitJob.diff(+cwd: String, +thread: String, +a: String, +b: String) -> IO(String):
  Bool.pick(IO(String), String.eq(b, "-1"), GT.Git.checkpoint.diff_worktree(cwd, thread, Maybe.default(&2, Nat, Nat.read(a), 0n)),
    GT.Git.checkpoint.diff(cwd, thread, Maybe.default(&2, Nat, Nat.read(a), 0n), Maybe.default(&2, Nat, Nat.read(b), 0n)))

# One-shot writing (thread titles, branch names, commit messages)
# ----------------------------------------------------------------
# The text generation model (AG.Text.of: text.provider, text.model) run by
# AG.OneShot.args under timeout(1); the text goes on stdin
# (AG.OneShot.input). "" when the CLI is missing, fails, or takes longer
# than a minute; callers fall back.

def OneShot.out(r: P.Ran, +provider: String) -> String:
  match r:
    case P.Ran{0, out}:
      AG.OneShot.answer(provider, out)
    case _:
      ""

def OneShot.done(r: Socket & Result<&1, &1, U32 & String, Unit>, +pid: U32, +provider: String) -> IO(String):
  (s, _) = r
  do IO<String>:
    s2 : Socket <- H.Sock.shutdown(s)
    d : Socket & List<&2, U32> <- P.Proc.drain(1000000n, s2, P.Chunks{Nil{}})
    ran : P.Ran <- P.Proc.finish(pid, d)
    return OneShot.out(ran, provider)

def OneShot.started(started: U32 & Socket, +input: String, +provider: String) -> IO(String):
  (+pid, s) = started
  IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, String, H.Sock.send_text(s, input), r => OneShot.done(r, pid, provider))

def OneShot.spawned(r: Result<&1, &1, U32 & String, U32 & Socket>, +input: String, +provider: String) -> IO(String):
  match r:
    case Done{started}:
      OneShot.started(started, input, provider)
    case Fail{_}:
      IO.pure(String, "")

# the writer's answer to one text under a system prompt, in /tmp
def OneShot.run(+tx: AG.Tx, +system: String, +input: String) -> IO(String):
  IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, String, H.Proc.spawn("sh", PT.Port.timeout(AG.OneShot.args(tx, system)), "/tmp", 0),
    r => OneShot.spawned(r, AG.OneShot.input(AG.Tx.provider(tx), system, input), AG.Tx.provider(tx)))

# a thread's first message: the writer titles it, and the hub keeps the
# title only while the thread still has the placeholder (thread.titled)
def TitleJob.run(+tx: AG.Tx, +inbox: Chan(Msg), +thread: String, +placeholder: String, +text: String) -> IO(Unit):
  IO.bind(String, Unit, OneShot.run(tx, AG.Title.system(), text), a =>
    GitJob.say(inbox, "thread.titled", [J.KV{"thread", J.Str{thread}}, J.KV{"placeholder", J.Str{placeholder}}, J.KV{"title", J.Str{a}}]))

# a side question (/btw): the writing model reads the thread and the
# question; the answer goes to the asking client only
def BtwJob.run(+tx: AG.Tx, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +q: String, +input: String) -> IO(Unit):
  IO.bind(String, Unit, OneShot.run(tx, AG.Btw.system(), input), a =>
    GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"thread", J.Str{thread}}, J.KV{"q", J.Str{q}}, J.KV{"btw", J.Str{a}}])))

# a summary for the composer's reference menu (refs.bend's Sum.*): of a
# thread (kind "thread") or a project ("project"); the hub keeps it
# (summary.set {id, kind, text})
def SumJob.run(+tx: AG.Tx, +inbox: Chan(Msg), +id: String, +kind: String, +input: String) -> IO(Unit):
  IO.bind(String, Unit, OneShot.run(tx, RF.Sum.system(), input), a =>
    GitJob.say(inbox, "summary.set", [J.KV{"id", J.Str{id}}, J.KV{"kind", J.Str{kind}}, J.KV{"text", J.Str{a}}]))

def AgentJob.named(+r: String, +inbox: Chan(Msg), +thread: String) -> IO(Unit):
  Bool.pick(IO(Unit), GT.Git.failed(r), GitJob.note(inbox, thread, "error", "branch name: " ++ r),
    GitJob.say(inbox, "thread.branch", [J.KV{"thread", J.Str{thread}}, J.KV{"branch", J.Str{r}}]))

# a name another branch has already gets the temporary one's hex
def AgentJob.renamed(+r: String, +cwd: String, +old: String, +frag: String, +inbox: Chan(Msg), +thread: String) -> IO(Unit):
  Bool.pick(IO(Unit), GT.Git.failed(r),
    IO.bind(String, Unit, GT.Git.branch.rename(cwd, old, AG.Name.branch(frag) ++ "-" ++ String.drop(old, 10n)), r2 => AgentJob.named(r2, inbox, thread)),
    AgentJob.named(r, inbox, thread))

def AgentJob.rename(+frag: String, +cwd: String, +old: String, +inbox: Chan(Msg), +thread: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(frag), IO.pure(Unit, Unit{}),
    IO.bind(String, Unit, GT.Git.branch.rename(cwd, old, AG.Name.branch(frag)), r => AgentJob.renamed(r, cwd, old, frag, inbox, thread)))

# a worktree thread's first message names its branch; without an answer
# the temporary name stays
def AgentJob.name(+tx: AG.Tx, +inbox: Chan(Msg), +thread: String, +cwd: String, +old: String, +text: String) -> IO(Unit):
  IO.bind(String, Unit, OneShot.run(tx, AG.Name.system(), String.take(text, 4000n)), a => AgentJob.rename(AG.Name.fragment(a), cwd, old, inbox, thread))

def AgentJob.write(+tx: AG.Tx, +d: String, +title: String) -> IO(String):
  Bool.pick(IO(String), Bool.or(String.is_empty(String.trim(d)), GT.Git.failed(d)), IO.pure(String, title),
    IO.bind(String, String, OneShot.run(tx, AG.Commit.system(), AG.Commit.input(title, d)), a => IO.pure(String, AG.Commit.message(a, title))))

# design review: a pull request just opened gets its parts change
# (server/review.bend's RvJob.pr) after the answer
def GitJob.pr.url(+r: String) -> String:
  String.trim(String.drop(GT.Git.last_line(r), 4n))

def GitJob.pr.note(+inbox: Chan(Msg), +thread: String, +what: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(what), IO.pure(Unit, Unit{}), GitJob.note(inbox, thread, "info", what))

def GitJob.text.pr(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +what: String, +r: String) -> IO(Unit):
  do IO<Unit>:
    GitJob.text.k(inbox, client, id, thread, what, r)
    Bool.pick(IO(Unit), Bool.and(Bool.not(GT.Git.failed(r)), String.starts_with(GT.Git.last_line(r), "pr: http")),
      IO.bind(String, Unit, RVS.RvJob.pr(cwd, GitJob.pr.url(r)), w => GitJob.pr.note(inbox, thread, w)), IO.pure(Unit, Unit{}))

def AgentJob.commit(+msg: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +action: String) -> IO(Unit):
  IO.bind(String, Unit, GT.Git.act(cwd, action, msg, "", ""), r => GitJob.text.pr(inbox, client, id, thread, cwd, action, r))

# Commit & push / Open PR with a message written from the thread's diff
# (checkpoint 0 against the work tree); the thread's title when there is
# no diff or no answer
def AgentJob.act(+tx: AG.Tx, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +action: String, +title: String) -> IO(Unit):
  do IO<Unit>:
    d : String <- GT.Git.checkpoint.diff_worktree(cwd, thread, 0n)
    msg : String <- AgentJob.write(tx, d, Bool.pick(String, String.is_empty(title), "Backplane: changes from this thread", title))
    AgentJob.commit(msg, inbox, client, id, thread, cwd, action)

def GitJob.go(i: Nat, +tx: AG.Tx, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String, +seed: Nat) -> IO(Unit):
  match i:
    case 0n:
      WtJob.run(inbox, thread, cwd, a, b)
    case 1n:
      IO.bind(Nat, Unit, GitJob.count(cwd, thread), n => GitJob.cp0.k(n, cwd, thread))
    case 2n:
      IO.bind(Nat, Unit, GitJob.count(cwd, thread), n => IO.bind(String, Unit, GT.Git.checkpoint.capture(cwd, thread, n), r => IO.pure(Unit, Unit{})))
    case 3n:
      IO.bind(String, Unit, GitJob.diff(cwd, thread, a, b), r => GitJob.diff.k(inbox, client, id, thread, r))
    case 4n:
      IO.bind(String, Unit, GT.Git.checkpoint.revert(cwd, thread, Maybe.default(&2, Nat, Nat.read(a), 0n)), r => GitJob.text.k(inbox, client, id, thread, "reverted to turn " ++ a, r))
    case 5n:
      IO.bind(String, Unit, GT.Git.act(cwd, a, Bool.pick(String, String.is_empty(b), "Backplane: changes from this thread", b), "", ""), r => GitJob.text.pr(inbox, client, id, thread, cwd, a, r))
    case 6n:
      IO.bind(String, Unit, GT.Git.status(cwd), r => GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"status", J.Str{r}}, J.KV{"thread", J.Str{thread}}])))
    case 7n:
      AgentJob.name(tx, inbox, thread, cwd, a, b)
    case 8n:
      AgentJob.act(tx, inbox, client, id, thread, cwd, a, b)
    case _:
      IO.pure(Unit, Unit{})

def GitJob.repo(ok: Bool, +tx: AG.Tx, +job: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String, +seed: Nat) -> IO(Unit):
  match ok:
    case True{}:
      GitJob.go(C.Codec.pos(["worktree", "checkpoint0", "checkpoint", "diff", "revert", "act", "status", "name", "act_gen"], job), tx, inbox, client, id, thread, cwd, a, b, home, seed)
    case False{}:
      Bool.pick(IO(Unit), String.is_empty(id), IO.pure(Unit, Unit{}), GitJob.reply(inbox, client, id, False{}, J.Json.obj([J.KV{"text", J.Str{"not a git repository: " ++ cwd}}])))

# one git job; nothing happens outside a git repository (checkpoints are
# skipped, requests answer why)
def GitJob.run.git(+tx: AG.Tx, +job: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String) -> IO(Unit):
  do IO<Unit>:
    ok : Bool <- GT.Git.is_repo(cwd)
    t : U32 <- H.Clock.epoch()
    GitJob.repo(ok, tx, job, inbox, client, id, thread, cwd, a, b, home, U32.to_nat(t))

# Sidebar jobs
# ------------
# A thread's files for the file picker (git's view of the tree in a
# repository, else a bounded walk; at most 3000), and 9:00 tomorrow in the
# machine's time zone for a snooze.

def Files.limit() -> String:
  "3000"

def Files.cmd(repo: Bool) -> String:
  match repo:
    case True{}:
      "git ls-files --cached --others --exclude-standard | head -n " ++ Files.limit()
    case False{}:
      "find . -maxdepth 8 -type f -not -path '*/.*' -not -path '*/node_modules/*' 2>/dev/null | head -n " ++ Files.limit() ++ " | sed 's|^\\./||'"

def Files.str(blank: Bool, l: String, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match blank:
    case True{}:
      rest
    case False{}:
      J.Str{l} <> rest

def Files.strs(ls: List<&2, String>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Files.str(String.is_empty(String.trim(l)), l, Files.strs(t))

def Files.sent(r: P.Ran, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String) -> IO(Unit):
  match r:
    case P.Ran{0, out}:
      GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"files", J.Json.arr(Files.strs(String.lines(out)))}, J.KV{"thread", J.Str{thread}}]))
    case P.Ran{_, out}:
      GitJob.reply(inbox, client, id, False{}, J.Json.obj([J.KV{"text", J.Str{out}}]))

def Files.list(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String) -> IO(Unit):
  do IO<Unit>:
    repo : Bool <- GT.Git.is_repo(cwd)
    r : P.Ran <- P.Proc.run("sh", ["-c", Files.cmd(repo)], cwd)
    Files.sent(r, inbox, client, id, thread)

# a thread's mechanical parts for the Mechanical page: every model and
# render under the project's mech/ folder, sorted, one cksum line each
# (core/mech.bend reads them); none when there is no such folder
def Mech.cmd() -> String:
  "[ -d mech ] || exit 0; find mech -maxdepth 4 -type f \\( -iname '*.step' -o -iname '*.stp' -o -iname '*.glb' -o -iname '*.png' \\) -not -path '*/.*' 2>/dev/null | LC_ALL=C sort | head -n 400 | while IFS= read -r f; do cksum \"$f\"; done"

def Mech.sent(r: P.Ran, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String) -> IO(Unit):
  match r:
    case P.Ran{0, out}:
      GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"mech", J.Str{out}}, J.KV{"root", J.Str{cwd}}, J.KV{"thread", J.Str{thread}}]))
    case P.Ran{_, out}:
      GitJob.reply(inbox, client, id, False{}, J.Json.obj([J.KV{"text", J.Str{out}}]))

def Mech.list(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", Mech.cmd()], cwd), r => Mech.sent(r, inbox, client, id, thread, cwd))

# a part's renders on request (mech.render: the Mech page's Create
# renders). This build has no renderer (no native binary ships), so the
# answer is the parts listed again with the reason, never a silent failure
def Mech.unavailable() -> String:
  "Part renders are not available in this build. The 3D view still shows the part."

def Mech.rendered(+err: String, l: P.Ran, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +path: String) -> IO(Unit):
  match l:
    case P.Ran{_, out}:
      GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"mech", J.Str{out}}, J.KV{"root", J.Str{cwd}}, J.KV{"thread", J.Str{thread}},
        J.KV{"rendered", J.Str{path}}, J.KV{"error", J.Str{err}}]))

def Mech.render(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +path: String) -> IO(Unit):
  do IO<Unit>:
    l : P.Ran <- P.Proc.run("sh", ["-c", Mech.cmd()], cwd)
    Mech.rendered(Mech.unavailable(), l, inbox, client, id, thread, cwd, path)

def Snooze.tz(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      String.trim(out)
    case _:
      "+0000"

# a heartbeat every Alive.every() ms: the clients that hear nothing for
# longer than Alive.limit() take the link for dead and reconnect
def Heart.loop(fuel: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        IO.sleep(U32.from_nat(DK.Alive.every()))
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Heart{})
        Heart.loop(f, inbox)

# every five seconds the hub beats: held progress lines land (Hub.beat)
def Beat.loop(fuel: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        IO.sleep(5000)
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Beat{})
        Beat.loop(f, inbox)

# snooze until 9:00 tomorrow here: the hub hears the time as thread.snooze
def Snooze.tomorrow(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +now: String) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("date", ["+%z"], "/")
    GitJob.say(inbox, "thread.snooze", [J.KV{"thread", J.Str{thread}}, J.KV{"until", J.Num{Nat.show(M.Snooze.tomorrow(Maybe.default(&2, Nat, Nat.read(now), 0n), Snooze.tz(r)))}}])
    GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"thread", J.Str{thread}}]))

def Env.or(r: Result<&1, &1, U32 & String, String>, d: String) -> String:
  match r:
    case Done{+v}:
      Bool.pick(String, String.is_empty(v), d, v)
    case Fail{_}:
      d

def Env.get(name: String, d: String) -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env(name)
    return Env.or(r, d)

# a tick at start (threads idle past the window settle at once), then
# every minute
def Ticker.loop(fuel: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Tick{})
        tz : P.Ran <- P.Proc.run("date", ["+%z"], "/")
        ok2 : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, BotTick{Snooze.tz(tz)})
        +delay : String <- Env.get("BACKPLANE_WAKE_TICK_MS", "60000")
        IO.sleep(WK.Wake.tick.ms(delay))
        Ticker.loop(f, inbox)

def KiCadJob.selected(ok: Bool, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +rt: K.Rt) -> IO(Unit):
  match ok:
    case False{}:
      GitJob.reply(inbox, client, id, False{}, J.Json.obj([J.KV{"kicad", J.Str{K.Rt.status(rt)}}, J.KV{"thread", J.Str{thread}}]))
    case True{}:
      do IO<Unit>:
        fs : K.Files <- K.Files.read(cwd)
        +err : String <- K.KiCad.open(rt, fs)
        GitJob.reply(inbox, client, id, String.is_empty(err), J.Json.obj([J.KV{"text", J.Str{err}}, J.KV{"thread", J.Str{thread}}]))

# Compatibility for non-hub jobs; normal Open uses the hub's cached Rt.
def KiCadJob.run(+inbox: Chan(Msg), +client: U32, +id: String, +cwd: String, +home: String) -> IO(Unit):
  do IO<Unit>:
    text : String <- K.Io.text(home ++ "/kicad-health.json")
    +rt : K.Rt = K.Rt.cached(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))
    KiCadJob.selected(String.eq(K.Rt.status(rt), "ok"), inbox, client, id, "", cwd, rt)

def KiCadJob.route(kicad: Bool, +inbox: Chan(Msg), client: U32, id: String, thread: String, cwd: String, +rt: K.Rt, other: IO(Unit)) -> IO(Unit):
  match kicad:
    case True{}: IO.spawn(Unit, KiCadJob.selected(String.eq(K.Rt.status(rt), "ok"), inbox, client, id, thread, cwd, rt))
    case False{}: other

# Export jobs use the hub's cached health and run outside the event loop.
def KExportJob.activity(done: Bool, +inbox: Chan(Msg), +thread: String, request: String, +result: J.Json) -> IO(Unit):
  match done:
    case False{}: IO.pure(Unit, Unit{})
    case True{}:
      do IO<Unit>:
        +now : Nat <- Now.nat()
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{[M.ActivityLogged{thread, "export10:" ++ request ++ ":" ++ Nat.show(now), "info", "kicad.export10", J.Json.get_str(result, "text", ""), now}]})
        return Unit{}

def KExportJob.pro(fs: K.Files) -> String:
  match fs:
    case K.Files{pro, _, _, _}: pro

def KExportJob.run(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, cwd: String, +home: String, rt: K.Rt, +a: String) -> IO(Unit):
  do IO<Unit>:
    fs : K.Files <- K.Files.read(cwd)
    +pro : String = KExportJob.pro(fs)
    +args : J.Json = Maybe.default(&2, J.Json, J.Json.parse(a), J.Null{})
    +r : J.Json <- KEX.KExport.export(rt, home, pro, J.Json.get_bool(args, "anyway", False{}), J.Json.get_str(args, "report", ""), False{})
    +opened : J.Json <- KEX.KExport.open(String.eq(J.Json.get_str(r, "export10", ""), "done"), r, pro)
    KExportJob.activity(String.eq(J.Json.get_str(r, "export10", ""), "done"), inbox, thread, U32.show(client) ++ ":" ++ id, r)
    GitJob.reply(inbox, client, id, Bool.not(String.eq(J.Json.get_str(r, "export10", ""), "error")), KH.KH.put.obj(opened, "thread", J.Str{thread}))

def KExportJob.route(exporting: Bool, +inbox: Chan(Msg), client: U32, id: String, thread: String, cwd: String, home: String, rt: K.Rt, a: String, other: IO(Unit)) -> IO(Unit):
  match exporting:
    case True{}: IO.spawn(Unit, KExportJob.run(inbox, client, id, thread, cwd, home, rt, a))
    case False{}: other

# Design steps (core/hist.bend): after a tool call, the thread's KiCad
# files snapshot (Git.step.capture); a change is logged in the thread as
# an Act of tone "step" whose summary is its record. `entry` is the tool
# call or message it follows ("" at a turn's end).

def StepJob.log(+inbox: Chan(Msg), +thread: String, +id: String, +rec: String) -> IO(Unit):
  do IO<Unit>:
    now : Nat <- Now.nat()
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{[M.ActivityLogged{thread, id, HI.Step.tone(), "kicad", rec, now}]})
    return Unit{}

def StepJob.done(+inbox: Chan(Msg), +thread: String, +entry: String, +out: String) -> IO(Unit):
  +rec = HI.Hist.record(out, entry)
  Bool.pick(IO(Unit), String.is_empty(rec), IO.pure(Unit, Unit{}),
    StepJob.log(inbox, thread, "step:" ++ String.trim(HI.Hist.at(String.split(out, '\n'), 0n)), rec))

def StepJob.run(+inbox: Chan(Msg), +thread: String, +cwd: String, +entry: String, +home: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(cwd), IO.pure(Unit, Unit{}),
    IO.bind(String, Unit, GT.Git.step.capture(cwd, thread, GT.Git.step.reg(home)), out => StepJob.done(inbox, thread, entry, out)))

def RefsJob.files(f: K.Files) -> List<&2, J.KV>:
  match f:
    case K.Files{_, pcb, sch, _}:
      [J.KV{"pcb", J.Str{pcb}}, J.KV{"sch", J.Str{sch}}]

# what the thread's files may be compared with (Git.refs), and its
# canonical board and schematic
def RefsJob.reply(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +ls: List<&2, String>, f: K.Files) -> IO(Unit):
  GitJob.reply(inbox, client, id, True{}, J.Json.obj(List.append(&2, J.KV, [J.KV{"thread", J.Str{thread}}, J.KV{"git", J.Str{HI.Hist.at(ls, 0n)}},
    J.KV{"top", J.Str{HI.Hist.at(ls, 1n)}}, J.KV{"refs", J.Str{String.join(List.drop(&2, String, ls, 2n), "\n")}}], RefsJob.files(f))))

def RefsJob.run(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String) -> IO(Unit):
  do IO<Unit>:
    out : String <- GT.Git.refs(cwd, thread)
    f : K.Files <- K.Files.read(cwd)
    RefsJob.reply(inbox, client, id, thread, String.split(out, '\n'), f)

def FileJob.done(r: P.Ran, +inbox: Chan(Msg), +client: U32, +id: String) -> IO(Unit):
  match r:
    case P.Ran{0, _}:
      GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"text", J.Str{""}}]))
    case P.Ran{_, out}:
      GitJob.reply(inbox, client, id, False{}, J.Json.obj([J.KV{"text", J.Str{String.trim(out)}}]))

# a file link from a thread, opened here or shown in its folder (core's
# Link.file.cmd: the path goes as an argument, never as shell text)
def FileJob.run(+inbox: Chan(Msg), +client: U32, +id: String, +path: String, +how: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", LK.Link.file.cmd(how), "sh", path], "/"), r => FileJob.done(r, inbox, client, id))

# The viewer's Files tab (core/fm.bend): one folder of a thread's project,
# its folders then its files ("d|f <tab> size <tab> name"), hidden ones and
# symlinked folders left out, 2000 of each at most
def DirJob.sh() -> String:
  "cd \"$1\" 2>/dev/null || { echo 'no such folder'; exit 1; }; rr=$(pwd -P); [ -z \"$2\" ] || cd \"./$2\" 2>/dev/null || { echo 'no such folder'; exit 1; }; case \"$(pwd -P)/\" in \"$rr\"/*) ;; *) echo 'no such folder'; exit 1;; esac; for f in *; do [ -d \"$f\" ] && [ ! -L \"$f\" ] && printf 'd\\t0\\t%s\\n' \"$f\"; done | head -n 2000; for f in *; do [ -f \"$f\" ] && printf 'f\\t%s\\t%s\\n' \"$(wc -c < \"$f\" | tr -d ' ')\" \"$f\"; done | head -n 2000; true"

def DirJob.sent(r: P.Ran, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +rel: String) -> IO(Unit):
  match r:
    case P.Ran{0, out}:
      GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"fmEntries", J.Str{out}}, J.KV{"fmDir", J.Str{rel}}, J.KV{"fmRoot", J.Str{cwd}}, J.KV{"thread", J.Str{thread}}]))
    case P.Ran{_, out}:
      GitJob.reply(inbox, client, id, False{}, J.Json.obj([J.KV{"text", J.Str{String.trim(out)}}]))

def DirJob.run(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +rel: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", DirJob.sh(), "sh", cwd, rel], "/"), r => DirJob.sent(r, inbox, client, id, thread, cwd, rel))

# a PDF's page drawn to a PNG for the window (which cannot draw a PDF), by
# poppler's pdftoppm when this machine has it, kept in <home>/pdf per file
# version and page: "<png> <tab> <pages>"
def PdfJob.sh() -> String:
  "command -v pdftoppm >/dev/null 2>&1 || { echo 'nopoppler'; exit 3; }; [ -f \"$1\" ] || { echo 'no such file'; exit 1; }; m=$(stat -c %Y \"$1\" 2>/dev/null || stat -f %m \"$1\"); k=$(printf '%s %s' \"$1\" \"$m\" | cksum | cut -d' ' -f1); mkdir -p \"$3\"; o=\"$3/$k-$2\"; [ -f \"$o.png\" ] || pdftoppm -png -r 110 -f \"$2\" -l \"$2\" -singlefile \"$1\" \"$o\" >/dev/null 2>&1 || { echo \"could not draw page $2\"; exit 1; }; n=$(pdfinfo \"$1\" 2>/dev/null | awk '/^Pages:/{print $2}'); printf '%s\\t%s' \"$o.png\" \"${n:-0}\""

def PdfJob.why(+out: String) -> String:
  Bool.pick(String, String.contains(out, "nopoppler"), "PDFs show here with poppler's pdftoppm (the poppler or poppler-utils package)", String.trim(out))

def PdfJob.sent(r: P.Ran, +inbox: Chan(Msg), +client: U32, +id: String, +path: String, +page: String) -> IO(Unit):
  match r:
    case P.Ran{0, +out}:
      GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"pdfPath", J.Str{path}}, J.KV{"pdfPage", J.Str{page}},
        J.KV{"pdfPng", J.Str{Maybe.default(&2, String, List.head(&2, String, String.split(out, '\t')), "")}},
        J.KV{"pdfPages", J.Str{String.trim(Maybe.default(&2, String, List.get(&2, String, String.split(out, '\t'), 1n), "0"))}}]))
    case P.Ran{_, +out}:
      GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"pdfPath", J.Str{path}}, J.KV{"pdfPage", J.Str{page}}, J.KV{"pdfErr", J.Str{PdfJob.why(out)}}]))

def PdfJob.run(+inbox: Chan(Msg), +client: U32, +id: String, +home: String, +path: String, +page: String) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", PdfJob.sh(), "sh", path, page, home ++ "/pdf"], "/"), r => PdfJob.sent(r, inbox, client, id, path, page))

def GitJob.run.k(i: Nat, +tx: AG.Tx, +job: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String) -> IO(Unit):
  match i:
    case 0n:
      Files.list(inbox, client, id, thread, cwd)
    case 1n:
      Snooze.tomorrow(inbox, client, id, thread, a)
    case 2n:
      WtJob.run(inbox, thread, cwd, a, b)
    case 3n:
      MergeJob.run(inbox, cwd, a)
    case 4n:
      RmJob.run(inbox, thread, cwd, a, b)
    case 5n:
      KiCadJob.run(inbox, client, id, cwd, home)
    case 6n:
      TitleJob.run(tx, inbox, thread, a, b)
    case 7n:
      BtwJob.run(tx, inbox, client, id, thread, a, b)
    case 8n:
      SumJob.run(tx, inbox, thread, a, b)
    case 9n:
      Mech.list(inbox, client, id, thread, cwd)
    case 10n:
      Mech.render(inbox, client, id, thread, cwd, a)
    case 11n:
      StepJob.run(inbox, thread, cwd, a, home)
    case 12n:
      RefsJob.run(inbox, client, id, thread, cwd)
    case 13n:
      FileJob.run(inbox, client, id, a, b)
    case 14n:
      DirJob.run(inbox, client, id, thread, cwd, a)
    case 15n:
      PdfJob.run(inbox, client, id, home, a, b)
    # the project card (core/order.bend): a = "on|off\n<server>", b = the intent the person wrote
    case 16n:
      IO.bind(J.Json, Unit, ODI.Od.look(home, cwd, thread, a), r => GitJob.reply(inbox, client, id, True{}, r))
    case 17n:
      IO.bind(J.Json, Unit, ODI.Od.intent(home, cwd, thread, a, b), r => GitJob.reply(inbox, client, id, True{}, r))
    case _:
      GitJob.run.git(tx, job, inbox, client, id, thread, cwd, a, b, home)

# design review's jobs (server/review.bend), answered as they finish
# (a report whose project's own checks wait for the person's OK has the
# hub ask, once: review.bend's Rvc.checks.out "pending")
def RvJob.ask(+inbox: Chan(Msg), +rep: J.Json) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(J.Json.get_str(rep, "pending", "")), IO.pure(Unit, Unit{}),
    IO.bind(Result<&1, &1, Msg, Unit>, Unit, Chan.send(Msg, inbox, ChecksAsk{J.Json.get_str(rep, "thread", ""), J.Json.get_str(rep, "pending", ""), J.Json.get_str(rep, "pendrun", "")}), r => IO.pure(Unit, Unit{})))

def RvJob.reply(r: RVS.Res, +inbox: Chan(Msg), +client: U32, +id: String) -> IO(Unit):
  match r:
    case RVS.Res{ok, +o}:
      do IO<Unit>:
        RvJob.ask(inbox, J.Json.get_obj(o, "report"))
        GitJob.reply(inbox, client, id, ok, o)

def GitJob.run.r(rv: Bool, +tx: AG.Tx, +job: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String) -> IO(Unit):
  match rv:
    case True{}:
      IO.bind(RVS.Res, Unit, RVS.RvJob.run(job, thread, cwd, a, b, home), r => RvJob.reply(r, inbox, client, id))
    case False{}:
      GitJob.run.k(C.Codec.pos(["files", "snooze", "worktree", "merged", "wt_remove", "kicad", "title", "btw", "summary", "mech", "render", "step", "refs", "open", "dir", "pdf", "fab", "intent"], job), tx, job, inbox, client, id, thread, cwd, a, b, home)

# a job the hub asked for: a git job, or one of the sidebar's
def GitJob.run(+tx: AG.Tx, +job: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String) -> IO(Unit):
  GitJob.run.r(String.starts_with(job, "rv."), tx, job, inbox, client, id, thread, cwd, a, b, home)

# Grok (grok agent stdio, ACP) and Muse (muse serve, MSP)
# -------------------------------------------------------
# One process per turn. The turn's protocol (MS.Proto: AC.Acp.line or
# MS.Msp.line) decides what to write back and what to report; the reports
# are Codex-shaped lines for the hub.

def Grok.perm(r: M.RuntimeMode) -> List<&2, String>:
  match r:
    case M.Supervised{}:
      ["--permission-mode", "default", "agent"]
    case M.AcceptEdits{}:
      ["--permission-mode", "acceptEdits", "agent"]
    case M.AutoMode{}:
      ["--permission-mode", "auto", "agent"]
    case M.FullAccess{}:
      ["agent", "--always-approve"]

def Grok.args(r: M.RuntimeMode, +model: String, +effort: String) -> List<&2, String>:
  List.concat(&2, String, [Grok.perm(r), Codex.opt("-m", model), AG.Effort.args("grok", effort), ["stdio"]])

# whether Grok's tool requests go to the user (all modes but full access)
def Grok.asks(r: M.RuntimeMode) -> Bool:
  match r:
    case M.FullAccess{}:
      False{}
    case _:
      True{}

# Muse: `muse serve` with the thread's model; the workspace's own rules and
# skills load (as Codex and Grok read AGENTS.md); full access also drops
# Muse's shell sandbox. The approval mode goes over the wire (Muse.mode).
def Muse.free(r: M.RuntimeMode) -> List<&2, String>:
  match r:
    case M.FullAccess{}:
      ["--disable-sandbox"]
    case _:
      Nil{}

def Muse.args(r: M.RuntimeMode, +model: String) -> List<&2, String>:
  List.concat(&2, String, [["serve"], Codex.opt("--model", model), ["--trust-workspace"], Muse.free(r)])

def Muse.mode(r: M.RuntimeMode) -> String:
  match r:
    case M.Supervised{}:
      MS.Msp.mode.k(0n)
    case M.AcceptEdits{}:
      MS.Msp.mode.k(1n)
    case M.AutoMode{}:
      MS.Msp.mode.k(2n)
    case M.FullAccess{}:
      MS.Msp.mode.k(3n)

def Grok.report(ls: List<&2, String>, +thread: String, +inbox: Chan(Msg)) -> IO(Unit):
  match ls:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case l <> t:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AgentLine{thread, l})
        Grok.report(t, thread, inbox)

def Grok.write(ls: List<&2, String>, +input: Chan(String)) -> IO(Unit):
  match ls:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case l <> t:
      do IO<Unit>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, input, l)
        Grok.write(t, input)

def Grok.acp(o: AC.AcpOut) -> AC.Acp:
  match o:
    case AC.AcpOut{a, _, _, _}:
      a

def Grok.done(a: AC.Acp) -> Bool:
  match a:
    case AC.Acp{_, _, d}:
      d

def Grok.answered(m: Maybe<&1, String>, +input: Chan(String)) -> IO(Unit):
  match m:
    case Some{+line}:
      Bool.pick(IO(Unit), String.is_empty(line), IO.pure(Unit, Unit{}), Grok.write([line], input))
    case None{}:
      IO.pure(Unit, Unit{})

# a permission request waits off the reader: the hub allows a tool already
# allowed for the session, or opens an approval and parks the request;
# the ACP answer then goes to grok
def Grok.ask.one(+line: String, +thread: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +reply : Chan(String) <- Chan.new(String, 1)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Mcp{thread, line, reply})
    m : Maybe<&1, String> <- Chan.recv(String, reply)
    Grok.answered(m, input)

def Grok.ask(ls: List<&2, String>, +thread: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  match ls:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case l <> t:
      do IO<Unit>:
        IO.spawn(Unit, Grok.ask.one(l, thread, input, inbox))
        Grok.ask(t, thread, input, inbox)

def Grok.out(o: AC.AcpOut, +thread: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(AC.Acp):
  match o:
    case AC.AcpOut{a, send, emit, asks}:
      do IO<AC.Acp>:
        Grok.write(send, input)
        Grok.report(emit, thread, inbox)
        Grok.ask(asks, thread, input, inbox)
        return a

# every line of a chunk, in order, through the ACP state
def Grok.lines(ls: List<&2, String>, a: AC.Acp, +t: MS.Proto, +thread: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(AC.Acp):
  match ls:
    case Nil{}:
      IO.pure(AC.Acp, a)
    case l <> rest:
      IO.bind(AC.Acp, AC.Acp, Grok.out(MS.Proto.line(a, t, l), thread, input, inbox), a2 => Grok.lines(rest, a2, t, thread, input, inbox))

# a per-turn process is done: its turn already ended through its own
# events, so its exit tells the hub nothing (a queued turn may be running)
def Turn.end(s: Socket, +pid: U32, +tag: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Socket.close(s)
    code : U32 <- H.Proc.wait(pid)
    AIO.Adopt.drop(B.Tag.thread(tag), pid)
    tail : String <- H.Proc.errtail(pid)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, TurnDone{tag})
    return Unit{}

# a per-turn process that ended before its turn did: reaped, and a signal
# asks its scope why (an out-of-memory kill says so, with the limit; the
# scope is reset), here in the reader and never on the hub. The words for
# the failed turn: the scope's, else dflt, with the exit code or signal and
# the end of what it wrote to stderr, secrets dropped (core/errtail.bend)
def Turn.close(s: Socket, +pid: U32, +tag: String, +gen: String, +dflt: String) -> IO(String):
  do IO<String>:
    Socket.close(s)
    +code : U32 <- H.Proc.wait(pid)
    AIO.Adopt.drop(B.Tag.thread(tag), pid)
    +tail : String <- H.Proc.errtail(pid)
    +k : Nat <- Keyring.probe()
    return KR.Keyring.say(ER.Exit.say(dflt, B.Exit.why(code), code, tail), k)

def Turn.failed(s: Socket, +pid: U32, +tag: String, +gen: String, +dflt: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +msg : String <- Turn.close(s, pid, tag, gen, dflt)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AgentLine{tag, AC.Acp.failed(msg)})
    ok2 : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, TurnDone{tag})
    return Unit{}

# output closed: a turn that never ended fails; the exit itself is not news
# (but an out-of-memory end is told)
def Codex.end(s: Socket, +pid: U32, ended: Bool, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  match ended:
    case True{}:
      Turn.end(s, pid, thread, gen, inbox)
    case False{}:
      Turn.failed(s, pid, thread, gen, "codex exited before the turn ended", inbox)

# the stream ended: a last line with no newline still counts (a turn's
# final event written just before the process died), then the usual end
def Codex.last(carry: U.Lines, s: Socket, +ended: Bool, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +rest : String = U.Lines.rest(carry)
    e2 : Bool <- Codex.lines(Bool.pick(List<&2, String>, String.is_empty(String.trim(rest)), Nil{}, [rest]), ended, thread, inbox)
    Codex.end(s, pid, e2, thread, gen, inbox)

def Codex.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, carry: U.Lines, +ended: Bool, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg), k: Socket -> U.Lines -> Bool -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Codex.last(carry, s, ended, pid, thread, gen, inbox)
    case Done{bs}:
      Codex.fed(U.Lines.feed(carry, bs), s, ended, thread, inbox, k)
    case Fail{_}:
      Codex.last(carry, s, ended, pid, thread, gen, inbox)

def Codex.reader(fuel: Nat, s: Socket, carry: U.Lines, +ended: Bool, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Codex.end(s, pid, ended, thread, gen, inbox)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => Codex.got(r, carry, ended, pid, thread, gen, inbox, s2 => c2 => e2 => Codex.reader(f, s2, c2, e2, pid, thread, gen, inbox)))

def Codex.read(started: U32 & Socket, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  (+pid, s) = started
  do IO<Unit>:
    s2 : Socket <- H.Sock.shutdown(s)
    +input : Chan(String) <- Chan.new(String, 1)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, TurnPid{thread, pid, input, gen})
    AIO.Adopt.note(thread, pid)
    s3 : Socket <- IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Socket, H.Sock.dup(s2), d => Agent.watch.start(d, pid, thread, gen, inbox))
    Codex.reader(Fuel.forever(), s3, U.Lines.new(), False{}, pid, B.Tag.of(thread, pid), gen, inbox)

def Codex.started(r: Result<&1, &1, U32 & String, U32 & Socket>, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case Done{started}:
      Codex.read(started, thread, gen, inbox)
    case Fail{e}:
      Codex.failed(e, thread, gen, inbox)

def Codex.refuse(+thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, SpawnFailed{thread, gen, "{\"type\":\"turn.failed\",\"not_started\":true,\"message\":" ++ J.Json.show(J.Str{KR.Keyring.refusal("codex")}) ++ "}"})
    return Unit{}

def Codex.run(+args: List<&2, String>, +cwd: String, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  Keyring.gate("codex", IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, Unit, H.Proc.spawn("codex", args, cwd, 3), r => Codex.started(r, thread, gen, inbox)),
    Codex.refuse(thread, gen, inbox))

def Grok.stop(s: Socket, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Proc.kill(pid, 15)
    Turn.end(s, pid, thread, gen, inbox)

def Grok.next(done: Bool, s: Socket, carry: U.Lines, a: AC.Acp, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg), k: Socket -> U.Lines -> AC.Acp -> IO(Unit)) -> IO(Unit):
  match done:
    case True{}:
      Grok.stop(s, pid, thread, gen, inbox)
    case False{}:
      k(s, carry, a)

def Grok.after(+a: AC.Acp, s: Socket, carry: U.Lines, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg), k: Socket -> U.Lines -> AC.Acp -> IO(Unit)) -> IO(Unit):
  Grok.next(Grok.done(a), s, carry, a, pid, thread, gen, inbox, k)

def Grok.fed(r: U.Lines & List<&2, String>, s: Socket, a: AC.Acp, +t: MS.Proto, +pid: U32, +thread: String, +gen: String, +input: Chan(String), +inbox: Chan(Msg),
    k: Socket -> U.Lines -> AC.Acp -> IO(Unit)) -> IO(Unit):
  (carry, ls) = r
  IO.bind(AC.Acp, Unit, Grok.lines(ls, a, t, thread, input, inbox), a2 => Grok.after(a2, s, carry, pid, thread, gen, inbox, k))

# the agent closed its output before the turn ended
def Grok.gone(s: Socket, +why: String, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  Turn.failed(s, pid, thread, gen, why, inbox)

def Grok.last.k(done: Bool, s: Socket, +cmd: String, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  match done:
    case True{}:
      Turn.end(s, pid, thread, gen, inbox)
    case False{}:
      Grok.gone(s, cmd, pid, thread, gen, inbox)

def Grok.last.a(+a: AC.Acp, s: Socket, +cmd: String, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  Grok.last.k(Grok.done(a), s, cmd, pid, thread, gen, inbox)

# the stream ended: a last line with no newline still counts (the turn's
# end written just before the process died); only a turn it does not end
# fails as one that exited early
def Grok.last(carry: U.Lines, s: Socket, a: AC.Acp, +t: MS.Proto, +pid: U32, +thread: String, +gen: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +rest : String = U.Lines.rest(carry)
    a2 : AC.Acp <- Grok.lines(Bool.pick(List<&2, String>, String.is_empty(String.trim(rest)), Nil{}, [rest]), a, t, thread, input, inbox)
    Grok.last.a(a2, s, MS.Proto.gone(t), pid, thread, gen, inbox)

def Grok.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, carry: U.Lines, a: AC.Acp, +t: MS.Proto, +pid: U32, +thread: String, +gen: String, +input: Chan(String), +inbox: Chan(Msg),
    k: Socket -> U.Lines -> AC.Acp -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Grok.last(carry, s, a, t, pid, thread, gen, input, inbox)
    case Done{bs}:
      Grok.fed(U.Lines.feed(carry, bs), s, a, t, pid, thread, gen, input, inbox, k)
    case Fail{_}:
      Grok.last(carry, s, a, t, pid, thread, gen, input, inbox)

def Grok.reader(fuel: Nat, s: Socket, carry: U.Lines, a: AC.Acp, +t: MS.Proto, +pid: U32, +thread: String, +gen: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Agent.end(s, pid, thread, gen, inbox)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => Grok.got(r, carry, a, t, pid, thread, gen, input, inbox, s2 => c2 => a2 => Grok.reader(f, s2, c2, a2, t, pid, thread, gen, input, inbox)))

# a Grok process whose socket could not be shared is stopped and its turn
# fails as one that could not start
def Grok.dup.failed(e: U32 & String, rd: Socket, +cmd: String, +pid: U32, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  (_, msg) = e
  do IO<Unit>:
    Socket.close(rd)
    Proc.kill(pid, 15)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, SpawnFailed{thread, gen, AC.Acp.failed("could not start " ++ cmd ++ ": " ++ msg)})
    return Unit{}

def Grok.duped(r: Socket & Result<&1, &1, U32 & String, Socket>, +pid: U32, +t: MS.Proto, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  (rd, res) = r
  match res:
    case Done{wr}:
      do IO<Unit>:
        +input : Chan(String) <- Chan.new(String, 64)
        IO.spawn(Unit, Agent.writer(Fuel.forever(), wr, input))
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, TurnPid{thread, pid, input, gen})
        AIO.Adopt.note(thread, pid)
        ok2 : Result<&1, &1, String, Unit> <- Chan.send(String, input, MS.Proto.init(t))
        rd2 : Socket <- IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Socket, H.Sock.dup(rd), d => Agent.watch.start(d, pid, thread, gen, inbox))
        Grok.reader(Fuel.forever(), rd2, U.Lines.new(), AC.Acp.new(), t, pid, B.Tag.of(thread, pid), gen, input, inbox)
    case Fail{e}:
      Grok.dup.failed(e, rd, MS.Proto.name(t), pid, thread, gen, inbox)

def Grok.started.ok(started: U32 & Socket, +t: MS.Proto, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  (+pid, s) = started
  IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Unit, H.Sock.dup(s), r => Grok.duped(r, pid, t, thread, gen, inbox))

def Grok.failed(e: U32 & String, +p: MS.Proto, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  (_, msg) = e
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, SpawnFailed{thread, gen, AC.Acp.failed("could not start " ++ MS.Proto.name(p) ++ ": " ++ msg ++ ". " ++ MS.Proto.install(p))})
    return Unit{}

def Grok.started(r: Result<&1, &1, U32 & String, U32 & Socket>, +t: MS.Proto, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case Done{started}:
      Grok.started.ok(started, t, thread, gen, inbox)
    case Fail{e}:
      Grok.failed(e, t, thread, gen, inbox)

def Grok.refuse(+t: MS.Proto, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, SpawnFailed{thread, gen, AC.Acp.failed(KR.Keyring.refusal(MS.Proto.name(t)))})
    return Unit{}

def Grok.run(+args: List<&2, String>, +t: MS.Proto, +cwd: String, +thread: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  Keyring.gate(MS.Proto.name(t), IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, Unit, H.Proc.spawn(MS.Proto.cmd(t), args, cwd, 3), r => Grok.started(r, t, thread, gen, inbox)),
    Grok.refuse(t, thread, gen, inbox))

# Updates
# -------

def Cfg.home(c: Cfg) -> String:
  match c:
    case Cfg{h, _, _, _, _, _, _, _, +tcfg, +views}:
      h

def Upgrade.note(kvs: List<&2, J.KV>) -> String:
  J.Json.show(J.Json.obj(J.KV{"t", J.Str{"update"}} <> kvs))

def Upgrade.say(+inbox: Chan(Msg), text: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Announce{text})
    return Unit{}

def Upgrade.exec.fail(e: U32 & String, +inbox: Chan(Msg)) -> IO(Unit):
  (_, m) = e
  Upgrade.say(inbox, Upgrade.note([J.KV{"error", J.Str{"restart failed: " ++ m}}]))

def Upgrade.exec(r: Result<&1, &1, U32 & String, Unit>, +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case Done{_}:
      IO.pure(Unit, Unit{})
    case Fail{e}:
      Upgrade.exec.fail(e, inbox)

def Upgrade.found.k(none: Bool, +tag: String, +inbox: Chan(Msg)) -> IO(Unit):
  match none:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      Upgrade.say(inbox, Upgrade.note([J.KV{"version", J.Str{tag}}]))

def Upgrade.found(+tag: String, +inbox: Chan(Msg)) -> IO(Unit):
  Upgrade.found.k(String.is_empty(tag), tag, inbox)

# 15 s after start, then hourly: the hub looks for a newer release on the
# update channel (it holds the setting)
def Upgrade.check(fuel: Nat, +wait: U32, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        IO.sleep(wait)
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, UpdateCheck{})
        Upgrade.check(f, 3600000, inbox)

def Upgrade.start.go(on: Bool, +inbox: Chan(Msg)) -> IO(Unit):
  match on:
    case True{}:
      IO.spawn(Unit, Upgrade.check(Fuel.forever(), 15000, inbox))
    case False{}:
      IO.pure(Unit, Unit{})

# Remote access (Tailscale)
# ------------------------

def Cfg.net(c: Cfg) -> Net:
  match c:
    case Cfg{_, _, _, _, _, _, _, n, +tcfg, +views}:
      n

def Cfg.port(c: Cfg) -> U32:
  match c:
    case Cfg{_, _, _, p, _, _, _, _, +tcfg, +views}:
      p

def Net.token(n: Net) -> String:
  match n:
    case Net{_, _, t, _, _, _, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}:
      t

# updates come from a package (SV.Chan.managed): the updater stays off
def Net.managed(n: Net) -> Bool:
  match n:
    case Net{_, _, _, _, _, _, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}:
      mgd

# Effects
# -------

# The loopback MCP endpoint of one thread's native agent. Shared pairing
# credentials stay out of provider arguments and generated configuration:
# the URL carries the segment's own key (core/auth.bend's Auth.mcp.key).
# (gen: the process's generation, B.Gen.mcp: a call from a process that is
# no longer the thread's may only read)
def Hub.mcp_url.seg(+cfg: Cfg, +seg: String) -> String:
  "http://127.0.0.1:" ++ U32.show(Cfg.port(cfg)) ++ "/mcp/" ++ seg ++ "?k=" ++ AUTH.Auth.mcp.key(Net.token(Cfg.net(cfg)), seg)

def Hub.mcp_url.g(+cfg: Cfg, +thread: String, +gen: String) -> String:
  Hub.mcp_url.seg(cfg, B.Gen.mcp(thread, gen))

def Hub.mcp_url(+cfg: Cfg, +thread: String) -> String:
  Hub.mcp_url.seg(cfg, thread)

def Cfg.mtext(c: Cfg) -> String:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, mt, _, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      mt

def Cfg.rt(c: Cfg) -> K.Rt:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      rt

def Hub.prompt.kicad(rt: K.Rt, ipc: Bool) -> String:
  match rt:
    case K.Rt{+cli, _, +v, +c, +h}:
      Q.Prompt.kicad.health(J.Json.get_str(h, "status", "checking"), cli, v, c, J.Json.get_str(h, "why", ""), ipc)

def Hub.prompt.files(+cwd: String, fs: K.Files) -> String:
  match fs:
    case K.Files{+pro, +pcb, +sch, +all}:
      Q.Prompt.project(cwd, pro, pcb, sch, all)

# one thread's instructions: the shared runtime and skills, then this
# machine's KiCad, the project's canonical files, and what it may delegate
# (keys: the API keys' block, server/apikeys.bend's Ak.prompt, "" for none)
def Cfg.bn(c: Cfg) -> BN.Bn:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, _, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      bnx

def Hub.prompt.machines.k(+m: String) -> String:
  Bool.pick(String, String.is_empty(m), "", "\n\n" ++ m)

def Hub.prompt.machines(+cfg: Cfg) -> String:
  Hub.prompt.machines.k(FM.Fm.prompt(FR.Fh.ins(BN.Bn.far(Cfg.bn(cfg)))))

# a buy thread's own instructions (Buy it made it: setting buy.<thread>)
def Hub.prompt.buy(+st: M.State, +thread: String) -> String:
  Bool.pick(String, BH.Bots.buy.is(st, thread), "\n\n" ++ OR.Order.prompt.buy(), "")

# everything but the language line (agent.lang)
def Hub.prompt.base(+cfg: Cfg, +st: M.State, +thread: String, +cwd: String, fs: K.Files, +keys: String) -> String:
  String.join([Cfg.prompt(cfg),
    Hub.prompt.kicad(Cfg.rt(cfg), Bool.not(String.eq(M.Setting.get(st, "kicad.ipc", "on"), "off"))),
    Hub.prompt.files(cwd, fs),
    Q.Prompt.orchestration(M.Task.may_delegate(B.Task.depth(st, thread)), Cfg.mtext(cfg)), Q.Prompt.background(), OR.Order.prompt.project.k(OR.Order.enabled(M.Setting.get(st, "order.enabled", "off")))], "\n\n") ++ Bool.pick(String, String.is_empty(keys), "", "\n\n" ++ keys) ++ Hub.prompt.machines(cfg) ++ BH.Bots.prompt.k(RUN.Run.bot(st, thread), st, Cfg.home(cfg)) ++ Hub.prompt.buy(st, thread)

def Hub.prompt(+cfg: Cfg, +st: M.State, +thread: String, +cwd: String, fs: K.Files, +keys: String) -> String:
  Q.Prompt.lang.add(Hub.prompt.base(cfg, st, thread, cwd, fs, keys), M.Setting.get(st, "agent.lang", "off"))

# pcbparts (a free parts-search MCP over the fab's and distributors' stock,
# by manufacturer part number), given to every agent beside Backplane's own
def Parts.mcp.url() -> String:
  "https://pcbparts.dev/mcp"

def Parts.mcp.claude() -> String:
  "\"pcbparts\":{\"type\":\"http\",\"url\":\"" ++ Parts.mcp.url() ++ "\"}"

def Hub.agent_args(+cfg: Cfg, +thread: String, +model: String, +session: String, +r: M.RuntimeMode, +i: M.InteractionMode, prompt: String, +effort: String, +gen: String) -> List<&2, String>:
  match cfg:
    case Cfg{+home, _, _, _, _, +dirs, _, _, +tcfg, +views}:
      List.append(&2, String, Q.Claude.args(model, session, r, i, prompt, List.append(&2, String, dirs, [home ++ "/attachments"])), List.append(&2, String, AG.Effort.args("claude", effort), List.append(&2, String, Q.Claude.plugins(dirs),
        ["--mcp-config", "{\"mcpServers\":{\"backplane\":{\"type\":\"http\",\"url\":\"" ++ Hub.mcp_url.g(cfg, thread, gen) ++ "\"}," ++ Parts.mcp.claude() ++ "}}"])))

# what a Claude process was started with; a turn asking for something else
# (another model, mode or effort) restarts it
def Agent.key(+model: String, r: M.RuntimeMode, i: M.InteractionMode, +effort: String) -> String:
  model ++ "|" ++ C.Runtime.name(r) ++ "|" ++ C.Interaction.name(i) ++ "|" ++ AG.Effort.of("claude", effort)

def Agent.same(m: Maybe<&2, Agent>, +key: String) -> Bool:
  match m:
    case Some{Agent{_, _, _, k, _}}:
      String.eq(k, key)
    case None{}:
      False{}

# an agent starting in a KiCad project opens its thread's panel on the
# board, unless the person has opened or shut it by hand (core's Auto.view,
# client.bend's Viewer.auto)
def Hub.auto.k(empty: Bool, +cl: List<&2, Client>, +text: String) -> IO(List<&2, Client>):
  match empty:
    case True{}:
      IO.pure(List<&2, Client>, cl)
    case False{}:
      Wire.all(cl, text)

def Hub.auto(+cl: List<&2, Client>, +thread: String, fs: K.Files) -> IO(List<&2, Client>):
  match fs:
    case K.Files{_, +pcb, +sch, _}:
      Hub.auto.k(String.is_empty(B.Auto.view(thread, pcb, sch)), cl, B.Auto.view(thread, pcb, sch))

# the process a thread's agent replaces: its background work is told lost
# first, synchronously, so the new process's rows are never touched (law
# replaced_keeps_new_rows); a note to a running turn waits behind it
def Hub.replaced(has: Bool, +st: M.State, +now: Nat, +thread: String) -> List<&2, M.Change>:
  match has:
    case True{}:
      B.Queue.changes(B.Bg.lost(st, now, thread, "replaced"))
    case False{}:
      Nil{}

def Agent.is(m: Maybe<&2, Agent>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

# `codex exec` for one turn, with this thread's MCP endpoint and the
# Backplane instructions (the same runtime block Claude gets)
def Codex.args(+cfg: Cfg, +thread: String, +model: String, +session: String, r: M.RuntimeMode, +text: String, prompt: String, +effort: String, +gen: String, mailrun: Bool) -> List<&2, String>:
  List.concat(&2, String, [Codex.head.k(String.is_empty(session), session), Codex.sandbox(r), Codex.opt("-m", model), AG.Effort.args("codex", effort), AT.Att.codex(text), MD.Disposition.codex(mailrun),
    ["-c", "mcp_servers.backplane.url=\"" ++ Hub.mcp_url.g(cfg, thread, gen) ++ "\"", "-c", "mcp_servers.pcbparts.url=\"" ++ Parts.mcp.url() ++ "\"",
     "-c", "developer_instructions=" ++ J.Json.show(J.Str{prompt}), text]])

# what a tailscale command printed ("" when it failed)
def Tailnet.out(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      out
    case _:
      ""

def Tailnet.first(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(out)), ""))
    case _:
      ""

def Tailnet.dns.k(+name: String) -> String:
  Bool.pick(String, String.ends_with(name, "."), String.take(name, Nat.sub(String.length(name), 1n)), name)

def Tailnet.dns(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      Tailnet.dns.k(J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(out), J.Null{}), "Self"), "DNSName", ""))
    case _:
      ""

def Tailnet.https.k(none: Bool, +name: String, +token: String, +inbox: Chan(Msg)) -> IO(Unit):
  match none:
    case True{}:
      Log.info("tailscale serve did not report a DNS name")
    case False{}:
      do IO<Unit>:
        IO.print("Backplane over HTTPS on your tailnet: https://" ++ name ++ "/ (pair in Settings)")
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, LinkFound{TN.Tailnet.report("reachable", TN.Tailnet.link("https://" ++ name, token))})
        return Unit{}

def Tailnet.https(on: Bool, +port: U32, +token: String, +inbox: Chan(Msg)) -> IO(Unit):
  match on:
    case True{}:
      do IO<Unit>:
        s : P.Ran <- P.Proc.run("tailscale", ["serve", "--bg", "--https=443", "http://127.0.0.1:" ++ U32.show(port)], "")
        st : P.Ran <- P.Proc.run("tailscale", ["status", "--json"], "")
        +name : String = Tailnet.dns(st)
        Tailnet.https.k(String.is_empty(name), name, token, inbox)
    case False{}:
      IO.pure(Unit, Unit{})

def Token.digit(+d: U32) -> Char:
  Chr{U32.add(d, Bool.pick(U32, U32.is_lt(d, 10), 48, 87))}

def Token.hex.go(n: Nat, +x: U32, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+m:
      Token.hex.go(m, U32.shrn(x, 4n), SCon{Token.digit(U32.and(x, 15)), acc})

# the pairing token: made once (128 random bits, hex), kept in the home
def Token.hex(+x: U32) -> String:
  Token.hex.go(8n, x, "")

def Token.fresh() -> IO(String):
  do IO<String>:
    a : U32 <- IO.try(U32, IO.random_u32())
    b : U32 <- IO.try(U32, IO.random_u32())
    c : U32 <- IO.try(U32, IO.random_u32())
    d : U32 <- IO.try(U32, IO.random_u32())
    return Token.hex(a) ++ Token.hex(b) ++ Token.hex(c) ++ Token.hex(d)

def Token.closed(w: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = w
  File.close(f)

def Token.write(r: Result<&1, &1, U32 & String, File>, +t: String) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write(f, t), w => Token.closed(w))
    case Fail{_}:
      IO.pure(Unit, Unit{})

def Token.save(+path: String, +t: String) -> IO(String):
  do IO<String>:
    f : Result<&1, &1, U32 & String, File> <- File.open(path, "w")
    Token.write(f, t)
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(path, 384)
    return t

def Token.load.k(none: Bool, +path: String, +have: String) -> IO(String):
  match none:
    case True{}:
      IO.bind(String, String, Token.fresh(), t => Token.save(path, t))
    case False{}:
      IO.pure(String, have)

def Token.load(+home: String) -> IO(String):
  do IO<String>:
    +have : String <- Files.text(home ++ "/token")
    Token.load.k(String.is_empty(String.trim(have)), home ++ "/token", String.trim(have))

def Cfg.with_token(c: Cfg, +t: String) -> Cfg:
  match c:
    case Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, _, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, tcfg, views}

def Cfg.with_managed(c: Cfg, m: Bool) -> Cfg:
  match c:
    case Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, _, lk, fc, ksx}, +tcfg, +views}:
      Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, m, lk, fc, ksx}, tcfg, views}

# Startup
# -------

def Upgrade.start(+cfg: Cfg, +inbox: Chan(Msg)) -> IO(Unit):
  Upgrade.start.go(Bool.not(Net.managed(Cfg.net(cfg))), inbox)

def Names.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Names.copy(t)

# the install is done: the hub is asked, and restarts only if nothing has
# started work since the update began (Hub.update_ready, Restart{}); with
# its own arguments, not its name (Bend 2.0.36's IO.args() starts with it)
def Upgrade.restart.at(+path: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Upgrade.say(inbox, Upgrade.note([J.KV{"state", J.Str{"restarting"}}]))
    args : List<String> <- IO.args()
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Reexec{EX.Exe.path(path), Names.copy(RVS.Rvc.args.drop(args))})
    return Unit{}

def Upgrade.restart(+inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(String, Unit, H.Sys.exe_path(), path => Upgrade.restart.at(path, inbox))

# Safe deploy (hub.bend's Deploy.*, docs/deploy.md)
# -------------------------------------------------
# A waiting deploy asks, each tick that no tool call runs, whether the
# build locks are free (BACKPLANE_DEPLOY_LOCKS, colon-separated, for tests;
# default the machine-wide build locks: flock -n on each; where flock(1)
# is missing they count as free). When it goes, the staged package is
# moved into the running install's folder the way scripts/dev.sh's
# install_into does (renames; web.old kept), then the hub restarts in place.

def Deploy.locks.sh() -> String:
  "command -v flock >/dev/null 2>&1 || exit 0; for l in $(echo \"$1\" | tr ':' ' '); do [ -e \"$l\" ] || continue; flock -n \"$l\" true || exit 1; done; exit 0"

def Deploy.probe.k(r: P.Ran, +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case P.Ran{+code, _}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, DeployLocks{U32.is_eq(code, 0)})
        return Unit{}

def Deploy.probe.l(+locks: String, +inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", Deploy.locks.sh(), "sh", locks], "/"), r => Deploy.probe.k(r, inbox))

def Deploy.probe(+inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(String, Unit, Env.get("BACKPLANE_DEPLOY_LOCKS", "/tmp/bp-bend.lock:/tmp/bp-wt-build.lock"), l => Deploy.probe.l(l, inbox))

def Deploy.swap.sh() -> String:
  "set -eu; src=$1; prefix=$2; [ -d \"$src\" ] || { echo \"no staged package at $src\"; exit 1; }; mkdir -p \"$prefix\"; if [ -f \"$src/cef-runtime.py\" ]; then python3 \"$src/cef-runtime.py\" install \"$src\" \"$prefix\"; fi; if [ -d \"$src/web\" ]; then rm -rf \"$prefix/web.old\"; [ -d \"$prefix/web\" ] && mv \"$prefix/web\" \"$prefix/web.old\"; mv \"$src/web\" \"$prefix/web\"; fi; if [ -d \"$src/kistack\" ]; then rm -rf \"$prefix/kistack.old\"; [ -d \"$prefix/kistack\" ] && mv \"$prefix/kistack\" \"$prefix/kistack.old\"; mv \"$src/kistack\" \"$prefix/kistack\"; fi; for f in backplane backplane-serve backplane-browser backplane-browser.stamp backplane-step2glb backplane-voice backplane-kipc LICENSE; do if [ -e \"$src/$f\" ]; then mv -f \"$src/$f\" \"$prefix/$f\"; fi; done; rmdir \"$src\" 2>/dev/null || true; exit 0"

def Deploy.swapped(ok: Bool, +out: String, +path: String, +inbox: Chan(Msg)) -> IO(Unit):
  match ok:
    case True{}:
      Upgrade.restart.at(path, inbox)
    case False{}:
      do IO<Unit>:
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, DeployFailed{String.take(String.trim(out), 300n)})
        return Unit{}

def Deploy.swap.k(r: P.Ran, +path: String, +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case P.Ran{+code, +out}:
      Deploy.swapped(U32.is_eq(code, 0), out, path, inbox)

def Deploy.swap(+stage: String, +exe: String, +path: String, +inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", Deploy.swap.sh(), "sh", stage, exe], "/"), r => Deploy.swap.k(r, path, inbox))

def Deploy.go.k(none: Bool, +stage: String, +exe: String, +path: String, +inbox: Chan(Msg)) -> IO(Unit):
  match none:
    case True{}:
      Upgrade.restart.at(path, inbox)
    case False{}:
      Deploy.swap(stage, exe, path, inbox)

# (the running binary's path is read before the swap: once its file is
# replaced the kernel reads it back with " (deleted)" after it)
def Deploy.go.p(+stage: String, +exe: String, +inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(String, Unit, H.Sys.exe_path(), path => Deploy.go.k(String.is_empty(stage), stage, exe, path, inbox))

def Deploy.go(+stage: String, +inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(String, Unit, H.Sys.exe_dir(), exe => Deploy.go.p(stage, exe, inbox))

# Deploying origin/main (hub.bend's Ship.*, core/ship.bend, docs/deploy.md)
# -------------------------------------------------------------------------
# Off the hub: git reads origin/main (Ship.look.sh: its commit, and which
# candidate commits contain it); a build runs as a job of its own under
# <home>/deploy/<commit>/ (run.sh, its log out, pkg the staged package, code
# its exit status), started with setsid inside deploy.wrap (a command
# prefix, default empty: then `systemd-run --user --scope` when the user's
# systemd answers), so neither a hub OOM nor the restart it asks for cuts
# it. The hub only watches it (Ship.check.sh every 5 s), also after its own
# restart; a job is lost only when it is gone without an exit status.

def Ship.look.sh() -> String:
  "export GIT_TERMINAL_PROMPT=0\ncd \"$1\" 2>/dev/null || { echo \"ERR no checkout at $1\"; exit 0; }\ne=$(git fetch -q origin main 2>&1) || { echo \"ERR git fetch origin main: $(printf '%s' \"$e\" | tail -n 1)\"; exit 0; }\nt=$(git rev-parse --verify -q origin/main) || { echo \"ERR no origin/main in $1\"; exit 0; }\necho \"HEAD $t\"\nfor c in $(printf '%s' \"$2\" | tr ',' ' '); do git merge-base --is-ancestor \"$t\" \"$c\" 2>/dev/null && echo \"IN $c\"; done\nexit 0\n"

def Ship.run.sh() -> String:
  "pid_id() { q=$1; if [ -r \"/proc/$q/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$q/stat\" | cut -d' ' -f20); echo \"$b:$st\"; elif ps -p \"$q\" >/dev/null 2>&1; then ps -o lstart= -p \"$q\" | tr -s ' ' '_'; fi; }\nalive() { p=$(cat \"$1/$2\" 2>/dev/null); [ -n \"$p\" ] && [ \"$p\" -gt 1 ] 2>/dev/null && [ -n \"$(pid_id \"$p\")\" ] && [ \"$(pid_id \"$p\")\" = \"$(cat \"$1/$2id\" 2>/dev/null)\" ]; }\nd=$1; repo=$2; c=$3\necho $$ > \"$d/pid\"; pid_id $$ > \"$d/pidid\"\nlog=$d/out; : > \"$log\"\nrm -rf \"$d/wt\" \"$d/pkg\"\nrc=0\ngit -C \"$repo\" worktree add -q --detach \"$d/wt\" \"$c\" >>\"$log\" 2>&1 || rc=$?\nif [ \"$rc\" = 0 ]; then (cd \"$d/wt\" && BP_BUILD_LOCKED=1 exec scripts/dev.sh stage \"$d/pkg\") >>\"$log\" 2>&1 </dev/null || rc=$?; fi\ngit -C \"$repo\" worktree remove --force \"$d/wt\" >>\"$log\" 2>&1; rm -rf \"$d/wt\"; git -C \"$repo\" worktree prune >/dev/null 2>&1\nif [ \"$rc\" = 0 ] && [ ! -d \"$d/pkg\" ]; then echo \"no package was staged in $d/pkg\" >>\"$log\"; rc=1; fi\nif [ \"$rc\" = 0 ]; then for x in \"$d\"/../*/pkg; do [ -d \"$x\" ] && [ ! \"$x\" -ef \"$d/pkg\" ] && rm -rf \"$x\"; done; fi\necho \"$rc\" > \"$d/code.tmp\" && mv \"$d/code.tmp\" \"$d/code\"\n"

def Ship.launch.sh() -> String:
  "pid_id() { q=$1; if [ -r \"/proc/$q/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$q/stat\" | cut -d' ' -f20); echo \"$b:$st\"; elif ps -p \"$q\" >/dev/null 2>&1; then ps -o lstart= -p \"$q\" | tr -s ' ' '_'; fi; }\nalive() { p=$(cat \"$1/$2\" 2>/dev/null); [ -n \"$p\" ] && [ \"$p\" -gt 1 ] 2>/dev/null && [ -n \"$(pid_id \"$p\")\" ] && [ \"$(pid_id \"$p\")\" = \"$(cat \"$1/$2id\" 2>/dev/null)\" ]; }\nb=$1; repo=$2; c=$3; wrap=$4; run=$5\nd=$b/$(printf '%s' \"$c\" | cut -c1-12)\nmkdir -p \"$d\" || { echo \"ERR cannot make $d\"; exit 0; }\nif [ -e \"$d/started\" ]; then echo \"RUNNING\"; exit 0; fi\nfor x in \"$b\"/*; do [ \"$x\" = \"$d\" ] && continue; [ -d \"$x/wt\" ] || continue; if [ -e \"$x/code\" ] || { ! alive \"$x\" pid && ! alive \"$x\" gpid; }; then git -C \"$repo\" worktree remove --force \"$x/wt\" >/dev/null 2>&1; rm -rf \"$x/wt\"; fi; done\ngit -C \"$repo\" worktree prune >/dev/null 2>&1\nif [ -z \"$wrap\" ] && command -v systemd-run >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then wrap=\"systemd-run --user --scope -q --\"; fi\nprintf '%s\\n' \"$run\" > \"$d/run.sh\"\nprintf '%s sh \"%s/run.sh\" \"%s\" \"%s\" \"%s\"\\n' \"$wrap\" \"$d\" \"$d\" \"$repo\" \"$c\" > \"$d/go.sh\"\n: > \"$d/started\"\nif command -v setsid >/dev/null 2>&1; then setsid nohup sh \"$d/go.sh\" > \"$d/launch.out\" 2>&1 < /dev/null &\nelse nohup sh \"$d/go.sh\" > \"$d/launch.out\" 2>&1 < /dev/null &\nfi\ng=$!\necho \"$g\" > \"$d/gpid\"; pid_id \"$g\" > \"$d/gpidid\"\necho \"PID $g $(cat \"$d/gpidid\")\"\n"

def Ship.check.sh() -> String:
  "pid_id() { q=$1; if [ -r \"/proc/$q/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$q/stat\" | cut -d' ' -f20); echo \"$b:$st\"; elif ps -p \"$q\" >/dev/null 2>&1; then ps -o lstart= -p \"$q\" | tr -s ' ' '_'; fi; }\nalive() { p=$(cat \"$1/$2\" 2>/dev/null); [ -n \"$p\" ] && [ \"$p\" -gt 1 ] 2>/dev/null && [ -n \"$(pid_id \"$p\")\" ] && [ \"$(pid_id \"$p\")\" = \"$(cat \"$1/$2id\" 2>/dev/null)\" ]; }\nd=$1\nst() { echo \"VERSION $(sed -n 's/.*built \\([^ ]*\\).*/\\1/p' \"$d/out\" 2>/dev/null | tail -n 1)\"; echo \"STAGE $d/pkg\"; tail -n 15 \"$d/out\" 2>/dev/null | sed 's/\\x1b\\[[0-9;]*m//g'; }\n[ -e \"$d/code\" ] && { echo \"STATUS done $(cat \"$d/code\")\"; st; exit 0; }\nif alive \"$d\" pid || alive \"$d\" gpid; then echo \"STATUS running\"; exit 0; fi\n[ -e \"$d/code\" ] && { echo \"STATUS done $(cat \"$d/code\")\"; st; exit 0; }\necho \"STATUS gone\"; st\n"

def Ship.base(+home: String) -> String:
  home ++ "/deploy"

def Ship.dir(+home: String, +commit: String) -> String:
  Ship.base(home) ++ "/" ++ String.take(commit, 12n)

def ShipJob.say(+inbox: Chan(Msg), m: Msg) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, m)
    return Unit{}

def ShipJob.out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      out

# origin/main read for a deploy_main call, which the hub then answers
def ShipJob.ask(+repo: String, +cands: String, +thread: String, +body: String, +reply: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", Ship.look.sh(), "sh", repo, cands], "/")
    ShipJob.say(inbox, ShipAsked{thread, body, reply, ShipJob.out(r)})

# origin/main read again (why: next, follow)
def ShipJob.next(+repo: String, +cands: String, +why: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", Ship.look.sh(), "sh", repo, cands], "/")
    ShipJob.say(inbox, ShipNewest{why, ShipJob.out(r)})

def ShipJob.got.k(run: Bool, +out: String, +commit: String, +inbox: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match run:
    case True{}:
      k(Unit{})
    case False{}:
      ShipJob.say(inbox, ShipDone{commit, out})

def ShipJob.got(r: P.Ran, +commit: String, +inbox: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match r:
    case P.Ran{_, +out}:
      ShipJob.got.k(String.eq(SHP.Ran.status(out), "running"), out, commit, inbox, k)

# the build's job looked at every 5 s until it ends or is gone
def ShipJob.loop(fuel: Nat, +d: String, +commit: String, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        r : P.Ran <- P.Proc.run("sh", ["-c", Ship.check.sh(), "sh", d], "/")
        ShipJob.got(r, commit, inbox, u => IO.bind(Unit, Unit, IO.sleep(5000), v => ShipJob.loop(f, d, commit, inbox)))

def ShipJob.pid.k(+out: String, +commit: String, +inbox: Chan(Msg)) -> IO(Unit):
  match out:
    case SNil{}:
      IO.pure(Unit, Unit{})
    case SCon{_, _}:
      ShipJob.say(inbox, ShipPid{commit, out})

# launched now: the hub records its pid (ShipPid); launched before (a
# restart) or not at all, nothing is recorded and the look tells
def ShipJob.launched(+out: String, +commit: String, +inbox: Chan(Msg)) -> IO(Unit):
  ShipJob.pid.k(SHP.Look.pick(String.lines(out), "PID "), commit, inbox)

def ShipJob.run(+repo: String, +wrap: String, +home: String, +commit: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", Ship.launch.sh(), "sh", Ship.base(home), repo, commit, wrap, Ship.run.sh()], "/")
    ShipJob.launched(ShipJob.out(r), commit, inbox)
    ShipJob.loop(Fuel.forever(), Ship.dir(home, commit), commit, inbox)

def Upgrade.done.k(ok: Bool, force: Bool, +err: String, +inbox: Chan(Msg)) -> IO(Unit):
  match ok:
    case True{}:
      do IO<Unit>:
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, UpdateReady{force})
        return Unit{}
    case False{}:
      Upgrade.say(inbox, Upgrade.note([J.KV{"error", J.Str{err}}]))

def Upgrade.done(force: Bool, +err: String, +inbox: Chan(Msg)) -> IO(Unit):
  Upgrade.done.k(String.is_empty(err), force, err, inbox)

# install Backplane KiCad beside our data and restart to pick it up
def Install.go(+home: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Upgrade.say(inbox, Upgrade.note([J.KV{"state", J.Str{"installing Backplane KiCad"}}]))
    err : String <- K.KiCad.install(home)
    Upgrade.done(False{}, err, inbox)

# install the channel's newest release and restart into it
def Upgrade.go.k(force: Bool, +home: String, +channel: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Upgrade.say(inbox, Upgrade.note([J.KV{"state", J.Str{"updating"}}]))
    exe : String <- H.Sys.exe_dir()
    err : String <- UP.Update.apply(channel, exe, home)
    Upgrade.done(force, err, inbox)

def Upgrade.go(managed: Bool, force: Bool, +home: String, +channel: String, +inbox: Chan(Msg)) -> IO(Unit):
  match managed:
    case True{}:
      Upgrade.say(inbox, Upgrade.note([J.KV{"error", J.Str{"this install updates through its package"}}]))
    case False{}:
      Upgrade.go.k(force, home, channel, inbox)

def Agent.pid_is(m: Maybe<&2, Agent>, +pid: U32) -> Bool:
  match m:
    case Some{Agent{_, _, p, _, _}}:
      U32.is_eq(p, pid)
    case None{}:
      False{}

def Cfg.models(c: Cfg) -> String:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, ms, _, _, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      ms

def Cfg.pk(c: Cfg) -> Maybe<&2, Chan(PK.PMsg)>:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, _, _, pk, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      pk

def Park.put(m: Maybe<&2, Chan(PK.PMsg)>, +msg: PK.PMsg) -> IO(Unit):
  match m:
    case Some{q}:
      do IO<Unit>:
        ok : Result<&1, &1, PK.PMsg, Unit> <- Chan.send(PK.PMsg, q, msg)
        return Unit{}
    case None{}:
      IO.pure(Unit, Unit{})

# a phone push for an ended turn, off the hub loop; the tokens APNs calls
# dead come back as device.drop requests, as if a client had sent them
def Hub.drop_dead(ts: List<&2, String>, +inbox: Chan(Msg)) -> IO(Unit):
  match ts:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case t <> rest:
      IO.bind(Result<&1, &1, Msg, Unit>, Unit, Chan.send(Msg, inbox, Rpc{0, PU.Push.drop_request(t)}), _ => Hub.drop_dead(rest, inbox))

def Hub.island(+home: String, st: M.State, +inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(List<&2, String>, Unit, PU.Push.island.dead(home, st), ds => Hub.drop_dead(ds, inbox))

def Hub.phone(+home: String, st: M.State, +thread: String, +inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(List<&2, String>, Unit, PU.Push.turn_ended.dead(home, st, thread), ds => Hub.drop_dead(ds, inbox))

def Desk.lead.held(seen: Bool, m: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match seen:
    case True{}:
      None{}
    case False{}:
      m

def Desk.tell.to(m: Maybe<&2, U32>, +cl: List<&2, Client>, +st: M.State, +thread: String, +rec: NO.AlertRec) -> IO(List<&2, Client>):
  match m:
    case Some{+c}:
      Wire.to(cl, c, DK.Desk.notify(thread, PU.Push.title(B.Hub.thread(st, thread)), PU.Push.alert.body(st, thread, rec), NO.Alert.kind(rec), NO.Alert.rkey(rec)))
    case None{}:
      IO.pure(List<&2, Client>, cl)

# Time zones
# ----------
# The hub looks a zone's POSIX rule up in the system's zoneinfo (the footer
# line of its TZif file, `tail -n 1`), so routines and the person's zone
# follow daylight saving. A zone it cannot read that way but the system
# knows is taken as the offset it has now. A name is checked first
# (SCH.Zone.valid): letters, digits, _ + - between slashes, nothing else.

def ZoneJob.path(+name: String) -> String:
  "/usr/share/zoneinfo/" ++ name

def ZoneJob.footer(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      String.trim(out)
    case P.Ran{_, _}:
      ""

def ZoneJob.offset.k(+z: String) -> String:
  Bool.pick(String, Bool.and(Nat.is_eq(String.length(z), 5n), Bool.or(String.starts_with(z, "+"), String.starts_with(z, "-"))), SCH.Zone.fixed.rule(z), "")

# date +%z of a zone the system knows: +hhmm / -hhmm, else ""
def ZoneJob.offset(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      ZoneJob.offset.k(String.trim(out))
    case P.Ran{_, _}:
      ""

def ZoneJob.fallback(+name: String) -> IO(String):
  do IO<String>:
    r : P.Ran <- P.Proc.run("sh", ["-c", "[ -f \"$2\" ] && TZ=\"$1\" date +%z", "sh", name, ZoneJob.path(name)], "/")
    return ZoneJob.offset(r)

def ZoneJob.rule.k(ok: Bool, +line: String, +name: String) -> IO(String):
  match ok:
    case True{}:
      IO.pure(String, line)
    case False{}:
      ZoneJob.fallback(name)

def ZoneJob.rule.v(valid: Bool, +name: String) -> IO(String):
  match valid:
    case True{}:
      do IO<String>:
        r : P.Ran <- P.Proc.run("tail", ["-n", "1", ZoneJob.path(name)], "/")
        +line : String = ZoneJob.footer(r)
        ZoneJob.rule.k(BO.Maybe.is(TZN.TzRule, TZN.Tz.parse(line)), line, name)
    case False{}:
      IO.pure(String, "")

# a zone's rule, "" when the name is not valid or the system does not know it
def ZoneJob.rule(+name: String) -> IO(String):
  ZoneJob.rule.v(SCH.Zone.valid(name), name)

# this machine's own zone: $TZ, else /etc/localtime's target, else /etc/timezone
def ZoneJob.machine.sh() -> String:
  "z=${TZ#:}; if [ -z \"$z\" ]; then l=$(readlink /etc/localtime 2>/dev/null); case $l in */zoneinfo/*) z=${l#*zoneinfo/};; esac; fi; if [ -z \"$z\" ] && [ -f /etc/timezone ]; then z=$(cat /etc/timezone); fi; echo \"$z\""

def ZoneJob.machine.zone(+z: String) -> String:
  Bool.pick(String, SCH.Zone.valid(z), z, "")

def ZoneJob.machine() -> IO(String):
  do IO<String>:
    r : P.Ran <- P.Proc.run("sh", ["-c", ZoneJob.machine.sh()], "/")
    return ZoneJob.machine.zone(ZoneJob.footer(r))

# the setting for a zone's rule when it differs from the one held
def ZoneJob.log(+name: String, +rule: String, +old: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Bool.or(String.is_empty(rule), String.eq(rule, old)), Nil{}, [M.SettingSet{"tz.rule." ++ name, rule, now}])

def ZoneJob.one.k(parts: List<&2, String>, +now: Nat) -> IO(List<&2, M.Change>):
  match parts:
    case +name <> +old <> Nil{}:
      IO.bind(String, List<&2, M.Change>, ZoneJob.rule(name), rule => IO.pure(List<&2, M.Change>, ZoneJob.log(name, rule, old, now)))
    case +name <> Nil{}:
      IO.bind(String, List<&2, M.Change>, ZoneJob.rule(name), rule => IO.pure(List<&2, M.Change>, ZoneJob.log(name, rule, "", now)))
    case _:
      IO.pure(List<&2, M.Change>, Nil{})

# one "name<tab>rule held" line of work
def ZoneJob.one(line: String, +now: Nat) -> IO(List<&2, M.Change>):
  ZoneJob.one.k(String.split(line, Chr{9}), now)

def ZoneJob.each(xs: List<&2, String>, +now: Nat, acc: List<&2, M.Change>) -> IO(List<&2, M.Change>):
  match xs:
    case Nil{}:
      IO.pure(List<&2, M.Change>, acc)
    case +l <> t:
      IO.bind(List<&2, M.Change>, List<&2, M.Change>, ZoneJob.one(l, now), c => ZoneJob.each(t, now, List.append(&2, M.Change, acc, c)))

def ZoneJob.machine.log(+z: String, +old: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Bool.or(String.is_empty(z), String.eq(z, old)), Nil{}, [M.SettingSet{"tz.machine", z, now}])

# a newly found machine zone comes with its rule, so a routine in the
# person's zone (which falls back to the machine's) is due from the next
# tick on, not one tick after the rule is looked up
def ZoneJob.machine.rule(same: Bool, +z: String, +old: String, +now: Nat) -> IO(List<&2, M.Change>):
  match same:
    case True{}:
      IO.pure(List<&2, M.Change>, Nil{})
    case False{}:
      IO.bind(String, List<&2, M.Change>, ZoneJob.rule(z), rule => IO.pure(List<&2, M.Change>, List.append(&2, M.Change, ZoneJob.machine.log(z, old, now), ZoneJob.log(z, rule, "", now))))

def ZoneJob.machine.log.go(detect: Bool, +old: String, +now: Nat) -> IO(List<&2, M.Change>):
  match detect:
    case True{}:
      IO.bind(String, List<&2, M.Change>, ZoneJob.machine(), +z => ZoneJob.machine.rule(Bool.or(String.is_empty(z), String.eq(z, old)), z, old, now))
    case False{}:
      IO.pure(List<&2, M.Change>, Nil{})

def ZoneJob.send(cs: List<&2, M.Change>, +inbox: Chan(Msg)) -> IO(Unit):
  match cs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case _ <> _:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{cs})
        return Unit{}

# the rules of the zones in `todo`, this machine's zone when `detect`, and
# the day's mark when `mark`: one Commit with what changed
def ZoneJob.run(+inbox: Chan(Msg), todo: List<&2, String>, detect: Bool, +old: String, mark: Bool, +now: Nat) -> IO(Unit):
  do IO<Unit>:
    cs1 : List<&2, M.Change> <- ZoneJob.each(todo, now, Nil{})
    cs2 : List<&2, M.Change> <- ZoneJob.machine.log.go(detect, old, now)
    ZoneJob.send(List.concat(&2, M.Change, [cs1, cs2, Bool.pick(List<&2, M.Change>, mark, [M.SettingSet{"tz.checked", Nat.show(now), now}], Nil{})]), inbox)

# the minute tick: zones with no rule yet are looked up at once, all of them
# (and this machine's own) once a day
def Hub.zone.run(go: Bool, +inbox: Chan(Msg), todo: List<&2, String>, detect: Bool, +old: String, mark: Bool, +now: Nat) -> IO(Unit):
  match go:
    case True{}:
      IO.spawn(Unit, ZoneJob.run(inbox, todo, detect, old, mark, now))
    case False{}:
      IO.pure(Unit, Unit{})

def Hub.zone.tick.k(+stale: Bool, +st: M.State, +inbox: Chan(Msg), +now: Nat) -> IO(Unit):
  Hub.zone.run(Bool.or(stale, Bool.or(Bool.not(List.is_empty(&2, String, SCH.Zone.todo(st, stale))), String.is_empty(M.Setting.get(st, "tz.machine", "")))),
    inbox, SCH.Zone.todo(st, stale), Bool.or(stale, String.is_empty(M.Setting.get(st, "tz.machine", ""))), M.Setting.get(st, "tz.machine", ""), stale, now)

def Hub.zone.tick(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        Hub.zone.tick.k(SCH.Zone.stale(st, now), st, inbox, now)
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# an alert as a desktop notification on the desk used last: none when a
# desk watches the thread and the alert is one a watcher holds (a note is
# never held)
def Desk.tell(seen: Bool, +cl: List<&2, Client>, +st: M.State, +thread: String, +rec: NO.AlertRec) -> IO(List<&2, Client>):
  Desk.tell.to(Desk.lead.held(seen, DK.Desk.lead(Clients.looks(cl))), cl, st, thread, rec)

# the reply now, or (for a request that waits on the user) parked until
# the ask it opened is answered
def Hub.mcp.send(later: Bool, +park: String, +body: String, +text: String, +reply: Chan(String), +cfg: Cfg) -> IO(Unit):
  match later:
    case True{}:
      Park.put(Cfg.pk(cfg), PK.PPark{park, body, reply})
    case False{}:
      do IO<Unit>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, text)
        return Unit{}

def Net.with_br(n: Net, q: Chan(BR.BMsg)) -> Net:
  match n:
    case Net{a, b, c, d, e, f, _, g, i, mc, pvx, bnx, mgd, lk, fc, ksx}:
      Net{a, b, c, d, e, f, Some{q}, g, i, mc, pvx, bnx, mgd, lk, fc, ksx}

def Net.with_ks(n: Net, q: Chan(KSV.KMsg)) -> Net:
  match n:
    case Net{a, b, c, d, e, f, g, h, i, mc, pvx, bnx, mgd, lk, fc, _}:
      Net{a, b, c, d, e, f, g, h, i, mc, pvx, bnx, mgd, lk, fc, Some{q}}

def Cfg.ks(c: Cfg) -> Maybe<&2, Chan(KSV.KMsg)>:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, x}, +tcfg, +views}:
      x

def Net.with_tm(n: Net, q: Chan(TM.TMsg)) -> Net:
  match n:
    case Net{a, b, c, d, e, f, g, h, _, mc, pvx, bnx, mgd, lk, fc, ksx}:
      Net{a, b, c, d, e, f, g, h, Some{q}, mc, pvx, bnx, mgd, lk, fc, ksx}

def Net.with_pv(n: Net, q: Chan(PV.PMsg)) -> Net:
  match n:
    case Net{a, b, c, d, e, f, g, h, i, mc, _, bnx, mgd, lk, fc, ksx}:
      Net{a, b, c, d, e, f, g, h, i, mc, Some{q}, bnx, mgd, lk, fc, ksx}

def Cfg.pv(c: Cfg) -> Maybe<&2, Chan(PV.PMsg)>:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, _, _, _, _, _, pv, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      pv

def Cfg.tm(c: Cfg) -> Maybe<&2, Chan(TM.TMsg)>:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, _, _, _, tm, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      tm

# a shell's output, to the hub (which sends it to every client)
def Term.fwd.got(m: Maybe<&1, TM.TOut>, +inbox: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{TM.TOut{th, bs}}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, TermOut{th, bs})
        k(Unit{})
    case Some{TM.TTo{c, key, co, ro, bs}}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, TermTo{c, key, co, ro, bs})
        k(Unit{})
    case None{}:
      IO.pure(Unit, Unit{})

def Term.fwd(fuel: Nat, +out: Chan(TM.TOut), +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, TM.TOut>, Unit, Chan.recv(TM.TOut, out), m => Term.fwd.got(m, inbox, u => Term.fwd(f, out, inbox)))

def Term.put(m: Maybe<&2, Chan(TM.TMsg)>, +msg: TM.TMsg) -> IO(Unit):
  match m:
    case Some{q}:
      do IO<Unit>:
        ok : Result<&1, &1, TM.TMsg, Unit> <- Chan.send(TM.TMsg, q, msg)
        return Unit{}
    case None{}:
      IO.pure(Unit, Unit{})

# The phones' viewer
# ------------------

# a plot for a phone, to the hub (which writes it to that phone's socket)
def Plot.fwd.got(m: Maybe<&1, PV.POut>, +inbox: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{PV.POut{client, text}}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Push{client, text})
        k(Unit{})
    case Some{PV.PBin{client, bs}}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, PushBin{client, bs})
        k(Unit{})
    case None{}:
      IO.pure(Unit, Unit{})

def Plot.fwd(fuel: Nat, +out: Chan(PV.POut), +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, PV.POut>, Unit, Chan.recv(PV.POut, out), m => Plot.fwd.got(m, inbox, u => Plot.fwd(f, out, inbox)))

def Plot.put(m: Maybe<&2, Chan(PV.PMsg)>, +msg: PV.PMsg) -> IO(Unit):
  match m:
    case Some{q}:
      do IO<Unit>:
        ok : Result<&1, &1, PV.PMsg, Unit> <- Chan.send(PV.PMsg, q, msg)
        return Unit{}
    case None{}:
      IO.pure(Unit, Unit{})

def CefCfg.depart(c: Cfg, +client: U32) -> Cfg:
  match c:
    case Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, +bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx,
        BN.Bn.with_cef(bnx, CF.Cef.session.leave(BN.Bn.cef(bnx), client)), mgd, lk, fc, ksx}, tcfg, views}

def Hub.leave(h: Hub, +client: U32, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        Hub.leave.close(Clients.get(cl, client))
        Plot.put(Cfg.pv(cfg), PV.PStop{client})
        +cl2 : List<&2, Client> = Clients.drop(cl, client)
        cl3 : List<&2, Client> <- Desk.say(DK.Desk.text(Clients.looks(cl), now), cl2, now)
        return Hub{st, f, ls, cl3, as, inbox, CefCfg.depart(cfg, client), note}

# kicad.watch: kind "board", "schematic", "3d", "3d-bare" or "3d-full"
# shows it, anything else stops
def Plot.watch(+client: U32, +kind: String, +root: String, +path: String) -> PV.PMsg:
  Bool.pick(PV.PMsg, String.eq(kind, "board"), PV.PWatch{client, 0, root, path},
    Bool.pick(PV.PMsg, String.eq(kind, "schematic"), PV.PWatch{client, 1, root, path},
      Bool.pick(PV.PMsg, String.eq(kind, "3d"), PV.PWatch{client, 2, root, path},
        Bool.pick(PV.PMsg, String.eq(kind, "3d-bare"), PV.PWatch{client, 3, root, path},
          Bool.pick(PV.PMsg, String.eq(kind, "3d-full"), PV.PWatch{client, 4, root, path}, PV.PStop{client})))))

def Term.msg(+op: String, +thread: String, +cwd: String, data: List<&2, U32>, +cols: U32, +rows: U32) -> TM.TMsg:
  Bool.pick(TM.TMsg, String.eq(op, "open"), TM.TOpen{thread, cwd, cols, rows},
    Bool.pick(TM.TMsg, String.eq(op, "resize"), TM.TSize{thread, cols, rows}, TM.TIn{thread, data}))

# User and project skills
# -----------------------

def Skills.ukeep(empty: Bool, s: AG.USkill, rest: List<&2, AG.USkill>) -> List<&2, AG.USkill>:
  match empty:
    case True{}:
      rest
    case False{}:
      s <> rest

def Skills.uread(ds: List<&2, String>, +root: String) -> IO(List<&2, AG.USkill>):
  match ds:
    case Nil{}:
      IO.pure(List<&2, AG.USkill>, Nil{})
    case +d <> t:
      do IO<List<&2, AG.USkill>>:
        +path : String = root ++ "/" ++ d ++ "/SKILL.md"
        +text : String <- Files.text(path)
        rest : List<&2, AG.USkill> <- Skills.uread(t, root)
        return Skills.ukeep(String.is_empty(text), AG.USkill.parse(text, d, path), rest)

def Skills.names(r: Result<&1, &1, U32 & String, List<String>>) -> List<&2, String>:
  match r:
    case Done{xs}:
      Names.copy(xs)
    case Fail{_}:
      Nil{}

def Skills.dir(+root: String) -> IO(List<&2, AG.USkill>):
  IO.bind(Result<&1, &1, U32 & String, List<String>>, List<&2, AG.USkill>, H.FS.list(root), r => Skills.uread(Skills.names(r), root))

def Skills.scan.k(+home: String, +cwd: String) -> IO(List<&2, AG.USkill>):
  do IO<List<&2, AG.USkill>>:
    us : List<&2, AG.USkill> <- Skills.dir(home ++ "/.claude/skills")
    ps : List<&2, AG.USkill> <- Skills.dir(cwd ++ "/.claude/skills")
    exe : String <- H.Sys.exe_dir()
    ks : List<&2, AG.USkill> <- Skills.dir(K.KiStack.dir(exe) ++ "/skills")
    return AG.USkill.merge(AG.USkill.merge(us, ps), ks)

# ~/.claude/skills, then <cwd>/.claude/skills (the user's win a clash),
# then KiStack's
def Skills.scan(+cwd: String) -> IO(List<&2, AG.USkill>):
  IO.bind(String, List<&2, AG.USkill>, Env.get("HOME", "/nonexistent"), home => Skills.scan.k(home, cwd))

def AgentJob.skills(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String) -> IO(Unit):
  IO.bind(List<&2, AG.USkill>, Unit, Skills.scan(cwd), ss =>
    GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"skills", AG.USkill.json(ss)}, J.KV{"thread", J.Str{thread}}])))

# a job for the hub's Git effect: the skills a composer offers (anywhere),
# or a git job (in a repository)
def Jobs.run(+tx: AG.Tx, +job: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.eq(job, "skills"), AgentJob.skills(inbox, client, id, thread, cwd), GitJob.run(tx, job, inbox, client, id, thread, cwd, a, b, home))

# a thread's reasoning effort setting ("" for the provider's default)
def Hub.effort_of(+st: M.State, +thread: String) -> String:
  M.Setting.get(st, AG.Effort.key(thread), "")

def Mention.user_text(+o: J.Json) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content"))), J.Null{}), "text", "")

def Mention.line.k(m: Maybe<&2, J.Json>, +line: String, +ss: List<&2, AG.USkill>) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, String.eq(J.Json.get_str(o, "type", ""), "user"), B.Claude.user.u(AG.Mention.expand("claude", Mention.user_text(o), ss), J.Json.get_str(o, "uuid", "")), line)
    case None{}:
      line

# a Claude stream-json user line with its `$skill` mentions as `/skill`
def Mention.line(+line: String, +ss: List<&2, AG.USkill>) -> String:
  Mention.line.k(J.Json.parse(line), line, ss)

def Hub.agent_send.k(has: Bool, m: Maybe<&2, Agent>, +line: String, +cwd: String) -> IO(Unit):
  match has:
    case True{}:
      IO.bind(List<&2, AG.USkill>, Unit, Skills.scan(cwd), ss => Hub.agent_send(m, Mention.line(line, ss)))
    case False{}:
      Hub.agent_send(m, line)

# a line for Claude with its `$skill` mentions as `/skill`
def Agent.line.final(has: Bool, +line: String, +cwd: String) -> IO(String):
  match has:
    case True{}:
      IO.bind(List<&2, AG.USkill>, String, Skills.scan(cwd), ss => IO.pure(String, Mention.line(line, ss)))
    case False{}:
      IO.pure(String, line)

# The command ledger (core/cmds.bend)
# -----------------------------------
# Every user line the hub writes to a Claude process carries its uuid; the
# ledger counts it from the moment the hub takes it (a message with images
# waits for them, and the ones after it wait behind it), and Stop makes
# every unanswered one a tomb. Lines from the process are judged against
# it (Hub.line.run).

def ARun.cmds(r: ARun) -> CM.Cmds:
  match r:
    case ARun{c, _, _, _, _, _, _, _}:
      c

def ARun.held(r: ARun) -> List<&2, CM.Held>:
  match r:
    case ARun{_, h, _, _, _, _, _, _}:
      h

def ARun.put(r: ARun, c: CM.Cmds, h: List<&2, CM.Held>) -> ARun:
  match r:
    case ARun{_, _, l, n, g, q, pa, pl}:
      ARun{c, h, l, n, g, q, pa, pl}

def Hub.agent_send.all(+m: Maybe<&2, Agent>, ls: List<&2, String>) -> IO(Unit):
  match ls:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case l <> t:
      do IO<Unit>:
        Hub.agent_send(m, l)
        Hub.agent_send.all(m, t)

# the held lines whose turn has come go out, in order
def Claude.flush.k(fl: CM.Flush, +r: ARun, h: Hub, +thread: String) -> IO(Hub):
  match fl h:
    case CM.Flush{out, +sent, rest} Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      do IO<Hub>:
        Hub.agent_send.all(Agents.get(as, thread), out)
        return Hub{st, f, ls, cl, Agents.with(as, thread, ARun.put(r, CM.Cmds.sent.all(sent, ARun.cmds(r)), rest)), inbox, cfg, note}

def Claude.flush(+r: ARun, h: Hub, +thread: String) -> IO(Hub):
  Claude.flush.k(CM.Held.flush(ARun.held(r), ARun.cmds(r)), r, h, thread)

def Claude.raw(h: Hub, +thread: String, line: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      do IO<Hub>:
        Hub.agent_send(Agents.get(as, thread), line)
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# 1: a user message (its uuid first, B.Claude.tag); 2: the interrupt
def Claude.put.kind(+line: String) -> Nat:
  Bool.pick(Nat, String.starts_with(line, "{\"uuid\":"), 1n,
    Bool.pick(Nat, Bool.and(String.starts_with(line, "{\"type\":\"control_request\""), String.contains(line, "\"subtype\":\"interrupt\"")), 2n, 0n))

def Claude.put.k(i: Nat, +r: ARun, h: Hub, +thread: String, +line: String) -> IO(Hub):
  match i:
    case 1n:
      Claude.flush(ARun.put(r, CM.Cmds.accept(ARun.cmds(r), B.Claude.uuid.of(line)), CM.Held.push(ARun.held(r), B.Claude.uuid.of(line), line, True{})), h, thread)
    case 2n:
      IO.bind(Hub, Hub, Claude.raw(h, thread, line), h2 => Claude.flush(ARun.put(r, CM.Cmds.stop(ARun.cmds(r), Nil{}), ARun.held(r)), h2, thread))
    case _:
      Claude.raw(h, thread, line)

def Claude.put.r(m: Maybe<&2, ARun>, h: Hub, +thread: String, +line: String) -> IO(Hub):
  match m:
    case Some{+r}:
      Claude.put.k(Claude.put.kind(line), r, h, thread, line)
    case None{}:
      Claude.raw(h, thread, line)

# a line for a thread's agent
def Claude.put(h: Hub, +thread: String, +line: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Claude.put.r(Agents.run(as, thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, line)

# a line heard now: the silence note may come again after a new silence
def ARun.heard.set(r: ARun, +now: Nat) -> ARun:
  match r:
    case ARun{c, h, l, _, g, _, pa, pl}:
      ARun{c, h, l, now, g, False{}, pa, pl}

def ARun.prog_at(r: ARun) -> Nat:
  match r:
    case ARun{_, _, _, _, _, _, pa, _}:
      pa

def ARun.prog_last(r: ARun) -> List<&2, String>:
  match r:
    case ARun{_, _, _, _, _, _, _, pl}:
      pl

# the progress lines held back and when the last went through
# (B.Prog.line)
def ARun.prog.set(r: ARun, +at: Nat, pend: List<&2, String>) -> ARun:
  match r:
    case ARun{c, h, l, n, g, q, _, _}:
      ARun{c, h, l, n, g, q, at, pend}

def ARun.gen(r: ARun) -> String:
  match r:
    case ARun{_, _, _, _, g, _, _, _}:
      g

# (a message prepared for an earlier process, or stopped while its images
# were read, is dropped: B.Gen.write)
def Claude.ready.k(ok: Bool, +r: ARun, h: Hub, +thread: String, +u: String, +line: String) -> IO(Hub):
  match ok:
    case True{}:
      Claude.flush(ARun.put(r, ARun.cmds(r), CM.Held.ready(ARun.held(r), u, line)), h, thread)
    case False{}:
      do IO<Hub>:
        Log.info("dropped a prepared message for " ++ thread ++ ": its process is gone or it was stopped")
        Claude.flush(r, h, thread)

def Claude.ready.r(m: Maybe<&2, ARun>, h: Hub, +thread: String, +u: String, +line: String, +gen: String) -> IO(Hub):
  match m:
    case Some{+r}:
      Claude.ready.k(B.Gen.write(gen, ARun.gen(r), CM.Cmds.state(ARun.cmds(r), u)), r, h, thread, u, line)
    case None{}:
      IO.pure(Hub, h)

def Claude.ready.u(empty: Bool, h: Hub, +thread: String, +u: String, +line: String, +gen: String) -> IO(Hub):
  match empty h:
    case True{} _:
      Claude.raw(h, thread, line)
    case False{} Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Claude.ready.r(Agents.run(as, thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, u, line, gen)

def Claude.ready(h: Hub, +thread: String, +line: String, +gen: String) -> IO(Hub):
  Claude.ready.u(String.is_empty(B.Claude.uuid.of(line)), h, thread, B.Claude.uuid.of(line), line, gen)

def Claude.prep.r(m: Maybe<&2, ARun>, h: Hub, +thread: String, +u: String) -> Hub:
  match m h:
    case Some{+r} Hub{st, f, ls, cl, as, inbox, cfg, note}:
      Hub{st, f, ls, cl, Agents.with(as, thread, ARun.put(r, CM.Cmds.accept(ARun.cmds(r), u), CM.Held.push(ARun.held(r), u, "", False{}))), inbox, cfg, note}
    case None{} _:
      h

# a message whose images are being read is taken now: those after it wait
def Claude.prep(on: Bool, h: Hub, +thread: String, +u: String) -> Hub:
  match on h:
    case True{} Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Claude.prep.r(Agents.run(as, thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, u)
    case False{} _:
      h

# a Codex or Grok turn's KiCad files, read in its own job (the hub's loop
# never waits on the walk); the hub is told to open the panel
def Files.prepared(+thread: String, +cwd: String, +inbox: Chan(Msg)) -> IO(K.Files):
  do IO<K.Files>:
    +fs : K.Files <- K.Files.read(cwd)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AutoView{thread, fs})
    return fs

# opencode: `opencode acp` under `env`, its model and permissions as
# config in OPENCODE_CONFIG_CONTENT (no flag takes them); what it asks
# goes to the person like Grok's (Grok.asks)
def Oc.perm(r: M.RuntimeMode) -> String:
  match r:
    case M.Supervised{}:
      "ask"
    case M.AcceptEdits{}:
      "edit"
    case _:
      "allow"

def Oc.config(+model: String, +perm: String) -> String:
  J.Json.show(J.Json.obj(List.append(&2, J.KV,
    Bool.pick(List<&2, J.KV>, Bool.or(String.is_empty(model), String.eq(model, "default")), Nil{}, [J.KV{"model", J.Str{model}}]),
    [J.KV{"permission", J.Json.obj([J.KV{"edit", J.Str{Bool.pick(String, String.eq(perm, "ask"), "ask", "allow")}},
      J.KV{"bash", J.Str{Bool.pick(String, String.eq(perm, "allow"), "allow", "ask")}}, J.KV{"webfetch", J.Str{Bool.pick(String, String.eq(perm, "allow"), "allow", "ask")}}])}])))

def Oc.args(r: M.RuntimeMode, +model: String) -> List<&2, String>:
  ["OPENCODE_CONFIG_CONTENT=" ++ Oc.config(model, Oc.perm(r)), "opencode", "acp"]

# one Codex turn: the thread's skills join its instructions and its
# `$skill` mentions become instructions to read them
def Codex.go(+cfg: Cfg, +st: M.State, +thread: String, +cwd: String, +model: String, +session: String, r: M.RuntimeMode, +text: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +fs : K.Files <- Files.prepared(thread, cwd, inbox)
    +ss : List<&2, AG.USkill> <- Skills.scan(cwd)
    +keys : String <- AKS.Ak.prompt(Cfg.home(cfg))
    Codex.run(Codex.args(cfg, thread, model, session, r, AG.Mention.expand("codex", text, ss), Hub.prompt(cfg, st, thread, cwd, fs, keys) ++ AG.USkill.prompt(ss), Hub.effort_of(st, thread), gen, MD.Disposition.launch(st, thread)), cwd, thread, gen, inbox)

def Grok.go(+cfg: Cfg, +st: M.State, +thread: String, +cwd: String, +model: String, +session: String, +r: M.RuntimeMode, +text: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +fs : K.Files <- Files.prepared(thread, cwd, inbox)
    +ss : List<&2, AG.USkill> <- Skills.scan(cwd)
    +keys : String <- AKS.Ak.prompt(Cfg.home(cfg))
    Grok.run(Grok.args(r, model, Hub.effort_of(st, thread)),
      MS.PAcp{AC.AcpTurn{cwd, Hub.mcp_url.g(cfg, thread, gen), Hub.prompt(cfg, st, thread, cwd, fs, keys) ++ AG.USkill.prompt(ss), AG.Mention.expand("grok", text, ss), session, Grok.asks(r)}}, cwd, thread, gen, inbox)

# one opencode turn: Grok's runner, the same ACP
def Oc.go(+cfg: Cfg, +st: M.State, +thread: String, +cwd: String, +model: String, +session: String, +r: M.RuntimeMode, +text: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +fs : K.Files <- Files.prepared(thread, cwd, inbox)
    +ss : List<&2, AG.USkill> <- Skills.scan(cwd)
    +keys : String <- AKS.Ak.prompt(Cfg.home(cfg))
    Grok.run(Oc.args(r, model),
      MS.POc{AC.AcpTurn{cwd, Hub.mcp_url.g(cfg, thread, gen), Hub.prompt(cfg, st, thread, cwd, fs, keys) ++ AG.USkill.prompt(ss), AG.Mention.expand("opencode", text, ss), session, Grok.asks(r)}}, cwd, thread, gen, inbox)

# one Muse turn: Grok's runner over MSP; the turn's gen makes its command
# ids (Muse refuses one it has seen)
def Muse.go(+cfg: Cfg, +st: M.State, +thread: String, +cwd: String, +model: String, +session: String, +r: M.RuntimeMode, +text: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +fs : K.Files <- Files.prepared(thread, cwd, inbox)
    +ss : List<&2, AG.USkill> <- Skills.scan(cwd)
    +keys : String <- AKS.Ak.prompt(Cfg.home(cfg))
    Grok.run(Muse.args(r, model),
      MS.PMsp{MS.MspTurn{cwd, Hub.mcp_url.g(cfg, thread, gen), Hub.prompt(cfg, st, thread, cwd, fs, keys) ++ AG.USkill.prompt(ss), AG.Mention.expand("muse", text, ss), session,
        Muse.mode(r), AG.Effort.of("muse", Hub.effort_of(st, thread)), gen}}, cwd, thread, gen, inbox)

# one Antigravity turn: Grok's runner over Google's own agent (agy.bend),
# started through its runner script with the sign-in the settings name
# (agy.auth, agy.account, agy.gcp.project, agy.gcp.location)
def Agt.mode(r: M.RuntimeMode) -> String:
  match r:
    case M.Supervised{}:
      AGY.Agy.mode.k(0n)
    case M.AcceptEdits{}:
      AGY.Agy.mode.k(1n)
    case M.AutoMode{}:
      AGY.Agy.mode.k(2n)
    case M.FullAccess{}:
      AGY.Agy.mode.k(3n)

def Agt.args(+home: String, +st: M.State) -> List<&2, String>:
  AGY.Agy.args(AGX.Agx.root(home), AKS.Ak.path(home), M.Setting.get(st, "agy.auth", ""), M.Setting.get(st, "agy.account", ""),
    M.Setting.get(st, "agy.gcp.project", ""), M.Setting.get(st, "agy.gcp.location", ""))

def Agt.go(+cfg: Cfg, +st: M.State, +thread: String, +cwd: String, +model: String, +session: String, +r: M.RuntimeMode, +text: String, +gen: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +fs : K.Files <- Files.prepared(thread, cwd, inbox)
    +ss : List<&2, AG.USkill> <- Skills.scan(cwd)
    +keys : String <- AKS.Ak.prompt(Cfg.home(cfg))
    w : Bool <- AGX.Agx.write(AGX.Agx.root(Cfg.home(cfg)))
    Grok.run(Agt.args(Cfg.home(cfg), st),
      MS.PAgy{AGY.AgyTurn{AC.AcpTurn{cwd, Hub.mcp_url.g(cfg, thread, gen), Hub.prompt(cfg, st, thread, cwd, fs, keys) ++ AG.USkill.prompt(ss), AG.Mention.expand("antigravity", text, ss), session, Grok.asks(r)},
        AGY.Agy.method(M.Setting.get(st, "agy.auth", "")), Agt.mode(r), model}}, cwd, thread, gen, inbox)

def Term.nums(bs: List<&2, U32>) -> List<&2, String>:
  match bs:
    case Nil{}:
      Nil{}
    case b <> t:
      U32.show(b) <> Term.nums(t)

def Term.json(+thread: String, bs: List<&2, U32>) -> String:
  "{\"t\":\"term\",\"thread\":" ++ J.Json.show(J.Str{thread}) ++ ",\"b\":[" ++ String.join(Term.nums(bs), ",") ++ "]}"

def Net.with_pk(n: Net, q: Chan(PK.PMsg)) -> Net:
  match n:
    case Net{a, b, c, d, e, f, g, _, i, mc, pvx, bnx, mgd, lk, fc, ksx}:
      Net{a, b, c, d, e, f, g, Some{q}, i, mc, pvx, bnx, mgd, lk, fc, ksx}

def Browser.exe() -> IO(String):
  do IO<String>:
    +d : String <- H.Sys.exe_dir()
    ok : Bool <- K.Io.is_file(d ++ "/backplane-browser")
    return Bool.pick(String, ok, d ++ "/backplane-browser", "backplane-browser")

def Cfg.br(c: Cfg) -> Maybe<&2, Chan(BR.BMsg)>:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, _, br, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      br

def Desktop.policy(m: Maybe<&2, Chan(BR.BMsg)>, +owner: String, +known: Bool, +tch: TE.TeachSt) -> IO(Unit):
  match m:
    case Some{q}:
      do IO<Unit>:
        sent : Result<&1, &1, BR.BMsg, Unit> <- Chan.send(BR.BMsg, q, BR.BPolicy{owner, known, DB.Desktop.drive(known, TE.Teach.bot.drive(TE.Teach.get(tch, owner))), DB.Desktop.recording(known, TE.Teach.bot.mode(TE.Teach.get(tch, owner)))})
        return Unit{}
    case None{}: IO.pure(Unit, Unit{})

def Desktop.frame_owner(+text: String) -> String:
  +o = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
  +p = J.Json.get_obj(o, "payload")
  +t = J.Json.get_obj(p, "target")
  J.Json.get_str(t, "owner", J.Json.get_str(p, "owner", J.Json.get_str(J.Json.get_obj(p, "event"), "owner", "")))

# This text is internal to the hub actor's authenticated connected message.
# Ordinary client frames retain their original bytes and cannot supply it.
def Desktop.connection_info(kind: String, text: String, version: String, hub: String) -> String:
  match kind:
    case "connected": J.Json.show(J.Json.obj([J.KV{"version", J.Str{version}}, J.KV{"hub", J.Str{hub}}]))
    case _: text

def Desktop.forward(m: Maybe<&2, Chan(BR.BMsg)>, client: U32, kind: String, text: String, +out: Chan(String), e: String, l: String, +owner: String, +known: Bool, +tch: TE.TeachSt) -> IO(Unit):
  match m:
    case Some{q}:
      do IO<Unit>:
        sent : Result<&1, &1, BR.BMsg, Unit> <- Chan.send(BR.BMsg, q, BR.BRemote{client, kind, text, out, e, l, owner, known,
          DB.Desktop.drive(known, TE.Teach.bot.drive(TE.Teach.get(tch, owner))), DB.Desktop.recording(known, TE.Teach.bot.mode(TE.Teach.get(tch, owner)))})
        return Unit{}
    case None{}: Chan.close(String, out)

def Preview.shot(+o: J.Json) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, String.is_empty(J.Json.get_str(o, "data", "")), Nil{},
    [J.Json.obj([J.KV{"type", J.Str{"image"}}, J.KV{"data", J.Str{J.Json.get_str(o, "data", "")}}, J.KV{"mimeType", J.Str{J.Json.get_str(o, "mimeType", "image/png")}}])])

def Preview.drop.fs(fs: J.Json) -> J.Json:
  match fs:
    case J.Field{"screenshot", _, t}:
      Preview.drop.fs(t)
    case J.Field{k, v, t}:
      J.Field{k, v, Preview.drop.fs(t)}
    case _:
      fs

def Preview.drop_shot(o: J.Json) -> J.Json:
  match o:
    case J.Obj{fs}:
      J.Obj{Preview.drop.fs(fs)}
    case _:
      o

def Preview.blocks(+r: J.Json) -> List<&2, J.Json>:
  J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{J.Json.show(Preview.drop_shot(r))}}])
    <> Preview.shot(J.Json.get_obj(r, "screenshot"))

def Preview.content.k(ok: Bool, r: J.Json, +err: String) -> J.Json:
  match ok:
    case True{}:
      J.Json.obj([J.KV{"content", J.Json.arr(Preview.blocks(r))}, J.KV{"isError", J.Flag{False{}}}])
    case False{}:
      J.Json.obj([J.KV{"content", J.Json.arr([J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{err}}])])}, J.KV{"isError", J.Flag{True{}}}])

# a browser tool's reply as an MCP result: the helper's result as text,
# a screenshot as an image block, an error flagged
def Preview.content(+o: J.Json) -> J.Json:
  Preview.content.k(J.Json.get_bool(o, "ok", False{}), J.Json.get_obj(o, "result"), J.Json.get_str(o, "error", "browser error"))

def Preview.answer(+id: String, +line: String) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":" ++ id ++ ",\"result\":" ++ J.Json.show(Preview.content(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}))) ++ "}"

def Preview.run.k(m: Maybe<&2, Chan(BR.BMsg)>, +page: String, +id: String, +op: String, args: J.Json, +reply: Chan(String)) -> IO(Unit):
  match m:
    case Some{q}:
      do IO<Unit>:
        line : String <- BR.Browser.ask_agent_on(q, page, op, args)
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, Preview.answer(id, line))
        return Unit{}
    case None{}:
      do IO<Unit>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, Preview.answer(id, BR.Browser.fail("no browser in this Backplane")))
        return Unit{}

# a preview_* tool call, answered by the browser off the hub's thread, on
# the page of `page` (a bot's own; "" is the shared page)
def Preview.run(+cfg: Cfg, +page: String, +o: J.Json, +reply: Chan(String)) -> IO(Unit):
  Preview.run.k(Cfg.br(cfg), page, J.Json.as_raw(J.Json.get(o, "id"), "null"), String.drop(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), 8n),
    J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), reply)

def Preview.is(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), String.starts_with(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "preview_"))

# opening or navigating brings the Browser tab forward in every window
def Preview.shows(+o: J.Json) -> Bool:
  Bool.or(String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "preview_open"), String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "preview_navigate"))

# a shell's output goes to every client (each keeps its own screen)
def Hub.term_out(h: Hub, +thread: String, data: List<&2, U32>) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.all(cl, Term.json(thread, data))
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

# a chat terminal from the start: a blank cols x rows screen, then bytes
def Term.json.reset(+key: String, +cols: U32, +rows: U32, bs: List<&2, U32>) -> String:
  "{\"t\":\"term\",\"thread\":" ++ J.Json.show(J.Str{key}) ++ ",\"b\":[" ++ String.join(Term.nums(bs), ",") ++ "],\"reset\":[" ++ U32.show(cols) ++ "," ++ U32.show(rows) ++ "]}"

def Hub.term_to.k(all: Bool, +cl: List<&2, Client>, +client: U32, +text: String) -> IO(List<&2, Client>):
  match all:
    case True{}:
      Wire.all(cl, text)
    case False{}:
      Wire.to(cl, client, text)

def Hub.term_to(h: Hub, +client: U32, +key: String, +cols: U32, +rows: U32, data: List<&2, U32>) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Hub.term_to.k(U32.is_eq(client, TM.Et.all()), cl, client, Term.json.reset(key, cols, rows, data))
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

def Cfg.with_rt(c: Cfg, +rt: K.Rt) -> Cfg:
  match c:
    case Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, old, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, K.Rt.set(rt, "auth.probe", J.Flag{J.Json.get_bool(K.Rt.health(rt), "auth.probe", J.Json.get_bool(K.Rt.health(old), "auth.probe", False{}))}), brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, tcfg, views}

def Cfg.with_models(c: Cfg, +ms: String, +mt: String) -> Cfg:
  match c:
    case Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, _, _, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, tcfg, views}

def Hub.models.k(h: Hub) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.all(cl, "{\"t\":\"info\",\"info\":" ++ J.Json.show(Hub.info(cfg)) ++ "}")
        cl3 : List<&2, Client> <- Wire.pair(cl2, Hub.pair_msg(cfg))
        return Hub{st, f, ls, cl3, as, inbox, cfg, note}

# One check at a time. A retry while busy is remembered; an answer for
# an old path is discarded and immediately checked with the new setting.
def KJob.run(+inbox: Chan(Msg), +home: String, +setting: String, force: Bool, +last: K.Rt) -> IO(Unit):
  do IO<Unit>:
    exe : String <- H.Sys.exe_dir()
    rt : K.Rt <- K.Rt.refresh(home, setting, exe, force, last)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, KFound{setting, rt})
    return Unit{}

def Hub.kcheck.k(busy: Bool, h: Hub, force: Bool) -> IO(Hub):
  match busy h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      IO.pure(Hub, Hub{st, f, ls, cl, as, inbox, Cfg.with_rt(cfg, K.Rt.set(Cfg.rt(cfg), "again", J.Flag{Bool.or(force, J.Json.get_bool(K.Rt.health(Cfg.rt(cfg)), "again", False{}))})), note})
    case False{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, KJob.run(inbox, Cfg.home(cfg), M.Setting.get(st, "kicad.path", ""), force, Cfg.rt(cfg)))
        return Hub{st, f, ls, cl, as, inbox, Cfg.with_rt(cfg, K.Rt.set(Cfg.rt(cfg), "busy", J.Flag{True{}})), note}

def Hub.kcheck.before(changed: Bool, busy: Bool, h: Hub, force: Bool) -> IO(Hub):
  match changed:
    case True{}: IO.bind(Hub, Hub, Hub.models.k(h), h2 => Hub.kcheck.k(busy, h2, force))
    case False{}: Hub.kcheck.k(busy, h, force)

def Hub.kcheck(h: Hub, force: Bool) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      +changed = Bool.and(Bool.not(String.eq(M.Setting.get(st, "kicad.path", ""), J.Json.get_str(K.Rt.health(Cfg.rt(cfg)), "setting", ""))), Bool.not(String.eq(K.Rt.status(Cfg.rt(cfg)), "checking")))
      Hub.kcheck.before(changed, J.Json.get_bool(K.Rt.health(Cfg.rt(cfg)), "busy", False{}),
        Hub{st, f, ls, cl, as, inbox, Cfg.with_rt(cfg, K.Rt.set(Cfg.rt(cfg), "status", J.Str{Bool.pick(String, changed, "checking", K.Rt.status(Cfg.rt(cfg)))})), note}, force)

def KJob.again(again: Bool, +inbox: Chan(Msg)) -> IO(Unit):
  match again:
    case True{}: Self.post(inbox, KProbe{True{}})
    case False{}: IO.pure(Unit, Unit{})

def Hub.kfound.k(current: Bool, +again: Bool, h: Hub, +rt: K.Rt) -> IO(Hub):
  match current h:
    case False{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.kcheck(Hub{st, f, ls, cl, as, inbox, Cfg.with_rt(cfg, K.Rt.set(Cfg.rt(cfg), "busy", J.Flag{False{}})), note}, True{})
    case True{} Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        K.Rt.save(Cfg.home(cfg), K.Rt.set(K.Rt.set(rt, "busy", J.Flag{False{}}), "again", J.Flag{False{}}))
        KJob.again(again, inbox)
        Hub.models.k(Hub{st, f, ls, cl, as, inbox, Cfg.with_rt(cfg, K.Rt.set(K.Rt.set(rt, "busy", J.Flag{False{}}), "again", J.Flag{False{}})), note})

# A probe does not broadcast if only the file fingerprint changed.
def Hub.kfound.same(same: Bool, h: Hub, +setting: String, +rt: K.Rt) -> IO(Hub):
  match same h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        K.Rt.save(Cfg.home(cfg), K.Rt.set(K.Rt.set(rt, "busy", J.Flag{False{}}), "again", J.Flag{False{}}))
        return Hub{st, f, ls, cl, as, inbox, Cfg.with_rt(cfg, K.Rt.set(K.Rt.set(rt, "busy", J.Flag{False{}}), "again", J.Flag{False{}})), note}
    case False{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.kfound.k(String.eq(setting, M.Setting.get(st, "kicad.path", "")), J.Json.get_bool(K.Rt.health(Cfg.rt(cfg)), "again", False{}), Hub{st, f, ls, cl, as, inbox, cfg, note}, rt)

# kicad_ipc and kicad_ipc_status
# ------------------------------
# A command for a project's headless KiCad (src/server/kserve.bend). It is
# answered off the hub's thread: the project is found from the path, the
# runtime's health and the kicad.ipc setting say whether IPC may be used
# (core/kserve.bend's KS.gate), commands a headless KiCad cannot answer are
# refused at once, and anything else goes to the project's server. When IPC
# is unavailable the error says why and names the fallback, and the thread
# gets an activity line "KiCad: fell back to files: <why>".

def KIpc.is(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), String.starts_with(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "kicad_ipc"))

def KIpc.args(+o: J.Json) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments")

def KIpc.is_status(+o: J.Json) -> Bool:
  String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "kicad_ipc_status")

def KIpc.id(+o: J.Json) -> String:
  J.Json.as_raw(J.Json.get(o, "id"), "null")

# the command: kicad_ipc_status asks for the runtime's capabilities
def KIpc.type(+o: J.Json) -> String:
  Bool.pick(String, KIpc.is_status(o), "kiapi.backplane.GetCapabilities", J.Json.get_str(KIpc.args(o), "type", ""))

def KIpc.path(+o: J.Json) -> String:
  J.Json.get_str(KIpc.args(o), "path", "")

# the command's fields (a missing message is {})
def KIpc.msg(m: J.Json) -> J.Json:
  match m:
    case J.Obj{fs}:
      J.Obj{fs}
    case _:
      J.Json.obj(Nil{})

def KIpc.reply(+id: String, ok: Bool, body: J.Json) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":" ++ id ++ ",\"result\":" ++ J.Json.show(MC.Mcp.content(ok, body)) ++ "}"

def KIpc.send(+reply: Chan(String), +id: String, ok: Bool, body: J.Json) -> IO(Unit):
  BR.Browser.say(reply, KIpc.reply(id, ok, body))

def KIpc.fail(+reply: Chan(String), +id: String, +text: String) -> IO(Unit):
  KIpc.send(reply, id, False{}, MC.Mcp.fail(text))

def KIpc.log(+inbox: Chan(Msg), +thread: String, +id: String, +why: String) -> IO(Unit):
  do IO<Unit>:
    +now : Nat <- Now.nat()
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{[M.ActivityLogged{thread, "kicad-ipc:" ++ Nat.show(now) ++ ":" ++ id, "info", "kicad", KSC.KS.activity(why), now}]})
    return Unit{}

# IPC is unavailable: the error names the fallback, the thread is told
def KIpc.fallback(+inbox: Chan(Msg), +thread: String, +reply: Chan(String), +id: String, +why: String, +cli: String) -> IO(Unit):
  do IO<Unit>:
    KIpc.log(inbox, thread, id, why)
    KIpc.fail(reply, id, KSC.KS.unavailable(why, cli))

def KIpc.pro.of(fs: K.Files) -> String:
  match fs:
    case K.Files{pro, _, _, _}:
      pro

def KIpc.pro.dir(+dir: String) -> IO(String):
  IO.bind(K.Files, String, K.Files.read(dir), fs => IO.pure(String, KIpc.pro.of(fs)))

def KIpc.pro.k(pro: Bool, absolute: Bool, +path: String) -> IO(String):
  match pro absolute:
    case True{} _:
      IO.pure(String, path)
    case False{} True{}:
      KIpc.pro.dir(K.Files.dir(path))
    case False{} False{}:
      IO.pure(String, "")

# the project of a path: the .kicad_pro itself, or the one beside a board or schematic ("" for none)
def KIpc.pro(+path: String) -> IO(String):
  KIpc.pro.k(String.ends_with(path, ".kicad_pro"), String.starts_with(path, "/"), path)

# the capabilities the hub acts on (idle-exit only from API 2)
def KIpc.caps.of(+h: J.Json) -> List<&2, String>:
  KSC.KS.caps.safe(KH.KH.strings(J.Json.elems(J.Json.get_obj(h, "caps"))), J.Json.get_nat(h, "api", 0n))

def KIpc.caps(rt: K.Rt) -> List<&2, String>:
  match rt:
    case K.Rt{_, _, _, _, +h}:
      KIpc.caps.of(h)

def KIpc.env(rt: K.Rt) -> KSV.KEnv:
  match rt:
    case K.Rt{+cli, _, v, _, +h}:
      KSV.KEnv{cli, K.Rt.root(cli), KIpc.caps.of(h), v}

# why IPC cannot be used right now ("" when it can)
def KIpc.gate(rt: K.Rt, +setting: String) -> String:
  match rt:
    case K.Rt{_, _, +v, _, +h}:
      KSC.KS.gate(J.Json.get_str(h, "status", "checking"), v, J.Json.get_str(h, "why", ""), setting)

# what is wrong with the request itself ("" when nothing)
def KIpc.check(+ty: String, +path: String, +pro: String, +msg: J.Json, caps: List<&2, String>) -> String:
  Bool.pick(String, String.is_empty(ty), "type is required: the protobuf command, e.g. kiapi.board.commands.GetItems",
    Bool.pick(String, Bool.not(String.starts_with(path, "/")), "path must be the absolute path of the project's .kicad_pro, .kicad_pcb or .kicad_sch",
      Bool.pick(String, String.is_empty(pro), "no KiCad project (.kicad_pro) found for " ++ path,
        Bool.pick(String, KSC.KS.headless_refused(ty), KSC.KS.refusal(), KSC.KS.guard(ty, msg, caps)))))

def KIpc.body(+o: J.Json) -> J.Json:
  J.Json.obj([J.KV{"type", J.Str{J.Json.get_str(o, "type", "")}}, J.KV{"message", J.Json.get_obj(o, "message")}])

# KiCad's own error, with its status
def KIpc.err(+o: J.Json) -> String:
  Bool.pick(String, String.is_empty(J.Json.get_str(o, "status", "")), J.Json.get_str(o, "error", "KiCad did not answer"),
    "KiCad answered " ++ J.Json.get_str(o, "status", "") ++ ": " ++ J.Json.get_str(o, "error", ""))

def KIpc.fin.plain(ok: Bool, +o: J.Json, +reply: Chan(String), +id: String) -> IO(Unit):
  match ok:
    case True{}:
      KIpc.send(reply, id, True{}, KIpc.body(o))
    case False{}:
      KIpc.fail(reply, id, KIpc.err(o))

def KIpc.fin.ok(status: Bool, ok: Bool, +o: J.Json, +reply: Chan(String), +id: String, +lines: String, +argv: String, +version: String) -> IO(Unit):
  match status:
    case True{}:
      KIpc.send(reply, id, True{}, J.Json.obj([J.KV{"server", J.Str{KSC.KS.note(lines)}}, J.KV{"servers", J.Str{lines}}, J.KV{"argv", J.Str{argv}}, J.KV{"version", J.Str{version}}, J.KV{"capabilities", J.Json.get_obj(o, "message")},
        J.KV{"detail", J.Str{Bool.pick(String, ok, "", KIpc.err(o))}}]))
    case False{}:
      KIpc.fin.plain(ok, o, reply, id)

def KIpc.fin.k(fallback: Bool, ok: Bool, +o: J.Json, +inbox: Chan(Msg), +thread: String, +reply: Chan(String), +id: String, status: Bool, +lines: String, +argv: String, +version: String) -> IO(Unit):
  match fallback:
    case True{}:
      do IO<Unit>:
        KIpc.log(inbox, thread, id, J.Json.get_str(o, "why", ""))
        KIpc.fail(reply, id, J.Json.get_str(o, "error", ""))
    case False{}:
      KIpc.fin.ok(status, ok, o, reply, id, lines, argv, version)

# the project's server is asked, and the answer goes back
def KIpc.run(+q: Chan(KSV.KMsg), +pro: String, +env: KSV.KEnv, +ty: String, msg: J.Json, status: Bool, +inbox: Chan(Msg), +thread: String, +reply: Chan(String), +id: String) -> IO(Unit):
  do IO<Unit>:
    +line : String <- KSV.KServe.ask_on(q, pro, env, ty, msg)
    +lines : String <- KSV.KServe.info_on(q)
    +argv : String <- KSV.KServe.argv_on(q, pro)
    +o : J.Json = KSV.KServe.parse(line)
    KIpc.fin.k(J.Json.get_bool(o, "fallback", False{}), J.Json.get_bool(o, "ok", False{}), o, inbox, thread, reply, id, status, lines, argv, KSV.KEnv.version(env))

def KIpc.go.run(m: Maybe<&2, Chan(KSV.KMsg)>, +rt: K.Rt, +inbox: Chan(Msg), +thread: String, +o: J.Json, +reply: Chan(String), +pro: String) -> IO(Unit):
  match m:
    case Some{q}:
      KIpc.run(q, pro, KIpc.env(rt), KIpc.type(o), KIpc.msg(J.Json.get_obj(KIpc.args(o), "message")), KIpc.is_status(o), inbox, thread, reply, KIpc.id(o))
    case None{}:
      KIpc.fallback(inbox, thread, reply, KIpc.id(o), "this Backplane has no KiCad IPC helper", K.Rt.cli(rt))

def KIpc.go.gate(free: Bool, +gate: String, m: Maybe<&2, Chan(KSV.KMsg)>, +rt: K.Rt, +inbox: Chan(Msg), +thread: String, +o: J.Json, +reply: Chan(String), +pro: String) -> IO(Unit):
  match free:
    case False{}:
      KIpc.fallback(inbox, thread, reply, KIpc.id(o), gate, K.Rt.cli(rt))
    case True{}:
      KIpc.go.run(m, rt, inbox, thread, o, reply, pro)

def KIpc.go.k(fine: Bool, +plain: String, +gate: String, m: Maybe<&2, Chan(KSV.KMsg)>, +rt: K.Rt, +inbox: Chan(Msg), +thread: String, +o: J.Json, +reply: Chan(String), +pro: String) -> IO(Unit):
  match fine:
    case False{}:
      KIpc.fail(reply, KIpc.id(o), plain)
    case True{}:
      KIpc.go.gate(String.is_empty(gate), gate, m, rt, inbox, thread, o, reply, pro)

def KIpc.go(m: Maybe<&2, Chan(KSV.KMsg)>, +rt: K.Rt, +setting: String, +inbox: Chan(Msg), +thread: String, +o: J.Json, +reply: Chan(String), +pro: String) -> IO(Unit):
  +plain = KIpc.check(KIpc.type(o), KIpc.path(o), pro, KIpc.msg(J.Json.get_obj(KIpc.args(o), "message")), KIpc.caps(rt))
  KIpc.go.k(String.is_empty(plain), plain, KIpc.gate(rt, setting), m, rt, inbox, thread, o, reply, pro)

def KIpc.job(m: Maybe<&2, Chan(KSV.KMsg)>, +rt: K.Rt, +setting: String, +inbox: Chan(Msg), +thread: String, +o: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(String, Unit, KIpc.pro(KIpc.path(o)), pro => KIpc.go(m, rt, setting, inbox, thread, o, reply, pro))

# Retry
def KIpc.clear(m: Maybe<&2, Chan(KSV.KMsg)>) -> IO(Unit):
  match m:
    case Some{q}:
      KSV.KServe.clear_on(q)
    case None{}:
      IO.pure(Unit, Unit{})

def Hub.kicad_ipc(h: Hub, +thread: String, +o: J.Json, +reply: Chan(String)) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, KIpc.job(Cfg.ks(cfg), Cfg.rt(cfg), M.Setting.get(st, "kicad.ipc", "on"), inbox, thread, o, reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# (a check's answer keeps the headless servers' lines the hub holds)
def Hub.kfound(h: Hub, +setting: String, +rt0: K.Rt) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      +rt = K.Rt.set(rt0, "ipc", J.Str{J.Json.get_str(K.Rt.health(Cfg.rt(cfg)), "ipc", "")})
      Hub.kfound.same(Bool.and(String.eq(setting, M.Setting.get(st, "kicad.path", "")), Bool.and(Bool.not(J.Json.get_bool(K.Rt.health(Cfg.rt(cfg)), "again", False{})),
        String.eq(J.Json.show(J.Json.obj(Info.kicad(rt))), J.Json.show(J.Json.obj(Info.kicad(Cfg.rt(cfg))))))), Hub{st, f, ls, cl, as, inbox, cfg, note}, setting, rt)

# the headless KiCad servers' lines changed: every client hears them with the info
def Hub.kiservers(h: Hub, +text: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.models.k(Hub{st, f, ls, cl, as, inbox, Cfg.with_rt(cfg, K.Rt.set(Cfg.rt(cfg), "ipc", J.Str{text})), note})

# Retry: the failed servers are forgotten, and KiCad is checked again
def Hub.kretry(h: Hub) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        KIpc.clear(Cfg.ks(cfg))
        Hub.kcheck(Hub{st, f, ls, cl, as, inbox, cfg, note}, True{})

def Cfg.with_machines(c: Cfg, +mc: String) -> Cfg:
  match c:
    case Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, _, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, tcfg, views}

# a new pairing link, its message made once and kept for every join
def Cfg.with_link(c: Cfg, +lk: String) -> Cfg:
  match c:
    case Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, _, fc, ksx}, +tcfg, +views}:
      Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, BN.Bn.with_pair(bnx, lk ++ "\n" ++ Hub.pair_msg.of(lk)), mgd, lk, fc, ksx}, tcfg, views}

# the tailnet listener is up: every client hears the pairing link. Each
# one either is the owner's own device or came in with the token already,
# so it tells none of them anything new
def Hub.link(h: Hub, +lk: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      Hub.models.k(Hub{st, f, ls, cl, as, inbox, Cfg.with_link(cfg, lk), note})

# where a thread's attachments go (the upload itself runs off the hub)
def Hub.att_dir(h: Hub, +thread: String, +reply: Chan(String)) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, AT.Att.dir(Cfg.home(cfg), thread))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# the owner's other machines changed: every client hears
def Hub.machines(h: Hub, +mc: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      Hub.models.k(Hub{st, f, ls, cl, as, inbox, Cfg.with_machines(cfg, mc), note})

# the providers' models changed: new threads' prompts and every client see
# the new list
def Hub.models(h: Hub, +ms: String, +mt: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      Hub.models.k(Hub{st, f, ls, cl, as, inbox, Cfg.with_models(cfg, ms, mt), note})

def Names.list(r: Result<&1, &1, U32 & String, List<String>>) -> List<&2, String>:
  match r:
    case Done{xs}:
      Names.copy(xs)
    case Fail{_}:
      Nil{}

def Names.of(path: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    r : Result<&1, &1, U32 & String, List<String>> <- H.FS.list(path)
    return List.sort(~String, ~String.is_le, Names.list(r))

def Skills.keep(empty: Bool, s: Q.Skill, rest: List<&2, Q.Skill>) -> List<&2, Q.Skill>:
  match empty:
    case True{}:
      rest
    case False{}:
      s <> rest

# the SKILL.md of every skill dir under root, parsed
def Skills.read(ds: List<&2, String>, +root: String) -> IO(List<&2, Q.Skill>):
  match ds:
    case Nil{}:
      IO.pure(List<&2, Q.Skill>, Nil{})
    case +d <> t:
      do IO<List<&2, Q.Skill>>:
        +path : String = root ++ "/" ++ d ++ "/SKILL.md"
        +text : String <- Files.text(path)
        rest : List<&2, Q.Skill> <- Skills.read(t, root)
        return Skills.keep(String.is_empty(text), Q.Skill.parse(text, d, path), rest)

def Skills.kistack.dir(+dir: String) -> IO(String & List<&2, Q.Skill>):
  do IO<String & List<&2, Q.Skill>>:
    ds : List<&2, String> <- Names.of(dir ++ "/skills")
    ss : List<&2, Q.Skill> <- Skills.read(ds, dir ++ "/skills")
    return (dir, ss)

# the bundled KiStack (K.KiStack.dir), or none ("": no --plugin-dir) when
# this build carries none
def Skills.kistack.k(have: Bool, +dir: String) -> IO(String & List<&2, Q.Skill>):
  match have:
    case True{}:
      Skills.kistack.dir(dir)
    case False{}:
      IO.pure(String & List<&2, Q.Skill>, ("", Nil{}))

def Skills.kistack(+dir: String) -> IO(String & List<&2, Q.Skill>):
  IO.bind(Bool, String & List<&2, Q.Skill>, K.Io.is_file(dir ++ "/.claude-plugin/plugin.json"), have => Skills.kistack.k(have, dir))

def Main.args(xs: List<String>, c: Cfg) -> Cfg:
  match xs c:
    case "--port" <> p <> t Cfg{h, w, ho, _, pr, ds, v, nt, +tcfg, +views}:
      Main.args(t, Cfg{h, w, ho, Maybe.default(&2, U32, U32.read(p), 3787), pr, ds, v, nt, tcfg, views})
    case "--host" <> x <> t Cfg{h, w, _, po, pr, ds, v, nt, +tcfg, +views}:
      Main.args(t, Cfg{h, w, x, po, pr, ds, v, nt, tcfg, views})
    case "--web" <> x <> t Cfg{h, _, ho, po, pr, ds, v, nt, +tcfg, +views}:
      Main.args(t, Cfg{h, x, ho, po, pr, ds, v, nt, tcfg, views})
    case "--home" <> x <> t Cfg{_, w, ho, po, pr, ds, v, nt, +tcfg, +views}:
      Main.args(t, Cfg{x, w, ho, po, pr, ds, v, nt, tcfg, views})
    case "--no-tailscale" <> t Cfg{h, w, ho, po, pr, ds, v, Net{_, _, tok, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      Main.args(t, Cfg{h, w, ho, po, pr, ds, v, Net{False{}, False{}, tok, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, tcfg, views})
    case "--tailscale" <> t Cfg{h, w, ho, po, pr, ds, v, Net{_, https, tok, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      Main.args(t, Cfg{h, w, ho, po, pr, ds, v, Net{True{}, https, tok, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, tcfg, views})
    case "--tailscale-https" <> t Cfg{h, w, ho, po, pr, ds, v, Net{_, _, tok, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      Main.args(t, Cfg{h, w, ho, po, pr, ds, v, Net{True{}, True{}, tok, ms, mt, rt, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, tcfg, views})
    case _ <> t _:
      Main.args(t, c)
    case Nil{} _:
      c

# Model refresh
# -------------
# The providers' model lists are read again every Models.every() ms; a
# list that changed goes to the hub, which tells every client.

def Models.every() -> U32:
  900000

# the last model list found (src/core/boot.bend)
def Models.cache(+home: String) -> String:
  home ++ "/models.cache"

def Net.models(n: Net) -> String:
  match n:
    case Net{_, _, _, ms, _, _, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}:
      ms

# Machines
# --------
# Every Machines.every() ms: the owner's online tailnet peers (tailscale
# status) and any hubs named in BACKPLANE_PEERS, each asked GET /hello;
# those that answer run Backplane. The list goes to every client.

def Machines.every() -> U32:
  120000

# a hub that may be there: a name and its base URL
type Cand is Data:
  Cand{name: String, url: String, named: Bool}

def Machines.cands(ms: List<&2, TN.Machine>, +port: U32) -> List<&2, Cand>:
  match ms:
    case Nil{}:
      Nil{}
    case TN.Machine{n, ip, _} <> t:
      Cand{n, "http://" ++ ip ++ ":" ++ U32.show(port), False{}} <> Machines.cands(t, port)

def Machines.name(+url: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(Bool.pick(String, String.starts_with(url, "http://"), String.drop(url, 7n), url), ':')), url)

def Machines.static.k(blank: Bool, +u: String, rest: List<&2, Cand>) -> List<&2, Cand>:
  match blank:
    case True{}:
      rest
    case False{}:
      Cand{Machines.name(u), u, True{}} <> rest

# hubs named in BACKPLANE_PEERS ("http://host:port,..."), for machines
# that are not on the tailnet
def Machines.static(us: List<&2, String>) -> List<&2, Cand>:
  match us:
    case Nil{}:
      Nil{}
    case +u <> t:
      Machines.static.k(String.is_empty(String.trim(u)), String.trim(u), Machines.static(t))

def Machines.urls(cs: List<&2, Cand>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case Cand{_, u, _} <> t:
      ("'" ++ u ++ "'") <> Machines.urls(t)

# ask every hub at once; each answer is "<url> <byte> <byte> ...": /hello
# answers CBOR, so od writes its bytes out as decimal numbers
def Machines.probe.sh(+urls: List<&2, String>) -> String:
  "for u in " ++ String.join(urls, " ") ++ "; do (r=$(curl -s -m 3 \"$u/hello\" | od -An -tu1 -v | tr -s ' \\n' '  '); [ -n \"${r# }\" ] && echo \"$u $r\") & done; wait"

def Machines.answer(ls: List<&2, String>, +url: String) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      Bool.pick(String, String.starts_with(l, url ++ " "), String.drop(l, Nat.add(String.length(url), 1n)), Machines.answer(t, url))

# (named: one from BACKPLANE_PEERS, the only kind a hub started with
# --no-tailscale pairs with: UC.Mate.due)
def Machines.keep(+v: String, +n: String, +url: String, named: Bool, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, String.is_empty(v), rest,
    J.Json.obj([J.KV{"name", J.Str{n}}, J.KV{"url", J.Str{url}}, J.KV{"version", J.Str{v}}, J.KV{"named", J.Json.bool(named)}]) <> rest)

def Machines.one(+c: Cand, +ls: List<&2, String>, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match c:
    case Cand{+n, +u, named}:
      Machines.keep(TN.Hello.version(TN.Hello.bytes(Machines.answer(ls, u))), n, u, named, rest)

def Machines.list(cs: List<&2, Cand>, +ls: List<&2, String>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case +c <> t:
      Machines.one(c, ls, Machines.list(t, ls))

def Machines.found(cs: List<&2, Cand>, r: P.Ran) -> J.Json:
  J.Json.arr(Machines.list(cs, String.lines(Tailnet.out(r))))

# the hubs that answer /hello
def Machines.peers(+cs: List<&2, Cand>) -> IO(J.Json):
  Bool.pick(IO(J.Json), List.is_empty(&2, Cand, cs), IO.pure(J.Json, J.Json.arr(Nil{})),
    IO.bind(P.Ran, J.Json, P.Proc.run("sh", ["-c", Machines.probe.sh(Machines.urls(cs))], "/"), r => IO.pure(J.Json, Machines.found(cs, r))))

def Machines.me(m: TN.Machine, +port: U32, +domain: String) -> J.Json:
  match m:
    case TN.Machine{n, +ip, _}:
      J.Json.obj([J.KV{"name", J.Str{n}}, J.KV{"url", J.Str{"http://" ++ Bool.pick(String, String.is_empty(ip), "127.0.0.1", ip) ++ ":" ++ U32.show(port)}}, J.KV{"domain", J.Str{domain}}])

# {"self": {"name", "url", "domain"}, "peers": [{"name", "url", "version"}]}
# (self's url is its loopback address when it has no tailnet one, so two
# hubs named in each other's BACKPLANE_PEERS still agree which asks to pair)
# (domain: the tailnet's MagicDNS domain, how clients know a linked
# machine's address is one of the owner's)
def Machines.ask(+status: String, +port: U32, +extra: String) -> IO(String):
  IO.bind(J.Json, String, Machines.peers(List.append(&2, Cand, Machines.cands(TN.Tailnet.online(TN.Tailnet.peers(status)), port), Machines.static(String.split(extra, ',')))),
    ps => IO.pure(String, J.Json.show(J.Json.obj([J.KV{"self", Machines.me(TN.Tailnet.self(status), port, TN.Tailnet.domain(status))}, J.KV{"peers", ps}]))))

def Machines.tell(+inbox: Chan(Msg), +now: String, +last: String) -> IO(String):
  Bool.pick(IO(String), String.eq(now, last), IO.pure(String, last),
    IO.bind(Result<&1, &1, Msg, Unit>, String, Chan.send(Msg, inbox, MachinesFound{now}), ok => IO.pure(String, now)))

def Machines.loop(fuel: Nat, +inbox: Chan(Msg), +port: U32, +extra: String, +last: String) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        st : P.Ran <- P.Proc.run("sh", ["-c", (PT.Port.tmo() ++ "command -v tailscale >/dev/null && tmo 10 tailscale status --json")], "/")
        found : String <- Machines.ask(Tailnet.out(st), port, extra)
        now : String <- Machines.tell(inbox, found, last)
        IO.sleep(Machines.every())
        Machines.loop(f, inbox, port, extra, now)

def Machines.start(+inbox: Chan(Msg), +port: U32) -> IO(Unit):
  do IO<Unit>:
    extra : String <- Env.get("BACKPLANE_PEERS", "")
    IO.spawn(Unit, Machines.loop(Fuel.forever(), inbox, port, extra, ""))

def Main.version() -> String:
  VR.Version.current()

# the strict command line (core/cli.bend), the first thing both mains do:
# go only runs the program; --help, --version and a bad flag do nothing else
def Main.cli(c: CL.Cli, go: Unit -> IO(Unit)) -> IO(Unit):
  match c:
    case CL.CliRun{}:
      go(Unit{})
    case CL.CliHelp{}:
      IO.print(CL.Cli.usage())
    case CL.CliVersion{}:
      IO.print("backplane " ++ Main.version())
    case CL.CliBad{why}:
      IO.die(Unit, 2, "backplane: " ++ why ++ "\n" ++ CL.Cli.usage())

def Main.failed(e: U32 & String, h: Hub, addr: String) -> IO(Unit):
  (_, msg) = e
  do IO<Unit>:
    Hub.close(h)
    IO.die(Unit, 1, "backplane: cannot listen on " ++ addr ++ ": " ++ msg)

def Main.nolog(e: U32 & String) -> IO(Unit):
  (_, msg) = e
  IO.die(Unit, 1, "backplane: cannot open the event log: " ++ msg)

# what the machine offers, found once at start
type Boot is Data:
  Boot{kdir: String, skills: List<&2, Q.Skill>, rt: K.Rt, models: String, mtext: String, freecad: String}

def Models.strs.go(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> Models.strs.go(t)

def Models.strs(xs: List<&2, String>) -> J.Json:
  J.Json.arr(Models.strs.go(xs))

def Models.listed(o: J.Json) -> Bool:
  String.eq(J.Json.get_str(o, "visibility", "list"), "list")

def Models.slugs.k(keep: Bool, +slug: String, rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      slug <> rest
    case False{}:
      rest

def Models.slugs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      Models.slugs.k(Bool.and(Models.listed(o), Bool.not(String.is_empty(J.Json.get_str(o, "slug", "")))), J.Json.get_str(o, "slug", ""), Models.slugs(t))

# Codex's cached model list ({"models": [{"slug", "visibility"}]})
def Models.codex(+text: String) -> List<&2, String>:
  Models.slugs(J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "models")))

def Models.claude() -> List<&2, String>:
  ["fable", "opus", "sonnet", "haiku"]

# Claude Code's own model list (what its /model picker offers): asked with
# an `initialize` control request, answered before any model is called
def Claude.probe.sh() -> String:
  (PT.Port.tmo() ++ "command -v claude >/dev/null || exit 1; printf '%s\\n' '{\"type\":\"control_request\",\"request_id\":\"models\",\"request\":{\"subtype\":\"initialize\"}}' | tmo 30 claude -p --input-format stream-json --output-format stream-json --verbose 2>/dev/null | grep -m1 '\"control_response\"'")

# the models in a probe's answer ([] when it failed or is too old)
def Claude.listed(r: P.Ran) -> List<&2, J.Json>:
  match r:
    case P.Ran{0, out}:
      J.Json.elems(J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(String.trim(out)), J.Null{}), "response"), "response"), "models"))
    case _:
      Nil{}

def Claude.values.k(keep: Bool, +v: String, rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      v <> rest
    case False{}:
      rest

# what a thread may pick ("default" is the picker's own first row)
def Claude.values(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      Claude.values.k(Bool.not(Bool.or(String.is_empty(J.Json.get_str(o, "value", "")), String.eq(J.Json.get_str(o, "value", ""), "default"))), J.Json.get_str(o, "value", ""), Claude.values(t))

# "Opus 5.5 · claude-opus-5-5"; a full model name gets its description instead
def Claude.label(+o: J.Json) -> String:
  J.Json.get_str(o, "displayName", J.Json.get_str(o, "value", "")) ++ " · "
    ++ Bool.pick(String, String.eq(J.Json.get_str(o, "value", ""), J.Json.get_str(o, "resolvedModel", "")), J.Json.get_str(o, "description", ""), J.Json.get_str(o, "resolvedModel", ""))

def Claude.labels(xs: List<&2, J.Json>) -> List<&2, J.KV>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      J.KV{J.Json.get_str(o, "value", ""), J.Str{Claude.label(o)}} <> Claude.labels(t)

# the effort levels each model takes (none for a model without effort)
def Claude.efforts(xs: List<&2, J.Json>) -> List<&2, J.KV>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      J.KV{J.Json.get_str(o, "value", ""), J.Json.get_obj(o, "supportedEffortLevels")} <> Claude.efforts(t)

def Claude.models.k(none: Bool, +xs: List<&2, J.Json>) -> List<&2, String>:
  Bool.pick(List<&2, String>, none, Models.claude(), Claude.values(xs))

def Models.on(has: Bool, +xs: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, has, xs, Nil{})

# the probe's list, or the known aliases when the CLI cannot say
def Claude.models(+has: Bool, +xs: List<&2, J.Json>) -> List<&2, String>:
  Models.on(has, Claude.models.k(List.is_empty(&2, J.Json, xs), xs))

def Models.line(+name: String, +xs: List<&2, String>) -> String:
  Bool.pick(String, List.is_empty(&2, String, xs), "", name ++ " (" ++ String.join(xs, ", ") ++ ")")

def Models.join(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, Bool.pick(String, String.is_empty(b), a, a ++ "; " ++ b))

def Models.text(+cl: List<&2, String>, +cx: List<&2, String>) -> String:
  Models.join(Models.line("claude", cl), Models.line("codex", cx))

def Models.text3(+cl: List<&2, String>, +cx: List<&2, String>, +gk: List<&2, String>) -> String:
  Models.join(Models.text(cl, cx), Models.line("grok", gk))

def Models.text4(+cl: List<&2, String>, +cx: List<&2, String>, +gk: List<&2, String>, +ms: List<&2, String>) -> String:
  Models.join(Models.text3(cl, cx, gk), Models.line("muse", ms))

def Models.text5(+cl: List<&2, String>, +cx: List<&2, String>, +gk: List<&2, String>, +ms: List<&2, String>, +ag: List<&2, String>) -> String:
  Models.join(Models.text4(cl, cx, gk, ms), Models.line("antigravity", ag))

def Models.text6(+cl: List<&2, String>, +cx: List<&2, String>, +gk: List<&2, String>, +ms: List<&2, String>, +ag: List<&2, String>, +oc: List<&2, String>) -> String:
  Models.join(Models.text5(cl, cx, gk, ms, ag), Models.line("opencode", oc))

# what the providers offer: the models JSON clients read, and a line for
# the agents' prompt
type Found is Data:
  Found{models: String, text: String}

# the app's pages are never framed by another page (clickjacking): the
# PDFs the Files tab frames itself (/img) are not pages
def Conn.frames(+path: String) -> String:
  Bool.pick(String, String.starts_with(W.Http.ctype(path), "text/html"), "X-Frame-Options: DENY\r\nContent-Security-Policy: frame-ancestors 'none'\r\n", "")

def Conn.served(s: Socket, +path: String, +gz: Bool, bs: List<&2, U32>) -> IO(Unit):
  match bs:
    case Nil{}:
      Conn.finish(s, W.Http.text("404 Not Found", "not found"))
    case b <> t:
      Conn.finish(s, W.Http.response.c("200 OK", W.Http.ctype(path), b <> t, Conn.cache(path), Conn.frames(path) ++ Bool.pick(String, gz, "Content-Encoding: gzip\r\nVary: Accept-Encoding\r\n", "Vary: Accept-Encoding\r\n")))

# a precompressed .gz beside the file when the browser takes gzip
def Conn.gz(bs: List<&2, U32>, s: Socket, +web: String, +path: String) -> IO(Unit):
  match bs:
    case Nil{}:
      IO.bind(List<&2, U32>, Unit, Files.read(web ++ path), plain => Conn.served(s, path, False{}, plain))
    case b <> t:
      Conn.served(s, path, True{}, b <> t)

def Conn.static.go(gz: Bool, s: Socket, +web: String, +path: String) -> IO(Unit):
  match gz:
    case True{}:
      IO.bind(List<&2, U32>, Unit, Files.read(web ++ path ++ ".gz"), bs => Conn.gz(bs, s, web, path))
    case False{}:
      IO.bind(List<&2, U32>, Unit, Files.read(web ++ path), bs => Conn.served(s, path, False{}, bs))

def Conn.static.k(ok: Bool, s: Socket, +web: String, +path: String, gz: Bool) -> IO(Unit):
  match ok:
    case True{}:
      Conn.static.go(gz, s, web, path)
    case False{}:
      Conn.finish(s, W.Http.text("400 Bad Request", "bad path"))

def Conn.static(s: Socket, +web: String, +path: String, gz: Bool) -> IO(Unit):
  Conn.static.k(Conn.safe(path), s, web, Conn.file(path), gz)

def Conn.pos(xs: List<&2, String>, +x: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      i
    case +h <> t:
      Bool.pick(Nat, String.eq(h, x), i, Conn.pos(t, x, 1n+i))

# Images in threads
# -----------------
# GET /img?path=<an image an agent named>: the file itself, to a client
# the hub already lets in (it could read the file through a terminal as
# well); only image files, at most IM.Img.cap() bytes.

def Img.sent(bs: List<&2, U32>, s: Socket, +path: String) -> IO(Unit):
  match bs:
    case Nil{}:
      Conn.finish(s, W.Http.text("404 Not Found", "not found"))
    case b <> t:
      Conn.finish(s, W.Http.response.c("200 OK", IM.Img.ctype(path), b <> t, "no-cache", ""))

def Img.read(ok: Bool, s: Socket, +path: String) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(List<&2, U32>, Unit, Files.read(path), bs => Img.sent(bs, s, path))
    case False{}:
      Conn.finish(s, W.Http.text("404 Not Found", "not an image, or too large"))

def Img.stat.k(st: U32 & U32 & U32, s: Socket, +path: String) -> IO(Unit):
  (kind, rest) = st
  (size, _) = rest
  Img.read(Bool.and(U32.is_eq(kind, 0), U32.is_le(size, IM.Img.cap())), s, path)

def Img.stat(r: Result<&1, &1, U32 & String, U32 & U32 & U32>, s: Socket, +path: String) -> IO(Unit):
  match r:
    case Done{st}:
      Img.stat.k(st, s, path)
    case Fail{_}:
      Conn.finish(s, W.Http.text("404 Not Found", "not found"))

def Img.found(+path: String, s: Socket) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, U32 & U32 & U32>, Unit, H.FS.stat(path), r => Img.stat(r, s, path))

def Img.serve.k(image: Bool, s: Socket, +path: String) -> IO(Unit):
  match image:
    case True{}:
      IO.bind(String, Unit, Env.get("HOME", ""), home => Img.found(Bool.pick(String, String.starts_with(path, "~/"), home ++ String.drop(path, 1n), path), s))
    case False{}:
      Conn.finish(s, W.Http.text("404 Not Found", "not an image"))

def Img.serve(allowed: Bool, s: Socket, +path: String) -> IO(Unit):
  match allowed:
    case True{}:
      Img.serve.k(IM.Img.viewable(path), s, path)
    case False{}:
      Conn.finish(s, W.Http.text("401 Unauthorized", "this Backplane needs its pairing token"))

# GET /hello says a Backplane hub runs here (and which version), so other
# machines of the owner can find it; it needs no token and tells nothing more
# the hub's last count of threads running or waiting on work (its gauge
# hub.busy, set each minute)
def Hello.busy(ss: List<&2, PF.Stat>) -> Nat:
  match ss:
    case Nil{}:
      0n
    case +x <> t:
      Bool.pick(Nat, String.eq(PF.Stat.name(x), "hub.busy"), PF.Stat.last(x), Hello.busy(t))

def Conn.get(hello: Bool, s: Socket, +web: String, +path: String, gz: Bool) -> IO(Unit):
  match hello:
    case True{}:
      IO.bind(String, Unit, PS.Perf.take(), t => Conn.finish(s, W.Http.response("200 OK", "application/cbor", TN.Hello.answer.c(Main.version(), Hello.busy(PF.Stats.parse(t)), VR.Version.commit()), "")))
    case False{}:
      Conn.static(s, web, path, gz)

# GET /debug/perf (a table), /debug/perf.json and /debug/log (the last
# log lines): what this process measured since it started (docs/perf.md),
# for a client the hub lets in
def Debug.answer(+i: Nat, +r: String) -> List<&2, U32>:
  match i:
    case 2n:
      W.Http.text("200 OK", PF.Stats.table(PF.Stats.parse(r)))
    case 3n:
      W.Http.response("200 OK", "application/json", U.Utf8.encode(J.Json.show(PF.Stats.json(PF.Stats.parse(r)))), "")
    case _:
      W.Http.text("200 OK", r)

def Debug.read(+i: Nat) -> IO(String):
  match i:
    case 4n:
      PS.Perf.logs()
    case _:
      PS.Perf.take()

def Debug.serve(allowed: Bool, +i: Nat, s: Socket) -> IO(Unit):
  match allowed:
    case True{}:
      IO.bind(String, Unit, Debug.read(i), r => Conn.finish(s, Debug.answer(i, r)))
    case False{}:
      Conn.finish(s, W.Http.text("401 Unauthorized", "this Backplane needs its pairing token"))

def Conn.get.k(i: Nat, s: Socket, +web: String, +p: String, gz: Bool, +allowed: Bool) -> IO(Unit):
  match i:
    case 0n:
      Conn.get(True{}, s, web, W.Http.route(p), gz)
    case 1n:
      Img.serve(allowed, s, ATT.Att.from_uri("file://" ++ W.Http.param(p, "path")))
    case 2n:
      Debug.serve(allowed, 2n, s)
    case 3n:
      Debug.serve(allowed, 3n, s)
    case 4n:
      Debug.serve(allowed, 4n, s)
    case _:
      Conn.get(False{}, s, web, W.Http.route(p), gz)

def Conn.view.mode(view: String, +mobile: String, +frontend: String) -> String:
  match view:
    case "1": Bool.pick(String, Bool.and(String.eq(mobile, "1"), Bool.not(String.eq(frontend, "1"))), "view-mobile", Bool.pick(String, Bool.and(String.eq(frontend, "1"), Bool.not(String.eq(mobile, "1"))), "view-web", "view-invalid"))
    case _: PRESENT.Session.mode(mobile, frontend)

def Conn.route.k(ws: Bool, s: Socket, req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match ws req:
    case True{} W.Request{_, +p, +hs}:
      Conn.gate(Conn.allowed(trusted, hs, p, token), CF.Cef.link(W.Http.header_of(hs, "tailscale-user-login"), W.Http.header_of(hs, "origin")), s, W.Http.header_of(hs, "sec-websocket-key"), rest, id, inbox,
        Maybe.default(&2, Nat, Nat.read(W.Http.param(p, "since")), 0n), W.Http.param(p, "origin"), AUTH.Auth.legacy(trusted, hs, p, token), Conn.view.mode(W.Http.param(p, "view"), W.Http.param(p, "mobile"), W.Http.param(p, "frontend")))
    case False{} W.Request{_, +p, +hs}:
      Conn.get.k(Conn.pos(["/hello", "/img", "/debug/perf", "/debug/perf.json", "/debug/log"], W.Http.route(p), 0n), s, web, p, String.contains(W.Http.header_of(hs, "accept-encoding"), "gzip"), Conn.allowed(trusted, hs, p, token))

def LockHttp.wait.next(waiting: Bool, s: Socket, +text: String, +body: String, +inbox: Chan(Msg), k: Maybe<&1, String> -> Socket -> IO(Unit)) -> IO(Unit):
  match waiting:
    case False{}:
      Mcp.answer(Some{text}, s)
    case True{}:
      do IO<Unit>:
        IO.sleep(1000)
        +reply : Chan(String) <- Chan.new(String, 1)
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, LockHttp{"status", body, reply})
        m : Maybe<&1, String> <- Chan.recv(String, reply)
        k(m, s)

# Long polling belongs to the HTTP connection; the hub remains responsive.
def LockHttp.wait.more(m: Maybe<&1, String>, s: Socket, +body: String, +inbox: Chan(Msg), k: Maybe<&1, String> -> Socket -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{+text}:
      LockHttp.wait.next(Bool.and(Bool.not(J.Json.get_bool(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "held", False{})), Bool.not(String.contains(text, "\"error\""))), s, text, body, inbox, k)
    case None{}:
      Mcp.answer(None{}, s)

def LockHttp.wait(fuel: Nat, m: Maybe<&1, String>, s: Socket, +body: String, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Mcp.answer(m, s)
    case 1n+f:
      LockHttp.wait.more(m, s, body, inbox, m2 => s2 => LockHttp.wait(f, m2, s2, body, inbox))

def LockHttp.ask(s: Socket, +body: String, +op: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +reply : Chan(String) <- Chan.new(String, 1)
    sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, LockHttp{op, body, reply})
    m : Maybe<&1, String> <- Chan.recv(String, reply)
    LockHttp.wait(Bool.pick(Nat, Bool.and(String.eq(op, "acquire"), J.Json.get_bool(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "wait", False{})), 25n, 0n), m, s, body, inbox)

def LockHttp.body.k(full: Bool, s: Socket, +buf: List<&2, U32>, +op: String, +inbox: Chan(Msg), k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  match full:
    case True{}:
      LockHttp.ask(s, U.Utf8.decode(buf), op, inbox)
    case False{}:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536), r => Mcp.got(r, buf, k))

def LockHttp.body(fuel: Nat, s: Socket, +buf: List<&2, U32>, +len: Nat, +op: String, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Conn.finish(s, W.Http.text("413 Payload Too Large", "too large"))
    case 1n+f:
      LockHttp.body.k(Nat.is_ge(List.length(&2, U32, buf), len), s, buf, op, inbox, s2 => b2 => LockHttp.body(f, s2, b2, len, op, inbox))

def LockHttp.gate(local: Bool, s: Socket, +op: String, +len: Nat, rest: List<&2, U32>, +inbox: Chan(Msg)) -> IO(Unit):
  match local:
    case True{}:
      LockHttp.body(4n, s, rest, len, op, inbox)
    case False{}:
      Conn.finish(s, W.Http.text("403 Forbidden", "locks require loopback"))

def LockHttp.peer(r: Socket & String, +op: String, +len: Nat, rest: List<&2, U32>, +inbox: Chan(Msg)) -> IO(Unit):
  (s, peer) = r
  LockHttp.gate(String.starts_with(peer, "127."), s, op, len, rest, inbox)

def Conn.route.mcp.raw(mcp: Bool, s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match mcp req:
    case True{} W.Request{_, +p, +hs}:
      +seg = String.drop(W.Http.route(p), 5n)
      Mcp.gate(AUTH.Auth.mcp(trusted, hs, p, token, seg), AUTH.Auth.mcp.legacy(trusted, hs, p, token, seg), s, seg,
        Maybe.default(&2, Nat, Nat.read(W.Http.header_of(hs, "content-length")), 0n), rest, inbox)
    case False{} _:
      Conn.route.k(Conn.is_ws(req), s, req, rest, web, id, inbox, trusted, token)

def Conn.is_lock(req: W.Request) -> Bool:
  match req:
    case W.Request{m, p, _}:
      Bool.and(String.eq(m, "POST"), String.starts_with(W.Http.route(p), "/lock/"))

def Conn.lock.browser(ok: Bool, s: Socket, +op: String, +len: Nat, rest: List<&2, U32>, +inbox: Chan(Msg)) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(Socket & String, Unit, H.Sock.peer(s), r => LockHttp.peer(r, op, len, rest, inbox))
    case False{}:
      Conn.finish(s, W.Http.text("403 Forbidden", "cross-origin request refused"))

def Conn.route.lock(lock: Bool, s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match lock req:
    case True{} W.Request{_, +p, +hs}:
      Conn.lock.browser(Bool.and(AUTH.Auth.browser(hs), AUTH.Auth.trusted(True{}, hs)), s, String.drop(W.Http.route(p), 6n), J.Json.get_nat(J.Json.obj([J.KV{"n", J.Num{W.Http.header_of(hs, "content-length")}}]), "n", 0n), rest, inbox)
    case False{} _:
      Conn.route.mcp.raw(Conn.is_mcp(req), s, req, rest, web, id, inbox, trusted, token)

def Conn.route.mcp(mcp: Bool, s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  Conn.route.lock(Conn.is_lock(req), s, req, rest, web, id, inbox, trusted, token)

def Refresh.news(news: Bool, +ms: String, +mt: String, +inbox: Chan(Msg), +cache: String, +last: String) -> IO(String):
  match news:
    case True{}:
      do IO<String>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, ModelsFound{ms, mt})
        w : String <- Token.save(cache, BT.Cache.of(ms, mt))
        return ms
    case False{}:
      IO.pure(String, last)

# a probe's list goes to clients and the cache when it is news
def Refresh.found(fd: Found, +inbox: Chan(Msg), +cache: String, +last: String) -> IO(String):
  match fd:
    case Found{+ms, +mt}:
      Refresh.news(BT.Models.news(ms, last), ms, mt, inbox, cache, last)

# (Antigravity's "agy": what is installed and its accounts, AGY.Agy.info.json)
def Models.found(+cl: List<&2, String>, +cx: List<&2, String>, +gk: List<&2, String>, +ms: List<&2, String>, +ag: List<&2, String>, +oc: List<&2, String>, agy: J.Json, about: J.Json) -> Found:
  Found{J.Json.show(J.Json.obj([J.KV{"claude", Models.strs(cl)}, J.KV{"codex", Models.strs(cx)}, J.KV{"grok", Models.strs(gk)}, J.KV{"muse", Models.strs(ms)}, J.KV{"antigravity", Models.strs(ag)},
    J.KV{"opencode", Models.strs(oc)}, J.KV{"agy", agy}, J.KV{"about", about}])),
    Models.text6(cl, cx, gk, ms, ag, oc)}

def Models.boot(ks: String & List<&2, Q.Skill>, +rt: K.Rt, fd: Found, +fc: String) -> Boot:
  (kdir, skills) = ks
  match fd:
    case Found{ms, mt}:
      Boot{kdir, skills, rt, ms, mt, fc}

def Models.grok.k(bullet: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match bullet:
    case True{}:
      Maybe.default(&2, String, List.head(&2, String, String.split(String.drop(String.trim(l), 2n), ' ')), "") <> rest
    case False{}:
      rest

def Models.grok.line(+l: String, rest: List<&2, String>) -> List<&2, String>:
  Models.grok.k(Bool.or(String.starts_with(String.trim(l), "* "), String.starts_with(String.trim(l), "- ")), l, rest)

def Models.grok.go(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      Models.grok.line(l, Models.grok.go(t))

# `grok models`: "  * grok-4.6 (default)" / "  - grok-4.5" lines
def Models.grok(r: P.Ran) -> List<&2, String>:
  match r:
    case P.Ran{0, out}:
      Models.grok.go(String.lines(out))
    case _:
      Nil{}

def Main.has(r: P.Ran) -> Bool:
  match r:
    case P.Ran{0, _}:
      True{}
    case _:
      False{}

# KiStack as the build bundled it (K.KiStack.dir), the KiCad CLI,
# and which providers and models are installed
# What each provider is: its CLI's version, and a label per model. This
# table is only for a Claude CLI too old to answer the probe.
def About.claude() -> List<&2, J.KV>:
  [J.KV{"default", J.Str{"Claude Code's own default"}}, J.KV{"fable", J.Str{"Fable 5.1 · claude-fable-5-1"}},
   J.KV{"opus", J.Str{"Opus 5.5 · claude-opus-5-5"}}, J.KV{"sonnet", J.Str{"Sonnet 5 · claude-sonnet-5"}},
   J.KV{"haiku", J.Str{"Haiku 4.5 · claude-haiku-4-5"}}]

def About.codex.go(xs: List<&2, J.Json>) -> List<&2, J.KV>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      J.KV{J.Json.get_str(o, "slug", ""), J.Str{J.Json.get_str(o, "display_name", "") ++ " · " ++ J.Json.get_str(o, "description", "")}} <> About.codex.go(t)

def About.codex(+text: String) -> List<&2, J.KV>:
  J.KV{"default", J.Str{"Codex's configured default"}} <> About.codex.go(J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "models")))

def About.grok.k(dflt: Bool, +slug: String, rest: List<&2, J.KV>, bullet: Bool) -> List<&2, J.KV>:
  match bullet:
    case True{}:
      J.KV{slug, J.Str{Bool.pick(String, dflt, "Grok's default model", "Grok")}} <> rest
    case False{}:
      rest

def About.grok.go(ls: List<&2, String>) -> List<&2, J.KV>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      About.grok.k(String.contains(l, "(default)"), Maybe.default(&2, String, List.head(&2, String, String.split(String.drop(String.trim(l), 2n), ' ')), ""), About.grok.go(t), Bool.or(String.starts_with(String.trim(l), "* "), String.starts_with(String.trim(l), "- ")))

def About.grok(r: P.Ran) -> List<&2, J.KV>:
  match r:
    case P.Ran{0, out}:
      J.KV{"default", J.Str{"Grok's own default"}} <> About.grok.go(String.lines(out))
    case _:
      Nil{}

# `muse serve` asked for model/list (Muse.probe.sh): the catalog in the
# answer to request 2, among the lines it wrote
def Muse.catalog.k(m: Maybe<&2, J.Json>, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case Some{+o}:
      Bool.pick(List<&2, J.Json>, String.eq(J.Json.as_raw(J.Json.get(o, "id"), ""), "2"), J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "result"), "models")), rest)
    case None{}:
      rest

def Muse.catalog.go(ls: List<&2, String>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Muse.catalog.k(J.Json.parse(String.trim(l)), Muse.catalog.go(t))

def Muse.catalog(r: P.Ran) -> List<&2, J.Json>:
  match r:
    case P.Ran{0, out}:
      Muse.catalog.go(String.lines(out))
    case _:
      Nil{}

def Muse.probe.sh() -> String:
  PT.Port.tmo() ++ "command -v muse >/dev/null && printf '%s\\n' "
    ++ "'{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"clientInfo\":{\"name\":\"backplane\",\"version\":\"1\"}}}' "
    ++ "'{\"jsonrpc\":\"2.0\",\"method\":\"initialized\",\"params\":{}}' "
    ++ "'{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"model/list\",\"params\":{}}' | tmo 15 muse serve --no-session-log 2>/dev/null"

def Models.muse(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      J.Json.get_str(o, "modelId", "") <> Models.muse(t)

def About.muse.go(xs: List<&2, J.Json>) -> List<&2, J.KV>:
  match xs:
    case Nil{}:
      Nil{}
    case +o <> t:
      J.KV{J.Json.get_str(o, "modelId", ""), J.Str{J.Json.get_str(o, "displayLabel", J.Json.get_str(o, "modelId", ""))
        ++ Bool.pick(String, J.Json.get_bool(o, "isDefault", False{}), " · Muse's default", "")}} <> About.muse.go(t)

def About.muse(+xs: List<&2, J.Json>) -> List<&2, J.KV>:
  Bool.pick(List<&2, J.KV>, List.is_empty(&2, J.Json, xs), Nil{}, J.KV{"default", J.Str{"Muse's own default"}} <> About.muse.go(xs))

def About.cli(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(out)), ""))
    case _:
      ""

def About.one(+cli: String, labels: List<&2, J.KV>) -> J.Json:
  J.Json.obj([J.KV{"cli", J.Str{cli}}, J.KV{"labels", J.Json.obj(labels)}])

# opencode's models: those of every provider it is signed in to, as
# "provider/model<TAB>name" lines (`opencode models --verbose` through a
# file: piped, its last lines were sometimes lost)
def Oc.models.sh() -> String:
  PT.Port.tmo() ++ "command -v opencode >/dev/null || exit 0; f=$(mktemp); tmo 30 opencode models --verbose > \"$f\" 2>/dev/null; "
    ++ "awk '/^[A-Za-z0-9][^ ]*\\/[^ ]*$/ {slug=$0; next} /^  \"name\": \"/ && slug != \"\" {n=$0; sub(/^  \"name\": \"/, \"\", n); sub(/\",?$/, \"\", n); print slug \"\\t\" n; slug=\"\"}' \"$f\"; rm -f \"$f\""

def Oc.row(+l: String) -> List<&2, String>:
  String.split(l, '\t')

def Models.oc.k(model: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match model:
    case True{}:
      Maybe.default(&2, String, List.head(&2, String, Oc.row(l)), "") <> rest
    case False{}:
      rest

def Models.oc.go(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Models.oc.k(String.contains(l, "/"), l, Models.oc.go(t))

def Models.oc(+out: String) -> List<&2, String>:
  Models.oc.go(String.lines(out))

def About.oc.k(model: Bool, +l: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match model:
    case True{}:
      J.KV{Maybe.default(&2, String, List.head(&2, String, Oc.row(l)), ""), J.Str{String.join(List.drop(&2, String, Oc.row(l), 1n), " ")}} <> rest
    case False{}:
      rest

def About.oc.go(ls: List<&2, String>) -> List<&2, J.KV>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      About.oc.k(String.contains(l, "/"), l, About.oc.go(t))

def About.oc(+out: String) -> List<&2, J.KV>:
  J.KV{"default", J.Str{"opencode's configured default"}} <> About.oc.go(String.lines(out))

def Oc.out(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      out
    case _:
      ""

# the models info with opencode's part read again (the picker asks when it
# opens on opencode: models.refresh); the others' parts as they were
def Oc.patch.j(+j: J.Json, +out: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"claude", J.Json.get_obj(j, "claude")}, J.KV{"codex", J.Json.get_obj(j, "codex")}, J.KV{"grok", J.Json.get_obj(j, "grok")},
    J.KV{"muse", J.Json.get_obj(j, "muse")}, J.KV{"antigravity", J.Json.get_obj(j, "antigravity")}, J.KV{"opencode", Models.strs(Models.oc(out))},
    J.KV{"agy", J.Json.get_obj(j, "agy")}, J.KV{"about", J.Json.obj([J.KV{"claude", J.Json.get_obj(J.Json.get_obj(j, "about"), "claude")},
      J.KV{"codex", J.Json.get_obj(J.Json.get_obj(j, "about"), "codex")}, J.KV{"grok", J.Json.get_obj(J.Json.get_obj(j, "about"), "grok")},
      J.KV{"muse", J.Json.get_obj(J.Json.get_obj(j, "about"), "muse")}, J.KV{"antigravity", J.Json.get_obj(J.Json.get_obj(j, "about"), "antigravity")},
      J.KV{"opencode", About.one(J.Json.get_str(J.Json.get_obj(J.Json.get_obj(j, "about"), "opencode"), "cli", ""), About.oc(out))}])}]))

def Oc.patch(+ms: String, +out: String) -> String:
  Oc.patch.j(Maybe.default(&2, J.Json, J.Json.parse(ms), J.Null{}), out)

def OcJob.run(+inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", Oc.models.sh()], "/")
    sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, OcFound{Oc.out(r)})
    return Unit{}

def Hub.oc.found(h: Hub, +text: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.models(Hub{st, f, ls, cl, as, inbox, cfg, note}, Oc.patch(Net.models(Cfg.net(cfg)), text), Cfg.mtext(cfg))

def Claude.about(+cli: String, +xs: List<&2, J.Json>) -> J.Json:
  Bool.pick(J.Json, List.is_empty(&2, J.Json, xs), About.one(cli, About.claude()),
    J.Json.obj([J.KV{"cli", J.Str{cli}}, J.KV{"labels", J.Json.obj(Claude.labels(xs))}, J.KV{"efforts", J.Json.obj(Claude.efforts(xs))}]))

# which providers are installed and what models they offer (at start, and
# again every Models.every() ms: new models appear without a restart)
def Models.detect(+user: String, +home: String) -> IO(Found):
  do IO<Found>:
    cl : P.Ran <- P.Proc.run("sh", ["-c", "command -v claude"], "/")
    cx : P.Ran <- P.Proc.run("sh", ["-c", "command -v codex"], "/")
    +cache : String <- K.Io.text(user ++ "/.codex/models_cache.json")
    +gk : P.Ran <- P.Proc.run("sh", ["-c", (PT.Port.tmo() ++ "command -v grok >/dev/null && tmo 15 grok models")], "/")
    pc : P.Ran <- P.Proc.run("sh", ["-c", Claude.probe.sh()], "/")
    vc : P.Ran <- P.Proc.run("sh", ["-c", (PT.Port.tmo() ++ "tmo 10 claude --version")], "/")
    vx : P.Ran <- P.Proc.run("sh", ["-c", (PT.Port.tmo() ++ "tmo 10 codex --version")], "/")
    vg : P.Ran <- P.Proc.run("sh", ["-c", (PT.Port.tmo() ++ "tmo 10 grok --version")], "/")
    mu : P.Ran <- P.Proc.run("sh", ["-c", Muse.probe.sh()], "/")
    vm : P.Ran <- P.Proc.run("sh", ["-c", (PT.Port.tmo() ++ "command -v muse >/dev/null && tmo 10 muse --version")], "/")
    oc : P.Ran <- P.Proc.run("sh", ["-c", Oc.models.sh()], "/")
    +ocs : String = Oc.out(oc)
    vo : P.Ran <- P.Proc.run("sh", ["-c", (PT.Port.tmo() ++ "command -v opencode >/dev/null && tmo 10 opencode --version")], "/")
    +xs : List<&2, J.Json> = Claude.listed(pc)
    +mx : List<&2, J.Json> = Muse.catalog(mu)
    +ag : AGY.AgyInfo <- AGX.Agx.probe(AGX.Agx.root(home))
    return Models.found(Claude.models(Main.has(cl), xs), Models.on(Main.has(cx), Models.codex(cache)), Models.grok(gk), Models.muse(mx), AGY.Agy.info.models(ag), Models.oc(ocs), AGY.Agy.info.json(ag),
      J.Json.obj([J.KV{"claude", Claude.about(About.cli(vc), xs)}, J.KV{"codex", About.one(About.cli(vx), About.codex(cache))}, J.KV{"grok", About.one(About.cli(vg), About.grok(gk))},
        J.KV{"muse", About.one(About.cli(vm), About.muse(mx))}, J.KV{"antigravity", AGY.Agy.info.about(ag)},
        J.KV{"opencode", About.one(About.cli(vo), About.oc(ocs))}]))

# The CLIs are looked for on PATH, which a launcher's or a service's hub
# lacks the login shell's part of (nvm, volta...): before each detection the
# login shell's PATH and the usual install directories are merged into ours,
# so every child started afterwards sees them (src/core/clipath.bend).
def Path.out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      out

def Path.set(same: Bool, +next: String) -> IO(Unit):
  match same:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      H.Env.put("PATH", next)

def Path.refresh(+home: String) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", CPA.CliPath.probe.sh()], "/")
    +cur : String <- Env.get("PATH", "")
    +next : String = CPA.CliPath.next(cur, home, Path.out(r))
    Path.set(String.eq(cur, next), next)

# which of the four CLIs are on PATH now (Antigravity's agy lives in
# <home>/antigravity, AGX.Agx.probe, not on PATH), "claude codex " style;
# a stand-in's block (core/standin.bend) is none
def Models.present() -> IO(String):
  do IO<String>:
    r : P.Ran <- P.Proc.run("sh", ["-c", SI.Standin.present.sh()], "/")
    return Path.out(r)

# a full detection when it is due, else nothing; answers the list kept
def Refresh.step(due: Bool, +inbox: Chan(Msg), +user: String, +home: String, +cache: String, +last: String) -> IO(String):
  match due:
    case True{}:
      do IO<String>:
        fd : Found <- Models.detect(user, home)
        Refresh.found(fd, inbox, cache, last)
    case False{}:
      IO.pure(String, last)

# every minute the PATH is merged again and the CLIs present are compared
# with those at the last full detection: a CLI installed or removed shows
# at once; otherwise the full detection (versions, models) runs every
# Models.every() ms, `left` minutes from now. The first runs at once (the
# window is up by then).
def Refresh.loop(fuel: Nat, +inbox: Chan(Msg), +user: String, +home: String, +cache: String, +last: String, +set: String, +left: Nat) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        Path.refresh(user)
        +here : String <- Models.present()
        +due : Bool = Bool.or(Nat.is_eq(left, 0n), Bool.not(String.eq(here, set)))
        now : String <- Refresh.step(due, inbox, user, home, cache, last)
        IO.sleep(60000)
        Refresh.loop(f, inbox, user, home, cache, now, here, Bool.pick(Nat, due, Nat.sub(Nat.div(U32.to_nat(Models.every()), 60000n), 1n), Nat.sub(left, 1n)))

# Boat's status.json (a local file the Boat runner rewrites; no network)
# every 30 s; the hub hears of it when it changed, and every 2 minutes
# anyway, so a runner that stopped writing turns its chips stale (BoatThreads)
def Chips.post(same: Bool, +inbox: Chan(Msg), +text: String) -> IO(Unit):
  match same:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, BoatThreads{text})
        return Unit{}

def Chips.loop(+fuel: Nat, +inbox: Chan(Msg), +path: String, +last: String) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        +text : String <- K.Io.text(path)
        Chips.post(Bool.and(String.eq(text, last), Bool.not(Nat.is_eq(Nat.mod(f, 4n), 0n))), inbox, text)
        IO.sleep(30000)
        Chips.loop(f, inbox, path, text)

def Refresh.start(+inbox: Chan(Msg), +cfg: Cfg) -> IO(Unit):
  do IO<Unit>:
    +user : String <- Env.get("HOME", "")
    xs : String <- Env.get("XDG_STATE_HOME", "")
    IO.spawn(Unit, Chips.loop(Fuel.forever(), inbox, UZ.Chip.path(xs, user), ""))
    IO.spawn(Unit, Refresh.loop(Fuel.forever(), inbox, user, Cfg.home(cfg), Models.cache(Cfg.home(cfg)), Net.models(Cfg.net(cfg)), "?", 0n))

def Main.prepare(+user: String, +exe: String) -> IO(Boot):
  do IO<Boot>:
    ks : String & List<&2, Q.Skill> <- Skills.kistack(K.KiStack.dir(exe))
    +rt : K.Rt = K.Rt.empty()
    +fc : String <- FC.FreeCAD.find()
    return Models.boot(ks, rt, Found{"", ""}, fc)

# the home and port the arguments name, before anything else is found
# (the server binary takes the home's lock with them first)
def Main.where(+user: String, args: List<String>) -> Cfg:
  Main.args(args, Cfg{user ++ "/.backplane-bend", "", "127.0.0.1", 3787, "", Nil{}, "", Net{True{}, False{}, "", "", "", K.Rt.empty(), None{}, None{}, None{}, "", None{}, BN.Bn.empty(), False{}, "", "", None{}}, TI.Tio.empty(False{}), Nil{}})

# the directories Claude threads get (--add-dir, --plugin-dir): the bundled
# KiStack, none when the build carries none
def Main.dirs(+kdir: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(kdir), Nil{}, [kdir])

def Main.config(+user: String, +exe: String, args: List<String>, b: Boot) -> Cfg:
  Boot{kdir, skills, rt, ms, mt, +fc} = b
  Main.args(args, Cfg{user ++ "/.backplane-bend", exe ++ "/web", "127.0.0.1", 3787,
    Q.Prompt.runtime("Backplane") ++ "\n\n" ++ Q.Prompt.mech(fc), Main.dirs(kdir), Main.version(), Net{True{}, False{}, "", ms, mt, rt, None{}, None{}, None{}, "", None{}, BN.Bn.empty(), False{}, "", fc, None{}}, TI.Tio.empty(False{}), Nil{}})

def Sys.exe_dir.io() -> IO(String):
  H.Sys.exe_dir()

# Media jobs (Hub.media): attachments and update checks
# -----------------------------------------------------

def Media.fail(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +msg: String) -> IO(Unit):
  GitJob.reply(inbox, client, id, False{}, J.Json.obj([J.KV{"text", J.Str{msg}}, J.KV{"thread", J.Str{thread}}, J.KV{"upload", J.Flag{True{}}}]))

def Media.attached(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +path: String) -> IO(Unit):
  GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"attached", J.Str{path}}, J.KV{"thread", J.Str{thread}}]))

def Media.wrote.k(res: Result<&1, &1, U32 & String, Unit>) -> String:
  match res:
    case Done{_}:
      ""
    case Fail{_}:
      "cannot write the attachment"

def Media.wrote(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(String):
  (f, res) = r
  do IO<String>:
    File.close(f)
    return Media.wrote.k(res)

def Media.opened(r: Result<&1, &1, U32 & String, File>, bs: List<&2, U32>) -> IO(String):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, String, File.write_bytes(f, bs), Media.wrote)
    case Fail{_}:
      IO.pure(String, "cannot open the attachment file")

# write (mode "w") or append ("a") bytes; "" or what went wrong
def Media.put.file(+path: String, +mode: String, bs: List<&2, U32>) -> IO(String):
  IO.bind(Result<&1, &1, U32 & String, File>, String, File.open(path, mode), r => Media.opened(r, bs))

def Media.size(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> Nat:
  match r:
    case Done{(0, (sz, _))}:
      U32.to_nat(sz)
    case _:
      0n

# a regular file's size (None for anything else)
def Media.stat(r: Result<&1, &1, U32 & String, U32 & U32 & U32>) -> Maybe<&2, Nat>:
  match r:
    case Done{(0, (sz, _))}:
      Some{U32.to_nat(sz)}
    case _:
      None{}

def Media.put.done(+err: String, +last: Bool, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +path: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(err),
    Bool.pick(IO(Unit), last, Media.attached(inbox, client, id, thread, path), GitJob.reply(inbox, client, id, True{}, J.Json.obj(Nil{}))),
    Media.fail(inbox, client, id, thread, err))

def Media.put.k(over: Bool, +first: Bool, +last: Bool, bs: List<&2, U32>, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +path: String) -> IO(Unit):
  match over:
    case True{}:
      do IO<Unit>:
        rm : Result<&1, &1, U32 & String, Unit> <- H.FS.remove(path)
        Media.fail(inbox, client, id, thread, "attachments are limited to " ++ Nat.show(Nat.div(AT.Att.cap(), 1048576n)) ++ " MB")
    case False{}:
      IO.bind(String, Unit, Media.put.file(path, Bool.pick(String, first, "w", "a"), bs), e => Media.put.done(e, last, inbox, client, id, thread, path))

def Media.put.go(+have: Nat, +first: Bool, +last: Bool, +bs: List<&2, U32>, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +path: String) -> IO(Unit):
  Media.put.k(Nat.is_gt(Nat.add(have, List.length(&2, U32, bs)), AT.Att.cap()), first, last, bs, inbox, client, id, thread, path)

def Media.put.parts(ps: List<&2, String>, +b: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +home: String) -> IO(Unit):
  match ps:
    case +i <> +l <> +name <> _:
      do IO<Unit>:
        mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(AT.Att.dir(home, thread))
        st : Result<&1, &1, U32 & String, U32 & U32 & U32> <- H.FS.stat(AT.Att.dir(home, thread) ++ "/" ++ AT.Att.safe(name))
        Media.put.go(Bool.pick(Nat, String.eq(i, "0"), 0n, Media.size(st)), String.eq(i, "0"), String.eq(l, "1"), AT.Att.b64(b),
          inbox, client, id, thread, AT.Att.dir(home, thread) ++ "/" ++ AT.Att.safe(name))
    case _:
      Media.fail(inbox, client, id, thread, "bad upload")

# one piece of an upload (a = "<index>|<last>|<name>", b = base64); the
# first piece starts the file, the rest append; a file never passes the cap
# and never lands outside <home>/attachments/<thread>/
def Media.put(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +a: String, +b: String, +home: String) -> IO(Unit):
  Media.put.parts(String.split(a, '|'), b, inbox, client, id, thread, home)

def Media.copy.k(ok: Bool, fits: Bool, +src: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +dest: String) -> IO(Unit):
  match ok fits:
    case True{} True{}:
      do IO<Unit>:
        bs : List<&2, U32> <- Files.read(src)
        Media.put.go(0n, True{}, True{}, bs, inbox, client, id, thread, dest)
    case True{} False{}:
      Media.fail(inbox, client, id, thread, "attachments are limited to " ++ Nat.show(Nat.div(AT.Att.cap(), 1048576n)) ++ " MB")
    case False{} _:
      Media.fail(inbox, client, id, thread, "not a file: " ++ src)

def Media.copy.m(m: Maybe<&2, Nat>, +src: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +dest: String) -> IO(Unit):
  match m:
    case Some{+n}:
      Media.copy.k(True{}, Nat.is_le(n, AT.Att.cap()), src, inbox, client, id, thread, dest)
    case None{}:
      Media.copy.k(False{}, True{}, src, inbox, client, id, thread, dest)

# a file on this machine (the native window's paste or path), copied in
def Media.file(+inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +src: String, +name: String, +home: String) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(AT.Att.dir(home, thread))
    st : Result<&1, &1, U32 & String, U32 & U32 & U32> <- H.FS.stat(src)
    Media.copy.m(Media.stat(st), src, inbox, client, id, thread, AT.Att.dir(home, thread) ++ "/" ++ AT.Att.safe(name))

def Media.img.k(ok: Bool, +path: String) -> IO(Maybe<&2, AT.Img>):
  match ok:
    case True{}:
      IO.bind(List<&2, U32>, Maybe<&2, AT.Img>, Files.read(path), bs => IO.pure(Maybe<&2, AT.Img>, Some{AT.Img{AT.Att.mime(path), SH.B64.encode(bs)}}))
    case False{}:
      IO.pure(Maybe<&2, AT.Img>, None{})

def Media.img.m(m: Maybe<&2, Nat>, +path: String) -> IO(Maybe<&2, AT.Img>):
  match m:
    case Some{+n}:
      Media.img.k(Bool.and(Nat.is_gt(n, 0n), Nat.is_le(n, U32.to_nat(AT.Att.inline_max()))), path)
    case None{}:
      IO.pure(Maybe<&2, AT.Img>, None{})

# an attached image small enough to send inline
def Media.img(+path: String) -> IO(Maybe<&2, AT.Img>):
  do IO<Maybe<&2, AT.Img>>:
    st : Result<&1, &1, U32 & String, U32 & U32 & U32> <- H.FS.stat(path)
    Media.img.m(Media.stat(st), path)

def Media.imgs.add(m: Maybe<&2, AT.Img>, acc: List<&2, AT.Img>) -> List<&2, AT.Img>:
  match m:
    case Some{i}:
      i <> acc
    case None{}:
      acc

def Media.imgs(ps: List<&2, String>, acc: List<&2, AT.Img>) -> IO(List<&2, AT.Img>):
  match ps:
    case Nil{}:
      IO.pure(List<&2, AT.Img>, List.reverse(&2, AT.Img, acc))
    case +p <> t:
      IO.bind(Maybe<&2, AT.Img>, List<&2, AT.Img>, Media.img(p), m => Media.imgs(t, Media.imgs.add(m, acc)))

# look for a release on a channel now (announced like the hourly check)
def Media.check.go(+channel: String, +inbox: Chan(Msg), +client: U32, +id: String) -> IO(Unit):
  do IO<Unit>:
    r : Maybe<&2, UP.Release> <- UP.Update.latest(channel)
    +tag : String = UP.Update.newer(r)
    Upgrade.found(tag, inbox)
    Bool.pick(IO(Unit), String.is_empty(id), IO.pure(Unit, Unit{}), GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"version", J.Str{tag}}, J.KV{"channel", J.Str{channel}}])))

# (a packaged install looks for nothing: its updates come from the package)
def Media.check(managed: Bool, +channel: String, +inbox: Chan(Msg), +client: U32, +id: String) -> IO(Unit):
  match managed:
    case True{}:
      Bool.pick(IO(Unit), String.is_empty(id), IO.pure(Unit, Unit{}), GitJob.reply(inbox, client, id, True{}, J.Json.obj([J.KV{"version", J.Str{""}}, J.KV{"channel", J.Str{channel}}])))
    case False{}:
      Media.check.go(channel, inbox, client, id)

# a Claude message with images: read them, then hand the stream-json line
# (image blocks, then the text) to the thread's agent
# (uuid: the command's, kept on the line: core/cmds.bend)
def Media.claude(+inbox: Chan(Msg), +thread: String, +cwd: String, +text: String, +uuid: String, +gen: String) -> IO(Unit):
  do IO<Unit>:
    ims : List<&2, AT.Img> <- Media.imgs(AT.Att.images(text), Nil{})
    +ss : List<&2, AG.USkill> <- Skills.scan(cwd)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AgentWrite{thread, B.Claude.tag(AT.Att.claude(AG.Mention.expand("claude", text, ss), ims), uuid), gen})
    return Unit{}

# a Git out: the media jobs here (an upload piece runs on the hub, so the
# pieces of a file land in order), git jobs off the hub
def Media.run.k(i: Nat, +tx: AG.Tx, +job: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String, +managed: Bool, +gen: String) -> IO(Unit):
  match i:
    case 0n:
      Media.put(inbox, client, id, thread, a, b, home)
    case 1n:
      IO.spawn(Unit, Media.file(inbox, client, id, thread, a, b, home))
    case 2n:
      IO.spawn(Unit, Media.claude(inbox, thread, cwd, a, b, gen))
    case 3n:
      IO.spawn(Unit, Media.check(managed, a, inbox, client, id))
    case _:
      IO.spawn(Unit, Jobs.run(tx, job, inbox, client, id, thread, cwd, a, b, home))

def Media.run(+tx: AG.Tx, +job: String, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String, +managed: Bool, +gen: String) -> IO(Unit):
  Media.run.k(C.Codec.pos(["attach.put", "attach.file", "attach.claude", "update.check"], job), tx, job, inbox, client, id, thread, cwd, a, b, home, managed, gen)

# thread_wait: the check (MC.Wait.now) goes through the hub each second,
# off the hub, until the thread is done or the time is up (by the clock: a
# check of a thread elsewhere asks its machine, which takes a while); its
# last answer is the reply
def Wait.next(stop: Bool, text: String, +reply: Chan(String), k: Unit -> IO(Unit)) -> IO(Unit):
  match stop:
    case True{}:
      IO.bind(Result<&1, &1, String, Unit>, Unit, Chan.send(String, reply, text), ok => IO.pure(Unit, Unit{}))
    case False{}:
      IO.bind(Unit, Unit, IO.sleep(1000), k)

def Wait.got(m: Maybe<&1, String>, last: Bool, +reply: Chan(String), k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{+text}:
      Wait.next(Bool.or(MC.Wait.done(text), last), text, reply, k)
    case None{}:
      IO.pure(Unit, Unit{})

def Wait.loop(+fuel: Nat, +end: Nat, +thread: String, +check: String, +reply: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        +r2 : Chan(String) <- Chan.new(String, 1)
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Mcp{thread, check, r2})
        m : Maybe<&1, String> <- Chan.recv(String, r2)
        t : Nat <- Now.nat()
        Wait.got(m, Bool.or(Nat.is_eq(f, 0n), Nat.is_ge(t, end)), reply, _ => Wait.loop(f, end, thread, check, reply, inbox))

# a message's line, its images read: it goes once every message the hub
# took before it has gone (Claude.ready)
def Hub.agent_write(h: Hub, +thread: String, +line: String, +gen: String) -> IO(Hub):
  Claude.ready(h, thread, line, gen)

def Hub.update_check(h: Hub) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Media.check(Net.managed(Cfg.net(cfg)), M.Setting.get(st, "update.channel", "stable"), inbox, 0, ""))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Agent.pid_of(m: Maybe<&2, Agent>) -> Maybe<&2, U32>:
  match m:
    case Some{Agent{_, _, p, _, _}}:
      Some{p}
    case None{}:
      None{}

def Agent.key.of(m: Maybe<&2, Agent>) -> String:
  match m:
    case Some{Agent{_, _, _, k, _}}:
      k
    case None{}:
      ""

# the thread's agent until its process reports: a placeholder of the
# generation being started (pid 0, never signalled: Proc.kill)
def Hub.placeholder(h: Hub, +thread: String, +gen: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      do IO<Hub>:
        Hub.agent_stop(Agents.get(as, thread))
        c : Chan(String) <- Chan.new(String, 1)
        return Hub{st, f, ls, cl, Agent{thread, c, 0, B.Gen.key(gen), Nil{}} <> Agents.drop(as, thread), inbox, cfg, note}

def Hub.turn_pid.k(ok: Bool, h: Hub, +thread: String, +pid: U32, input: Chan(String), +gen: String) -> IO(Hub):
  match ok h:
    case True{} Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      do IO<Hub>:
        Hub.agent_stop(Agents.get(as, thread))
        return Hub{st, f, ls, cl, Agent{thread, input, pid, B.Gen.turn(gen), Nil{}} <> Agents.drop(as, thread), inbox, cfg, note}
    case False{} _:
      do IO<Hub>:
        Chan.close(String, input)
        Proc.kill(pid, 15)
        return h

# a turn process started: it is the thread's only if the thread's agent is
# the placeholder of its generation (B.Gen.accepts); any other (its turn
# stopped, or a later one started) is stopped at once
def Hub.turn_pid(h: Hub, +thread: String, +pid: U32, input: Chan(String), +gen: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.turn_pid.k(B.Gen.accepts(Agent.key.of(Agents.get(as, thread)), gen), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, pid, input, gen)

def Hub.turn_done.k(same: Bool, h: Hub, +thread: String) -> Hub:
  match same h:
    case True{} Hub{st, f, ls, cl, as, inbox, cfg, note}:
      Hub{st, f, ls, cl, Agents.drop(as, thread), inbox, cfg, note}
    case False{} _:
      h

# a turn process exited: forgotten, unless another replaced it already
def Hub.turn_done(h: Hub, +tag: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      IO.pure(Hub, Hub.turn_done.k(B.Tag.heard(B.Tag.pid(tag), Agent.pid_of(Agents.get(as, B.Tag.thread(tag)))), Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Tag.thread(tag)))

# Bots
# ----
# The IO of docs/bots.md around the pure decider (core/bothub.bend):
# effects (B.BotFx), signed requests from webhooks and linked machines,
# Google's tools, a bot's own browser page. Secrets are handled by
# botnet.bend and never reach the log, the event log or a broadcast.

def Cfg.machines(c: Cfg) -> String:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{_, _, _, _, _, _, _, _, _, mc, _, _, _, _, _, _}, +tcfg, +views}:
      mc

def Live.of(+cfg: Cfg) -> List<&2, LV.Buf>:
  BN.Bn.live(Cfg.bn(cfg))

def Cfg.with_bn(c: Cfg, b: BN.Bn) -> Cfg:
  match c:
    case Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, _, mgd, lk, fc, ksx}, +tcfg, +views}:
      Cfg{h, w, ho, p, pr, ds, v, Net{tn, hs, t, ms, mt, rt, brx, pkx, tmx, mc, pvx, b, mgd, lk, fc, ksx}, tcfg, views}

def Cfg.cef(c: Cfg) -> CF.CefSession:
  BN.Bn.cef(Cfg.bn(c))

def Cfg.with_cef(+c: Cfg, cp: CF.CefSession) -> Cfg:
  Cfg.with_bn(c, BN.Bn.with_cef(Cfg.bn(c), cp))

def Cfg.tch(c: Cfg) -> TE.TeachSt:
  BN.Bn.tch(Cfg.bn(c))

def Cfg.with_tch(+c: Cfg, t: TE.TeachSt) -> Cfg:
  Cfg.with_bn(c, BN.Bn.with_tch(Cfg.bn(c), t))

# th: the thread whose stream changed ("*": any of them)
def Cfg.thin.live(+cfg: Cfg, streams: Map<&2, TLIVE.Stream>, +th: String) -> Cfg:
  Cfg.with_thin_cache(cfg, ThinCache{Cfg.thin.lazy(cfg), Cfg.thin.epoch(cfg), Cfg.thin.next(Cfg.thin.revision(cfg)), streams, Cfg.thin.mail.lazy(cfg), Cfg.thin.waiting(cfg), TPACE.Notes.touch(Cfg.thin.notes(cfg), th), Cfg.thin.store(cfg)})

def Live.with(+cfg: Cfg, +lv: List<&2, LV.Buf>) -> Cfg:
  Cfg.thin.live(Cfg.with_bn(cfg, BN.Bn.with_live(Cfg.bn(cfg), lv)), TLIVE.Streams.snapshot(lv, Cfg.thin.next(Cfg.thin.revision(cfg)), Map.new(&2, TLIVE.Stream)), "*")

def Live.with_delta(+cfg: Cfg, +lv: List<&2, LV.Buf>, +thread: String, text: String) -> Cfg:
  Cfg.thin.live(Cfg.with_bn(cfg, BN.Bn.with_live(Cfg.bn(cfg), lv)), TLIVE.Streams.delta(Cfg.thin.streams(cfg), lv, thread, text, Cfg.thin.next(Cfg.thin.revision(cfg))), thread)

# the tool calls open now (core/deploy.bend's Open.*), kept in memory only:
# after a restart there are none, which is right, the old processes are gone
def Tools.of(+cfg: Cfg) -> List<&2, String>:
  BN.Bn.tools(Cfg.bn(cfg))

def Tools.with(+cfg: Cfg, tl: List<&2, String>) -> Cfg:
  Cfg.with_bn(cfg, BN.Bn.with_tools(Cfg.bn(cfg), tl))

def Hub.tools(h: Hub) -> List<&2, String> & Hub:
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      (Tools.of(cfg), Hub{st, f, ls, cl, as, inbox, cfg, note})

# A step is one append. A write that fails is taken back: the log is cut to
# the length it had before (`truncate`, a short process), so none of the
# step stays on disk and no torn line is joined by a later write. When the
# cut fails too, the length and the reason are held (Bn.broken) and every
# commit is refused with a visible error, trying the cut again first, until
# the log is reconciled. Nothing is folded unless it was written whole.
def Hub.cut.ok(r: P.Ran) -> Bool:
  match r:
    case P.Ran{c, _}:
      U32.is_eq(c, 0)

def Hub.cut(+len: String, +cfg: Cfg) -> IO(Bool):
  IO.bind(P.Ran, Bool, P.Proc.run("truncate", ST.Store.trunc.args(len, Store.path(Cfg.home(cfg))), "/"), r => IO.pure(Bool, Hub.cut.ok(r)))

def Hub.held(+hold: String, st: M.State, f: File, ls: SO.Lg, cl: List<&2, Client>,
    as: List<&2, Agent>, inbox: Chan(Msg), +cfg: Cfg, note: String) -> Hub & Wrote:
  (Hub{st, f, ls, cl, as, inbox, Cfg.with_bn(cfg, BN.Bn.with_broken(Cfg.bn(cfg), hold)), note}, Wrote{Nil{}, False{}, ST.Store.held.msg(hold)})

# a write failed: cut back (the log is as it was) or held
def Hub.commit.failed(cut: Bool, +len: String, +err: String, st: M.State, f: File, ls: SO.Lg, cl: List<&2, Client>,
    as: List<&2, Agent>, inbox: Chan(Msg), +cfg: Cfg, note: String) -> IO(Hub & Wrote):
  match cut:
    case True{}:
      IO.pure(Hub & Wrote, (Hub{st, f, ls, cl, as, inbox, cfg, note}, Wrote{Nil{}, False{}, err}))
    case False{}:
      IO.pure(Hub & Wrote, Hub.held(ST.Store.hold(len, err), st, f, ls, cl, as, inbox, cfg, note))

# Pending activity is invalidated only after the privacy settings were stored.
def Cfg.telemetry.privacy_committed(cs: List<&2, M.Change>, cfg: Cfg) -> Cfg:
  match cfg:
    case Cfg{h, w, ho, p, pr, ds, v, nt, tm, views}:
      Cfg{h, w, ho, p, pr, ds, v, nt, TUI.Privacy.committed(cs, tm), views}

def Hub.commit.done(r: File & Result<&1, &1, U32 & String, Unit>, +disk: List<&2, String>, +len: String, +cs: List<&2, M.Change>, st: M.State, ls: SO.Lg, cl: List<&2, Client>,
    as: List<&2, Agent>, +inbox: Chan(Msg), +cfg: Cfg, note: String) -> IO(Hub & Wrote):
  (f, res) = r
  match res:
    case Done{_}:
      do IO<Hub & Wrote>:
        Bot.dir.changed.all(cs, inbox)
        Hub.commit.k(Hub.fold(cs, disk, Folded{st, SO.Cm.new(ls, M.Switch.num(len, 0n)), Nil{}}), cs, f, cl, as, inbox, Cfg.telemetry.privacy_committed(cs, Live.with(cfg, LV.Live.ends(cs, Live.of(cfg)))), note)
    case Fail{e}:
      do IO<Hub & Wrote>:
        cut : Bool <- Hub.cut(len, cfg)
        Hub.commit.failed(cut, len, Hub.err.msg(e), st, f, ls, cl, as, inbox, cfg, note)

def Hub.commit.write(+disk: List<&2, String>, f: File, +len: String, +cs: List<&2, M.Change>, st: M.State, ls: SO.Lg, cl: List<&2, Client>,
    as: List<&2, Agent>, +inbox: Chan(Msg), +cfg: Cfg, note: String) -> IO(Hub & Wrote):
  IO.bind(File & Result<&1, &1, U32 & String, Unit>, Hub & Wrote, File.write(f, ST.Store.text(disk)),
    w => Hub.commit.done(w, disk, len, cs, st, ls, cl, as, inbox, cfg, note))

def Hub.commit.at(r: File & Result<&1, &1, U32 & String, U32>, +cs: List<&2, M.Change>, st: M.State, ls: SO.Lg, cl: List<&2, Client>,
    as: List<&2, Agent>, +inbox: Chan(Msg), +cfg: Cfg, note: String) -> IO(Hub & Wrote):
  (f, res) = r
  match res:
    case Done{len}:
      Hub.commit.write(Hub.lines.disk(cs), f, U32.show(len), cs, st, ls, cl, as, inbox, cfg, note)
    case Fail{e}:
      IO.pure(Hub & Wrote, (Hub{st, f, ls, cl, as, inbox, cfg, note}, Wrote{Nil{}, False{}, Hub.err.msg(e)}))

def Hub.commit.go(+cs: List<&2, M.Change>, st: M.State, f: File, ls: SO.Lg, cl: List<&2, Client>,
    as: List<&2, Agent>, +inbox: Chan(Msg), +cfg: Cfg, note: String) -> IO(Hub & Wrote):
  IO.bind(File & Result<&1, &1, U32 & String, U32>, Hub & Wrote, File.size(f), r => Hub.commit.at(r, cs, st, ls, cl, as, inbox, cfg, note))

# the log was held: cut it back now; commit if that worked, else refuse
def Hub.commit.heal(cut: Bool, +hold: String, +cs: List<&2, M.Change>, st: M.State, f: File, ls: SO.Lg, cl: List<&2, Client>,
    as: List<&2, Agent>, +inbox: Chan(Msg), +cfg: Cfg, note: String) -> IO(Hub & Wrote):
  match cut:
    case True{}:
      Hub.commit.go(cs, st, f, ls, cl, as, inbox, Cfg.with_bn(cfg, BN.Bn.with_broken(Cfg.bn(cfg), "")), note)
    case False{}:
      IO.pure(Hub & Wrote, Hub.held(hold, st, f, ls, cl, as, inbox, cfg, note))

def Hub.commit.gate(ok: Bool, +cs: List<&2, M.Change>, st: M.State, f: File, ls: SO.Lg, cl: List<&2, Client>,
    as: List<&2, Agent>, +inbox: Chan(Msg), +cfg: Cfg, note: String) -> IO(Hub & Wrote):
  match ok:
    case True{}:
      Hub.commit.go(cs, st, f, ls, cl, as, inbox, cfg, note)
    case False{}:
      do IO<Hub & Wrote>:
        +hold : String = BN.Bn.broken(Cfg.bn(cfg))
        cut : Bool <- Hub.cut(ST.Store.hold.len(hold), cfg)
        Hub.commit.heal(cut, hold, cs, st, f, ls, cl, as, inbox, cfg, note)

# persist a step's changes in one write, then fold them in
def Hub.commit.w(+cs: List<&2, M.Change>, h: Hub) -> IO(Hub & Wrote):
  match cs h:
    case Nil{} _:
      IO.pure(Hub & Wrote, (h, Wrote{Nil{}, True{}, ""}))
    case _ Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      Hub.commit.gate(String.is_empty(BN.Bn.broken(Cfg.bn(cfg))), cs, st, f, ls, cl, as, inbox, cfg, note)

def Hub.commit.b(r: Hub & List<&2, String>, moved: Bool) -> IO(Hub & List<&2, String>):
  (h, fr) = r
  IO.bind(Hub, Hub & List<&2, String>, Hub.busy.if(moved, h), h2 => IO.pure(Hub & List<&2, String>, (h2, fr)))

# persist changes, fold them in; answers the hub and the new lines (newest first)
def Hub.commit(+cs: List<&2, M.Change>, h: Hub, fresh: List<&2, String>) -> IO(Hub & List<&2, String>):
  IO.bind(Hub & Wrote, Hub & List<&2, String>, Hub.commit.w(cs, h), r => Hub.commit.b(Hub.commit.r(r, fresh), B.Busy.moved(cs)))

def Cfg.host(c: Cfg) -> String:
  match c:
    case Cfg{_, _, ho, _, _, _, _, _, +tcfg, +views}:
      ho

def Cfg.tailnet(c: Cfg) -> Bool:
  match c:
    case Cfg{_, _, _, _, _, _, _, Net{tn, _, _, _, _, _, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}, +tcfg, +views}:
      tn

def Hub.broadcast(h: Hub, +text: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.all(cl, text)
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

# the heartbeat to every client, view clients included (Wire.beat)
def Hub.beat.all(h: Hub, +text: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.beat(cl, text)
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

# a thread's browser page: a bot's is named by the bot, any other
# thread's by the thread (BO.World.page)
def Bot.page(+st: M.State, +thread: String) -> String:
  BO.World.page(M.State.bots(st), thread)

def Bot.is_bot(m: Maybe<&2, BO.Bot>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

# opening or navigating brings the Browser tab forward in the viewer of
# the thread that asked (a bot shows its page in its own Browser tab)
def Preview.view(+thread: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"view"}}, J.KV{"thread", J.Str{thread}}, J.KV{"kind", J.Str{"browser"}}, J.KV{"path", J.Str{""}}]))

# the person has the page (teaching or taking over): the bot's call is
# refused and the helper never hears of it
def Preview.refuse(+id: String, +reply: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, Preview.answer(id, BR.Browser.fail(TE.Teach.refusal())))
    return Unit{}

def Preview.go(may: Bool, +cfg: Cfg, +page: String, +o: J.Json, +reply: Chan(String)) -> IO(Unit):
  match may:
    case True{}:
      Preview.run(cfg, page, o, reply)
    case False{}:
      Preview.refuse(J.Json.as_raw(J.Json.get(o, "id"), "null"), reply)

# requests: "bots.*" to the bots' decider, the rest to the hub's
def Bot.request.k(bots: Bool, +o: J.Json, +st: M.State, +now: Nat, +client: U32) -> B.Step:
  match bots:
    case True{}:
      BH.Bots.request(st, now, client, o)
    case False{}:
      BH.Bots.hub(st, now, client, o)

def Bot.request(m: Maybe<&2, J.Json>, +st: M.State, +now: Nat, +client: U32, +text: String) -> B.Step:
  match m:
    case Some{+o}:
      Bot.request.k(BH.Bots.is(J.Json.get_str(o, "m", "")), o, st, now, client)
    case None{}:
      B.Hub.request_text(st, now, client, text)

# Memory reaches only the agent: a bot's turn text gets its <memory>
# block here, after the timeline kept the text as it was sent

# a turn's text for its agent (bothub.bend's Bots.turn), with a block
# naming the threads on the owner's other machines it names (xref.bend)
def Far.turn(+cfg: Cfg, +st: M.State, +now: Nat, +thread: String, +text: String) -> String:
  BH.Bots.turn(st, now, thread, text) ++ XR.Xref.block(text, FR.Fh.ins(BN.Bn.far(Cfg.bn(cfg))))

def Bot.line.k(m: Maybe<&2, J.Json>, +cfg: Cfg, +st: M.State, +now: Nat, +thread: String, +line: String) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, String.eq(J.Json.get_str(o, "type", ""), "user"), B.Claude.user.u(Far.turn(cfg, st, now, thread, Mention.user_text(o)), J.Json.get_str(o, "uuid", "")), line)
    case None{}:
      line

# a Claude stream-json line for a bot's thread: a user message gets the memory
def Bot.line(bot: Bool, +cfg: Cfg, +st: M.State, +now: Nat, +thread: String, +line: String) -> String:
  match bot:
    case True{}:
      Bot.line.k(J.Json.parse(line), cfg, st, now, thread, line)
    case False{}:
      line

# a Claude message with images carries its text in the job's `a`
def Bot.attach(claude: Bool, +cfg: Cfg, +st: M.State, +now: Nat, +thread: String, +a: String) -> String:
  match claude:
    case True{}:
      Far.turn(cfg, st, now, thread, a)
    case False{}:
      a

# Effects
# -------

def BotJob.reply(+inbox: Chan(Msg), +client: U32, +id: String, +ok: Bool, kvs: List<&2, J.KV>) -> IO(Unit):
  do IO<Unit>:
    sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Push{client, J.Json.show(J.Json.obj(List.append(&2, J.KV, [J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{ok}}], kvs)))})
    return Unit{}

def BotJob.fail(+inbox: Chan(Msg), +client: U32, +id: String, msg: String) -> IO(Unit):
  BotJob.reply(inbox, client, id, False{}, [J.KV{"error", J.Str{msg}}])

def ZoneJob.set.ok(none: Bool, +rule: String, +name: String, +old: String, +inbox: Chan(Msg), +client: U32, +id: String, +now: Nat) -> IO(Unit):
  match none:
    case True{}:
      BotJob.fail(inbox, client, id, "I don't know a time zone called " ++ name)
    case False{}:
      do IO<Unit>:
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{List.append(&2, M.Change, ZoneJob.log(name, rule, old, now), [M.SettingSet{"person.tz", name, now}])})
        BotJob.reply(inbox, client, id, True{}, [J.KV{"zone", J.Str{name}}])

# zone.set: the person's zone, once the system knows it
def ZoneJob.set.done(+rule: String, +name: String, +old: String, +inbox: Chan(Msg), +client: U32, +id: String, +now: Nat) -> IO(Unit):
  ZoneJob.set.ok(String.is_empty(rule), rule, name, old, inbox, client, id, now)

def ZoneJob.set(+inbox: Chan(Msg), +name: String, +old: String, +client: U32, +id: String) -> IO(Unit):
  do IO<Unit>:
    +now : Nat <- Now.nat()
    rule : String <- ZoneJob.rule(name)
    ZoneJob.set.done(rule, name, old, inbox, client, id, now)

def BotJob.client(+b: String) -> U32:
  Maybe.default(&2, U32, U32.read(b), 0)

def BotJob.id(+c: String) -> String:
  Bool.pick(String, String.is_empty(c), "0", c)

# a bot's home folder (dir <home>/bots/), or a scratch thread's
# (<home>/scratch/, M.Scratch), a git repository (so its turns get
# checkpoints), becomes its thread's folder
def BotJob.home.k(safe: Bool, +inbox: Chan(Msg), +dir: String, +bot: String, +thread: String) -> IO(Unit):
  match safe:
    case True{}:
      do IO<Unit>:
        +path : String = dir ++ bot
        mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(path)
        r : P.Ran <- P.Proc.run("git", ["init", "-q"], path)
        +now : Nat <- Now.nat()
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{[M.WorktreeSet{thread, "", path, now}]})
        return Unit{}
    case False{}:
      Log.info("bots: not a folder name: " ++ bot)

# a scratch thread's folder is made at once, before the effects after it
# (its first turn may start in the same step); git init follows
def BotJob.scratch(safe: Bool, +inbox: Chan(Msg), +dir: String, +thread: String) -> IO(Unit):
  match safe:
    case True{}:
      do IO<Unit>:
        mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir ++ thread)
        IO.spawn(Unit, BotJob.home.k(True{}, inbox, dir, thread, thread))
    case False{}:
      IO.pure(Unit, Unit{})

# an expired scratch thread's folder goes with it
def BotJob.scratch.gone(safe: Bool, +home: String, +thread: String) -> IO(Unit):
  match safe:
    case True{}:
      do IO<Unit>:
        r : P.Ran <- P.Proc.run("rm", ["-rf", "--", home ++ "/scratch/" ++ thread], home)
        return Unit{}
    case False{}:
      IO.pure(Unit, Unit{})

# the backup before a bot is made the uberbot, in
# <home>/backups/uber-<now>-<bot>: its home folder, and the log lines that
# name it or its thread (the log itself is never touched)
def BotJob.uber.sh() -> String:
  "set -e; d=\"$1/backups/uber-$2-$3\"; mkdir -p \"$d\"; if [ -d \"$1/bots/$3\" ]; then cp -a \"$1/bots/$3\" \"$d/home\"; fi; grep -F -e \"$3\" -e \"$4\" \"$1/events.jsonl\" > \"$d/events.jsonl\" || [ $? -eq 1 ]"

def BotJob.uber.ran(r: P.Ran) -> Bool:
  match r:
    case P.Ran{0, _}:
      True{}
    case P.Ran{_, _}:
      False{}

# only a backup that worked logs the promotion
def BotJob.uber.done(ok: Bool, +inbox: Chan(Msg), +client: U32, +id: String, +bot: String, +uid: String, +now: Nat) -> IO(Unit):
  match ok:
    case True{}:
      do IO<Unit>:
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{[M.UberSet{bot, uid, now}]})
        BotJob.reply(inbox, client, id, True{}, [J.KV{"uid", J.Str{uid}}])
    case False{}:
      BotJob.fail(inbox, client, id, "could not back the bot up; nothing changed")

def BotJob.uber(safe: Bool, +inbox: Chan(Msg), +home: String, +bot: String, +thread: String, +client: U32, +id: String) -> IO(Unit):
  match safe:
    case True{}:
      do IO<Unit>:
        +now : Nat <- Now.nat()
        +seed : String <- BN.Bn.random(4n)
        r : P.Ran <- P.Proc.run("sh", ["-c", BotJob.uber.sh(), "sh", home, Nat.show(now), bot, Bool.pick(String, String.is_empty(thread), bot, thread)], home)
        BotJob.uber.done(BotJob.uber.ran(r), inbox, client, id, bot, UB.Uber.uid(now, seed), now)
    case False{}:
      BotJob.fail(inbox, client, id, "not a bot id")

def BotJob.hooked(+secret: String, +inbox: Chan(Msg), +client: U32, +id: String, +hook: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(secret), BotJob.fail(inbox, client, id, "could not keep the webhook's secret"),
    BotJob.reply(inbox, client, id, True{}, [J.KV{"hook", J.Str{hook}}, J.KV{"secret", J.Str{secret}}, J.KV{"path", J.Str{"/hook/" ++ hook}}]))

# a webhook's secret: made and kept here, shown once to the client that asked
def BotJob.hook(+inbox: Chan(Msg), +home: String, +hook: String, +client: U32, +id: String) -> IO(Unit):
  IO.bind(String, Unit, BN.Bn.hook.make(home, hook), s => BotJob.hooked(s, inbox, client, id, hook))

def BotJob.sent(r: BN.Resp, +peer: String, +path: String) -> IO(Unit):
  match r:
    case BN.Resp{+code, _}:
      Bool.pick(IO(Unit), Bool.and(Nat.is_ge(code, 200n), Nat.is_lt(code, 300n)), IO.pure(Unit, Unit{}),
        Log.info("bots: " ++ path ++ " to " ++ peer ++ " failed (" ++ Nat.show(code) ++ ")"))

# a signed POST to a linked machine
def BotJob.send(+home: String, +url: String, +peer: String, +path: String, +body: String) -> IO(Unit):
  do IO<Unit>:
    +now : Nat <- Now.nat()
    r : BN.Resp <- BN.Bn.to_peer(home, url, peer, now, path, True{}, body)
    BotJob.sent(r, peer, path)

def BotJob.invited(ok: Bool, +inbox: Chan(Msg), +client: U32, +id: String, +url: String, +peer: String, +secret: String) -> IO(Unit):
  match ok:
    case True{}:
      BotJob.reply(inbox, client, id, True{}, [J.KV{"invite", J.Str{SG.Invite.make(url, peer, secret)}}, J.KV{"peer", J.Str{peer}}])
    case False{}:
      BotJob.fail(inbox, client, id, "could not keep the invite's secret")

# an invite: a new peer id and secret, kept here; the other machine links
# with it (POST /bots/link) and both keep the secret
def BotJob.invite(+inbox: Chan(Msg), +home: String, +tailnet: Bool, +host: String, +port: U32, +given: String, +client: U32, +id: String) -> IO(Unit):
  do IO<Unit>:
    +here : String <- BN.Bn.url.here(tailnet, host, port)
    +r : String <- BN.Bn.random(2n)
    +peer : String = "p" ++ r
    +secret : String <- BN.Bn.secret.fresh()
    ok : Bool <- BN.Bn.peer.save(home, peer, secret)
    BotJob.invited(ok, inbox, client, id, Bool.pick(String, String.is_empty(String.trim(given)), here, String.trim(given)), peer, secret)

def BotJob.linked.k(ok: Bool, +name: String, +code: Nat, +body: String, +inbox: Chan(Msg), +home: String, +url: String, +peer: String, +client: U32, +id: String, +now: Nat) -> IO(Unit):
  match ok:
    case True{}:
      do IO<Unit>:
        +shown : String = Bool.pick(String, String.is_empty(name), peer, name)
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{[M.PeerSet{peer, shown, url, False{}, now}]})
        BotJob.reply(inbox, client, id, True{}, [J.KV{"peer", J.Str{peer}}, J.KV{"name", J.Str{shown}}])
    case False{}:
      do IO<Unit>:
        BN.Bn.peer.forget(home, peer)
        BotJob.fail(inbox, client, id, "the other machine did not link (" ++ Nat.show(code) ++ ") " ++ String.take(String.trim(body), 200n))

def BotJob.linked(r: BN.Resp, +inbox: Chan(Msg), +home: String, +url: String, +peer: String, +client: U32, +id: String, +now: Nat) -> IO(Unit):
  match r:
    case BN.Resp{+code, +body}:
      BotJob.linked.k(Nat.is_eq(code, 200n), BN.Bn.clean(J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "name", "")), code, body,
        inbox, home, url, peer, client, id, now)

# joining with an invite: keep its secret, tell the inviter where we are
# (signed POST /bots/link), and link once it answers with its name
def BotJob.join(+inbox: Chan(Msg), +home: String, +tailnet: Bool, +host: String, +port: U32, +url: String, +peer: String, +secret: String, +client: U32, +id: String) -> IO(Unit):
  do IO<Unit>:
    ok : Bool <- BN.Bn.peer.save(home, peer, secret)
    +here : String <- BN.Bn.url.here(tailnet, host, port)
    +me : String <- BN.Bn.name()
    +now : Nat <- Now.nat()
    r : BN.Resp <- BN.Bn.signed(home, url, peer, secret, now, "/bots/link", True{}, J.Json.show(J.Json.obj([J.KV{"url", J.Str{here}}, J.KV{"name", J.Str{me}}])))
    BotJob.linked(r, inbox, home, url, peer, client, id, now)

def BotJob.join.go(ok: Bool, linked: Bool, +inbox: Chan(Msg), +cfg: Cfg, +url: String, +peer: String, +secret: String, +client: U32, +id: String) -> IO(Unit):
  match ok linked:
    case True{} False{}:
      IO.spawn(Unit, BotJob.join(inbox, Cfg.home(cfg), Cfg.tailnet(cfg), Cfg.host(cfg), Cfg.port(cfg), url, peer, secret, client, id))
    case True{} True{}:
      BotJob.fail(inbox, client, id, "already linked to that machine")
    case False{} _:
      BotJob.fail(inbox, client, id, "not a usable invite")

def Bot.peer.live.k(p: BO.Peer) -> Bool:
  match p:
    case BO.Peer{_, _, _, _, revoked}:
      Bool.not(revoked)

def Bot.peer.live(+st: M.State, +id: String) -> Bool:
  Bot.peer.live.k(BO.World.peer(M.State.bots(st), id))

def Bot.peer.url.k(p: BO.Peer) -> String:
  match p:
    case BO.Peer{_, _, u, _, revoked}:
      Bool.pick(String, revoked, "", u)

# a live peer's address ("" for none)
def Bot.peer.url(+st: M.State, +id: String) -> String:
  Bot.peer.url.k(BO.World.peer(M.State.bots(st), id))

def Bot.peer.name.k(p: BO.Peer, +id: String) -> String:
  match p:
    case BO.Peer{_, +n, _, _, _}:
      Bool.pick(String, String.is_empty(n), id, n)

# what bots here call a peer ("name@<this>")
def Bot.peer.name(+st: M.State, +id: String) -> String:
  Bot.peer.name.k(BO.World.peer(M.State.bots(st), id), id)

def BotJob.join.k(m: Maybe<&2, SG.Inv>, +st: M.State, +inbox: Chan(Msg), +cfg: Cfg, +client: U32, +id: String) -> IO(Unit):
  match m:
    case Some{SG.Inv{+url, +peer, +secret}}:
      BotJob.join.go(Bool.and(BN.Bn.url.ok(url), BN.Bn.safe(peer)), Bot.peer.live(st, peer), inbox, cfg, url, peer, secret, client, id)
    case None{}:
      BotJob.fail(inbox, client, id, "not an invite")

def BotJob.send.k(+url: String, +cfg: Cfg, +peer: String, +path: String, +body: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(url), Log.info("bots: no linked machine " ++ peer),
    IO.spawn(Unit, BotJob.send(Cfg.home(cfg), url, peer, path, body)))

def BotJob.close(m: Maybe<&2, Chan(BR.BMsg)>, +bot: String) -> IO(Unit):
  match m:
    case Some{q}:
      IO.bind(String, Unit, BR.Browser.ask_on(q, bot, "close", J.Json.obj(Nil{})), r => IO.pure(Unit, Unit{}))
    case None{}:
      IO.pure(Unit, Unit{})

def Bot.redirect(+port: U32) -> String:
  "http://127.0.0.1:" ++ U32.show(port) ++ "/oauth/google"

# "configure" (phones) and "config" are the same op, and so are "paste"
# (the window) and "finish"; "connect" saves the client when one was typed,
# then begins. A phone's "redirect" (its own loopback listener) replaces the
# hub's for begin and connect; finish uses the one its sign-in began with.
def BotJob.google.pos(+op: String) -> Nat:
  Bool.pick(Nat, String.eq(op, "configure"), 0n,
    Bool.pick(Nat, String.eq(op, "paste"), 2n, C.Codec.pos(["config", "begin", "finish", "status", "disconnect", "connect"], op)))

def BotJob.google.then(+text: String, +home: String, +redirect: String, +here: String) -> IO(String):
  Bool.pick(IO(String), String.starts_with(text, "error"), IO.pure(String, text), GS.Google.begin(home, redirect, here))

def BotJob.google.connect(typed: Bool, +home: String, +p: J.Json, +redirect: String, +here: String) -> IO(String):
  match typed:
    case False{}:
      GS.Google.begin(home, redirect, here)
    case True{}:
      IO.bind(String, String, GS.Google.configure(home, J.Json.get_str(p, "clientId", ""), J.Json.get_str(p, "clientSecret", "")),
        t => BotJob.google.then(t, home, redirect, here))

def BotJob.google.op(i: Nat, +home: String, +p: J.Json, +redirect: String, +here: String) -> IO(String):
  match i:
    case 0n:
      GS.Google.configure(home, J.Json.get_str(p, "clientId", ""), J.Json.get_str(p, "clientSecret", ""))
    case 1n:
      GS.Google.begin(home, redirect, here)
    case 2n:
      GS.Google.finish(home, J.Json.get_str(p, "pasted", ""), redirect)
    case 3n:
      GS.Google.status(home)
    case 4n:
      GS.Google.disconnect(home, String.trim(J.Json.get_str(p, "account", "")))
    case 5n:
      BotJob.google.connect(Bool.not(String.is_empty(String.trim(J.Json.get_str(p, "clientId", "")))), home, p, redirect, here)
    case _:
      IO.pure(String, "error: unknown Google request")

# a status answer carries the signed-in addresses too, one a line
def BotJob.google.kvs(status: Bool, +key: String, +text: String, +raw: String) -> List<&2, J.KV>:
  match status:
    case True{}:
      [J.KV{key, J.Str{text}}, J.KV{"googleAccounts", J.Str{GG.Accts.line(raw)}}]
    case False{}:
      [J.KV{key, J.Str{text}}]

def BotJob.google.done(+text: String, +raw: String, +begin: Bool, +inbox: Chan(Msg), +client: U32, +id: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.starts_with(text, "error"), BotJob.fail(inbox, client, id, Bool.pick(String, String.starts_with(text, "error: "), String.drop(text, 7n), text)),
    BotJob.reply(inbox, client, id, True{}, BotJob.google.kvs(String.starts_with(raw, "{"), Bool.pick(String, begin, "googleUrl", "google"), text, raw)))

# where other machines reach this hub, for a sign-in to name (begin and
# connect only: it asks tailscale)
def BotJob.google.here(begin: Bool, +tailnet: Bool, +host: String, +port: U32) -> IO(String):
  match begin:
    case True{}:
      BN.Bn.url.here(tailnet, host, port)
    case False{}:
      IO.pure(String, "")

# Google's setup and sign-in (src/server/google.bend), off the hub
def BotJob.google(+inbox: Chan(Msg), +home: String, +tailnet: Bool, +host: String, +port: U32, +params: String, +client: U32, +id: String) -> IO(Unit):
  do IO<Unit>:
    +p : J.Json = Maybe.default(&2, J.Json, J.Json.parse(params), J.Null{})
    +i : Nat = BotJob.google.pos(J.Json.get_str(p, "op", ""))
    +here : String <- BotJob.google.here(Bool.or(Nat.is_eq(i, 1n), Nat.is_eq(i, 5n)), tailnet, host, port)
    +raw : String <- BotJob.google.op(i, home, p, GG.Google.redirect.pick(J.Json.get_str(p, "redirect", ""), Bot.redirect(port)), here)
    +text : String <- GS.Google.say(raw)
    BotJob.google.done(text, raw, Bool.or(Nat.is_eq(i, 1n), Nat.is_eq(i, 5n)), inbox, client, id)

def Bot.ran(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      out
    case _:
      ""

def Bot.serve(on: Bool) -> IO(P.Ran):
  match on:
    case True{}:
      P.Proc.run("sh", ["-c", (PT.Port.tmo() ++ "command -v tailscale >/dev/null && tmo 5 tailscale serve status --json")], "/")
    case False{}:
      IO.pure(P.Ran, P.Ran{1, ""})

def Bot.hook.base.k(+env: String, +serve: String, +here: String) -> String:
  Bool.pick(String, String.is_empty(env), Bool.pick(String, String.is_empty(serve), here, serve),
    Bool.pick(String, String.ends_with(env, "/"), String.take(env, Nat.sub(String.length(env), 1n)), env))

# where a phone reaches this hub's webhooks: BACKPLANE_HOOK_URL (a
# funnel, a relay), else the HTTPS address `tailscale serve` gives the
# hub's port, else its tailnet address (plain HTTP)
def Bot.hook.base(+tailnet: Bool, +host: String, +port: U32) -> IO(String):
  do IO<String>:
    +env : String <- BN.Bf.env("BACKPLANE_HOOK_URL")
    r : P.Ran <- Bot.serve(tailnet)
    +here : String <- BN.Bn.url.here(tailnet, host, port)
    return Bot.hook.base.k(String.trim(env), TN.Tailnet.serve(Bot.ran(r), U32.show(port)), here)

def BotJob.token.or(none: Bool, +t: String, +home: String, +hook: String) -> IO(String):
  match none:
    case True{}:
      BN.Bn.token.make(home, hook)
    case False{}:
      IO.pure(String, t)

def BotJob.token.got(+t: String, +home: String, +hook: String) -> IO(String):
  BotJob.token.or(String.is_empty(t), t, home, hook)

# 0: the hook's token (made the first time), 1: a new one, 2: none
def BotJob.token.tok(op: Nat, +home: String, +hook: String) -> IO(String):
  match op:
    case 0n:
      IO.bind(String, String, BN.Bn.token.of(home, hook), t => BotJob.token.got(t, home, hook))
    case 1n:
      BN.Bn.token.make(home, hook)
    case _:
      IO.bind(Unit, String, BN.Bn.token.forget(home, hook), u => IO.pure(String, ""))

def BotJob.token.done(bad: Bool, +tok: String, +url: String, +inbox: Chan(Msg), +client: U32, +id: String, +hook: String) -> IO(Unit):
  match bad:
    case True{}:
      BotJob.fail(inbox, client, id, "could not keep the webhook's token")
    case False{}:
      BotJob.reply(inbox, client, id, True{}, [J.KV{"tokenHook", J.Str{hook}}, J.KV{"token", J.Str{tok}}, J.KV{"tokenUrl", J.Str{url ++ "/hook/" ++ hook}}])

# a webhook's bearer token, shown to the client that asked (only clients
# the server lets in reach this), with the URL a phone posts to
def BotJob.token(+op: Nat, +inbox: Chan(Msg), +home: String, +tailnet: Bool, +host: String, +port: U32, +hook: String, +client: U32, +id: String) -> IO(Unit):
  do IO<Unit>:
    +tok : String <- BotJob.token.tok(op, home, hook)
    +url : String <- Bot.hook.base(tailnet, host, port)
    BotJob.token.done(Bool.and(String.is_empty(tok), Nat.is_lt(op, 2n)), tok, url, inbox, client, id, hook)

# Dictation (server/voice.bend): the key, the dictionary, and live sessions
# of this machine's microphone, whose messages go to the client that
# started them. "add" also comes from a bot's voice_dictionary_add (client
# 0: no one hears the answer).

# the key (masked), the dictionary and the microphones, and the API keys'
# names (server/apikeys.bend; never a secret)
def VoiceJob.status(+inbox: Chan(Msg), +home: String, +client: U32, +id: String) -> IO(Unit):
  do IO<Unit>:
    kvs : List<&2, J.KV> <- VO.Vo.status(home)
    +d : String <- VO.Vo.devices()
    +ks : String <- AKS.Ak.names(home)
    BotJob.reply(inbox, client, id, True{}, List.append(&2, J.KV, kvs, [J.KV{"voiceDevices", J.Str{d}}, J.KV{"apiKeys", J.Str{ks}}]))

# "" when done (then the status goes back), else "error: why"
def VoiceJob.done(+err: String, +inbox: Chan(Msg), +home: String, +client: U32, +id: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(err), VoiceJob.status(inbox, home, client, id), BotJob.fail(inbox, client, id, String.drop(err, 7n)))

def VoiceJob.fwd.k(end: Bool, k: Unit -> IO(Unit)) -> IO(Unit):
  match end:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      k(Unit{})

def VoiceJob.fwd(m: Maybe<&1, String>, +inbox: Chan(Msg), +client: U32, k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{+text}:
      do IO<Unit>:
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Push{client, text})
        VoiceJob.fwd.k(String.contains(text, "\"ev\":\"idle\""), k)
    case None{}:
      IO.pure(Unit, Unit{})

# a session's messages to its client, until idle
def VoiceJob.forward(fuel: Nat, +out: Chan(String), +inbox: Chan(Msg), +client: U32) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, out), m => VoiceJob.fwd(m, inbox, client, u => VoiceJob.forward(f, out, inbox, client)))

def VoiceJob.live(+inbox: Chan(Msg), +home: String, +client: U32, +id: String, +thread: String, +device: String) -> IO(Unit):
  do IO<Unit>:
    BotJob.reply(inbox, client, id, True{}, Nil{})
    +out : Chan(String) <- Chan.new(String, 256)
    IO.spawn(Unit, VO.Vo.live(home, thread, device, out))
    VoiceJob.forward(Fuel.forever(), out, inbox, client)

def VoiceJob.stopped(on: Bool, +inbox: Chan(Msg), +client: U32, +id: String, +thread: String) -> IO(Unit):
  match on:
    case True{}:
      BotJob.reply(inbox, client, id, True{}, Nil{})
    case False{}:
      do IO<Unit>:
        BotJob.reply(inbox, client, id, True{}, Nil{})
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Push{client, VO.Vo.msg(thread, "idle", Nil{})})
        return Unit{}

# Antigravity (core/agy.bend, server/agyio.bend)
# ----------------------------------------------
# Settings' "agy" requests (through bots.voice, op "agy", its "do"):
# install the pinned agent, sign in (or check a key) with the method the
# client names, take Google's way back pasted from another device, cancel,
# sign out or forget an account. What happens goes to the client that
# asked as info "agy.job" (JSON: what, state, msg, url); what is installed
# and signed in goes to every client in the models info, read again after
# each.

def AgyJob.info(+client: U32, +inbox: Chan(Msg), +what: String, +state: String, +msg: String, +url: String) -> IO(Unit):
  do IO<Unit>:
    sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Push{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj([J.KV{"agy.job",
      J.Str{J.Json.show(J.Json.obj([J.KV{"what", J.Str{what}}, J.KV{"state", J.Str{state}}, J.KV{"msg", J.Str{msg}}, J.KV{"url", J.Str{url}}]))}}])}]))})
    return Unit{}

def AgyJob.found(fd: Found, +inbox: Chan(Msg)) -> IO(Unit):
  match fd:
    case Found{ms, mt}:
      do IO<Unit>:
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, ModelsFound{ms, mt})
        return Unit{}

# the models info read again, for every client
def AgyJob.refresh(+inbox: Chan(Msg), +home: String) -> IO(Unit):
  do IO<Unit>:
    user : String <- Env.get("HOME", "")
    fd : Found <- Models.detect(user, home)
    AgyJob.found(fd, inbox)

def AgyJob.installed(+err: String, +client: U32, +inbox: Chan(Msg), +home: String) -> IO(Unit):
  do IO<Unit>:
    AgyJob.info(client, inbox, "install", Bool.pick(String, String.is_empty(err), "done", "failed"),
      Bool.pick(String, String.is_empty(err), "Antigravity " ++ AGY.Agy.version() ++ " is installed.", err), "")
    AgyJob.refresh(inbox, home)

def AgyJob.install(+client: U32, +id: String, +inbox: Chan(Msg), +home: String) -> IO(Unit):
  do IO<Unit>:
    BotJob.reply(inbox, client, id, True{}, Nil{})
    AgyJob.info(client, inbox, "install", "running", "Downloading Google's Antigravity agent " ++ AGY.Agy.version() ++ " (a few hundred MB) and checking it against Backplane's pinned SHA-256.", "")
    +err : String <- AGX.Agx.install(AGX.Agx.root(home))
    AgyJob.installed(err, client, inbox, home)

# The sign-in's process: its lines through AgySign.line, what it says to
# the client; it ends once signed in or failed, or when its output closes

def AgyJob.said.k(i: Nat, +rest: String, +browser: Bool, +client: U32, +inbox: Chan(Msg), +home: String) -> IO(Unit):
  match i:
    case 0n:
      do IO<Unit>:
        ok : Bool <- BN.Bf.secret(AGX.Agx.root(home) ++ "/signin.url", rest)
        AgyJob.info(client, inbox, "signin", "waiting", "Open Google's sign-in and approve it. On another device the last page will not load: copy its address and paste it here.", rest)
    case 1n:
      do IO<Unit>:
        ok : Bool <- AGX.Agx.models.save(AGX.Agx.root(home), rest)
        AgyJob.info(client, inbox, "signin", "done", Bool.pick(String, browser, "Signed in with Google.", "Connected to Antigravity."), "")
        AgyJob.refresh(inbox, home)
    case _:
      AgyJob.info(client, inbox, "signin", "failed", rest, "")

def AgyJob.said(+say: String, +browser: Bool, +client: U32, +inbox: Chan(Msg), +home: String) -> IO(Unit):
  AgyJob.said.k(C.Codec.pos(["url", "ok", "fail"], AGY.Agy.base(String.split(say, '\t'))), String.drop(say, 1n+String.length(AGY.Agy.base(String.split(say, '\t')))), browser, client, inbox, home)

def AgyJob.says(ys: List<&2, String>, +browser: Bool, +client: U32, +inbox: Chan(Msg), +home: String) -> IO(Unit):
  match ys:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +y <> t:
      IO.bind(Unit, Unit, AgyJob.said(y, browser, client, inbox, home), u => AgyJob.says(t, browser, client, inbox, home))

def AgyJob.sends(xs: List<&2, String>, +input: Chan(String)) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +x <> t:
      do IO<Unit>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, input, x)
        AgyJob.sends(t, input)

type AgyCtx is Data:
  AgyCtx{method: String, client: U32, home: String}

def AgyCtx.method(c: AgyCtx) -> String:
  match c:
    case AgyCtx{m, _, _}:
      m

def AgyCtx.client(c: AgyCtx) -> U32:
  match c:
    case AgyCtx{_, cl, _}:
      cl

def AgyCtx.home(c: AgyCtx) -> String:
  match c:
    case AgyCtx{_, _, h}:
      h

def AgyJob.out(o: AGY.AgySignOut, +cx: AgyCtx, +input: Chan(String), +inbox: Chan(Msg)) -> IO(AGY.AgySign):
  match o:
    case AGY.AgySignOut{sg, xs, ys}:
      do IO<AGY.AgySign>:
        AgyJob.sends(xs, input)
        AgyJob.says(ys, AGY.Agy.browser(AgyCtx.method(cx)), AgyCtx.client(cx), inbox, AgyCtx.home(cx))
        return sg

def AgyJob.lines(ls: List<&2, String>, sg: AGY.AgySign, +cx: AgyCtx, +input: Chan(String), +inbox: Chan(Msg)) -> IO(AGY.AgySign):
  match ls:
    case Nil{}:
      IO.pure(AGY.AgySign, sg)
    case l <> rest:
      IO.bind(AGY.AgySign, AGY.AgySign, AgyJob.out(AGY.AgySign.line(sg, AgyCtx.method(cx), AgyCtx.home(cx), l), cx, input, inbox), s2 => AgyJob.lines(rest, s2, cx, input, inbox))

# the process is done with: stopped, reaped, its pid file gone; a sign-in
# that never finished says so
def AgyJob.end(s: Socket, +sg: AGY.AgySign, +pid: U32, +cx: AgyCtx, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Proc.kill(pid, 15)
    Socket.close(s)
    code : U32 <- H.Proc.wait(pid)
    BN.Bf.rm(AGX.Agx.pidfile(AGX.Agx.root(AgyCtx.home(cx))))
    BN.Bf.rm(AGX.Agx.root(AgyCtx.home(cx)) ++ "/signin.url")
    Bool.pick(IO(Unit), AGY.AgySign.done(sg), IO.pure(Unit, Unit{}),
      AgyJob.info(AgyCtx.client(cx), inbox, "signin", "failed", "The sign-in stopped before it finished.", ""))

def AgyJob.next(done: Bool, s: Socket, carry: U.Lines, sg: AGY.AgySign, +pid: U32, +cx: AgyCtx, +inbox: Chan(Msg), k: Socket -> U.Lines -> AGY.AgySign -> IO(Unit)) -> IO(Unit):
  match done:
    case True{}:
      AgyJob.end(s, sg, pid, cx, inbox)
    case False{}:
      k(s, carry, sg)

def AgyJob.after(+sg: AGY.AgySign, s: Socket, carry: U.Lines, +pid: U32, +cx: AgyCtx, +inbox: Chan(Msg), k: Socket -> U.Lines -> AGY.AgySign -> IO(Unit)) -> IO(Unit):
  AgyJob.next(AGY.AgySign.done(sg), s, carry, sg, pid, cx, inbox, k)

def AgyJob.fed(r: U.Lines & List<&2, String>, s: Socket, sg: AGY.AgySign, +pid: U32, +cx: AgyCtx, +input: Chan(String), +inbox: Chan(Msg),
    k: Socket -> U.Lines -> AGY.AgySign -> IO(Unit)) -> IO(Unit):
  (carry, ls) = r
  IO.bind(AGY.AgySign, Unit, AgyJob.lines(ls, sg, cx, input, inbox), s2 => AgyJob.after(s2, s, carry, pid, cx, inbox, k))

def AgyJob.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, carry: U.Lines, sg: AGY.AgySign, +pid: U32, +cx: AgyCtx, +input: Chan(String), +inbox: Chan(Msg),
    k: Socket -> U.Lines -> AGY.AgySign -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      AgyJob.end(s, sg, pid, cx, inbox)
    case Done{bs}:
      AgyJob.fed(U.Lines.feed(carry, bs), s, sg, pid, cx, input, inbox, k)
    case Fail{_}:
      AgyJob.end(s, sg, pid, cx, inbox)

def AgyJob.reader(fuel: Nat, s: Socket, carry: U.Lines, sg: AGY.AgySign, +pid: U32, +cx: AgyCtx, +input: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      AgyJob.end(s, sg, pid, cx, inbox)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => AgyJob.got(r, carry, sg, pid, cx, input, inbox, s2 => c2 => g2 => AgyJob.reader(f, s2, c2, g2, pid, cx, input, inbox)))

def AgyJob.duped(r: Socket & Result<&1, &1, U32 & String, Socket>, +pid: U32, +cx: AgyCtx, +inbox: Chan(Msg)) -> IO(Unit):
  (rd, res) = r
  match res:
    case Done{wr}:
      do IO<Unit>:
        +input : Chan(String) <- Chan.new(String, 16)
        IO.spawn(Unit, Agent.writer(Fuel.forever(), wr, input))
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, input, AGY.Agy.init())
        AgyJob.reader(Fuel.forever(), rd, U.Lines.new(), AGY.AgySign.new(), pid, cx, input, inbox)
    case Fail{_}:
      AgyJob.end(rd, AGY.AgySign.new(), pid, cx, inbox)

def AgyJob.spawned.ok(started: U32 & Socket, +cx: AgyCtx, +inbox: Chan(Msg)) -> IO(Unit):
  (+pid, s) = started
  do IO<Unit>:
    ok : Bool <- BN.Bf.secret(AGX.Agx.pidfile(AGX.Agx.root(AgyCtx.home(cx))), U32.show(pid))
    IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Unit, H.Sock.dup(s), d => AgyJob.duped(d, pid, cx, inbox))

def AgyJob.spawn.failed(e: U32 & String, +cx: AgyCtx, +inbox: Chan(Msg)) -> IO(Unit):
  (_, msg) = e
  AgyJob.info(AgyCtx.client(cx), inbox, "signin", "failed", "Antigravity could not start: " ++ msg, "")

def AgyJob.spawned(r: Result<&1, &1, U32 & String, U32 & Socket>, +cx: AgyCtx, +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case Done{started}:
      AgyJob.spawned.ok(started, cx, inbox)
    case Fail{e}:
      AgyJob.spawn.failed(e, cx, inbox)

# the sign-in in progress, stopped (server/agyio.bend's Agx.cancel)
def AgyJob.cancel(+home: String) -> IO(Unit):
  AGX.Agx.cancel(AGX.Agx.root(home))

# a sign-in (or a key's check) with what the client names: one at a time
# (an earlier one is stopped first), five minutes at most
def AgyJob.signin(+p: J.Json, +client: U32, +id: String, +inbox: Chan(Msg), +home: String) -> IO(Unit):
  do IO<Unit>:
    BotJob.reply(inbox, client, id, True{}, Nil{})
    AgyJob.cancel(home)
    w : Bool <- AGX.Agx.write(AGX.Agx.root(home))
    +m : String = AGY.Agy.method(J.Json.get_str(p, "method", ""))
    AgyJob.info(client, inbox, "signin", "running", Bool.pick(String, AGY.Agy.browser(m), "Starting Google's sign-in…", "Checking Antigravity with the " ++ AGY.Agy.label(m) ++ "…"), "")
    IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, Unit,
      H.Proc.spawn("sh", PT.Port.timeout("300" <> "sh" <> AGY.Agy.args(AGX.Agx.root(home), AKS.Ak.path(home), m, J.Json.get_str(p, "account", ""), J.Json.get_str(p, "project", ""), J.Json.get_str(p, "location", ""))), AGX.Agx.root(home), 0),
      r => AgyJob.spawned(r, AgyCtx{m, client, home}, inbox))

def AgyJob.back.sent(+err: String, +client: U32, +id: String, +inbox: Chan(Msg)) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(err), BotJob.reply(inbox, client, id, True{}, Nil{}), BotJob.fail(inbox, client, id, err))

# the sign-in in progress, stopped
# Google's way back, pasted: taken only for the sign-in that asked, then
# fetched here
def AgyJob.back.k(+pending: String, +back: String, +client: U32, +id: String, +inbox: Chan(Msg)) -> IO(Unit):
  Bool.pick(IO(Unit), AGY.Agy.callback.ok(pending, back),
    IO.bind(String, Unit, AGX.Agx.forward(String.trim(back)), e => AgyJob.back.sent(e, client, id, inbox)),
    BotJob.fail(inbox, client, id, "That address is not this sign-in's way back from Google: copy the whole address of the page Google sent you to."))

def AgyJob.back(+back: String, +client: U32, +id: String, +inbox: Chan(Msg), +home: String) -> IO(Unit):
  IO.bind(String, Unit, BN.Bf.text(AGX.Agx.root(home) ++ "/signin.url"), u => AgyJob.back.k(String.trim(u), back, client, id, inbox))

def AgyJob.out.done(+client: U32, +id: String, +inbox: Chan(Msg), +home: String, +msg: String) -> IO(Unit):
  do IO<Unit>:
    BotJob.reply(inbox, client, id, True{}, Nil{})
    AgyJob.info(client, inbox, "signout", "done", msg, "")
    AgyJob.refresh(inbox, home)

def AgyJob.op(i: Nat, +p: J.Json, +client: U32, +id: String, +inbox: Chan(Msg), +home: String) -> IO(Unit):
  match i:
    case 0n:
      do IO<Unit>:
        BotJob.reply(inbox, client, id, True{}, Nil{})
        AgyJob.refresh(inbox, home)
    case 1n:
      AgyJob.install(client, id, inbox, home)
    case 2n:
      AgyJob.signin(p, client, id, inbox, home)
    case 3n:
      AgyJob.back(J.Json.get_str(p, "url", ""), client, id, inbox, home)
    case 4n:
      do IO<Unit>:
        AgyJob.cancel(home)
        BotJob.reply(inbox, client, id, True{}, Nil{})
    case 5n:
      IO.bind(Unit, Unit, AGX.Agx.signout(AGX.Agx.root(home), J.Json.get_str(p, "account", ""), False{}),
        u => AgyJob.out.done(client, id, inbox, home, "Signed out of " ++ AGY.Agy.acct(J.Json.get_str(p, "account", "")) ++ "."))
    case 6n:
      IO.bind(Unit, Unit, AGX.Agx.signout(AGX.Agx.root(home), J.Json.get_str(p, "account", ""), True{}),
        u => AgyJob.out.done(client, id, inbox, home, "Removed " ++ AGY.Agy.acct(J.Json.get_str(p, "account", "")) ++ "."))
    case _:
      BotJob.fail(inbox, client, id, "unknown antigravity request")

def AgyJob.go(+p: J.Json, +inbox: Chan(Msg), +home: String, +client: U32, +id: String) -> IO(Unit):
  AgyJob.op(C.Codec.pos(["status", "install", "signin", "back", "cancel", "signout", "forget"], J.Json.get_str(p, "do", "")), p, client, id, inbox, home)

# Settings > Usage, Boat side (core/usage.bend's Bj): Boat's CLI read
# (read-only, `boat limits --json --no-update`, 20 s at most) only when it
# is due, its answer kept in <home>/boat-usage.json with the last good one
# on a failure; the Boat lead's job status read from its state folder; both
# go to the client that asked as info "boat.usage". Never `boat dashboard`
# (its link carries a sign-in token), login or anything that changes the
# account.
def BoatUse.read(due: Bool, +home: String, +old: String, +now: Nat) -> IO(String):
  match due:
    case True{}:
      do IO<String>:
        r : P.Ran <- P.Proc.run("sh", ["-c", PT.Port.tmo() ++ "tmo 20 boat limits --json --no-update 2>/dev/null"], "/")
        +c : String = UZ.Bj.cache.next(old, Path.out(r), now)
        w : String <- Token.save(home ++ "/boat-usage.json", c)
        return c
    case False{}:
      IO.pure(String, old)

def BoatUse.state(+xs: String, +user: String) -> String:
  Bool.pick(String, String.is_empty(xs), user ++ "/.local/state", xs) ++ "/backplane/boat/status.json"

def BoatUse.go(+p: J.Json, +inbox: Chan(Msg), +home: String, +client: U32, +id: String) -> IO(Unit):
  do IO<Unit>:
    BotJob.reply(inbox, client, id, True{}, Nil{})
    +now : Nat <- Now.nat()
    +old : String <- K.Io.text(home ++ "/boat-usage.json")
    +cache : String <- BoatUse.read(UZ.Bj.due(old, String.eq(J.Json.get_str(p, "do", ""), "refresh"), now), home, old, now)
    user : String <- Env.get("HOME", "")
    xs : String <- Env.get("XDG_STATE_HOME", "")
    +status : String <- K.Io.text(BoatUse.state(xs, user))
    sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Push{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj([J.KV{"boat.usage", J.Str{UZ.Bj.info(cache, status)}}])}]))})
    return Unit{}

def VoiceJob.op(i: Nat, +inbox: Chan(Msg), +home: String, +p: J.Json, +client: U32, +id: String) -> IO(Unit):
  match i:
    case 0n:
      VoiceJob.status(inbox, home, client, id)
    case 1n:
      IO.bind(String, Unit, VO.Vo.set_key(home, J.Json.get_str(p, "key", "")), e => VoiceJob.done(e, inbox, home, client, id))
    case 2n:
      IO.bind(String, Unit, VO.Vo.unkey(home), e => VoiceJob.done(e, inbox, home, client, id))
    case 3n:
      IO.bind(String, Unit, VO.Vo.set_dict(home, J.Json.get_str(p, "dict", "")), e => VoiceJob.done(e, inbox, home, client, id))
    case 4n:
      IO.bind(String, Unit, VO.Vo.add(home, J.Json.get_str(p, "term", "")), e => VoiceJob.done(e, inbox, home, client, id))
    case 5n:
      VoiceJob.live(inbox, home, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "device", ""))
    case 6n:
      IO.bind(Bool, Unit, VO.Vo.stop(home), on => VoiceJob.stopped(on, inbox, client, id, J.Json.get_str(p, "thread", "")))
    case 7n:
      IO.bind(String, Unit, VO.Vo.devices(), d => BotJob.reply(inbox, client, id, True{}, [J.KV{"voiceDevices", J.Str{d}}]))
    # Settings' API keys: keep one, forget one (the status, names only, goes back)
    case 8n:
      IO.bind(String, Unit, AKS.Ak.add(home, J.Json.get_str(p, "name", ""), J.Json.get_str(p, "value", "")), e => VoiceJob.done(e, inbox, home, client, id))
    case 9n:
      IO.bind(String, Unit, AKS.Ak.del(home, J.Json.get_str(p, "name", "")), e => VoiceJob.done(e, inbox, home, client, id))
    # Settings' Antigravity (AgyJob)
    case 10n:
      AgyJob.go(p, inbox, home, client, id)
    # Settings > Usage's Boat side (BoatUse)
    case 11n:
      BoatUse.go(p, inbox, home, client, id)
    case _:
      BotJob.fail(inbox, client, id, "unknown voice request")

def VoiceJob.go(+p: J.Json, +inbox: Chan(Msg), +home: String, +client: U32, +id: String) -> IO(Unit):
  VoiceJob.op(C.Codec.pos(["status", "key", "unkey", "dict", "add", "start", "stop", "devices", "api-add", "api-del", "agy", "usage"], J.Json.get_str(p, "op", "")), inbox, home, p, client, id)

def VoiceJob.run(+inbox: Chan(Msg), +home: String, +params: String, +client: U32, +id: String) -> IO(Unit):
  VoiceJob.go(Maybe.default(&2, J.Json, J.Json.parse(params), J.Null{}), inbox, home, client, id)

# The bots' browsers: who drives, teaching, task runs (core/teach.bend)
# ---------------------------------------------------------------------
# IO that needs no hub state: files under <home>/bots/<id>/tasks/ and the
# person's input passed to the helper. Nothing here keeps a secret: a
# recorded secret is only a label (the recorder masks it in the page).

def Tk.dir(+home: String, +bot: String) -> String:
  home ++ "/bots/" ++ bot ++ "/tasks"

# best effort: a line added to a file (the folder made)
def Tk.append(+path: String, +line: String) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", "mkdir -p -- \"$(dirname -- \"$1\")\" && printf '%s\\n' \"$2\" >> \"$1\"", "sh", path, line], "/")
    return Unit{}

def Tk.draft.clear(+home: String, +bot: String) -> IO(Unit):
  PU.Push.rm(Tk.dir(home, bot) ++ "/.draft.jsonl")

def Tk.draft.add(+home: String, +bot: String, +line: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(line), IO.pure(Unit, Unit{}), Tk.append(Tk.dir(home, bot) ++ "/.draft.jsonl", line))

# <home>/bots/<bot>/tasks/<slug>.json, the copy of a task (a plain id and slug only)
def Tk.file.go(ok: Bool, +home: String, +bot: String, +slug: String, +json: String) -> IO(Unit):
  match ok:
    case True{}:
      do IO<Unit>:
        mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Tk.dir(home, bot))
        PU.Push.write(Tk.dir(home, bot) ++ "/" ++ slug ++ ".json", json)
    case False{}:
      IO.pure(Unit, Unit{})

def Tk.file(+home: String, +bot: String, +slug: String, +json: String) -> IO(Unit):
  Tk.file.go(Bool.and(BN.Bn.safe(bot), TE.Teach.part(slug, False{})), home, bot, slug, json)

def Tk.gone(+home: String, +bot: String, +slug: String) -> IO(Unit):
  Bool.pick(IO(Unit), Bool.and(BN.Bn.safe(bot), TE.Teach.part(slug, False{})), PU.Push.rm(Tk.dir(home, bot) ++ "/" ++ slug ++ ".json"), IO.pure(Unit, Unit{}))

# the recorder on or off for a bot's page
def Tk.record(m: Maybe<&2, Chan(BR.BMsg)>, +bot: String, on: Bool) -> IO(Unit):
  match m:
    case Some{q}:
      do IO<Unit>:
        r : String <- BR.Browser.ask_on(q, bot, "record", J.Json.obj([J.KV{"on", J.Flag{on}}]))
        return Unit{}
    case None{}:
      IO.pure(Unit, Unit{})

# teach begins: an empty draft file, the recorder on
def Tk.begin(m: Maybe<&2, Chan(BR.BMsg)>, +home: String, +bot: String) -> IO(Unit):
  do IO<Unit>:
    Tk.draft.clear(home, bot)
    Tk.record(m, bot, True{})

# the page's size from a status reply (the viewport a click lands in)
def Tk.size.r(+r: J.Json) -> TKS.TkPt:
  TKS.TkPt{J.Json.get_nat(r, "width", 1280n), J.Json.get_nat(r, "height", 800n)}

def Tk.size(+line: String) -> TKS.TkPt:
  Tk.size.r(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), "result"))

def Tk.pt.w(p: TKS.TkPt) -> Nat:
  match p:
    case TKS.TkPt{w, _}:
      w

def Tk.pt.h(p: TKS.TkPt) -> Nat:
  match p:
    case TKS.TkPt{_, h}:
      h

def Tk.pt.x(p: TKS.TkPt) -> Nat:
  match p:
    case TKS.TkPt{x, _}:
      x

def Tk.pt.y(p: TKS.TkPt) -> Nat:
  match p:
    case TKS.TkPt{_, y}:
      y

# the person's input ops, in order
def Tk.ops() -> List<&2, String>:
  ["click", "mouse", "type", "press", "scroll", "navigate", "back", "reload"]

def Tk.num.k(m: Maybe<&2, J.Json>, +key: String) -> List<&2, J.KV>:
  match m:
    case Some{J.Num{r}}:
      [J.KV{key, J.Num{r}}]
    case _:
      Nil{}

def Tk.num(+o: J.Json, +key: String) -> List<&2, J.KV>:
  Tk.num.k(J.Json.get(o, key), key)

# the mouse event's kind: only move, down or up
def Tk.mouse.type(+t: String) -> String:
  Bool.pick(String, Bool.or(String.eq(t, "down"), String.eq(t, "up")), t, "move")

# what the helper is asked for an op; click and mouse positions come in
# thousandths of the frame and are scaled to the page (Task.scale)
def Tk.args.k(i: Nat, +o: J.Json, +p: TKS.TkPt) -> J.Json:
  match i:
    case 0n:
      J.Json.obj([J.KV{"x", J.Json.nat(Tk.pt.x(p))}, J.KV{"y", J.Json.nat(Tk.pt.y(p))}])
    case 1n:
      J.Json.obj([J.KV{"type", J.Str{Tk.mouse.type(J.Json.get_str(o, "type", "move"))}}, J.KV{"x", J.Json.nat(Tk.pt.x(p))}, J.KV{"y", J.Json.nat(Tk.pt.y(p))}])
    case 2n:
      J.Json.obj([J.KV{"text", J.Str{J.Json.get_str(o, "text", "")}}])
    case 3n:
      J.Json.obj([J.KV{"key", J.Str{J.Json.get_str(o, "key", "")}}])
    case 4n:
      J.Json.obj(List.append(&2, J.KV, Tk.num(o, "dx"), Tk.num(o, "dy")))
    case 5n:
      J.Json.obj([J.KV{"url", J.Str{J.Json.get_str(o, "url", "")}}])
    case _:
      J.Json.obj(Nil{})

# the click's point in the page, scaled from the thousandths of the frame
def Tk.at(+o: J.Json, +pt: TKS.TkPt) -> TKS.TkPt:
  TKS.Task.scale(J.Json.get_nat(o, "x", 0n), J.Json.get_nat(o, "y", 0n), 1000n, 1000n, Tk.pt.w(pt), Tk.pt.h(pt))

def Tk.args(i: Nat, +o: J.Json, +pt: TKS.TkPt) -> J.Json:
  Tk.args.k(i, o, Tk.at(o, pt))

def Tk.view(need: Bool, +q: Chan(BR.BMsg), +bot: String) -> IO(TKS.TkPt):
  match need:
    case True{}:
      IO.bind(String, TKS.TkPt, BR.Browser.ask_on(q, bot, "status", J.Json.obj(Nil{})), line => IO.pure(TKS.TkPt, Tk.size(line)))
    case False{}:
      IO.pure(TKS.TkPt, TKS.TkPt{1280n, 800n})

def Tk.input.go(skip: Bool, +i: Nat, +q: Chan(BR.BMsg), +bot: String, +o: J.Json, +pt: TKS.TkPt) -> IO(Unit):
  match skip:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        r : String <- BR.Browser.ask_on(q, bot, Maybe.default(&2, String, List.get(&2, String, Tk.ops(), i), "status"), Tk.args(i, o, pt))
        return Unit{}

def Tk.input.k(+i: Nat, m: Maybe<&2, Chan(BR.BMsg)>, +bot: String, +o: J.Json) -> IO(Unit):
  match m:
    case Some{+q}:
      IO.bind(TKS.TkPt, Unit, Tk.view(Nat.is_lt(i, 2n), q, bot), pt => Tk.input.go(Nat.is_ge(i, 8n), i, q, bot, o, pt))
    case None{}:
      IO.pure(Unit, Unit{})

# the person's input on a bot's page, passed to the helper on that page
def Tk.input(m: Maybe<&2, Chan(BR.BMsg)>, +bot: String, +o: J.Json) -> IO(Unit):
  Tk.input.k(C.Codec.pos(Tk.ops(), J.Json.get_str(o, "op", "")), m, bot, o)

# one B.BotFx: home, gone, hook, unhook, send, invite, join, unpeer, google,
# token, retoken, untoken, voice
def Bot.fx(i: Nat, +st: M.State, +inbox: Chan(Msg), +cfg: Cfg, +a: String, +b: String, +c: String) -> IO(Unit):
  match i:
    case 0n:
      IO.spawn(Unit, BotJob.home.k(BN.Bn.safe(a), inbox, Cfg.home(cfg) ++ "/bots/", a, b))
    case 1n:
      IO.spawn(Unit, BotJob.close(Cfg.br(cfg), a))
    case 2n:
      IO.spawn(Unit, BotJob.hook(inbox, Cfg.home(cfg), a, BotJob.client(b), BotJob.id(c)))
    case 3n:
      IO.spawn(Unit, BN.Bn.hook.forget(Cfg.home(cfg), a))
    case 4n:
      BotJob.send.k(Bot.peer.url(st, a), cfg, a, b, c)
    case 5n:
      IO.spawn(Unit, BotJob.invite(inbox, Cfg.home(cfg), Cfg.tailnet(cfg), Cfg.host(cfg), Cfg.port(cfg), a, BotJob.client(b), BotJob.id(c)))
    case 6n:
      BotJob.join.k(SG.Invite.parse(a), st, inbox, cfg, BotJob.client(b), BotJob.id(c))
    case 7n:
      IO.spawn(Unit, BN.Bn.peer.forget(Cfg.home(cfg), a))
    case 8n:
      IO.spawn(Unit, BotJob.google(inbox, Cfg.home(cfg), Cfg.tailnet(cfg), Cfg.host(cfg), Cfg.port(cfg), a, BotJob.client(b), BotJob.id(c)))
    case 9n:
      IO.spawn(Unit, BotJob.token(0n, inbox, Cfg.home(cfg), Cfg.tailnet(cfg), Cfg.host(cfg), Cfg.port(cfg), a, BotJob.client(b), BotJob.id(c)))
    case 10n:
      IO.spawn(Unit, BotJob.token(1n, inbox, Cfg.home(cfg), Cfg.tailnet(cfg), Cfg.host(cfg), Cfg.port(cfg), a, BotJob.client(b), BotJob.id(c)))
    case 11n:
      IO.spawn(Unit, BotJob.token(2n, inbox, Cfg.home(cfg), Cfg.tailnet(cfg), Cfg.host(cfg), Cfg.port(cfg), a, BotJob.client(b), BotJob.id(c)))
    case 12n:
      IO.spawn(Unit, VoiceJob.run(inbox, Cfg.home(cfg), a, BotJob.client(b), BotJob.id(c)))
    case 13n:
      BotJob.scratch(BN.Bn.safe(a), inbox, Cfg.home(cfg) ++ "/scratch/", a)
    case 14n:
      IO.spawn(Unit, BotJob.scratch.gone(BN.Bn.safe(a), Cfg.home(cfg), a))
    case 15n:
      IO.spawn(Unit, BotJob.uber(Bool.and(BN.Bn.safe(a), Bool.or(String.is_empty(BO.Bot.thread(BO.World.bot(M.State.bots(st), a))), BN.Bn.safe(BO.Bot.thread(BO.World.bot(M.State.bots(st), a))))), inbox, Cfg.home(cfg), a,
        BO.Bot.thread(BO.World.bot(M.State.bots(st), a)), BotJob.client(b), BotJob.id(c)))
    case 16n:
      Self.post(inbox, UberDo{"home:" ++ a})
    case 17n:
      IO.spawn(Unit, ZoneJob.set(inbox, a, M.Setting.get(st, "tz.rule." ++ a, ""), BotJob.client(b), BotJob.id(c)))
    case 18n:
      IO.spawn(Unit, Tk.file(Cfg.home(cfg), a, b, c))
    case 19n:
      IO.spawn(Unit, Tk.gone(Cfg.home(cfg), a, b))
    case _:
      IO.pure(Unit, Unit{})

# Terminals in the chat at the person's next message
# --------------------------------------------------
# What the thread's terminals show (those the agent has not seen as they
# are) goes after the message, and the ones not pinned stop: they grey
# out in the chat until a click starts them again (core/emb.bend).

def EmbNote.ask(m: Maybe<&2, Chan(TM.TMsg)>, +thread: String) -> IO(String):
  match m:
    case Some{q}:
      do IO<String>:
        +r : Chan(String) <- Chan.new(String, 1)
        ok : Result<&1, &1, TM.TMsg, Unit> <- Chan.send(TM.TMsg, q, TM.TNote{thread, r})
        a : Maybe<&1, String> <- Chan.recv(String, r)
        return Maybe.default(&1, String, a, "")
    case None{}:
      IO.pure(String, "")

def EmbNote.kill(ks: List<&2, String>, +m: Maybe<&2, Chan(TM.TMsg)>) -> IO(Unit):
  match ks:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case k <> t:
      IO.bind(Unit, Unit, Term.put(m, TM.TKill{k}), u => EmbNote.kill(t, m))

def EmbNote.take.k(has: Bool, +cfg: Cfg, +st: M.State, +thread: String) -> IO(String):
  match has:
    case True{}:
      do IO<String>:
        +n : String <- EmbNote.ask(Cfg.tm(cfg), thread)
        EmbNote.kill(EM.Emb.loose(M.State.mine(st, thread), st), Cfg.tm(cfg))
        return n
    case False{}:
      IO.pure(String, "")

def EmbNote.take(+cfg: Cfg, +st: M.State, +thread: String) -> IO(String):
  EmbNote.take.k(EM.Emb.any(M.State.mine(st, thread)), cfg, st, thread)

def EmbNote.user.k(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{+o}:
      String.eq(J.Json.get_str(o, "type", ""), "user")
    case None{}:
      False{}

def EmbNote.user(+line: String) -> Bool:
  EmbNote.user.k(J.Json.parse(line))

def EmbNote.put.k(m: Maybe<&2, J.Json>, +line: String, +n: String) -> String:
  match m:
    case Some{+o}:
      B.Claude.user(Mention.user_text(o) ++ n)
    case None{}:
      line

def EmbNote.put(+line: String, +n: String) -> String:
  Bool.pick(String, String.is_empty(n), line, EmbNote.put.k(J.Json.parse(line), line, n))

# a Claude user line, with the note after its text
def EmbNote.claude(user: Bool, +cfg: Cfg, +st: M.State, +thread: String, +line: String) -> IO(String):
  match user:
    case True{}:
      IO.bind(String, String, EmbNote.take(cfg, st, thread), n => IO.pure(String, EmbNote.put(line, n)))
    case False{}:
      IO.pure(String, line)

# terminal_read: the actor's answer, again every 250 ms while the program
# runs and waitMs lasts
def TermRead.ask(+q: Chan(TM.TMsg), +key: String) -> IO(String):
  do IO<String>:
    +r : Chan(String) <- Chan.new(String, 1)
    ok : Result<&1, &1, TM.TMsg, Unit> <- Chan.send(TM.TMsg, q, TM.TRead{key, r})
    a : Maybe<&1, String> <- Chan.recv(String, r)
    return Maybe.default(&1, String, a, "gone\n")

def TermRead.answer(+rid: String, +text: String) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":" ++ rid ++ ",\"result\":" ++ J.Json.show(J.Json.obj([
    J.KV{"content", J.Json.arr([J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{Bool.pick(String, String.starts_with(text, "gone"), "There is no such terminal in this thread.", String.join(List.drop(&2, String, String.split(text, '\n'), 1n), "\n"))}}])])},
    J.KV{"isError", J.Flag{String.starts_with(text, "gone")}}])) ++ "}"

def TermRead.got(last: Bool, +text: String, +rid: String, +reply: Chan(String), k: Unit -> IO(Unit)) -> IO(Unit):
  match last:
    case False{}:
      Bool.pick(IO(Unit), String.starts_with(text, "running\n"), IO.bind(Unit, Unit, IO.sleep(250), k),
        IO.bind(Result<&1, &1, String, Unit>, Unit, Chan.send(String, reply, TermRead.answer(rid, text)), ok => IO.pure(Unit, Unit{})))
    case True{}:
      IO.bind(Result<&1, &1, String, Unit>, Unit, Chan.send(String, reply, TermRead.answer(rid, text)), ok => IO.pure(Unit, Unit{}))

def TermRead.loop(fuel: Nat, +q: Chan(TM.TMsg), +key: String, +rid: String, +reply: Chan(String)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.bind(String, Unit, TermRead.ask(q, key), t => TermRead.got(True{}, t, rid, reply, u => IO.pure(Unit, Unit{})))
    case 1n+f:
      IO.bind(String, Unit, TermRead.ask(q, key), t => TermRead.got(False{}, t, rid, reply, u => TermRead.loop(f, q, key, rid, reply)))

def TermRead.run(m: Maybe<&2, Chan(TM.TMsg)>, +key: String, +wait: Nat, +rid: String, +reply: Chan(String)) -> IO(Unit):
  match m:
    case Some{q}:
      TermRead.loop(Nat.div(Nat.min(wait, 120000n), 250n), q, key, rid, reply)
    case None{}:
      IO.bind(Result<&1, &1, String, Unit>, Unit, Chan.send(String, reply, TermRead.answer(rid, "gone\n")), ok => IO.pure(Unit, Unit{}))

def TermRead.is(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "terminal_read"))

def Hub.term_read(h: Hub, +thread: String, +o: J.Json, +reply: Chan(String)) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +a : J.Json = J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments")
        IO.spawn(Unit, TermRead.run(Cfg.tm(cfg), EM.Emb.key(thread, J.Json.get_str(a, "id", "")), U32.to_nat(J.Json.get_u32(a, "waitMs", 0)), J.Json.as_raw(J.Json.get(o, "id"), "null"), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# Watched work (core's Watch.*)
# -----------------------------
# Each piece of watched work has a folder, <home>/work/<id>. A work_run's
# command runs from there through a small script that writes its output
# to out and, when it ends, its exit code to code (written to code.tmp and
# renamed, so code appears whole); started marks that it was launched, so
# a launch asked again (after a restart) never starts it twice; it runs
# set apart with setsid, a child of the hub's own service, so a restart of
# Backplane can cut it off: then it reads as lost and the thread is told.
# The code file is the outcome: a run gone without one, or a pid reused or
# gone, reads as "gone", outcome unknown, never success. A watcher checks
# every 2 s.

def Watch.dir(+home: String, +id: String) -> String:
  home ++ "/work/" ++ id

# The shell each watch script runs as; none for a hub on a copied log
# (BACKPLANE_COPY=1): it never launches, re-launches or signals anything
# the log names, and every watch reads as gone (laws watch_copy_*)
def Watch.argv.k(copy: Bool, xs: List<&2, String>) -> List<&2, String>:
  match copy:
    case True{}:
      Nil{}
    case False{}:
      xs

def Watch.launch.sh() -> String:
  "d=$1; c=$2; cmd=$3\npid_id() { q=$1; if [ -r \"/proc/$q/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$q/stat\" | cut -d' ' -f20); echo \"$b:$st\"; elif ps -p \"$q\" >/dev/null 2>&1; then ps -o lstart= -p \"$q\" | tr -s ' ' '_'; fi; }\nmkdir -p \"$d\" || { echo gone; exit 0; }\n[ -e \"$d/cancel\" ] && { echo gone; exit 0; }\n[ -e \"$d/code\" ] && { echo running; exit 0; }\nif [ -e \"$d/started\" ]; then { [ -e \"$d/out\" ] || [ -e \"$d/how\" ]; } && { echo running; exit 0; }; fi\nprintf '%s\\n' \"$cmd\" > \"$d/cmd\"\n{ printf ': > \"%s/out\"\\n' \"$d\"; printf 'cd \"%s\" || exit 127\\n' \"$c\"; printf '[ -e \"%s/cancel\" ] && exit 143\\n' \"$d\"; printf 'sh \"%s/cmd\" > \"%s/out\" 2>&1\\n' \"$d\" \"$d\"; printf 'echo $? > \"%s/code.tmp\" && mv \"%s/code.tmp\" \"%s/code\"\\n' \"$d\" \"$d\" \"$d\"; } > \"$d/run.sh\"\n: > \"$d/started\"\nif command -v setsid >/dev/null 2>&1; then setsid nohup sh \"$d/run.sh\" >/dev/null 2>&1 < /dev/null &\nelse perl -MPOSIX -e 'POSIX::setsid(); exec {$ARGV[0]} @ARGV or exit 127' nohup sh \"$d/run.sh\" >/dev/null 2>&1 < /dev/null &\nfi\necho $! > \"$d/pid\"\npid_id $! > \"$d/pidid\"\necho pid > \"$d/how\"\necho running\n"

def Watch.check.sh() -> String:
  "k=$1; t=$2; id=$3; d=$4\ncase $k in\nrun)\n  if [ -e \"$d/cancel\" ]; then\n    (\nd=$4\n# A committed job may still be in its launch fiber when the hub dies.\nif [ -e \"$d/intent\" ]; then\n  : > \"$d/cancel\"\n  for attempt in 1 2; do\n    [ -s \"$d/pid\" ] && [ -s \"$d/pidid\" ] && break\n    sleep 1\n  done\nfi\npid_id() {\n  q=$1\n  if [ -r \"/proc/$q/stat\" ]; then\n    b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null)\n    st=$(sed 's/^.*) //' \"/proc/$q/stat\" | cut -d' ' -f20)\n    echo \"$b:$st\"\n  elif ps -p \"$q\" >/dev/null 2>&1; then\n    ps -o lstart= -p \"$q\" | tr -s ' ' '_'\n  fi\n}\np=$(cat \"$d/pid\" 2>/dev/null)\nsaved=$(cat \"$d/pidid\" 2>/dev/null)\n[ -n \"$p\" ] && [ \"$p\" -gt 1 ] 2>/dev/null && [ -n \"$saved\" ] && [ \"$(pid_id \"$p\")\" = \"$saved\" ] || exit 0\nkill -TERM \"-$p\" 2>/dev/null || :\nkill -TERM \"$p\" 2>/dev/null || :\nfor attempt in 1 2; do\n  if kill -0 \"-$p\" 2>/dev/null || kill -0 \"$p\" 2>/dev/null; then sleep 1; else exit 0; fi\ndone\nkill -KILL \"-$p\" 2>/dev/null || :\n[ \"$(pid_id \"$p\")\" != \"$saved\" ] || kill -KILL \"$p\" 2>/dev/null || :\nfor attempt in 1 2; do\n  if kill -0 \"-$p\" 2>/dev/null || kill -0 \"$p\" 2>/dev/null; then sleep 1; else break; fi\ndone\nexit 0\n\n    )\n  fi\n  [ -e \"$d/stop\" ] && { echo stop; exit 0; }\n  [ -e \"$d/code\" ] && { echo \"done $(cat \"$d/code\")\"; exit 0; }\n  p=$(cat \"$d/pid\" 2>/dev/null); [ -n \"$p\" ] && kill -0 \"$p\" 2>/dev/null && { echo running; exit 0; }\n  [ -e \"$d/code\" ] && { echo \"done $(cat \"$d/code\")\"; exit 0; }\n  echo gone ;;\npid)\n  [ -e \"$d/stop\" ] && { echo stop; exit 0; }\n  if [ -r \"/proc/$t/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$t/stat\" | cut -d' ' -f20); n=\"$b:$st\"\n  elif ps -p \"$t\" >/dev/null 2>&1; then n=$(ps -o lstart= -p \"$t\" | tr -s ' ' '_')\n  else echo gone; exit 0; fi\n  [ \"$id\" = - ] && { echo \"ident $n\"; echo running; exit 0; }\n  [ \"$n\" = \"$id\" ] && echo running || echo gone ;;\nfile)\n  [ -e \"$d/stop\" ] && { echo stop; exit 0; }\n  [ -f \"$t\" ] && grep -qF \"$id\" \"$t\" 2>/dev/null && { echo found; exit 0; }\n  echo running ;;\n*) echo gone ;;\nesac\n"

def Watch.stop.sh() -> String:
  "pid_id() { q=$1; if [ -r \"/proc/$q/stat\" ]; then b=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null); st=$(sed 's/^.*) //' \"/proc/$q/stat\" | cut -d' ' -f20); echo \"$b:$st\"; elif ps -p \"$q\" >/dev/null 2>&1; then ps -o lstart= -p \"$q\" | tr -s ' ' '_'; fi; }\nk=$1; t=$2; id=$3; d=$4\nmkdir -p \"$d\"\ncase $k in\nunwatch) : > \"$d/stop\" ;;\nrun) : > \"$d/cancel\"; p=$(cat \"$d/pid\" 2>/dev/null); n=$(pid_id \"$p\"); [ -n \"$p\" ] && [ \"$p\" -gt 1 ] && [ -n \"$n\" ] && [ \"$n\" = \"$(cat \"$d/pidid\" 2>/dev/null)\" ] && { kill -TERM \"-$p\" 2>/dev/null; kill -TERM \"$p\" 2>/dev/null; } ;;\npid) [ \"$t\" -gt 1 ] 2>/dev/null && [ \"$id\" != - ] && [ \"$(pid_id \"$t\")\" = \"$id\" ] && kill -TERM \"$t\" 2>/dev/null ;;\nesac\ntrue\n"

def Watch.tail.sh() -> String:
  "d=$1; [ -e \"$d/out\" ] && tail -c 2048 \"$d/out\"; true\n"

def Watch.launch.argv(copy: Bool, +home: String, +id: String, +cwd: String, +target: String) -> List<&2, String>:
  Watch.argv.k(copy, ["-c", Watch.launch.sh(), "sh", Watch.dir(home, id), cwd, target])

def Watch.check.argv(copy: Bool, +kind: String, +target: String, +ident: String, +home: String, +id: String) -> List<&2, String>:
  Watch.argv.k(copy, ["-c", Watch.check.sh(), "sh", kind, target, ident, Watch.dir(home, id)])

def Watch.stop.argv(copy: Bool, +kind: String, +target: String, +ident: String, +home: String, +id: String) -> List<&2, String>:
  Watch.argv.k(copy, ["-c", Watch.stop.sh(), "sh", kind, target, ident, Watch.dir(home, id)])

def Watch.intent.argv(copy: Bool, +home: String, +id: String) -> List<&2, String>:
  Watch.argv.k(copy, ["-c", "mkdir -p \"$1\" && : > \"$1/intent\"", "sh", Watch.dir(home, id)])

# a watch script run: none (no argv) answers "gone"
def Watch.sh(argv: List<&2, String>) -> IO(P.Ran):
  match argv:
    case Nil{}:
      IO.pure(P.Ran, P.Ran{0, "gone"})
    case +a <> t:
      P.Proc.run("sh", a <> t, "/")

def Watch.lines(+out: String) -> List<&2, String>:
  M.Bg.lines(out)

def Watch.last(ls: List<&2, String>, +acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      Watch.last(t, Bool.pick(String, String.starts_with(l, "ident "), acc, String.trim(l)))

def Watch.ident.of(ls: List<&2, String>, +acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      Watch.ident.of(t, Bool.pick(String, String.starts_with(l, "ident "), String.trim(String.drop(l, 6n)), acc))

def Watch.ran(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      out

def Watch.tail.text(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      out

# the outcome's words for the thread: the code, where the output is, its end
def Watch.detail(+kind: String, +status: String, +dir: String, +tail: String) -> String:
  Bool.pick(String, String.eq(kind, "run"),
    Bool.pick(String, String.starts_with(status, "exit "), "exit code " ++ String.drop(status, 5n) ++ "\noutput: " ++ dir ++ "/out" ++ Bool.pick(String, String.is_empty(tail), "", "\nits end:\n" ++ tail), "output so far: " ++ dir ++ "/out"),
    "")

def Watch.report(+inbox: Chan(Msg), +thread: String, +id: String, +kind: String, +status: String, +dir: String) -> IO(Unit):
  do IO<Unit>:
    tr : P.Ran <- P.Proc.run("sh", ["-c", Watch.tail.sh(), "sh", dir], "/")
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, WatchEnded{thread, id, status, Watch.detail(kind, status, dir, Watch.tail.text(tr))})
    return Unit{}

# a check's verdict: "running" carries on, "stop" (unwatched) ends quietly,
# anything else is reported
def Watch.verdict(+last: String) -> String:
  Bool.pick(String, String.starts_with(last, "done "), "exit " ++ String.drop(last, 5n), Bool.pick(String, String.eq(last, "found"), "done", last))

def Watch.step(i: Nat, +status: String, +ident: String, +thread: String, +id: String, +kind: String, +dir: String, +inbox: Chan(Msg), k: String -> IO(Unit)) -> IO(Unit):
  match i:
    case 0n:
      do IO<Unit>:
        IO.sleep(2000)
        k(ident)
    case 1n:
      IO.pure(Unit, Unit{})
    case _:
      Watch.report(inbox, thread, id, kind, Watch.verdict(status), dir)

def Watch.ident.post(same: Bool, +inbox: Chan(Msg), +thread: String, +id: String, +ident: String) -> IO(Unit):
  match same:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, WatchIdent{thread, id, ident})
        return Unit{}

def Watch.got.k(+ls: List<&2, String>, +ident: String, +thread: String, +id: String, +kind: String, +dir: String, +inbox: Chan(Msg), k: String -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    Watch.ident.post(String.eq(Watch.ident.of(ls, ident), ident), inbox, thread, id, Watch.ident.of(ls, ident))
    Watch.step(C.Codec.pos(["running", "stop"], Watch.last(ls, "gone")), Watch.last(ls, "gone"), Watch.ident.of(ls, ident), thread, id, kind, dir, inbox, k)

def Watch.got(r: P.Ran, +ident: String, +thread: String, +id: String, +kind: String, +dir: String, +inbox: Chan(Msg), k: String -> IO(Unit)) -> IO(Unit):
  Watch.got.k(Watch.lines(Watch.ran(r)), ident, thread, id, kind, dir, inbox, k)

def Watch.timeout.next(locked: Bool, +ident: String, k: String -> IO(Unit)) -> IO(Unit):
  match locked:
    case True{}:
      k(ident)
    case False{}:
      IO.pure(Unit, Unit{})

def Watch.tick(late: Bool, +locked: Bool, +ident: String, +home: String, +thread: String, +id: String, +kind: String, +target: String, +inbox: Chan(Msg), k: String -> IO(Unit)) -> IO(Unit):
  match late:
    case True{}:
      do IO<Unit>:
        Watch.report(inbox, thread, id, kind, "timeout", Watch.dir(home, id))
        Watch.timeout.next(locked, ident, k)
    case False{}:
      IO.bind(Bool, Unit, P.Proc.copy(), copy => IO.bind(P.Ran, Unit, Watch.sh(Watch.check.argv(copy, kind, target, ident, home, id)),
        r => Watch.got(r, ident, thread, id, kind, Watch.dir(home, id), inbox, k)))

# the watcher: a check every 2 s until the work ends, its deadline passes,
# or it is no longer watched
def Watch.loop(fuel: Nat, +locked: Bool, +ident: String, +home: String, +thread: String, +id: String, +kind: String, +target: String, +deadline: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Nat, Unit, Now.nat(), +now => Watch.tick(Bool.and(Nat.is_gt(deadline, 0n), Nat.is_ge(now, deadline)), locked, ident, home, thread, id, kind, target, inbox,
        i2 => Watch.loop(f, locked, i2, home, thread, id, kind, target, Bool.pick(Nat, Bool.and(locked, Bool.and(Nat.is_gt(deadline, 0n), Nat.is_ge(now, deadline))), 0n, deadline), inbox)))

def Watch.launched(r: P.Ran, +thread: String, +id: String, +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case P.Ran{_, out}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, WatchLaunched{thread, id})
        return Unit{}

def Watch.start.k(argv: List<&2, String>, +thread: String, +id: String, +inbox: Chan(Msg)) -> IO(Unit):
  match argv:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +a <> t:
      IO.bind(P.Ran, Unit, P.Proc.run("sh", a <> t, "/"), r => Watch.launched(r, thread, id, inbox))

def Watch.start(run: Bool, +home: String, +thread: String, +id: String, +target: String, +cwd: String, +inbox: Chan(Msg)) -> IO(Unit):
  match run:
    case True{}:
      IO.bind(Bool, Unit, P.Proc.copy(), copy => Watch.start.k(Watch.launch.argv(copy, home, id, cwd, target), thread, id, inbox))
    case False{}:
      IO.pure(Unit, Unit{})

# an Await: a work_run's command launched (unless it ran already), then
# watched
def LockJob.later(+inbox: Chan(Msg), +name: String, +holder: String, +n: Nat) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(100)
    sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, LockProbe{name, holder, n})
    return Unit{}

def LockJob.out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      out

# who holds the flock outside the hub, asked off the hub's loop (a shell and
# a few tools: it ran on the loop every 100 ms while a lock waited, and
# held every other message up meanwhile)
def LockJob.who(+inbox: Chan(Msg), +name: String, +holder: String) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", LG.Guard.who(), "sh", name], "/")
    sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, LockSaw{name, holder, LockJob.out(r)})
    return Unit{}

# the probes between two asks of who holds a lock outside (2 s)
def LockJob.every() -> Nat:
  20n

def LockJob.who.if(ask: Bool, +inbox: Chan(Msg), +name: String, +holder: String) -> IO(Unit):
  match ask:
    case True{}:
      IO.spawn(Unit, LockJob.who(inbox, name, holder))
    case False{}:
      IO.pure(Unit, Unit{})

def Watch.intent(locked: Bool, +home: String, +id: String) -> IO(Unit):
  match locked:
    case True{}:
      do IO<Unit>:
        copy : Bool <- P.Proc.copy()
        r : P.Ran <- Watch.sh(Watch.intent.argv(copy, home, id))
        return Unit{}
    case False{}:
      IO.pure(Unit, Unit{})

def Watch.job(+locked: Bool, +home: String, +thread: String, +id: String, +kind: String, +target: String, +ident: String, +deadline: Nat, +cwd: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Watch.start(String.eq(kind, "run"), home, thread, id, target, cwd, inbox)
    Watch.loop(Fuel.forever(), locked, ident, home, thread, id, kind, target, deadline, inbox)

def Watch.stop.job(+home: String, +id: String, +kind: String, +target: String, +ident: String) -> IO(Unit):
  do IO<Unit>:
    copy : Bool <- P.Proc.copy()
    r : P.Ran <- Watch.sh(Watch.stop.argv(copy, kind, target, ident, home, id))
    return Unit{}

# Threads elsewhere (far.bend, docs/bots.md)
# ------------------------------------------
# This hub shares its bots and their threads with the owner's other
# machines it is linked to: each commit's shared changes are pushed to
# each (POST /far/push, one push out per link at a time), and a machine
# that missed some pulls them (POST /far/log, paged). What the others
# share with this one is kept as a mirror per link (in memory and in
# <home>/far/<link>.jsonl) and handed to every client ({"t":"far"}); a
# client's request about something elsewhere goes to its hub (POST
# /far/rpc). Every request between hubs is signed like /bots/deliver. The
# network is only ever used off the hub, in jobs that answer by message.

def Far.fh(h: Hub) -> FR.Fh & Hub:
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      (BN.Bn.far(Cfg.bn(cfg)), Hub{st, f, ls, cl, as, inbox, cfg, note})

def Far.with(h: Hub, fh: FR.Fh) -> Hub:
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub{st, f, ls, cl, as, inbox, Cfg.with_bn(cfg, BN.Bn.with_far(Cfg.bn(cfg), fh)), note}

# the machines' list (self and peers, as the switcher shows it)
def Far.machines(+cfg: Cfg) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(Cfg.machines(cfg)), J.Null{})

# whether a linked machine is one of the owner's own (only those get this
# hub's threads, or may act on them)
def Far.some(m: Maybe<&2, String>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

# (and not one the owner turned sync off with: "sync.<name>", UC.Mate.off)
def Far.own(+st: M.State, +cfg: Cfg, p: BO.Peer) -> Bool:
  match p:
    case BO.Peer{_, +n, +u, _, r}:
      Bool.and(Bool.and(Bool.not(r), Bool.not(UC.Mate.off(st, n))), Far.some(UC.Far.machine(Far.machines(cfg), n, u)))

def Far.keep(ok: Bool, p: BO.Peer, rest: List<&2, BO.Peer>) -> List<&2, BO.Peer>:
  match ok:
    case True{}:
      p <> rest
    case False{}:
      rest

def Far.own.go(ps: List<&2, BO.Peer>, +st: M.State, +cfg: Cfg) -> List<&2, BO.Peer>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Far.keep(Far.own(st, cfg, p), p, Far.own.go(t, st, cfg))

def Far.own.id(+st: M.State, +cfg: Cfg, +id: String) -> Bool:
  Bool.and(Bot.peer.live(st, id), Far.own(st, cfg, BO.World.peer(M.State.bots(st), id)))

def Far.dir(+home: String) -> String:
  home ++ "/far"

def Far.file(+home: String, +link: String) -> String:
  Far.dir(home) ++ "/" ++ link ++ ".jsonl"

# Jobs (off the hub)
# ------------------

def Far.send(+inbox: Chan(Msg), m: Msg) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, m)
    return Unit{}

def Far.code(r: BN.Resp) -> Nat:
  match r:
    case BN.Resp{c, _}:
      c

def Far.body(r: BN.Resp) -> String:
  match r:
    case BN.Resp{_, b}:
      b

# a push too big for a request says only where it ends, past a gap, so
# the other end pulls it in pages
# The first durable change identifies the history. Cache its hash at
# startup or the first (small) commit, before accepting new pushes.
def Far.history(+ls: List<&2, String>) -> String:
  Bool.pick(String, List.is_empty(&2, String, ls), "", SG.Sign.header("00", 0n, FR.Fr.history.body(Maybe.default(&2, String, List.last(&2, String, ls), ""), 1n, 1n)))

# at start: lines the log holds that this build cannot decode give the
# history a new identity (Fr.history.body)
def Far.history.at(+ls: SO.Lg, +seq: Nat) -> String:
  Bool.pick(String, Nat.is_eq(SO.Lg.n(ls), 0n), "", SG.Sign.header("00", 0n, FR.Fr.history.body(SO.Lg.first(ls), SO.Lg.n(ls), seq)))

def Far.push.body(+s: FR.Send, +name: String) -> String:
  Bool.pick(String, Nat.is_gt(String.length(FR.Send.body(s, name)), 200000n), FR.Send.body(FR.Send.gap(s), name), FR.Send.body(s, name))

def Far.push.job(s: FR.Send, +epoch: String, +home: String, +url: String, +inbox: Chan(Msg)) -> IO(Unit):
  match s:
    case FR.Send{+link, +since, +to, xs}:
      do IO<Unit>:
        now : Nat <- Now.nat()
        +name : String <- BN.Bn.name()
        +r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/push", True{}, FR.Fr.with.epoch(Far.push.body(FR.Send{link, since, to, xs}, name), epoch))
        Far.send(inbox, FarPushed{link, Nat.is_eq(Far.code(r), 200n), to})

def Far.pull.job(+link: String, +since: Nat, +epoch: String, +home: String, +url: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    now : Nat <- Now.nat()
    +r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/log", True{}, J.Json.show(J.Json.obj([J.KV{"since", J.Json.nat(since)}, J.KV{"epoch", J.Str{epoch}}])))
    Far.send(inbox, FarPulled{link, since, Far.code(r), Far.body(r)})

def Far.rpc.job(+client: U32, +id: String, +link: String, +body: String, +home: String, +url: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    now : Nat <- Now.nat()
    +r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/rpc", True{}, body)
    Far.send(inbox, FarReplied{client, id, link, Far.code(r), Far.body(r)})

# a request this hub answers itself with the owning hub's help (core/fview.bend's
# Fv.near: far.view, mech.list); its answer goes to the client as a
# forwarded one does
def Far.near.p(+body: String) -> J.Json:
  J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "p")

def Far.near.run(+meth: String, +home: String, +link: String, +url: String, +p: J.Json) -> IO(String):
  Bool.pick(IO(String), String.eq(meth, "mech.list"), FVS.Fvs.mech(home, link, url, J.Json.get_str(p, "thread", "")),
    FVS.Fvs.sync(home, link, url, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "want", ""), J.Json.get_str(p, "have", "")))

def Far.near.job(+client: U32, +id: String, +link: String, +meth: String, +body: String, +home: String, +url: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +text : String <- Far.near.run(meth, home, link, url, Far.near.p(body))
    Far.send(inbox, FarReplied{client, id, link, 200n, text})

# a tool call of an agent here, for a machine of the owner's: its owner runs
# it (fmcp.bend) and the answer goes straight back to the agent's request
def Far.mcp.job(+id: J.Json, +link: String, +body: String, +home: String, +url: String, +reply: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    now : Nat <- Now.nat()
    +n : String <- BN.Bn.random(4n)
    +r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/rpc", True{}, FR.Fr.nonced(body, n))
    ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, FM.Fm.answer(id, link, Far.code(r), Far.body(r)))
    return Unit{}

# a page of the log's lines (oldest first, numbered after n) into the feed
def Feed.page(xs: List<&2, String>, +n: Nat, +st: M.State, acc: FR.Feed) -> FR.Feed:
  match xs:
    case Nil{}:
      acc
    case +l <> t:
      Feed.page(t, 1n+n, st, FR.Fr.feed.step(FR.Fr.line.parsed(J.Json.parse(l)), st, 1n+n, l, acc))

def Far.feed.more(none: Bool, +xs: List<&2, String>, +at: Nat, +st: M.State, acc: FR.Feed, k: Nat -> FR.Feed -> IO(FR.Feed)) -> IO(FR.Feed):
  match none:
    case True{}:
      IO.pure(FR.Feed, acc)
    case False{}:
      k(Nat.add(at, List.length(&2, String, xs)), Feed.page(xs, at, st, acc))

# (the fiber given back between pages, as F1's slices did: the JS hub runs
# every job on its one fiber)
def Far.feed.got(+xs: List<&2, String>, +at: Nat, +st: M.State, acc: FR.Feed, k: Nat -> FR.Feed -> IO(FR.Feed)) -> IO(FR.Feed):
  do IO<FR.Feed>:
    IO.sleep(1)
    Far.feed.more(List.is_empty(&2, String, xs), xs, at, st, acc, k)

def Far.feed.at(go: Bool, +ls: SO.Lg, +st: M.State, +at: Nat, acc: FR.Feed, k: Nat -> FR.Feed -> IO(FR.Feed)) -> IO(FR.Feed):
  match go:
    case False{}:
      IO.pure(FR.Feed, acc)
    case True{}:
      IO.bind(List<&2, String>, FR.Feed, SO.Sto.lines(ls, at, 500n), xs => Far.feed.got(xs, at, st, acc, k))

def Far.feed.loop(fuel: Nat, +ls: SO.Lg, +st: M.State, +at: Nat, acc: FR.Feed) -> IO(FR.Feed):
  match fuel:
    case 0n:
      IO.pure(FR.Feed, acc)
    case 1n+f:
      Far.feed.at(Nat.is_lt(at, SO.Lg.n(ls)), ls, st, at, acc, a2 => acc2 => Far.feed.loop(f, ls, st, a2, acc2))

# this hub's shared lines (newest first), read from the store 500 lines at a
# time when a linked machine first asks (Far.feed.want): the hub holds no
# log lines; F1 moves the feed onto far_line
def Far.feed.job(+ls: SO.Lg, +st: M.State, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    fd : FR.Feed <- Far.feed.loop(1n+SO.Lg.n(ls), ls, st, 0n, FR.Feed{Nil{}, Map.new(&2, Nat)})
    Far.send(inbox, FarFeed{fd, ""})

# a mirror kept on disk, read back
def Far.load.job(+link: String, +name: String, +home: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +text : String <- Files.text(Far.file(home, link))
    Far.send(inbox, FarLoaded{link, name, text})

def Far.keep.job(+path: String, +text: String) -> IO(Unit):
  do IO<Unit>:
    ok : Bool <- BN.Bf.append(path, text)
    return Unit{}

def Far.out.put.k(p: FR.Fh & Hub, o: FR.Out) -> IO(Hub):
  (fh, h) = p
  IO.pure(Hub, Far.with(h, FR.Fh.out.set(fh, o)))

def Far.out.put(h: Hub, o: FR.Out) -> IO(Hub):
  Far.out.put.k(Far.fh(h), o)

def Far.push.next.k(m: Maybe<&2, FR.Send>, o: FR.Out, h: Hub, +url: String) -> IO(Hub):
  match m h:
    case Some{s} Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.push.job(s, FR.Fh.epoch(BN.Bn.far(Cfg.bn(cfg))), Cfg.home(cfg), url, inbox))
        Far.out.put(Hub{st, f, ls, cl, as, inbox, cfg, note}, o)
    case None{} _:
      Far.out.put(h, o)

# Pushing
# -------

def Far.push.next(r: Maybe<&2, FR.Send> & FR.Out, h: Hub, +url: String) -> IO(Hub):
  (m, o) = r
  Far.push.next.k(m, o, h, url)

def Far.out.of(p: FR.Fh & Hub, +link: String) -> FR.Out & Hub:
  (fh, h) = p
  (FR.Fh.out(fh, link), h)

def Far.push.one.k(r: FR.Out & Hub, +url: String, +from: Nat, +to: Nat, +xs: List<&2, FR.Item>) -> IO(Hub):
  (o, h) = r
  Far.push.next(FR.Out.next(FR.Out.add(o, from, to, xs)), h, url)

def Far.push.go(ps: List<&2, BO.Peer>, h: Hub, +from: Nat, +to: Nat, +xs: List<&2, FR.Item>) -> IO(Hub):
  match ps:
    case Nil{}:
      IO.pure(Hub, h)
    case BO.Peer{+i, _, +u, _, _} <> t:
      IO.bind(Hub, Hub, Far.push.one.k(Far.out.of(Far.fh(h), i), u, from, to, xs), h2 => Far.push.go(t, h2, from, to, xs))

# When discovery first recognizes a linked machine as the owner's, offer
# our current head. Its mirror can pull even if our commits happened
# before discovery, or our hub restarted without an observed away tick.
# Only discovery's newly returned peers are offered; no extra timer.
def Far.offer.job(+link: String, +head: Nat, +epoch: String, +home: String, +url: String) -> IO(Unit):
  do IO<Unit>:
    now : Nat <- Now.nat()
    +name : String <- BN.Bn.name()
    r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/push", True{},
      FR.Fr.with.epoch(FR.Send.body(FR.Send{link, head, head, Nil{}}, name), epoch))
    return Unit{}

# An empty head hint is deliberately outside the item queue: Out.next
# sends only batches with items. It carries no acknowledgement for that
# queue, and cannot reset a mirror without an authoritative pull.
def Far.offer.go(ps: List<&2, BO.Peer>, +head: Nat, +epoch: String, +home: String) -> IO(Unit):
  match ps:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case BO.Peer{+link, _, +url, _, _} <> t:
      do IO<Unit>:
        IO.spawn(Unit, Far.offer.job(link, head, epoch, home, url))
        Far.offer.go(t, head, epoch, home)

def Far.offer(h: Hub, +ps: List<&2, BO.Peer>) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        Far.offer.go(Far.own.go(ps, st, cfg), M.State.seq(st), FR.Fh.epoch(BN.Bn.far(Cfg.bn(cfg))), Cfg.home(cfg))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Far.feed.put(p: FR.Fh & Hub, ls: List<&2, FR.Line>) -> Hub:
  (fh, h) = p
  Far.with(h, FR.Fh.feed.add(fh, ls))

def Far.push.k(empty: Bool, h: Hub, +lines: List<&2, FR.Line>, +from: Nat, +to: Nat) -> IO(Hub):
  match empty h:
    case True{} _:
      IO.pure(Hub, h)
    case False{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Far.push.go(Far.own.go(BO.World.peers(M.State.bots(st)), st, cfg), Far.feed.put(Far.fh(Hub{st, f, ls, cl, as, inbox, cfg, note}), lines), from, to,
        FR.Fr.lines.items(lines, Nil{}))

def Far.push.lines(+lines: List<&2, FR.Line>, h: Hub, +from: Nat, +to: Nat) -> IO(Hub):
  Far.push.k(List.is_empty(&2, FR.Line, lines), h, lines, from, to)

# a commit took the log from `from`: its shared changes join the feed and
# go to each of the owner's linked machines
def Far.history.first.k(first: Bool, fh: FR.Fh, h: Hub, +fresh: List<&2, String>) -> Hub:
  match first:
    case True{}:
      Far.with(h, FR.Fh.epoch.set(fh, Far.history(fresh)))
    case False{}:
      Far.with(h, fh)

def Far.history.first(first: Bool, p: FR.Fh & Hub, +fresh: List<&2, String>) -> Hub:
  (fh, h) = p
  Far.history.first.k(first, fh, h, fresh)

# (and each project's last number moves to its newest change: Fr.catalog)
def Far.lasts.k(p: FR.Fh & Hub, cs: List<&2, M.Change>, +from: Nat, +st: M.State) -> Hub:
  (fh, h) = p
  Far.with(h, FR.Fh.lasts.add(fh, cs, from, st))

def Far.push.real(h: Hub, +cs: List<&2, M.Change>, +fresh: List<&2, String>, +from: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Far.push.lines(FR.Fr.feed.add(cs, List.reverse(&2, String, fresh), from, st, Nil{}), Far.history.first(Nat.is_eq(from, 0n), Far.fh(Far.lasts.k(Far.fh(Hub{st, f, ls, cl, as, inbox, cfg, note}), cs, from, st)), fresh), from, M.State.seq(st))

def Far.push.c(copy: Bool, h: Hub, +cs: List<&2, M.Change>, +fresh: List<&2, String>, +from: Nat) -> IO(Hub):
  match copy:
    case True{}:
      IO.pure(Hub, h)
    case False{}:
      Far.push.real(h, cs, fresh, from)

# a commit's shared changes to the owner's other machines; none from a hub
# on a copied log
def Far.push(h: Hub, +cs: List<&2, M.Change>, +fresh: List<&2, String>, +from: Nat) -> IO(Hub):
  IO.bind(Bool, Hub, P.Proc.copy(), copy => Far.push.c(copy, h, cs, fresh, from))

# where retained work for a link may go now: its address while it is one
# of the owner's machines with sync on (Far.own.id), else nowhere ("": a
# request that fails at once, nothing leaves), so turning sync off stops
# what was queued before as well
def Far.url.now(+st: M.State, +cfg: Cfg, +link: String) -> String:
  Bool.pick(String, Far.own.id(st, cfg, link), Bot.peer.url(st, link), "")

# Live text of threads elsewhere (core/farlive.bend)
# ---------------------------------------------------
# The owner's streamed reply goes to the owner's other machines as it is
# written: never a change, never in the mirror or the log. This hub's
# deltas (the B.Live out, already merged by Delta.merge) for a shared
# thread are queued per own peer (FLV.LQ) and sent as one request at a time,
# `POST /far/push` with `live` and no items; the answer says whether the
# peer has any client (else it is left alone for a few seconds) and which
# threads it wants whole. A peer takes them into its own Live buffers under
# the far id and tells its clients the same delta a local stream sends.

def Far.live.job(+link: String, +at: Nat, xs: List<&2, FLV.LI>, +home: String, +url: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +now : Nat <- Now.nat()
    +name : String <- BN.Bn.name()
    +r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/push", True{}, FLV.FL.body(name, at, xs))
    Far.send(inbox, FarLiveAck{link, Far.code(r), Far.body(r)})

def Far.live.set(h: Hub, +q: FLV.LQ) -> Hub:
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub{st, f, ls, cl, as, inbox, Cfg.with_bn(cfg, BN.Bn.with_fl(Cfg.bn(cfg), FLV.LQs.set(BN.Bn.fl(Cfg.bn(cfg)), q))), note}

def Far.live.q(h: Hub, +link: String) -> FLV.LQ & Hub:
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      (FLV.LQs.get(BN.Bn.fl(Cfg.bn(cfg)), link), Hub{st, f, ls, cl, as, inbox, cfg, note})

# the request that goes now, if one is due: everything owed, snapshots read
# from the buffers
def Far.live.next.k(due: Bool, h: Hub, +q: FLV.LQ, +link: String, +url: String) -> IO(Hub):
  match due h:
    case True{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        +xs : List<&2, FLV.LI> = FLV.FL.fill(FLV.LQ.pend(q), Live.of(cfg))
        IO.spawn(Unit, Far.live.job(link, M.State.seq(st), xs, Cfg.home(cfg), url, inbox))
        return Far.live.set(Hub{st, f, ls, cl, as, inbox, cfg, note}, FLV.LQ.start(q))
    case False{} _:
      IO.pure(Hub, h)

def Far.live.next(h: Hub, +q: FLV.LQ, +link: String, +url: String) -> IO(Hub):
  Far.live.next.k(Bool.and(FLV.LQ.due(q), Bool.not(String.is_empty(url))), Far.live.set(h, q), q, link, url)

def Far.live.add.k(r: FLV.LQ & Hub, +link: String, +url: String, +now: Nat, +th: String, +gen: Nat, +off: Nat, +text: String) -> IO(Hub):
  (q, h) = r
  Far.live.next(h, FLV.LQ.add(q, now, th, gen, off, text), link, url)

def Far.live.go(ps: List<&2, BO.Peer>, h: Hub, +now: Nat, +th: String, +gen: Nat, +off: Nat, +text: String) -> IO(Hub):
  match ps:
    case Nil{}:
      IO.pure(Hub, h)
    case BO.Peer{+i, _, +u, _, _} <> t:
      IO.bind(Hub, Hub, Far.live.add.k(Far.live.q(h, i), i, u, now, th, gen, off, text), h2 => Far.live.go(t, h2, now, th, gen, off, text))

def Far.live.out.k(shared: Bool, h: Hub, +th: String, +off: Nat, +text: String, +now: Nat) -> IO(Hub):
  match shared h:
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Far.live.go(Far.own.go(BO.World.peers(M.State.bots(st)), st, cfg), Hub{st, f, ls, cl, as, inbox, cfg, note}, now, th, FLV.FL.gen(Live.of(cfg), th), off, text)
    case False{} _:
      IO.pure(Hub, h)

# a delta this hub's agent streamed (already in the buffer; `off` is the
# buffer's length before it) goes to the owner's other machines
def Far.live.out(h: Hub, +th: String, +off: Nat, +text: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Far.live.out.k(FR.Fr.thread.shared(st, th), Hub{st, f, ls, cl, as, inbox, cfg, note}, th, off, text, now)

def Far.live.need(ns: List<&2, String>, q: FLV.LQ) -> FLV.LQ:
  match ns:
    case Nil{}:
      q
    case +n <> t:
      Far.live.need(t, FLV.LQ.need(q, n, 0n))

# (a request too big for the peer is no sign it has no client: the next
# delta finds the gap and asks for a snapshot, bounded by FL.snap.cap())
def Far.live.watch(+code: Nat, +o: J.Json) -> Bool:
  Bool.or(Nat.is_eq(code, 413n), Bool.and(Nat.is_eq(code, 200n), J.Json.get_bool(o, "watch", False{})))

def Far.live.ack.k(r: FLV.LQ & Hub, +link: String, +url: String, +watch: Bool, +now: Nat, ns: List<&2, String>) -> IO(Hub):
  (q, h) = r
  Far.live.next(h, Far.live.need(ns, FLV.LQ.done(q, watch, now)), link, url)

# a request's answer: the peer watches or not; the threads it wants whole
def Far.live.ack.o(+o: J.Json, h: Hub, +link: String, +code: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        Far.live.ack.k(Far.live.q(Hub{st, f, ls, cl, as, inbox, cfg, note}, link), link, Far.url.now(st, cfg, link), Far.live.watch(code, o), now, FLV.FL.needs.of(J.Json.elems(J.Json.get_obj(o, "need"))))

def Far.live.ack(h: Hub, +link: String, +code: Nat, +body: String) -> IO(Hub):
  Far.live.ack.o(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), h, link, code)

# the receiving side: deltas for each client
def Far.live.tell(ms: List<&2, FLV.Dm>, +cl: List<&2, Client>) -> IO(List<&2, Client>):
  match ms:
    case Nil{}:
      IO.pure(List<&2, Client>, cl)
    case FLV.Dm{+t, +x} <> r:
      do IO<List<&2, Client>>:
        cl2 : List<&2, Client> <- Wire.delta(cl, B.Delta.msg(t, x))
        Far.live.tell(r, cl2)

# a whole buffer replaced one: every client takes the snapshot
def Far.live.snap(snap: Bool, +cl: List<&2, Client>, bs: List<&2, LV.Buf>) -> IO(List<&2, Client>):
  match snap:
    case True{}:
      Wire.all(cl, LV.Live.msg(bs))
    case False{}:
      IO.pure(List<&2, Client>, cl)

# a batch of durable changes arrived: the live text of what it ends goes
# (the clients clear theirs when they fold it)
def Far.live.ended.k(+es: List<&2, FLV.End>, h: Hub) -> Hub:
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub{st, f, ls, cl, as, inbox, Live.with(cfg, FLV.FL.ends(es, Live.of(cfg))), note}

def Far.live.ended(h: Hub, +link: String, xs: List<&2, FR.Item>) -> Hub:
  Far.live.ended.k(FLV.FL.ended(xs, link), h)

# (an end that left a newer stream's text standing: the clients, which
# cleared it as they folded the end, take the buffers again)
def Far.live.resync(kept: Bool, h: Hub) -> IO(Hub):
  match kept h:
    case True{} Hub{st, f, ls, +cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.all(cl, LV.Live.msg(Live.of(cfg)))
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}
    case False{} _:
      IO.pure(Hub, h)

def Far.live.resync.k(p: Bool & Hub) -> IO(Hub):
  (k, h) = p
  Far.live.resync(k, h)

def Far.live.kept.p(h: Hub, +link: String, xs: List<&2, FR.Item>) -> Bool & Hub:
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      (FLV.FL.kept(FLV.FL.ended(xs, link), Live.of(cfg)), Hub{st, f, ls, cl, as, inbox, cfg, note})

def Far.live.resync.h(h: Hub, +link: String, xs: List<&2, FR.Item>) -> IO(Hub):
  Far.live.resync.k(Far.live.kept.p(h, link, xs))

def Far.live.gone(h: Hub, +link: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +bs : List<&2, LV.Buf> = FLV.FL.drop.link(Live.of(cfg), link ++ "~")
        cl2 : List<&2, Client> <- Wire.all(cl, LV.Live.msg(bs))
        return Hub{st, f, ls, cl2, as, inbox, Live.with(cfg, bs), note}

# a turn that ended here, told to the owner's other machines: each raises
# it on its own desk and phones (core/farlive.bend, "note")
def Far.note.job(+link: String, n: J.Json, +home: String, +url: String) -> IO(Unit):
  do IO<Unit>:
    +now : Nat <- Now.nat()
    +name : String <- BN.Bn.name()
    +r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/push", True{}, FLV.FL.note.body(name, n))
    return Unit{}

def Far.note.go(ps: List<&2, BO.Peer>, +home: String, +n: J.Json) -> IO(Unit):
  match ps:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case BO.Peer{+i, _, +u, _, _} <> t:
      do IO<Unit>:
        IO.spawn(Unit, Far.note.job(i, n, home, u))
        Far.note.go(t, home, n)

# (the title and body are read only when the note goes: an alert with no
# peer to tell reads nothing of its reply)
def Far.note.k(send: Bool, h: Hub, +rec: NO.AlertRec, +thread: String) -> IO(Hub):
  match send h:
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        Far.note.go(Far.own.go(BO.World.peers(M.State.bots(st)), st, cfg), Cfg.home(cfg), FLV.FL.note.json(thread, NO.Alert.kind(rec), NO.Alert.rkey(rec),
          PU.Push.title(B.Hub.thread(st, thread)), PU.Push.alert.body(st, thread, rec)))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      IO.pure(Hub, h)

# an alert was raised here and nobody here is watching it: the owner's
# other machines raise it too (once per machine, under the alert's one key)
def Far.note.out(h: Hub, +rec: NO.AlertRec, +thread: String, seen: Bool) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Far.note.k(Bool.and(Bool.and(Bool.not(seen), Bool.not(List.is_empty(&2, BO.Peer, Far.own.go(BO.World.peers(M.State.bots(st)), st, cfg)))), FR.Fr.thread.shared(st, thread)),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, rec, thread)

# the phones of this hub hear of a far turn's end
def Far.note.push(+home: String, st: M.State, +thread: String, +title: String, +body: String, +akey: String, +inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(List<&2, String>, Unit, PU.Push.note.dead(home, st, thread, title, body, akey, ""), ds => Hub.drop_dead(ds, inbox))

# the desk used last shows it (none when a desk watches the thread)
def Far.note.desk(m: Maybe<&2, U32>, +cl: List<&2, Client>, +farid: String, +o: J.Json) -> IO(List<&2, Client>):
  match m:
    case Some{+c}:
      Wire.to(cl, c, DK.Desk.notify(farid, J.Json.get_str(o, "title", ""), J.Json.get_str(o, "body", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "key", "")))
    case None{}:
      IO.pure(List<&2, Client>, cl)

# a failed push holds its link for a backoff on a timer of its own; the
# timer's message lets the retained batch go again
def Far.retry.arm(holding: Bool, +link: String, +secs: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  match holding:
    case True{}:
      IO.spawn(Unit, Self.post.wait(inbox, FarResend{link}, U32.from_nat(Nat.mul(secs, 1000n))))
    case False{}:
      IO.pure(Unit, Unit{})

def Far.pushed.go(+o: FR.Out, h: Hub, +ok: Bool, +to: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        Far.retry.arm(Bool.not(ok), FR.Out.link(o), FR.Out.delay(Nat.add(FR.Out.fails(o), 1n)), inbox)
        Far.push.next(FR.Out.next(FR.Out.done(o, ok, to)), Hub{st, f, ls, cl, as, inbox, cfg, note}, Far.url.now(st, cfg, FR.Out.link(o)))

def Far.pushed.k(r: FR.Out & Hub, ok: Bool, +to: Nat) -> IO(Hub):
  (o, h) = r
  Far.pushed.go(o, h, ok, to)

# a push ended: the next waiting one goes
def Hub.far.pushed(h: Hub, +link: String, ok: Bool, +to: Nat) -> IO(Hub):
  Far.pushed.k(Far.out.of(Far.fh(h), link), ok, to)

def Far.retry.go(+o: FR.Out, h: Hub) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Far.push.next(FR.Out.next(FR.Out.release(o)), Hub{st, f, ls, cl, as, inbox, cfg, note}, Far.url.now(st, cfg, FR.Out.link(o)))

def Far.retry.k(r: FR.Out & Hub) -> IO(Hub):
  (o, h) = r
  Far.retry.go(o, h)

# the backoff ended: the retained batch goes again
def Hub.far.resend(h: Hub, +link: String) -> IO(Hub):
  Far.retry.k(Far.out.of(Far.fh(h), link))

# Mirrors
# -------

def Far.in.of(p: FR.Fh & Hub, +link: String, +name: String) -> FR.In & Hub:
  (fh, h) = p
  (FR.Fh.in(fh, link, name), h)

def Far.in.put.k(p: FR.Fh & Hub, i: FR.In) -> Hub:
  (fh, h) = p
  Far.with(h, FR.Fh.in.set(fh, i))

def Far.in.put(h: Hub, i: FR.In) -> Hub:
  Far.in.put.k(Far.fh(h), i)

# every client hears a mirror's news
def Far.tell(h: Hub, +text: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.broadcast(Hub{st, f, ls, cl, as, inbox, cfg, note}, FR.Fr.with.epoch(text,
        FR.In.epoch(FR.Fh.in(BN.Bn.far(Cfg.bn(cfg)), J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "link", ""), ""))))

def Far.pull.go(busy: Bool, +fh: FR.Fh, h: Hub, +link: String, +since: Nat) -> IO(Hub):
  match busy h:
    case True{} _:
      IO.pure(Hub, Far.with(h, fh))
    case False{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.pull.job(link, since, FR.In.epoch(FR.Fh.in(fh, link, link)), Cfg.home(cfg), Bot.peer.url(st, link), inbox))
        return Far.with(Hub{st, f, ls, cl, as, inbox, cfg, note}, FR.Fh.pull.on(fh, link))

def Far.pull.f(+fh: FR.Fh, h: Hub, +link: String, +since: Nat) -> IO(Hub):
  Far.pull.go(FR.Fh.pulling(fh, link), fh, h, link, since)

# a pull, unless one is out for the link
def Far.pull.p(p: FR.Fh & Hub, +link: String, +since: Nat) -> IO(Hub):
  (fh, h) = p
  Far.pull.f(fh, h, link, since)

def Far.pull(h: Hub, +link: String, +since: Nat) -> IO(Hub):
  Far.pull.p(Far.fh(h), link, since)

def Far.mirror.head.k(p: FR.Fh & Hub, +link: String) -> Nat & Hub:
  (+fh, h) = p
  (FR.In.head(FR.Fh.in(fh, link, link)), Far.with(h, fh))

def Far.mirror.head(h: Hub, +link: String) -> Nat & Hub:
  Far.mirror.head.k(Far.fh(h), link)

def Far.pull.here.k(r: Nat & Hub, +link: String) -> IO(Hub):
  (hd, h) = r
  Far.pull(h, link, hd)

# a pull from where the mirror is
def Far.pull.here(h: Hub, +link: String) -> IO(Hub):
  Far.pull.here.k(Far.mirror.head(h, link), link)

# Recognition must pull too: the other side's earlier hint may have
# arrived before this side recognized ownership and been refused. Pull
# even a healthy cached mirror, so a quick history replacement is found.
def Far.returned.go(ps: List<&2, BO.Peer>, h: Hub) -> IO(Hub):
  match ps:
    case Nil{}:
      IO.pure(Hub, h)
    case BO.Peer{+link, _, _, _, _} <> t:
      IO.bind(Hub, Hub, Far.pull.here(h, link), h2 => Far.returned.go(t, h2))

def Far.returned(h: Hub, +ps: List<&2, BO.Peer>) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      IO.bind(Hub, Hub, Far.offer(Hub{st, f, ls, cl, as, inbox, cfg, note}, ps),
        h2 => Far.returned.go(Far.own.go(ps, st, cfg), h2))

def Far.store(h: Hub, +link: String, +text: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        BN.Bf.dir(Far.dir(Cfg.home(cfg)))
        Far.keep.job(Far.file(Cfg.home(cfg), link), text)
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Far.took.told(h: Hub, +link: String, ys: List<&2, FR.Item>) -> IO(Hub):
  Far.live.resync.h(h, link, ys)

def Far.took.ended(h: Hub, +link: String, +from: Nat, +hd: Nat, +ys: List<&2, FR.Item>, +name: String) -> IO(Hub):
  IO.bind(Hub, Hub, IO.bind(Hub, Hub, Far.store(h, link, FR.Fr.file.batch(ys, hd)), h2 => Far.tell(h2, FR.Fr.msg(link, name, False{}, from, hd, ys))),
    h3 => Far.took.told(h3, link, ys))

def Far.took.go(none: Bool, moved: Bool, h: Hub, +link: String, +from: Nat, +hd: Nat, +ys: List<&2, FR.Item>, +name: String) -> IO(Hub):
  match none moved:
    case True{} False{}:
      IO.pure(Hub, h)
    case True{} True{}:
      Far.tell(h, FR.Fr.msg(link, name, False{}, from, hd, Nil{}))
    case False{} _:
      Far.took.ended(Far.live.ended(h, link, ys), link, from, hd, ys, name)

# a batch taken into the mirror: kept, told, and on a gap the rest pulled
def Far.took.k(g: FR.Got, +i: FR.In, h: Hub, +since: Nat, +to: Nat, +xs: List<&2, FR.Item>, +name: String, moved: Bool) -> IO(Hub):
  match g:
    case FR.Got{+_, +_, True{}}:
      Far.pull(Far.in.put(h, FR.In.name.set(i, name)), FR.In.link(i), FR.In.head(i))
    case FR.Got{+hd, ys, False{}}:
      Far.took.go(List.is_empty(&2, FR.Item, ys), moved, Far.in.put(h, FR.In.name.set(FR.In.put(i, since, to, xs), name)), FR.In.link(i), FR.In.head(i), hd, ys, name)

def Far.take.i(+i: FR.In, h: Hub, +since: Nat, +to: Nat, +xs: List<&2, FR.Item>, +name: String) -> IO(Hub):
  Far.took.k(FR.In.got(i, since, to, xs), i, h, since, to, xs, Bool.pick(String, String.is_empty(name), FR.In.name.of(i), name), FR.In.away(i))

def Far.take.p(r: FR.In & Hub, +since: Nat, +to: Nat, +xs: List<&2, FR.Item>, +name: String) -> IO(Hub):
  (i, h) = r
  Far.take.i(i, h, since, to, xs, name)

# a batch from a link (a push, or a page pulled)
def Far.take(h: Hub, +link: String, +since: Nat, +to: Nat, +xs: List<&2, FR.Item>, +name: String) -> IO(Hub):
  Far.take.p(Far.in.of(Far.fh(h), link, name), since, to, xs, name)

def Far.reset.put(+i: FR.In, h: Hub) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        BN.Bf.dir(Far.dir(Cfg.home(cfg)))
        ok : Bool <- BN.Bf.secret(Far.file(Cfg.home(cfg), FR.In.link(i)), J.Json.show(J.Json.obj([J.KV{"epoch", J.Str{FR.In.epoch(i)}}])) ++ "\n")
        h2 : Hub <- Far.live.gone(Hub{st, f, ls, cl, as, inbox, cfg, note}, FR.In.link(i))
        Far.tell(Far.in.put(h2, i), FR.In.msg(i))

def Far.reset.k(reset: Bool, +i: FR.In, h: Hub, +epoch: String) -> IO(Hub):
  match reset:
    case True{}:
      Far.reset.put(FR.In.epoch.set(FR.In.new(FR.In.link(i), FR.In.name.of(i)), epoch), h)
    case False{}:
      IO.pure(Hub, h)

def Far.reset.in(r: FR.In & Hub, +epoch: String) -> IO(Hub):
  (+i, h) = r
  Far.reset.k(FR.Fr.epoch.changed(FR.In.epoch(i), epoch), i, h, epoch)

def Far.take.epoch(h: Hub, +link: String, +epoch: String, +since: Nat, +to: Nat, +xs: List<&2, FR.Item>, +name: String) -> IO(Hub):
  IO.bind(Hub, Hub, Far.reset.in(Far.in.of(Far.fh(h), link, name), epoch),
    h2 => Far.take(h2, link, since, to, xs, name))

def Far.away.go(was: Bool, +i: FR.In, h: Hub) -> IO(Hub):
  match was:
    case True{}:
      IO.pure(Hub, Far.in.put(h, i))
    case False{}:
      IO.bind(Hub, Hub, Far.live.gone(Far.in.put(h, FR.In.away.set(i, True{})), FR.In.link(i)), h2 => Far.tell(h2, FR.Fr.msg(FR.In.link(i), FR.In.name.of(i), True{}, FR.In.head(i), FR.In.head(i), Nil{})))

def Far.away.i(+i: FR.In, h: Hub) -> IO(Hub):
  Far.away.go(FR.In.away(i), i, h)

# a link that does not answer: its mirror shows, away
def Far.away.p(r: FR.In & Hub) -> IO(Hub):
  (i, h) = r
  Far.away.i(i, h)

def Far.away(h: Hub, +link: String) -> IO(Hub):
  Far.away.p(Far.in.of(Far.fh(h), link, link))

def Far.feed.of(p: FR.Fh & Hub) -> Maybe<&2, List<&2, FR.Line>> & Hub:
  (+fh, h) = p
  (FR.Fh.feed(fh), Far.with(h, fh))

# the client a linked machine's request acts as: none (its answer goes
# back over HTTP, not to a client)
def Far.client() -> U32:
  4000000000

def Far.more(more: Bool, h: Hub, +link: String) -> IO(Hub):
  match more:
    case True{}:
      Far.pull.here(h, link)
    case False{}:
      IO.pure(Hub, h)

# Messages back from the jobs
# ---------------------------

# A pulled page's catalog (every project the owner shares, Fr.catalog):
# kept for clients that join later, and told to every client now, beside
# where the mirror is, so each lists the link's projects at once and
# marks those still catching up
def Far.cat.tell(r: FR.In & Hub, +text: String) -> IO(Hub):
  (+i, h) = r
  Far.tell(h, FR.Fr.with.cat(FR.Fr.msg(FR.In.link(i), FR.In.name.of(i), FR.In.away(i), FR.In.head(i), FR.In.head(i), Nil{}), text))

def Far.cat.put(p: FR.Fh & Hub, +link: String, +text: String) -> Hub:
  (fh, h) = p
  Far.with(h, FR.Fh.cat.set(fh, link, text))

def Far.cat.k(none: Bool, h: Hub, +link: String, +text: String) -> IO(Hub):
  match none:
    case True{}:
      IO.pure(Hub, h)
    case False{}:
      Far.cat.tell(Far.in.of(Far.fh(Far.cat.put(Far.fh(h), link, text)), link, link), text)

def Far.cat.got(h: Hub, +link: String, +cat: J.Json) -> IO(Hub):
  Far.cat.k(List.is_empty(&2, J.Json, J.Json.elems(cat)), h, link, J.Json.show(cat))

def Far.page.k(ok: Bool, m: Maybe<&2, J.Json>, h: Hub, +link: String, +since: Nat) -> IO(Hub):
  match ok m:
    case True{} Some{+o}:
      IO.bind(Hub, Hub, Far.take.epoch(h, link, J.Json.get_str(o, "epoch", ""), J.Json.get_nat(o, "since", since), J.Json.get_nat(o, "head", since), FR.Fr.items.of(J.Json.elems(J.Json.get_obj(o, "items"))), ""),
        h2 => IO.bind(Hub, Hub, Far.cat.got(h2, link, J.Json.get_obj(o, "catalog")),
          h3 => Far.more(J.Json.get_bool(o, "more", False{}), h3, link)))
    case _ _:
      Far.away(h, link)

def Far.pull.done(p: FR.Fh & Hub, +link: String) -> Hub:
  (fh, h) = p
  Far.with(h, FR.Fh.pull.off(fh, link))

# a page came back (or the pull failed: the link is away until it answers)
def Far.retry.job(+link: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(2000)
    Far.send(inbox, FarRetry{link})

def Far.pulled.k(busy: Bool, h: Hub, +link: String, +since: Nat, +code: Nat, +body: String) -> IO(Hub):
  match busy h:
    case True{} Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.retry.job(link, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      Far.page.k(Nat.is_eq(code, 200n), J.Json.parse(body), h, link, since)

# a page came back (or the pull failed: the link is away until it answers;
# a hub still making its feed is asked again in a moment)
def Hub.far.pulled(h: Hub, +link: String, +since: Nat, +code: Nat, +body: String) -> IO(Hub):
  Far.pulled.k(Nat.is_eq(code, 503n), Far.pull.done(Far.fh(h), link), link, since, code, body)

# a request sent on for a client came back: its answer goes to the client
def Hub.far.replied.k(h: Hub, +client: U32, +id: String, +link: String, +code: Nat, +body: String) -> IO(Hub):
  Hub.push(h, client, Bool.pick(String, Nat.is_eq(code, 200n), FR.Fr.reply.on(Maybe.default(&2, J.Json, J.Json.parse(body), J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{"invalid reply from that machine"}}, J.KV{"id", J.Num{"0"}}])), id, link),
    J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{False{}}},
      J.KV{"error", J.Str{Bool.pick(String, Nat.is_eq(code, 0n), "that machine is away", "that machine did not take it: " ++ String.take(body, 120n))}}]))))

def Hub.far.replied(h: Hub, +client: U32, +id: String, +link: String, +code: Nat, +body: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.far.replied.k(Hub{st, f, ls, cl, as, inbox, cfg, note}, client, id, link, code,
        Bool.pick(String, Nat.is_eq(code, 200n), FV.Fv.fix(Cfg.home(cfg), link, body), body))

# (the feed leaves the history epoch alone: Far.start installed it, and
# the startup feed's "" would reset it)
def Far.fed.k(p: FR.Fh & Hub, ls: FR.Feed, epoch: String) -> Hub:
  (fh, h) = p
  Far.with(h, FR.Fh.feed.ready(fh, ls))

def Hub.far.fed(h: Hub, ls: FR.Feed, +epoch: String) -> IO(Hub):
  IO.pure(Hub, Far.fed.k(Far.fh(h), ls, epoch))

# a mirror read back from disk: shown (away) until its machine answers,
# then pulled from where it ends
def Far.loaded.put(+i: FR.In, h: Hub) -> IO(Hub):
  do IO<Hub>:
    h2 : Hub <- Far.tell(Far.in.put(h, i), FR.In.msg(i))
    Far.pull(h2, FR.In.link(i), FR.In.head(i))

def Far.loaded.choose(r: FR.In & Hub, disk: FR.In) -> IO(Hub):
  (live, h) = r
  Far.loaded.put(FR.In.loaded(live, disk), h)

def Hub.far.loaded(h: Hub, +link: String, +name: String, +text: String) -> IO(Hub):
  Far.loaded.choose(Far.in.of(Far.fh(h), link, name), FR.Fr.file.read(text, link, name))

# (one already online is pulled too, through the same coalesced pull: a
# lost last push is so repaired within the minute)
def Far.here.was.k(away: Bool, h: Hub, +link: String) -> IO(Hub):
  match away:
    case True{}:
      Far.pull.here(h, link)
    case False{}:
      Far.pull.here(h, link)

def Far.here.was(r: FR.In & Hub, +link: String) -> IO(Hub):
  (i, h) = r
  Far.here.was.k(FR.In.away(i), h, link)

# The minute: linked machines that answered pull what they lack (a mirror
# that was away, or none yet); those that did not are away
def Far.here.one(ok: Bool, h: Hub, +link: String) -> IO(Hub):
  match ok:
    case True{}:
      Far.here.was(Far.in.of(Far.fh(h), link, link), link)
    case False{}:
      Far.away(h, link)

def Far.here.go(ps: List<&2, BO.Peer>, h: Hub, +ok: List<&2, String>) -> IO(Hub):
  match ps:
    case Nil{}:
      IO.pure(Hub, h)
    case BO.Peer{+i, _, _, _, _} <> t:
      IO.bind(Hub, Hub, Far.here.one(FR.Fr.may.go(ok, i), h, i), h2 => Far.here.go(t, h2, ok))

def Far.live(ps: List<&2, BO.Peer>) -> List<&2, BO.Peer>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Far.keep(Bot.peer.live.k(p), p, Far.live(t))

def Hub.far.here(h: Hub, +answered: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Far.here.go(Far.live(BO.World.peers(M.State.bots(st))), Hub{st, f, ls, cl, as, inbox, cfg, note}, String.lines(answered))

# At start: this hub's feed is made off the hub; each link's mirror is
# read back from disk (then pulled)
def Far.start.go(ps: List<&2, BO.Peer>, +home: String, +inbox: Chan(Msg)) -> IO(Unit):
  match ps:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case BO.Peer{+i, +n, _, _, _} <> t:
      do IO<Unit>:
        IO.spawn(Unit, Far.load.job(i, n, home, inbox))
        Far.start.go(t, home, inbox)

def Far.start.epoch(p: FR.Fh & Hub, +ls: SO.Lg, +seq: Nat) -> Hub:
  (fh, h) = p
  Far.with(h, FR.Fh.epoch.set(fh, Far.history.at(ls, seq)))

def Far.start(h: Hub) -> IO(Hub):
  match h:
    case Hub{+st, f, +ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        Far.start.go(Far.live(BO.World.peers(M.State.bots(st))), Cfg.home(cfg), inbox)
        return Far.start.epoch(Far.fh(Hub{st, f, ls, cl, as, inbox, cfg, note}), ls, M.State.seq(st))

def Far.mirrors(+cfg: Cfg) -> List<&2, FR.In>:
  FR.Fh.ins(BN.Bn.far(Cfg.bn(cfg)))

def Hub.announce(r: Hub & List<&2, String>) -> IO(Hub):
  (h, fresh) = r
  match h fresh:
    case _ Nil{}:
      IO.pure(Hub, h)
    case Hub{st, f, +ls, cl, as, +inbox, +cfg, note} +l <> +t:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.changes(cl, Hub.changes_msg(List.reverse(&2, String, l <> t)), SO.Lg.n(ls), List.length(&2, String, l <> t),
          List.length(&2, FR.In, Far.mirrors(cfg)), inbox)
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

def Hub.spawned.fail(e: U32 & String, h: Hub, +thread: String) -> IO(Hub):
  (_, +msg) = e
  do IO<Hub>:
    +now : Nat <- Now.nat()
    Log.info("could not start agent: " ++ msg)
    r : Hub & List<&2, String> <- Hub.commit([M.ActivityLogged{thread, "spawn-" ++ Nat.show(now), "error", "spawn", "could not start claude: " ++ msg, now}, M.TurnChanged{thread, M.Failed{}, now}], h, Nil{})
    Hub.announce(r)

def Hub.spawned(r: Result<&1, &1, U32 & String, U32 & Socket>, h: Hub, +thread: String, +key: String, +gen: String) -> IO(Hub):
  match r h:
    case Done{started} Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      Hub.spawned.ok(started, Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, key, gen)
    case Fail{e} _:
      Hub.spawned.fail(e, h, thread)

# A Claude process is started by a job of its own (its files and keys are
# read there, never on the hub's loop). The agent is registered first, with
# a placeholder: pid 0 (never signalled), the generation's run and the
# input channel the process's writer will read, so a line for the thread
# meanwhile waits in it. ClaudePid gives the pid (a stale generation's
# process is stopped), ClaudeFail ends the turn as a process that could
# not start does.
def Claude.fail(+thread: String, +gen: String, +msg: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, ClaudeFail{thread, gen, msg})
    return Unit{}

def Claude.duped(r: Socket & Result<&1, &1, U32 & String, Socket>, +pid: U32, +thread: String, +gen: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  (rd1, res) = r
  match res:
    case Done{wr}:
      do IO<Unit>:
        rd0 : Socket <- AIO.Adopt.keep(thread, pid, rd1)
        IO.spawn(Unit, Agent.writer(Fuel.forever(), wr, input))
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, ClaudePid{thread, pid, gen})
        rd : Socket <- IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Socket, H.Sock.dup(rd0), d => Agent.watch.start(d, pid, thread, gen, inbox))
        Agent.reader(Fuel.forever(), rd, U.Lines.new(), pid, thread, gen, B.Tag.of(thread, pid), inbox)
    case Fail{_}:
      do IO<Unit>:
        Proc.kill(pid, 15)
        IO.spawn(Unit, Agent.end(rd1, pid, thread, gen, inbox))
        Claude.fail(thread, gen, "its output could not be shared", inbox)

def Claude.started.ok(started: U32 & Socket, +thread: String, +gen: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  (+pid, s) = started
  IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Unit, H.Sock.dup(s), r => Claude.duped(r, pid, thread, gen, input, inbox))

# why a process could not start, naming its folder when that is gone (a
# thread whose worktree was removed: posix_spawn's chdir answers only
# "No such file or directory", which reads as a missing program)
def Spawn.why(st: Result<&1, &1, U32 & String, U32 & U32 & U32>, +cwd: String, +msg: String) -> String:
  match st:
    case Done{_}:
      msg
    case Fail{_}:
      Bool.pick(String, String.is_empty(cwd), msg, "its folder " ++ cwd ++ " is gone (" ++ msg ++ ")")

def Claude.started.fail(e: U32 & String, +thread: String, +gen: String, +cwd: String, +inbox: Chan(Msg)) -> IO(Unit):
  (_, +msg) = e
  IO.bind(Result<&1, &1, U32 & String, U32 & U32 & U32>, Unit, H.FS.stat(cwd), st => Claude.fail(thread, gen, Spawn.why(st, cwd, msg), inbox))

def Claude.started(r: Result<&1, &1, U32 & String, U32 & Socket>, +thread: String, +gen: String, +cwd: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case Done{started}:
      Claude.started.ok(started, thread, gen, input, inbox)
    case Fail{e}:
      Claude.started.fail(e, thread, gen, cwd, inbox)

def Claude.job(+cfg: Cfg, +st: M.State, +thread: String, +cwd: String, +model: String, +session: String, +r: M.RuntimeMode, +i: M.InteractionMode, +effort: String, +gen: String, +input: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +fs : K.Files <- K.Files.read(cwd)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AutoView{thread, fs})
    +keys : String <- AKS.Ak.prompt(Cfg.home(cfg))
    Keyring.gate("claude", IO.bind(Result<&1, &1, U32 & String, U32 & Socket>, Unit, H.Proc.spawn("env", "MCP_TOOL_TIMEOUT=86400000" <> "CLAUDE_CODE_ENABLE_TODO_TOOLS=1" <> "claude" <> Hub.agent_args(cfg, thread, model, session, r, i, Hub.prompt(cfg, st, thread, cwd, fs, keys), effort, gen), cwd, 3),
      res => Claude.started(res, thread, gen, cwd, input, inbox)), Claude.fail(thread, gen, KR.Keyring.refusal("claude"), inbox))

def Agents.pid.k(same: Bool, a: Agent, +pid: U32) -> Agent:
  match same a:
    case True{} Agent{t, i, _, k, rs}:
      Agent{t, i, pid, k, rs}
    case False{} _:
      a

# the thread's agent with its pid set
def Agents.pid(as: List<&2, Agent>, +th: String, +pid: U32) -> List<&2, Agent>:
  match as:
    case Nil{}:
      Nil{}
    case Agent{+t, i, p, k, rs} <> rest:
      Agents.pid.k(String.eq(t, th), Agent{t, i, p, k, rs}, pid) <> Agents.pid(rest, th, pid)

def Hub.claude_pid.k(cur: Bool, h: Hub, +thread: String, +pid: U32) -> IO(Hub):
  match cur h:
    case True{} Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      IO.pure(Hub, Hub{st, f, ls, cl, Agents.pid(as, thread, pid), inbox, cfg, note})
    case False{} _:
      do IO<Hub>:
        Proc.kill(pid, 15)
        return h

# a Claude process started: the thread's only if its agent is still of
# its generation (B.Exit.current); any other is stopped at once
def Hub.claude_pid(h: Hub, +thread: String, +pid: U32, +gen: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.claude_pid.k(B.Exit.current(gen, Agents.gen(as, thread)), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, pid)

def Hub.claude_fail.k(cur: Bool, h: Hub, +thread: String, +msg: String) -> IO(Hub):
  match cur h:
    case True{} Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      do IO<Hub>:
        Hub.agent_stop(Agents.get(as, thread))
        Hub.spawned.fail((0, msg), Hub{st, f, ls, cl, Agents.drop(as, thread), inbox, cfg, note}, thread)
    case False{} _:
      IO.pure(Hub, h)

def Hub.claude_fail(h: Hub, +thread: String, +gen: String, +msg: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.claude_fail.k(B.Exit.current(gen, Agents.gen(as, thread)), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, msg)

def Hub.start.spawn(m: Maybe<&2, Agent>, h: Hub, +thread: String, +cwd: String, +model: String, +session: String, +r: M.RuntimeMode, +i: M.InteractionMode, +effort: String, +gen: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, +as, +inbox, +cfg, note}:
      do IO<Hub>:
        Hub.agent_stop(m)
        +now : Nat <- Now.nat()
        +input : Chan(String) <- Chan.new(String, 256)
        IO.spawn(Unit, Claude.job(cfg, st, thread, cwd, model, session, r, i, effort, gen, input, inbox))
        return Hub{st, f, ls, cl, Agent{thread, input, 0, Agent.key(model, r, i, effort), [ARun.new.h(gen, now)]} <> Agents.drop(as, thread), inbox, cfg, note}

def Hub.start.r(r: Hub & List<&2, String>, m: Maybe<&2, Agent>, +thread: String, +cwd: String, +model: String, +session: String, +rt: M.RuntimeMode, +i: M.InteractionMode, +effort: String, +gen: String) -> IO(Hub):
  IO.bind(Hub, Hub, Hub.announce(r), h => Hub.start.spawn(m, h, thread, cwd, model, session, rt, i, effort, gen))

def Hub.start.go(same: Bool, +m: Maybe<&2, Agent>, h: Hub, +thread: String, +cwd: String, +model: String, +session: String, +r: M.RuntimeMode, +i: M.InteractionMode, +effort: String) -> IO(Hub):
  match same h:
    case True{} _:
      IO.pure(Hub, h)
    case False{} Hub{+st, f, ls, +cl, +as, inbox, +cfg, note}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        cr : Hub & List<&2, String> <- Hub.commit(Hub.replaced(Agent.is(m), st, now, thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, Nil{})
        Hub.start.r(cr, m, thread, cwd, model, session, r, i, effort, Hub.gen(st, now))

def Hub.start.k(+m: Maybe<&2, Agent>, h: Hub, +thread: String, +cwd: String, +model: String, +session: String, +r: M.RuntimeMode, +i: M.InteractionMode, +effort: String) -> IO(Hub):
  Hub.start.go(Agent.same(m, Agent.key(model, r, i, effort)), m, h, thread, cwd, model, session, r, i, effort)

def Hub.autoview(h: Hub, +thread: String, fs: K.Files) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Hub.auto(cl, thread, fs)
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

# A user line naming `$skills` goes to the thread's agent through the same
# job as one with images (Media.claude, which scans the skills off the hub
# and answers AgentWrite): the command is taken now (Claude.prep), the ones
# after it wait behind it
def Line.skills.u(go: Bool, +o: J.Json, h: Hub, +thread: String, +line: String, +cwd: String) -> IO(Hub):
  match go h:
    case True{} Hub{st, f, ls, cl, +as, +inbox, cfg, note}:
      do IO<Hub>:
        +u : String = J.Json.get_str(o, "uuid", "")
        IO.spawn(Unit, Media.claude(inbox, thread, cwd, Mention.user_text(o), u, Agents.gen(as, thread)))
        return Claude.prep(True{}, Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, u)
    case False{} _:
      Claude.put(h, thread, line)

def Line.skills.k(m: Maybe<&2, J.Json>, h: Hub, +thread: String, +line: String, +cwd: String) -> IO(Hub):
  match m:
    case Some{+o}:
      Line.skills.u(Bool.and(String.eq(J.Json.get_str(o, "type", ""), "user"), Bool.not(String.is_empty(J.Json.get_str(o, "uuid", "")))), o, h, thread, line, cwd)
    case None{}:
      Claude.put(h, thread, line)

def Line.skills(has: Bool, h: Hub, +thread: String, +line: String, +cwd: String) -> IO(Hub):
  match has:
    case True{}:
      Line.skills.k(J.Json.parse(line), h, thread, line, cwd)
    case False{}:
      Claude.put(h, thread, line)

def Far.view.cwd(+st: M.State, +thread: String) -> String:
  B.Hub.cwd(st, B.Hub.thread(st, thread))

# viewer_show on a thread the owner's other machines mirror: the owner
# tells them (view.show), each brings the file into its cache and shows it
# to its clients as a view message of the thread there.
def Far.shown.job(+link: String, +url: String, +home: String, +thread: String, +rel: String, +kind: String) -> IO(Unit):
  do IO<Unit>:
    now : Nat <- Now.nat()
    +n : String <- BN.Bn.random(4n)
    r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/rpc", True{},
      J.Json.show(J.Json.obj([J.KV{"id", J.Num{"0"}}, J.KV{"m", J.Str{"view.show"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{thread}}, J.KV{"path", J.Str{rel}}, J.KV{"kind", J.Str{kind}}, J.KV{"n", J.Str{n}}])}])))
    return Unit{}

def Far.shown.go(ps: List<&2, BO.Peer>, +home: String, +thread: String, +rel: String, +kind: String) -> IO(Unit):
  match ps:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case BO.Peer{+link, _, +url, _, _} <> t:
      do IO<Unit>:
        IO.spawn(Unit, Far.shown.job(link, url, home, thread, rel, kind))
        Far.shown.go(t, home, thread, rel, kind)

# the file's path from the thread's folder ("" when it is outside it)
def Far.shown.rel(+cwd: String, +path: String) -> String:
  Bool.pick(String, String.starts_with(path, "/"), FV.Fv.rel(cwd, path), path)

def Far.shown.k(ok: Bool, +st: M.State, +cfg: Cfg, +thread: String, +rel: String, +kind: String) -> IO(Unit):
  match ok:
    case True{}:
      Far.shown.go(Far.own.go(BO.World.peers(M.State.bots(st)), st, cfg), Cfg.home(cfg), thread, rel, kind)
    case False{}:
      IO.pure(Unit, Unit{})

def Far.shown.o(m: Maybe<&2, J.Json>, +st: M.State, +cfg: Cfg) -> IO(Unit):
  match m:
    case Some{+o}:
      Far.shown.k(Bool.and(FR.Fr.thread.shared(st, J.Json.get_str(o, "thread", "")),
        Bool.and(Bool.not(String.is_empty(J.Json.get_str(o, "path", ""))), FV.Fv.file.ok(Far.shown.rel(Far.view.cwd(st, J.Json.get_str(o, "thread", "")), J.Json.get_str(o, "path", ""))))),
        st, cfg, J.Json.get_str(o, "thread", ""), Far.shown.rel(Far.view.cwd(st, J.Json.get_str(o, "thread", "")), J.Json.get_str(o, "path", "")), J.Json.get_str(o, "kind", ""))
    case None{}:
      IO.pure(Unit, Unit{})

# a view message the hub broadcast (viewer_show), told on
def Far.shown(+st: M.State, +cfg: Cfg, +text: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.starts_with(text, "{\"t\":\"view\""), Far.shown.o(J.Json.parse(text), st, cfg), IO.pure(Unit, Unit{}))

# Reports to launchers elsewhere
# ------------------------------
# One outbox line (hub.bend's Lr.far.line) sent to the launcher's machine as
# launch_report (fmcp.bend's Fm.report): "" when that machine took it. A
# machine that did not keeps its later lines back too, so they arrive in
# order when it is back (the minute tick sends the outbox again).
def Lr.far.text(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{J.Str{+t}}:
      t
    case _:
      ""

def Lr.far.body(+link: String, +l: String) -> String:
  FM.Fm.request(link, "launch_report", FR.Fr.id(link, M.Bg.field(l, 2n)),
    J.Json.obj([J.KV{"threadId", J.Str{M.Bg.field(l, 1n)}}, J.KV{"key", J.Str{M.Bg.field(l, 0n)}}, J.KV{"text", J.Str{Lr.far.text(J.Json.parse(M.Bg.field(l, 3n)))}}]))

def Lr.far.res(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, J.Json.get_bool(J.Json.get_obj(o, "mcp"), "ok", False{}), "", "not taken")
    case None{}:
      "not understood"

def Lr.far.ask(+home: String, +url: String, +link: String, +body: String) -> IO(String):
  do IO<String>:
    now : Nat <- Now.nat()
    +n : String <- BN.Bn.random(4n)
    +r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/rpc", True{}, FR.Fr.nonced(body, n))
    return Bool.pick(String, Nat.is_eq(Far.code(r), 200n), Lr.far.res(J.Json.parse(Far.body(r))), "away")

def Lr.far.one(+l: String, +st: M.State, +cfg: Cfg) -> IO(String):
  +link = FR.Fr.link(M.Bg.field(l, 1n))
  Bool.pick(IO(String), Far.own.id(st, cfg, link), Lr.far.ask(Cfg.home(cfg), Bot.peer.url(st, link), link, Lr.far.body(link, l)), IO.pure(String, "sync with that machine is off here"))

def Lr.far.held(bad: List<&2, String>, +link: String) -> Bool:
  match bad:
    case Nil{}:
      False{}
    case +b <> t:
      Bool.or(String.eq(b, link), Lr.far.held(t, link))

def Lr.far.done(+err: String, +key: String, +inbox: Chan(Msg)) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(err), Far.send(inbox, LaunchSent{key}), IO.pure(Unit, Unit{}))

def Lr.far.try(held: Bool, +l: String, +st: M.State, +cfg: Cfg) -> IO(String):
  match held:
    case True{}:
      IO.pure(String, "held")
    case False{}:
      Lr.far.one(l, st, cfg)

def Lr.far.run(ls: List<&2, String>, +bad: List<&2, String>, +st: M.State, +cfg: Cfg, +inbox: Chan(Msg)) -> IO(Unit):
  match ls:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +l <> t:
      do IO<Unit>:
        +err : String <- Lr.far.try(Lr.far.held(bad, FR.Fr.link(M.Bg.field(l, 1n))), l, st, cfg)
        Lr.far.done(err, M.Bg.field(l, 0n), inbox)
        Lr.far.run(t, Bool.pick(List<&2, String>, String.is_empty(err), bad, FR.Fr.link(M.Bg.field(l, 1n)) <> bad), st, cfg, inbox)

# One bounded job per distinct target, outside the hub loop.
def WakeJob.pr.sent(+r: P.Ran, +target: String, +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case P.Ran{0, out}:
      Self.post(inbox, WakeSeen{"pr", target, out})
    case _:
      IO.pure(Unit, Unit{})

def WakeJob.prs(xs: List<&2, String>, +cwd: String, +inbox: Chan(Msg)) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +h <> t:
      do IO<Unit>:
        r : P.Ran <- WJ.WakeJob.pr(h, cwd)
        WakeJob.pr.sent(r, h, inbox)
        WakeJob.prs(t, cwd, inbox)

def WakeJob.files(xs: List<&2, String>, +inbox: Chan(Msg)) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +h <> t:
      do IO<Unit>:
        stamp : String <- WJ.WakeJob.file(h)
        Self.post(inbox, WakeSeen{"file", h, stamp})
        WakeJob.files(t, inbox)

# ---- mail 2b ----
# Appends and their normal event commits are serialized by Hub.outs.
# A header already present skips body IO, preventing conflicting retry
# records when the conversation metadata changed after the first append.
def MailJob.failed(+reason: String) -> IO(Unit):
  do IO<Unit>:
    PS.Perf.add("mail.append_fail", 1)
    PS.Log.error("mail", reason)
    return Unit{}

def MailJob.append.failed(h: Hub, reason: String) -> IO(Hub):
  do IO<Hub>:
    MailJob.failed(reason)
    return h

# ---- mail 2c: body IO, then the same Hub.post as thread_launch ----
def MailJob.routed(h: Hub, +body: MB.Body) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        Self.post(inbox, MailRouted{body})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def MailJob.route(h: Hub, cs: List<&2, M.Change>, fresh: List<&2, String>, from: Nat, body: MB.Body) -> IO(Hub):
  IO.bind(Hub, Hub, Far.push(h, cs, fresh, from), h2 => MailJob.routed(h2, body))

def MailJob.append.committed(r: Hub & Wrote, +cs: List<&2, M.Change>, +from: Nat, +body: MB.Body) -> IO(Hub):
  (h, w) = r
  match w:
    case Wrote{+fresh, True{}, _}:
      IO.bind(Hub, Hub, Hub.announce((h, fresh)), h2 => MailJob.route(h2, cs, fresh, from, body))
    case Wrote{_, False{}, err}:
      do IO<Hub>:
        PS.Log.error("mail", "header commit failed: " ++ err)
        PS.Perf.add("hub.write_fail", 1)
        return h

def MailJob.append.result(r: MST.StoreResult<Bool>, +changes: List<&2, M.Change>, +from: Nat, +body: MB.Body, h: Hub) -> IO(Hub):
  match r:
    case MST.Stored{_}: IO.bind(Hub & Wrote, Hub, Hub.commit.w(changes, h), got => MailJob.append.committed(got, changes, from, body))
    case MST.Missing{path}: MailJob.append.failed(h, "append missing: " ++ path)
    case MST.StoreError{path, reason}: MailJob.append.failed(h, path ++ ": " ++ reason)

def MailJob.append.fresh(seen: Bool, +body: MB.Body, home: String, changes: List<&2, M.Change>, from: Nat, h: Hub) -> IO(Hub):
  match seen:
    case True{}: IO.pure(Hub, h)
    case False{}:
      do IO<Hub>:
        r : MST.StoreResult<Bool> <- MST.MailStore.append(home, body)
        MailJob.append.result(r, changes, from, body, h)

def MailJob.append.parsed(body: Maybe<&2, MB.Body>, +home: String, changes: List<&2, M.Change>, h: Hub) -> IO(Hub):
  match body h:
    case None{} kept: MailJob.append.failed(kept, "invalid MailAppend body")
    case Some{+body} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      MailJob.append.fresh(ML.Box.has(M.State.mail(st), ML.Mail.id(MB.Body.mail(body))), body, home, changes, M.State.seq(st), Hub{st, f, ls, cl, as, inbox, cfg, note})

def MailJob.recover.result(+r: MST.StoreResult<List<&2, M.Change>>, +inbox: Chan(Msg)) -> IO(Unit):
  match r:
    case MST.Stored{+changes}: Self.post(inbox, Commit{changes})
    case MST.Missing{_}: IO.pure(Unit, Unit{})
    case MST.StoreError{+path, +reason}: PS.Log.error("mail.recover", path ++ ": " ++ reason)

def MailJob.recover(+home: String, +box: ML.Box, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    r : MST.StoreResult<List<&2, M.Change>> <- MST.MailStore.recover(home, box)
    MailJob.recover.result(r, inbox)

def MailJob.page.more(+r: MST.StoreResult<List<&2, MB.Body>>, +s: M.State, +conv: String, +bot: String, +bs: List<&2, MB.Body>) -> MST.StoreResult<J.Json>:
  match r:
    case MST.Stored{+extra}: MST.Stored{B.Mail.read.json(s, conv, bot, bs, Bool.not(List.is_empty(&2, MB.Body, extra)))}
    case MST.Missing{_}: MST.Stored{B.Mail.read.json(s, conv, bot, bs, False{})}
    case MST.StoreError{+p, +e}: MST.StoreError{p, e}

def MailJob.page.full(+full: Bool, +home: String, +s: M.State, +conv: String, +bot: String, +bs: List<&2, MB.Body>) -> IO(MST.StoreResult<J.Json>):
  match full:
    case False{}: IO.pure(MST.StoreResult<J.Json>, MST.Stored{B.Mail.read.json(s, conv, bot, bs, False{})})
    case True{}:
      do IO<MST.StoreResult<J.Json>>:
        +last : MB.Body = Maybe.default(&2, MB.Body, List.last(&2, MB.Body, bs), MB.Body{ML.Conv.empty(), ML.Box.newest(ML.Box.empty(), ""), ""})
        r : MST.StoreResult<List<&2, MB.Body>> <- MST.MailStore.read(home, conv, ML.Mail.id(MB.Body.mail(last)), 1n)
        return MailJob.page.more(r, s, conv, bot, bs)

def MailJob.page.result(+r: MST.StoreResult<List<&2, MB.Body>>, +home: String, +s: M.State, +conv: String, +bot: String, +n: Nat) -> IO(MST.StoreResult<J.Json>):
  match r:
    case MST.Stored{+bs}: MailJob.page.full(Bool.and(Nat.is_gt(n, 0n), Nat.is_eq(List.length(&2, MB.Body, bs), n)), home, s, conv, bot, bs)
    case MST.Missing{_}: IO.pure(MST.StoreResult<J.Json>, MST.Stored{B.Mail.read.json(s, conv, bot, Nil{}, False{})})
    case MST.StoreError{+p, +e}: IO.pure(MST.StoreResult<J.Json>, MST.StoreError{p, e})

# A room's header IDs define one canonical page across legacy RoomPosted
# and journal-backed mail. No body is synthesized from a truncated header.
def MailJob.room.body(source: Maybe<&2, String>, +home: String, +cv: ML.Conv, +m: ML.Mail) -> IO(MST.StoreResult<MB.Body>):
  match source:
    case Some{text}: IO.pure(MST.StoreResult<MB.Body>, MST.Stored{RH.RoomHistory.body(cv, m, text)})
    case None{}: MST.MailStore.exact(home, ML.Conv.id(cv), ML.Mail.id(m))

def MailJob.room.body.result(r: MST.StoreResult<MB.Body>, +conv: String, +id: String, k: MB.Body -> IO(MST.StoreResult<J.Json>)) -> IO(MST.StoreResult<J.Json>):
  match r:
    case MST.Stored{body}: k(body)
    case MST.Missing{_}: IO.pure(MST.StoreResult<J.Json>, MST.StoreError{conv, "mail body unavailable: " ++ id})
    case MST.StoreError{path, err}: IO.pure(MST.StoreResult<J.Json>, MST.StoreError{path, err})

def MailJob.room.bodies(ms: List<&2, ML.Mail>, +home: String, +s: M.State, +cv: ML.Conv, +bot: String, +texts: Map<&2, String>, +more: Bool, acc: List<&2, MB.Body>) -> IO(MST.StoreResult<J.Json>):
  match ms:
    case Nil{}: IO.pure(MST.StoreResult<J.Json>, MST.Stored{B.Mail.read.json(s, ML.Conv.id(cv), bot, List.reverse(&2, MB.Body, acc), more)})
    case +m <> t:
      do IO<MST.StoreResult<J.Json>>:
        r : MST.StoreResult<MB.Body> <- MailJob.room.body(RH.RoomHistory.text(texts, ML.Mail.id(m)), home, cv, m)
        MailJob.room.body.result(r, ML.Conv.id(cv), ML.Mail.id(m), body => MailJob.room.bodies(t, home, s, cv, bot, texts, more, body <> acc))

def MailJob.room.selected(page: Maybe<&2, RH.RoomPage>, +home: String, +s: M.State, +conv: String, +bot: String) -> IO(MST.StoreResult<J.Json>):
  match page:
    case None{}: IO.pure(MST.StoreResult<J.Json>, MST.StoreError{conv, "unknown room conversation or mail cursor"})
    case Some{RH.RoomPage{+headers, more}}:
      MailJob.room.bodies(headers, home, s, ML.Box.conv(M.State.mail(s), conv), bot, RH.RoomHistory.sources(s, conv, headers), more, Nil{})

# A room's page from the store (server/thinstore.bend's Ms.*): the headers
# after the cursor and the posts' texts, by query, as RH.RoomHistory reads
# them from the box and the world
def MailJob.room.stored(page: Maybe<&2, RH.RoomPage>, +h: String, +home: String, +s: M.State, +conv: String, +bot: String) -> IO(MST.StoreResult<J.Json>):
  match page:
    case None{}: IO.pure(MST.StoreResult<J.Json>, MST.StoreError{conv, "unknown room conversation or mail cursor"})
    case Some{RH.RoomPage{+headers, more}}:
      do IO<MST.StoreResult<J.Json>>:
        texts : Map<&2, String> <- TSTO.Ms.room.sources(h, conv, headers)
        MailJob.room.bodies(headers, home, s, ML.Box.conv(M.State.mail(s), conv), bot, texts, more, Nil{})

def MailJob.room.kind(memory: Bool, +h: String, +home: String, +s: M.State, +conv: String, +bot: String, +before: String, +n: Nat) -> IO(MST.StoreResult<J.Json>):
  match memory:
    case True{}: MailJob.room.selected(RH.RoomHistory.page(s, conv, before, n), home, s, conv, bot)
    case False{}:
      do IO<MST.StoreResult<J.Json>>:
        page : Maybe<&2, RH.RoomPage> <- TSTO.Ms.room.page(h, conv, before, n)
        MailJob.room.stored(page, h, home, s, conv, bot)

def MailJob.page.kind(room: Bool, +h: String, +home: String, +s: M.State, +conv: String, +bot: String, +before: String, +n: Nat) -> IO(MST.StoreResult<J.Json>):
  match room:
    case True{}: MailJob.room.kind(String.is_empty(h), h, home, s, conv, bot, before, n)
    case False{}:
      do IO<MST.StoreResult<J.Json>>:
        r : MST.StoreResult<List<&2, MB.Body>> <- MST.MailStore.read(home, conv, before, n)
        MailJob.page.result(r, home, s, conv, bot, n)

# h: the store's handle ("": read the box in memory)
def MailJob.page(+h: String, +home: String, +s: M.State, +conv: String, +bot: String, +before: String, +n: Nat) -> IO(MST.StoreResult<J.Json>):
  MailJob.page.kind(String.starts_with(conv, "room:"), h, home, s, conv, bot, before, n)

def MailJob.read.reply(+r: MST.StoreResult<J.Json>, +client: U32, +id: String) -> B.Out:
  match r:
    case MST.Stored{+j}: B.Reply.data(client, id, j)
    case MST.Missing{+p}: B.Reply.err(client, id, "missing " ++ p)
    case MST.StoreError{+p, +e}: B.Reply.err(client, id, p ++ ": " ++ e)

def MailJob.read.text(out: B.Out) -> String:
  match out:
    case B.Send{_, +text}: text
    case _: ""

# the uberbot's fyi digest: the newest bodies of each conversation asked for
def DigestJob.add(r: MST.StoreResult<List<&2, MB.Body>>, +acc: List<&2, MB.Body>) -> List<&2, MB.Body>:
  match r:
    case MST.Stored{+bs}: List.append(&2, MB.Body, acc, bs)
    case _: acc

def DigestJob.read(convs: List<&2, String>, +home: String, +acc: List<&2, MB.Body>) -> IO(List<&2, MB.Body>):
  match convs:
    case Nil{}: IO.pure(List<&2, MB.Body>, acc)
    case +c <> +t: IO.bind(MST.StoreResult<List<&2, MB.Body>>, List<&2, MB.Body>, MST.MailStore.read(home, c, "", 10n), r => DigestJob.read(t, home, DigestJob.add(r, acc)))

def DigestJob.run(convs: List<&2, String>, +home: String, +inbox: Chan(Msg), +bot: String, +ids: List<&2, String>) -> IO(Unit):
  IO.bind(List<&2, MB.Body>, Unit, DigestJob.read(convs, home, Nil{}), bs => Self.post(inbox, DigestGot{bot, ids, bs}))

def RunJob.start(r: MST.StoreResult<List<&2, MB.Body>>, +inbox: Chan(Msg), +s: M.State, +now: Nat, +th: String, +head: String) -> IO(Unit):
  match r:
    case MST.Stored{bs}: Self.post(inbox, RunReady{th, head, bs})
    case MST.Missing{_}: Self.post(inbox, RunReady{th, head, Nil{}})
    case MST.StoreError{+path, +err}: Self.post(inbox, Commit{[M.ActivityLogged{th, B.Hub.id("run-read", now, M.State.seq(s)), "error", "mail", path ++ ": " ++ err, now}, M.TurnChanged{th, M.Failed{}, now}, M.ThreadSettled{th, False{}, now}]})

def Hub.out(h: Hub, o: B.Out) -> IO(Hub):
  match h o:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note} B.RunStart{+thread, conv, head}:
      do IO<Hub>:
        r : MST.StoreResult<List<&2, MB.Body>> <- MST.MailStore.read(Cfg.home(cfg), conv, "", 20n)
        +now : Nat <- Now.nat()
        RunJob.start(r, inbox, st, now, thread, head)
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, +inbox, +cfg, note} B.DigestRead{bot, ids, convs}:
      do IO<Hub>:
        IO.spawn(Unit, DigestJob.run(convs, Cfg.home(cfg), inbox, bot, ids))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, cfg, note} B.CharterWrite{path, text}:
      do IO<Hub>:
        fout : Result<&1, &1, U32 & String, File> <- File.open(path, "w")
        Token.write(fout, text)
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.MailAppend{line, _, changes}:
      MailJob.append.parsed(MB.Body.parse(line), Cfg.home(cfg), changes, Hub{st, f, ls, cl, as, inbox, cfg, note})
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note} B.MailRead{+client, +id, conv, before, n}:
      do IO<Hub>:
        r : MST.StoreResult<J.Json> <- MailJob.page(Cfg.thin.store(cfg), Cfg.home(cfg), st, conv, "*", before, n)
        cl2 : List<&2, Client> <- Wire.to(cl, client, MailJob.read.text(MailJob.read.reply(r, client, id)))
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, cfg, note} B.Send{+client, +text}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.to(cl, client, text)
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.Live{+thread, +text}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        cl2 : List<&2, Client> <- Wire.delta(cl, B.Delta.msg(thread, text))
        +off : Nat = FLV.FL.size(Live.of(cfg), thread)
        Far.live.out(Hub{st, f, ls, cl2, as, inbox, Live.with_delta(cfg, LV.Live.add.at(Live.of(cfg), thread, text, Nat.mul(now, 1000n)), thread, text), note}, thread, off, text, now)
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note} B.Broadcast{+text}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.all.of(cl, text, All{Bool.pick(Nat, String.starts_with(text, "{\"t\":\"delta"), 2n, 0n), 0n, 0n, 0n, Nil{}})
        Far.shown(st, cfg, text)
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, +as, inbox, cfg, note} B.AgentStart{+thread, +cwd, _, +model, +session, r, i}:
      Hub.start.k(Agents.get(as, thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, cwd, model, session, r, i, Hub.effort_of(st, thread))
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note} B.AgentSend{+thread, +line}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        +l1 : String = Bot.line(Bool.or(Bool.or(RUN.Run.is(st, thread), BO.World.is_bot_thread(M.State.bots(st), thread)), RF.Refs.maybe(line)), cfg, st, now, thread, line)
        +l2 : String <- EmbNote.claude(EmbNote.user(l1), cfg, st, thread, l1)
        Line.skills(String.contains(l2, "$"), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, l2, B.Hub.cwd(st, B.Hub.thread(st, thread)))
    case Hub{st, f, ls, cl, +as, inbox, +cfg, note} B.AgentStop{+thread}:
      do IO<Hub>:
        Hub.agent_stop(Agents.get(as, thread))
        return Hub{st, f, ls, cl, Agents.drop(as, thread), inbox, Tools.with(cfg, DP.Open.gone(Tools.of(cfg), thread)), note}
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.ToolOpen{+thread, +call}:
      IO.pure(Hub, Hub{st, f, ls, cl, as, inbox, Tools.with(cfg, DP.Open.add(Tools.of(cfg), thread, call)), note})
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.ToolShut{+thread, +call}:
      IO.pure(Hub, Hub{st, f, ls, cl, as, inbox, Tools.with(cfg, DP.Open.shut(Tools.of(cfg), thread, call)), note})
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.ToolsGone{+thread}:
      IO.pure(Hub, Hub{st, f, ls, cl, as, inbox, Tools.with(cfg, DP.Open.gone(Tools.of(cfg), thread)), note})
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.PlotInfo{+client, +id, +key, +at, +idx}:
      do IO<Hub>:
        Plot.put(Cfg.pv(cfg), PV.PInfo{client, id, key, at, idx})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.Watch{+client, +kind, +root, +path}:
      do IO<Hub>:
        Plot.put(Cfg.pv(cfg), Plot.watch(client, kind, root, path))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.CodexTurn{+thread, +cwd, +model, +session, r, text}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        +t2 : String = Far.turn(cfg, st, now, thread, text)
        +g : String = Hub.gen(st, now)
        IO.spawn(Unit, IO.bind(String, Unit, EmbNote.take(cfg, st, thread), n => Codex.go(cfg, st, thread, cwd, model, session, r, t2 ++ n, g, inbox)))
        Hub.placeholder(Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, g)
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.ChecksTrust{+hash}:
      do IO<Hub>:
        r : P.Ran <- P.Proc.run("sh", ["-c", "case $1 in *[!0-9a-f]*|'') exit 1;; esac; printf '%s\\n' \"$1\" >> \"$2\"", "sh", hash, Cfg.home(cfg) ++ "/checks.trusted"], "/")
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, +inbox, cfg, note} B.ModelsRefresh{}:
      do IO<Hub>:
        IO.spawn(Unit, OcJob.run(inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, cfg, note} B.KiCadRetry{}:
      Hub.kretry(Hub{st, f, ls, cl, as, inbox, cfg, note})
    case Hub{st, f, ls, cl, as, +inbox, +cfg, note} B.KiCadInstall{}:
      do IO<Hub>:
        IO.spawn(Unit, Install.go(Cfg.home(cfg), inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.GrokTurn{+thread, +cwd, +model, +session, r, +text}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        +t2 : String = Far.turn(cfg, st, now, thread, text)
        +g : String = Hub.gen(st, now)
        IO.spawn(Unit, IO.bind(String, Unit, EmbNote.take(cfg, st, thread), n => Grok.go(cfg, st, thread, cwd, model, session, r, t2 ++ n, g, inbox)))
        Hub.placeholder(Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, g)
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.MuseTurn{+thread, +cwd, +model, +session, r, +text}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        +t2 : String = Far.turn(cfg, st, now, thread, text)
        +g : String = Hub.gen(st, now)
        IO.spawn(Unit, IO.bind(String, Unit, EmbNote.take(cfg, st, thread), n => Muse.go(cfg, st, thread, cwd, model, session, r, t2 ++ n, g, inbox)))
        Hub.placeholder(Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, g)
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.AgyTurn{+thread, +cwd, +model, +session, r, +text}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        +t2 : String = Far.turn(cfg, st, now, thread, text)
        +g : String = Hub.gen(st, now)
        IO.spawn(Unit, IO.bind(String, Unit, EmbNote.take(cfg, st, thread), n => Agt.go(cfg, st, thread, cwd, model, session, r, t2 ++ n, g, inbox)))
        Hub.placeholder(Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, g)
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.OcTurn{+thread, +cwd, +model, +session, r, +text}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        +t2 : String = Far.turn(cfg, st, now, thread, text)
        +g : String = Hub.gen(st, now)
        IO.spawn(Unit, IO.bind(String, Unit, EmbNote.take(cfg, st, thread), n => Oc.go(cfg, st, thread, cwd, model, session, r, t2 ++ n, g, inbox)))
        Hub.placeholder(Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, g)
    case Hub{st, f, ls, cl, as, +inbox, cfg, note} B.LockGuard{+name, old, +holder}:
      do IO<Hub>:
        LG.Guard.drop(name, old)
        IO.spawn(Unit, LockJob.later(inbox, name, holder, 0n))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.Await{+thread, +id, +kind, +target, +ident, +deadline, +cwd}:
      do IO<Hub>:
        Watch.intent(B.Locks.work.has(st, id), Cfg.home(cfg), id)
        IO.spawn(Unit, Watch.job(B.Locks.work.has(st, id), Cfg.home(cfg), thread, id, kind, target, ident, deadline, cwd, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.WatchStop{_, +id, +kind, +target, +ident}:
      do IO<Hub>:
        IO.spawn(Unit, Watch.stop.job(Cfg.home(cfg), id, kind, target, ident))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, cfg, note} B.DeskLook{+client, +thread, +focused}:
      Hub.desk(Hub{st, f, ls, cl, as, inbox, cfg, note}, client, thread, focused)
    # the island alone, off the hub loop (its push's own process)
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.IslandSync{}:
      do IO<Hub>:
        IO.spawn(Unit, Hub.island(Cfg.home(cfg), st, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    # the island only: the alert, if the turn raised one, is its own out
    # (Alerted), written with the thread's record
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.TurnEnded{+thread, _}:
      do IO<Hub>:
        IO.spawn(Unit, Hub.phone(Cfg.home(cfg), st, thread, inbox))
        Self.post(inbox, KProbe{False{}})
        Self.post(inbox, Release{})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    # an alert record was written: the phones' push, the desk used last and
    # the owner's other machines, unless a desk in use is showing the thread
    case Hub{+st, f, ls, +cl, as, +inbox, +cfg, note} B.Alerted{+thread}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        +rec : NO.AlertRec = NO.Alert.get(st, thread)
        +seen : Bool = Bool.and(NO.Alert.holds(NO.Alert.kind(rec)), DK.Desk.seen(Clients.looks(cl), thread, now))
        PU.Push.held(seen, thread)
        cl2 : List<&2, Client> <- Desk.tell(seen, cl, st, thread, rec)
        IO.spawn(Unit, Far.note.push(Cfg.home(cfg), st, thread, PU.Push.title(B.Hub.thread(st, thread)), PU.Push.alert.body(st, thread, rec), DK.Desk.hold(seen, NO.Alert.rkey(rec)), inbox))
        Far.note.out(Hub{st, f, ls, cl2, as, inbox, cfg, note}, rec, thread, seen)
    # a bot's answer to a person elsewhere, captured by the step that left
    # its thread ready (hub.bend's Answer.out)
    # reports to launchers on other machines, sent in order
    case Hub{st, f, ls, cl, as, +inbox, +cfg, note} B.WakePoll{prs, files}:
      do IO<Hub>:
        IO.spawn(Unit, WakeJob.prs(prs, Cfg.home(cfg), inbox))
        IO.spawn(Unit, WakeJob.files(files, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.LaunchFar{}:
      do IO<Hub>:
        IO.spawn(Unit, Lr.far.run(B.Lr.far.lines(st), Nil{}, st, cfg, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, +inbox, cfg, note} B.BotReply{+thread, +to, +text, _}:
      do IO<Hub>:
        Self.post(inbox, Replied{thread, to, text})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, +inbox, cfg, note} B.Fs{+client, +id, +op, +path}:
      do IO<Hub>:
        IO.spawn(Unit, FsJob.run(op, inbox, client, id, path))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, +inbox, cfg, note} B.Teach{+client, +id, +m, +p}:
      do IO<Hub>:
        Self.post(inbox, TeachReq{client, id, m, p})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.TermOp{+op, +thread, +cwd, data, +cols, +rows}:
      do IO<Hub>:
        Term.put(Cfg.tm(cfg), Term.msg(op, thread, cwd, data, cols, rows))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.TermRun{key, ti, cwd, cmd, c, r, again}:
      do IO<Hub>:
        Term.put(Cfg.tm(cfg), TM.TRun{key, ti, cwd, cmd, c, r, again})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.TermSnap{c, key}:
      do IO<Hub>:
        Term.put(Cfg.tm(cfg), TM.TSnap{c, key})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.TermKill{key}:
      do IO<Hub>:
        Term.put(Cfg.tm(cfg), TM.TKill{key})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, +as, +inbox, +cfg, note} B.Git{+client, +id, +job, +thread, +cwd, +a, +b}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        KExportJob.route(String.eq(job, "kexport"), inbox, client, id, thread, cwd, Cfg.home(cfg), Cfg.rt(cfg), a, KiCadJob.route(String.eq(job, "kicad"), inbox, client, id, thread, cwd, Cfg.rt(cfg), Media.run(AG.Text.of(st), job, inbox, client, id, thread, cwd, Bot.attach(String.eq(job, "attach.claude"), cfg, st, now, thread, a), b, Cfg.home(cfg), Net.managed(Cfg.net(cfg)), Agents.gen(as, thread))))
        return Claude.prep(Bool.and(String.eq(job, "attach.claude"), Bool.not(String.is_empty(b))), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, b)
    case Hub{st, f, ls, cl, as, inbox, +cfg, note} B.AskAnswered{+ask, +answer}:
      do IO<Hub>:
        Park.put(Cfg.pk(cfg), PK.PAnswer{ask, answer})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.UpgradeForce{}:
      do IO<Hub>:
        IO.spawn(Unit, Upgrade.go(Net.managed(Cfg.net(cfg)), True{}, Cfg.home(cfg), M.Setting.get(st, "update.channel", "stable"), inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, +inbox, cfg, note} B.DeployProbe{}:
      do IO<Hub>:
        IO.spawn(Unit, Deploy.probe(inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, +inbox, cfg, note} B.DeployGo{+stage}:
      do IO<Hub>:
        IO.spawn(Unit, Deploy.go(stage, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.ShipRun{+commit}:
      do IO<Hub>:
        IO.spawn(Unit, ShipJob.run(B.Ship.repo(st), B.Ship.wrap(st), Cfg.home(cfg), commit, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, as, +inbox, cfg, note} B.ShipNext{+why}:
      do IO<Hub>:
        IO.spawn(Unit, ShipJob.next(B.Ship.repo(st), B.Ship.cands(st), why, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{st, f, ls, cl, as, +inbox, cfg, note} B.Restart{}:
      do IO<Hub>:
        IO.spawn(Unit, Upgrade.restart(inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.Upgrade{}:
      do IO<Hub>:
        IO.spawn(Unit, Upgrade.go(Net.managed(Cfg.net(cfg)), False{}, Cfg.home(cfg), M.Setting.get(st, "update.channel", "stable"), inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note} B.BotFx{+kind, +a, +b, +c}:
      do IO<Hub>:
        Bot.fx(C.Codec.pos(["home", "gone", "hook", "unhook", "send", "invite", "join", "unpeer", "google", "token", "retoken", "untoken", "voice", "scratch", "scratch-gone", "uber", "uber-home", "zone", "task-file", "task-gone"], kind), st, inbox, cfg, a, b, c)
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.outs.go(os: List<&2, B.Out>, h: Hub) -> IO(Hub):
  match os:
    case Nil{}:
      IO.pure(Hub, h)
    case o <> t:
      IO.bind(Hub, Hub, Hub.out(h, o), h2 => Hub.outs.go(t, h2))

# a step's outs, run in order; a hub on a copied log runs none that act
# outside its home (core/copy.bend's Copy.outs)
def Hub.outs(os: List<&2, B.Out>, h: Hub) -> IO(Hub):
  IO.bind(Bool, Hub, P.Proc.copy(), copy => Hub.outs.go(CP.Copy.outs(copy, os), h))

# the window in this process: every mirror, whole
def Far.join.here(ms: List<&2, FR.In>, m: Maybe<&2, Client>) -> IO(Maybe<&2, Client>):
  match ms:
    case Nil{}:
      IO.pure(Maybe<&2, Client>, m)
    case +i <> t:
      IO.bind(Maybe<&2, Client>, Maybe<&2, Client>, Wire.text.m(m, FR.In.msg(i)), m2 => Far.join.here(t, m2))

def Hub.join.notes(m: Maybe<&2, Client>, +note: String, +desk: String) -> IO(Maybe<&2, Client>):
  IO.bind(Maybe<&2, Client>, Maybe<&2, Client>, Wire.text.if(String.is_empty(note), m, note), m2 => Wire.text.if(String.is_empty(desk), m2, desk))

def Hub.join.state(c: Client, w: Wire) -> IO(Maybe<&2, Client>):
  Wire.offer(c, w, 0n)

# a join in one frame is live at once: the streamed text follows it
def Hub.join.live(more: Bool, m: Maybe<&2, Client>, +cfg: Cfg) -> IO(Maybe<&2, Client>):
  match more:
    case True{}:
      IO.pure(Maybe<&2, Client>, m)
    case False{}:
      Wire.text.m(m, LV.Live.msg(Live.of(cfg)))

def Hub.join.more(more: Bool, +inbox: Chan(Msg), +client: U32, +gen: U32) -> IO(Unit):
  match more:
    case True{}:
      Self.post.later(inbox, JoinMore{client, gen})
    case False{}:
      IO.pure(Unit, Unit{})

# Joining, a chunk at a time
# --------------------------
# While a client takes its log it gets no live changes (Wire.changes);
# each JoinMore (and each report from its writer) sends the next chunk if
# it has room (OB.Cr.room): the rest of the log, then each far mirror in
# chunks (FR.In.chunk), then a last frame with "boot": false that makes it
# live. Changes made meanwhile are part of the log by then, so nothing is
# missed or sent twice, and the hub's inbox is read between chunks.

def Clients.swap.k(same: Bool, c: Client, +m: Maybe<&2, Client>) -> Maybe<&2, Client>:
  match same:
    case True{}:
      m
    case False{}:
      Some{c}

# the client with this id replaced by m (dropped for None)
def Clients.swap(cs: List<&2, Client>, +id: U32, +m: Maybe<&2, Client>) -> List<&2, Client>:
  match cs:
    case Nil{}:
      Nil{}
    case +c <> t:
      Clients.add(Clients.swap.k(Client.is(c, id), c, m), Clients.swap(t, id, m))

def Boot.due(c: Client, +gen: U32) -> Bool:
  match c:
    case Client{_, _, _, +r, _, _, _}:
      Bool.and(Bool.and(OB.Cr.boot(r), U32.is_eq(OB.Cr.gen(r), gen)), OB.Cr.room(r))

def Boot.cr(c: Client, +r: OB.Cr) -> Client:
  match c:
    case Client{i, o, l, _, k, lgc, mb}:
      Client{i, o, l, r, k, lgc, mb}

def Boot.r(c: Client) -> OB.Cr:
  match c:
    case Client{_, _, _, r, _, _, _}:
      r

def Boot.next(more: Bool, h: Hub, k: Hub -> IO(Hub)) -> IO(Hub):
  match more:
    case True{}:
      k(h)
    case False{}:
      IO.pure(Hub, h)

# the next JoinMore for a joining client, when it has room and none is
# on its way already: one at a time, so the hub sends one chunk per
# message it reads and anything else waits behind at most a few
def Kick.k(go: Bool, c: Client, +inbox: Chan(Msg)) -> IO(Client):
  match go c:
    case True{} Client{+i, o, l, +r, k, lgc, mb}:
      do IO<Client>:
        Self.post.later(inbox, JoinMore{i, OB.Cr.gen(r)})
        return Client{i, o, l, OB.Cr.wait.set(r, True{}), k, lgc, mb}
    case False{} _:
      IO.pure(Client, c)

def Kick.due(+r: OB.Cr) -> Bool:
  Bool.and(Bool.and(OB.Cr.boot(r), Bool.not(OB.Cr.wait(r))), OB.Cr.room(r))

def Client.mobile.pending(c: Client) -> Bool:
  match c:
    case Client{_, _, _, _, _, _, mb}: Bool.not(List.is_empty(&2, MobileStart, mb))

def Kick.one(+c: Client, +id: U32, +inbox: Chan(Msg)) -> IO(Client):
  Kick.k(Bool.and(Bool.not(Client.mobile.pending(c)), Bool.and(Client.is(c, id), Kick.due(Boot.r(c)))), c, inbox)

def Kick.go(cs: List<&2, Client>, +id: U32, +inbox: Chan(Msg)) -> IO(List<&2, Client>):
  match cs:
    case Nil{}:
      IO.pure(List<&2, Client>, Nil{})
    case +c <> t:
      do IO<List<&2, Client>>:
        c2 : Client <- Kick.one(c, id, inbox)
        t2 : List<&2, Client> <- Kick.go(t, id, inbox)
        return c2 <> t2

def Hub.kick(h: Hub, +id: U32) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Kick.go(cl, id, inbox)
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

# a chunk out, then the next one asked for (Hub.kick)
def Boot.send(h: Hub, c: Client, w: Wire, +n: Nat, +id: U32) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        m : Maybe<&2, Client> <- Wire.offer(c, w, n)
        Hub.kick(Hub{st, f, ls, Clients.swap(cl, id, m), as, inbox, cfg, note}, id)

def Boot.text(+text: String) -> Wire:
  WText{text}

# the last frame of a join (or a catch-up), then every thread's streamed
# text so far, which replaces the client's (core/live.bend)
def Boot.last(h: Hub, c: Client, +id: U32) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        m : Maybe<&2, Client> <- Wire.offer(c, Boot.text(Hub.boot_msg(Nil{}, True{})), 0n)
        m2 : Maybe<&2, Client> <- Wire.text.m(m, LV.Live.msg(Live.of(cfg)))
        return Hub{st, f, ls, Clients.swap(cl, id, m2), as, inbox, cfg, note}

def Boot.far.c(fc: FR.FC, h: Hub, +c: Client, +id: U32, +cat: String) -> IO(Hub):
  match fc:
    case FR.FC{+text, +next, True{}}:
      Boot.send(h, Boot.cr(c, OB.Cr.far.set(Boot.r(c), 1n+OB.Cr.fl(Boot.r(c)), 0n)), Boot.text(FR.Fr.with.cat(text, cat)), String.length(FR.Fr.with.cat(text, cat)), id)
    case FR.FC{+text, +next, False{}}:
      Boot.send(h, Boot.cr(c, OB.Cr.far.set(Boot.r(c), OB.Cr.fl(Boot.r(c)), next)), Boot.text(text), String.length(text), id)

# a mirror's last chunk carries the link's catalog, when one came
def Boot.far.cat(p: FR.Fh & Hub, fc: FR.FC, +link: String, +c: Client, +id: U32) -> IO(Hub):
  (+fh, h) = p
  Boot.far.c(fc, Far.with(h, fh), c, id, FR.Fh.cat(fh, link))

# the log is all out, then the far mirrors; the last frame makes it live
def Boot.far.k(m: Maybe<&2, FR.In>, h: Hub, +c: Client, +id: U32) -> IO(Hub):
  match m:
    case Some{+i}:
      Boot.far.cat(Far.fh(h), FR.In.chunk(i, OB.Cr.fseq(Boot.r(c))), FR.In.link(i), c, id)
    case None{}:
      Boot.last(h, Boot.cr(c, OB.Cr.live(Boot.r(c))), id)

# how many lines a join's next chunk takes from item at of n: what
# OB.Join.next(OB.Join.window(...)) took when the hub held the lines
def Join.min(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

def Join.take(+n: Nat, +at: Nat) -> Nat:
  Join.min(Nat.sub(n, at), Join.min(OB.Join.most(), OB.Join.lines()))

# where a chunk leaves the cursor; a chunk the store could not give (none
# while lines are due) skips to the end rather than asking forever
def Join.adv(+ch: List<&2, String>, +at: Nat, +n: Nat) -> Nat:
  Bool.pick(Nat, List.is_empty(&2, String, ch), n, Nat.add(at, List.length(&2, String, ch)))

# the next chunk, read from the store (SO.Sto.lines) from the cursor
def Boot.go(log: Bool, +c: Client, h: Hub, +n: Nat, +id: U32) -> IO(Hub):
  match log h:
    case True{} Hub{st, f, +ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        +ch : List<&2, String> <- SO.Sto.lines(ls, OB.Cr.at(Boot.r(c)), Join.take(n, OB.Cr.at(Boot.r(c))))
        Boot.send(Hub{st, f, ls, cl, as, inbox, cfg, note},
          Boot.cr(c, OB.Cr.at.set(Boot.r(c), Join.adv(ch, OB.Cr.at(Boot.r(c)), n))),
          Hub.boot_items(ch), 0n, id)
    case False{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Boot.far.k(List.get(&2, FR.In, Far.mirrors(cfg), OB.Cr.fl(Boot.r(c))), Hub{st, f, ls, cl, as, inbox, cfg, note}, c, id)

# Window lookup uses the actual local wire length kept by successful
# commits, not State.seq or a walk of the full shared log. Near its head,
# every small catch-up window used to walk the entire growing log twice.
def Boot.k.rest(empty: Bool, +c: Client, h: Hub, +id: U32) -> IO(Hub):
  match empty h:
    case True{} Hub{st, f, +ls, cl, as, inbox, +cfg, note}:
      Boot.go(Nat.is_lt(OB.Cr.at(Boot.r(c)), LogCount.n(cfg)), c, Hub{st, f, ls, cl, as, inbox, cfg, note}, LogCount.n(cfg), id)
    case False{} _:
      Boot.go(True{}, c, h, 0n, id)

def Boot.k(due: Bool, +c: Client, h: Hub, +id: U32) -> IO(Hub):
  match due:
    case True{}:
      Boot.k.rest(List.is_empty(&2, String, OB.Cr.rest(Boot.r(c))), c, h, id)
    case False{}:
      IO.pure(Hub, h)

def Boot.m(m: Maybe<&2, Client>, h: Hub, +id: U32, +gen: U32) -> IO(Hub):
  match m:
    case Some{+c}:
      Boot.k(Boot.due(c, gen), c, h, id)
    case None{}:
      IO.pure(Hub, h)

def Boot.unwait.k(same: Bool, c: Client) -> Client:
  match same c:
    case True{} Client{i, o, l, r, k, lgc, mb}:
      Client{i, o, l, OB.Cr.wait.set(r, False{}), k, lgc, mb}
    case False{} _:
      c

def Boot.unwait(cs: List<&2, Client>, +id: U32, +gen: U32) -> List<&2, Client>:
  match cs:
    case Nil{}:
      Nil{}
    case +c <> t:
      Boot.unwait.k(Bool.and(Client.is(c, id), U32.is_eq(OB.Cr.gen(Boot.r(c)), gen)), c) <> Boot.unwait(t, id, gen)

def Hub.boot.c(h: Hub, +client: U32, +gen: U32) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      Boot.m(Clients.get(cl, client), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, gen)

# a JoinMore: the next chunk of a joining client's log, if it still joins
# under this generation and has room; then the one after is asked for
def Hub.boot(h: Hub, +client: U32, +gen: U32) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      Hub.boot.c(Hub{st, f, ls, Boot.unwait(cl, client, gen), as, inbox, cfg, note}, client, gen)

# a writer's report: the client's credit, then its next chunk asked for
# if it joins (Hub.kick)
def Hub.drained(h: Hub, +client: U32, +gen: U32, +n: Nat, +b: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      Hub.kick(Hub{st, f, ls, Clients.cr(cl, client, gen, n, b), as, inbox, cfg, note}, client)

def Far.rpc.may(ok: Bool, h: Hub, +client: U32, +id: String, +link: String, +body: String, +now: Nat) -> IO(Hub):
  match ok h:
    case True{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.rpc.job(client, id, link, body, Cfg.home(cfg), Bot.peer.url(st, link), inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      Hub.far.replied(h, client, id, link, 403n, "that is done on its own machine")

# a viewer's request about a thread elsewhere: this hub answers with the
# owning hub's help (Fv.near), or the owner answers it (Fv.reads)
def Far.rpc.near(h: Hub, +client: U32, +id: String, +link: String, +meth: String, +body: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.near.job(client, id, link, meth, body, Cfg.home(cfg), Bot.peer.url(st, link), inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# (only to the owner's own machines with sync on: Far.own.id)
def Far.rpc.pick(near: Bool, own: Bool, +may: Bool, h: Hub, +client: U32, +id: String, +link: String, +meth: String, +body: String, +now: Nat) -> IO(Hub):
  match near own:
    case True{} True{}:
      Far.rpc.near(h, client, id, link, meth, body)
    case False{} True{}:
      Far.rpc.may(may, h, client, id, link, body, now)
    case _ False{}:
      Far.rpc.may(False{}, h, client, id, link, body, now)

def Hub.announce.far(r: Hub & List<&2, String>, cs: List<&2, M.Change>, +from: Nat) -> IO(Hub):
  (h, +fresh) = r
  IO.bind(Hub, Hub, Hub.announce((h, fresh)), h2 => Far.push(h2, cs, fresh, from))

def Hub.write_fail.note(+err: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"update"}}, J.KV{"error", J.Str{"log write failed: " ++ err}}]))

# a step touched the uberbot's log, or promoted a bot
def Uber.touched.op(+n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(n, 1n), 1n, 2n)

def Uber.touched(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.UberSet{_, _, _} <> t:
      1n
    case M.UberOp{_, _, _, _, _, _} <> t:
      Uber.touched.op(Uber.touched(t))
    case _ <> t:
      Uber.touched(t)

def Uber.after.k(n: Nat, h: Hub) -> IO(Hub):
  match n h:
    case 0n _:
      IO.pure(Hub, h)
    case 1n Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        Far.send(inbox, UberDo{"init"})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case _ Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        Far.send(inbox, UberDo{"kick"})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.uber.after(h: Hub, +cs: List<&2, M.Change>) -> IO(Hub):
  Uber.after.k(Uber.touched(cs), h)

def Cfg.telemetry(cfg: Cfg) -> List<&2, TI.TmCfg>:
  match cfg:
    case Cfg{_, _, _, _, _, _, _, _, tcfg, +views}: tcfg

def Cfg.with_tm(cfg: Cfg, tm2: List<&2, TI.TmCfg>) -> Cfg:
  match cfg:
    case Cfg{h, w, ho, p, pr, ds, v, nt, _, +views}: Cfg{h, w, ho, p, pr, ds, v, nt, tm2, views}

def Hub.telemetry.permit(h: Hub, gen: Nat, +view: TLM.TmView, reply: Chan(Bool)) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        ok : Result<&1, &1, Bool, Unit> <- Chan.send(Bool, reply, Bool.and(TI.Tio.owns(Cfg.telemetry(cfg), gen), TLM.Tm.current(view, st)))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def TelemetryJob.permit(+inbox: Chan(Msg), gen: Nat, view: TLM.TmView) -> IO(Bool):
  do IO<Bool>:
    +reply : Chan(Bool) <- Chan.new(Bool, 1)
    Self.post(inbox, TelemetryPermit{gen, view, reply})
    ok : Maybe<&1, Bool> <- Chan.recv(Bool, reply)
    return Maybe.default(&1, Bool, ok, False{})

def TelemetryJob.run(+tm: List<&2, TI.TmCfg>, home: String, +view: TLM.TmView, +now: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    id : String <- TI.Tio.job(tm, home, view, now, u => TelemetryJob.permit(inbox, now, view))
    Self.post(inbox, TelemetryDone{now, view, id})

def Hub.telemetry.k(go: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match go h:
    case True{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, TelemetryJob.run(Cfg.telemetry(cfg), Cfg.home(cfg), TI.Tio.view(st, Cfg.version(cfg), now, Cfg.telemetry(cfg)), now, inbox))
        return Hub{st, f, ls, cl, as, inbox, Cfg.with_tm(cfg, TI.Tio.busy(Cfg.telemetry(cfg), now)), note}
    case False{} x: IO.pure(Hub, x)

def Hub.telemetry.cfg(h: Hub) -> Cfg:
  match h:
    case Hub{_, _, _, _, _, _, cfg, _}: cfg

def Hub.telemetry(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.telemetry.k(TI.Tio.can(Cfg.telemetry(cfg), now), Hub{st, f, ls, cl, as, inbox, cfg, note}, now)

# Unchanged minute results do not send redundant info frames to clients.
def Hub.telemetry.id.changed(changed: Bool, h: Hub) -> IO(Hub):
  match changed h:
    case False{} x: IO.pure(Hub, x)
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.all(cl, "{\"t\":\"info\",\"info\":{\"telemetry.id\":" ++ J.Json.show(J.Str{TI.Tio.id(Cfg.telemetry(cfg))}) ++ "}}")
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

def Hub.telemetry.again(current: Bool, h: Hub, now: Nat) -> IO(Hub):
  match current:
    case True{}: IO.pure(Hub, h)
    case False{}: Hub.telemetry(h, now)

def Hub.telemetry.done(h: Hub, gen: Nat, +view: TLM.TmView, +id: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +cfg2 : Cfg = Cfg.with_tm(cfg, TI.Tio.done(Cfg.telemetry(cfg), gen, Bool.pick(String, TLM.Tm.current(view, st), id, "")))
        h2 : Hub <- Hub.telemetry.id.changed(Bool.not(String.eq(TI.Tio.id(Cfg.telemetry(cfg)), TI.Tio.id(Cfg.telemetry(cfg2)))), Hub{st, f, ls, cl, as, inbox, cfg2, note})
        Hub.telemetry.again(TLM.Tm.current(view, st), h2, now)

def Hub.telemetry.scrub.k(clear: Bool, h: Hub) -> IO(Hub):
  match clear h:
    case False{} x: IO.pure(Hub, x)
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.all(cl, "{\"t\":\"info\",\"info\":{\"telemetry.id\":\"\"}}")
        return Hub{st, f, ls, cl2, as, inbox, Cfg.with_tm(cfg, TI.Tio.id.clear(Cfg.telemetry(cfg))), note}

def Hub.telemetry.scrub(h: Hub) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.telemetry.scrub.k(Bool.and(Bool.not(TLM.Tm.share(st)), Bool.not(String.is_empty(TI.Tio.id(Cfg.telemetry(cfg))))), Hub{st, f, ls, cl, as, inbox, cfg, note})

def Hub.step.wrote(r: Hub & Wrote, +cs: List<&2, M.Change>, os: List<&2, B.Out>, +from: Nat) -> IO(Hub & Bool):
  (h, w) = r
  match w:
    case Wrote{+fresh, True{}, _}:
      do IO<Hub & Bool>:
        t0 : U32 <- PS.Perf.us()
        h1 : Hub <- Hub.busy.if(B.Busy.moved(cs), h)
        t1 : U32 <- PS.Perf.span("hub.step.busy", t0)
        h2 : Hub <- Hub.announce.far((h1, fresh), cs, from)
        t2 : U32 <- PS.Perf.span("hub.step.far", t1)
        h3 : Hub <- Hub.outs(os, h2)
        t3 : U32 <- PS.Perf.span("hub.step.outs", t2)
        h4 : Hub <- Hub.uber.after(h3, cs)
        h5 : Hub <- Hub.telemetry.scrub(h4)
        return (h5, True{})
    case Wrote{_, False{}, +err}:
      do IO<Hub & Bool>:
        PS.Log.error("hub", "log write failed: " ++ err)
        PS.Perf.add("hub.write_fail", 1)
        h2 : Hub <- Hub.note(h, Hub.write_fail.note(err))
        return (h2, False{})

def Hub.step.go(+st: M.State, h: Hub, cs: List<&2, M.Change>, os: List<&2, B.Out>) -> IO(Hub & Bool):
  do IO<Hub & Bool>:
    t0 : U32 <- PS.Perf.us()
    +cs2 : List<&2, M.Change> = B.Pend.fix(st, cs)
    t1 : U32 <- PS.Perf.span("hub.step.pend", t0)
    r : Hub & Wrote <- Hub.commit.w(cs2, h)
    t2 : U32 <- PS.Perf.span("hub.step.commit", t1)
    Hub.step.wrote(r, cs2, os, M.State.seq(st))

def Hub.step.b(st: M.State, h: Hub, +cs: List<&2, M.Change>, os: List<&2, B.Out>) -> IO(Hub & Bool):
  Hub.step.go(st, h, cs, B.Island.outs(cs, B.Alerts.outs(cs, B.Tools.outs(cs, os))))

# the asks the step opens get their alert records, and every record written
# tells the server (hub.bend's Alerts)
def Hub.step.a(+st: M.State, h: Hub, cs: List<&2, M.Change>, os: List<&2, B.Out>) -> IO(Hub & Bool):
  Hub.step.b(st, h, B.Alerts.changes(st, cs), os)

# the step's wakes (hub.bend's Wake.after) and riding mail (Ride.after: a
# turn that ended with mail waiting starts the next) first, so their
# messages get
# alert records like any other change
def Hub.step.wakes(step: B.Step, +st: M.State, h: Hub) -> IO(Hub & Bool):
  match step:
    case B.Step{cs, os}:
      Hub.step.a(st, h, cs, os)

# Keep the affine hub and copied state boxed while computing the pure step.
# Their expanded records plus caps/now/step exceed Bend's 247-word call
# limit when held in one continuation. Unbox only at the existing handoff.
def Hub.step.fallback.done(held: List<Hub>, states: List<&2, M.State>, step: B.Step) -> IO(Hub & Bool):
  match held states:
    case h <> hs st <> ss: Hub.step.wakes(step, st, h)
    case _ _: IO.die(Hub & Bool, 1, "missing fallback step context")

def Hub.step.fallback.fold(states: List<&2, M.State>, caps: J.Json, now: Nat, step: B.Step, held: List<Hub>) -> IO(Hub & Bool):
  match states:
    case +st <> rest: Hub.step.fallback.done(held, [st], B.Fallback.validate(st, caps, now, step))
    case Nil{}: IO.die(Hub & Bool, 1, "missing fallback state")

def Hub.step.fallback.kept(r: J.Json & BN.Bn, +cfg: Cfg, h: Hub) -> J.Json & List<Hub>:
  (caps, bn) = r
  match h:
    case Hub{s, f, ls, cl, as, inbox, _, note}:
      (caps, [Hub{s, f, ls, cl, as, inbox, Cfg.with_bn(cfg, bn), note}])

# the models' JSON every step needs, kept parsed with its text in Bn
# (BN.Bn.models.json): parsing Cfg.models at every step was seconds of the
# JS hub's first minute after boot
def Hub.step.fallback.models(h: Hub) -> J.Json & List<Hub>:
  match h:
    case Hub{s, f, ls, cl, as, inbox, +cfg, note}:
      Hub.step.fallback.kept(BN.Bn.models.json(Cfg.bn(cfg), Cfg.models(cfg)), cfg, Hub{s, f, ls, cl, as, inbox, cfg, note})

def Hub.step.fallback.captured(pair: J.Json & List<Hub>, states: List<&2, M.State>, now: Nat, step: B.Step) -> IO(Hub & Bool):
  (caps, held) = pair
  Hub.step.fallback.fold(states, caps, now, step, held)

def Hub.step.fallback(h: Hub, states: List<&2, M.State>, now: Nat, step: B.Step) -> IO(Hub & Bool):
  Hub.step.fallback.captured(Hub.step.fallback.models(h), states, now, step)

def Hub.step.k(p: M.State & Hub, cs: List<&2, M.Change>, os: List<&2, B.Out>) -> IO(Hub & Bool):
  (+st, h) = p
  IO.bind(Nat, Hub & Bool, Now.nat(), +now => Hub.step.fallback(h, [st], now, B.Locks.after(st, now, B.Ride.after(st, now, B.Wake.after(st, B.Run.after(st, now, B.Step{cs, os}))))))

# a step's changes are stored (with the pending counts they move), then
# its effects run; answers whether it was stored
def Hub.step.ok(h: Hub, s: B.Step) -> IO(Hub & Bool):
  match s:
    case B.Step{+cs, +os}:
      Hub.step.k(Hub.state(h), cs, os)

def Hub.step.drop(r: Hub & Bool) -> Hub:
  (h, _) = r
  h

def Hub.step(h: Hub, s: B.Step) -> IO(Hub):
  IO.bind(Hub & Bool, Hub, Hub.step.ok(h, s), r => IO.pure(Hub, Hub.step.drop(r)))

# Teaching a bot's browser (docs/teach.md)
# ----------------------------------------
# bots.browser and bots.teach reach the hub as TeachReq (B.Teach): the hub
# keeps each bot's driver and draft in its Cfg (core/teach.bend) and tells
# every client the info keys drive.<bot>, teach.<bot>, teach.draft.<bot>.

# The embedded browser (core/cef.bend, tools/cef)
# -----------------------------------------------
# The window says its X window (browser.embed); with the setting
# browser.engine "cef" the hub moves the helper to the CEF engine, whose
# pages are native children of that window. The window says where the
# shown page goes (browser.place). Every native operation runs in order on
# one worker (CefQueue): shield on, show, then the shield the page's drive
# calls for; engine replies carry a generation, and stale ones are dropped.
# The state is in memory (botnet.bend's Bn.cef).

# a page id names a real thread or a live bot
def Cef.page.ok(+st: M.State, +page: String) -> Bool:
  CF.Cef.owner(page, B.Hub.owns(st, page), BO.Bot.live(BO.World.bot(M.State.bots(st), page)))

def Hub.preview(h: Hub, +thread: String, +o: J.Json, +reply: Chan(String)) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        Desktop.policy(Cfg.br(cfg), Bot.page(st, thread), Cef.page.ok(st, Bot.page(st, thread)), Cfg.tch(cfg))
        IO.spawn(Unit, Preview.go(TE.Teach.bot_may(Cfg.tch(cfg), Bot.page(st, thread)), cfg, Bot.page(st, thread), o, reply))
        cl2 : List<&2, Client> <- Hub.auto.k(Bool.not(Bool.and(Bool.not(Bot.is_bot(RUN.Run.bot(st, thread))), Preview.shows(o))), cl, Preview.view(thread))
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

def CefJob.ask(m: Maybe<&2, Chan(BR.BMsg)>, +op: String, +args: J.Json) -> IO(String):
  match m:
    case Some{q}: BR.Browser.ask_on(q, J.Json.get_str(args, "page", ""), op, args)
    case None{}: IO.pure(String, BR.Browser.fail("no browser helper"))

# the engine request; the helper keeps its flags, so a restarted helper runs
# the same engine (cef starts the helper; chrome does not)
def CefJob.engine_ask(m: Maybe<&2, Chan(BR.BMsg)>, +on: Bool, +args: J.Json) -> IO(String):
  match m:
    case Some{q}: BR.Browser.engine_ask(q, CF.Cef.argv(on, U32.from_nat(J.Json.get_nat(args, "parent", 0n)), J.Json.get_str(args, "display", "")), args, on)
    case None{}: IO.pure(String, BR.Browser.fail("no browser helper"))

def CefJob.focus(on: Bool, +m: Maybe<&2, Chan(BR.BMsg)>, +page: String) -> IO(Unit):
  match on:
    case True{}:
      IO.bind(String, Unit, CefJob.ask(m, "focus", J.Json.obj([J.KV{"page", J.Str{page}}])), r => IO.pure(Unit, Unit{}))
    case False{}: IO.pure(Unit, Unit{})

def CefJob.shield(+m: Maybe<&2, Chan(BR.BMsg)>, +page: String, +drive: String) -> IO(Unit):
  do IO<Unit>:
    r : String <- CefJob.ask(m, "shield", J.Json.obj([J.KV{"on", J.Flag{CF.Cef.shield(drive)}}]))
    CefJob.focus(CF.Cef.shield.focus(drive), m, page)

def CefJob.place(show: Bool, +m: Maybe<&2, Chan(BR.BMsg)>, +p: J.Json, drive: String) -> IO(Unit):
  match show:
    case True{}:
      do IO<Unit>:
        # Shield first, so a freshly mapped agent page cannot take a stray click.
        r : String <- CefJob.ask(m, "shield", J.Json.obj([J.KV{"on", J.Flag{True{}}}]))
        shown : String <- CefJob.ask(m, "show", p)
        CefJob.shield(m, J.Json.get_str(p, "page", ""), drive)
    case False{}:
      IO.bind(String, Unit, CefJob.ask(m, "hide", J.Null{}), r => IO.pure(Unit, Unit{}))

def CefJob.done(+inbox: Chan(Msg), request: String, live: String, err: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, CefDone{request, live, err})
    return Unit{}

def CefJob.switched(ok: Bool, +m: Maybe<&2, Chan(BR.BMsg)>, +inbox: Chan(Msg), +request: String, +args: J.Json, +err: String) -> IO(Unit):
  match ok:
    case True{}: CefJob.done(inbox, request, J.Json.get_str(args, "engine", "chrome"), "")
    case False{}:
      do IO<Unit>:
        r : String <- CefJob.engine_ask(m, False{}, CF.Cef.args(False{}, 0, ""))
        CefJob.done(inbox, request, "chrome", err)

def CefJob.reply(+r: J.Json, +m: Maybe<&2, Chan(BR.BMsg)>, +inbox: Chan(Msg), request: String, args: J.Json) -> IO(Unit):
  CefJob.switched(J.Json.get_bool(r, "ok", False{}), m, inbox, request, args, J.Json.get_str(r, "error", "CEF host failed"))

def CefJob.engine(+m: Maybe<&2, Chan(BR.BMsg)>, +inbox: Chan(Msg), +args: J.Json, token: String) -> IO(Unit):
  do IO<Unit>:
    r : String <- CefJob.engine_ask(m, String.eq(J.Json.get_str(args, "engine", ""), "cef"), args)
    CefJob.reply(Maybe.default(&2, J.Json, J.Json.parse(r), J.Null{}), m, inbox, token, args)

# A whole native operation finishes before the next begins. In particular,
# hide cannot overtake show's shield step, and stale engine replies are ignored.
def CefQueue.submit(m: Maybe<&2, Chan(CF.CefWork)>, w: CF.CefWork) -> IO(Unit):
  match m:
    case Some{q}:
      do IO<Unit>:
        ok : Result<&1, &1, CF.CefWork, Unit> <- Chan.send(CF.CefWork, q, w)
        return Unit{}
    case None{}: IO.pure(Unit, Unit{})

def CefQueue.act(op: Nat, +m: Maybe<&2, Chan(BR.BMsg)>, +inbox: Chan(Msg), +args: J.Json, drive: String, token: String) -> IO(Unit):
  match op:
    case 0n: CefJob.engine(m, inbox, args, token)
    case 1n: CefJob.place(CF.Cef.visible(args), m, args, drive)
    case 2n: CefJob.shield(m, J.Json.get_str(args, "page", ""), drive)
    case _: CefJob.switched(False{}, m, inbox, token, J.Null{}, J.Json.get_str(args, "error", "CEF host exited"))

def CefQueue.run(w: CF.CefWork, m: Maybe<&2, Chan(BR.BMsg)>, inbox: Chan(Msg)) -> IO(Unit):
  match w:
    case CF.CefWork{op, args, drive, token}:
      CefQueue.act(C.Codec.pos(["engine", "place", "shield"], op), m, inbox, args, drive, token)

def CefQueue.got(w: Maybe<&1, CF.CefWork>, +m: Maybe<&2, Chan(BR.BMsg)>, +inbox: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match w:
    case Some{work}: IO.bind(Unit, Unit, CefQueue.run(work, m, inbox), k)
    case None{}: IO.pure(Unit, Unit{})

def CefQueue.loop(fuel: Nat, +q: Chan(CF.CefWork), +m: Maybe<&2, Chan(BR.BMsg)>, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, CF.CefWork>, Unit, Chan.recv(CF.CefWork, q), w => CefQueue.got(w, m, inbox, u => CefQueue.loop(f, q, m, inbox)))

def Hub.cef.queue.ensure(known: Bool, h: Hub) -> IO(Hub):
  match known h:
    case True{} _: IO.pure(Hub, h)
    case False{} Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        +q : Chan(CF.CefWork) <- Chan.new(CF.CefWork, 256)
        IO.spawn(Unit, CefQueue.loop(Fuel.forever(), q, Cfg.br(cfg), inbox))
        return Hub{st, f, ls, cl, as, inbox, Cfg.with_cef(cfg, CF.Cef.session.queued(Cfg.cef(cfg), q)), note}

def CefQueue.known(m: Maybe<&2, Chan(CF.CefWork)>) -> Bool:
  match m:
    case Some{_}: True{}
    case None{}: False{}

# the placed page's shield follows its drive, sent only when it changed
def Hub.cef.shield.go(due: Bool, h: Hub, +drive: String) -> IO(Hub):
  match due h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        CefQueue.submit(CF.Cef.session.queue(Cfg.cef(cfg)), CF.CefWork{"shield", CF.Cef.session.place(Cfg.cef(cfg)), drive, ""})
        return Hub{st, f, ls, cl, as, inbox, Cfg.with_cef(cfg, CF.Cef.session.shield_set(Cfg.cef(cfg), CF.Cef.shield.val(drive))), note}
    case False{} _: IO.pure(Hub, h)

def Hub.cef.driven(h: Hub, +b: TE.TeachBot) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.shield.go(Bool.and(Bool.and(String.eq(CF.Cef.session.live(Cfg.cef(cfg)), "cef"), String.eq(TE.Teach.bot.id(b), CF.Cef.session.page(Cfg.cef(cfg)))),
        CF.Cef.shield.due(CF.Cef.session.shield(Cfg.cef(cfg)), TE.Teach.bot.drive(b))), Hub{st, f, ls, cl, as, inbox, cfg, note}, TE.Teach.bot.drive(b))

def Hub.tch.put(h: Hub, +b: TE.TeachBot) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        Desktop.policy(Cfg.br(cfg), TE.Teach.bot.id(b), Cef.page.ok(st, TE.Teach.bot.id(b)), TE.Teach.put(Cfg.tch(cfg), b))
        cl2 : List<&2, Client> <- Wire.all(cl, TE.Teach.msg(b))
        Hub.cef.driven(Hub{st, f, ls, cl2, as, inbox, Cfg.with_tch(cfg, TE.Teach.put(Cfg.tch(cfg), b)), note}, b)

def Hub.tch.ok(h: Hub, +client: U32, +id: String) -> IO(Hub):
  Hub.out(h, B.Reply.ok(client, id))

def Hub.tch.err(h: Hub, +client: U32, +id: String, +msg: String) -> IO(Hub):
  Hub.out(h, B.Reply.err(client, id, msg))

# the draft's newest step joins its file, and clients hear the draft
def Hub.tch.step(h: Hub, +b: TE.TeachBot) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.draft.add(Cfg.home(cfg), TE.Teach.bot.id(b), TE.Teach.last(b)))
        Hub.tch.put(Hub{st, f, ls, cl, as, inbox, cfg, note}, b)

# recording off, the draft kept for review
def Hub.tch.stop(h: Hub, +b: TE.TeachBot) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.record(Cfg.br(cfg), TE.Teach.bot.id(b), False{}))
        Hub.tch.put(Hub{st, f, ls, cl, as, inbox, cfg, note}, TE.Teach.stop(b))

def Hub.browser.go(h: Hub, +client: U32, +id: String, +o: J.Json) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.input(Cfg.br(cfg), J.Json.get_str(o, "bot", ""), o))
        Hub.tch.ok(Hub{st, f, ls, cl, as, inbox, cfg, note}, client, id)

# the person's input goes through only while they have the browser
def Hub.browser.input(idle: Bool, h: Hub, +client: U32, +id: String, +o: J.Json) -> IO(Hub):
  match idle:
    case True{}:
      Hub.tch.err(h, client, id, "take over the browser first")
    case False{}:
      Hub.browser.go(h, client, id, o)

def Hub.browser.k(i: Nat, h: Hub, +client: U32, +id: String, +o: J.Json, +b: TE.TeachBot) -> IO(Hub):
  match i:
    case 0n:
      IO.bind(Hub, Hub, Hub.tch.put(h, TE.Teach.takeover(b)), h2 => Hub.tch.ok(h2, client, id))
    case 1n:
      IO.bind(Hub, Hub, Hub.tch.put(h, TE.Teach.handback(b)), h2 => Hub.tch.ok(h2, client, id))
    case _:
      Hub.browser.input(String.is_empty(TE.Teach.bot.drive(b)), h, client, id, o)

# bots.browser {bot, op, ...}
# Toolbar commands are explicit person input: take over before sending them.
# The helper reply comes back asynchronously, including navigation failures.
def BrowseJob.reply(+inbox: Chan(Msg), +client: U32, +id: String, +line: String) -> IO(Unit):
  +o = Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})
  Far.send(inbox, Push{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}},
    J.KV{"ok", J.Flag{J.Json.get_bool(o, "ok", False{})}}, J.KV{"error", J.Str{J.Json.get_str(o, "error", "")}}]))})

def BrowseJob.run(m: Maybe<&2, Chan(BR.BMsg)>, +inbox: Chan(Msg), +client: U32, +id: String, +o: J.Json) -> IO(Unit):
  match m:
    case Some{q}:
      IO.bind(String, Unit, BR.Browser.ask_on(q, J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "command", ""), CF.Cef.toolbar.args(o)),
        line => BrowseJob.reply(inbox, client, id, line))
    case None{}: BrowseJob.reply(inbox, client, id, BR.Browser.fail("browser helper unavailable"))

def Hub.browser.toolbar.start(h: Hub, +client: U32, +id: String, +o: J.Json) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, BrowseJob.run(Cfg.br(cfg), inbox, client, id, o))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.browser.toolbar.ok(ok: Bool, h: Hub, +client: U32, +id: String, +o: J.Json, +b: TE.TeachBot) -> IO(Hub):
  match ok:
    case True{}:
      IO.bind(Hub, Hub, Hub.tch.put(h, TE.Teach.takeover(b)), h2 => Hub.browser.toolbar.start(h2, client, id, o))
    case False{}: Hub.tch.err(h, client, id, "unsupported browser toolbar command")

def Hub.browser.toolbar(toolbar: Bool, h: Hub, +client: U32, +id: String, +o: J.Json, +b: TE.TeachBot) -> IO(Hub):
  match toolbar:
    case True{}: Hub.browser.toolbar.ok(CF.Cef.toolbar.allowed(J.Json.get_str(o, "command", "")), h, client, id, o, b)
    case False{}: Hub.browser.k(C.Codec.pos(["takeover", "handback"], J.Json.get_str(o, "op", "")), h, client, id, o, b)

def Hub.browser.valid(h: Hub, +client: U32, +id: String, +o: J.Json) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.browser.toolbar(String.eq(J.Json.get_str(o, "op", ""), "toolbar"), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, id, o,
        TE.Teach.get(Cfg.tch(cfg), J.Json.get_str(o, "bot", "")))

def Hub.browser.known(ok: Bool, h: Hub, +client: U32, +id: String, +o: J.Json) -> IO(Hub):
  match ok:
    case True{}: Hub.browser.valid(h, client, id, o)
    case False{}: Hub.tch.err(h, client, id, "unknown browser owner")

def Hub.browser(h: Hub, +client: U32, +id: String, +o: J.Json) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.browser.known(Cef.page.ok(st, J.Json.get_str(o, "bot", "")), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, id, o)

def Hub.teach.start(h: Hub, +client: U32, +id: String, +bot: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.begin(Cfg.br(cfg), Cfg.home(cfg), bot))
        h2 : Hub <- Hub.tch.put(Hub{st, f, ls, cl, as, inbox, cfg, note}, TE.Teach.start(bot, now))
        Hub.tch.ok(h2, client, id)

def Hub.teach.note(h: Hub, +client: U32, +id: String, +b: TE.TeachBot, +text: String) -> IO(Hub):
  do IO<Hub>:
    h2 : Hub <- Hub.tch.step(h, TE.Teach.add(b, TE.Teach.note(String.take(String.trim(text), 500n))))
    Hub.tch.ok(h2, client, id)

def Hub.teach.stop(h: Hub, +client: U32, +id: String, +b: TE.TeachBot) -> IO(Hub):
  do IO<Hub>:
    h2 : Hub <- Hub.tch.stop(h, b)
    Hub.tch.ok(h2, client, id)

def Hub.teach.discard(h: Hub, +client: U32, +id: String, +bot: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.record(Cfg.br(cfg), bot, False{}))
        IO.spawn(Unit, Tk.draft.clear(Cfg.home(cfg), bot))
        h2 : Hub <- Hub.tch.put(Hub{st, f, ls, cl, as, inbox, cfg, note}, TE.Teach.blank(bot))
        Hub.tch.ok(h2, client, id)

# save: the draft, with the marked values as parameters, is the person's
# task (a TaskSet and its file); the bot is woken to learn it
def Hub.teach.save.go(h: Hub, +client: U32, +id: String, +b: TE.TeachBot, +o: J.Json, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +bot : String = TE.Teach.bot.id(b)
        +name : String = String.trim(J.Json.get_str(o, "name", ""))
        +task : TKS.TkTask = TE.Teach.build(b, name, J.Json.get_str(o, "description", ""), J.Json.elems(J.Json.get_obj(o, "params")), now)
        +text : String = TE.Teach.task.text(task)
        +slug : String = TE.Teach.task.slug(task)
        +step : B.Step = BH.Step.cat(B.Step{[M.TaskSet{bot, slug, text, now}], [B.Reply.ok(client, id), B.BotFx{"task-file", bot, slug, text}]},
          BH.Bots.wake(st, now, "", BH.Bots.bot(st, bot), BH.Bots.msg(now, B.Hub.seq(st), 0n), TE.Teach.taught(name, task), 0n))
        h2 : Hub <- Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, step)
        IO.spawn(Unit, Tk.record(Cfg.br(cfg), bot, False{}))
        IO.spawn(Unit, Tk.draft.clear(Cfg.home(cfg), bot))
        Hub.tch.put(h2, TE.Teach.blank(bot))

def Hub.teach.save(bad: Bool, h: Hub, +client: U32, +id: String, +b: TE.TeachBot, +o: J.Json, +now: Nat) -> IO(Hub):
  match bad:
    case True{}:
      Hub.tch.err(h, client, id, "name the task and record at least one step")
    case False{}:
      Hub.teach.save.go(h, client, id, b, o, now)

def Hub.teachop.k(i: Nat, h: Hub, +client: U32, +id: String, +o: J.Json, +b: TE.TeachBot, +now: Nat) -> IO(Hub):
  match i:
    case 0n:
      Hub.teach.start(h, client, id, J.Json.get_str(o, "bot", ""), now)
    case 1n:
      Hub.teach.note(h, client, id, b, J.Json.get_str(o, "text", ""))
    case 2n:
      Hub.teach.stop(h, client, id, b)
    case 3n:
      Hub.teach.discard(h, client, id, J.Json.get_str(o, "bot", ""))
    case 4n:
      Hub.teach.save(Bool.or(String.is_empty(String.trim(J.Json.get_str(o, "name", ""))), List.is_empty(&2, TKS.TkStep, TE.Teach.bot.steps(b))), h, client, id, b, o, now)
    case _:
      Hub.tch.err(h, client, id, "unknown teach request")

# bots.teach {bot, op: start | note | stop | discard | save, ...}
def Hub.teachop(h: Hub, +client: U32, +id: String, +o: J.Json, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.teachop.k(C.Codec.pos(["start", "note", "stop", "discard", "save"], J.Json.get_str(o, "op", "")), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, id, o,
        TE.Teach.get(Cfg.tch(cfg), J.Json.get_str(o, "bot", "")), now)

def Hub.teach.req(i: Nat, h: Hub, +client: U32, +id: String, +o: J.Json, +now: Nat) -> IO(Hub):
  match i:
    case 0n:
      Hub.browser(h, client, id, o)
    case 1n:
      Hub.teachop(h, client, id, o, now)
    case _:
      Hub.tch.err(h, client, id, "unknown request")

def Hub.teach(h: Hub, +client: U32, +id: String, +m: String, +p: String, +now: Nat) -> IO(Hub):
  Hub.teach.req(C.Codec.pos(["bots.browser", "bots.teach"], m), h, client, id, Maybe.default(&2, J.Json, J.Json.parse(p), J.Null{}), now)

# a step the recorder saw joins the bot's draft while it records
def Hub.rec.add(added: Bool, h: Hub, b: TE.TeachBot) -> IO(Hub):
  match added:
    case True{}:
      Hub.tch.step(h, b)
    case False{}:
      IO.pure(Hub, h)

def Hub.rec.k(+b: TE.TeachBot, h: Hub, +step: J.Json) -> IO(Hub):
  Hub.rec.add(Nat.is_lt(List.length(&2, TKS.TkStep, TE.Teach.bot.steps(b)), List.length(&2, TKS.TkStep, TE.Teach.bot.steps(TE.Teach.add(b, step)))), h, TE.Teach.add(b, step))

def Hub.rec(h: Hub, +bot: String, +step: J.Json) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.rec.k(TE.Teach.get(Cfg.tch(cfg), bot), Hub{st, f, ls, cl, as, inbox, cfg, note}, step)

def Hub.browser_ev.k(rec: Bool, h: Hub, +bot: String, +step: J.Json) -> IO(Hub):
  match rec:
    case True{}:
      Hub.rec(h, bot, step)
    case False{}:
      IO.pure(Hub, h)

# The private desktop candidate carries the original epoch/lease, capture
# revision and document through both queues. No document metadata is broadcast.
# This pure state replacement is the Teach commit point. Release the broker
# fence BEFORE any policy message, client broadcast or best-effort draft IO.
def Hub.desktop_record.finished(added: Bool, h: Hub, +b: TE.TeachBot) -> IO(Hub):
  match added:
    case False{}: IO.pure(Hub, h)
    case True{}:
      match h:
        case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
          do IO<Hub>:
            IO.spawn(Unit, Tk.draft.add(Cfg.home(cfg), TE.Teach.bot.id(b), TE.Teach.last(b)))
            Hub.tch.put(Hub{st, f, ls, cl, as, inbox, cfg, note}, b)

def Hub.desktop_record.commit(h: Hub, +candidate: J.Json, +released: Chan(Unit)) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +e : J.Json = J.Json.get_obj(J.Json.get_obj(candidate, "payload"), "event")
        +old : TE.TeachBot = TE.Teach.get(Cfg.tch(cfg), J.Json.get_str(e, "owner", ""))
        +b : TE.TeachBot = TE.Teach.add(old, J.Json.get_obj(e, "step"))
        +added : Bool = Nat.is_lt(List.length(&2, TKS.TkStep, TE.Teach.bot.steps(old)), List.length(&2, TKS.TkStep, TE.Teach.bot.steps(b)))
        committed : Hub = Hub{st, f, ls, cl, as, inbox, Cfg.with_tch(cfg, TE.Teach.put(Cfg.tch(cfg), b)), note}
        done : Result<&1, &1, Unit, Unit> <- Chan.send(Unit, released, Unit{})
        Hub.desktop_record.finished(added, committed, b)

def Hub.desktop_record.permitted(valid: Bool, h: Hub, candidate: J.Json, +released: Chan(Unit)) -> IO(Hub):
  match valid:
    case False{}:
      do IO<Hub>:
        Chan.close(Unit, released)
        return h
    case True{}: Hub.desktop_record.commit(h, candidate, released)

def Hub.desktop_record.received(r: Maybe<&1, J.Json>, h: Hub, +released: Chan(Unit)) -> IO(Hub):
  match r:
    case None{}:
      do IO<Hub>:
        Chan.close(Unit, released)
        return h
    case Some{+candidate}:
      Hub.desktop_record.permitted(String.eq(J.Json.get_str(candidate, "event", ""), "desktop-record-candidate"), h, candidate, released)

def Hub.desktop_record.sent(r: Result<&1, &1, BR.BMsg, Unit>, h: Hub, +permit: Chan(J.Json), +released: Chan(Unit)) -> IO(Hub):
  match r:
    case Fail{_}:
      do IO<Hub>:
        Chan.close(Unit, released)
        return h
    case Done{_}:
      IO.bind(Maybe<&1, J.Json>, Hub, Chan.recv(J.Json, permit), r => Hub.desktop_record.received(r, h, released))

def Hub.desktop_record.request(ok: Bool, m: Maybe<&2, Chan(BR.BMsg)>, h: Hub, candidate: J.Json) -> IO(Hub):
  match ok m:
    case True{} Some{q}:
      do IO<Hub>:
        +permit : Chan(J.Json) <- Chan.new(J.Json, 1)
        +released : Chan(Unit) <- Chan.new(Unit, 1)
        sent : Result<&1, &1, BR.BMsg, Unit> <- Chan.send(BR.BMsg, q, BR.BCaptureCommit{candidate, permit, released})
        Hub.desktop_record.sent(sent, h, permit, released)
    case _ _:
      IO.pure(Hub, h)

def Hub.desktop_record(h: Hub, +candidate: J.Json) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      +owner = J.Json.get_str(J.Json.get_obj(J.Json.get_obj(candidate, "payload"), "event"), "owner", "")
      Hub.desktop_record.request(Bool.and(Cef.page.ok(st, owner), TE.Teach.recording(TE.Teach.get(Cfg.tch(cfg), owner))), Cfg.br(cfg), Hub{st, f, ls, cl, as, inbox, cfg, note}, candidate)

def Hub.browser_ev.record_kind(desktop: Bool, h: Hub, +o: J.Json) -> IO(Hub):
  match desktop:
    case True{}: Hub.desktop_record(h, o)
    case False{}: Hub.browser_ev.k(String.eq(J.Json.get_str(o, "event", ""), "record"), h, J.Json.get_str(o, "page", ""), J.Json.get_obj(o, "step"))

def Hub.browser_ev.record(h: Hub, +o: J.Json) -> IO(Hub):
  Hub.browser_ev.record_kind(String.eq(J.Json.get_str(o, "event", ""), "desktop-record-candidate"), h, o)

# a press on a page's shield: the person takes the page over (the shield
# then comes off and the page is focused: Hub.tch.put); no reply goes out
def Hub.cef.click.k(take: Bool, h: Hub, +b: TE.TeachBot) -> IO(Hub):
  match take:
    case True{}:
      Hub.tch.put(h, TE.Teach.takeover(b))
    case False{}:
      IO.pure(Hub, h)

def Hub.cef.click(real: Bool, h: Hub, +page: String) -> IO(Hub):
  match real h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.click.k(CF.Cef.shield.take(TE.Teach.bot.drive(TE.Teach.get(Cfg.tch(cfg), page))), Hub{st, f, ls, cl, as, inbox, cfg, note}, TE.Teach.get(Cfg.tch(cfg), page))
    case False{} _:
      IO.pure(Hub, h)

def Hub.cef.click.o(h: Hub, +o: J.Json) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.cef.click(Cef.page.ok(st, J.Json.get_str(o, "page", "")), Hub{st, f, ls, cl, as, inbox, cfg, note}, J.Json.get_str(o, "page", ""))

# the CEF host failed while live: back to chrome, through the queue
def Hub.cef.hostdead(failed: Bool, h: Hub, +o: J.Json) -> IO(Hub):
  match failed h:
    case True{} Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        CefQueue.submit(CF.Cef.session.queue(Cfg.cef(cfg)), CF.CefWork{"failed", o, "", CF.Cef.session.token(Cfg.cef(cfg))})
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _: IO.pure(Hub, h)

def Hub.cef.engine_ev(h: Hub, +o: J.Json) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.hostdead(Bool.and(String.eq(CF.Cef.session.live(Cfg.cef(cfg)), "cef"), Bool.not(J.Json.get_bool(o, "ok", True{}))),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, o)

# the helper exited: it is started again (with the engine's flags) by
# showing the placed page and its shield again, a few times at most
def Hub.cef.revive(go: Bool, h: Hub) -> IO(Hub):
  match go h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        CefQueue.submit(CF.Cef.session.queue(Cfg.cef(cfg)), CF.CefWork{"place", CF.Cef.session.place(Cfg.cef(cfg)),
          TE.Teach.bot.drive(TE.Teach.get(Cfg.tch(cfg), CF.Cef.session.page(Cfg.cef(cfg)))), ""})
        return Hub{st, f, ls, cl, as, inbox, Cfg.with_cef(cfg, CF.Cef.session.tried(Cfg.cef(cfg))), note}
    case False{} _: IO.pure(Hub, h)

def Hub.cef.exited(h: Hub) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.revive(CF.Cef.revive(String.eq(CF.Cef.session.live(Cfg.cef(cfg)), "cef"), CF.Cef.visible(CF.Cef.session.place(Cfg.cef(cfg))), CF.Cef.session.tries(Cfg.cef(cfg))),
        Hub{st, f, ls, cl, as, inbox, cfg, note})

# a page navigated: it exists; every client hears its one key (and the
# key of the oldest, if it was pushed out)
def Hub.cef.seen.k(new: Bool, h: Hub, +page: String) -> IO(Hub):
  match new h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.all(cl, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj(CF.Cef.pages.told(page, CF.Cef.pages.out(CF.Cef.session.pages(Cfg.cef(cfg)))))}])))
        return Hub{st, f, ls, cl2, as, inbox, Cfg.with_cef(cfg, CF.Cef.session.pages_set(Cfg.cef(cfg), CF.Cef.pages.add(CF.Cef.session.pages(Cfg.cef(cfg)), page))), note}
    case False{} _:
      IO.pure(Hub, h)

def Hub.cef.seen.o(h: Hub, +o: J.Json) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.seen.k(Bool.and(Cef.page.ok(st, J.Json.get_str(o, "page", "")), Bool.not(CF.Cef.pages.has(CF.Cef.session.pages(Cfg.cef(cfg)), J.Json.get_str(o, "page", "")))),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, J.Json.get_str(o, "page", ""))

def Hub.browser.tabs(h: Hub, +o: J.Json) -> IO(Hub):
  IO.bind(Hub, Hub, Hub.cef.seen.o(h, o), h2 => Hub.broadcast(h2, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"info"}},
    J.KV{"info", J.Json.obj([J.KV{"browser.tabs." ++ J.Json.get_str(o, "page", ""), J.Str{J.Json.show(o)}}])}]))))

def Hub.browser_ev.c(i: Nat, h: Hub, +o: J.Json) -> IO(Hub):
  match i:
    case 0n: Hub.browser_ev.record(h, o)
    case 1n: Hub.cef.engine_ev(h, o)
    case 2n: Hub.cef.click.o(h, o)
    case 3n: Hub.cef.exited(h)
    case 4n: Hub.cef.seen.o(h, o)
    case 5n: Hub.cef.seen.o(h, o)
    case 6n: Hub.browser.tabs(h, o)
    case _: IO.pure(Hub, h)

def Hub.browser_ev.record_dispatch(desktop: Bool, h: Hub, +o: J.Json) -> IO(Hub):
  match desktop:
    case True{}: Hub.desktop_record(h, o)
    case False{}: Hub.browser_ev.c(C.Codec.pos(["record", "engine", "shield-click", "helper-exit", "navigated", "load", "tabs"], J.Json.get_str(o, "event", "")), h, o)

def Hub.browser_ev.o(h: Hub, +o: J.Json) -> IO(Hub):
  Hub.browser_ev.record_dispatch(String.eq(J.Json.get_str(o, "event", ""), "desktop-record-candidate"), h, o)

# a helper event about a bot's page: a recorder step (navigations and loads
# need nothing here: the recorder reports the person's own navigations)
def Hub.browser_ev(h: Hub, +line: String) -> IO(Hub):
  Hub.browser_ev.o(h, Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}))

# the person answered a pause: the browser is the bot's again
def Hub.back.k(+b: TE.TeachBot, h: Hub) -> IO(Hub):
  Hub.tch.put(h, TE.Teach.handback(b))

def Hub.back(h: Hub, +bot: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.back.k(TE.Teach.get(Cfg.tch(cfg), bot), Hub{st, f, ls, cl, as, inbox, cfg, note})

# a recording ends after ten minutes
def Hub.tch.expire(yes: Bool, h: Hub, +b: TE.TeachBot) -> IO(Hub):
  match yes:
    case True{}:
      Hub.tch.stop(h, b)
    case False{}:
      IO.pure(Hub, h)

def Hub.tch.tick(bs: List<&2, TE.TeachBot>, h: Hub, +now: Nat) -> IO(Hub):
  match bs:
    case Nil{}:
      IO.pure(Hub, h)
    case +b <> t:
      IO.bind(Hub, Hub, Hub.tch.expire(TE.Teach.expired(b, now), h, b), h2 => Hub.tch.tick(t, h2, now))

def Hub.tch.ticks(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.tch.tick(TE.Teach.st.bots(Cfg.tch(cfg)), Hub{st, f, ls, cl, as, inbox, cfg, note}, now)

def Hub.rpc.k(p: M.State & Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, Bot.request(J.Json.parse(text), st, now, client, text))

def Hub.rpc.here(h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  Hub.rpc.k(Hub.state(h), client, text, now)

# A client's request about something elsewhere goes to its machine; any
# other stays here
def Far.rpc.go(m: Maybe<&2, FR.Route>, h: Hub, +client: U32, +o: J.Json, +text: String, +now: Nat) -> IO(Hub):
  match m h:
    case Some{FR.Route{+link, +meth, +body}} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      Far.rpc.pick(FV.Fv.near.is(meth), Far.own.id(st, cfg, link), Bool.or(FR.Fr.may(meth), FV.Fv.read(meth)), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, J.Json.as_raw(J.Json.get(o, "id"), "0"), link, meth, FR.Fr.nonced(body, U32.show(client) ++ "." ++ J.Json.as_raw(J.Json.get(o, "id"), "0") ++ "." ++ Nat.show(now)), now)
    case None{} _:
      Hub.rpc.here(h, client, text, now)

def Hub.rpc.o(m: Maybe<&2, J.Json>, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match m:
    case Some{+o}:
      Far.rpc.go(FR.Fr.route(o), h, client, o, text, now)
    case None{}:
      Hub.rpc.here(h, client, text, now)

# a request about something on a linked machine goes there (Far.rpc.go)
# whether a request may be for another machine: it names a thing there
# (`~`) or a machine (`on`); Hub.rpc.o decides from its parsed params
# (a text with JSON escapes is parsed whatever it holds: an escaped key or
# value could name a machine the raw text does not show)
def Hub.rpc.farish(+text: String) -> Bool:
  Bool.or(Bool.or(String.contains(text, "~"), String.contains(text, "\"on\"")), String.contains(text, "\\u"))

def Hub.rpc.far(maybe: Bool, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match maybe:
    case True{}:
      Hub.rpc.o(J.Json.parse(text), h, client, text, now)
    case False{}:
      Hub.rpc.here(h, client, text, now)

# `/give` naming threads or projects on the owner's other machines
# (fmcp.bend's Fm.give): each machine is asked in turn, off the hub, to
# queue the conversation (thread_handoff). When the text names nothing here
# the client's answer waits for them: ok only when every machine took it.
# Otherwise the hub's own part answers as usual (its text without the
# machine words, Fm.give.local), and a machine that did not take it is
# told to the client as an error.
type GiveTo is Data:
  GiveTo{link: String, url: String, body: String, own: Bool, err: String}

def Far.give.tos(ps: List<&2, FM.Plan>, +st: M.State, +cfg: Cfg) -> List<&2, GiveTo>:
  match ps:
    case Nil{}:
      Nil{}
    case FM.PFar{+link, +rq} <> t:
      GiveTo{link, Bot.peer.url(st, link), rq, Far.own.id(st, cfg, link), ""} <> Far.give.tos(t, st, cfg)
    case FM.PAnswer{r} <> t:
      GiveTo{"", "", "", False{}, FM.Fm.res.msg(r)} <> Far.give.tos(t, st, cfg)
    case _ <> t:
      Far.give.tos(t, st, cfg)

def Far.give.res.m(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, J.Json.get_bool(J.Json.get_obj(o, "mcp"), "ok", False{}), "",
        J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "mcp"), "body"), "error", J.Json.get_str(o, "error", "that machine did not take it")))
    case None{}:
      "that machine's answer was not understood"

def Far.give.res(+code: Nat, +body: String) -> String:
  Bool.pick(String, Nat.is_eq(code, 200n), Far.give.res.m(J.Json.parse(body)),
    Bool.pick(String, Nat.is_eq(code, 0n), "that machine is away", "that machine did not take it: " ++ String.take(body, 120n)))

def Far.give.ask(+home: String, +url: String, +link: String, +body: String) -> IO(String):
  do IO<String>:
    now : Nat <- Now.nat()
    +n : String <- BN.Bn.random(4n)
    +r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/rpc", True{}, FR.Fr.nonced(body, n))
    return Far.give.res(Far.code(r), Far.body(r))

def Far.give.one(g: GiveTo, +home: String) -> IO(String):
  match g:
    case GiveTo{+link, +url, +body, own, +err}:
      Bool.pick(IO(String), Bool.not(String.is_empty(err)), IO.pure(String, err),
        Bool.pick(IO(String), own, Far.give.ask(home, url, link, body), IO.pure(String, "sync with that machine is off here")))

def Far.give.keep(+e: String, +rest: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(e), rest, e <> rest)

def Far.give.run(gs: List<&2, GiveTo>, +home: String) -> IO(List<&2, String>):
  match gs:
    case Nil{}:
      IO.pure(List<&2, String>, Nil{})
    case g <> t:
      do IO<List<&2, String>>:
        +e : String <- Far.give.one(g, home)
        rest : List<&2, String> <- Far.give.run(t, home)
        return Far.give.keep(e, rest)

def Far.give.said(here: Bool, +client: U32, +id: String, es: List<&2, String>, +inbox: Chan(Msg)) -> IO(Unit):
  match here es:
    case False{} Nil{}:
      Far.send(inbox, Push{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{True{}}}]))})
    case False{} +e <> +t:
      Far.send(inbox, Push{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{String.join(e <> t, "; ")}}]))})
    case True{} Nil{}:
      IO.pure(Unit, Unit{})
    case True{} +e <> +t:
      Far.send(inbox, Push{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"update"}}, J.KV{"error", J.Str{"/give: " ++ String.join(e <> t, "; ")}}]))})

def Far.give.job(gs: List<&2, GiveTo>, here: Bool, +client: U32, +id: String, +home: String, +inbox: Chan(Msg)) -> IO(Unit):
  IO.bind(List<&2, String>, Unit, Far.give.run(gs, home), es => Far.give.said(here, client, id, es, inbox))

def Far.give.k(here: Bool, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match here:
    case True{}:
      Hub.rpc.far(Hub.rpc.farish(text), h, client, text, now)
    case False{}:
      IO.pure(Hub, h)

def Far.give.start(ps: List<&2, FM.Plan>, +here: Bool, h: Hub, +client: U32, +o: J.Json, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.give.job(Far.give.tos(ps, st, cfg), here, client, J.Json.as_raw(J.Json.get(o, "id"), "0"), Cfg.home(cfg), inbox))
        Far.give.k(here, Hub{st, f, ls, cl, as, inbox, cfg, note}, client, J.Json.show(FM.Fm.give.local(o)), now)

def Far.give.p(ps: List<&2, FM.Plan>, +st: M.State, h: Hub, +client: U32, +o: J.Json, +text: String, +now: Nat) -> IO(Hub):
  match ps:
    case Nil{}:
      Hub.rpc.far(Hub.rpc.farish(text), h, client, text, now)
    case +p <> +t:
      Far.give.start(p <> t, FM.Fm.give.here(st, FM.Fm.give.local(o)), h, client, o, now)

def Far.give(m: Maybe<&2, J.Json>, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match m h:
    case Some{+o} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Far.give.p(FM.Fm.give(st, FR.Fh.ins(BN.Bn.far(Cfg.bn(cfg))), o, now), st, Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, text, now)
    case None{} _:
      Hub.rpc.far(Hub.rpc.farish(text), h, client, text, now)

def Hub.rpc.g(give: Bool, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match give:
    case True{}:
      Far.give(J.Json.parse(text), h, client, text, now)
    case False{}:
      Hub.rpc.far(Hub.rpc.farish(text), h, client, text, now)

# Browser pages on show (castio.bend)
# -----------------------------------
# A client showing a page's Browser tab says browser.seen {page} about
# every second: it gets every frame of that page from then on (for a few
# seconds), and the helper's cast lease for the page is renewed (at most
# once a second), so the page's frames go out at up to --cast-fps.

def Wire.to.all(ids: List<&2, U32>, +cl: List<&2, Client>, +text: String) -> IO(List<&2, Client>):
  match ids:
    case Nil{}:
      IO.pure(List<&2, Client>, cl)
    case +i <> t:
      IO.bind(List<&2, Client>, List<&2, Client>, Wire.to(cl, i, text), cl2 => Wire.to.all(t, cl2, text))

def Hub.shot.to(h: Hub, +text: String, +page: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        ids : List<&2, U32> <- CST.Cast.to(page)
        cl2 : List<&2, Client> <- Wire.to.all(ids, cl, text)
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

def Cast.lease(due: Bool, m: Maybe<&2, Chan(BR.BMsg)>, +page: String) -> IO(Unit):
  match due m:
    case True{} Some{q}:
      IO.spawn(Unit, IO.bind(String, Unit, BR.Browser.ask_on(q, page, "cast", J.Json.obj([J.KV{"ms", J.Num{"2500"}}])), r => IO.pure(Unit, Unit{})))
    case _ _:
      IO.pure(Unit, Unit{})

def Hub.cast(h: Hub, +client: U32, +page: String, +id: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        due : Bool <- CST.Cast.seen(client, page)
        Cast.lease(due, Cfg.br(cfg), page)
        Hub.out(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Reply.ok(client, id))

def Hub.cast.o(h: Hub, +client: U32, +o: J.Json) -> IO(Hub):
  Hub.cast(h, client, J.Json.get_str(J.Json.get_obj(o, "p"), "page", ""), J.Json.as_raw(J.Json.get(o, "id"), "0"))

def Hub.rpc.c(seen: Bool, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match seen:
    case True{}:
      Hub.cast.o(h, client, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))
    case False{}:
      Hub.rpc.g(String.contains(text, "bots.give"), h, client, text, now)

def Hub.cef.say(+cp: CF.CefSession) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"info"}}, J.KV{"info", J.Json.obj(CF.Cef.session.info(cp))}]))

def Hub.cef.done(current: Bool, h: Hub, +live: String, +err: String) -> IO(Hub):
  match current h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +cp : CF.CefSession = CF.Cef.session.done(Cfg.cef(cfg), live, err)
        cl2 : List<&2, Client> <- Wire.all(cl, Hub.cef.say(cp))
        return Hub{st, f, ls, cl2, as, inbox, Cfg.with_cef(cfg, cp), note}
    case False{} _: IO.pure(Hub, h)

def Hub.cef.finished(h: Hub, +request: String, live: String, err: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.done(CF.Cef.current(Cfg.cef(cfg), request), Hub{st, f, ls, cl, as, inbox, cfg, note}, live, err)

# the engine the setting and the window's embed call for, asked of the
# helper when it differs from what was last asked (a short key compared)
def Hub.cef.reconcile.queue(h: Hub, +on: Bool, +key: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +cp : CF.CefSession = CF.Cef.session.done(CF.Cef.session.request_set(Cfg.cef(cfg), key), "chrome", "")
        CefQueue.submit(CF.Cef.session.queue(cp), CF.CefWork{"engine", CF.Cef.args(on, CF.Cef.session.xid(cp), CF.Cef.session.display(cp)), "", CF.Cef.session.token(cp)})
        cl2 : List<&2, Client> <- Wire.all(cl, Hub.cef.say(cp))
        return Hub{st, f, ls, cl2, as, inbox, Cfg.with_cef(cfg, cp), note}

def Hub.cef.reconcile.k(changed: Bool, h: Hub, +on: Bool, +key: String) -> IO(Hub):
  match changed h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      IO.bind(Hub, Hub, Hub.cef.queue.ensure(CefQueue.known(CF.Cef.session.queue(Cfg.cef(cfg))), Hub{st, f, ls, cl, as, inbox, cfg, note}), h2 => Hub.cef.reconcile.queue(h2, on, key))
    case False{} _: IO.pure(Hub, h)

def Hub.cef.reconcile.on(h: Hub, +on: Bool) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.reconcile.k(Bool.not(String.eq(CF.Cef.session.request(Cfg.cef(cfg)), CF.Cef.want.key(on, CF.Cef.session.xid(Cfg.cef(cfg)), CF.Cef.session.display(Cfg.cef(cfg))))),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, on, CF.Cef.want.key(on, CF.Cef.session.xid(Cfg.cef(cfg)), CF.Cef.session.display(Cfg.cef(cfg))))

def Hub.cef.reconcile(h: Hub) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.reconcile.on(Hub{st, f, ls, cl, as, inbox, cfg, note}, CF.Cef.engine(CF.Cef.requested(M.Setting.get(st, "browser.engine", CF.Cef.default())), CF.Cef.session.known(Cfg.cef(cfg)), True{}))

# only after the messages that can change the engine (Cef.after)
def Hub.cef.after(go: Bool, h: Hub) -> IO(Hub):
  match go:
    case True{}: Hub.cef.reconcile(h)
    case False{}: IO.pure(Hub, h)

def Hub.cef.near(h: Hub, +client: U32) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      IO.pure(Hub, Hub{st, f, ls, cl, as, inbox, Cfg.with_cef(cfg, CF.Cef.session.near(Cfg.cef(cfg), client)), note})

def Hub.cef.place(ok: Bool, h: Hub, +client: U32, +id: String, +p: J.Json) -> IO(Hub):
  match ok h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +drive : String = TE.Teach.bot.drive(TE.Teach.get(Cfg.tch(cfg), J.Json.get_str(p, "page", "")))
        CefQueue.submit(CF.Cef.session.queue(Cfg.cef(cfg)), CF.CefWork{"place", p, drive, ""})
        Hub.tch.ok(Hub{st, f, ls, cl, as, inbox, Cfg.with_cef(cfg, CF.Cef.session.place_set(Cfg.cef(cfg), p, CF.Cef.shield.val(drive))), note}, client, id)
    case False{} _: Hub.tch.err(h, client, id, "not the embedded browser window or unknown page")

def Hub.cef.embed(ok: Bool, h: Hub, +client: U32, +id: String, +p: J.Json) -> IO(Hub):
  match ok h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.tch.ok(Hub{st, f, ls, cl, as, inbox, Cfg.with_cef(cfg, CF.Cef.session.embed_set(Cfg.cef(cfg), client, U32.from_nat(J.Json.get_nat(p, "xid", 0n)), J.Json.get_str(p, "display", ""))), note}, client, id)
    case False{} _: Hub.tch.err(h, client, id, "only this machine's window can embed the browser")

# browser.embed {xid, display} and browser.place {page, x, y, w, h} or {}:
# only from this machine's window (Cef.from); a place only from the window
# that embedded, only while cef runs, and only for a real page
def Hub.cef.rpc.k(embed: Bool, h: Hub, +client: U32, +id: String, +p: J.Json) -> IO(Hub):
  match embed h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.embed(CF.Cef.from(client, CF.Cef.session.is_local(Cfg.cef(cfg), client)), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, id, p)
    case False{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.cef.place(Bool.and(Bool.and(CF.Cef.from(client, CF.Cef.session.is_local(Cfg.cef(cfg), client)), U32.is_eq(client, CF.Cef.session.client(Cfg.cef(cfg)))),
        Bool.and(String.eq(CF.Cef.session.live(Cfg.cef(cfg)), "cef"), Bool.or(Bool.not(CF.Cef.visible(p)), Cef.page.ok(st, J.Json.get_str(p, "page", ""))))),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, client, id, p)

def Hub.cef.rpc(cef: Bool, h: Hub, +client: U32, +text: String, +now: Nat, +o: J.Json) -> IO(Hub):
  match cef:
    case True{}: Hub.cef.rpc.k(String.eq(J.Json.get_str(o, "m", ""), "browser.embed"), h, client, J.Json.as_raw(J.Json.get(o, "id"), "0"), J.Json.get_obj(o, "p"))
    case False{}: Hub.rpc.c(String.eq(J.Json.get_str(o, "m", ""), "browser.seen"), h, client, text, now)

def Hub.rpc.ocef(h: Hub, +client: U32, +text: String, +now: Nat, +o: J.Json) -> IO(Hub):
  Hub.cef.rpc(Bool.or(String.eq(J.Json.get_str(o, "m", ""), "browser.embed"), String.eq(J.Json.get_str(o, "m", ""), "browser.place")), h, client, text, now, o)

def Hub.rpc.cefroute(cef: Bool, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match cef:
    case True{}: Hub.rpc.ocef(h, client, text, now, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))
    case False{}: Hub.rpc.c(String.contains(text, "\"browser.seen\""), h, client, text, now)

# A mobile connection asks for a recent presentation before starting its
# canonical log. It is an ordinary remote client: it never takes the trusted
# in-process WState/JoinHere path. The timeout falls back to the old join.
def Hub.presentation.timeout(mode: String, +inbox: Chan(Msg), client: U32, gen: U32) -> IO(Unit):
  match mode:
    case "mobile": IO.spawn(Unit, Self.post.wait(inbox, MobileTimeout{client, gen}, 8000))
    case _: IO.pure(Unit, Unit{})

def Hub.presentation.greeting(+mode: String, gen: U32) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{mode}}, J.KV{"protocol", J.Json.nat(1n)}, J.KV{"generation", J.Str{U32.show(gen)}}]))

def Hub.presentation.first(mode: String, gen: U32, epoch: String) -> String:
  match mode:
    case "view-mobile": J.Json.show(TVW.Wire.greeting("mobile", U32.show(gen), epoch))
    case "view-web": J.Json.show(TVW.Wire.greeting("web", U32.show(gen), epoch))
    case "view-invalid": J.Json.show(J.Json.obj([J.KV{"t", J.Str{"view.error"}}, J.KV{"error", TVW.Wire.error("INVALID_REQUEST")}]))
    case _: Hub.presentation.greeting(mode, gen)

def Hub.mobile.join(h: Hub, +client: U32, out: Chan(Wire), since: Nat, origin: String, +gen: U32, sock: U32, legacy: Bool, +mode: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        m : Maybe<&2, Client> <- Wire.text(Client{client, out, DK.Look.none(), OB.Cr.wait.set(OB.Cr.new(gen, False{}, True{}, 0n), True{}), sock, legacy, [MobileStart{since, origin, mode}]},
          Hub.presentation.first(mode, gen, Cfg.thin.epoch(cfg)) )
        Hub.presentation.timeout(mode, inbox, client, gen)
        return Hub{st, f, ls, Clients.add(m, cl), as, inbox, cfg, note}

# Preserve the writer's exact accumulated credit across the handshake.
# Reusing Hub.join's new-credit constructor would reset sent/acked totals
# while the same writer still reports the presentation frames it sent.
def Hub.mobile.history.begin(+n: Nat, +s0: Nat, +origin: String, +xs: List<&2, String>, h: Hub, c: Client) -> IO(Hub):
  match h c:
    case Hub{st, f, +ls, cl, as, +inbox, +cfg, note} Client{+id, out, look, +cr, sock, legacy, _}:
      +k = List.length(&2, String, OB.Join.next(xs))
      +more = Bool.or(Nat.is_lt(Nat.add(s0, k), n), Bool.not(List.is_empty(&2, FR.In, Far.mirrors(cfg))))
      do IO<Hub>:
        m : Maybe<&2, Client> <- Wire.offer(Client{id, out, look,
          PRESENT.Session.history_credit(cr, Nat.add(s0, k), more, Nil{}), sock, legacy, Nil{}},
          Hub.log_items(origin, cfg, s0, List.take(&2, String, xs, k), more), 0n)
        m1 : Maybe<&2, Client> <- Hub.join.live(more, m, cfg)
        Hub.join.more(more, inbox, id, OB.Cr.gen(cr))
        return Hub{st, f, ls, Clients.swap(cl, id, m1), as, inbox, cfg, note}

def Hub.mobile.history.window(+n: Nat, +origin: String, +since: Nat, oldOrigin: String, +ls: SO.Lg, h: Hub, c: Client) -> IO(Hub):
  +s0 = PRESENT.Session.resume_cursor(since, oldOrigin, origin, n)
  IO.bind(List<&2, String>, Hub, SO.Sto.lines(ls, s0, Join.take(n, s0)), xs => Hub.mobile.history.begin(n, s0, origin, xs, h, c))

def Hub.mobile.history.start(h: Hub, c: Client) -> IO(Hub):
  match h c:
    case Hub{st, f, +ls, cl, as, inbox, +cfg, note} Client{id, out, look, cr, sock, legacy, MobileStart{since, origin, _} <> _}:
      Hub.mobile.history.window(LogCount.n(cfg), Hub.origin(ls), since, origin, ls,
        Hub{st, f, ls, cl, as, inbox, cfg, note}, Client{id, out, look, cr, sock, legacy, Nil{}})
    case _ _:
      IO.pure(Hub, h)

def Hub.mobile.timeout.live(same: Bool, h: Hub, c: Client) -> IO(Hub):
  match same:
    case True{}: Hub.mobile.history.start(h, c)
    case False{}: IO.pure(Hub, h)

def Hub.mobile.timeout.k(m: Maybe<&2, Client>, h: Hub, gen: U32) -> IO(Hub):
  match m:
    case Some{+c}:
      Hub.mobile.timeout.live(U32.is_eq(OB.Cr.gen(Boot.r(c)), gen), h, c)
    case None{}:
      IO.pure(Hub, h)

def Hub.mobile.timeout(h: Hub, +client: U32, gen: U32) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      Hub.mobile.timeout.k(Clients.get(cl, client), Hub{st, f, ls, cl, as, inbox, cfg, note}, gen)

def Client.presentation.mode(c: Client) -> String:
  match c:
    case Client{_, _, _, _, _, _, MobileStart{_, _, mode} <> _}: mode
    case Client{_, _, _, _, _, _, ThinStart{mode, _} <> _}: mode
    case _: ""

def Client.presentation.resume(c: Client, resume: PRESENT.Resume) -> Client:
  match c resume:
    case Client{id, out, look, cr, sock, legacy, MobileStart{_, _, mode} <> _} PRESENT.Cursor{since, origin}:
      Client{id, out, look, cr, sock, legacy, [MobileStart{since, origin, mode}]}
    case _ _: c

def Hub.presentation.history.valid(valid: Bool, h: Hub, +client: U32, +id: String, c: Client) -> IO(Hub):
  match valid:
    case False{}: Hub.tch.err(h, client, id, "history needs the fully applied local cursor and origin")
    case True{}: IO.bind(Hub, Hub, Hub.mobile.history.start(h, c), h2 => Hub.tch.ok(h2, client, id))

def Hub.presentation.history(h: Hub, +client: U32, +id: String, +p: J.Json, +c: Client) -> IO(Hub):
  Hub.presentation.history.valid(PRESENT.Session.resume_allowed(Client.presentation.mode(c), PRESENT.Session.cursor(p)), h, client, id,
    Client.presentation.resume(c, PRESENT.Session.cursor(p)))

def Hub.frontend.result(a: UC.Act, +revision: Nat) -> J.Json:
  match a:
    case UC.Act{u, cs}: WEBFRONT.WebSession.result(u, J.Json.arr(PRESENT.Session.commands(cs)), revision)

def Hub.frontend.act(+u: UC.Ui, action: String, value: String) -> UC.Act:
  match action:
    case "": UC.Act{u, Nil{}}
    case _: WEBFRONT.WebSession.act(u, action, value)

def Hub.presentation.render(web: Bool, +st: M.State, +info: J.Json, +live: J.Json, +now: Nat, +p: J.Json) -> J.Json:
  match web:
    case False{}: MSE.Session.render(st, info, live, now, p)
    case True{}:
      Hub.frontend.result(Hub.frontend.act(WEBFRONT.WebSession.input(st, info, live, now, J.Json.get_str(p, "cid", ""), J.Json.get_obj(p, "session")),
        J.Json.get_str(p, "action", ""), J.Json.get_str(p, "value", "")), PRESENT.Session.number(p, "revision", 0n))

def Hub.mobile.rpc.action(history: Bool, +web: Bool, h: Hub, +client: U32, +id: String, +p: J.Json, now: Nat, c: Client) -> IO(Hub):
  match history h:
    case True{} _:
      Hub.presentation.history(h, client, id, p, c)
    case False{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.out(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Reply.data(client, id,
        Hub.presentation.render(web, st, Hub.info(cfg), J.Json.obj([J.KV{"t", J.Json.str("live")}, J.KV{"items", J.Json.arr(LV.Live.items(Live.of(cfg)))}]), now, p)))

def Hub.mobile.rpc.legacy.generation(same: Bool, history: Bool, +web: Bool, h: Hub, +client: U32, +id: String, p: J.Json, now: Nat, c: Client) -> IO(Hub):
  match same:
    case True{}: Hub.mobile.rpc.action(history, web, h, client, id, p, now, c)
    case False{}: Hub.tch.err(h, client, id, "the mobile connection changed; reconnect")

def Hub.mobile.rpc.reject_thin(thin: Bool, same: Bool, history: Bool, web: Bool, h: Hub, client: U32, id: String, p: J.Json, now: Nat, c: Client) -> IO(Hub):
  match thin:
    case True{}: Hub.tch.err(h, client, id, "on-demand view connections do not offer canonical history")
    case False{}: Hub.mobile.rpc.legacy.generation(same, history, web, h, client, id, p, now, c)

def Hub.mobile.rpc.generation(same: Bool, history: Bool, web: Bool, h: Hub, client: U32, id: String, p: J.Json, now: Nat, +c: Client) -> IO(Hub):
  Hub.mobile.rpc.reject_thin(Client.thin(c), same, history, web, h, client, id, p, now, c)

def Hub.mobile.rpc.have(history: Bool, +web: Bool, h: Hub, +client: U32, +id: String, +p: J.Json, +now: Nat, m: Maybe<&2, Client>) -> IO(Hub):
  match m:
    case Some{+c}:
      Hub.mobile.rpc.generation(String.eq(J.Json.get_str(p, "generation", ""), U32.show(OB.Cr.gen(Boot.r(c)))), history, web, h, client, id, p, now, c)
    case None{}:
      Hub.tch.err(h, client, id, "the mobile connection is no longer open")

def Hub.mobile.rpc(history: Bool, +web: Bool, h: Hub, +client: U32, +id: String, p: J.Json, now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      Hub.mobile.rpc.have(history, web, Hub{st, f, ls, cl, as, inbox, cfg, note}, client, id, p, now, Clients.get(cl, client))

def Hub.rpc.mobile.dispatch(history: Bool, web: Bool, h: Hub, +client: U32, +o: J.Json, now: Nat) -> IO(Hub):
  Hub.mobile.rpc(history, web, h, client, J.Json.as_raw(J.Json.get(o, "id"), "0"), J.Json.get_obj(o, "p"), now)

def Hub.rpc.mobile.kind(i: Nat, +o: J.Json, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match i:
    case 0n: Hub.rpc.mobile.dispatch(False{}, False{}, h, client, o, now)
    case 1n: Hub.rpc.mobile.dispatch(True{}, False{}, h, client, o, now)
    case 2n: Hub.rpc.mobile.dispatch(False{}, True{}, h, client, o, now)
    case 3n: Hub.rpc.mobile.dispatch(True{}, True{}, h, client, o, now)
    case _: Hub.rpc.cefroute(Bool.or(String.contains(text, "\"browser.embed\""), String.contains(text, "\"browser.place\"")), h, client, text, now)

# On-demand view RPCs are separate from optional early full-log presentation.
# This context is connection-local and never a kept-state/cache authority.
def ThinContext.blank() -> ThinContext:
  ThinContext{"", "", J.Null{}, "", 0n, 0n, Nil{}, 0n}

def Client.thin.context(+c: Client) -> ThinContext:
  match c:
    case Client{_, _, _, _, _, _, ThinStart{_, contexts} <> _}: Maybe.default(&2, ThinContext, List.head(&2, ThinContext, contexts), ThinContext.blank())
    case _: ThinContext.blank()

def ThinContext.view(+c: ThinContext) -> String:
  match c:
    case ThinContext{_, _, _, v, _, _, _, _}: v

def ThinContext.target(+c: ThinContext) -> J.Json:
  match c:
    case ThinContext{_, _, t, _, _, _, _, _}: t

def ThinContext.cid(+c: ThinContext) -> String:
  match c:
    case ThinContext{_, cid, _, _, _, _, _, _}: cid

def ThinContext.anchor(+c: ThinContext) -> Nat:
  match c:
    case ThinContext{_, _, _, _, _, a, _, _}: a

def ThinContext.hub(+c: ThinContext) -> String:
  match c:
    case ThinContext{h, _, _, _, _, _, _, _}: h

def ThinContext.actors(c: ThinContext) -> List<&2, TA.Actor>:
  match c:
    case ThinContext{_, _, _, _, _, _, actors, _}: actors

def ThinContext.control(c: ThinContext) -> Nat:
  match c:
    case ThinContext{_, _, _, _, _, _, _, control}: control

def ThinContext.with_actors(c: ThinContext, actors: List<&2, TA.Actor>) -> ThinContext:
  match c:
    case ThinContext{h, cid, t, view, revision, anchor, _, control}: ThinContext{h, cid, t, view, revision, anchor, actors, control}

def ThinContext.with_control(c: ThinContext, control: Nat) -> ThinContext:
  match c:
    case ThinContext{h, cid, t, view, revision, anchor, actors, _}: ThinContext{h, cid, t, view, revision, anchor, actors, control}

def Client.thin.with(+c: Client, +context: ThinContext) -> Client:
  match c:
    case Client{i, out, look, cr, sock, legacy, MobileStart{_, _, mode} <> _}: Client{i, out, look, cr, sock, legacy, [ThinStart{mode, [context]}]}
    case Client{i, out, look, cr, sock, legacy, ThinStart{mode, _} <> _}: Client{i, out, look, cr, sock, legacy, [ThinStart{mode, [context]}]}
    case _: c

def Hub.thin.error(h: Hub, +client: U32, +o: J.Json, +code: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.to(cl, client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", Maybe.default(&2, J.Json, J.Json.get(o, "id"), J.Null{})}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", TVW.Wire.error(code)}])))
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

def Hub.thin.result(+result: J.Json, +p: J.Json, +view: String, +renderer: String, +revision: Nat) -> J.Json:
  TV.Obj.put(TV.Obj.put(TV.Obj.put(TV.Obj.put(TV.Obj.put(TV.Obj.put(TV.Obj.put(TV.Obj.put(TV.Obj.put(TV.Obj.put(result,
    "version", J.Num{"1"}), "hub", J.Str{J.Json.get_str(p, "hub", "")}), "generation", J.Str{J.Json.get_str(p, "generation", "")}),
    "epoch", J.Str{J.Json.get_str(p, "epoch", "")}), "requestRevision", Maybe.default(&2, J.Json, J.Json.get(p, "requestRevision"), J.Null{})),
    "target", J.Json.get_obj(p, "target")), "viewId", J.Str{view}), "renderer", J.Str{renderer}), "viewRevision", J.Str{Nat.show(revision)}), "remoteReady", J.Flag{J.Json.get_bool(result, "remoteReady", False{})})

# Only the validated thin UI action success path calls this helper. Ordinary
# RPC/MCP, background refresh, rejected requests and oversized replies do not.
def Cfg.telemetry.ui_action(+st: M.State, cfg: Cfg, +o: J.Json, delivered: Bool, now: Nat) -> Cfg:
  match cfg:
    case Cfg{h, w, ho, p, pr, ds, v, nt, tm, views}:
      Cfg{h, w, ho, p, pr, ds, v, nt, TUI.Human.mark(True{}, delivered, st,
        J.Json.get_str(o, "m", ""), J.Json.get_str(J.Json.get_obj(o, "p"), "action", ""), tm, now), views}

def Hub.thin.deliver(+fits: Bool, h: Hub, +client: U32, +o: J.Json, +answer: J.Json, +size: Nat, +c: Client, +context: ThinContext, +now: Nat) -> IO(Hub):
  match fits h:
    case False{} _: Hub.thin.error(h, client, o, "LIMIT")
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        +cl2 : List<&2, Client> <- Wire.to.view(Clients.swap(cl, client, Some{Client.thin.with(c, context)}), client, answer, size)
        return Hub{st, f, ls, cl2, as, inbox, Cfg.telemetry.ui_action(st, cfg, o, Maybe.is_some(&2, Client, Clients.get(cl2, client)), now), note}

def Hub.thin.respond(h: Hub, +client: U32, +o: J.Json, +result: J.Json, +c: Client, +context: ThinContext, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      +p = J.Json.get_obj(o, "p")
      +answer = (J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", Maybe.default(&2, J.Json, J.Json.get(o, "id"), J.Null{})}, J.KV{"ok", J.Flag{True{}}},
        J.KV{"result", Hub.thin.result(result, p, ThinContext.view(context), Bool.pick(String, String.eq(Client.thin.mode(c), "view-web"), "web", "mobile"), Cfg.thin.revision(cfg))}]))
      +size = J.Json.bytes(answer)
      Hub.thin.deliver(Nat.is_le(size, Bool.pick(Nat, String.eq(J.Json.get_str(o, "m", ""), "view.search"), 65536n, 262144n)),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, answer, size, c, context, now)

def Hub.thin.cursor.empty(+empty: Bool, +text: String, +epoch: String, +target: J.Json, +query: String) -> Maybe<&2, Nat>:
  match empty:
    case True{}: Some{0n}
    case False{}: TVW.Cursor.read(text, epoch, target, query)

def Hub.thin.cursor(+p: J.Json, +epoch: String, +query: String) -> Maybe<&2, Nat>:
  +text = J.Json.get_str(p, "cursor", "")
  Hub.thin.cursor.empty(String.is_empty(text), text, epoch, J.Json.get_obj(p, "target"), query)

def Hub.thin.current(+p: J.Json, +context: ThinContext) -> Bool:
  Bool.and(Bool.not(String.is_empty(ThinContext.view(context))), Bool.and(String.eq(J.Json.get_str(p, "viewId", ""), ThinContext.view(context)),
    Bool.and(String.eq(J.Json.get_str(p, "hub", ""), ThinContext.hub(context)), TVW.Target.same(J.Json.get_obj(p, "target"), ThinContext.target(context)))))

def Hub.thin.query.valid(+p: J.Json) -> Bool:
  TV.Query.valid(J.Json.get_str(p, "query", ""))

def Hub.thin.open_view(same: Bool, old: String, fresh: String) -> String:
  match same:
    case True{}: old
    case False{}: fresh

def Hub.thin.context.for(+opening: Bool, +p: J.Json, +c: Client, +id: String, +anchor: Nat) -> ThinContext:
  match opening:
    case True{}: ThinContext{J.Json.get_str(p, "hub", ""), J.Json.get_str(p, "cid", ""), J.Json.get_obj(p, "target"),
      Hub.thin.open_view(Bool.and(Bool.not(String.is_empty(ThinContext.view(Client.thin.context(c)))), TVW.Target.same(ThinContext.target(Client.thin.context(c)), J.Json.get_obj(p, "target"))),
        ThinContext.view(Client.thin.context(c)), U32.show(OB.Cr.gen(Boot.r(c))) ++ ":" ++ id), PRESENT.Session.number(p, "requestRevision", 0n), anchor,
      ThinContext.actors(Client.thin.context(c)), ThinContext.control(Client.thin.context(c))}
    case False{}: ThinContext{ThinContext.hub(Client.thin.context(c)), ThinContext.cid(Client.thin.context(c)), ThinContext.target(Client.thin.context(c)), ThinContext.view(Client.thin.context(c)), PRESENT.Session.number(p, "requestRevision", 0n), anchor, ThinContext.actors(Client.thin.context(c)), ThinContext.control(Client.thin.context(c))}

def Hub.thin.input(+action: Bool, +p: J.Json, +cid: String) -> J.Json:
  match action:
    case True{}: TV.Obj.put(p, "cid", J.Str{cid})
    case False{}: TV.Obj.put(TV.Obj.put(p, "cid", J.Str{cid}), "action", J.Str{""})

def Hub.thin.rendered_valid(valid: Bool, actor: TA.Actor, result: J.Json, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, context: ThinContext) -> IO(Hub):
  match valid:
    case False{}: Hub.thin.error(h, client, o, "LIMIT")
    case True{}: Hub.thin.respond(h, client, o, result, c, ThinContext.with_actors(context, [actor]), now)

def Hub.thin.rendered(rendered: TA.Rendered, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, context: ThinContext) -> IO(Hub):
  match rendered:
    case TA.Rendered{+actor, result}: Hub.thin.rendered_valid(TA.Actor.capacity(actor), actor, result, h, client, o, c, now, context)

# A thread's page and the index the renderers read it from: the whole
# index's, or (with a store) a page index of just its rows (TSTO.Ts.page)
def Hub.thin.fetch(memory: Bool, +cfg: Cfg, +thread: String, +anchor: Nat) -> IO(TSTO.Page):
  match memory:
    case True{}: IO.pure(TSTO.Page, TSTO.Page{Cfg.thin(cfg), THIN.Index.page(Cfg.thin(cfg), thread, anchor, 32n)})
    case False{}: TSTO.Ts.page(Cfg.thin.store(cfg), thread, anchor, 32n, Cfg.thin(cfg))

def Hub.thin.render.page(+pg: TSTO.Page, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, +context: ThinContext, +p: J.Json, +key: String, +ms: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      +web = String.eq(Client.thin.mode(c), "view-web")
      +stream = TLIVE.Stream.get(Cfg.thin.streams(cfg), TV.Target.thread(st, J.Json.get_obj(p, "target")))
      +actor = TA.Actor.mail_cache(TA.Actor.refresh(ThinContext.actors(context), web, st, Hub.info(cfg),
        J.Json.obj([J.KV{"t", J.Str{"live"}}, J.KV{"items", J.Json.arr(Nil{})}]), now, p, ThinContext.view(context)), ThinContext.actors(context), Cfg.thin.mail(cfg))
      Hub.thin.rendered(TA.Rendered.remember(TA.Actor.render(actor, web, st, TSTO.Page.index(pg), p, Cfg.thin.epoch(cfg), now, TSTO.Page.ids(pg), stream), key, ms, stream),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now, context)

def Hub.thin.render.full(h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, +context: ThinContext, +anchor: Nat, +p: J.Json, +key: String, +ms: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +pg : TSTO.Page <- Hub.thin.fetch(String.is_empty(Cfg.thin.store(cfg)), cfg, TV.Target.thread(st, J.Json.get_obj(p, "target")), anchor)
        Hub.thin.render.page(pg, Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now, context, p, key, ms)

# a view.update whose render memo still holds (thin-actor.bend's
# Memo.reply: only the live row can have changed) answers with it spliced
# (thin-splice.bend); anything else renders in full and is remembered
def Hub.thin.render.memo(+m: Maybe<&2, J.Json>, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, +context: ThinContext, +anchor: Nat, +p: J.Json, +key: String, +ms: Nat) -> IO(Hub):
  match m:
    case Some{+res}:
      Hub.thin.respond(h, client, o, res, c, context, now)
    case None{}:
      Hub.thin.render.full(h, client, o, c, now, context, anchor, p, key, ms)

def Hub.thin.render.at(h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, +context: ThinContext, +anchor: Nat, +ms: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      +p = Hub.thin.input(String.eq(J.Json.get_str(o, "m", ""), "view.action"), J.Json.get_obj(o, "p"), ThinContext.cid(context))
      +web = String.eq(Client.thin.mode(c), "view-web")
      +thread = TV.Target.thread(st, J.Json.get_obj(p, "target"))
      +key = TA.Memo.key(M.State.seq(st), J.Json.show(Hub.info(cfg)), p, anchor, ThinContext.view(context), web, thread)
      Hub.thin.render.memo(TA.Memo.reply(TA.Actor.memos(ThinContext.actors(context)), String.eq(J.Json.get_str(o, "m", ""), "view.update"), key, ms, web,
        TLIVE.Stream.get(Cfg.thin.streams(cfg), thread), thread, Cfg.thin.epoch(cfg), J.Json.get_obj(p, "target"), TA.Actor.loc(ThinContext.actors(context))),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now, context, anchor, p, key, ms)

def Hub.thin.render(h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, +context: ThinContext, +anchor: Nat) -> IO(Hub):
  IO.bind(Nat, Hub, IO.now(), +ms => Hub.thin.render.at(h, client, o, c, now, context, anchor, ms))

# a search page: over the whole index, or (with a store) over the
# candidates it fetched (TSTO.Ts.search); the same rows either way
def Hub.thin.find(memory: Bool, +cfg: Cfg, +q: String, +thread: String, +before: Nat) -> IO(TSTO.Found):
  match memory:
    case True{}: IO.pure(TSTO.Found, TSTO.Found{THIN.Index.search(Cfg.thin(cfg), q, thread, before), Cfg.thin(cfg)})
    case False{}: TSTO.Ts.search(Cfg.thin.store(cfg), q, thread, before)

def Hub.thin.search.valid(+valid: Bool, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, +context: ThinContext, +cursor: Maybe<&2, Nat>) -> IO(Hub):
  match valid h cursor:
    case False{} _ _: Hub.thin.error(h, client, o, "LIMIT")
    case True{} _ None{}: Hub.thin.error(h, client, o, "STALE_CURSOR")
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note} Some{+before}:
      +p = J.Json.get_obj(o, "p")
      do IO<Hub>:
        +found : TSTO.Found <- Hub.thin.find(String.is_empty(Cfg.thin.store(cfg)), cfg, J.Json.get_str(p, "query", ""), TV.Target.thread(st, J.Json.get_obj(p, "target")), before)
        Hub.thin.respond(Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, TV.Search.result(TSTO.Found.page(found), TSTO.Found.index(found), p, Cfg.thin.epoch(cfg)), c, context, now)

def Hub.thin.page(+cursor: Maybe<&2, Nat>, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat) -> IO(Hub):
  match cursor:
    case None{}: Hub.thin.error(h, client, o, "STALE_CURSOR")
    case Some{+before}: Hub.thin.render(h, client, o, c, now, Hub.thin.context.for(False{}, J.Json.get_obj(o, "p"), c, "", before), before)

def Hub.thin.message_read(+read: TV.MessageRead, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat) -> IO(Hub):
  match read h:
    case TV.MessageRead{"", row, start} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.thin.respond(Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o,
        TV.Message.result(row, start, J.Json.get_obj(o, "p"), Cfg.thin.epoch(cfg)), c, Client.thin.context(c), now)
    case TV.MessageRead{code, _, _} _: Hub.thin.error(h, client, o, code)

def Hub.thin.live_message_result(valid: Bool, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, +stream: TLIVE.Stream, +start: Nat, +thread: String) -> IO(Hub):
  match valid h:
    case False{} _: Hub.thin.error(h, client, o, "STALE_CURSOR")
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.thin.respond(Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, TV.LiveMessage.result(stream, start, thread, J.Json.get_obj(o, "p"), Cfg.thin.epoch(cfg)), c, Client.thin.context(c), now)

def Hub.thin.live_message_cursor(cursor: Maybe<&2, Nat>, h: Hub, client: U32, o: J.Json, c: Client, now: Nat, +stream: TLIVE.Stream, thread: String) -> IO(Hub):
  match cursor:
    case None{}: Hub.thin.error(h, client, o, "STALE_CURSOR")
    case Some{+start}: Hub.thin.live_message_result(Bool.and(Nat.is_gt(TLIVE.Stream.size(stream), 0n), Nat.is_lt(start, THIN.Blocks.n(TLIVE.Stream.blocks(stream)))), h, client, o, c, now, stream, start, thread)

# the rows a message read may name, whole (TSTO.Ts.whole); the whole
# index has them already
def Hub.thin.whole(memory: Bool, +cfg: Cfg, +thread: String, +p: J.Json, +pg: TSTO.Page) -> IO(TSTO.Page):
  match memory:
    case True{}: IO.pure(TSTO.Page, pg)
    case False{}: TSTO.Ts.whole(Cfg.thin.store(cfg), thread, TSTO.Ts.cursor.ord(p), J.Json.get_str(p, "entryId", ""), pg)

def Hub.thin.message_kind(live: Bool, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, +thread: String) -> IO(Hub):
  match live h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      +stream = TLIVE.Stream.get(Cfg.thin.streams(cfg), thread)
      Hub.thin.live_message_cursor(TV.LiveMessage.start(stream, J.Json.get_obj(o, "p"), Cfg.thin.epoch(cfg)), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now, stream, thread)
    case False{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      +p = J.Json.get_obj(o, "p")
      do IO<Hub>:
        +pg : TSTO.Page <- Hub.thin.fetch(String.is_empty(Cfg.thin.store(cfg)), cfg, thread, ThinContext.anchor(Client.thin.context(c)))
        +whole : TSTO.Page <- Hub.thin.whole(String.is_empty(Cfg.thin.store(cfg)), cfg, thread, p, pg)
        Hub.thin.message_read(TV.Message.read(TSTO.Page.index(whole), thread, p, Cfg.thin.epoch(cfg), TSTO.Page.ids(pg), st), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now)

def Hub.thin.message(h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      +thread = TV.Target.thread(st, ThinContext.target(Client.thin.context(c)))
      Hub.thin.message_kind(String.eq(J.Json.get_str(J.Json.get_obj(o, "p"), "entryId", ""), TLIVE.Stream.id(thread)), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now, thread)

def Hub.thin.dispatch(+kind: Nat, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat) -> IO(Hub):
  match kind h:
    case 0n _: Hub.thin.render(h, client, o, c, now, Hub.thin.context.for(True{}, J.Json.get_obj(o, "p"), c, J.Json.as_raw(J.Json.get(o, "id"), "0"), 0n), 0n)
    case 1n Hub{st, f, ls, cl, as, inbox, +cfg, note}: Hub.thin.page(Hub.thin.cursor(J.Json.get_obj(o, "p"), Cfg.thin.epoch(cfg), ""), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now)
    case 2n Hub{st, f, ls, cl, as, inbox, +cfg, note}: Hub.thin.search.valid(Hub.thin.query.valid(J.Json.get_obj(o, "p")), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now, Client.thin.context(c), Hub.thin.cursor(J.Json.get_obj(o, "p"), Cfg.thin.epoch(cfg), J.Json.get_str(J.Json.get_obj(o, "p"), "query", "")))
    case 3n _: Hub.thin.render(h, client, TV.Obj.put(o, "p", TV.Obj.put(J.Json.get_obj(o, "p"), "action", J.Str{""})), c, now, Client.thin.context(c), ThinContext.anchor(Client.thin.context(c)))
    case 4n _: Hub.thin.render(h, client, o, c, now, Client.thin.context(c), ThinContext.anchor(Client.thin.context(c)))
    case 5n _: Hub.thin.message(h, client, o, c, now)
    case 6n _: Hub.thin.respond(h, client, o, J.Json.obj([J.KV{"remoteReady", J.Flag{False{}}}, J.KV{"cmds", J.Json.arr(Nil{})}, J.KV{"cancelled", J.Flag{False{}}}]), c, Client.thin.context(c), now)
    case _ _: Hub.thin.error(h, client, o, "INVALID_REQUEST")

def Hub.thin.authorized(+valid: Bool, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat, +kind: Nat) -> IO(Hub):
  match valid:
    case False{}: Hub.thin.error(h, client, o, "STALE_VIEW")
    case True{}: Hub.thin.dispatch(kind, h, client, o, c, now)

def Hub.thin.request(+valid: Bool, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat) -> IO(Hub):
  match valid h:
    case False{} _: Hub.thin.error(h, client, o, "STALE_CONNECTION")
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      +kind = C.Codec.pos(["view.open", "view.page", "view.search", "view.update", "view.action", "view.message", "view.cancel"], J.Json.get_str(o, "m", ""))
      Hub.thin.authorized(Bool.and(TV.Target.exists(st, J.Json.get_obj(J.Json.get_obj(o, "p"), "target")), Bool.and(
        Bool.or(Nat.is_eq(kind, 0n), Hub.thin.current(J.Json.get_obj(o, "p"), Client.thin.context(c))),
        Bool.or(Bool.not(Bool.or(Nat.is_eq(kind, 0n), Bool.or(Nat.is_eq(kind, 1n), Bool.or(Nat.is_eq(kind, 3n), Nat.is_eq(kind, 4n))))), TVW.Wire.ordinary.rid(TVW.Wire.nat(J.Json.get_obj(J.Json.get_obj(o, "p"), "session"), "rid", PRESENT.Session.nat.max()))))), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now, kind)

def Hub.thin.admit(fresh: Bool, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat) -> IO(Hub):
  match fresh h:
    case False{} _ : Hub.thin.error(h, client, o, "STALE_CONNECTION")
    case True{} Hub{st, f, ls, cl, as, inbox, cfg, note}:
      +c2 = Client.thin.with(c, ThinContext.with_control(Client.thin.context(c), PRESENT.Session.number(o, "id", 0n)))
      Hub.thin.request(True{}, Hub{st, f, ls, Clients.swap(cl, client, Some{c2}), as, inbox, cfg, note}, client, o, c2, now)

def Hub.thin.checked(valid: Bool, h: Hub, +client: U32, +o: J.Json, +c: Client, +now: Nat) -> IO(Hub):
  match valid:
    case False{}: Hub.thin.error(h, client, o, "STALE_CONNECTION")
    case True{}: Hub.thin.admit(TA.Control.fresh(PRESENT.Session.number(o, "id", 0n), ThinContext.control(Client.thin.context(c))), h, client, o, c, now)

def Hub.thin.have(+c: Maybe<&2, Client>, h: Hub, +client: U32, +o: J.Json, +now: Nat) -> IO(Hub):
  match c h:
    case None{} _: Hub.thin.error(h, client, o, "STALE_CONNECTION")
    case Some{+c} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.thin.checked(Bool.and(Bool.and(Client.thin(c), Nat.is_lt(Cfg.thin.revision(cfg), PRESENT.Session.nat.max())), Bool.and(TVW.Wire.control.id(TVW.Wire.nat(o, "id", PRESENT.Session.nat.max())), TVW.Wire.request(J.Json.get_obj(o, "p"), U32.show(OB.Cr.gen(Boot.r(c))), Cfg.thin.epoch(cfg)))), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, c, now)

def Hub.thin.rpc.now(h: Hub, +client: U32, +o: J.Json, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}: Hub.thin.have(Clients.get(cl, client), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, now)

# Until the index is built (THIN.Lazy, Thin.warm) a thin request waits in
# the cache rather than build it on the hub's time; Hub.thin.warmed answers
# the waiting ones in the order they came
def Hub.thin.rpc.ready(ready: Bool, h: Hub, +client: U32, +o: J.Json, +now: Nat) -> IO(Hub):
  match ready h:
    case True{} _: Hub.thin.rpc.now(h, client, o, now)
    case False{} Hub{st, f, ls, cl, as, inbox, +cfg, note}: IO.pure(Hub, Hub{st, f, ls, cl, as, inbox, Cfg.thin.wait(cfg, ThinWait{client, o, now}), note})

def Hub.thin.rpc(h: Hub, +client: U32, +o: J.Json, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}: Hub.thin.rpc.ready(Bool.and(THIN.Lazy.ready(Cfg.thin.lazy(cfg)), TMAIL.Lazy.ready(Cfg.thin.mail.lazy(cfg))), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, o, now)

def Hub.thin.answer(ws: List<&2, ThinWait>, h: Hub) -> IO(Hub):
  match ws:
    case Nil{}: IO.pure(Hub, h)
    case ThinWait{client, o, now} <> tail:
      do IO<Hub>:
        h2 : Hub <- Hub.thin.rpc.now(h, client, o, now)
        Hub.thin.answer(tail, h2)

def Mem.settled.k(none: Bool, +line: String) -> IO(Unit):
  match none:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      Log.info(line)

def Mem.settled(+line: String) -> IO(Unit):
  Mem.settled.k(String.is_empty(line), line)

def Hub.thin.warmed.k(h: Hub, i: THIN.Index, m: TMAIL.Cache) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.thin.answer(List.reverse(&2, ThinWait, Cfg.thin.waiting(cfg)), Hub{st, f, ls, cl, as, inbox, Cfg.thin.warm(cfg, i, m), note})

# the boot's thin index arrived: the JS heap is settled once (H.Mem.settle,
# until store mode keeps the index small), then the waiting reads answered
def Hub.thin.warmed(h: Hub, i: THIN.Index, m: TMAIL.Cache) -> IO(Hub):
  IO.bind(String, Hub, H.Mem.settle(), line => IO.bind(Unit, Hub, Mem.settled(line), _ => Hub.thin.warmed.k(h, i, m)))

def Hub.thin.read_guard(valid: Bool, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match valid:
    case False{}: IO.pure(Hub, h)
    case True{}: Hub.rpc.mobile.kind(C.Codec.pos(["mobile.screen", "mobile.history", "frontend.screen", "frontend.history"], ""),
      Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), h, client, text, now)

def Hub.thin.read_client(c: Maybe<&2, Client>, h: Hub, +client: U32, +gen: U32, +view: String, +target: J.Json, +text: String, +now: Nat) -> IO(Hub):
  match c:
    case None{}: IO.pure(Hub, h)
    case Some{+c}: Hub.thin.read_guard(Bool.and(Client.thin(c), TA.Job.valid(Client.id(c), client, OB.Cr.gen(Boot.r(c)), gen, ThinContext.view(Client.thin.context(c)), view, ThinContext.target(Client.thin.context(c)), target, J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{text}}]))), h, client, text, now)

def Hub.thin.read_job(h: Hub, +client: U32, +gen: U32, +view: String, +target: J.Json, +text: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}: Hub.thin.read_client(Clients.get(cl, client), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, gen, view, target, text, now)

def Hub.rpc.view(+thin: Bool, +o: J.Json, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match thin:
    case True{}: Hub.thin.rpc(h, client, o, now)
    case False{}: Hub.rpc.mobile.kind(C.Codec.pos(["mobile.screen", "mobile.history", "frontend.screen", "frontend.history"], J.Json.get_str(o, "m", "")), o, h, client, text, now)

def Hub.rpc.parse(h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  +o = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
  Hub.rpc.view(String.starts_with(J.Json.get_str(o, "m", ""), "view."), o, h, client, text, now)

# Thin requests are bounded before JSON parsing. Oversized packets close only
# that captured client; no malformed request can trigger compatibility replay.
def Hub.thin.close_client(h: Hub, +client: U32) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      do IO<Hub>:
        cl2 : List<&2, Client> <- Wire.to.raw(cl, client, WClose{}, 0n)
        return Hub{st, f, ls, Clients.drop(cl2, client), as, inbox, cfg, note}

def Hub.rpc.thin_short(short: Bool, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match short:
    case True{}: Hub.rpc.parse(h, client, text, now)
    case False{}: Hub.thin.close_client(h, client)

def Hub.rpc.thin_cap(thin: Bool, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match thin:
    case True{}: Hub.rpc.thin_short(TVW.Wire.within(text, 262144n), h, client, text, now)
    case False{}: Hub.rpc.parse(h, client, text, now)

# A request from no connected client is dropped (a client that left),
# except client 0: the hub's own jobs (GitJob.say: a thread's worktree, its
# notes; a dead push token) and the in-process window send theirs as 0, and
# a headless hub has no client 0. Dropping those lost every new thread's
# worktree, so its first turn never started (far_e2e on a headless hub).
def Hub.rpc.own(own: Bool, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match own:
    case True{}: Hub.rpc.thin_cap(False{}, h, client, text, now)
    case False{}: IO.pure(Hub, h)

def Hub.rpc.client(c: Maybe<&2, Client>, h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match c:
    case Some{c}: Hub.rpc.thin_cap(Client.thin(c), h, client, text, now)
    case None{}: Hub.rpc.own(U32.is_eq(client, 0), h, client, text, now)

def Hub.rpc(h: Hub, +client: U32, +text: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}:
      Hub.rpc.client(Clients.get(cl, client), Hub{st, f, ls, cl, as, inbox, cfg, note}, client, text, now)

# the threads' Boat chips that Boat's status changes (logged only then)
def Hub.chips(p: M.State & Hub, +status: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Step{UZ.Chip.changes(st, status, now), Nil{}})

def Hub.line.k(p: M.State & Hub, +thread: String, +line: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Hub.agent_line(st, now, thread, line))

# a progress line: the ledger sees it, and it is written only once in
# B.Prog.gap() seconds with the held ones (B.Prog.line); a line held lands
# on the beat (Hub.beat)
def Hub.line.prog.k(p: B.PStep, +r: ARun, h: Hub, +thread: String, +o: J.Json, +now: Nat) -> IO(Hub):
  match p h:
    case B.PStep{+at, +pend, +flush} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, Agents.with(as, thread, ARun.prog.set(ARun.heard.set(ARun.put(r, CM.Cmds.see(ARun.cmds(r), o), ARun.held(r)), now), at, pend)), inbox, cfg, note},
        B.Prog.flush(st, thread, now, flush))

def Hub.line.prog(+r: ARun, h: Hub, +thread: String, +line: String, +o: J.Json, +now: Nat) -> IO(Hub):
  Hub.line.prog.k(B.Prog.line(ARun.prog_at(r), ARun.prog_last(r), now, line, o), r, h, thread, o, now)

# a Claude line judged by its process's ledger, which sees it before the
# step runs (the step may send the next command)
def Hub.line.run.k(prog: Bool, +r: ARun, h: Hub, +thread: String, +line: String, +o: J.Json, +now: Nat) -> IO(Hub):
  match prog h:
    case True{} _:
      Hub.line.prog(r, h, thread, line, o, now)
    case False{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, Agents.with(as, thread, ARun.heard.set(ARun.put(r, CM.Cmds.see(ARun.cmds(r), o), ARun.held(r)), now)), inbox, cfg, note},
        B.Hub.agent_line.vo(CM.Cmds.verdict(ARun.cmds(r), B.Hub.running(B.Hub.thread(st, thread)), o), st, now, thread, line, o, CM.Cmds.text(ARun.cmds(r), o)))

def Hub.line.run(+r: ARun, h: Hub, +thread: String, +line: String, +o: J.Json, +now: Nat) -> IO(Hub):
  Hub.line.run.k(B.Prog.is(o), r, h, thread, line, o, now)

def Hub.line.m(m: Maybe<&2, ARun>, h: Hub, +thread: String, +line: String, +now: Nat) -> IO(Hub):
  match m:
    case Some{+r}:
      Hub.line.run(r, h, thread, line, Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), now)
    case None{}:
      Hub.line.k(Hub.state(h), thread, line, now)

def Hub.line(h: Hub, +thread: String, +line: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.line.m(Agents.run(as, thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, line, now)

def Hub.gone.k(current: Bool, h: Hub, +thread: String, +code: U32, +why: String, +now: Nat) -> IO(Hub):
  match current h:
    case True{} Hub{+st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, Agents.drop(as, thread), inbox, cfg, note}, B.Hub.agent_gone.why(st, now, thread, code, why))
    case False{} _:
      IO.pure(Hub, h)

# a process exited; one already replaced (a restart for a new model or
# mode) is old news
def Hub.gone(h: Hub, +thread: String, +code: U32, +pid: U32, +why: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.gone.k(Agent.pid_is(Agents.get(as, thread), pid), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, code, why, now)

# each minute the hub's size goes into the gauges
def Hub.tick(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, +ls, +cl0, +as, +inbox, cfg, note}:
      do IO<Hub>:
        +cl : List<&2, Client> <- Desk.say(DK.Desk.text(Clients.looks(cl0), Nat.sub(now, 60n)), cl0, now)
        PS.Perf.set("hub.clients", U32.from_nat(List.length(&2, Client, cl)))
        PS.Perf.set("hub.agents", U32.from_nat(List.length(&2, Agent, as)))
        PS.Perf.set("hub.changes", U32.from_nat(SO.Lg.n(ls)))
        PS.Perf.set("hub.busy", U32.from_nat(M.Work.threads(st)))
        # children the last exec handed over that ended meanwhile are waited
        # for (effects/adopt.js), so none stays a zombie
        left : U32 <- AIO.Adopt.reap()
        PS.Perf.set("hub.inherited", left)
        Self.post(inbox, KProbe{False{}})
        Self.post(inbox, Release{})
        Self.post(inbox, Silence{})
        h2 : Hub <- Hub.telemetry(Hub{st, f, ls, cl, as, inbox, cfg, note}, now)
        Hub.step(h2, B.Hub.tick(st, now))

# a tool call's answer goes once its step is stored: a call whose changes
# could not be written is told so, never that it worked
def Hub.mcp.fail(+body: String) -> String:
  MC.Mcp.error(Maybe.default(&2, J.Json, J.Json.get(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "id"), J.Null{}), "-32000", "Backplane could not record this (the log write failed); nothing was done")

def Hub.mcp.after(r: Hub & Bool, +park: String, +body: String, +text: String, +reply: Chan(String)) -> IO(Hub):
  (h, ok) = r
  match h ok:
    case Hub{hs, f, ls, cl, as, inbox, +cfg, note} True{}:
      do IO<Hub>:
        Hub.mcp.send(Bool.not(String.is_empty(park)), park, body, text, reply, cfg)
        return Hub{hs, f, ls, cl, as, inbox, cfg, note}
    case Hub{hs, f, ls, cl, as, inbox, +cfg, note} False{}:
      do IO<Hub>:
        Hub.mcp.send(False{}, "", body, Hub.mcp.fail(body), reply, cfg)
        return Hub{hs, f, ls, cl, as, inbox, cfg, note}

def MailJob.mcp.json(+r: MST.StoreResult<J.Json>, +id: J.Json) -> String:
  match r:
    case MST.Stored{+j}: MC.Mcp.result(id, MC.Mcp.content(True{}, j))
    case MST.Missing{+p}: MC.Mcp.result(id, MC.Mcp.content(False{}, MC.Mcp.fail("missing " ++ p)))
    case MST.StoreError{+p, +e}: MC.Mcp.result(id, MC.Mcp.content(False{}, MC.Mcp.fail(p ++ ": " ++ e)))

def Hub.mail.mcp.read(h: Hub, +cs: List<&2, M.Change>, +conv: String, +before: String, +n: Nat, +reply: Chan(String), +body: String) -> IO(Hub):
  match h:
    case Hub{+st, f, +ls, +cl, +as, +inbox, +cfg, +note}:
      do IO<Hub>:
        r : MST.StoreResult<J.Json> <- MailJob.page(Cfg.thin.store(cfg), Cfg.home(cfg), st, conv, "*", before, n)
        IO.bind(Hub & Bool, Hub, Hub.step.ok(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{cs, Nil{}}), got => Hub.mcp.after(got, "", body, MailJob.mcp.json(r, J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "id")), reply))

def Hub.mcp.k(o: MC.McpOut, h: Hub, +reply: Chan(String), +body: String) -> IO(Hub):
  match o:
    case MC.McpOut{_, B.Step{cs, B.MailRead{_, _, conv, before, n} <> Nil{}}, _}:
      Hub.mail.mcp.read(h, cs, conv, before, n, reply, body)
    case MC.McpOut{+text, st, +park}:
      IO.bind(Hub & Bool, Hub, Hub.step.ok(h, st), r => Hub.mcp.after(r, park, body, text, reply))

def Hub.mcp.hub.normal(h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.mcp.k(BH.Ride.mcp(BH.Bots.mcp(st, now, thread, Maybe.default(&2, J.Json, J.Json.parse(Cfg.models(cfg)), J.Null{}), Cfg.version(cfg),
          Maybe.default(&2, J.Json, J.Json.parse(BN.Bn.dir(Cfg.bn(cfg))), J.Json.arr(Nil{})), GG.Google.tool_defs(), body), st, now, thread),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, reply, body)

def LockTool.preflight(free: Bool, +name: String, +holder: String, +home: String) -> IO(Bool):
  match free:
    case True{}:
      LG.Guard.take(name, holder, home, LG.Guard.script())
    case False{}:
      IO.pure(Bool, False{})

def LockTool.args(a: J.Json, +ready: Bool) -> J.Json:
  match a:
    case J.Obj{fields}:
      J.Obj{J.Field{"_lockReady", J.Flag{ready}, fields}}
    case _:
      J.Json.obj([J.KV{"_lockReady", J.Flag{ready}}])

def LockTool.body(+o: J.Json, +a: J.Json, ready: Bool) -> String:
  J.Json.show(J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"id", Maybe.default(&2, J.Json, J.Json.get(o, "id"), J.Null{})}, J.KV{"method", J.Str{"tools/call"}}, J.KV{"params", J.Json.obj([J.KV{"name", J.Str{J.Json.get_str(J.Json.get_obj(o, "params"), "name", "")}}, J.KV{"arguments", LockTool.args(a, ready)}])}]))

# A failed log write or rejected command must relinquish a preflight guard.
def LockTool.cleanup.keep(keep: Bool, h: Hub, +name: String, +holder: String) -> IO(Hub):
  match keep:
    case True{}:
      IO.pure(Hub, h)
    case False{}:
      do IO<Hub>:
        LG.Guard.drop(name, holder)
        return h

def LockTool.cleanup(h: Hub, +name: String, +holder: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      LockTool.cleanup.keep(String.eq(LCK.Lock.owner(M.Locks.get(st, name)), holder), Hub{st, f, ls, cl, as, inbox, cfg, note}, name, holder)

def Hub.mcp.lock.k(lock: Bool, h: Hub, +thread: String, +o: J.Json, +a: J.Json, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match lock h:
    case False{} _:
      Hub.mcp.hub.normal(h, thread, body, reply, now)
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +iswork : Bool = String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "work_run")
        +name : String = J.Json.get_str(a, Bool.pick(String, iswork, "lock", "name"), "")
        +holder : String = Bool.pick(String, iswork, B.Watch.id(st, thread), thread)
        +ready : Bool <- LockTool.preflight(Bool.and(Bool.and(LCK.Lock.name.valid(name), Bool.or(Bool.not(iswork), Bool.not(String.is_empty(String.trim(J.Json.get_str(a, "command", "")))))), Bool.or(String.is_empty(LCK.Lock.owner(M.Locks.get(st, name))), String.eq(LCK.Lock.owner(M.Locks.get(st, name)), holder))), name, holder, Cfg.home(cfg))
        h2 : Hub <- Hub.mcp.hub.normal(Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, LockTool.body(o, a, ready), reply, now)
        LockTool.cleanup(h2, name, holder)

def Hub.mcp.hub.raw(h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  +o = Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{})
  +a = J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments")
  +name = J.Json.get_str(J.Json.get_obj(o, "params"), "name", "")
  Hub.mcp.lock.k(Bool.or(String.eq(name, "lock_acquire"), Bool.and(String.eq(name, "work_run"), Bool.not(String.is_empty(J.Json.get_str(a, "lock", ""))))), h, thread, o, a, body, reply, now)

# Resolve #N against the subscriber's project remote before persisting WakeSet.
def WakeJob.repo.path(+url: String) -> String:
  +path = Bool.pick(String, String.starts_with(url, "git@github.com:"), String.drop(url, 15n),
    Bool.pick(String, String.starts_with(url, "https://github.com/"), String.drop(url, 19n),
      Bool.pick(String, String.starts_with(url, "ssh://git@github.com/"), String.drop(url, 21n), "")))
  Bool.pick(String, String.ends_with(path, ".git"), String.take(path, Nat.sub(String.length(path), 4n)), path)

def WakeJob.resolve.body(+o: J.Json, +a: J.Json, +repo: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"id", J.Json.get_obj(o, "id")}, J.KV{"method", J.Str{"tools/call"}},
    J.KV{"params", J.Json.obj([J.KV{"name", J.Str{"wake_on"}}, J.KV{"arguments", J.Json.obj([J.KV{"kind", J.Str{"pr"}},
      J.KV{"target", J.Str{repo ++ J.Json.get_str(a, "target", "")}}, J.KV{"events", J.Json.arr(M.Wake.strs(MC.Mcp.wake.events(a, "pr")))},
      J.KV{"once", J.Flag{J.Json.get_bool(a, "once", True{})}}, J.KV{"note", J.Str{J.Json.get_str(a, "note", "")}}])}])}]))

def Hub.mcp.wake.resolve.raw(h: Hub, +thread: String, +o: J.Json, +a: J.Json, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        url : String <- GH.GhRun.url(B.Hub.project.root(B.Hub.project(st, MC.Mcp.project_of(B.Hub.thread(st, thread)))))
        Hub.mcp.hub.raw(Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, WakeJob.resolve.body(o, a, WakeJob.repo.path(url)), reply, now)

def Hub.mcp.wake.project(has: Bool, h: Hub, +thread: String, +o: J.Json, +a: J.Json, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match has:
    case True{}:
      Hub.mcp.wake.resolve.raw(h, thread, o, a, body, reply, now)
    case False{}:
      Hub.mcp.hub.raw(h, thread, body, reply, now)

def Hub.mcp.wake.resolve(h: Hub, +thread: String, +o: J.Json, +a: J.Json, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.mcp.wake.project(Bool.not(String.is_empty(MC.Mcp.project_of(B.Hub.thread(st, thread)))), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, o, a, body, reply, now)

def Hub.mcp.wake.k(resolve: Bool, h: Hub, +thread: String, +o: J.Json, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match resolve:
    case False{}:
      Hub.mcp.hub.raw(h, thread, body, reply, now)
    case True{}:
      Hub.mcp.wake.resolve(h, thread, o, J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), body, reply, now)

def Hub.mcp.hub.pr(h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  +o = Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{})
  Hub.mcp.wake.k(Bool.and(String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "wake_on"),
    Bool.and(String.eq(J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), "kind", ""), "pr"),
      String.starts_with(J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), "target", ""), "#"))), h, thread, o, body, reply, now)

# Snapshot a file before acknowledging wake_on so changes after its reply
# cannot be swallowed by a late first poll. The core still handles exists.
def WakeJob.file.body(+o: J.Json, +a: J.Json, +stamp: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"id", J.Json.get_obj(o, "id")}, J.KV{"method", J.Str{"tools/call"}},
    J.KV{"params", J.Json.obj([J.KV{"name", J.Str{"wake_on"}}, J.KV{"arguments", J.Json.obj([J.KV{"kind", J.Str{"file"}},
      J.KV{"target", J.Str{J.Json.get_str(a, "target", "")}}, J.KV{"events", J.Json.arr(M.Wake.strs(MC.Mcp.wake.events(a, "file")))},
      J.KV{"once", J.Flag{J.Json.get_bool(a, "once", True{})}}, J.KV{"note", J.Str{J.Json.get_str(a, "note", "")}}, J.KV{"_baseline", J.Str{stamp}}])}])}]))

def Hub.mcp.wake.file(h: Hub, +thread: String, +o: J.Json, +a: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  do IO<Hub>:
    stamp : String <- WJ.WakeJob.file(J.Json.get_str(a, "target", ""))
    Hub.mcp.hub.raw(h, thread, WakeJob.file.body(o, a, stamp), reply, now)

def Hub.mcp.file.k(file: Bool, h: Hub, +thread: String, +o: J.Json, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match file:
    case True{}:
      Hub.mcp.wake.file(h, thread, o, J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), reply, now)
    case False{}:
      Hub.mcp.hub.pr(h, thread, body, reply, now)

def Hub.mcp.hub.w(h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  +o = Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{})
  Hub.mcp.file.k(Bool.and(String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "wake_on"), String.eq(J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), "kind", ""), "file")), h, thread, o, body, reply, now)

# deploy_main with deploy.repo set: origin/main is read off the hub first
# (ShipJob.ask), then the call is answered (Hub.ship.asked)
def Hub.mcp.ship(go: Bool, h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match go h:
    case True{} Hub{+st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, ShipJob.ask(B.Ship.repo(st), B.Ship.cands(st), thread, body, reply, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      Hub.mcp.hub.w(h, thread, body, reply, now)

def Hub.mcp.ship.is(+body: String) -> Bool:
  String.eq(J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "params"), "name", ""), "deploy_main")

def Hub.mcp.hub.go(h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.mcp.ship(Bool.and(Hub.mcp.ship.is(body), Bool.not(String.is_empty(B.Ship.repo(st)))), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, body, reply, now)

# deploy_main and deploy_restart on a hub on a copied log: refused
def Hub.mcp.deploy.is(+body: String) -> Bool:
  CN.Copy.has(["deploy_main", "deploy_restart"], J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "params"), "name", ""))

def Hub.mcp.hub.c(refuse: Bool, h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match refuse:
    case True{}:
      Hub.mcp.k(MC.Mcp.reply(Maybe.default(&2, J.Json, J.Json.get(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "id"), J.Null{}), MC.Res.fail("a hub on a copied log (BACKPLANE_COPY=1) does not deploy or restart")), h, reply, "")
    case False{}:
      Hub.mcp.hub.go(h, thread, body, reply, now)

def Hub.mcp.hub(h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  IO.bind(Bool, Hub, P.Proc.copy(), copy => Hub.mcp.hub.c(Bool.and(copy, Hub.mcp.deploy.is(body)), h, thread, body, reply, now))

# the approved folder, resolved: added (or found) and the tool answered
def Hub.newproj.made(h: Hub, +root: String, +name: String, +id: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.mcp.k(MC.Mcp.reply(Maybe.default(&2, J.Json, J.Json.parse(id), J.Null{}), BH.Bots.newproj.made(st, now, root, name)),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, reply, "")

def Hub.mcp.w(+secs: Nat, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match secs h:
    case 0n _:
      Hub.mcp.hub(h, thread, body, reply, now)
    case 1n+k Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Wait.loop(1n+k, Nat.add(now, 1n+k), thread, MC.Wait.now(o), reply, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# a line from an agent: a turn process's only while it is the one the hub
# runs for its thread (B.Tag.heard)
def Hub.tagged.k(heard: Bool, h: Hub, +thread: String, line: String, +now: Nat) -> IO(Hub):
  match heard:
    case True{}:
      Hub.line(h, thread, line, now)
    case False{}:
      IO.pure(Hub, h)

def Hub.tagged(h: Hub, +tag: String, line: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.tagged.k(B.Tag.heard(B.Tag.pid(tag), Agent.pid_of(Agents.get(as, B.Tag.thread(tag)))), Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Tag.thread(tag), line, now)

# then turns the restart cut short carry on (B.Hub.carry)
def Hub.recover.carry(h: Hub, +before: M.State, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Hub.carry.a(before, st, now, Agents.threads(as)))

def Hub.recover.go(h: Hub, st: B.Step, before: M.State, +now: Nat) -> IO(Hub):
  IO.bind(Hub, Hub, Hub.step(h, st), h2 => Hub.recover.carry(h2, before, now))

# (and the Scratch project is made the first time, M.Scratch.ensure)
def Hub.recover.k(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, +as, inbox, +cfg, note}:
      Hub.recover.go(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step.before(M.Scratch.ensure(st, Cfg.home(cfg) ++ "/scratch", now), Nil{}, B.Hub.recover.a(st, now, Agents.threads(as))), st, now)

# (and threads elsewhere start: Far.start)
def Hub.recover.stale(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, cfg, note}:
      Hub.recover.k(Hub{st, f, ls, cl, as, inbox, cfg, note}, now)

# Agents kept through a restart in place (core/adopt.bend)
# --------------------------------------------------------
# Before the exec the hub writes the handoff (each Claude agent's
# generation, key and ledger; effects/adopt.js keeps the descriptors on
# their sockets open across the exec). At start, before recovery, each
# recorded process is adopted, stopped or left (AD.Adopt.decide); an
# adopted one is read and written as a freshly started one is, and
# recovery leaves its thread alone. BACKPLANE_ADOPT=0 adopts nothing (each
# recorded process still running is stopped).

def Adopt.home.hand(+home: String) -> String:
  home ++ "/agents.handoff"

def Adopt.home.record(+home: String) -> String:
  home ++ "/agents.pids"

def Adopt.hand.of(a: Agent) -> String:
  match a:
    case Agent{t, _, p, k, ARun{c, _, _, _, g, _, _, _} <> _}:
      AD.Adopt.hand.line(t, U32.to_nat(p), g, k, c) ++ "\n"
    case _:
      ""

def Adopt.hand.all(as: List<&2, Agent>) -> String:
  match as:
    case Nil{}:
      ""
    case +a <> t:
      Adopt.hand.of(a) ++ Adopt.hand.all(t)

def Hub.reexec.k(r: Result<&1, &1, U32 & String, Unit>, h: Hub, +home: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        rm : Result<&1, &1, U32 & String, Unit> <- H.FS.remove(Adopt.home.hand(home))
        Upgrade.exec(r, inbox)
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# hand the agents off, then exec (an exec that fails takes the handoff back)
def Hub.reexec(h: Hub, +path: String, args: List<&2, String>) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, +cfg, note}:
      do IO<Hub>:
        +home : String = Cfg.home(cfg)
        t : String <- Token.save(Adopt.home.hand(home), Adopt.hand.all(as))
        Log.info("restarting in place; " ++ Nat.show(List.length(&2, String, Agents.threads(as))) ++ " agents handed off")
        r : Result<&1, &1, U32 & String, Unit> <- H.Sys.exec(path, args)
        Hub.reexec.k(r, Hub{st, f, ls, cl, as, inbox, cfg, note}, home)

def Adopt.idents(es: List<&2, AD.AEntry>) -> IO(List<&2, String>):
  match es:
    case Nil{}:
      IO.pure(List<&2, String>, Nil{})
    case e <> t:
      do IO<List<&2, String>>:
        +i : String <- AIO.Adopt.ident(U32.from_nat(AD.AEntry.pid(e)))
        rest : List<&2, String> <- Adopt.idents(t)
        return i <> rest

# a handed-off descriptor not kept is closed (one not handed off is a
# number from another process's table: never touched)
def Adopt.close(+hd: Bool, +fd: Nat) -> IO(Unit):
  match hd:
    case True{}:
      H.Fd.close(U32.from_nat(fd))
    case False{}:
      IO.pure(Unit, Unit{})

# a stopped process the hub still parents (an exec keeps its children) is
# reaped once it ends; one a crash left to init answers at once
def Adopt.reap(ok: Bool, +pid: U32) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(U32, Unit, H.Proc.wait(pid), c => IO.pure(Unit, Unit{}))
    case False{}:
      IO.pure(Unit, Unit{})

def Adopt.stop.e(e: AD.AEntry) -> IO(Unit):
  match e:
    case AD.AEntry{+t, +p, +i, +fd, +hd, _, _, _}:
      do IO<Unit>:
        +ok : Bool <- AIO.Adopt.stop(U32.from_nat(p), i)
        Log.info("agent of " ++ t ++ " (pid " ++ Nat.show(p) ++ ") not kept through the restart: " ++ Bool.pick(String, ok, "stopped", "already gone"))
        Adopt.close(hd, fd)
        IO.spawn(Unit, Adopt.reap(ok, U32.from_nat(p)))

# an adopted process, read and written as Hub.spawned.dup does it
def Hub.adopted.dup(r: Socket & Result<&1, &1, U32 & String, Socket>, e: AD.AEntry, h: Hub, +now: Nat) -> IO(Hub):
  (rd0, res) = r
  match res e h:
    case Done{wr} AD.AEntry{+t, +p, _, _, _, +g, +k, c} Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        +pid : U32 = U32.from_nat(p)
        +input : Chan(String) <- Chan.new(String, 256)
        IO.spawn(Unit, Agent.writer(Fuel.forever(), wr, input))
        rd : Socket <- IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Socket, H.Sock.dup(rd0), d => Agent.watch.start(d, pid, t, g, inbox))
        IO.spawn(Unit, Agent.reader(Fuel.forever(), rd, U.Lines.new(), pid, t, g, B.Tag.of(t, pid), inbox))
        Log.info("kept the agent of " ++ t ++ " (pid " ++ Nat.show(p) ++ ") through the restart")
        return Hub{st, f, ls, cl, Agent{t, input, pid, k, [ARun{c, Nil{}, Nil{}, now, g, False{}, 0n, Nil{}}]} <> Agents.drop(as, t), inbox, cfg, note}
    case Fail{_} _ _:
      do IO<Hub>:
        Socket.close(rd0)
        Adopt.stop.e(e)
        return h

def Hub.adopted(r: Result<&1, &1, U32 & String, Socket>, +e: AD.AEntry, h: Hub, +now: Nat) -> IO(Hub):
  match r:
    case Done{s0}:
      do IO<Hub>:
        s1 : Socket <- AIO.Adopt.keep(AD.AEntry.thread(e), U32.from_nat(AD.AEntry.pid(e)), s0)
        d : Socket & Result<&1, &1, U32 & String, Socket> <- H.Sock.dup(s1)
        Hub.adopted.dup(d, e, h, now)
    case Fail{_}:
      do IO<Hub>:
        Adopt.stop.e(e)
        return h

def Hub.adopt.one(v: AD.AVerdict, h: Hub, +now: Nat) -> IO(Hub):
  match v:
    case AD.VAdopt{AD.AEntry{t, p, i, +fd, hd, g, k, c}}:
      IO.bind(Result<&1, &1, U32 & String, Socket>, Hub, AIO.Adopt.take(U32.from_nat(fd)), r => Hub.adopted(r, AD.AEntry{t, p, i, fd, hd, g, k, c}, h, now))
    case AD.VStop{e}:
      IO.bind(Unit, Hub, Adopt.stop.e(e), u => IO.pure(Hub, h))
    case AD.VLeave{AD.AEntry{_, _, _, +fd, +hd, _, _, _}}:
      IO.bind(Unit, Hub, Adopt.close(hd, fd), u => IO.pure(Hub, h))

def Hub.adopt.all(vs: List<&2, AD.AVerdict>, h: Hub, +now: Nat) -> IO(Hub):
  match vs:
    case Nil{}:
      IO.pure(Hub, h)
    case v <> t:
      IO.bind(Hub, Hub, Hub.adopt.one(v, h, now), h2 => Hub.adopt.all(t, h2, now))

def Hub.adopt.es(+es: List<&2, AD.AEntry>, h: Hub, +allow: Bool, +now: Nat) -> IO(Hub):
  do IO<Hub>:
    +nows : List<&2, String> <- Adopt.idents(es)
    Hub.adopt.all(AD.Adopt.decide(es, nows, allow), h, now)

# the agents a hub takes over or stops at boot: none for a hub on a copied
# log, whose records name the real hub's agents (law adopt_copy_none)
def Adopt.copy.entries(copy: Bool, +rec: String, +hand: String) -> List<&2, AD.AEntry>:
  match copy:
    case True{}:
      Nil{}
    case False{}:
      AD.Adopt.entries(rec, hand)

def Hub.adopt(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +home : String = Cfg.home(cfg)
        +rec : String <- Files.text(Adopt.home.record(home))
        +hand : String <- Files.text(Adopt.home.hand(home))
        rm : Result<&1, &1, U32 & String, Unit> <- H.FS.remove(Adopt.home.hand(home))
        AIO.Adopt.record(Adopt.home.record(home))
        +off : String <- Env.get("BACKPLANE_ADOPT", "")
        copy : Bool <- P.Proc.copy()
        Hub.adopt.es(Adopt.copy.entries(copy, rec, hand), Hub{st, f, ls, cl, as, inbox, cfg, note}, Bool.not(String.eq(off, "0")), now)

def Hub.mail.recover.on(+on: Bool, h: Hub) -> IO(Hub):
  match on h:
    case False{} kept: IO.pure(Hub, kept)
    case True{} Hub{+st, f, +ls, +cl, +as, +inbox, +cfg, +note}:
      do IO<Hub>:
        IO.spawn(Unit, MailJob.recover(Cfg.home(cfg), M.State.mail(st), inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.mail.recover.start(h: Hub) -> IO(Hub):
  match h:
    case Hub{+st, f, +ls, +cl, +as, +inbox, +cfg, +note}:
      Hub.mail.recover.on(String.eq(M.Setting.get(st, "mail.on", "off"), "on"), Hub{st, f, ls, cl, as, inbox, cfg, note})

def Hub.recover(h: Hub, +now: Nat) -> IO(Hub):
  IO.bind(Hub, Hub, Hub.mail.recover.start(h), h0 => IO.bind(Hub, Hub, Far.start(h0), h2 => Hub.recover.stale(h2, now)))

# The minute
# ----------

# Each worker publishes as soon as its peer answers. The hub owns slots;
# no worker reads or replaces a snapshot of another worker's result.
def Bot.dir.job(+peer: String, +url: String, +pname: String, +home: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +now : Nat <- Now.nat()
    r : BN.Resp <- BN.Bn.dir.fetch(home, url, peer, now)
    seen : Nat <- Now.nat()
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, BotsFound{peer, url, BN.Bn.fetched(r, peer, pname, seen)})
    return Unit{}

def Bot.dir.launch(ids: List<&2, String>, +st: M.State, +cfg: Cfg, +inbox: Chan(Msg)) -> IO(Unit):
  match ids:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +i <> t:
      do IO<Unit>:
        IO.spawn(Unit, Bot.dir.job(i, Bot.peer.url(st, i), Bot.peer.name(st, i), Cfg.home(cfg), inbox))
        Bot.dir.launch(t, st, cfg, inbox)

def Hub.dir.plan(h: Hub, +plan: BH.DirPlan) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        Bot.dir.launch(BH.DirPlan.launch(plan), st, cfg, inbox)
        return Hub{st, f, ls, cl, as, inbox, Cfg.with_bn(cfg, BN.Bn.with_work(Cfg.bn(cfg), BH.DirPlan.work(plan))), note}

def Hub.dir.queue(h: Hub, ps: List<&2, BO.Peer>) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.dir.plan(Hub{st, f, ls, cl, as, inbox, cfg, note}, BH.DirWork.plan(BH.DirWork.queue(ps, BN.Bn.work(Cfg.bn(cfg)))))

def Hub.bots_found.k(same: Bool, h: Hub, +d: String) -> IO(Hub):
  match same h:
    case True{} _:
      IO.pure(Hub, h)
    case False{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        BN.Bn.cache.save(Cfg.home(cfg), d)
        Hub.models.k(Hub{st, f, ls, cl, as, inbox, Cfg.with_bn(cfg, BN.Bn.with_dir(Cfg.bn(cfg), d)), note})

# Compare with the current directory, never the worker's starting copy.
def Hub.dir.publish(h: Hub, +d: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.bots_found.k(String.eq(BN.Bn.dir(Cfg.bn(cfg)), d), Hub{st, f, ls, cl, as, inbox, cfg, note}, d)

# Only the result of a still-linked peer at the same URL is accepted.
# Always release its slot, including for failures and revoked links.
def Hub.bots_found(h: Hub, +peer: String, +url: String, +result: Maybe<&2, List<&2, J.Json>>) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +prev : List<&2, J.Json> = J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(BN.Bn.dir(Cfg.bn(cfg))), J.Null{}))
        +rows : List<&2, J.Json> = BH.Dir.accept(Bool.and(Bot.peer.live(st, peer), String.eq(Bot.peer.url(st, peer), url)), result, prev, peer)
        h2 : Hub <- Hub.dir.publish(Hub{st, f, ls, cl, as, inbox,
          Cfg.with_bn(cfg, BN.Bn.with_work(Cfg.bn(cfg), BH.DirWork.finish(BN.Bn.work(Cfg.bn(cfg)), peer))), note},
          J.Json.show(J.Json.arr(BH.Dir.linked(rows, BO.World.peers(M.State.bots(st))))))
        h3 : Hub <- Far.here.one(Bool.and(Bool.and(Bot.peer.live(st, peer), String.eq(Bot.peer.url(st, peer), url)), Maybe.is_some(&2, List<&2, J.Json>, result)), h2, peer)
        Hub.dir.queue(h3, Nil{})

# Link changes prune the cache too, so revoked peers stay gone on restart.
def Hub.dir.refresh(h: Hub, +peer: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +ps : List<&2, BO.Peer> = BO.World.peers(M.State.bots(st))
        h2 : Hub <- Hub.dir.publish(Hub{st, f, ls, cl, as, inbox, cfg, note}, J.Json.show(J.Json.arr(BH.Dir.linked(
          J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(BN.Bn.dir(Cfg.bn(cfg))), J.Null{})), ps))))
        Hub.dir.queue(h2, BH.Dir.request(ps, peer))

# Pairing the owner's machines (UC.Mate.*): each minute, every machine
# of the owner's this hub should link to and is not linked to yet is
# asked for an invite (POST /bots/pair), and the invite it answers is
# joined like a pasted one. Only the minute asks, so two asks never
# overlap (the ask and the link take at most 15 s each).
def Pair.joined(m: Maybe<&2, SG.Inv>, +inbox: Chan(Msg), +home: String, +tailnet: Bool, +host: String, +port: U32) -> IO(Unit):
  match m:
    case Some{SG.Inv{+url, +peer, +secret}}:
      BotJob.join(inbox, home, tailnet, host, port, url, peer, secret, Far.client(), "0")
    case None{}:
      Log.info("pair: the answer held no invite")

def Pair.got(r: BN.Resp, +from: String, +inbox: Chan(Msg), +home: String, +tailnet: Bool, +host: String, +port: U32) -> IO(Unit):
  match r:
    case BN.Resp{+code, +body}:
      Bool.pick(IO(Unit), Nat.is_eq(code, 200n),
        Pair.joined(SG.Invite.parse(J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "invite", "")), inbox, home, tailnet, host, port),
        Log.info("pair: " ++ from ++ " did not pair (" ++ Nat.show(code) ++ ") " ++ String.take(String.trim(body), 120n)))

# (this hub's address as its machines list found it, the one the other
# side can reach and knows it by; else as it listens: a hub bound to
# 0.0.0.0 would name an address nobody reaches)
def Pair.here(+self: String, +tailnet: Bool, +host: String, +port: U32) -> IO(String):
  Bool.pick(IO(String), String.is_empty(self), BN.Bn.url.here(tailnet, host, port), IO.pure(String, self))

def Pair.ask(+url: String, +self: String, +inbox: Chan(Msg), +home: String, +tailnet: Bool, +host: String, +port: U32) -> IO(Unit):
  do IO<Unit>:
    +here : String <- Pair.here(self, tailnet, host, port)
    +me : String <- BN.Bn.name()
    r : BN.Resp <- BN.Bn.plain(BN.Bn.url.base(url) ++ "/bots/pair?name=" ++ me ++ "&url=" ++ here)
    Pair.got(r, url, inbox, home, tailnet, host, port)

def Pair.go(ms: List<&2, UC.MRow>, +inbox: Chan(Msg), +cfg: Cfg) -> IO(Unit):
  match ms:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case UC.MRow{_, +url, _} <> t:
      do IO<Unit>:
        IO.spawn(Unit, Pair.ask(url, J.Json.get_str(J.Json.get_obj(Far.machines(cfg), "self"), "url", ""), inbox, Cfg.home(cfg), Cfg.tailnet(cfg), Cfg.host(cfg), Cfg.port(cfg)))
        Pair.go(t, inbox, cfg)

def Hub.pair(h: Hub) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        Pair.go(UC.Mate.due(Cfg.tailnet(cfg), Far.machines(cfg), BO.World.peers(M.State.bots(st)), st), inbox, cfg)
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# The uberbot's sync (core/ubersync.bend, docs/uber.md)
# ------------------------------------------------------
# One request, uber.sync over /far/rpc, to each of the owner's machines: on
# a minute's tick (anti-entropy and the holder's heartbeat) and after any
# step that logged an op or promoted a bot. One request is out per link;
# one wanted meanwhile waits for it (again.<link>). What this hub remembers
# of the others (answers, the holder's beat) is in memory (FR.Fh.ub), never
# in the log.

def Uber.mem.map.k(p: FR.Fh & Hub) -> Map<&2, String> & Hub:
  (fh, h) = p
  (FR.Fh.ub.map(fh), h)

def Uber.mem.map(h: Hub) -> Map<&2, String> & Hub:
  Uber.mem.map.k(Far.fh(h))

def Uber.mem.put(p: FR.Fh & Hub, +k: String, +v: String) -> Hub:
  (fh, h) = p
  Far.with(h, FR.Fh.ub.set(fh, k, v))

def Uber.mem.set(h: Hub, +k: String, +v: String) -> Hub:
  Uber.mem.put(Far.fh(h), k, v)

def Uber.mem.notes(ns: List<&2, J.KV>, h: Hub) -> Hub:
  match ns:
    case Nil{}:
      h
    case J.KV{+k, +v} <> t:
      Uber.mem.notes(t, Uber.mem.set(h, k, J.Json.as_raw(Some{v}, "")))

def Uber.job(+st: M.State, +link: String, +url: String, +pvv: String, +home: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    now : Nat <- Now.nat()
    +name : String <- BN.Bn.name()
    +n : String <- BN.Bn.random(4n)
    +r : BN.Resp <- BN.Bn.to_peer(home, url, link, now, "/far/rpc", True{}, FR.Fr.nonced(UBS.Ub.sync.req(st, name, pvv), n))
    Far.send(inbox, UberAnswer{link, Far.code(r), Far.body(r)})

def Uber.kick.go(h: Hub, +link: String, +url: String, +pvv: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Uber.job(st, link, url, pvv, Cfg.home(cfg), inbox))
        return Uber.mem.set(Uber.mem.set(Hub{st, f, ls, cl, as, inbox, cfg, note}, "busy." ++ link, "1"), "again." ++ link, "")

def Uber.kick.b(busy: Bool, h: Hub, +link: String, +url: String, +pvv: String) -> IO(Hub):
  match busy:
    case True{}:
      IO.pure(Hub, Uber.mem.set(h, "again." ++ link, "1"))
    case False{}:
      Uber.kick.go(h, link, url, pvv)

def Uber.kick.m(p: Map<&2, String> & Hub, +link: String, +url: String) -> IO(Hub):
  (+um, h) = p
  Uber.kick.b(Bool.not(String.is_empty(UBS.Ub.net.get(um, "busy." ++ link))), h, link, url, UBS.Ub.net.get(um, "pvv." ++ link))

# a request to one machine now, or when the one out comes back
def Uber.kick.one(h: Hub, +link: String, +url: String) -> IO(Hub):
  Uber.kick.m(Uber.mem.map(h), link, url)

def Uber.kick.all(ps: List<&2, BO.Peer>, h: Hub) -> IO(Hub):
  match ps:
    case Nil{}:
      IO.pure(Hub, h)
    case BO.Peer{+i, _, +u, _, _} <> t:
      IO.bind(Hub, Hub, Uber.kick.one(h, i, u), h2 => Uber.kick.all(t, h2))

def Uber.kick.on(on: Bool, h: Hub) -> IO(Hub):
  match on h:
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Uber.kick.all(Far.own.go(BO.World.peers(M.State.bots(st)), st, cfg), Hub{st, f, ls, cl, as, inbox, cfg, note})
    case False{} _:
      IO.pure(Hub, h)

def Uber.kick.p(p: M.State & Hub) -> IO(Hub):
  (st, h) = p
  Uber.kick.on(UBS.Ub.on(st), h)

# sync with every machine of the owner's (when this one has an uberbot)
def Uber.kick(h: Hub) -> IO(Hub):
  Uber.kick.p(Hub.state(h))

def Uber.took.u(p: M.State & Hub, +link: String) -> IO(Hub):
  (st, h) = p
  Uber.kick.one(h, link, Bot.peer.url(st, link))

def Uber.took.m(h: Hub, more: Bool, +link: String) -> IO(Hub):
  match more:
    case True{}:
      Uber.took.u(Hub.state(h), link)
    case False{}:
      IO.pure(Hub, h)

def Uber.took.n(h: Hub, +t: UBS.SyncIn, +link: String, +now: Nat) -> IO(Hub):
  match t:
    case UBS.SyncIn{_, +machine, +same, +beat, +vv, +more}:
      Uber.took.m(Uber.mem.set(Uber.mem.notes(UBS.Ub.net.note(machine, same, beat, now), h), "pvv." ++ link, vv), more, link)

def Uber.took.k(some: Bool, h: Hub, +t: UBS.SyncIn, +link: String, +now: Nat) -> IO(Hub):
  match some:
    case True{}:
      IO.bind(Hub, Hub, Hub.step(h, B.Step{UBS.SyncIn.changes(t), Nil{}}), h2 => Uber.took.n(h2, t, link, now))
    case False{}:
      Uber.took.n(h, t, link, now)

# what a machine's object did here: its changes logged, what was learned of
# it remembered, and another round when either side still lacks ops
def Uber.took(h: Hub, +t: UBS.SyncIn, +link: String, +now: Nat) -> IO(Hub):
  Uber.took.k(Bool.not(List.is_empty(&2, M.Change, UBS.SyncIn.changes(t))), h, t, link, now)

# a request wanted while one was out goes now
def Uber.again.k(go: Bool, h: Hub, +link: String) -> IO(Hub):
  match go:
    case True{}:
      Uber.took.u(Hub.state(h), link)
    case False{}:
      IO.pure(Hub, h)

def Uber.again.p(p: Map<&2, String> & Hub, +link: String) -> IO(Hub):
  (+um, h) = p
  Uber.again.k(Bool.not(String.is_empty(UBS.Ub.net.get(um, "again." ++ link))), h, link)

def Uber.answer.p(p: M.State & Hub, +o: J.Json, +link: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Uber.took(h, UBS.Ub.take(st, o, link, now), link, now)

def Uber.answer.go(h: Hub, +o: J.Json, +link: String, +now: Nat) -> IO(Hub):
  Uber.answer.p(Hub.state(h), o, link, now)

def Hub.uber.answered(ok: Bool, m: Maybe<&2, J.Json>, h: Hub, +link: String, +now: Nat) -> IO(Hub):
  match ok m:
    case True{} Some{+o}:
      Uber.answer.go(h, o, link, now)
    case _ _:
      IO.pure(Hub, h)

# another machine's answer to our request
def Hub.uber.answer(h: Hub, +link: String, +code: Nat, +body: String, +now: Nat) -> IO(Hub):
  do IO<Hub>:
    h2 : Hub <- Hub.uber.answered(Nat.is_eq(code, 200n), J.Json.parse(body), Uber.mem.set(h, "busy." ++ link, ""), link, now)
    Uber.again.p(Uber.mem.map(h2), link)

# This machine's id, made once
def Hub.uber.ensure(empty: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match empty:
    case True{}:
      do IO<Hub>:
        +seed : String <- BN.Bn.random(6n)
        Hub.step(h, B.Step{[M.SettingSet{"machine.id", "m" ++ seed, now}], Nil{}})
    case False{}:
      IO.pure(Hub, h)

def Uber.if(some: Bool, h: Hub, cs: List<&2, M.Change>) -> IO(Hub):
  match some:
    case True{}:
      Hub.step(h, B.Step{cs, Nil{}})
    case False{}:
      IO.pure(Hub, h)

def Uber.grace() -> IO(String):
  Env.get("BACKPLANE_UBER_GRACE", "")

def Uber.grace.n(+s: String) -> Nat:
  Bool.pick(Nat, String.is_empty(s), UBS.Ub.grace(), M.Switch.num(s, 0n))

def Uber.init.k(cs: List<&2, M.Change>, h: Hub) -> IO(Hub):
  match cs:
    case Nil{}:
      IO.pure(Hub, h)
    case c <> t:
      Hub.step(h, B.Step{c <> t, Nil{}})

def Uber.init.p(p: M.State & Hub, +now: Nat) -> IO(Hub):
  (st, h) = p
  Uber.init.k(UBS.Ub.init(st, now), h)

# the uberbot's first ops, if it has none
def Uber.init(h: Hub, +now: Nat) -> IO(Hub):
  Uber.init.p(Hub.state(h), now)

# the holder has been quiet for the grace and this machine is first of the
# rest: it takes the lease, with the old holder's last entries from its
# mirror
def Uber.takeover.k(takes: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match takes h:
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{UBS.Ub.takeover.cs(st, now, XR.Xref.handoff(UBS.Ub.handoff.thread(st), UBS.Ub.holder(st), FR.Fh.ins(BN.Bn.far(Cfg.bn(cfg))))), Nil{}})
    case False{} _:
      IO.pure(Hub, h)

def Uber.takeover.s(p: M.State & Hub, +um: Map<&2, String>, +now: Nat, +grace: Nat) -> IO(Hub):
  (st, h) = p
  Uber.takeover.k(UBS.Ub.takes(um, st, now, grace), h, now)

def Uber.takeover.p(p: Map<&2, String> & Hub, +now: Nat, +grace: Nat) -> IO(Hub):
  (+um, h) = p
  Uber.takeover.s(Hub.state(h), um, now, grace)

def Uber.boot.k(first: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match first:
    case True{}:
      IO.pure(Hub, Uber.mem.set(h, "boot", Nat.show(now)))
    case False{}:
      IO.pure(Hub, h)

def Uber.boot.p(p: Map<&2, String> & Hub, +now: Nat) -> IO(Hub):
  (+um, h) = p
  Uber.boot.k(String.is_empty(UBS.Ub.net.get(um, "boot")), h, now)

def Uber.boot(h: Hub, +now: Nat) -> IO(Hub):
  Uber.boot.p(Uber.mem.map(h), now)

# the minute: an id, the first ops, a takeover if it is time, then a round
# with every machine (the holder's heartbeat and the anti-entropy)
def Hub.uber.ensure.p(p: M.State & Hub, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.uber.ensure(String.is_empty(UBS.Ub.me(st)), h, now)

def Hub.desktop.forward(h: Hub, client: U32, +kind: String, +text: String, out: Chan(String), e: String, l: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        Desktop.forward(Cfg.br(cfg), client, kind, Desktop.connection_info(kind, text, Cfg.version(cfg), M.Setting.get(st, "machine.id", "")), out, e, l, Desktop.frame_owner(text), Cef.page.ok(st, Desktop.frame_owner(text)), Cfg.tch(cfg))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# A brand-new home must establish its real persisted machine identity before
# this authenticated connection snapshots it. This reuses the normal one-time
# hub identity operation; it neither runs the network round nor invents a UI ID.
def Hub.desktop.ensure(connected: Bool, h: Hub, now: Nat) -> IO(Hub):
  match connected:
    case True{}: Hub.uber.ensure.p(Hub.state(h), now)
    case False{}: IO.pure(Hub, h)

def Hub.desktop(h: Hub, +client: U32, +kind: String, +text: String, +out: Chan(String), +e: String, +l: String, now: Nat) -> IO(Hub):
  IO.bind(Hub, Hub, Hub.desktop.ensure(String.eq(kind, "connected"), h, now), ready => Hub.desktop.forward(ready, client, kind, text, out, e, l))

def Hub.uber.tick(h: Hub, +now: Nat) -> IO(Hub):
  do IO<Hub>:
    h1 : Hub <- Hub.uber.ensure.p(Hub.state(h), now)
    h2 : Hub <- Uber.boot(h1, now)
    h3 : Hub <- Uber.init(h2, now)
    +g : String <- Uber.grace()
    h4 : Hub <- Uber.takeover.p(Uber.mem.map(h3), now, Uber.grace.n(g))
    Uber.kick(h4)

def Hub.uber.do.k(init: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match init:
    case True{}:
      IO.bind(Hub, Hub, Uber.init(h, now), h2 => Uber.kick(h2))
    case False{}:
      Uber.kick(h)

# the machine that holds the uberbot's conversation says so to the others
# between the minute's ticks when the grace is short (min(60 s, grace/4);
# the Heart message comes every 15 s), so a short silence is not a lapse
def Hub.uber.beat.k(go: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match go:
    case True{}:
      Uber.kick(Uber.mem.set(h, "beatat", Nat.show(now)))
    case False{}:
      IO.pure(Hub, h)

def Hub.uber.beat.s(p: M.State & Hub, +um: Map<&2, String>, +now: Nat, +every: Nat) -> IO(Hub):
  (st, h) = p
  Hub.uber.beat.k(Bool.and(UBS.Ub.beat(st), Bool.and(Nat.is_lt(every, 60n), Nat.is_ge(Nat.sub(now, UBS.Ub.net.nat(um, "beatat")), every))), h, now)

def Hub.uber.beat.m(p: Map<&2, String> & Hub, +now: Nat, +every: Nat) -> IO(Hub):
  (+um, h) = p
  Hub.uber.beat.s(Hub.state(h), um, now, every)

def Hub.uber.beat(h: Hub, +now: Nat) -> IO(Hub):
  do IO<Hub>:
    +g : String <- Uber.grace()
    Hub.uber.beat.m(Uber.mem.map(h), now, UBS.Ub.beat.every(Uber.grace.n(g)))

# this machine was made the home (bots.uber_home): the old holder's last
# entries, from its mirror, wait ahead of the next message
def Uber.home.take(h: Hub, +old: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{UBS.Ub.handoff.cs(st, now, XR.Xref.handoff(UBS.Ub.chat.far(st, old), old, FR.Fh.ins(BN.Bn.far(Cfg.bn(cfg))))), Nil{}})

def Hub.uber.do.h(home: Bool, h: Hub, +what: String, +now: Nat) -> IO(Hub):
  match home:
    case True{}:
      Uber.home.take(h, String.drop(what, 5n), now)
    case False{}:
      Hub.uber.do.k(String.eq(what, "init"), h, now)

def Hub.uber.do(h: Hub, +what: String, +now: Nat) -> IO(Hub):
  Hub.uber.do.h(String.starts_with(what, "home:"), h, what, now)

# Routines keep their minute cadence; the ticker's first message also
# refreshes directories immediately after the hub/window has opened.
def Hub.bot_tick(h: Hub, +tz: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        h2 : Hub <- Hub.dir.refresh(Hub{st, f, ls, cl, as, inbox, cfg, note}, "")
        h3 : Hub <- Hub.pair(h2)
        h4 : Hub <- Hub.step(h3, BH.Bots.tick(st, now, BH.Tz{M.Snooze.tz.secs(tz), M.Snooze.tz.east(tz)}))
        h5 : Hub <- Hub.zone.tick(h4, now)
        h5t : Hub <- Hub.tch.ticks(h5, now)
        Hub.uber.tick(h5t, now)

# Discovery and signed inbound traffic are return signals. Fetch just the
# returning peer, and never bounce every successful directory request.
def Bot.dir.machines(n: Net) -> String:
  match n:
    case Net{_, _, _, _, _, _, _, _, _, mc, _, _, _, _, _, _}:
      mc

def Hub.dir.returned(h: Hub, +mc: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        h2 : Hub <- Hub.machines(Hub{st, f, ls, cl, as, inbox, cfg, note}, mc)
        +returned : List<&2, BO.Peer> = BH.Dir.returned(BO.World.peers(M.State.bots(st)),
          J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(Bot.dir.machines(Cfg.net(cfg))), J.Null{}), "peers")),
          J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(mc), J.Null{}), "peers")))
        h3 : Hub <- Hub.dir.queue(h2, returned)
        Far.returned(h3, returned)

def Bot.dir.awake(refresh: Bool, +inbox: Chan(Msg), +peer: String) -> IO(Unit):
  match refresh:
    case True{}:
      Self.post(inbox, BotsRefresh{peer})
    case False{}:
      IO.pure(Unit, Unit{})

# Requests from outside
# ---------------------
# A webhook (POST /hook/<id>) or a linked machine (POST /bots/deliver,
# POST /bots/link, GET /bots/dir) is checked off the hub: its secret read,
# its signature compared (Sign.check: fresh, constant time). Back on the
# hub the replay cache accepts its key once, and only then does anything
# change. A bot's screen (GET /bots/<id>/screen.jpg) and Google's redirect
# (GET /oauth/google) come from clients the server already lets in.

def Bot.cap() -> Nat:
  262144n

# a form may carry a recording (a Pebble ring's voice note, M4A): 2 MB,
# a few minutes of it
def Bot.cap.form() -> Nat:
  2097152n

def Bot.cap.of(+ct: String) -> Nat:
  Bool.pick(Nat, RG.Ring.is_form(ct), Bot.cap.form(), Bot.cap())

def Bot.answer(+reply: Chan(List<&2, U32>), bs: List<&2, U32>) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, List<&2, U32>, Unit> <- Chan.send(List<&2, U32>, reply, bs)
    return Unit{}

def Hub.answer(h: Hub, +reply: Chan(List<&2, U32>), bs: List<&2, U32>) -> IO(Hub):
  do IO<Hub>:
    Bot.answer(reply, bs)
    return h

def Bot.json(+status: String, +text: String) -> List<&2, U32>:
  W.Http.response(status, "application/json", U.Utf8.encode(text), "")

# POST /far/rpc uber.sync: the answer is what the asking machine lacks
def Hub.uber.in(h: Hub, +link: String, +o: J.Json, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        +name : String <- BN.Bn.name()
        +p : J.Json = J.Json.get_obj(o, "p")
        +ans : String = UBS.Ub.sync.ans(st, name, p)
        h2 : Hub <- Uber.took(Hub{st, f, ls, cl, as, inbox, cfg, note}, UBS.Ub.take(st, p, link, now), link, now)
        Hub.answer(h2, reply, Bot.json("200 OK", ans))

# The requests between hubs
# -------------------------

# POST /far/push: a batch from a linked machine (the owner's own)
# A push from another history is only a resync hint. Ask the owner for
# its current epoch instead of trusting a potentially late old push.
def Far.push.receive.k(changed: Bool, h: Hub, +link: String, +since: Nat, +head: Nat, +xs: List<&2, FR.Item>, +name: String) -> IO(Hub):
  match changed:
    case True{}:
      Far.pull.here(h, link)
    case False{}:
      Far.take(h, link, since, head, xs, name)

def Far.push.receive(r: FR.In & Hub, +epoch: String, +since: Nat, +head: Nat, +xs: List<&2, FR.Item>, +name: String) -> IO(Hub):
  (+i, h) = r
  Far.push.receive.k(FR.Fr.epoch.changed(FR.In.epoch(i), epoch), h, FR.In.link(i), since, head, xs, name)

def Far.live.in.k(+a: FLV.Applied, h: Hub, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +watch : Bool = Bool.not(List.is_empty(&2, Client, cl))
        cl2 : List<&2, Client> <- Far.live.tell(FLV.Applied.dms(a), cl)
        cl3 : List<&2, Client> <- Far.live.snap(FLV.Applied.snap(a), cl2, FLV.Applied.bs(a))
        Bot.answer(reply, Bot.json("200 OK", FLV.FL.ack(watch, FLV.Applied.needs(a))))
        return Hub{st, f, ls, cl3, as, inbox, Live.with(cfg, FLV.Applied.bs(a)), note}

# POST /far/push with `live`: the owner's streamed text
def Far.live.in(h: Hub, +link: String, +at: Nat, xs: List<&2, J.Json>, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Far.live.in.k(FLV.FL.apply(xs, link, at, Live.of(cfg)), Hub{st, f, ls, cl, as, inbox, cfg, note}, reply)

# a peer's turn ended there, and is news: this hub's desk and phones
def Far.note.in.k(+o: J.Json, h: Hub, +link: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, +cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        +now : Nat <- Now.nat()
        +farid : String = FR.Fr.id(link, J.Json.get_str(o, "th", ""))
        +seen : Bool = Bool.and(NO.Alert.holds(J.Json.get_str(o, "kind", "")), DK.Desk.seen(Clients.looks(cl), farid, now))
        cl2 : List<&2, Client> <- Far.note.desk(Desk.lead.held(seen, DK.Desk.lead(Clients.looks(cl))), cl, farid, o)
        IO.spawn(Unit, Far.note.push(Cfg.home(cfg), st, farid, J.Json.get_str(o, "title", ""), J.Json.get_str(o, "body", ""), DK.Desk.hold(seen, J.Json.get_str(o, "key", "")), inbox))
        Bot.answer(reply, Bot.json("200 OK", "{\"ok\":true}"))
        return Hub{st, f, ls, cl2, as, inbox, cfg, note}

def Far.push.in.n(note: Maybe<&2, J.Json>, +o: J.Json, h: Hub, +peer: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match note:
    case Some{+n}:
      Far.note.in.k(n, h, peer, reply)
    case None{}:
      do IO<Hub>:
        Bot.answer(reply, Bot.json("200 OK", "{\"ok\":true}"))
        Far.push.receive(Far.in.of(Far.fh(h), peer, BN.Bn.clean(J.Json.get_str(o, "name", ""))), J.Json.get_str(o, "epoch", ""), J.Json.get_nat(o, "since", 0n), J.Json.get_nat(o, "head", 0n), FR.Fr.items.of(J.Json.elems(J.Json.get_obj(o, "items"))),
          BN.Bn.clean(J.Json.get_str(o, "name", "")))

# a push carries changes (`items`) or, on its own, the owner's streamed text
# (`live`, core/farlive.bend: ephemeral, never in the mirror)
def Far.push.in.k(live: Maybe<&2, J.Json>, +o: J.Json, h: Hub, +peer: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match live:
    case Some{+l}:
      Far.live.in(h, peer, J.Json.get_nat(o, "at", 0n), J.Json.elems(l), reply)
    case None{}:
      Far.push.in.n(J.Json.get(o, "note"), o, h, peer, reply)

def Hub.far.push.in(ok: Bool, m: Maybe<&2, J.Json>, h: Hub, +peer: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match ok m:
    case True{} Some{+o}:
      Far.push.in.k(J.Json.get(o, "live"), o, h, peer, reply)
    case True{} None{}:
      Hub.answer(h, reply, W.Http.text("400 Bad Request", "not JSON"))
    case False{} _:
      Hub.answer(h, reply, W.Http.text("403 Forbidden", "threads are shared with the owner's own machines"))

# The feed is made when a linked machine first asks for it, never at
# start (FR.Fh.feed.ask): that ask starts the job, and it and every other
# until the job is done are told "not ready", so the machine pulls again.
def Far.feed.asked.set(p: FR.Fh & Hub) -> Hub:
  (fh, h) = p
  Far.with(h, FR.Fh.feed.ask(fh))

def Far.feed.want.k(asked: Bool, h: Hub) -> IO(Hub):
  match asked h:
    case True{} _:
      IO.pure(Hub, h)
    case False{} Hub{+st, f, +ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.feed.job(ls, st, inbox))
        return Far.feed.asked.set(Far.fh(Hub{st, f, ls, cl, as, inbox, cfg, note}))

def Far.feed.want(h: Hub) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Far.feed.want.k(FR.Fh.feed.asked(BN.Bn.far(Cfg.bn(cfg))), Hub{st, f, ls, cl, as, inbox, cfg, note})

def Far.log.go(m: Maybe<&2, List<&2, FR.Line>>, h: Hub, +since: Nat, +epoch: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match m h:
    case Some{+fs} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      +own = FR.Fh.epoch(BN.Bn.far(Cfg.bn(cfg)))
      +from = Bool.pick(Nat, FR.Fr.epoch.changed(epoch, own), 0n, since)
      Hub.answer(Hub{st, f, ls, cl, as, inbox, cfg, note}, reply, Bot.json("200 OK", FR.Fr.log.json(from, FR.Fr.catalog(st, FR.Fh.lasts(BN.Bn.far(Cfg.bn(cfg)))),
        FR.Fr.with.epoch(FR.Page.json(FR.Fr.page(fs, from, M.State.seq(st), 400000n)), own))))
    case None{} _:
      IO.bind(Hub, Hub, Far.feed.want(h), h2 => Hub.answer(h2, reply, W.Http.text("503 Service Unavailable", "not ready")))

def Far.log.k(r: Maybe<&2, List<&2, FR.Line>> & Hub, +since: Nat, +epoch: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  (m, h) = r
  Far.log.go(m, h, since, epoch, reply)

# POST /far/log: a page of what this hub shares after `since`
def Hub.far.log(ok: Bool, m: Maybe<&2, J.Json>, h: Hub, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match ok m:
    case True{} Some{+o}:
      Far.log.k(Far.feed.of(Far.fh(h)), J.Json.get_nat(o, "since", 0n), J.Json.get_str(o, "epoch", ""), reply)
    case True{} None{}:
      Hub.answer(h, reply, W.Http.text("400 Bad Request", "not JSON"))
    case False{} _:
      Hub.answer(h, reply, W.Http.text("403 Forbidden", "threads are shared with the owner's own machines"))

# Extract the actual decider reply before sending the step's effects.
def Far.rpc.reply(os: List<&2, B.Out>) -> String:
  match os:
    case Nil{}:
      "{\"t\":\"reply\",\"id\":0,\"ok\":false,\"error\":\"no reply from owning hub\"}"
    case B.Send{_, text} <> _:
      text
    case _ <> t:
      Far.rpc.reply(t)

# (the answer says done only once the step is stored: a write that failed
# is an error to the asking machine, as it is to a client here)
def Far.rpc.stored(r: Hub & Bool, +text: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match r:
    case (h, True{}):
      Hub.answer(h, reply, Bot.json("200 OK", text))
    case (h, False{}):
      Hub.answer(h, reply, W.Http.text("500 Internal Server Error", "that machine could not store it"))

def Hub.far.rpc.step(step: B.Step, h: Hub, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match step:
    case B.Step{cs, +os}:
      IO.bind(Hub & Bool, Hub, Hub.step.ok(h, B.Step{cs, os}), r => Far.rpc.stored(r, Far.rpc.reply(os), reply))

# a folder listed, made or added for a linked machine: the job resolves it
# off the hub and the hub answers (FarDo) once the project is made
def Far.fs.take(m: Maybe<&1, Msg>, +reply: Chan(List<&2, U32>), +inbox: Chan(Msg)) -> IO(Unit):
  match m:
    case Some{Push{_, +text}}:
      Bot.answer(reply, Bot.json("200 OK", text))
    case Some{Rpc{_, +text}}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, FarDo{text, reply})
        return Unit{}
    case _:
      Bot.answer(reply, Bot.json("200 OK", "{\"t\":\"reply\",\"id\":0,\"ok\":false,\"error\":\"no answer from that machine's folders\"}"))

def Far.fs.op(+m: String) -> String:
  Bool.pick(String, String.eq(m, "project.add"), "add", Bool.pick(String, String.eq(m, "project.new"), "new",
    Bool.pick(String, String.eq(m, "fs.mkdir"), "mkdir", Bool.pick(String, String.eq(m, "fs.find"), "find", "list"))))

def Far.fs.job(+m: String, +path: String, +reply: Chan(List<&2, U32>), +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +tmp : Chan(Msg) <- Chan.new(Msg, 4)
    FsJob.run(Far.fs.op(m), tmp, 0, "0", path)
    r : Maybe<&1, Msg> <- Chan.recv(Msg, tmp)
    Far.fs.take(r, reply, inbox)

def Hub.far.rpc.fs(async: Bool, h: Hub, +o: J.Json, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match async h:
    case True{} Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.fs.job(J.Json.get_str(o, "m", ""), J.Json.get_str(J.Json.get_obj(o, "p"), "path", ""), reply, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.far.rpc.step(Bot.request(Some{o}, st, now, Far.client(), text), Hub{st, f, ls, cl, as, inbox, cfg, note}, reply)

# a remote agent's tool call is run by fmcp.bend (Fm.serve); any other
# request is a client's about a shared thread or the owner's folders
def Hub.far.rpc.m(mcp: Bool, h: Hub, +o: J.Json, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match mcp h:
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.far.rpc.step(FM.Fm.serve(st, now, Far.client(), o), Hub{st, f, ls, cl, as, inbox, cfg, note}, reply)
    case False{} _:
      Hub.far.rpc.fs(FR.Fr.async(J.Json.get_str(o, "m", "")), h, o, text, reply, now)

def Hub.far.rpc.k(may: Bool, h: Hub, +o: J.Json, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match may h:
    case True{} h2:
      Hub.far.rpc.m(FM.Fm.is(o), h2, o, text, reply, now)
    case False{} _:
      Hub.answer(h, reply, W.Http.text("403 Forbidden", "not something this machine shares"))

# a folder resolved for a linked machine: the request goes on as it is
# (project.create) with the answer back on the open request
def Hub.far.do(m: Maybe<&2, J.Json>, h: Hub, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match m h:
    case Some{+o} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.far.rpc.step(Bot.request(Some{o}, st, now, Far.client(), text), Hub{st, f, ls, cl, as, inbox, cfg, note}, reply)
    case None{} _:
      Hub.answer(h, reply, W.Http.text("400 Bad Request", "not JSON"))

# A viewer's read about a shared thread (core/fview.bend's Fv.reads) is
# answered off the hub: its job runs on a channel of its own, and the one
# message it sends is the request's answer. view.ver and view.get are the
# folder's files (farview.bend); the others are the jobs the hub's own
# requests become (Fv.jobs), run as they are here.
def Far.view.msg(msg: Msg, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match msg:
    case Push{_, +text}:
      Bot.answer(reply, Bot.json("200 OK", text))
    case _:
      Bot.answer(reply, W.Http.text("500 Internal Server Error", "no answer"))

def Far.view.relay(m: Maybe<&1, Msg>, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match m:
    case Some{+msg}:
      Far.view.msg(msg, reply)
    case None{}:
      Bot.answer(reply, W.Http.text("500 Internal Server Error", "no answer"))

# (the job runs here, in this fiber; a note after it ends the wait for an
# answer a job never sent)
def Far.view.git(+st: M.State, +job: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  do IO<Unit>:
    +pin : Chan(Msg) <- Chan.new(Msg, 8)
    Jobs.run(AG.Text.of(st), job, pin, 0, "0", thread, cwd, a, b, home)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, pin, Announce{""})
    m : Maybe<&1, Msg> <- Chan.recv(Msg, pin)
    Far.view.relay(m, reply)

def Far.view.run(ok: Bool, +st: M.State, +job: String, +thread: String, +cwd: String, +a: String, +b: String, +home: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match ok:
    case True{}:
      Far.view.git(st, job, thread, cwd, a, b, home, reply)
    case False{}:
      Bot.answer(reply, W.Http.text("403 Forbidden", "not something this machine shares"))

def Far.view.out(os: List<&2, B.Out>, +st: M.State, +home: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match os:
    case Nil{}:
      Bot.answer(reply, W.Http.text("404 Not Found", "nothing to show"))
    case B.Send{_, +text} <> _:
      Bot.answer(reply, Bot.json("200 OK", text))
    case B.Git{_, _, +job, +thread, +cwd, +a, +b} <> _:
      Far.view.run(FV.Fv.job(job), st, job, thread, cwd, a, b, home, reply)
    case _ <> t:
      Far.view.out(t, st, home, reply)

def Far.view.step(step: B.Step, +st: M.State, +home: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match step:
    case B.Step{_, +os}:
      Far.view.out(os, st, home, reply)

def Far.view.ver.job(+root: String, +thread: String, +want: String, +have: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  IO.bind(List<&2, U32>, Unit, FVS.Fvs.ver(root, thread, want, have), bs => Bot.answer(reply, bs))

def Far.view.get.job(+root: String, +path: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  IO.bind(List<&2, U32>, Unit, FVS.Fvs.get(root, path), bs => Bot.answer(reply, bs))

def Hub.far.view.k(i: Nat, h: Hub, +o: J.Json, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match i h:
    case 0n Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.view.ver.job(Far.view.cwd(st, J.Json.get_str(J.Json.get_obj(o, "p"), "thread", "")), J.Json.get_str(J.Json.get_obj(o, "p"), "thread", ""),
          J.Json.get_str(J.Json.get_obj(o, "p"), "want", ""), J.Json.get_str(J.Json.get_obj(o, "p"), "have", ""), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case 1n Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.view.get.job(Far.view.cwd(st, J.Json.get_str(J.Json.get_obj(o, "p"), "thread", "")), J.Json.get_str(J.Json.get_obj(o, "p"), "path", ""), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case _ Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.view.step(Bot.request(Some{o}, st, now, Far.client(), text), st, Cfg.home(cfg), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.far.view(ok: Bool, h: Hub, +o: J.Json, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match ok h:
    case True{} _:
      Hub.far.view.k(C.Codec.pos(["view.ver", "view.get"], J.Json.get_str(o, "m", "")), h, o, text, reply, now)
    case False{} _:
      Hub.answer(h, reply, W.Http.text("403 Forbidden", "not something this machine shares"))

def Hub.far.rpc.v(view: Bool, +st: M.State, h: Hub, +o: J.Json, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match view:
    case True{}:
      Hub.far.view(FV.Fv.allowed(st, o), h, o, text, reply, now)
    case False{}:
      Hub.far.rpc.k(FM.Fm.permit(st, o), h, o, text, reply, now)

# On the other machine: the file comes into the cache, then the view message
def Far.show.root(+res: String) -> String:
  Bool.pick(String, J.Json.get_bool(Maybe.default(&2, J.Json, J.Json.parse(res), J.Null{}), "ok", False{}),
    J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(res), J.Null{}), "result"), "root", ""), "")

def Far.show.text(+link: String, +thread: String, +path: String, +kind: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"view"}}, J.KV{"thread", J.Str{FR.Fr.id(link, thread)}}, J.KV{"path", J.Str{path}}, J.KV{"kind", J.Str{kind}}]))

def Far.show.say(+root: String, +inbox: Chan(Msg), +link: String, +thread: String, +rel: String, +kind: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(root), IO.pure(Unit, Unit{}), Far.send(inbox, Shot{Far.show.text(link, thread, root ++ "/" ++ rel, kind)}))

def Far.show.job(+link: String, +url: String, +home: String, +thread: String, +rel: String, +kind: String, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +res : String <- FVS.Fvs.sync(home, link, url, thread, rel, "")
    Far.show.say(Far.show.root(res), inbox, link, thread, rel, kind)

def Hub.far.show(h: Hub, +link: String, +o: J.Json, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.show.job(link, Bot.peer.url(st, link), Cfg.home(cfg), J.Json.get_str(J.Json.get_obj(o, "p"), "thread", ""), J.Json.get_str(J.Json.get_obj(o, "p"), "path", ""),
          J.Json.get_str(J.Json.get_obj(o, "p"), "kind", ""), inbox))
        Hub.answer(Hub{st, f, ls, cl, as, inbox, cfg, note}, reply, Bot.json("200 OK", FVS.Fvs.ok(Nil{})))

# POST /far/rpc: a request about one of this hub's shared threads (Fr.allowed)
def Hub.far.rpc.s(show: Bool, +arg: String, view: Bool, +st: M.State, h: Hub, +o: J.Json, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match show:
    case True{}:
      Hub.far.show(h, arg, o, reply)
    case False{}:
      Hub.far.rpc.v(view, st, h, o, text, reply, now)

# (uber.sync is the uberbot's: the owner's machines exchange its ops)
def Hub.far.rpc.u(uber: Bool, show: Bool, +arg: String, view: Bool, +st: M.State, h: Hub, +o: J.Json, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match uber:
    case True{}:
      Hub.uber.in(h, arg, o, reply, now)
    case False{}:
      Hub.far.rpc.s(show, arg, view, st, h, o, text, reply, now)

def Hub.far.rpc(ok: Bool, +arg: String, m: Maybe<&2, J.Json>, h: Hub, +text: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match ok m h:
    case True{} Some{+o} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.far.rpc.u(String.eq(J.Json.get_str(o, "m", ""), "uber.sync"), FV.Fv.show.is(J.Json.get_str(o, "m", "")), arg, FV.Fv.read(J.Json.get_str(o, "m", "")), st, Hub{st, f, ls, cl, as, inbox, cfg, note}, o, text, reply, now)
    case True{} None{} _:
      Hub.answer(h, reply, W.Http.text("400 Bad Request", "not JSON"))
    case False{} _ _:
      Hub.answer(h, reply, W.Http.text("403 Forbidden", "threads are shared with the owner's own machines"))

def Bot.shot.sent(bs: List<&2, U32>, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match bs:
    case Nil{}:
      Bot.answer(reply, W.Http.text("404 Not Found", "no screen yet"))
    case b <> t:
      Bot.answer(reply, W.Http.response.c("200 OK", "image/jpeg", b <> t, "no-store", ""))

# a bot's newest browser frame, as the helper wrote it
def Bot.shot(+home: String, +bot: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(bot), Bot.answer(reply, W.Http.text("404 Not Found", "no such bot")),
    IO.bind(List<&2, U32>, Unit, Files.read(BR.Br.shot_file(home, bot)), bs => Bot.shot.sent(bs, reply)))

def Bot.oauth.page(+text: String) -> String:
  GG.Google.page(Bool.pick(String, String.starts_with(text, "error"), text, "Google is connected to Backplane. You can close this tab."))

def Bot.oauth.here(+home: String, +port: U32, +path: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  IO.bind(String, Unit, GS.Google.finish(home, path, Bot.redirect(port)),
    t => Bot.answer(reply, W.Http.response("200 OK", "text/html; charset=utf-8", U.Utf8.encode(Bot.oauth.page(t)), "")))

def Bot.oauth.to(+to: String, +home: String, +port: U32, +path: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(to), Bot.oauth.here(home, port, path, reply),
    Bot.answer(reply, W.Http.response("302 Found", "text/plain; charset=utf-8", U.Utf8.encode("Signing in on " ++ to), "Location: " ++ to ++ "\r\n")))

# Google sent the browser back with a code: finish signing in, or, when
# the sign-in was started on another of the owner's hubs (the browser runs
# here, that hub elsewhere), send the browser on to it (GG.Google.hop)
def Bot.oauth(+home: String, +port: U32, +path: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  IO.bind(String, Unit, GS.Google.pending(home),
    mine => Bot.oauth.to(GG.Google.hop(GG.Url.query(path), mine), home, port, path, reply))

def Bot.me(+link: Bool) -> IO(String):
  Bool.pick(IO(String), link, BN.Bn.name(), IO.pure(String, ""))

# the secret of the hook or the peer, the signature against it, and the
# replay key (or "") back to the hub
def Bot.refuse(+id: String, +key: String, body: String, +reply: Chan(List<&2, U32>), +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, BotVerified{"hook", id, key, body, "", reply})
    return Unit{}

# a form (the Pebble Index ring's) becomes a JSON object; any other body
# goes as it came. Only a call let in is read.
def Bot.bearer.body(form: Bool, +body: String, +size: String) -> String:
  match form:
    case True{}:
      RG.Ring.form(body, size)
    case False{}:
      body

# A ring's recording, let in, is heard here before the bot gets the form
# (VO.Vo.ring: GPT-Live-Transcribe with the dictionary): the phone gets its
# 202 at once, and the form, with the words heard, goes on when they are
# (the hub's own answer then goes nowhere). A form under a bearer token
# becomes JSON as usual.
def Bot.heard(+home: String, +kind: String, +id: String, +key: String, +ct: String, +body: String, raw: List<&2, U32>, +me: String, form: Bool, +size: String,
    +reply: Chan(List<&2, U32>), +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    Bot.answer(reply, W.Http.text("202 Accepted", "accepted"))
    +nonce : String <- BN.Bn.random(6n)
    +heard : String <- VO.Vo.ring(home, nonce, ct, body, raw)
    +none : Chan(List<&2, U32>) <- Chan.new(List<&2, U32>, 1)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, BotVerified{kind, id, key, Bot.bearer.body(form, heard, size), me, none})
    return Unit{}

def Bot.verify.go(heard: Bool, +home: String, +kind: String, +id: String, +key: String, +ct: String, +body: String, raw: List<&2, U32>, +me: String,
    +reply: Chan(List<&2, U32>), +inbox: Chan(Msg)) -> IO(Unit):
  match heard:
    case True{}:
      Bot.heard(home, kind, id, key, ct, body, raw, me, False{}, "", reply, inbox)
    case False{}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, BotVerified{kind, id, key, body, me, reply})
        return Unit{}

# the replay key: a form with a recording is checked over its bytes (the
# ring signs those; the recording is not text)
def Bot.verify.key(text: Bool, +hook: Bool, +secret: String, +now: Nat, +key: String, +ts: String, +sig: String, +gh: String, +dl: String, +body: String,
    raw: List<&2, U32>) -> String:
  match text:
    case True{}:
      BN.Bn.check(hook, secret, now, key, ts, sig, gh, dl, body)
    case False{}:
      BN.Bn.check.raw(secret, now, key, gh, raw)

# the secret of the hook or the peer, the signature against it, and the
# replay key (or "") back to the hub
def Bot.verify(+hook: Bool, +home: String, +kind: String, +id: String, +ts: String, +sig: String, +gh: String, +dl: String, +body: String,
    +ct: String, +raw: List<&2, U32>, +reply: Chan(List<&2, U32>), +now: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    +secret : String <- BN.Bn.secret.of(hook, home, id)
    +me : String <- Bot.me(String.eq(kind, "link"))
    +key : String = Bot.verify.key(List.is_empty(&2, U32, raw), hook, secret, now, kind ++ ":" ++ id, ts, sig, gh, dl, body, raw)
    Bot.verify.go(Bool.and(Bool.not(List.is_empty(&2, U32, raw)), Bool.not(String.is_empty(key))), home, kind, id, key, ct, body, raw,
      Bool.pick(String, String.contains(key, ":i:"), "ring", me), reply, inbox)

def Bot.bearer.go(heard: Bool, form: Bool, +home: String, +key: String, +ct: String, +size: String, +id: String, +body: String, raw: List<&2, U32>,
    +reply: Chan(List<&2, U32>), +inbox: Chan(Msg)) -> IO(Unit):
  match heard:
    case True{}:
      Bot.heard(home, "hook", id, key, ct, body, raw, "", True{}, size, reply, inbox)
    case False{}:
      Bot.refuse(id, key, Bot.bearer.body(form, body, size), reply, inbox)

def Bot.bearer.k(+home: String, +key: String, +ct: String, +size: String, +id: String, +body: String, +raw: List<&2, U32>,
    +reply: Chan(List<&2, U32>), +inbox: Chan(Msg)) -> IO(Unit):
  Bot.bearer.go(Bool.and(Bool.and(Bool.not(BN.Bn.refused(key)), RG.Ring.is_form(ct)), Bool.not(List.is_empty(&2, U32, raw))),
    Bool.and(Bool.not(BN.Bn.refused(key)), RG.Ring.is_form(ct)), home, key, ct, size, id, body, raw, reply, inbox)

# a webhook called with a bearer token (packed by Bot.gh as
# "bearer\n<token>\n<content type>\n<X-Audio-Size>"): the token compared
# with the hook's own off the hub
def Bot.bearer(fs: List<&2, String>, +home: String, +id: String, +body: String, +full: String, +raw: List<&2, U32>, +reply: Chan(List<&2, U32>), +now: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  match fs:
    case _ <> +tok <> +ct <> +size <> _:
      do IO<Unit>:
        +kept : String <- BN.Bn.token.of(home, id)
        +nonce : String <- BN.Bn.random(2n)
        Bot.bearer.k(home, BN.Bn.bearer(kept, tok, "hook:" ++ id, Nat.show(now) ++ "." ++ nonce), Bool.pick(String, String.is_empty(full), ct, full), size, id, body, raw, reply, inbox)
    case _:
      Bot.refuse(id, "!bearer", body, reply, inbox)

# how a webhook call proves itself: 0 not at all, 1 a bearer token, 2 a
# signature (ours, GitHub's, the ring's). A signature wins over a token.
def Bot.hook.how(+sig: String, +gh: String) -> Nat:
  Bool.pick(Nat, Bool.not(String.is_empty(sig)), 2n,
    Bool.pick(Nat, String.starts_with(gh, "bearer\n"), 1n, Bool.pick(Nat, String.is_empty(gh), 0n, 2n)))

def Bot.hook.in(i: Nat, +home: String, +id: String, +ts: String, +sig: String, +gh: String, +dl: String, +body: String, +ct: String, +raw: List<&2, U32>,
    +reply: Chan(List<&2, U32>), +now: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  match i:
    case 0n:
      Bot.refuse(id, "!none", body, reply, inbox)
    case 1n:
      Bot.bearer(String.split(gh, '\n'), home, id, body, ct, raw, reply, now, inbox)
    case _:
      Bot.verify(True{}, home, "hook", id, ts, sig, gh, dl, body, ct, raw, reply, now, inbox)

def Bot.http.job(i: Nat, +home: String, +port: U32, +kind: String, +arg: String, +peer: String, +ts: String, +sig: String, +gh: String, +dl: String,
    +body: String, +ct: String, +raw: List<&2, U32>, +reply: Chan(List<&2, U32>), +now: Nat, +inbox: Chan(Msg)) -> IO(Unit):
  match i:
    case 0n:
      Bot.shot(home, arg, reply)
    case 1n:
      Bot.oauth(home, port, arg, reply)
    case 2n:
      Bot.hook.in(Bot.hook.how(sig, gh), home, arg, ts, sig, gh, dl, body, ct, raw, reply, now, inbox)
    case _:
      Bot.verify(False{}, home, kind, peer, ts, sig, gh, dl, body, "", Nil{}, reply, now, inbox)

def Bot.page.sent(none: Bool, +html: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match none:
    case True{}:
      Bot.answer(reply, W.Http.text("404 Not Found", "no page"))
    case False{}:
      Bot.answer(reply, W.Http.response.c("200 OK", "text/html; charset=utf-8", U.Utf8.encode(SPC.Space.page(html)), "no-store",
        "Content-Security-Policy: " ++ SPC.Space.page.csp() ++ "\r\nX-Content-Type-Options: nosniff\r\n"))

# a bot's space page (Space.page: no scripts, nothing from the network)
# a bot's page: one here from the read model, one mirrored from a linked
# machine ("<link>~<id>") from its mirror
def Hub.bot.page.html(+st: M.State, +cfg: Cfg, +bot: String) -> String:
  Bool.pick(String, FR.Fr.is(bot), FR.In.page(FR.Fh.in(BN.Bn.far(Cfg.bn(cfg)), FR.Fr.link(bot), ""), FR.Fr.local(bot)),
    BO.Page.html(BO.World.spage(M.State.bots(st), bot)))

def Hub.bot.page(h: Hub, +bot: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +html : String = Hub.bot.page.html(st, cfg, bot)
        IO.spawn(Unit, Bot.page.sent(Bool.or(String.is_empty(bot), String.is_empty(html)), html, reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.bot_http.job(h: Hub, +kind: String, +arg: String, +peer: String, +ts: String, +sig: String, +gh: String, +dl: String, +body: String,
    +ct: String, +raw: List<&2, U32>, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Bot.http.job(C.Codec.pos(["shot", "oauth", "hook"], kind), Cfg.home(cfg), Cfg.port(cfg), kind, arg, peer, ts, sig, gh, dl, body, ct, raw, reply, now, inbox))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

# POST /bots/pair?name=<its name>&url=<its address>: one of the owner's
# own machines (trusted: Tailscale vouches, as for the websocket) asks to
# link; the answer is a fresh invite ({"invite"}), unless it is none of
# the machines this hub found (403: the link could not reach back), sync
# with it is off here (403) or a live link already reaches it (409)
def Pair.invited(ok: Bool, +code: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match ok:
    case True{}:
      Bot.answer(reply, Bot.json("200 OK", J.Json.show(J.Json.obj([J.KV{"invite", J.Str{code}}]))))
    case False{}:
      Bot.answer(reply, W.Http.text("500 Internal Server Error", "could not keep the invite's secret"))

def Pair.invite(+home: String, +tailnet: Bool, +host: String, +port: U32, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  do IO<Unit>:
    +here : String <- BN.Bn.url.here(tailnet, host, port)
    +r : String <- BN.Bn.random(2n)
    +peer : String = "p" ++ r
    +secret : String <- BN.Bn.secret.fresh()
    ok : Bool <- BN.Bn.peer.save(home, peer, secret)
    Pair.invited(ok, SG.Invite.make(here, peer, secret), reply)

def Hub.bot.pair.k(i: Nat, h: Hub, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match i h:
    case 0n Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Pair.invite(Cfg.home(cfg), Cfg.tailnet(cfg), Cfg.host(cfg), Cfg.port(cfg), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case 1n _:
      Hub.answer(h, reply, W.Http.text("403 Forbidden", "sync with that machine is off here"))
    case 3n _:
      Hub.answer(h, reply, W.Http.text("403 Forbidden", "not one of the machines this hub found"))
    case _ _:
      Hub.answer(h, reply, W.Http.text("409 Conflict", "already linked"))

def Hub.bot.pair(h: Hub, +arg: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.bot.pair.k(UC.Mate.answer(Far.machines(cfg), BO.World.peers(M.State.bots(st)), st,
        BN.Bn.clean(AT.Att.unpct(W.Http.param(arg, "name"))), AT.Att.unpct(W.Http.param(arg, "url"))), Hub{st, f, ls, cl, as, inbox, cfg, note}, reply)

# A window on this hub from another machine (core/rview.bend): the files
# of a thread's folder for its viewers, read only. /view/ver lists them
# with a digest and waits for a change when the client holds it already,
# /view/get is one file's bytes, /view/git one file of a commit. Clients
# let in as for the websocket (Conn.allowed); the folder is the thread's
# own (for a thread mirrored here, the cache this hub keeps of the owner's).
def View.git.sent(bs: List<&2, U32>, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match bs:
    case Nil{}:
      Bot.answer(reply, W.Http.text("404 Not Found", "no such version"))
    case b <> t:
      Bot.answer(reply, W.Http.response.c("200 OK", "application/octet-stream", b <> t, "no-store", ""))

def View.git.job(+path: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  IO.bind(List<&2, U32>, Unit, GT.Git.version(path), bs => View.git.sent(bs, reply))

def Hub.bot.view.k(ok: Bool, +kind: Nat, h: Hub, +root: String, +th: String, +want: String, +have: String, +path: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match ok:
    case True{}:
      do IO<Hub>:
        IO.spawn(Unit, Bool.pick(IO(Unit), Nat.is_eq(kind, 1n), Far.view.ver.job(root, th, want, have, reply),
          Bool.pick(IO(Unit), Nat.is_eq(kind, 2n), Far.view.get.job(root, path, reply), View.git.job(path, reply))))
        return h
    case False{}:
      Hub.answer(h, reply, W.Http.text("403 Forbidden", "not a thread of this machine, or not a file the viewers read"))

def Hub.bot.view(h: Hub, +arg: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.bot.view.k(RVW.Rview.ask.ok(W.Http.route(arg), RVW.Rview.root(Cfg.home(cfg), st, AT.Att.unpct(W.Http.param(arg, "thread")), AT.Att.unpct(W.Http.param(arg, "root"))), RVW.Rview.proot(st, AT.Att.unpct(W.Http.param(arg, "thread"))), AT.Att.unpct(W.Http.param(arg, "want")), AT.Att.unpct(W.Http.param(arg, "path"))),
        RVW.Rview.kind(W.Http.route(arg)), Hub{st, f, ls, cl, as, inbox, cfg, note}, RVW.Rview.root(Cfg.home(cfg), st, AT.Att.unpct(W.Http.param(arg, "thread")), AT.Att.unpct(W.Http.param(arg, "root"))),
        AT.Att.unpct(W.Http.param(arg, "thread")), AT.Att.unpct(W.Http.param(arg, "want")), AT.Att.unpct(W.Http.param(arg, "have")), AT.Att.unpct(W.Http.param(arg, "path")), reply)

# GET /bots/<id>/tasks/shot?run=<run>&n=<n> (or &f=<file>): a picture of a
# task run (a pause's, or a step's), only from that bot's own tasks folder;
# the run and file names are plain (no / or ..)
def Bot.tshot.file(+p: String) -> String:
  Bool.pick(String, String.is_empty(AT.Att.unpct(W.Http.param(p, "f"))),
    TE.Teach.pause.file(Maybe.default(&2, Nat, Nat.read(W.Http.param(p, "n")), 0n)), AT.Att.unpct(W.Http.param(p, "f")))

def Bot.tshot.go(ok: Bool, +path: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(List<&2, U32>, Unit, Files.read(path), bs => Bot.shot.sent(bs, reply))
    case False{}:
      Bot.answer(reply, W.Http.text("404 Not Found", "no such picture"))

def Bot.hook.live.k(hk: BO.Hook, +st: M.State) -> Bool:
  match hk:
    case BO.Hook{_, +bot, _, _, revoked, _, _}:
      Bool.and(Bool.not(revoked), BH.Bots.live(st, bot))

def Bot.hook.live(+st: M.State, +id: String) -> Bool:
  Bot.hook.live.k(BO.World.hook(M.State.bots(st), id), st)

# a ring's voice note reaches the bot as the person's words (Bots.ring);
# any other webhook's body as untrusted data (Bots.hooked)
def Bot.hook.step(ring: Bool, +st: M.State, +now: Nat, +hook: String, +body: String) -> B.Step:
  match ring:
    case True{}:
      BH.Bots.ring(st, now, hook, body)
    case False{}:
      BH.Bots.hooked(st, now, hook, body)

def Hub.bot.hook(live: Bool, h: Hub, +hook: String, +body: String, ring: Bool, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match live h:
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        Bot.answer(reply, W.Http.text("202 Accepted", "accepted"))
        Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, Bot.hook.step(ring, st, now, hook, body))
    case False{} _:
      Hub.answer(h, reply, W.Http.text("401 Unauthorized", "no such webhook"))

def Hub.bot.deliver(live: Bool, m: Maybe<&2, J.Json>, h: Hub, +peer: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match live m h:
    case True{} Some{+o} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        Bot.answer(reply, Bot.json("200 OK", "{\"ok\":true}"))
        Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, BH.Bots.incoming(st, now, Bot.peer.name(st, peer), o))
    case True{} None{} _:
      Hub.answer(h, reply, W.Http.text("400 Bad Request", "not JSON"))
    case False{} _ _:
      Hub.answer(h, reply, W.Http.text("401 Unauthorized", "not a linked machine"))

def Hub.bot.link.k(ok: Bool, h: Hub, +url: String, +name: String, +peer: String, +me: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match ok:
    case True{}:
      do IO<Hub>:
        Bot.answer(reply, Bot.json("200 OK", J.Json.show(J.Json.obj([J.KV{"name", J.Str{me}}]))))
        Hub.step(h, B.Step{[M.PeerSet{peer, Bool.pick(String, String.is_empty(name), peer, name), url, False{}, now}], Nil{}})
    case False{}:
      Hub.answer(h, reply, W.Http.text("400 Bad Request", "no http address"))

# an invite used: the machine that holds it says where it is; once only
def Hub.bot.link(fresh: Bool, m: Maybe<&2, J.Json>, h: Hub, +peer: String, +me: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match fresh m:
    case True{} Some{+o}:
      Hub.bot.link.k(BN.Bn.url.ok(J.Json.get_str(o, "url", "")), h, J.Json.get_str(o, "url", ""), BN.Bn.clean(J.Json.get_str(o, "name", "")), peer, me, reply, now)
    case True{} None{}:
      Hub.answer(h, reply, W.Http.text("400 Bad Request", "not JSON"))
    case False{} _:
      Hub.answer(h, reply, W.Http.text("409 Conflict", "already linked"))

def Hub.bot.dir(live: Bool, h: Hub, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match live h:
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.answer(Hub{st, f, ls, cl, as, inbox, cfg, note}, reply, Bot.json("200 OK", BN.Bn.mine(st)))
    case False{} _:
      Hub.answer(h, reply, W.Http.text("401 Unauthorized", "not a linked machine"))

def Hub.bot.act(i: Nat, h: Hub, +arg: String, +body: String, +me: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match i h:
    case 0n Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.bot.hook(Bot.hook.live(st, arg), Hub{st, f, ls, cl, as, inbox, cfg, note}, arg, body, String.eq(me, "ring"), reply, now)
    case 1n Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.bot.deliver(Bot.peer.live(st, arg), J.Json.parse(body), Hub{st, f, ls, cl, as, inbox, cfg, note}, arg, reply, now)
    case 2n Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.bot.link(Bool.not(Bot.peer.live(st, arg)), J.Json.parse(body), Hub{st, f, ls, cl, as, inbox, cfg, note}, arg, me, reply, now)
    case 3n Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.bot.dir(Bot.peer.live(st, arg), Hub{st, f, ls, cl, as, inbox, cfg, note}, reply)
    case 4n Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.far.push.in(Far.own.id(st, cfg, arg), J.Json.parse(body), Hub{st, f, ls, cl, as, inbox, cfg, note}, arg, reply)
    case 5n Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.far.log(Far.own.id(st, cfg, arg), J.Json.parse(body), Hub{st, f, ls, cl, as, inbox, cfg, note}, reply)
    case 6n Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.far.rpc(Far.own.id(st, cfg, arg), arg, J.Json.parse(body), Hub{st, f, ls, cl, as, inbox, cfg, note}, body, reply, now)
    case _ _:
      Hub.answer(h, reply, W.Http.text("404 Not Found", "not found"))

def Hub.bot.seen(bad: Bool, m: Maybe<&2, SG.Seen>, h: Hub, +why: String, +kind: String, +arg: String, body: String, +me: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match bad m h:
    case True{} _ _:
      Hub.answer(h, reply, W.Http.text("401 Unauthorized", why))
    case False{} None{} _:
      Hub.answer(h, reply, W.Http.text("401 Unauthorized", "already seen"))
    case False{} Some{s} Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        Bot.dir.awake(Bool.and(Bool.or(String.eq(kind, "dir"), String.eq(kind, "deliver")), BH.Dir.stale(
          J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(BN.Bn.dir(Cfg.bn(cfg))), J.Null{})), arg)), inbox, arg)
        Hub.bot.act(C.Codec.pos(["hook", "deliver", "link", "dir", "far-push", "far-log", "far-rpc"], kind), Hub{st, f, ls, cl, as, inbox, Cfg.with_bn(cfg, BN.Bn.with_seen(Cfg.bn(cfg), s)), note},
        arg, body, me, reply, now)

# a signed request (or a webhook's bearer token), checked: genuine (key),
# not seen before, then acted on; a refusal says why
def Hub.bot_verified(h: Hub, +kind: String, +arg: String, +key: String, body: String, +me: String, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.bot.seen(BN.Bn.refused(key), BN.Bn.accept(Cfg.bn(cfg), key, now), Hub{st, f, ls, cl, as, inbox, cfg, note}, BN.Bn.denied(key), kind, arg, body, me, reply, now)

# Connections for bots' routes
# ----------------------------

type BotReq is Data:
  BotReq{kind: String, arg: String, peer: String, ts: String, sig: String, gh: String, delivery: String}

def Conn.bot.kind.k(+post: Bool, +get: Bool, +r: String) -> Nat:
  Bool.pick(Nat, Bool.and(post, String.starts_with(r, "/hook/")), 1n,
    Bool.pick(Nat, Bool.and(post, String.eq(r, "/bots/deliver")), 2n,
      Bool.pick(Nat, Bool.and(post, String.eq(r, "/bots/link")), 3n,
        Bool.pick(Nat, Bool.and(get, String.eq(r, "/bots/dir")), 4n,
          Bool.pick(Nat, Bool.and(get, Bool.and(String.starts_with(r, "/bots/"), String.ends_with(r, "/screen.jpg"))), 5n,
            Bool.pick(Nat, Bool.and(get, String.eq(r, "/oauth/google")), 6n,
              Bool.pick(Nat, Bool.and(get, Bool.and(String.starts_with(r, "/bots/"), String.ends_with(r, "/space.html"))), 7n,
                Bool.pick(Nat, Bool.and(post, String.eq(r, "/far/push")), 8n,
                  Bool.pick(Nat, Bool.and(post, String.eq(r, "/far/log")), 9n,
                    Bool.pick(Nat, Bool.and(post, String.eq(r, "/far/rpc")), 10n,
                      Bool.pick(Nat, Bool.and(post, String.eq(r, "/bots/pair")), 11n, Bool.pick(Nat, Bool.and(get, Nat.is_gt(RVW.Rview.kind(r), 0n)), 12n,
                        Bool.pick(Nat, Bool.and(get, Bool.and(String.starts_with(r, "/bots/"), String.ends_with(r, "/tasks/shot"))), 13n, 0n)))))))))))))

# which bots' route a request is (0: none)
def Conn.bot.kind(req: W.Request) -> Nat:
  match req:
    case W.Request{+m, +p, _}:
      Conn.bot.kind.k(String.eq(m, "POST"), String.eq(m, "GET"), W.Http.route(p))

# /bots/<id>/screen.jpg: the id ("" when it is not a plain id)
def Conn.bot.shot_id(+r: String) -> String:
  Bool.pick(String, BN.Bn.safe(String.take(String.drop(r, 6n), Nat.sub(String.length(r), 17n))), String.take(String.drop(r, 6n), Nat.sub(String.length(r), 17n)), "")

def Bot.tshot(+home: String, +p: String, +reply: Chan(List<&2, U32>)) -> IO(Unit):
  Bot.tshot.go(Bool.and(BN.Bn.safe(Conn.bot.shot_id(W.Http.route(p))), Bool.and(TE.Teach.run.ok(AT.Att.unpct(W.Http.param(p, "run"))), TE.Teach.file.ok(Bot.tshot.file(p)))),
    home ++ "/bots/" ++ Conn.bot.shot_id(W.Http.route(p)) ++ "/" ++ TE.Teach.run.dir(AT.Att.unpct(W.Http.param(p, "run"))) ++ "/" ++ Bot.tshot.file(p), reply)

def Hub.bot.tshot(h: Hub, +arg: String, +reply: Chan(List<&2, U32>)) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Bot.tshot(Cfg.home(cfg), arg, reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.bot_http.page(i: Nat, h: Hub, +kind: String, +arg: String, +peer: String, +ts: String, +sig: String, +gh: String, +dl: String, +body: String,
    +ct: String, +raw: List<&2, U32>,
    +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  match i:
    case 0n:
      Hub.bot.page(h, arg, reply)
    case 1n:
      Hub.bot.pair(h, arg, reply)
    case 2n:
      Hub.bot.view(h, arg, reply)
    case 3n:
      Hub.bot.tshot(h, arg, reply)
    case _:
      Hub.bot_http.job(h, kind, arg, peer, ts, sig, gh, dl, body, ct, raw, reply, now)

def Hub.bot_http(h: Hub, +kind: String, +arg: String, +peer: String, +ts: String, +sig: String, +gh: String, +dl: String, +body: String,
    +ct: String, +raw: List<&2, U32>, +reply: Chan(List<&2, U32>), +now: Nat) -> IO(Hub):
  Hub.bot_http.page(C.Codec.pos(["page", "pair", "view", "tshot"], kind), h, kind, arg, peer, ts, sig, gh, dl, body, ct, raw, reply, now)

# /bots/<id>/space.html: the id ("" when it is not a plain id)
# (a plain id, or a mirrored bot's "<link>~<id>" of two plain ids)
def Conn.bot.page_ok(+id: String) -> Bool:
  Bool.or(BN.Bn.safe(id), Bool.and(FR.Fr.is(id), Bool.and(BN.Bn.safe(FR.Fr.link(id)), BN.Bn.safe(FR.Fr.local(id)))))

def Conn.bot.page_id(+r: String) -> String:
  Bool.pick(String, Conn.bot.page_ok(String.take(String.drop(r, 6n), Nat.sub(String.length(r), 17n))), String.take(String.drop(r, 6n), Nat.sub(String.length(r), 17n)), "")

def Conn.bot.answer(m: Maybe<&1, List<&2, U32>>, s: Socket) -> IO(Unit):
  match m:
    case Some{bs}:
      Conn.finish(s, bs)
    case None{}:
      Conn.finish(s, W.Http.text("503 Service Unavailable", "the hub is gone"))

# the hub answers the whole response on a channel made for this request
def Conn.bot.ask(s: Socket, rq: BotReq, body: String, +ct: String, raw: List<&2, U32>, +inbox: Chan(Msg)) -> IO(Unit):
  match rq:
    case BotReq{k, a, p, t, sg, g, d}:
      do IO<Unit>:
        +reply : Chan(List<&2, U32>) <- Chan.new(List<&2, U32>, 1)
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, BotHttp{k, a, p, t, sg, g, d, body, ct, raw, reply})
        m : Maybe<&1, List<&2, U32>> <- Chan.recv(List<&2, U32>, reply)
        Conn.bot.answer(m, s)

# the body as text, and its bytes too when it is a form with a recording
def Conn.bot.bytes(s: Socket, +rq: BotReq, +bs: List<&2, U32>, +text: String, +ct: String, +inbox: Chan(Msg)) -> IO(Unit):
  Conn.bot.ask(s, rq, text, ct, Bool.pick(List<&2, U32>, Bool.and(RG.Ring.is_form(ct), String.contains(text, "name=\"audio\"")), bs, Nil{}), inbox)

def Conn.bot.body.k(full: Bool, s: Socket, +buf: List<&2, U32>, +len: Nat, +rq: BotReq, +ct: String, +inbox: Chan(Msg), k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  match full:
    case True{}:
      Conn.bot.bytes(s, rq, List.take(&2, U32, buf, len), U.Utf8.decode(List.take(&2, U32, buf, len)), ct, inbox)
    case False{}:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536), r => Mcp.got(r, buf, k))

# read the body (Content-Length bytes, capped before reading)
def Conn.bot.body(fuel: Nat, s: Socket, +buf: List<&2, U32>, +len: Nat, +rq: BotReq, +ct: String, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      Conn.finish(s, W.Http.text("408 Request Timeout", "the body did not arrive"))
    case 1n+f:
      Conn.bot.body.k(Nat.is_ge(List.length(&2, U32, buf), len), s, buf, len, rq, ct, inbox, s2 => b2 => Conn.bot.body(f, s2, b2, len, rq, ct, inbox))

def Conn.bot.signed.k(ok: Bool, big: Bool, s: Socket, rest: List<&2, U32>, +len: Nat, +rq: BotReq, +ct: String, +inbox: Chan(Msg)) -> IO(Unit):
  match ok big:
    case False{} _:
      Conn.finish(s, W.Http.text("401 Unauthorized", "unknown sender"))
    case True{} True{}:
      Conn.finish(s, W.Http.text("413 Payload Too Large", "at most 256 KB, or 2 MB for a form"))
    case True{} False{}:
      Conn.bot.body(4096n, s, rest, len, rq, ct, inbox)

def Conn.bot.len(+hs: List<&2, W.Header>) -> Nat:
  Maybe.default(&2, Nat, Nat.read(W.Http.header_of(hs, "content-length")), 0n)

def Bot.bearer.of(+auth: String) -> String:
  Bool.pick(String, Bool.or(String.starts_with(auth, "Bearer "), String.starts_with(auth, "bearer ")), String.trim(String.drop(auth, 7n)), "")

# Authorization: Bearer <token>, with what reading the body needs, packed
# after "bearer\n" ("" when there is no token)
def Bot.gh.bearer(+tok: String, +hs: List<&2, W.Header>) -> String:
  Bool.pick(String, String.is_empty(tok), "",
    "bearer\n" ++ tok ++ "\n" ++ W.Http.header_of(hs, "content-type") ++ "\n" ++ W.Http.header_of(hs, "x-audio-size"))

def Bot.gh.hub(+g: String, +hs: List<&2, W.Header>) -> String:
  Bool.pick(String, String.is_empty(g), Bot.gh.bearer(Bot.bearer.of(W.Http.header_of(hs, "authorization")), hs), g)

# a third party's signature: the Pebble Index ring's (its X-Index-*
# headers, packed after "index\n" for BN.Bn.check), else GitHub's, else a
# bearer token
def Bot.gh.k(ring: Bool, +hs: List<&2, W.Header>) -> String:
  match ring:
    case True{}:
      "index\n" ++ String.join([W.Http.header_of(hs, "x-index-webhook-version"), W.Http.header_of(hs, "x-index-timestamp"),
        W.Http.header_of(hs, "x-index-delivery"), W.Http.header_of(hs, "x-index-trigger"), W.Http.header_of(hs, "x-index-test"),
        W.Http.header_of(hs, "x-index-signature")], "\n")
    case False{}:
      Bot.gh.hub(W.Http.header_of(hs, "x-hub-signature-256"), hs)

def Bot.gh(+hs: List<&2, W.Header>) -> String:
  Bot.gh.k(Bool.not(String.is_empty(W.Http.header_of(hs, "x-index-signature"))), hs)

# a signed request (no token: webhooks come from outside; peers sign)
def Conn.bot.signed(+kind: String, +arg: String, s: Socket, +hs: List<&2, W.Header>, rest: List<&2, U32>, +inbox: Chan(Msg)) -> IO(Unit):
  Conn.bot.signed.k(BN.Bn.safe(arg), Nat.is_gt(Conn.bot.len(hs), Bot.cap.of(W.Http.header_of(hs, "content-type"))), s, rest, Conn.bot.len(hs),
    BotReq{kind, arg, W.Http.header_of(hs, "x-backplane-peer"), W.Http.header_of(hs, "x-backplane-timestamp"), W.Http.header_of(hs, "x-backplane-signature"),
      Bot.gh(hs), W.Http.header_of(hs, "x-github-delivery")}, W.Http.header_of(hs, "content-type"), inbox)

# a client's own route (a bot's screen, Google's redirect): the same
# authorisation as the websocket's
def Conn.bot.own(ok: Bool, s: Socket, rq: BotReq, +inbox: Chan(Msg)) -> IO(Unit):
  match ok:
    case True{}:
      Conn.bot.ask(s, rq, "", "", Nil{}, inbox)
    case False{}:
      Conn.finish(s, W.Http.text("401 Unauthorized", "this Backplane needs its pairing token"))

# Uploads over HTTP
# -----------------
# POST /attach?thread=<id>&key=<key>&name=<name> with the file as the body: the
# phones' way to attach (no base64, no pieces through the hub). The hub
# only names the folder; the bytes after the head go straight from the
# socket to the file (H.Sock.save) off the hub. The answer is what an
# attach.put answers: {"attached", "thread"}, or {"text", "thread",
# "upload"} when it failed.

def Conn.is_att(req: W.Request) -> Bool:
  match req:
    case W.Request{m, p, _}:
      Bool.and(String.eq(m, "POST"), String.eq(W.Http.route(p), "/attach"))

def Upload.json(+status: String, +ok: Bool, +thread: String, +v: String) -> List<&2, U32>:
  W.Http.response(status, "application/json", U.Utf8.encode(J.Json.show(J.Json.obj(Bool.pick(List<&2, J.KV>, ok,
    [J.KV{"attached", J.Str{v}}, J.KV{"thread", J.Str{thread}}],
    [J.KV{"text", J.Str{v}}, J.KV{"thread", J.Str{thread}}, J.KV{"upload", J.Flag{True{}}}])))), "")

def Upload.saved(r: Socket & Result<&1, &1, U32 & String, Unit>, +thread: String, +path: String) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{_}:
      Conn.finish(s, Upload.json("200 OK", True{}, thread, path))
    case Fail{_}:
      do IO<Unit>:
        rm : Result<&1, &1, U32 & String, Unit> <- H.FS.remove(path)
        Conn.finish(s, Upload.json("400 Bad Request", False{}, thread, "the upload did not arrive whole"))

# the head's leftover (the body's start) is written; the rest is copied
# (a body that came whole with the head has nothing left to copy: the copy
# would wait for bytes that never come)
def Upload.rest(none: Bool, s: Socket, +left: Nat, +thread: String, +path: String) -> IO(Unit):
  match none:
    case True{}:
      Conn.finish(s, Upload.json("200 OK", True{}, thread, path))
    case False{}:
      IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.save(s, path, U32.from_nat(left)), r => Upload.saved(r, thread, path))

def Upload.wrote.k(ok: Bool, s: Socket, +err: String, +left: Nat, +thread: String, +path: String) -> IO(Unit):
  match ok:
    case True{}:
      Upload.rest(Nat.is_eq(left, 0n), s, left, thread, path)
    case False{}:
      Conn.finish(s, Upload.json("500 Internal Server Error", False{}, thread, err))

def Upload.wrote(+err: String, s: Socket, +left: Nat, +thread: String, +path: String) -> IO(Unit):
  Upload.wrote.k(String.is_empty(err), s, err, left, thread, path)

def Upload.into(m: Maybe<&1, String>, s: Socket, +thread: String, +name: String, +len: Nat, rest: List<&2, U32>) -> IO(Unit):
  match m:
    case Some{+dir}:
      do IO<Unit>:
        mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
        +head : List<&2, U32> = List.take(&2, U32, rest, len)
        +path : String = dir ++ "/" ++ name
        e : String <- Media.put.file(path, "w", head)
        Upload.wrote(e, s, Nat.sub(len, List.length(&2, U32, head)), thread, path)
    case None{}:
      Conn.finish(s, W.Http.text("503 Service Unavailable", "the hub is gone"))

# a client that waits for leave to send the body (Expect: 100-continue) is
# told to go on
def Upload.go(cont: Bool, s: Socket) -> IO(Socket):
  match cont:
    case True{}:
      Conn.send(s, U.Utf8.encode("HTTP/1.1 100 Continue\r\n\r\n"))
    case False{}:
      IO.pure(Socket, s)

def Upload.gate(ok: Bool, big: Bool, s: Socket, +cont: Bool, +thread: String, +name: String, +len: Nat, +rest: List<&2, U32>, +inbox: Chan(Msg)) -> IO(Unit):
  match ok big:
    case False{} _:
      Conn.finish(s, W.Http.text("401 Unauthorized", "this Backplane needs its pairing token"))
    case True{} True{}:
      Conn.finish(s, Upload.json("413 Payload Too Large", False{}, thread, "attachments are limited to " ++ Nat.show(Nat.div(AT.Att.cap(), 1048576n)) ++ " MB"))
    case True{} False{}:
      do IO<Unit>:
        +reply : Chan(String) <- Chan.new(String, 1)
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, AttDir{thread, reply})
        m : Maybe<&1, String> <- Chan.recv(String, reply)
        s2 : Socket <- Upload.go(Bool.and(cont, List.is_empty(&2, U32, rest)), s)
        Upload.into(m, s2, thread, name, len, rest)

def Conn.route.att(att: Bool, s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match att req:
    case True{} W.Request{_, +p, +hs}:
      Upload.gate(Conn.allowed(trusted, hs, p, token), Nat.is_gt(Conn.bot.len(hs), AT.Att.cap()), s,
        String.contains(String.to_lower(W.Http.header_of(hs, "expect")), "100-continue"),
        AT.Att.unpct(W.Http.param(p, "thread")), AT.Att.stored(AT.Att.unpct(W.Http.param(p, "key")), AT.Att.unpct(W.Http.param(p, "name"))), Conn.bot.len(hs), rest, inbox)
    case False{} _:
      Conn.route.mcp(Conn.is_mcp(req), s, req, rest, web, id, inbox, trusted, token)

def Conn.route.bot(k: Nat, s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match k req:
    case 1n W.Request{_, +p, +hs}:
      Conn.bot.signed("hook", String.drop(W.Http.route(p), 6n), s, hs, rest, inbox)
    case 2n W.Request{_, _, +hs}:
      Conn.bot.signed("deliver", W.Http.header_of(hs, "x-backplane-peer"), s, hs, rest, inbox)
    case 3n W.Request{_, _, +hs}:
      Conn.bot.signed("link", W.Http.header_of(hs, "x-backplane-peer"), s, hs, rest, inbox)
    case 4n W.Request{_, _, +hs}:
      Conn.bot.signed("dir", W.Http.header_of(hs, "x-backplane-peer"), s, hs, rest, inbox)
    case 5n W.Request{_, +p, +hs}:
      Conn.bot.own(Conn.allowed(trusted, hs, p, token), s, BotReq{"shot", Conn.bot.shot_id(W.Http.route(p)), "", "", "", "", ""}, inbox)
    case 6n W.Request{+method, +p, +hs}:
      Conn.bot.own(AUTH.Auth.oauth(trusted, hs, method, p, token), s, BotReq{"oauth", p, "", "", "", "", ""}, inbox)
    case 7n W.Request{_, +p, +hs}:
      Conn.bot.own(Conn.allowed(trusted, hs, p, token), s, BotReq{"page", Conn.bot.page_id(W.Http.route(p)), "", "", "", "", ""}, inbox)
    case 8n W.Request{_, _, +hs}:
      Conn.bot.signed("far-push", W.Http.header_of(hs, "x-backplane-peer"), s, hs, rest, inbox)
    case 9n W.Request{_, _, +hs}:
      Conn.bot.signed("far-log", W.Http.header_of(hs, "x-backplane-peer"), s, hs, rest, inbox)
    case 10n W.Request{_, _, +hs}:
      Conn.bot.signed("far-rpc", W.Http.header_of(hs, "x-backplane-peer"), s, hs, rest, inbox)
    case 11n W.Request{_, +p, +hs}:
      Conn.bot.own(Conn.allowed(trusted, hs, p, token), s, BotReq{"pair", p, "", "", "", "", ""}, inbox)
    case 12n W.Request{_, +p, +hs}:
      Conn.bot.own(Conn.allowed(trusted, hs, p, token), s, BotReq{"view", p, "", "", "", "", ""}, inbox)
    case 13n W.Request{_, +p, +hs}:
      Conn.bot.own(Conn.allowed(trusted, hs, p, token), s, BotReq{"tshot", p, "", "", "", "", ""}, inbox)
    case _ _:
      Conn.route.att(Conn.is_att(req), s, req, rest, web, id, inbox, trusted, token)

def Conn.pair.reply(ok: Bool, s: Socket, +token: String, https: Bool) -> IO(Unit):
  match ok:
    case True{}:
      Conn.finish(s, W.Http.response("204 No Content", "text/plain", Nil{}, AUTH.Auth.pair.cookies(token, https)))
    case False{}:
      Conn.finish(s, W.Http.text("401 Unauthorized", "pairing refused"))

# Cache identity is public, domain-separated data; it is never a credential.
# The browser writes its nonauth identity cookie using its actual URL scheme.
def Conn.cache.reply(ok: Bool, s: Socket, token: String) -> IO(Unit):
  match ok:
    case True{}:
      Conn.finish(s, W.Http.response("200 OK", "application/json", U.Utf8.encode(J.Json.show(J.Json.obj([
        J.KV{"version", J.Json.nat(1n)}, J.KV{"cacheKey", J.Str{AUTH.Auth.cache_key(token)}}]))), "Cache-Control: no-store\r\n"))
    case False{}:
      Conn.finish(s, W.Http.response("401 Unauthorized", "text/plain", U.Utf8.encode("cache identity refused"), "Cache-Control: no-store\r\n"))

def Conn.capability.reply(ok: Bool, s: Socket) -> IO(Unit):
  match ok:
    case True{}: Conn.finish(s, W.Http.response("200 OK", "application/json", U.Utf8.encode(J.Json.show(TVW.Wire.capabilities())), "Cache-Control: no-store\r\n"))
    case False{}: Conn.finish(s, W.Http.response("401 Unauthorized", "text/plain", U.Utf8.encode("paired authentication required"), "Cache-Control: no-store\r\n"))

def Conn.route.capability(capability: Bool, s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match capability req:
    case True{} W.Request{method, _, hs}: Conn.capability.reply(AUTH.Auth.cache_request(method, hs, token), s)
    case False{} _: Conn.route.bot(Conn.bot.kind(req), s, req, rest, web, id, inbox, trusted, token)

def Conn.route.cache(cache: Bool, s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match cache req:
    case True{} W.Request{method, _, hs}:
      Conn.cache.reply(AUTH.Auth.cache_request(method, hs, token), s, token)
    case False{} W.Request{+method, +path, +hs}:
      Conn.route.capability(String.eq(W.Http.route(path), "/client/capabilities"), s, W.Request{method, path, hs}, rest, web, id, inbox, trusted, token)

def Conn.route.auth(pair: Bool, s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match pair req:
    case True{} W.Request{method, _, +hs}:
      Conn.pair.reply(Bool.and(String.eq(method, "POST"), AUTH.Auth.pair(hs, token)), s, token, String.starts_with(W.Http.header_of(hs, "origin"), "https://"))
    case False{} W.Request{+method, +path, +hs}:
      Conn.route.cache(String.eq(W.Http.route(path), "/auth/cache"), s, W.Request{method, path, hs}, rest, web, id, inbox, trusted, token)

def Desktop.input(m: Maybe<&1, DT.Input>, +client: U32, +out: Chan(String), +e: String, +l: String, +inbox: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{DT.Input{kind, text}}:
      do IO<Unit>:
        sent : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Desktop{client, kind, text, out, e, l})
        k(Unit{})
    case None{}: IO.pure(Unit, Unit{})

def Desktop.reader(fuel: Nat, +input: Chan(DT.Input), +client: U32, +out: Chan(String), +e: String, +l: String, +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+n:
      IO.bind(Maybe<&1, DT.Input>, Unit, Chan.recv(DT.Input, input), m => Desktop.input(m, client, out, e, l, inbox, _ => Desktop.reader(n, input, client, out, e, l, inbox)))

def Desktop.duped(r: Socket & Result<&1, &1, U32 & String, Socket>, rest: List<&2, U32>, +client: U32, +inbox: Chan(Msg), +e: String, +l: String) -> IO(Unit):
  (rd, result) = r
  match result:
    case Done{wr}:
      do IO<Unit>:
        +out : Chan(String) <- Chan.new(String, 2)
        +input : Chan(DT.Input) <- Chan.new(DT.Input, 32)
        IO.spawn(Unit, DT.Transport.writer(Fuel.forever(), wr, out, 1n))
        connected : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Desktop{client, "connected", "", out, e, l})
        IO.spawn(Unit, Desktop.reader(Fuel.forever(), input, client, out, e, l, inbox))
        DT.Transport.reader(Fuel.forever(), rd, rest, input, out)
    case Fail{_}: Socket.close(rd)

def Desktop.upgrade(ok: Bool, s: Socket, +hs: List<&2, W.Header>, rest: List<&2, U32>, +client: U32, +inbox: Chan(Msg)) -> IO(Unit):
  match ok:
    case False{}: Conn.finish(s, W.Http.text("401 Unauthorized", "desktop browser requires main-only bearer authentication"))
    case True{}:
      do IO<Unit>:
        +e : String <- Token.fresh()
        +l : String <- Token.fresh()
        s2 : Socket <- Conn.send(s, W.Ws.accept(W.Http.header_of(hs, "sec-websocket-key")))
        IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Unit, H.Sock.dup(s2), r => Desktop.duped(r, rest, client, inbox, e, l))

def Desktop.peer(r: Socket & String, req: W.Request, rest: List<&2, U32>, client: U32, inbox: Chan(Msg), token: String) -> IO(Unit):
  (s, peer) = r
  match req:
    case W.Request{method, path, +hs}:
      Desktop.upgrade(DB.Desktop.auth(peer, method, path, hs, token), s, hs, rest, client, inbox)

def Desktop.request_path(req: W.Request) -> String:
  match req: case W.Request{_, p, _}: p

def Desktop.route(desktop: Bool, s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match desktop:
    case True{}:
      IO.bind(Socket & String, Unit, H.Sock.peer(s), r => Desktop.peer(r, req, rest, id, inbox, token))
    case False{}:
      Conn.route.auth(String.eq(W.Http.route(Desktop.request_path(req)), "/auth/pair"), s, req, rest, web, id, inbox, trusted, token)

def Conn.route(s: Socket, +req: W.Request, rest: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match req:
    case W.Request{+method, +path, +hs}:
      Desktop.route(String.eq(W.Http.route(path), "/desktop/browser/v1"), s, W.Request{method, path, hs}, rest, web, id, inbox, trusted, token)

def Conn.head.k(m: Maybe<&2, W.Split>, s: Socket, buf: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg),
    +trusted: Bool, +token: String, k: Socket -> List<&2, U32> -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{W.Split{head, rest}}:
      Conn.route(s, W.Http.request(head), rest, web, id, inbox, trusted, token)
    case None{}:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536), r => Conn.head.got(r, buf, k))

# read until the request head is complete (at most 64 reads)
def Conn.head(fuel: Nat, s: Socket, +buf: List<&2, U32>, +web: String, +id: U32, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match fuel:
    case 0n:
      Conn.finish(s, W.Http.text("431 Request Header Fields Too Large", "too large"))
    case 1n+f:
      Conn.head.k(W.Http.split(buf), s, buf, web, id, inbox, trusted, token, s2 => b2 => Conn.head(f, s2, b2, web, id, inbox, trusted, token))

# Accepting
# ---------

# a tailnet connection from a device its owner also owns is trusted like
# loopback (Tailscale vouches for it); others still need the token
def Accept.whois(r: Socket & String, +owner: String, +id: U32, +web: String, +inbox: Chan(Msg), +token: String) -> IO(Unit):
  (s, ip) = r
  do IO<Unit>:
    w : P.Ran <- P.Proc.run("sh", PT.Port.timeout(["5", "tailscale", "whois", "--json", ip]), "/")
    Conn.head(64n, s, Nil{}, web, id, inbox, TN.Tailnet.trusts(owner, TN.Tailnet.login(Tailnet.out(w))), token)

def Accept.local(r: Socket & String, +id: U32, +web: String, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  (s, ip) = r
  Conn.head(64n, s, Nil{}, web, id, inbox, Bool.and(trusted, CF.Cef.loopback(ip)), token)

def Accept.trust(ask: Bool, s: Socket, +owner: String, +id: U32, +web: String, +inbox: Chan(Msg), +trusted: Bool, +token: String) -> IO(Unit):
  match ask:
    case True{}:
      IO.bind(Socket & String, Unit, H.Sock.peer(s), r => Accept.whois(r, owner, id, web, inbox, token))
    case False{}:
      IO.bind(Socket & String, Unit, H.Sock.peer(s), r => Accept.local(r, id, web, inbox, trusted, token))

def Accept.got(r: Listener & Result<&1, &1, U32 & String, Socket>, +id: U32, +web: String, +inbox: Chan(Msg), +trusted: Bool, +token: String, +owner: String, k: Listener -> IO(Unit)) -> IO(Unit):
  (l, res) = r
  match res:
    case Done{s}:
      do IO<Unit>:
        IO.spawn(Unit, Accept.trust(Bool.and(Bool.not(trusted), Bool.not(String.is_empty(owner))), s, owner, id, web, inbox, trusted, token))
        k(l)
    case Fail{_}:
      k(l)

# connections from one listener; ids count up from `id` (loopback from 1,
# the tailnet from 2^31, so they never meet)
def Accept.loop(fuel: Nat, l: Listener, +id: U32, +web: String, +inbox: Chan(Msg), +trusted: Bool, +token: String, +owner: String) -> IO(Unit):
  match fuel:
    case 0n:
      Listener.close(l)
    case 1n+f:
      IO.bind(Listener & Result<&1, &1, U32 & String, Socket>, Unit, TCP.accept(l),
        r => Accept.got(r, id, web, inbox, trusted, token, owner, l2 => Accept.loop(f, l2, (id + 1 : U32), web, inbox, trusted, token, owner)))

# the tailnet address answers when the main listener already covers it
# (--host 0.0.0.0, or the address itself): the extra bind then fails by design
def Tailnet.covered(+host: String, +ip: String) -> Bool:
  Bool.or(String.eq(host, "0.0.0.0"), String.eq(host, ip))

def Tailnet.listened(r: Result<&1, &1, U32 & String, Listener>, +host: String, +ip: String, +port: U32, +web: String, +inbox: Chan(Msg), +token: String) -> IO(Unit):
  match r:
    case Done{l}:
      do IO<Unit>:
        w : P.Ran <- P.Proc.run("tailscale", ["whois", "--json", ip], "/")
        +owner : String = TN.Tailnet.login(Tailnet.out(w))
        IO.print("Backplane on your tailnet: http://" ++ ip ++ ":" ++ U32.show(port) ++ "/ (" ++ Bool.pick(String, String.is_empty(owner), "pair in Settings", owner ++ "'s devices; pair in Settings") ++ ")")
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, LinkFound{TN.Tailnet.report("reachable", TN.Tailnet.link("http://" ++ ip ++ ":" ++ U32.show(port), token))})
        IO.spawn(Unit, Accept.loop(Fuel.forever(), l, 2147483648, web, inbox, False{}, token, owner))
    case Fail{_}:
      do IO<Unit>:
        Log.info("could not listen on the tailnet address " ++ ip)
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, LinkFound{TN.Tailnet.report(Bool.pick(String, Tailnet.covered(host, ip), "reachable", "unavailable"), "")})
        return Unit{}

def Tailnet.listen.k(none: Bool, +host: String, +ip: String, +port: U32, +web: String, +inbox: Chan(Msg), +token: String) -> IO(Unit):
  match none:
    case True{}:
      do IO<Unit>:
        Log.info("tailscale is not up (tailscale ip -4 gave nothing)")
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, LinkFound{TN.Tailnet.report("unavailable", "")})
        return Unit{}
    case False{}:
      IO.bind(Result<&1, &1, U32 & String, Listener>, Unit, H.Net.listen(ip, port), r => Tailnet.listened(r, host, ip, port, web, inbox, token))

# listen on the tailnet address too (clients there need the token), and
# serve HTTPS through `tailscale serve` when asked
def Tailnet.start(n: Net, +host: String, +port: U32, +web: String, +inbox: Chan(Msg)) -> IO(Unit):
  match n:
    case Net{True{}, https, +token, _, _, _, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}:
      do IO<Unit>:
        r : P.Ran <- P.Proc.run("tailscale", ["ip", "-4"], "")
        +ip : String = Tailnet.first(r)
        Tailnet.listen.k(String.is_empty(ip), host, ip, port, web, inbox, token)
        Tailnet.https(https, port, token, inbox)
    case Net{False{}, _, _, _, _, _, brx, pkx, tmx, mc, pvx, bnx, mgd, lk, fc, ksx}:
      IO.pure(Unit, Unit{})

# Google's tools for bots
# -----------------------
# gmail_search, gmail_read, calendar_events, google_accounts run at once, off the hub;
# gmail_send and calendar_create first open an approval ask (the reply
# waits in the park until the user answers).

def Gg.is(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"),
    Nat.is_lt(C.Codec.pos(GG.Google.tool_names(), J.Json.get_str(J.Json.get_obj(o, "params"), "name", "")), List.length(&2, String, GG.Google.tool_names())))

def Gg.tool(+o: J.Json) -> String:
  J.Json.get_str(J.Json.get_obj(o, "params"), "name", "")

def Gg.args(+o: J.Json) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments")

def Gg.say(+reply: Chan(String), +id: J.Json, +ok: Bool, text: String) -> IO(Unit):
  do IO<Unit>:
    sent : Result<&1, &1, String, Unit> <- Chan.send(String, reply, MC.Mcp.result(id, J.Json.obj([J.KV{"content", J.Json.arr([J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{text}}])])},
      J.KV{"isError", J.Flag{Bool.not(ok)}}])))
    return Unit{}

def Gg.said(+t: String, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  Gg.say(reply, id, Bool.not(String.starts_with(t, "error")), t)

def Gg.call(+home: String, +tool: String, +args: J.Json, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(String, Unit, GS.Google.call(home, tool, args), t => Gg.said(t, id, reply))

def Gg.wait.k(m: Maybe<&1, String>, +home: String, +tool: String, +args: J.Json, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  match m:
    case Some{+answer}:
      Bool.pick(IO(Unit), Bool.or(String.eq(answer, "accept"), String.eq(answer, "session")), Gg.call(home, tool, args, id, reply),
        Gg.say(reply, id, False{}, "The user declined this."))
    case None{}:
      IO.pure(Unit, Unit{})

# the user's answer (raw from the park), then the call or a refusal
def Gg.wait(+r2: Chan(String), +home: String, +tool: String, +args: J.Json, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, r2), m => Gg.wait.k(m, home, tool, args, id, reply))

def Gg.detail.k(send: Bool, +a: J.Json) -> String:
  match send:
    case True{}:
      "To: " ++ J.Json.get_str(a, "to", "") ++ Bool.pick(String, String.is_empty(J.Json.get_str(a, "cc", "")), "", "\nCc: " ++ J.Json.get_str(a, "cc", "")) ++
        "\nSubject: " ++ J.Json.get_str(a, "subject", "") ++ "\n\n" ++ String.take(J.Json.get_str(a, "body", ""), 1500n)
    case False{}:
      GG.Cal.create.detail(a)

# what the user approves: the mail's recipients, subject and text, or the event
def Gg.detail(+o: J.Json) -> String:
  Bool.pick(String, String.is_empty(J.Json.get_str(Gg.args(o), "account", "")), "", "Account: " ++ J.Json.get_str(Gg.args(o), "account", "") ++ "\n")
    ++ Gg.detail.k(String.eq(Gg.tool(o), "gmail_send"), Gg.args(o))

# Calendar update and delete: the event is fetched off the hub first, so
# the person sees what changes; CalAsk comes back to open the ask
def Gc2.json(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def Cp.ask.k(obj: Bool, ask: Bool, +inbox: Chan(Msg), +thread: String, +tool: String, +id: J.Json, +reply: Chan(String), +r: String) -> IO(Unit):
  match obj:
    case True{}:
      do IO<Unit>:
        +j : J.Json = Gc2.json(r)
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, CalAsk{thread, tool, J.Json.show(J.Json.get_obj(j, "args")), J.Json.get_str(j, "detail", ""), J.Json.show(id), reply, ask})
        return Unit{}
    case False{}:
      Gg.say(reply, id, False{}, r)

def Cp.ask(ask: Bool, +inbox: Chan(Msg), +thread: String, +tool: String, +id: J.Json, +reply: Chan(String), +r: String) -> IO(Unit):
  Cp.ask.k(String.starts_with(r, "{"), ask, inbox, thread, tool, id, reply, r)

def Cp.run(ask: Bool, +inbox: Chan(Msg), +home: String, +thread: String, +tool: String, +args: J.Json, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(String, Unit, GS.Google.prepare(home, String.eq(tool, "calendar_update"), args), r => Cp.ask(ask, inbox, thread, tool, id, reply, r))

def Hub.cal.ask(ask: Bool, h: Hub, +thread: String, +tool: String, +args: J.Json, +detail: String, +id: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match ask h:
    case False{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Gg.call(Cfg.home(cfg), tool, args, id, reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +ask : String = B.Hub.id("q", now, B.Hub.seq(st))
        +r2 : Chan(String) <- Chan.new(String, 1)
        Park.put(Cfg.pk(cfg), PK.PPark{ask, "", r2})
        IO.spawn(Unit, Gg.wait(r2, Cfg.home(cfg), tool, args, id, reply))
        Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{[M.AskOpened{ask, thread, "approval", tool, detail, "", now}], Nil{}})

def Hub.google.k(ask: Bool, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match ask h:
    case False{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Gg.call(Cfg.home(cfg), Gg.tool(o), Gg.args(o), J.Json.get_obj(o, "id"), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +ask : String = B.Hub.id("q", now, B.Hub.seq(st))
        +r2 : Chan(String) <- Chan.new(String, 1)
        Park.put(Cfg.pk(cfg), PK.PPark{ask, "", r2})
        IO.spawn(Unit, Gg.wait(r2, Cfg.home(cfg), Gg.tool(o), Gg.args(o), J.Json.get_obj(o, "id"), reply))
        Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{[M.AskOpened{ask, thread, "approval", Gg.tool(o), Gg.detail(o), "", now}], Nil{}})

def Hub.google.p(prep: Bool, ask: Bool, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match prep h:
    case True{} Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Cp.run(ask, inbox, Cfg.home(cfg), thread, Gg.tool(o), Gg.args(o), J.Json.get_obj(o, "id"), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} h2:
      Hub.google.k(ask, h2, thread, o, reply, now)

def Hub.google.ok(h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.google.p(Bool.or(String.eq(Gg.tool(o), "calendar_update"), String.eq(Gg.tool(o), "calendar_delete")),
        Bool.and(GG.Google.asks(Gg.tool(o)), Bool.not(Bool.or(MC.Perm.session_ok(M.Asks.all(st), thread, Gg.tool(o)), MC.Perm.run.google(st, thread, Gg.tool(o))))), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, o, reply, now)

# the hub's settings gmail.sig.<address> as a JSON object (not secrets)
def Gg.sigs.go(ks: List<&2, String>, +st: M.State, +acc: List<&2, J.KV>) -> List<&2, J.KV>:
  match ks:
    case Nil{}:
      acc
    case +k <> t:
      Gg.sigs.go(t, st, Bool.pick(List<&2, J.KV>, String.starts_with(k, "gmail.sig."), J.KV{k, J.Str{M.Setting.get(st, k, "")}} <> acc, acc))

def Gg.sigs(+st: M.State) -> J.Json:
  J.Json.obj(Gg.sigs.go(M.Setting.keys(st), st, Nil{}))

# the prepared folder goes when the hub never took the ask
def Gg.gmail.sent(r: Result<&1, &1, Msg, Unit>, +home: String, +plan: String) -> IO(Unit):
  match r:
    case Done{_}:
      IO.pure(Unit, Unit{})
    case Fail{_}:
      GS.Gm.drop(home, plan)

def Gg.gmail.k(p: GS.GPrep, +home: String, +inbox: Chan(Msg), +thread: String, +tool: String, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  match p:
    case GS.GFail{m}:
      Gg.say(reply, id, False{}, m)
    case GS.GDone{m}:
      Gg.said(m, id, reply)
    case GS.GAsk{d, +plan}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, GmailAsk{thread, tool, d, plan, J.Json.show(id), reply})
        Gg.gmail.sent(ok, home, plan)

# gmail_send / gmail_draft prepared off the hub (core/google.bend's
# Gmail.*, server/google.bend's Gm.run), then the ask is opened by the hub
def Gg.gmail(+home: String, +tool: String, +args: J.Json, +sigs: J.Json, +inbox: Chan(Msg), +thread: String, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(GS.GPrep, Unit, GS.Gm.run(home, tool, args, sigs), p => Gg.gmail.k(p, home, inbox, thread, tool, id, reply))

def Gg.gsend(+home: String, +plan: String, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(String, Unit, GS.Gm.send(home, plan), t => Gg.said(t, id, reply))

def Gg.gdrop(+home: String, +plan: String, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(Unit, Unit, GS.Gm.drop(home, plan), u => Gg.say(reply, id, False{}, "The user declined this."))

def Gg.gwait.k(m: Maybe<&1, String>, +home: String, +plan: String, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  match m:
    case Some{+answer}:
      Bool.pick(IO(Unit), Bool.or(String.eq(answer, "accept"), String.eq(answer, "session")), Gg.gsend(home, plan, id, reply), Gg.gdrop(home, plan, id, reply))
    case None{}:
      GS.Gm.drop(home, plan)

def Gg.gwait(+r2: Chan(String), +home: String, +plan: String, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, r2), m => Gg.gwait.k(m, home, plan, id, reply))

def Hub.gmail.k(ask: Bool, h: Hub, +thread: String, +tool: String, +detail: String, +plan: String, +id: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match ask h:
    case False{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Gg.gsend(Cfg.home(cfg), plan, id, reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +ask : String = B.Hub.id("q", now, B.Hub.seq(st))
        +r2 : Chan(String) <- Chan.new(String, 1)
        Park.put(Cfg.pk(cfg), PK.PPark{ask, "", r2})
        IO.spawn(Unit, Gg.gwait(r2, Cfg.home(cfg), plan, id, reply))
        Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{[M.AskOpened{ask, thread, "approval", tool, detail, "", now}], Nil{}})

# the prepared email arrived: asked about (unless the session allows the tool), then sent
def Hub.gmail.ask(h: Hub, +thread: String, +tool: String, +detail: String, +plan: String, +idtxt: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.gmail.k(Bool.and(GG.Google.asks(tool), Bool.not(Bool.or(MC.Perm.session_ok(M.Asks.all(st), thread, tool), MC.Perm.run.google(st, thread, tool)))), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, tool, detail, plan,
        Maybe.default(&2, J.Json, J.Json.parse(idtxt), J.Null{}), reply, now)

def Hub.gmail.start(h: Hub, +thread: String, +o: J.Json, +reply: Chan(String)) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Gg.gmail(Cfg.home(cfg), Gg.tool(o), Gg.args(o), Gg.sigs(st), inbox, thread, J.Json.get_obj(o, "id"), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.google.m(mail: Bool, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match mail h:
    case True{} _:
      Hub.gmail.start(h, thread, o, reply)
    case False{} _:
      Hub.google.ok(h, thread, o, reply, now)

# alerts Google would refuse go back to the agent before anyone is asked
def Hub.google.v(fine: Bool, +err: String, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match fine:
    case True{}:
      Hub.google.m(Bool.or(String.eq(Gg.tool(o), "gmail_send"), String.eq(Gg.tool(o), "gmail_draft")), h, thread, o, reply, now)
    case False{}:
      do IO<Hub>:
        Gg.say(reply, J.Json.get_obj(o, "id"), False{}, err)
        return h

def Hub.google(bot: Bool, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match bot:
    case True{}:
      +err = GG.Cal.check(Gg.args(o))
      Hub.google.v(String.is_empty(err), err, h, thread, o, reply, now)
    case False{}:
      do IO<Hub>:
        Gg.say(reply, J.Json.get_obj(o, "id"), False{}, "only bots have this tool")
        return h

# A bot's new project (project_create)
# ------------------------------------
# The hub asks the person first (BH.Bots.newproj.asks); once they accept,
# the folder is made when missing and resolved off the hub, and comes back
# as NewProject, where BH.Bots.newproj.made adds it.

# the folder from $1 (never empty), made when missing, printed resolved
def Np.sh() -> String:
  "exec 2>&1; set -e; [ -n \"$1\" ] || { echo \"name a folder\"; exit 2; }; " ++ FsJob.path.sh() ++
    "[ -e \"$d\" ] || [ -L \"$d\" ] || mkdir -p -- \"$d\"; " ++ FsJob.dir.sh() ++ "echo \"$d\""

def Np.ran(r: P.Ran, +inbox: Chan(Msg), +thread: String, +name: String, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  match r:
    case P.Ran{0, out}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, NewProject{thread, FsJob.last(String.lines(String.trim(out))), name, J.Json.show(id), reply})
        return Unit{}
    case P.Ran{_, out}:
      Gg.say(reply, id, False{}, "error: " ++ String.trim(out))

def Np.run(+inbox: Chan(Msg), +thread: String, +a: J.Json, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(P.Ran, Unit, FsJob.sh(Np.sh(), String.trim(J.Json.get_str(a, "path", ""))), r => Np.ran(r, inbox, thread, J.Json.get_str(a, "name", ""), id, reply))

def Np.wait.k(m: Maybe<&1, String>, +inbox: Chan(Msg), +thread: String, +a: J.Json, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  match m:
    case Some{+answer}:
      Bool.pick(IO(Unit), BH.Bots.newproj.yes(answer), Np.run(inbox, thread, a, id, reply),
        Gg.say(reply, id, False{}, "The person declined this."))
    case None{}:
      IO.pure(Unit, Unit{})

def Np.wait(+r2: Chan(String), +inbox: Chan(Msg), +thread: String, +a: J.Json, +id: J.Json, +reply: Chan(String)) -> IO(Unit):
  IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, r2), m => Np.wait.k(m, inbox, thread, a, id, reply))

def Hub.newproj.k(ask: Bool, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match ask h:
    case False{} Hub{st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Np.run(inbox, thread, Gg.args(o), J.Json.get_obj(o, "id"), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case True{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        +ask : String = B.Hub.id("q", now, B.Hub.seq(st))
        +r2 : Chan(String) <- Chan.new(String, 1)
        Park.put(Cfg.pk(cfg), PK.PPark{ask, "", r2})
        IO.spawn(Unit, Np.wait(r2, inbox, thread, Gg.args(o), J.Json.get_obj(o, "id"), reply))
        Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{[M.AskOpened{ask, thread, "approval", "project_create", BH.Bots.newproj.detail(Gg.args(o)), "", now}], Nil{}})

def Hub.newproj(bot: Bool, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match bot h:
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.newproj.k(BH.Bots.newproj.asks(M.Asks.all(st), thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, o, reply, now)
    case False{} _:
      do IO<Hub>:
        Gg.say(reply, J.Json.get_obj(o, "id"), False{}, "only bots have this tool")
        return h

# Task runs (the bot's task_* tools; docs/teach.md)
# -------------------------------------------------

def Tk.say(+reply: Chan(String), +rid: String, ok: Bool, text: String) -> IO(Unit):
  Gg.say(reply, Maybe.default(&2, J.Json, J.Json.parse(rid), J.Null{}), ok, text)

def Tk.rundir(+home: String, +bot: String, +run: String) -> String:
  home ++ "/bots/" ++ bot ++ "/" ++ TE.Teach.run.dir(run)

# the run's folder, its first Act, and the plan back to the bot
def Tk.start(+inbox: Chan(Msg), +dir: String, +thread: String, +run: String, +slug: String, +plan: String, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{[M.ActivityLogged{thread, "task:" ++ run, "task", "task", TE.Teach.act.text(slug, run, "running", 0n, "", Nil{}), now}]})
    Tk.say(reply, rid, True{}, plan)

def Hub.task.run.f(r: Result<&2, &2, String, TKS.TkTask>, h: Hub, +thread: String, +bot: String, +slug: String, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match r h:
    case Done{+filled} Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        +run : String = TE.Teach.run.id(slug, now)
        IO.spawn(Unit, Tk.start(inbox, Tk.rundir(Cfg.home(cfg), bot, run), thread, run, slug,
          J.Json.show(J.Json.obj([J.KV{"run", J.Str{run}}, J.KV{"plan", TE.Teach.plan(filled)}])), rid, reply, now))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case Fail{+e} _:
      do IO<Hub>:
        IO.spawn(Unit, Tk.say(reply, rid, False{}, e))
        return h

def Hub.task.run.p(m: Maybe<&2, TKS.TkTask>, h: Hub, +thread: String, +bot: String, +slug: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match m:
    case Some{+t}:
      Hub.task.run.f(TKS.Task.fill(t, TE.Teach.args.of(J.Json.get_obj(a, "params"))), h, thread, bot, slug, rid, reply, now)
    case None{}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.say(reply, rid, False{}, "that task is damaged: save it again with task_save"))
        return h

def Hub.task.run.k(m: Maybe<&2, String>, h: Hub, +thread: String, +bot: String, +slug: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match m:
    case Some{+j}:
      Hub.task.run.p(TKS.Task.read(j), h, thread, bot, slug, a, rid, reply, now)
    case None{}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.say(reply, rid, False{}, "no such task: task_list shows yours"))
        return h

def Hub.task.run(h: Hub, +thread: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.task.run.k(BO.World.task(M.State.bots(st), Bot.page(st, thread), TKS.Task.slug(J.Json.get_str(a, "slug", ""))),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, Bot.page(st, thread), TKS.Task.slug(J.Json.get_str(a, "slug", "")), a, rid, reply, now)

# task_step: a line in the run's log; with shot, the page's picture
def Tk.step.shot(want: Bool, m: Maybe<&2, Chan(BR.BMsg)>, +bot: String, +dir: String, +n: Nat) -> IO(Unit):
  match want m:
    case True{} Some{+q}:
      do IO<Unit>:
        r : String <- BR.Browser.ask_on(q, bot, "screenshot", J.Json.obj([J.KV{"path", J.Str{dir ++ "/" ++ TE.Teach.step.file(n)}}, J.KV{"fullPage", J.Flag{False{}}}]))
        return Unit{}
    case _ _:
      IO.pure(Unit, Unit{})

def Tk.step.job(m: Maybe<&2, Chan(BR.BMsg)>, +bot: String, +dir: String, +run: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Unit):
  do IO<Unit>:
    +n : Nat = J.Json.get_nat(a, "n", 0n)
    +shot : Bool = J.Json.get_bool(a, "shot", False{})
    Tk.append(dir ++ "/log.jsonl", J.Json.show(J.Json.obj([J.KV{"n", J.Json.nat(n)}, J.KV{"ok", J.Flag{J.Json.get_bool(a, "ok", True{})}}, J.KV{"note", J.Str{String.take(J.Json.get_str(a, "note", ""), 500n)}},
      J.KV{"shot", J.Str{Bool.pick(String, shot, TE.Teach.step.file(n), "")}}, J.KV{"at", J.Json.nat(now)}])))
    Tk.step.shot(shot, m, bot, dir, n)
    Tk.say(reply, rid, True{}, J.Json.show(J.Json.obj([J.KV{"ok", J.Flag{True{}}}, J.KV{"shot", J.Str{Bool.pick(String, shot, "/bots/" ++ bot ++ "/tasks/shot?run=" ++ run ++ "&f=" ++ TE.Teach.step.file(n), "")}}])))

def Hub.task.step.k(ok: Bool, h: Hub, +thread: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match ok h:
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.step.job(Cfg.br(cfg), Bot.page(st, thread), Tk.rundir(Cfg.home(cfg), Bot.page(st, thread), J.Json.get_str(a, "run", "")), J.Json.get_str(a, "run", ""), a, rid, reply, now))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      do IO<Hub>:
        IO.spawn(Unit, Tk.say(reply, rid, False{}, "no such run"))
        return h

def Hub.task.step(h: Hub, +thread: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  Hub.task.step.k(TE.Teach.run.ok(J.Json.get_str(a, "run", "")), h, thread, a, rid, reply, now)

def Tk.pause.shot(m: Maybe<&2, Chan(BR.BMsg)>, +bot: String, +path: String) -> IO(Unit):
  match m:
    case Some{q}:
      do IO<Unit>:
        r : String <- BR.Browser.ask_on(q, bot, "screenshot", J.Json.obj([J.KV{"path", J.Str{path}}, J.KV{"fullPage", J.Flag{False{}}}]))
        return Unit{}
    case None{}:
      IO.pure(Unit, Unit{})

# task_pause: the page's picture, then the ask (Hub.task.paused)
def Tk.pause.job(m: Maybe<&2, Chan(BR.BMsg)>, +inbox: Chan(Msg), +dir: String, +thread: String, +bot: String, +run: String, +n: Nat, +kind: String, +question: String, +risky: Bool, +rid: String, +reply: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(dir)
    Tk.pause.shot(m, bot, dir ++ "/" ++ TE.Teach.pause.file(n))
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, TaskPaused{thread, bot, run, TE.Teach.run.slug(run), n, kind, question, risky, rid, reply})
    return Unit{}

def Hub.task.pause.k(ok: Bool, h: Hub, +thread: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match ok h:
    case True{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.pause.job(Cfg.br(cfg), inbox, Tk.rundir(Cfg.home(cfg), Bot.page(st, thread), J.Json.get_str(a, "run", "")), thread, Bot.page(st, thread), J.Json.get_str(a, "run", ""),
          J.Json.get_nat(a, "n", now), J.Json.get_str(a, "kind", "stuck"), String.take(J.Json.get_str(a, "question", ""), 300n), J.Json.get_bool(a, "risky", False{}), rid, reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      do IO<Hub>:
        IO.spawn(Unit, Tk.say(reply, rid, False{}, "no such run"))
        return h

def Hub.task.pause(h: Hub, +thread: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  Hub.task.pause.k(TE.Teach.run.ok(J.Json.get_str(a, "run", "")), h, thread, a, rid, reply, now)

# the person's answer to a pause: the bot gets it, the browser is handed back
def Tk.wait.k(m: Maybe<&1, String>, +inbox: Chan(Msg), +bot: String, +rid: String, +reply: Chan(String)) -> IO(Unit):
  match m:
    case Some{+answer}:
      do IO<Unit>:
        Self.post(inbox, TeachBack{bot})
        Tk.say(reply, rid, True{}, TE.Pause.reply(answer))
    case None{}:
      IO.pure(Unit, Unit{})

def Tk.wait(+r2: Chan(String), +inbox: Chan(Msg), +bot: String, +rid: String, +reply: Chan(String)) -> IO(Unit):
  IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, r2), m => Tk.wait.k(m, inbox, bot, rid, reply))

# the ask's record is what alerts the person, once, under core/notice.bend's
# policy (Policy.ask: a needs-you alert, key ask-<thread>)
def Hub.task.paused(h: Hub, +thread: String, +bot: String, +run: String, +slug: String, +n: Nat, +kind: String, +question: String, +risky: Bool, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        +ask : String = B.Hub.id("q", now, B.Hub.seq(st))
        +r2 : Chan(String) <- Chan.new(String, 1)
        Park.put(Cfg.pk(cfg), PK.PPark{ask, "", r2})
        IO.spawn(Unit, Tk.wait(r2, inbox, bot, rid, reply))
        Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note},
          B.Step{[M.AskOpened{ask, thread, "browser", "task_pause", TE.Pause.detail(bot, run, slug, n, kind, question, risky), TE.Pause.options(risky), now}], Nil{}})

# task_end: the end line, the picture list, the oldest runs pruned, the final Act
def Tk.end.sh() -> String:
  "cd \"$2\" 2>/dev/null && { ls -1t | tail -n +21 | while IFS= read -r d; do rm -rf -- \"$d\"; done; }; cd \"$1\" 2>/dev/null || exit 0; if [ -f log.jsonl ]; then wc -l < log.jsonl | tr -d ' '; else echo 0; fi; ls -1 | grep -E '[.]jpg$'; true"

def Tk.end.url.put(ok: Bool, url: String, rest: List<&2, String>) -> List<&2, String>:
  match ok:
    case True{}:
      url <> rest
    case False{}:
      rest

def Tk.end.urls(ns: List<&2, String>, +bot: String, +run: String) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case +f <> t:
      Tk.end.url.put(TE.Teach.file.ok(f), "/bots/" ++ bot ++ "/tasks/shot?run=" ++ run ++ "&f=" ++ f, Tk.end.urls(t, bot, run))

def Tk.end.done(r: P.Ran, +inbox: Chan(Msg), +thread: String, +bot: String, +run: String, +status: String, +summary: String, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Unit):
  match r:
    case P.Ran{_, +out}:
      do IO<Unit>:
        +lines : List<&2, String> = String.lines(String.trim(out))
        +n : Nat = Maybe.default(&2, Nat, Nat.read(Maybe.default(&2, String, List.head(&2, String, lines), "0")), 0n)
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{[M.ActivityLogged{thread, "task:" ++ run ++ ".end", "task", "task",
          TE.Teach.act.text(TE.Teach.run.slug(run), run, status, n, summary, Tk.end.urls(List.drop(&2, String, lines, 1n), bot, run)), now}]})
        Tk.say(reply, rid, True{}, "run ended")

def Tk.end.job(+inbox: Chan(Msg), +home: String, +thread: String, +bot: String, +run: String, +status: String, +summary: String, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Unit):
  do IO<Unit>:
    Tk.append(Tk.rundir(home, bot, run) ++ "/log.jsonl", J.Json.show(J.Json.obj([J.KV{"end", J.Flag{True{}}}, J.KV{"status", J.Str{status}}, J.KV{"summary", J.Str{String.take(summary, 500n)}}, J.KV{"at", J.Json.nat(now)}])))
    r : P.Ran <- P.Proc.run("sh", ["-c", Tk.end.sh(), "sh", Tk.rundir(home, bot, run), home ++ "/bots/" ++ bot ++ "/tasks/" ++ TE.Teach.run.slug(run) ++ "/runs"], "/")
    Tk.end.done(r, inbox, thread, bot, run, status, summary, rid, reply, now)

def Hub.task.end.k(ok: Bool, h: Hub, +thread: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match ok h:
    case True{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Tk.end.job(inbox, Cfg.home(cfg), thread, Bot.page(st, thread), J.Json.get_str(a, "run", ""), J.Json.get_str(a, "status", "ok"), J.Json.get_str(a, "summary", ""), rid, reply, now))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      do IO<Hub>:
        IO.spawn(Unit, Tk.say(reply, rid, False{}, "no such run"))
        return h

def Hub.task.end(h: Hub, +thread: String, +a: J.Json, +rid: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  Hub.task.end.k(TE.Teach.run.ok(J.Json.get_str(a, "run", "")), h, thread, a, rid, reply, now)

def Hub.task.k(i: Nat, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match i:
    case 0n:
      Hub.task.run(h, thread, Gg.args(o), J.Json.show(J.Json.get_obj(o, "id")), reply, now)
    case 1n:
      Hub.task.step(h, thread, Gg.args(o), J.Json.show(J.Json.get_obj(o, "id")), reply, now)
    case 2n:
      Hub.task.pause(h, thread, Gg.args(o), J.Json.show(J.Json.get_obj(o, "id")), reply, now)
    case _:
      Hub.task.end(h, thread, Gg.args(o), J.Json.show(J.Json.get_obj(o, "id")), reply, now)

def Hub.task(bot: Bool, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match bot:
    case True{}:
      Hub.task.k(C.Codec.pos(TE.Teach.io_names(), Gg.tool(o)), h, thread, o, reply, now)
    case False{}:
      do IO<Hub>:
        Gg.say(reply, J.Json.get_obj(o, "id"), False{}, "only bots have this tool")
        return h

def Hub.mcp.tk(tk: Bool, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match tk h:
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.task(BO.World.is_bot_thread(M.State.bots(st), thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, o, reply, now)
    case False{} _:
      Hub.mcp.w(MC.Wait.secs(o), h, thread, o, body, reply, now)

def Hub.mcp.np(np: Bool, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match np h:
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.newproj(BO.World.is_bot_thread(M.State.bots(st), thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, o, reply, now)
    case False{} _:
      Hub.mcp.tk(TE.Teach.tool.is(o), h, thread, o, body, reply, now)

def Hub.mcp.gg(g: Bool, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match g h:
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.google(BO.World.is_bot_thread(M.State.bots(st), thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, o, reply, now)
    case False{} _:
      Hub.mcp.np(BH.Bots.newproj.is(o), h, thread, o, body, reply, now)

def KExportMcp.run(+rt: K.Rt, +home: String, +o: J.Json, +reply: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    +args : J.Json = J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments")
    +r : J.Json <- KEX.KExport.export(rt, home, J.Json.get_str(args, "path", ""), J.Json.get_bool(args, "force", False{}), "", True{})
    sent : Result<&1, &1, String, Unit> <- Chan.send(String, reply, MC.Mcp.result(Maybe.default(&2, J.Json, J.Json.get(o, "id"), J.Null{}), MC.Mcp.content(Bool.or(String.eq(J.Json.get_str(r, "export10", ""), "done"), String.eq(J.Json.get_str(r, "export10", ""), "confirm")), r)))
    return Unit{}

def Hub.mcp.kexport(exporting: Bool, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match exporting h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, KExportMcp.run(Cfg.rt(cfg), Cfg.home(cfg), o, reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      Hub.mcp.gg(Gg.is(o), h, thread, o, body, reply, now)

# Ordering: a buy thread's tools (core/order.bend, server/orderio.bend)
# ---------------------------------------------------------------------
# order_quote reads the order's manifest, asks the person (an approval
# card listing every file) and only then uploads, waits for the quote and
# posts it with its Buy now link; the record in <home>/orders keeps the token,
# and a watcher follows the order (Od.watch), resumed at start.

def Od.server(+st: M.State) -> String:
  B.Buy.server(st)

def Od.post(+inbox: Chan(Msg), +thread: String, +id: String, text: String) -> IO(Unit):
  do IO<Unit>:
    +now : Nat <- Now.nat()
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Commit{[M.MessagePosted{thread, id, M.Assistant{}, text, now}]})
    return Unit{}

def Od.wake(+inbox: Chan(Msg), +thread: String, +text: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, OrderWake{thread, text})
    return Unit{}

# an order's progress in its thread, with its note alert (Hub.order.note)
def Od.progress(+inbox: Chan(Msg), +thread: String, +id: String, +text: String, +note: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, OrderNote{thread, id, text, note})
    return Unit{}

def Od.status.k(+st: String, +last: String, +q: J.Json, +inbox: Chan(Msg), +thread: String, +qid: String) -> IO(Unit):
  Bool.pick(IO(Unit), Bool.or(String.eq(st, last), String.is_empty(OR.Order.status.text(st, J.Json.get_str(J.Json.get_obj(q, "order"), "tracking", "")))),
    IO.pure(Unit, Unit{}), Od.progress(inbox, thread, "o:" ++ qid ++ ":" ++ st, OR.Order.status.text(st, J.Json.get_str(J.Json.get_obj(q, "order"), "tracking", "")),
      OR.Order.status.note(st, J.Json.get_str(J.Json.get_obj(q, "order"), "tracking", ""))))

def Od.watch.next(+st: String, +last: String, again: String -> IO(Unit)) -> IO(Unit):
  Bool.pick(IO(Unit), OR.Order.done(st), IO.pure(Unit, Unit{}), again(Bool.pick(String, String.is_empty(st), last, st)))

# one look at a quote's order every minute while it is unpaid, every ten
# after; a change is said in the buy thread and kept in the record
def Od.watch(n: Nat, +inbox: Chan(Msg), +home: String, +server: String, +qid: String, +token: String, +thread: String, +last: String) -> IO(Unit):
  match n:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+m:
      do IO<Unit>:
        IO.sleep(OR.Order.every(last))
        +install : String <- ODI.Od.install(home)
        +q : J.Json <- ODI.Od.get(home, server, install, qid, token)
        +st : String = OR.Order.state(q)
        Od.status.k(st, last, q, inbox, thread, qid)
        ODI.Od.keep(home, qid, token, thread, server, Bool.pick(String, String.is_empty(st), last, st))
        Od.watch.next(st, last, l => Od.watch(m, inbox, home, server, qid, token, thread, l))

def Od.resume.one(+inbox: Chan(Msg), +home: String, +j: J.Json) -> IO(Unit):
  Bool.pick(IO(Unit), Bool.or(OR.Order.done(J.Json.get_str(j, "status", "")), String.is_empty(J.Json.get_str(j, "token", ""))), IO.pure(Unit, Unit{}),
    Od.watch(9000n, inbox, home, J.Json.get_str(j, "server", ""), J.Json.get_str(j, "id", ""), J.Json.get_str(j, "token", ""), J.Json.get_str(j, "thread", ""), J.Json.get_str(j, "status", "")))

def Od.resume.all(ps: List<&2, String>, +inbox: Chan(Msg), +home: String) -> IO(Unit):
  match ps:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +p <> t:
      do IO<Unit>:
        text : String <- GS.Gf.text(ODI.Od.orders(home) ++ "/" ++ p)
        IO.spawn(Unit, Od.resume.one(inbox, home, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})))
        Od.resume.all(t, inbox, home)

# at start: follow every order not yet delivered
def Od.resume(+inbox: Chan(Msg), +home: String) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", "ls -1 2>/dev/null | grep '^q_.*\\.json$' | head -n 200"], ODI.Od.orders(home))
    Od.resume.all(String.lines(ODI.Od.ran(r)), inbox, home)

def Od.quoted.k(ok: Bool, +q: J.Json, +inbox: Chan(Msg), +home: String, +server: String, +qid: String, +token: String, +thread: String) -> IO(Unit):
  match ok:
    case True{}:
      do IO<Unit>:
        Od.post(inbox, thread, "q:" ++ qid, OR.Order.quote.md(J.Json.get_str(J.Json.get_obj(q, "quote"), "markdown", ""), ODI.Od.pay(server, qid, token),
          J.Json.get_str(J.Json.get_obj(q, "quote"), "pay_label", "Buy now"), OR.Order.payable(q)))
        ODI.Od.keep(home, qid, token, thread, server, "quoted")
        Od.wake(inbox, thread, "The quote is posted in this thread. " ++ OR.Order.said(q))
        Od.watch(9000n, inbox, home, server, qid, token, thread, "quoted")
    case False{}:
      Od.wake(inbox, thread, "The quote could not be worked out: " ++ J.Json.get_str(q, "error", "no answer from the order server") ++ ".")

def Od.quoted(+q: J.Json, +inbox: Chan(Msg), +home: String, +server: String, +qid: String, +token: String, +thread: String) -> IO(Unit):
  Od.quoted.k(Bool.not(String.is_empty(J.Json.get_str(q, "status", ""))), q, inbox, home, server, qid, token, thread)

def Od.sent(+up: J.Json, +inbox: Chan(Msg), +home: String, +server: String, +install: String, +thread: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(J.Json.get_str(up, "token", "")),
    Od.wake(inbox, thread, "The files were not taken: " ++ J.Json.get_str(up, "error", "no answer") ++ "."),
    IO.bind(J.Json, Unit, ODI.Od.poll(60n, home, server, install, J.Json.get_str(up, "id", ""), J.Json.get_str(up, "token", "")),
      q => Od.quoted(q, inbox, home, server, J.Json.get_str(up, "id", ""), J.Json.get_str(up, "token", ""), thread)))

# the person said yes: upload, wait, post, and tell the agent
def Od.go(+inbox: Chan(Msg), +home: String, +server: String, +dir: String, ps: List<&2, String>, +thread: String) -> IO(Unit):
  do IO<Unit>:
    +install : String <- ODI.Od.install(home)
    +up : J.Json <- ODI.Od.upload(home, server, install, dir, ps)
    ODI.Od.keep(home, J.Json.get_str(up, "id", "none"), J.Json.get_str(up, "token", ""), thread, server, "quoting")
    Od.sent(up, inbox, home, server, install, thread)

# Send (or allowed for the session): upload; Not now: the agent is told;
# the empty answer (the run was retired: Stop, archive, delete) tells no one
def Od.wait.a(i: Nat, +inbox: Chan(Msg), +home: String, +server: String, +dir: String, ps: List<&2, String>, +thread: String) -> IO(Unit):
  match i:
    case 0n:
      Od.go(inbox, home, server, dir, ps, thread)
    case 1n:
      Od.go(inbox, home, server, dir, ps, thread)
    case 2n:
      Od.wake(inbox, thread, OR.Order.quote.declined())
    case _:
      IO.pure(Unit, Unit{})

def Od.wait.k(m: Maybe<&1, String>, +inbox: Chan(Msg), +home: String, +server: String, +dir: String, ps: List<&2, String>, +thread: String) -> IO(Unit):
  match m:
    case Some{+answer}:
      Od.wait.a(C.Codec.pos(["accept", "session", "decline"], answer), inbox, home, server, dir, ps, thread)
    case None{}:
      IO.pure(Unit, Unit{})

def Od.wait(+r2: Chan(String), +inbox: Chan(Msg), +home: String, +server: String, +dir: String, ps: List<&2, String>, +thread: String) -> IO(Unit):
  IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, r2), m => Od.wait.k(m, inbox, home, server, dir, ps, thread))

def Od.ask(ok: Bool, h: Hub, +thread: String, +dir: String, +abs: String, +m: J.Json, +ps: List<&2, String>, +rid: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match ok h:
    case False{} _:
      do IO<Hub>:
        Gg.say(reply, rid, False{}, "no manifest.json with files to send in " ++ dir ++ " (paths must stay inside the folder, plain characters only, and the folder must hold plain files, no links)")
        return h
    case True{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        +ask : String = B.Hub.id("q", now, B.Hub.seq(st))
        +r2 : Chan(String) <- Chan.new(String, 1)
        Park.put(Cfg.pk(cfg), PK.PPark{ask, "", r2})
        IO.spawn(Unit, Od.wait(r2, inbox, Cfg.home(cfg), Od.server(st), abs, ps, thread))
        # (the agent hears at once and ends its turn; the person's Send may
        # come any time, Od.wait)
        Gg.say(reply, rid, True{}, OR.Order.quote.asked())
        Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{[M.AskOpened{ask, thread, "approval", "order_quote", OR.Order.ask.detail(dir, m, ps), "", now}], Nil{}})

# (a folder holding links is refused: a link named like a design file could
# carry any file of the person's out, under a name they would approve)
def Od.quote(h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        +dir : String = String.trim(J.Json.get_str(Gg.args(o), "dir", ""))
        +abs : String = B.Hub.cwd(st, B.Hub.thread(st, thread)) ++ "/" ++ dir
        text : String <- GS.Gf.text(abs ++ "/manifest.json")
        links : P.Ran <- P.Proc.run("sh", ["-c", "find . -type l 2>/dev/null | head -n 1"], abs)
        +m : J.Json = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
        +ps : List<&2, String> = ODI.Od.files(m)
        Od.ask(Bool.and(Bool.and(Bool.and(ODI.Od.safe(dir), ODI.Od.all.safe(ps)), Bool.not(List.is_empty(&2, String, ps))), String.is_empty(String.trim(ODI.Od.ran(links)))),
          Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, dir, abs, m, ps, J.Json.get_obj(o, "id"), reply, now)

def Od.status.said(none: Bool, +j: J.Json, +home: String, +install: String, +rid: J.Json, +reply: Chan(String)) -> IO(Unit):
  match none:
    case True{}:
      Gg.say(reply, rid, True{}, "no quote yet in this thread")
    case False{}:
      IO.bind(J.Json, Unit, ODI.Od.get(home, J.Json.get_str(j, "server", ""), install, J.Json.get_str(j, "id", ""), J.Json.get_str(j, "token", "")),
        q => Gg.say(reply, rid, True{}, OR.Order.said(q)))

# order_status: the newest record of this thread's, asked again
def Od.status.ask(+home: String, +thread: String, +rid: J.Json, +reply: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", "grep -l '\"thread\":\"" ++ thread ++ "\"' q_*.json 2>/dev/null | xargs ls -t 2>/dev/null | head -n 1"], ODI.Od.orders(home))
    text : String <- GS.Gf.text(ODI.Od.orders(home) ++ "/" ++ String.trim(ODI.Od.ran(r)))
    +j : J.Json = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
    +install : String <- ODI.Od.install(home)
    Od.status.said(String.is_empty(J.Json.get_str(j, "token", "")), j, home, install, rid, reply)

# order_ask: a question card with the choices as buttons (the person may
# type instead); the answer goes back to the agent as it is
def Od.choices(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{+x} <> t:
      x <> Od.choices(t)
    case _ <> t:
      Od.choices(t)

# (it parks nothing: the answer is posted in the thread as the person's
# message, hub.bend's Hub.answer.order, however long they take)
def Od.question(h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        +ask : String = B.Hub.id("q", now, B.Hub.seq(st))
        Gg.say(reply, J.Json.get_obj(o, "id"), True{}, OR.Order.ask.asked())
        Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{[M.AskOpened{ask, thread, "input", "order_ask",
          J.Json.get_str(Gg.args(o), "question", ""), String.join(Od.choices(J.Json.elems(J.Json.get_obj(Gg.args(o), "options"))), "\n"), now}], Nil{}})

def Od.gate.said(+said: J.Json, +inbox: Chan(Msg), +thread: String, +rid: J.Json, +reply: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, OrderGate{thread, OR.Order.gate(said), OR.Order.gate.md(J.Json.get_str(said, "summary", ""), J.Json.elems(J.Json.get_obj(said, "issues")))})
    Gg.say(reply, rid, True{}, OR.Order.gate.said(OR.Order.gate(said)))

def Od.gate.go(+cwd: String, +inbox: Chan(Msg), +thread: String, +rid: J.Json, +reply: Chan(String), +now: Nat) -> IO(Unit):
  IO.bind(J.Json, Unit, ODI.Od.review(cwd, J.Json.obj([J.KV{"action", J.Str{"check"}}]), now), said => Od.gate.said(said, inbox, thread, rid, reply))

# order_review: checked or recorded in the thread's folder, the answer as JSON
def Od.review.go(+cwd: String, +args: J.Json, +rid: J.Json, +reply: Chan(String), +now: Nat) -> IO(Unit):
  IO.bind(J.Json, Unit, ODI.Od.review(cwd, args, now), r => Gg.say(reply, rid, True{}, J.Json.show(r)))

def Od.review.or.gate(gate: Bool, +cwd: String, +inbox: Chan(Msg), +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Unit):
  match gate:
    case True{}:
      Od.gate.go(cwd, inbox, thread, J.Json.get_obj(o, "id"), reply, now)
    case False{}:
      Od.review.go(cwd, Gg.args(o), J.Json.get_obj(o, "id"), reply, now)

def Hub.order.t(i: Nat, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match i h:
    case 0n h2:
      Od.quote(h2, thread, o, reply, now)
    case 2n h2:
      Od.question(h2, thread, o, reply, now)
    case 3n Hub{+st, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Od.review.or.gate(String.eq(J.Json.get_str(Gg.args(o), "action", ""), "gate"), B.Hub.cwd(st, B.Hub.thread(st, thread)), inbox, thread, o, reply, now))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case _ Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Od.status.ask(Cfg.home(cfg), thread, J.Json.get_obj(o, "id"), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.order.k(buy: Bool, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match buy h:
    case False{} _:
      do IO<Hub>:
        Gg.say(reply, J.Json.get_obj(o, "id"), False{}, "only a buy thread has this tool")
        return h
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.order.t(C.Codec.pos(OR.Order.names(), Gg.tool(o)), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, o, reply, now)

# an order's progress posted in the thread, and with a note the person's
# devices are told (a "note" alert, notice.bend's Alert.set: push, desks,
# phones) unless they turned off alerts for settled work (notify.settled),
# keyed by the progress so each says it once
def Hub.order.ping(on: Bool, +st: M.State, +now: Nat, +thread: String, +id: String, +ping: String) -> List<&2, M.Change>:
  match on:
    case True{}:
      [NO.Alert.set(st, now, thread, "note", id, NO.Alert.memory(st, thread), NO.Alert.text(ping, 300n))]
    case False{}:
      []

# the question for a project's own checks (review.bend's Rvc.trusted), one
# open at a time for the same checks.json (B.Checks.asked)
def Hub.checks.ask.k(asked: Bool, h: Hub, +thread: String, +hash: String, +text: String, +now: Nat) -> IO(Hub):
  match asked:
    case True{}:
      IO.pure(Hub, h)
    case False{}:
      Hub.step(h, B.Step{[M.AskOpened{"checks:" ++ hash ++ ":" ++ Nat.show(now), thread, "input", "checks_ok",
        "Run this project's own checks? .backplane/checks.json runs these commands with sh, in the project's folder:\n\n" ++ text ++ "\nAllow them only if you trust where this project came from. Then press Refresh on Checks.",
        B.Checks.yes() ++ "\nNot now", now}], Nil{}})

def Hub.checks.ask(h: Hub, +thread: String, +hash: String, +text: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.checks.ask.k(Bool.or(String.is_empty(thread), B.Checks.asked(st, hash)), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, hash, text, now)

def Hub.order.note(h: Hub, +thread: String, +id: String, +text: String, +ping: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{M.MessagePosted{thread, id, M.Assistant{}, text, now} <>
        Hub.order.ping(Bool.and(Bool.not(String.is_empty(ping)), NO.Policy.on(st, "settled")), st, now, thread, id, ping), Nil{}})

def Hub.order.wake(h: Hub, +thread: String, +text: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Hub.post(st, now, thread, B.Hub.id("m", now, B.Hub.seq(st)), M.System{}, "[Backplane] " ++ text))

# the review gate in the buy thread: nothing when passed; else the
# question (an order_ask: the person's answer comes as their message), after
# the issues when there are some
def Hub.order.gate(kind: Nat, h: Hub, +thread: String, +md: String, +now: Nat) -> IO(Hub):
  match kind h:
    case 0n _:
      IO.pure(Hub, h)
    case 1n Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{[M.AskOpened{B.Hub.id("q", now, B.Hub.seq(st)), thread, "input", "order_ask",
        OR.Order.gate.review.q(), "Review for manufacturing\nOrder without a review", now}], Nil{}})
    case _ Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Step{[M.MessagePosted{thread, "gate:" ++ B.Hub.id("q", now, B.Hub.seq(st)), M.Assistant{}, md, now},
        M.AskOpened{B.Hub.id("q", now, B.Hub.seq(st)), thread, "input", "order_ask", OR.Order.gate.fix.q(), "Fix them first\nOrder anyway", now}], Nil{}})

# (with ordering off nothing is sent or recorded, even by an agent that
# remembers the tools from before it was turned off)
def Hub.order.on(on: Bool, buy: Bool, h: Hub, +thread: String, +o: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match on:
    case True{}:
      Hub.order.k(buy, h, thread, o, reply, now)
    case False{}:
      do IO<Hub>:
        Gg.say(reply, J.Json.get_obj(o, "id"), False{}, "Ordering is off; the person can turn it on in Settings")
        return h

def Hub.mcp.od(od: Bool, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match od h:
    case True{} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.order.on(OR.Order.enabled(M.Setting.get(st, "order.enabled", "off")), Bool.or(BH.Bots.buy.is(st, thread), String.eq(Gg.tool(o), "order_review")), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, o, reply, now)
    case False{} _:
      Hub.mcp.kexport(Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "kicad_export10")), h, thread, o, body, reply, now)

def Hub.mcp.route(pv: Bool, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match pv:
    case True{}:
      Hub.preview(h, thread, o, reply)
    case False{}:
      Hub.mcp.od(OR.Order.is(o), h, thread, o, body, reply, now)

def Hub.mcp.ki(ki: Bool, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match ki:
    case True{}:
      Hub.kicad_ipc(h, thread, o, reply)
    case False{}:
      Hub.mcp.route(Preview.is(o), h, thread, o, body, reply, now)

def Hub.mcp.tr(tr: Bool, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match tr:
    case True{}:
      Hub.term_read(h, thread, o, reply)
    case False{}:
      Hub.mcp.ki(KIpc.is(o), h, thread, o, body, reply, now)

# thread_read on a thread on another of the owner's machines: read from
# this hub's mirror of it (xref.bend)
def Hub.mcp.x(x: Bool, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match x h:
    case True{} Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        sent : Result<&1, &1, String, Unit> <- Chan.send(String, reply, XR.Xref.read(o, FR.Fh.ins(BN.Bn.far(Cfg.bn(cfg)))))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      Hub.mcp.tr(TermRead.is(o), h, thread, o, body, reply, now)

# (only to the owner's own machines with sync on)
def Hub.mcp.far(own: Bool, h: Hub, +link: String, +rq: String, +o: J.Json, body: String, +reply: Chan(String)) -> IO(Hub):
  match own h:
    case True{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Far.mcp.job(Maybe.default(&2, J.Json, J.Json.get(o, "id"), J.Null{}), link, rq, Cfg.home(cfg), Bot.peer.url(st, link), reply))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      Hub.mcp.k(MC.Mcp.reply(Maybe.default(&2, J.Json, J.Json.get(o, "id"), J.Null{}), MC.Res.fail("sync with that machine is off here")), h, reply, body)

# a tool call for a machine of the owner's goes there (fmcp.bend); machine_list
# is answered here; any other runs as before
def Hub.mcp.f(p: FM.Plan, h: Hub, +thread: String, +o: J.Json, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match p h:
    case FM.PHere{} _:
      Hub.mcp.x(XR.Xref.read.is(o), h, thread, o, body, reply, now)
    case FM.PAnswer{r} _:
      Hub.mcp.k(MC.Mcp.reply(Maybe.default(&2, J.Json, J.Json.get(o, "id"), J.Null{}), r), h, reply, body)
    case FM.PFar{+link, +rq} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      Hub.mcp.far(Far.own.id(st, cfg, link), Hub{st, f, ls, cl, as, inbox, cfg, note}, link, rq, o, body, reply)

def Hub.mcp.p(+o: J.Json, h: Hub, +thread: String, body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.mcp.f(FM.Fm.plan.o(o, st, FR.Fh.ins(BN.Bn.far(Cfg.bn(cfg))), thread), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, o, body, reply, now)

def Hub.mcp(h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  Hub.mcp.p(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), h, thread, body, reply, now)

def Msg.kind(m: Msg) -> String:
  match m:
    case LockProbe{_, _, _}:
      "lock_probe"
    case LockSaw{_, _, _}:
      "lock_saw"
    case LockHttp{_, _, _}:
      "lock_http"
    case AgentWrite{_, _, _}:
      "agent_write"
    case UpdateCheck{}:
      "update_check"
    case UpdateReady{_}:
      "update_ready"
    case DeployLocks{_}:
      "deploy_locks"
    case DeployFailed{_}:
      "deploy_failed"
    case ShipAsked{_, _, _, _}:
      "ship_asked"
    case ShipNewest{_, _}:
      "ship_newest"
    case ShipPid{_, _}:
      "ship_pid"
    case ShipDone{_, _}:
      "ship_done"
    case Mcp{_, _, _}:
      "mcp"
    case McpOpen{_, _, _}:
      "mcp_open"
    case McpShut{_, _}:
      "mcp_shut"
    case Announce{_}:
      "announce"
    case Join{_, _, _, _, _, _, _}:
      "join"
    case MobileJoin{_, _, _, _, _, _, _, _}:
      "mobile_join"
    case MobileTimeout{_, _}:
      "mobile_timeout"
    case JoinHere{_, _, _}:
      "join_here"
    case Drained{_, _, _, _}:
      "drained"
    case JoinMore{_, _}:
      "join_more"
    case Ping{_, _}:
      "ping"
    case Leave{_}:
      "leave"
    case ThinRead{_, _, _, _, _}:
      "thin-read"
    case Rpc{_, _}:
      "rpc"
    case AgentLine{_, _}:
      "agent_line"
    case AgentGone{_, _, _, _}:
      "agent_gone"
    case TelemetryDone{_, _, _}:
      "telemetry_done"
    case TelemetryPermit{_, _, _}:
      "telemetry_permit"
    case Tick{}:
      "tick"
    case Heart{}:
      "heart"
    case ThinFlush{}:
      "thin_flush"
    case AutoView{_, _}:
      "auto_view"
    case ClaudePid{_, _, _}:
      "claude_pid"
    case ClaudeFail{_, _, _}:
      "claude_fail"
    case Push{_, _}:
      "push"
    case PushBin{_, _}:
      "push_bin"
    case TermOut{_, _}:
      "term_out"
    case TermTo{_, _, _, _, _}:
      "term_to"
    case KProbe{_}: "kicad_probe"
    case KFound{_, _}: "kicad_health"
    case KiServers{_}: "kicad_servers"
    case ModelsFound{_, _}:
      "models"
    case OcFound{_}:
      "models_opencode"
    case ChecksAsk{_, _, _}:
      "checks_ask"
    case MachinesFound{_}:
      "machines"
    case LinkFound{_}:
      "link"
    case TurnPid{_, _, _, _}:
      "turn_pid"
    case TurnDone{_}:
      "turn_done"
    case Recover{}:
      "recover"
    case Reexec{_, _}:
      "reexec"
    case MailRouted{_}: "mail-route"
    case RunReady{_, _, _}: "run-ready"
    case DigestGot{_, _, _}: "digest"
    case Commit{_}:
      "commit"
    case OrderNote{_, _, _, _}:
      "order_note"
    case OrderWake{_, _}:
      "order_wake"
    case OrderGate{_, _, _}:
      "order_gate"
    case BoatThreads{_}:
      "boat_threads"
    case ThinWarm{_, _}:
      "thin_warm"
    case UberDo{_}:
      "uber_do"
    case UberAnswer{_, _, _}:
      "uber_answer"
    case BotTick{_}:
      "bot_tick"
    case BotsFound{_, _, _}:
      "bots_found"
    case BotsRefresh{_}:
      "bots_refresh"
    case BotHttp{_, _, _, _, _, _, _, _, _, _, _}:
      "bot_http"
    case BotVerified{_, _, _, _, _, _}:
      "bot_verified"
    case ShotTo{_, _}:
      "shot"
    case Shot{_}:
      "shot"
    case TeachReq{_, _, _, _}:
      "teach_req"
    case BrowserEv{_}:
      "browser_ev"
    case Desktop{_, _, _, _, _, _}: "desktop_browser"
    case CefDone{_, _, _}:
      "cef_done"
    case Near{_}:
      "cef_near"
    case TeachBack{_}:
      "teach_back"
    case TaskPaused{_, _, _, _, _, _, _, _, _, _}:
      "task_paused"
    case NewProject{_, _, _, _, _}:
      "new_project"
    case CalAsk{_, _, _, _, _, _, _}:
      "cal_ask"
    case GmailAsk{_, _, _, _, _, _}:
      "gmail_ask"
    case AttDir{_, _}:
      "att_dir"
    case FarFeed{_, _}:
      "far_feed"
    case FarPushed{_, _, _}:
      "far_pushed"
    case FarLiveAck{_, _, _}:
      "far_live_ack"
    case FarResend{_}:
      "far_resend"
    case FarPulled{_, _, _, _}:
      "far_pulled"
    case FarReplied{_, _, _, _, _}:
      "far_replied"
    case FarLoaded{_, _, _}:
      "far_loaded"
    case FarHere{_}:
      "far_here"
    case FarRetry{_}:
      "far_retry"
    case FarDo{_, _}:
      "far_do"
    case Replied{_, _, _}:
      "replied"
    case LaunchSent{_}:
      "launch_sent"
    case Release{}:
      "release"
    case SpawnFailed{_, _, _}:
      "spawn_failed"
    case AgentExited{_, _, _}:
      "agent_exited"
    case Silence{}:
      "silence"
    case KeyringSeen{_}:
      "keyring_seen"
    case Beat{}:
      "beat"
    case WatchLaunched{_, _}:
      "watch_launched"
    case WatchIdent{_, _, _}:
      "watch_ident"
    case WatchEnded{_, _, _, _}:
      "watch_ended"
    case WakeSeen{_, _, _}:
      "wake_seen"
    case Bulk{_}:
      "bulk"

def Hub.spawn_failed.k(ok: Bool, h: Hub, +thread: String, +line: String, +now: Nat) -> IO(Hub):
  match ok h:
    case True{} Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.line(Hub{st, f, ls, cl, Agents.drop(as, thread), inbox, cfg, note}, thread, line, now)
    case False{} _:
      IO.pure(Hub, h)

# a turn process that could not start: heard from the current generation
# only (law stale_spawn_fail_quiet)
def Hub.spawn_failed(h: Hub, +thread: String, +gen: String, +line: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.spawn_failed.k(B.Gen.accepts(Agent.key.of(Agents.get(as, thread)), gen), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, line, now)

def Hub.mcp.stale(+body: String) -> String:
  MC.McpOut.reply(MC.Mcp.reply(Maybe.default(&2, J.Json, J.Json.get(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "id"), J.Null{}),
    MC.Res.fail("stale process: this agent process is no longer the thread's, so it may read but not change anything")))

def Hub.mcp.charter.allowed(ok: Bool, +s: M.State, +r: ML.Run, h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat, +o: J.Json) -> IO(Hub):
  match ok:
    case False{}: Hub.mcp.k(MC.Mcp.reply(J.Json.get_obj(o, "id"), MC.Res{False{}, J.Str{"Outside the charter; escalated"}, B.Run.escalate(s, now, ML.Run.bot(r), ML.Run.conv(r), "bot", "Outside the charter: " ++ J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""))}), h, reply, body)
    case True{}:
      IO.bind(Hub, Hub, Hub.step(h, MC.Mcp.run.decide(s, now, r, J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "charter: " ++ CH.Charter.line(ML.Box.charter(M.State.mail(s), ML.Run.bot(r)), J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments")))), h2 => Hub.mcp(h2, thread, body, reply, now))

# Check direct MCP tools too: Gmail, deploy and work_run must not bypass
# the run's policy simply because they execute in the hub rather than a
# provider's shell. permission_prompt itself goes through Perm.run.
def Hub.mcp.charter.risky(risky: Bool, +s: M.State, +r: ML.Run, h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat, +o: J.Json) -> IO(Hub):
  match risky:
    case False{}: Hub.mcp(h, thread, body, reply, now)
    case True{}: Hub.mcp.charter.allowed(CH.Charter.allows(ML.Box.charter(M.State.mail(s), ML.Run.bot(r)), J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments")), s, r, h, thread, body, reply, now, o)

def Hub.mcp.charter.got(r: Maybe<&2, ML.Run>, +s: M.State, h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match r:
    case None{}: Hub.mcp(h, thread, body, reply, now)
    case Some{+r}:
      +o = Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{})
      Hub.mcp.charter.risky(Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), CH.Charter.risky(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"))), s, r, h, thread, body, reply, now, o)

def Hub.mcp.charter(h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}: Hub.mcp.charter.got(RUN.Run.of(st, thread), st, Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, body, reply, now)

# a standing rule that refuses the call (rules.bend's Rules.gate) answers
# it here, at once, before any tool runs
def Hub.mcp.rules.k(none: Bool, +msg: String, h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match none:
    case True{}:
      Hub.mcp.charter(h, thread, body, reply, now)
    case False{}:
      do IO<Hub>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, msg)
        return h

def Hub.mcp.rules.m(+msg: String, h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  Hub.mcp.rules.k(String.is_empty(msg), msg, h, thread, body, reply, now)

def Hub.mcp.rules(h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.mcp.rules.m(RLS.Rules.gate(st, thread, body), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, body, reply, now)

def Hub.mcp.gate(refused: Bool, h: Hub, +thread: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match refused:
    case True{}:
      do IO<Hub>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, Hub.mcp.stale(body))
        return h
    case False{}:
      Hub.mcp.rules(h, thread, body, reply, now)

# an MCP call names its thread and the process's generation
# ("<thread>.<gen>", B.Gen.*): one from a process the thread no longer
# runs may read, never change anything (law mcp_stale_gen_refused)
def Hub.mcp.open(h: Hub, +thread: String, +call: String, +body: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      IO.pure(Hub, Hub{st, f, ls, cl, as, inbox, Tools.with(cfg, DP.Open.mcp(Tools.of(cfg), thread, call, body)), note})

def Hub.mcp.shut(h: Hub, +thread: String, +call: String) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      IO.pure(Hub, Hub{st, f, ls, cl, as, inbox, Tools.with(cfg, DP.Open.mcp.shut(Tools.of(cfg), thread, call)), note})

def Hub.mcp.g(h: Hub, +tag: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.mcp.gate(B.Gen.refused(B.Gen.tag(tag), Agents.gen(as, B.Gen.thread(tag)), Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{})),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Gen.thread(tag), body, reply, now)

def Hub.exited.k(cur: Bool, h: Hub, +thread: String, +code: U32) -> IO(Hub):
  match cur:
    case True{}:
      do IO<Hub>:
        Log.info("agent for " ++ thread ++ " exited with code " ++ U32.show(code) ++ "; its reader ends at most 2 s later")
        return h
    case False{}:
      IO.pure(Hub, h)

# an agent's watcher saw its process exit: the reader ends (its last line
# read) and the exit is handled there, once; a stale generation's report
# changes nothing (B.Exit.current)
def Hub.exited(h: Hub, +thread: String, +gen: String, +code: U32) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.exited.k(B.Exit.current(gen, Agents.gen(as, thread)), Hub{st, f, ls, cl, as, inbox, cfg, note}, thread, code)

# Silence: a running Claude turn with no line for 10 minutes and no work
# out is noted once (B.Quiet.due); the note comes again only after a line
# and a new silence
type Quieted is Data:
  Quieted{as: List<&2, Agent>, cs: List<&2, M.Change>}

def Quiet.mark.r(rs: List<&2, ARun>) -> List<&2, ARun>:
  match rs:
    case ARun{c, hd, l, n, g, _, pa, pl} <> t:
      ARun{c, hd, l, n, g, True{}, pa, pl} <> t
    case Nil{}:
      Nil{}

def Quiet.mark(a: Agent) -> Agent:
  match a:
    case Agent{t, i, p, k, rs}:
      Agent{t, i, p, k, Quiet.mark.r(rs)}

def Quiet.one.k(due: Bool, a: Agent, +t: String, +st: M.State, +now: Nat, q: Quieted) -> Quieted:
  match due q:
    case True{} Quieted{as, +cs}:
      Quieted{Quiet.mark(a) <> as, B.Quiet.act(t, now, Nat.add(M.State.seq(st), List.length(&2, M.Change, cs))) <> cs}
    case False{} Quieted{as, cs}:
      Quieted{a <> as, cs}

def Quiet.heard(rs: List<&2, ARun>) -> Nat:
  match rs:
    case ARun{_, _, _, n, _, _, _, _} <> _:
      n
    case Nil{}:
      0n

def Quiet.said(rs: List<&2, ARun>) -> Bool:
  match rs:
    case ARun{_, _, _, _, _, q, _, _} <> _:
      q
    case Nil{}:
      True{}

def Quiet.one(a: Agent, +st: M.State, +now: Nat, q: Quieted) -> Quieted:
  match a:
    case Agent{+t, i, p, k, +rs}:
      Quiet.one.k(B.Quiet.due(B.Hub.running(B.Hub.thread(st, t)), Quiet.heard(rs), now, Quiet.said(rs), M.Work.busy(st, t)), Agent{t, i, p, k, rs}, t, st, now, q)

def Quiet.all(as: List<&2, Agent>, +st: M.State, +now: Nat, q: Quieted) -> Quieted:
  match as:
    case Nil{}:
      q
    case a <> t:
      Quiet.all(t, st, now, Quiet.one(a, st, now, q))

def Hub.quiet.k(q: Quieted, h: Hub) -> IO(Hub):
  match q h:
    case Quieted{as, cs} Hub{st, f, ls, cl, _, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, List.reverse(&2, Agent, as), inbox, cfg, note}, B.Step{cs, Nil{}})

def Hub.quiet.n(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.quiet.k(Quiet.all(as, st, now, Quieted{Nil{}, Nil{}}), Hub{st, f, ls, cl, Nil{}, inbox, cfg, note})

# Stuck turns (hub.bend's Stuck.*): each minute, a Claude thread whose turn
# runs though its ledger has nothing at work, silent Stuck.after() seconds
# with no work out, has its turn ended and its ledger cleared
type StuckOne is Data:
  StuckOne{thread: String, code: Nat}

def Stuck.of(rs: List<&2, ARun>, +run: Bool, +busy: Nat, +now: Nat, +wait: Nat) -> Nat:
  match rs:
    case ARun{c, _, _, n, _, _, _, _} <> _:
      B.Stuck.code.w(run, c, n, now, busy, wait)
    case Nil{}:
      0n

def Stuck.add(+code: Nat, +t: String, +xs: List<&2, StuckOne>) -> List<&2, StuckOne>:
  Bool.pick(List<&2, StuckOne>, Nat.is_eq(code, 0n), xs, StuckOne{t, code} <> xs)

def Stuck.list(as: List<&2, Agent>, +st: M.State, +now: Nat, +wait: Nat) -> List<&2, StuckOne>:
  match as:
    case Nil{}:
      Nil{}
    case Agent{+t, _, _, _, rs} <> rest:
      Stuck.add(Stuck.of(rs, B.Hub.running(B.Hub.thread(st, t)), M.Work.busy(st, t), now, wait), t, Stuck.list(rest, st, now, wait))

def ARun.cleared(r: ARun) -> ARun:
  match r:
    case ARun{c, hd, l, n, g, q, pa, pl}:
      ARun{CM.Cmds.cleared(c), hd, l, n, g, q, pa, pl}

def Stuck.clear(h: Hub, +t: String) -> Hub:
  match h:
    case Hub{st, f, ls, cl, +as, inbox, cfg, note}:
      Hub{st, f, ls, cl, Agents.with(as, t, ARun.cleared(Maybe.default(&2, ARun, Agents.run(as, t), ARun.new()))), inbox, cfg, note}

def Hub.stuck.one(x: StuckOne, h: Hub, +now: Nat) -> IO(Hub):
  match x h:
    case StuckOne{+t, +code} Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        Log.info("the turn of " ++ t ++ " ran on with its agent's ledger idle: ended")
        Hub.step(Stuck.clear(Hub{st, f, ls, cl, as, inbox, cfg, note}, t), B.Stuck.step(st, now, t, code))

def Hub.stuck.go(xs: List<&2, StuckOne>, h: Hub, +now: Nat) -> IO(Hub):
  match xs:
    case Nil{}:
      IO.pure(Hub, h)
    case x <> t:
      IO.bind(Hub, Hub, Hub.stuck.one(x, h, now), h2 => Hub.stuck.go(t, h2, now))

def Hub.stuck.w(+wait: String, h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, +as, inbox, cfg, note}:
      Hub.stuck.go(Stuck.list(as, st, now, Bool.pick(Nat, String.is_empty(wait), B.Stuck.after(), AD.Adopt.num(wait))), Hub{st, f, ls, cl, as, inbox, cfg, note}, now)

def Hub.stuck(h: Hub, +now: Nat) -> IO(Hub):
  IO.bind(String, Hub, Env.get("BACKPLANE_STUCK_S", ""), w => Hub.stuck.w(w, h, now))

def Hub.quiet(h: Hub, +now: Nat) -> IO(Hub):
  IO.bind(Hub, Hub, Hub.quiet.n(h, now), h2 => Hub.stuck(h2, now))

def Hub.quiet.beat.k(due: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match due h:
    case True{} Hub{+st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Hub.island(Cfg.home(cfg), st, inbox))
        Hub.quiet(Hub{st, f, ls, cl, as, inbox, cfg, note}, now)
    case False{} _:
      Hub.quiet(h, now)

# the minute's tick; once an hour while any thread works, the phones'
# island is pushed again, so it is never left to go stale (push.bend's
# Island.fresh)
def Hub.quiet.beat(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.quiet.beat.k(PU.Push.island.beat(st, now), Hub{st, f, ls, cl, as, inbox, cfg, note}, now)

# Beat: a thread's held progress lines land once B.Prog.gap() seconds have
# passed since its last write (B.Prog.beat), so the newest always shows
type Beated is Data:
  Beated{as: List<&2, Agent>, cs: List<&2, M.Change>}

def Beat.run.k(+p: B.PStep, +st: M.State, a: Agent, +t: String, +r: ARun, +now: Nat, q: Beated) -> Beated:
  match a q:
    case Agent{t0, i, pid, k, _} Beated{as, cs}:
      Beated{Agent{t0, i, pid, k, [ARun.prog.set(r, B.PStep.at(p), B.PStep.pend(p))]} <> as, List.append(&2, M.Change, B.Queue.changes(B.Prog.flush(st, t, now, B.PStep.flush(p))), cs)}

def Beat.none(a: Agent, q: Beated) -> Beated:
  match q:
    case Beated{as, cs}:
      Beated{a <> as, cs}

def Beat.one.k(m: Maybe<&2, ARun>, a: Agent, +st: M.State, +t: String, +now: Nat, q: Beated) -> Beated:
  match m:
    case Some{+r}:
      Beat.run.k(B.Prog.beat(ARun.prog_at(r), ARun.prog_last(r), now), st, a, t, r, now, q)
    case None{}:
      Beat.none(a, q)

def Beat.head(+rs: List<&2, ARun>) -> Maybe<&2, ARun>:
  List.head(&2, ARun, rs)

def Beat.one(a: Agent, +st: M.State, +now: Nat, q: Beated) -> Beated:
  match a:
    case Agent{+t, i, p, k, +rs}:
      Beat.one.k(Beat.head(rs), Agent{t, i, p, k, rs}, st, t, now, q)

def Beat.all(as: List<&2, Agent>, +st: M.State, +now: Nat, q: Beated) -> Beated:
  match as:
    case Nil{}:
      q
    case a <> t:
      Beat.all(t, st, now, Beat.one(a, st, now, q))

def Hub.beat.k(q: Beated, h: Hub) -> IO(Hub):
  match q h:
    case Beated{as, cs} Hub{st, f, ls, cl, _, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, List.reverse(&2, Agent, as), inbox, cfg, note}, B.Step{cs, Nil{}})

def Hub.beat(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.beat.k(Beat.all(as, st, now, Beated{Nil{}, Nil{}}), Hub{st, f, ls, cl, Nil{}, inbox, cfg, note})

def Hub.wake.seen(p: M.State & Hub, +kind: String, +target: String, +result: String, +now: Nat) -> IO(Hub):
  (+st, h) = p
  Hub.step(h, Bool.pick(B.Step, String.eq(kind, "pr"), B.Wake.pr(st, now, target, WK.Wake.pr.parse(result)), B.Wake.file(st, now, target, result)))

def Hub.watched.k(i: Nat, +st: M.State, +thread: String, +id: String, +a: String, +b: String, +now: Nat) -> B.Step:
  match i:
    case 0n:
      B.Watch.launched(st, now, thread, id)
    case 1n:
      B.Watch.identified(st, now, thread, id, a)
    case _:
      B.Watch.end(st, now, thread, id, a, b)

# what a watcher reported, folded in (core's Watch.*)
def Hub.watched(p: M.State & Hub, +i: Nat, +thread: String, +id: String, +a: String, +b: String, +now: Nat) -> IO(Hub):
  (+st, h) = p
  Hub.step(h, Hub.watched.k(i, st, thread, id, a, b, now))

def Hub.release(p: M.State & Hub, +now: Nat) -> IO(Hub):
  (+st, h) = p
  do IO<Hub>:
    PS.Perf.set("hub.busy", U32.from_nat(M.Work.threads(st)))
    Hub.step(h, B.Rec.release(st, now))

def Hub.replied(p: M.State & Hub, +thread: String, +to: String, +text: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, BH.Reply.send(st, now, thread, to, text))

def Hub.update_ready.k(p: M.State & Hub, force: Bool, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Update.ready(st, force, now))

def Hub.update_ready(h: Hub, force: Bool, +now: Nat) -> IO(Hub):
  Hub.update_ready.k(Hub.state(h), force, now)

def Hub.deploy_locks.k(p: M.State & Hub, free: Bool, open: List<&2, String>, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Deploy.check(st, now, free, open))

def Hub.deploy_locks.t(p: List<&2, String> & Hub, free: Bool, +now: Nat) -> IO(Hub):
  (tl, h) = p
  Hub.deploy_locks.k(Hub.state(h), free, tl, now)

def Hub.deploy_locks(h: Hub, free: Bool, +now: Nat) -> IO(Hub):
  Hub.deploy_locks.t(Hub.tools(h), free, now)

# (a waiting deploy is looked at with every heartbeat, not only the minute's
# tick: a tool call that ends lets it go within 15 s)
def Hub.deploy_poll.k(p: M.State & Hub, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Deploy.tick(st, now))

def Hub.deploy_poll(h: Hub, +now: Nat) -> IO(Hub):
  Hub.deploy_poll.k(Hub.state(h), now)

def Hub.deploy_failed.k(p: M.State & Hub, +err: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Deploy.failed(st, now, err))

def Hub.deploy_failed(h: Hub, err: String, +now: Nat) -> IO(Hub):
  Hub.deploy_failed.k(Hub.state(h), err, now)

# deploy_main (hub.bend's Ship.*): the call answered with origin/main read
def Hub.ship.asked(h: Hub, +thread: String, +body: String, +reply: Chan(String), +out: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.mcp.k(MC.Mcp.reply(Maybe.default(&2, J.Json, J.Json.get(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "id"), J.Null{}),
          MC.Mcp.ship(st, now, thread, J.Json.get_obj(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "params"), "arguments"), out)),
        Hub{st, f, ls, cl, as, inbox, cfg, note}, reply, body)

def Hub.ship.newest.k(p: M.State & Hub, +why: String, +out: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Ship.newest(st, now, why, out))

def Hub.ship.newest(h: Hub, +why: String, +out: String, +now: Nat) -> IO(Hub):
  Hub.ship.newest.k(Hub.state(h), why, out, now)

def Hub.ship.pid.k(p: M.State & Hub, +commit: String, +out: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Ship.pid(st, now, commit, out))

def Hub.ship.pid(h: Hub, +commit: String, +out: String, +now: Nat) -> IO(Hub):
  Hub.ship.pid.k(Hub.state(h), commit, out, now)

def Hub.ship.done.k(p: M.State & Hub, +commit: String, +out: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Ship.done(st, now, commit, out))

def Hub.ship.done(h: Hub, +commit: String, +out: String, +now: Nat) -> IO(Hub):
  Hub.ship.done.k(Hub.state(h), commit, out, now)

def Hub.launch.sent(p: M.State & Hub, +key: String, +now: Nat) -> IO(Hub):
  (st, h) = p
  Hub.step(h, B.Lr.far.sent(st, now, key))

def Hub.lock.probe.after.k(pending: Bool, h: Hub, +name: String, +holder: String) -> IO(Hub):
  match pending h:
    case True{} Hub{s, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, LockJob.later(inbox, name, holder, 0n))
        return Hub{s, f, ls, cl, as, inbox, cfg, note}
    case False{} _:
      IO.pure(Hub, h)

def Hub.lock.probe.after(h: Hub, +name: String, +holder: String) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.lock.probe.after.k(Bool.and(String.eq(LCK.Lock.owner(M.Locks.get(st, name)), holder), Bool.not(LCK.Lock.ready(M.Locks.get(st, name)))), Hub{st, f, ls, cl, as, inbox, cfg, note}, name, holder)

# who holds it outside, heard from LockJob.who: recorded against the lock
# as it is now
def Hub.lock.saw(h: Hub, +name: String, +holder: String, +out: String, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Locks.outside(st, now, name, holder,
        J.Json.get_nat(Maybe.default(&2, J.Json, J.Json.parse(out), J.Null{}), "pid", 0n), J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(out), J.Null{}), "cmd", "")))

def Hub.lock.probe.taken(taken: Bool, h: Hub, +st: M.State, +name: String, +holder: String, +n: Nat, +now: Nat) -> IO(Hub):
  match taken h:
    case True{} _:
      IO.bind(Hub, Hub, Hub.step(h, B.Locks.ready(st, now, name, holder)), h2 => Hub.lock.probe.after(h2, name, holder))
    case False{} Hub{s, f, ls, cl, as, +inbox, cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, LockJob.later(inbox, name, holder, 1n+n))
        LockJob.who.if(Nat.is_eq(Nat.mod(n, LockJob.every()), 0n), inbox, name, holder)
        return Hub{s, f, ls, cl, as, inbox, cfg, note}

def Hub.lock.probe.take(h: Hub, +st: M.State, +name: String, +holder: String, +n: Nat, +now: Nat) -> IO(Hub):
  match h:
    case Hub{s, f, ls, cl, as, inbox, +cfg, note}:
      IO.bind(Bool, Hub, LG.Guard.take(name, holder, Cfg.home(cfg), LG.Guard.script()), taken => Hub.lock.probe.taken(taken, Hub{s, f, ls, cl, as, inbox, cfg, note}, st, name, holder, n, now))

def Hub.lock.probe.k(own: Bool, h: Hub, +st: M.State, +name: String, +holder: String, +n: Nat, +now: Nat) -> IO(Hub):
  match own:
    case False{}:
      IO.pure(Hub, h)
    case True{}:
      Hub.lock.probe.take(h, st, name, holder, n, now)

def Hub.lock.probe(h: Hub, +name: String, +holder: String, +n: Nat, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      Hub.lock.probe.k(Bool.and(Bool.not(String.is_empty(holder)), String.eq(LCK.Lock.owner(M.Locks.get(st, name)), holder)), Hub{st, f, ls, cl, as, inbox, cfg, note}, st, name, holder, n, now)

def Hub.lock.http.done(r: Hub & Bool, +name: String, +holder: String, requeued: Bool, +reply: Chan(String)) -> IO(Hub):
  (h, ok) = r
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}:
      do IO<Hub>:
        sent : Result<&1, &1, String, Unit> <- Chan.send(String, reply, J.Json.show(Bool.pick(J.Json, ok, B.Locks.answer.re(M.Locks.get(st, name), holder, requeued), J.Json.obj([J.KV{"error", J.Str{"log write failed"}}]))))
        return Hub{st, f, ls, cl, as, inbox, cfg, note}

def Hub.lock.http.valid(valid: Bool, h: Hub, +op: String, +a: J.Json, +reply: Chan(String), +now: Nat) -> IO(Hub):
  match valid h:
    case False{} _:
      do IO<Hub>:
        sent : Result<&1, &1, String, Unit> <- Chan.send(String, reply, "{\"error\":\"invalid lock request\"}")
        return h
    case True{} Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        +name : String = J.Json.get_str(a, "name", "")
        +holder : String = J.Json.get_str(a, "holder", "")
        +lost : Bool = B.Locks.http.lost(op, M.Locks.get(st, name), holder)
        +ready : Bool <- LockTool.preflight(Bool.and(Bool.or(String.eq(op, "acquire"), lost), Bool.or(String.is_empty(LCK.Lock.owner(M.Locks.get(st, name))), String.eq(LCK.Lock.owner(M.Locks.get(st, name)), holder))), name, holder, Cfg.home(cfg))
        r : Hub & Bool <- Hub.step.ok(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Locks.http.step(op, st, now, name, holder, J.Json.get_str(a, "note", ""), ready))
        h2 : Hub <- Hub.lock.http.done(r, name, holder, lost, reply)
        LockTool.cleanup(h2, name, holder)

def Hub.lock.http(h: Hub, +op: String, +body: String, +reply: Chan(String), +now: Nat) -> IO(Hub):
  +a = Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{})
  Hub.lock.http.valid(Bool.and(LCK.Lock.name.valid(J.Json.get_str(a, "name", "")), Bool.and(String.starts_with(J.Json.get_str(a, "holder", ""), "pid:"), M.Launch.has(["acquire", "renew", "release", "cancel", "status"], op))), h, op, a, reply, now)

def Hub.mail.routed(h: Hub, body: MB.Body, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}: Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Mail.route(st, now, body))

def Hub.digest.got(h: Hub, +bot: String, +ids: List<&2, String>, +bs: List<&2, MB.Body>, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}: Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Digest.got(st, now, bot, ids, bs))

def Hub.run.ready(h: Hub, +th: String, head: String, bs: List<&2, MB.Body>, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, cfg, note}: Hub.step(Hub{st, f, ls, cl, as, inbox, cfg, note}, B.Run.start.on(B.Run.ready(st, th), st, now, th, head, bs))

# Two inboxes
# ------------
# The hub reads one channel (pri): clients' requests, joins and writers'
# reports, MCP calls and ticks come straight in. Everything else (agents'
# lines, jobs, the hub's own posts) goes to the hub's inbox, and Bulk.fwd
# passes it on only while it holds one of Bulk.room() tokens, one given
# back as each is handled: however fast an agent floods, a request (Stop)
# waits behind at most that many of its lines.

def Bulk.room() -> Nat:
  16n

def Bulk.fill(n: Nat, +tok: Chan(Unit)) -> IO(Unit):
  match n:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+k:
      do IO<Unit>:
        ok : Result<&1, &1, Unit, Unit> <- Chan.send(Unit, tok, Unit{})
        Bulk.fill(k, tok)

def Bulk.moved(m: Maybe<&1, Msg>, +pri: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{msg}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, pri, Bulk{msg})
        k(Unit{})
    case None{}:
      IO.pure(Unit, Unit{})

def Bulk.took(t: Maybe<&1, Unit>, +inbox: Chan(Msg), +pri: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match t:
    case Some{_}:
      IO.bind(Maybe<&1, Msg>, Unit, Chan.recv(Msg, inbox), m => Bulk.moved(m, pri, k))
    case None{}:
      IO.pure(Unit, Unit{})

def Bulk.fwd(fuel: Nat, +inbox: Chan(Msg), +pri: Chan(Msg), +tok: Chan(Unit)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, Unit>, Unit, Chan.recv(Unit, tok), t => Bulk.took(t, inbox, pri, u => Bulk.fwd(f, inbox, pri, tok)))

def Bulk.back(+tok: Chan(Unit), h: Hub, k: Hub -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, Unit, Unit> <- Chan.send(Unit, tok, Unit{})
    k(h)

# The web port was taken at start (often only for a moment, by the process
# this one replaces): try it again every few seconds, and once it is ours,
# start what Main.serve starts beside the hub, so a restart never leaves the
# hub unreachable from other machines and phones
def Listen.up(l: Listener, +web: String, +inbox: Chan(Msg), +cfg: Cfg, +addr: String) -> IO(Unit):
  do IO<Unit>:
    Log.info("web server on: " ++ addr)
    IO.spawn(Unit, Accept.loop(Fuel.forever(), l, 1, web, inbox, True{}, Net.token(Cfg.net(cfg)), ""))
    Tailnet.start(Cfg.net(cfg), Cfg.host(cfg), Cfg.port(cfg), web, inbox)
    Machines.start(inbox, Cfg.port(cfg))

def Listen.got(r: Result<&1, &1, U32 & String, Listener>, +web: String, +inbox: Chan(Msg), +cfg: Cfg, +addr: String, k: Unit -> IO(Unit)) -> IO(Unit):
  match r:
    case Done{l}:
      Listen.up(l, web, inbox, cfg, addr)
    case Fail{_}:
      k(Unit{})

def Listen.retry(fuel: Nat, +web: String, +inbox: Chan(Msg), +cfg: Cfg, +addr: String) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      do IO<Unit>:
        IO.sleep(2000)
        l : Result<&1, &1, U32 & String, Listener> <- H.Net.listen(Cfg.host(cfg), Cfg.port(cfg))
        Listen.got(l, web, inbox, cfg, addr, _ => Listen.retry(f, web, inbox, cfg, addr))

# "<pid> <port>" of another live Backplane holding the lock (the same
# program as this one, by its process name), else ""




# Browser frames
# --------------



# a frame goes to every client (or, past the helper's --jpeg-fps, only to
# the clients showing its page: castio.bend); a recorder step, navigation
# or load to the hub's teach state



# page-frame events from the browser, to the hub


# the headless KiCad servers' lines, to the hub

def Main.torn.k(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Result<&1, &1, U32 & String, File>):
  (f, res) = r
  match res:
    case Done{_}:
      IO.pure(Result<&1, &1, U32 & String, File>, Done{f})
    case Fail{e}:
      IO.pure(Result<&1, &1, U32 & String, File>, Fail{e})

# a log whose last line was torn by a crash ends without a newline: the
# next line starts on its own (ST.Store.tail_fix)
def Main.torn.w(none: Bool, f: File, +fix: String) -> IO(Result<&1, &1, U32 & String, File>):
  match none:
    case True{}:
      IO.pure(Result<&1, &1, U32 & String, File>, Done{f})
    case False{}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Result<&1, &1, U32 & String, File>, File.write(f, fix), Main.torn.k)

def Main.torn(r: Result<&1, &1, U32 & String, File>, +fix: String) -> IO(Result<&1, &1, U32 & String, File>):
  match r:
    case Done{f}:
      Main.torn.w(String.is_empty(fix), f, fix)
    case Fail{e}:
      IO.pure(Result<&1, &1, U32 & String, File>, Fail{e})

# Only paused credential-dependent work triggers a bounded, read-only probe.
def Keyring.resume.job(+inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    state : Nat <- Keyring.probe()
    Self.post(inbox, KeyringSeen{state})

def Hub.auth.beat.k(wait: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match wait h:
    case True{} Hub{st, f, ls, cl, as, +inbox, +cfg, note}:
      do IO<Hub>:
        IO.spawn(Unit, Keyring.resume.job(inbox))
        Hub.beat(Hub{st, f, ls, cl, as, inbox, Cfg.with_rt(cfg, K.Rt.set(Cfg.rt(cfg), "auth.probe", J.Flag{True{}})), note}, now)
    case False{} _:
      Hub.beat(h, now)

# Whether any thread waits for its provider's sign-in (B.Auth.any: a
# setting looked up for every thread) is asked at every beat, but only a
# commit can change it: the answer is kept in Bn with the state's sequence
# and asked again only when the log moved (law auth_beat_kept). On the JS
# hub the walk was most of an idle hub's CPU.
def Auth.kept.key(+st: M.State) -> String:
  Nat.show(1n+M.State.seq(st)) ++ " "

def Auth.kept.k(hit: Bool, +kept: String, +st: M.State) -> Bool:
  match hit:
    case True{}:
      String.ends_with(kept, " 1")
    case False{}:
      B.Auth.any(B.Rec.threads(st), st)

# the answer for st: the one kept for its sequence, else asked
def Auth.kept(+kept: String, +st: M.State) -> Bool:
  Auth.kept.k(String.starts_with(kept, Auth.kept.key(st)), kept, st)

def Hub.auth.beat.any(+any: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.auth.beat.k(Bool.and(any, Bool.not(J.Json.get_bool(K.Rt.health(Cfg.rt(cfg)), "auth.probe", False{}))),
        Hub{st, f, ls, cl, as, inbox, Cfg.with_bn(cfg, BN.Bn.with_auth(Cfg.bn(cfg), Auth.kept.key(st) ++ Bool.pick(String, any, "1", "0"))), note}, now)

def Hub.auth.beat(h: Hub, now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.auth.beat.any(Auth.kept(BN.Bn.auth(Cfg.bn(cfg)), st), Hub{st, f, ls, cl, as, inbox, cfg, note}, now)

def Hub.auth.ready.clear(h: Hub, st: M.State, open: Bool, now: Nat) -> IO(Hub):
  match h:
    case Hub{s, f, ls, cl, as, inbox, +cfg, note}:
      Hub.step(Hub{s, f, ls, cl, as, inbox, Cfg.with_rt(cfg, K.Rt.set(Cfg.rt(cfg), "auth.probe", J.Flag{False{}})), note}, B.Auth.recover(open, st, now))

def Hub.auth.ready(p: M.State & Hub, open: Bool, now: Nat) -> IO(Hub):
  (+st, h) = p
  Hub.auth.ready.clear(h, st, open, now)

# "<pid> <port>" of another live hub on this home, else "" (this process's
# own lock is none)

def Lock.taken(g: OH.HubGot, +home: String) -> IO(String):
  match g:
    case OH.HubFail{why}:
      IO.die(String, 1, "backplane: cannot lock " ++ home ++ ": " ++ why)
    case OH.HubOurs{}:
      IO.pure(String, "")
    case OH.HubHeld{+pid, +port}:
      IO.pure(String, OH.OneHub.line(pid, port))

# Stand-in agents (core/standin.bend): with BACKPLANE_AGENTS=<dir> the
# provider CLIs come from <dir> or nowhere
def Agents.only.k(r: P.Ran, +blocks: String) -> IO(Unit):
  match r:
    case P.Ran{0, +dir}:
      do IO<Unit>:
        +cur : String <- Env.get("PATH", "")
        H.Env.put("PATH", SI.Standin.path(String.trim(dir), blocks, cur))
        Log.info("stand-in agents only, from " ++ String.trim(dir))
    case P.Ran{_, +out}:
      IO.die(Unit, 1, "backplane: " ++ String.trim(out))

def Agents.only.go(none: Bool, +dir: String, +home: String) -> IO(Unit):
  match none:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      IO.bind(P.Ran, Unit, P.Proc.run("sh", SI.Standin.args(dir, SI.Standin.blocks(home)), "/"), r => Agents.only.k(r, SI.Standin.blocks(home)))

def Agents.only.of(+dir: String, +home: String) -> IO(Unit):
  Agents.only.go(String.is_empty(dir), dir, home)

# a hub on a copied log runs agents only as stand-in blocks: with no
# BACKPLANE_AGENTS, an empty folder of its own (every CLI blocked)
def Agents.only.copy(copy: Bool, +dir: String, +home: String) -> IO(Unit):
  match copy:
    case True{}:
      do IO<Unit>:
        Log.info(CP.Copy.note())
        +d : String = Bool.pick(String, String.is_empty(dir), home ++ "/copy-agents", dir)
        made : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(d)
        Agents.only.of(d, home)
    case False{}:
      Agents.only.of(dir, home)

def Agents.only(+home: String) -> IO(Unit):
  IO.bind(String, Unit, Env.get("BACKPLANE_AGENTS", ""), dir => IO.bind(Bool, Unit, P.Proc.copy(), copy => Agents.only.copy(copy, dir, home)))

def Lock.path(+home: String) -> String:
  home ++ "/hub.lock"

# One hub per home
# ----------------
# Two hubs on one home (a window opened while `--headless` ran for the same
# folder) each kept a state of their own over the same log: a message sent
# in the window never reached the web or the phones, and theirs never the
# window. A hub notes its pid and port in <home>/hub.lock as it starts; a
# later start that finds that hub alive joins it (the window) or leaves it
# be (`--headless`): src/app/main.bend's Start.run.


# the shell's parent is this process, so $PPID is the hub's pid
def Lock.write(+home: String, +port: U32) -> IO(Unit):
  do IO<Unit>:
    r : P.Ran <- P.Proc.run("sh", ["-c", "echo \"$PPID $1\" > \"$2\"", "sh", U32.show(port), Lock.path(home)], "/")
    return Unit{}

def Lock.take.k(free: Bool, +old: String, +home: String, +port: U32) -> IO(String):
  match free:
    case True{}:
      IO.bind(String, String, HL.HubLock.take(Lock.path(home), U32.show(port)), r => Lock.taken(OH.OneHub.got(r), home))
    case False{}:
      IO.pure(String, old)

def Lock.find.sh() -> String:
  "f=$1/hub.lock; [ -f \"$f\" ] || exit 0; read pid port kind < \"$f\"; [ \"$kind\" = flock ] && exit 0; [ -n \"$pid\" ] && [ \"$pid\" != \"$PPID\" ] || exit 0; kill -0 \"$pid\" 2>/dev/null || exit 0; [ \"$(ps -p \"$pid\" -o comm= 2>/dev/null)\" = \"$(ps -p \"$PPID\" -o comm= 2>/dev/null)\" ] && echo \"$pid $port\""

# the holder the probe names: only a "<pid> <port>" line (OH.OneHub.legacy),
# never the shell's own noise (Cygwin bash's /tmp warning once stopped a hub
# on a fresh home)
def Lock.out(r: P.Ran) -> String:
  match r:
    case P.Ran{0, +out}:
      OH.OneHub.legacy(out)
    case _:
      ""

# One hub per home
# ----------------
# Two hubs on one home (a window opened while `--headless` ran for the same
# folder, or `backplane --version` run by hand, which once started a hub on
# the live home) each kept a state of their own over the same log: a
# message sent in the window never reached the web or the phones, and
# theirs never the window. A hub holds an exclusive flock on
# <home>/hub.lock for as long as it lives (hublock.bend, effects/hublock.js,
# core/onehub.bend): taken before anything touches the event store, let go
# by the kernel when the process dies, kept through a restart in place
# (Sys.exec hands the descriptor over). The file says its pid and port
# ("<pid> <port> flock"). A later start that finds it held joins that hub
# (the window), or says so and ends with status 1 (`--headless`, the
# server binary): src/app/main.bend's Start.run, src/server/main.bend.
# A file without "flock" was written by an older hub, which held no lock:
# it is believed while its pid is alive and runs the same program.


# "<pid> <port>" of a live older Backplane named by the file (the same
# program as this one, by its process name), else ""; a file a locking
# hub wrote is left to the lock


def Lock.legacy(+home: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", Lock.find.sh(), "sh", home], "/"), r => IO.pure(String, Lock.out(r)))

def Lock.find.k(+held: String, +home: String) -> IO(String):
  Bool.pick(IO(String), String.is_empty(held), Lock.legacy(home), IO.pure(String, held))

# take the home's lock for this process: "" when it is ours (taken now,
# again, or handed over by the exec of a restart), else "<pid> <port>" of
# the hub that holds it; a lock that cannot be taken at all stops here
def Lock.take(+home: String, +port: U32) -> IO(String):
  do IO<String>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(home)
    +old : String <- Lock.legacy(home)
    Lock.take.k(String.is_empty(old), old, home, port)

def Lock.find(+home: String) -> IO(String):
  IO.bind(String, String, HL.HubLock.held(Lock.path(home)), held => Lock.find.k(OH.OneHub.pair(held), home))

def Lock.word(+held: String, +i: Nat) -> String:
  OH.OneHub.word(held, i)

# a second hub on a home ends here, before it touches the event store
def Lock.refuse(free: Bool, +held: String, +home: String) -> IO(Unit):
  match free:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      IO.die(Unit, 1, "backplane: " ++ OH.OneHub.says(home, Lock.word(held, 0n), Lock.word(held, 1n)))

def Shot.msg.to(all: Bool, +line: String, +o: J.Json) -> Msg:
  match all:
    case True{}:
      Shot{BR.Browser.shot(line)}
    case False{}:
      ShotTo{BR.Browser.shot(line), J.Json.get_str(o, "page", "")}

def Shot.msg.k(frame: Bool, +line: String, +o: J.Json) -> Msg:
  match frame:
    case True{}:
      Shot.msg.to(J.Json.get_bool(o, "all", True{}), line, o)
    case False{}:
      BrowserEv{line}

def Shot.msg.o(+line: String, +o: J.Json) -> Msg:
  Shot.msg.k(String.eq(J.Json.get_str(o, "event", ""), "frame"), line, o)

def Shot.msg(+line: String) -> Msg:
  Shot.msg.o(line, Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}))

def Shot.fwd.got(m: Maybe<&1, String>, +inbox: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{+line}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Shot.msg(line))
        k(Unit{})
    case None{}:
      IO.pure(Unit, Unit{})

def Shot.fwd(fuel: Nat, +ev: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, ev), m => Shot.fwd.got(m, inbox, u => Shot.fwd(f, ev, inbox)))

def KiSrv.fwd.got(m: Maybe<&1, String>, +inbox: Chan(Msg), k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{+text}:
      do IO<Unit>:
        ok : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, KiServers{text})
        k(Unit{})
    case None{}:
      IO.pure(Unit, Unit{})

def KiSrv.fwd(fuel: Nat, +ev: Chan(String), +inbox: Chan(Msg)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, ev), m => KiSrv.fwd.got(m, inbox, u => KiSrv.fwd(f, ev, inbox)))

# replay the log, start the hub (and the web server), then run front
# with the hub's inbox; `need` makes a taken web port fatal
# the project folders the person has, for the history registry's first
# record (git.bend's Git.step.migrate)
def HistReg.roots.k(rm: Bool, +r: String, rest: List<&2, String>) -> List<&2, String>:
  match rm:
    case True{}:
      rest
    case False{}:
      r <> rest

def HistReg.roots.go(ps: List<&2, M.Project>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{_, _, +r, _, rm} <> t:
      HistReg.roots.k(rm, r, HistReg.roots.go(t))

def HistReg.roots(+st: M.State) -> List<&2, String>:
  match st:
    case M.State{+ps, _, _, _, _, _, _, _, _, _, _}:
      HistReg.roots.go(Map.values(&2, M.Project, ps))

# A new UI receives only cached normal Browser controls. In desktop mode this
# never invokes an agent command, starts a helper or changes guest authority.
def Desktop.refresh(m: Maybe<&2, Chan(BR.BMsg)>) -> IO(Unit):
  match m:
    case None{}: IO.pure(Unit, Unit{})
    case Some{q}:
      do IO<Unit>:
        sent : Result<&1, &1, BR.BMsg, Unit> <- Chan.send(BR.BMsg, q, BR.BRefresh{})
        return Unit{}

# the window in this process: the read model (devices left out, as the
# wire leaves them) and the log message with no items
def Hub.join.here(h: Hub, +client: U32, +out: Chan(Wire), +gen: U32, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, +ls, +cl, as, inbox, +cfg, +note}:
      do IO<Hub>:
        m : Maybe<&2, Client> <- Hub.join.state(Client{client, out, DK.Look.none(), OB.Cr.new(gen, True{}, False{}, 0n), 0, False{}, Nil{}}, WState{M.State.private(st), Hub.log_msg.k(ls, cfg)})
        m1 : Maybe<&2, Client> <- Far.join.here(Far.mirrors(cfg), m)
        m2 : Maybe<&2, Client> <- Wire.text.m(m1, LV.Live.msg(Live.of(cfg)))
        m3 : Maybe<&2, Client> <- Hub.join.notes(m2, note, Desk.join(DK.Desk.text(Clients.looks(cl), now)))
        m4 : Maybe<&2, Client> <- Wire.text.m(m3, Hub.pair_msg(cfg))
        Desktop.refresh(Cfg.br(cfg))
        return Hub{st, f, ls, Clients.add(m4, cl), as, inbox, cfg, note}

def Hub.join.k(+n: Nat, +s0: Nat, +xs: List<&2, String>, h: Hub, +client: U32, +out: Chan(Wire), +origin: String, +gen: U32, +sock: U32, +legacy: Bool, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, +ls, +cl, as, +inbox, +cfg, +note}:
      +k = List.length(&2, String, OB.Join.next(xs))
      +more = Bool.or(Nat.is_lt(Nat.add(s0, k), n), Bool.not(List.is_empty(&2, FR.In, Far.mirrors(cfg))))
      do IO<Hub>:
        m : Maybe<&2, Client> <- Wire.offer(Client{client, out, DK.Look.none(), OB.Cr.wait.set(OB.Cr.new(gen, U32.is_eq(client, 0), more, Nat.add(s0, k)), more), sock, legacy, Nil{}},
          Hub.log_items(origin, cfg, s0, List.take(&2, String, xs, k), more), 0n)
        m1 : Maybe<&2, Client> <- Hub.join.live(more, m, cfg)
        m2 : Maybe<&2, Client> <- Hub.join.notes(m1, note, Desk.join(DK.Desk.text(Clients.looks(cl), now)))
        m3 : Maybe<&2, Client> <- Wire.text.if(legacy, m2, Hub.pair_msg(cfg))
        Hub.join.more(more, inbox, client, gen)
        Desktop.refresh(Cfg.br(cfg))
        return Hub{st, f, ls, Clients.add(m3, cl), as, inbox, cfg, note}

# a client joined: the log from what it has (`since`, if it came from this
# log), its first chunk now and the rest a chunk at a time (Hub.boot); the
# origin and the log's length are found once
def Hub.join(h: Hub, +client: U32, +out: Chan(Wire), +since: Nat, +origin: String, +gen: U32, +sock: U32, +legacy: Bool, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, +ls, cl, as, inbox, cfg, note}:
      +n = SO.Lg.n(ls)
      +o = Hub.origin(ls)
      +s0 = Bool.pick(Nat, Bool.and(String.eq(origin, o), Nat.is_le(since, n)), since, 0n)
      IO.bind(List<&2, String>, Hub, SO.Sto.lines(ls, s0, Join.take(n, s0)), xs => Hub.join.k(n, s0, xs, Hub{st, f, ls, cl, as, inbox, cfg, note}, client, out, o, gen, sock, legacy, now))

type ThinReadJob is Data:
  ThinReadJob{client: U32, gen: U32, view: String, target: J.Json, text: String}

type ThinDrained is Data:
  ThinDrained{clients: List<&2, Client>, jobs: List<&2, ThinReadJob>, dirty: Bool}

def ThinRead.jobs(cs: List<&2, J.Json>, +client: U32, +gen: U32, +view: String, +target: J.Json) -> List<&2, ThinReadJob>:
  match cs:
    case Nil{}: Nil{}
    case c <> tail: ThinReadJob{client, gen, view, target, J.Json.get_str(c, "text", "")} <> ThinRead.jobs(tail, client, gen, view, target)

def Client.thin.drain(+c: Client) -> ThinDrained:
  match c:
    case Client{+id, out, look, +cr, sock, legacy, ThinStart{mode, ThinContext{hub, cid, +target, +view, revision, anchor, +actor <> _, control} <> _} <> _}:
      ThinDrained{[Client{id, out, look, cr, sock, legacy, [ThinStart{mode, [ThinContext{hub, cid, target, view, revision, anchor, [TA.Actor.clean(actor)], control}]}]}],
        ThinRead.jobs(TA.Actor.queued(actor), id, OB.Cr.gen(cr), view, target), TA.Actor.dirty(actor)}
    case _: ThinDrained{[c], Nil{}, False{}}

def ThinDrained.append(a: ThinDrained, b: ThinDrained) -> ThinDrained:
  match a b:
    case ThinDrained{ac, ar, ad} ThinDrained{bc, br, bd}: ThinDrained{List.append(&2, Client, ac, bc), List.append(&2, ThinReadJob, ar, br), Bool.or(ad, bd)}

def Clients.thin.drain(cs: List<&2, Client>) -> ThinDrained:
  match cs:
    case Nil{}: ThinDrained{Nil{}, Nil{}, False{}}
    case c <> tail: ThinDrained.append(Client.thin.drain(c), Clients.thin.drain(tail))

def ThinRead.post(jobs: List<&2, ThinReadJob>, +inbox: Chan(Msg)) -> IO(Unit):
  match jobs:
    case Nil{}: IO.pure(Unit, Unit{})
    case ThinReadJob{client, gen, view, target, text} <> tail:
      do IO<Unit>:
        Self.post(inbox, ThinRead{client, gen, view, target, text})
        ThinRead.post(tail, inbox)

def Cfg.thin.dirty(dirty: Bool, cfg: Cfg) -> Cfg:
  match dirty:
    case False{}: cfg
    case True{}: Cfg.thin.bump(cfg)

def Hub.thin.drained(result: ThinDrained, h: Hub) -> IO(Hub):
  match result h:
    case ThinDrained{cl, jobs, dirty} Hub{st, f, ls, _, as, +inbox, cfg, note}:
      do IO<Hub>:
        ThinRead.post(jobs, inbox)
        return Hub{st, f, ls, cl, as, inbox, Cfg.thin.dirty(dirty, cfg), note}

def Hub.thin.drain(h: Hub) -> IO(Hub):
  match h:
    case Hub{st, f, ls, +cl, as, inbox, cfg, note}: Hub.thin.drained(Clients.thin.drain(cl), Hub{st, f, ls, cl, as, inbox, cfg, note})

# Only an actual projection revision change emits this small notification.
# The client coalesces update requests; pages/search/chunks stay gesture-only.
def Wire.thin.notice_fits(fits: Bool, c: Client, +text: String) -> IO(Maybe<&2, Client>):
  match fits:
    case True{}: Wire.offer(c, WText{text}, Wire.text.charge(text))
    case False{}: Wire.offer(c, WClose{}, 0n)

def Wire.thin.notice_ready(ready: Bool, +c: Client, +epoch: String, +revision: Nat) -> IO(Maybe<&2, Client>):
  match ready:
    case False{}: IO.pure(Maybe<&2, Client>, Some{c})
    case True{}:
      +context = Client.thin.context(c)
      +text = J.Json.show(TVW.Wire.changed(U32.show(OB.Cr.gen(Boot.r(c))), epoch, revision, ThinContext.view(context), ThinContext.target(context)))
      Wire.thin.notice_fits(TVW.Wire.within(text, 1024n), c, text)

def Wire.thin.notice_alive(alive: Bool, +c: Client, epoch: String, revision: Nat) -> IO(Maybe<&2, Client>):
  match alive:
    case False{}: Wire.offer(c, WClose{}, 0n)
    case True{}: Wire.thin.notice_ready(Bool.not(String.is_empty(ThinContext.view(Client.thin.context(c)))), c, epoch, revision)

def Wire.thin.notice_one(thin: Bool, c: Client, epoch: String, +revision: Nat) -> IO(Maybe<&2, Client>):
  match thin:
    case False{}: IO.pure(Maybe<&2, Client>, Some{c})
    case True{}: Wire.thin.notice_alive(Nat.is_lt(revision, PRESENT.Session.nat.max()), c, epoch, revision)

# Paced notices (core/thin-pace.bend): each thin view is told what this
# step touched of its own thread at once, anything else at most every
# Pace.soft() ms; what it is owed waits for the flush timer (ThinFlush).
type ThinNoticed is Data:
  ThinNoticed{clients: List<&2, Client>, paces: Map<&2, TPACE.Pace>, wait: Nat}

def ThinNoticed.add(+one: Maybe<&2, Client>, +key: String, +pace: TPACE.Pace, +wait: Nat, rest: ThinNoticed) -> ThinNoticed:
  match rest:
    case ThinNoticed{cs, ps, w}:
      ThinNoticed{Clients.add(one, cs), Map.set(&2, TPACE.Pace, ps, key, pace), TPACE.Wait.min(wait, w)}

def ThinNoticed.keep(+one: Maybe<&2, Client>, rest: ThinNoticed) -> ThinNoticed:
  match rest:
    case ThinNoticed{cs, ps, w}:
      ThinNoticed{Clients.add(one, cs), ps, w}

def Wire.thin.paced.send(+send: Bool, +c: Client, +epoch: String, +revision: Nat) -> IO(Maybe<&2, Client>):
  match send:
    case True{}: Wire.thin.notice_one(True{}, c, epoch, revision)
    case False{}: IO.pure(Maybe<&2, Client>, Some{c})

def Wire.thin.paced.one(+paced: TPACE.Paced, +c: Client, +epoch: String, +revision: Nat, +key: String, rest: ThinNoticed) -> IO(ThinNoticed):
  do IO<ThinNoticed>:
    one : Maybe<&2, Client> <- Wire.thin.paced.send(TPACE.Paced.send(paced), c, epoch, revision)
    return ThinNoticed.add(one, key, TPACE.Paced.pace(paced), TPACE.Paced.wait(paced), rest)

def Wire.thin.paced.thin(+thin: Bool, +c: Client, +st: M.State, +touch: TPACE.Touch, +ps: Map<&2, TPACE.Pace>, +now: Nat, +epoch: String, +revision: Nat, rest: ThinNoticed) -> IO(ThinNoticed):
  match thin:
    case False{}: IO.pure(ThinNoticed, ThinNoticed.keep(Some{c}, rest))
    case True{}:
      +key = U32.show(Client.id(c))
      Wire.thin.paced.one(TPACE.Pace.step(TPACE.Pace.of(ps, key), now, TPACE.Touch.level(touch, TV.Target.thread(st, ThinContext.target(Client.thin.context(c))))), c, epoch, revision, key, rest)

def Wire.thin.paced(clients: List<&2, Client>, +st: M.State, +touch: TPACE.Touch, +ps: Map<&2, TPACE.Pace>, +now: Nat, +epoch: String, +revision: Nat) -> IO(ThinNoticed):
  match clients:
    case Nil{}: IO.pure(ThinNoticed, ThinNoticed{Nil{}, Map.new(&2, TPACE.Pace), 0n})
    case +c <> tail:
      do IO<ThinNoticed>:
        rest : ThinNoticed <- Wire.thin.paced(tail, st, touch, ps, now, epoch, revision)
        Wire.thin.paced.thin(Client.thin(c), c, st, touch, ps, now, epoch, revision, rest)

def Cfg.thin.with_notes(+cfg: Cfg, notes: TPACE.Notes) -> Cfg:
  Cfg.with_thin_cache(cfg, ThinCache{Cfg.thin.lazy(cfg), Cfg.thin.epoch(cfg), Cfg.thin.revision(cfg), Cfg.thin.streams(cfg), Cfg.thin.mail.lazy(cfg), Cfg.thin.waiting(cfg), notes, Cfg.thin.store(cfg)})

def Hub.thin.timer(+want: Bool, +inbox: Chan(Msg), +wait: Nat) -> IO(Unit):
  match want:
    case True{}: IO.spawn(Unit, Self.post.wait(inbox, ThinFlush{}, U32.from_nat(wait)))
    case False{}: IO.pure(Unit, Unit{})

def Hub.thin.noticed(r: ThinNoticed, h: Hub, +now: Nat) -> IO(Hub):
  match r h:
    case ThinNoticed{cl2, ps, +wait} Hub{st, f, ls, _, as, +inbox, +cfg, note}:
      +flush = TPACE.Notes.flush(Cfg.thin.notes(cfg))
      +want = TPACE.Notes.timer(flush, now, wait)
      do IO<Hub>:
        Hub.thin.timer(want, inbox, wait)
        return Hub{st, f, ls, cl2, as, inbox, Cfg.thin.with_notes(cfg, TPACE.Notes{TPACE.Touch.none(), ps, Bool.pick(Nat, want, Nat.add(now, wait), flush)}), note}

# tell the thin views what this step touched (now: ms)
def Hub.thin.notify(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{+st, f, ls, cl, as, inbox, +cfg, note}:
      do IO<Hub>:
        r : ThinNoticed <- Wire.thin.paced(cl, st, TPACE.Notes.touched(Cfg.thin.notes(cfg)), TPACE.Notes.paces(Cfg.thin.notes(cfg)), now, Cfg.thin.epoch(cfg), Cfg.thin.revision(cfg))
        Hub.thin.noticed(r, Hub{st, f, ls, Nil{}, as, inbox, cfg, note}, now)

def Hub.thin.invalidate_if(changed: Bool, h: Hub, +now: Nat) -> IO(Hub):
  match changed:
    case False{}: IO.pure(Hub, h)
    case True{}: Hub.thin.notify(h, now)

def Hub.thin.invalidate(before: Nat, h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.thin.invalidate_if(Bool.or(Bool.not(Nat.is_eq(before, Cfg.thin.revision(cfg))), Nat.is_eq(Cfg.thin.revision(cfg), PRESENT.Session.nat.max())), Hub{st, f, ls, cl, as, inbox, cfg, note}, now)

# the flush timer fired: what views are owed and due is told now (its time
# cleared first, so a wait still owed sets the timer again)
def Hub.thin.flush(h: Hub, +now: Nat) -> IO(Hub):
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}:
      Hub.thin.notify(Hub{st, f, ls, cl, as, inbox, Cfg.thin.with_notes(cfg, TPACE.Notes{TPACE.Notes.touched(Cfg.thin.notes(cfg)), TPACE.Notes.paces(Cfg.thin.notes(cfg)), 0n}), note}, now)

def Hub.handle(h: Hub, m: Msg, +now: Nat) -> IO(Hub):
  match m:
    case LockProbe{name, holder, n}:
      Hub.lock.probe(h, name, holder, n, now)
    case LockSaw{name, holder, out}:
      Hub.lock.saw(h, name, holder, out, now)
    case LockHttp{op, body, reply}:
      Hub.lock.http(h, op, body, reply, now)
    case AgentWrite{thread, line, gen}:
      Hub.agent_write(h, thread, line, gen)
    case UpdateCheck{}:
      Hub.update_check(h)
    case UpdateReady{force}:
      Hub.update_ready(h, force, now)
    case DeployLocks{free}:
      Hub.deploy_locks(h, free, now)
    case DeployFailed{err}:
      Hub.deploy_failed(h, err, now)
    case ShipAsked{thread, body, reply, out}:
      Hub.ship.asked(h, thread, body, reply, out, now)
    case ShipNewest{why, out}:
      Hub.ship.newest(h, why, out, now)
    case ShipPid{commit, out}:
      Hub.ship.pid(h, commit, out, now)
    case ShipDone{commit, out}:
      Hub.ship.done(h, commit, out, now)
    case Mcp{thread, body, reply}:
      Hub.mcp.g(h, B.Tag.thread(thread), body, reply, now)
    case McpOpen{thread, call, body}:
      Hub.mcp.open(h, B.Gen.thread(B.Tag.thread(thread)), call, body)
    case McpShut{thread, call}:
      Hub.mcp.shut(h, B.Gen.thread(B.Tag.thread(thread)), call)
    case Announce{text}:
      Hub.note(h, text)
    case Join{client, out, since, origin, gen, sock, legacy}:
      Hub.join(h, client, out, since, origin, gen, sock, legacy, now)
    case MobileJoin{client, out, since, origin, gen, sock, legacy, mode}:
      Hub.mobile.join(h, client, out, since, origin, gen, sock, legacy, mode)
    case MobileTimeout{client, gen}:
      Hub.mobile.timeout(h, client, gen)
    case JoinHere{client, out, gen}:
      Hub.join.here(h, client, out, gen, now)
    case Drained{client, gen, n, b}:
      Hub.drained(h, client, gen, n, b)
    case JoinMore{client, gen}:
      Hub.boot(h, client, gen)
    case Ping{client, p}:
      Hub.pong(h, client, p)
    case Leave{client}:
      Hub.leave(h, client, now)
    case ThinRead{client, gen, view, target, text}:
      Hub.thin.read_job(h, client, gen, view, target, text, now)
    case Rpc{client, text}:
      Hub.rpc(h, client, text, now)
    case AgentLine{tag, line}:
      Hub.tagged(h, tag, line, now)
    case TurnPid{thread, pid, input, gen}:
      Hub.turn_pid(h, thread, pid, input, gen)
    case TurnDone{tag}:
      Hub.turn_done(h, tag)
    case Recover{}:
      Hub.recover(h, now)
    case Reexec{path, args}:
      Hub.reexec(h, path, args)
    case AgentGone{thread, code, pid, why}:
      Hub.gone(h, thread, code, pid, why, now)
    case TelemetryPermit{gen, view, reply}:
      Hub.telemetry.permit(h, gen, view, reply)
    case TelemetryDone{gen, view, id}:
      Hub.telemetry.done(h, gen, view, id, now)
    case Tick{}:
      Hub.tick(h, now)
    case ThinFlush{}:
      IO.bind(Nat, Hub, IO.now(), ms => Hub.thin.flush(h, ms))
    case Heart{}:
      IO.bind(Hub, Hub, IO.bind(Hub, Hub, IO.bind(Hub, Hub, Hub.step(h, B.Step.none()), h1 => Hub.beat.all(h1, DK.Alive.frame())), h2 => Hub.uber.beat(h2, now)), h3 => Hub.deploy_poll(h3, now))
    case AutoView{thread, fs}:
      Hub.autoview(h, thread, fs)
    case ClaudePid{thread, pid, gen}:
      Hub.claude_pid(h, thread, pid, gen)
    case ClaudeFail{thread, gen, msg}:
      Hub.claude_fail(h, thread, gen, msg)
    case Push{client, text}:
      Hub.push(h, client, text)
    case PushBin{client, bs}:
      Hub.push.bin(h, client, bs)
    case TermOut{thread, data}:
      Hub.term_out(h, thread, data)
    case TermTo{c, key, co, ro, data}:
      Hub.term_to(h, c, key, co, ro, data)
    case KProbe{force}: Hub.kcheck(h, force)
    case KFound{setting, rt}: Hub.kfound(h, setting, rt)
    case KiServers{text}: Hub.kiservers(h, text)
    case ModelsFound{ms, mt}:
      Hub.models(h, ms, mt)
    case OcFound{text}:
      Hub.oc.found(h, text)
    case ChecksAsk{thread, hash, text}:
      Hub.checks.ask(h, thread, hash, text, now)
    case MachinesFound{mc}:
      Hub.dir.returned(h, mc)
    case LinkFound{lk}:
      Hub.link(h, lk)
    case MailRouted{body}: Hub.mail.routed(h, body, now)
    case RunReady{thread, head, bs}: Hub.run.ready(h, thread, head, bs, now)
    case DigestGot{bot, ids, bs}: Hub.digest.got(h, bot, ids, bs, now)
    case Commit{cs}:
      Hub.step(h, B.Step{cs, Nil{}})
    case OrderNote{thread, id, text, +ping}:
      Hub.order.note(h, thread, id, text, ping, now)
    case OrderWake{+thread, +text}:
      Hub.order.wake(h, thread, text, now)
    case OrderGate{+thread, kind, +md}:
      Hub.order.gate(kind, h, thread, md, now)
    case BoatThreads{status}:
      Hub.chips(Hub.state(h), status, now)
    case ThinWarm{i, m}:
      Hub.thin.warmed(h, i, m)
    case UberDo{what}:
      Hub.uber.do(h, what, now)
    case UberAnswer{link, code, body}:
      Hub.uber.answer(h, link, code, body, now)
    case BotTick{tz}:
      Hub.bot_tick(h, tz, now)
    case BotsFound{peer, url, result}:
      Hub.bots_found(h, peer, url, result)
    case BotsRefresh{peer}:
      Hub.dir.refresh(h, peer)
    case BotHttp{kind, arg, peer, ts, sig, gh, dl, body, ct, raw, reply}:
      Hub.bot_http(h, kind, arg, peer, ts, sig, gh, dl, body, ct, raw, reply, now)
    case BotVerified{kind, arg, key, body, me, reply}:
      Hub.bot_verified(h, kind, arg, key, body, me, reply, now)
    case NewProject{_, root, name, id, reply}:
      Hub.newproj.made(h, root, name, id, reply, now)
    case CalAsk{thread, tool, args, detail, id, reply, ask}:
      Hub.cal.ask(ask, h, thread, tool, Gc2.json(args), detail, Gc2.json(id), reply, now)
    case GmailAsk{thread, tool, detail, plan, id, reply}:
      Hub.gmail.ask(h, thread, tool, detail, plan, id, reply, now)
    case Shot{text}:
      Hub.broadcast(h, text)
    case ShotTo{text, page}:
      Hub.shot.to(h, text, page)
    case TeachReq{client, id, m, p}:
      Hub.teach(h, client, id, m, p, now)
    case BrowserEv{line}:
      Hub.browser_ev(h, line)
    case Desktop{client, kind, text, out, e, l}: Hub.desktop(h, client, kind, text, out, e, l, now)
    case CefDone{request, live, err}:
      Hub.cef.finished(h, request, live, err)
    case Near{client}:
      Hub.cef.near(h, client)
    case TeachBack{bot}:
      Hub.back(h, bot)
    case TaskPaused{thread, bot, run, slug, n, kind, question, risky, rid, reply}:
      Hub.task.paused(h, thread, bot, run, slug, n, kind, question, risky, rid, reply, now)
    case AttDir{thread, reply}:
      Hub.att_dir(h, thread, reply)
    case FarFeed{ls2, epoch}:
      Hub.far.fed(h, ls2, epoch)
    case FarPushed{link, ok, to}:
      Hub.far.pushed(h, link, ok, to)
    case FarLiveAck{link, code, body}:
      Far.live.ack(h, link, code, body)
    case FarResend{link}:
      Hub.far.resend(h, link)
    case FarPulled{link, since, code, body}:
      Hub.far.pulled(h, link, since, code, body)
    case FarReplied{client, id, link, code, body}:
      Hub.far.replied(h, client, id, link, code, body)
    case FarLoaded{link, name, text}:
      Hub.far.loaded(h, link, name, text)
    case FarHere{ans}:
      Hub.far.here(h, ans)
    case FarRetry{link}:
      Far.pull.here(h, link)
    case FarDo{+text, +reply}:
      Hub.far.do(J.Json.parse(text), h, text, reply, now)
    case Replied{+thread, +to, +text}:
      Hub.replied(Hub.state(h), thread, to, text, now)
    case LaunchSent{+key}:
      Hub.launch.sent(Hub.state(h), key, now)
    case Release{}:
      Hub.release(Hub.state(h), now)
    case SpawnFailed{thread, gen, line}:
      Hub.spawn_failed(h, thread, gen, line, now)
    case AgentExited{thread, gen, code}:
      Hub.exited(h, thread, gen, code)
    case Silence{}:
      Hub.quiet.beat(h, now)
    case KeyringSeen{state}:
      Hub.auth.ready(Hub.state(h), Nat.is_eq(state, 1n), now)
    case Beat{}:
      Hub.auth.beat(h, now)
    case WatchLaunched{thread, id}:
      Hub.watched(Hub.state(h), 0n, thread, id, "", "", now)
    case WatchIdent{thread, id, ident}:
      Hub.watched(Hub.state(h), 1n, thread, id, ident, "", now)
    case WatchEnded{thread, id, status, detail}:
      Hub.watched(Hub.state(h), 2n, thread, id, status, detail, now)
    case WakeSeen{kind, target, result}:
      Hub.wake.seen(Hub.state(h), kind, target, result, now)
    # unwrapped by Hub.got, never nested
    case Bulk{_}:
      IO.pure(Hub, h)

def Hub.thin.snapshot(h: Hub) -> Nat & Hub:
  match h:
    case Hub{st, f, ls, cl, as, inbox, +cfg, note}: (Cfg.thin.revision(cfg), Hub{st, f, ls, cl, as, inbox, cfg, note})

# each message's handling is timed as hub.<kind> (docs/perf.md)
def Hub.got.one_at(+msg: Msg, snapshot: Nat & Hub, k: Hub -> IO(Unit)) -> IO(Unit):
  (before, h) = snapshot
  do IO<Unit>:
    now : Nat <- Now.nat()
    t0 : U32 <- PS.Perf.us()
    h1 : Hub <- Hub.handle(h, msg, now)
    h2 : Hub <- Hub.cef.after(CF.Cef.after(Msg.kind(msg)), h1)
    h2a : Hub <- Hub.thin.drain(h2)
    ms : Nat <- IO.now()
    h3 : Hub <- Hub.thin.invalidate(before, h2a, ms)
    t1 : U32 <- PS.Perf.lap("hub." ++ Msg.kind(msg), t0)
    k(h3)

def Hub.got.one(+msg: Msg, h: Hub, k: Hub -> IO(Unit)) -> IO(Unit):
  Hub.got.one_at(msg, Hub.thin.snapshot(h), k)

def Hub.got(m: Maybe<&1, Msg>, h: Hub, +tok: Chan(Unit), k: Hub -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{Bulk{+msg}}:
      Hub.got.one(msg, h, h2 => Bulk.back(tok, h2, k))
    case Some{+msg}:
      Hub.got.one(msg, h, k)
    case None{}:
      Hub.close(h)

def Hub.loop(fuel: Nat, h: Hub, +pri: Chan(Msg), +tok: Chan(Unit)) -> IO(Unit):
  match fuel:
    case 0n:
      Hub.close(h)
    case 1n+f:
      IO.bind(Maybe<&1, Msg>, Unit, Chan.recv(Msg, pri), m => Hub.got(m, h, tok, h3 => Hub.loop(f, h3, pri, tok)))

# without the web server (its port was taken): hub and ticker only
def Main.headless(h: Hub, +inbox: Chan(Msg), +cfg: Cfg, +tok: Chan(Unit), front: Chan(Msg) -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    IO.spawn(Unit, Ticker.loop(Fuel.forever(), inbox))
    IO.spawn(Unit, Beat.loop(Fuel.forever(), inbox))
    IO.spawn(Unit, Heart.loop(Fuel.forever(), inbox))
    Refresh.start(inbox, cfg)
    IO.spawn(Unit, Od.resume(inbox, Cfg.home(cfg)))
    Self.post(inbox, KProbe{True{}})
    IO.spawn(Unit, Hub.loop(Fuel.forever(), h, inbox, tok))
    Upgrade.start(cfg, inbox)
    front(inbox)

def Main.serve(l: Listener, h: Hub, +web: String, +inbox: Chan(Msg), +cfg: Cfg, +tok: Chan(Unit), front: Chan(Msg) -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    IO.spawn(Unit, Accept.loop(Fuel.forever(), l, 1, web, inbox, True{}, Net.token(Cfg.net(cfg)), ""))
    Tailnet.start(Cfg.net(cfg), Cfg.host(cfg), Cfg.port(cfg), web, inbox)
    IO.spawn(Unit, Ticker.loop(Fuel.forever(), inbox))
    IO.spawn(Unit, Beat.loop(Fuel.forever(), inbox))
    IO.spawn(Unit, Heart.loop(Fuel.forever(), inbox))
    Refresh.start(inbox, cfg)
    IO.spawn(Unit, Od.resume(inbox, Cfg.home(cfg)))
    Self.post(inbox, KProbe{True{}})
    Machines.start(inbox, Cfg.port(cfg))
    IO.spawn(Unit, Hub.loop(Fuel.forever(), h, inbox, tok))
    Upgrade.start(cfg, inbox)
    front(inbox)

def Main.listened(r: Result<&1, &1, U32 & String, Listener>, h: Hub, +web: String, +inbox: Chan(Msg), +addr: String, need: Bool, +cfg: Cfg, +tok: Chan(Unit), front: Chan(Msg) -> IO(Unit)) -> IO(Unit):
  match r need:
    case Done{l} _:
      do IO<Unit>:
        IO.print("Backplane listening on http://" ++ addr)
        Main.serve(l, h, web, inbox, cfg, tok, front)
    case Fail{e} True{}:
      Main.failed(e, h, addr)
    case Fail{_} False{}:
      do IO<Unit>:
        Log.info("web server off: " ++ addr ++ " is taken; trying again every 2 s")
        IO.spawn(Unit, Listen.retry(Fuel.forever(), web, inbox, cfg, addr))
        Main.headless(h, inbox, cfg, tok, front)

# Browser frames
# --------------



# a frame goes to every client (or, past the helper's --jpeg-fps, only to
# the clients showing its page: castio.bend); a recorder step, navigation
# or load to the hub's teach state



# page-frame events from the browser, to the hub


# the headless KiCad servers' lines, to the hub

# the thin index and the mail cache built after the hub listens, on their
# own thread, from the state it booted with: what the boot used to build
# before it (THIN.Lazy, TMAIL.Lazy); the hub takes them as ThinWarm
def Thin.warm.go(+st: M.State, +inbox: Chan(Msg)) -> IO(Unit):
  do IO<Unit>:
    t0 : U32 <- PS.Perf.us()
    +i : THIN.Index = THIN.Index.of(st)
    Log.info("thin index: " ++ Nat.show(THIN.Index.n(i)) ++ " rows")
    t1 : U32 <- PS.Perf.span("thin.warm", t0)
    +m : TMAIL.Cache = TMAIL.Cache.of(st)
    Log.info("mail cache at " ++ Nat.show(TMAIL.Cache.revision(m)))
    t2 : U32 <- PS.Perf.span("mail.warm", t1)
    r : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, ThinWarm{i, m})
    return Unit{}

def Thin.warm.if(built: Bool, +st: M.State, +inbox: Chan(Msg)) -> IO(Unit):
  match built:
    case True{}: IO.pure(Unit, Unit{})
    case False{}: IO.spawn(Unit, Thin.warm.go(st, inbox))

# both are Later from the same boot state, or both built (an empty home)
def Thin.warm(+c: ThinCache, +inbox: Chan(Msg)) -> IO(Unit):
  Thin.warm.if(Bool.and(THIN.Lazy.ready(ThinCache.lazy(c)), TMAIL.Lazy.ready(ThinCache.mail(c))), THIN.Lazy.base(ThinCache.lazy(c)), inbox)

# the thin views on store.db when the boot opened one (none: as before)
def Thin.store.if(none: Bool, +cfg: Cfg, +st: M.State, +h: String) -> IO(Cfg):
  match none:
    case True{}:
      IO.pure(Cfg, cfg)
    case False{}:
      Thin.store.open(cfg, st, h)

def Main.opened(r: Result<&1, &1, U32 & String, File>, +st: M.State, ls: SO.Lg, cfg: Cfg, need: Bool, front: Chan(Msg) -> IO(Unit), t0: U32) -> IO(Unit):
  match r cfg:
    case Done{f} Cfg{+home, +web, +host, +port, +pr, +ds, +v, nt0, +tcfg, +views}:
      do IO<Unit>:
        +inbox : Chan(Msg) <- Chan.new(Msg, 4096)
        +pri : Chan(Msg) <- Chan.new(Msg, 4096)
        +tok : Chan(Unit) <- Chan.new(Unit, 32)
        Bulk.fill(Bulk.room(), tok)
        IO.spawn(Unit, Bulk.fwd(Fuel.forever(), inbox, pri, tok))
        rc : Result<&1, &1, Msg, Unit> <- Chan.send(Msg, inbox, Recover{})
        +bq : Chan(BR.BMsg) <- Chan.new(BR.BMsg, 256)
        bexe : String <- Browser.exe()
        # page-frame events from the browser, broadcast by the hub as {"t":"shot",...}
        +bev : Chan(String) <- Chan.new(String, 256)
        IO.spawn(Unit, Shot.fwd(Fuel.forever(), bev, inbox))
        +desktopMode : String <- Env.get("BACKPLANE_DESKTOP_BROWSER", "0")
        IO.spawn(Unit, BR.Browser.run(String.eq(desktopMode, "1"), bq, bev, BR.Br.at(bexe, home), J.Json.show(J.Json.obj([J.KV{"version", J.Str{v}}, J.KV{"hub", J.Str{M.Setting.get(st, "machine.id", "")}}]))))
        # the headless KiCad servers (src/server/kserve.bend): their lines go to the hub as KiServers
        +kq : Chan(KSV.KMsg) <- Chan.new(KSV.KMsg, 256)
        +kev : Chan(String) <- Chan.new(String, 64)
        kexe : String <- KSV.KServe.exe()
        IO.spawn(Unit, KiSrv.fwd(Fuel.forever(), kev, inbox))
        IO.spawn(Unit, KSV.KServe.run(kq, kev, kexe, home))
        +pq : Chan(PK.PMsg) <- Chan.new(PK.PMsg, 256)
        IO.spawn(Unit, PK.Park.loop(Fuel.forever(), pq, Nil{}))
        +tq : Chan(TM.TMsg) <- Chan.new(TM.TMsg, 1024)
        +to : Chan(TM.TOut) <- Chan.new(TM.TOut, 4096)
        shell : String <- Env.get("SHELL", "/bin/bash")
        IO.spawn(Unit, TM.Term.start(tq, shell, home, to))
        IO.spawn(Unit, Term.fwd(Fuel.forever(), to, inbox))
        +vq : Chan(PV.PMsg) <- Chan.new(PV.PMsg, 1024)
        +vo : Chan(PV.POut) <- Chan.new(PV.POut, 1024)
        IO.spawn(Unit, PV.Pv.loop(Fuel.forever(), vq, PV.Pv.empty(), vo, home ++ "/plot"))
        IO.spawn(Unit, Plot.fwd(Fuel.forever(), vo, inbox))
        +nt : Net = Net.with_ks(Net.with_pv(Net.with_tm(Net.with_pk(Net.with_br(nt0, bq), pq), tq), vq), kq)
        t1 : U32 <- PS.Perf.span("boot.helpers", t0)
        # the agents a restart in place handed off are kept before anything
        # else can reach the hub (adopt.bend); recovery then leaves them be
        +now : Nat <- Now.nat()
        h0 : Hub <- Hub.adopt(Hub{st, f, ls, Nil{}, Nil{}, inbox, Cfg{home, web, host, port, pr, ds, v, nt, tcfg, views}, ""}, now)
        t2 : U32 <- PS.Perf.span("boot.adopt", t1)
        l : Result<&1, &1, U32 & String, Listener> <- H.Net.listen(host, port)
        t3 : U32 <- PS.Perf.span("boot.listen", t2)
        Thin.warm(ThinCache.first(views), inbox)
        Main.listened(l, h0, web, pri, host ++ ":" ++ U32.show(port), need, Cfg{home, web, host, port, pr, ds, v, nt, tcfg, views}, tok, front)
    case Fail{e} _:
      Main.nolog(e)

# The state a boot starts from (S3 hot boot)
# ------------------------------------------
# store.db's records plus the log's tail (src/server/hotboot.bend). A hub
# with no usable store.db (none, another codec, another log) never replays
# the whole log into memory: a child of its own binary (--store-rebuild,
# S1's stobuild) streams the log into store.db, and the hot boot then
# loads it. A failed build is tried once more (its files removed first);
# a second failure stops the hub with the reason. The whole replay
# (Boot.cold) is only the developer's escape, BACKPLANE_HOT_BOOT=0
# (Boot.next; law boot_next_never_cold).

# the escape's whole replay: the lines straight from the file, then only
# the lines kept
def Boot.cold(+home: String) -> IO(HB.Hot):
  do IO<HB.Hot>:
    +rd : List<&2, String> <- H.EvLog.read_from(Store.path(home), "0")
    +ld : ST.Loaded = ST.Store.load(ST.Store.read.body(rd), ST.Loaded{M.State.empty(), Nil{}})
    return HB.Hot{True{}, "", ST.Loaded.st(ld), ST.Loaded.wire(ld), ST.Store.read.tail(rd)}

# what a boot does next: use the store it loaded, build store.db (builds
# left), stop (none left), or the escape's whole replay
type BootNext is Data:
  BootUse{}
  BootBuild{}
  BootStop{}
  BootCold{}

def Boot.next(escape: Bool, ok: Bool, left: Nat) -> BootNext:
  match escape ok left:
    case True{} _ _:
      BootCold{}
    case False{} True{} _:
      BootUse{}
    case False{} False{} 0n:
      BootStop{}
    case False{} False{} 1n+_:
      BootBuild{}

def Boot.is_cold(n: BootNext) -> Bool:
  match n:
    case BootCold{}:
      True{}
    case _:
      False{}

# how many builds a boot tries
def Boot.builds() -> Nat:
  2n

# The child: the hub's own binary with --store-rebuild (EX.Exe.child: the
# running image even after an update replaced the file), its JS heap kept
# small (it shares the hub's memory cap) and Bun's stack as the hub's
def Boot.child.env() -> List<&2, String>:
  ["BUN_JSC_forceRAMSize=671088640", "BUN_JSC_maxPerThreadStackUsage=536870912"]

def Boot.build(+home: String) -> IO(P.Ran):
  +db = home ++ "/store.db"
  do IO<P.Ran>:
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-f", db, db ++ "-wal", db ++ "-shm"], "", 4096, 30000)
    exe : String <- H.Sys.exe_path()
    t0 : U32 <- PS.Perf.us()
    r : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn("env", List.append(&2, String, Boot.child.env(), [EX.Exe.child(exe), "--store-rebuild", home]), "/", 1)
    +ran : P.Ran <- P.Proc.run.k(r)
    t1 : U32 <- PS.Perf.span("boot.store_build", t0)
    return ran

def Boot.ran.last(+out: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.reverse(&2, String, String.split(String.trim(out), '\n'))), "")

def Boot.ran.say(r: P.Ran) -> String:
  match r:
    case P.Ran{0, +out}:
      "store.db built: " ++ Boot.ran.last(out)
    case P.Ran{code, +out}:
      "store.db build failed (exit " ++ U32.show(code) ++ "): " ++ Boot.ran.last(out)

def Boot.act(n: BootNext, +hot: HB.Hot, +home: String, again: Unit -> IO(HB.Hot)) -> IO(HB.Hot):
  match n:
    case BootUse{}:
      do IO<HB.Hot>:
        Log.info("hot boot from " ++ HB.Hot.path(home))
        return hot
    case BootBuild{}:
      do IO<HB.Hot>:
        Log.info("store.db not used (" ++ HB.Hot.why(hot) ++ "): building it from the log")
        ran : P.Ran <- Boot.build(home)
        Log.info(Boot.ran.say(ran))
        again(Unit{})
    case BootStop{}:
      IO.die(HB.Hot, 1, "backplane: store.db could not be built from the event log (" ++ HB.Hot.why(hot) ++ "); not starting. Its log: " ++ home ++ "/events.jsonl")
    case BootCold{}:
      do IO<HB.Hot>:
        Log.info("full replay: " ++ HB.Hot.why(hot))
        Boot.cold(home)

def Boot.got(+hot: HB.Hot, +home: String, +left: Nat, again: Unit -> IO(HB.Hot)) -> IO(HB.Hot):
  Boot.act(Boot.next(False{}, HB.Hot.ok(hot), left), hot, home, again)

# load the store; if it is not usable, build it and load again (left
# builds at most)
def Boot.loop(left: Nat, +home: String) -> IO(HB.Hot):
  match left:
    case 0n:
      IO.bind(HB.Hot, HB.Hot, HB.Hot.load(home), hot => Boot.got(hot, home, 0n, u => IO.pure(HB.Hot, HB.Hot.no("no build left"))))
    case 1n+f:
      IO.bind(HB.Hot, HB.Hot, HB.Hot.load(home), hot => Boot.got(hot, home, 1n+f, u => Boot.loop(f, home)))

def Boot.state.on(on: Bool, +home: String) -> IO(HB.Hot):
  match on:
    case True{}:
      Boot.loop(Boot.builds(), home)
    case False{}:
      Boot.act(Boot.next(True{}, False{}, 0n), HB.Hot.no("BACKPLANE_HOT_BOOT=0"), home, u => IO.pure(HB.Hot, HB.Hot.no("escape")))

def Boot.state(+home: String) -> IO(HB.Hot):
  do IO<HB.Hot>:
    +on : String <- Env.get("BACKPLANE_HOT_BOOT", "1")
    Boot.state.on(Bool.not(String.eq(on, "0")), home)

def Main.boot(cfg: Cfg, need: Bool, front: Chan(Msg) -> IO(Unit)) -> IO(Unit):
  match cfg:
    case Cfg{+home, w, h, +p, pr, ds, v, nt, +tcfg, +views}:
      do IO<Unit>:
        # the home's lock first: a second hub ends before the event store
        +held : String <- Lock.take(home, p)
        Lock.refuse(String.is_empty(held), held, home)
        H.Env.put("BACKPLANE_HOME", home)
        Agents.only(home)
        t0 : U32 <- PS.Perf.us()
        # store.db's records and the log's tail (hotboot.bend), or the
        # whole log replayed when there is no usable store
        +bt : HB.Hot <- Boot.state(home)
        t2 : U32 <- PS.Perf.span("boot.replay", t0)
        +torn : String = HB.Hot.torn(bt)
        +st : M.State = HB.Hot.st(bt)
        GT.Git.step.migrate(home, HistReg.roots(st))
        f0 : Result<&1, &1, U32 & String, File> <- File.open(Store.path(home), "a")
        f : Result<&1, &1, U32 & String, File> <- Main.torn(f0, torn)
        # S1: Sto.boot here (the store brought up to the log's end; the hub
        # keeps no log lines, SO.Lg)
        +ls : SO.Lg <- SO.Sto.boot(home, st)
        ts : U32 <- PS.Perf.span("boot.store", t2)
        Log.info("replayed " ++ Nat.show(SO.Lg.n(ls)) ++ " changes from " ++ Store.path(home))
        tok : String <- Token.load(home)
        +mc : String <- Files.text(Models.cache(home))
        +bc : String <- BN.Bf.text(BN.Bn.cache(home))
        exe : String <- H.Sys.exe_dir()
        t3 : U32 <- PS.Perf.span("boot.files", ts)
        rt0 : K.Rt <- K.Rt.boot(home, M.Setting.get(st, "kicad.path", ""), exe)
        t4 : U32 <- PS.Perf.span("boot.kicad", t3)
        +rt : K.Rt = K.Rt.set(rt0, "ipc", J.Str{""})
        +off : String <- Env.get("BACKPLANE_NO_UPDATE", "")
        +ai : String <- Env.get("APPIMAGE", "")
        +tc : List<&2, TI.TmCfg> <- TI.Tio.config(TI.Tio.sb(tcfg))
        finalCfg : Cfg = Cfg.with_managed(Cfg.with_models(Cfg.with_bn(Cfg.with_token(Cfg.with_rt(Cfg{home, w, h, p, pr, ds, v, nt, tc, views}, rt), tok), BN.Bn.with_dir(BN.Bn.empty(), BN.Bn.cache.load(bc, BO.World.peers(M.State.bots(st))))), BT.Cache.json(mc), BT.Cache.text(mc)), SV.Chan.managed(off, ai))
        +cfg1 : Cfg = LogCount.init(finalCfg, ls)
        Log.info("log count " ++ Nat.show(LogCount.n(cfg1)))
        t5 : U32 <- PS.Perf.span("boot.cfg", t4)
        +ti : THIN.Lazy = THIN.Lazy.of(st)
        t6 : U32 <- PS.Perf.span("boot.thin", t5)
        +tmc : TMAIL.Lazy = TMAIL.Lazy.of(st)
        t7 : U32 <- PS.Perf.span("boot.mail", t6)
        +org : String = Bool.pick(String, Nat.is_eq(SO.Lg.n(ls), 0n), "", Hub.origin(ls))
        Log.info("origin " ++ org)
        t8 : U32 <- PS.Perf.span("boot.origin", t7)
        # the thin views read store.db once it is open and written on every
        # commit (Thin.store.open: the index keeps only recent threads)
        cfg2 : Cfg <- Thin.store.if(String.is_empty(SO.Lg.db(ls)), Cfg.with_thin_cache(cfg1, ThinCache{ti, org, 1n, Map.new(&2, TLIVE.Stream), tmc, Nil{}, TPACE.Notes.empty(), ""}), st, SO.Lg.db(ls))
        Main.opened(f, st, ls, cfg2, need, front, t8)
