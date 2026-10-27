import XCTest
final class Normal74UITests: XCTestCase {
 func testNormalLaunchAndForeground() {
  let app=XCUIApplication(); app.activate()
  let home=app.buttons["tab.home"], threads=app.buttons["tab.threads"]
  XCTAssertTrue(home.waitForExistence(timeout: 30) && threads.exists)
  let bot=app.descendants(matching: .any)["bot.screen.b1790292962-8017"]
  let composer=app.descendants(matching: .any)["thread.composer"]
  let pair=app.textFields["pair.address"]
  print("safe-normal-initial foreground=\(app.state == .runningForeground) paired-ui=\(!pair.exists) bub=\(bot.exists) composer=\(composer.exists) home=\(home.exists) threads=\(threads.exists)")
  XCTAssertTrue(app.state == .runningForeground && !pair.exists)
  XCUIDevice.shared.press(.home); app.activate()
  XCTAssertTrue(home.waitForExistence(timeout: 20))
  print("safe-normal-foreground foreground=\(app.state == .runningForeground) paired-ui=\(!pair.exists) bub=\(bot.exists) composer=\(composer.exists)")
  XCTAssertTrue(app.state == .runningForeground && !pair.exists)
 }
}
