//! One measured row for the drawing, change and file pages.
use super::viewer;
use crate::{i18n::{t, trs}, theme, ui::with_rect};
use crate::{Backplane, pane_tool::PaneTool};
use gpui::{prelude::*, *};
use serde_json::Value;

pub struct Header {
    pub identity: Identity,
    pub controls: Controls,
}

pub struct Controls {
    pub actions: AnyElement,
    pub tools: Vec<Tool>,
    pub close: AnyElement,
}

pub struct Identity {
    page: String,
    label: String,
    rel: String,
    name: String,
    version: String,
    detail: String,
}

impl Identity {
    pub fn new(page: &str, facts: &Value) -> Self {
        let field = |v: &Value, k: &str| v[k].as_str().unwrap_or_default().to_owned();
        let fallback_key = match page {
            "board" => "viewer.board", "schematic" => "viewer.schematic", "gerbers" => "gpui.gerbers",
            "split" => "gpui.split", "diff" => "viewer.diff", "review" => "viewer.review", "files" => "viewer.files", _ => page,
        };
        let label = facts["label"].as_str().map(trs).unwrap_or_else(|| t(fallback_key));
        let source = if page == "split" {
            ["board", "schematic"].iter().find_map(|kind| facts["panes"].as_array()?.iter()
                .find(|p| p["page"] == *kind && p["source"]["rel"].is_string()).map(|p| &p["source"]))
                .unwrap_or(&facts["source"])
        } else { &facts["source"] };
        let rel = field(source, "rel");
        let mut name = field(source, "name");
        let scope = field(facts, "scope");
        let mut version = field(facts, "version");
        if rel.is_empty() && name.is_empty() {
            if facts["missing"] == true { name = t("history.source.unknown"); }
            else if !scope.is_empty() { name = trs(&scope); }
            else if let Some(crumb) = facts["trail"].as_array().and_then(|a| a.last()) {
                let label = field(crumb, "label");
                name = if crumb["value"] == "" { trs(&label) } else { label };
            }
        } else if !scope.is_empty() && version.is_empty() {
            version = scope.clone();
        }
        let mut detail = facts["set"]["madeAt"].as_i64().and_then(|stamp| time::OffsetDateTime::from_unix_timestamp(stamp).ok()).map(|stamp| stamp.date().to_string()).unwrap_or_default();
        if let Some(panes) = facts["panes"].as_array() {
            detail = panes.iter().filter_map(|pane| {
                let source = pane["source"]["rel"].as_str()?;
                Some([trs(pane["label"].as_str()?).as_str(), source].join(" · "))
            }).collect::<Vec<_>>().join("\n");
        }
        Self { page: page.into(), label, rel, name, version, detail }
    }

    pub fn with_set_note(mut self, note: &str) -> Self {
        if !note.is_empty() {
            if !self.detail.is_empty() { self.detail.push('\n'); }
            self.detail.push_str(note);
        }
        self
    }

    fn text(&self, path: &str) -> String {
        [&self.label, path, &self.version].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
    }
}

#[derive(Clone)]
pub enum Press {
    Hub(String, String),
    File(super::filesui::Do),
    Boards { folder: String },
    Edit,
    Sheets,
    Layers,
    Fit(usize),
    GerbView(Vec<String>),
    Split(&'static str),
    FocusProbe,
    ClearProbe,
}

impl Press {
    pub(crate) fn run(&self, this: &mut Backplane, window: &mut Window, cx: &mut Context<Backplane>) {
        match self {
            Self::Hub(action, value) => {
                let _ = this.tx.try_send(crate::hub::Command::Action(action.clone(), value.clone()));
                cx.notify();
            }
            Self::File(action) => super::filesui::act(this, action.clone(), Some(window), cx),
            Self::Boards { .. } => crate::viewers::boards::toggle(this, window, cx),
            Self::Edit => { this.kicad_edit(cx); }
            Self::Sheets => super::sheets::toggle(this, 0, window, cx),
            Self::Layers => crate::viewers::layers::toggle(this, cx),
            Self::Fit(slot) => {
                let pv = this.screens.pv(*slot);
                pv.touched = false;
                pv.fit_wh = (0., 0.);
                cx.notify();
            }
            Self::GerbView(files) => this.open_in_gerbview(files.clone(), cx),
            Self::Split(pane) => viewer::toggle_split(this, pane, cx),
            Self::FocusProbe => {
                for i in [1, 2] { super::plotview::bring_into_view(this, i); }
                cx.notify();
            }
            Self::ClearProbe => {
                for i in 0..3 {
                    let pv = this.screens.pv(i);
                    pv.picked = None;
                    pv.card = None;
                    pv.lit.clear();
                }
                super::plotview::keep_probe(this, None);
                super::plotview::report_state(this);
                cx.notify();
            }
        }
    }
}

pub struct ToolAction {
    body: ToolBody,
    popup: Option<AnyElement>,
    tip: Option<(String, String)>,
}

enum ToolBody {
    Button(WeakEntity<Backplane>, Press),
    Status(AnyElement),
}

pub type Tool = PaneTool<ToolAction>;

pub fn button(cx: &Context<Backplane>, id: impl Into<String>, label: String, press: Press) -> Tool {
    let body = ToolBody::Button(cx.entity().downgrade(), press);
    PaneTool::new(id, label, ToolAction { body, popup: None, tip: None })
}

pub fn picker(cx: &Context<Backplane>, id: &str, label: String, open: bool, popup: Option<AnyElement>, press: Press) -> Tool {
    let mut tool = button(cx, id, label, press);
    tool.action.popup = popup;
    tool.active = open;
    tool
}

pub fn hint(mut tool: Tool, text: String, key: &str) -> Tool {
    tool.action.tip = Some((text, key.to_owned()));
    tool
}

pub fn content(id: &str, label: String, element: impl IntoElement) -> Tool {
    let body = ToolBody::Status(element.into_any_element());
    let mut tool = PaneTool::new(id, label, ToolAction { body, popup: None, tip: None });
    tool.enabled = false;
    tool
}

pub fn command(cx: &Context<Backplane>, id: impl Into<String>, label: String, active: bool,
    action: &str, value: String) -> Tool {
    let mut tool = button(cx, id, label, Press::Hub(action.to_owned(), value));
    tool.active = active;
    tool
}

fn listener(entity: &WeakEntity<Backplane>, press: &Press) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static + use<> {
    let entity = entity.clone();
    let press = press.clone();
    move |_, w, cx| { let _ = entity.update(cx, |this, cx| press.run(this, w, cx)); }
}

fn face(light: bool, tool: &Tool, entity: &WeakEntity<Backplane>, press: &Press, menu: bool) -> AnyElement {
    let c = |name| theme::color(light, name);
    let d = viewer::tall(div().id(SharedString::from(tool.id.clone())))
        .px_2().gap_1().text_size(crate::scale::tpx(12.))
        .when(!menu, |d| d.border_1().border_color(c(if tool.active { "accent" } else { "line" })).bg(c("raise")))
        .when(menu, |d| d.w_full())
        .when(tool.active, |d| d.text_color(c("accent")))
        .when(!tool.enabled, |d| d.opacity(0.35))
        .when_some(tool.action.tip.as_ref(), |d, (text, key)| d.tooltip(crate::keys::tip(text, key, light)))
        .children(tool.icon.map(|icon| crate::art::icon(icon, 13., c("text"))));
    let d = if matches!(press, Press::FocusProbe) {
        d.max_w(px(240.)).child(div().id("split-probe-name").min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis()
            .cursor_pointer().tooltip(crate::keys::tip(&trs("Bring it into view"), "", light)).child(tool.label.clone())
            .on_click(listener(entity, press)))
    } else if matches!(press, Press::ClearProbe) {
        d.cursor_pointer().child(crate::art::icon("x", 12., c("dim"))).on_click(listener(entity, press))
    } else if let Press::Boards { folder } = press {
        d.when(!menu, |d| d.w(px(260.))).cursor_pointer().hover(|s| s.bg(c("panel")))
            .child(div().flex_1().min_w_0().flex().flex_col().justify_center().line_height(crate::scale::tpx(13.))
                .child(div().overflow_hidden().whitespace_nowrap().text_ellipsis().child(tool.label.clone()))
                .when(!folder.is_empty(), |d| d.child(div().overflow_hidden().whitespace_nowrap().text_ellipsis()
                    .text_size(crate::scale::tpx(10.)).text_color(c("dim")).child(folder.clone()))))
            .child(crate::art::icon("chevron-down", 12., c("dim"))).on_click(listener(entity, press))
    } else {
        d.child(tool.label.clone()).when(tool.enabled, |d| d.cursor_pointer().hover(|s| s.bg(c("panel"))).on_click(listener(entity, press)))
    };
    match press {
        Press::Edit => with_rect(d, || "ui:Edit".into()).into_any_element(),
        _ if tool.id == "follow-agent" => with_rect(d, || "follow-agent".into()).into_any_element(),
        Press::Hub(action, _) if action == "view" && tool.id == "gerbers-vs-board" =>
            with_rect(d, || "ui:vs board".into()).into_any_element(), // i18n-ok: physical probe ID
        Press::GerbView(_) => with_rect(d, || "ui:Open in GerbView".into()).into_any_element(), // i18n-ok: physical probe ID
        _ => d.into_any_element(),
    }
}

/// Keep the highest priority controls first, breaking ties in toolbar order.
fn layout(room: f32, more: f32, items: &[(u8, f32)]) -> Vec<bool> {
    if items.iter().map(|i| i.1).sum::<f32>() <= room { return vec![true; items.len()]; }
    let mut order: Vec<_> = (0..items.len()).collect();
    order.sort_by_key(|&i| items[i].0);
    let mut remaining = room - more;
    let mut visible = vec![false; items.len()];
    for i in order {
        if items[i].1 <= remaining {
            remaining -= items[i].1;
            visible[i] = true;
        }
    }
    visible
}

fn priority(id: &str, pane_action: bool) -> u8 {
    if id == "layers-btn" || id.starts_with("plot-fit-") || id.starts_with("rv-")
        || id == "fm-up" || id.starts_with("crumb-") { 0 }
    else if pane_action { 1 } else { 2 }
}

fn shorten(identity: &Identity, basename: &str, room: f32, measure: impl Fn(&str) -> f32) -> String {
    let chars: Vec<_> = basename.chars().collect();
    let candidate = |keep: usize| {
        let left: String = chars[..keep / 2].iter().collect();
        let right: String = chars[chars.len() - keep.div_ceil(2)..].iter().collect();
        identity.text(&format!("{left}…{right}"))
    };
    let mut lo = 0;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if measure(&candidate(mid)) <= room { lo = mid; } else { hi = mid - 1; }
    }
    candidate(lo)
}

// GPUI can measure a control at its intrinsic size, then prepaint that same element
// at the position assigned to its canvas. This keeps nested menus and handlers intact.
fn measured(mut el: AnyElement, w: &mut Window, cx: &mut App) -> (f32, AnyElement) {
    let sz = el.layout_as_root(size(AvailableSpace::MaxContent, AvailableSpace::MaxContent), w, cx);
    let slot = canvas(move |b, w, cx| { el.prepaint_at(b.origin, w, cx); el }, |_, mut el, w, cx| el.paint(w, cx))
        .flex_none().w(sz.width).h(sz.height);
    (sz.width.into(), slot.into_any_element())
}

pub fn pane_header(light: bool, identity: Identity, tools: Vec<PaneTool<ToolAction>>, controls: Controls) -> AnyElement {
    let page = identity.page.clone();
    let identity_page = page.clone();
    let c = move |n: &str| theme::color(light, n);
    let inner = canvas(move |bounds, w, cx| {
        let state = w.use_keyed_state(SharedString::from(format!("pane-menu-{page}")), cx, |_, cx| {
            (None::<usize>, cx.focus_handle().tab_stop(true), std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default())))
        });
        let (selected, focus, trigger_rect) = state.read(cx).clone();
        let gap = 4.;
        let more = f32::from(crate::scale::tpx(super::head::BTN_H));
        let (close_width, close) = measured(controls.close, w, cx);
        let path = if identity.rel.is_empty() { &identity.name } else { &identity.rel };
        let basename = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let full = identity.text(path);
        let short = if path == basename { full.clone() } else { identity.text(&format!("…/{basename}")) };
        let measure = |s: &str| f32::from(w.text_system().shape_line(s.to_owned().into(), crate::scale::tpx(12.), &[w.text_style().to_run(s.len())], None).width).ceil();
        let title_min = measure(&short).min(f32::from(crate::scale::tpx(150.)));
        let room = f32::from(bounds.size.width);
        let mut widths = vec![];
        let mut items = vec![];
        crate::script::header_present();
        let page_count = tools.len();
        for (i, tool) in tools.into_iter().chain(controls.tools).enumerate() {
            let id = tool.id.clone();
            let label = tool.label.clone();
            let action = match &tool.action.body {
                ToolBody::Button(entity, press) if tool.enabled => Some((entity.clone(), press.clone())),
                _ => None,
            };
            let inline = match tool.action.body {
                ToolBody::Button(ref entity, ref press) => face(light, &tool, entity, press, false),
                ToolBody::Status(el) => el,
            };
            let (width, inline) = measured(with_rect(div().child(inline), || format!("pane-tool:{page}:{id}")).into_any_element(), w, cx);
            widths.push((priority(&id, i >= page_count), width + gap));
            items.push((id, label, tool.active, inline, action, tool.action.popup));
        }
        let visible = layout(room - close_width - gap - title_min, more + gap, &widths);
        let folded = visible.iter().any(|&v| !v);
        let title_room = room - close_width - gap - widths.iter().zip(&visible).filter(|(_, v)| **v).map(|(i, _)| i.1).sum::<f32>()
            - if folded { more + gap } else { 0. };
        let measure = |s: &str| f32::from(w.text_system().shape_line(s.to_owned().into(), crate::scale::tpx(12.), &[w.text_style().to_run(s.len())], None).width).ceil();
        let text = if measure(&full) <= title_room { full.clone() } else if measure(&short) <= title_room { short }
            else { shorten(&identity, basename, title_room, measure) };
        let tooltip = [full, identity.detail].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
        let title = with_rect(div().id("pane-identity"), || format!("pane-identity:{page}:{text}"))
            .flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_size(crate::scale::tpx(12.)).tooltip(crate::keys::tip(&tooltip, "", light)).child(text);
        let mut strip = with_rect(div().flex_none().flex().items_center().gap(px(gap)), || format!("pane-tools:{page}"));
        let mut rows = Vec::new();
        let mut actions = Vec::new();
        let mut popups = Vec::new();
        for ((id, label, active, inline, action, popup), show) in items.into_iter().zip(visible) {
            crate::script::header_action(&page, &id, &label, !show, action.as_ref().map(|(_, p)| p.clone()));
            if show {
                strip = strip.child(div().relative().child(inline).children(popup));
                continue;
            }
            popups.extend(popup);
            let mut row = with_rect(with_rect(div().id(SharedString::from(format!("menu-{id}"))), || format!("pane-menu-row:{page}:{id}")),
                || format!("pane-tool:{page}:{id}")).w_full().px_2().py_1().text_size(crate::scale::tpx(12.))
                .when(active, |r| r.text_color(c("accent")).child("✓ ")).child(label);
            if let Some((entity, press)) = action {
                let index = actions.len();
                actions.push((entity.clone(), press.clone()));
                let state = state.clone();
                row = row.when(selected == Some(index), |r| r.bg(c("raise"))).cursor_pointer().hover(|r| r.bg(c("raise")))
                    .on_click(move |_, w, cx| {
                        state.update(cx, |s, cx| { s.0 = None; cx.notify(); });
                        let _ = entity.update(cx, |this, cx| press.run(this, w, cx));
                    });
            } else { row = row.opacity(0.6); }
            rows.push(row);
        }
        if folded {
            let toggle = state.clone();
            let keys = state.clone();
            let rect = trigger_rect.clone();
            let button = with_rect(div().id("pane-more"), || format!("pane-more:{page}"))
                .track_focus(&focus).tab_stop(true).focus(|s| s.border_1().border_color(c("accent")))
                .size(px(more)).flex().justify_center().items_center().cursor_pointer().child("⋯")
                .child(canvas(move |b, _, _| rect.set(b), |_, _, _, _| {}).absolute().inset_0())
                .on_click(move |_, w, cx| toggle.update(cx, |s, cx| {
                    s.1.focus(w); s.0 = if s.0.is_some() { None } else { Some(0) }; cx.notify();
                }))
                .on_key_down(move |e, w, cx| {
                    let n = actions.len().max(1);
                    let mut activate = None;
                    let handled = keys.update(cx, |s, cx| {
                        s.0 = match (s.0, e.keystroke.key.as_str()) {
                            (None, "enter" | "space" | "down") => Some(0),
                            (Some(i), "down") => Some((i + 1) % n),
                            (Some(i), "up") => Some((i + n - 1) % n),
                            (Some(i), "enter" | "space") => { activate = actions.get(i); None }
                            (Some(_), "escape") => None,
                            (Some(_), "tab") => { s.0 = None; cx.notify(); return false; }
                            _ => return false,
                        }; cx.notify(); true
                    });
                    if handled { cx.stop_propagation(); w.prevent_default(); }
                    if let Some((entity, press)) = activate { let _ = entity.update(cx, |this, cx| press.run(this, w, cx)); }
                });
            let mut anchor = div().relative().child(button).children(popups);
            if selected.is_some() {
                let outside = state.clone();
                let menu = with_rect(div().id("pane-tools-menu"), || format!("pane-menu:{page}")).occlude().p_1().flex().flex_col()
                    .border_1().border_color(c("line")).bg(c("panel")).children(rows)
                    .on_mouse_down_out(move |e, _, cx| { if !trigger_rect.get().contains(&e.position) {
                        outside.update(cx, |s, cx| { s.0 = None; cx.notify(); });
                    } });
                anchor = anchor.child(div().absolute().top(px(more)).right_0().child(deferred(anchored().anchor(Corner::TopRight).snap_to_window_with_margin(px(4.)).child(menu)).with_priority(2)));
            }
            strip = strip.child(anchor);
        } else if selected.is_some() { state.update(cx, |s, cx| { s.0 = None; cx.notify(); }); }
        let mut row = div().size_full().flex().items_center().gap(px(gap)).whitespace_nowrap()
            .child(title).when(!widths.is_empty(), |r| r.child(strip)).child(close).into_any_element();
        row.prepaint_as_root(bounds.origin, size(bounds.size.width.into(), bounds.size.height.into()), w, cx);
        row
    }, |_, mut row, w, cx| row.paint(w, cx)).size_full();
    with_rect(div().flex_shrink_0(), || format!("pane-header:{}", identity_page)).child(viewer::bar(light).child(inner)).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{layout, Identity};
    #[test]
    fn primary_tools_survive_before_pane_actions_and_secondary_tools() {
        for (scale, counts) in [(1., [2, 3, 4, 5]), (1.25, [1, 2, 3, 5])] {
            let items = [(2, 240.), (0, 70.), (0, 40.), (1, 100.), (1, 115.)].map(|(p,w)| (p, w*scale));
            for (width, count) in [321., 480., 617., 1015.].into_iter().zip(counts) {
                let shown = layout(width - 155.*scale, 30.*scale, &items);
                assert_eq!(shown.iter().filter(|v| **v).count(), count);
                assert!(shown[1]);
                assert_eq!(shown[0], count == 5);
            }
        }
    }
    use serde_json::json;
    #[test]
    fn identities_use_presenter_scope_trail_and_set_metadata() {
        for (page, facts, expected) in [
            ("diff", json!({"scope":"Thread changes"}), "Diff · Thread changes"),
            ("review", json!({"scope":"Working copy"}), "Review · Working copy"),
            ("files", json!({"trail":[{"label":"Project","value":""}]}), "Files · Project"),
        ] {
            let id = Identity::new(page, &facts);
            assert_eq!(id.text(&id.name), expected);
        }
        let id = Identity::new("gerbers", &json!({"source":{"rel":"mfg/gerbers","name":"gerbers"}, "set":{"madeAt":123}}));
        assert_eq!(id.text(&id.rel), "Gerbers · mfg/gerbers");
        assert_eq!(id.detail, "1970-01-01");
    }
    #[test]
    fn split_names_the_available_design() {
        let mut facts = json!({"panes":[{"page":"schematic","source":{"rel":"amp.sch"}},{"page":"board","missing":true}]});
        assert_eq!(Identity::new("split", &facts).rel, "amp.sch");
        facts["panes"][1] = json!({"page":"board","source":{"rel":"layout/amp.pcb"}});
        assert_eq!(Identity::new("split", &facts).rel, "layout/amp.pcb");
    }
}
