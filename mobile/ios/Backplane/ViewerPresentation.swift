import SwiftUI

private struct ViewerHubKey: EnvironmentKey { static let defaultValue = "" }

private struct ViewerDismissKey: EnvironmentKey {
    static let defaultValue: @MainActor () -> Void = {}
}
extension EnvironmentValues {
    var backplaneViewerHub: String {
        get { self[ViewerHubKey.self] }
        set { self[ViewerHubKey.self] = newValue }
    }
    var dismissBackplaneViewer: @MainActor () -> Void {
        get { self[ViewerDismissKey.self] }
        set { self[ViewerDismissKey.self] = newValue }
    }
}

// The dismissal callback and the controls carry the presentation they belong
// to. Reopening waits for the prior cover's actual dismissal completion.
struct ViewerPresentationContent: View {
    let model: AppModel
    let thread: String
    @ViewBuilder var body: some View {
        if let token = model.visibleViewerToken(for: thread), let v = model.viewerData(for: thread) {
            Group {
                if v.open == "files", let f = v.files { FilesScreen(model: model, viewer: v, page: f) }
                else if v.open == "image" || v.open == "pdf" || v.open == "text", let f = v.file { FileScreen(model: model, viewer: v, file: f) }
                else if v.open == "review", let page = v.review { ReviewScreen(model: model, viewer: v, page: page) }
                else if v.open == "renders", let page = v.mech { MechScreen(model: model, viewer: v, page: page) }
                else { PlotScreen(model: model, viewer: v, presentation: token) }
            }
            .interactiveDismissDisabled()
            .accessibilityAction(.escape) { model.closeViewer(token) }
            .environment(\.isEnabled, true)
            .environment(\.backplaneViewerHub, token.owner.hub)
            .environment(\.dismissBackplaneViewer, { model.closeViewer(token) })
        }
    }
}

struct ViewerConnectionStatus: View {
    @Environment(\.backplaneViewerHub) private var hub
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    var body: some View {
        let phase = model.conn[hub]?.phase ?? .offline
        if phase != .online || !model.contentReady {
            let shown = phase == .online ? HubConn.Phase.syncing : phase
            Text(HubDot.word(shown, lang)).font(.caption).foregroundStyle(.secondary)
                .accessibilityIdentifier("viewer.connection." + String(describing: shown))
        }
    }
}

// This modifier sits outside the disabled stale conversation subtree.
struct ViewerPresentationHost: ViewModifier {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let thread: String
    @State private var viewerPresented = false
    @State private var viewerDismissing = false
    @State private var presentedViewer: ViewerCloseIntent.Token?
    func body(content: Content) -> some View {
        let dismissalToken = presentedViewer
        let choices = model.viewerChoices(for: thread)
        return content
        .toolbar {
            if !choices.isEmpty {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        ForEach(choices, id: \.value) { choice in
                            Button(choice.label) { model.openViewer(choice.value, for: thread) }
                                .accessibilityIdentifier("viewer.choice." + choice.value)
                        }
                    } label: { Image(systemName: "cpu") }
                    .accessibilityLabel(NativeMessages.native_thread_board(lang))
                    .accessibilityIdentifier("viewer.open")
                }
            }
        }
        .onChange(of: model.visibleViewerToken(for: thread), initial: true) { _, token in
            if let token {
                presentedViewer = token
                if !viewerDismissing { viewerPresented = true }
            } else if viewerPresented {
                viewerDismissing = true
                viewerPresented = false
            }
        }
        .fullScreenCover(isPresented: Binding(get: { viewerPresented }, set: { up in
            guard !up, presentedViewer == dismissalToken else { return }
            if let dismissalToken { model.closeViewer(dismissalToken) }
            viewerDismissing = true
            viewerPresented = false
        }), onDismiss: {
            viewerDismissing = false
            if let token = model.visibleViewerToken(for: thread) {
                presentedViewer = token
                viewerPresented = true
            } else { presentedViewer = nil }
        }) { ViewerPresentationContent(model: model, thread: thread) }
    }
}
