import SwiftUI
import UIKit
import UserNotifications

// The tabs of the app, in the order of the bar.
enum AppTab: String, CaseIterable, Identifiable {
    case home, threads, bots, routines, settings
    var id: String { rawValue }

    var title: String { title("en") }

    func title(_ tag: String) -> String {
        switch self {
        case .home: NativeMessages.native_tab_home(tag)
        case .threads: NativeMessages.tour_title_threads(tag)
        case .bots: NativeMessages.tour_title_bots(tag)
        case .routines: NativeMessages.bots_tab_routines(tag)
        case .settings: NativeMessages.palette_settings(tag)
        }
    }

    var icon: String {
        switch self {
        case .home: "house"
        case .threads: "bubble.left.and.bubble.right"
        case .bots: "pawprint"
        case .routines: "clock.arrow.circlepath"
        case .settings: "gearshape"
        }
    }
}

// Settings about this device: kept in UserDefaults, applied at once, never a
// round trip to a hub. The thread list's view also goes to Bend
// (Backplane.local), which draws the list; nothing here is written to a hub.
@MainActor
@Observable
final class AppSettings {
    static let shared = AppSettings()
    @ObservationIgnored private let d = UserDefaults.standard

    // "system", "light" or "dark"
    var appearance: String { didSet { d.set(appearance, forKey: "app.appearance") } }
    var lang: String { didSet { d.set(lang, forKey: "app.lang") } }
    @ObservationIgnored let langPreview: Bool
    // "projects" or "active"; empty until the hub's choice was carried over
    var threadList: String { didSet { d.set(threadList, forKey: "app.threadList") } }
    // a tab's raw value, or empty: Home with an uberbot, else Threads
    var defaultTab: String { didSet { d.set(defaultTab, forKey: "app.defaultTab") } }
    var haptics: Bool { didSet { d.set(haptics, forKey: "app.haptics") } }
    @ObservationIgnored var migrated: Bool {
        get { d.bool(forKey: "app.migrated.v1") }
        set { d.set(newValue, forKey: "app.migrated.v1") }
    }

    private init() {
        let forced = d.string(forKey: "BackplaneLang")
        langPreview = forced == "ja" || forced == "en"
        lang = langPreview ? forced! : (d.string(forKey: "app.lang") ?? "auto")
        appearance = d.string(forKey: "app.appearance") ?? "system"
        threadList = d.string(forKey: "app.threadList") ?? ""
        defaultTab = d.string(forKey: "app.defaultTab") ?? ""
        haptics = d.object(forKey: "app.haptics") as? Bool ?? true
    }

    var colorScheme: ColorScheme? {
        appearance == "light" ? .light : appearance == "dark" ? .dark : nil
    }

    // The first screen of this build: what the hub had chosen (its theme, its
    // thread list) becomes this device's choice, once. Never written back.
    func migrate(theme: String?, view: String?) {
        guard !migrated else { return }
        migrated = true
        if theme == "light" || theme == "dark" { appearance = theme! }
        if view == "projects" || view == "active" { threadList = view! }
    }
}

// A tap's feedback, unless it is switched off (read straight from the
// defaults: the plot view calls it off the main actor)
enum Haptics {
    static func impact(_ style: UIImpactFeedbackGenerator.FeedbackStyle = .medium) {
        guard UserDefaults.standard.object(forKey: "app.haptics") as? Bool ?? true else { return }
        UIImpactFeedbackGenerator(style: style).impactOccurred()
    }

    static func success() {
        guard UserDefaults.standard.object(forKey: "app.haptics") as? Bool ?? true else { return }
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }
}
