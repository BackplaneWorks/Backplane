//! One measured row for the drawing, change and file pages.
use super::viewer;
use crate::{i18n::trs, theme, ui::with_rect};
use crate::{Backplane, pane_tool::PaneTool};
use gpui::{prelude::*, *};
use serde_json::Value;

pub struct Header {
    pub identity: Identity,
    pub controls: Controls,
}

pub struct Controls {
    pub actions: AnyElement,
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
        let label = match page {
            "board" => "Board", "schematic" => "Schematic", "gerbers" => "Gerbers",
            "split" => "Split", "diff" => "Diff", "review" => "Review", "files" => "Files", _ => page,
        };
        let label = trs(facts["label"].as_str().unwrap_or(label));
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
            if facts["missing"] == true { name = trs("Missing"); }
            else if !scope.is_empty() { name = trs(&scope); }
            else if let Some(crumb) = facts["trail"].as_array().and_then(|a| a.last()) {
                name = trs(&field(crumb, "label"));
            }
        } else if !scope.is_empty() && version.is_empty() {
            version = scope.clone();
        }
        let detail = facts["set"]["madeAt"].as_u64().map(|stamp| format!("madeAt {stamp}")).unwrap_or_default();
        Self { page: page.into(), label, rel, name, version, detail }
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
    Sheets,
    Layers,
    Fit(usize),
    GerbView(Vec<String>),
    Split(&'static str),
    FocusProbe,
    ClearProbe,
}

impl Press {
    fn run(&self, this: &mut Backplane, window: &mut Window, cx: &mut Context<Backplane>) {
        match self {
            Self::Hub(action, value) => {
                let _ = this.tx.try_send(crate::hub::Command::Action(action.clone(), value.clone()));
                cx.notify();
            }
            Self::File(action) => super::filesui::act(this, action.clone(), Some(window), cx),
            Self::Boards { .. } => crate::viewers::boards::toggle(this, window, cx),
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
            .child(div().id("split-probe-clear").flex_none().p(px(2.)).cursor_pointer()
                .tooltip(crate::keys::tip(&trs("Clear"), "Esc", light)).child(crate::art::icon("x", 12., c("dim")))
                .on_click(listener(entity, &Press::ClearProbe)))
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
        Press::Hub(action, _) if action == "view" && tool.id == "gerbers-vs-board" =>
            with_rect(d, || "ui:vs board".into()).into_any_element(),
        Press::GerbView(_) => with_rect(d, || "ui:Open in GerbView".into()).into_any_element(),
        _ => d.into_any_element(),
    }
}

/// Widths include each item's trailing gap. Reserve the overflow button only if needed.
fn visible(room: f32, more: f32, widths: &[f32]) -> usize {
    if widths.iter().sum::<f32>() <= room { return widths.len() }
    let mut used = more;
    widths.iter().take_while(|w| { used += **w; used <= room }).count()
}

/// Move all optional pane actions together if they cannot fit beside the
/// identity, Close and the button for any tools already in the menu.
fn fold_actions(room: f32, actions: f32, more: f32, has_hidden_tools: bool) -> bool {
    actions > 0. && actions + if has_hidden_tools { more } else { 0. } > room
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
        let state = w.use_keyed_state(SharedString::from(format!("pane-menu-{page}")), cx, |_, _| false);
        let gap = 4.;
        let more = f32::from(crate::scale::tpx(super::head::BTN_H));
        let (action_width, actions) = measured(controls.actions, w, cx);
        let (close_width, close) = measured(controls.close, w, cx);
        let path = if identity.rel.is_empty() { &identity.name } else { &identity.rel };
        let basename = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let full = identity.text(path);
        let short = if path == basename { full.clone() } else { identity.text(&format!("…/{basename}")) };
        let measure = |s: &str| f32::from(w.text_system().shape_line(s.to_owned().into(), crate::scale::tpx(12.), &[w.text_style().to_run(s.len())], None).width).ceil();
        let (full_width, short_width) = (measure(&full), measure(&short));
        let room = f32::from(bounds.size.width);
        let mut items = vec![];
        let mut widths = vec![];
        for tool in tools {
            let (inline, row) = match tool.action.body {
                ToolBody::Button(ref entity, ref press) => (
                    face(light, &tool, entity, press, false), Some(face(light, &tool, entity, press, true))),
                ToolBody::Status(el) => (el, None),
            };
            let popup = tool.action.popup;
            let probe = || format!("pane-tool:{page}:{}", tool.id);
            let (width, inline) = measured(with_rect(div().flex_none().child(inline), probe).into_any_element(), w, cx);
            widths.push(width + gap);
            items.push((inline, row.map(|r| with_rect(div().child(r), probe).into_any_element()), popup));
        }
        let available = room - close_width - short_width - 2. * gap;
        let count = visible(available - action_width, more + gap, &widths);
        let mut overflow = vec![];
        let mut inline_items = vec![];
        for (i, (inline, row, popup)) in items.into_iter().enumerate() {
            let el = if i < count { inline } else { row.unwrap_or(inline) };
            let el = if let Some(popup) = popup { div().relative().child(el).child(popup).into_any_element() } else { el };
            if i < count { inline_items.push(el); } else { overflow.push(el); }
        }
        let fold_actions = fold_actions(available, action_width, more + gap, !overflow.is_empty());
        let inline_actions = if fold_actions {
            overflow.push(actions);
            None
        } else { Some(actions) };
        let control_width = close_width + if fold_actions { 0. } else { action_width };
        let reserve = widths[..count].iter().sum::<f32>() + if overflow.is_empty() { 0. } else { more + gap };
        let text = if full_width <= room - control_width - reserve - 2. * gap { full.clone() } else { short };
        let tooltip = [full, identity.detail].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
        let title = with_rect(div().id("pane-identity"), || format!("pane-identity:{page}:{text}"))
            .min_w_0().overflow_hidden().whitespace_nowrap().text_size(crate::scale::tpx(12.)).tooltip(crate::keys::tip(&tooltip, "", light)).child(text);
        let mut strip = with_rect(div().flex_none().flex().items_center().gap(px(gap)), || format!("pane-tools:{page}")).children(inline_items);
        if !overflow.is_empty() {
            let toggle = state.clone();
            let button = with_rect(div().id("pane-more"), || format!("pane-more:{page}")).relative().size(px(more)).flex().justify_center().items_center().cursor_pointer().child("⋯")
                .on_click(move |_, _, cx| toggle.update(cx, |open, cx| { *open = !*open; cx.notify(); }));
            let mut anchor = div().relative().child(button);
            if *state.read(cx) {
                let menu = div().id("pane-tools-menu").occlude().p_1().flex().flex_col().items_start().gap(px(gap)).border_1().border_color(c("line")).bg(c("panel")).children(overflow);
                anchor = anchor.child(div().absolute().top(px(more)).right_0().child(deferred(anchored().snap_to_window_with_margin(px(4.)).child(menu)).with_priority(2)));
            }
            strip = strip.child(anchor);
        }
        let controls = div().flex_none().flex().items_center().children(inline_actions).child(close);
        let mut row = div().size_full().flex().items_center().justify_between().whitespace_nowrap()
            .child(div().flex_1().min_w_0().overflow_hidden().child(title)).child(strip).child(controls).into_any_element();
        row.prepaint_as_root(bounds.origin, size(bounds.size.width.into(), bounds.size.height.into()), w, cx);
        row
    }, |_, mut row, w, cx| row.paint(w, cx)).size_full();
    with_rect(div().flex_shrink_0(), || format!("pane-header:{}", identity_page)).child(viewer::bar(light).child(inner)).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{fold_actions, visible, Identity};
    #[test]
    fn pane_actions_are_wholly_inline_or_wholly_in_the_menu() {
        assert!(fold_actions(96., 192., 28., true));
        assert!(fold_actions(96., 100., 28., false));
        assert!(!fold_actions(96., 68., 28., true));
        assert!(!fold_actions(96., 96., 28., false));
        assert!(!fold_actions(0., 0., 28., true));
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
        assert!(id.detail.contains("123"));
    }
    #[test]
    fn split_names_the_available_design() {
        let mut facts = json!({"panes":[{"page":"schematic","source":{"rel":"amp.sch"}},{"page":"board","missing":true}]});
        assert_eq!(Identity::new("split", &facts).rel, "amp.sch");
        facts["panes"][1] = json!({"page":"board","source":{"rel":"layout/amp.pcb"}});
        assert_eq!(Identity::new("split", &facts).rel, "layout/amp.pcb");
    }
    #[test]
    fn rightmost_tools_fold_first_and_the_overflow_has_room() {
        let widths = [30., 40., 20.];
        for (room, n) in [(100.,3), (90.,3), (89.,2), (79.,1), (40.,1), (39.,0), (0.,0)] {
            assert_eq!(visible(room, 10., &widths), n);
        }
        assert_eq!(visible(0., 10., &[]), 0);
    }
}
