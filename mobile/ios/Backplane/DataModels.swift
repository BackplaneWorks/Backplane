import Foundation

// docs/gpui/PHONE-DATA.md (data/1): the plain data the hub sends, as typed values. Decoding is forgiving (a missing field is empty, an extra one ignored)
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
    enum CodingKeys: String, CodingKey { case id, kind, text, at, clientId, replaces, live, tool, ask, attachments }
    init(id: String, kind: String, text: String, at: Int, clientId: String? = nil, live: Bool = false) {
        self.id = id; self.kind = kind; self.text = text; self.at = at; self.clientId = clientId; self.live = live
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); kind = c.str(.kind); text = c.str(.text); at = c.int(.at)
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
    enum CodingKeys: String, CodingKey { case id, title, root, threadCount, updatedAt, waiting }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); title = c.str(.title); root = c.str(.root); threadCount = c.int(.threadCount); updatedAt = c.int(.updatedAt); waiting = c.int(.waiting)
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
    var preview: String
    var waiting: Int
    enum CodingKeys: String, CodingKey { case id, project, title, state, updatedAt, pinned, settled, archived, provider, model, preview, waiting }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); project = c.str(.project); title = c.str(.title); state = c.str(.state); updatedAt = c.int(.updatedAt)
        pinned = c.bool(.pinned); settled = c.bool(.settled); archived = c.bool(.archived)
        provider = c.str(.provider); model = c.str(.model); preview = c.str(.preview); waiting = c.int(.waiting)
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
struct DModelRow: Decodable, Identifiable {
    var value: String; var label: String; var ok: Bool; var id: String { value }
    enum CodingKeys: String, CodingKey { case value, label, ok }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); value = c.str(.value); label = c.str(.label).isEmpty ? c.str(.value) : c.str(.label); ok = c.bool(.ok) }
}
struct DModels: Decodable { var models: [String: [DModelRow]]
    enum CodingKeys: String, CodingKey { case models }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); models = (try? c.decode([String: [DModelRow]].self, forKey: .models)) ?? [:] } }
struct DMention: Decodable, Identifiable {
    var sigil: String; var mid: String; var label: String; var detail: String; var id: String { sigil + mid }
    enum CodingKeys: String, CodingKey { case sigil, id, label, detail }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); sigil = c.str(.sigil); mid = c.str(.id); label = c.str(.label); detail = c.str(.detail) }
}
struct DMentions: Decodable { var items: [DMention]
    enum CodingKeys: String, CodingKey { case items }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); items = (try? c.decode([DMention].self, forKey: .items)) ?? [] } }

struct DModelGroup: Identifiable { var provider: String; var rows: [DModelRow]; var id: String { provider } }
