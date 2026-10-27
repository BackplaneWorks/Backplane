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
        XCTAssertTrue(app.buttons["pair.connect"].exists)
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
}
