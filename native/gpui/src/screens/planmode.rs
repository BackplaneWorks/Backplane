//! Thread Plan Mode controls and visual cues (Claude threads, plus any thread holding an open plan ask):
//! - Topbar status badge (`top_mark`)
//! - Slim banner above the composer (`bar`) showing phase and switching `1 turn` / `Always` / `Off`
//! - Compact composer toolbar chip (`chip`) with click cycling + Shift+Tab shortcut
//! - Composer border tone (`composer_border`) and primary button styling (`send_button`)
//!
//! All state comes straight from the hub's `thread.planMode` snapshot (`native/host/src/present/keys.rs`).
use crate::{hub::Command, i18n::trs, screens::{attach::raw_id, settings::Secret}, string, theme, ui::{ic, with_rect}, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase { Off, Armed, Planning, Pending, Executing }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanMode {
    pub phase: Phase,
    pub interaction: bool,
    pub once: bool,
    pub pending: bool,
}

/// Parse `thread["planMode"]`; returns `None` when not applicable (e.g., non-Claude threads with no plan ask).
pub fn parse(v: &Value) -> Option<PlanMode> {
    if !v.is_object() { return None }
    let phase = match v["phase"].as_str().unwrap_or("") {
        "armed" => Phase::Armed,
        "planning" => Phase::Planning,
        "pending" => Phase::Pending,
        "executing" => Phase::Executing,
        _ => Phase::Off,
    };
    Some(PlanMode {
        phase,
        interaction: v["interaction"] == "plan",
        once: v["once"] == true,
        pending: v["pending"] == true,
    })
}

pub fn active(m: &PlanMode) -> bool { m.phase != Phase::Off }

/// Color token for Plan Mode surfaces: lavender (`input`) for planning/review, green (`ok`) while building.
pub fn accent_token(m: &PlanMode) -> &'static str {
    match m.phase {
        Phase::Executing => "ok",
        Phase::Off => "line",
        _ => "input",
    }
}

/// Composer box border color token driven by the thread's Plan Mode phase.
pub fn composer_border(thread: &Value) -> &'static str {
    match parse(&thread["planMode"]) {
        Some(m) if matches!(m.phase, Phase::Armed | Phase::Planning | Phase::Pending) => "input",
        Some(m) if m.phase == Phase::Executing => "ok",
        _ => "accent",
    }
}

/// Primary send button label and background color token.
pub fn send_button(thread: &Value, voice_flashing: bool) -> (String, &'static str) {
    let base = super::sidebox::send_label(thread);
    if voice_flashing { return (base, "ok") }
    if let Some(m) = parse(&thread["planMode"]) {
        if matches!(m.phase, Phase::Armed | Phase::Planning | Phase::Pending) && base == "Send" {
            return (trs("Plan"), "input");
        }
        if m.phase == Phase::Executing {
            return (trs(&base), "ok");
        }
    }
    (trs(&base), "accent")
}

/// Next cycle action when toggling Plan Mode from composer chip or Shift+Tab:
/// `!interaction` -> `1 turn` (`thread.planonce`), `once` -> `Always` (`thread.modes plan`), else -> `Off` (`thread.modes default`).
pub fn cycle_rpc(th: &str, m: &PlanMode) -> Option<(&'static str, Value)> {
    if th.is_empty() { return None }
    Some(match (m.interaction, m.once) {
        (false, _) => ("thread.planonce", json!({"thread": th, "on": true})),
        (true, true) => ("thread.modes", json!({"thread": th, "interaction": "plan"})),
        (true, false) => ("thread.modes", json!({"thread": th, "interaction": "default"})),
    })
}

pub fn send_rpc(this: &mut Backplane, method: &'static str, params: Value, cx: &mut Context<Backplane>) {
    let _ = this.tx.try_send(Command::Rpc(method.into(), Secret(params), "planmode"));
    cx.notify();
}

/// Cycle Plan Mode on the active thread (Shift+Tab in Composer or toolbar chip click).
pub fn cycle(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let thread = &this.data["thread"];
    let Some(m) = parse(&thread["planMode"]) else { return };
    let th = this.selected.clone();
    if let Some((method, params)) = cycle_rpc(&th, &m) { send_rpc(this, method, params, cx); }
}

/// Inline status mark next to the thread title in the top bar while Plan Mode is active or executing.
pub fn top_mark(light: bool, thread: &Value) -> Option<AnyElement> {
    let m = parse(&thread["planMode"])?;
    if !active(&m) { return None }
    let c = |n: &str| theme::color(light, n);
    let tone = accent_token(&m);
    let (icon, label) = match m.phase {
        Phase::Armed if m.once => ("file-text", trs("Plan (1 turn)")),
        Phase::Armed => ("file-text", trs("Plan")),
        Phase::Planning => ("file-text", trs("Planning")),
        Phase::Pending => ("file-text", trs("Review plan")),
        Phase::Executing => ("play", trs("Building plan")),
        Phase::Off => return None,
    };
    Some(div().id("planmode-top").flex().items_center().gap(px(4.))
        .child(div().text_color(c("faint")).child("/"))
        .child(div().id("planmode-top-pill").flex().items_center().gap(px(4.)).px(px(5.)).py(px(1.)).border_1().border_color(c(tone)).bg(c("panel"))
            .text_size(crate::scale::tpx(11.)).text_color(c(tone))
            .tooltip(crate::keys::tip("Plan before acting (1 turn, always, or off)", "", light))
            .child(ic(light, icon, 11., tone))
            .child(label))
        .into_any_element())
}

/// Compact Plan Mode chip in the bottom composer bar. Only shown when `thread.planMode` is present.
pub fn chip(cx: &mut Context<Backplane>, light: bool, thread: &Value, key_hint: &str) -> Option<AnyElement> {
    let m = parse(&thread["planMode"])?;
    let c = |n: &str| theme::color(light, n);
    let on = active(&m);
    let tone = accent_token(&m);
    let (icon, label) = match m.phase {
        Phase::Off => ("file-text", trs("Plan")),
        Phase::Executing => ("play", trs("Building")),
        Phase::Pending => ("file-text", trs("Review")),
        _ if m.once => ("file-text", trs("Plan · 1t")),
        _ => ("file-text", trs("Plan")),
    };
    let label_color = if on { tone } else { "dim" };
    /* i18n-ok: ctl id */ let el = with_rect(crate::screens::bar::fit(div().id("plan-chip")), || "ui:Plan mode".to_string())
        .flex_shrink_0().cursor_pointer().px_2().py_1().border_1()
        .border_color(c(if on { tone } else { "line" }))
        .bg(c(if on { "raise" } else { "panel" }))
        .hover(|s| s.bg(c("raise")))
        .text_size(crate::scale::tpx(12.)).flex().items_center().gap(px(5.))
        .text_color(c(label_color))
        .tooltip(crate::keys::tip("Plan before acting (1 turn, always, or off)", key_hint, light))
        .child(ic(light, icon, 13., label_color))
        .child(div().whitespace_nowrap().child(label))
        .on_click(cx.listener(|this, _, _, cx| cycle(this, cx)));
    Some(el.into_any_element())
}

fn scope_btn(cx: &mut Context<Backplane>, light: bool, id: &'static str, label: String, on: bool, th: String, method: &'static str, once_val: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().id(id).px(px(6.)).py(px(1.)).cursor_pointer().text_size(crate::scale::tpx(11.)).border_1()
        .border_color(c(if on { "input" } else { "line" }))
        .bg(c(if on { "raise" } else { "panel" }))
        .text_color(c(if on { "input" } else { "dim" }))
        .hover(|s| s.bg(c("raise")).text_color(c("text")))
        .child(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            let tid = if th.is_empty() { this.selected.clone() } else { th.clone() };
            if tid.is_empty() { return }
            let params = if method == "thread.planonce" {
                json!({"thread": tid, "on": once_val})
            } else {
                json!({"thread": tid, "interaction": "plan"})
            };
            send_rpc(this, method, params, cx);
        })).into_any_element()
}

/// Slim banner above the composer while Plan Mode is active (`armed`, `planning`, `pending`, or `executing`).
pub fn bar(cx: &mut Context<Backplane>, light: bool, thread: &Value) -> Option<AnyElement> {
    let m = parse(&thread["planMode"])?;
    if !active(&m) { return None }
    let th = raw_id(&string(thread, "id")).to_string();
    let c = |n: &str| theme::color(light, n);
    let tone = accent_token(&m);
    let (icon, msg) = match m.phase {
        Phase::Armed if m.once => ("file-text", trs("Plan mode (1 turn) — explores and proposes a plan before editing")),
        Phase::Armed => ("file-text", trs("Plan mode — explores and proposes a plan before editing")),
        Phase::Planning => ("file-text", trs("Planning — drafting a plan before edits")),
        Phase::Pending => ("file-text", trs("Proposed plan awaits review below")),
        Phase::Executing => ("play", trs("Building the approved plan")),
        Phase::Off => return None,
    };
    let mut row = div().id("planmode-bar").w_full().flex().items_center().gap_2().px_3().py_1().border_1()
        .border_color(c(tone)).bg(c("panel")).text_size(crate::scale::tpx(12.))
        .child(ic(light, icon, 13., tone))
        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("text")).child(msg));
    if matches!(m.phase, Phase::Armed | Phase::Planning) {
        let th_off = th.clone();
        row = row
            .child(scope_btn(cx, light, "pm-once", trs("1 turn"), m.once, th.clone(), "thread.planonce", true))
            .child(scope_btn(cx, light, "pm-always", trs("Always"), !m.once, th, "thread.modes", false))
            .child(div().id("pm-off").px(px(6.)).py(px(1.)).cursor_pointer().text_size(crate::scale::tpx(11.)).border_1()
                .border_color(c("line")).text_color(c("dim")).hover(|s| s.bg(c("raise")).text_color(c("text")))
                .child(trs("Off"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let tid = if th_off.is_empty() { this.selected.clone() } else { th_off.clone() };
                    if !tid.is_empty() { send_rpc(this, "thread.modes", json!({"thread": tid, "interaction": "default"}), cx); }
                })));
    }
    Some(row.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn parses_phases_and_drives_composer_visuals() {
        assert_eq!(parse(&Value::Null), None);
        let t_off = json!({"planMode": {"phase": "off", "interaction": "default", "once": false}, "send": "Send", "sendAct": "send"});
        assert_eq!(composer_border(&t_off), "accent");
        assert_eq!(send_button(&t_off, false), ("Send".into(), "accent"));
        assert_eq!(cycle_rpc("t1", &parse(&t_off["planMode"]).unwrap()), Some(("thread.planonce", json!({"thread": "t1", "on": true}))));

        let t_once = json!({"planMode": {"phase": "armed", "interaction": "plan", "once": true}, "send": "Send", "sendAct": "send"});
        let m = parse(&t_once["planMode"]).unwrap();
        assert!(active(&m) && m.once && m.interaction);
        assert_eq!(composer_border(&t_once), "input");
        assert_eq!(send_button(&t_once, false), ("Plan".into(), "input"));
        assert_eq!(cycle_rpc("t1", &m), Some(("thread.modes", json!({"thread": "t1", "interaction": "plan"}))));

        let t_always = json!({"planMode": {"phase": "planning", "interaction": "plan", "once": false}, "send": "Send", "sendAct": "send"});
        assert_eq!(cycle_rpc("t1", &parse(&t_always["planMode"]).unwrap()), Some(("thread.modes", json!({"thread": "t1", "interaction": "default"}))));

        // Pending after a 1-turn plan where interaction already returned to default: clicking chip/Shift+Tab re-arms 1 turn
        let t_pend = json!({"planMode": {"phase": "pending", "interaction": "default", "once": false, "pending": true}, "send": "Send", "sendAct": "send"});
        let mp = parse(&t_pend["planMode"]).unwrap();
        assert!(active(&mp) && mp.pending && !mp.interaction);
        assert_eq!(cycle_rpc("t1", &mp), Some(("thread.planonce", json!({"thread": "t1", "on": true}))));

        let t_exec = json!({"planMode": {"phase": "executing", "interaction": "default", "once": false}, "send": "Send", "sendAct": "send"});
        assert_eq!(composer_border(&t_exec), "ok");
        assert_eq!(send_button(&t_exec, false), ("Send".into(), "ok"));
    }

    #[test]
    fn icon_names_used_exist_in_bundle() {
        let icons: serde_json::Map<String, Value> = serde_json::from_str(include_str!("../../../../assets/icons.json")).unwrap();
        for name in ["file-text", "play"] { assert!(icons.contains_key(name), "missing icon {name}"); }
    }
}
