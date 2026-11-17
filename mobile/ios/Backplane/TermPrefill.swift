import Foundation

// Run in terminal's pure part (native/gpui/src/screens/term.rs prefill_bytes, prefill_ready): the bytes that type a command at a shell's prompt
// without running it, and when the shell is ready for them. Tests/CopyableTests.swift.

enum TermPrefill {
    /// How long Run in terminal waits for a shell that never turns on bracketed paste (a plain `sh`) before it types the command anyway.
    static let wait: TimeInterval = 0.9

    /// The bytes that type `cmd` at a shell's prompt without running it (term.rs prefill_bytes). Bracketed paste (?2004) keeps its lines,
    /// none run; else one line: a `\` continuation joined, lines joined with ` && `, comments dropped. Never an Enter.
    static func bytes(_ cmd: String, bpaste: Bool, paste: (String) -> [UInt8]) -> [UInt8] {
        let c = Copyable.exact(cmd)
        if bpaste { return paste(c) }
        let joined = c.replacingOccurrences(of: "\\\r\n", with: " ").replacingOccurrences(of: "\\\n", with: " ")
        let one = joined.replacingOccurrences(of: "\r\n", with: "\n").split(separator: "\n", omittingEmptySubsequences: false)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty && !$0.hasPrefix("#") }.joined(separator: " && ")
        return Array(one.utf8).filter { $0 != 0x1b && $0 != 13 && $0 != 10 }
    }
    static func bytes(_ cmd: String, vt: Vt) -> [UInt8] { bytes(cmd, bpaste: vt.bpaste, paste: vt.paste) }

    /// The shell is at its prompt (term.rs prefill_ready): it turned on bracketed paste, or it printed something and `wait` went by, or 4×`wait`
    /// went by. nil: a full-screen program has the screen (never typed into).
    static func ready(alt: Bool, bpaste: Bool, printed: Bool, waited: TimeInterval) -> Bool? {
        if alt { return nil }
        return bpaste || (printed && waited >= wait) || waited >= wait * 4
    }
    static func ready(_ vt: Vt, waited: TimeInterval) -> Bool? {
        let c = vt.cursor
        return ready(alt: vt.alt, bpaste: vt.bpaste, printed: c.x > 0 || c.y > 0 || vt.history > 0, waited: waited)
    }
}

