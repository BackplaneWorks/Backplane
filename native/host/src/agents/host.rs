//! What the hub learned of this machine that an agent's prompt and arguments name: FreeCAD, the hub's own binary (the renderer),
//! the KiCad runtime, the models line, KiStack's folder, the bundled skills' folder. Discovery proper belongs to the files/kicad and settings streams;
//! `probe` reads the environment and PATH as the JS hub's boot does, and tests give their own.
use super::prompt::Kicad;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct HostInfo { pub home: PathBuf, pub user_home: String, pub fc: String, pub render: String, pub kicad: Kicad, pub models: Value, pub kistack: Option<PathBuf>, pub bundled: Option<PathBuf> }

impl Default for HostInfo {
    fn default() -> HostInfo { HostInfo { home: PathBuf::new(), user_home: "/nonexistent".into(), fc: String::new(), render: String::new(), kicad: Kicad { status: "missing".into(), ..Default::default() }, models: json!({"claude":["fable","opus","sonnet","haiku"]}), kistack: None, bundled: None } }
}
fn on_path(name: &str) -> Option<PathBuf> { std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|c| c.is_file())) }

/// The first of `rel` under /Applications and ~/Applications that is a file (macOS app bundles), else "".
fn mac_app(rel: &str) -> String {
    let home = crate::plat::dirs::home_or_empty();
    [format!("/Applications/{rel}"), format!("{home}/Applications/{rel}")].into_iter().find(|p| Path::new(p).is_file()).unwrap_or_default()
}

impl HostInfo {
    pub fn probe(home: &Path) -> HostInfo {
        let mut h = HostInfo { home: home.to_path_buf(), user_home: crate::plat::dirs::home().unwrap_or_else(|| "/nonexistent".into()), render: std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(), ..Default::default() };
        h.fc = std::env::var("BACKPLANE_FREECAD").ok().filter(|s| !s.is_empty()).or_else(|| on_path("freecadcmd").map(|p| p.to_string_lossy().into_owned())).unwrap_or_else(|| mac_app("FreeCAD.app/Contents/Resources/bin/freecadcmd"));
        if let Some(cli) = std::env::var("BACKPLANE_KICAD_CLI").ok().filter(|s| !s.is_empty()).map(PathBuf::from).or_else(|| on_path("kicad-cli")).or_else(|| ["KiCad/KiCad.app/Contents/MacOS/kicad-cli", "KiCad.app/Contents/MacOS/kicad-cli"].iter().map(|r| mac_app(r)).find(|p| !p.is_empty()).map(PathBuf::from)) {
            let out = crate::proc::std_cmd(crate::proc::Kind::Local, &cli).arg("--version").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok();
            h.kicad = match out { Some(o) if o.status.success() => { let v = String::from_utf8_lossy(&o.stdout).trim().to_string(); Kicad { status: if v.contains("-bp.") { "ok" } else { "stock" }.into(), cli: cli.to_string_lossy().into_owned(), version: v, commit: std::env::var("BACKPLANE_KICAD_COMMIT").unwrap_or_default(), why: String::new() } }
                _ => Kicad { status: "broken".into(), cli: cli.to_string_lossy().into_owned(), why: "it does not run".into(), ..Default::default() } };
        }
        if let Some(m) = std::env::var("BACKPLANE_MODELS").ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) { h.models = m }
        h.kistack = std::env::var("BACKPLANE_KISTACK").ok().map(PathBuf::from).or_else(|| std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("kistack")))).filter(|p| p.is_dir());
        h.bundled = super::bundled::install(home); // Backplane's own skills, in this home (bundled.rs)
        super::skills::set_env(super::skills::env_from(&h.user_home, home, h.kistack.as_deref()));
        h
    }
    /// Cfg.mtext: "claude (a, b); codex (c)"
    pub fn models_text(&self) -> String { self.models.as_object().map(|o| o.iter().filter(|(_, v)| v.as_array().map_or(false, |a| !a.is_empty())).map(|(k, v)| format!("{k} ({})", v.as_array().unwrap().iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("; ")).unwrap_or_default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_models_line_lists_providers_that_have_models() { let h = HostInfo { models: json!({"claude":["a","b"],"codex":[],"grok":["g"]}), ..Default::default() }; assert_eq!(h.models_text(), "claude (a, b); grok (g)"); }
}
