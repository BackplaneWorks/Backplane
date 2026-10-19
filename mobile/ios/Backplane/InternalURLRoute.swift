import Foundation

// URL entry and message taps share classification; Ref.parse in Bend owns
// reference syntax, existence and destinations. A pairing flow is explicit.
enum InternalURLRoute: Equatable {
    case reference(String, origin: String?)
    case pair(String)
    case ignored
    case external

    static func route(_ url: URL) -> InternalURLRoute {
        if url.scheme == "backplane" {
            switch url.host {
            case "pair":
                guard let link = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "url" })?.value,
                      !link.isEmpty else { return .ignored }
                return .pair(url.absoluteString)
            case "google": return .ignored // GoogleLoop owns the callback.
            case "open" where url.absoluteString == "backplane://open?pick=1": return .ignored
            default: return .reference(url.absoluteString, origin: nil)
            }
        }
        if (url.scheme == "https" || url.scheme == "http"), url.path.isEmpty || url.path == "/",
           url.fragment?.hasPrefix("/") == true,
           let c = URLComponents(url: url, resolvingAgainstBaseURL: false), let host = c.host {
            return .reference(url.absoluteString, origin: host + (c.port.map { ":" + String($0) } ?? ""))
        }
        return .external
    }

    // Explicit web origin wins over the containing message's hub. An unknown
    // origin stays explicit and is rejected by Bend, never retried on focus.
    static func request(_ uri: String, origin: String?, sourceHub: String?) -> String {
        let object = ["uri": uri, "hub": origin ?? sourceHub ?? ""]
        let data = try! JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        return String(decoding: data, as: UTF8.self)
    }
}
