//! API keys: one `<home>/secrets/api.env`, 0600 in a 0700 folder, a
//! `NAME='value'` line each. Only names ever leave this file: a client gets the names, an agent the names and the path.
use std::{io::Write, path::{Path, PathBuf}};

fn up(c: char) -> char { if c.is_ascii_lowercase() { c.to_ascii_uppercase() } else { c } }
fn word(c: char) -> bool { c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' }

/// Keys.name: upper case, anything but a letter, digit or `_` made `_`, never starting with a digit; at most 64 characters.
pub fn name(raw: &str) -> String {
    let n: String = raw.trim().chars().take(64).map(|c| { let u = up(c); if word(u) { u } else { '_' } }).collect();
    if n.starts_with(|c: char| c.is_ascii_digit()) { format!("_{n}") } else { n }
}

/// Keys.quote: single quotes, each `'` written `'\''`, line breaks dropped.
pub fn quote(v: &str) -> String {
    let mut o = String::from("'");
    for c in v.trim().chars() { match c { '\'' => o.push_str("'\\''"), '\n' | '\r' => {} _ => o.push(c) } }
    o.push('\''); o
}

pub fn line(n: &str, v: &str) -> String { format!("{}={}", name(n), quote(v)) }

/// Keys.of: the name a line sets ("" for a blank line, a comment, or anything else).
pub fn of(l: &str) -> String {
    let t = l.trim();
    let mut parts = t.split('=');
    match (parts.next(), parts.next()) { (Some(n), Some(_)) if !n.is_empty() && name(n) == n => n.to_string(), _ => String::new() }
}

/// String.lines: split on line breaks.
fn lines(t: &str) -> Vec<String> { t.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect() }

/// Keys.names: the names the file sets, in its order.
pub fn names(text: &str) -> Vec<String> { lines(text).iter().map(|l| of(l)).filter(|n| !n.is_empty()).collect() }

fn keep(ls: Vec<String>) -> Vec<String> { ls.into_iter().filter(|l| !l.trim().is_empty()).collect() }
fn text(ls: &[String]) -> String { if ls.is_empty() { String::new() } else { format!("{}\n", ls.join("\n")) } }

/// Keys.drop: the file without the name's line (every other line kept, blank ones dropped).
pub fn drop_key(t: &str, n: &str) -> String { text(&keep(lines(t).into_iter().filter(|l| of(l) != n).collect())) }

/// Keys.set: the name set to value (an old line for it replaced).
pub fn set(t: &str, n: &str, v: &str) -> String {
    let k = name(n);
    let mut ls = keep(lines(t).into_iter().filter(|l| of(l) != k).collect());
    ls.push(line(n, v));
    text(&ls)
}

/// Keys.bad: why a new key cannot be kept ("" when it can).
pub fn bad(n: &str, v: &str) -> String {
    if name(n).is_empty() { "Give the key a name".into() }
    else if n.trim().chars().count() > 64 { "Use at most 64 characters for the key name".into() }
    else if n != name(n) { format!("Use the environment variable name: {}", name(n)) }
    else if v.trim().is_empty() { "Paste the key itself".into() }
    else { String::new() }
}

/// The values the file holds (`NAME='value'`, `'\''` for a quote), for the mask (vault.js bp_vault_api).
pub fn values(t: &str) -> Vec<String> {
    let mut out = vec![];
    for l in t.split('\n') {
        let l = l.trim_end();
        let Some((k, rest)) = l.split_once('=') else { continue };
        if k.is_empty() || !k.chars().all(word) { continue }
        let Some(inner) = rest.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) else { continue };
        out.push(inner.replace("'\\''", "'"));
    }
    out
}

fn shq(p: &str) -> String { format!("'{}'", p.replace('\'', "'\\''")) }

/// Keys.prompt: the `<api_keys>` block agents get (nothing when there are none). `names` is one name a line.
pub fn prompt(path: &str, names_text: &str) -> String {
    let ns = keep(lines(names_text));
    if ns.is_empty() { return String::new() }
    let list = ns.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ");
    let first = ns.first().cloned().unwrap_or_else(|| "NAME".into());
    format!("<api_keys>The user keeps API keys for you in `{path}` (`NAME='value'` lines): {list}. \
Use a key without ever seeing it: load the file in the same shell command that needs it, e.g. `set -a; . {q}; set +a; curl -H \"Authorization: Bearer ${first}\" ...`, or in a script read it from the environment that command gives it (`os.environ[\"NAME\"]`). \
Never print, echo, cat, grep or log the file or a key's value, never write a value into a file, a commit, a message or a tool's arguments, and never pass it on the command line where it would show. If a command's output could show a key, cut it off or redact it before it reaches you. \
To see which names exist now (the user may add some while you work): `sed -n 's/=.*//p' {q}`. When a task needs a key that is not there, ask the user to add it in Backplane's Settings, API keys, and say which name to give it.</api_keys>", q = shq(path))
}

pub fn path(home: &Path) -> PathBuf { home.join("secrets").join("api.env") }

fn read(home: &Path) -> String { std::fs::read_to_string(path(home)).unwrap_or_default() }

/// Ak.write: the secrets folder (0700) and the file (0600), written through a temporary file and renamed.
fn write(home: &Path, text: &str) -> Result<(), String> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let dir = home.join("secrets");
    std::fs::create_dir_all(&dir).map_err(|_| "error: the keys file could not be written".to_string())?;
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    let tmp = dir.join(format!("api.env.{}.tmp", std::process::id()));
    let w = (|| -> std::io::Result<()> {
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        f.write_all(text.as_bytes())?; f.sync_all()?;
        std::fs::rename(&tmp, path(home))
    })();
    if w.is_err() { let _ = std::fs::remove_file(&tmp); return Err("error: the keys file could not be written".into()) }
    super::mask::reload_api(); // masked from this instant, not from the cache's next read
    Ok(())
}

/// Ak.names: the names kept, one a line.
pub fn names_kept(home: &Path) -> String { names(&read(home)).join("\n") }
/// Ak.add: keep a key under its name; Err("error: why") when it cannot be kept.
pub fn add(home: &Path, n: &str, v: &str) -> Result<(), String> {
    let b = bad(n, v);
    if !b.is_empty() { return Err(format!("error: {b}")) }
    write(home, &set(&read(home), n, v))
}
/// Ak.del: forget the key of that name.
pub fn del(home: &Path, n: &str) -> Result<(), String> { write(home, &drop_key(&read(home), n)) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn keys_name_env() { assert_eq!(name(" openai key "), "OPENAI_KEY"); }
    #[test] fn keys_name_digit() { assert_eq!(name("2captcha"), "_2CAPTCHA"); }
    #[test] fn keys_quote_single() { assert_eq!(line("a", "it's"), "A='it'\\''s'"); }
    #[test] fn keys_set_replaces() { assert_eq!(set("A='1'\nB='2'\n", "a", "3"), "B='2'\nA='3'\n"); }
    #[test] fn keys_drop_keeps_others() { assert_eq!(drop_key("# mine\nA='1'\nB='2'\n", "A"), "# mine\nB='2'\n"); }
    #[test] fn keys_names_of_lines() { assert_eq!(names("# note\nA='x=y'\n\nB='2'\n"), vec!["A", "B"]); }
    #[test] fn keys_prompt_none() { assert_eq!(prompt("/h/secrets/api.env", ""), ""); }
    #[test] fn keys_prompt_load_quotes_path() { assert!(prompt("/a'b folder/api.env", "TOKEN").contains("set -a; . '/a'\\''b folder/api.env'; set +a")); }
    #[test] fn keys_prompt_list_quotes_path() { assert!(prompt("/a'b folder/api.env", "TOKEN").contains("sed -n 's/=.*//p' '/a'\\''b folder/api.env'")); }
    #[test] fn bad_names_are_refused_with_the_reason() {
        assert_eq!(bad("", "x"), "Give the key a name"); assert_eq!(bad("openai key", "x"), "Use the environment variable name: OPENAI_KEY");
        assert_eq!(bad("A", "  "), "Paste the key itself"); assert_eq!(bad("A", "v"), "");
    }
    #[test] fn values_are_read_back_with_their_quotes() { assert_eq!(values("A='it'\\''s'\n# c\nB='2'\nlower='x'\n"), vec!["it's", "2"]); }
    #[test] fn a_key_is_kept_under_0600_and_only_its_name_is_listed() {
        use std::os::unix::fs::PermissionsExt;
        let h = std::env::temp_dir().join(format!("bp-keys-{}", std::process::id())); let _ = std::fs::remove_dir_all(&h);
        add(&h, "TOKEN_A", "s3cret-value").unwrap(); add(&h, "TOKEN_B", "other-value-1").unwrap(); add(&h, "TOKEN_A", "replaced-val").unwrap();
        assert_eq!(names_kept(&h), "TOKEN_B\nTOKEN_A");
        assert_eq!(std::fs::metadata(path(&h)).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(h.join("secrets")).unwrap().permissions().mode() & 0o777, 0o700);
        del(&h, "TOKEN_B").unwrap(); assert_eq!(names_kept(&h), "TOKEN_A");
        assert!(add(&h, "bad name", "v").is_err());
        let _ = std::fs::remove_dir_all(&h);
    }
}
