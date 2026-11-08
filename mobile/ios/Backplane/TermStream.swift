import Foundation

// A terminal's output as the hub streams it (docs/PHONE-DATA.md, Terminals; native/host/src/term/stream.rs): each run `g` counts the bytes it printed, `o`, and every
// live frame says where its bytes start. This is the offset rule (`merge`, the same cases as stream.rs's tests) and what a phone does with one push (`apply`).
// Pure: no socket, no screen.
enum TermMerge: Equatable {
    case skip            // it holds all of it already
    case take(Int)       // feed the frame's bytes from this index on; it then holds up to `o + len`
    case gap             // bytes between are missing: ask `term.read {from: held}`
}

/// What the phone holds of a terminal: run `g` up to offset `o`.
struct TermHeld: Equatable { var g: Int; var o: Int }

/// What a push asks of the phone.
enum TermAction: Equatable {
    case none
    case state(String)                                          // the terminal's state words
    case feed([UInt8], held: TermHeld)                          // feed these bytes to the screen, then hold this
    case snapshot(cols: Int, rows: Int, [UInt8], held: TermHeld) // a new screen of this size, drawn by these bytes (history stays when it is the same run)
    case reread                                                 // asks `term.read` again with what it holds
}

enum TermStream {
    /// What a client does with a live frame of the run it holds, whose bytes start at `o` and are `len` long, while it holds up to `held`.
    static func merge(held: Int, o: Int, len: Int) -> TermMerge {
        let end = o + len
        if o > held { return .gap }
        if end <= held { return .skip }
        return .take(held - o)
    }

    /// A push `{t:"d", k:"term", key, b, g?, o?, reset?, state?}` (already decoded: `bytes`, `reset` is [cols, rows]) against what is held (nil: nothing).
    /// A state is returned first when there is one; the bytes part follows as a second action.
    static func apply(held: TermHeld?, g: Int?, o: Int?, bytes: [UInt8], reset: [Int]?, state: String?) -> [TermAction] {
        var out: [TermAction] = []
        if let state { out.append(.state(state)) }
        guard let g, let o else { if !bytes.isEmpty || reset != nil { out.append(.reread) }; return out }
        if let reset, reset.count == 2 { out.append(.snapshot(cols: reset[0], rows: reset[1], bytes, held: TermHeld(g: g, o: o))); return out }
        // a push of another `g`, or with no `g`, means read again (nothing held: only when it carries bytes)
        guard let held else { if !bytes.isEmpty { out.append(.reread) }; return out }
        if held.g != g { out.append(.reread); return out }
        if bytes.isEmpty { return out }
        switch merge(held: held.o, o: o, len: bytes.count) {
        case .skip: break
        case .gap: out.append(.reread)
        case .take(let i): out.append(.feed(Array(bytes[i...]), held: TermHeld(g: g, o: o + bytes.count)))
        }
        return out
    }
}
