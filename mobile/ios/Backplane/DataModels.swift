import Foundation

// docs/PHONE-DATA.md (data/1): the plain data the hub sends, as typed values. Decoding is forgiving (a missing field is empty, an extra one ignored)
// so a hub that grows a field never breaks an older phone; everything here is also what the disk cache stores.

extension KeyedDecodingContainer {
    func str(_ k: Key) -> String { (try? decode(String.self, forKey: k)) ?? "" }
    func int(_ k: Key) -> Int {
        if let v = try? decode(Int.self, forKey: k) { return v }
        if let d = try? decode(Double.self, forKey: k), d.isFinite { return Int(d) }
        return 0
    }
    func bool(_ k: Key) -> Bool { (try? decode(Bool.self, forKey: k)) ?? false }
}

struct DOption: Codable, Equatable, Identifiable {
    var id: String
    var label: String
    enum CodingKeys: String, CodingKey { case id, label }
    init(id: String, label: String) { self.id = id; self.label = label }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); id = c.str(.id); label = c.str(.label).isEmpty ? c.str(.id) : c.str(.label) }
}

struct DTool: Codable, Equatable {
    var name: String
    var summary: String
    var state: String   // running | done | failed
    var sub: String = ""    // an Agent call's subagent (feature subview): `thread.sub {thread, key: sub}` opens its view
    var result: String = "" // a subagent's tool call: its result (in a subagent's view)
    enum CodingKeys: String, CodingKey { case name, summary, state, sub, result }
    init(name: String, summary: String, state: String) { self.name = name; self.summary = summary; self.state = state }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); name = c.str(.name); summary = c.str(.summary); state = c.str(.state); sub = c.str(.sub); result = c.str(.result) }
}

struct DAsk: Codable, Equatable {
    var id: String
    var state: String   // open | answered
    var question: String
    var options: [DOption]
    var answered: String?
    var isOpen: Bool { state == "open" }
    enum CodingKeys: String, CodingKey { case id, state, question, options, answered }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); state = c.str(.state); question = c.str(.question)
        options = (try? c.decode([DOption].self, forKey: .options)) ?? []
        answered = try? c.decode(String.self, forKey: .answered)
    }
}

struct DAttachment: Codable, Equatable, Identifiable {
    var id: String
    var name: String
    var mime: String
    var bytes: Int
    enum CodingKeys: String, CodingKey { case id, name, mime, bytes }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); id = c.str(.id); name = c.str(.name); mime = c.str(.mime); bytes = c.int(.bytes) }
}

struct DEntry: Codable, Equatable, Identifiable {
    var id: String
    var kind: String    // user | assistant | tool | ask | note | step
    var text: String
    var at: Int
    var label: String = ""   // a design step: what changed; a note: its name
    var tone: String = ""    // a note: error, info…
    var clientId: String?
    var replaces: String?   // the final entry that ends a stream names the live entry it replaces
    var live: Bool
    var agentOnly = false   // a message only the agent reads (data/1 `agentOnly`): never shown
    var tool: DTool?
    var ask: DAsk?
    var term: DTermInfo?   // a chat terminal (feature `terms`, kind "term")
    var handoff: DHandoff?   // a `/give` received (kind "handoff"): its card
    var attachments: [DAttachment] = []
    // phone-only, never on the wire: a message sent and not yet confirmed, or one the hub refused
    var sending = false
    var failed = false
    var localImages: [Data] = []   // the pictures of a message this phone is sending (JPEG), shown before the hub has them
    enum CodingKeys: String, CodingKey { case id, kind, text, at, label, tone, clientId, replaces, live, agentOnly, tool, ask, term, handoff, attachments }
    init(id: String, kind: String, text: String, at: Int, clientId: String? = nil, live: Bool = false) {
        self.id = id; self.kind = kind; self.text = text; self.at = at; self.clientId = clientId; self.live = live
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); kind = c.str(.kind); text = c.str(.text); at = c.int(.at); label = c.str(.label); tone = c.str(.tone)
        clientId = try? c.decode(String.self, forKey: .clientId)
        replaces = try? c.decode(String.self, forKey: .replaces)
        live = c.bool(.live)
        agentOnly = c.bool(.agentOnly)
        tool = try? c.decode(DTool.self, forKey: .tool)
        ask = try? c.decode(DAsk.self, forKey: .ask)
        term = try? c.decode(DTermInfo.self, forKey: .term)
        handoff = try? c.decode(DHandoff.self, forKey: .handoff)
        attachments = (try? c.decode([DAttachment].self, forKey: .attachments)) ?? []
    }
}

/// A `/give` received (docs/PHONE-DATA.md, Handoffs): where it came from, the person's note, the summary and what the work touched; a reference opens by `link.resolve {url, thread}`.
struct DHandoff: Codable, Equatable {
    var from: String; var title: String; var note: String; var summary: String; var refs: [DRef]
    enum CodingKeys: String, CodingKey { case from, title, note, summary, refs }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); from = c.str(.from); title = c.str(.title); note = c.str(.note); summary = c.str(.summary); refs = (try? c.decode([DRef].self, forKey: .refs)) ?? [] }
}
struct DRef: Codable, Equatable {
    var kind: String; var label: String; var url: String; var thread: String
    enum CodingKeys: String, CodingKey { case kind, label, url, thread }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); kind = c.str(.kind); label = c.str(.label); url = c.str(.url); thread = c.str(.thread) }
}

/// Where a project or thread of another machine runs (docs/PHONE-DATA.md, Your machines): `short` on a row, `name` in the header, `label` "on desktop" after a project's name,
/// `runs` "runs on desktop" over the composer; `away` draws it dimmer.
struct DMark: Codable, Equatable {
    var name: String; var short: String; var away: Bool; var label: String; var runs: String
    enum CodingKeys: String, CodingKey { case name, short, away, label, runs }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); name = c.str(.name); short = c.str(.short); away = c.bool(.away); label = c.str(.label); runs = c.str(.runs) }
}
/// Where a new thread may start: `value` is the project's id there (this machine's, or `<link>~<id>`).
struct DNewOn: Codable, Equatable, Hashable {
    var value: String; var label: String; var away: Bool
    enum CodingKeys: String, CodingKey { case value, label, away }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); value = c.str(.value); label = c.str(.label); away = c.bool(.away) }
}
/// A chip of the machine switcher over the lists: "" every machine, "@here" this one, else a linked machine's name.
struct DChip: Codable, Equatable, Identifiable {
    var value: String; var label: String; var on: Bool; var away: Bool; var hint: String
    var id: String { value }
    enum CodingKeys: String, CodingKey { case value, label, on, away, hint }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); value = c.str(.value); label = c.str(.label); on = c.bool(.on); away = c.bool(.away); hint = c.str(.hint) }
}

struct DProject: Codable, Equatable, Identifiable {
    var id: String
    var title: String
    var root: String
    var threadCount: Int
    var settledCount: Int?   // the hub's count of its settled threads (feature `settled`); nil from a hub before it
    var updatedAt: Int
    var waiting: Int
    var icon: String   // the badge the hub names: an icon and its colour as "#rrggbb"
    var css: String
    var mark: DMark?         // another machine's project: where it is
    var also: String         // this machine's project that is also on a linked machine: "also on desktop"
    var newOn: [DNewOn]      // where a new thread can start, when more than one machine has this project
    enum CodingKeys: String, CodingKey { case id, title, root, threadCount, settledCount, updatedAt, waiting, icon, css, mark, also, newOn }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); title = c.str(.title); root = c.str(.root); threadCount = c.int(.threadCount); settledCount = try? c.decode(Int.self, forKey: .settledCount); updatedAt = c.int(.updatedAt); waiting = c.int(.waiting)
        icon = c.str(.icon); css = c.str(.css)
        mark = try? c.decode(DMark.self, forKey: .mark); also = c.str(.also); newOn = (try? c.decode([DNewOn].self, forKey: .newOn)) ?? []
    }
}

struct DThread: Codable, Equatable, Identifiable, NestedThread {
    var id: String
    var project: String
    var title: String
    var state: String   // idle | working | asking | failed
    var updatedAt: Int
    var pinned: Bool
    var settled: Bool
    var settledAt: Int   // when it was settled, unix ms; 0 when it is not
    var archived: Bool
    var provider: String
    var model: String
    var effort: String
    var runtime: String
    var chip: String    // the composer's "claude / opus / high", in the hub's words
    var preview: String
    var waiting: Int
    var parent: String   // the thread it is a subthread of; "" at the top level
    var history: Bool    // an old Scratch chat (feature history): listed under Conversation history, never under a project
    var status: String   // the sidebar dot's word (feature status): finished is the desktop's green dot (a turn done, not yet settled); "" from an older hub
    var mark: DMark?     // the machine it runs on when it is another one; nil for this hub's own
    enum CodingKeys: String, CodingKey { case id, project, title, state, updatedAt, pinned, settled, settledAt, archived, provider, model, effort, runtime, chip, preview, waiting, parent, history, status, mark }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); project = c.str(.project); title = c.str(.title); state = c.str(.state); updatedAt = c.int(.updatedAt)
        pinned = c.bool(.pinned); settled = c.bool(.settled); settledAt = c.int(.settledAt); archived = c.bool(.archived)
        provider = c.str(.provider); model = c.str(.model); effort = c.str(.effort); runtime = c.str(.runtime); chip = c.str(.chip); preview = c.str(.preview); waiting = c.int(.waiting); parent = c.str(.parent); history = c.bool(.history); status = c.str(.status); mark = try? c.decode(DMark.self, forKey: .mark)
    }
}

// replies
struct DProjectsPage: Decodable { var projects: [DProject]; var next: String?; var machines: [DChip]?
    enum CodingKeys: String, CodingKey { case projects, next, machines }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); projects = (try? c.decode([DProject].self, forKey: .projects)) ?? []; next = try? c.decode(String.self, forKey: .next); machines = try? c.decode([DChip].self, forKey: .machines) } }
struct DThreadsPage: Decodable { var threads: [DThread]; var next: String?; var machines: [DChip]?
    enum CodingKeys: String, CodingKey { case threads, next, machines }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); threads = (try? c.decode([DThread].self, forKey: .threads)) ?? []; next = try? c.decode(String.self, forKey: .next); machines = try? c.decode([DChip].self, forKey: .machines) } }
struct DEntriesPage: Decodable { var thread: DThread?; var entries: [DEntry]; var next: String?
    enum CodingKeys: String, CodingKey { case thread, entries, next }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); thread = try? c.decode(DThread.self, forKey: .thread); entries = (try? c.decode([DEntry].self, forKey: .entries)) ?? []; next = try? c.decode(String.self, forKey: .next) } }
struct DProjectReply: Decodable { var project: DProject }
struct DThreadReply: Decodable { var thread: DThread }
struct DEntryReply: Decodable { var entry: DEntry }
struct DDirs: Decodable { var path: String; var parent: String; var dirs: [String]
    enum CodingKeys: String, CodingKey { case path, parent, dirs }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); path = c.str(.path); parent = c.str(.parent); dirs = (try? c.decode([String].self, forKey: .dirs)) ?? [] } }
struct DSubReply: Decodable { var sub: Int
    enum CodingKeys: String, CodingKey { case sub }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); sub = c.int(.sub) } }
/// `badge.list`: the tints and icons a project may take.
struct DBadgeList: Decodable {
    struct Tint: Decodable { var name: String; var css: String }
    var colors: [Tint]; var icons: [String]
}
struct DEmpty: Decodable { init(from d: Decoder) throws {} }
struct DAttachmentData: Decodable { var mime: String; var data: String
    enum CodingKeys: String, CodingKey { case mime, data }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); mime = c.str(.mime); data = c.str(.data) } }

/// `thread.queue` → `{rows, more}`; `queue.act` → `{}`, or `{text}` from q-edit / o-edit (rows: Outbox.swift `DQueueRow`)
struct DQueueReply: Decodable {
    var rows: [DQueueRow]
    var more: String
    enum CodingKeys: String, CodingKey { case rows, more }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); rows = (try? c.decode([DQueueRow].self, forKey: .rows)) ?? []; more = c.str(.more) }
}
struct DQueueActReply: Decodable { var text: String? }

struct DError: Error, LocalizedError {
    var code: String
    var message: String
    var errorDescription: String? { message.isEmpty ? code : message }
}

// MARK: data/1 additions (hello.features: actions, search, models, mentions, bots, notify; native/host/src/data/ext.rs)
struct DBot: Codable, Equatable, Identifiable {
    var id: String; var name: String; var thread: String; var uber: Bool
    enum CodingKeys: String, CodingKey { case id, name, thread, uber }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); id = c.str(.id); name = c.str(.name); thread = c.str(.thread); uber = c.bool(.uber) }
}
struct DRoom: Codable, Equatable, Hashable, Identifiable {
    var id: String; var name: String
    enum CodingKeys: String, CodingKey { case id, name }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); id = c.str(.id); name = c.str(.name) }
}
struct DDirectory: Decodable { var bots: [DBot]; var rooms: [DRoom]
    enum CodingKeys: String, CodingKey { case bots, rooms }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); bots = (try? c.decode([DBot].self, forKey: .bots)) ?? []; rooms = (try? c.decode([DRoom].self, forKey: .rooms)) ?? [] } }
struct DPost: Decodable, Identifiable {
    var id: String; var from: String; var text: String; var at: Int
    enum CodingKeys: String, CodingKey { case id, from, text, at }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); id = c.str(.id); from = c.str(.from); text = c.str(.text); at = c.int(.at) }
}
struct DPosts: Decodable { var posts: [DPost]
    enum CodingKeys: String, CodingKey { case posts }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); posts = (try? c.decode([DPost].self, forKey: .posts)) ?? [] } }
struct DThreadOpt: Decodable { var thread: DThread?
    enum CodingKeys: String, CodingKey { case thread }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); thread = try? c.decode(DThread.self, forKey: .thread) } }
/// `history.list`: Conversation history, the old Scratch chats (`live` always shown, `threads` the rest a page at a time, `count` every chat).
struct DHistory: Decodable { var live: [DThread] = []; var threads: [DThread] = []; var next: String?; var count = 0
    enum CodingKeys: String, CodingKey { case live, threads, next, count }
    init() {}
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); live = (try? c.decode([DThread].self, forKey: .live)) ?? []; threads = (try? c.decode([DThread].self, forKey: .threads)) ?? []; next = try? c.decode(String.self, forKey: .next); count = (try? c.decode(Int.self, forKey: .count)) ?? 0 } }
struct DSearch: Decodable { var threads: [DThread]; var projects: [DProject]; var messages: [DHit]
    enum CodingKeys: String, CodingKey { case threads, projects, messages }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); threads = (try? c.decode([DThread].self, forKey: .threads)) ?? []; projects = (try? c.decode([DProject].self, forKey: .projects)) ?? []; messages = (try? c.decode([DHit].self, forKey: .messages)) ?? [] } }
// the picker under the composer (`thread.picker`): sections the hub lists, drawn in order; a press is `thread.pick`
struct DChoice: Decodable, Identifiable {
    var label: String; var value: String; var on: Bool; var ok: Bool; var why: String; var id: String { value }
    enum CodingKeys: String, CodingKey { case label, value, on, ok, why }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); value = c.str(.value); label = c.str(.label).isEmpty ? c.str(.value) : c.str(.label); on = c.bool(.on); ok = (try? c.decode(Bool.self, forKey: .ok)) ?? true; why = c.str(.why) }
}
struct DPickerSection: Decodable, Identifiable {
    var title: String; var action: String; var choices: [DChoice]; var id: String { action }
    enum CodingKeys: String, CodingKey { case title, action, choices }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); title = c.str(.title); action = c.str(.action); choices = (try? c.decode([DChoice].self, forKey: .choices)) ?? [] }
}
struct DPicker: Decodable { var chip: String; var sections: [DPickerSection]
    enum CodingKeys: String, CodingKey { case chip, sections }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); chip = c.str(.chip); sections = (try? c.decode([DPickerSection].self, forKey: .sections)) ?? [] } }
struct DPickerReply: Decodable { var picker: DPicker }
/// Buy it (feature buy, `thread.buy`): the desktop header's button for a thread, in the hub's words; `order.buy` answers the buy thread made.
struct DBuy: Decodable, Equatable { var label = ""; var tip = ""
    enum K: String, CodingKey { case label, tip }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: K.self); label = c.str(.label); tip = c.str(.tip) } }
struct DBuyReply: Decodable { var buy: DBuy?
    enum K: String, CodingKey { case buy }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: K.self); buy = try? c.decode(DBuy.self, forKey: .buy) } }
struct DThreadRef: Decodable { var thread: String
    enum K: String, CodingKey { case thread }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: K.self); thread = c.str(.thread) } }
/// What was said (`search`'s `messages`, `find`'s `matches`): `msg` is the entry it is in, `text` a line around the first word; `search` also names the
/// thread, its title and its project's. `at` in ms (an older hub said seconds).
struct DHit: Decodable, Identifiable { var thread = "", title = "", project = "", msg = "", role = "", text = ""; var at = 0; var id: String { thread + "|" + msg + "|\(at)" }
    enum K: String, CodingKey { case thread, title, project, msg, role, text, at }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: K.self); thread = c.str(.thread); title = c.str(.title); project = c.str(.project); msg = c.str(.msg); role = c.str(.role); text = c.str(.text); at = c.int(.at); if at > 0 && at < 100_000_000_000 { at *= 1000 } } }
/// `find {thread, q}` (feature find): every message of the thread with every word, oldest first; `total` of them, `more` past the hub's cap.
struct DFind: Decodable { var q = ""; var matches: [DHit] = []; var total = 0; var more = false
    enum K: String, CodingKey { case q, matches, total, more }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: K.self); q = c.str(.q); matches = (try? c.decode([DHit].self, forKey: .matches)) ?? []; total = c.int(.total); more = c.bool(.more) } }
struct DHits: Decodable { var messages: [DHit]
    enum K: String, CodingKey { case messages }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: K.self); messages = (try? c.decode([DHit].self, forKey: .messages)) ?? [] } }
/// A buy thread's Order card (feature order, `thread.order`): the hub's, drawn as it says.
struct DOrderStep: Decodable, Equatable { var label = ""; var mark = ""
    init(from d: Decoder) throws { let c = try d.container(keyedBy: K.self); label = (try? c.decode(String.self, forKey: .label)) ?? ""; mark = (try? c.decode(String.self, forKey: .mark)) ?? "" }
    enum K: String, CodingKey { case label, mark } }
struct DOrder: Decodable, Equatable {
    var name = "", line = "", total = "", customs = "", pay = "", label = ""; var waiting = false; var steps: [DOrderStep] = []
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: K.self); func s(_ k: K) -> String { (try? c.decode(String.self, forKey: k)) ?? "" }
        name = s(.name); line = s(.line); total = s(.total); customs = s(.customs); pay = s(.pay); label = s(.label)
        waiting = (try? c.decode(Bool.self, forKey: .waiting)) ?? false; steps = (try? c.decode([DOrderStep].self, forKey: .steps)) ?? []
    }
    enum K: String, CodingKey { case name, line, total, customs, pay, label, waiting, steps }
    /// Only an http(s) link opens.
    var payURL: URL? { (pay.hasPrefix("https://") || pay.hasPrefix("http://")) ? URL(string: pay) : nil }
}
struct DOrderReply: Decodable { var order: DOrder? }
/// What a new thread will run with, chosen before it exists.
struct DNewThreadChoice: Equatable {
    var provider: String?; var model: String?; var effort: String?; var runtime: String?
    var params: [String: Any] { var p: [String: Any] = [:]; if let provider { p["provider"] = provider }; if let model { p["model"] = model }; if let effort { p["effort"] = effort }; if let runtime { p["runtime"] = runtime }; return p }
    mutating func set(_ action: String, _ value: String) {
        switch action {
        case "provider": provider = value; model = ""; effort = ""
        case "model": model = value
        case "effort": effort = value
        default: runtime = value
        }
    }
}
struct DMention: Decodable, Identifiable {
    var sigil: String; var mid: String; var label: String; var detail: String; var id: String { sigil + mid }
    var word: String   // what goes in the draft, the desktop's word (`>bom-review`, `%demo`): what /give and the reference cards read; "" from an older hub
    enum CodingKeys: String, CodingKey { case sigil, id, label, detail, word }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); sigil = c.str(.sigil); mid = c.str(.id); label = c.str(.label); detail = c.str(.detail); word = c.str(.word) }
}
struct DMentions: Decodable { var items: [DMention]
    enum CodingKeys: String, CodingKey { case items }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); items = (try? c.decode([DMention].self, forKey: .items)) ?? [] } }

/// `update.status`: the hub decides the words; a phone only shows them.
struct DUpdate: Decodable { let current: String; let channel: String; let line: String; let canCheck: Bool }
// A thread's background work and locks (thread.work, the "work" push): the rows the hub decides, each with what its button sends.
struct DWorkRow: Decodable, Hashable { let title, kind, state, doing: String; let act, value, label: String?; var key: String? = nil; var meta: String? = nil; var detail: DWorkDetail? = nil
    /// a subagent's row names it (`key`, with its drill-down): a tap opens its view (feature subview)
    var subKey: String { detail != nil ? key ?? "" : "" }
}
struct DWorkDetail: Decodable, Hashable {}
/// One subagent's own view (feature subview: `thread.sub`, the `sub` push): its head as its panel row says it, and its steps as entries, oldest first.
struct DSubView: Decodable, Equatable {
    var key, thread, title, state, meta, took, place, ask, outcome: String
    var live: Bool; var act, value, label: String?; var more: Int; var entries: [DEntry]
    enum CodingKeys: String, CodingKey { case key, thread, title, state, meta, took, place = "where", ask, outcome, live, act, value, label, more, entries }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        key = c.str(.key); thread = c.str(.thread); title = c.str(.title); state = c.str(.state); meta = c.str(.meta); took = c.str(.took); place = c.str(.place); ask = c.str(.ask); outcome = c.str(.outcome)
        live = (try? c.decode(Bool.self, forKey: .live)) ?? false; act = try? c.decode(String.self, forKey: .act); value = try? c.decode(String.self, forKey: .value); label = try? c.decode(String.self, forKey: .label)
        more = (try? c.decode(Int.self, forKey: .more)) ?? 0; entries = (try? c.decode([DEntry].self, forKey: .entries)) ?? []
    }
}
struct DSubViewReply: Decodable { let view: DSubView? }
struct DSubRoute: Hashable { let thread: String; let key: String }
/// A long result shows three lines until tapped (the desktop's subview.rs `result_lines`).
func dSubResultLines(_ text: String, open: Bool) -> (String, Bool) {
    let ls = text.components(separatedBy: "\n"); if open || ls.count <= 3 { return (text, false) }
    return (ls.prefix(3).joined(separator: "\n"), true)
}
struct DWork: Decodable { let word: String; let live: Int; let rows: [DWorkRow] }
struct DWorkReply: Decodable { let subs: DWork? }
// Who holds a lock (locks.list, the "locks" push): `line` is the hub's words for it.
struct DLock: Decodable, Identifiable { let name, holder, thread, line: String; let waiting: Int; var id: String { name } }
struct DLocks: Decodable { let locks: [DLock] }

/// The GitHub star suggestion while the hub says it is out and undecided (feature "star", native/host/src/state/star.rs). The hub decides
/// when; the app shows exactly this: from star.offer at every (re)connect, and from a `{"t":"star","offer"}` frame (null hides it).
struct DStarOffer: Codable, Equatable {
    var token: String; var text: String; var url: String; var label: String
    var valid: DStarOffer? { !token.isEmpty && !text.isEmpty && url.hasPrefix("https://") ? self : nil }
    static func of(_ o: Any?) -> DStarOffer? {
        guard let d = o as? [String: Any] else { return nil }
        let g = { (k: String) in d[k] as? String ?? "" }
        return DStarOffer(token: g("token"), text: g("text"), url: g("url"), label: g("label")).valid
    }
}
struct DStarReply: Decodable { var offer: DStarOffer? }
