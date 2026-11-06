#if GPUI_NATIVE
import SwiftUI

// The GPUI experiment's build has no web shell: nothing here creates or links
// WKWebView for the app's screens. Only the two id helpers survive.
enum WebShell {
    static func local(_ thread: String) -> String {
        guard let bar = thread.firstIndex(of: "|") else { return thread }
        return String(thread[thread.index(after: bar)...])
    }
}

struct AppRoot: View {
    @Bindable var model: AppModel
    var body: some View { RootView(model: model) }
}
#else
import AVFoundation
import SwiftUI
import UIKit
import UserNotifications
import WebKit

// The app's screen: the paired hub's phone web client (docs/phone-web.md)
// in a WKWebView, the page a phone's browser gets, with what a page cannot
// do kept native (docs/phone-app-shell.md: window.backplaneApp, bridge 1).
// AppModel still runs every hub's socket and client behind it, so
// alerts, the Live Activity, push registration and pairing are what they
// were; the native screens come back with "Classic screens"
// (AppSettings.classic).
//
// The page is handed the pairing once: the link's token rides in the
// address's fragment (#token=…, never sent over the network), the page
// trades it at /auth/pair for its HttpOnly cookie in this app's cookie
// store and drops it from its history; later opens carry no secret.
@MainActor
@Observable
final class WebShell: NSObject {
    enum Phase: Equatable { case loading, shown, unreachable }
    private(set) var phase = Phase.loading
    // the hub on screen (Pairing.key) and its pairing link
    private(set) var hub = ""
    @ObservationIgnored private var link = ""
    // counts the WKWebViews made (the host swaps in each new one)
    private(set) var page = 0
    // what the page says it shows (setState): the open thread, whose
    // alerts stay quiet while the app is in front
    private(set) var thread = ""
    // the page's theme (setState): the window, its status bar and the sheets
    // over the page follow it (BackplaneApp's preferredColorScheme)
    private(set) var theme: ColorScheme?
    // the page's pairing was refused (pairingFailed): the app re-pairs
    var refused = false
    // a page of the hub's own the page opened as a new window
    var popup: WKWebView?
    // the app's own settings over the page (openAppSettings)
    var settings = false
    @ObservationIgnored private(set) var webView: WKWebView
    // a thread asked for (a notification, the island) before the page was up
    @ObservationIgnored private var pending: String?
    // the page's bridge said hello: events reach it (the shim keeps those
    // sent before the page listens)
    @ObservationIgnored private var bridged = false
    @ObservationIgnored private var downloads: [ObjectIdentifier: URL] = [:]
    @ObservationIgnored private var retry: Timer?
    @ObservationIgnored private var away: Date?
    // what AppModel answers for the page
    @ObservationIgnored var pair: (URL) -> Void = { _ in }
    @ObservationIgnored var switchTo: (String, URL?) -> Bool = { _, _ in false }
    @ObservationIgnored var hubs: () -> [[String: Any]] = { [] }
    @ObservationIgnored var name: (String) -> String = { $0 }
    @ObservationIgnored var addHub: () -> Void = {}
    @ObservationIgnored var enableNotifications: () async -> Void = {}
    // an alert the page was sent (a desk's Notify): thread ("host:port|id"), key, title, body
    @ObservationIgnored var alert: (String, String, String, String) -> Void = { _, _, _, _ in }

    override init() {
        webView = WKWebView(frame: .zero)
        super.init()
        webView = make()
        _ = NotificationCenter.default.addObserver(forName: UIContentSizeCategory.didChangeNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.textSize() }
        }
    }

    private func make() -> WKWebView {
        let c = WKWebViewConfiguration()
        c.websiteDataStore = .default()
        c.allowsInlineMediaPlayback = true
        c.defaultWebpagePreferences.preferredContentMode = .mobile
        let u = c.userContentController
        if let s = BackplaneAppBridge.script(hub: hub, name: name(hub), textScale: Self.zoom) {
            u.addUserScript(WKUserScript(source: s, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        }
        u.add(WeakHandler(self), name: BackplaneAppBridge.channel)
        let w = WKWebView(frame: .zero, configuration: c)
        w.navigationDelegate = self
        w.uiDelegate = self
        // the page pads itself with the safe-area insets (viewport-fit=cover)
        // and never scrolls as a whole; its own lists scroll
        w.scrollView.contentInsetAdjustmentBehavior = .never
        w.scrollView.bounces = false
        // Back is the page's: its ‹, and its own swipe in from the left edge,
        // which moves the screen with the finger (WebKit's would race it)
        w.allowsBackForwardNavigationGestures = false
        w.allowsLinkPreview = false
        w.isOpaque = false
        w.backgroundColor = .systemBackground
        w.underPageBackgroundColor = .systemBackground
        #if DEBUG
        if #available(iOS 16.4, *) { w.isInspectable = true }
        #endif
        w.pageZoom = Self.zoom
        w.overrideUserInterfaceStyle = Self.system
        return w
    }

    // The phone's own appearance. The page's prefers-color-scheme reads it
    // (a page whose theme is unset follows the phone), not the window's,
    // which follows the page: else the page would follow itself.
    private static var system: UIUserInterfaceStyle {
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        return scene?.screen.traitCollection.userInterfaceStyle ?? .unspecified
    }

    // Dynamic Type: the page is zoomed with the system's text size, within
    // what the phone layout still fits (a page at least 314 pt wide)
    static var zoom: CGFloat {
        let scale = UIFontMetrics(forTextStyle: .body).scaledValue(for: 100) / 100
        return min(1.25, max(0.85, (scale * 100).rounded() / 100))
    }

    private func textSize() { webView.pageZoom = Self.zoom }

    // where the page is: the hub's own address with the phone layout asked
    // for, and the pairing token once (the page has no cookie for it yet)
    private static func address(_ link: String, token: String?) -> URL? {
        guard let base = Pairing.http(link, path: "/", token: false),
              var c = URLComponents(url: base, resolvingAgainstBaseURL: false) else { return nil }
        c.queryItems = [URLQueryItem(name: "phone", value: "1")]
        if let token { c.fragment = "token=" + token }
        return c.url
    }

    static func token(_ link: String) -> String? {
        Pairing.socket(link).flatMap { URLComponents(url: $0, resolvingAgainstBaseURL: false) }?
            .queryItems?.first(where: { $0.name == "token" })?.value
    }

    // an address on this hub's origin (host and port)
    func ours(_ url: URL?) -> Bool {
        guard let url, url.scheme == "http" || url.scheme == "https", let host = url.host() else { return false }
        return !hub.isEmpty && (url.port.map { "\(host):\($0)" } ?? host) == hub
    }

    private func ours(_ origin: WKSecurityOrigin) -> Bool {
        guard origin.protocol == "http" || origin.protocol == "https" else { return false }
        return !hub.isEmpty && (origin.port > 0 ? "\(origin.host):\(origin.port)" : origin.host) == hub
    }

    // show a hub's page, at url when given (a link from another hub's page);
    // a new WKWebView, so nothing of another hub's page stays behind
    func show(_ key: String, link: String, at url: URL? = nil, again: Bool = false) {
        guard key != hub || link != self.link || url != nil || again else { return }
        retire()
        hub = key
        self.link = link
        thread = ""
        refused = false
        webView = make()
        page += 1
        load(at: url, token: again)
    }

    // nothing paired any more
    func clear() {
        retire()
        hub = ""
        link = ""
        thread = ""
        refused = false
        phase = .loading
        page += 1
    }

    private func retire() {
        retry?.invalidate()
        retry = nil
        bridged = false
        popup = nil
        webView.stopLoading()
        webView.configuration.userContentController.removeAllScriptMessageHandlers()
        webView.navigationDelegate = nil
        webView.uiDelegate = nil
    }

    // token: hand the page the pairing even when its cookie looks current
    private func load(at url: URL? = nil, token again: Bool = false) {
        phase = .loading
        bridged = false
        let link = self.link, w = webView
        let token = Self.token(link)
        w.configuration.websiteDataStore.httpCookieStore.getAllCookies { [weak self] cookies in
            MainActor.assumeIsolated {
                guard let self, self.webView === w, self.link == link else { return }
                let host = Pairing.http(link, path: "/", token: false)?.host() ?? ""
                let kept = !again && token != nil && cookies.contains {
                    $0.name == "bp_token" && $0.value == token && $0.domain.trimmingCharacters(in: CharacterSet(charactersIn: ".")) == host
                }
                let page = kept ? url.flatMap { self.ours($0) ? $0 : nil } ?? Self.address(link, token: nil) : Self.address(link, token: token)
                guard let page else { self.unreachable(); return }
                w.load(URLRequest(url: page))
            }
        }
    }

    func reload() {
        guard !hub.isEmpty else { return }
        if phase == .shown, webView.url != nil { webView.reload() } else { load() }
    }

    // the page's token was refused and the person chose to try it again
    func repairRetry() {
        guard !hub.isEmpty else { return }
        show(hub, link: link, again: true)
    }

    // the app comes and goes: the page hears it (resume, pause), and a page
    // that never loaded is tried again
    func foreground(_ yes: Bool) {
        if yes {
            webView.overrideUserInterfaceStyle = Self.system
            let ms = away.map { Int(Date().timeIntervalSince($0) * 1000) } ?? 0
            away = nil
            send("resume", ["awayMs": ms])
            if phase == .unreachable { load() }
        } else {
            away = Date()
            send("pause")
        }
    }

    // a thread to show (a notification's or the island's tap)
    func open(_ thread: String) {
        let id = Self.local(thread)
        guard !id.isEmpty else { return }
        guard bridged else { pending = id; return }
        send("open-thread", ["thread": id])
    }

    // "host:port|id" is the
    // page's id; a bare id (the hub's own push) is already
    static func local(_ thread: String) -> String {
        guard let bar = thread.firstIndex(of: "|") else { return thread }
        return String(thread[thread.index(after: bar)...])
    }

    static func owner(_ thread: String) -> String? {
        thread.firstIndex(of: "|").map { String(thread[..<$0]) }
    }

    // MARK: the bridge (mobile/shell/backplane-app.js)

    private func deliver(_ message: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: message),
              let json = String(data: data, encoding: .utf8) else { return }
        webView.evaluateJavaScript("window.backplaneApp && window.backplaneApp._receive(\(json))", completionHandler: nil)
    }

    private func send(_ event: String, _ data: [String: Any] = [:]) {
        guard bridged else { return }
        deliver(["event": event, "data": data])
    }

    private func answer(_ id: Int?, _ value: Any) {
        guard let id else { return }
        deliver(["id": id, "ok": true, "value": value])
    }

    private func refuse(_ id: Int?, _ error: String) {
        guard let id else { return }
        deliver(["id": id, "ok": false, "error": error])
    }

    fileprivate func received(_ message: WKScriptMessage) {
        // only the hub's own page, in the main frame
        guard message.frameInfo.isMainFrame, message.webView === webView, ours(message.frameInfo.securityOrigin),
              let text = message.body as? String, let data = text.data(using: .utf8),
              let msg = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let op = msg["op"] as? String else { return }
        let id = msg["id"] as? Int
        let args = msg["args"] as? [String: Any] ?? [:]
        switch op {
        case "hello":
            bridged = true
            if let p = pending { pending = nil; send("open-thread", ["thread": p]) }
        case "setState":
            if let t = args["thread"] as? String { thread = t }
            if let t = args["theme"] as? String { theme = t == "light" ? .light : t == "dark" ? .dark : nil }
            if let n = args["needs"] as? Int { UNUserNotificationCenter.current().setBadgeCount(max(0, n), withCompletionHandler: nil) }
            if let bg = (args["bg"] as? String).flatMap(Self.color) {
                webView.backgroundColor = bg
                webView.underPageBackgroundColor = bg
            }
        case "requestCamera":
            Task {
                switch AVCaptureDevice.authorizationStatus(for: .video) {
                case .authorized: answer(id, true)
                case .notDetermined: answer(id, await AVCaptureDevice.requestAccess(for: .video))
                default: answer(id, false)
                }
            }
        case "share":
            var items: [Any] = []
            if let t = args["text"] as? String, !t.isEmpty { items.append(t) }
            if let s = args["url"] as? String, let u = URL(string: s), u.scheme != nil { items.append(u) }
            if items.isEmpty, let t = args["title"] as? String, !t.isEmpty { items.append(t) }
            share(items)
            answer(id, !items.isEmpty)
        case "openExternal":
            guard let s = args["url"] as? String, let u = URL(string: s),
                  ["http", "https", "mailto", "tel"].contains(u.scheme?.lowercased() ?? "") else { return answer(id, false) }
            UIApplication.shared.open(u)
            answer(id, true)
        case "haptic":
            switch args["kind"] as? String {
            case "success": Haptics.notify(.success)
            case "warning": Haptics.notify(.warning)
            case "error": Haptics.notify(.error)
            case "selection": Haptics.selection()
            case "medium": Haptics.impact(.medium)
            default: Haptics.impact(.light)
            }
        case "hubs":
            answer(id, hubs())
        case "switchHub":
            guard let key = args["key"] as? String else { return answer(id, false) }
            answer(id, key == hub || switchTo(key, nil))
        case "addHub":
            addHub()
        case "openAppSettings":
            settings = true
        case "notifications":
            Task {
                let s = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
                answer(id, ["allowed": Self.allowed(s), "asked": s != .notDetermined])
            }
        case "requestNotifications":
            Task {
                let before = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
                if before == .denied, let u = URL(string: UIApplication.openNotificationSettingsURLString) {
                    _ = await UIApplication.shared.open(u)
                } else if before == .notDetermined {
                    await enableNotifications()
                }
                let s = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
                answer(id, ["allowed": Self.allowed(s)])
            }
        case "notify":
            // posted like the app's own alerts (one per key, so the same alert twice shows once)
            let t = args["thread"] as? String ?? ""
            alert(t.isEmpty ? "" : hub + "|" + t, args["key"] as? String ?? "", args["title"] as? String ?? "", args["body"] as? String ?? "")
        case "pairingFailed":
            // a hub out of reach is not a refusal: the page retries on its own
            if args["cause"] as? String != "hub_unreachable" { refused = true }
        default:
            refuse(id, "unknown op")
        }
    }

    private static func allowed(_ s: UNAuthorizationStatus) -> Bool {
        s == .authorized || s == .provisional || s == .ephemeral
    }

    // a CSS colour as the page reports it: #rgb, #rrggbb or rgb(r, g, b)
    static func color(_ css: String) -> UIColor? {
        let s = css.trimmingCharacters(in: .whitespaces).lowercased()
        var parts: [Double] = []
        if s.hasPrefix("#") {
            var hex = String(s.dropFirst())
            if hex.count == 3 { hex = hex.map { "\($0)\($0)" }.joined() }
            guard hex.count == 6, let v = UInt32(hex, radix: 16) else { return nil }
            parts = [Double((v >> 16) & 0xff), Double((v >> 8) & 0xff), Double(v & 0xff)]
        } else if s.hasPrefix("rgb"), let open = s.firstIndex(of: "("), let close = s.lastIndex(of: ")"), open < close {
            parts = s[s.index(after: open)..<close].split(whereSeparator: { $0 == "," || $0 == " " || $0 == "/" })
                .prefix(3).compactMap { Double($0) }
        }
        guard parts.count == 3 else { return nil }
        return UIColor(red: parts[0] / 255, green: parts[1] / 255, blue: parts[2] / 255, alpha: 1)
    }

    // MARK: what the shell does for the page

    private func up() {
        phase = .shown
        retry?.invalidate()
        retry = nil
    }

    private func unreachable() {
        phase = .unreachable
        retry?.invalidate()
        retry = Timer.scheduledTimer(withTimeInterval: 15, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.phase == .unreachable, UIApplication.shared.applicationState == .active else { return }
                self.load()
            }
        }
    }

    private func present(_ vc: UIViewController) {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        guard var top = scenes.flatMap(\.windows).first(where: \.isKeyWindow)?.rootViewController else { return }
        while let next = top.presentedViewController { top = next }
        if let pop = vc.popoverPresentationController {
            pop.sourceView = webView
            pop.sourceRect = CGRect(x: webView.bounds.midX, y: webView.bounds.maxY - 80, width: 1, height: 1)
        }
        top.present(vc, animated: true)
    }

    private func share(_ items: [Any]) {
        guard !items.isEmpty else { return }
        let vc = UIActivityViewController(activityItems: items, applicationActivities: nil)
        // Save Image would need the photo library's permission; Save to Files does not
        vc.excludedActivityTypes = [.saveToCameraRoll]
        present(vc)
    }

    private static func external(_ url: URL) {
        UIApplication.shared.open(url)
    }
}

extension WebShell: WKNavigationDelegate {
    func webView(_ w: WKWebView, decidePolicyFor a: WKNavigationAction,
                 decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let u = a.request.url else { return decisionHandler(.cancel) }
        if a.shouldPerformDownload { return decisionHandler(.download) }
        switch u.scheme?.lowercased() {
        case "backplane":
            decisionHandler(.cancel)
            pair(u)
        case "about", "blob", "data":
            decisionHandler(.allow)
        case "http", "https":
            if ours(u) { return decisionHandler(.allow) }
            // a page in a frame (a PDF, a preview) loads where it is
            if a.targetFrame?.isMainFrame == false { return decisionHandler(.allow) }
            decisionHandler(.cancel)
            // another paired hub's page (Connect on a bot elsewhere) opens in the app, the rest outside it
            if let host = u.host(), switchTo(u.port.map { "\(host):\($0)" } ?? host, u) { return }
            Self.external(u)
        default:
            decisionHandler(.cancel)
            Self.external(u)
        }
    }

    func webView(_ w: WKWebView, decidePolicyFor r: WKNavigationResponse,
                 decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        decisionHandler(r.canShowMIMEType ? .allow : .download)
    }

    func webView(_ w: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload) {
        download.delegate = self
    }

    func webView(_ w: WKWebView, navigationResponse: WKNavigationResponse, didBecome download: WKDownload) {
        download.delegate = self
    }

    func webView(_ w: WKWebView, didFinish navigation: WKNavigation!) {
        guard w === webView else { return }
        up()
    }

    func webView(_ w: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        guard w === webView, Self.failed(error) else { return }
        unreachable()
    }

    func webView(_ w: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        guard w === webView, phase != .shown, Self.failed(error) else { return }
        unreachable()
    }

    // a cancelled load (another replaced it) or one that became a download is no failure
    private static func failed(_ error: Error) -> Bool {
        let e = error as NSError
        if e.domain == NSURLErrorDomain, e.code == NSURLErrorCancelled { return false }
        if e.domain == "WebKitErrorDomain", e.code == 102 { return false }
        return true
    }

    // iOS ended the page's process (memory): open it again
    func webViewWebContentProcessDidTerminate(_ w: WKWebView) {
        guard w === webView else { return }
        bridged = false
        load()
    }
}

extension WebShell: WKUIDelegate {
    // a new window: the hub's own page (an image, a file) in a sheet that
    // shares this page's cookies; anything else outside the app
    func webView(_ w: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
                 for a: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        guard let u = a.request.url else { return nil }
        guard ours(u) else {
            if u.scheme == "backplane" { pair(u) } else if u.scheme != "about" { Self.external(u) }
            return nil
        }
        let p = WKWebView(frame: .zero, configuration: configuration)
        p.navigationDelegate = self
        p.uiDelegate = self
        p.allowsBackForwardNavigationGestures = true
        popup = p
        return p
    }

    func webViewDidClose(_ w: WKWebView) {
        if w === popup { popup = nil }
    }

    // the camera and microphone for the hub's own page; iOS still asks the
    // person the first time (NSCameraUsageDescription)
    func webView(_ w: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo,
                 type: WKMediaCaptureType, decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        decisionHandler(ours(origin) ? .grant : .deny)
    }

    func webView(_ w: WKWebView, runJavaScriptAlertPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping () -> Void) {
        let a = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        a.addAction(UIAlertAction(title: "OK", style: .default) { _ in completionHandler() })
        present(a)
    }

    func webView(_ w: WKWebView, runJavaScriptConfirmPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping (Bool) -> Void) {
        let a = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        a.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in completionHandler(false) })
        a.addAction(UIAlertAction(title: "OK", style: .default) { _ in completionHandler(true) })
        present(a)
    }
}

// a file the page saves (a download, a blob it made): kept in the app's
// temporary folder and offered to the share sheet (Save to Files, AirDrop)
extension WebShell: WKDownloadDelegate {
    func download(_ download: WKDownload, decideDestinationUsing response: URLResponse, suggestedFilename: String,
                  completionHandler: @escaping (URL?) -> Void) {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("downloads", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let name = (suggestedFilename as NSString).lastPathComponent
        let to = dir.appendingPathComponent(name.isEmpty ? "download" : name)
        downloads[ObjectIdentifier(download)] = to
        completionHandler(to)
    }

    func downloadDidFinish(_ download: WKDownload) {
        guard let to = downloads.removeValue(forKey: ObjectIdentifier(download)) else { return }
        share([to])
    }

    func download(_ download: WKDownload, didFailWithError error: Error, resumeData: Data?) {
        downloads[ObjectIdentifier(download)] = nil
        NSLog("download failed: %@ %d", (error as NSError).domain, (error as NSError).code)
    }
}

// WKUserContentController keeps its handlers strongly
private final class WeakHandler: NSObject, WKScriptMessageHandler {
    weak var shell: WebShell?
    init(_ shell: WebShell) { self.shell = shell }

    func userContentController(_ c: WKUserContentController, didReceive message: WKScriptMessage) {
        MainActor.assumeIsolated { shell?.received(message) }
    }
}

// MARK: the screen

// The hub's phone web client, or the native screens: Classic screens, and
// an app not paired yet (it pairs natively, then hands the page the pairing)
struct AppRoot: View {
    @Bindable var model: AppModel

    var body: some View {
        if model.webShown {
            WebShellView(model: model, shell: model.web)
        } else {
            RootView(model: model)
        }
    }
}

struct WebShellView: View {
    @Bindable var model: AppModel
    let shell: WebShell

    private var tag: String { model.screen?.locale ?? "en" }

    var body: some View {
        @Bindable var shell = shell
        ZStack {
            WebHost(shell: shell, page: shell.page).ignoresSafeArea()
            if shell.refused || model.conn[shell.hub]?.failure == .authentication {
                // the hub no longer takes this phone's pairing (revoked): a new invitation
                NavigationStack {
                    PairView(tag: tag, link: "", status: model.pairingError, repair: true) { _ = model.pair($0) }
                        .toolbar { ToolbarItem(placement: .cancellationAction) { Button(tag == "ja" ? "再試行" : "Try again") { shell.repairRetry() } } }
                }
            } else if shell.phase == .unreachable {
                Unreachable(model: model, shell: shell)
            }
        }
        .background(Color(uiColor: .systemBackground).ignoresSafeArea())
        .sheet(isPresented: Binding(get: { shell.popup != nil }, set: { if !$0 { shell.popup = nil } })) {
            if let p = shell.popup { Popup(web: p) }
        }
        // the hubs this phone is paired with: switch, unpair, pair another
        .sheet(isPresented: $model.webPairing) {
            NavigationStack {
                Group {
                    if let s = model.screen {
                        HubsView(model: model, screen: s)
                    } else {
                        PairView(tag: tag, link: "", status: model.pairingError) { _ = model.pair($0) }
                    }
                }
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button(NativeMessages.tour_done(tag)) { model.webPairing = false } } }
            }
        }
        .sheet(isPresented: $shell.settings) { ShellSettings(model: model) }
        .onAppear { model.syncWeb() }
        // a hub just paired (pair() puts its link last) is shown; an unpaired one goes
        .onChange(of: model.links) { old, new in
            let fresh = new.last.flatMap { old.contains($0) ? nil : Pairing.key($0) }
            if fresh != nil { model.webPairing = false; shell.settings = false }
            model.syncWeb(prefer: fresh)
        }
    }
}

// the shell's WKWebView, swapped in when the shell makes a new one (another hub)
private struct WebHost: UIViewRepresentable {
    let shell: WebShell
    let page: Int

    func makeUIView(context: Context) -> UIView {
        let v = UIView()
        v.backgroundColor = .systemBackground
        return v
    }

    func updateUIView(_ v: UIView, context: Context) {
        let w = shell.webView
        guard w.superview !== v else { return }
        v.subviews.forEach { $0.removeFromSuperview() }
        w.frame = v.bounds
        w.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        v.addSubview(w)
    }
}

private struct Popup: View {
    let web: WKWebView
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Page(web: web).ignoresSafeArea(edges: .bottom)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }

    private struct Page: UIViewRepresentable {
        let web: WKWebView
        func makeUIView(context: Context) -> WKWebView { web }
        func updateUIView(_ w: WKWebView, context: Context) {}
    }
}

// the hub's page could not be loaded: which hub, Retry (it is also tried
// every 15 s and on coming back to the app), the other paired hubs, and the
// classic screens, which show the state this phone kept
private struct Unreachable: View {
    @Bindable var model: AppModel
    let shell: WebShell

    private var tag: String { model.screen?.locale ?? "en" }

    var body: some View {
        VStack(spacing: 16) {
            Spacer()
            Image(systemName: "wifi.exclamationmark").font(.system(size: 44)).foregroundStyle(.secondary)
            Text(tag == "ja" ? "\(model.hubName(shell.hub)) に接続できません" : "Can't reach \(model.hubName(shell.hub))")
                .font(.headline).multilineTextAlignment(.center)
            Text(tag == "ja" ? "コンピューターの電源が入っていて、このスマートフォンが同じネットワークか tailnet にあるか確認してください。"
                             : "Check that the computer is on and that this phone is on its network or tailnet.")
                .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
            Button(tag == "ja" ? "再試行" : "Try again") { shell.reload() }
                .buttonStyle(.borderedProminent).controlSize(.large)
            ForEach(model.links.compactMap(Pairing.key).filter { $0 != shell.hub }, id: \.self) { key in
                Button(model.hubName(key)) { model.showWeb(key) }.frame(minHeight: 44)
            }
            Button(tag == "ja" ? "ハブ" : "Hubs") { model.webPairing = true }.frame(minHeight: 44)
            Button(tag == "ja" ? "クラシック画面を使う" : "Use the classic screens") { model.settings.classic = true }
                .frame(minHeight: 44)
            Spacer()
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(uiColor: .systemBackground))
    }
}

// The app's own settings, over the page (the page's Settings → This phone
// app): the hubs, Classic screens, notifications, the version
private struct ShellSettings: View {
    @Bindable var model: AppModel
    @Bindable private var settings = AppSettings.shared
    @Environment(\.dismiss) private var dismiss
    @State private var notifications: UNAuthorizationStatus?

    private var tag: String { model.screen?.locale ?? "en" }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    NavigationLink(tag == "ja" ? "ハブ" : "Hubs") {
                        if let s = model.screen {
                            HubsView(model: model, screen: s)
                        } else {
                            PairView(tag: tag, link: "", status: model.pairingError) { _ = model.pair($0) }
                        }
                    }
                    Toggle(tag == "ja" ? "クラシック画面" : "Classic screens", isOn: $settings.classic)
                } footer: {
                    Text(tag == "ja" ? "クラシック画面は以前のネイティブ画面です。このリリースの間だけ残ります。"
                                     : "The classic screens are the app's earlier native screens, kept for this release.")
                }
                Section(NativeMessages.native_settings_notifications(tag)) {
                    if notifications == .notDetermined {
                        Button(tag == "ja" ? "通知を有効にする" : "Enable notifications") {
                            Task {
                                await model.enableNotifications()
                                notifications = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
                            }
                        }
                    } else if notifications != nil {
                        Button(NativeMessages.native_settings_system_open(tag)) {
                            if let u = URL(string: UIApplication.openNotificationSettingsURLString) { UIApplication.shared.open(u) }
                        }
                    }
                }
                Section {
                    LabeledContent(tag == "ja" ? "バージョン" : "Version", value: BackplaneAppBridge.version + " (" + BackplaneAppBridge.build + ")")
                }
            }
            .navigationTitle(tag == "ja" ? "アプリ" : "This phone app")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button(NativeMessages.tour_done(tag)) { dismiss() } } }
            .task { notifications = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus }
        }
    }
}

// MARK: window.backplaneApp

// The shim both apps inject (mobile/shell/backplane-app.js, copied into the
// bundle by mobile/ios/build.sh), with this app's facts where /*@info*/null
// stands
enum BackplaneAppBridge {
    static let channel = "backplane"
    static let features = ["camera", "share", "open", "haptic", "hubs", "notify", "files", "settings"]

    static var version: String { Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "" }
    static var build: String { Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "" }

    private static let shim: String? = {
        guard let url = Bundle.main.url(forResource: "backplane-app", withExtension: "js"),
              let text = try? String(contentsOf: url, encoding: .utf8), text.contains("/*@info*/null") else {
            NSLog("backplane-app.js is missing from the bundle: the page runs without the app's bridge (mobile/ios/build.sh copies it)")
            return nil
        }
        return text
    }()

    static func script(hub: String, name: String, textScale: CGFloat) -> String? {
        guard let shim, !hub.isEmpty else { return nil }
        let info: [String: Any] = ["platform": "ios", "version": version, "build": build, "features": features,
                                   "hub": ["key": hub, "name": name], "textScale": Double(textScale)]
        guard let data = try? JSONSerialization.data(withJSONObject: info),
              let json = String(data: data, encoding: .utf8) else { return nil }
        return shim.replacingOccurrences(of: "/*@info*/null", with: json)
    }
}

#endif
