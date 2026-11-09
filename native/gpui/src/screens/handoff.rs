//! The handoff card (`/give`): what a thread received, drawn from the hub's entry `{kind:"handoff", text, title, from, note, refs}`.
use crate::{hub::Command, string, theme, ui::ic, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

/// What a click on a handoff card's reference asks of the window: `(action, value)` for `Command::Action`, decided here so the card holds no rule. A thread opens by its link; a message
/// opens its thread (the message is marked once it is there); a board opens on Board (Schematic for a `.kicad_sch`) and any other file on Files, in the thread the card is in.
pub fn open(r: &Value) -> Option<(&'static str, String)> {
    let (kind, url, path) = (string(r, "kind"), string(r, "url"), string(r, "path"));
    match kind.as_str() {
        "thread" | "message" if !url.is_empty() => Some(("link-open", url.split('#').next().unwrap_or("").to_string())),
        "board" => Some(("view", if path.to_ascii_lowercase().ends_with(".kicad_sch") { "schematic" } else { "board" }.to_string())),
        "file" => Some(("view", "files".to_string())),
        _ => None,
    }
}

/// A handoff received (`/give`): where it came from (a way back), the summary, the note, and what the work touched as cards that open it.
pub fn card(cx: &mut Context<Backplane>, light: bool, e: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let id = string(e, "id");
    let (title, from, note, summary) = (string(e, "title"), string(e, "from"), string(e, "note"), string(e, "text"));
    let back = div().id(SharedString::from(format!("ho-back-{id}"))).flex().items_center().gap_2().text_size(px(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
        .child(ic(light, "chevron-left", 12., "dim")).child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(crate::i18n::tf("gpui.handoff.from", &[("title", title.clone().into())])))
        .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("link-open".into(), format!("backplane://thread/{from}"))); cx.notify(); }));
    let mut refs = div().flex().flex_wrap().gap_2().mt_1();
    for (i, r) in e["refs"].as_array().into_iter().flatten().enumerate() {
        let (kind, label) = (string(r, "kind"), string(r, "label"));
        let mark = match kind.as_str() { "thread" => "\u{25b8}", "message" => "\u{201c}", "board" => "\u{25a6}", _ => "\u{00b7}" };
        let mut card = div().id(SharedString::from(format!("ho-ref-{id}-{i}"))).flex().items_center().gap_2().px_2().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.))
            .child(div().text_color(c("accent")).font_weight(FontWeight::BOLD).child(mark)).child(div().max_w(px(260.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(label));
        if let Some((act, val)) = open(r) {
            let msg = string(r, "msg"); let thread = string(r, "thread");
            card = card.cursor_pointer().hover(|s| s.bg(c("raise"))).on_click(cx.listener(move |this, _, _, cx| {
                let _ = this.tx.try_send(Command::Action(act.into(), val.clone()));
                if !msg.is_empty() { this.screens.find.thread = thread.clone(); this.find_go(msg.clone()); }
                cx.notify();
            }));
        }
        refs = refs.child(card);
    }
    div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().child(
        div().w_full().max_w(px(640.)).px_3().py_2().border_1().border_color(c("line")).bg(c("raise")).flex().flex_col().gap_1().child(back)
            .child(div().text_size(px(13.)).child(summary))
            .children((!note.is_empty()).then(|| div().text_size(px(12.)).text_color(c("dim")).child(crate::i18n::tf("gpui.handoff.note", &[("note", note.clone().into())]))))
            .child(refs)).into_any_element()
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[core::prelude::v1::test] fn a_handoffs_references_open_what_they_name() {
        let t = |v: Value| open(&v);
        assert_eq!(t(json!({"kind":"thread","url":"backplane://thread/t1"})), Some(("link-open", "backplane://thread/t1".to_string())));
        assert_eq!(t(json!({"kind":"message","url":"backplane://thread/t1#m3"})), Some(("link-open", "backplane://thread/t1".to_string())), "the thread; the message is marked once it is there");
        assert_eq!(t(json!({"kind":"board","path":"a/x.kicad_pcb"})), Some(("view", "board".to_string())));
        assert_eq!(t(json!({"kind":"board","path":"a/x.KICAD_SCH"})), Some(("view", "schematic".to_string())));
        assert_eq!(t(json!({"kind":"file","path":"a.md"})), Some(("view", "files".to_string())));
        assert_eq!(t(json!({"kind":"model"})), None);
    }
}
