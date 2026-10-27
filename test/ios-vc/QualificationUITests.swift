import XCTest

// Runs against a dedicated simulator only. Never launch with BACKPLANE_LINK:
// the initial pairing must use the normal UI and survive a subsequent launch.
final class QualificationUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    func testFreshSimulatorShowsPairing() {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.textFields["pair.address"].waitForExistence(timeout: 20))
        let address = app.textFields["pair.address"]
        let connect = app.buttons["pair.connect"]
        XCTAssertTrue(connect.exists)
        address.tap()
        address.typeText("invalid")
        connect.tap()
        XCTAssertTrue(app.staticTexts["pair.address.error"].waitForExistence(timeout: 5))
    }

    // Run after pairing through the app UI. No credential is passed to XCTest.
    func testPersistedPairingAndNavigation() {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.tabBars.firstMatch.waitForExistence(timeout: 45), "A paired hub should reach the app")
        XCTAssertFalse(app.textFields["pair.address"].exists, "Pairing must persist across launches")
        let threads = app.buttons["tab.threads"]
        if threads.waitForExistence(timeout: 5) { threads.tap() }
        let rows = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "thread.row."))
        XCTAssertTrue(rows.firstMatch.waitForExistence(timeout: 30), "Real hub thread rows must load")
        rows.firstMatch.tap()
        XCTAssertTrue(app.textViews["thread.composer"].waitForExistence(timeout: 20) || app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 5))
        app.terminate()
        app.launch()
        XCTAssertTrue(app.tabBars.firstMatch.waitForExistence(timeout: 45), "Warm launch must reconnect without a pairing override")
        XCTAssertFalse(app.textFields["pair.address"].exists)
    }
    private func tab(_ name: String, label: String, in app: XCUIApplication) -> XCUIElement {
        let identified = app.buttons["tab." + name]
        if identified.waitForExistence(timeout: 3) { return identified }
        return app.tabBars.buttons[label]
    }

    func testRepeatedNavigationViewerAndForeground() {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.tabBars.firstMatch.waitForExistence(timeout: 45))
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
        // The selected real thread must belong to a project with a saved board.
        let open = app.buttons["viewer.open"]
        XCTAssertTrue(open.waitForExistence(timeout: 15), "Select a real board project thread before running this test")
        for _ in 0..<2 {
            open.tap()
            let board = app.buttons["viewer.choice.board"]
            XCTAssertTrue(board.waitForExistence(timeout: 10)); board.tap()
            XCTAssertTrue(app.descendants(matching: .any)["viewer.ready"].waitForExistence(timeout: 60), "Board geometry should arrive")
            let close = app.buttons["viewer.close"]
            XCTAssertTrue(close.waitForExistence(timeout: 10)); close.tap()
            XCTAssertTrue(app.descendants(matching: .any)["thread.composer"].waitForExistence(timeout: 15))
        }
    }

}
