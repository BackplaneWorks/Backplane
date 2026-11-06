import SwiftUI
import UniformTypeIdentifiers

struct ThreadComposer: View {
    // a draft above this many UTF-8 bytes is shown as a preview, not laid out in the field (see the composer row)
    static let longDraftBytes = 64_000
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let thread: ThreadView
    @FocusState private var focused: Bool
    @State private var showPermissions = false
    private var canSend: Bool { model.canSendComposer(for: thread.id) }
    private func perform(_ action: String, _ value: String = "") {
        guard model.canSendComposer(for: thread.id) else { return }
        model.actComposer(action, value, for: thread.id)
    }
    var body: some View {
        #if DEBUG
        let _ = RenderCount.tick("threadComposer")
        #endif
          VStack(spacing: 0) {
            Group {
            // the terminals pinned to the top of the thread
            ForEach((thread.embs ?? []).filter { $0.pinned }, id: \.key) { e in
                EmbView(model: model, emb: e, top: true, owner: thread.id).padding(.horizontal).padding(.top, 8)
            }
            ForEach(thread.asks ?? []) { a in
                AskCard(model: model, ask: a, owner: thread.id).padding(.horizontal).padding(.top, 8)
            }
            ForEach(thread.queue ?? [], id: \.msg) { q in
                HStack(spacing: 6) {
                    Text(q.tag).bold()
                    Text(q.text).lineLimit(1)
                    Spacer(minLength: 0)
                    ForEach(q.buttons, id: \.label) { b in
                        Button(b.label) { perform(b.action, b.value) }.buttonStyle(.borderless)
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
                .padding(.horizontal).padding(.top, 8)
            }
            }
            .disabled(!canSend)
            HStack {
                Menu {
                    Section(NativeMessages.native_thread_model(lang)) {
                        ForEach(thread.picker.models, id: \.value) { c in
                            Button { perform("model", c.value) } label: {
                                if c.on { Label(c.label, systemImage: "checkmark") } else { Text(c.label) }
                            }
                        }
                    }
                    if !thread.picker.efforts.isEmpty {
                        Section(NativeMessages.native_thread_effort(lang)) {
                            ForEach(thread.picker.efforts, id: \.value) { c in
                                Button { perform("effort", c.value) } label: {
                                    if c.on { Label(c.label, systemImage: "checkmark") } else { Text(c.label) }
                                }
                            }
                        }
                    }
                    if let ps = thread.picker.providers, !ps.isEmpty {
                        Section(NativeMessages.native_bot_provider(lang)) {
                            ForEach(ps, id: \.value) { c in
                                Button { perform("effort", c.value) } label: {
                                    if c.on { Label(c.label, systemImage: "checkmark") } else { Text(c.label) }
                                }
                            }
                        }
                    }
                } label: {
                    HStack(spacing: 4) {
                        Text(thread.picker.label).lineLimit(1)
                        Image(systemName: "chevron.down")
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }
                .accessibilityLabel(NativeMessages.native_thread_model_effort(lang, thread.picker.label))
                .disabled(!canSend)
                if let permissions = thread.permissions {
                  HStack(spacing: 0) {
                    Button {
                        showPermissions = true
                    } label: {
                        Image(systemName: "lock.shield")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .accessibilityLabel(permissions.current)
                    .accessibilityIdentifier("thread.permissions")
                    .disabled(!canSend)
                  }
                    .popover(isPresented: $showPermissions, arrowEdge: .bottom) {
                        VStack(alignment: .leading, spacing: 0) {
                            ForEach(permissions.choices, id: \.value) { choice in
                                Button {
                                    showPermissions = false
                                    perform("thread-permissions", choice.value)
                                } label: {
                                    HStack {
                                        Text(choice.label)
                                        Spacer()
                                        if choice.on { Image(systemName: "checkmark") }
                                    }
                                    .padding(.horizontal, 16).padding(.vertical, 12)
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                                .accessibilityIdentifier("thread.permission.choice." + choice.value)
                                .disabled(!canSend)
                            }
                        }
                        .frame(width: 260)
                        .presentationCompactAdaptation(.popover)
                    }
                }
                // the link is down, or coming back after it was up (a reconnect,
                // not the first connect): say so, not "loading"
                if let status = model.composerStatus(for: thread.id), status.phase == .offline {
                    HStack(spacing: 4) {
                        HubDot(phase: .offline)
                        Text(status.failure.map { NativeMessages.native_hub_failure(lang, $0.reason, $0.code) }
                             ?? HubDot.word(.offline, lang)).lineLimit(2)
                    }
                    .font(.caption2).foregroundStyle(.secondary)
                    .accessibilityIdentifier("thread.reconnecting")
                } else if let status = model.composerStatus(for: thread.id), status.phase == .syncing || status.phase == .connecting {
                    HStack(spacing: 4) {
                        ProgressView().controlSize(.mini)
                        Text(status.stage == "recent" || status.phase == .connecting ? NativeMessages.native_thread_loading_recent(lang) : NativeMessages.native_thread_loading_older(lang))
                            .lineLimit(1)
                    }
                    .font(.caption2).foregroundStyle(.secondary)
                    .accessibilityIdentifier("thread.loading")
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal).padding(.top, 8)
            if canSend { ComposerExtras(model: model, thread: thread) }
            if let v = thread.voice { VoiceBar(model: model, voice: v, owner: thread.id) }
            if model.sendFailure == thread.id {
                Text(NativeMessages.native_thread_send_failed(lang))
                    .font(.caption).foregroundStyle(.red).padding(.horizontal)
                    .accessibilityIdentifier("thread.send-failed")
            }
            HStack(alignment: .bottom, spacing: 8) {
                AttachButton(model: model, owner: thread.id).disabled(!canSend)
                if let v = thread.voice {
                    let live = v.on || !v.state.isEmpty
                    Button { model.voicePress(for: thread.id, on: live) } label: {
                        Image(systemName: live ? "mic.fill" : "mic").font(.system(size: 20))
                            .foregroundStyle(live ? Color.red : Color.secondary)
                            .frame(minWidth: 36, minHeight: 44)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(live ? v.stop : v.mic)
                    .accessibilityIdentifier("thread.mic")
                    .disabled(!canSend)
                }
                // A draft this long (a pasted file, a log) is not edited in the field: SwiftUI's multi-line TextField lays the
                // whole text out on the main thread at every screen update (8-22 s at 20-100 KB: views timed out, "Updating
                // conversation..." and a send that never went). It is kept whole, sent whole, and shown as a preview.
                if model.composerText(for: thread).utf8.count > Self.longDraftBytes {
                    let long = model.composerText(for: thread)
                    VStack(alignment: .leading, spacing: 6) {
                        Text(String(long.prefix(400)) + "…").font(.callout).lineLimit(5).foregroundStyle(.primary)
                        HStack {
                            Text(String(format: lang == "ja" ? "長い下書き · %d 文字" : "Long draft · %d characters", long.count))
                                .font(.caption).foregroundStyle(.secondary)
                            Spacer()
                            Button(lang == "ja" ? "消去" : "Clear") { model.editComposer("", for: thread.id) }
                                .font(.caption).accessibilityIdentifier("thread.composer.clear")
                        }
                    }
                    .padding(.horizontal, 12).padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(BackplaneDesign.raised)
                    .accessibilityElement(children: .combine)
                    .accessibilityIdentifier("thread.composer")
                } else {
                    ComposerInput(model: model, thread: thread, focus: $focused)
                        .padding(.horizontal, 12).padding(.vertical, 8)
                        .foregroundStyle(.primary)
                        .background(BackplaneDesign.raised)
                }
                // a long press sends with the other follow-up mode (queue or steer);
                // while a turn runs with nothing typed the button stops it
                let blank = model.composerText(for: thread).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                let stop = model.composerSendAct(for: thread) == "interrupt" && blank
                Image(systemName: stop ? "stop.circle.fill" : "arrow.up.circle.fill").font(.system(size: 32))
                    .frame(minWidth: 44, minHeight: 44)
                    .foregroundStyle(stop ? Color.red : blank ? Color.secondary : Color.accentColor)
                    .onTapGesture {
                        if model.canSendComposer(for: thread.id) { if stop { model.interruptComposer(for: thread.id) } else if !blank { model.sendComposer(for: thread.id) } }
                    }
                    .onLongPressGesture {
                        if model.canSendComposer(for: thread.id), !model.composerText(for: thread).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { model.sendComposer("send-alt", for: thread.id) }
                    }
                    .accessibilityLabel(model.composerSend(for: thread))
                    .accessibilityIdentifier("thread.send")
                    .accessibilityAddTraits(.isButton)
                    .disabled(!canSend)
            }
            .padding(.horizontal).padding(.vertical, 8)
          }
          .background(BackplaneDesign.panel)
          // a picture or a file dropped on the composer (iPad drag and drop) is attached like the paperclip's
          .onDrop(of: [UTType.image, UTType.fileURL], isTargeted: nil) { providers in
              guard canSend else { return false }
              let owner = thread.id
              for p in providers {
                  if p.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                      _ = p.loadObject(ofClass: URL.self) { url, _ in
                          guard let url else { return }
                          // a copy in our temp folder: the dropped file's scope ends with this callback
                          let tmp = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + "-" + url.lastPathComponent)
                          let scoped = url.startAccessingSecurityScopedResource()
                          defer { if scoped { url.stopAccessingSecurityScopedResource() } }
                          guard (try? FileManager.default.copyItem(at: url, to: tmp)) != nil else { return }
                          Task { @MainActor in model.attach(file: tmp, name: url.lastPathComponent, for: owner) }
                      }
                  } else if p.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                      p.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { data, _ in
                          guard let data else { return }
                          Task { @MainActor in model.attach(data, name: "dropped.png", for: owner) }
                      }
                  }
              }
              return true
          }

    }
}

// Dictation over the composer (screen.bend's Mob.voice), as calm as the
// web's strip: a dot, one word for the state, the level and Stop, never the
// words (they appear only in the draft); a note when something is wrong;
// after a segment its fixes, each a chip that undoes it; the word to
// clarify, with its choices.
struct VoiceBar: View {
    let model: AppModel
    let voice: VoiceView
    let owner: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !voice.note.isEmpty {
                HStack(spacing: 6) {
                    Text(voice.note).font(.caption).foregroundStyle(voice.bad ? Color.red : Color.orange).lineLimit(3)
                    Spacer(minLength: 0)
                    Button { model.actComposer("voice-err", "", for: owner) } label: { Image(systemName: "xmark").font(.caption) }
                        .buttonStyle(.borderless)
                }
            }
            if voice.on {
                HStack(spacing: 8) {
                    Circle().fill(Color.red).frame(width: 8, height: 8)
                    Text(voice.status).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    GeometryReader { g in
                        ZStack(alignment: .leading) {
                            Rectangle().fill(Color.secondary.opacity(0.2))
                            Rectangle().fill(Color.accentColor).frame(width: g.size.width * CGFloat(min(max(voice.level, 0), 100)) / 100)
                        }
                    }
                    .frame(width: 72, height: 4)
                    .animation(.linear(duration: 0.2), value: voice.level)
                    Spacer(minLength: 0)
                    Button(voice.stop) { model.voicePress(for: owner, on: true) }
                        .font(.caption).buttonStyle(.borderless)
                        .accessibilityIdentifier("voice.stop")
                }
                .accessibilityIdentifier("voice.strip")
            }
            if !voice.fixes.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(voice.fixes, id: \.value) { f in
                            Button(f.label) { model.actComposer(f.action, f.value, for: owner) }
                                .font(.caption).buttonStyle(.bordered)
                                .accessibilityHint(f.tip ?? "")
                        }
                    }
                }
            }
            if let a = voice.ask {
                VStack(alignment: .leading, spacing: 4) {
                    Text(a.text).font(.caption)
                    if a.fixing, let f = a.field {
                        TextField(f.hint, text: Binding(get: { f.value }, set: { model.field(f.name, $0) }))
                            .textInputAutocapitalization(.never).autocorrectionDisabled()
                            .font(.callout).padding(6).background(BackplaneDesign.raised)
                            .onSubmit { model.actComposer("voice-fixed", "", for: owner) }
                            .accessibilityIdentifier("voice.spelling")
                    }
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 6) {
                            ForEach(a.buttons, id: \.self) { b in
                                if b.primary == true {
                                    Button(b.label) { model.actComposer(b.action, b.value, for: owner) }.font(.caption).buttonStyle(.borderedProminent)
                                } else {
                                    Button(b.label) { model.actComposer(b.action, b.value, for: owner) }.font(.caption).buttonStyle(.bordered)
                                }
                            }
                        }
                    }
                }
                .accessibilityIdentifier("voice.ask")
            }
        }
        .padding(.horizontal).padding(.top, voice.on || !voice.note.isEmpty || !voice.fixes.isEmpty || voice.ask != nil ? 8 : 0)
    }
}
