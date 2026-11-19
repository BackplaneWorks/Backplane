import SwiftUI
import UIKit
import ImageIO

// A reply's pictures (feature replyImages): read through the hub (`message.image`, the snapshot the desktop's /message-image serves), drawn
// where ReplyImages puts them: one picture inline, several as a carousel; a tap opens them all full screen at the one tapped.

struct DReplyPic: Identifiable, Hashable { let entry: String; let path: String; var id: String { entry + "\n" + path } }

/// The viewer asked for: a reply's pictures, from the one tapped.
struct DReplyOpen: Identifiable { let entry: String; let all: [String]; let start: Int; var id: String { entry + "\n\(start)" } }

/// Pictures read through the hub, decoded: small copies for the thread, whole ones for the viewer.
@MainActor enum DReplyStore {
    static let thumbs = NSCache<NSString, UIImage>()
    static let fulls: NSCache<NSString, UIImage> = { let c = NSCache<NSString, UIImage>(); c.countLimit = 6; return c }()
    /// The longest side a thread's copy is decoded at (points x screen scale are well under it).
    static let thumbSide: CGFloat = 1400
    static func bytes(_ pic: DReplyPic) async -> Data? {
        #if DEBUG
        if let dir = ProcessInfo.processInfo.environment["BACKPLANE_MDPREVIEW_PICS"] {
            try? await Task.sleep(for: .milliseconds(Int(ProcessInfo.processInfo.environment["BACKPLANE_MDPREVIEW_DELAY"] ?? "") ?? 0))
            return try? Data(contentsOf: URL(fileURLWithPath: dir).appendingPathComponent(ReplyImages.name(pic.path)))
        }
        #endif
        return await DataClient.shared.replyImage(entry: pic.entry, path: pic.path)
    }
    static func thumb(_ pic: DReplyPic) async -> UIImage? {
        if let i = thumbs.object(forKey: pic.id as NSString) { return i }
        guard let d = await bytes(pic) else { return nil }
        let side = thumbSide
        guard let i = await Task.detached(priority: .userInitiated, operation: { downsample(d, side: side) }).value else { return nil }
        thumbs.setObject(i, forKey: pic.id as NSString); return i
    }
    static func full(_ pic: DReplyPic) async -> (UIImage, Data)? {
        guard let d = await bytes(pic) else { return nil }
        if let i = fulls.object(forKey: pic.id as NSString) { return (i, d) }
        guard let i = await Task.detached(priority: .userInitiated, operation: { UIImage(data: d)?.preparingForDisplay() }).value else { return nil }
        fulls.setObject(i, forKey: pic.id as NSString); return (i, d)
    }
    nonisolated static func downsample(_ d: Data, side: CGFloat) -> UIImage? {
        guard let src = CGImageSourceCreateWithData(d as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true,
                                                                  kCGImageSourceShouldCacheImmediately: true, kCGImageSourceThumbnailMaxPixelSize: side] as CFDictionary)
        else { return UIImage(data: d) }
        return UIImage(cgImage: cg)
    }
}

/// One slot of a reply: a single picture inline, or a carousel of several.
struct DReplySlot: View {
    let entry: String
    let paths: [String]
    let open: (String) -> Void
    var body: some View {
        if paths.count == 1 { DReplyTile(pic: DReplyPic(entry: entry, path: paths[0]), carousel: false) { open(paths[0]) } }
        else if !paths.isEmpty { DReplyCarousel(entry: entry, paths: paths, open: open) }
    }
}

/// A picture in the thread: a placeholder while it loads, the picture, or a tile that says it is unavailable (with its file name).
struct DReplyTile: View {
    let pic: DReplyPic
    let carousel: Bool
    let tap: () -> Void
    @State private var image: UIImage?
    @State private var failed = false
    private var name: String { ReplyImages.name(pic.path) }
    var body: some View {
        Group {
            if let image {
                if carousel {
                    Image(uiImage: image).resizable().scaledToFit().frame(maxWidth: .infinity, maxHeight: .infinity).background(BackplaneDesign.panel).dbox()
                } else {
                    Image(uiImage: image).resizable().scaledToFit().frame(maxWidth: min(image.size.width, 520), maxHeight: 360).dbox()
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            } else {
                VStack(spacing: 8) {
                    if failed {
                        Image(systemName: "photo.badge.exclamationmark").font(.title2).foregroundStyle(BackplaneDesign.dim)
                        Text(Tr.t("Image unavailable")).dmono(12).foregroundStyle(BackplaneDesign.text)
                    } else { ProgressView() }
                    Text(name).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1).truncationMode(.middle)
                }
                .padding(12).frame(maxWidth: .infinity, minHeight: carousel ? nil : (failed ? 96 : 160), maxHeight: carousel ? .infinity : nil)
                .background(BackplaneDesign.panel).dbox()
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { tap() }
        .dImageMenu(image)
        .task(id: pic) {
            failed = false
            if let i = await DReplyStore.thumb(pic) { image = i } else { failed = true }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(name)
        .accessibilityAddTraits(.isButton)
        .accessibilityIdentifier(image != nil ? "reply.image" : failed ? "reply.image.unavailable" : "reply.image.loading")
    }
}

/// Several pictures in a row that swipes a page at a time (the next one shows at the edge), with the shown one's name, a count and page marks.
struct DReplyCarousel: View {
    let entry: String
    let paths: [String]
    let open: (String) -> Void
    @State private var at: Int? = 0
    static let height: CGFloat = 280
    var body: some View {
        let i = min(max(at ?? 0, 0), paths.count - 1)
        VStack(alignment: .leading, spacing: 6) {
            ScrollView(.horizontal, showsIndicators: false) {
                LazyHStack(spacing: 8) {
                    ForEach(Array(paths.enumerated()), id: \.offset) { k, p in
                        DReplyTile(pic: DReplyPic(entry: entry, path: p), carousel: true) { open(p) }
                            .containerRelativeFrame(.horizontal) { w, _ in paths.count > 1 ? w * 0.86 : w }
                            .id(k)
                    }
                }.scrollTargetLayout()
            }
            .scrollTargetBehavior(.viewAligned)
            .scrollPosition(id: $at)
            .frame(height: Self.height)
            .accessibilityIdentifier("reply.carousel")
            HStack(spacing: 8) {
                Text(ReplyImages.name(paths[i])).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1).truncationMode(.middle)
                Spacer(minLength: 8)
                if paths.count <= ReplyImages.maxDots {
                    HStack(spacing: 5) { ForEach(paths.indices, id: \.self) { k in Rectangle().fill(k == i ? BackplaneDesign.accent : BackplaneDesign.line).frame(width: 6, height: 6) } }
                        .accessibilityHidden(true)
                }
                Text(ReplyImages.counter(i, of: paths.count)).dmono(11).foregroundStyle(BackplaneDesign.dim).monospacedDigit().accessibilityIdentifier("reply.carousel.count")
            }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: the viewer

/// A reply's pictures over everything: swipe left and right between them, pinch or double-tap to zoom, drag to pan when zoomed, swipe down
/// or Close to leave. Save to Photos and Share image for the one shown.
struct DReplyViewer: View {
    let open: DReplyOpen
    let close: () -> Void
    @State private var at = 0
    @State private var drop: CGFloat = 0
    @State private var shown: [Int: (UIImage, Data)] = [:]
    @State private var note = ""
    init(open: DReplyOpen, close: @escaping () -> Void) { self.open = open; self.close = close; _at = State(initialValue: open.start) }
    var body: some View {
        let fade = 1 - min(0.7, drop / 500)
        ZStack {
            Color.black.opacity(fade).ignoresSafeArea()
            TabView(selection: $at) {
                ForEach(open.all.indices, id: \.self) { k in
                    DReplyPage(pic: DReplyPic(entry: open.entry, path: open.all[k]), active: k == at, drop: $drop, leave: close) { shown[k] = $0 }
                        .tag(k)
                }
            }
            .tabViewStyle(.page(indexDisplayMode: .never))
            .ignoresSafeArea()
            .offset(y: drop)
            .accessibilityIdentifier("reply.viewer")
        }
        .overlay(alignment: .top) {
            HStack(alignment: .center, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(ReplyImages.name(open.all[min(at, open.all.count - 1)])).dmono(12).foregroundStyle(.white).lineLimit(1).truncationMode(.middle)
                    if open.all.count > 1 { Text(ReplyImages.counter(at, of: open.all.count)).dmono(11).foregroundStyle(.white.opacity(0.75)).monospacedDigit().accessibilityIdentifier("reply.viewer.count") }
                }
                .padding(.horizontal, 8).padding(.vertical, 5).background(.black.opacity(0.45))
                Spacer(minLength: 0)
                Button(action: close) { Image(systemName: "xmark").font(.system(size: 17, weight: .semibold)).foregroundStyle(.white).frame(width: 44, height: 44).background(.black.opacity(0.45)) }
                    .accessibilityLabel(Tr.t("Close")).accessibilityIdentifier("reply.viewer.close")
            }
            .padding(.horizontal, 14).padding(.top, 4)
            .opacity(fade)
        }
        .overlay(alignment: .bottom) {
            VStack(spacing: 10) {
                if !note.isEmpty { Text(Tr.t(note)).dmono(12).foregroundStyle(.white).padding(.horizontal, 12).padding(.vertical, 8).background(.black.opacity(0.7)) }
                if let s = shown[at] {
                    HStack(spacing: 28) { DImageActions(image: s.0, data: s.1, note: { say($0) }) }
                        .labelStyle(.iconOnly).font(.title2).foregroundStyle(.white)
                        .padding(.horizontal, 16).padding(.vertical, 8).background(.black.opacity(0.45))
                        .padding(.bottom, 8)
                }
            }.opacity(fade)
        }
        .presentationBackground(.clear)
        .statusBarHidden()
    }
    private func say(_ s: String) {
        note = s
        Task { try? await Task.sleep(for: .seconds(3)); if note == s { note = "" } }
    }
}

/// One page of the viewer: the whole picture, loaded through the hub, in a zooming view.
private struct DReplyPage: View {
    let pic: DReplyPic
    let active: Bool
    @Binding var drop: CGFloat
    let leave: () -> Void
    let loaded: ((UIImage, Data)) -> Void
    @State private var image: UIImage?
    @State private var failed = false
    var body: some View {
        ZStack {
            if let image {
                DZoom(image: image, active: active) { y, ended, vy in
                    if !ended { drop = max(0, y) }
                    else if y > 120 || vy > 900 { leave() }
                    else { withAnimation(.spring(duration: 0.25)) { drop = 0 } }
                }
            } else if failed {
                VStack(spacing: 8) {
                    Image(systemName: "photo.badge.exclamationmark").font(.largeTitle).foregroundStyle(.white.opacity(0.6))
                    Text(Tr.t("Image unavailable")).dmono(13).foregroundStyle(.white)
                    Text(ReplyImages.name(pic.path)).dmono(11).foregroundStyle(.white.opacity(0.6)).lineLimit(1).truncationMode(.middle)
                }.padding(24).accessibilityIdentifier("reply.viewer.unavailable")
            } else { ProgressView().tint(.white) }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .task(id: pic) {
            if let r = await DReplyStore.full(pic) { image = r.0; loaded(r) } else { failed = true }
        }
    }
}

/// UIKit's zooming scroll view around one picture: pinch, pan when zoomed, double tap in and out. At fitted size a downward pan reports
/// itself (`drag`: distance, ended, speed) so the viewer can be swiped away; sideways pans go to the pages.
private struct DZoom: UIViewRepresentable {
    let image: UIImage
    let active: Bool
    let drag: (CGFloat, Bool, CGFloat) -> Void
    func makeUIView(context: Context) -> DZoomView { let v = DZoomView(); v.set(image); v.drag = drag; return v }
    func updateUIView(_ v: DZoomView, context: Context) {
        if v.image !== image { v.set(image) }
        v.drag = drag
        if !active { v.fit(animated: false) } // a page left behind is fitted again when it is next shown
    }
}

final class DZoomView: UIScrollView, UIScrollViewDelegate {
    private let pic = UIImageView()
    private(set) var image: UIImage?
    var drag: ((CGFloat, Bool, CGFloat) -> Void)?
    private var laidOut: CGSize = .zero
    private lazy var down = UIPanGestureRecognizer(target: self, action: #selector(pan(_:)))
    override init(frame: CGRect) {
        super.init(frame: frame)
        delegate = self
        showsVerticalScrollIndicator = false; showsHorizontalScrollIndicator = false
        contentInsetAdjustmentBehavior = .never; decelerationRate = .fast; bouncesZoom = true
        backgroundColor = .clear
        pic.contentMode = .scaleAspectFit
        addSubview(pic)
        let dbl = UITapGestureRecognizer(target: self, action: #selector(doubleTap(_:))); dbl.numberOfTapsRequired = 2
        addGestureRecognizer(dbl); addGestureRecognizer(down)
        isAccessibilityElement = true; accessibilityIdentifier = "reply.viewer.page"; accessibilityTraits = .image
    }
    required init?(coder: NSCoder) { fatalError() }
    func set(_ i: UIImage) {
        zoomScale = 1; pic.transform = .identity
        image = i; pic.image = i; laidOut = .zero
        accessibilityLabel = nil
        setNeedsLayout()
    }
    private var fitted: Bool { zoomScale <= minimumZoomScale * 1.01 }
    override func layoutSubviews() {
        super.layoutSubviews()
        guard let image, bounds.width > 0, bounds.height > 0 else { return }
        if laidOut != bounds.size {
            let wasFitted = laidOut == .zero || fitted
            let oldRatio = minimumZoomScale > 0 ? zoomScale / minimumZoomScale : 1
            laidOut = bounds.size
            zoomScale = 1
            pic.transform = .identity
            pic.frame = CGRect(origin: .zero, size: image.size)
            contentSize = image.size
            let lo = min(bounds.width / image.size.width, bounds.height / image.size.height)
            minimumZoomScale = lo
            // as far as 4 screen pixels to one of the picture's, and never less than 4 times fitted
            maximumZoomScale = max(lo * 4, 4 / max(1, window?.screen.scale ?? 3))
            zoomScale = wasFitted ? lo : min(maximumZoomScale, max(lo, lo * oldRatio))
            say()
        }
        center()
    }
    func fit(animated: Bool) { if minimumZoomScale > 0, !fitted { setZoomScale(minimumZoomScale, animated: animated) } }
    private func center() {
        let w = pic.frame.width, h = pic.frame.height
        contentInset = UIEdgeInsets(top: max(0, (bounds.height - h) / 2), left: max(0, (bounds.width - w) / 2), bottom: 0, right: 0)
    }
    func viewForZooming(in scrollView: UIScrollView) -> UIView? { pic }
    func scrollViewDidZoom(_ scrollView: UIScrollView) { center(); say() }
    /// the zoom, times fitted, as the page's accessibility value ("1.0x" fitted)
    private func say() { accessibilityValue = ReplyImages.zoomLabel(current: Double(zoomScale), min: Double(minimumZoomScale)) }
    @objc private func doubleTap(_ g: UITapGestureRecognizer) {
        let to = CGFloat(ReplyImages.doubleTap(current: Double(zoomScale), min: Double(minimumZoomScale), max: Double(maximumZoomScale)))
        if to <= minimumZoomScale { setZoomScale(to, animated: true); return }
        let p = g.location(in: pic)
        let size = CGSize(width: bounds.width / to, height: bounds.height / to)
        zoom(to: CGRect(x: p.x - size.width / 2, y: p.y - size.height / 2, width: size.width, height: size.height), animated: true)
    }
    @objc private func pan(_ g: UIPanGestureRecognizer) {
        let t = g.translation(in: superview), v = g.velocity(in: superview)
        switch g.state {
        case .changed: drag?(t.y, false, v.y)
        case .ended, .cancelled, .failed: drag?(t.y, true, v.y)
        default: break
        }
    }
    override func gestureRecognizerShouldBegin(_ g: UIGestureRecognizer) -> Bool {
        if g === down { let v = down.velocity(in: self); return fitted && v.y > 0 && v.y > abs(v.x) * 1.2 }
        return super.gestureRecognizerShouldBegin(g)
    }
}
