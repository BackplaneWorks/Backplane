import SwiftUI
import UIKit
import UserNotifications

// The app's side of phone push (native/host/src/push). The hub decides everything: what is sent, when, the badge and what clears. This takes a token, opens what is tapped,
// and takes away what the hub says was dealt with.
#if DATA_APP
final class PushDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    func application(_ app: UIApplication, didFinishLaunchingWithOptions o: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        UNUserNotificationCenter.current().delegate = self
        return true
    }
    func application(_ app: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken token: Data) {
        let hex = token.map { String(format: "%02x", $0) }.joined()
        MainActor.assumeIsolated { DataClient.shared.tokenGot(hex) }
    }
    func application(_ app: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) { NSLog("push: %@", error.localizedDescription) }
    // the silent clear
    func application(_ app: UIApplication, didReceiveRemoteNotification info: [AnyHashable: Any]) async -> UIBackgroundFetchResult {
        guard let keys = info["clear"] as? [String] else { return .noData }
        let badge = (info["aps"] as? [String: Any])?["badge"] as? Int
        await DataClient.shared.cleared(keys, badge: badge)
        return .newData
    }
    // in front: quiet for the thread on screen. Completion-handler forms, finished on the main thread: UIKit asserts that its completion runs there, and the async forms
    // run off it (a tapped alert crashed the app with "Call must be made on main thread")
    nonisolated func userNotificationCenter(_ c: UNUserNotificationCenter, willPresent n: UNNotification, withCompletionHandler done: @escaping (UNNotificationPresentationOptions) -> Void) {
        let thread = n.request.content.userInfo["thread"] as? String ?? ""
        DispatchQueue.main.async { MainActor.assumeIsolated { done(DataClient.shared.viewing == thread ? [] : [.banner, .list]) } }
    }
    nonisolated func userNotificationCenter(_ c: UNUserNotificationCenter, didReceive r: UNNotificationResponse, withCompletionHandler done: @escaping () -> Void) {
        let i = r.notification.request.content.userInfo
        let (thread, ask) = (i["thread"] as? String ?? "", i["ask"] as? String ?? "")
        DispatchQueue.main.async { MainActor.assumeIsolated { DataClient.shared.openPush(thread: thread, ask: ask); done() } }
    }
}

// the hub's quiet hours; they hold for every phone
struct DataQuietSection: View {
    @Bindable var client: DataClient
    private var on: Bool { client.quiet != "off" && !client.quiet.isEmpty }
    private func parts() -> (Date, Date) {
        let f = client.quiet.split(separator: "-").map(String.init)
        return (Self.date(f.first ?? "22:00"), Self.date(f.count > 1 ? f[1] : "07:00"))
    }
    private static func date(_ s: String) -> Date {
        let p = s.split(separator: ":").compactMap { Int($0) }
        return Calendar.current.date(bySettingHour: p.first ?? 0, minute: p.count > 1 ? p[1] : 0, second: 0, of: Date()) ?? Date()
    }
    private static func text(_ d: Date) -> String { let c = Calendar.current.dateComponents([.hour, .minute], from: d); return String(format: "%02d:%02d", c.hour ?? 0, c.minute ?? 0) }
    private func set(_ a: Date, _ b: Date) { Task { await client.setQuiet(Self.text(a) + "-" + Self.text(b)) } }
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            DHeading(text: "Quiet hours")
            DChoices(options: [(label: "on", value: true), (label: "off", value: false)], value: Binding(get: { on }, set: { v in Task { await client.setQuiet(v ? "22:00-07:00" : "off") } })).accessibilityIdentifier("settings.quiet")
            if on {
                DatePicker("From", selection: Binding(get: { parts().0 }, set: { set($0, parts().1) }), displayedComponents: .hourAndMinute).dmono(13)
                DatePicker("To", selection: Binding(get: { parts().1 }, set: { set(parts().0, $0) }), displayedComponents: .hourAndMinute).dmono(13)
            }
            DHeading(text: "Urgent asks come through")
            DChoices(options: [(label: "on", value: true), (label: "off", value: false)], value: Binding(get: { client.quietUrgent }, set: { v in Task { await client.setQuietUrgent(v) } })).accessibilityIdentifier("settings.urgent")
            Text("In your time zone, on every phone. Held alerts arrive as one when quiet hours end.").dmono(11).foregroundStyle(BackplaneDesign.dim)
        }
    }
}
#endif
