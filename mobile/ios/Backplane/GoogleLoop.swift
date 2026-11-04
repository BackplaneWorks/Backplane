import AuthenticationServices
import Network
import UIKit

// Google's sign-in from the phone (RFC 8252 7.3, docs/bots.md "Google"):
// a listener on a loopback port of the app's own, the hub asked to begin
// with that redirect ("connect@<port>"), Google's page in an
// authentication session, and the address Google sent the browser back to
// handed to the hub ("done:<address>"), which trades the code. The hub
// keeps the accounts; each sign-in adds one.
@MainActor
final class GoogleLoop: NSObject, ASWebAuthenticationPresentationContextProviding {
    static let shared = GoogleLoop()
    private var listener: NWListener?
    private var session: ASWebAuthenticationSession?
    private var port: UInt16 = 0
    private var opened = false
    private var act: ((String, String) -> Void)?

    // a listener up, then the hub asked for a sign-in to it
    func start(_ act: @escaping (String, String) -> Void) {
        stop()
        self.act = act
        let p = NWParameters.tcp
        p.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        guard let l = try? NWListener(using: p) else { act("google", "connect"); return }
        listener = l
        l.newConnectionHandler = { [weak self] c in Task { @MainActor in self?.serve(c) } }
        l.stateUpdateHandler = { [weak self] st in
            Task { @MainActor in
                guard let self, self.listener === l else { return }
                switch st {
                case .ready:
                    self.port = l.port?.rawValue ?? 0
                    act("google", "connect@\(self.port)")
                case .failed:
                    self.stop()
                    act("google", "connect")
                default: break
                }
            }
        }
        l.start(queue: .main)
    }

    // the hub's sign-in link: opened once, when it sends Google back here
    func open(_ link: String) {
        guard listener != nil, !opened, port != 0, let u = URL(string: link),
              let r = URLComponents(url: u, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "redirect_uri" })?.value,
              r == "http://127.0.0.1:\(port)/oauth/google" else { return }
        opened = true
        let s = ASWebAuthenticationSession(url: u, callbackURLScheme: AppIdentity.scheme) { [weak self] _, _ in
            Task { @MainActor in self?.stop() }
        }
        s.presentationContextProvider = self
        session = s
        if !s.start() { stop() }
    }

    private func serve(_ c: NWConnection) {
        c.start(queue: .main)
        c.receive(minimumIncompleteLength: 1, maximumLength: 16384) { [weak self] data, _, _, _ in
            Task { @MainActor in
                guard let self else { return }
                let head = data.flatMap { String(data: $0, encoding: .utf8) } ?? ""
                let parts = head.split(separator: "\r\n").first?.split(separator: " ") ?? []
                let path = parts.count > 1 ? String(parts[1]) : ""
                guard path.hasPrefix("/oauth/google?") else {
                    c.send(content: Data("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".utf8), completion: .contentProcessed { _ in c.cancel() })
                    return
                }
                let reply = "HTTP/1.1 302 Found\r\nLocation: \(AppIdentity.scheme)://google\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                c.send(content: Data(reply.utf8), completion: .contentProcessed { _ in c.cancel() })
                self.act?("google", "done:http://127.0.0.1:\(self.port)\(path)")
                self.listener?.cancel()
                self.listener = nil
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
