import SwiftUI

// The resolved Bend tag from Screen.locale. No native locale policy.
private struct BackplaneLanguageKey: EnvironmentKey {
    static let defaultValue = "en"
}

extension EnvironmentValues {
    var backplaneLanguage: String {
        get { self[BackplaneLanguageKey.self] }
        set { self[BackplaneLanguageKey.self] = newValue }
    }
}
