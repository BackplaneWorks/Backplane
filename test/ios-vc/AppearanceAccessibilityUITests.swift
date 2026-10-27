import XCTest

// Use only on a dedicated, already paired simulator. No typing, send, pairing,
// screenshots, or accessibility-tree logging; the runner controls presets.
final class AppearanceAccessibilityUITests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    private func launchPaired() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertTrue(app.buttons["tab.threads"].waitForExistence(timeout: 60), "Requires a paired compact-width simulator")
        XCTAssertFalse(app.textFields["pair.address"].exists, "Never capture pairing UI in this suite")
        return app
    }

    private func reveal(_ button: XCUIElement, in app: XCUIApplication) {
        if !button.isHittable { app.swipeLeft() }
        if !button.isHittable { app.swipeRight() }
    }

    func testCompactTabLabelsAndSelection() {
        let app = launchPaired()
        for name in ["home", "threads", "bots", "routines", "settings"] {
            let button = app.buttons["tab." + name]
            XCTAssertTrue(button.exists, "Missing tab \(name)")
            reveal(button, in: app)
            XCTAssertTrue(button.isHittable, "Tab \(name) must be reachable at this text size")
            XCTAssertFalse(button.label.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, "Tab \(name) needs a spoken label")
            button.tap()
            let selected = XCTNSPredicateExpectation(predicate: NSPredicate(format: "selected == YES"), object: button)
            XCTAssertEqual(XCTWaiter.wait(for: [selected], timeout: 5), .completed, "Tab \(name) must expose its selected state")
        }
    }

    func testProjectSearchKeyboardClearanceWithoutEditing() {
        let app = launchPaired()
        let threads = app.buttons["tab.threads"]
        reveal(threads, in: app)
        XCTAssertTrue(threads.isHittable)
        threads.tap()
        let search = app.textFields["projects.search"]
        XCTAssertTrue(search.waitForExistence(timeout: 30))
        XCTAssertTrue(search.isHittable)
        search.tap()
        let keyboard = app.keyboards.firstMatch
        XCTAssertTrue(keyboard.waitForExistence(timeout: 10))
        XCTAssertTrue(search.isHittable, "Search must remain reachable with the keyboard up")
        XCTAssertLessThanOrEqual(search.frame.maxY, keyboard.frame.minY + 1, "Search must clear the keyboard")
        XCTAssertGreaterThanOrEqual(search.frame.minY, app.frame.minY, "Search must remain within the app viewport")
    }

    @available(iOS 17.0, *)
    func testCurrentScreenAccessibilityAudit() throws {
        let app = launchPaired()
        // XCTest's automated checks do not substitute for VoiceOver navigation.
        try app.performAccessibilityAudit()
    }
}
