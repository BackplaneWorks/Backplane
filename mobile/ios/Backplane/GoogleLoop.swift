import AuthenticationServices
import Network
import UIKit

// Google's sign-in from the phone (docs/GOOGLE.md "Sign-in", data/1): a listener on a loopback port of the app's own, the hub asked to begin with
// that redirect (account.act `redirect`), Google's page in an authentication session, and the address the browser came back to sent to the hub as
// `code`. The page it answers is a 302 to backplane://google (which the app's link handler ignores).
@MainActor
final class GoogleLoop: NSObject, ASWebAuthenticationPresentationContextProviding {
    static let shared = GoogleLoop()
    private var listener: NWListener?
    private var session: ASWebAuthenticationSession?
    private(set) var port: UInt16 = 0
    private var opened = false
    /// The account the caught address belongs to (its id from the hub's reply).
    var profile = ""
    /// Sends the caught address to the hub (profile, address); set by the account screen.
    var onCatch: ((String, String) -> Void)?

    /// Opens the listener; its port, or nil (then no redirect is sent and the hub's paste path works as it is).
    func listen() async -> UInt16? {
        stop()
        let p = NWParameters.tcp
        p.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        guard let l = try? NWListener(using: p) else { return nil }
        listener = l
        l.newConnectionHandler = { [weak self] c in Task { @MainActor in self?.serve(c, l) } }
        let ok: Bool = await withCheckedContinuation { k in
            var done = false
            l.stateUpdateHandler = { st in
                if done { return }
                switch st {
                case .ready: done = true; k.resume(returning: true)
                case .failed, .cancelled: done = true; k.resume(returning: false)
                default: break
                }
            }
            l.start(queue: .main)
        }
        guard ok, listener === l, let pt = l.port?.rawValue else { stop(); return nil }
        port = pt
        return pt
    }

    /// The hub's sign-in link, opened once and only when it sends Google back to this listener.
    func open(_ link: String) {
        guard listener != nil, !opened, port != 0, GoogleLoopLogic.matches(link, GoogleLoopLogic.redirect(port)), let u = URL(string: link) else { return }
        opened = true
        let s = ASWebAuthenticationSession(url: u, callbackURLScheme: AppIdentity.scheme) { _, _ in }
        s.presentationContextProvider = self
        session = s
        if !s.start() { session = nil }
    }

    private func serve(_ c: NWConnection, _ l: NWListener) {
        c.start(queue: .main)
        c.receive(minimumIncompleteLength: 1, maximumLength: 16384) { [weak self] data, _, _, _ in
            Task { @MainActor in
                guard let self else { return }
                let path = GoogleLoopLogic.requestPath(data.flatMap { String(data: $0, encoding: .utf8) } ?? "")
                guard GoogleLoopLogic.isCatch(path), let path else {
                    c.send(content: Data("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".utf8), completion: .contentProcessed { _ in c.cancel() })
                    return
                }
                let reply = "HTTP/1.1 302 Found\r\nLocation: \(AppIdentity.scheme)://google\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                c.send(content: Data(reply.utf8), completion: .contentProcessed { _ in c.cancel() })
                let address = "http://127.0.0.1:\(self.port)\(path)", who = self.profile
                if self.listener === l { self.stop() }
                self.onCatch?(who, address)
            }
        }
    }

    func stop() {
        listener?.cancel()
        listener = nil
        session = nil
        opened = false
        port = 0
    }

    nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated {
            UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
                .flatMap(\.windows).first(where: \.isKeyWindow) ?? ASPresentationAnchor()
        }
    }
}
