import Foundation

// Native HTTP adapter only. The shared bridge owns every view RPC and its
// authority. Discovery must complete before a socket can offer legacy replay.
enum ViewCapability {
    private final class NoRedirect: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
        func urlSession(_ session: URLSession, task: URLSessionTask,
                        willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest,
                        completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
    }
    private struct Capability: Decodable {
        let version: Int
        let views: Views
        struct Views: Decodable {
            let version: Int
            let websocketQuery: String
            let renderers: [String]
            let compatibility: String
        }
    }
    static func accepts(_ data: Data) -> Bool {
        guard data.count <= 16384, let c = try? JSONDecoder().decode(Capability.self, from: data) else { return false }
        return c.version == 1 && c.views.version == 1 && c.views.websocketQuery == "view=1"
            && c.views.renderers.contains("mobile") && c.views.compatibility == "explicit-only"
    }
    // What discovery found: a hub that is down or starting (refused, a
    // timeout, 502-504) is not a hub without views, and is asked again.
    enum Found { case supported, unsupported, unreachable }

    static func discover(_ link: String) async -> Bool { await find(link) == .supported }

    static func find(_ link: String) async -> Found {
        guard let url = Pairing.http(link, path: "/client/capabilities", token: false) else { return .unsupported }
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 2)
        if let socket = Pairing.socket(link), let token = URLComponents(url: socket, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "token" })?.value {
            request.setValue("Bearer " + token, forHTTPHeaderField: "Authorization")
        }
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 2
        config.timeoutIntervalForResource = 2
        let session = URLSession(configuration: config, delegate: NoRedirect(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        do {
            let (bytes, response) = try await session.bytes(for: request)
            guard let http = response as? HTTPURLResponse else { return .unreachable }
            if [502, 503, 504].contains(http.statusCode) { return .unreachable }
            guard http.statusCode == 200, response.expectedContentLength <= 16384 else { return .unsupported }
            var data = Data()
            for try await byte in bytes {
                guard data.count < 16384 else { return .unsupported }
                data.append(byte)
            }
            return accepts(data) ? .supported : .unsupported
        } catch { return .unreachable }
    }
}
