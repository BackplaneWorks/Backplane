import UIKit
import UserNotifications

// Turn-end alerts. While the app is in front it posts the alerts Bend
// raises (and, on the web screen, those the page is sent); while it sleeps
// the hub's APNs pushes carry them. A remote one arriving in front is
// dropped, since the local one already showed; on the web screen it shows
// unless the same alert (its key) showed in the last 20 s.
@MainActor
final class Notifier: NSObject, UNUserNotificationCenterDelegate {
    var open: (String) -> Void = { _ in }
    var viewing: (String) -> Bool = { _ in false }
    var remoteInFront: () -> Bool = { false }
    private var recent: [String: Date] = [:]

    // an alert with this key showed lately; else it is marked as showing now
    private func again(_ key: String) -> Bool {
        let now = Date()
        recent = recent.filter { now.timeIntervalSince($0.value) < 20 }
        if recent[key] != nil { return true }
        recent[key] = now
        return false
    }

    private static func key(thread: String, key: String) -> String {
        key.isEmpty ? "turn-" + WebShell.local(thread) : key
    }

    func setUp() {
        let c = UNUserNotificationCenter.current()
        c.delegate = self
        // Pairing and cold launch should reach content before an optional OS
        // permission dialog. Existing permission still registers this launch.
        Task {
            let status = await c.notificationSettings().authorizationStatus
            if status == .authorized || status == .provisional || status == .ephemeral {
                UIApplication.shared.registerForRemoteNotifications()
            }
        }
    }

    func enable() async {
        let c = UNUserNotificationCenter.current()
        c.delegate = self
        if (try? await c.requestAuthorization(options: [.alert, .sound, .badge])) == true {
            UIApplication.shared.registerForRemoteNotifications()
        }
    }

    // one notification per key ("turn-<thread>", or "room-<room>" for a room
    // exchange, the same on every paired hub): a newer alert replaces the last
    func post(thread: String, key: String, title: String, body: String) {
        // this phone's own switch (Settings > On this phone); the hub's alert policy is separate
        guard UserDefaults.standard.object(forKey: "app.alerts") as? Bool ?? true else { return }
        let n = UNMutableNotificationContent()
        n.title = title
        n.body = body
        if UserDefaults.standard.object(forKey: "app.alertSound") as? Bool ?? true { n.sound = .default }
        let id = key.isEmpty ? "turn-" + thread : key
        if remoteInFront(), again(Self.key(thread: thread, key: key)) { return }
        n.threadIdentifier = id
        n.userInfo = ["thread": thread]
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: id, content: n, trigger: nil))
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent n: UNNotification) async -> UNNotificationPresentationOptions {
        let thread = n.request.content.userInfo["thread"] as? String ?? ""
        let key = n.request.content.userInfo["key"] as? String ?? ""
        let remote = n.request.trigger is UNPushNotificationTrigger
        let alerts = UserDefaults.standard.object(forKey: "app.alerts") as? Bool ?? true
        let sound = UserDefaults.standard.object(forKey: "app.alertSound") as? Bool ?? true
        let shown: UNNotificationPresentationOptions = sound ? [.banner, .sound, .list] : [.banner, .list]
        return await MainActor.run {
            if !alerts || viewing(thread) { return [] }
            if remote { return remoteInFront() && !again(Self.key(thread: thread, key: key)) ? shown : [] }
            return shown
        }
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive r: UNNotificationResponse) async {
        let thread = r.notification.request.content.userInfo["thread"] as? String ?? ""
        await MainActor.run { if !thread.isEmpty { open(thread) } }
    }
}
