import SwiftUI
import UIKit
import ImageIO

// Animated GIF (and multi-frame raster) decoding and playback for iOS.
// SwiftUI's `Image(uiImage:)` only draws frame 0; `DAnimatedUIImageView` drives per-frame delays with a CADisplayLink
// that pauses automatically when offscreen or removed from the window, and `AnimatedImage.decode` bounds memory/frame count.

final class AnimatedImage: @unchecked Sendable {
    let frames: [UIImage]
    let delays: [TimeInterval]
    let loopCount: Int // 0 means loop forever
    var firstFrame: UIImage { frames[0] }
    var size: CGSize { frames[0].size }

    static let maxUploadBytes = 8 * 1024 * 1024
    static let defaultMaxFrames = 240
    static let defaultMaxDecodedBytes = 24 * 1024 * 1024

    init(frames: [UIImage], delays: [TimeInterval], loopCount: Int) {
        self.frames = frames
        self.delays = delays
        self.loopCount = loopCount
    }

    static func isGIF(_ data: Data) -> Bool {
        guard data.count >= 6 else { return false }
        let b = data.prefix(6)
        return b.elementsEqual([0x47, 0x49, 0x46, 0x38, 0x37, 0x61]) // GIF87a
            || b.elementsEqual([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]) // GIF89a
    }

    /// Per-frame delay in seconds: prefers unclamped delay, falls back to clamped delay, and clamps < 0.02s (including 0) to 0.1s per browser/GIF spec.
    static func frameDelay(unclamped: Double?, clamped: Double?) -> TimeInterval {
        let raw: Double
        if let u = unclamped, u > 0 { raw = u }
        else if let c = clamped, c > 0 { raw = c }
        else { return 0.1 }
        return raw < 0.02 ? 0.1 : raw
    }

    /// Computes a max pixel side that keeps `frameCount * w * h * 4 <= maxDecodedBytes` and `<= requestedMax`, or nil when full resolution fits.
    static func boundedSide(width: CGFloat, height: CGFloat, frameCount: Int, requestedMax: CGFloat?, maxDecodedBytes: Int) -> CGFloat? {
        guard width > 0, height > 0, frameCount > 0, maxDecodedBytes > 0 else { return requestedMax }
        let longest = max(width, height)
        var target = requestedMax.map { min(longest, $0) } ?? longest
        let scale = target / longest
        let estBytes = Double(frameCount) * Double(width * scale) * Double(height * scale) * 4.0
        if estBytes > Double(maxDecodedBytes) {
            let factor = sqrt(Double(maxDecodedBytes) / estBytes)
            target = max(32, floor(target * CGFloat(factor)))
        }
        return target < longest - 0.5 ? target : nil
    }

    /// Decodes a multi-frame image (animated GIF / WebP). Returns nil for single-frame images so callers keep the fast static UIImage path.
    static func decode(_ data: Data, maxPixelSize: CGFloat? = nil, maxFrames: Int = defaultMaxFrames, maxDecodedBytes: Int = defaultMaxDecodedBytes) -> AnimatedImage? {
        guard let src = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary) else { return nil }
        let rawCount = CGImageSourceGetCount(src)
        guard rawCount > 1 else { return nil }
        let count = min(rawCount, maxFrames)

        var loopCount = 0
        if let props = CGImageSourceCopyProperties(src, nil) as? [CFString: Any] {
            if let gif = props[kCGImagePropertyGIFDictionary] as? [CFString: Any], let lc = gif[kCGImagePropertyGIFLoopCount] as? Int {
                loopCount = max(0, lc)
            } else if let webp = props[kCGImagePropertyWebPDictionary] as? [CFString: Any], let lc = webp[kCGImagePropertyWebPLoopCount] as? Int {
                loopCount = max(0, lc)
            }
        }

        var w: CGFloat = 0, h: CGFloat = 0
        if let p0 = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any] {
            w = (p0[kCGImagePropertyPixelWidth] as? CGFloat) ?? 0
            h = (p0[kCGImagePropertyPixelHeight] as? CGFloat) ?? 0
        }
        let side = boundedSide(width: w, height: h, frameCount: count, requestedMax: maxPixelSize, maxDecodedBytes: maxDecodedBytes)

        var frames: [UIImage] = []
        var delays: [TimeInterval] = []
        frames.reserveCapacity(count)
        delays.reserveCapacity(count)

        for i in 0..<count {
            let cg: CGImage?
            if let side {
                let opts: [CFString: Any] = [
                    kCGImageSourceCreateThumbnailFromImageAlways: true,
                    kCGImageSourceCreateThumbnailWithTransform: true,
                    kCGImageSourceShouldCacheImmediately: true,
                    kCGImageSourceThumbnailMaxPixelSize: side
                ]
                cg = CGImageSourceCreateThumbnailAtIndex(src, i, opts as CFDictionary)
            } else {
                cg = CGImageSourceCreateImageAtIndex(src, i, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary)
            }
            guard let cg else { continue }
            var unclamped: Double?
            var clamped: Double?
            if let fp = CGImageSourceCopyPropertiesAtIndex(src, i, nil) as? [CFString: Any] {
                if let gif = fp[kCGImagePropertyGIFDictionary] as? [CFString: Any] {
                    unclamped = gif[kCGImagePropertyGIFUnclampedDelayTime] as? Double
                    clamped = gif[kCGImagePropertyGIFDelayTime] as? Double
                } else if let webp = fp[kCGImagePropertyWebPDictionary] as? [CFString: Any] {
                    unclamped = webp[kCGImagePropertyWebPUnclampedDelayTime] as? Double
                    clamped = webp[kCGImagePropertyWebPDelayTime] as? Double
                }
            }
            frames.append(UIImage(cgImage: cg))
            delays.append(frameDelay(unclamped: unclamped, clamped: clamped))
        }
        guard frames.count > 1 else { return nil }
        return AnimatedImage(frames: frames, delays: delays, loopCount: loopCount)
    }
}

/// Decoded image kept in NSCache: always has a static `image` (frame 0 for GIFs) and optional `animated` frames.
final class DDecodedImage: @unchecked Sendable {
    let image: UIImage
    let animated: AnimatedImage?
    init(image: UIImage, animated: AnimatedImage? = nil) { self.image = image; self.animated = animated }

    static func decode(_ data: Data, maxPixelSize: CGFloat? = nil) -> DDecodedImage? {
        if AnimatedImage.isGIF(data), let anim = AnimatedImage.decode(data, maxPixelSize: maxPixelSize) {
            return DDecodedImage(image: anim.firstFrame, animated: anim)
        }
        if let side = maxPixelSize {
            guard let src = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
                  let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, [
                    kCGImageSourceCreateThumbnailFromImageAlways: true,
                    kCGImageSourceCreateThumbnailWithTransform: true,
                    kCGImageSourceShouldCacheImmediately: true,
                    kCGImageSourceThumbnailMaxPixelSize: side
                  ] as CFDictionary)
            else { return UIImage(data: data).map { DDecodedImage(image: $0) } }
            return DDecodedImage(image: UIImage(cgImage: cg))
        }
        guard let img = UIImage(data: data)?.preparingForDisplay() ?? UIImage(data: data) else { return nil }
        return DDecodedImage(image: img)
    }
}

private final class DisplayLinkProxy: NSObject {
    weak var owner: DAnimatedUIImageView?
    init(_ owner: DAnimatedUIImageView) { self.owner = owner }
    @objc func tick(_ link: CADisplayLink) { owner?.step(link) }
}

/// UIImageView subclass that plays `AnimatedImage` frames using CADisplayLink, pausing automatically when offscreen.
final class DAnimatedUIImageView: UIImageView {
    private(set) var animated: AnimatedImage?
    private var displayLink: CADisplayLink?
    private var frameIndex = 0
    private var accumulated: CFTimeInterval = 0
    private var completedLoops = 0
    var isPlaybackEnabled = true { didSet { updatePlayback() } }

    deinit { stopLink() }

    func set(animated anim: AnimatedImage) {
        if animated === anim { updatePlayback(); return }
        animated = anim
        frameIndex = 0
        accumulated = 0
        completedLoops = 0
        image = anim.firstFrame
        invalidateIntrinsicContentSize()
        updatePlayback()
    }

    func set(staticImage: UIImage) {
        stopLink()
        animated = nil
        frameIndex = 0
        accumulated = 0
        completedLoops = 0
        image = staticImage
        invalidateIntrinsicContentSize()
    }

    override var intrinsicContentSize: CGSize { animated?.size ?? image?.size ?? super.intrinsicContentSize }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        updatePlayback()
    }

    private func updatePlayback() {
        if window != nil && isPlaybackEnabled && animated != nil {
            startLink()
        } else {
            stopLink()
        }
    }

    private func startLink() {
        guard displayLink == nil, animated != nil else { return }
        let link = CADisplayLink(target: DisplayLinkProxy(self), selector: #selector(DisplayLinkProxy.tick(_:)))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    private func stopLink() {
        displayLink?.invalidate()
        displayLink = nil
    }

    fileprivate func step(_ link: CADisplayLink) {
        guard let anim = animated, anim.frames.count > 1 else { stopLink(); return }
        let dt = min(max(0, link.targetTimestamp - link.timestamp), 0.25)
        accumulated += dt
        var delay = anim.delays[frameIndex]
        while accumulated >= delay {
            accumulated -= delay
            let next = frameIndex + 1
            if next >= anim.frames.count {
                completedLoops += 1
                if anim.loopCount > 0 && completedLoops >= anim.loopCount {
                    stopLink()
                    return
                }
                frameIndex = 0
            } else {
                frameIndex = next
            }
            image = anim.frames[frameIndex]
            delay = anim.delays[frameIndex]
        }
    }
}

/// SwiftUI view that plays an `AnimatedImage` inside a `DAnimatedUIImageView`.
struct DAnimatedImageView: UIViewRepresentable {
    let animated: AnimatedImage
    var contentMode: UIView.ContentMode = .scaleAspectFit
    var isPlaying: Bool = true

    func makeUIView(context: Context) -> DAnimatedUIImageView {
        let v = DAnimatedUIImageView()
        v.contentMode = contentMode
        v.clipsToBounds = true
        v.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        v.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
        v.setContentHuggingPriority(.defaultLow, for: .horizontal)
        v.setContentHuggingPriority(.defaultLow, for: .vertical)
        v.isPlaybackEnabled = isPlaying
        v.set(animated: animated)
        return v
    }

    func updateUIView(_ v: DAnimatedUIImageView, context: Context) {
        v.contentMode = contentMode
        v.isPlaybackEnabled = isPlaying
        v.set(animated: animated)
    }

    static func dismantleUIView(_ v: DAnimatedUIImageView, coordinator: ()) {
        v.isPlaybackEnabled = false
    }
}

/// Unified picture view: animates multi-frame GIFs via `DAnimatedImageView`, draws static images with SwiftUI `Image`.
struct DPictureView: View {
    let image: UIImage
    var animated: AnimatedImage? = nil
    var contentMode: ContentMode = .fit
    var isPlaying: Bool = true
    var body: some View {
        if let animated {
            DAnimatedImageView(animated: animated, contentMode: contentMode == .fill ? .scaleAspectFill : .scaleAspectFit, isPlaying: isPlaying)
                .aspectRatio(animated.size, contentMode: contentMode)
        } else {
            Image(uiImage: image).resizable().aspectRatio(contentMode: contentMode)
        }
    }
}
