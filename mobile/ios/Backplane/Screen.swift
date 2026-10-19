import Foundation

// The screen src/mobile/screen.bend emits. Plain data; no decisions.

// a swipe button: the action it sends with its value, or (a snooze)
// choices whose values it sends instead
struct Swipe: Decodable, Hashable {
    let label, action, value, tone: String
    let options: [SwipeChoice]
}

struct SwipeChoice: Decodable, Hashable {
    let label, value: String
}

struct Row: Decodable, Identifiable, Hashable {
    let id, title, state, ago: String
    // what its dot says: approval, input, working, monitoring, failed,
    // queued, complete, stopped, idle
    let status: String?
    let pinned: Bool
    let lead, trail: [Swipe]
    // the active view's rows: the project's name, and faded (settled lately)
    let project: String?
    let faded: Bool?
    // "2 agents" while subagents work for it, shown where the age goes
    let agents: String?
}

struct Project: Decodable, Identifiable {
    // machine: the hub it is on, named when the phone has several
    let id, title, root, machine: String
    let open: Bool
    let threads: [Row]
    let snoozedShelf: String
    let snoozed: [Row]
    let shelf: String
    let settled: [Row]
    // the Archived shelf: "toggle-settled" with value opens or shuts it
    let value: String?
    let archOpen: Bool?
    let archived: [Row]?
    // no live thread: its header shows faded, after the live ones
    let quiet: Bool?
}

// the folded shelf of older projects at the end of the list ("side-older"),
// and the active view's settled threads ("side-settled")
struct Older: Decodable, Equatable {
    let count: Int
    let open: Bool
}

// a row's or project's menu (a long press): each item sends action with value
struct MenuItem: Decodable, Hashable {
    let label, action, value, tone: String
}

struct RowMenu: Decodable, Equatable {
    let title: String
    // "t" a thread's, "p" a project's, "f" a file link's (shown in the thread)
    let kind: String?
    let items: [MenuItem]
}

// a toolbar or menu item: it sends action with value, or (a snooze)
// offers choices whose values it sends instead; danger asks first
struct Tool: Decodable, Hashable {
    let label, action: String
    // core/icons.bend's name for it (Views' icon(_:) turns it into a symbol)
    let icon: String?
    let on: Bool
    let value: String?
    let danger: Bool?
    let options: [SwipeChoice]?
}

// a markdown node from Md.render: an element with kids, or text
struct Block: Decodable, Hashable {
    let tag: String?
    let kids: [Block]?
    let text: String?
    // a link's target, and whether it is a file's (Mob.href)
    let href: String?
    let file: Bool?
    // an ordered item's number as written
    let n: String?

    var plain: String { text ?? (kids ?? []).map(\.plain).joined() }
}

// an attachment of a user message (url: where its hub serves it)
struct Chip: Decodable, Hashable {
    let label, path: String
    let image: Bool
    let url: String
}

// an image a reply names
struct Shot: Decodable, Hashable {
    let path, url: String
}

// kind: user, assistant, act (a tool line), fold (a run of tool calls:
// "fold" with value opens or shuts it), link (opens thread value)
struct Entry: Decodable, Identifiable {
    let id, kind, text: String
    let tone, label: String?
    let blocks: [Block]?
    let attachments: [Chip]?
    let images: [Shot]?
    let open: Bool?
    let value: String?
    // a terminal in the chat (kind "emb"): its item in the thread's embs
    let key: String?
    // a reminder (kind "reminder"): when, in the person's zone
    let when: String?
    // a message: when it was sent, or the turn finished (core/stamp.bend)
    let stamp: String?
}

// what the agent waits on the user for: an approval, a question or a
// plan; each button sends "answer" with its value
struct AskButton: Decodable, Hashable {
    let label, value: String
    let primary: Bool
    // what it sends ("" or none is "answer"; a browser card's Take over is "tk-takeover")
    let action: String?
}

struct Ask: Decodable, Identifiable {
    let id, kind, head, detail: String
    let blocks: [Block]
    let buttons: [AskButton]
    // a browser task's pause: the hub path of its screenshot
    let image: String?
}

// the subagent panel (src/core/subs.bend): what the thread delegated and
// the agent's own; a row sends act with value (a task opens its thread, a
// subagent opens or shuts its steps)
struct SubStep: Decodable, Hashable {
    let kind, text: String
}

struct SubRow: Decodable, Identifiable {
    let id, title, who, state, doing, act, value: String
    // the state as shown, when it differs from the token (Japanese)
    let stateLabel: String?
    let agent, live, open: Bool
    let n: Int
    let steps: [SubStep]
    // the action a Stop sends with the row's id ("" or absent: no Stop)
    let cancel: String?
}

struct Subs: Decodable {
    let busy: String
    let rows: [SubRow]
    // "3 at work" or "3 done"; open: the rows show (done, it folds to its
    // head); a tap on the head sends act with value
    let word: String?
    let open: Bool?
    let act: String?
    let value: String?
}

// a skill the `$` being typed may complete to ("skill" with its name)
struct Skill: Decodable, Hashable {
    let name, desc: String
    let on: Bool?
}

// a side question (/btw) and its answer, until closed
struct Btw: Decodable, Equatable {
    let q, a: String
}

// what the thread changed: k 0 context, 1 added, 2 removed, 3 meta, 4 a hunk head
struct DiffLine: Decodable, Hashable {
    let k: Int
    let t, o, n: String
}

struct DiffFile: Decodable, Identifiable {
    let name, status: String
    let lines: [DiffLine]
    var id: String { name }
}

struct Diff: Decodable {
    let summary: String
    let files: [DiffFile]
}

// the thread's shell: rows of styled runs (colours 0xRRGGBB) and the cursor
struct TermRun: Decodable, Hashable {
    let t: String
    let fg, bg: UInt32
    let b, u: Bool
}

struct TermCursor: Decodable {
    let x, y: Int
    let on: Bool
}

struct Term: Decodable {
    let title: String
    let fg, bg: UInt32
    let lines: [[TermRun]]
    let cursor: TermCursor
}

// a terminal in the chat (core/emb.bend): its key, size, whether it is live
// or pinned, the value "emb-restart" sends, and its screen while live
struct Emb: Decodable {
    let key, id, title: String
    let cols, rows: Int
    let live, pinned: Bool
    let restart: String
    let term: Term?
}

// the board viewer (src/mobile/view.bend): the source open ("" closed),
// the key its plots carry, the choices, and how to draw
struct Choice: Decodable, Hashable {
    let label, value: String
    let on: Bool
}

// a tapped item's card: its kind, then key = value rows, and the info
// "Mention in chat" puts in the draft
struct CardRow: Decodable, Hashable {
    let k, v: String
}

struct Card: Decodable {
    let info, title: String
    let rows: [CardRow]
}

// the key of the plot it draws layers from, how far a tap reaches
// (points), the layers a 3D view lays on the board's top and bottom faces,
// the board's colour before its model arrives, the field of view, the
// piece picked ("chunk,info" of the held chunks) and its card
struct Viewer: Decodable {
    let open, key, layers: String
    let choices: [Choice]
    let bg: UInt32
    let fade: Int
    let margin, zmin, zmax, tap: Float
    let top, bottom: [Int]
    let slab: UInt32
    let fov: Float
    let picked: String
    let card: Card?
    // the viewer's own light ground, and each layer's colour on it (by layer)
    let light: Bool?
    let look: [UInt32]?
    // the schematic's sheets ("view-sheet" value), the layers the user can
    // turn off ("view-layer" layer) and those off (a bit each), and
    // whether the 3D model shows its parts ("view-parts")
    let sheets: [SheetRow]?
    let layerList: [LayerRow]?
    let off: UInt32?
    // the layer focus (core/layers.bend): the layers drawn dimmed (a bit
    // each), the one drawn last (255 none), the list's All/Dim/Only (0-2)
    // and whether it has one (a board), and the Layers button's words
    let dim: UInt32?
    let lift: Int?
    let layerMode: Int?
    let layerModes: Bool?
    let layerHead: String?
    let parts: Bool?
    // what the hub says about the source (parts with no 3D model)
    let note: String?
    // the Mechanical page, when that is what is open
    let mech: MechPage?
    // the design history's bar under a board or schematic (nil: none)
    let hist: HistBar?
    // the Files tab (open "files"), and an image or a PDF it opened
    let files: FilesPage?
    let file: FileView?
    // the Review tab (open "review")
    let review: ReviewPage?
    // the review comments of the file on show
    let comments: ReviewComments?
}

// Review comments (client.bend's Rc): the pins, at places in mm of the
// file's coordinates ("rv-pin" id opens one), the one open, and the place
// waiting for its note ("x\ty\tlayer\titem", "" none)
struct ReviewPin: Decodable, Hashable {
    let id: String
    let x, y: Double
    let resolved: Bool
    let count: Int
}

struct ReviewReply: Decodable, Hashable {
    let text, by: String
}

struct ReviewOpen: Decodable, Hashable {
    let id, text, by, place: String
    let resolved: Bool
    let replies: [ReviewReply]
}

struct ReviewComments: Decodable {
    let pins: [ReviewPin]
    let open: ReviewOpen?
    let at: String
}

// the Checks page's boxes ("rv-tick" index) and why the gate fails ("" it passes)
struct ReviewTask: Decodable, Hashable {
    let index: Int
    let text, group: String
    let done: Bool
}

struct ReviewChecks: Decodable {
    let gate, line: String
    let tasks: [ReviewTask]
}

// the Review tab: the page chips ("rv-page" page), the title, the report's
// Markdown nodes (empty: say note), whether it is being made
struct ReviewTab: Decodable, Hashable {
    let page, label: String
    let on: Bool
}

struct ReviewPage: Decodable {
    let tabs: [ReviewTab]
    let title, note: String
    let busy: Bool
    let blocks: [Block]?
    let checks: ReviewChecks?
    let release: ReleaseForm?
}

// the Release page's form: the tag that will be made (the field, else the
// proposed one), the proposed one, and the two toggles
struct ReleaseForm: Decodable {
    let tag, next: String
    let publish, force: Bool
    let err: String
    let text: String?
}

// the Files tab: where the folder is, its rows (a tap sends action with
// value; "" when the file is only listed), whether the hub has listed it
struct FileCrumb: Decodable, Hashable {
    let label, action, value: String
}

struct FileRow: Decodable, Hashable {
    let label, action, value, detail, kind, icon: String
}

struct FilesPage: Decodable {
    let trail: [FileCrumb]
    let rows: [FileRow]
    let ready: Bool
}

// an image or a PDF from the Files tab: its name, "image" or "pdf", where
// the hub serves it (a path the app makes a URL of, AppModel.web)
struct FileView: Decodable {
    let name, kind, url: String
}

// The design history (src/mobile/view.bend's Hist.json): the steps in the
// track and the one shown (0 the live file), what to call it, what the
// agent said before it, playing, changes marked, a comparison on show and
// its sides' labels, the side whose version menu is open ("" shut), a tick
// per step then the live file (id ""), that menu, and the legend's
// colours (0xRRGGBB: removed, changed, added)
struct HistBar: Decodable, Equatable {
    enum CodingKeys: String, CodingKey {
        case n, at, title, entry, said, playing, diff, cmp, a, b, side, ticks, menu, keys
        case showing = "as"
    }
    let n, at: Int
    let title, entry, said: String
    let playing, diff, cmp: Bool
    let a, b, side: String
    // which of Old, Diff and New shows in a comparison (Rv.showing)
    let showing: String?
    let ticks: [HistTick]
    let menu: [HistRow]?
    let keys: [UInt32]
}

struct HistTick: Decodable, Hashable {
    let id: String
    let on: Bool
}

// a version the menu offers ("cmp-pick" value)
struct HistRow: Decodable, Hashable {
    let label, sub, value: String
}

// The Mechanical page (src/mobile/view.bend's Mech.json): what to say
// while there are no parts, the parts ("mech-part" value), the part on
// show (its path for "mech-3d"), its renders and what to say without any,
// and where to get FreeCAD when the hub has none
struct MechPage: Decodable {
    let say, note: String
    let parts: [Choice]
    let name, path: String
    let shots: [MechShot]
    let empty: String
    // the part's path from the project ("mech-render", "mech-renders"), a
    // render job running, the render button's label, why the last failed
    let rel: String?
    let busy: Bool?
    let render: String?
    let err: String?
}

struct MechShot: Decodable, Hashable {
    let view, url: String
}

struct SheetRow: Decodable, Hashable {
    let label, value: String
    let on, loop: Bool
}

struct LayerRow: Decodable, Hashable {
    let layer: Int
    let name: String
    let on: Bool
    // the active layer of the focus (core/layers.bend)
    let active: Bool?
}

// the composer's model chip: its label, the models ("model" sends one)
// and the efforts the current one takes ("effort")
struct ModelPicker: Decodable {
    let label: String
    let models, efforts: [Choice]
    // another provider takes the thread up from its next turn
    let providers: [Choice]?
}

// a message waiting in the thread's queue and its buttons (each sends
// action with value)
struct QButton: Decodable {
    let label, action, value: String
}

// the agent's todo list: "Todo 2/5" and a line per step
struct TodoLine: Decodable {
    let text, status: String
}

struct Todos: Decodable {
    let head: String
    let lines: [TodoLine]
    // open, and a tap on the head sends fold with toggle; more: a long
    // list's last line ("Show all 24", "Show fewer"; "" when it fits)
    let open: Bool?
    let toggle: String?
    let more: String?
    let moreValue: String?
}

struct QueueRow: Decodable {
    let msg, text, tag: String
    let buttons: [QButton]
}

struct ThreadView: Decodable {
    let id, title, branch, state: String
    // the thread's Boat chip ("Boat · large · running"; "" for none), a tap opens Settings
    let boat: String?
    let tools: [Tool]
    let entries: [Entry]
    // sent, not yet stored by the hub; and a message waiting for this turn
    let sending: [String]
    let queued: String
    let queue: [QueueRow]?
    let todos: Todos?
    // how many older entries the page leaves out ("earlier" shows more)
    let earlier: Int?
    let live: [Block]
    let working, draft, send: String
    // "interrupt" while a turn runs with nothing typed (the button is Stop)
    let sendAct: String?
    // the phase the working line's dot shows (as a row's status)
    let phase: String?
    let picker: ModelPicker
    let viewer: Viewer
    // the menu under the toolbar's ellipsis, after the tools
    let menu: [Tool]?
    let parent: Entry?
    let subs: Subs?
    let asks: [Ask]?
    let skills: [Skill]?
    let btw: Btw?
    // what the next message attaches, and what is still uploading (chunk:
    // the web's piece size; the app sends a file whole, POST /attach)
    let attaching: [Chip]?
    let uploading: String?
    let chunk: Int?
    // the most one attachment may hold, in bytes
    let cap: Int?
    let diff: Diff?
    let term: Term?
    // the thread's terminals in the chat (entries of kind "emb" name them)
    let embs: [Emb]?
}

// a delete a row asked for, waiting for yes ("row-delete" id) or no
struct Deleting: Decodable, Equatable {
    let id, title, body, yes, no: String
}

// the first-run tour's step on screen (core/onboard.bend): the counter
// ("3 of 8"), title and text, and the buttons' labels; actions "ob-skip",
// "ob-back" (not on the first step) and "ob-next"
struct Tour: Decodable, Equatable {
    let step, count: Int
    let counter, title, body, next, back, skip: String
    let first: Bool
    // the uberbot step's buttons: each sends its action and value as they are
    let choices: [TourChoice]
}

struct TourChoice: Decodable, Equatable {
    let label, action, value: String
}

// a thread being renamed: its id, the alert's title, the title so far, the
// buttons ("rename-save" with "id\u{1f}title", or "rename-no")
struct Renaming: Decodable, Equatable {
    let id, title, text, yes, no: String
}

// the list's search, always at its top: its query ("proj-find-q"), the hint
struct Search: Decodable, Equatable {
    let open: Bool
    let query, hint: String
}

// the project picker: its path field, the field's hint, an error, and the
// rows (a tap sends action with value; one with no action is only shown)
struct FolderRow: Decodable, Hashable {
    let label, action, value, kind: String
}

// settings: rows of a label, a note and buttons (each sends action with value)
struct SetButton: Codable, Hashable {
    let label, action, value: String
    let on: Bool
}

// a field to type in: each change goes out as "bfield" (name, text); a
// secret one shows dots
struct SetField: Codable, Hashable {
    let name, hint: String
    let secret: Bool
    let value: String
}

struct SetRow: Codable, Hashable {
    let label, note: String
    let buttons: [SetButton]
    let fields: [SetField]?
    // the pairing code's rows ("1" dark) while it is shown, and what to do with it
    let qr: [String]?
    let qrnote: String?
    // about this device, not the hub: App settings has it, the hub's page skips it
    let local: Bool?
    // notif-policy's "Notify me for": drawn in App settings' Notifications section, not on the hub's page
    let notify: Bool?
}

struct Settings: Codable, Equatable {
    let rows: [SetRow]
}

// thread search ("search") or the file picker ("files"): its query and rows
struct Find: Decodable {
    let mode, query: String
    let rows: [FolderRow]
}

struct Folders: Decodable {
    let text, hint, error: String
    let items: [FolderRow]
}

// a paired hub (key: its host:port), and a machine a hub knows of that
// this phone is not paired with yet
struct HubRow: Decodable, Hashable {
    let key, name: String
    let online: Bool
    // its version, and how many of its changes this phone holds
    let version: String?
    let changes: Int?
}

struct Found: Decodable, Hashable {
    let name, url: String
}

// Bots (src/mobile/bots.bend). A cat in the list: act ("bot", or
// "remote" for a bot only reported by a linked machine; older bridges
// leave it out and say remote) sends its id; mood and note say how it is;
// cat names its rig ("look:mood"), which the bridge gives once
// (AppModel.cats).
struct BotRow: Decodable, Identifiable {
    let nav: String?
    let id, name, mood, note, peer, cat: String
    let act: String?
    let look: Int
    let sel, remote: Bool
    let machine: String?
}

// a room in the list ("room" sends its id)
struct RoomRow: Decodable, Identifiable {
    let nav: String?
    let id, name: String
    let members: Int
    let sel: Bool
    let machine: String?
}

// the new bot form: its fields ("bfield" name, persona, provider),
// "bot-create" makes it, "form-close" "@bnew" drops it
struct ProviderChoice: Decodable, Hashable {
    let label, provider: String
    let on: Bool
}

struct NewBot: Decodable {
    let name, hint, persona, provider: String
    let providers: [ProviderChoice]
}

// the new room form: "bfield" rname, "room-pick" a bot's id, "room-save"
struct RoomPick: Decodable, Hashable {
    let id, name: String
    let on: Bool
}

struct NewRoom: Decodable {
    let name: String
    let picks: [RoomPick]
}

struct BotTab: Decodable, Hashable {
    let id, label: String
}

struct BotMemory: Decodable, Hashable {
    let key, kind, text, tags, updated: String
}

struct BotRoutine: Decodable, Hashable {
    let id, name, cron, when, prompt, last: String
    let on: Bool
    // routines-cal: routine, once or reminder; what it says next, in the person's zone
    let kind, label, next, rel, zone, doneText: String?
    let done: Bool?
}

// the routine form ("routine-edit" id, or "" for a new one, opens it)
struct RoutineForm: Decodable {
    let open: Bool
    let id, name, cron, prompt: String
    let hint: String?
}

struct BotHook: Decodable, Hashable {
    let id, name, path, last: String
    let count: Int
}

struct BotPeer: Decodable, Hashable {
    let id, name, url: String
}

// Google's sign-in state and the OAuth client's fields ("google" op)
struct BotGoogle: Decodable {
    let status, url, gid, gsecret, gpaste: String
    let accounts: [String]
    // the account whose signature is being edited ("" for none), its text, and each account's saved signature
    let sigFor, sigText: String
    let sigs: [String]
}

struct BotSettings: Decodable {
    let persona, personaField, invite, purl, join: String
    let google: BotGoogle
    let peers: [BotPeer]
}

// the hub's latest frame of the bot's page: n changes with every new one
struct BotBrowser: Decodable {
    let url, n: String
    // browser tasks (src/core/client.bend's Teach.*): the strip (mode 0 idle,
    // 1 recording, 2 save sheet, 3 the person has the browser), its buttons
    // (each sends its own action), whether the frame takes input ("bb-click"
    // "x y" in thousandths, "bb-scroll" dy, "bb-type", "bb-key", "bb-nav"
    // with the field bburl, "bb-back", "bb-reload"), the fields' text, the
    // sheet's rows, and the pauses waiting on the person
    let mode: Int?
    let title, clock, bburl, tknote, tkname, tkdesc: String?
    let steps: Int?
    let btns: [TBtn]?
    let input: Bool?
    let rows: [TStep]?
    let asks: [Ask]?
}

struct TBtn: Decodable, Hashable {
    let label, action, value: String
    let primary: Bool
}

struct TStep: Decodable, Identifiable, Hashable {
    let n: Int
    let text, chip, pname, field: String
    let secret, marked: Bool
    var id: Int { n }
}

// the Tasks tab: a card each (open: its run form, one field per param), then the recent runs
struct TaskParam: Decodable, Hashable {
    let field, label, example, value: String
}

struct TaskItem: Decodable, Identifiable, Hashable {
    let slug, name, desc, last: String
    let open, asking: Bool
    let params: [TaskParam]
    var id: String { slug }
}

struct TaskRun: Decodable, Identifiable, Hashable {
    let run, slug, status, summary: String
    let shots: [String]
    var id: String { run }
}

struct BotTasks: Decodable {
    let items: [TaskItem]
    let runs: [TaskRun]
}

struct BotPost: Decodable, Identifiable {
    let id, from, text, ago: String
    let mine: Bool
}

// what the main area shows when it is not a thread: a bot (kind "bot",
// its tab's content; the chat is the screen's thread), a room, or a bot
// on a linked machine ("remote")
struct BotView: Decodable {
    let kind, id, name: String
    // the name as shown: "Kit · desk" for a bot on a linked machine
    let title: String?
    let mood, note, tab, peer, members, draft, secret, secretFor, hname, cat: String?
    let look: Int?
    let tabs: [BotTab]?
    let space: SpaceModel?
    let page: SpacePageModel?
    let browser: BotBrowser?
    let tasks: BotTasks?
    let memory: [BotMemory]?
    let routines: [BotRoutine]?
    let routine: RoutineForm?
    let hooks: [BotHook]?
    let settings: BotSettings?
    let posts: [BotPost]?
}

struct HubIsland: Decodable {
    let hub: String
    let island: IslandAttributes.ContentState
}

struct Screen: Decodable {
    let locale: String?
    let online: Bool
    // hub: the one in focus (its thread is shown, its plots are drawn)
    let version, error, note, sel, empty, hub: String
    let hubs: [HubRow]
    let found: [Found]
    let projects: [Project]
    // "projects" (sections) or "active" (one flat list of rows, active)
    let lang: Settings?
    let view: String?
    // a thread this phone asked for is still to come (its page opens at once)
    let making: Bool?
    let active: [Row]?
    let older: Older?
    let settled: Older?
    let rowMenu: RowMenu?
    let bots: [BotRow]
    let rooms: [RoomRow]
    // the uberbot's card and the status plane (Plane.swift); none without an uberbot
    let uber: UberCard?
    let plane: PlaneModel?
    let newBot: NewBot?
    let newRoom: NewRoom?
    let bot: BotView?
    let deleting: Deleting?
    // a project remove to confirm ("proj-remove" id, or "proj-keep")
    let removing: Deleting?
    let renaming: Renaming?
    let tour: Tour?
    let search: Search?
    let folders: Folders?
    let settings: Settings?
    let find: Find?
    let island: IslandAttributes.ContentState
    // each paired hub's own island, for its Live Activity (hubs.bend's
    // Hubs.islands); island above is all of them, for the picker
    let islands: [HubIsland]?
    let thread: ThreadView?
    // the hub's theme ("light", "dark"; empty follows the phone's): read only
    // once, to carry it into App settings (AppSettings.migrate)
    let theme: String?
    // the tab bar's badges and the routines (src/mobile/tabs.bend)
    let tabs: TabsInfo?
}

struct TabBadge: Decodable {
    let badge: Int
}

// a routine of any bot, as a row of the Routines tab: a tap opens its bot's
// Routines tab (nav, act, value, as a plane row's)
struct RoutineRow: Decodable, Identifiable {
    let nav: String?
    let id, act, value, title, sub, state, ago: String
    let bot, machine: String?
    let away: Bool?
    // the kind of schedule (a recurring routine, a one-time one, a reminder), its
    // label ("Once", "Reminder"…), the time zone it runs in, and a done one's line
    let sched, label, zone, doneText: String?
}

struct RoutinesInfo: Decodable {
    let badge: Int
    let rows: [RoutineRow]
}

struct TabsInfo: Decodable {
    let home, bots: TabBadge?
    let routines: RoutinesInfo?
}

struct Cmd: Decodable {
    let type: String
    let text: String?
    // send: the CBOR frame, as base64, for the hub keyed hub
    let data, hub: String?
    // notify; key is shared by alerts about the same item (src/core/notice.bend)
    let thread, title, kind, body, key: String?
    // later: an action (with its value) to send after ms; jump: the entry to show
    let ms: Double?
    let action, value, id: String?
}

struct Resume: Decodable {
    let since, origin: String
}

struct Out: Decodable, @unchecked Sendable {
    let screen: Screen?
    let cmds: [Cmd]

    private struct Cmds: Decodable {
        let cmds: [Cmd]
    }

    // an answer; with screen false only its commands (a newer screen follows)
    // The commands are decoded on their own: they carry the frames the hubs
    // wait for, so a screen Swift cannot read never drops them (the last
    // screen stays up, and the failure is logged with its path)
    static func decode(_ text: String, screen: Bool) -> Out? {
        let d = Data(text.utf8)
        let cmds: [Cmd]
        do { cmds = try JSONDecoder().decode(Cmds.self, from: d).cmds } catch {
            NSLog("Backplane answer unreadable: %@", String(describing: error))
            return nil
        }
        guard screen else { return Out(screen: nil, cmds: cmds) }
        do {
            let s = try JSONDecoder().decode(ScreenOnly.self, from: d)
            return Out(screen: s.screen, cmds: cmds)
        } catch {
            NSLog("Backplane screen unreadable: %@", String(describing: error))
            return Out(screen: nil, cmds: cmds)
        }
    }

    private struct ScreenOnly: Decodable {
        let screen: Screen?
    }
}

// One value that may fail to decode on its own (nil then), so one bad item
// or section never takes the screen around it with it
struct Lossy<T: Decodable>: Decodable {
    let value: T?
    init(from decoder: Decoder) throws {
        do { value = try T(from: decoder) } catch {
            NSLog("Backplane skipped %@ at %@: %@", String(describing: T.self),
                  decoder.codingPath.map(\.stringValue).joined(separator: "."), String(describing: error))
            value = nil
        }
    }
}

extension KeyedDecodingContainer {
    // a section: absent, null or unreadable is nil
    func lossy<T: Decodable>(_ key: Key) -> T? {
        ((try? decodeIfPresent(Lossy<T>.self, forKey: key)) ?? nil)?.value
    }
    // a list: absent, null or unreadable is empty; unreadable items are skipped
    func lossyList<T: Decodable>(_ key: Key) -> [T] {
        (((try? decodeIfPresent([Lossy<T>].self, forKey: key)) ?? nil) ?? []).compactMap(\.value)
    }
}

// The screen as Bend sends it (src/mobile/screen.bend). Every field is read
// on its own: a field Bend leaves out or a shape this build does not know
// costs that field, never the whole screen (a blank app).
extension Screen {
    private enum K: String, CodingKey {
        case locale, online, version, error, note, sel, empty, hub, hubs, found, projects, lang, view, making, active, older, settled,
             rowMenu, bots, rooms, uber, plane, newBot, newRoom, bot, deleting, removing, renaming, tour, search,
             folders, settings, find, island, islands, thread, theme, tabs
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        locale = c.lossy(.locale)
        online = c.lossy(.online) ?? false
        version = c.lossy(.version) ?? ""
        error = c.lossy(.error) ?? ""
        note = c.lossy(.note) ?? ""
        sel = c.lossy(.sel) ?? ""
        empty = c.lossy(.empty) ?? ""
        hub = c.lossy(.hub) ?? ""
        hubs = c.lossyList(.hubs)
        found = c.lossyList(.found)
        projects = c.lossyList(.projects)
        lang = c.lossy(.lang)
        view = c.lossy(.view)
        making = c.lossy(.making)
        active = c.lossy(.active).map { (l: [Lossy<Row>]) in l.compactMap(\.value) }
        older = c.lossy(.older)
        settled = c.lossy(.settled)
        rowMenu = c.lossy(.rowMenu)
        bots = c.lossyList(.bots)
        rooms = c.lossyList(.rooms)
        uber = c.lossy(.uber)
        plane = c.lossy(.plane)
        newBot = c.lossy(.newBot)
        newRoom = c.lossy(.newRoom)
        bot = c.lossy(.bot)
        deleting = c.lossy(.deleting)
        removing = c.lossy(.removing)
        renaming = c.lossy(.renaming)
        tour = c.lossy(.tour)
        search = c.lossy(.search)
        folders = c.lossy(.folders)
        settings = c.lossy(.settings)
        find = c.lossy(.find)
        island = c.lossy(.island) ?? IslandAttributes.ContentState(running: 0, headline: "", lines: [])
        islands = c.lossy(.islands).map { (l: [Lossy<HubIsland>]) in l.compactMap(\.value) }
        thread = c.lossy(.thread)
        theme = c.lossy(.theme)
        tabs = c.lossy(.tabs)
    }
}
