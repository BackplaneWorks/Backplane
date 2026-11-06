@preconcurrency import AVFoundation
import Foundation

// Dictation from this phone's microphone (docs/voice.md): the hub cannot
// hear the person, so the phone records 16 kHz mono s16le and uploads it,
// in order, in ~200 ms pieces to the address the hub's start reply named
// (/voice/in?s=<key>, the bridge's "voice-audio" command), with the pairing
// token; the last POST carries &end=1. Recording starts on the press and
// what is heard before the address comes waits for it. Nothing here decides
// anything about dictation (src/web/voice-mic.js is the web's twin):
// asks the hub, the hub answers, and its strip is drawn from the screen.
@MainActor
final class VoiceRecorder {
    static let rate: Double = 16000
    static let pieceSeconds: Double = 0.2

    private(set) var active = false
    // the hub (host:port) the session belongs to
    private(set) var hub = ""
    private var link = ""
    private var url: URL?
    private var token: String?
    private var held: [Data] = []
    private var ending = false
    private var buf = Data()
    private var engine: AVAudioEngine?
    private var converter: AVAudioConverter?
    private var timer: Timer?
    private var chain: Task<Void, Never>?
    private var started = Date()

    // the press: start recording into hub's session (false: no microphone)
    func start(hub: String, link: String) async -> Bool {
        if active { return true }
        reset()
        guard await Self.permission() else { return false }
        self.hub = hub
        self.link = link
        active = true
        started = Date()
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playAndRecord, mode: .measurement, options: [.defaultToSpeaker, .allowBluetoothHFP, .mixWithOthers])
            try session.setActive(true, options: [])
            let engine = AVAudioEngine()
            let input = engine.inputNode
            let from = input.outputFormat(forBus: 0)
            guard let to = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: Self.rate, channels: 1, interleaved: true),
                  let converter = AVAudioConverter(from: from, to: to) else { throw VoiceError.format }
            self.converter = converter
            input.installTap(onBus: 0, bufferSize: 4096, format: from) { [weak self] buffer, _ in
                guard let pcm = Self.convert(buffer, converter, to) else { return }
                Task { @MainActor [weak self] in self?.take(pcm) }
            }
            engine.prepare()
            try engine.start()
            self.engine = engine
            timer = Timer.scheduledTimer(withTimeInterval: Self.pieceSeconds, repeats: true) { [weak self] _ in
                Task { @MainActor [weak self] in self?.tick() }
            }
            return true
        } catch {
            active = false
            release()
            return false
        }
    }

    // the hub's start reply: where the audio goes
    func address(hub: String, path: String) {
        guard hub == self.hub, active || ending, let target = Self.target(link, path) else { return }
        url = target.url
        token = target.token
        for p in held { post(p, last: false) }
        held = []
        if ending { ending = false; post(Data(), last: true) }
    }

    // the person stopped: what is left goes, with the end mark
    func stop() {
        guard active else { return }
        timer?.invalidate(); timer = nil
        if url != nil { flush(last: true) } else { held.append(piece()); ending = true }
        active = false
        release()
    }

    // the hub ended the session (idle, an error): nothing more to send
    func gone(hub: String) {
        guard hub == self.hub else { return }
        timer?.invalidate(); timer = nil
        active = false
        ending = false
        held = []
        release()
    }

    private func reset() {
        active = false; url = nil; token = nil; held = []; ending = false; buf = Data(); chain = nil
    }

    private func take(_ d: Data) {
        guard active else { return }
        buf.append(d)
    }

    private func tick() {
        // a start the hub never answered (an older hub, a refused request)
        if url == nil, Date().timeIntervalSince(started) > 8 { gone(hub: hub); return }
        flush(last: false)
    }

    private func piece() -> Data {
        let d = buf
        buf = Data()
        return d
    }

    private func flush(last: Bool) {
        let b = piece()
        if b.isEmpty && !last { return }
        guard url != nil else { held.append(b); return }
        post(b, last: last)
    }

    // in order: each POST waits for the one before it
    private func post(_ body: Data, last: Bool) {
        guard let base = url else { return }
        guard var c = URLComponents(url: base, resolvingAgainstBaseURL: false) else { return }
        if last {
            let items = c.queryItems ?? []
            c.queryItems = items + [URLQueryItem(name: "end", value: "1")]
        }
        guard let u = c.url else { return }
        var req = URLRequest(url: u, timeoutInterval: 30)
        req.httpMethod = "POST"
        req.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
        if let token { req.setValue("Bearer " + token, forHTTPHeaderField: "Authorization") }
        let before = chain
        chain = Task.detached {
            await before?.value
            _ = try? await URLSession.shared.upload(for: req, from: body)
        }
    }

    private func release() {
        engine?.inputNode.removeTap(onBus: 0)
        engine?.stop()
        engine = nil
        converter = nil
        try? AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
    }

    // where a path on the paired hub is (/voice/in?s=…), and its token
    static func target(_ link: String, _ path: String) -> (url: URL, token: String?)? {
        guard let p = URLComponents(string: path), let ws = Pairing.socket(link),
              let wc = URLComponents(url: ws, resolvingAgainstBaseURL: false) else { return nil }
        let token = wc.queryItems?.first { $0.name == "token" }?.value
        guard let u = Pairing.http(link, path: p.path, query: p.queryItems ?? [], token: false) else { return nil }
        return (u, token)
    }

    static func permission() async -> Bool {
        await withCheckedContinuation { k in
            AVAudioApplication.requestRecordPermission { ok in k.resume(returning: ok) }
        }
    }

    // a tap's buffer at the microphone's rate as 16 kHz mono s16le
    nonisolated static func convert(_ buffer: AVAudioPCMBuffer, _ converter: AVAudioConverter, _ to: AVAudioFormat) -> Data? {
        let ratio = to.sampleRate / buffer.format.sampleRate
        let cap = AVAudioFrameCount(Double(buffer.frameLength) * ratio + 32)
        guard let out = AVAudioPCMBuffer(pcmFormat: to, frameCapacity: cap) else { return nil }
        var fed = false
        var err: NSError?
        converter.convert(to: out, error: &err) { _, status in
            if fed { status.pointee = .noDataNow; return nil }
            fed = true
            status.pointee = .haveData
            return buffer
        }
        guard err == nil, out.frameLength > 0, let ch = out.int16ChannelData else { return nil }
        return Data(bytes: ch[0], count: Int(out.frameLength) * 2)
    }

    enum VoiceError: Error { case format }
}
