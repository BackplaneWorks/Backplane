//! Build the header from the displayed page and verified source facts.
use super::{files_trail, s};
use serde_json::{json, Value};

/// Normalize separators on behalf of Windows peers too. Accept an absolute path
/// only when it names a descendant of this particular project.
pub(super) fn relative(root: &str, path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let root = root.replace('\\', "/");
    let rel = path.strip_prefix(&format!("{}/", root.trim_end_matches('/')))
        .filter(|_| !root.is_empty()).unwrap_or(&path);
    if rel.is_empty() || rel.split('/').any(|p| p.is_empty() || matches!(p,"."|"..") || p.contains(':') || p.chars().any(char::is_control)) { return None }
    Some(rel.to_owned())
}

pub(super) struct Sources<'a> {
    pub root: &'a str, pub board: &'a str, pub schematic: &'a str,
    pub file: &'a str, pub sheet: &'a str, pub directory: &'a str,
    pub set: &'a str, pub made_at: Option<u64>,
}

pub(super) fn identity(page: &str, viewer: &Value, sources: &Sources, exists: &impl Fn(&str) -> Option<bool>) -> Value {
    let label = match page {
        "board"=>"Board", "schematic"=>"Schematic", "3d"=>"3D", "gerbers"=>"Gerbers",
        "split"=>"Split", "mech"=>"Mech", "files"=>"Files", "review"=>"Review", "diff"=>"Diff",
        "image"=>"Image", "pdf"=>"PDF", "text"=>"Text", "browser"=>"Browser", "simulation"=>"Simulation", _=>return Value::Null,
    };
    let mut out = json!({"page":page,"label":label});
    let path = match page {
        "board"|"3d"|"split"=>sources.board, "schematic"|"simulation"=>sources.schematic,
        "gerbers"=>sources.set, "mech"=>s(&viewer["mech"], "rel"), "files"=>sources.directory,
        "image"|"pdf"|"text"=>sources.file, _=>"",
    };
    if page == "files" {
        let dir = relative(sources.root, path);
        out["trail"] = json!(files_trail(dir.as_deref().unwrap_or("")));
    }
    if !matches!(page,"review"|"diff"|"browser") && !(page=="files" && path.is_empty()) {
        match relative(sources.root,path) {
            Some(rel) => match exists(&rel) {
                Some(true) => {
                    out["source"] = json!({"rel":rel, "name":rel.rsplit('/').next()});
                    match page {
                        "schematic" if !sources.sheet.is_empty() => out["sheet"] = json!(rel),
                        "mech" => out["part"] = json!(rel),
                        "gerbers" => out["set"] = json!({"rel":rel,"madeAt":sources.made_at}),
                        _ => {}
                    }
                }
                Some(false) => out["missing"] = json!(true),
                None => {}
            },
            None => out["missing"] = json!(true),
        }
    }
    match page {
        "split" => out["panes"] = json!(["schematic","board","3d"].map(|p| identity(p,viewer,sources,exists))),
        "diff" => out["scope"] = json!("Thread changes"),
        "review" => {
            let versions = &viewer["designPanels"]["bom"]["versions"];
            if viewer["review"]["tabs"].as_array().into_iter().flatten().any(|t| t["page"] == "bom" && t["on"] == true) && versions.is_array() {
                let sides: Vec<_> = versions.as_array().unwrap().iter().map(|v| v.as_str().filter(|s| !s.is_empty()).unwrap_or("Working copy")).collect();
                out["scope"] = json!(sides.join(" → "));
            } else { out["scope"] = json!("Working copy"); }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_sources_are_relative_and_match_each_pane() {
        let facts = Sources {root:"/design",board:"/design/alternate/main.kicad_pcb",schematic:"/design/child/main.kicad_sch",
            sheet:"child/main.kicad_sch",file:"docs/top.pdf",directory:"docs",set:"mfg/one",made_at:Some(123)};
        let viewer=json!({"mech":{"rel":"mech/part.step"},"review":{"tabs":[{"page":"bom","on":true}]},"designPanels":{"bom":{"versions":["a","b"]}}});
        let id=|page|identity(page,&viewer,&facts,&|_|Some(true));
        for (page,path) in [("board","alternate/main.kicad_pcb"),("3d","alternate/main.kicad_pcb"),("schematic","child/main.kicad_sch"),
            ("simulation","child/main.kicad_sch"),("files","docs"),("text","docs/top.pdf"),("image","docs/top.pdf"),("pdf","docs/top.pdf"),("mech","mech/part.step")] {
            assert_eq!(id(page)["source"]["rel"],path);
        }
        assert_eq!(id("files")["trail"][1]["value"],"docs");
        assert_eq!(id("split")["panes"][0],id("schematic"));
        assert_eq!(id("split")["panes"][1],id("board"));
        assert_eq!(id("split")["panes"][2],id("3d"));
        assert_eq!(id("review")["scope"],"a → b");
        assert_eq!(id("diff")["scope"],"Thread changes");
        assert_eq!(id("browser"),json!({"page":"browser","label":"Browser"}));
        assert_eq!(id("gerbers")["set"],json!({"rel":"mfg/one","madeAt":123}));
        let missing=identity("board",&viewer,&facts,&|_|Some(false));
        assert!(missing["source"].is_null() && missing["missing"]==true);
        for bad in ["/outside/design", "/design-alt/file", "../local", "C:\\folder\\file"] {assert!(relative(facts.root,bad).is_none());}
        assert_ne!(relative("/p","a/x.sch"),relative("/p","b/x.sch"));
    }
}
