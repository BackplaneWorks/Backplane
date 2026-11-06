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
    // phone-only, never on the wire: a message sent and not yet confirmed, or one the hub refused
    var sending = false
    var failed = false
    enum CodingKeys: String, CodingKey { case id, kind, text, at, clientId, replaces, live, tool, ask }
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

struct DError: Error, LocalizedError {
    var code: String
    var message: String
    var errorDescription: String? { message.isEmpty ? code : message }
}
