//! The Context & Prompt inspector. Every row comes from the thread's hub.
use crate::{hub::Command, i18n::trs, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use super::{accounts::button, settings::Secret};

#[derive(Default)]
pub struct Inspector {
    thread: String,
    reply: Value,
    tab: usize,
    expanded: String,
    busy: bool,
    error: String,
    scroll: ScrollHandle,
}

pub fn open(app: &mut Backplane, thread: String, cx: &mut Context<Backplane>) {
    app.screens.thread_context = Inspector { thread, ..Default::default() };
    request(app, None, cx);
}

fn request(app: &mut Backplane, action: Option<(&str, &Value)>, cx: &mut Context<Backplane>) {
    let state = &mut app.screens.thread_context;
    state.busy = true;
    state.error.clear();
    let (method, params) = match action {
        Some((action, row)) => ("thread.context.mcp", json!({"thread":state.thread,"action":action,"path":row["path"],"sha256":row["sha256"]})),
        None => ("thread.context", json!({"thread":state.thread})),
    };
    if app.tx.try_send(Command::Rpc(method.into(), Secret(params), "thread-context")).is_err() {
        state.busy = false;
        state.error = trs("Could not reach the hub.");
    }
    cx.notify();
}

pub fn got(app: &mut Backplane, reply: Value, cx: &mut Context<Backplane>) {
    let state = &mut app.screens.thread_context;
    if state.thread.is_empty() { return }
    if reply["ok"] == true {
        let thread = reply["result"]["thread"].as_str();
        if Some(state.thread.as_str()) != thread && state.thread.rsplit('~').next() != thread { return }
        state.reply = reply["result"].clone();
        state.error.clear();
    } else { state.error = super::settings::reply_error(&reply); }
    state.busy = false;
    cx.notify();
}

pub fn close(app: &mut Backplane, cx: &mut Context<Backplane>) -> bool {
    if app.screens.thread_context.thread.is_empty() { return false }
    app.screens.thread_context = Inspector::default();
    cx.notify();
    true
}

fn rows<'a>(data: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    data[key].as_array().into_iter().flatten()
}

fn code(text: String, light: bool) -> Div {
    div().p_3().bg(theme::color(light, "raise")).text_size(px(12.)).child(text)
}

fn badge(text: &str, light: bool) -> Div {
    let tone = match text { "Native" | "Enabled" => "accent", "Invalid" | "Untrusted" => "warn", _ => "dim" };
    div().flex_shrink_0().px_2().py_1().bg(theme::color(light, "raise")).text_color(theme::color(light, tone)).text_size(px(11.)).child(trs(text))
}

pub fn view(state: &Inspector, cx: &mut Context<Backplane>, light: bool, viewport: Size<Pixels>) -> Option<AnyElement> {
    if state.thread.is_empty() { return None }
    let c = |k: &str| theme::color(light, k);
    let data = &state.reply;
    let loaded = data.is_object();
    let mut tabs = div().flex().gap_1().px_4().pb_3();
    /* i18n-ok: button translates these labels */ for (i, title) in ["Instructions", "System Prompt", "Skills", "Project MCPs"].into_iter().enumerate() {
        tabs = tabs.child(button(cx, light, format!("context-tab-{i}"), title, state.tab == i, false, move |app, _| {
            let state = &mut app.screens.thread_context;
            state.tab = i;
            state.expanded.clear();
            state.scroll = ScrollHandle::new();
        }));
    }
    let mut body = div().id("context-records").flex_1().min_h_0().overflow_y_scroll().track_scroll(&state.scroll).px_4().py_3().flex().flex_col().gap_3();
    if state.busy { body = body.child(div().text_color(c("dim")).child(trs("Reading thread context…"))); }
    if !state.error.is_empty() { body = body.child(div().text_color(c("bad")).child(state.error.clone())); }
    if loaded {
        match state.tab {
            0 => {
                if data["instructions"].as_array().is_some_and(Vec::is_empty) { body = body.child(trs("No project instructions found.")); }
                for (i, f) in rows(data, "instructions").enumerate() {
                    let path = string(f, "path");
                    let key = path.clone();
                    let expanded = state.expanded == path;
                    let row = div().id(("instruction", i)).p_3().flex().flex_col().gap_2().border_1().border_color(c("line"))
                        .cursor_pointer().hover(|d| d.bg(c("raise")))
                        .child(div().flex().justify_between().gap_2().child(div().min_w_0().child(path)).child(badge(f["status"].as_str().unwrap_or(""), light)))
                        .child(div().text_size(px(11.)).text_color(c("dim")).child(crate::i18n::tf("thread.context.size", &[("bytes", f["bytes"].as_u64().unwrap_or_default().into()), ("tokens", f["estimated_tokens"].as_u64().unwrap_or_default().into())])))
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.screens.thread_context.expanded = if expanded { String::new() } else { key.clone() };
                            cx.notify();
                        }))
                        .when(expanded, |d| d.child(code(string(f, "preview"), light)).child(div().text_color(c("dim")).child(string(f, "note"))));
                    body = body.child(row);
                }
            }
            1 => {
                let full = string(data, "prompt");
                body = body.child(div().flex().justify_between().items_center().gap_2()
                    .child(div().text_color(c("dim")).child(trs("Backplane’s launch prompt. Native files are loaded by the provider.")))
                    .child(button(cx, light, "context-copy".into(), "Copy prompt", false, false, move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(full.clone()))))); // i18n-ok: button translates its label
                /* i18n-ok: labels translated below */ for (key, title) in [("base", "Backplane"), ("instructions", "Injected project instructions"), ("skills", "Available skills index")] {
                    let text = string(&data["sections"], key);
                    if !text.is_empty() { body = body.child(div().font_weight(FontWeight::SEMIBOLD).child(trs(title))).child(code(text, light)); }
                }
            }
            2 => {
                /* i18n-ok: scope names translated below */ for scope in ["Project", "User", "Bundled"] {
                    let group: Vec<_> = rows(data, "skills").filter(|s| s["scope"] == scope).collect();
                    if group.is_empty() { continue }
                    body = body.child(div().font_weight(FontWeight::SEMIBOLD).child(trs(scope)));
                    for s in group {
                        body = body.child(div().p_3().border_1().border_color(c("line")).flex().flex_col().gap_1()
                            .child(div().flex().justify_between().gap_2().child(div().font_weight(FontWeight::SEMIBOLD).child(string(s, "name"))).child(badge(s["status"].as_str().unwrap_or(""), light)))
                            .child(string(s, "description"))
                            .child(div().text_size(px(11.)).text_color(c("dim")).child([string(s, "source"), string(s, "provider"), string(s, "model")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")))
                            .child(div().text_size(px(11.)).text_color(c("dim")).child(string(s, "path"))));
                    }
                }
            }
            _ => {
                if data["mcp"].as_array().is_some_and(Vec::is_empty) { body = body.child(trs("No project MCP configurations found.")); }
                for (i, config) in rows(data, "mcp").enumerate() {
                    let state_label = config["state"].as_str().unwrap_or("");
                    let mut card = div().p_3().border_1().border_color(c("line")).flex().flex_col().gap_2()
                        .child(div().flex().justify_between().gap_2().child(div().min_w_0().child(string(config, "path"))).child(badge(state_label, light)));
                    for server in rows(config, "servers") {
                        let url = string(server, "url");
                        let endpoint = if url.is_empty() {
                            let args = rows(server, "args").filter_map(Value::as_str).map(|a| format!("{a:?}")).collect::<Vec<_>>().join(" ");
                            format!("{:?} {args}", string(server, "command")) // i18n-ok: exact executable and arguments
                        } else { url };
                        card = card.child(div().flex().justify_between()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(string(server, "name")))
                            .child(badge(server["status"].as_str().unwrap_or(""), light)))
                            .child(code(endpoint, light));
                    }
                    let error = string(config, "error");
                    if !error.is_empty() { card = card.child(div().text_color(c("bad")).child(error)); }
                    /* i18n-ok: labels translated by button */ let action = match state_label { "Untrusted" => Some(("trust", "Trust and enable")), "Enabled" => Some(("disable", "Disable")), "Disabled" => Some(("enable", "Enable")), _ => None };
                    if let Some((action, label)) = action.filter(|_| !state.busy) {
                        let config = config.clone();
                        card = card.child(div().flex().justify_end().child(button(cx, light, format!("context-mcp-{i}"), label, action == "trust", false, move |app, cx| request(app, Some((action, &config)), cx))));
                    }
                    body = body.child(card);
                }
            }
        }
    }
    Some(div().absolute().inset_0().bg(rgba(0x00000066)).flex().items_center().justify_center()
        .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.stop_propagation()))
        .child(div().w((viewport.width - px(32.)).min(px(840.))).h((viewport.height - px(32.)).min(px(700.)))
            .bg(c("panel")).border_1().border_color(c("line")).shadow_lg().flex().flex_col()
            .child(div().px_4().py_3().flex().justify_between().items_start()
                .child(div().min_w_0().flex().flex_col().gap_1()
                    .child(div().text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child(trs("Context & Prompt")))
                    .child(div().text_size(px(12.)).text_color(c("dim")).child(string(data, "cwd"))))
                .child(div().flex().gap_2()
                    .child(button(cx, light, "context-refresh".into(), "Refresh", false, false, |app, cx| request(app, None, cx))) // i18n-ok: button translates its label
                    .child(button(cx, light, "context-close".into(), "Close", false, false, |app, cx| { close(app, cx); })))) // i18n-ok: button translates its label
            .child(tabs).child(div().h(px(1.)).bg(c("line"))).child(body))
        .into_any_element())
}
