import Foundation

// Pure unit tests for GIF header detection, delay sanitization, and memory-bounded frame sizing
// matching `AnimatedImage` in Backplane/AnimatedImage.swift.
@main struct AnimatedImageTests {
    static func isGIF(_ bytes: [UInt8]) -> Bool {
        guard bytes.count >= 6 else { return false }
        let h = Array(bytes.prefix(6))
        return h == [0x47, 0x49, 0x46, 0x38, 0x37, 0x61] || h == [0x47, 0x49, 0x46, 0x38, 0x39, 0x61]
    }
    static func frameDelay(unclamped: Double?, clamped: Double?) -> Double {
        let raw: Double
        if let u = unclamped, u > 0 { raw = u }
        else if let c = clamped, c > 0 { raw = c }
        else { return 0.1 }
        return raw < 0.02 ? 0.1 : raw
    }
    static func boundedSide(width: Double, height: Double, frameCount: Int, requestedMax: Double?, maxDecodedBytes: Int) -> Double? {
        guard width > 0, height > 0, frameCount > 0, maxDecodedBytes > 0 else { return requestedMax }
        let longest = max(width, height)
        var target = requestedMax.map { min(longest, $0) } ?? longest
        let scale = target / longest
        let estBytes = Double(frameCount) * (width * scale) * (height * scale) * 4.0
        if estBytes > Double(maxDecodedBytes) {
            let factor = sqrt(Double(maxDecodedBytes) / estBytes)
            target = max(32, floor(target * factor))
        }
        return target < longest - 0.5 ? target : nil
    }
    static func main() {
        assert(isGIF(Array("GIF89a\0\0".utf8)))
        assert(isGIF(Array("GIF87a\0\0".utf8)))
        assert(!isGIF([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A]))
        assert(!isGIF([0x47, 0x49, 0x46]))

        assert(abs(frameDelay(unclamped: 0.05, clamped: 0.1) - 0.05) < 1e-6)
        assert(abs(frameDelay(unclamped: 0.0, clamped: 0.15) - 0.15) < 1e-6)
        assert(abs(frameDelay(unclamped: 0.01, clamped: 0.01) - 0.1) < 1e-6)
        assert(abs(frameDelay(unclamped: nil, clamped: nil) - 0.1) < 1e-6)

        assert(boundedSide(width: 200, height: 200, frameCount: 10, requestedMax: nil, maxDecodedBytes: 24 * 1024 * 1024) == nil)
        let capped = boundedSide(width: 1000, height: 1000, frameCount: 60, requestedMax: nil, maxDecodedBytes: 24 * 1024 * 1024)
        assert(capped != nil && capped! <= 320 && capped! >= 300)
        print("AnimatedImageTests: ok")
    }
}
