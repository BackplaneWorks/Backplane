//! Slash commands in the composer (src/core/slash.bend, pure; laws `slash_*`): a draft that is one word starting with `/` is a command being typed;
//! the composer offers the commands whose name starts with it. Backplane's own come first (any provider), then Claude's (only in a Claude thread).

/// `(name, about)`: the commands every thread has.
pub const OWN: [(&str, &str); 3] = [
    ("/btw", "Ask a side question; the thread never sees it"),
    ("/give", "Hand this conversation to a >thread, %project or @bot"),
    ("/secret", "Give an agent a key or token in a masked box, never in the chat"),
];
/// The commands only a Claude thread has.
pub const CLAUDE: [(&str, &str); 2] = [("/compact", "Summarize the conversation to free context"), ("/context", "Show what fills the context window")];

/// Refs.sep: the characters that end a word (space, line feed, tab, carriage return).
fn sep(c: char) -> bool { matches!(c, ' ' | '\n' | '\t' | '\r') }

/// Refs.token: the word being typed, the draft's last, after its last separator ("" when the draft ends with one).
pub fn token(draft: &str) -> &str { draft.rsplit(sep).next().unwrap_or("") }

/// Slash.typing: the draft is one word, starting with "/", no separator yet.
pub fn typing(draft: &str) -> bool { draft.starts_with('/') && token(draft) == draft }

/// Slash.cmds: the commands whose name starts with "/" + `q`, in table order.
pub fn cmds(claude: bool, q: &str) -> Vec<(&'static str, &'static str)> {
    let p = format!("/{q}");
    OWN.iter().chain(if claude { CLAUDE.iter() } else { [].iter() }).filter(|(n, _)| n.starts_with(&p)).copied().collect()
}
pub fn names(cs: &[(&str, &str)]) -> Vec<String> { cs.iter().map(|(n, _)| n.to_string()).collect() }

#[cfg(test)]
mod tests {
    use super::*;
    // laws slash_* (LAWS.bend): the same statements
    #[test] fn slash_typing_word() { assert!(typing("/gi")) }
    #[test] fn slash_typing_alone() { assert!(typing("/")) }
    #[test] fn slash_not_after_space() { assert!(!typing("/give >a")) }
    #[test] fn slash_not_midline() { assert!(!typing("hi /gi")) }
    #[test] fn slash_typing_needs_slash() { assert!(!typing("give")) }
    #[test] fn slash_claude_prefix() { assert_eq!(names(&cmds(true, "c")), ["/compact", "/context"]) }
    #[test] fn slash_others_own_only() { assert_eq!(names(&cmds(false, "")), ["/btw", "/give", "/secret"]) }
    #[test] fn slash_none_unknown() { assert!(cmds(true, "zz").is_empty()) }
    // Refs.last.go: a separator resets the word, so a trailing one leaves "", tabs and carriage returns count
    #[test] fn token_is_the_last_word() {
        assert_eq!(token("a b"), "b"); assert_eq!(token("a b "), ""); assert_eq!(token("a\nb"), "b"); assert_eq!(token("a\tb\r"), ""); assert_eq!(token(""), "");
    }
}
