import Foundation

// The cat control's decisions (feature `cats`, docs/PHONE-DATA.md): how a colour typed is read, what a rig is cached by, and what the
// preview asks `cat.rig` while a name is typed. Foundation only, so Tests/CatPickTests.swift compiles it alone.
enum CatPick {
    /// A colour as the hub keeps it: "#rrggbb" in lower case from "#RRGGBB" or "rrggbb" (spaces around are fine); nil for anything else.
    /// (bots/cat.rs `parse_color`: the same rule, so a colour this accepts is never refused.)
    static func hex(_ s: String) -> String? {
        var h = s.trimmingCharacters(in: .whitespaces)
        if h.hasPrefix("#") { h.removeFirst() }
        guard h.count == 6, h.allSatisfy({ $0.isHexDigit }) else { return nil }
        return "#" + h.lowercased()
    }
    /// A rig's cache key: the hub answers the same rig for the same look, colour and mood. A colour that does not parse is the coat's own,
    /// as the hub reads it.
    static func key(look: Int, color: String, mood: String) -> String { "\(look)|\(hex(color) ?? "")|\(mood.isEmpty ? "idle" : mood)" }
    /// Whether the bot's look was picked (Different cat) rather than following its name: the look its name gives (`cat.rig {name}`)
    /// differs from the one `bots.list` has. Not yet known: following the name.
    static func picked(rowLook: Int, nameLook: Int?) -> Bool { nameLook.map { $0 != rowLook } ?? false }
    /// The preview's `cat.rig` parameters while `typed` is in the name field: a picked look stays whatever the name, so it is asked by
    /// look; else by the name typed (the hub hashes it). An empty field shows the bot's own cat.
    static func ask(typed: String, color: String, rowLook: Int, picked: Bool) -> [String: Any] {
        let n = typed.trimmingCharacters(in: .whitespaces)
        if picked || n.isEmpty { return ["look": rowLook, "color": color, "mood": "idle"] }
        return ["name": n, "color": color, "mood": "idle"]
    }
}
