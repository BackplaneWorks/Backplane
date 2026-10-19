import Foundation

// Inspect only the small leading fields of the top-level CBOR envelope.
// History/changes always stay in their original FIFO. Only the mobile
// greeting and replies in its reserved request-id range use the person lane.
final class WirePriority: @unchecked Sendable {
    private struct Dictionaries: Decodable { let keys, words: [String] }
    private let lock = NSLock()
    private var dictionaries: Dictionaries?

    func install(_ text: String) {
        let decoded = try? JSONDecoder().decode(Dictionaries.self, from: Data(text.utf8))
        lock.withLock { dictionaries = decoded }
    }

    func person(_ data: Data) -> Bool {
        guard let dictionary = lock.withLock({ dictionaries }) else { return false }
        return data.withUnsafeBytes { bytes in
            var cursor = Cursor(bytes: bytes)
            guard let (major, count) = cursor.head(), major == 5, count <= 32 else { return false }
            for _ in 0..<count {
                guard let key = cursor.atom(keys: dictionary.keys, words: dictionary.words, key: true),
                      let value = cursor.atom(keys: dictionary.keys, words: dictionary.words, key: false) else { return false }
                if key == "t", value == "mobile" { return true }
                if key == "id" { return (UInt64(value) ?? 0) >= 4_000_000_000_000 }
            }
            return false
        }
    }

    private struct Cursor {
        let bytes: UnsafeRawBufferPointer
        var at = 0
        mutating func byte() -> UInt8? {
            guard at < bytes.count else { return nil }
            defer { at += 1 }
            return bytes[at]
        }
        mutating func head() -> (UInt8, UInt64)? {
            guard let first = byte() else { return nil }
            let additional = first & 31
            if additional < 24 { return (first >> 5, UInt64(additional)) }
            let count: Int
            switch additional {
            case 24: count = 1
            case 25: count = 2
            case 26: count = 4
            case 27: count = 8
            default: return nil
            }
            var value: UInt64 = 0
            for _ in 0..<count {
                guard let next = byte() else { return nil }
                value = (value << 8) | UInt64(next)
            }
            return (first >> 5, value)
        }
        mutating func atom(keys: [String], words: [String], key: Bool) -> String? {
            guard let (major, value) = head() else { return nil }
            if major == 0 {
                if key { return value < UInt64(keys.count) ? keys[Int(value)] : nil }
                return String(value)
            }
            if major == 3 {
                guard value <= 512, value <= UInt64(bytes.count - at) else { return nil }
                let end = at + Int(value)
                defer { at = end }
                return String(bytes: bytes[at..<end], encoding: .utf8)
            }
            if major == 6, value == 6 {
                guard let (kind, index) = head(), kind == 0, index < UInt64(words.count) else { return nil }
                return words[Int(index)]
            }
            if major == 6, value == 7 { return atom(keys: keys, words: words, key: false) }
            if major == 7 { return String(value) }
            // Never walk an items/result container, including a huge log.
            return nil
        }
    }
}
