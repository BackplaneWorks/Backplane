import Foundation

/// JSON in and out of the hub's frames without ever crashing the app.
///
/// `JSONSerialization.data(withJSONObject:)` does not throw a Swift error for a value it cannot write: it raises an Objective-C
/// `NSInvalidArgumentException`, which `try?` does not catch, and the app aborts. A top-level null, string or number is such a value
/// (TestFlight 202611071658: the hub's `{"k":"work","subs":null}` push when a thread's work ended), and so is NaN or anything that
/// is not a JSON type. Every value that came off the wire, or goes onto it, goes through here.
enum WireJSON {
    /// The value as JSON text, or nil when JSONSerialization would raise on it. Top-level null, strings and numbers are written as fragments.
    static func data(_ v: Any) -> Data? {
        if JSONSerialization.isValidJSONObject(v) { return try? JSONSerialization.data(withJSONObject: v) }
        // a fragment: isValidJSONObject wants an array or object at the top, so check it wrapped in one
        guard JSONSerialization.isValidJSONObject([v]), let d = try? JSONSerialization.data(withJSONObject: [v]) else { return nil }
        return d.count >= 2 ? d.subdata(in: 1..<(d.count - 1)) : nil
    }
    /// The value decoded as T, or nil when it is missing, null, not JSON or not a T.
    static func decode<T: Decodable>(_ v: Any?, as: T.Type = T.self) -> T? {
        guard let v, !(v is NSNull), let d = data(v) else { return nil }
        return try? JSONDecoder().decode(T.self, from: d)
    }
}
