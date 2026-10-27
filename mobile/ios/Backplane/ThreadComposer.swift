import SwiftUI

struct ThreadComposer: View {
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
          VStack(spacing: 0) {
            Group {
            // the terminals pinned to the top of the thread
            ForEach((thread.embs ?? []).filter { $0.pinned }, id: \.key) { e in
                EmbView(model: model, emb: e, top: true, owner: thread.id).padding(.horizontal).padding(.top, 8)
            }
            ForEach(thread.asks ?? []) { a in
                AskCard(model: model, ask: a, owner: thread.id).padding(.horizontal).padding(.top, 8)
            }
            if let td = thread.todos {
                VStack(alignment: .leading, spacing: 2) {
                    // the head opens and shuts the card
                    Button { if let t = td.toggle, !t.isEmpty { perform("fold", t) } } label: {
                        HStack {
                            Text(td.head).bold()
                            Spacer(minLength: 0)
                            Image(systemName: (td.open ?? true) ? "chevron.down" : "chevron.right").foregroundStyle(.secondary)
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    // every step scrolls within a third of the screen
                    ScrollView {
                        VStack(alignment: .leading, spacing: 2) {
                            ForEach(Array(td.lines.enumerated()), id: \.offset) { _, l in
                                Text(l.text).lineLimit(1).foregroundStyle(l.status == "completed" ? .secondary : .primary)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }
                    .frame(maxHeight: 220)
                    .fixedSize(horizontal: false, vertical: true)
                    if let more = td.more, !more.isEmpty {
                        Button(more) { perform("fold", td.moreValue ?? "") }.buttonStyle(.borderless)
                    }
                }
                .font(.caption)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal).padding(.top, 8)
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
            if model.sendFailure == thread.id {
                Text(NativeMessages.native_thread_send_failed(lang))
                    .font(.caption).foregroundStyle(.red).padding(.horizontal)
                    .accessibilityIdentifier("thread.send-failed")
            }
            HStack(alignment: .bottom, spacing: 8) {
                AttachButton(model: model, owner: thread.id).disabled(!canSend)
                TextField("", text: Binding(get: { model.composerText(for: thread) }, set: { model.editComposer($0, for: thread.id) }), prompt: Text(NativeMessages.native_thread_composer(lang)).foregroundStyle(.secondary), axis: .vertical)
                    .disabled(!model.canEditComposer(for: thread.id))
                    .lineLimit(1...6)
                    .focused($focused)
                    .accessibilityLabel(NativeMessages.native_thread_composer(lang))
                    .accessibilityIdentifier("thread.composer")
                    .padding(.horizontal, 12).padding(.vertical, 8)
                    .foregroundStyle(.primary)
                    .background(BackplaneDesign.raised)
                // a long press sends with the other follow-up mode (queue or steer);
                // while a turn runs with nothing typed the button stops it
                let blank = model.composerText(for: thread).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                let stop = thread.sendAct == "interrupt" && blank
                Image(systemName: stop ? "stop.circle.fill" : "arrow.up.circle.fill").font(.system(size: 32))
                    .foregroundStyle(stop ? Color.red : blank ? Color.secondary : Color.accentColor)
                    .onTapGesture {
                        if model.canSendComposer(for: thread.id) { if stop { model.interruptComposer(for: thread.id) } else if !blank { model.sendComposer(for: thread.id) } }
                    }
                    .onLongPressGesture {
                        if model.canSendComposer(for: thread.id), !model.composerText(for: thread).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { model.sendComposer("send-alt", for: thread.id) }
                    }
                    .accessibilityLabel(thread.send)
                    .accessibilityIdentifier("thread.send")
                    .accessibilityAddTraits(.isButton)
                    .disabled(!canSend)
            }
            .padding(.horizontal).padding(.vertical, 8)
          }
          .background(BackplaneDesign.panel)

    }
}
