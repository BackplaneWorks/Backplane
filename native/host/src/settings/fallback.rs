//! The fallback chain (core/fallback.bend): one ordered, opt-in list of `provider/model` per hub. Never wraps: an exhausted suffix blocks until a
//! person intervenes. Ported here: parsing, validity against the installed models, editing and picking. The outage classifier
//! (`Error.classify`, which reads provider error envelopes) belongs with the agents' turn handling and is not ported.
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Target { pub provider: String, pub model: String }
impl Target {
    pub fn key(&self) -> String { format!("{}/{}", self.provider, self.model) }
    /// Target.parse: the provider is the text before the first `/`, the model everything after it.
    pub fn parse(s: &str) -> Target { let mut p = s.split('/'); let provider = p.next().unwrap_or("").to_string(); Target { provider, model: p.collect::<Vec<_>>().join("/") } }
}

/// Chain.parse
pub fn parse(s: &str) -> Vec<Target> { if s.is_empty() { vec![] } else { s.split(',').map(Target::parse).collect() } }
/// Chain.show
pub fn show(xs: &[Target]) -> String { xs.iter().map(Target::key).collect::<Vec<_>>().join(",") }
pub fn has(xs: &[Target], key: &str) -> bool { xs.iter().any(|t| t.key() == key) }

/// Caps.valid: the provider lists models and the target names one of them (or none).
pub fn caps_valid(caps: &Value, t: &Target) -> bool {
    let xs = caps.get(&t.provider).and_then(Value::as_array).cloned().unwrap_or_default();
    !xs.is_empty() && (t.model.is_empty() || xs.iter().any(|m| m.as_str() == Some(t.model.as_str())))
}
/// Chain.valid: every target is offered, none twice.
pub fn valid(xs: &[Target], caps: &Value) -> bool { xs.iter().enumerate().all(|(i, t)| caps_valid(caps, t) && !has(&xs[i + 1..], &t.key())) }

/// Chain.remove / Chain.up (the Settings lists' edits).
pub fn remove(xs: &[Target], key: &str) -> Vec<Target> { xs.iter().filter(|t| t.key() != key).cloned().collect() }
pub fn up(xs: &[Target], key: &str) -> Vec<Target> { let mut v = xs.to_vec(); if let Some(i) = v.iter().position(|t| t.key() == key) { if i > 0 { v.swap(i, i - 1) } } v }
/// Chain.suffix: what follows `key` when it is in the chain, else the whole chain.
pub fn suffix(xs: &[Target], key: &str) -> Vec<Target> { match xs.iter().position(|t| t.key() == key) { Some(i) => xs[i + 1..].to_vec(), None => xs.to_vec() } }

#[cfg(test)]
mod tests {
    use super::*; use serde_json::json;
    fn caps() -> Value { json!({"claude":["opus","sonnet"],"codex":["gpt-6"],"grok":[]}) }
    #[test] fn a_target_is_a_provider_and_everything_after_the_first_slash() { assert_eq!(Target::parse("openai/gpt/5"), Target { provider: "openai".into(), model: "gpt/5".into() }); assert_eq!(Target::parse("claude").key(), "claude/"); }
    #[test] fn chains_parse_and_show() { let c = parse("claude/opus,codex/gpt-6"); assert_eq!(c.len(), 2); assert_eq!(show(&c), "claude/opus,codex/gpt-6"); assert!(parse("").is_empty()); }
    #[test] fn a_chain_is_valid_against_what_is_installed_and_names_each_target_once() {
        assert!(valid(&parse("claude/opus,codex/gpt-6"), &caps())); assert!(valid(&parse("claude/"), &caps()));
        assert!(!valid(&parse("claude/nope"), &caps()), "unknown model"); assert!(!valid(&parse("grok/x"), &caps()), "a provider with no models"); assert!(!valid(&parse("muse/x"), &caps()), "unknown provider");
        assert!(!valid(&parse("claude/opus,claude/opus"), &caps()), "twice");
    }
    #[test] fn editing_moves_and_removes() {
        let c = parse("a/1,b/2,c/3"); assert_eq!(show(&up(&c, "c/3")), "a/1,c/3,b/2"); assert_eq!(show(&up(&c, "a/1")), "a/1,b/2,c/3"); assert_eq!(show(&remove(&c, "b/2")), "a/1,c/3");
        assert_eq!(show(&suffix(&c, "a/1")), "b/2,c/3"); assert_eq!(show(&suffix(&c, "x/9")), "a/1,b/2,c/3");
    }

    #[test] fn fallback_never_wraps() { assert_eq!(show(&suffix(&parse("claude/sonnet,codex/gpt-6.1-sol"), "codex/gpt-6.1-sol")), ""); }
}
