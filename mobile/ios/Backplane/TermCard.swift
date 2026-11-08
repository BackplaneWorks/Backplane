import Foundation

// What a terminal card in a thread shows (pure; Tests/TermCardTests.swift): a few rows of the screen, and the state words.
enum TermCard {
    /// The card's rows: the last `n` rows of the screen up to the last one with text or the cursor. `cursor` is the cursor's row within them (nil: not shown).
    static func rows(_ lines: [VLine], cursor: VCur, n: Int) -> (lines: [VLine], cursorRow: Int?) {
        var last = lines.lastIndex(where: { !$0.runs.isEmpty }) ?? -1
        if cursor.visible { last = max(last, min(cursor.y, lines.count - 1)) }
        if last < 0 { return ([], nil) }
        let start = max(0, last + 1 - n)
        return (Array(lines[start...last]), cursor.visible && cursor.y >= start && cursor.y <= last ? cursor.y - start : nil)
    }
    /// A command that ended (`done N`) leaves its shell open, so it can still be stopped; `exited N`, `stopped` and `failed to start` are over.
    static func live(_ state: String) -> Bool { state == "running" || state.hasPrefix("done") }
    /// The header's word: the hub's own words, with "failed to start" kept short.
    static func word(_ state: String) -> String { state.isEmpty ? "" : state }
}
