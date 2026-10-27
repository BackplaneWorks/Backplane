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
                .overlay(alignment: .topLeading) {
                    #if DEBUG && targetEnvironment(simulator)
                    if ProcessInfo.processInfo.environment["BACKPLANE_QUALIFICATION"] == "1" {
                        Text("Simulator connection: " + model.qualificationConnection)
                            .font(.caption2.monospaced())
                            .padding(3)
                            .background(.ultraThinMaterial)
                            .allowsHitTesting(false)
                            .accessibilityIdentifier("qualification.connection." + model.qualificationConnection)
                    }
                    #endif
                }
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
