//! The ordered entries for a thread's right rail.
use serde_json::{json, Value};

#[derive(Default)]
pub struct RailInput<'a> {
    pub board: bool,
    pub schematic: bool,
    pub mech: bool,
    pub gerbers: bool,
    pub simulation: bool,
    pub altium: bool,
    pub page: &'a str,
    pub diff: bool,
}

pub fn rail(input: RailInput<'_>) -> Value {
    let mut entries = Vec::new();
    let page = if input.diff { "diff" } else { input.page };
    let mut add = |id, label, icon, group, key, why: Option<&str>| {
        let state = if page == id { "on" } else if why.is_some() { "disabled" } else { "off" };
        let mut entry = json!({ "id": id, "label": label, "icon": icon, "group": group, "key": key, "state": state });
        if let Some(why) = why.filter(|_| page != id) { entry["why"] = json!(why); }
        entries.push(entry);
    };
    let no_board = (!input.board).then_some("No board in this project");
    add("board", "Board", "circuit-board", "design", "b", no_board);
    add("schematic", "Schematic", "cpu", "design", "s", (!input.schematic).then_some("No schematic in this project"));
    add("3d", "3D", "box", "design", "3", no_board);
    if input.gerbers || page == "gerbers" { add("gerbers", "Gerbers", "layers", "design", "g", None); }
    if input.simulation && input.schematic && !input.altium { add("simulation", "Simulation", "gauge", "design", "i", None); }
    add("split", "Split", "layout-panel-left", "layout", "v", (!(input.board && input.schematic)).then_some("Split needs a board and a schematic"));
    add("mech", "Mech", "wrench", "layout", "m", (!input.mech).then_some("No mechanical parts in this project"));
    add("files", "Files", "folder", "work", "f", None);
    add("diff", "Diff", "diff", "work", "d", None);
    add("review", "Review", "circle-check", "work", "r", None);
    add("browser", "Browser", "globe", "work", "w", None);
    add("terminal", "Terminal", "terminal", "tools", "`", None);
    add("commands", "Commands", "search", "tools", "", None);
    Value::Array(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_availability_and_selection() {
        for board in [false, true] { for schematic in [false, true] { for mech in [false, true] {
            for page in ["", "board", "schematic", "3d", "split", "mech", "files", "diff", "review", "browser", "terminal", "commands"] {
                for diff in [false, true] {
                    let rows = rail(RailInput { board, schematic, mech, page, diff, ..Default::default() });
                    let ids = ["board", "schematic", "3d", "split", "mech", "files", "diff", "review", "browser", "terminal", "commands"];
                    let enabled = [board, schematic, board, board && schematic, mech, true, true, true, true, true, true];
                    assert_eq!(rows.as_array().unwrap().len(), ids.len());
                    for ((row, id), enabled) in rows.as_array().unwrap().iter().zip(ids).zip(enabled) {
                        assert_eq!(row["id"], id);
                        assert_eq!(row["state"], if id == if diff { "diff" } else { page } { "on" } else if !enabled { "disabled" } else { "off" });
                        assert_eq!(row.get("why").is_some(), row["state"] == "disabled");
                    }
                }
            }
        }}}
    }

    #[test]
    fn every_rail_action_has_a_label_for_its_tooltip_and_palette_an_icon_and_a_distinct_key() {
        let rows = rail(RailInput { board: true, schematic: true, mech: true, gerbers: true, simulation: true, ..Default::default() });
        let mut keys = std::collections::HashSet::new();
        for row in rows.as_array().unwrap() {
            for field in ["id", "label", "icon"] {
                assert!(row[field].as_str().is_some_and(|s| !s.is_empty()), "{row} lacks {field}");
            }
            // Commands opens the palette with its configurable platform shortcut.
            let key = if row["id"] == "commands" { "palette" } else { row["key"].as_str().unwrap() };
            assert!(!key.is_empty() && keys.insert(key), "{row}");
        }
    }

    #[test]
    fn rail_optional_design_entries() {
        for gerbers in [false, true] { for simulation in [false, true] { for altium in [false, true] { for schematic in [false, true] {
            for page in ["gerbers", "simulation"] {
                let rows = rail(RailInput { gerbers, simulation, altium, schematic, page, ..Default::default() });
                let optional: Vec<_> = rows.as_array().unwrap().iter().skip(3).take_while(|r| r["group"] == "design").collect();
                let expected: Vec<_> = [("gerbers", gerbers || page == "gerbers"), ("simulation", simulation && schematic && !altium)].into_iter().filter_map(|(id, show)| show.then_some(id)).collect();
                assert_eq!(optional.len(), expected.len());
                for (row, id) in optional.iter().zip(expected) {
                    assert_eq!(row["id"], id);
                    assert_eq!(row["state"], if id == page { "on" } else { "off" });
                }
            }
        }}}}
    }
}
