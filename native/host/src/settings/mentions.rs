//! The `&` model mention (a mention like `@`, `>` and `%`, refs.bend's pattern): `&claude/opus` names a provider/model. It is resolved when a message is sent against the
//! hub's `models.all` rows and checked, never acted on: it does NOT change the thread's provider or model. Each reference is `{word, provider, model, state}` with
//! state `ok`, `unknown` (not a provider or model this hub lists) or `unavailable` (listed, but `ok: false` now). The first `/` splits provider from model, so
//! `&opencode/openai/gpt-5` is provider `opencode`, model `openai/gpt-5`. The agent's `<references>` block gets one structured line per distinct reference.
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct ModelRef { pub word: String, pub provider: String, pub model: String, pub state: &'static str }
impl ModelRef {
    pub fn json(&self) -> Value { json!({"word":self.word,"provider":self.provider,"model":self.model,"state":self.state}) }
    /// The agent's `<references>` line.
    pub fn line(&self) -> String { format!("model {}/{}: {}", self.provider, self.model, match self.state { "ok" => "usable", "unavailable" => "listed but not usable now", _ => "not a model this hub lists" }) }
}
fn slug(c: char) -> bool { c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '+') }

/// The `&word`s of a text, in order: a `&` that begins a word (start of text or after whitespace/an opening bracket or quote) then slug characters containing a `/`
/// (a bare `&sonnet` has no provider: it is `unknown`, shown so, never guessed). Trailing `.`, `,`, `:`, `/` are punctuation, not the id.
pub fn words(text: &str) -> Vec<String> {
    let cs: Vec<char> = text.chars().collect(); let mut out: Vec<String> = vec![]; let mut i = 0;
    while i < cs.len() {
        let start = i == 0 || cs[i - 1].is_whitespace() || matches!(cs[i - 1], '(' | '[' | '{' | '"' | '\'' | '`' | ',');
        if cs[i] == '&' && start {
            let mut j = i + 1; while j < cs.len() && slug(cs[j]) { j += 1 }
            while j > i + 1 && matches!(cs[j - 1], '.' | ',' | ':' | '/') { j -= 1 }
            if j > i + 1 { let w: String = cs[i..j].iter().collect(); if !out.contains(&w) { out.push(w) } }
            i = j.max(i + 1);
        } else { i += 1 }
    }
    out
}

/// Resolve each `&word` against the `models.all` JSON (`{provider: [{value, ok, ..}]}`).
pub fn resolve(text: &str, all: &str) -> Vec<ModelRef> {
    let all: Value = serde_json::from_str(all).unwrap_or(Value::Null);
    words(text).into_iter().map(|w| {
        let id = &w[1..];
        let (provider, model) = id.split_once('/').map(|(p, m)| (p.to_string(), m.to_string())).unwrap_or((String::new(), id.to_string()));
        let row = all[&provider].as_array().and_then(|rows| rows.iter().find(|r| r["value"].as_str() == Some(model.as_str())));
        let state = match row { None => "unknown", Some(r) if r["ok"].as_bool().unwrap_or(false) => "ok", Some(_) => "unavailable" };
        ModelRef { word: w, provider, model, state }
    }).collect()
}

/// The text's `<references>` block for its model mentions ("" when it has none); agents appends it with the other references.
pub fn block(refs: &[ModelRef]) -> String { if refs.is_empty() { String::new() } else { format!("\n\n<references>\n{}\n</references>", refs.iter().map(ModelRef::line).collect::<Vec<_>>().join("\n")) } }

#[cfg(test)]
mod tests {
    use super::*;
    fn all() -> String { json!({"claude":[{"value":"sonnet","ok":true},{"value":"opus","ok":true}],"codex":[{"value":"gpt-6.1-sol","ok":false}],"opencode":[{"value":"openai/gpt-5","ok":true}]}).to_string() }
    #[test] fn words_are_ampersand_slugs_that_begin_a_word() {
        assert_eq!(words("use &claude/sonnet for the lanes and &claude/opus, then (&codex/gpt-6.1-sol)."), ["&claude/sonnet", "&claude/opus", "&codex/gpt-6.1-sol"]);
        assert_eq!(words("a&b rock&roll && &"), Vec::<String>::new(), "mid-word and bare ampersands are text");
        assert_eq!(words("&claude/opus &claude/opus"), ["&claude/opus"], "once each");
        assert_eq!(words("see &opencode/openai/gpt-5."), ["&opencode/openai/gpt-5"], "the first slash splits, the last dot is punctuation");
    }
    #[test] fn each_reference_is_ok_unknown_or_unavailable_and_none_changes_a_thread() {
        let r = resolve("use &claude/sonnet, &codex/gpt-6.1-sol, &claude/nope, &sonnet, &opencode/openai/gpt-5 and &zed/x", &all());
        let s: Vec<(&str, &str, &str)> = r.iter().map(|m| (m.provider.as_str(), m.model.as_str(), m.state)).collect();
        assert_eq!(s, [("claude","sonnet","ok"), ("codex","gpt-6.1-sol","unavailable"), ("claude","nope","unknown"), ("","sonnet","unknown"), ("opencode","openai/gpt-5","ok"), ("zed","x","unknown")]);
        assert_eq!(r[0].json(), json!({"word":"&claude/sonnet","provider":"claude","model":"sonnet","state":"ok"}));
    }
    #[test] fn the_agent_block_has_one_structured_line_each() {
        assert_eq!(block(&resolve("&claude/opus &codex/gpt-6.1-sol", &all())), "\n\n<references>\nmodel claude/opus: usable\nmodel codex/gpt-6.1-sol: listed but not usable now\n</references>");
        assert_eq!(block(&resolve("nothing here", &all())), "");
    }
}
