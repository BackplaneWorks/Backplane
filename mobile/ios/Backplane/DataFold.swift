import Foundation

// A thread's rows as the desktop draws them: a run of tool calls and design steps is one row, opened with a tap (the hub's present/thread.rs `entries`, whose
// words test/mobile/fold_labels.json holds this file to; scripts/mobile/fold-check.sh runs it). The same as the Android app's DataFold.kt. Pure: entries in, rows out.
struct DFold: Equatable, Identifiable {
    var id: String          // the run's first entry
    var items: [DEntry]
    var label: String
    var live: Bool
}

enum DRow: Equatable, Identifiable {
    case entry(DEntry)
    case fold(DFold)
    var id: String { switch self { case .entry(let e): e.id; case .fold(let f): "fold:" + f.id } }
}

enum DataFold {
    static func folds(_ e: DEntry) -> Bool { e.kind == "tool" || e.kind == "step" }

    private static func calls(_ n: Int) -> String { "\(n) tool \(n == 1 ? "call" : "calls")" }
    private static func steps(_ n: Int) -> String { "\(n) design \(n == 1 ? "change" : "changes")" }

    static func label(_ run: [DEntry], live: Bool) -> String {
        let tools = run.filter { $0.kind == "tool" }
        let d = run.count - tools.count
        guard let last = tools.last else { return steps(d) }
        if live {
            let name = last.tool?.name ?? ""
            let first = String((last.text.split(separator: "\n", omittingEmptySubsequences: false).first ?? "").prefix(160)).trimmingCharacters(in: .whitespacesAndNewlines)
            let what = name == "To-do" || first.isEmpty ? name : first
            return calls(tools.count) + (d == 0 ? "" : " · " + steps(d)) + " · running: " + what
        }
        var names: [String] = []
        for t in tools { if let n = t.tool?.name, n != "To-do", !names.contains(n) { names.append(n) } }
        var out = calls(tools.count)
        if !names.isEmpty { out += " · " + names.prefix(3).joined(separator: ", ") + (names.count > 3 ? ", …" : "") }
        if d > 0 { out += " · " + steps(d) }
        return out
    }

    /// `running`: the thread's turn runs, so a run at the very end of the thread is still growing.
    static func rows(_ entries: [DEntry], running: Bool) -> [DRow] {
        var out: [DRow] = []; var i = 0
        while i < entries.count {
            guard folds(entries[i]) else { out.append(.entry(entries[i])); i += 1; continue }
            var j = i
            while j < entries.count, folds(entries[j]) { j += 1 }
            let run = Array(entries[i..<j]); let live = running && j == entries.count
            out.append(.fold(DFold(id: run[0].id, items: run, label: label(run, live: live), live: live)))
            i = j
        }
        return out
    }
}
