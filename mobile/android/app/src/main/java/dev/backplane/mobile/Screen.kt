package dev.backplane.mobile

import org.json.JSONArray
import org.json.JSONObject

// The screen src/mobile/screen.bend emits. Plain data; no decisions.

// a swipe button: the action it sends with its value, or (a snooze)
// choices whose values it sends instead
data class SwipeChoice(val label: String, val value: String)

data class Swipe(val label: String, val action: String, val value: String, val tone: String, val options: List<SwipeChoice>)

// status: what its dot says (approval, input, working, monitoring, failed,
// queued, complete, stopped, idle)
data class Row(
    val id: String, val title: String, val state: String, val ago: String, val pinned: Boolean,
    val lead: List<Swipe>, val trail: List<Swipe>, val status: String = "",
    // the active view's rows: the project's name, and faded (settled lately)
    val project: String = "", val faded: Boolean = false,
    // "2 agents" while subagents work for it, shown where the age goes
    val agents: String = "",
    // Visible hierarchy; native indentation is capped at four levels.
    val depth: Int = 0, val expanded: Boolean = false, val children: Int = 0,
    val canChild: Boolean = false,
)

// machine: the hub it is on, named when the phone has several
data class Project(
    val id: String, val title: String, val root: String, val machine: String, val open: Boolean,
    val threads: List<Row>, val snoozedShelf: String, val snoozed: List<Row>,
    val shelf: String, val settled: List<Row>,
    // the Archived shelf: "toggle-settled" with value opens or shuts it
    val value: String = "", val archOpen: Boolean = false, val archived: List<Row> = emptyList(),
    // no live thread: its header shows faded, after the live ones
    val quiet: Boolean = false,
)

// the folded shelf of older projects at the end of the list ("side-older"),
// and the active view's settled threads ("side-settled")
data class Older(val count: Int, val open: Boolean)

// a row's or project's menu (a long press): each item sends action with value
data class RowMenuItem(val label: String, val action: String, val value: String, val tone: String)
data class RowMenu(val title: String, val items: List<RowMenuItem>)

// a delete a row asked for, waiting for yes ("row-delete" id) or no
data class Deleting(val id: String, val title: String, val body: String, val yes: String, val no: String)
// a thread being renamed: its id, the dialog's title, the title so far, the
// buttons ("rename-save" with "id\u001ftitle", or "rename-no")
data class Renaming(val id: String, val title: String, val text: String, val yes: String, val no: String)
// the first-run tour's step on screen (core/onboard.bend): the counter
// ("3 of 8"), title and text, the buttons' labels; actions "ob-skip",
// "ob-back" (not on the first step) and "ob-next"
data class Tour(val counter: String, val title: String, val body: String, val next: String, val back: String, val skip: String, val first: Boolean,
    // the uberbot step's buttons: each sends its action and value as they are
    val choices: List<TourChoice> = emptyList())

data class TourChoice(val label: String, val action: String, val value: String)

// the list's search, always at its top: its query ("proj-find-q"), the hint
data class Search(val open: Boolean, val query: String, val hint: String)

// the project picker: its path field, the field's hint, an error, and the
// rows (a tap sends action with value; one with no action is only shown);
// entry: a search result's message, shown once its thread opens
data class FolderRow(val label: String, val action: String, val value: String, val kind: String, val entry: String = "")

data class Folders(val text: String, val hint: String, val error: String, val items: List<FolderRow>)

// a toolbar or menu item: it sends action with value, or (a snooze)
// offers choices whose values it sends instead; danger asks first
data class Tool(
    val label: String, val action: String, val on: Boolean,
    val value: String = "", val danger: Boolean = false, val options: List<SwipeChoice> = emptyList(),
    // core/icons.bend's name for it (toolIcon turns it into a Material icon)
    val icon: String = "",
)

sealed interface Block {
    // a link's target, and whether it is a file's (Mob.href)
    // n: an ordered item's number as written
    data class El(val tag: String, val kids: List<Block>, val href: String? = null, val file: Boolean = false, val n: String? = null) : Block
    data class Txt(val text: String) : Block
}

// an attachment of a user message (url: where its hub serves it)
data class Chip(val label: String, val path: String, val image: Boolean, val url: String)

// an image a reply names
data class Shot(val path: String, val url: String)

// kind: user, assistant, act (a tool line), fold (a run of tool calls:
// "fold" with value opens or shuts it), link (opens thread value), emb (a
// terminal in the chat: key names its item in the thread's embs)
data class Entry(
    val id: String, val kind: String, val text: String,
    val tone: String, val label: String, val blocks: List<Block>,
    val attachments: List<Chip> = emptyList(), val images: List<Shot> = emptyList(),
    val open: Boolean = false, val value: String = "", val key: String = "", val at: String = "",
    val stamp: String = "",
    val partial: MessagePartial? = null,
)

// what the agent waits on the user for: an approval, a question or a
// plan; each button sends "answer" with its value
// action: what the button sends ("" is "answer"; a browser card's Take over is "tk-takeover")
data class AskButton(val label: String, val value: String, val primary: Boolean, val action: String = "")
// image: the hub path of a browser task's screenshot at the pause ("" for none)
data class Ask(
    val id: String, val kind: String, val head: String, val detail: String, val blocks: List<Block>, val buttons: List<AskButton>,
    val image: String = "", val risky: Boolean = false,
)

// the subagent panel (src/core/subs.bend): what the thread delegated and
// the agent's own; a row sends act with value (a task opens its thread, a
// subagent opens or shuts its steps)
data class SubStep(val kind: String, val text: String)
data class SubRow(
    val id: String, val agent: Boolean, val title: String, val who: String, val state: String, val doing: String,
    val live: Boolean, val open: Boolean, val n: Int, val steps: List<SubStep>, val act: String, val value: String,
    // the action a Stop sends with the row's id ("" for no Stop)
    val cancel: String = "",
    // the state as shown, when it differs from the token (Japanese)
    val stateLabel: String = "",
)
// word: "3 at work" or "3 done"; open: the rows show (done, it folds to its
// head); a tap on the head sends act with value
data class Subs(val busy: String, val rows: List<SubRow>, val word: String = "", val open: Boolean = true, val act: String = "", val value: String = "")

// a skill the `$` being typed may complete to ("skill" with its name)
data class Skill(val name: String, val desc: String, val on: Boolean = false)

// what the thread changed: k 0 context, 1 added, 2 removed, 3 meta, 4 a hunk head
data class DiffLine(val k: Int, val t: String, val o: String, val n: String)
data class DiffFile(val name: String, val status: String, val lines: List<DiffLine>)
data class Diff(val summary: String, val files: List<DiffFile>)

// the thread's shell: rows of styled runs (colours 0xRRGGBB) and the cursor
data class TermRun(val t: String, val fg: Int, val bg: Int, val b: Boolean, val u: Boolean)
data class TermCursor(val x: Int, val y: Int, val on: Boolean)
data class Term(val title: String, val fg: Int, val bg: Int, val lines: List<List<TermRun>>, val cursor: TermCursor)

// a terminal in the chat (core/emb.bend): its key, size, whether it is live
// or pinned, the value "emb-restart" sends, and its screen while live
data class Emb(
    val key: String, val id: String, val title: String, val cols: Int, val rows: Int,
    val live: Boolean, val pinned: Boolean, val restart: String, val term: Term?,
)

// settings: rows of a label, a note and buttons (each sends action with value)
data class SetButton(val label: String, val action: String, val value: String, val on: Boolean)
// a field to type in: each change goes out as "bfield" (name, text); a
// secret one shows dots
// vault: the secret box's value field (screen.bend's Mob.sbox), kept by the
// app and handed over only at Save (Core.vaultText), never sent as typed
data class SetField(val name: String, val hint: String, val secret: Boolean, val value: String, val vault: Boolean = false)
// qr: the pairing code's rows ('1' dark) while it is shown
data class UsageDay(val date: String, val reading: String, val known: Boolean, val fraction: Int)
data class SetRow(val label: String, val note: String, val buttons: List<SetButton>, val fields: List<SetField> = emptyList(),
                  val qr: List<String> = emptyList(), val qrnote: String = "",
                  // about this device, not the hub: App settings has it, the hub's page skips it
                  val local: Boolean = false,
                  // notif-policy's "Notify me for": drawn in App settings' Notifications section, not on the hub's page
                  val notify: Boolean = false, val chart: List<UsageDay> = emptyList())
data class Settings(val rows: List<SetRow>)

// the tab bar's badges and every bot's routines (src/mobile/tabs.bend)
data class RoutineRow(
    val nav: String, val id: String, val act: String, val value: String, val title: String, val sub: String,
    val state: String, val ago: String, val bot: String, val machine: String, val away: Boolean,
    // a one-time routine or reminder, the time zone it runs in, and a done one's line
    val sched: String = "", val zone: String = "", val doneText: String = "",
)
data class Tabs(val home: Int = 0, val bots: Int = 0, val routines: Int = 0, val routineRows: List<RoutineRow> = emptyList())

// thread search ("search") or the file picker ("files"): its query and rows
data class Find(val mode: String, val query: String, val rows: List<FolderRow>)

// the board viewer (src/mobile/view.bend): the source open ("" closed),
// the Review tab: page chips ("rv-page" page), the title, the report's
// Markdown nodes (empty: say note), whether it is being made
data class ReviewTab(val page: String, val label: String, val on: Boolean)
data class ReviewPage(val tabs: List<ReviewTab>, val title: String, val note: String, val busy: Boolean, val blocks: List<Block>, val checks: ReviewChecks? = null, val release: ReleaseForm? = null)

// the Release page's form: the tag that will be made (the field, else the proposed one), the proposed one, the two toggles, why not ("")
data class ReleaseForm(val tag: String, val next: String, val publish: Boolean, val force: Boolean, val err: String, val text: String = "")

// the Checks page's boxes ("rv-tick" index) and why the gate fails ("" it passes)
data class ReviewTask(val index: Int, val text: String, val group: String, val done: Boolean)
data class ReviewChecks(val gate: String, val line: String, val tasks: List<ReviewTask>)

// Review comments (client.bend's Rc): the pins, at places in mm of the
// file's coordinates ("rv-pin" id opens one), the one open, and the place
// waiting for its note ("x\ty\tlayer\titem", "" none)
data class ReviewPin(val id: String, val x: Float, val y: Float, val resolved: Boolean, val count: Int)
data class ReviewReply(val text: String, val by: String)
data class ReviewOpen(val id: String, val text: String, val by: String, val place: String, val resolved: Boolean, val replies: List<ReviewReply>)
data class ReviewComments(val pins: List<ReviewPin>, val open: ReviewOpen?, val at: String)

// the key its plots carry, the choices, and how to draw
data class Choice(val label: String, val value: String, val on: Boolean)

// a tapped item's card: its kind, then key = value rows, and the info
// "Mention in chat" puts in the draft
data class Card(val info: String, val title: String, val rows: List<Pair<String, String>>)

// also: the key of the plot it draws layers from, how far a tap reaches
// (dp), the layers a 3D view lays on the board's top and bottom faces, the
// board's colour before its model arrives, the field of view, the piece
// picked ("chunk,info" of the held chunks) and its card
data class Viewer(
    val open: String, val key: String, val layers: String, val choices: List<Choice>, val bg: Int, val fade: Float,
    val margin: Float, val zmin: Float, val zmax: Float, val tap: Float, val top: IntArray, val bottom: IntArray,
    val slab: Int, val fov: Float, val picked: String, val card: Card?,
    // the viewer's own light ground, and each layer's colour on it (by layer)
    val light: Boolean = false, val look: IntArray = IntArray(0),
    // the schematic's sheets ("view-sheet" value), the layers the user can
    // turn off ("view-layer" layer) and those off (a bit each), and
    // whether the 3D model shows its parts ("view-parts")
    val sheets: List<Sheet> = emptyList(), val layerList: List<LayerRow> = emptyList(), val off: Int = 0, val parts: Boolean = true,
    // what the hub says about the source (parts with no 3D model)
    val note: String = "",
    // the Mechanical page, when that is what is open
    val mech: MechPage? = null,
    // the design history's bar under a board or schematic (null: none)
    val hist: HistBar? = null,
    // the Files tab (open "files"), and an image or a PDF it opened
    val files: FilesPage? = null, val file: FileView? = null,
    // the Review tab (open "review")
    val review: ReviewPage? = null,
    val comments: ReviewComments? = null,
    // the layer focus (core/layers.bend): the layers drawn dimmed (a bit
    // each), the one drawn last (255 none), the list's All/Dim/Only (0-2)
    // and whether it has one (a board), and the Layers button's words
    val dim: Int = 0, val lift: Int = 255, val layerMode: Int = 0, val layerModes: Boolean = false, val layerHead: String = "Layers",
)

// the Files tab: where the folder is, its rows (a tap sends action with
// value; "" when the file is only listed), whether the hub has listed it
data class FileCrumb(val label: String, val action: String, val value: String)
data class FileRow(val label: String, val action: String, val value: String, val detail: String, val kind: String, val icon: String)
data class FilesPage(val trail: List<FileCrumb>, val rows: List<FileRow>, val ready: Boolean)
// an image or a PDF from the Files tab: its name, "image" or "pdf", where
// the hub serves it (a path the app makes a URL of, Core.web)
data class FileView(val name: String, val kind: String, val url: String)

// The design history (src/mobile/view.bend's Hist.json): the steps in the
// track and the one shown (0 the live file), what to call it, what the
// agent said before it, playing, changes marked, a comparison on show and
// its sides' labels, the side whose version menu is open ("" shut), a tick
// per step then the live file (id ""), that menu (null shut), and the
// legend's colours (0xRRGGBB: removed, changed, added)
data class HistBar(
    val n: Int, val at: Int, val title: String, val entry: String, val said: String,
    val playing: Boolean, val diff: Boolean, val cmp: Boolean, val a: String, val b: String, val side: String,
    val ticks: List<HistTick>, val menu: List<HistRow>?, val keys: IntArray,
    // which of Old, Diff and New shows in a comparison (Rv.showing)
    val showing: String = "",
)

data class HistTick(val id: String, val on: Boolean)

// a version the menu offers ("cmp-pick" value)
data class HistRow(val label: String, val sub: String, val value: String)

// The Mechanical page (src/mobile/view.bend's Mech.json): what to say
// while there are no parts, the parts ("mech-part" value), the part on
// show (its path for "mech-3d"), its renders and what to say without any,
// and where to get FreeCAD when the hub has none
data class MechPage(
    val say: String, val note: String, val parts: List<Choice>, val name: String, val path: String,
    val shots: List<MechShot>, val empty: String,
    // the part's path from the project ("mech-render", "mech-renders"), a
    // render job running, the render button's label, why the last failed
    val rel: String = "", val busy: Boolean = false, val render: String = "", val err: String = "",
)

data class MechShot(val view: String, val url: String)

// box: its sheet symbol on the sheet on show ("x0 y0 x1 y1" mm) when it is
// directly under it; a tap there opens it (mobile/view.bend)
data class Sheet(val label: String, val value: String, val on: Boolean, val loop: Boolean, val box: String = "")

data class LayerRow(val layer: Int, val name: String, val on: Boolean, val active: Boolean = false)

// the composer's model chip: its label, the models ("model" sends one)
// and the efforts the current one takes ("effort")
data class ModelPicker(val label: String, val models: List<Choice>, val efforts: List<Choice>, val providers: List<Choice> = emptyList())

// a message waiting in the thread's queue and its buttons (each sends
// action with value)
data class QButton(val label: String, val action: String, val value: String)
data class QueueRow(val msg: String, val text: String, val tag: String, val buttons: List<QButton>)
// the agent's todo list: "Todo 2/5" and a line per step
data class TodoLine(val text: String, val status: String)
// the todo list in the top bar beside the subagents: its count ("2/5"); a
// tap sends fold with toggle, and only while open (shut until the person
// opens it) its lines show under the bar; more: a long list's last line
// ("Show all 24", "Show fewer"; "" when it fits)
data class TodoItem(val id: String, val text: String, val status: String, val scope: String, val check: String, val delete: String, val up: String, val down: String)
data class Todos(val head: String, val lines: List<TodoLine>, val open: Boolean = false, val toggle: String = "", val more: String = "", val moreValue: String = "", val count: String = "", val items: List<TodoItem>? = null, val addScope: String = "")

data class ThreadView(
    val id: String, val title: String, val branch: String, val state: String,
    val tools: List<Tool>, val entries: List<Entry>, val live: List<Block>,
    val working: String, val draft: String, val send: String,
    // "interrupt" while a turn runs with nothing typed (the button is Stop)
    val sendAct: String,
    val sending: List<String>, val queued: String, val picker: ModelPicker, val viewer: Viewer,
    val queue: List<QueueRow> = emptyList(),
    val todos: Todos? = null,
    // how many older entries the page leaves out (the "earlier" action shows more)
    val earlier: Int = 0,
    // how many newer pages lie below the one shown (paged up into the
    // history; the "later" action shows the next one down)
    val later: Int = 0,
    // the menu under the toolbar's overflow, after the tools
    val menu: List<Tool> = emptyList(),
    val parent: Entry? = null,
    val subs: Subs? = null,
    val asks: List<Ask> = emptyList(),
    val skills: List<Skill> = emptyList(),
    // a side question (/btw) and its answer, until closed
    val btw: Btw? = null,
    // what the next message attaches, and what is still uploading (chunk:
    // the web's piece size; the app sends a file whole, POST /attach)
    val attaching: List<Chip> = emptyList(),
    val uploading: String = "",
    val chunk: Int = 196_608,
    // the most one attachment may hold, in bytes
    val cap: Int = 10_485_760,
    val diff: Diff? = null,
    val term: Term? = null,
    // the thread's terminals in the chat (entries of kind "emb" name them)
    val embs: List<Emb> = emptyList(),
    // the phase the working line's dot shows (as a row's status)
    val phase: String = "",
    val livePartial: MessagePartial? = null,
    // the thread's Boat chip ("Boat · large · running"; "" for none), a tap opens Settings
    val boat: String = "",
    val canChild: Boolean = false,
)

data class Btw(val q: String, val a: String)

data class IslandLine(val thread: String, val title: String, val doing: String)

data class Island(val running: Int, val headline: String, val lines: List<IslandLine>)

// a paired hub (key: its host:port), and a machine a hub knows of that
// this phone is not paired with yet
data class HubRow(val key: String, val name: String, val online: Boolean)

data class Found(val name: String, val url: String)

// Bots (src/mobile/bots.bend). A cat in the list: act ("bot", or
// "remote" for a bot only reported by a linked machine) sends its id;
// mood and note say how it is; cat names its rig ("look:mood"), which the
// bridge gives once (Core.cats).
data class BotRow(
    val id: String, val name: String, val mood: String, val note: String, val peer: String, val cat: String,
    val look: Int, val sel: Boolean, val remote: Boolean, val machine: String, val act: String, val nav: String = "",
)

// a room in the list ("room" sends its id)
data class RoomRow(val id: String, val name: String, val members: Int, val sel: Boolean, val machine: String, val nav: String = "")

// the new bot form: its fields ("bfield" name, persona, provider),
// "bot-create" makes it, "form-close" "@bnew" drops it
data class ProviderChoice(val label: String, val provider: String, val on: Boolean)
data class NewBot(val name: String, val persona: String, val provider: String, val providers: List<ProviderChoice>, val hint: String = "Name")

// the new room form: "bfield" rname, "room-pick" a bot's id, "room-save"
data class RoomPick(val id: String, val name: String, val on: Boolean)
data class NewRoom(val name: String, val picks: List<RoomPick>)

data class BotTab(val id: String, val label: String)
data class BotMemory(val key: String, val kind: String, val text: String, val tags: String, val updated: String)
// schedule: the cron line described ("when")
data class BotRoutine(
    val id: String, val name: String, val cron: String, val schedule: String, val prompt: String,
    val last: String, val on: Boolean,
    // routines-cal: routine, once or reminder; what it says next, in the person's zone
    val kind: String = "routine", val label: String = "Repeats", val next: String = "", val rel: String = "",
    val zone: String = "your time", val done: Boolean = false, val doneText: String = "",
)
// the routine form ("routine-edit" id, or "" for a new one, opens it)
data class RoutineForm(val open: Boolean, val id: String, val name: String, val cron: String, val prompt: String, val hint: String = "")
data class BotHook(val id: String, val name: String, val path: String, val last: String, val count: Int)
data class BotPeer(val id: String, val name: String, val url: String)
// Google's sign-in state and the OAuth client's fields ("google" op)
data class BotGoogle(val status: String, val url: String, val gid: String, val gsecret: String, val gpaste: String, val accounts: List<String>, val sigFor: String, val sigText: String)
data class BotSettings(
    val persona: String, val personaField: String, val invite: String, val purl: String, val join: String,
    val google: BotGoogle, val peers: List<BotPeer>,
)
// Browser tasks (src/core/client.bend's Teach.*). The strip: mode 0 idle,
// 1 recording, 2 review (the save sheet), 3 the person has the browser; its
// buttons send their own action. While `input` the frame takes the
// person's taps ("bb-click" "x y" in thousandths), drags ("bb-scroll" dy),
// text ("bb-type"), keys ("bb-key"), the address ("bb-nav", field bburl),
// "bb-back" and "bb-reload".
data class TBtn(val label: String, val action: String, val value: String, val primary: Boolean)
data class TStep(val n: Int, val text: String, val chip: String, val secret: Boolean, val marked: Boolean, val pname: String, val field: String)
// the hub's latest frame of the bot's page: n changes with every new one
data class BotBrowser(
    val url: String, val n: String, val mode: Int = 0, val title: String = "", val clock: String = "", val steps: Int = 0,
    val btns: List<TBtn> = emptyList(), val input: Boolean = false, val addr: String = "", val note: String = "",
    val name: String = "", val desc: String = "", val rows: List<TStep> = emptyList(), val asks: List<Ask> = emptyList(),
)
// the Tasks tab: a card each (open: its run form, one field per param), then the recent runs
data class TaskParam(val field: String, val label: String, val example: String, val value: String)
data class TaskItem(val slug: String, val name: String, val desc: String, val open: Boolean, val asking: Boolean, val last: String, val params: List<TaskParam>)
data class TaskRun(val run: String, val slug: String, val status: String, val summary: String, val shots: List<String>)
data class BotTasks(val items: List<TaskItem>, val runs: List<TaskRun>)
data class BotPost(val id: String, val from: String, val text: String, val ago: String, val mine: Boolean)

// what the main area shows when it is not a thread: a bot (kind "bot",
// its tab's content; the chat is the screen's thread), a room, or a bot
// on a linked machine ("remote")
data class BotView(
    val kind: String, val id: String, val name: String, val mood: String, val note: String, val tab: String,
    val peer: String, val members: String, val draft: String, val secret: String, val secretFor: String,
    val cat: String, val tabs: List<BotTab>, val space: SpaceModel?, val page: SpacePageModel?, val browser: BotBrowser?,
    val memory: List<BotMemory>, val routines: List<BotRoutine>, val routine: RoutineForm?,
    val hooks: List<BotHook>, val settings: BotSettings?, val posts: List<BotPost>,
    // the name as shown: "Kit · desk" for a bot on a linked machine
    val title: String,
    val tasks: BotTasks? = null,
    val navTab: String = "", val subtabs: List<BotTab> = emptyList(),
    val mailroom: MailroomModel? = null, val mailroomLoading: Boolean = false,
)

// hub: the one in focus (its thread is shown, its plots are drawn)
data class Screen(
    val online: Boolean, val version: String, val error: String, val note: String,
    val sel: String, val empty: String, val projects: List<Project>, val thread: ThreadView?,
    val island: Island, val deleting: Deleting?, val folders: Folders?,
    val hub: String, val hubs: List<HubRow>, val found: List<Found>,
    val bots: List<BotRow> = emptyList(), val rooms: List<RoomRow> = emptyList(),
    val newBot: NewBot? = null, val newRoom: NewRoom? = null, val bot: BotView? = null,
    val settings: Settings? = null, val find: Find? = null,
    // a project remove to confirm ("proj-remove" id, or "proj-keep")
    val removing: Deleting? = null, val search: Search? = null,
    // shared language labels and choices for this device
    val lang: Settings? = null,
    // the hub's theme ("light", "dark"; "" follows the phone's)
    val theme: String = "",
    // "projects" (sections) or "active" (one flat list of rows, active)
    val view: String = "projects", val active: List<Row> = emptyList(),
    val older: Older? = null, val settled: Older? = null, val rowMenu: RowMenu? = null,
    val renaming: Renaming? = null,
    val tour: Tour? = null,
    // a thread this phone asked for is still to come (its page opens at once)
    val making: Boolean = false,
    // the uberbot's card and the status plane (Plane.kt); none without an uberbot
    val uber: UberCard? = null, val plane: PlaneModel? = null,
    val tabs: Tabs = Tabs(),
    val locale: String = "en",
    // Legacy conversations remain selectable outside project sections.
    val legacy: List<Row> = emptyList(),
    val bubMove: BubMove? = null,
)

data class Cmd(
    val type: String, val text: String,
    // a "send": the CBOR frame, as base64, for the hub keyed hub
    val data: String = "", val hub: String = "",
    // a "notify": the thread whose turn ended, its title, "done"/"fail", what to say, and the
    // key alerts about the same item share
    val thread: String = "", val title: String = "", val kind: String = "", val body: String = "",
    val key: String = "",
    // a browser task paused: the hub path of its screenshot, for the banner
    val image: String = "",
    // a "later": an action (with its value) to send after ms; a "jump": the entry to show
    val ms: Long = 0, val action: String = "", val value: String = "", val id: String = "",
)

// an answer from the engine: its screen (none from a quiet call, or when a
// newer one follows) and its commands
data class SyncUpdate(val hub: String, val phase: String, val detail: String = "", val mode: String = "",
    val viewId: String = "", val viewRevision: String = "", val requestRevision: Long = 0, val remoteReady: Boolean? = null) {
    val thinPresenting get() = phase == "presenting" && detail == "view-v1" && mode == "view-v1" && remoteReady == false &&
        requestRevision in 1..4294967295L
    val thinPresented get() = phase == "presented" && detail == "view-v1" && mode == "view-v1" && remoteReady == true &&
        viewId.isNotEmpty() && viewRevision.isNotEmpty() && requestRevision in 1..4294967295L
}
data class Reply(val screen: Screen?, val cmds: List<Cmd>, val screenJSON: String? = null,
                 val sync: List<SyncUpdate> = emptyList(), val deadline: Long? = null)

fun parseSync(o: JSONObject): List<SyncUpdate> {
    val rows = o.optJSONArray("sync") ?: return emptyList()
    return (0 until rows.length()).map { rows.getJSONObject(it).let { row -> SyncUpdate(row.optString("hub"), row.optString("phase"), row.opt("detail") as? String ?: "", row.opt("mode") as? String ?: "", row.opt("viewId") as? String ?: "", row.opt("viewRevision") as? String ?: "", (row.opt("requestRevision").takeIf { it is Int || it is Long } as? Number)?.toLong() ?: 0, row.opt("remoteReady") as? Boolean) } }
}

private fun <T> JSONArray?.map(f: (JSONObject) -> T): List<T> =
    if (this == null) emptyList() else (0 until length()).map { f(getJSONObject(it)) }

private fun blocks(a: JSONArray?): List<Block> = a.map { o ->
    if (o.has("tag")) Block.El(o.getString("tag"), blocks(o.optJSONArray("kids")), if (o.has("href")) o.optString("href") else null, o.optBoolean("file"),
        if (o.has("n")) o.optString("n") else null)
    else Block.Txt(o.optString("text"))
}

private fun swipe(o: JSONObject) = Swipe(
    o.optString("label"), o.optString("action"), o.optString("value"), o.optString("tone"),
    o.optJSONArray("options").map { SwipeChoice(it.optString("label"), it.optString("value")) },
)

private fun row(o: JSONObject) = Row(
    o.optString("id"), o.optString("title"), o.optString("state"), o.optString("ago"), o.optBoolean("pinned"),
    o.optJSONArray("lead").map(::swipe), o.optJSONArray("trail").map(::swipe), o.optString("status"),
    o.optString("project"), o.optBoolean("faded"), o.optString("agents"),
    o.optInt("depth"), o.optBoolean("expanded"), o.optInt("children"), o.optBoolean("canChild"),
)

private fun chips(a: JSONArray?) =
    a.map { Chip(it.optString("label"), it.optString("path"), it.optBoolean("image"), it.optString("url")) }

private fun entry(o: JSONObject) = Entry(
    o.optString("id"), o.optString("kind"), o.optString("text"), o.optString("tone"),
    o.optString("label"), blocks(o.optJSONArray("blocks")),
    chips(o.optJSONArray("attachments")),
    o.optJSONArray("images").map { Shot(it.optString("path"), it.optString("url")) },
    o.optBoolean("open"), o.optString("value"), o.optString("key"), o.optString("when"),
    o.optString("stamp"), o.optJSONObject("partial")?.let(::messagePartial),
)

private fun tool(o: JSONObject) = Tool(
    o.optString("label"), o.optString("action"), o.optBoolean("on"), o.optString("value"), o.optBoolean("danger"),
    o.optJSONArray("options").map { SwipeChoice(it.optString("label"), it.optString("value")) },
    o.optString("icon"),
)

private fun term(o: JSONObject) = Term(
    o.optString("title"), o.optInt("fg"), o.optInt("bg"),
    o.optJSONArray("lines").let { a ->
        if (a == null) emptyList() else (0 until a.length()).map { i ->
            a.optJSONArray(i).map { TermRun(it.optString("t"), it.optInt("fg"), it.optInt("bg"), it.optBoolean("b"), it.optBoolean("u")) }
        }
    },
    (o.optJSONObject("cursor") ?: JSONObject()).let { TermCursor(it.optInt("x"), it.optInt("y"), it.optBoolean("on")) },
)

private fun folderRows(a: JSONArray?) =
    a.map { FolderRow(it.optString("label"), it.optString("action"), it.optString("value"), it.optString("kind"), it.optString("entry")) }

private fun thread(o: JSONObject) = threadOf(o).copy(
    earlier = o.optInt("earlier", 0),
    later = o.optInt("later", 0),
    menu = o.optJSONArray("menu").map(::tool),
    parent = o.optJSONObject("parent")?.let(::entry),
    subs = o.optJSONObject("subs")?.let { u ->
        Subs(u.optString("busy"), u.optJSONArray("rows").map {
            SubRow(it.optString("id"), it.optBoolean("agent"), it.optString("title"), it.optString("who"), it.optString("state"),
                it.optString("doing"), it.optBoolean("live"), it.optBoolean("open"), it.optInt("n"),
                it.optJSONArray("steps").map { st -> SubStep(st.optString("kind"), st.optString("text")) }, it.optString("act"), it.optString("value"),
                it.optString("cancel"), it.optString("stateLabel"))
        }, u.optString("word"), u.optBoolean("open", true), u.optString("act"), u.optString("value"))
    },
    asks = o.optJSONArray("asks").map(::askOf),
    skills = o.optJSONArray("skills").map { Skill(it.optString("name"), it.optString("desc"), it.optBoolean("on")) },
    btw = o.optJSONObject("btw")?.let { Btw(it.optString("q"), it.optString("a")) },
    attaching = chips(o.optJSONArray("attaching")),
    uploading = o.optString("uploading"),
    chunk = o.optInt("chunk", 196_608),
    cap = o.optInt("cap", 10_485_760),
    diff = o.optJSONObject("diff")?.let { d ->
        Diff(d.optString("summary"), d.optJSONArray("files").map { f ->
            DiffFile(f.optString("name"), f.optString("status"),
                f.optJSONArray("lines").map { DiffLine(it.optInt("k"), it.optString("t"), it.optString("o"), it.optString("n")) })
        })
    },
    term = o.optJSONObject("term")?.let(::term),
    embs = o.optJSONArray("embs").map {
        Emb(it.optString("key"), it.optString("id"), it.optString("title"), it.optInt("cols", 80), it.optInt("rows", 24),
            it.optBoolean("live"), it.optBoolean("pinned"), it.optString("restart"), it.optJSONObject("term")?.let(::term))
    },
    phase = o.optString("phase"),
    livePartial = o.optJSONObject("livePartial")?.let(::messagePartial),
    boat = o.optString("boat"),
)

private fun threadOf(o: JSONObject) = ThreadView(
    o.optString("id"), o.optString("title"), o.optString("branch"), o.optString("state"),
    o.optJSONArray("tools").map(::tool),
    o.optJSONArray("entries").map(::entry),
    blocks(o.optJSONArray("live")), o.optString("working"), o.optString("draft"), o.optString("send"),
    o.optString("sendAct", "send"), strs(o.optJSONArray("sending")), o.optString("queued"), picker(o.optJSONObject("picker") ?: JSONObject()),
    viewer(o.optJSONObject("viewer") ?: JSONObject()),
    o.optJSONArray("queue").map {
        QueueRow(it.optString("msg"), it.optString("text"), it.optString("tag"),
            it.optJSONArray("buttons").map { b -> QButton(b.optString("label"), b.optString("action"), b.optString("value")) })
    },
    o.optJSONObject("todos")?.let { td -> Todos(td.optString("head"), td.optJSONArray("lines").map { TodoLine(it.optString("text"), it.optString("status")) },
        td.optBoolean("open", false), td.optString("toggle"), td.optString("more"), td.optString("moreValue"), td.optString("count"),
            if (td.has("items")) td.optJSONArray("items").map { TodoItem(it.optString("id"), it.optString("text"), it.optString("status"), it.optString("scope"), it.optString("check"), it.optString("delete"), it.optString("up"), it.optString("down")) } else null, td.optString("addScope")) },
    canChild = o.optBoolean("canChild"),
)

private fun choices(a: JSONArray?) = a.map { Choice(it.optString("label"), it.optString("value"), it.optBoolean("on")) }

private fun picker(o: JSONObject) =
    ModelPicker(o.optString("label"), choices(o.optJSONArray("models")), choices(o.optJSONArray("efforts")), choices(o.optJSONArray("providers")))

private fun ints(a: JSONArray?): IntArray = if (a == null) IntArray(0) else IntArray(a.length()) { a.optInt(it) }

private fun viewer(o: JSONObject) = Viewer(
    o.optString("open"), o.optString("key"), o.optString("layers"),
    o.optJSONArray("choices").map { Choice(it.optString("label"), it.optString("value"), it.optBoolean("on")) },
    o.optInt("bg"), o.optDouble("fade", 220.0).toFloat(), o.optDouble("margin", 0.9).toFloat(),
    o.optDouble("zmin", 0.5).toFloat(), o.optDouble("zmax", 0.5).toFloat(), o.optDouble("tap", 14.0).toFloat(),
    ints(o.optJSONArray("top")), ints(o.optJSONArray("bottom")), o.optInt("slab"), o.optDouble("fov", 35.0).toFloat(),
    o.optString("picked"),
    o.optJSONObject("card")?.let { c ->
        Card(c.optString("info"), c.optString("title"), c.optJSONArray("rows").map { it.optString("k") to it.optString("v") })
    },
    o.optBoolean("light"), ints(o.optJSONArray("look")),
    o.optJSONArray("sheets").map { Sheet(it.optString("label"), it.optString("value"), it.optBoolean("on"), it.optBoolean("loop"), it.optString("box")) },
    o.optJSONArray("layerList").map { LayerRow(it.optInt("layer"), it.optString("name"), it.optBoolean("on"), it.optBoolean("active")) },
    o.optInt("off"), o.optBoolean("parts", true), o.optString("note"),
    o.optJSONObject("mech")?.let { p ->
        MechPage(p.optString("say"), p.optString("note"), choices(p.optJSONArray("parts")), p.optString("name"), p.optString("path"),
            p.optJSONArray("shots").map { MechShot(it.optString("view"), it.optString("url")) }, p.optString("empty"),
            p.optString("rel"), p.optBoolean("busy"), p.optString("render"), p.optString("err"))
    },
    o.optJSONObject("hist")?.let { h ->
        HistBar(h.optInt("n"), h.optInt("at"), h.optString("title"), h.optString("entry"), h.optString("said"),
            h.optBoolean("playing"), h.optBoolean("diff"), h.optBoolean("cmp"), h.optString("a"), h.optString("b"), h.optString("side"),
            h.optJSONArray("ticks").map { HistTick(it.optString("id"), it.optBoolean("on")) },
            h.optJSONArray("menu")?.let { a -> a.map { HistRow(it.optString("label"), it.optString("sub"), it.optString("value")) } },
            ints(h.optJSONArray("keys")), h.optString("as"))
    },
    files = o.optJSONObject("files")?.let { f ->
        FilesPage(f.optJSONArray("trail").map { FileCrumb(it.optString("label"), it.optString("action"), it.optString("value")) },
            f.optJSONArray("rows").map { FileRow(it.optString("label"), it.optString("action"), it.optString("value"), it.optString("detail"), it.optString("kind"), it.optString("icon")) },
            f.optBoolean("ready"))
    },
    file = o.optJSONObject("file")?.let { FileView(it.optString("name"), it.optString("kind"), it.optString("url")) },
    review = o.optJSONObject("review")?.let { r ->
        ReviewPage(r.optJSONArray("tabs").map { ReviewTab(it.optString("page"), it.optString("label"), it.optBoolean("on")) },
            r.optString("title"), r.optString("note"), r.optBoolean("busy"), blocks(r.optJSONArray("blocks")),
            r.optJSONObject("checks")?.let { c ->
                ReviewChecks(c.optString("gate"), c.optString("line"),
                    c.optJSONArray("tasks").map { ReviewTask(it.optInt("index"), it.optString("text"), it.optString("group"), it.optBoolean("done")) })
            },
            r.optJSONObject("release")?.let { f ->
                ReleaseForm(f.optString("tag"), f.optString("next"), f.optBoolean("publish"), f.optBoolean("force"), f.optString("err"), f.optString("text"))
            })
    },
    comments = o.optJSONObject("comments")?.let { c ->
        ReviewComments(c.optJSONArray("pins").map { ReviewPin(it.optString("id"), it.optDouble("x").toFloat(), it.optDouble("y").toFloat(), it.optBoolean("resolved"), it.optInt("count")) },
            c.optJSONObject("open")?.let { p ->
                ReviewOpen(p.optString("id"), p.optString("text"), p.optString("by"), p.optString("place"), p.optBoolean("resolved"),
                    p.optJSONArray("replies").map { ReviewReply(it.optString("text"), it.optString("by")) })
            }, c.optString("at"))
    },
    dim = o.optInt("dim"), lift = o.optInt("lift", 255), layerMode = o.optInt("layerMode"), layerModes = o.optBoolean("layerModes"),
    layerHead = o.optString("layerHead", "Layers"),
)

private fun strs(a: JSONArray?): List<String> =
    if (a == null) emptyList() else (0 until a.length()).map { a.optString(it) }

private fun island(o: JSONObject?) = Island(
    o?.optInt("running") ?: 0, o?.optString("headline") ?: "",
    o?.optJSONArray("lines").map { IslandLine(it.optString("thread"), it.optString("title"), it.optString("doing")) },
)

private fun askOf(a: JSONObject) =
    Ask(a.optString("id"), a.optString("kind"), a.optString("head"), a.optString("detail"), blocks(a.optJSONArray("blocks")),
        a.optJSONArray("buttons").map { AskButton(it.optString("label"), it.optString("value"), it.optBoolean("primary"), it.optString("action")) },
        a.optString("image"), a.optBoolean("risky"))

private fun botBrowser(o: JSONObject) = BotBrowser(
    o.optString("url"), o.optString("n"), o.optInt("mode"), o.optString("title"), o.optString("clock"), o.optInt("steps"),
    o.optJSONArray("btns").map { TBtn(it.optString("label"), it.optString("action"), it.optString("value"), it.optBoolean("primary")) },
    o.optBoolean("input"), o.optString("bburl"), o.optString("tknote"), o.optString("tkname"), o.optString("tkdesc"),
    o.optJSONArray("rows").map {
        TStep(it.optInt("n"), it.optString("text"), it.optString("chip"), it.optBoolean("secret"), it.optBoolean("marked"),
            it.optString("pname"), it.optString("field"))
    },
    o.optJSONArray("asks").map(::askOf),
)

private fun botTasks(o: JSONObject) = BotTasks(
    o.optJSONArray("items").map { t ->
        TaskItem(t.optString("slug"), t.optString("name"), t.optString("desc"), t.optBoolean("open"), t.optBoolean("asking"), t.optString("last"),
            t.optJSONArray("params").map { TaskParam(it.optString("field"), it.optString("label"), it.optString("example"), it.optString("value")) })
    },
    o.optJSONArray("runs").map { r -> TaskRun(r.optString("run"), r.optString("slug"), r.optString("status"), r.optString("summary"), strs(r.optJSONArray("shots"))) },
)

private fun botView(o: JSONObject) = BotView(
    o.optString("kind"), o.optString("id"), o.optString("name"), o.optString("mood"), o.optString("note"),
    o.optString("tab", "chat"), o.optString("peer"), o.optString("members"), o.optString("draft"),
    o.optString("secret"), o.optString("secretFor"), o.optString("cat"),
    o.optJSONArray("tabs").map { BotTab(it.optString("id"), it.optString("label")) },
    o.optJSONObject("space")?.let(::spaceModel),
    o.optJSONObject("page")?.let { SpacePageModel(it.optString("bot"), it.optString("url"), it.optString("n")) },
    o.optJSONObject("browser")?.let(::botBrowser),
    o.optJSONArray("memory").map {
        BotMemory(it.optString("key"), it.optString("kind"), it.optString("text"), it.optString("tags"), it.optString("updated"))
    },
    o.optJSONArray("routines").map {
        BotRoutine(it.optString("id"), it.optString("name"), it.optString("cron"), it.optString("when"), it.optString("prompt"),
            it.optString("last"), it.optBoolean("on"), it.optString("kind", "routine"), it.optString("label", "Repeats"), it.optString("next"),
            it.optString("rel"), it.optString("zone", "your time"), it.optBoolean("done"), it.optString("doneText"))
    },
    o.optJSONObject("routine")?.let {
        RoutineForm(it.optBoolean("open"), it.optString("id"), it.optString("name"), it.optString("cron"), it.optString("prompt"), it.optString("hint"))
    },
    o.optJSONArray("hooks").map {
        BotHook(it.optString("id"), it.optString("name"), it.optString("path"), it.optString("last"), it.optInt("count"))
    },
    o.optJSONObject("settings")?.let { st ->
        val g = st.optJSONObject("google") ?: JSONObject()
        BotSettings(st.optString("persona"), st.optString("personaField"), st.optString("invite"), st.optString("purl"),
            st.optString("join"),
            BotGoogle(g.optString("status"), g.optString("url"), g.optString("gid"), g.optString("gsecret"), g.optString("gpaste"), strs(g.optJSONArray("accounts")), g.optString("sigFor"), g.optString("sigText")),
            st.optJSONArray("peers").map { BotPeer(it.optString("id"), it.optString("name"), it.optString("url")) })
    },
    o.optJSONArray("posts").map {
        BotPost(it.optString("id"), it.optString("from"), it.optString("text"), it.optString("ago"), it.optBoolean("mine"))
    },
    o.optString("title", o.optString("name")),
    o.optJSONObject("tasks")?.let(::botTasks),
    mailroom = o.optJSONObject("mailroom")?.let(::mailroomModel),
    mailroomLoading = o.optBoolean("mailroomLoading"),
    navTab = o.optString("navTab", o.optString("tab", "chat")),
    subtabs = o.optJSONArray("subtabs").map { BotTab(it.optString("id"), it.optString("label")) },
)

fun parseScreen(o: JSONObject) = Screen(
    o.optBoolean("online"), o.optString("version"), o.optString("error"), o.optString("note"),
    o.optString("sel"), o.optString("empty"),
    o.optJSONArray("projects").map {
        Project(it.optString("id"), it.optString("title"), it.optString("root"), it.optString("machine"), it.optBoolean("open"),
            it.optJSONArray("threads").map(::row), it.optString("snoozedShelf"), it.optJSONArray("snoozed").map(::row),
            it.optString("shelf"), it.optJSONArray("settled").map(::row),
            it.optString("value"), it.optBoolean("archOpen"), it.optJSONArray("archived").map(::row), it.optBoolean("quiet"))
    },
    o.optJSONObject("thread")?.let(::thread),
    island(o.optJSONObject("island")),
    o.optJSONObject("deleting")?.let {
        Deleting(it.optString("id"), it.optString("title"), it.optString("body"), it.optString("yes"), it.optString("no"))
    },
    o.optJSONObject("folders")?.let { p ->
        Folders(p.optString("text"), p.optString("hint"), p.optString("error"),
            folderRows(p.optJSONArray("items")))
    },
    o.optString("hub"),
    o.optJSONArray("hubs").map { HubRow(it.optString("key"), it.optString("name"), it.optBoolean("online")) },
    o.optJSONArray("found").map { Found(it.optString("name"), it.optString("url")) },
    o.optJSONArray("bots").map {
        BotRow(it.optString("id"), it.optString("name"), it.optString("mood"), it.optString("note"), it.optString("peer"),
            it.optString("cat"), it.optInt("look"), it.optBoolean("sel"), it.optBoolean("remote"), it.optString("machine"),
            it.optString("act", if (it.optBoolean("remote")) "remote" else "bot"), it.optString("nav"))
    },
    o.optJSONArray("rooms").map {
        RoomRow(it.optString("id"), it.optString("name"), it.optInt("members"), it.optBoolean("sel"), it.optString("machine"), it.optString("nav"))
    },
    o.optJSONObject("newBot")?.let { f ->
        NewBot(f.optString("name"), f.optString("persona"), f.optString("provider"),
            f.optJSONArray("providers").map { ProviderChoice(it.optString("label"), it.optString("provider"), it.optBoolean("on")) },
            f.optString("hint", "Name"))
    },
    o.optJSONObject("newRoom")?.let { f ->
        NewRoom(f.optString("name"), f.optJSONArray("picks").map { RoomPick(it.optString("id"), it.optString("name"), it.optBoolean("on")) })
    },
    o.optJSONObject("bot")?.let(::botView),
    o.optJSONObject("settings")?.let(::parseSettings),
    o.optJSONObject("find")?.let { Find(it.optString("mode"), it.optString("query"), folderRows(it.optJSONArray("rows"))) },
    o.optJSONObject("removing")?.let {
        Deleting(it.optString("id"), it.optString("title"), it.optString("body"), it.optString("yes"), it.optString("no"))
    },
    o.optJSONObject("search")?.let { Search(it.optBoolean("open"), it.optString("query"), it.optString("hint")) },
    lang = o.optJSONObject("lang")?.let(::parseSettings),
    theme = o.optString("theme"),
    view = o.optString("view").ifEmpty { "projects" },
    making = o.optBoolean("making"),
    tabs = o.optJSONObject("tabs")?.let(::parseTabs) ?: Tabs(),
    uber = o.optJSONObject("uber")?.let(::parseUber),
    plane = o.optJSONObject("plane")?.let(::parsePlane),
    active = o.optJSONArray("active").map(::row),
    legacy = o.optJSONArray("legacy").map(::row),
    bubMove = o.optJSONObject("bubMove")?.let(::parseBubMove),
    older = o.optJSONObject("older")?.let { Older(it.optInt("count"), it.optBoolean("open")) },
    settled = o.optJSONObject("settled")?.let { Older(it.optInt("count"), it.optBoolean("open")) },
    rowMenu = o.optJSONObject("rowMenu")?.let { mn ->
        RowMenu(mn.optString("title"), mn.optJSONArray("items").map {
            RowMenuItem(it.optString("label"), it.optString("action"), it.optString("value"), it.optString("tone"))
        })
    },
    renaming = o.optJSONObject("renaming")?.let {
        Renaming(it.optString("id"), it.optString("title"), it.optString("text"), it.optString("yes"), it.optString("no"))
    },
    tour = o.optJSONObject("tour")?.let {
        Tour(it.optString("counter"), it.optString("title"), it.optString("body"), it.optString("next"), it.optString("back"), it.optString("skip"), it.optBoolean("first"),
            it.optJSONArray("choices")?.let { a -> (0 until a.length()).map { i -> a.getJSONObject(i).let { c -> TourChoice(c.optString("label"), c.optString("action"), c.optString("value")) } } } ?: emptyList())
    },
    locale = o.optString("locale", "en"),
)

fun parseSettings(st: JSONObject) = Settings(st.optJSONArray("rows").map { r ->
    SetRow(r.optString("label"), r.optString("note"),
        r.optJSONArray("buttons").map { SetButton(it.optString("label"), it.optString("action"), it.optString("value"), it.optBoolean("on")) },
        r.optJSONArray("fields").map { SetField(it.optString("name"), it.optString("hint"), it.optBoolean("secret"), it.optString("value"), it.optBoolean("vault")) },
        r.optJSONArray("qr")?.let { q -> List(q.length()) { q.optString(it) } } ?: emptyList(), r.optString("qrnote"), r.optBoolean("local"), r.optBoolean("notify"),
        r.optJSONArray("chart").map { UsageDay(it.optString("date"), it.optString("reading"), it.optBoolean("known"), it.optInt("fraction")) })
})

// settings written back as the JSON the screen carries them in (the hub page's cache)
fun settingsJson(st: Settings): JSONObject = JSONObject().put("rows", JSONArray(st.rows.map { r ->
    JSONObject().put("label", r.label).put("note", r.note).put("local", r.local).put("notify", r.notify).put("qrnote", r.qrnote)
        .put("buttons", JSONArray(r.buttons.map { JSONObject().put("label", it.label).put("action", it.action).put("value", it.value).put("on", it.on) }))
        .put("fields", JSONArray(r.fields.map { JSONObject().put("name", it.name).put("hint", it.hint).put("secret", it.secret).put("value", it.value).put("vault", it.vault) }))
        .put("chart", JSONArray(r.chart.map { JSONObject().put("date", it.date).put("reading", it.reading).put("known", it.known).put("fraction", it.fraction) }))
        .put("qr", JSONArray(r.qr))
}))

fun parseTabs(o: JSONObject) = Tabs(
    o.optJSONObject("home")?.optInt("badge") ?: 0, o.optJSONObject("bots")?.optInt("badge") ?: 0,
    o.optJSONObject("routines")?.optInt("badge") ?: 0,
    o.optJSONObject("routines")?.optJSONArray("rows").map {
        RoutineRow(it.optString("nav"), it.optString("id"), it.optString("act"), it.optString("value"), it.optString("title"), it.optString("sub"),
            it.optString("state"), it.optString("ago"), it.optString("bot"), it.optString("machine"), it.optBoolean("away"),
            it.optString("sched"), it.optString("zone"), it.optString("doneText"))
    },
)

fun parseCmds(o: JSONObject): List<Cmd> =
    o.optJSONArray("cmds").map {
        Cmd(it.optString("type"), it.optString("text"), it.optString("data"), it.optString("hub"),
            it.optString("thread"), it.optString("title"), it.optString("kind"), it.optString("body"), it.optString("key"), it.optString("image"),
            it.optDouble("ms", 0.0).toLong(), it.optString("action"), it.optString("value"), it.optString("id"))
    }
