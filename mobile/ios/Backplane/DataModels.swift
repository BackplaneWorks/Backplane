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
    enum CodingKeys: String, CodingKey { case name, summary, state }
    init(name: String, summary: String, state: String) { self.name = name; self.summary = summary; self.state = state }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); name = c.str(.name); summary = c.str(.summary); state = c.str(.state) }
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
    var tool: DTool?
    var ask: DAsk?
    var attachments: [DAttachment] = []
    // phone-only, never on the wire: a message sent and not yet confirmed, or one the hub refused
    var sending = false
    var failed = false
    var localImages: [Data] = []   // the pictures of a message this phone is sending (JPEG), shown before the hub has them
    enum CodingKeys: String, CodingKey { case id, kind, text, at, label, tone, clientId, replaces, live, tool, ask, attachments }
    init(id: String, kind: String, text: String, at: Int, clientId: String? = nil, live: Bool = false) {
        self.id = id; self.kind = kind; self.text = text; self.at = at; self.clientId = clientId; self.live = live
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); kind = c.str(.kind); text = c.str(.text); at = c.int(.at); label = c.str(.label); tone = c.str(.tone)
        clientId = try? c.decode(String.self, forKey: .clientId)
        replaces = try? c.decode(String.self, forKey: .replaces)
        live = c.bool(.live)
        tool = try? c.decode(DTool.self, forKey: .tool)
        ask = try? c.decode(DAsk.self, forKey: .ask)
        attachments = (try? c.decode([DAttachment].self, forKey: .attachments)) ?? []
    }
}

struct DProject: Codable, Equatable, Identifiable {
    var id: String
    var title: String
    var root: String
    var threadCount: Int
    var updatedAt: Int
    var waiting: Int
    var icon: String   // the badge the hub names: an icon and its colour as "#rrggbb"
    var css: String
    enum CodingKeys: String, CodingKey { case id, title, root, threadCount, updatedAt, waiting, icon, css }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); title = c.str(.title); root = c.str(.root); threadCount = c.int(.threadCount); updatedAt = c.int(.updatedAt); waiting = c.int(.waiting)
        icon = c.str(.icon); css = c.str(.css)
    }
}

struct DThread: Codable, Equatable, Identifiable {
    var id: String
    var project: String
    var title: String
    var state: String   // idle | working | asking | failed
    var updatedAt: Int
    var pinned: Bool
    var settled: Bool
    var archived: Bool
    var provider: String
    var model: String
    var effort: String
    var runtime: String
    var chip: String    // the composer's "claude / opus / high", in the hub's words
    var preview: String
    var waiting: Int
    var parent: String   // the thread it is a subthread of; "" at the top level
    enum CodingKeys: String, CodingKey { case id, project, title, state, updatedAt, pinned, settled, archived, provider, model, effort, runtime, chip, preview, waiting, parent }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); project = c.str(.project); title = c.str(.title); state = c.str(.state); updatedAt = c.int(.updatedAt)
        pinned = c.bool(.pinned); settled = c.bool(.settled); archived = c.bool(.archived)
        provider = c.str(.provider); model = c.str(.model); effort = c.str(.effort); runtime = c.str(.runtime); chip = c.str(.chip); preview = c.str(.preview); waiting = c.int(.waiting); parent = c.str(.parent)
    }
}

// replies
struct DProjectsPage: Decodable { var projects: [DProject]; var next: String?
    enum CodingKeys: String, CodingKey { case projects, next }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); projects = (try? c.decode([DProject].self, forKey: .projects)) ?? []; next = try? c.decode(String.self, forKey: .next) } }
struct DThreadsPage: Decodable { var threads: [DThread]; var next: String?
    enum CodingKeys: String, CodingKey { case threads, next }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); threads = (try? c.decode([DThread].self, forKey: .threads)) ?? []; next = try? c.decode(String.self, forKey: .next) } }
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
struct DSearch: Decodable { var threads: [DThread]; var projects: [DProject]
    enum CodingKeys: String, CodingKey { case threads, projects }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); threads = (try? c.decode([DThread].self, forKey: .threads)) ?? []; projects = (try? c.decode([DProject].self, forKey: .projects)) ?? [] } }
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
    enum CodingKeys: String, CodingKey { case sigil, id, label, detail }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); sigil = c.str(.sigil); mid = c.str(.id); label = c.str(.label); detail = c.str(.detail) }
}
struct DMentions: Decodable { var items: [DMention]
    enum CodingKeys: String, CodingKey { case items }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); items = (try? c.decode([DMention].self, forKey: .items)) ?? [] } }

/// `update.status`: the hub decides the words; a phone only shows them.
struct DUpdate: Decodable { let current: String; let channel: String; let line: String; let canCheck: Bool }
