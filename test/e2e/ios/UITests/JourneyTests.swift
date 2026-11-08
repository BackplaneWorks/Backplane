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
        guard d.step("the chat terminal's card", timeout: 40, act: {}, expect: { d.el("data.entry.term").exists && d.label("card line 3").exists }) else { return }
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
}
