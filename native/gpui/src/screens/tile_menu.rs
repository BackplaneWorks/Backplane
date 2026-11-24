//! Layout controls shared by the single-page viewer and the tile renderer.
use super::pane_header::{Header, Press, button, picker};
use crate::{
    Backplane,
    hub::Command,
    i18n::{t, trs},
    theme,
    ui::with_rect,
};
use gpui::{prelude::*, *};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

type Owner = (String, String);

#[derive(Default)]
struct Measurements {
    thread: String,
    sizes: HashMap<String, [f32; 2]>,
}

#[derive(Clone)]
struct Navigation {
    focus: FocusHandle,
    take_focus: Rc<Cell<bool>>,
    row: usize,
    column: usize,
    keyboard: bool,
    scroll: ScrollHandle,
}

const SIDES: [(&str, &str, &str); 4] = [
    ("left", "◀", "viewer.tile.left"),
    ("right", "▶", "viewer.tile.right"),
    ("top", "▲", "viewer.tile.top"),
    ("bottom", "▼", "viewer.tile.bottom"),
];

#[derive(Clone, Default)]
pub struct State {
    open: Option<Owner>,
    nav: Option<Navigation>,
    measured: Rc<RefCell<Measurements>>,
    refusal: Option<(String, String, String)>,
    refusal_generation: u64,
}

impl State {
    pub fn record_bounds(&self, thread: &str, tile: &str, size: [f32; 2]) {
        let mut measured = self.measured.borrow_mut();
        if measured.thread != thread {
            measured.thread = thread.to_owned();
            measured.sizes.clear();
        }
        if size.iter().all(|n| n.is_finite() && *n > 0.) {
            measured.sizes.insert(tile.to_owned(), size);
        } else {
            measured.sizes.remove(tile);
        }
    }

    fn add_payload(&self, owner: &Owner, kind: &str, side: Option<&str>) -> Value {
        let mut payload = json!({"target":owner.1, "kind":kind});
        if let Some(side) = side {
            payload["side"] = json!(side);
        }
        {
            let measured = self.measured.borrow();
            if measured.thread == owner.0 {
                if let Some(size) = measured.sizes.get(&owner.1) {
                    payload["bounds"] = json!(size);
                }
            }
        }
        payload
    }

    pub fn bounds_probe(&self, thread: &str, tile: &str) -> AnyElement {
        let state = self.clone();
        let (thread, tile) = (thread.to_owned(), tile.to_owned());
        canvas(
            move |bounds, _, _| {
                state.record_bounds(
                    &thread,
                    &tile,
                    [bounds.size.width.into(), bounds.size.height.into()],
                )
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }
}

#[derive(Clone)]
pub enum Action {
    Open(Option<(String, String)>),
    Send(&'static str, Value),
    Fit(String),
    Add {
        owner: Owner,
        kind: String,
        side: Option<&'static str>,
    },
}

impl Action {
    pub(super) fn run(&self, this: &mut Backplane, cx: &mut Context<Backplane>) {
        match self {
            Self::Fit(id) => {
                if let Some(slot) = this.screens.plots.tile_slot(id) {
                    let plot = this.screens.pv(slot);
                    plot.touched = false;
                    plot.fit_wh = (0., 0.);
                }
            }
            Self::Open(target) => {
                let state = &mut this.screens.tile_menu;
                state.open = target.clone();
                state.nav = target.as_ref().map(|_| Navigation {
                    focus: cx.focus_handle(),
                    take_focus: Rc::new(Cell::new(true)),
                    row: 0,
                    column: 0,
                    keyboard: false,
                    scroll: ScrollHandle::new(),
                });
            }
            Self::Add { owner, kind, side } => {
                let state = &this.screens.tile_menu;
                if state.open.as_ref() != Some(owner) || this.data["thread"]["id"] != owner.0 {
                    return;
                }
                send_add(this, owner, kind, *side, cx);
                return;
            }
            Self::Send(action, payload) => {
                this.screens.tile_menu.open = None;
                this.screens.tile_menu.nav = None;
                let _ = this
                    .tx
                    .try_send(Command::Action((*action).into(), payload.to_string()));
            }
        }
        this.ui_epoch += 1;
        cx.notify();
    }
}

fn send_add(
    this: &mut Backplane,
    owner: &Owner,
    kind: &str,
    side: Option<&str>,
    cx: &mut Context<Backplane>,
) {
    let payload = this.screens.tile_menu.add_payload(owner, kind, side);
    Action::Send("layout-add", payload).run(this, cx);
}

pub(super) fn picker_target(thread: &Value) -> Option<(String, String)> {
    let viewer = &thread["viewer"];
    let layout = &viewer["layout"];
    let root = &layout["root"];
    viewer["rail"].as_array()?;
    let thread_id = thread["id"].as_str().filter(|id| !id.is_empty())?;
    let target = if root["type"] == "tile" {
        root["id"].as_str()
    } else if super::viewer::is_tiled(viewer) {
        let focus = layout["focus"].as_str()?;
        fn contains(node: &Value, id: &str) -> bool {
            match node["type"].as_str() {
                Some("tile") => node["id"] == id,
                Some("split") => contains(&node["a"], id) || contains(&node["b"], id),
                _ => false,
            }
        }
        contains(root, focus).then_some(focus)
    } else {
        None
    }?;
    (!target.is_empty()).then(|| (thread_id.to_owned(), target.to_owned()))
}

pub(crate) fn rail_add(this: &mut Backplane, kind: &str, cx: &mut Context<Backplane>) {
    if let Some(owner) = picker_target(&this.data["thread"]) {
        send_add(this, &owner, kind, None, cx);
    }
}

pub(crate) fn refused(this: &mut Backplane, reply: &Value, cx: &mut Context<Backplane>) {
    let key = reply["key"].as_str().unwrap_or("");
    let mut text = t(key);
    if text == key {
        text = crate::string(reply, "text");
    }
    this.m("refusal", json!({"code":reply["code"], "text":text}));
    if let (Some(thread), Some(tile)) = (reply["thread"].as_str(), reply["tile"].as_str()) {
        show_refusal(this, thread, tile, text, cx);
        return;
    } else {
        this.note = text;
    }
    this.ui_epoch += 1;
    cx.notify();
}

pub(super) fn show_refusal(
    this: &mut Backplane,
    thread: &str,
    tile: &str,
    text: String,
    cx: &mut Context<Backplane>,
) {
    let state = &mut this.screens.tile_menu;
    state.refusal = Some((thread.to_owned(), tile.to_owned(), text));
    state.refusal_generation += 1;
    let generation = state.refusal_generation;
    cx.spawn(async move |this, cx| {
        Timer::after(std::time::Duration::from_secs(5)).await;
        let _ = this.update(cx, |app, cx| {
            let state = &mut app.screens.tile_menu;
            if state.refusal_generation == generation && state.refusal.take().is_some() {
                app.ui_epoch += 1;
                cx.notify();
            }
        });
    })
    .detach();
    this.ui_epoch += 1;
    cx.notify();
}

fn action(name: &'static str, payload: Value) -> Press {
    Press::Tile(Action::Send(name, payload))
}

// Register entries during prepaint, after the script registry starts the frame.
fn entry(
    cx: &Context<Backplane>,
    light: bool,
    page: &str,
    id: String,
    label: String,
    press: Press,
) -> Stateful<Div> {
    let entity = cx.entity().downgrade();
    with_rect(
        with_rect(div().id(SharedString::from(id.clone())), || id.clone()),
        || format!("pane-tool:{page}:{id}"),
    )
    .relative()
    .cursor_pointer()
    .px_1()
    .py_1()
    .hover(|s| s.bg(theme::color(light, "raise")))
    .when(crate::script::header_tracking(), |d| {
        let page = page.to_owned();
        let registered = press.clone();
        d.child(
            canvas(
                move |_, _, _| {
                    crate::script::header_action(&page, &id, &label, false, Some(registered));
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
    })
    .on_click(move |_, w, cx| {
        cx.stop_propagation();
        let _ = entity.update(cx, |this, cx| press.run(this, w, cx));
    })
}

fn add(owner: &Owner, kind: &str, column: usize) -> Press {
    Press::Tile(Action::Add {
        owner: owner.clone(),
        kind: kind.into(),
        side: column
            .checked_sub(1)
            .and_then(|i| SIDES.get(i))
            .map(|s| s.0),
    })
}

fn popup(
    cx: &Context<Backplane>,
    light: bool,
    viewer: &Value,
    owner: &Owner,
    nav: &Navigation,
) -> AnyElement {
    let c = |token| theme::color(light, token);
    let tile = owner.1.as_str();
    let mut options = Vec::new();
    let mut list = div()
        .id("add-beside-options")
        .flex()
        .flex_col()
        .max_h(crate::scale::tpx(340.))
        .overflow_y_scroll()
        .track_scroll(&nav.scroll);
    for (index, row) in viewer["rail"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| {
            r["id"]
                .as_str()
                .is_some_and(|id| !matches!(id, "split" | "terminal" | "commands"))
        })
        .enumerate()
    {
        let kind = row["id"].as_str().unwrap();
        let label = trs(row["label"].as_str().unwrap_or(kind));
        let group = SharedString::from(format!("add-row-{kind}"));
        let icon = row["icon"]
            .as_str()
            .and_then(crate::art::icon_name)
            .map(|icon| crate::art::icon(icon, 14., c("dim")));
        let caption = div()
            .flex()
            .items_center()
            .gap_2()
            .children(icon)
            .child(label.clone());
        let mut line = div()
            .group(group.clone())
            .flex()
            .items_center()
            .px_1()
            .h(crate::scale::tpx(27.));
        if row["state"] == "disabled" {
            let why = trs(row["why"].as_str().unwrap_or(""));
            line = line.child(
                div()
                    .id(SharedString::from(format!("disabled-{kind}")))
                    .size_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .opacity(0.55)
                    .tooltip(crate::keys::tip(&why, "", light))
                    .child(caption)
                    .child(
                        div()
                            .text_size(crate::scale::tpx(10.))
                            .text_color(c("dim"))
                            .child(why),
                    ),
            );
        } else {
            let focused = nav.keyboard && nav.row == options.len();
            options.push((index, kind.to_owned()));
            line = line
                .when(focused, |d| d.bg(c("raise")))
                .hover(|s| s.bg(c("raise")))
                .child(
                    entry(
                        cx,
                        light,
                        tile,
                        format!("add-beside:{kind}"),
                        label,
                        add(owner, kind, 0),
                    )
                    .flex_1()
                    .child(caption),
                );
            let mut arrows = div().flex().items_center().when(!focused, |d| {
                d.invisible().group_hover(group, |s| s.visible())
            });
            for (i, (side, glyph, key)) in SIDES.iter().enumerate() {
                let hint = t(key);
                arrows = arrows.child(
                    entry(
                        cx,
                        light,
                        tile,
                        format!("add-beside:{kind}:{side}"),
                        hint.clone(),
                        add(owner, kind, i + 1),
                    )
                    .tooltip(crate::keys::tip(&hint, "", light))
                    .when(focused && nav.column == i + 1, |d| {
                        d.bg(c("panel")).text_color(c("accent"))
                    })
                    .child(*glyph),
                );
            }
            line = line.child(arrows);
        }
        list = list.child(line);
    }
    let owner = owner.clone();
    let entity = cx.entity().downgrade();
    let outside = entity.clone();
    let focus = nav.focus.clone();
    let pending = nav.take_focus.clone();
    let menu = div()
        .id("add-beside-menu")
        .occlude()
        .min_w(crate::scale::tpx(210.))
        .p_1()
        .bg(c("panel"))
        .border_1()
        .border_color(c("line"))
        .text_size(crate::scale::tpx(12.))
        .track_focus(&nav.focus)
        .child(
            canvas(
                move |_, w, _| {
                    if pending.replace(false) {
                        w.focus(&focus);
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .text_color(c("dim"))
                .child(t("viewer.tile.add")),
        )
        .child(list)
        .on_key_down(move |ev, w, cx| {
            let key = ev.keystroke.key.as_str();
            if !matches!(
                key,
                "escape" | "up" | "down" | "left" | "right" | "enter" | "space"
            ) {
                return;
            }
            cx.stop_propagation();
            w.prevent_default();
            let _ = entity.update(cx, |app, cx| {
                if key == "escape" {
                    Action::Open(None).run(app, cx);
                    return;
                }
                if options.is_empty() {
                    return;
                }
                let Some(nav) = app.screens.tile_menu.nav.as_mut() else {
                    return;
                };
                nav.row = nav.row.min(options.len() - 1);
                let first = !nav.keyboard;
                nav.keyboard = true;
                match key {
                    "up" if first => nav.row = options.len() - 1,
                    "down" if first => nav.row = 0,
                    "up" => nav.row = (nav.row + options.len() - 1) % options.len(),
                    "down" => nav.row = (nav.row + 1) % options.len(),
                    "left" => nav.column = (nav.column + SIDES.len()) % (SIDES.len() + 1),
                    "right" => nav.column = (nav.column + 1) % (SIDES.len() + 1),
                    _ => {
                        let press = add(&owner, &options[nav.row].1, nav.column);
                        press.run(app, w, cx);
                        return;
                    }
                }
                nav.scroll.scroll_to_item(options[nav.row].0);
                app.ui_epoch += 1;
                cx.notify();
            });
        })
        .on_mouse_down_out(move |_, _, cx| {
            let _ = outside.update(cx, |app, cx| Action::Open(None).run(app, cx));
        });
    div()
        .absolute()
        .top(crate::scale::tpx(super::head::BTN_H))
        .right_0()
        .child(
            deferred(
                anchored()
                    .anchor(Corner::TopRight)
                    .snap_to_window_with_margin(px(4.))
                    .child(menu),
            )
            .with_priority(3),
        )
        .into_any_element()
}

fn add_tool(
    cx: &Context<Backplane>,
    light: bool,
    viewer: &Value,
    owner: &Owner,
    state: &State,
) -> super::pane_header::Tool {
    let open = state.open.as_ref() == Some(owner);
    let menu = state
        .nav
        .as_ref()
        .filter(|_| open)
        .map(|nav| popup(cx, light, viewer, owner, nav));
    let mut tool = picker(
        cx,
        &format!("tile:{}:add-beside", owner.1),
        t("viewer.tile.add"),
        open,
        menu,
        Press::Tile(Action::Open((!open).then(|| owner.clone()))),
    );
    tool.icon = Some("layout-panel-left");
    super::pane_header::hint(tool, t("viewer.tile.add.tip"), "")
}

/// Build `(control_id, i18n_label_key, hub_command)` for a tile's Maximize / Restore menu entry.
fn menu_maximize(id: &str, restored: bool) -> (String, &'static str, &'static str) {
    let (verb, label, command) = if restored {
        ("restore", "viewer.tile.restore", "layout-restore")
    } else {
        ("maximize", "viewer.tile.maximize", "layout-maximize")
    };
    (format!("tile:{id}:{verb}"), label, command)
}

/// Pass the original thread picture and the painted layout leaf, including for a maximized tile.
pub fn decorate(
    cx: &Context<Backplane>,
    light: bool,
    thread: &Value,
    leaf: &Value,
    embedded: bool,
    state: &State,
    mut h: Header,
) -> Header {
    let Some(id) = leaf["id"].as_str() else {
        return h;
    };
    let thread_id = crate::string(thread, "id");
    let owner = (thread_id.clone(), id.to_owned());
    let viewer = &thread["viewer"];
    if embedded || !viewer["rail"].is_array() {
        return h;
    }
    let tiled = super::viewer::is_tiled(viewer);
    if tiled
        && matches!(
            leaf["kind"].as_str(),
            Some("board" | "gerbers" | "schematic")
        )
        && !leaf["error"].is_string()
        && !leaf["empty"].is_string()
    {
        h.controls.tools.push(button(
            cx,
            format!("tile:{id}:fit"),
            trs("Fit"),
            Press::Tile(Action::Fit(id.into())),
        ));
    }
    if tiled {
        h.controls.tools.push(button(
            cx,
            format!("tile:{id}:more:add-beside"),
            t("viewer.tile.add"),
            Press::Tile(Action::Open(Some(owner.clone()))),
        ));
        let (control_id, label, command) = menu_maximize(id, viewer["layout"]["maximized"] == id);
        h.controls.tools.push(button(
            cx,
            control_id,
            t(label),
            action(command, json!({"tile": id})),
        ));
        h.controls.tools.push(button(
            cx,
            format!("tile:{id}:more:close"),
            t("common.close"),
            action("layout-close", json!({"tile": id})),
        ));
    }
    h.controls
        .tools
        .push(add_tool(cx, light, viewer, &owner, state));
    if let Some((thread, tile, text)) = &state.refusal {
        if *thread == thread_id && tile == id {
            h.identity.refusal(text);
        }
    }
    if tiled {
        let grip = entry(
            cx,
            light,
            id,
            format!("tile:{id}:focus"),
            "⠿".into(),
            action("layout-focus", json!({"tile":id})),
        )
        .child("⠿");
        h.identity.grip = Some(
            div()
                .id(SharedString::from(format!("tile-grip:{id}")))
                .tooltip(crate::keys::tip(&t("viewer.tile.focus"), "", light))
                .child(grip)
                .into_any_element(),
        );
        h.controls.close = div()
            .id(SharedString::from(format!("tile-close:{id}")))
            .tooltip(crate::keys::tip(&t("common.close"), "", light))
            .child(
                entry(
                    cx,
                    light,
                    id,
                    format!("tile:{id}:close"),
                    "×".into(),
                    action("layout-close", json!({"tile":id})),
                )
                .child("×"),
            )
            .into_any_element();
    }
    h
}

fn corner_fits(group: f32, button: f32, page: f32, has_leading_control: bool) -> bool {
    !has_leading_control && group >= button && page >= group + button + 16.
}

/// Use the same trigger alongside the controls on pages with a corner toolbar.
pub(super) fn corner(
    cx: &Context<Backplane>,
    light: bool,
    thread: &Value,
    state: &State,
    controls: AnyElement,
) -> AnyElement {
    let viewer = &thread["viewer"];
    let leaf = &viewer["layout"]["root"];
    let Some(id) = leaf["id"]
        .as_str()
        .filter(|_| leaf["type"] == "tile" && viewer["rail"].is_array())
    else {
        return controls;
    };
    let thread_id = crate::string(thread, "id");
    let owner = (thread_id.clone(), id.to_owned());
    let kind = leaf["kind"].as_str().unwrap_or_default();
    let size = crate::scale::tpx(super::head::BTN_H);
    let kind = kind.to_owned();
    let boards_before = kind == "3d" && crate::viewers::boards::list(viewer).len() > 1;
    let measurements = state.measured.clone();
    let tool = add_tool(cx, light, viewer, &owner, state);
    // Overlay Add at the leading edge of the measured group so its controls stay fixed.
    let add = canvas(
        move |bounds, w, cx| {
            let measured = measurements.borrow();
            let page_width = (measured.thread == owner.0)
                .then(|| measured.sizes.get(&owner.1))
                .flatten()
                .map_or(0., |size| size[0]);
            let available = if kind == "3d" {
                page_width.min(super::viewer::CORNER_ROOM)
            } else {
                page_width
            };
            let width = if corner_fits(
                f32::from(bounds.size.width),
                size.into(),
                available,
                boards_before,
            ) {
                size
            } else {
                px(0.)
            };
            let mut header = super::pane_header::pane_header(
                light,
                super::pane_header::Identity::corner(&kind),
                vec![],
                super::pane_header::Controls {
                    tools: vec![tool],
                    close: Empty.into_any_element(),
                },
            );
            let origin = point(bounds.origin.x - width, bounds.origin.y);
            header.prepaint_as_root(origin, gpui::size(width.into(), size.into()), w, cx);
            header
        },
        |_, mut header, w, cx| header.paint(w, cx),
    )
    .absolute()
    .inset_0();
    let message = state
        .refusal
        .as_ref()
        .filter(|(thread, tile, _)| thread == &thread_id && tile == id);
    div()
        .relative()
        .flex()
        .items_center()
        .children(
            message.map(|(_, _, text)| div().text_size(crate::scale::tpx(12.)).child(text.clone())),
        )
        .child(div().relative().child(controls).child(add))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{SIDES, State};
    use serde_json::json;

    #[test]
    fn corner_add_requires_a_vacant_button_beside_close() {
        assert!(super::corner_fits(24., 24., 615., false));
        assert!(super::corner_fits(120., 24., 182., false));
        assert!(!super::corner_fits(150., 30., 182., false));
        assert!(!super::corner_fits(24., 24., 615., true));
        assert!(!super::corner_fits(30., 30., 48., false));
    }

    #[test]
    fn palette_and_rail_use_the_sole_or_focused_drawn_leaf() {
        let mut thread =
            json!({"id":"thread","viewer":{"rail":[],"layout":{"root":{"type":"tile","id":"a"}}}});
        assert_eq!(super::picker_target(&thread).unwrap().1, "a");
        thread["viewer"]["layout"] = json!({"focus":"b","root":{"type":"split","a":{"type":"tile","id":"a"},"b":{"type":"tile","id":"b"}}});
        thread["viewer"]["tiled"] = json!(true);
        assert_eq!(super::picker_target(&thread).unwrap().1, "b");
        thread["viewer"]["layout"]["focus"] = json!("stale");
        assert!(super::picker_target(&thread).is_none());
        thread["viewer"]["layout"]["focus"] = json!("a");
        thread["viewer"]["tiled"] = json!(false);
        assert!(super::picker_target(&thread).is_none());
    }

    #[test]
    fn default_and_directional_add_use_the_acted_tiles_current_size() {
        let state = State::default();
        let owner = ("thread".to_owned(), "t2".to_owned());
        state.record_bounds("thread", "t1", [600., 400.]);
        state.record_bounds("thread", "t2", [280., 520.]);
        assert_eq!(
            state.add_payload(&owner, "board", None),
            json!({"target":"t2", "kind":"board", "bounds":[280.,520.]})
        );
        state.record_bounds("thread", "t2", [390., 520.]);
        for (side, _, _) in SIDES {
            assert_eq!(
                state.add_payload(&owner, "board", Some(side)),
                json!({"target":"t2", "kind":"board", "bounds":[390.,520.], "side":side})
            );
        }
        state.record_bounds("another-thread", "t2", [400., 500.]);
        assert!(
            state
                .add_payload(&owner, "board", None)
                .get("bounds")
                .is_none()
        );
    }

    #[test]
    fn maximize_and_restore_use_direct_tile_control_ids() {
        assert_eq!(
            super::menu_maximize("t1", false),
            (
                "tile:t1:maximize".to_owned(),
                "viewer.tile.maximize",
                "layout-maximize"
            )
        );
        assert_eq!(
            super::menu_maximize("t1", true),
            (
                "tile:t1:restore".to_owned(),
                "viewer.tile.restore",
                "layout-restore"
            )
        );
    }
}
