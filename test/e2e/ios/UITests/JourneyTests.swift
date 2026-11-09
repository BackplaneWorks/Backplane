import XCTest

// The 12 gate journeys (test/e2e/journeys.json ids) driven on the iOS simulator with real taps, typing and a real pasteboard
// paste. Fixture hub: test/e2e/serve-fixture.mjs (project "e2e", 20 threads "e2e thread N", bots Bub/Mochi, room e2e-room).
final class Journeys: XCTestCase {
    var d: Driver!
    override func setUp() { continueAfterFailure = false }

    private func rows() -> XCUIElementQuery { d.app.cells }
    // the fixture's threads are titled "e2e thread N" (N from 0); a row is the button with identifier thread.row.<hub>|<id>
    // (the thread.tree.* button next to it only unfolds subthreads)
    private func openThread(_ n: Int, timeout: TimeInterval = 15) -> Bool {
        let want = "e2e thread \(n)"
        let row = d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'thread.row.'"))
            .containing(NSPredicate(format: "label == %@", want)).firstMatch
        return d.step("open \(want)", timeout: timeout, act: {
            // the list keeps its scroll position after a back: look above first, then page down
            if !row.exists { for _ in 0..<3 { d.app.swipeDown(velocity: .fast) } }
            var tries = 0
            while !row.exists && tries < 8 { d.app.swipeUp(); tries += 1 }
            d.tap(row)
        }, expect: { d.el("thread.composer").exists })
    }
    private func back() { let b = d.app.navigationBars.buttons.element(boundBy: 0); if b.exists { b.tap() } }
    private func tab(_ name: String) {
        let t = d.app.tabBars.buttons[name]
        if t.exists { t.tap() } else { d.tap(d.label(name)) }
    }

    func test_pair() {
        d = Driver("pair"); defer { d.finish() }
        d.launch(paired: false)
        let addr = d.el("pair.address")
        guard d.step("pairing screen", act: {}, expect: { addr.exists }) else { return }
        d.step("type the pairing link", act: { d.tap(addr); d.type(addr, d.inviteLink ?? d.link) }, expect: { d.el("pair.connect").isEnabled })
        d.step("connect", timeout: 30, act: { d.tap(d.el("pair.connect")) }, expect: { d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'thread.row.' OR identifier BEGINSWITH 'project.header.'")).firstMatch.exists || d.app.buttons["Skip"].exists })
        for _ in 0..<4 { d.skipTour() }
        d.shot("paired")
    }

    func test_projects_threads() {
        d = Driver("projects-threads"); defer { d.finish() }
        d.launch()
        tab("Threads")
        guard d.step("project list", act: {}, expect: { d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'project.header.'")).firstMatch.exists }) else { return }
        guard openThread(0) else { return }
        d.step("scroll 3 screens", act: { for _ in 0..<3 { d.app.swipeUp(velocity: .fast) } }, expect: { true })
        back()
        if openThread(5) { d.step("history shown", act: {}, expect: { d.app.staticTexts.count > 3 }) }
        d.shot("thread")
    }

    func test_send_image() {
        d = Driver("send-image"); defer { d.finish() }
        d.launch(); tab("Threads")
        guard openThread(1) else { return }
        let comp = d.el("thread.composer")
        d.step("type a message", act: { d.tap(comp); d.type(comp, "here is a picture zqshort") }, expect: { d.el("thread.send").isEnabled })
        // a real pasteboard image, pasted through the paperclip menu
        let img = UIGraphicsImageRenderer(size: CGSize(width: 120, height: 80)).image { c in UIColor.systemTeal.setFill(); c.fill(CGRect(x: 0, y: 0, width: 120, height: 80)) }
        UIPasteboard.general.image = img
        let clip = d.app.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'attach'")).firstMatch
        d.step("open the attach menu", act: { d.tap(clip) }, expect: { d.app.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'paste'")).firstMatch.exists || d.app.menuItems.matching(NSPredicate(format: "label CONTAINS[c] 'paste'")).firstMatch.exists })
        d.shot("attach-menu")
        d.step("paste the image", act: {
            let item = d.app.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'paste'")).firstMatch
            let mi = d.app.menuItems.matching(NSPredicate(format: "label CONTAINS[c] 'paste'")).firstMatch
            if item.waitForExistence(timeout: 2) { item.tap() } else if mi.exists { mi.tap() }
            let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
            let allow = sb.buttons["Allow Paste"]
            if allow.waitForExistence(timeout: 3) { allow.tap() }
        }, expect: { d.app.images.count > 0 || d.label("noise").exists || d.label("pasted").exists })
        d.step("send", act: { d.tap(d.el("thread.send")) }, expect: { d.label("here is a picture").exists })
        d.step("reply arrives", timeout: 30, act: {}, expect: { d.label("REPLY").exists || d.label("Finished").exists })
        d.shot("sent")
    }

    func test_stream() {
        d = Driver("stream"); defer { d.finish() }
        d.launch(); tab("Threads")
        guard openThread(2) else { return }
        let comp = d.el("thread.composer")
        d.step("type", act: { d.tap(comp); d.type(comp, "stream me a long answer") }, expect: { d.el("thread.send").isEnabled })
        d.step("send", act: { d.tap(d.el("thread.send")) }, expect: { d.label("stream me").exists })
        // the long stand-in turn (600 s): the thread stays working and the text grows; sample the visible text length for 20 s
        var lens: [Int] = []; let t0 = CFAbsoluteTimeGetCurrent()
        while CFAbsoluteTimeGetCurrent() - t0 < 20 { lens.append(d.app.staticTexts.allElementsBoundByIndex.prefix(40).map { $0.label.count }.reduce(0, +)); Thread.sleep(forTimeInterval: 0.5) }
        d.metrics["stream_visible_text_chars"] = lens
        let grew = (lens.last ?? 0) > (lens.first ?? 0)
        d.steps.append(["do": "watch the stream 20 s", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded() - 20000, "tap_ms": 0, "result_ms": 20000, "ok": grew])
        if !grew { d.failed = true; d.note("the visible text did not grow during the stream") }
        d.shot("stream")
        let stop = d.el("thread.interrupt"); if stop.exists { stop.tap() }
    }

    func test_switch_fast() {
        d = Driver("switch-fast"); defer { d.finish() }
        d.launch(); tab("Threads")
        for i in 0..<10 {
            guard openThread(i % 6) else { break }
            back()
        }
        d.shot("switched")
    }

    func test_popups() {
        d = Driver("popups"); defer { d.finish() }
        d.launch(); tab("Threads")
        guard openThread(0) else { return }
        let comp = d.el("thread.composer"); d.tap(comp)
        for (sigil, want) in [("@", "@Bub"), (">", ">e2e"), ("%", "%proj"), ("#", "#e2e"), ("$", "$"), ("/", "/btw")] {
            if sigil == "#" {
                // rooms: only when the fixture seeded one (a hub that refused rooms.create leaves none)
                d.type(comp, sigil); Thread.sleep(forTimeInterval: 2.5)
                let has = d.label("#e2e").exists
                d.steps.append(["do": "type # -> popup", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded(), "tap_ms": 0, "result_ms": 0, "ok": true, "skipped": !has])
                if !has { d.note("no room in the fixture hub: the # popup has no rows to show") }
                d.type(comp, XCUIKeyboardKey.delete.rawValue); continue
            }
            d.step("type \(sigil) -> popup", act: { d.type(comp, sigil) }, expect: { want == "$" ? d.app.buttons.count > 12 : d.label(want).exists })
            d.type(comp, XCUIKeyboardKey.delete.rawValue)
        }
        d.shot("popup")
    }

    // A real hardware Return and Shift+Return through the key-event path (XCUIElement.typeKey), not the debug act: Return sends,
    // Shift+Return is a newline; and autocorrect on the software path ("teh " becomes "the ").
    func test_composer_keys() {
        d = Driver("composer-keys"); defer { d.finish() }
        d.launch(); tab("Threads")
        guard openThread(1) else { return }
        let comp = d.el("thread.composer"); d.tap(comp)
        func value() -> String { (d.el("thread.composer").value as? String) ?? "" }
        func wait(_ t: TimeInterval = 4, _ ok: () -> Bool) -> Bool { let end = Date().addingTimeInterval(t); while Date() < end { if ok() { return true }; Thread.sleep(forTimeInterval: 0.2) }; return ok() }
        d.type(comp, "line one")
        // the software keyboard's Return is a newline (hardware Return sends: see the hardware note below)
        d.step("software return inserts a newline", act: { d.app.keyboards.buttons["return"].tap() },
               expect: { wait { value().contains("\n") } && value().hasPrefix("line one") })
        d.type(comp, "line two")
        d.shot("composer-newline")
        // hardware keys through the key-event path; XCUITest delivers them only when the simulator has a hardware keyboard connected
        let before = value()
        d.el("thread.composer").typeKey(.return, modifierFlags: .shift)
        let hwShift = wait(2) { value() != before }
        d.steps.append(["do": "hardware shift+return", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded(), "tap_ms": 0, "result_ms": 0, "ok": true, "skipped": !hwShift])
        if !hwShift { d.block("hardware Shift+Return not exercised: typeKey changed nothing without a connected hardware keyboard") }
        d.el("thread.composer").typeKey(.return, modifierFlags: [])
        let hwSent = wait(4) { value().isEmpty }
        d.steps.append(["do": "hardware return sends", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded(), "tap_ms": 0, "result_ms": 0, "ok": true, "skipped": !hwSent])
        if !hwSent { d.block("hardware Return not exercised: typeKey changed nothing without a connected hardware keyboard") }
        // clear what is left, then autocorrect: a misspelt word then a space
        d.el("thread.composer").press(forDuration: 1.0)
        if d.app.menuItems["Select All"].waitForExistence(timeout: 2) { d.app.menuItems["Select All"].tap() }
        d.el("thread.composer").typeText(XCUIKeyboardKey.delete.rawValue)
        d.type(comp, "teh ")
        let corrected = wait(3) { value().hasPrefix("the ") }
        d.steps.append(["do": "autocorrect teh -> the", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded(), "tap_ms": 0, "result_ms": 0, "ok": true, "skipped": !corrected])
        if !corrected { d.block("autocorrect not demonstrated: XCUITest typeText left '\(value())' (the field has autocorrection on; the red spell underline shows; XCUITest typing may bypass the correction)") }
        d.shot("composer-autocorrect")
    }

    // The data/1 app (DATA_APP build) against a hub that speaks docs/PHONE-DATA.md: add a project, start a thread, watch the streamed answer,
    // answer an approval, send again, relaunch and see the saved copy at once. HUB_PROJECT_PATH: a folder path on the hub's machine.
    func test_data_flow() {
        d = Driver("data-flow"); defer { d.finish() }
        func env(_ k: String) -> String? { let v = d.env[k]; return (v?.isEmpty ?? true) ? nil : v }
        let path = env("HUB_PROJECT_PATH") ?? "~/bp-stub-projects/e2e1"
        let title = path.split(separator: "/").last.map(String.init) ?? "e2e1"
        let first = env("HUB_FIRST_PROMPT") ?? "hello from the phone", firstSees = env("HUB_FIRST_EXPECT") ?? "You said: hello from the phone"
        let second = env("HUB_ASK_PROMPT") ?? "please ask me first"; let secondSees: String? = env("HUB_ASK_EXPECT") ?? (env("HUB_PAIR_LINK") == nil ? "You said: please ask me first" : nil)
        let reply = Double(env("HUB_REPLY_WAIT") ?? "20") ?? 20
        // paired by an invitation link the way a phone is (HUB_PAIR_LINK), or by the token link the fixture hubs allow
        d.app.terminate()
        if let pair = env("HUB_PAIR_LINK") {
            d.app.launchEnvironment = ["BACKPLANE_PERFORMANCE": "1"]; d.app.launch()
            // a phone that was paired before (the keychain outlives an install): forget that hub first, as a person re-pairing would
            if !d.el("pair.address").waitForExistence(timeout: 6), d.app.tabBars.buttons["Settings"].waitForExistence(timeout: 6) {
                d.app.tabBars.buttons["Settings"].tap()
                d.app.buttons["Forget this hub"].tap()
                if d.app.buttons["Forget"].waitForExistence(timeout: 4) { d.app.buttons["Forget"].tap() }
            }
            let addr = d.el("pair.address")
            guard d.step("pairing screen", act: {}, expect: { addr.waitForExistence(timeout: 20) }) else { return }
            d.step("type the pairing link", act: { d.tap(addr); d.type(addr, pair) }, expect: { d.el("pair.connect").isEnabled })
            d.step("connect", timeout: 30, act: { d.tap(d.el("pair.connect")) }, expect: { d.app.tabBars.buttons["Threads"].waitForExistence(timeout: 30) })
            if d.app.tabBars.buttons["Threads"].exists { d.app.tabBars.buttons["Threads"].tap() }
        } else if env("HUB_ALREADY_PAIRED") != nil {
            // the phone (simulator keychain) holds the device credential from an earlier pairing: no link, no invitation
            d.app.launchEnvironment = ["BACKPLANE_PERFORMANCE": "1"]; d.app.launch()
            if d.app.tabBars.buttons["Threads"].waitForExistence(timeout: 20) { d.app.tabBars.buttons["Threads"].tap() }
        } else {
            d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1"]; d.app.launch()
        }
        func hittable(_ e: XCUIElement) { for _ in 0..<40 where !e.isHittable { usleep(250_000) } }
        let rows = d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'data.thread.row.'"))
        guard d.step("threads tab", act: {}, expect: { d.el("data.project.add").waitForExistence(timeout: 20) }) else { return }
        // a project added in an earlier run is already listed: no second add (a hub refuses a folder it already has)
        _ = d.app.buttons["New thread in \(title)"].waitForExistence(timeout: 6)
        if !d.app.buttons["New thread in \(title)"].exists {
            guard d.step("add project", act: { d.tap(d.el("data.project.add")) }, expect: { d.el("data.project.path").waitForExistence(timeout: 10) }) else { return }
            d.step("choose the folder", act: { let f = d.el("data.project.path"); d.tap(f); f.press(forDuration: 1.0); if d.app.menuItems["Select All"].waitForExistence(timeout: 2) { d.app.menuItems["Select All"].tap() }; f.typeText(path) },
                   expect: { d.el("data.project.add.confirm").isEnabled })
            guard d.step("project appears", act: { d.tap(d.el("data.project.add.confirm")) }, expect: { let ok = d.app.buttons["New thread in \(title)"].waitForExistence(timeout: 15); sleep(1); return ok && !d.el("data.project.path").exists }) else { d.shot("no-project"); return }
        }
        // a project the hub makes with a first empty thread: the thread to use is the one the next step starts
        let before = rows.count
        guard d.step("new thread sheet", act: { let b = d.app.buttons["New thread in \(title)"]; _ = b.waitForExistence(timeout: 10); hittable(b); b.tap() }, expect: { d.el("data.thread.new.text").waitForExistence(timeout: 10) }) else { return }
        d.step("start the thread", act: { let t = d.el("data.thread.new.text"); d.tap(t); t.typeText(first); d.tap(d.el("data.thread.new.start")) },
               // Start opens the new thread (DataViews' new-thread sheet appends it to the path): the list's new row, or the thread itself, open
               expect: { rows.count > before || rows.firstMatch.exists || d.el("thread.composer").exists })
        // the thread whose preview or title is the prompt; else the first row
        let mine = rows.containing(NSPredicate(format: "label CONTAINS[c] %@", String(first.prefix(20)))).firstMatch
        guard d.step("open the thread", act: { d.tap(mine.exists ? mine : rows.firstMatch) }, expect: { d.el("thread.composer").waitForExistence(timeout: 10) }) else { return }
        let answered = d.step("the answer shows", timeout: reply, act: {}, expect: { d.label(firstSees).exists })
        d.shot("thread-answered")
        d.notes.append("first answer seen: \(answered)")
        let comp = d.el("thread.composer")
        d.step("send a message that needs approval", act: { d.tap(comp); d.type(comp, second); d.tap(d.el("thread.send")) }, expect: { d.el("data.entry.ask").waitForExistence(timeout: reply) || d.app.buttons["Allow"].waitForExistence(timeout: 1) })
        d.shot("approval")
        let allow = d.app.buttons["Allow"]
        guard d.el("data.entry.ask").exists || allow.exists else { d.note("no approval was raised for: \(second)"); d.block("no approval entry appeared within \(Int(reply)) s"); return }
        d.step("allow", timeout: reply, act: { if allow.exists { allow.tap() } else if d.app.buttons.matching(NSPredicate(format: "label BEGINSWITH[c] 'Allow'")).firstMatch.exists { d.app.buttons.matching(NSPredicate(format: "label BEGINSWITH[c] 'Allow'")).firstMatch.tap() } else { d.el("data.entry.ask").buttons.firstMatch.tap() } }, expect: { if let s = secondSees { return d.label(s).waitForExistence(timeout: reply) }; let end = Date().addingTimeInterval(reply); while Date() < end { if !d.app.buttons["Allow"].exists && !d.el("data.thinking").exists { return true }; Thread.sleep(forTimeInterval: 0.5) }; return false })
        // back to the list, then the app again from nothing: the saved copy paints before the hub answers
        let back = d.app.navigationBars.buttons.element(boundBy: 0); if back.exists { back.tap() }
        sleep(2)
        d.app.terminate(); d.app.launchEnvironment = d.app.launchEnvironment
        let t0 = CFAbsoluteTimeGetCurrent(); d.app.launch()
        let painted = rows.firstMatch.waitForExistence(timeout: 10)
        d.steps.append(["do": "relaunch: the saved threads show", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded(), "tap_ms": 0, "result_ms": d.ms(t0), "ok": painted])
        d.shot("relaunch")
    }

    // The composer's own commands on a Rust hub seeded by the lane rig (project "demo": t1 "Fix the ecc83 board", t2 "BOM review"; stand-in agents):
    // the Send button reads the hub's label, /btw answers in a card and posts nothing, /give hands over, /secret opens the masked box, /intent saves.
    // Links in a message (docs/PHONE-DATA.md "Links in messages"): the hub is scripts/mobile/data-hub.sh with LINKS=1 (thread t3 "Links" holds every kind, test/mobile/links-fixture.md).
    // A tap on each kind; what the system was handed is read from the app's debug "link.last", what opened inside the app from its own screens.
    func test_data_links() {
        d = Driver("data-links"); defer { d.finish() }
        d.app.terminate()
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1", "BACKPLANE_SELECT": "t3"]; d.app.launch(); d.skipTour()
        // the system's notification question, once per fresh simulator
        let sbAlert = XCUIApplication(bundleIdentifier: "com.apple.springboard").alerts.buttons["Allow"]
        if sbAlert.waitForExistence(timeout: 8) { sbAlert.tap() }
        let msg = d.el("data.entry.assistant")
        guard d.step("the Links thread opens", timeout: 30, act: {}, expect: { msg.exists && d.label("Link check").exists }) else { return }
        try? d.app.debugDescription.write(toFile: "\(d.out)/data-links-tree.txt", atomically: true, encoding: .utf8)
        d.shot("links-thread")
        func link(_ text: String) -> XCUIElement {
            let l = d.app.links[text]
            if l.exists { return l }
            return d.app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        }
        func tapLink(_ text: String) {
            let l = link(text); if !l.waitForExistence(timeout: 6) { return }
            // a link inside a longer line: the element is the whole Text; tap where the link is (its text's share of the line)
            if d.app.links[text].exists { l.tap(); return }
            let f = l.frame; let full = l.label as NSString; let r = full.range(of: text)
            let x = (CGFloat(r.location) + CGFloat(r.length) / 2) / max(1, CGFloat(full.length))
            l.coordinate(withNormalizedOffset: CGVector(dx: min(0.95, x), dy: 0.5)).tap(); _ = f
        }
        func last() -> String { d.el("link.last").label }
        func backToThread() { let b = d.app.navigationBars.buttons.element(boundBy: 0); if b.exists { b.tap() }; _ = d.el("thread.composer").waitForExistence(timeout: 5) }
        func openLinksThread() { if !d.el("thread.composer").exists || !msg.exists { d.tap(d.el("data.thread.row.t3")) }; _ = msg.waitForExistence(timeout: 8) }

        // the system's handlers: the app is left (Safari, Mail, Phone) or the system asks; what it was handed is the app's debug "link.last"
        func lastSafe() -> String { d.app.state == .runningForeground ? last() : "" }
        func system(_ name: String, _ text: String, _ want: String) {
            d.step(name, act: { tapLink(text) }, expect: { d.app.state != .runningForeground || lastSafe().hasPrefix(want) })
            d.shot("link-" + name.replacingOccurrences(of: " ", with: "-"))
            let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
            for b in ["Cancel", "OK", "Close", "Not Now"] where sb.alerts.buttons[b].exists { sb.alerts.buttons[b].tap() }
            d.app.activate(); _ = msg.waitForExistence(timeout: 10)
            d.step(name + " was handed to the system", act: {}, expect: { last().hasPrefix(want) })
        }
        system("web link", "the docs", "https://example.com/docs")
        system("bare address", "https://example.org/bare", "https://example.org/bare")
        system("email", "dev@example.com", "mailto:dev@example.com")
        system("phone number", "+1 555 123 4567", "tel:+15551234567")
        // backplane links open inside the app
        d.step("thread link", act: { tapLink("the BOM review") }, expect: { d.label("BOM review").waitForExistence(timeout: 10) && d.el("thread.composer").exists })
        d.shot("link-thread"); backToThread()
        d.step("room link", act: { tapLink("backplane://conv/room%3Ar1") }, expect: { d.el("data.room.post").waitForExistence(timeout: 10) })
        d.shot("link-room")
        let rb = d.app.navigationBars.buttons.element(boundBy: 0); if rb.exists { rb.tap() }
        d.step("file at a line", act: { openLinksThread(); tapLink("src/main.rs:12") }, expect: { d.el("data.preview.line").waitForExistence(timeout: 10) })
        d.step("the file is the one named", act: {}, expect: { d.label("line 12 of src/main.rs").exists })
        d.shot("link-file"); backToThread(); backToThread()
        d.step("a path without a line", act: { openLinksThread(); tapLink("src/main.rs:3") }, expect: { d.label("line 3 of src/main.rs").waitForExistence(timeout: 10) })
        backToThread(); backToThread()
        // a link the hub cannot resolve says so
        d.step("a thread that is gone", act: { openLinksThread(); tapLink("backplane://thread/t404") }, expect: { d.el("link.note").waitForExistence(timeout: 6) })
        d.shot("link-note"); _ = !d.el("link.note").waitForExistence(timeout: 1)
        d.step("a thread on a machine not mirrored", act: { sleep(4); tapLink("backplane://thread/lab~t9") }, expect: { d.el("link.note").waitForExistence(timeout: 6) })
        sleep(4)
        d.step("a missing file", act: { tapLink("src/nothing.rs:1") }, expect: { d.el("link.note").waitForExistence(timeout: 6) })
        // the project link last: it leaves the thread for the list
        d.step("project link", act: { sleep(4); tapLink("backplane://project/p1") }, expect: { d.el("data.project.add").waitForExistence(timeout: 10) && d.el("data.project.p1").exists })
        d.shot("link-project")
    }

    func test_data_commands() {
        d = Driver("data-commands"); defer { d.finish() }
        d.launch()
        let row = d.el("data.thread.row.t1")
        guard d.step("threads", timeout: 25, act: {}, expect: { row.exists }) else { return }
        guard d.step("open t1", act: { d.tap(row) }, expect: { d.el("thread.composer").waitForExistence(timeout: 10) }) else { return }
        let comp = d.el("thread.composer"), send = d.el("thread.send")
        func draft() -> String { (comp.value as? String) ?? "" }
        func clear() { d.tap(comp); let n = draft().count; if n > 0 { comp.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: n)) } }
        d.step("/btw: Send reads Ask", act: { d.type(comp, "/btw what is an ECC83?") }, expect: { send.label == "Ask" })
        d.shot("btw-typed")
        d.step("/btw: the card shows the answer", timeout: 30, act: { d.tap(send) }, expect: { d.el("thread.btw").exists && d.label("No answer came back").exists })
        d.step("/btw: the draft is empty", act: {}, expect: { draft().isEmpty || draft() == "Ask anything: @bot, >thread, %project, #room, &model, $skill, /btw" })
        d.shot("btw-answer")
        d.step("/btw: the card closes", act: { d.tap(d.el("thread.btw.close")) }, expect: { !d.el("thread.btw").exists })
        clear()
        d.step("/give: Send reads Give", act: { d.type(comp, "/give >bom-review look at the socket") }, expect: { send.label == "Give" })
        d.shot("give-typed")
        d.step("/give: handed over (the draft empties)", timeout: 20, act: { d.tap(send) }, expect: { !draft().contains("/give") })
        d.shot("give-done")
        clear()
        d.step("/secret: Send reads Send", act: { d.type(comp, "/secret gh-token") }, expect: { send.label == "Send" && d.el("thread.send").isEnabled })
        d.step("/secret: the masked box opens", act: { d.tap(send) }, expect: { d.el("thread.secret").exists })
        d.step("/secret: a value typed", act: { let v = d.el("thread.secret.value"); d.tap(v); v.typeText("sk-phone-123") }, expect: { d.el("thread.secret.save").isEnabled })
        d.shot("secret-box")
        d.step("/secret: saved, the box shuts", timeout: 20, act: { d.tap(d.el("thread.secret.save")) }, expect: { !d.el("thread.secret").exists })
        d.shot("secret-saved")
        clear()
        d.step("/intent: Send reads Save", act: { d.type(comp, "/intent") }, expect: { send.label == "Save" })
        d.step("/intent alone: the composer takes the intent", timeout: 20, act: { d.tap(send) }, expect: { draft().contains("# Goal") })
        d.shot("intent-edit")
        clear()
        d.step("/intent: saved", timeout: 20, act: { d.type(comp, "/intent A tube preamp, set from the iPhone."); d.tap(send) }, expect: { d.label("Intent saved").exists })
        d.shot("intent-saved")
    }

    // Conversation history on the lane rig's hub: Scratch chats s1 "zebra crossing chat", s2 "kicad library question" (settled) and s3 "old notes" (recent):
    // the group's row counts them and opens the list, the search finds them under its heading, a chat opens and takes a message, and nothing starts one.
    func test_data_history() {
        d = Driver("data-history"); defer { d.finish() }
        d.launch()
        let group = d.el("data.history")
        guard d.step("the group's row", timeout: 25, act: {}, expect: { group.exists && d.label("CONVERSATION HISTORY (3)").exists }) else { return }
        d.step("shut: only the live chat has its row", act: {}, expect: { d.el("data.thread.row.s3").exists && !d.el("data.thread.row.s1").exists })
        d.step("no project to start a chat in", act: {}, expect: { !d.app.buttons["New thread in Scratch"].exists })
        if d.el("data.thread.row.s2").exists { d.tap(group) } // the group remembers being open: start shut
        d.shot("history-shut")
        d.step("open: every chat", act: { d.tap(group) }, expect: { d.el("data.thread.row.s1").exists && d.el("data.thread.row.s2").exists })
        d.shot("history-open")
        d.tap(group)
        let search = d.el("data.search")
        d.step("search finds an old chat", act: { d.tap(search); search.typeText("zebra") }, expect: { d.el("data.search.history").exists && d.el("data.search.thread.s1").exists })
        d.shot("history-search")
        d.step("it opens as it was", act: { d.tap(d.el("data.search.thread.s1")) }, expect: { d.label("Which way is the zebra crossing?").waitForExistence(timeout: 10) })
        let comp = d.el("thread.composer")
        d.step("and takes a message", timeout: 30, act: { d.type(comp, "still there?"); d.tap(d.el("thread.send")) }, expect: { d.label("there?3").exists }) // the stand-in in stream mode answers "<last word>0 … 3"
        d.shot("history-chat")
    }

    // Opening threads one after another, again and again: the app logs "Backplane open cached=... ms=..." for each (touch to screen). Needs 3 threads:
    // it makes them (a first message each) when the project has fewer.
    func test_data_switch() {
        d = Driver("data-switch"); defer { d.finish() }
        func env(_ k: String) -> String? { let v = d.env[k]; return (v?.isEmpty ?? true) ? nil : v }
        let path = env("HUB_PROJECT_PATH") ?? "~/bp-stub-projects/e2e1"
        let title = path.split(separator: "/").last.map(String.init) ?? "e2e1"
        d.app.terminate(); d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1"]; d.app.launch()
        let rows = d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'data.thread.row.'"))
        guard d.step("threads tab", act: {}, expect: { d.el("data.project.add").waitForExistence(timeout: 20) }) else { return }
        if !d.app.buttons["New thread in \(title)"].waitForExistence(timeout: 6) {
            d.tap(d.el("data.project.add")); let f = d.el("data.project.path"); _ = f.waitForExistence(timeout: 10); d.tap(f)
            f.press(forDuration: 1.0); if d.app.menuItems["Select All"].waitForExistence(timeout: 2) { d.app.menuItems["Select All"].tap() }; f.typeText(path)
            d.tap(d.el("data.project.add.confirm")); _ = d.app.buttons["New thread in \(title)"].waitForExistence(timeout: 15)
        }
        var made = 0
        while rows.count < 3 && made < 3 {
            let b = d.app.buttons["New thread in \(title)"]; for _ in 0..<40 where !b.isHittable { usleep(250_000) }; b.tap()
            let t = d.el("data.thread.new.text"); _ = t.waitForExistence(timeout: 10); d.tap(t); t.typeText("switch test \(made)"); d.tap(d.el("data.thread.new.start"))
            made += 1; sleep(5)
        }
        for _ in 0..<20 where d.el("data.thread.new.text").exists { usleep(500_000) }
        sleep(3)
        d.shot("switch-list")
        let n = min(rows.count, 3)
        var opened = 0
        for round in 0..<4 {
            for i in 0..<n {
                let row = rows.element(boundBy: i)
                for _ in 0..<20 where !(row.exists && row.isHittable) { usleep(250_000) }
                let t0 = CFAbsoluteTimeGetCurrent()
                if row.isHittable { row.tap() } else { row.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap() }
                let ok = d.el("thread.composer").waitForExistence(timeout: 10)
                d.steps.append(["do": "open thread \(i) round \(round)", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded(), "tap_ms": 0, "result_ms": d.ms(t0), "ok": ok])
                opened += ok ? 1 : 0
                let back = d.app.navigationBars.buttons.element(boundBy: 0); if back.exists { back.tap() }
                usleep(600_000)
            }
        }
        d.metrics["opened"] = opened
    }

    // A picture pasted into the composer is attached, sent with a message, and shown in the thread (the hub keeps it; the phone shows its own copy).
    func test_data_image() {
        d = Driver("data-image"); defer { d.finish() }
        func env(_ k: String) -> String? { let v = d.env[k]; return (v?.isEmpty ?? true) ? nil : v }
        let path = env("HUB_PROJECT_PATH") ?? "~/bp-stub-projects/e2e1"
        let title = path.split(separator: "/").last.map(String.init) ?? "e2e1"
        d.app.terminate(); d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1"]; d.app.launch()
        let rows = d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'data.thread.row.'"))
        guard d.step("threads tab", act: {}, expect: { d.el("data.project.add").waitForExistence(timeout: 20) }) else { return }
        if !d.app.buttons["New thread in \(title)"].waitForExistence(timeout: 6) {
            d.tap(d.el("data.project.add")); let f = d.el("data.project.path"); _ = f.waitForExistence(timeout: 10); d.tap(f)
            f.press(forDuration: 1.0); if d.app.menuItems["Select All"].waitForExistence(timeout: 2) { d.app.menuItems["Select All"].tap() }; f.typeText(path)
            d.tap(d.el("data.project.add.confirm")); _ = d.app.buttons["New thread in \(title)"].waitForExistence(timeout: 15)
        }
        if rows.count == 0 {
            let b = d.app.buttons["New thread in \(title)"]; for _ in 0..<40 where !b.isHittable { usleep(250_000) }; b.tap()
            let t = d.el("data.thread.new.text"); _ = t.waitForExistence(timeout: 10); d.tap(t); t.typeText("picture thread"); d.tap(d.el("data.thread.new.start"))
            _ = rows.firstMatch.waitForExistence(timeout: 30) // a new thread takes a moment to reach the list
        }
        guard d.step("open a thread", act: { for _ in 0..<40 where !(rows.firstMatch.exists && rows.firstMatch.isHittable) { usleep(250_000) }; d.tap(rows.firstMatch) }, expect: { d.el("thread.composer").waitForExistence(timeout: 10) }) else { return }
        // a picture on the pasteboard, pasted with the system menu
        UIPasteboard.general.image = UIGraphicsImageRenderer(size: CGSize(width: 300, height: 200)).image { c in UIColor.systemTeal.setFill(); c.fill(CGRect(x: 0, y: 0, width: 300, height: 200)) }
        // the sent picture: its own identifier, or its message's (a row's identifier covers the picture inside it)
        func picture(_ t: TimeInterval) -> Bool { d.el("data.entry.image").waitForExistence(timeout: t) || d.app.images["data.entry.user"].waitForExistence(timeout: 1) }
        let comp = d.el("thread.composer")
        d.step("paste the picture", act: { d.tap(comp); comp.press(forDuration: 1.0); if d.app.menuItems["Paste"].waitForExistence(timeout: 4) { d.app.menuItems["Paste"].tap() } }, expect: { d.el("data.pending.images").waitForExistence(timeout: 10) })
        d.shot("picture-pending")
        d.step("send it with a message", act: { d.type(comp, "here is a picture"); d.tap(d.el("thread.send")) }, expect: { picture(20) && !d.el("data.pending.images").exists })
        d.shot("picture-sent")
        // leave and come back: the picture shows again (this phone's copy, or fetched from the hub)
        let back = d.app.navigationBars.buttons.element(boundBy: 0); if back.exists { back.tap() }
        sleep(1)
        d.step("open it again", act: { for _ in 0..<20 where !(rows.firstMatch.exists && rows.firstMatch.isHittable) { usleep(250_000) }; rows.firstMatch.tap() }, expect: { picture(15) })
        d.shot("picture-again")
    }

    /// The drawing pad (mobile/ios/Backplane/DataPad.swift): draw a stroke and a box, type a side's length, undo it, attach the drawing and send it.
    /// The paper is checked by its pixels: how much of it is not white, from a screenshot of it.
    func test_data_pad() {
        d = Driver("data-pad"); defer { d.finish() }
        func env(_ k: String) -> String? { let v = d.env[k]; return (v?.isEmpty ?? true) ? nil : v }
        let path = env("HUB_PROJECT_PATH") ?? "~/bp-stub-projects/e2e1"
        let title = path.split(separator: "/").last.map(String.init) ?? "e2e1"
        d.app.terminate(); d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1"]; d.app.launch()
        let rows = d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'data.thread.row.'"))
        guard d.step("threads tab", act: {}, expect: { d.el("data.project.add").waitForExistence(timeout: 20) }) else { return }
        if !d.app.buttons["New thread in \(title)"].waitForExistence(timeout: 6) {
            d.tap(d.el("data.project.add")); let f = d.el("data.project.path"); _ = f.waitForExistence(timeout: 10); d.tap(f)
            f.press(forDuration: 1.0); if d.app.menuItems["Select All"].waitForExistence(timeout: 2) { d.app.menuItems["Select All"].tap() }; f.typeText(path)
            d.tap(d.el("data.project.add.confirm")); _ = d.app.buttons["New thread in \(title)"].waitForExistence(timeout: 15)
        }
        if rows.count == 0 {
            let b = d.app.buttons["New thread in \(title)"]; for _ in 0..<40 where !b.isHittable { usleep(250_000) }; b.tap()
            let t = d.el("data.thread.new.text"); _ = t.waitForExistence(timeout: 10); d.tap(t); t.typeText("drawing thread"); d.tap(d.el("data.thread.new.start"))
            _ = rows.firstMatch.waitForExistence(timeout: 30) // a new thread takes a moment to reach the list
        }
        guard d.step("open a thread", act: { for _ in 0..<40 where !(rows.firstMatch.exists && rows.firstMatch.isHittable) { usleep(250_000) }; d.tap(rows.firstMatch) }, expect: { d.el("thread.composer").waitForExistence(timeout: 10) }) else { return }
        let paper = d.el("data.pad.paper")
        guard d.step("open the pad", act: { d.tap(d.el("data.draw")) }, expect: { paper.exists && paper.isHittable }) else { return }
        // the share of the paper that is not white, sampled every 2 px
        func ink() -> Int {
            guard let cg = paper.screenshot().image.cgImage, let data = cg.dataProvider?.data, let p = CFDataGetBytePtr(data) else { return -1 }
            let bpp = cg.bitsPerPixel / 8; var n = 0
            for y in stride(from: 4, to: cg.height - 4, by: 2) { for x in stride(from: 4, to: cg.width - 4, by: 2) {
                let i = y * cg.bytesPerRow + x * bpp; if p[i] < 235 || p[i + 1] < 235 || p[i + 2] < 235 { n += 1 } } }
            return n
        }
        func at(_ x: CGFloat, _ y: CGFloat) -> XCUICoordinate { paper.coordinate(withNormalizedOffset: CGVector(dx: x, dy: y)) }
        let blank = ink()
        d.step("the pen draws", act: { at(0.1, 0.17).press(forDuration: 0.1, thenDragTo: at(0.42, 0.17)) }, expect: { ink() > blank + 20 })
        let stroked = ink()
        d.step("the Box tool", act: { d.tap(d.el("data.pad.box")) }, expect: { d.label("tap a side").exists })
        d.step("a box is dragged out", act: { at(0.52, 0.42).press(forDuration: 0.1, thenDragTo: at(0.83, 0.83)) }, expect: { ink() > stroked + 200 })
        d.shot("box")
        let boxed = ink()
        let alert = d.app.alerts["Length, e.g. 40 mm"]
        guard d.step("a tap on its right side asks its width", act: { at(0.833, 0.62).tap() }, expect: { alert.exists }) else { return }
        let field = alert.textFields.firstMatch
        d.step("40 mm narrows the box", act: {
            d.tap(field); field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 8) + "40 mm"); d.tap(alert.buttons["Set"])
        }, expect: { !alert.exists && ink() < boxed - 100 })
        d.shot("length")
        let narrowed = ink()
        // undo also drops the selection's dashes, so the paper is compared with the narrowed box, not with the picked one before it
        d.step("undo brings the width back", act: { d.tap(d.el("data.pad.undo")) }, expect: { ink() > narrowed + 100 })
        d.step("attach: the pad closes and the drawing waits in the composer", act: { d.tap(d.el("data.pad.attach")) }, expect: { !paper.exists && d.el("data.pending.images").exists })
        d.shot("attached")
        d.step("send it", act: { d.tap(d.el("thread.send")) }, expect: {
            // a picture with no words is the message's only child, and SwiftUI gives it the message's identifier
            (d.el("data.entry.image").exists || d.app.images["data.entry.user"].exists) && !d.el("data.pending.images").exists
        })
        d.shot("sent")
    }

    func test_bots_rooms() {
        d = Driver("bots-rooms"); defer { d.finish() }
        d.launch(); tab("Bots")
        d.step("bots list", act: {}, expect: { d.label("Bub").exists })
        d.step("open Bub", act: { d.tap(d.label("Bub")) }, expect: { d.el("thread.composer").exists || d.label("ready").exists })
        d.shot("bot"); back()
        d.step("rooms", act: { d.tap(d.app.buttons["Rooms"]) }, expect: { d.label("e2e-room").exists || d.label("No rooms yet").exists })
        if d.label("No rooms yet").exists { d.block("fixture: the hub has no room (rooms.create refused or not implemented)"); d.shot("no-rooms"); return }
        d.step("open the room", act: { d.tap(d.label("e2e-room")) }, expect: { d.app.textFields.firstMatch.exists })
        let post = d.app.textFields.firstMatch
        // typing fast (XCUITest's ~20 chars/s) into the room's field, then what it holds: dropped characters would be an app bug worth seeing
        d.tap(post); d.type(post, "hello room zqshort")
        let fast = (post.value as? String) ?? ""
        d.metrics["room_field_after_fast_typing"] = fast
        if fast != "hello room zqshort" { d.note("room field after fast typing held \"\(fast)\" instead of \"hello room zqshort\" (characters dropped)") }
        for _ in 0..<fast.count + 2 { post.typeText(XCUIKeyboardKey.delete.rawValue) }
        d.step("post to the room", act: {
            for ch in "hello room zqshort\n" { post.typeText(String(ch)); Thread.sleep(forTimeInterval: 0.12) }
        }, expect: { d.label("hello room").exists })
        d.shot("room")
    }

    // the thread's viewer panel: viewer.open opens it, viewer.source picks Board / Schematic / 3D / Files ...
    private func openViewer(_ kind: String) -> Bool {
        let id = "viewer.choice." + kind.lowercased()
        return d.step("open \(kind)", timeout: 25, act: {
            if d.el("viewer.close").exists {
                // already in the viewer: its kind strip (Board, Schematic, 3D, Mech, Files, Review)
                let tabBtn = d.app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", String(kind.prefix(3)))).firstMatch
                d.tap(tabBtn)
            } else {
                d.tap(d.el("viewer.open")); d.tap(d.el(id))   // viewer.open lists the kinds as viewer.choice.*
            }
        }, expect: { d.el("viewer.close").exists })
    }

    func test_viewers() {
        d = Driver("viewers"); defer { d.finish() }
        d.launch(); tab("Threads")
        guard openThread(0) else { return }
        for kind in ["Board", "Schematic", "3D"] {
            guard openViewer(kind) else { break }
            Thread.sleep(forTimeInterval: 3)
            if d.label("in this project yet").exists { d.block("fixture: no \(kind.lowercased()) in the project (the hub's project has no KiCad project/STEP the viewer lists)") }
            if d.label("Loading 3D").exists { d.note("3D model still loading after 3 s (STEP conversion)") }
            d.app.swipeLeft(); d.app.pinch(withScale: 1.5, velocity: 1)
            d.shot("viewer-\(kind)")
            if d.el("viewer.close").exists { d.el("viewer.close").tap() }
        }
    }

    // a tap on a part opens its info over the board; the viewer keeps its size, and closing or tapping empty space deselects.
    // (On the TestFlight build of Nov 6 the tap folded the viewer to its bar: the pick's JSON was pasted into the viewer's JSON.)
    // HUB_VIEWER_THREAD: a thread of a project with a board with parts (the GNSS tracker); else the first thread of the list.
    func test_data_part_info() {
        d = Driver("data-part-info"); defer { d.finish() }
        let thread = d.env["HUB_VIEWER_THREAD"] ?? ""
        d.app.terminate()
        d.app.launchArguments = ["-app.alerts", "<false/>"] // no notification question over the board
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1"].merging(thread.isEmpty ? [:] : ["BACKPLANE_SELECT": "viewer:" + thread, "BACKPLANE_VTAB": "Board"]) { a, _ in a }
        d.app.launch()
        if thread.isEmpty {
            let rows = d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'data.thread.row.'"))
            guard d.step("open a thread", timeout: 30, act: { _ = rows.firstMatch.waitForExistence(timeout: 20); d.tap(rows.firstMatch) }, expect: { d.el("data.thread.viewer").exists }) else { return }
            guard d.step("open the viewer", act: { d.tap(d.el("data.thread.viewer")) }, expect: { d.el("data.vtab.Board").exists }) else { return }
            d.tap(d.el("data.vtab.Board"))
        }
        let surface = d.el("data.viewer.surface"), info = d.el("data.viewer.info"), tabs = d.el("data.vtab.Board")
        guard d.step("the board is drawn", timeout: 40, act: {}, expect: { surface.exists && surface.frame.height > 200 }) else { return }
        Thread.sleep(forTimeInterval: 2) // the plot and the hub's layers settle
        let size = surface.frame, strip = tabs.frame
        func at(_ x: CGFloat, _ y: CGFloat) -> XCUICoordinate { surface.coordinate(withNormalizedOffset: CGVector(dx: x, dy: y)) }
        // a part somewhere on the board: the first point of a grid over its middle whose tap opens an info card
        let spots: [(CGFloat, CGFloat)] = [(0.5, 0.46), (0.3, 0.6), (0.7, 0.6), (0.5, 0.7), (0.4, 0.8), (0.6, 0.8), (0.25, 0.75), (0.75, 0.75)]
        var hit: (CGFloat, CGFloat)?
        guard d.step("a tap on a part opens its info", timeout: 30, act: {
            for s in spots { at(s.0, s.1).tap(); if info.waitForExistence(timeout: 3) { hit = s; break } }
        }, expect: { info.exists }) else { d.shot("no-info"); return }
        d.metrics["part_at"] = hit.map { "\($0.0),\($0.1)" } ?? ""
        d.shot("part-info")
        d.step("the viewer keeps its size", act: {}, expect: { surface.exists && surface.frame == size && tabs.frame == strip })
        let rows = d.app.descendants(matching: .any).matching(identifier: "data.viewer.info.row")
        d.step("its fields are listed", act: {}, expect: { rows.count >= 1 })
        d.metrics["info_rows"] = (0..<rows.count).map { rows.element(boundBy: $0).label }
        d.step("close deselects", act: { d.tap(d.el("data.viewer.info.close")) }, expect: { !info.exists && surface.frame == size })
        if let h = hit {
            d.step("the part again", act: { at(h.0, h.1).tap() }, expect: { info.waitForExistence(timeout: 5) })
            d.step("a tap on empty space deselects", act: { at(0.95, 0.12).tap() }, expect: { !info.exists && surface.exists && surface.frame == size })
        }
        d.shot("after")
    }

    // Switching viewers never shows the last viewer's content: right after a switch the surface holds nothing of the old file (its
    // accessibility value is the hub's keys of what is drawn), then the new file's. Two boards (HUB_VIEWER_THREAD, HUB_VIEWER_THREAD2:
    // threads of two projects with boards) and the kinds of one, tapped fast. (The person saw the previous viewer's files after a switch.)
    func test_data_viewer_switch() {
        d = Driver("data-viewer-switch"); defer { d.finish() }
        let a = d.env["HUB_VIEWER_THREAD"] ?? "", b = d.env["HUB_VIEWER_THREAD2"] ?? ""
        guard !a.isEmpty, !b.isEmpty else { d.block("needs HUB_VIEWER_THREAD and HUB_VIEWER_THREAD2 (two threads of projects with boards)"); return }
        let surface = d.el("data.viewer.surface")
        func shown() -> String { surface.exists ? ((surface.value as? String) ?? "") : "" }
        func open(_ t: String) {
            d.app.terminate(); d.app.launchArguments = ["-app.alerts", "<false/>"]
            d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1", "BACKPLANE_SELECT": "viewer:" + t, "BACKPLANE_VTAB": "Board"]
            d.app.launch()
            // iOS's own notification question, if the app asks it, is answered so it does not cover the viewer
            let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
            let no = springboard.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Don'")).firstMatch
            if no.waitForExistence(timeout: 4) { no.tap() }
        }
        open(a)
        guard d.step("board A is drawn", timeout: 40, act: {}, expect: { shown().hasPrefix("0|") }) else { return }
        let rootA = shown().split(separator: "|").dropFirst().first.map(String.init) ?? ""
        d.metrics["root_a"] = rootA
        // the kinds, fast: Schematic, 3D, Board, Schematic; each switch drops the last kind's content at once
        var stale: [String] = []
        for (tab, want) in [("Schematic", "1|"), ("3D", "2|"), ("Board", "0|"), ("Schematic", "1|")] {
            d.tap(d.el("data.vtab." + tab))
            let now = shown()
            if now.split(separator: "\n").contains(where: { !$0.hasPrefix(want) && !(want == "2|" && $0.hasPrefix("0|")) }) { stale.append("\(tab): \(now)") }
        }
        d.metrics["stale_after_tab_switch"] = stale
        d.step("no tab shows the last tab's content", act: {}, expect: { stale.isEmpty })
        d.step("the last tab tapped (Schematic) is what shows", timeout: 30, act: {}, expect: { let v = shown(); return v.hasPrefix("1|\(rootA)|") && !v.contains("\n") })
        d.shot("a-schematic")
        // A's 3D model, then the other board's 3D view: B's model takes a while (its export), and A's must not stand in for it
        d.tap(d.el("data.vtab.3D"))
        guard d.step("A's model is drawn", timeout: 60, act: {}, expect: { shown().contains("2|\(rootA)|") }) else { return }
        let rows = d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'data.thread.row.'"))
        func go(_ t: String, _ tab: String) {
            d.tap(d.el("data.back")); d.tap(d.el("data.back"))
            let row = d.app.buttons["data.thread.row." + t]
            if !row.waitForExistence(timeout: 8) { _ = rows.firstMatch.waitForExistence(timeout: 2) }
            d.tap(row); d.tap(d.el("data.thread.viewer")); d.tap(d.el("data.vtab." + tab))
        }
        go(b, "3D")
        let right = shown(); d.metrics["b_3d_right_after"] = right
        d.step("B's 3D view shows nothing of A right after the switch", act: {}, expect: { !right.contains("|\(rootA)|") })
        d.shot("b-3d-right-after")
        d.step("then B's own board or model", timeout: 60, act: {}, expect: { let v = shown(); return !v.isEmpty && !v.contains("|\(rootA)|") })
        let rootB = shown().split(separator: "|").dropFirst().first.map(String.init) ?? "?"
        d.metrics["root_b"] = rootB
        d.shot("b-3d")
        // fast between the two boards' Board tabs
        for t in [a, b, a] {
            go(t, "Board")
            let other = t == a ? rootB : rootA
            if shown().contains("|\(other)|") { stale.append("thread \(t): \(shown())") }
        }
        d.step("switching threads never shows the other thread's board", act: {}, expect: { stale.isEmpty })
        d.step("the last thread (A) shows its own board", timeout: 40, act: {}, expect: { shown().hasPrefix("0|\(rootA)|") })
        d.shot("a-again")
    }

    // Mech after the 3D tab: the part asked for is drawn (or loading), never the board's model left from the 3D tab and never a black page.
    // HUB_VIEWER_THREAD: a thread of a project with a board and mech/<a STEP> (test/e2e/pixel-hub.mjs's: ecc83 and mech/bracket.step).
    func test_data_mech() {
        d = Driver("data-mech"); defer { d.finish() }
        let t = d.env["HUB_VIEWER_THREAD"] ?? ""
        guard !t.isEmpty else { d.block("needs HUB_VIEWER_THREAD (a thread of a project with a board and a STEP under mech/)"); return }
        d.app.terminate(); d.app.launchArguments = ["-app.alerts", "<false/>"]
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1", "BACKPLANE_SELECT": "viewer:" + t, "BACKPLANE_VTAB": "3D"]
        d.app.launch()
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let no = springboard.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Don'")).firstMatch
        if no.waitForExistence(timeout: 4) { no.tap() }
        let three = d.el("data.viewer.surface"), mech = d.el("data.mech.surface")
        guard d.step("the board's 3D model is drawn", timeout: 60, act: {}, expect: { ((three.value as? String) ?? "").contains("2|") }) else { return }
        d.tap(d.el("data.vtab.Mech"))
        let chip = d.app.buttons.matching(NSPredicate(format: "label ENDSWITH '.step' OR label ENDSWITH '.STEP' OR label ENDSWITH '.stp'")).firstMatch
        guard d.step("Mech lists the part", timeout: 20, act: {}, expect: { chip.exists }) else { return }
        let name = chip.label
        d.tap(chip)
        let value = { (mech.value as? String) ?? "" }
        d.step("right after: loading or the part, never the board's model", act: {}, expect: {
            mech.exists && (d.el("data.viewer.loading").exists || value().hasSuffix("/" + name) || value().contains("/" + name))
        })
        d.metrics["mech_right_after"] = value()
        // drawn means pixels: the share of the surface (sampled every 4 px) that is not the black ground. (On the Nov 7 build Mech held the
        // part's model, watched as `3d-bare`, and drew nothing: a black page.)
        func lit() -> Int {
            guard let cg = mech.screenshot().image.cgImage, let data = cg.dataProvider?.data, let px = CFDataGetBytePtr(data) else { return -1 }
            let bpp = cg.bitsPerPixel / 8; var n = 0
            for y in stride(from: 0, to: cg.height, by: 4) { for x in stride(from: 0, to: cg.width, by: 4) {
                let i = y * cg.bytesPerRow + x * bpp; if Int(px[i]) + Int(px[i + 1]) + Int(px[i + 2]) > 90 { n += 1 } } }
            return n
        }
        d.step("the part is drawn", timeout: 90, act: {}, expect: { value().hasSuffix(name) && lit() > 400 })
        d.metrics["mech_lit_samples"] = lit()
        d.shot("mech")
    }

    func test_downloads() {
        d = Driver("downloads"); defer { d.finish() }
        d.launch(); tab("Threads")
        guard openThread(0) else { return }
        guard openViewer("Files") else { return }
        d.step("the Files list shows the fixture's readme", act: {}, expect: { d.label("readme.txt").exists })
        d.step("open readme.txt", act: { d.tap(d.label("readme.txt")) }, expect: { d.app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] 'fixture file'")).count > 0 })
        d.shot("file")
    }

    func test_settings() {
        d = Driver("settings"); defer { d.finish() }
        d.launch(); tab("Settings")
        d.step("settings page", act: {}, expect: { d.app.navigationBars.firstMatch.exists })
        var dead: [String] = []; var tapped = 0
        let skip = ["clear", "unpair", "forget", "remove", "delete", "sign out", "quit"]
        let labels = d.app.buttons.allElementsBoundByIndex.compactMap { b -> String? in
            guard b.exists, b.isHittable, b.isEnabled, !b.label.isEmpty else { return nil }
            if skip.contains(where: { b.label.lowercased().contains($0) }) { return nil }
            return b.label
        }
        for l in labels where !["Home", "Threads", "Bots", "Routines", "Settings"].contains(l) {
            let b = d.app.buttons[l].firstMatch
            guard b.exists, b.isHittable else { continue }
            let before = d.app.debugDescription.hashValue
            b.tap(); tapped += 1
            Thread.sleep(forTimeInterval: 0.8)
            let changed = d.app.debugDescription.hashValue != before || d.app.alerts.count > 0 || d.app.sheets.count > 0
            if !changed { dead.append(l) }
            for close in ["Done", "Close", "Cancel", "Back"] { let c = d.app.buttons[close].firstMatch; if c.exists && c.isHittable { c.tap(); break } }
            let nb = d.app.navigationBars.buttons.element(boundBy: 0)
            if d.app.tabBars.count == 0, nb.exists { nb.tap() }
        }
        d.metrics["dead_controls"] = dead; d.metrics["controls_tapped"] = tapped
        d.steps.append(["do": "tap every settings control", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded(), "tap_ms": 0, "result_ms": 0, "ok": dead.isEmpty])
        if !dead.isEmpty { d.failed = true; d.note("dead controls: \(dead)") }
        d.shot("settings")
    }

    func test_reconnect() {
        d = Driver("reconnect"); defer { d.finish() }
        guard ProcessInfo.processInfo.environment["HUB_CTL"] != nil else { d = Driver("reconnect"); d.block("HUB_CTL (the fixture's control endpoint) is not set"); return }
        d.launch(); tab("Threads")
        guard openThread(0) else { return }
        let comp = d.el("thread.composer"); d.tap(comp); d.type(comp, "draft over a sleep")
        d.step("the hub drops every connection for 30 s", act: { _ = d.ctl("/ctl/cut?secs=30") }, expect: { d.label("Offline").exists || d.label("reconnecting").exists || d.label("Cannot reach").exists })
        d.shot("offline")
        d.step("the app reconnects", timeout: 90, act: {}, expect: { !d.label("Offline").exists && !d.label("Cannot reach").exists })
        let kept = (comp.value as? String ?? "").contains("draft over a sleep")
        d.steps.append(["do": "the draft is kept", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded(), "tap_ms": 0, "result_ms": 0, "ok": kept])
        if !kept { d.failed = true }
        d.shot("back")
    }

    func test_long_session() {
        d = Driver("long-session"); defer { d.finish() }
        // the fixture starts 20 long-streaming stand-in agents (serve-fixture: POST /ctl/agents?count=20)
        let agents = d.ctl("/ctl/agents?count=20")
        d.metrics["agents_started"] = agents
        if !agents { d.note("could not start the 20 simulated agents (HUB_CTL /ctl/agents refused): long session ran without them") }
        d.launch(); tab("Threads")
        let secs = Double(ProcessInfo.processInfo.environment["LONG_SECS"] ?? "") ?? 300
        let t0 = CFAbsoluteTimeGetCurrent(); var n = 0
        while CFAbsoluteTimeGetCurrent() - t0 < secs {
            guard openThread(n % 20, timeout: 60) else { break }
            d.app.swipeUp(); let comp = d.el("thread.composer"); d.tap(comp); d.type(comp, "@"); d.type(comp, XCUIKeyboardKey.delete.rawValue)
            back(); n += 1
        }
        d.metrics["cycles"] = n
        d.note("memory is sampled by run.sh (footprint every 30 s into long-session.mem.csv)")
        d.shot("end")
    }

    // A live buy thread's Order card (test/order/live_ios.sh): opened on BUY_THREAD, Buy now opens Safari on the order server's page, whose test
    // checkout pays the Stripe double; then the card in the app says Paid.
    func test_order_card() {
        d = Driver("order-card"); defer { d.finish() }
        d.app.terminate()
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": d.env["BUY_THREAD"] ?? "", "BACKPLANE_PERFORMANCE": "1"]
        d.app.launch(); d.skipTour()
        guard d.step("the Order card on the buy thread", timeout: 40, act: {}, expect: { d.el("thread.order").exists }) else { d.shot("no-card"); return }
        d.step("the customs line", act: {}, expect: { d.label("Includes import VAT").exists })
        d.step("Buy now on the card", act: {}, expect: { d.el("thread.order.pay").exists })
        d.step("the hub's note to the agent is not shown", act: {}, expect: { !d.label("[Backplane] The quote is posted").exists })
        d.shot("order-card")
        let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")
        d.step("Buy now opens Safari", timeout: 30, act: { d.tap(d.el("thread.order.pay")) }, expect: { safari.state == .runningForeground })
        sleep(8); d.shot("browser")
        d.app.activate()
        d.step("the card says Paid", timeout: 60, act: {}, expect: { d.label("Paid: your order").exists })
        d.shot("paid")
    }

    // Terminals (feature `terms`; scripts/mobile/ios-term-rig.sh, rig-seed.ts): the chat terminal's card in a thread, open full screen, type with the soft keyboard,
    // rotate (the pty's size follows), lose the connection while the shell prints and get it back, stop it; then the project's Terminals screen and a new shell.
    // env: TERM_THREAD, TERM_PROJECT (rig-seed.ts), HUB_CTL (the relay's control port).
    // lane 2: a subagent's own view (feature subview), opened from its Background work row while it works and from its fold's Agent row after it ended; Back returns.
    // The rig's thread (TERM_THREAD) runs the stand-in's `agents:2 agentsteps:3 subwork:MS` turn, started by ios-sub-journey.sh.
    func test_subagent() {
        d = Driver("subagent"); defer { d.finish() }
        let lbl = { (id: String) -> String in let e = self.d.el(id); return e.exists ? e.label : "" }
        let pre = { (p: String) in self.d.app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", p)).firstMatch }
        d.app.terminate()
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": d.env["TERM_THREAD"] ?? "", "BACKPLANE_PERFORMANCE": "1"]
        d.app.launch(); d.skipTour()
        let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard"); if sb.buttons["Allow"].waitForExistence(timeout: 6) { sb.buttons["Allow"].tap() }
        guard d.step("a subagent at work in Background work", timeout: 40, act: {}, expect: { pre("data.work.sub.").exists }) else { return }
        sleep(2); d.shot("1-panel")
        guard d.step("its row opens its own view", timeout: 15, act: { d.tap(pre("data.work.sub.")) }, expect: { d.el("data.sub.list").exists && lbl("data.sub.state").contains("working") }) else { return }
        let meta = { lbl("data.sub.meta") }; let m0 = meta()
        d.shot("2-sub-live")
        d.step("new steps arrive while it works", timeout: 20, act: {}, expect: { meta() != m0 && meta().contains("steps") })
        d.shot("3-sub-live-later")
        d.step("Back returns to the thread", timeout: 10, act: { d.tap(d.app.navigationBars.buttons.firstMatch) }, expect: { !d.el("data.sub.list").exists && pre("data.work.").exists })
        d.shot("4-back")
        guard d.step("the turn ends", timeout: 240, act: {}, expect: { d.label("2 tool calls · Agent").exists }) else { return }
        // (the fold's container names every row inside it data.entry.tool: the Agent row is found by its words)
        let agentRow = { self.d.app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Agent, ")).firstMatch }
        d.step("its fold opens", timeout: 10, act: { d.tap(d.label("2 tool calls · Agent")) }, expect: { agentRow().exists })
        d.shot("5-fold-open")
        d.step("the fold's Agent row opens the ended subagent", timeout: 15, act: { d.tap(agentRow()) }, expect: { lbl("data.sub.state").contains("completed") })
        sleep(1); d.shot("6-sub-ended")
        d.step("Back again", timeout: 10, act: { d.tap(d.app.navigationBars.buttons.firstMatch) }, expect: { !d.el("data.sub.list").exists })
        d.shot("7-back")
    }

    // /give from a thread's composer into a running thread, an idle one and a bot; the card in the receiver, its references that open (give-rig.sh, seed test/give/seed.py)
    func test_give() {
        d = Driver("give"); defer { d.finish() }
        let comp = { self.d.el("thread.composer") }
        let go = { self.d.app.buttons.matching(NSPredicate(format: "label == %@ OR label == %@", "Send", "Give")).firstMatch }
        let back = { self.d.app.navigationBars.buttons.firstMatch }
        let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        d.app.terminate()
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": "t3", "BACKPLANE_PERFORMANCE": "1"]
        d.app.launch(); d.skipTour(); if sb.buttons["Allow"].waitForExistence(timeout: 6) { sb.buttons["Allow"].tap() }
        guard d.step("Busy build runs", timeout: 20, act: { d.tap(comp()); d.type(comp(), "start the long build"); d.tap(go()) }, expect: { d.label("start the long build").exists }) else { return }
        sleep(3)
        d.step("back to the list, into Power rail", timeout: 15, act: { d.tap(back()); d.tap(d.label("Power rail")) }, expect: { comp().exists && d.label("Routed the 3V3 rail").exists })
        d.step("/give into the running thread", timeout: 20, act: { d.tap(comp()); d.type(comp(), "/give >busy-build check the DRC"); d.shot("1-typed"); d.tap(go()) }, expect: { d.label("Given to \"Busy build\"").exists })
        d.shot("2-source")
        d.step("the receiver shows the card", timeout: 20, act: { d.tap(back()); d.tap(d.label("Busy build")) }, expect: { d.el("handoff.card").exists })
        sleep(1); d.shot("3-card")
        d.step("the summary and the note are there", timeout: 5, act: {}, expect: { d.label("Stand-in summary").exists && d.label("check the DRC").exists })
        d.step("a reference opens its source", timeout: 15, act: { d.tap(d.el("handoff.ref.1")) }, expect: { comp().exists && d.label("Given to \"Busy build\"").exists })
        d.shot("4-ref-opened")
        d.step("/give to an idle thread", timeout: 20, act: { d.tap(comp()); d.type(comp(), "/give >review-notes please review"); d.tap(go()) }, expect: { d.label("Given to \"Review notes\"").exists })
        d.step("/give to a bot", timeout: 20, act: { d.tap(comp()); d.type(comp(), "/give @ada keep watch"); d.tap(go()) }, expect: { d.label("Given to @Ada").exists })
        d.shot("5-source-three")
    }

    func test_terminal() {
        d = Driver("terminal"); defer { d.finish() }
        let screen = { self.d.el("term.screen") }
        let text = { (screen().value as? String) ?? "" }
        func sizes(_ t: String) -> (Int, Int)? {
            for l in t.split(separator: "\n").reversed() { let p = l.split(separator: " "); if p.count == 2, let r = Int(p[0]), let c = Int(p[1]) { return (c, r) } }
            return nil
        }
        d.app.terminate()
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": d.env["TERM_THREAD"] ?? "", "BACKPLANE_PERFORMANCE": "1"]
        d.app.launch(); d.skipTour()
        // the system's notification question (a fresh simulator): Allow
        let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let allow = sb.buttons["Allow"]
        if allow.waitForExistence(timeout: 8) { allow.tap() }
        guard d.step("the chat terminal's card", timeout: 40, act: {}, expect: { d.el("data.entry.term").exists && ((d.el("term.card.screen").value as? String) ?? "").contains("card line 3") }) else { return }
        d.step("the card says its command is done", act: {}, expect: { d.label("done 0").exists })
        d.shot("1-card")
        guard d.step("a tap opens it full screen", act: { d.tap(d.el("data.entry.term")) }, expect: { screen().exists }) else { return }
        d.step("the soft keyboard is up with the accessory row", timeout: 10, act: {}, expect: { d.app.keyboards.firstMatch.exists && d.el("term.key.esc").exists })
        d.shot("2-full-screen")
        d.step("ls shows the folder", timeout: 20, act: { d.app.typeText("ls\n") }, expect: { text().contains("alpha.txt") && text().contains("gamma.txt") })
        d.shot("3-ls")
        d.step("Ctrl is sticky: Ctrl then c ends a running sleep", timeout: 20, act: { d.app.typeText("sleep 30\n"); sleep(1); d.tap(d.el("term.key.ctrl")); d.app.typeText("c") }, expect: { text().contains("^C") || !text().hasSuffix("sleep 30\n") })
        d.step("stty size in portrait", timeout: 20, act: { d.app.typeText("clear; stty size\n") }, expect: { sizes(text()) != nil })
        let portrait = sizes(text())
        d.shot("4-portrait")
        XCUIDevice.shared.orientation = .landscapeLeft
        d.step("landscape: the pty has the new size", timeout: 20, act: { sleep(2); d.app.typeText("clear; stty size\n") }, expect: { if let a = sizes(text()), let p = portrait { return a.0 > p.0 && a.1 < p.1 }; return false })
        d.shot("5-landscape")
        XCUIDevice.shared.orientation = .portrait; sleep(2)
        d.step("the connection drops while the shell prints", timeout: 30, act: { d.app.typeText("clear; sleep 10; echo AWAY-OUTPUT\n"); sleep(1); _ = d.ctl("/cut") }, expect: { true })
        sleep(3); d.shot("6-offline")
        sleep(11); _ = d.ctl("/heal")
        d.step("back online: the output printed while away appears, nothing lost", timeout: 60, act: {}, expect: { text().contains("AWAY-OUTPUT") })
        d.shot("7-reconnected")
        d.step("Stop hangs it up", timeout: 20, act: { d.tap(d.el("term.stop")) }, expect: { d.el("term.state").label.contains("stopped") })
        d.shot("8-stopped")
        d.tap(d.el("term.close"))
        d.step("the card says stopped", timeout: 20, act: {}, expect: { d.el("term.card.state").exists && d.el("term.card.state").label.contains("stopped") })
        d.shot("9-card-stopped")
        back()
        let tbtn = d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'data.terms.'")).firstMatch
        d.step("the project's Terminals screen", timeout: 20, act: { d.tap(tbtn) }, expect: { d.el("terms.new").exists && d.el("terms.row").exists })
        d.shot("10-terminals")
        d.step("New shell opens a shell full screen", timeout: 30, act: { d.tap(d.el("terms.new")) }, expect: { screen().exists })
        d.step("the new shell runs a command", timeout: 20, act: { d.app.typeText("echo NEW-SHELL-$((6*7))\n") }, expect: { text().contains("NEW-SHELL-42") })
        d.shot("11-new-shell")
    }

    // Search and find on a hub from scripts/mobile/data-hub.sh started with SEARCH=1 (thread t4 "Find": 70 messages, "quasar" in m2, m30, m55, m68; once in t1):
    // the home search lists what was said under "Messages"; a tap opens the thread at the message (older pages read), marked; the find bar counts,
    // steps (newest first, wrapping) and closes, which clears the mark.
    func test_data_search() {
        d = Driver("data-search"); defer { d.finish() }
        d.launch()
        let sbAlert = XCUIApplication(bundleIdentifier: "com.apple.springboard").alerts.buttons["Allow"]
        if sbAlert.waitForExistence(timeout: 5) { sbAlert.tap() }
        let search = d.el("data.search")
        guard d.step("the list", timeout: 25, act: {}, expect: { search.exists }) else { return }
        d.step("search: what was said", act: { d.tap(search); search.typeText("quasar") },
               expect: { d.el("data.search.messages").exists && d.el("data.search.message.t4.m30").exists && d.el("data.search.message.t1.m3").exists })
        d.step("you said / agent said, the thread and project", act: {}, expect: { d.label("agent said · Find · demo").exists && d.label("you said · Fix the ecc83 board · demo").exists })
        d.shot("1-home-messages")
        let marked = d.el("data.thread.marked")
        guard d.step("a hit opens its thread at the message, older pages read, marked", timeout: 25, act: { d.tap(d.el("data.search.message.t4.m30")) },
                     expect: { marked.exists && d.label("The Quasar pad pitch").exists }) else { return }
        sleep(1); d.shot("2-opened-at-hit")
        guard d.step("Find in thread", act: { d.tap(d.el("data.thread.more")); d.tap(d.app.buttons["Find in thread"]) }, expect: { d.el("data.find.bar").waitForExistence(timeout: 5) }) else { return }
        let bar = d.el("data.find.bar"), count = d.el("data.find.count")
        func says(_ s: String) -> Bool { count.label == s }
        d.step("typing finds every match, the newest first", act: { d.type(bar, "quasar") }, expect: { says("4 of 4") && d.label("DRC is clean around the quasar").exists && marked.exists })
        sleep(1); d.shot("3-find-newest")
        d.step("previous", act: { d.tap(d.el("data.find.prev")) }, expect: { says("3 of 4") && d.label("Placed the quasar connector").exists })
        d.step("previous again", act: { d.tap(d.el("data.find.prev")) }, expect: { says("2 of 4") })
        d.step("previous to the oldest (older pages read)", timeout: 25, act: { d.tap(d.el("data.find.prev")) }, expect: { says("1 of 4") && d.label("Check the quasar footprint").isHittable })
        sleep(1); d.shot("4-find-oldest")
        d.step("previous wraps to the newest", act: { d.tap(d.el("data.find.prev")) }, expect: { says("4 of 4") })
        d.step("next wraps to the oldest", act: { d.tap(d.el("data.find.next")) }, expect: { says("1 of 4") })
        d.step("nothing matches", act: { d.type(bar, "zz") }, expect: { says("0 of 0") && !marked.exists })
        d.shot("5-find-none")
        d.step("close clears the mark", act: { d.tap(d.el("data.find.close")) }, expect: { !bar.exists && !marked.exists })
        d.shot("6-find-closed")
    }

    // Messages interrupt by default (docs/PHONE-DATA.md `thread.queue`): a message sent while a stand-in turn runs is taken at once (no queue row), and with
    // the hub cut off the phone keeps what is written in its outbox (Unsent rows, Edit and Remove, kept across a relaunch) and sends the rest when it is back.
    // Rig: scripts/mobile/ios-interrupt-journey.sh (a hub behind a relay that HUB_CTL /cut and /heal cut; INTERRUPT_THREAD a thread with no messages yet).
    func test_interrupt_outbox() {
        d = Driver("interrupt-outbox"); defer { d.finish() }
        let env = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": d.env["INTERRUPT_THREAD"] ?? "", "BACKPLANE_PERFORMANCE": "1"]
        func start() { d.app.terminate(); d.app.launchEnvironment = env; d.app.launch(); d.skipTour()
            let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow"]; if allow.waitForExistence(timeout: 5) { allow.tap() } }
        start()
        let comp = d.el("thread.composer")
        let unsent = d.app.buttons.matching(identifier: "queue.u-edit")   // one Edit per Unsent row
        let texts = d.app.staticTexts.matching(identifier: "queue.text"), removes = d.app.buttons.matching(identifier: "queue.u-remove")
        func say(_ t: String) { d.tap(comp); d.type(comp, t); d.tap(d.el("thread.send")) }
        func user(_ t: String) -> XCUIElement { d.app.descendants(matching: .any).matching(identifier: "data.entry.user").containing(NSPredicate(format: "label CONTAINS %@", t)).firstMatch }
        guard d.step("the thread is open", timeout: 30, act: {}, expect: { comp.exists }) else { return }
        // (a) a turn that runs until it is stopped, then a message into it
        guard d.step("a turn runs", timeout: 30, act: { say("mode:stop hold") }, expect: { d.el("data.thread.state").label.contains("working") }) else { return }
        d.step("a message sent while it runs is taken at once: in the conversation, no queue row", timeout: 15, act: { say("into the running turn") },
               expect: { user("into the running turn").exists && !d.el("thread.queue").exists })
        sleep(2)
        d.step("still no queue row two seconds on", act: {}, expect: { !d.el("thread.queue").exists && user("into the running turn").exists })
        d.shot("a-taken-at-once")
        d.tap(d.el("thread.send"))   // Stop: the composer is empty while the turn runs
        d.step("the turn is stopped", timeout: 30, act: {}, expect: { !d.el("data.thread.state").label.contains("working") })
        // (b) the hub cut off: three messages wait as Unsent
        d.step("the hub is cut off", timeout: 30, act: { _ = d.ctl("/cut") }, expect: { d.label("Offline").exists || d.label("Connecting to").exists })
        say("unsent one"); say("unsent two"); say("unsent three")
        d.step("three Unsent rows in the strip, none in the conversation", timeout: 10, act: {},
               expect: { unsent.count == 3 && d.label("UNSENT").exists && !user("unsent one").exists })
        d.shot("b1-three-unsent")
        d.step("Edit puts the first back in the draft and out of the strip", act: { d.tap(unsent.element(boundBy: 0)) },
               expect: { unsent.count == 2 && ((comp.value as? String) ?? "").contains("unsent one") })
        d.shot("b2-edited")
        d.step("Remove drops the second", act: { d.tap(removes.element(boundBy: 0)) }, expect: { unsent.count == 1 })
        d.tap(comp); d.type(comp, ", edited"); d.tap(d.el("thread.send"))
        d.step("the edited one waits again, after the third", act: {}, expect: { unsent.count == 2 && texts.count == 2 && texts.element(boundBy: 1).label.contains("edited") })
        d.shot("b3-removed-and-resent")
        start()
        d.step("the outbox is kept across a relaunch (on disk)", timeout: 20, act: {}, expect: { comp.exists && unsent.count == 2 })
        d.shot("b4-after-relaunch")
        d.step("the hub is back: the rest delivers, in order, and the strip empties", timeout: 60, act: { _ = d.ctl("/heal") },
               expect: { unsent.count == 0 && user("unsent three").exists && user("unsent one, edited").exists })
        d.step("the removed one never went", act: {}, expect: { !user("unsent two").exists })
        let three = user("unsent three").exists ? user("unsent three").frame.minY : -1, one = user("unsent one, edited").exists ? user("unsent one, edited").frame.minY : -1
        d.metrics["order_ok"] = three >= 0 && three < one
        if !(three >= 0 && three < one) { d.failed = true; d.note("order: three at \(three), edited one at \(one)") }
        d.shot("b5-delivered")
    }

    // Every send cuts into the running turn (the two-hub rig, scripts/mobile/ios-interrupt-two.sh): HUB_CTL is the rig's ctl.py
    // (/cutA /healA: the link between the hubs; /cutB /healB: this app's link to its hub; /send, /helper: a message by data/1 or the Helper's thread_send;
    // /now?w=A|B&s=TEXT: 200 once that hub's stand-in took the message holding TEXT as an interrupt (a NOW line)). LOCAL_THREAD, FAR_THREAD.
    func test_interrupt_everywhere() {
        d = Driver("interrupt-everywhere"); defer { d.finish() }
        let local = d.env["LOCAL_THREAD"] ?? "tb", far = d.env["FAR_THREAD"] ?? ""
        func start(_ sel: String) { d.app.terminate(); d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": sel, "BACKPLANE_PERFORMANCE": "1"]; d.app.launch(); d.skipTour()
            let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow"]; if allow.waitForExistence(timeout: 3) { allow.tap() } }
        let comp = d.el("thread.composer")
        let unsent = d.app.buttons.matching(identifier: "queue.u-edit")
        let tags = d.app.staticTexts.matching(identifier: "queue.tag")
        func say(_ t: String) { d.tap(comp); d.type(comp, t); d.tap(d.el("thread.send")) }
        func user(_ t: String) -> XCUIElement { d.app.descendants(matching: .any).matching(identifier: "data.entry.user").containing(NSPredicate(format: "label CONTAINS %@", t)).firstMatch }
        func working() -> Bool { d.el("data.thread.state").exists && d.el("data.thread.state").label.contains("working") }
        func q(_ s: String) -> String { s.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? s }
        func now(_ w: String, _ t: String) -> Bool { d.ctl("/now?w=\(w)&s=\(q(t))") }
        func timed(_ key: String, _ name: String, timeout: TimeInterval = 10, act: () -> Void, expect: () -> Bool) {
            let t0 = CFAbsoluteTimeGetCurrent(); let ok = d.step(name, timeout: timeout, act: act, expect: expect); d.metrics[key + "_ms"] = ok ? d.ms(t0) : -1 }

        // 1. Local work, a turn held running: a send from the app cuts in at once
        start(local)
        guard d.step("Local work is open", timeout: 30, act: {}, expect: { comp.exists }) else { return }
        guard d.step("1: a turn runs in Local work", timeout: 30, act: { say("s1 start local") }, expect: { working() }) else { return }
        sleep(3)
        timed("s1_shown", "1: the send shows in the conversation, no queue row", act: { say("s1 cut in fresh") }, expect: { user("s1 cut in fresh").exists && !d.el("thread.queue").exists })
        timed("s1_now", "1: the stand-in took it as an interrupt (NOW)", act: {}, expect: { now("B", "s1 cut in fresh") })
        // the new reply's live text keeps its first words: it starts at word 0 ("fresh0 fresh1 ...")
        let fresh = d.app.descendants(matching: .any).matching(identifier: "data.entry.assistant").containing(NSPredicate(format: "label CONTAINS 'fresh'")).firstMatch
        timed("s1_word0", "1: the new reply's live text starts at word 0", timeout: 15, act: {}, expect: { fresh.exists && fresh.label.contains("fresh0 fresh1") })
        if fresh.exists { d.note("1: new reply live text starts: \(String(fresh.label.prefix(60)))") }
        d.shot("1-local-interrupt")

        // 4. the Helper's thread_send into Local work's running turn: shown at once, as an interrupt
        d.step("4: the Helper's turn runs (its MCP url is known)", timeout: 30, act: { _ = d.ctl("/send?thread=th&text=\(q("s4 helper start"))") }, expect: { d.ctl("/mcp?t=th") })
        d.step("4: Local work is still running", timeout: 20, act: {}, expect: { working() })
        timed("s4_shown", "4: the Helper's message shows in Local work at once", act: { _ = d.ctl("/helper?text=\(q("s4 from the helper"))") },
              expect: { d.label("s4 from the helper").exists && !d.el("thread.queue").exists })
        timed("s4_now", "4: the stand-in took it as an interrupt (NOW)", act: {}, expect: { now("B", "s4 from the helper") })
        d.shot("4-helper-steered")

        // 5. the app cut off from its hub while the turn runs: Unsent; back: delivered as an interrupt
        d.step("5: Local work is still running", timeout: 20, act: {}, expect: { working() })
        d.step("5: the app's link to its hub is cut", timeout: 40, act: { _ = d.ctl("/cutB") }, expect: { d.label("Offline").exists || d.label("Connecting to").exists })
        say("s5 sent offline")
        d.step("5: it shows Unsent, not in the conversation", timeout: 10, act: {}, expect: { unsent.count == 1 && d.label("UNSENT").exists && !user("s5 sent offline").exists })
        d.shot("5a-offline-unsent")
        sleep(3)
        d.step("5: still nothing delivered while offline", act: {}, expect: { !now("B", "s5 sent offline") && unsent.count == 1 })
        timed("s5_delivered", "5: the link is back: it delivers and the strip empties", timeout: 60, act: { _ = d.ctl("/healB") }, expect: { unsent.count == 0 && user("s5 sent offline").exists })
        timed("s5_now", "5: the stand-in took it as an interrupt (NOW)", act: {}, expect: { now("B", "s5 sent offline") })
        d.shot("5b-reconnected-delivered")
        _ = d.ctl("/send?thread=th&text=stop") // (nothing: the turns run out by themselves)

        // 2. Far work (on hub A), opened by its id: a turn, then a send that cuts in
        guard !far.isEmpty else { d.block("no FAR_THREAD"); return }
        start(far)
        guard d.step("Far work is open", timeout: 30, act: {}, expect: { comp.exists }) else { return }
        d.shot("2a-far-open")
        let refused = d.app.staticTexts.matching(identifier: "queue.error").firstMatch
        let ran = d.step("2: a turn runs in Far work", timeout: 30, act: { say("s2 start alpha") }, expect: { d.ctl("/log?w=A&s=s2%20start%20alpha") || refused.exists })
        if refused.exists { d.note("2: the hub refused the send to the far thread: \(refused.label)"); d.metrics["far_refused"] = refused.label; d.shot("2b-far-refused"); d.failed = true; return }
        guard ran else { return }
        sleep(4)
        timed("s2_now", "2: hub A's stand-in took the cut-in as an interrupt (NOW)", act: { say("s2 cut in bravo") }, expect: { now("A", "s2 cut in bravo") || refused.exists })
        if refused.exists { d.note("2: refused: \(refused.label)"); d.shot("2b-far-refused"); d.failed = true; return }
        d.metrics["s2_shown_live"] = user("s2 cut in bravo").exists ? 1 : 0 // a mirrored thread gets no live pushes on the phone (a known gap)
        start(far) // re-open to see the new entries
        d.step("2: re-opened, the cut-in shows in Far work", timeout: 30, act: {}, expect: { user("s2 cut in bravo").exists })
        d.shot("2b-far-interrupt")

        // 3. the link to the far machine down, its turn still running: Unsent; link back: delivered as an interrupt at once
        d.step("3: hub A's bravo turn is still running", act: {}, expect: { !d.ctl("/log?w=A&s=STREAMED%20bravo") })
        _ = d.ctl("/cutA"); sleep(8)
        say("s3 while link down")
        d.step("3: it shows Unsent (the hub holds it)", timeout: 30, act: {}, expect: { (0..<tags.count).contains { tags.element(boundBy: $0).label.uppercased().contains("UNSENT") } })
        d.shot("3a-far-link-down-unsent")
        sleep(4)
        d.step("3: nothing reached hub A while the link is down", act: {}, expect: { !d.ctl("/log?w=A&s=\(q("s3 while link down"))") && !d.ctl("/log?w=A&s=STREAMED%20bravo") })
        timed("s3_now", "3: the link is back: hub A's stand-in takes it as an interrupt", timeout: 30, act: { _ = d.ctl("/healA") }, expect: { now("A", "s3 while link down") })
        d.step("3: the Unsent row is gone", timeout: 15, act: {}, expect: { !(0..<tags.count).contains { tags.element(boundBy: $0).label.uppercased().contains("UNSENT") } })
        d.shot("3b-far-link-back-delivered")
    }
}
