//! Review mode as a thread shows it (#1305): the setting that holds it and the bar over the message box. Pure, so the presenter and the
//! screen audit draw it without the hub's agents (snapguard.rs decides and enforces it).
use serde_json::{json, Value};

/// The setting that says a thread's turns are reviews (read-only): "1" while they are; "0" the person switched it off (the bar stays, to switch it
/// back on, until their next message); "" not a review.
pub fn review_key(th: &str) -> String { format!("review.ro.{th}") }
/// Why a thread is read-only, as the thread shows it.
pub const REVIEW_WHY: &str = "Read-only: this request looked like a review, so Backplane refuses changes to the design files.";
/// The bar over the message box for the setting's value: what it says, its one button and the value that button sends (`thread.review {on}`); null
/// when the thread is no review and the person did not just switch one off.
pub fn bar(value: &str) -> Value {
    match value {
        "1" => json!({"on": true, "text": REVIEW_WHY, "button": "Allow edits", "tip": "Switch review mode off: the agent may change the design files", "value": false}),
        "0" => json!({"on": false, "text": "Edits allowed: you switched review mode off for this thread.", "button": "Read-only again", "tip": "Switch review mode back on: the design files are read-only again", "value": true}),
        _ => Value::Null,
    }
}
