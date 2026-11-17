//! What a code span or block is, for the copy button every client draws on it: a shell command (copy, and Run in terminal), a path, a web
//! address, a commit hash or an id (copy). Pure; the nodes carry the answer as `copy` (node_json), so the three clients agree.

/// Languages a fenced block names when it holds commands to type.
const SHELLS: &[&str] = &["sh", "bash", "shell", "zsh", "fish", "ksh", "shellscript", "powershell", "pwsh", "ps1", "ps", "cmd", "bat", "batch", "nu", "nushell"];

/// Programs whose name opens a command line. A line that starts with one of these (or a path to a program) reads as a command.
const PROGRAMS: &[&str] = &[
    "sudo", "doas", "apt", "apt-get", "apt-cache", "dpkg", "dnf", "yum", "rpm", "pacman", "yay", "zypper", "apk", "brew", "port", "snap", "flatpak", "nix", "nix-shell",
    "winget", "choco", "scoop", "pip", "pip3", "pipx", "uv", "uvx", "poetry", "conda", "mamba", "npm", "npx", "pnpm", "yarn", "bun", "bunx", "deno", "node", "python", "python3",
    "py", "ruby", "gem", "bundle", "go", "cargo", "rustup", "rustc", "make", "cmake", "ninja", "meson", "gcc", "clang", "git", "gh", "glab", "cd", "ls", "cat", "less",
    "head", "tail", "mkdir", "rmdir", "rm", "cp", "mv", "ln", "touch", "chmod", "chown", "chgrp", "find", "grep", "rg", "sed", "awk", "xargs", "tee", "echo", "printf",
    "export", "source", "env", "which", "whereis", "type", "curl", "wget", "ssh", "scp", "rsync", "sftp", "tar", "unzip", "zip", "gzip", "gunzip", "xz", "7z", "docker",
    "podman", "docker-compose", "kubectl", "helm", "systemctl", "journalctl", "service", "loginctl", "usermod", "useradd", "groupadd", "passwd", "kill", "killall", "ps", "top",
    "htop", "df", "du", "free", "lsblk", "lsusb", "lspci", "mount", "umount", "ip", "ping", "dig", "nslookup", "nc", "tailscale", "ufw", "iptables", "open", "xdg-open",
    "code", "vim", "nvim", "nano", "emacs", "tmux", "screen", "watch", "time", "nohup", "kicad-cli", "kicad", "freecad", "freecadcmd", "backplane", "backplane-hub",
    "openocd", "esptool", "esptool.py", "idf.py", "west", "platformio", "pio", "arduino-cli", "avrdude", "dfu-util", "picotool", "probe-rs", "minicom", "screen",
    "xcode-select", "xcodebuild", "defaults", "launchctl", "codesign", "spctl", "softwareupdate", "Set-ExecutionPolicy", "Get-ChildItem", "Invoke-WebRequest", "iwr", "irm",
    "adb", "fastboot", "gradle", "./gradlew", "mvn", "java", "javac", "dotnet", "terraform", "aws", "gcloud", "az", "fly", "vercel", "wrangler", "claude", "codex", "bash",
    "sh", "zsh", "fish", "pwsh", "powershell", "cmd", "reg", "setx", "net", "sc", "wsl", "ldconfig", "update-alternatives", "modprobe", "dmesg", "udevadm", "chsh",
];

/// A fenced block's text is a command to type: it names a shell, or names no language and every line of it reads as a command line
/// (comments, blank lines and the continuations of a line ending in `\` aside). A block in another language (rust, json...) is code, not a command.
pub fn block_is_command(text: &str, lang: &str) -> bool {
    let lang = lang.trim().to_ascii_lowercase();
    if SHELLS.contains(&lang.as_str()) { return !text.trim().is_empty() }
    if !(lang.is_empty() || lang == "text" || lang == "txt" || lang == "plain") { return false }
    let mut any = false; let mut cont = false;
    for l in text.lines() {
        let t = l.trim();
        if cont { cont = t.ends_with('\\'); continue }
        if t.is_empty() || t.starts_with('#') { continue }
        if !line_is_command(t) { return false }
        any = true; cont = t.ends_with('\\');
    }
    any
}

/// One line that starts the way a command does: `VAR=x` assignments, then a known program or a path to one (`./run.sh`, `~/bin/x`,
/// `/usr/bin/x`, `scripts/land.sh`), with a `$ ` prompt allowed in front. Prose ("Install the tools") is not.
pub fn line_is_command(line: &str) -> bool {
    let t = line.trim().strip_prefix("$ ").unwrap_or(line.trim());
    let mut words = t.split_whitespace().skip_while(|w| assignment(w)).peekable();
    let Some(first) = words.next() else { return false };
    if PROGRAMS.contains(&first) { return true }
    let pathy = first.starts_with("./") || first.starts_with("../") || first.starts_with("~/") || (first.starts_with('/') && first[1..].contains('/'))
        || (first.contains('/') && (first.ends_with(".sh") || first.ends_with(".py") || first.ends_with(".ps1")));
    pathy && first.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '~' | '+'))
}
fn assignment(w: &str) -> bool {
    let Some((k, _)) = w.split_once('=') else { return false };
    !k.is_empty() && k.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// What an inline code span is to copy: "cmd" (a command line of two words or more: `sudo apt-get install -y x`, `git status`), "url",
/// "path", "hash" (a commit's 7-40 hex digits with a digit and a letter), "id" (one word of letters, digits and `-_.:` with a digit in it,
/// such as `t1794855320-614` or `0.16.1`); None for anything else (a name, a word, a snippet of code), which gets no button.
pub fn inline_kind(text: &str) -> Option<&'static str> {
    let t = text.trim();
    if t.is_empty() || t.contains('\n') || t.chars().count() > 2000 { return None }
    if t.split_whitespace().count() >= 2 && line_is_command(t) { return Some("cmd") }
    if t.contains(char::is_whitespace) { return None }
    if t.starts_with("http://") || t.starts_with("https://") || t.starts_with("backplane://") { return Some("url") }
    if t.starts_with('/') || t.starts_with("~/") || t.starts_with("./") || t.starts_with("../") || (t.len() > 2 && t.as_bytes()[1] == b':' && matches!(t.as_bytes()[2], b'\\' | b'/'))
        || (t.contains('/') && !t.contains("://") && t.chars().any(|c| c.is_ascii_alphabetic())) { return Some("path") }
    let hex = t.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
    if hex && (7..=40).contains(&t.len()) && t.chars().any(|c| c.is_ascii_digit()) && t.chars().any(|c| c.is_ascii_alphabetic()) { return Some("hash") }
    if t.len() >= 3 && t.chars().any(|c| c.is_ascii_digit()) && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '@' | '~' | '|'))
        && t.chars().any(|c| c.is_ascii_alphanumeric()) { return Some("id") }
    None
}

/// The pieces of a plain text (a room's post) to draw copy buttons for: each fenced block whole, and each inline code span with a kind,
/// in order, as {text, cmd} (cmd: Run in terminal too). At most 16.
pub fn spans(text: &str) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut rest = text;
    while out.len() < 16 {
        let Some(i) = rest.find('`') else { break };
        let after = &rest[i..];
        if let Some(body) = after.strip_prefix("```") {
            let (info, code) = body.split_once('\n').unwrap_or((body, ""));
            let Some(end) = code.find("```") else { break };
            let block = code[..end].trim_end_matches('\n');
            let lang = info.split(|c: char| c.is_whitespace() || c == ',' || c == '{').next().unwrap_or("");
            if !block.trim().is_empty() { out.push(serde_json::json!({"text": block, "cmd": block_is_command(block, lang)})); }
            rest = &code[end + 3..]; continue
        }
        let inner = &after[1..];
        let Some(end) = inner.find(|c| c == '`' || c == '\n') else { break };
        if inner.as_bytes()[end] == b'`' {
            let t = &inner[..end];
            if let Some(k) = inline_kind(t) { out.push(serde_json::json!({"text": t, "cmd": k == "cmd"})); }
            rest = &inner[end + 1..];
        } else { rest = &inner[end..]; }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_plain_posts_copyable_pieces() {
        let v = spans("Run `sudo apt-get install -y rootlesskit slirp4netns` then see `foo()` and:\n```bash\nmake\nmake install\n```\nhash `3552c7cfd`.");
        assert_eq!(v, vec![serde_json::json!({"text": "sudo apt-get install -y rootlesskit slirp4netns", "cmd": true}), serde_json::json!({"text": "make\nmake install", "cmd": true}),
            serde_json::json!({"text": "3552c7cfd", "cmd": false})]);
        assert!(spans("no code here").is_empty());
        assert!(spans("an `unclosed span").is_empty());
    }
    #[test] fn fenced_blocks_that_hold_commands() {
        assert!(block_is_command("sudo apt-get install -y rootlesskit slirp4netns", ""));
        assert!(block_is_command("cd native/host\ncargo test --offline", "text"));
        assert!(block_is_command("# set up\nexport A=1\n\nscripts/land.sh", ""));
        assert!(block_is_command("docker run \\\n  --rm \\\n  img", ""));
        assert!(block_is_command("anything at all", "bash"));
        assert!(block_is_command("Get-ChildItem", "powershell"));
        assert!(block_is_command("$ git status", ""));
        assert!(block_is_command("RUST_LOG=debug ./target/fast/backplane-hub --foreground", ""));
        assert!(!block_is_command("", "sh"));
        assert!(!block_is_command("fn main() {}", "rust"));
        assert!(!block_is_command("git status", "rust"));
        assert!(!block_is_command("{\"a\": 1}", ""));
        assert!(!block_is_command("git status\nOn branch main", ""));      // a command and its output: not something to run as it is
        assert!(!block_is_command("Install the tools first.", ""));
        assert!(!block_is_command("# only a comment", ""));
    }
    #[test] fn inline_spans_by_kind() {
        assert_eq!(inline_kind("sudo apt-get install -y rootlesskit slirp4netns"), Some("cmd"));
        assert_eq!(inline_kind("git status"), Some("cmd"));
        assert_eq!(inline_kind("scripts/land.sh --dry"), Some("cmd"));
        assert_eq!(inline_kind("git"), None);                               // a program's name alone is a word
        assert_eq!(inline_kind("https://example.com/a"), Some("url"));
        assert_eq!(inline_kind("~/.backplane/home"), Some("path"));
        assert_eq!(inline_kind("/usr/bin/env"), Some("path"));
        assert_eq!(inline_kind("native/host/src/md/copy.rs"), Some("path"));
        assert_eq!(inline_kind("C:\\Users\\h"), Some("path"));
        assert_eq!(inline_kind("3552c7cfd"), Some("hash"));
        assert_eq!(inline_kind("8e41de6a2c1f0a9b8e41de6a2c1f0a9b8e41de6a"), Some("hash"));
        assert_eq!(inline_kind("t1794855320-614"), Some("id"));
        assert_eq!(inline_kind("0.16.1"), Some("id"));
        assert_eq!(inline_kind("Copy"), None);
        assert_eq!(inline_kind("foo()"), None);
        assert_eq!(inline_kind("a b"), None);
        assert_eq!(inline_kind("deadbeef"), None);                          // hex letters alone: a word
        assert_eq!(inline_kind(""), None);
    }
}
