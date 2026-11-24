import Foundation

// Clone from git (docs/CLONE.md): the hub's clone.* answers and pushes, read forgivingly, and the few decisions the Add project sheet makes from
// them (what clone.start is given, which push belongs to the job on show, where the sheet goes when the clone is done). Compiled alone in
// mobile/ios/Tests/CloneModelTests.swift (scripts/mobile/ios-unit-tests.sh), so nothing here reaches into the rest of the app.

private extension KeyedDecodingContainer {
    func cs(_ k: Key) -> String { (try? decodeIfPresent(String.self, forKey: k)) ?? "" }
    func cb(_ k: Key) -> Bool { (try? decodeIfPresent(Bool.self, forKey: k)) ?? false }
    func ci(_ k: Key) -> Int? {
        if let n = try? decodeIfPresent(Int.self, forKey: k) { return n }
        return (try? decodeIfPresent(Double.self, forKey: k)).map { Int($0) }
    }
}

/// One of the person's GitHub repositories (`clone.repos` rows). `value` is what clone.start takes.
struct DCloneRepo: Decodable, Hashable {
    var name = "", value = "", description = "", account = ""
    var priv = false
    enum CodingKeys: String, CodingKey { case name, value, description, account, priv = "private" }
    init(name: String, value: String = "", description: String = "", priv: Bool = false, account: String = "") { self.name = name; self.value = value; self.description = description; self.priv = priv; self.account = account }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); name = c.cs(.name); value = c.cs(.value); description = c.cs(.description); account = c.cs(.account); priv = c.cb(.priv) }
    /// clone.start's `repo`: the row's value, else its name
    var start: String { value.isEmpty ? name : value }
}

/// `clone.repos`: the suggestions, and how gh stands (`ok`, `nogh`, `signedout`, `error`) with the words for it.
struct DCloneRepos: Decodable {
    var repos: [DCloneRepo] = []; var more = 0; var state = ""; var loading = false; var hint = ""
    enum CodingKeys: String, CodingKey { case repos, more, state, loading, hint }
    init() {}
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); repos = (try? c.decode([DCloneRepo].self, forKey: .repos)) ?? []; more = c.ci(.more) ?? 0; state = c.cs(.state); loading = c.cb(.loading); hint = c.cs(.hint) }
    /// the hint under the field: only when gh cannot list (not installed, signed out, failed)
    var shownHint: String { ["nogh", "signedout", "error"].contains(state) ? hint : "" }
    var offersSignIn: Bool { state == "signedout" }
}

/// `clone.dest`: where the repository would go and whether it can (`ok`; `same`: a clone of it is there already and is added as it is).
struct DCloneDest: Decodable, Equatable {
    var ok = false, same = false
    var error = "", repo = "", origin = "", name = "", dest = "", destShown = "", parent = "", folder = "", defaultParent = "", defaultParentShown = ""
    enum CodingKeys: String, CodingKey { case ok, same, error, repo, origin, name, dest, destShown, parent, folder, defaultParent, defaultParentShown }
    init() {}
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        ok = c.cb(.ok); same = c.cb(.same); error = c.cs(.error); repo = c.cs(.repo); origin = c.cs(.origin); name = c.cs(.name); dest = c.cs(.dest)
        destShown = c.cs(.destShown); parent = c.cs(.parent); folder = c.cs(.folder); defaultParent = c.cs(.defaultParent); defaultParentShown = c.cs(.defaultParentShown)
    }
    /// the typed text is a repository address (the hub parsed it): the destination step shows
    var isRepo: Bool { !repo.isEmpty }
}

/// A clone job (`clone.start`/`status`/`retry` and the `clone` push).
struct DCloneJob: Decodable, Equatable {
    var id = "", repo = "", origin = "", dest = "", destShown = "", name = "", state = "", phase = "", detail = "", error = "", project = "", thread = ""
    var pct: Int?
    var auth = false, signin = false
    var notes: [String] = []
    /// the notes are to be read before the app goes to the clone (nil: a hub from before `hold`, where any note holds)
    var hold: Bool?
    var started = 0, ended = 0
    enum CodingKeys: String, CodingKey { case id, repo, origin, dest, destShown, name, state, phase, detail, error, project, thread, pct, auth, signin, notes, hold, started, ended }
    init(id: String, state: String = "running", repo: String = "", error: String = "") { self.id = id; self.state = state; self.repo = repo; self.error = error }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.cs(.id); repo = c.cs(.repo); origin = c.cs(.origin); dest = c.cs(.dest); destShown = c.cs(.destShown); name = c.cs(.name); state = c.cs(.state)
        phase = c.cs(.phase); detail = c.cs(.detail); error = c.cs(.error); project = c.cs(.project); thread = c.cs(.thread); pct = c.ci(.pct)
        auth = c.cb(.auth); signin = c.cb(.signin); notes = (try? c.decode([String].self, forKey: .notes)) ?? []; started = c.ci(.started) ?? 0; ended = c.ci(.ended) ?? 0
        hold = try? c.decodeIfPresent(Bool.self, forKey: .hold)
    }
    /// Open waits for the person to read the notes
    var holds: Bool { hold ?? !notes.isEmpty }
    var running: Bool { state == "running" }
    var over: Bool { ["done", "failed", "cancelled"].contains(state) }
    /// the bar's fraction; nil draws it indeterminate
    var fraction: Double? { pct.map { min(1, max(0, Double($0) / 100)) } }
}

/// `clone.signin` and its push: GitHub's device sign-in through gh on the hub's machine.
struct DCloneSignin: Decodable, Equatable {
    var state = "", code = "", url = "", error = ""
    enum CodingKeys: String, CodingKey { case state, code, url, error }
    init(state: String = "", code: String = "", url: String = "", error: String = "") { self.state = state; self.code = code; self.url = url; self.error = error }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); state = c.cs(.state); code = c.cs(.code); url = c.cs(.url); error = c.cs(.error) }
}

enum Clone {
    /// What clone.start and clone.dest are given for the field: the picked suggestion's value while the field still shows that suggestion, else the typed text.
    static func repo(typed: String, picked: DCloneRepo?) -> String {
        let t = typed.trimmingCharacters(in: .whitespacesAndNewlines)
        if let p = picked, t == p.name || t == p.start { return p.start }
        return t
    }
    /// A job id of the app's own (`i<ms>-<random>`, [A-Za-z0-9_-], at most 64): known before clone.start is sent, so a push that comes before the
    /// answer (a fast clone's `done`) is this job's already.
    static func newId(ms: Int = Int(Date().timeIntervalSince1970 * 1000), random: String = String((0..<8).map { _ in "abcdefghijklmnopqrstuvwxyz0123456789".randomElement()! })) -> String {
        let ok = Set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-")
        return String(("i\(ms)-" + random).filter { ok.contains($0) }.prefix(64))
    }
    /// The job the sheet shows after a push, an answer or a status read: the incoming one when it is this job (`id`) and no older than what is
    /// shown. An end (done, failed, cancelled) always beats running; between two of the same kind the later `ended` wins.
    static func follow(_ shown: DCloneJob?, _ incoming: DCloneJob?, id: String) -> DCloneJob? {
        guard let incoming, !id.isEmpty, incoming.id == id else { return shown }
        guard let s = shown, s.id == id else { return incoming }
        if s.over && incoming.running { return s }
        if s.over && incoming.over && incoming.ended < s.ended { return s }
        return incoming
    }
    /// Try again (sent as clone.start with a new id): the same repository as first given, into the folder the failed one had.
    static func retryParams(_ job: DCloneJob, repo: String, thread: Bool, text: String) -> [String: Any] {
        var p: [String: Any] = ["repo": repo.isEmpty ? job.repo : repo, "dest": job.dest, "thread": thread]
        if !text.isEmpty { p["text"] = text }
        return p
    }
    enum After: Equatable { case stay, thread(String), project(String), dismiss }
    /// Where a done clone goes: notes the hub holds for reading keep the sheet (`stay`) until the person goes on; else at once to its thread,
    /// else to its project.
    static func after(_ job: DCloneJob, read: Bool = false) -> After {
        guard job.state == "done" else { return .stay }
        if job.holds && !read { return .stay }
        if !job.thread.isEmpty { return .thread(job.thread) }
        if !job.project.isEmpty { return .project(job.project) }
        return .dismiss
    }
    /// clone.dest's params: the repository, and the folder only once one is chosen (none: the hub's default, a free name next to the last parent).
    static func destParams(repo: String, parent: String, name: String) -> [String: String] {
        var p = ["repo": repo]
        if !parent.isEmpty { p["parent"] = parent }
        if !name.trimmingCharacters(in: .whitespaces).isEmpty { p["name"] = name.trimmingCharacters(in: .whitespaces) }
        return p
    }
    /// Clone is pressable: the hub said the destination will do, and nothing is on its way.
    static func canStart(_ dest: DCloneDest?, busy: Bool) -> Bool { !busy && dest?.ok == true }
}
