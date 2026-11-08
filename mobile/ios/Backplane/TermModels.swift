import Foundation

// docs/PHONE-DATA.md, Terminals (feature `terms`): the plain data, decoded forgivingly like the rest of data/1.

/// `entry.term` of a `kind:"term"` entry: a chat terminal in a thread.
struct DTermInfo: Codable, Equatable {
    var key: String, id: String, title: String, cols: Int, rows: Int, cmd: String
    enum CodingKeys: String, CodingKey { case key, id, title, cols, rows, cmd }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); key = c.str(.key); id = c.str(.id); title = c.str(.title); cols = c.int(.cols); rows = c.int(.rows); cmd = c.str(.cmd) }
}

/// `term.read`, `term.watch` and `term.new`: the screen's run `g` at offset `o`, in bytes after `from`, or a snapshot (`reset`).
struct DTermRead: Decodable {
    var key = "", g = 0, o = 0, state = "", running = false, cols = 0, rows = 0, title = "", b = ""
    var from: Int?
    var reset: [Int]?
    enum CodingKeys: String, CodingKey { case key, g, o, state, running, cols, rows, title, b, from, reset }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        key = c.str(.key); g = c.int(.g); o = c.int(.o); state = c.str(.state); running = c.bool(.running); cols = c.int(.cols); rows = c.int(.rows); title = c.str(.title); b = c.str(.b)
        from = try? c.decode(Int.self, forKey: .from); reset = try? c.decode([Int].self, forKey: .reset)
    }
    var bytes: [UInt8] { Data(base64Encoded: b).map { [UInt8]($0) } ?? [] }
}

/// A row of `term.list`.
struct DTermRow: Decodable, Identifiable, Equatable {
    var key = "", kind = "", thread = "", title = "", state = "", running = false, cols = 0, rows = 0, cwd = "", at = 0
    var id: String { key }
    enum CodingKeys: String, CodingKey { case key, kind, thread, title, state, running, cols, rows, cwd, at }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        key = c.str(.key); kind = c.str(.kind); thread = c.str(.thread); title = c.str(.title); state = c.str(.state); running = c.bool(.running); cols = c.int(.cols); rows = c.int(.rows); cwd = c.str(.cwd); at = c.int(.at)
    }
}
struct DTermList: Decodable { var terms: [DTermRow]
    enum CodingKeys: String, CodingKey { case terms }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); terms = (try? c.decode([DTermRow].self, forKey: .terms)) ?? [] } }

/// A terminal opened full screen.
struct TermRoute: Identifiable, Hashable { var key: String; var title: String; var id: String { key } }
/// The Terminals screen of a project ("" for the hub's).
struct DTermsRoute: Hashable { var project: String }
