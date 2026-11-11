import XCTest

// Phone push on the simulator with real taps: the permission alert, a banner from the lock of the home screen, the deep link, and what must NOT arrive. HUB_CTL is test/e2e/push/ctl.mjs;
// every /uber, /other and /worker there is a real stand-in turn making the real MCP call. A step that must see nothing waits QUIET seconds on the home screen, then checks the hub's own count too.
final class Push: XCTestCase {
    var d: Driver!
    let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    let quiet: TimeInterval = 8
    override func setUp() { continueAfterFailure = true; d = Driver("push") }

    func ctl(_ path: String, _ q: [String: String] = [:]) -> [String: Any] {
        var c = URLComponents(string: (d.env["HUB_CTL"] ?? "") + path)!
        c.queryItems = q.map { URLQueryItem(name: $0.key, value: $0.value) }
        var r = URLRequest(url: c.url!); r.timeoutInterval = 40
        let sem = DispatchSemaphore(value: 0); var out: [String: Any] = [:]
        URLSession.shared.dataTask(with: r) { data, _, _ in out = (data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]) ?? [:]; sem.signal() }.resume()
        _ = sem.wait(timeout: .now() + 45); return out
    }
    func state() -> [String: Any] { ctl("/state") }
    func num(_ k: String) -> Int { state()[k] as? Int ?? -1 }
    func banner(_ text: String) -> XCUIElement { sb.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS[c] %@", text)).firstMatch }
    func home() { XCUIDevice.shared.press(.home); sleep(1) }
    /// taps the alert: its banner while it is up, else its entry in Notification Center (a banner hides after a few seconds)
    func open(_ text: String) {
        let b = banner(text); if b.exists { b.tap(); return }
        let top = sb.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.005)); top.press(forDuration: 0.1, thenDragTo: sb.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.7)))
        let n = banner(text); if n.waitForExistence(timeout: 5) { n.tap() }
    }
    /// `shot: false` when the next step taps the banner: a screenshot takes long enough for it to hide
    func arrives(_ name: String, _ text: String, shot: Bool = true, act: () -> Void) -> XCUIElement {
        let b = banner(text)
        d.step(name, timeout: 45, act: act, expect: { b.exists })
        if shot { d.shot(name.replacingOccurrences(of: " ", with: "-")) }
        return b
    }
    func silent(_ name: String, _ text: String, act: () -> Void) {
        let before = num("sent")
        act()
        let b = banner(text); var seen = false; let end = Date().addingTimeInterval(quiet)
        while Date() < end { if b.exists { seen = true; break }; Thread.sleep(forTimeInterval: 0.2) }
        let after = num("sent")
        d.step(name, timeout: 1, act: {}, expect: { !seen && after == before })
        d.note("\(name): banner \(seen), hub PushSent \(before) -> \(after)")
        d.shot(name.replacingOccurrences(of: " ", with: "-"))
    }

    func test_push() {
        d.launch()
        let allow = sb.buttons["Allow"]
        d.step("the permission alert is asked once, and allowed", timeout: 30, act: {}, expect: { allow.exists })
        d.shot("permission"); if allow.exists { allow.tap() }
        d.step("the phone registers its token with the hub", timeout: 45, act: {}, expect: { self.num("devices") >= 1 })
        d.note("device \(state()["envs"] ?? "")"); home()

        let first = "Approve the deploy to staging?"
        let b = arrives("the uberbot's notify_person arrives", first, shot: false) { _ = ctl("/uber", ["text": first]) }
        _ = b
        d.step("the tap opens its thread", timeout: 20, act: { self.open(first) }, expect: { self.d.app.state == .runningForeground && self.d.el("thread.send").exists })
        d.shot("deep-link-thread"); home()
        // its source is the uberbot: the hub's own record of the push names the uberbot's thread and the uberbot as its title
        let st = state(); let sent = st["lastSent"] as? [String: Any] ?? [:]
        d.step("the alert came from the uberbot", timeout: 1, act: {}, expect: { (sent["thread"] as? String) == (st["bubThread"] as? String) && (sent["title"] as? String) == "Bub" })
        d.note("source: \(sent["title"] ?? "") on \(sent["thread"] ?? ""), uberbot thread \(st["bubThread"] ?? "")")

        silent("a duplicate is suppressed", first) { let r = ctl("/uber", ["text": first]); d.note("repeat: \(String(describing: r["said"]).prefix(160))") }

        _ = ctl("/setting", ["k": "notify.quiet", "v": "00:00-23:59"]); sleep(1)
        let held = "Held through quiet hours"
        silent("quiet hours hold it", held) { _ = ctl("/uber", ["text": held]) }
        let hb = arrives("it arrives when quiet hours end", held) { _ = ctl("/setting", ["k": "notify.quiet", "v": "off"]) }
        _ = hb

        silent("another bot cannot push", "mochi wants a push") { let r = ctl("/other", ["text": "mochi wants a push"]); d.note("other bot: \(String(describing: r["said"]).prefix(160))") }
        silent("a worker thread cannot push", "worker wants a push") { let r = ctl("/worker", ["text": "worker wants a push"]); d.note("worker: \(String(describing: r["said"]).prefix(160))") }
        silent("a finished turn does not push", "Finished") { _ = ctl("/workerdone") }

        let ask = ctl("/ask"); let id = ask["ask"] as? String ?? ""
        d.step("a worker's approval is open", timeout: 1, act: {}, expect: { !id.isEmpty })
        silent("the approval alone does not push", "rm -rf") {}
        let text = "The worker needs your approval"
        let ab = arrives("the uberbot's approval alert arrives", text, shot: false) { _ = ctl("/uber", ["text": text, "ask": id]) }
        _ = ab
        d.step("the tap opens the exact approval", timeout: 20, act: { self.open(text) }, expect: { self.d.app.state == .runningForeground && self.d.el("data.entry.ask").exists })
        d.shot("deep-link-approval"); home()
        let cleared = num("cleared")
        _ = ctl("/answer", ["ask": id])
        d.step("answering it clears it, with the badge", timeout: 20, act: {}, expect: { self.num("cleared") > cleared })
        d.note("cleared \(String(describing: state()["lastCleared"]))"); d.shot("cleared")

        _ = ctl("/setting", ["k": "push.burst.s", "v": "15"]); sleep(1)
        let wt = state()["worker"] as? String ?? ""
        _ = arrives("the first of a burst arrives", "Burst one") { _ = ctl("/uber", ["text": "Burst one", "thread": wt]) }
        sleep(1); _ = ctl("/uber", ["text": "Burst two", "thread": wt]); sleep(1); _ = ctl("/uber", ["text": "Burst three", "thread": wt])
        _ = arrives("the rest collapse into one", "2 of 2 need you") {}
        d.note("after burst \(String(describing: state()["lastSent"]))")

        // the silent clear (DataPush's didReceiveRemoteNotification): the person opens the worker's thread elsewhere, the hub clears its items, and only the
        // app's handler can set the icon's badge to what the hub says
        let icon = sb.icons["Backplane"]; let before = icon.value as? String ?? ""; let c0 = num("cleared")
        d.note("badge before the clear: '\(before)'")
        _ = ctl("/seen", ["thread": wt])
        d.step("a silent clear runs the app's handler and changes the badge", timeout: 30, act: {}, expect: { self.num("cleared") > c0 && !before.isEmpty && (icon.value as? String ?? "") != before })
        d.note("badge after the clear: '\(icon.value as? String ?? "")'"); d.shot("silent-clear")
        d.finish()
        XCTAssertFalse(d.failed, "a push step failed: see push.json")
    }
}
