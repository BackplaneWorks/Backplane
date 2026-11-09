import XCTest

// Settings > Notifications on the simulator with real taps and typing (the hub's rows, notify.page / notify.page.act), judged by the hub's own events, and what it does to
// a push: quiet hours hold the uberbot's needs_you, off lets it go; a muted phone hears it only with Urgent on. HUB_CTL is test/e2e/push/ctl.mjs (MODE=sink: each payload the
// hub sends is handed to this simulator by `simctl push`); /uber is a real stand-in turn making the uberbot's notify_person. Shots: notify-*.png in OUT.
final class Notify: XCTestCase {
    var d: Driver!
    let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    override func setUp() { continueAfterFailure = true; d = Driver("notify") }

    func ctl(_ path: String, _ q: [String: String] = [:]) -> [String: Any] {
        var c = URLComponents(string: (d.env["HUB_CTL"] ?? "") + path)!
        c.queryItems = q.map { URLQueryItem(name: $0.key, value: $0.value) }
        var r = URLRequest(url: c.url!); r.timeoutInterval = 40
        let sem = DispatchSemaphore(value: 0); var out: [String: Any] = [:]
        URLSession.shared.dataTask(with: r) { data, _, _ in out = (data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]) ?? [:]; sem.signal() }.resume()
        _ = sem.wait(timeout: .now() + 45); return out
    }
    /// the event count now: later checks look only at what came after it
    func mark() -> Int { ctl("/ev", ["since": "999999999"])["n"] as? Int ?? 0 }
    func since(_ n: Int) -> [[String: Any]] { ctl("/ev", ["since": String(n)])["ev"] as? [[String: Any]] ?? [] }
    func setting(_ n: Int, _ key: String) -> String? { since(n).last { $0["$"] as? String == "SettingSet" && $0["key"] as? String == key }?["value"] as? String }
    func prefs(_ n: Int) -> Bool? { since(n).last { $0["$"] as? String == "DevicePrefs" }?["mute"] as? Bool }
    /// the hub's record of the uberbot's needs_you (the bot thread's own turn end also orders "done" alerts; they are not what is judged)
    func sent(_ n: Int) -> [String: Any]? { since(n).last { $0["$"] as? String == "PushSent" && $0["kind"] as? String == "needs_you" } }
    func ordered(_ n: Int) -> Bool { since(n).contains { $0["$"] as? String == "PushOrdered" } }
    func banner(_ text: String) -> XCUIElement { sb.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS[c] %@", text)).firstMatch }

    /// the page's buttons for one action, in the page's order (DPageRows gives every button of an action the id notify.page.<action>)
    func buttons(_ action: String) -> [XCUIElement] { d.app.buttons.matching(identifier: "notify.page." + action).allElementsBoundByIndex }
    func button(_ action: String, _ label: String, _ nth: Int = 0) -> XCUIElement {
        let bs = buttons(action).filter { $0.label == label }; return nth < bs.count ? bs[nth] : d.app.buttons["no such button \(action) \(label)"]
    }
    func lit(_ e: XCUIElement) -> Bool { e.exists && e.isSelected }
    /// scrolls until the element is on screen: up or down, whichever side of the window it is on
    func into(_ e: XCUIElement) {
        var n = 0
        while n < 14 && !(e.exists && e.isHittable) {
            if e.exists && e.frame.minY < d.app.windows.firstMatch.frame.midY { d.app.swipeDown(velocity: .slow) } else { d.app.swipeUp(velocity: .slow) }
            n += 1
        }
    }
    func tapIn(_ e: XCUIElement) { into(e); d.tap(e); sleep(1) }
    func settings() {
        d.app.activate(); sleep(1)
        if !d.el("settings.alerts").exists { let g = d.el("data.settings"); if g.waitForExistence(timeout: 10) { g.tap() } }
        _ = d.el("settings.alerts").waitForExistence(timeout: 10)
    }
    func clearType(_ e: XCUIElement, _ text: String) {
        into(e); e.tap(); sleep(1)
        guard e.exists else { return }
        let n = (e.value as? String)?.count ?? 8
        d.type(e, String(repeating: XCUIKeyboardKey.delete.rawValue, count: max(n, 8)) + text)
    }
    func waitBanner(_ text: String, _ secs: TimeInterval) -> Bool { banner(text).waitForExistence(timeout: secs) }
    func noBanner(_ text: String, _ secs: TimeInterval) -> Bool { let end = Date().addingTimeInterval(secs); while Date() < end { if banner(text).exists { return false }; Thread.sleep(forTimeInterval: 0.2) }; return true }
    func home() { XCUIDevice.shared.press(.home); sleep(1) }
    /// on this simulator: a banner with one of the texts within `secs`, else its entry in Notification Center (a banner shown while the app was in front, or one that
    /// hid before it was looked for); leaves the home screen up
    func delivered(_ texts: [String], _ secs: TimeInterval) -> Bool {
        let end = Date().addingTimeInterval(secs)
        var seen = false
        while Date() < end && !seen { seen = texts.contains(where: { banner($0).exists }); if !seen { Thread.sleep(forTimeInterval: 0.1) } }
        // a picture of it either way: a screenshot misses a banner's own window, Notification Center shows what was delivered
        let top = sb.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.005)); top.press(forDuration: 0.1, thenDragTo: sb.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.7)))
        sleep(2); seen = seen || texts.contains(where: { banner($0).exists })
        d.shot("center-\(texts[0].prefix(12).replacingOccurrences(of: " ", with: "-"))"); home(); return seen
    }
    /// quiet hours that cover now, in the person's zone (ctl.mjs sets America/New_York)
    func coverNow() -> (String, String) {
        var cal = Calendar(identifier: .gregorian); cal.timeZone = TimeZone(identifier: "America/New_York")!
        let f = { (d: Date) -> String in let c = cal.dateComponents([.hour, .minute], from: d); return String(format: "%02d:%02d", c.hour!, c.minute!) }
        return (f(Date().addingTimeInterval(-3600)), f(Date().addingTimeInterval(2 * 3600)))
    }

    func test_notify() {
        d.launch()
        let allow = sb.buttons["Allow"]
        if allow.waitForExistence(timeout: 20) { d.shot("permission"); allow.tap() }
        d.step("the phone registers its push token with the hub", timeout: 45, act: {}, expect: { (self.ctl("/state")["devices"] as? Int ?? 0) >= 1 })
        settings()
        let heading = d.app.staticTexts["Settled"]
        d.step("Settings shows the hub's Notifications page", timeout: 20, act: { self.into(heading) }, expect: { heading.exists && self.buttons("setting").count >= 10 })
        d.shot("page")

        // Settled: the second pair of the alert kinds' chips (Needs me, Settled, Reminders, Everything, then Urgent)
        var m = mark()
        d.step("Settled off", act: { self.tapIn(self.button("setting", "off", 1)) }, expect: { self.setting(m, "notify.settled") == "off" && self.lit(self.button("setting", "off", 1)) })
        d.shot("settled-off")
        m = mark()
        d.step("Settled on", act: { self.tapIn(self.button("setting", "on", 1)) }, expect: { self.setting(m, "notify.settled") == "on" && self.lit(self.button("setting", "on", 1)) })

        // Quiet hours: off drops the times row, on brings it back with the hub's default
        let qfrom = d.el("notify.field.qfrom"), qto = d.el("notify.field.qto")
        m = mark()
        d.step("Quiet hours off", act: { self.tapIn(self.button("notify-quiet", "off")) }, expect: { self.setting(m, "notify.quiet") == "off" && !qfrom.exists })
        d.shot("quiet-off")
        m = mark()
        d.step("Quiet hours on", act: { self.tapIn(self.button("notify-quiet", "on")) }, expect: { (self.setting(m, "notify.quiet") ?? "").contains("-") && qfrom.exists })
        let before = qfrom.exists ? (qfrom.value as? String) ?? "" : ""
        d.note("quiet hours on: \(setting(m, "notify.quiet") ?? "") from field '\(before)'")

        // a bad time: the hub's error on the row, nothing kept
        m = mark()
        let err = d.label("not the same time twice")
        d.step("25:00 + Save shows the hub's error and keeps nothing", timeout: 10, act: { self.clearType(qfrom, "25:00"); self.tapIn(self.button("notify-quiet", "Save")) },
               expect: { err.exists && self.setting(m, "notify.quiet") == nil })
        // the keyboard down and the row's note in view for the picture
        d.app.staticTexts["Urgent asks come through"].tap(); sleep(1); into(err); if err.frame.minY < 140 { d.app.swipeDown(velocity: .slow) }; d.shot("quiet-bad")
        sleep(2); d.step("…and still nothing after a wait", timeout: 1, act: {}, expect: { self.setting(m, "notify.quiet") == nil })

        let (a, b) = coverNow()
        m = mark()
        d.step("quiet hours \(a) to \(b) typed and saved", timeout: 15, act: { self.clearType(qfrom, a); self.clearType(qto, b); self.tapIn(self.button("notify-quiet", "Save")) },
               expect: { self.setting(m, "notify.quiet") == "\(a)-\(b)" && !err.exists })
        d.shot("quiet-saved")

        // quiet hours cover now: the uberbot's needs_you is ordered and held; turning them off lets it go
        home()
        let held = "Held through the quiet"
        m = mark()
        _ = ctl("/uber", ["text": held])
        d.step("in quiet hours a needs_you is ordered and held", timeout: 20, act: {}, expect: { self.ordered(m) })
        d.step("…and nothing is pushed", timeout: 1, act: {}, expect: { self.noBanner(held, 8) && self.sent(m) == nil })
        d.shot("quiet-held")
        settings()
        m = mark()
        // what was held goes as one: its banner is the held text, or "n of m need you" when other orders waited with it
        var got = false
        d.step("Quiet hours off sends what was held", timeout: 45, act: { self.tapIn(self.button("notify-quiet", "off")); self.home(); got = self.delivered([held, "need you"], 15) },
               expect: { self.setting(m, "notify.quiet") == "off" && (self.sent(m)?["sent"] as? Int ?? 0) >= 1 && got })
        d.note("released: \(String(describing: sent(m)))")
        d.shot("quiet-released")
        let goes = "Goes with quiet off"
        m = mark()
        d.step("with quiet hours off a needs_you goes at once", timeout: 45, act: { _ = self.ctl("/uber", ["text": goes]); got = self.delivered([goes], 20) },
               expect: { (self.sent(m)?["sent"] as? Int ?? 0) >= 1 && got })
        d.shot("quiet-off-goes")

        // mute: the app's Alerts switch, and the page's row for this phone
        settings()
        // the switch is a DChoices: its id is on the row's scroll view, the buttons are inside it
        let alertsOff = d.el("settings.alerts").buttons["off"], alertsOn = d.el("settings.alerts").buttons["on"]
        m = mark()
        d.step("Alerts on this phone off mutes it at the hub", act: { self.tapIn(alertsOff) }, expect: { self.prefs(m) == true })
        let muted = button("notify-mute", "muted")
        d.step("the page's row for this phone shows Muted", timeout: 15, act: { self.into(muted) }, expect: { self.lit(self.button("notify-mute", "muted")) && self.d.label("Muted: hears only").exists })
        d.shot("muted")
        m = mark()
        d.step("the page's hears unmutes it", act: { self.tapIn(self.button("notify-mute", "hears")) }, expect: { self.prefs(m) == false && self.lit(self.button("notify-mute", "hears")) })
        m = mark()
        d.step("the page's hears turns this phone's Alerts switch on", timeout: 5, act: {}, expect: { self.lit(alertsOn) })
        m = mark()
        d.step("the page's muted mutes it again", act: { self.tapIn(self.button("notify-mute", "muted")) }, expect: { self.prefs(m) == true && self.lit(self.button("notify-mute", "muted")) })
        d.step("…and turns this phone's Alerts switch off", timeout: 5, act: {}, expect: { self.lit(alertsOff) })
        let m0 = mark()

        home()
        let quietOne = "Muted and not urgent"
        m = mark()
        _ = ctl("/uber", ["text": quietOne])
        d.step("muted, Urgent off: the needs_you is not delivered", timeout: 30, act: {}, expect: { (self.sent(m)?["sent"] as? Int) == 0 && self.noBanner(quietOne, 8) })
        d.note("muted, urgent off: \(String(describing: sent(m)))")
        d.shot("muted-not-delivered")

        settings()
        d.step("coming back to the app (it registers again) keeps the phone muted", timeout: 3, act: {}, expect: { self.prefs(m0) != false })
        m = mark()
        d.step("Urgent on", act: { self.tapIn(self.button("setting", "on", 4)) }, expect: { self.setting(m, "notify.quiet.urgent") == "on" && self.lit(self.button("setting", "on", 4)) })
        d.shot("urgent-on")
        home()
        let urgent = "Approve the staging deploy"   // unlike the last one: the hub refuses a near repeat
        m = mark()
        d.step("muted, Urgent on: the needs_you comes through", timeout: 45, act: { _ = self.ctl("/uber", ["text": urgent]); got = self.delivered([urgent], 20) },
               expect: { (self.sent(m)?["sent"] as? Int ?? 0) >= 1 && (self.sent(m)?["through"] as? Bool) == true && got })
        d.note("muted, urgent on: \(String(describing: sent(m)))")
        d.shot("urgent-through")

        settings()
        m = mark()
        d.step("Urgent off", act: { self.tapIn(self.button("setting", "off", 4)) }, expect: { self.setting(m, "notify.quiet.urgent") == "off" })
        m = mark()
        d.step("Alerts on this phone on unmutes it", act: { self.tapIn(alertsOn) }, expect: { self.prefs(m) == false })
        d.step("the page's row shows it hears", timeout: 15, act: { self.into(self.button("notify-mute", "hears")) }, expect: { self.lit(self.button("notify-mute", "hears")) })
        d.shot("unmuted")
        d.finish()
        XCTAssertFalse(d.failed, "a notify step failed: see notify.json")
    }
}
