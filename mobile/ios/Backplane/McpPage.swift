import Foundation

// Settings > MCP servers, the pure half (no UI): the hub's rows (native/host/src/mcpx/page.rs) as values, which of a card's buttons are drawn,
// the page state the phone keeps (`ui`, given back by every mcp.page.act) and what is typed. Compiled alone with Tests/McpPageTests.swift.

/// A press: what a button, a switch or a tap on a card sends.
struct McpPress: Hashable {
    var label = "", action = "", value = ""
    var on = false
    init(label: String = "", action: String = "", value: String = "", on: Bool = false) { self.label = label; self.action = action; self.value = value; self.on = on }
    init?(_ o: Any?) {
        guard let o = o as? [String: Any], let a = o["action"] as? String, !a.isEmpty else { return nil }
        self.init(label: o["label"] as? String ?? "", action: a, value: o["value"] as? String ?? "", on: o["on"] as? Bool ?? false)
    }
}
struct McpField: Hashable {
    var name = "", hint = "", value = ""
    var secret = false, vault = false
    var lines = 1
    init(_ o: [String: Any]) {
        name = o["name"] as? String ?? ""; hint = o["hint"] as? String ?? ""; value = o["value"] as? String ?? ""
        secret = o["secret"] as? Bool ?? false; vault = o["vault"] as? Bool ?? false
        lines = max(1, McpRow.int(o["lines"]) ?? 1)
    }
}
/// A card's switch: a square switch, or (`style` "check") a tick box.
struct McpToggle: Hashable {
    var on = false, check = false
    var press: McpPress
}

enum McpKind: String { case plain, head, group, card, tool, notice, code, empty, panel, paste, actions }

struct McpRow: Identifiable {
    /// stable across refreshes while the page keeps its shape: the kind, the card's id or the label, the fields (so a box being typed in keeps its focus)
    var id = ""
    var kind: McpKind = .plain
    var label = "", note = "", tone = ""
    var buttons: [McpPress] = []
    var fields: [McpField] = []
    var inside = false
    // card
    var cardId = "", title = "", sub = "", meta = "", badge = ""
    var iconText = "", iconBg = ""
    var pillText = "", pillTone = ""
    var toggle: McpToggle?
    var press: McpPress?
    var open = false, compact = false
    // head
    var primary: Int?
    // code
    var text = ""

    static func int(_ v: Any?) -> Int? { (v as? Int) ?? (v as? Double).map(Int.init) ?? (v as? NSNumber)?.intValue }

    init(_ r: [String: Any]) {
        let k = r["kind"] as? String ?? ""
        kind = McpKind(rawValue: k) ?? .plain   // a kind this phone does not know is drawn as a plain row
        label = r["label"] as? String ?? ""; note = r["note"] as? String ?? ""; tone = r["tone"] as? String ?? ""
        buttons = (r["buttons"] as? [Any] ?? []).compactMap { McpPress($0) }
        fields = (r["fields"] as? [[String: Any]] ?? []).map(McpField.init)
        inside = r["in"] as? Bool ?? false
        cardId = r["id"] as? String ?? ""; title = r["title"] as? String ?? ""; sub = r["sub"] as? String ?? ""; meta = r["meta"] as? String ?? ""; badge = r["badge"] as? String ?? ""
        if let i = r["icon"] as? [String: Any] { iconText = i["text"] as? String ?? ""; iconBg = i["bg"] as? String ?? "" }
        if let p = r["pill"] as? [String: Any] { pillText = p["text"] as? String ?? ""; pillTone = p["tone"] as? String ?? "" }
        if let t = r["toggle"] as? [String: Any], let p = McpPress(t) { toggle = McpToggle(on: t["on"] as? Bool ?? false, check: (t["style"] as? String) == "check", press: p) }
        press = McpPress(r["press"])
        open = r["open"] as? Bool ?? false; compact = r["compact"] as? Bool ?? false
        primary = McpRow.int(r["primary"])
        text = r["text"] as? String ?? ""
    }

    /// The hub's `rows`, each with an id that holds while the page keeps its shape.
    static func parse(_ v: Any?) -> [McpRow] {
        var seen: [String: Int] = [:]
        return (v as? [[String: Any]] ?? []).map { o in
            var r = McpRow(o)
            let base = "\(r.kind.rawValue)|\(r.cardId.isEmpty ? r.label : r.cardId)|\(r.fields.map(\.name).joined(separator: ","))|\(r.inside ? "in" : "")"
            let n = seen[base, default: 0]; seen[base] = n + 1
            r.id = n == 0 ? base : "\(base)#\(n)"
            return r
        }
    }

    /// The buttons a card draws beside its switch: not those its switch or its press already are (an older client's stand-ins).
    var cardButtons: [McpPress] { buttons.filter { $0.action != toggle?.press.action && $0.action != press?.action } }
    /// The title a card shows.
    var shownTitle: String { title.isEmpty ? label : title }
    /// The tile's colour, "#rrggbb" as 0xrrggbb; nil for anything else.
    static func hex(_ s: String) -> UInt32? {
        guard s.count == 7, s.hasPrefix("#") else { return nil }
        return UInt32(s.dropFirst(), radix: 16)
    }
}

/// A card and the rows drawn inside it (those after it with `in`), or one row alone.
struct McpBlock: Identifiable {
    var row: McpRow
    var inner: [McpRow] = []
    var id: String { row.id }
    static func of(_ rows: [McpRow]) -> [McpBlock] {
        var out: [McpBlock] = []
        for r in rows {
            if r.inside, let last = out.last, last.row.kind == .card { out[out.count - 1].inner.append(r) } else { out.append(McpBlock(row: r)) }
        }
        return out
    }
}

/// The page's state on the phone: the hub's `ui` (kept as it is given back) and what the person typed (by field name; only what was typed, so
/// Edit's boxes show the server's own values until they are changed).
struct McpState {
    var ui: [String: String] = [:]
    var typed: [String: String] = [:]
    /// The presses after which every typed box is emptied.
    static let clearing: Set<String> = ["mcp-add", "mcp-import", "mcp-gallery", "mcp-save", "mcp-remove", "mcp-dup"]
    /// The form boxes the hub forgets when the page moves to the list, Add or an Edit (page.rs `step`).
    static let formFields = ["mname", "mcmd", "menv", "mcwd", "murl", "mhdr"]

    /// The hub's `ui` reply: strings only.
    static func strings(_ v: Any?) -> [String: String] {
        var m: [String: String] = [:]
        for (k, x) in v as? [String: Any] ?? [:] { if let s = x as? String { m[k] = s } }
        return m
    }
    /// A successful press: the hub's ui is the new state; the typed boxes it leaves behind are emptied.
    mutating func acted(_ action: String, _ value: String, ui newUI: [String: String]) {
        ui = newUI
        if McpState.clearing.contains(action) { typed = [:] }
        else if action == "mcp-view", value.isEmpty || value == "add" || value.hasPrefix("edit:") { for f in McpState.formFields { typed[f] = nil } }
    }
    /// What goes to the hub as `fields`: what was typed, never a vault box's value.
    func sent(vault: Set<String>) -> [String: String] { typed.filter { !vault.contains($0.key) } }
}

/// The vault's Save: `value` packed "name\u{1f}purpose\u{1f}thread" (as the desktop's settings::save_request): secret.set's params, or nil without a name or a value.
func mcpVaultSave(packed: String, value: String) -> [String: String]? {
    let parts = packed.split(separator: "\u{1f}", omittingEmptySubsequences: false).map(String.init)
    let name = (parts.first ?? "").trimmingCharacters(in: .whitespaces), v = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !name.isEmpty, !v.isEmpty else { return nil }
    return ["name": name, "purpose": parts.count > 1 ? parts[1] : "", "thread": parts.count > 2 ? parts[2] : "", "value": v]
}
