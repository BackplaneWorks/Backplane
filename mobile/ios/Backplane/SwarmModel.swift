import Foundation

// Swarms (docs/SWARM.md, Client data; feature `swarm`): a permanent room whose members are threads. The hub's data as typed values and the few
// decisions the phone makes from it, each a pure function with a test (Tests/SwarmModelTests.swift; Foundation only, compiled alone).

private extension KeyedDecodingContainer {
    func s(_ k: Key) -> String { (try? decode(String.self, forKey: k)) ?? "" }
    func n(_ k: Key) -> Int {
        if let v = try? decode(Int.self, forKey: k) { return v }
        if let d = try? decode(Double.self, forKey: k), d.isFinite { return Int(d) }
        return 0
    }
    func b(_ k: Key) -> Bool { (try? decode(Bool.self, forKey: k)) ?? false }
    func list(_ k: Key) -> [String] { (try? decode([String].self, forKey: k)) ?? [] }
}

/// A row of `swarm.list`.
struct DSwarmRow: Codable, Equatable, Hashable, Identifiable {
    var id: String; var name: String; var project: String; var goal: String; var state: String; var reason: String; var members: Int; var last: Int
    enum CodingKeys: String, CodingKey { case id, name, project, goal, state, reason, members, last }
    init(id: String, name: String, project: String, state: String, members: Int = 0, last: Int = 0) {
        self.id = id; self.name = name; self.project = project; goal = ""; self.state = state; reason = ""; self.members = members; self.last = last
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.s(.id); name = c.s(.name); project = c.s(.project); goal = c.s(.goal); state = c.s(.state); reason = c.s(.reason); members = c.n(.members); last = c.n(.last)
    }
}
struct DSwarmList: Decodable { var swarms: [DSwarmRow]
    enum CodingKeys: String, CodingKey { case swarms }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); swarms = (try? c.decode([DSwarmRow].self, forKey: .swarms)) ?? [] } }

/// What opens the swarm screen.
struct DSwarmRoute: Hashable { var id: String; var name: String }

/// `swarm.get`'s room.
struct DSwarmRoom: Decodable, Equatable {
    var id = "", name = "", kind = "", project = "", goal = "", state = "", reason = "", by = ""
    enum CodingKeys: String, CodingKey { case id, name, kind, project, goal, state, reason, by }
    init() {}
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.s(.id); name = c.s(.name); kind = c.s(.kind); project = c.s(.project); goal = c.s(.goal); state = c.s(.state); reason = c.s(.reason); by = c.s(.by)
    }
}
struct DSwarmMember: Decodable, Equatable, Identifiable {
    var member = "", thread = "", name = "", color = "", provider = "", model = "", role = "", state = "", title = ""
    var id: String { member }
    enum CodingKeys: String, CodingKey { case member, thread, name, color, provider, model, role, state, title }
    init(member: String, thread: String, name: String, state: String) { self.member = member; self.thread = thread; self.name = name; self.state = state }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        member = c.s(.member); thread = c.s(.thread); name = c.s(.name); color = c.s(.color); provider = c.s(.provider); model = c.s(.model); role = c.s(.role); state = c.s(.state); title = c.s(.title)
    }
}
struct DSwarmPost: Decodable, Equatable, Identifiable {
    var id = "", from = "", thread = "", color = "", text = "", reply = "", held = ""
    var tags: [String] = [], mentions: [String] = [], files: [String] = []
    var guardPost = false
    var at = 0
    enum CodingKeys: String, CodingKey { case id, from, thread, color, text, tags, reply, mentions, files, guardPost = "guard", held, at }
    init(id: String, from: String = "", text: String = "", tags: [String] = [], files: [String] = [], guardPost: Bool = false, held: String = "") {
        self.id = id; self.from = from; self.text = text; self.tags = tags; self.files = files; self.guardPost = guardPost; self.held = held
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.s(.id); from = c.s(.from); thread = c.s(.thread); color = c.s(.color); text = c.s(.text); reply = c.s(.reply); held = c.s(.held)
        tags = c.list(.tags); mentions = c.list(.mentions); files = c.list(.files); guardPost = c.b(.guardPost); at = c.n(.at)
    }
}
struct DSwarmClaim: Decodable, Equatable {
    var post = "", thread = "", state = ""; var files: [String] = []
    enum CodingKeys: String, CodingKey { case post, thread, files, state }
    init(post: String, state: String) { self.post = post; self.state = state }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); post = c.s(.post); thread = c.s(.thread); state = c.s(.state); files = c.list(.files) }
}
/// `swarm.get`: the room, its members with their live state, the posts (oldest first; only those after `since` when it was given) and the open claims.
struct DSwarmBoard: Decodable, Equatable {
    var room = DSwarmRoom(); var members: [DSwarmMember] = []; var posts: [DSwarmPost] = []; var claims: [DSwarmClaim] = []
    enum CodingKeys: String, CodingKey { case room, members, posts, claims }
    init() {}
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        room = (try? c.decode(DSwarmRoom.self, forKey: .room)) ?? DSwarmRoom(); members = (try? c.decode([DSwarmMember].self, forKey: .members)) ?? []
        posts = (try? c.decode([DSwarmPost].self, forKey: .posts)) ?? []; claims = (try? c.decode([DSwarmClaim].self, forKey: .claims)) ?? []
    }
}
struct DSwarmCreated: Decodable { var room: String
    enum CodingKeys: String, CodingKey { case room }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); room = c.s(.room) } }

/// One model a member may run (`models.list`: `{models:{provider:[{value,label,ok,why}]}}`).
struct DModelRow: Decodable, Equatable, Hashable {
    var value = "", label = "", why = ""; var ok = true
    enum CodingKeys: String, CodingKey { case value, label, ok, why }
    init(value: String, ok: Bool = true) { self.value = value; label = value; self.ok = ok }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); value = c.s(.value); label = c.s(.label).isEmpty ? value : c.s(.label); ok = (try? c.decode(Bool.self, forKey: .ok)) ?? true; why = c.s(.why) }
}
struct DModelsList: Decodable { var models: [String: [DModelRow]]
    enum CodingKeys: String, CodingKey { case models }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); models = (try? c.decode([String: [DModelRow]].self, forKey: .models)) ?? [:] } }

/// A member row of the New swarm sheet (and the Add member sheet).
struct DSwarmMemberDraft: Equatable, Identifiable {
    var id = UUID()
    var provider = ""; var model = ""; var role = ""
}

enum Swarm {
    static let minMembers = 2, maxMembers = 8

    /// The rows a project's list shows: its swarms that are active or paused, in the hub's order (newest first).
    static func listed(_ rows: [DSwarmRow], project: String) -> [DSwarmRow] { rows.filter { $0.project == project && ($0.state == "active" || $0.state == "paused") } }
    /// The threads a project's list leaves out: a swarm's members (their `swarm` is the room).
    static func members<T>(_ threads: [T], swarm: (T) -> String, id: (T) -> String) -> Set<String> { Set(threads.filter { !swarm($0).isEmpty }.map(id)) }

    /// A re-read after `since` added to what is shown: new posts at the end, a post already shown replaced in place (a held mark may come later).
    static func merge(_ old: [DSwarmPost], _ new: [DSwarmPost]) -> [DSwarmPost] {
        var out = old; var at = Dictionary(old.enumerated().map { ($0.element.id, $0.offset) }, uniquingKeysWith: { a, _ in a })
        for p in new { if let i = at[p.id] { out[i] = p } else { at[p.id] = out.count; out.append(p) } }
        return out
    }
    /// What `swarm.get {since}` is given: the newest post shown, "" for the whole board.
    static func since(_ posts: [DSwarmPost]) -> String { posts.last?.id ?? "" }

    /// How a post is drawn: a synthesis set apart, the hub's own posts grey, a held post dimmed, else a member's or the person's.
    enum Look: Equatable { case synthesis, hub, held, plain }
    static func look(_ p: DSwarmPost) -> Look {
        if p.tags.contains("synthesis") { return .synthesis }
        if p.guardPost { return .hub }
        if !p.held.isEmpty { return .held }
        return .plain
    }
    /// A post's time in ms (the hub's room posts are stamped in seconds).
    static func ms(_ at: Int) -> Int { at > 0 && at < 100_000_000_000 ? at * 1000 : at }
    /// The tags shown as labels (synthesis is drawn by its box instead).
    static func labels(_ p: DSwarmPost) -> [String] { p.tags.filter { $0 != "synthesis" } }
    /// A claim's files are waiting when its open claim says so (a peer holds a lock on one of them).
    static func waiting(_ p: DSwarmPost, _ claims: [DSwarmClaim]) -> Bool { claims.contains { $0.post == p.id && $0.state == "waiting" } }
    /// The claim is still open (no `done` has answered it).
    static func open(_ p: DSwarmPost, _ claims: [DSwarmClaim]) -> Bool { claims.contains { $0.post == p.id } }

    /// The controls of the state line, as the desktop's room page offers them (present `roomPage.swarm.actions`): pause or resume, and stop; none once it ended.
    static func actions(_ state: String) -> [String] {
        switch state { case "active", "": ["pause", "stop"]; case "paused": ["resume", "stop"]; default: [] }
    }
    static func ended(_ state: String) -> Bool { state == "done" || state == "stopped" }
    /// Members may be added and removed while the swarm runs or is paused.
    static func canEditMembers(_ state: String, count: Int) -> (add: Bool, remove: Bool) {
        let live = !ended(state); return (live && count < maxMembers, live && count > 1)
    }

    /// The providers a member may run on: those with a model the hub can run now, in name order (`models.list` is a map).
    static func providers(_ models: [String: [DModelRow]]) -> [String] { models.filter { $0.value.contains { $0.ok } }.keys.sorted() }
    /// A new member row: the first provider, its default model.
    static func draft(_ models: [String: [DModelRow]]) -> DSwarmMemberDraft { DSwarmMemberDraft(provider: providers(models).first ?? "") }
    /// The sheet can send: a topic and from 2 to 8 members, each with a provider.
    static func canCreate(topic: String, members: [DSwarmMemberDraft]) -> Bool {
        !topic.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && (minMembers...maxMembers).contains(members.count) && members.allSatisfy { !$0.provider.isEmpty }
    }
    /// One member as `swarm.create` and `swarm.act add` take it: empty model and role left out.
    static func member(_ m: DSwarmMemberDraft) -> [String: String] {
        var o = ["provider": m.provider]
        let model = m.model.trimmingCharacters(in: .whitespaces), role = m.role.trimmingCharacters(in: .whitespacesAndNewlines)
        if !model.isEmpty { o["model"] = model }; if !role.isEmpty { o["role"] = role }
        return o
    }
    /// `swarm.create`'s parameters; empty goal and seed left out.
    static func createParams(project: String, topic: String, goal: String, seed: String, members: [DSwarmMemberDraft]) -> [String: Any] {
        var p: [String: Any] = ["project": project, "topic": topic.trimmingCharacters(in: .whitespacesAndNewlines), "members": members.map(member)]
        let g = goal.trimmingCharacters(in: .whitespacesAndNewlines), s = seed.trimmingCharacters(in: .whitespacesAndNewlines)
        if !g.isEmpty { p["goal"] = g }; if !s.isEmpty { p["seed"] = s }
        return p
    }
}
