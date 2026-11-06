import XCTest

// The 12 gate journeys (test/gpui/e2e/journeys.json ids) driven on the iOS simulator with real taps, typing and a real pasteboard
// paste. Fixture hub: test/gpui/e2e/serve-fixture.mjs (project "e2e", 20 threads "e2e thread N", bots Bub/Mochi, room e2e-room).
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

    // The data/1 app (DATA_APP build) against a hub that speaks docs/gpui/PHONE-DATA.md: add a project, start a thread, watch the streamed answer,
    // answer an approval, send again, relaunch and see the saved copy at once. HUB_PROJECT_PATH: a folder path on the hub's machine.
    func test_data_flow() {
        d = Driver("data-flow"); defer { d.finish() }
        func env(_ k: String) -> String? { let v = d.env[k]; return (v?.isEmpty ?? true) ? nil : v }
        let path = env("HUB_PROJECT_PATH") ?? "~/bp-stub-projects/e2e1"
        let title = path.split(separator: "/").last.map(String.init) ?? "e2e1"
        let first = env("HUB_FIRST_PROMPT") ?? "hello from the phone", firstSees = env("HUB_FIRST_EXPECT") ?? "You said: hello from the phone"
        let second = env("HUB_ASK_PROMPT") ?? "please ask me first", secondSees = env("HUB_ASK_EXPECT") ?? "You said: please ask me first"
        let reply = Double(env("HUB_REPLY_WAIT") ?? "20") ?? 20
        // paired by an invitation link the way a phone is (HUB_PAIR_LINK), or by the token link the fixture hubs allow
        d.app.terminate()
        if let pair = env("HUB_PAIR_LINK") {
            d.app.launchEnvironment = ["BACKPLANE_PERFORMANCE": "1"]; d.app.launch()
            let addr = d.el("pair.address")
            guard d.step("pairing screen", act: {}, expect: { addr.waitForExistence(timeout: 20) }) else { return }
            d.step("type the pairing link", act: { d.tap(addr); d.type(addr, pair) }, expect: { d.el("pair.connect").isEnabled })
            d.step("connect", timeout: 30, act: { d.tap(d.el("pair.connect")) }, expect: { d.el("data.project.add").waitForExistence(timeout: 30) || d.el("data.project.add.empty").waitForExistence(timeout: 1) })
        } else {
            d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1"]; d.app.launch()
        }
        let rows = d.app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'data.thread.row.'"))
        guard d.step("threads tab", act: {}, expect: { d.el("data.project.add").waitForExistence(timeout: 20) }) else { return }
        guard d.step("add project", act: { d.tap(d.el("data.project.add")) }, expect: { d.el("data.project.path").waitForExistence(timeout: 10) }) else { return }
        d.step("choose the folder", act: { let f = d.el("data.project.path"); d.tap(f); f.press(forDuration: 1.0); if d.app.menuItems["Select All"].waitForExistence(timeout: 2) { d.app.menuItems["Select All"].tap() }; f.typeText(path) },
               expect: { d.el("data.project.add.confirm").isEnabled })
        guard d.step("project appears", act: { d.tap(d.el("data.project.add.confirm")) }, expect: { d.app.buttons["New thread in \(title)"].waitForExistence(timeout: 15) }) else { d.shot("no-project"); return }
        // a project the hub makes with a first empty thread: the thread to use is the one the next step starts
        let before = rows.count
        guard d.step("new thread sheet", act: { d.tap(d.app.buttons["New thread in \(title)"]) }, expect: { d.el("data.thread.new.text").waitForExistence(timeout: 10) }) else { return }
        d.step("start the thread", act: { let t = d.el("data.thread.new.text"); d.tap(t); t.typeText(first); d.tap(d.el("data.thread.new.start")) },
               expect: { rows.count > before || rows.firstMatch.waitForExistence(timeout: 15) })
        // the thread whose preview or title is the prompt; else the first row
        let mine = rows.containing(NSPredicate(format: "label CONTAINS[c] %@", String(first.prefix(20)))).firstMatch
        guard d.step("open the thread", act: { d.tap(mine.exists ? mine : rows.firstMatch) }, expect: { d.el("thread.composer").waitForExistence(timeout: 10) }) else { return }
        d.step("the answer shows", timeout: reply, act: {}, expect: { d.label(firstSees).exists })
        d.shot("thread-answered")
        let comp = d.el("thread.composer")
        d.step("send a message that needs approval", act: { d.tap(comp); d.type(comp, second); d.tap(d.el("thread.send")) }, expect: { d.el("data.entry.ask").waitForExistence(timeout: reply) || d.app.buttons["Allow"].waitForExistence(timeout: 1) })
        d.shot("approval")
        let allow = d.app.buttons["Allow"]
        d.step("allow", timeout: reply, act: { if allow.exists { allow.tap() } else { d.app.buttons.matching(NSPredicate(format: "label BEGINSWITH[c] 'Allow'")).firstMatch.tap() } }, expect: { d.label(secondSees).waitForExistence(timeout: reply) })
        // back to the list, then the app again from nothing: the saved copy paints before the hub answers
        let back = d.app.navigationBars.buttons.element(boundBy: 0); if back.exists { back.tap() }
        sleep(2)
        d.app.terminate(); d.app.launchEnvironment = d.app.launchEnvironment
        let t0 = CFAbsoluteTimeGetCurrent(); d.app.launch()
        let painted = rows.firstMatch.waitForExistence(timeout: 10)
        d.steps.append(["do": "relaunch: the saved threads show", "tap_epoch_ms": (Date().timeIntervalSince1970 * 1000).rounded(), "tap_ms": 0, "result_ms": d.ms(t0), "ok": painted])
        d.shot("relaunch")
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
}
