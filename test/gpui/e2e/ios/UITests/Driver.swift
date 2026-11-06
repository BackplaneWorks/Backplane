import XCTest

// The iOS journey driver's helpers: the app is driven by bundle id (dev.backplane.gpui, installed by scripts/gpui/ios-build.sh)
// with real taps and typing. Environment (run.sh passes it as TEST_RUNNER_*): HUB_HOST, HUB_PORT, HUB_TOKEN_FILE, OUT,
// optional HUB_CUT_CMD (reconnect), E2E_VIEWER_FIXTURE, LONG_SECS.
// Honest limits, written into every result: XCUIElement.tap() returns after the app is idle, so `tap_ms` includes that quiescence;
// `result_ms` is tap -> the expected element existing (polled every 20 ms). First-changed-frame latency and fps come from the video.
final class Driver {
    let app = XCUIApplication(bundleIdentifier: "dev.backplane.gpui")
    let journey: String
    let out: String
    var steps: [[String: Any]] = []
    var blocked: [String] = []
    var notes: [String] = []
    var metrics: [String: Any] = [:]
    var failed = false
    let started = CFAbsoluteTimeGetCurrent()

    init(_ journey: String) {
        self.journey = journey
        out = ProcessInfo.processInfo.environment["OUT"] ?? NSTemporaryDirectory()
        try? FileManager.default.createDirectory(atPath: out, withIntermediateDirectories: true)
        continueAfterFailure = false
    }
    var continueAfterFailure: Bool { get { false } set {} }

    var env: [String: String] { ProcessInfo.processInfo.environment }
    // a one-time pairing invitation from the hub (HUB_INVITE_FILE: drive.sh asks the hub for it through its tailnet address), the real
    // way a phone pairs; else the hub's token link
    var inviteLink: String? {
        guard let f = env["HUB_INVITE_FILE"], let t = try? String(contentsOfFile: f, encoding: .utf8) else { return nil }
        let l = t.trimmingCharacters(in: .whitespacesAndNewlines)
        return l.isEmpty ? nil : l
    }
    var link: String {
        let host = env["HUB_HOST"] ?? "127.0.0.1", port = env["HUB_PORT"] ?? "3811"
        let tok = (try? String(contentsOfFile: env["HUB_TOKEN_FILE"] ?? "", encoding: .utf8))?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return "http://\(host):\(port)/#token=\(tok)"
    }

    func el(_ id: String) -> XCUIElement { app.descendants(matching: .any)[id].firstMatch }
    func label(_ contains: String) -> XCUIElement {
        app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS[c] %@", contains)).firstMatch
    }

    // paired: BACKPLANE_LINK (the app's debug hook) so only the `pair` journey goes through the pairing screen
    func launch(paired: Bool = true) {
        app.terminate()
        app.launchEnvironment = (paired ? ["BACKPLANE_LINK": link] : [:]).merging(["BACKPLANE_PERFORMANCE": "1"]) { a, _ in a }
        app.launch()
        if paired { skipTour() }
    }
    func skipTour() {
        let skip = app.buttons["Skip"]
        if skip.waitForExistence(timeout: 10) { skip.tap(); sleep(1) }
    }

    func ms(_ since: CFAbsoluteTime) -> Double { ((CFAbsoluteTimeGetCurrent() - since) * 1000 * 10).rounded() / 10 }

    // one step: act, then wait for the expectation; both times in the result
    @discardableResult
    func step(_ name: String, timeout: TimeInterval = 15, act: () -> Void, expect: () -> Bool) -> Bool {
        let t0 = CFAbsoluteTimeGetCurrent()
        let epoch = (Date().timeIntervalSince1970 * 1000).rounded()
        act()
        let tap = ms(t0)
        var ok = expect()
        while !ok && CFAbsoluteTimeGetCurrent() - t0 < timeout { Thread.sleep(forTimeInterval: 0.02); ok = expect() }
        let result = ms(t0)
        steps.append(["do": name, "tap_epoch_ms": epoch, "tap_ms": tap, "result_ms": ok ? result : -1, "ok": ok])
        if !ok {
            failed = true
            // what the screen held when the expectation never came: a picture and the accessibility tree
            shot("fail-\(steps.count)")
            try? app.debugDescription.write(toFile: "\(out)/\(journey)-fail-\(steps.count).txt", atomically: true, encoding: .utf8)
        }
        return ok
    }
    // a tap that cannot crash the test: a missing element leaves the step to fail with its screenshot and tree
    func tap(_ e: XCUIElement) { if e.waitForExistence(timeout: 4) { e.tap() } }

    func type(_ e: XCUIElement, _ text: String) {
        guard e.waitForExistence(timeout: 4) else { return }
        // typeText needs keyboard focus: tap once more when the keyboard is not up (a back/forward may have dropped it)
        if !app.keyboards.firstMatch.exists { e.tap(); _ = app.keyboards.firstMatch.waitForExistence(timeout: 2) }
        if app.keyboards.firstMatch.exists { e.typeText(text) } else { note("no keyboard for typing \(text.prefix(12))") }
    }

    func note(_ s: String) { notes.append(s) }
    func block(_ why: String) { blocked.append(why) }

    func shot(_ name: String) {
        let s = XCUIScreen.main.screenshot()
        try? s.pngRepresentation.write(to: URL(fileURLWithPath: "\(out)/\(journey)-\(name).png"))
    }

    // the fixture hub's control endpoint (serve-fixture.mjs): POST /ctl/cut?secs=N, /ctl/restart; HUB_CTL=http://host:3851
    func ctl(_ path: String) -> Bool {
        guard let base = env["HUB_CTL"], let url = URL(string: base + path) else { return false }
        var r = URLRequest(url: url); r.httpMethod = "POST"; r.timeoutInterval = 15
        let sem = DispatchSemaphore(value: 0); var ok = false
        URLSession.shared.dataTask(with: r) { _, resp, _ in ok = (resp as? HTTPURLResponse)?.statusCode == 200; sem.signal() }.resume()
        _ = sem.wait(timeout: .now() + 20); return ok
    }

    // (older) the runner script (run.sh) executes HUB_CUT_CMD / HUB_RESTORE_CMD when these request files appear
    func request(_ what: String, timeout: TimeInterval = 60) -> Bool {
        let req = "\(out)/\(what).req", ack = "\(out)/\(what).ack"
        try? FileManager.default.removeItem(atPath: ack)
        FileManager.default.createFile(atPath: req, contents: Data())
        let end = Date().addingTimeInterval(timeout)
        while Date() < end { if FileManager.default.fileExists(atPath: ack) { return true }; Thread.sleep(forTimeInterval: 0.2) }
        return false
    }

    func finish() {
        let status = failed ? "fail" : (!blocked.isEmpty ? "blocked" : "pass")
        let r: [String: Any] = ["journey": journey, "client": "ios", "status": status, "blocked": blocked,
                                "metrics": metrics.merging(["steps": steps, "wall_s": ms(started) / 1000, "video_start_epoch_ms": Double(env["VIDEO_START_EPOCH_MS"] ?? "") ?? NSNull()]) { a, _ in a },
                                "notes": notes + ["tap_ms includes XCUITest's wait for the app to be idle; ack/fps are measured from the video"],
                                "capture": "\(out)/\(journey).mp4"]
        if let d = try? JSONSerialization.data(withJSONObject: r, options: [.prettyPrinted, .sortedKeys]) {
            try? d.write(to: URL(fileURLWithPath: "\(out)/\(journey).json"))
        }
        XCTAssertTrue(status != "fail", "journey \(journey) failed: \(steps.filter { ($0["ok"] as? Bool) == false })")
    }
}
