import Photos
import SwiftUI

// A picture in a thread (one the person sent, one a reply names, a Markdown image) can be kept: a long press offers Save to Photos and
// Share image, and its full view has both as buttons. Saving asks only to add to the library (NSPhotoLibraryAddUsageDescription), never to read it.

enum DImageSave {
    /// Saves the picture's own bytes when they are a picture (a PNG stays a PNG), else a PNG of what is drawn. Returns the note to show.
    @MainActor static func save(_ image: UIImage, data: Data?) async -> String {
        let status = await PHPhotoLibrary.requestAuthorization(for: .addOnly)
        guard status == .authorized || status == .limited else { return "Photos access is off: allow it in Settings to save pictures." }
        guard let bytes = data.flatMap({ UIImage(data: $0) != nil ? $0 : nil }) ?? image.pngData() else { return "Could not save the picture." }
        do {
            try await PHPhotoLibrary.shared().performChanges { PHAssetCreationRequest.forAsset().addResource(with: .photo, data: bytes, options: nil) }
            return "Saved to Photos"
        } catch { return "Could not save the picture." }
    }
}

/// Save to Photos and Share image, for a context menu or the full view's bar.
struct DImageActions: View {
    let image: UIImage
    var data: Data? = nil
    /// where the note goes: the thread's note over the screen unless the full view shows its own
    var note: (String) -> Void = { DataClient.shared.say($0) }
    var body: some View {
        Button(Tr.t("Save to Photos"), systemImage: "square.and.arrow.down") { Task { note(await DImageSave.save(image, data: data)) } }
            .accessibilityIdentifier("image.save")
        ShareLink(item: Image(uiImage: image), preview: SharePreview(Tr.t("Share image"), image: Image(uiImage: image))) {
            Label(Tr.t("Share image"), systemImage: "square.and.arrow.up")
        }.accessibilityIdentifier("image.share")
    }
}

extension View {
    /// a long press on a picture in a thread: Save to Photos, Share image
    func dImageMenu(_ image: UIImage?, data: Data? = nil) -> some View {
        contextMenu { if let image { DImageActions(image: image, data: data) } }
    }
}

/// A picture at full size over everything: pinch to zoom, Close, Save to Photos and Share image.
struct DImageFull: View {
    let image: UIImage
    var data: Data? = nil
    let close: () -> Void
    @State private var note = ""
    @State private var zoom: CGFloat = 1
    @GestureState private var pinch: CGFloat = 1
    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            Image(uiImage: image).resizable().scaledToFit().scaleEffect(max(1, zoom * pinch))
                .gesture(MagnificationGesture().updating($pinch) { v, s, _ in s = v }.onEnded { zoom = min(6, max(1, zoom * $0)) })
                .onTapGesture(count: 2) { zoom = zoom > 1 ? 1 : 2.5 }
                .dImageMenu(image, data: data)
                .accessibilityIdentifier("image.full")
        }
        .overlay(alignment: .topTrailing) {
            Button(action: close) { Image(systemName: "xmark.circle.fill").font(.title).foregroundStyle(.white) }.padding()
                .accessibilityLabel(Tr.t("Close")).accessibilityIdentifier("image.full.close")
        }
        .overlay(alignment: .bottom) {
            VStack(spacing: 10) {
                if !note.isEmpty { Text(Tr.t(note)).dmono(12).foregroundStyle(.white).padding(.horizontal, 12).padding(.vertical, 8).background(.black.opacity(0.7)).accessibilityIdentifier("image.full.note") }
                HStack(spacing: 28) {
                    DImageActions(image: image, data: data, note: { say($0) })
                }.labelStyle(.iconOnly).font(.title2).foregroundStyle(.white).padding(.bottom, 8)
            }
        }
    }
    private func say(_ s: String) {
        note = s
        Task { try? await Task.sleep(for: .seconds(3)); if note == s { note = "" } }
    }
}
