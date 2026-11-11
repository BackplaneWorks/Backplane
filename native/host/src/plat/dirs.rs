//! The person's folders, in each system's own terms: their home, the models cache and KiCad's settings. Pure functions of the OS and the
//! environment (`*_in`), so every system's answer is tested on every system; the plain functions read this process's.
//! Windows has no HOME and no /tmp: a POSIX path there means one thing to the hub (C:\tmp) and another to Git's shell (%TEMP%), so a value
//! that is not a Windows path (`C:\`, `C:/` or `\\host`) is ignored there, and nothing falls back to a `/tmp` literal (issue 1102).

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Os { Linux, Mac, Windows }
pub const OS: Os = if cfg!(windows) { Os::Windows } else if cfg!(target_os = "macos") { Os::Mac } else { Os::Linux };

/// The environment a function reads: a variable's value, None when unset or empty.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;
pub fn process_env(k: &str) -> Option<String> { std::env::var(k).ok().filter(|v| !v.is_empty()) }

/// A Windows path that names its drive or share: `C:\`, `C:/`, `\\host\share` (not `/tmp`, which each program reads its own way).
pub fn win_abs(p: &str) -> bool { let b = p.as_bytes(); p.starts_with("\\\\") || p.starts_with("//") || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\')) }
fn usable(os: Os, p: &str) -> bool { match os { Os::Windows => win_abs(p), _ => p.starts_with('/') } }
fn var(os: Os, env: Env, k: &str) -> Option<String> { env(k).filter(|v| usable(os, v)) }
/// `parts` under `base`, with the system's separator (a base that ends in one keeps it once).
pub fn join(os: Os, base: &str, parts: &[&str]) -> String {
    let sep = if os == Os::Windows { '\\' } else { '/' };
    let mut s = base.trim_end_matches(['/', '\\']).to_string();
    if s.is_empty() && base.starts_with(['/', '\\']) { s.push(sep) } // the root itself
    for p in parts { if !s.ends_with(sep) { s.push(sep) } s.push_str(p.trim_matches(['/', '\\'])) }
    s
}

/// The person's home folder: HOME; on Windows, where HOME is usually unset (or a POSIX one from Git's shell), USERPROFILE, then HOMEDRIVE+HOMEPATH.
pub fn home_in(os: Os, env: Env) -> Option<String> {
    if let Some(h) = var(os, env, "HOME") { return Some(h) }
    if os != Os::Windows { return None }
    var(os, env, "USERPROFILE").or_else(|| Some(format!("{}{}", env("HOMEDRIVE")?, env("HOMEPATH")?)).filter(|h| win_abs(h)))
}
pub fn home() -> Option<String> { home_in(OS, &process_env) }
/// `home`, or "" (a caller that joins it gets a relative path, never another folder's).
pub fn home_or_empty() -> String { home().unwrap_or_default() }

/// Where converted 3D models are kept: `$XDG_CACHE_HOME/backplane-bend/models` when it is set, else the system's own cache folder:
/// Linux `~/.cache/backplane-bend/models` (XDG), macOS `~/Library/Caches/Backplane/models`, Windows `%LOCALAPPDATA%\Backplane\cache\models`.
/// With no such folder at all, `tmp` (the system's temp folder, `std::env::temp_dir`, native on every system).
pub fn models_cache_in(os: Os, env: Env, tmp: &str) -> String {
    if let Some(x) = var(os, env, "XDG_CACHE_HOME") { return join(os, &x, &["backplane-bend", "models"]) }
    let home = home_in(os, env);
    match os {
        Os::Windows => match var(os, env, "LOCALAPPDATA").or_else(|| home.map(|h| join(os, &h, &["AppData", "Local"]))) {
            Some(l) => join(os, &l, &["Backplane", "cache", "models"]),
            None => join(os, tmp, &["Backplane", "cache", "models"]),
        },
        Os::Mac => match home { Some(h) => join(os, &h, &["Library", "Caches", "Backplane", "models"]), None => join(os, tmp, &["backplane-bend", "models"]) },
        Os::Linux => match home { Some(h) => join(os, &h, &[".cache", "backplane-bend", "models"]), None => join(os, tmp, &["backplane-bend", "models"]) },
    }
}
pub fn models_cache() -> String { models_cache_in(OS, &process_env, &std::env::temp_dir().to_string_lossy()) }

/// The folders KiCad keeps its settings in (`<root>/<version>/kicad_common.json`), in the order they are read: `KICAD_CONFIG_HOME` (KiCad's own
/// override), `$XDG_CONFIG_HOME/kicad`, then the system's: Windows `%APPDATA%\kicad`; macOS `~/Library/Preferences/kicad`; Linux `~/.config/kicad`
/// and Flatpak's. Every system's are listed where it can have them (a Linux path is no use on Windows).
pub fn kicad_config_roots_in(os: Os, env: Env) -> Vec<String> {
    let mut v = vec![];
    if let Some(k) = var(os, env, "KICAD_CONFIG_HOME") { v.push(k) }
    if let Some(x) = var(os, env, "XDG_CONFIG_HOME") { v.push(join(os, &x, &["kicad"])) }
    let home = home_in(os, env);
    match os {
        Os::Windows => {
            if let Some(a) = var(os, env, "APPDATA").or_else(|| home.as_ref().map(|h| join(os, h, &["AppData", "Roaming"]))) { v.push(join(os, &a, &["kicad"])) }
        }
        Os::Mac => if let Some(h) = &home { v.push(join(os, h, &["Library", "Preferences", "kicad"])); v.push(join(os, h, &[".config", "kicad"])) },
        Os::Linux => if let Some(h) = &home { v.push(join(os, h, &[".config", "kicad"])); v.push(join(os, h, &[".var", "app", "org.kicad.KiCad", "config", "kicad"])) },
    }
    v.dedup();
    v
}
pub fn kicad_config_roots() -> Vec<String> { kicad_config_roots_in(OS, &process_env) }

/// The version folder of KiCad's settings a `kicad-cli` belongs to ("10.99" for `.../kicad/10.99.0-bp.6/bin/kicad-cli.exe`, "9.0" for
/// `C:\Program Files\KiCad\9.0\bin\kicad-cli.exe`): its nearest folder named as a version, major.minor. None when no folder says (`/usr/bin`).
pub fn kicad_version_of(cli: &str) -> Option<String> {
    cli.rsplit(['/', '\\']).skip(1).find_map(|seg| {
        let mut it = seg.split(|c: char| !c.is_ascii_digit());
        let (a, b) = (it.next()?, it.next()?);
        let rest = &seg[a.len()..];
        (!a.is_empty() && !b.is_empty() && rest.starts_with('.')).then(|| format!("{a}.{b}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> { move |k| pairs.iter().find(|p| p.0 == k).map(|p| p.1.to_string()).filter(|v| !v.is_empty()) }
    const WIN: &[(&str, &str)] = &[("USERPROFILE", r"C:\Users\Ann Lee"), ("APPDATA", r"C:\Users\Ann Lee\AppData\Roaming"), ("LOCALAPPDATA", r"C:\Users\Ann Lee\AppData\Local")];

    #[test] fn windows_with_no_home_uses_its_own_folders_and_never_tmp() {
        let e = env(WIN);
        assert_eq!(home_in(Os::Windows, &e).as_deref(), Some(r"C:\Users\Ann Lee"));
        assert_eq!(models_cache_in(Os::Windows, &e, r"C:\Users\ANNLEE~1\AppData\Local\Temp"), r"C:\Users\Ann Lee\AppData\Local\Backplane\cache\models");
        assert_eq!(kicad_config_roots_in(Os::Windows, &e), [r"C:\Users\Ann Lee\AppData\Roaming\kicad"]);
        // nothing at all: the system's temp folder as it gave it, not /tmp
        let none = env(&[]);
        let c = models_cache_in(Os::Windows, &none, r"C:\Windows\Temp"); assert_eq!(c, r"C:\Windows\Temp\Backplane\cache\models"); assert!(!c.contains("/tmp"));
    }
    #[test] fn a_posix_path_on_windows_is_ignored_and_an_explicit_windows_one_wins() {
        // Git's shell can leave HOME=/home/x or XDG_CACHE_HOME=/tmp: C:\tmp to the hub, %TEMP% to the shell. Not used.
        let e = env(&[("HOME", "/home/ann"), ("XDG_CACHE_HOME", "/tmp/.cache"), ("XDG_CONFIG_HOME", "/home/ann/.config"), ("USERPROFILE", r"C:\Users\ann"), ("LOCALAPPDATA", r"C:\Users\ann\AppData\Local"), ("APPDATA", r"C:\Users\ann\AppData\Roaming")]);
        assert_eq!(home_in(Os::Windows, &e).as_deref(), Some(r"C:\Users\ann"));
        assert_eq!(models_cache_in(Os::Windows, &e, r"C:\T"), r"C:\Users\ann\AppData\Local\Backplane\cache\models");
        assert_eq!(kicad_config_roots_in(Os::Windows, &e), [r"C:\Users\ann\AppData\Roaming\kicad"]);
        // the workaround in the issue: a native XDG_CACHE_HOME and XDG_CONFIG_HOME are honoured, forward slashes and all
        let e = env(&[("XDG_CACHE_HOME", "C:/Users/ann/AppData/Local/Backplane/cache-diagnostic"), ("XDG_CONFIG_HOME", r"C:\cfg"), ("KICAD_CONFIG_HOME", r"D:\kicad cfg"), ("APPDATA", r"C:\R")]);
        assert_eq!(models_cache_in(Os::Windows, &e, r"C:\T"), r"C:/Users/ann/AppData/Local/Backplane/cache-diagnostic\backplane-bend\models");
        assert_eq!(kicad_config_roots_in(Os::Windows, &e), [r"D:\kicad cfg", r"C:\cfg\kicad", r"C:\R\kicad"]);
        assert_eq!(home_in(Os::Windows, &env(&[("HOMEDRIVE", "C:"), ("HOMEPATH", r"\Users\b")])).as_deref(), Some(r"C:\Users\b"));
    }
    #[test] fn linux_and_macos_keep_xdg_and_their_own_folders() {
        let e = env(&[("HOME", "/home/ann")]);
        assert_eq!(models_cache_in(Os::Linux, &e, "/tmp"), "/home/ann/.cache/backplane-bend/models", "Linux's cache is where it always was");
        assert_eq!(kicad_config_roots_in(Os::Linux, &e), ["/home/ann/.config/kicad", "/home/ann/.var/app/org.kicad.KiCad/config/kicad"]);
        assert_eq!(models_cache_in(Os::Mac, &env(&[("HOME", "/Users/ann")]), "/var/folders/x/T/"), "/Users/ann/Library/Caches/Backplane/models");
        assert_eq!(kicad_config_roots_in(Os::Mac, &env(&[("HOME", "/Users/ann")])), ["/Users/ann/Library/Preferences/kicad", "/Users/ann/.config/kicad"]);
        let e = env(&[("HOME", "/home/ann"), ("XDG_CACHE_HOME", "/c"), ("XDG_CONFIG_HOME", "/cfg/")]);
        assert_eq!(models_cache_in(Os::Linux, &e, "/tmp"), "/c/backplane-bend/models");
        assert_eq!(kicad_config_roots_in(Os::Linux, &e)[0], "/cfg/kicad");
        // no HOME: the temp folder the system gives, never a relative path
        assert_eq!(models_cache_in(Os::Linux, &env(&[]), "/tmp/"), "/tmp/backplane-bend/models");
        assert_eq!(models_cache_in(Os::Mac, &env(&[]), "/var/folders/x/T/"), "/var/folders/x/T/backplane-bend/models");
        assert!(kicad_config_roots_in(Os::Linux, &env(&[("XDG_CONFIG_HOME", "rel")])).is_empty(), "a relative folder is no folder");
        assert_eq!(join(Os::Linux, "/", &["a"]), "/a");
    }
    #[test] fn a_kicad_clis_folder_names_its_settings_version() {
        assert_eq!(kicad_version_of(r"C:\Users\a\AppData\Local\Backplane\kicad\10.99.0-bp.6\bin\kicad-cli.exe").as_deref(), Some("10.99"));
        assert_eq!(kicad_version_of(r"C:\Program Files\KiCad\9.0\bin\kicad-cli.exe").as_deref(), Some("9.0"));
        assert_eq!(kicad_version_of("/home/a/.local/share/backplane/kicad/10.99.0-bp.4/bin/kicad-cli").as_deref(), Some("10.99"));
        assert_eq!(kicad_version_of("/usr/bin/kicad-cli"), None);
        assert_eq!(kicad_version_of("/opt/kicad10/bin/kicad-cli"), None, "a name with a number is no version");
    }
}
