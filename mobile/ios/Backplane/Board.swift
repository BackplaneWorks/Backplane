import SwiftUI

// The task board (core/tbdata.bend, client.bend's BoardUi; the screen is
// src/mobile/screen.bend's Mob.board): the filters as chips, the sections
// with their rows, a field to add an item. A row's leading button ticks it
// on, a tap opens its detail, a swipe marks it Done or Drops it. All of it
// is the screen's; these views only draw it and send each action back. It
// is the hub in focus's board, and the screen's values carry that hub's
// key like every other action.

struct BoardLink: Decodable, Hashable {
    let label, action, value: String
}

struct BoardRow: Decodable, Identifiable, Hashable {
    let id, title, sub, status: String
    let soon, overdue, open: Bool
    let notes: String
    let links: [BoardLink]
    // an open row's: Blocked, Drop and Delete; the title and notes to edit;
    // Rename and Save notes
    let buttons: [SetButton]?
    let fields: [SetField]?
    let save: [SetButton]?
}

struct BoardSection: Decodable, Identifiable, Hashable {
    let title: String
    let count: Int
    let rows: [BoardRow]
    var id: String { title }
}

struct BoardModel: Decodable {
    let title: String
    let open: Int
    let filters: [SetButton]
    let sections: [BoardSection]
    let empty: String
    let add: SetField
    let addAction, closeAction: String
}

private func statusIcon(_ s: String) -> String {
    switch s {
    case "doing": "square.lefthalf.filled"
    case "done": "checkmark.square"
    case "blocked": "exclamationmark.square"
    case "dropped": "minus.square"
    default: "square"
    }
}

private func statusTint(_ s: String) -> Color {
    switch s {
    case "doing": PhaseColor.accent
    case "done": PhaseColor.ok
    case "blocked": PhaseColor.warn
    default: PhaseColor.faint
    }
}

// a field of an open row: typed text goes to the client as "bfield"
private struct BoardField: View {
    let model: AppModel
    let field: SetField
    @State private var text = ""

    var body: some View {
        TextField(field.hint, text: $text, axis: .vertical)
            .lineLimit(1...5)
            .textFieldStyle(.roundedBorder)
            .onAppear { text = field.value }
            .onChange(of: text) { _, t in model.field(field.name, t) }
    }
}

private struct BoardButtons: View {
    let model: AppModel
    let buttons: [SetButton]

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(buttons, id: \.self) { b in
                    if b.on {
                        Button(b.label) { model.act(b.action, b.value) }.buttonStyle(.borderedProminent).tint(b.label == "Delete?" ? .red : nil)
                    } else {
                        Button(b.label) { model.act(b.action, b.value) }.buttonStyle(.bordered)
                    }
                }
            }
        }
    }
}

private struct BoardRowView: View {
    let model: AppModel
    let row: BoardRow

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top, spacing: 12) {
                Button { model.act("tb-tick", row.id) } label: {
                    Image(systemName: statusIcon(row.status)).font(.title3).foregroundStyle(statusTint(row.status)).frame(width: 28, height: 28)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Move \(row.title) on")
                Button { model.act("tb-row", row.id) } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(row.title).foregroundStyle(row.status == "done" || row.status == "dropped" ? Color.secondary : Color.primary).lineLimit(row.open ? nil : 2)
                        if !row.sub.isEmpty {
                            Text(row.sub).font(.caption).foregroundStyle(row.overdue ? PhaseColor.bad : row.soon ? PhaseColor.warn : Color.secondary).lineLimit(1)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            if row.open {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(row.fields ?? [], id: \.self) { f in BoardField(model: model, field: f) }
                    if let s = row.save, !s.isEmpty { BoardButtons(model: model, buttons: s) }
                    ForEach(row.links, id: \.self) { l in
                        Button { model.act(l.action, l.value) } label: {
                            Text(l.label).font(.footnote).foregroundStyle(PhaseColor.accent).lineLimit(2).multilineTextAlignment(.leading)
                        }
                        .buttonStyle(.plain)
                    }
                    if let b = row.buttons, !b.isEmpty { BoardButtons(model: model, buttons: b) }
                }
                .padding(.leading, 40)
            }
        }
        .padding(.vertical, 4)
    }
}

struct BoardSheet: View {
    let model: AppModel
    let board: BoardModel
    @State private var text = ""
    @FocusState private var adding: Bool

    private func add() {
        guard !text.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        model.field(board.add.name, text)
        model.act(board.addAction)
        text = ""
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    HStack {
                        TextField(board.add.hint, text: $text)
                            .focused($adding)
                            .submitLabel(.done)
                            .onSubmit(add)
                            .onChange(of: text) { _, t in model.field(board.add.name, t) }
                        Button("Add", action: add).disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 6) {
                            ForEach(board.filters, id: \.self) { b in
                                if b.on {
                                    Button(b.label) { model.act(b.action, b.value) }.buttonStyle(.borderedProminent)
                                } else {
                                    Button(b.label) { model.act(b.action, b.value) }.buttonStyle(.bordered)
                                }
                            }
                        }
                    }
                }
                ForEach(board.sections) { s in
                    Section {
                        ForEach(s.rows) { r in
                            BoardRowView(model: model, row: r)
                                .swipeActions(edge: .trailing) {
                                    Button("Done") { model.act("tb-status", r.id + "\t" + "done") }.tint(PhaseColor.ok)
                                    Button("Drop") { model.act("tb-status", r.id + "\t" + "dropped") }.tint(.gray)
                                }
                        }
                    } header: {
                        HStack(spacing: 6) {
                            Text(s.title)
                            Text("\(s.count)").monospacedDigit()
                        }
                        .textCase(.uppercase)
                    }
                }
                if board.sections.isEmpty {
                    Section { Text(board.empty).foregroundStyle(.secondary) }
                }
            }
            .navigationTitle("\(board.title) · \(board.open) open")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { model.act(board.closeAction) } }
            }
            .onAppear { text = board.add.value }
        }
    }
}
