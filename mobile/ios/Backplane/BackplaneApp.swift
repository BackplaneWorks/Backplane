import SwiftUI
import UIKit

final class AppDelegate: NSObject, UIApplicationDelegate {
    var model: AppModel?

    func application(_ app: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken token: Data) {
        MainActor.assumeIsolated { model?.registered(token) }
    }

    func application(_ app: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        NSLog("push: %@", error.localizedDescription)
    }
}

@main
struct BackplaneApp: App {
    @UIApplicationDelegateAdaptor private var delegate: AppDelegate
    @Environment(\.scenePhase) private var phase
    @State private var model = AppModel()
    @State private var grace: UIBackgroundTaskIdentifier = .invalid

    var body: some Scene {
        WindowGroup {
            RootView(model: model)
                // this device's appearance (App settings); the hub's theme is not read
                .preferredColorScheme(model.settings.colorScheme)
                // Canonical references and legacy notification links share message-tap routing.
                .onOpenURL { model.open($0) }
                .onAppear { delegate.model = model }
        }
        .onChange(of: phase) {
            model.foreground(phase == .active)
            // a little time after leaving, so a turn ending now still alerts
            if phase == .background, grace == .invalid {
                grace = UIApplication.shared.beginBackgroundTask {
                    UIApplication.shared.endBackgroundTask(grace)
                    grace = .invalid
                }
            } else if phase == .active, grace != .invalid {
                UIApplication.shared.endBackgroundTask(grace)
                grace = .invalid
            }
        }
    }
}
