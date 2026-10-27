import Foundation

// Decoder contexts may contain a URL, a credential or message text. Only
// schema field names and array indices belong in connection diagnostics.
enum WireDecodeDiagnostic {
    private static func field(_ key: CodingKey) -> String {
        if let index = key.intValue { return "[\(index)]" }
        let type = String(reflecting: Swift.type(of: key))
        guard type.hasSuffix(".CodingKeys") || type.hasSuffix(".K") else { return "<key>" }
        return key.stringValue
    }

    static func summary(_ error: Error) -> String {
        let category: String
        let keys: [CodingKey]
        switch error {
        case DecodingError.keyNotFound(let key, let context):
            category = "missing"; keys = context.codingPath + [key]
        case DecodingError.typeMismatch(_, let context):
            category = "type"; keys = context.codingPath
        case DecodingError.valueNotFound(_, let context):
            category = "null"; keys = context.codingPath
        case DecodingError.dataCorrupted(let context):
            category = "invalid"; keys = context.codingPath
        default:
            return "unknown"
        }
        return category + ":" + keys.prefix(16).map(field).joined(separator: ".")
    }
}
