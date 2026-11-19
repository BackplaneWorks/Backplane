import XCTest

// The review bar on the iOS simulator (#1305), through real taps, against a temp-home hub (test/e2e/ios/review-bar.sh): a thread in review mode
// shows the read-only bar with why; Allow edits switches it off through `thread.review` (the bar then offers Read-only again); that switches it back on.
final class ReviewBarE2E: XCTestCase {
    var d: Driver!
    override func setUp() { continueAfterFailure = true }

    func test_review_bar() {
        d = Driver("review-bar"); defer { d.finish() }
        d.app.terminate()
        d.app.launchArguments = ["-app.alerts", "<false/>"]
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": "trev"]
        d.app.launch(); d.skipTour()
        let bar = d.el("thread.review"), text = d.el("thread.review.text"), button = d.el("thread.review.button")
        _ = d.step("the read-only bar says why, with Allow edits", timeout: 40, act: {}, expect: {
            bar.exists && text.label.hasPrefix("Read-only: this request looked like a review") && button.label == "Allow edits"
        })
        sleep(1); d.shot("read-only")
        _ = d.step("Allow edits switches review mode off: Read-only again", timeout: 15, act: { button.tap() }, expect: {
            button.label == "Read-only again" && text.label.hasPrefix("Edits allowed")
        })
        sleep(1); d.shot("edits-allowed")
        _ = d.step("Read-only again switches it back on", timeout: 15, act: { button.tap() }, expect: { button.label == "Allow edits" })
        sleep(1); d.shot("read-only-again")
    }
}
