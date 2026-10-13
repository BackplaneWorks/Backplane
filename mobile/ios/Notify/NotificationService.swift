import UserNotifications

// Notification Service Extension: a push with `mutable-content` and a
// top-level `image` (the hub path of a paused browser task's screenshot,
// docs/teach.md) gets the picture attached before it is shown. The hubs
// paired are the app's links, shared through the app group; the token each
// link carries goes along. Every failure shows the alert as it came.
final class NotificationService: UNNotificationServiceExtension {
    private var handler: ((UNNotificationContent) -> Void)?
    private var best: UNMutableNotificationContent?

    override func didReceive(_ request: UNNotificationRequest, withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void) {
        handler = contentHandler
        let content = (request.content.mutableCopy() as? UNMutableNotificationContent) ?? UNMutableNotificationContent()
        best = content
        guard let path = request.content.userInfo["image"] as? String, !path.isEmpty else { return contentHandler(content) }
        let links = UserDefaults(suiteName: Self.group)?.stringArray(forKey: "links") ?? []
        Task {
            for l in links {
                guard let url = Self.web(l, path), let a = await Self.fetch(url) else { continue }
                content.attachments = [a]
                break
            }
            contentHandler(content)
        }
    }

    override func serviceExtensionTimeWillExpire() {
        if let h = handler, let c = best { h(c) }
    }

    static let group = "group.dev.backplane.mobile"

    // the hub's address for path over http(s), with the link's token
    static func web(_ link: String, _ path: String) -> URL? {
        var s = link.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.hasPrefix("backplane://") {
            guard let q = URLComponents(string: s)?.queryItems?.first(where: { $0.name == "url" })?.value else { return nil }
            s = q
        }
        if !s.contains("://") { s = "http://" + s }
        guard let u = URLComponents(string: s), let host = u.host, let p = URLComponents(string: path) else { return nil }
        let rest = (u.fragment ?? "") + "&" + (u.query ?? "")
        let token = rest.range(of: "token=[0-9a-f]+", options: .regularExpression).map { String(rest[$0].dropFirst(6)) }
        var c = URLComponents()
        c.scheme = u.scheme == "https" || u.scheme == "wss" ? "https" : "http"
        c.host = host
        c.port = u.port
        c.path = p.path
        c.percentEncodedQuery = [p.percentEncodedQuery, token.map { "token=" + $0 }].compactMap { $0 }.joined(separator: "&")
        return c.url
    }

    static func fetch(_ url: URL) async -> UNNotificationAttachment? {
        var r = URLRequest(url: url)
        r.timeoutInterval = 15
        guard let (data, resp) = try? await URLSession.shared.data(for: r), (resp as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        let file = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        guard (try? data.write(to: file)) != nil else { return nil }
        return try? UNNotificationAttachment(identifier: "shot", url: file, options: nil)
    }
}
