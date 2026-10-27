import XCTest

// Runs against a dedicated simulator only. Never launch with BACKPLANE_LINK:
// the initial pairing must use the normal UI and survive a subsequent launch.
final class QualificationUITests: XCTestCase {
    private func qualificationStatus(_ app: XCUIApplication, label: String) {
        let status = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "qualification.connection.")).firstMatch
        if status.exists {
            print("qualification-status \(label) value=\(status.value as? String ?? "unavailable")")
            return
        }
        print("qualification-status \(label) absent")
    }
    private func closeRestoredViewer(_ app: XCUIApplication) {
        let close = app.buttons["viewer.close"]
        if close.exists {
            let settled = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == NO OR (enabled == YES AND hittable == YES)"), object: close)
            let result = XCTWaiter.wait(for: [settled], timeout: 10)
            let visible = close.exists
            print("safe-restored-viewer wait-complete=\(result == .completed) close-exists=\(visible)")
            if result != .completed && app.state == .runningForeground && !app.textFields["pair.address"].exists {
                let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "restored-viewer-stuck"; shot.lifetime = .keepAlways; add(shot)
            }
            XCTAssertEqual(result, .completed, "A restored viewer must become dismissible or disappear")
            if visible {
                XCTAssertTrue(close.isEnabled && close.isHittable, "A restored viewer must allow returning to its thread")
                close.tap()
            }
        }
    }
    override func setUp() {
        continueAfterFailure = false
    }

    func testFreshSimulatorShowsPairing() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertTrue(app.textFields["pair.address"].waitForExistence(timeout: 20))
        let address = app.textFields["pair.address"]
        let connect = app.buttons["pair.connect"]
        XCTAssertTrue(connect.exists)
        address.tap()
        address.typeText("ftp://invalid")
        connect.tap()
        XCTAssertTrue(app.staticTexts["pair.address.error"].waitForExistence(timeout: 5))
        address.tap()
        address.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: "ftp://invalid".count))
    }

    // Run after pairing through the app UI. No credential is passed to XCTest.
    func testPersistedPairingAndNavigation() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertTrue(app.buttons["tab.threads"].waitForExistence(timeout: 45), "A paired hub should reach the app")
        XCTAssertFalse(app.textFields["pair.address"].exists, "Pairing must persist across launches")
        let threads = app.buttons["tab.threads"]
        if threads.waitForExistence(timeout: 5) { threads.tap() }
        let rows = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "thread.row."))
        XCTAssertTrue(rows.firstMatch.waitForExistence(timeout: 30), "Real hub thread rows must load")
        rows.firstMatch.tap()
        XCTAssertTrue(app.textViews["thread.composer"].waitForExistence(timeout: 20) || app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 5))
        app.terminate()
        app.launch()
        XCTAssertTrue(app.buttons["tab.threads"].waitForExistence(timeout: 45), "Warm launch must reconnect without a pairing override")
        XCTAssertFalse(app.textFields["pair.address"].exists)
    }
    private func tab(_ name: String, label: String, in app: XCUIApplication) -> XCUIElement {
        let identified = app.buttons["tab." + name]
        if identified.waitForExistence(timeout: 3) { return identified }
        return app.tabBars.buttons[label]
    }

    func testRepeatedNavigationViewerAndForeground() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertTrue(app.buttons["tab.threads"].waitForExistence(timeout: 45))
        let home = tab("home", label: "Home", in: app)
        let threads = tab("threads", label: "Threads", in: app)
        for _ in 0..<3 {
            XCTAssertTrue(home.waitForExistence(timeout: 5)); home.tap()
            XCTAssertTrue(threads.waitForExistence(timeout: 5)); threads.tap()
        }
        let rows = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "thread.row."))
        XCTAssertTrue(rows.firstMatch.waitForExistence(timeout: 30))
        rows.firstMatch.tap()
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 20))
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 30), "Foreground should restore the thread")
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = "real-thread"
        shot.lifetime = .keepAlways
        add(shot)
    }

    func testBoardViewerForSelectedThread() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let threads = tab("threads", label: "Threads", in: app)
        XCTAssertTrue(threads.waitForExistence(timeout: 5)); threads.tap()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let search = app.textFields["projects.search"]
        XCTAssertTrue(search.waitForExistence(timeout: 10)); search.tap(); search.typeText("PoE")
        let project = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "project.row.")).firstMatch
        let found = project.waitForExistence(timeout: 20)
        let evidence = XCTAttachment(screenshot: app.screenshot()); evidence.name = "board-project-search"; evidence.lifetime = .keepAlways; add(evidence)
        XCTAssertTrue(found, "Saved board project must be listed")
        var rows = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "thread.row."))
        if !rows.firstMatch.waitForExistence(timeout: 5) {
            let settled = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Settled")).firstMatch
            if settled.waitForExistence(timeout: 5) { settled.tap() }
        }
        rows = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "thread.row."))
        XCTAssertTrue(rows.firstMatch.waitForExistence(timeout: 15), "An existing thread for the board project is required")
        rows.firstMatch.tap()
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15))
        for _ in 0..<2 {
            open.tap()
            let board = app.buttons["viewer.choice.board"]
            XCTAssertTrue(board.waitForExistence(timeout: 10)); board.tap()
            XCTAssertTrue(app.descendants(matching: .any)["viewer.ready"].waitForExistence(timeout: 60), "Board geometry should arrive")
            let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "real-board"; shot.lifetime = .keepAlways; add(shot)
            let close = app.buttons["viewer.close"]
            XCTAssertTrue(close.waitForExistence(timeout: 10)); close.tap()
            XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 15))
        }
    }

    // Invoke the nonsecret simulator disconnect URL before this test. The
    // parked state must survive the launch XCTest performs here.
    func testClientRemainsParkedAfterRelaunch() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        let open = XCUIApplication(bundleIdentifier: "com.apple.springboard").alerts.buttons["Open"]
        if open.waitForExistence(timeout: 5) { open.tap() }
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 20))
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.parked"].waitForExistence(timeout: 30), "Observe park before termination")
        app.terminate()
        app.launch()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.parked"].waitForExistence(timeout: 20), "Park must survive relaunch")
    }

    // Invoke the nonsecret simulator reconnect URL before this test.
    func testClientRecoversAfterReconnect() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        let open = XCUIApplication(bundleIdentifier: "com.apple.springboard").alerts.buttons["Open"]
        if open.waitForExistence(timeout: 5) { open.tap() }
        app.terminate()
        app.launch()
        closeRestoredViewer(app)
        let ready = app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60)
        qualificationStatus(app, label: "relaunch-ready-wait")
        XCTAssertTrue(ready, "The real hub must provide fresh content after reconnect")
        XCTAssertFalse(app.descendants(matching: .any)["qualification.connection.parked"].exists)
        XCTAssertFalse(app.textFields["pair.address"].exists, "Reconnect should retain pairing")
    }

    // Pasteboard is populated separately over private stdin. Never type a link,
    // attach screenshots, print the accessibility tree, or pass a credential.
    func testPairViaPrivatePasteboard() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        let field = app.textFields["pair.address"]
        XCTAssertTrue(field.waitForExistence(timeout: 20))
        field.press(forDuration: 1.2)
        let paste = app.menuItems["Paste"]
        XCTAssertTrue(paste.waitForExistence(timeout: 5))
        paste.tap()
        let allowPaste = app.alerts.buttons["Allow Paste"]
        if allowPaste.waitForExistence(timeout: 3) { allowPaste.tap() }
        let connect = app.buttons["pair.connect"]
        XCTAssertTrue(connect.waitForExistence(timeout: 5)); connect.tap()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 90))
        XCTAssertFalse(field.exists)
    }

    func testShowRealProjects() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let decline = springboard.alerts.buttons["Don’t Allow"]
        if decline.waitForExistence(timeout: 3) { decline.tap() }
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let threads = tab("threads", label: "Threads", in: app)
        XCTAssertTrue(threads.waitForExistence(timeout: 10)); threads.tap()
        XCTAssertFalse(app.tabBars.firstMatch.exists, "Compact navigation must not also show the native tab bar")
        let rows = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "thread.row."))
        XCTAssertTrue(rows.firstMatch.waitForExistence(timeout: 30), "Real project thread rows must be visible")
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = "real-projects"
        shot.lifetime = .keepAlways
        add(shot)
        let search = app.textFields["projects.search"]
        XCTAssertTrue(search.waitForExistence(timeout: 10)); search.tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 10))
        let keyboard = XCTAttachment(screenshot: app.screenshot()); keyboard.name = "projects-keyboard"; keyboard.lifetime = .keepAlways; add(keyboard)
        search.typeText("PoE")
        XCTAssertEqual(search.value as? String, "PoE", "Search must display the typed filter")
        let filtered = XCTAttachment(screenshot: app.screenshot()); filtered.name = "projects-typed-filter"; filtered.lifetime = .keepAlways; add(filtered)
        search.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 3))
    }

    func testReturnToVerifiedExistingBoardHome() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        closeRestoredViewer(app)
        let threads = app.buttons["tab.threads"]
        XCTAssertTrue(threads.waitForExistence(timeout: 45)); threads.tap()
        let rootReady = app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60)
        qualificationStatus(app, label: "project-root-ready")
        XCTAssertTrue(rootReady)
        let search = app.textFields["projects.search"]
        if !search.exists {
            closeRestoredViewer(app)
            let buttons = app.navigationBars.buttons
            for button in buttons.allElementsBoundByIndex {
                let label = ["Back", "Threads"].contains(button.label) ? button.label : "other"
                print("safe-nav label=\(label) enabled=\(button.isEnabled) hittable=\(button.isHittable)")
            }
            let back = buttons.firstMatch
            let hasBack = back.waitForExistence(timeout: 10)
            qualificationStatus(app, label: "threads-root-setup")
            if !app.textFields["pair.address"].exists {
                let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "threads-root-setup"; shot.lifetime = .keepAlways; add(shot)
            }
            XCTAssertTrue(hasBack, "A restored thread must expose its navigation Back control")
            XCTAssertTrue(back.isEnabled && back.isHittable, "Back must remain usable while the thread loads")
            back.tap()
        }
        XCTAssertTrue(search.waitForExistence(timeout: 15)); search.tap()
        search.typeText("PoE_LoRa_Tube")
        let project = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@ AND identifier ENDSWITH %@", "project.header.", "p1790229698-1087")).firstMatch
        let found = project.waitForExistence(timeout: 40)
        qualificationStatus(app, label: "board-project-filter")
        print("safe-search expected-project=\(found)")
        if !app.textFields["pair.address"].exists { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "board-project-filter"; shot.lifetime = .keepAlways; add(shot) }
        XCTAssertTrue(found, "The filtered real board project must appear")
        let fresh = app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 45)
        qualificationStatus(app, label: "board-filter-after-wait")
        XCTAssertTrue(fresh, "Filtered presentation must become fresh")
        let settled = app.buttons["Settled 1"]
        XCTAssertTrue(settled.waitForExistence(timeout: 20), "The filtered project must have its known nonempty Settled shelf")
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 45), "The filtered presentation must be fresh before Return")
        XCTAssertTrue(project.exists && settled.exists, "The same board project and nonempty Settled shelf must still be present")
        qualificationStatus(app, label: "before-verified-project-return")
        search.tap()
        search.typeText(XCUIKeyboardKey.return.rawValue)
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 30))
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        qualificationStatus(app, label: "board-thread-after-return")
        let status = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "qualification.connection.")).firstMatch
        let data = (status.value as? String ?? "").data(using: .utf8) ?? Data()
        let fields = (try? JSONSerialization.jsonObject(with: data)) as? [String: String]
        XCTAssertTrue((fields?["nav"] ?? "").hasSuffix("|t1790229700-1088"), "Return must select the known existing board thread")
        if !app.textFields["pair.address"].exists { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "board-home-after-return"; shot.lifetime = .keepAlways; add(shot) }
    }

    func testViewerMenuBoundaryDiagnostic() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        closeRestoredViewer(app)
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 60))
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        qualificationStatus(app, label: "before-view-menu")
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15)); open.tap()
        let choices = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "viewer.choice."))
        for choice in choices.allElementsBoundByIndex { print("safe-choice id=\(choice.identifier) enabled=\(choice.isEnabled) hittable=\(choice.isHittable)") }
        let board = app.buttons["viewer.choice.board"]
        let hasBoard = board.waitForExistence(timeout: 10)
        qualificationStatus(app, label: "after-view-menu")
        print("safe-menu board-exists=\(hasBoard) open-enabled=\(open.isEnabled) open-hittable=\(open.isHittable)")
        if !app.textFields["pair.address"].exists {
            let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "viewer-menu-boundary"; shot.lifetime = .keepAlways; add(shot)
        }
        XCTAssertTrue(hasBoard, "Board menu choice must appear")
        print("safe-choice board enabled=\(board.isEnabled) hittable=\(board.isHittable)")
        qualificationStatus(app, label: "before-board-choice")
        board.tap()
        let presented = app.buttons["viewer.close"].waitForExistence(timeout: 20)
        qualificationStatus(app, label: "after-board-choice")
        print("safe-viewer modal=\(presented) ready=\(app.descendants(matching: .any)["viewer.ready"].exists)")
        if !app.textFields["pair.address"].exists {
            let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "viewer-open-diagnostic"; shot.lifetime = .keepAlways; add(shot)
        }
        XCTAssertTrue(presented, "Board viewer must open")
    }

    func testExistingBoardViaDebugSelection() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        closeRestoredViewer(app)
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 60), "Existing board thread must open")
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15)); open.tap()
        let choice = app.buttons["viewer.choice.board"]
        XCTAssertTrue(choice.waitForExistence(timeout: 10))
        qualificationStatus(app, label: "board-before-choice")
        print("safe-choice board enabled=\(choice.isEnabled) hittable=\(choice.isHittable)")
        choice.tap()
        let close = app.buttons["viewer.close"]
        let presented = close.waitForExistence(timeout: 15)
        qualificationStatus(app, label: "board-after-open-wait")
        let early = XCTAttachment(screenshot: app.screenshot()); early.name = "saved-board-initial"; early.lifetime = .keepAlways; add(early)
        let loaded = app.descendants(matching: .any)["viewer.ready"].waitForExistence(timeout: 35)
        for id in ["viewer.ready", "viewer.canvas", "viewer.loading", "viewer.error", "viewer.mesh.ready", "viewer.mesh.status", "viewer.open", "viewer.close", "qualification.connection.ready"] {
            print("diagnostic \(id)=\(app.descendants(matching: .any)[id].exists)")
        }
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "saved-board"; shot.lifetime = .keepAlways; add(shot)
        XCTAssertTrue(presented, "Board viewer must open after UI tap")
        XCTAssertTrue(loaded, "Saved board geometry must arrive")
        print("viewer-close initial enabled=\(close.isEnabled) hittable=\(close.isHittable)")
        XCTAssertTrue(close.isEnabled && close.isHittable)
        close.tap()
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 20))
        open.tap(); XCTAssertTrue(choice.waitForExistence(timeout: 10)); choice.tap()
        let reopened = app.descendants(matching: .any)["viewer.ready"].waitForExistence(timeout: 60)
        for id in ["viewer.close", "viewer.ready", "viewer.loading", "viewer.error", "viewer.mesh.ready", "qualification.connection.ready"] {
            print("reopen-state \(id)=\(app.descendants(matching: .any)[id].exists)")
        }
        let reopenshot = XCTAttachment(screenshot: app.screenshot()); reopenshot.name = "board-reopen"; reopenshot.lifetime = .keepAlways; add(reopenshot)
        XCTAssertTrue(reopened, "The board must reopen with geometry")
    }

    func testExplicitReconnectAfterOfflineClose() {
        let app = XCUIApplication()
        app.terminate()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.parked"].waitForExistence(timeout: 20))
        XCTAssertFalse(app.buttons["viewer.close"].exists)
        XCTContext.runActivity(named: "QUALIFICATION_RECONNECT_NOW") { _ in }
        let open = XCUIApplication(bundleIdentifier: "com.apple.springboard").alerts.buttons["Open"]
        if open.waitForExistence(timeout: 10) { open.tap() }
        let ready = app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60)
        qualificationStatus(app, label: "explicit-reconnect")
        XCTAssertTrue(ready, "Explicit reconnect must restore fresh content")
        XCTAssertFalse(app.descendants(matching: .any)["qualification.connection.parked"].exists)
        XCTAssertFalse(app.textFields["pair.address"].exists)
        app.terminate(); app.launch()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        XCTAssertFalse(app.buttons["viewer.close"].exists)
    }

    func testOfflineViewerClose() {
        let app = XCUIApplication()
        app.terminate()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        closeRestoredViewer(app)
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 60))
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15)); open.tap()
        let board = app.buttons["viewer.choice.board"]
        XCTAssertTrue(board.waitForExistence(timeout: 10)); board.tap()
        let close = app.buttons["viewer.close"]
        XCTAssertTrue(close.waitForExistence(timeout: 20))
        XCTAssertTrue(app.descendants(matching: .any)["viewer.ready"].waitForExistence(timeout: 60))
        XCTContext.runActivity(named: "QUALIFICATION_PARK_NOW") { _ in }
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let confirm = springboard.alerts.buttons["Open"]
        if confirm.waitForExistence(timeout: 10) { confirm.tap() }
        XCTAssertTrue(close.waitForExistence(timeout: 15), "Viewer remains open after isolated disconnect")
        print("safe-offline-close enabled=\(close.isEnabled) hittable=\(close.isHittable)")
        XCTAssertTrue(close.isEnabled && close.isHittable, "Close must work while the client is parked")
        close.tap()
        XCTAssertFalse(close.waitForExistence(timeout: 5), "Close must dismiss locally")
        let parked = app.descendants(matching: .any)["qualification.connection.parked"]
        XCTAssertTrue(parked.waitForExistence(timeout: 20), "The isolated client must actually be parked")
        if !app.textFields["pair.address"].exists { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "viewer-offline-closed"; shot.lifetime = .keepAlways; add(shot) }
        app.terminate(); app.launch()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.parked"].waitForExistence(timeout: 20))
        XCTAssertFalse(app.buttons["viewer.close"].exists, "A closed viewer must stay closed after parked relaunch")
    }

    func testExistingBoard3DMesh() {
        let app = XCUIApplication()
        addTeardownBlock {
            self.qualificationStatus(app, label: "3d-final")
            print("safe-3d-final foreground=\(app.state == .runningForeground) viewer-close=\(app.buttons["viewer.close"].exists)")
            if app.state == .runningForeground && !app.textFields["pair.address"].exists {
                let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "3d-final"; shot.lifetime = .keepAlways; self.add(shot)
            }
        }
        app.terminate()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        closeRestoredViewer(app)
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 60))
        let ready = app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60)
        qualificationStatus(app, label: "3d-initial-readiness")
        print("safe-3d-initial foreground=\(app.state == .runningForeground) pair-field=\(app.textFields["pair.address"].exists) ready=\(ready) close=\(app.buttons["viewer.close"].exists)")
        if app.state == .runningForeground && !app.textFields["pair.address"].exists { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "3d-initial-readiness"; shot.lifetime = .keepAlways; add(shot) }
        XCTAssertTrue(ready, "Initial real thread must become ready before opening the viewer")
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15)); open.tap()
        let choice = app.buttons["viewer.choice.3d"]
        XCTAssertTrue(choice.waitForExistence(timeout: 10))
        qualificationStatus(app, label: "3d-before-choice")
        choice.tap()
        let presented = app.buttons["viewer.close"].waitForExistence(timeout: 15)
        qualificationStatus(app, label: "3d-after-open-wait")
        XCTAssertTrue(presented)
        let mesh = app.descendants(matching: .any)["viewer.mesh.ready"]
        let loaded = mesh.waitForExistence(timeout: 90)
        for id in ["viewer.mesh.ready", "viewer.mesh.status", "viewer.loading", "viewer.error", "viewer.ready", "viewer.canvas"] { print("diagnostic \(id)=\(app.descendants(matching: .any)[id].exists)") }
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "saved-board-3d"; shot.lifetime = .keepAlways; add(shot)
        XCTAssertTrue(loaded, "Decoded 3D mesh required; slab/canvas readiness alone is insufficient")
        let from = app.coordinate(withNormalizedOffset: CGVector(dx: 0.38, dy: 0.50))
        let to = app.coordinate(withNormalizedOffset: CGVector(dx: 0.67, dy: 0.56))
        from.press(forDuration: 0.2, thenDragTo: to)
        XCTAssertTrue(mesh.waitForExistence(timeout: 20), "3D mesh must remain loaded after the orbit gesture")
        let after = XCTAttachment(screenshot: app.screenshot()); after.name = "saved-board-3d-after-orbit"; after.lifetime = .keepAlways; add(after)
    }

    func testExistingBoard3DZoom() {
        let app = XCUIApplication()
        app.terminate()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        closeRestoredViewer(app)
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 60))
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15)); open.tap()
        let choice = app.buttons["viewer.choice.3d"]
        XCTAssertTrue(choice.waitForExistence(timeout: 10)); choice.tap()
        let close = app.buttons["viewer.close"]
        XCTAssertTrue(close.waitForExistence(timeout: 15))
        let mesh = app.descendants(matching: .any)["viewer.mesh.ready"]
        XCTAssertTrue(mesh.waitForExistence(timeout: 90), "The decoded mesh must be loaded before zoom")
        let before = XCTAttachment(screenshot: app.screenshot()); before.name = "saved-board-3d-before-zoom"; before.lifetime = .keepAlways; add(before)
        app.pinch(withScale: 1.7, velocity: 1.0)
        XCTAssertTrue(mesh.waitForExistence(timeout: 20), "The decoded mesh must remain loaded after zoom")
        let after = XCTAttachment(screenshot: app.screenshot()); after.name = "saved-board-3d-after-zoom"; after.lifetime = .keepAlways; add(after)
        XCTAssertTrue(close.isEnabled && close.isHittable)
        close.tap()
        XCTAssertFalse(close.waitForExistence(timeout: 5))
    }

    private func openDroppedViewer(_ kind: String, drop: String) -> XCUIApplication {
        let app = XCUIApplication()
        app.terminate()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_QUALIFICATION_DROP_PLOTS"] = drop
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        closeRestoredViewer(app)
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 60))
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15)); open.tap()
        let choice = app.buttons["viewer.choice." + kind]
        XCTAssertTrue(choice.waitForExistence(timeout: 10)); choice.tap()
        XCTAssertTrue(app.buttons["viewer.close"].waitForExistence(timeout: 20))
        return app
    }

    func testMissingBoardTimeoutAndRetry() {
        let app = openDroppedViewer("board", drop: "all")
        XCTAssertTrue(app.descendants(matching: .any)["viewer.loading"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.descendants(matching: .any)["viewer.error"].waitForExistence(timeout: 45), "Missing geometry must time out")
        qualificationStatus(app, label: "missing-board-error")
        let retry = app.buttons["viewer.retry"]
        XCTAssertTrue(retry.waitForExistence(timeout: 5) && retry.isHittable)
        if !app.textFields["pair.address"].exists { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "missing-board-timeout"; shot.lifetime = .keepAlways; add(shot) }
        retry.tap()
        qualificationStatus(app, label: "missing-board-after-tap")
        let recovered = app.descendants(matching: .any)["viewer.ready"].waitForExistence(timeout: 90)
        qualificationStatus(app, label: "missing-board-recovered")
        print("safe-board-retry ready=\(recovered) close=\(app.buttons["viewer.close"].exists) error=\(app.descendants(matching: .any)["viewer.error"].exists) loading=\(app.descendants(matching: .any)["viewer.loading"].exists)")
        if !app.textFields["pair.address"].exists { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "missing-board-after-retry"; shot.lifetime = .keepAlways; add(shot) }
        XCTAssertTrue(recovered, "Retry must load real board geometry")
        XCTAssertFalse(app.descendants(matching: .any)["viewer.error"].exists)
        app.buttons["viewer.close"].tap()
    }

    func testMissingMeshTimeoutAndRetry() {
        let app = openDroppedViewer("3d", drop: "mesh")
        XCTAssertTrue(app.descendants(matching: .any)["viewer.ready"].waitForExistence(timeout: 30), "Real base geometry must arrive")
        XCTAssertTrue(app.descendants(matching: .any)["viewer.loading"].waitForExistence(timeout: 10))
        XCTAssertFalse(app.descendants(matching: .any)["viewer.mesh.ready"].exists)
        XCTAssertTrue(app.descendants(matching: .any)["viewer.error"].waitForExistence(timeout: 45), "Missing mesh must time out")
        qualificationStatus(app, label: "missing-mesh-error")
        let retry = app.buttons["viewer.retry"]
        XCTAssertTrue(retry.waitForExistence(timeout: 5) && retry.isHittable)
        if !app.textFields["pair.address"].exists { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "missing-mesh-timeout"; shot.lifetime = .keepAlways; add(shot) }
        retry.tap()
        XCTAssertTrue(app.descendants(matching: .any)["viewer.mesh.ready"].waitForExistence(timeout: 90), "Retry must load the real mesh")
        qualificationStatus(app, label: "missing-mesh-recovered")
        XCTAssertFalse(app.descendants(matching: .any)["viewer.error"].exists)
        app.buttons["viewer.close"].tap()
    }

    func testMissingBoardCloseAndReopenDeadline() {
        let app = openDroppedViewer("board", drop: "all")
        XCTAssertTrue(app.descendants(matching: .any)["viewer.loading"].waitForExistence(timeout: 15))
        let oldError = expectation(for: NSPredicate(format: "exists == true"), evaluatedWith: app.descendants(matching: .any)["viewer.error"])
        oldError.isInverted = true
        wait(for: [oldError], timeout: 10)
        app.buttons["viewer.close"].tap()
        XCTAssertFalse(app.buttons["viewer.close"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["viewer.open"].waitForExistence(timeout: 15)); app.buttons["viewer.open"].tap()
        let board = app.buttons["viewer.choice.board"]
        XCTAssertTrue(board.waitForExistence(timeout: 10)); board.tap()
        XCTAssertTrue(app.buttons["viewer.close"].waitForExistence(timeout: 20))
        XCTAssertTrue(app.descendants(matching: .any)["viewer.loading"].waitForExistence(timeout: 10))
        let reopenedAt = ProcessInfo.processInfo.systemUptime
        XCTAssertTrue(app.descendants(matching: .any)["viewer.error"].waitForExistence(timeout: 45), "Reopened presentation must have its own deadline")
        XCTAssertGreaterThan(ProcessInfo.processInfo.systemUptime - reopenedAt, 20, "The retired presentation's deadline must not expire the reopened viewer")
        XCTAssertTrue(app.buttons["viewer.close"].isHittable)
        app.buttons["viewer.close"].tap()
    }

    func testExternallyLaunchedBoard3DZoom() {
        let app = XCUIApplication()
        app.activate()
        closeRestoredViewer(app)
        let ready = app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60)
        qualificationStatus(app, label: "external-zoom-initial")
        XCTAssertTrue(ready, "Externally launched real thread must be ready")
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 15))
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15)); open.tap()
        let choice = app.buttons["viewer.choice.3d"]
        XCTAssertTrue(choice.waitForExistence(timeout: 10)); choice.tap()
        let close = app.buttons["viewer.close"]
        XCTAssertTrue(close.waitForExistence(timeout: 20))
        let mesh = app.descendants(matching: .any)["viewer.mesh.ready"]
        XCTAssertTrue(mesh.waitForExistence(timeout: 90))
        let before = XCTAttachment(screenshot: app.screenshot()); before.name = "external-zoom-before"; before.lifetime = .keepAlways; add(before)
        app.pinch(withScale: 1.7, velocity: 1.0)
        XCTAssertTrue(mesh.waitForExistence(timeout: 20))
        let after = XCTAttachment(screenshot: app.screenshot()); after.name = "external-zoom-after"; after.lifetime = .keepAlways; add(after)
        XCTAssertTrue(close.isHittable); close.tap()
    }

    func testObserved3DDeadlineAndRetry() {
        let app = XCUIApplication()
        app.activate()
        closeRestoredViewer(app)
        let ready = app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60)
        qualificationStatus(app, label: "3d-budget-initial")
        XCTAssertTrue(ready, "The selected real thread must be fresh before opening 3D")
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15)); open.tap()
        let choice = app.buttons["viewer.choice.3d"]
        XCTAssertTrue(choice.waitForExistence(timeout: 10)); choice.tap()
        let close = app.buttons["viewer.close"]
        XCTAssertTrue(close.waitForExistence(timeout: 20))
        let mesh = app.descendants(matching: .any)["viewer.mesh.ready"]
        let error = app.descendants(matching: .any)["viewer.error"]
        let loading = app.descendants(matching: .any)["viewer.loading"]
        let retry = app.buttons["viewer.retry"]
        let observedAt = ProcessInfo.processInfo.systemUptime
        let timedOut = error.waitForExistence(timeout: 40)
        let loaded = mesh.exists
        print("safe-3d-budget elapsed=\(ProcessInfo.processInfo.systemUptime - observedAt) mesh=\(loaded) loading=\(loading.exists) error=\(timedOut) retry=\(retry.exists) close=\(close.exists)")
        qualificationStatus(app, label: "3d-budget-observed")
        if !app.textFields["pair.address"].exists { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "3d-budget-observed"; shot.lifetime = .keepAlways; add(shot) }
        var recovered = loaded
        if !loaded && timedOut && retry.isHittable {
            retry.tap()
            recovered = mesh.waitForExistence(timeout: 60)
            print("safe-3d-budget-retry mesh=\(recovered) loading=\(loading.exists) error=\(error.exists) close=\(close.exists)")
            qualificationStatus(app, label: "3d-budget-after-retry")
            if !app.textFields["pair.address"].exists { let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "3d-budget-after-retry"; shot.lifetime = .keepAlways; add(shot) }
        }
        let closeWorks = close.exists && close.isHittable
        if closeWorks { close.tap() }
        XCTAssertTrue(closeWorks, "Close must remain available")
        XCTAssertTrue(recovered, "Real mesh must load initially or after an actionable Retry")
    }

    func testObserveNormalLaunchedUI() {
        let app = XCUIApplication()
        app.activate()
        closeRestoredViewer(app)
        XCTAssertFalse(app.buttons["viewer.close"].exists, "Thread observation requires the restored viewer to be closed")
        let composer = app.descendants(matching: .any)["thread.composer"]
        let threads = app.buttons["tab.threads"]
        let composerPresent = composer.waitForExistence(timeout: 30)
        let threadsPresent = threads.waitForExistence(timeout: 10)
        print("safe-normal-ui foreground=\(app.state == .runningForeground) pair-field=\(app.textFields["pair.address"].exists) thread-composer=\(composerPresent) threads-tab=\(threadsPresent)")
        XCTAssertTrue(app.state == .runningForeground && (composerPresent || threadsPresent), "Normal app UI must be visible")
        XCTAssertFalse(app.textFields["pair.address"].exists)
        if threadsPresent { threads.tap() }
        let visibleThread = composer.waitForExistence(timeout: 15)
        let project = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "project.header.")).firstMatch.waitForExistence(timeout: 15)
        print("safe-normal-threads composer=\(visibleThread) project-header=\(project)")
        XCTAssertTrue(visibleThread || project, "Threads must show an existing thread or project")
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "normal-no-env-ui"; shot.lifetime = .keepAlways; add(shot)
    }

    func testPairedHubsPasteControl() {
        let app = XCUIApplication()
        app.terminate()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        closeRestoredViewer(app)
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let threads = app.buttons["tab.threads"]
        XCTAssertTrue(threads.waitForExistence(timeout: 15)); threads.tap()
        let hubs = app.buttons["navigation.hubs"]
        if !hubs.waitForExistence(timeout: 5) {
            let back = app.navigationBars.buttons.firstMatch
            XCTAssertTrue(back.waitForExistence(timeout: 10)); back.tap()
        }
        XCTAssertTrue(hubs.waitForExistence(timeout: 15)); hubs.tap()
        let paste = app.descendants(matching: .any)["hubs.paste"]
        XCTAssertTrue(paste.waitForExistence(timeout: 15), "Paired Hubs must expose the native Paste control")
        XCTAssertTrue(paste.isHittable, "The Paste control must be reachable")
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "paired-hubs-paste-control"; shot.lifetime = .keepAlways; add(shot)
    }

    func testInvalidSessionOnFreshSimulator() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertTrue(app.descendants(matching: .any)["pair.connection.error"].waitForExistence(timeout: 60), "Real hub must reject the deliberately invalid credential")
        XCTAssertTrue(app.buttons["pair.connection.retry"].exists)
        XCTAssertTrue(app.buttons["pair.connection.change"].exists)
        XCTAssertFalse(app.descendants(matching: .any)["qualification.connection.ready"].exists)
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        XCTAssertFalse(springboard.alerts.buttons["Allow"].exists, "Fresh pairing should not prompt for notifications")
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "invalid-session"; shot.lifetime = .keepAlways; add(shot)
    }

    func testSmallSessionDiagnostic() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        _ = app.descendants(matching: .any)["pair.connection.error"].waitForExistence(timeout: 15)
        for id in ["pair.address", "pair.connection.error", "pair.connection.progress", "pair.connection.retry", "pair.connection.change", "qualification.connection.unpaired", "qualification.connection.connecting", "qualification.connection.offline", "qualification.connection.ready"] {
            print("diagnostic \(id)=\(app.descendants(matching: .any)[id].exists)")
        }
        if !app.textFields["pair.address"].exists {
            let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "small-offline"; shot.lifetime = .keepAlways; add(shot)
        }
    }

    func testViewerChoiceDiagnostic() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 60))
        let ready = app.descendants(matching: .any)["qualification.connection.ready"]
        XCTAssertTrue(ready.waitForExistence(timeout: 60))
        app.buttons["viewer.open"].tap()
        let choice = app.buttons["viewer.choice.board"]
        XCTAssertTrue(choice.waitForExistence(timeout: 10))
        print("viewer-choice-before ready=\(ready.exists) hittable=\(choice.isHittable)")
        choice.tap()
        let close = app.buttons["viewer.close"]
        let opened = close.waitForExistence(timeout: 20)
        print("viewer-choice-after ready=\(ready.exists) opened=\(opened) connecting=\(app.descendants(matching: .any)["qualification.connection.connecting"].exists)")
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "viewer-choice-diagnostic"; shot.lifetime = .keepAlways; add(shot)
        XCTAssertTrue(opened, "The board viewer must open")
    }

    func testBubNavigationDiagnostic() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        app.buttons["tab.home"].tap()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let card = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@ AND label CONTAINS[cd] %@", "home.uber.", "Bub")).firstMatch
        XCTAssertTrue(card.waitForExistence(timeout: 30))
        print("bub-card-before ready=\(app.descendants(matching: .any)["qualification.connection.ready"].exists) hittable=\(card.isHittable)")
        card.tap()
        let chat = app.buttons["bot.tab.chat"]
        _ = chat.waitForExistence(timeout: 20)
        for prefix in ["bot.screen.", "bot.tab.", "thread.composer", "qualification.connection."] {
            for item in app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", prefix)).allElementsBoundByIndex {
                print("safe-id \(item.identifier) selected=\(item.isSelected)")
            }
        }
        if !app.textFields["pair.address"].exists {
            let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "bub-diagnostic"; shot.lifetime = .keepAlways; add(shot)
        }
    }

    private func captureComposerAppearance(largeType: Bool) {
        let app = XCUIApplication()
        app.terminate()
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        if largeType { app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityM"] }
        app.launch()
        closeRestoredViewer(app)
        let composer = app.textFields["thread.composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 60))
        let usable = XCTNSPredicateExpectation(predicate: NSPredicate(format: "enabled == YES AND hittable == YES"), object: composer)
        XCTAssertEqual(XCTWaiter.wait(for: [usable], timeout: 60), .completed)
        XCTAssertFalse(app.textFields["pair.address"].exists)
        let before = XCTAttachment(screenshot: app.screenshot()); before.name = largeType ? "large-type-composer" : "composer-prompt"; before.lifetime = .keepAlways; add(before)
        composer.tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 15))
        XCTAssertTrue(composer.isHittable)
        let keyboard = XCTAttachment(screenshot: app.screenshot()); keyboard.name = largeType ? "large-type-keyboard" : "composer-keyboard"; keyboard.lifetime = .keepAlways; add(keyboard)
    }
    func testComposerAppearance() { captureComposerAppearance(largeType: false) }
    func testLargeTypeComposerAppearance() { captureComposerAppearance(largeType: true) }
    func testLargeTypeSettingsTabReachability() {
        let app = XCUIApplication()
        app.terminate()
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityM"]
        app.launch(); closeRestoredViewer(app)
        let composer = app.textFields["thread.composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 60))
        let routines = app.buttons["tab.routines"]
        let settings = app.buttons["tab.settings"]
        let threads = app.buttons["tab.threads"]
        XCTAssertTrue(routines.waitForExistence(timeout: 15)); routines.swipeLeft()
        XCTAssertTrue(settings.waitForExistence(timeout: 15)); XCTAssertTrue(settings.isHittable)
        XCTAssertTrue(app.frame.contains(settings.frame), "Settings tab must be fully inside the screen")
        settings.tap(); XCTAssertTrue(settings.isSelected)
        XCTAssertTrue(app.navigationBars["Settings"].waitForExistence(timeout: 20), "The Settings screen must open")
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "large-type-settings"; shot.lifetime = .keepAlways; add(shot)
        if !threads.isHittable { routines.swipeRight() }
        XCTAssertTrue(threads.waitForExistence(timeout: 10)); XCTAssertTrue(threads.isHittable)
        threads.tap(); XCTAssertTrue(threads.isSelected)
        XCTAssertTrue(composer.waitForExistence(timeout: 30), "Returning to Threads must restore its composer")
    }
    func testLeaveDedicatedProReady() {
        let app = XCUIApplication()
        app.terminate()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        closeRestoredViewer(app)
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        XCTAssertFalse(app.descendants(matching: .any)["qualification.connection.parked"].exists)
        XCTAssertFalse(app.textFields["pair.address"].exists)
        app.terminate()
        app.launchEnvironment.removeAll()
        app.launch()
        closeRestoredViewer(app)
        XCTAssertTrue(app.buttons["tab.threads"].waitForExistence(timeout: 45))
        XCTAssertFalse(app.textFields["pair.address"].exists)
    }

    func testInitialThreadLaunchObservations() {
        let app = XCUIApplication()
        var outcomes: [Bool] = []
        for (label, selection) in [("board", "t1790229700-1088"), ("integration", "t1793120668-655983")] {
            app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
            app.launchEnvironment["BACKPLANE_SELECT"] = selection
            app.launch()
            closeRestoredViewer(app)
            let ready = app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60)
            qualificationStatus(app, label: "launch-" + label)
            let status = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "qualification.connection.")).firstMatch
            let data = (status.exists ? (status.value as? String ?? "") : "").data(using: .utf8) ?? Data()
            let fields = (try? JSONSerialization.jsonObject(with: data)) as? [String: String]
            let matching = (fields?["thread"] ?? "").hasSuffix("|" + selection)
            print("safe-launch-state phase=\(label) foreground=\(app.state == .runningForeground) pairing-field=\(app.textFields["pair.address"].exists) ready=\(ready) matching-thread=\(matching) viewer-close=\(app.buttons["viewer.close"].exists)")
            if app.state == .runningForeground && !app.textFields["pair.address"].exists {
                let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "launch-" + label; shot.lifetime = .keepAlways; add(shot)
            }
            outcomes.append(ready && matching)
            app.terminate()
        }
        XCTAssertTrue(outcomes.allSatisfy { $0 }, "Both selected thread launches must reach fresh content")
    }

    func testBoardDraftOwnerAccessibilityDiagnostic() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        closeRestoredViewer(app)
        let composer = app.textFields["thread.composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 60))
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        qualificationStatus(app, label: "draft-precondition-owner")
        let value = composer.value as? String
        let placeholder = composer.placeholderValue
        print("safe-draft-state value-is-nil=\(value == nil) value-is-empty=\(value?.isEmpty == true) placeholder-present=\(placeholder != nil) value-is-placeholder=\(value != nil && value == placeholder)")
    }

    func testDraftOwnershipAcrossLiveRefresh() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        closeRestoredViewer(app)
        let composer = app.textFields["thread.composer"]
        let ready = app.descendants(matching: .any)["qualification.connection.ready"]
        let home = app.buttons["tab.home"]
        let threads = app.buttons["tab.threads"]
        func isEmptyForOwner(_ suffix: String) -> Bool {
            let status = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "qualification.connection.")).firstMatch
            guard status.exists, let raw = status.value as? String, let data = raw.data(using: .utf8),
                  let fields = (try? JSONSerialization.jsonObject(with: data)) as? [String: String],
                  let thread = fields["thread"], let nav = fields["nav"], let owner = fields["composerOwner"] else { return false }
            let matches = thread.hasSuffix("|" + suffix) && nav == thread && owner == thread
            let editable = fields["composerEditable"] == "true"
            let empty = fields["composerEmpty"] == "true"
            print("safe-draft-model owner-matches=\(matches) editable=\(editable) empty=\(empty)")
            return matches && editable && empty
        }
        func expectValue(_ expected: String) {
            let match = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", expected), object: composer)
            XCTAssertEqual(XCTWaiter.wait(for: [match], timeout: 30), .completed, "The visible owner's local draft must match")
        }
        func typeMarker(_ marker: String) {
            XCTAssertTrue(composer.isEnabled && composer.isHittable)
            composer.tap()
            XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 15))
            composer.typeText(marker)
            expectValue(marker)
        }
        XCTAssertTrue(composer.waitForExistence(timeout: 60)); XCTAssertTrue(ready.waitForExistence(timeout: 60))
        qualificationStatus(app, label: "board-draft-owner")
        let currentStatus = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "qualification.connection.")).firstMatch
        let statusData = (currentStatus.value as? String ?? "").data(using: .utf8) ?? Data()
        let ownerFields = (try? JSONSerialization.jsonObject(with: statusData)) as? [String: String]
        XCTAssertTrue((ownerFields?["thread"] ?? "").hasSuffix("|t1790229700-1088"), "The exact board thread must own the composer")
        XCTAssertTrue(isEmptyForOwner("t1790229700-1088"), "Refuse to replace an unknown or nonempty board-thread draft")
        let boardTitle = app.navigationBars.firstMatch.identifier
        XCTAssertFalse(boardTitle.isEmpty)
        XCTAssertTrue(home.waitForExistence(timeout: 10)); home.tap()
        XCTAssertTrue(ready.waitForExistence(timeout: 60))
        if app.navigationBars["Bub"].exists {
            let back = app.navigationBars.buttons.firstMatch
            XCTAssertTrue(back.waitForExistence(timeout: 10)); back.tap()
            XCTAssertTrue(ready.waitForExistence(timeout: 60))
        }
        let bub = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@ AND label CONTAINS[cd] %@", "home.uber.", "Bub")).firstMatch
        XCTAssertTrue(bub.waitForExistence(timeout: 30)); bub.tap()
        XCTAssertTrue(app.navigationBars["Bub"].waitForExistence(timeout: 30))
        XCTAssertTrue(app.descendants(matching: .any)["bot.screen.b1790292962-8017"].waitForExistence(timeout: 30))
        XCTAssertTrue(ready.waitForExistence(timeout: 60))
        let chat = app.buttons["bot.tab.chat"]
        XCTAssertTrue(chat.waitForExistence(timeout: 15)); chat.tap()
        XCTAssertTrue(chat.isSelected); XCTAssertTrue(ready.waitForExistence(timeout: 60))
        XCTAssertTrue(composer.waitForExistence(timeout: 30))
        XCTAssertTrue(isEmptyForOwner("t1790292962-8017"), "Refuse to replace an unknown or nonempty Bub draft")
        var bubMarker = "vcBubLocal655983"
        let boardMarker = "vcBoardLocal655983"
        typeMarker(bubMarker)
        let sawSyncing = app.descendants(matching: .any)["qualification.connection.connecting"].waitForExistence(timeout: 20)
        print("safe-draft observed-syncing=\(sawSyncing)")
        if sawSyncing {
            XCTAssertTrue(composer.isEnabled && composer.isHittable, "Local draft editor must remain usable during refresh")
            composer.typeText("x")
            bubMarker += "x"
            expectValue(bubMarker)
        }
        XCTAssertTrue(threads.waitForExistence(timeout: 10)); threads.tap()
        XCTAssertTrue(app.navigationBars[boardTitle].waitForExistence(timeout: 30))
        XCTAssertTrue(composer.waitForExistence(timeout: 30)); XCTAssertTrue(ready.waitForExistence(timeout: 60))
        XCTAssertTrue(isEmptyForOwner("t1790229700-1088"), "Bub's marker must not leak into the board thread")
        typeMarker(boardMarker)
        if !app.textFields["pair.address"].exists {
            let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "board-owned-local-draft"; shot.lifetime = .keepAlways; add(shot)
        }
        for _ in 0..<2 {
            home.tap(); XCTAssertTrue(app.navigationBars["Bub"].waitForExistence(timeout: 30))
            XCTAssertTrue(chat.isSelected); expectValue(bubMarker)
            threads.tap(); XCTAssertTrue(app.navigationBars[boardTitle].waitForExistence(timeout: 30))
            expectValue(boardMarker)
        }
        XCUIDevice.shared.press(.home); app.activate()
        XCTAssertTrue(app.navigationBars[boardTitle].waitForExistence(timeout: 30)); expectValue(boardMarker)
        composer.tap(); composer.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: boardMarker.count))
        XCTAssertTrue(isEmptyForOwner("t1790229700-1088"), "Board marker must be cleared locally")
        home.tap(); XCTAssertTrue(app.navigationBars["Bub"].waitForExistence(timeout: 30)); expectValue(bubMarker)
        composer.tap(); composer.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: bubMarker.count))
        XCTAssertTrue(isEmptyForOwner("t1790292962-8017"), "Bub marker must be cleared locally")
        XCTAssertTrue(sawSyncing, "The run must observe a refresh while the Bub draft is present")
    }

    func testRealThreadAndBubNavigation() {
        let app = XCUIApplication()
        addTeardownBlock {
            self.qualificationStatus(app, label: "navigation-final")
            if app.state == .runningForeground && !app.textFields["pair.address"].exists {
                let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "navigation-final"; shot.lifetime = .keepAlways; self.add(shot)
            }
        }
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1793120668-655983"
        app.launch()
        closeRestoredViewer(app)
        let composer = app.descendants(matching: .any)["thread.composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 60), "Real integration thread must open")
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        print("thread-composer frame=\(composer.frame) hittable=\(composer.isHittable)")
        XCTAssertTrue(composer.isHittable, "The real thread composer must be visible and hittable")
        qualificationStatus(app, label: "initial-thread")
        let threadTitle = app.navigationBars.firstMatch.identifier
        XCTAssertFalse(threadTitle.isEmpty, "The selected thread must have a navigation title")
        let homeTab = app.buttons["tab.home"]
        let threadTab = app.buttons["tab.threads"]
        XCTAssertTrue(homeTab.waitForExistence(timeout: 10)); homeTab.tap()
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        qualificationStatus(app, label: "home-after-switch")
        if app.navigationBars["Bub"].exists || app.buttons["bot.tab.chat"].exists {
            XCTAssertTrue(app.navigationBars["Bub"].exists, "Home must restore its existing Bub destination")
            let back = app.navigationBars.buttons.firstMatch
            XCTAssertTrue(back.waitForExistence(timeout: 10)); back.tap()
            XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
            qualificationStatus(app, label: "home-after-back")
        }
        let bub = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@ AND label CONTAINS[cd] %@", "home.uber.", "Bub")).firstMatch
        XCTAssertTrue(bub.waitForExistence(timeout: 30), "The real Home card must identify Bub")
        bub.tap()
        let bubTitle = app.navigationBars["Bub"]
        XCTAssertTrue(bubTitle.waitForExistence(timeout: 30), "The Bub conversation must open")
        qualificationStatus(app, label: "bub-after-open")
        let bubReady = app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60)
        qualificationStatus(app, label: "bub-ready-wait")
        XCTAssertTrue(bubReady, "Bub's actual presentation must finish before choosing Chat")
        let botScreen = app.descendants(matching: .any)["bot.screen.b1790292962-8017"]
        XCTAssertTrue(botScreen.waitForExistence(timeout: 15), "The real Bub bot screen must be displayed")
        let chat = app.buttons["bot.tab.chat"]
        XCTAssertTrue(chat.waitForExistence(timeout: 10), "Bub's actual Chat tab must appear")
        chat.tap()
        XCTAssertTrue(chat.isSelected, "Bub chat must be selected")
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60), "Bub chat must finish presenting")
        let hasComposer = composer.waitForExistence(timeout: 30)
        if hasComposer { print("bub-composer frame=\(composer.frame) hittable=\(composer.isHittable)") }
        XCTAssertTrue(hasComposer, "Bub's real chat composer must appear")
        XCTAssertTrue(composer.isHittable, "Bub's real chat composer must be visible and hittable")
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        let screenshot = XCTAttachment(screenshot: app.screenshot()); screenshot.name = "real-bub-chat"; screenshot.lifetime = .keepAlways; add(screenshot)
        let usable = XCTNSPredicateExpectation(predicate: NSPredicate(format: "enabled == YES AND hittable == YES"), object: composer)
        let usability = XCTWaiter.wait(for: [usable], timeout: 45)
        qualificationStatus(app, label: "bub-composer-usability")
        XCTAssertEqual(usability, .completed, "Bub composer must become enabled and hittable after a refresh")
        composer.tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 10), "Tapping the existing composer must open the keyboard")
        XCTAssertTrue(composer.isHittable, "The composer must remain visible with the keyboard open")
        let keyboardShot = XCTAttachment(screenshot: app.screenshot()); keyboardShot.name = "bub-keyboard"; keyboardShot.lifetime = .keepAlways; add(keyboardShot)
        for _ in 0..<2 {
            XCTAssertTrue(threadTab.waitForExistence(timeout: 10)); threadTab.tap()
            XCTAssertTrue(threadTab.isSelected, "Threads must be selected")
            XCTAssertFalse(bubTitle.exists, "Bub must no longer be the visible screen")
            XCTAssertTrue(composer.waitForExistence(timeout: 30), "The thread stack must return to its composer")
            XCTAssertTrue(app.navigationBars[threadTitle].waitForExistence(timeout: 20), "The same real thread must be restored")
            XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
            qualificationStatus(app, label: "thread-restored")
            XCTAssertTrue(homeTab.waitForExistence(timeout: 10)); homeTab.tap()
            XCTAssertTrue(bubTitle.waitForExistence(timeout: 30), "Returning to Home must restore Bub's stack")
            XCTAssertTrue(botScreen.waitForExistence(timeout: 15))
            XCTAssertTrue(chat.isSelected)
            XCTAssertTrue(composer.waitForExistence(timeout: 30))
            XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
            qualificationStatus(app, label: "bub-restored")
        }
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(bubTitle.waitForExistence(timeout: 30), "Foreground must restore Bub")
        XCTAssertTrue(composer.waitForExistence(timeout: 30))
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
        qualificationStatus(app, label: "bub-after-reopen")
        let back = app.navigationBars.buttons.firstMatch
        XCTAssertTrue(back.waitForExistence(timeout: 10)); back.tap()
        XCTAssertTrue(bub.waitForExistence(timeout: 20), "Back must return to the Bub card")
        bub.tap()
        XCTAssertTrue(bubTitle.waitForExistence(timeout: 30), "The existing Bub must reopen")
        XCTAssertTrue(composer.waitForExistence(timeout: 30))
        XCTAssertTrue(app.descendants(matching: .any)["qualification.connection.ready"].waitForExistence(timeout: 60))
    }

}
