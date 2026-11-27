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
    pub tools: Vec<Tool>,
    pub close: AnyElement,
}

pub struct Identity {
    pub(super) grip: Option<AnyElement>,
    tile_focus: Option<bool>,
    corner: bool,
    has_source: bool,
    refusal: Option<String>,
    page: String,
    label: String,
    rel: String,
    name: String,
    version: String,
    detail: String,
    pub picker: Option<(WeakEntity<Backplane>, Press, Option<AnyElement>)>,
}

impl Identity {
    pub(super) fn corner(page: &str) -> Self {
        let mut identity = Self::new(page, &Value::Null);
        identity.corner = true;
        identity.label.clear();
        identity
    }

    pub(super) fn tile(mut self, focused: bool) -> Self {
        self.tile_focus = Some(focused);
        self
    }

    pub(super) fn refusal(&mut self, message: &str) {
        self.refusal = Some(message.to_owned());
    }

    pub fn new(page: &str, facts: &Value) -> Self {
        let field = |v: &Value, k: &str| v[k].as_str().unwrap_or_default().to_owned();
        let fallback_key = match page {
            "board" => "viewer.board", "schematic" => "viewer.schematic", "gerbers" => "gpui.gerbers",
            "3d" => "viewer.solid", "simulation" => "gpui.simulation", "kicad" => "gpui.f.name.kicad",
            "mech" => "viewer.mech", "browser" => "bots.tab.browser",
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
        Self { grip: None, refusal: None, tile_focus: None, corner: false, has_source: !field(source, "name").is_empty() || !rel.is_empty(), page: page.into(), label, rel, name, version, detail, picker: None }
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

type MenuState = (Option<usize>, FocusHandle, std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>);

fn toggle_more(menu: &Entity<MenuState>, w: &mut Window, cx: &mut App) {
    menu.update(cx, |state, cx| {
        state.1.focus(w);
        state.0 = if state.0.is_some() { None } else { Some(0) };
        cx.notify();
    });
}

#[derive(Clone)]
pub enum Press {
    More(Entity<MenuState>),
    MoreRow(Entity<MenuState>, Box<Press>),
    Hub(String, String),
    File(super::filesui::Do),
    Boards { folder: String },
    Browser(crate::viewers::chrome::Act),
    MechParts,
    /// a file of the hub's to this computer's Downloads (screens/dlbar.rs): hub address, name
    Download(String, String),
    Edit,
    Sheets,
    Layers,
    TileLayers(String),
    Fit(usize),
    Camera(WeakEntity<crate::viewers::solid::SolidPage>, crate::viewers::solid::SolidTool),
    GerbView(Vec<String>),
    Split(&'static str),
    Tile(super::tile_menu::Action),
    FocusProbe,
    ClearProbe,
    SolidCraftFull,
}

impl Press {
    pub(crate) fn run(&self, this: &mut Backplane, window: &mut Window, cx: &mut Context<Backplane>) {
        match self {
            Self::More(menu) => toggle_more(menu, window, cx),
            Self::MoreRow(menu, press) => {
                menu.update(cx, |state, cx| { state.0 = None; cx.notify(); });
                press.run(this, window, cx);
            }
            Self::Hub(action, value) => {
                let _ = this.tx.try_send(crate::hub::Command::Action(action.clone(), value.clone()));
                cx.notify();
            }
            Self::File(action) => super::filesui::act(this, action.clone(), Some(window), cx),
            Self::Boards { .. } => crate::viewers::boards::toggle(this, window, cx),
            Self::Browser(action) => {
                if let Some((_, page)) = this.screens.browser.clone() {
                    page.update(cx, |b, cx| b.header_press(action.clone(), window, cx));
                }
            }
            Self::MechParts => viewer::toggle_parts(this, window, cx),
            Self::Download(url, name) => this.download(url.clone(), name.clone(), super::dlbar::From::Other, cx),
            Self::Edit => { this.kicad_edit(cx); }
            Self::Sheets => super::sheets::toggle(this, 0, window, cx),
            Self::Layers => crate::viewers::layers::toggle(this, cx),
            Self::TileLayers(id) => {
                let Some(slot) = this.screens.plots.tile_slot(id) else { return };
                let focused = this.data["thread"]["viewer"]["layout"]["focus"] == id.as_str();
                if !focused {
                    let _ = this.tx.try_send(crate::hub::Command::Action("layout-focus".into(),
                        serde_json::json!({"tile": id}).to_string()));
                }
                let pv = this.screens.pv(slot);
                if focused && !pv.layers_open && super::popover::just_shut("layers") { return; }
                pv.layers_open = !focused || !pv.layers_open;
                this.ui_epoch += 1;
                cx.notify();
            }
            Self::Camera(page, tool) => { let _ = page.update(cx, |p, cx| p.run_tool(*tool, cx)); }
            Self::Fit(slot) => {
                let pv = this.screens.pv(*slot);
                pv.touched = false;
                pv.fit_wh = (0., 0.);
                cx.notify();
            }
            Self::GerbView(files) => this.open_in_gerbview(files.clone(), cx),
            Self::Split(pane) => viewer::toggle_split(this, pane, cx),
            Self::Tile(action) => action.run(this, cx),
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
            Self::SolidCraftFull => this.solidcraft_toggle_full(cx),
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

fn listener_key(entity: WeakEntity<Backplane>, press: Press) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) {
    move |event, w, cx| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space" | "down") {
            cx.stop_propagation();
            w.prevent_default();
            let _ = entity.update(cx, |this, cx| press.run(this, w, cx));
        }
    }
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
    let d = if tool.id.ends_with(":add-beside") {
        let key_press = listener_key(entity.clone(), press.clone());
        d.px_0().size(crate::scale::tpx(super::head::BTN_H)).justify_center()
            .focusable().tab_stop(true).focus(|s| s.border_color(c("accent")))
            .on_key_down(key_press).cursor_pointer().on_click(listener(entity, press))
    } else if matches!(press, Press::FocusProbe) {
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
        Press::SolidCraftFull => with_rect(d, || "solidcraft:full".into()).into_any_element(),
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

/// Main's choices for the legacy tools, including its reservation for More.
fn header_room(room: f32, close: f32, title_min: f32, more: f32, gap: f32,
    widths: &[(u8, f32)]) -> (Vec<bool>, f32) {
    let available = room - close - gap;
    let visible = layout(available - title_min, more + gap, widths);
    let used: f32 = widths.iter().zip(&visible).filter(|(_, show)| **show).map(|(item, _)| item.1).sum();
    let more_width = if visible.contains(&false) { more + gap } else { 0. };
    (visible, available - used - more_width)
}

#[derive(Debug, PartialEq)]
enum AddPlacement { Inline, Shorten, More, Omit }

/// Add consumes space at the leading edge of the right-aligned group.
fn place_add(legacy: &[bool], room: f32, title: f32, minimum: f32, slot: f32,
    button: f32, shortened: Option<f32>) -> AddPlacement {
    let remaining = room - slot;
    if remaining >= title { return AddPlacement::Inline; }
    if remaining >= minimum && shortened.is_some_and(|width|
        width <= remaining && title - width <= button) { return AddPlacement::Shorten; }
    if legacy.contains(&false) { AddPlacement::More } else { AddPlacement::Omit }
}

/// A tiled header always has More. Reserve its measured width once, even
/// when all page tools fit.
fn tiled_room(room: f32, close: f32, minimum: f32, more: f32, gap: f32,
    widths: &[(u8, f32)]) -> (Vec<bool>, f32) {
    let available = room - close - more - 2. * gap;
    let mut shown = layout(available - minimum, 0., widths);
    // Keep Fit folded when the higher-priority Layers button no longer fits.
    if widths.iter().zip(&shown).any(|((rank, _), show)| *rank == 0 && !show) {
        for ((rank, _), show) in widths.iter().zip(&mut shown) {
            if *rank == 3 { *show = false; }
        }
    }
    let used: f32 = widths.iter().zip(&shown).filter(|(_, show)| **show).map(|(item, _)| item.1).sum();
    (shown, available - used)
}

fn priority(id: &str, pane_action: bool) -> u8 {
    if id == "br-hand" { return 0 }
    if id == "br-url" { return 1 }
    if id == "solidcraft-full" || id == "solidcraft-pop" { return 0 }
    if id.starts_with("tile:") && id.ends_with(":fit") { 3 }
    else if id == "layers-btn" || id.starts_with("tile:") && id.ends_with(":layers") || id.starts_with("plot-fit-") || id.starts_with("rv-")
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

// PaneTab may lead the hub picture. A previous frame's file must never label this frame.
fn embedded_identity(frame: &str, facts: &Value) -> Identity {
    let page = crate::ui::kicad_rail_id(frame).unwrap_or("kicad");
    let mut identity = Identity::new(page, &Value::Null);
    match frame {
        "FOOTPRINT_EDITOR" => { identity.label = t("viewer.footprinteditor"); return identity; }
        "SYMBOL_EDITOR" => { identity.label = t("viewer.symboleditor"); return identity; }
        _ => {}
    }
    if facts["page"] == page {
        if let Some(label) = facts["label"].as_str() { identity.label = trs(label); }
        identity.rel = facts["source"]["rel"].as_str().unwrap_or_default().to_owned();
    }
    identity
}

pub fn embedded_title(frame: &str, facts: &Value, light: bool) -> impl IntoElement {
    let identity = embedded_identity(frame, facts);
    canvas(move |bounds, w, cx| {
        let full = identity.text(&identity.rel);
        let width = f32::from(bounds.size.width);
        let measure = |s: &str| f32::from(w.text_system().shape_line(s.to_owned().into(), crate::scale::tpx(12.),
            &[w.text_style().to_run(s.len())], None).width);
        let basename = identity.rel.rsplit('/').next().unwrap_or_default();
        let text = if measure(&full) <= width || basename.is_empty() { full.clone() }
            else if measure(&identity.text(basename)) <= width { identity.text(basename) }
            else { shorten(&identity, basename, width, measure) };
        let mut label = with_rect(div().id("embedded-identity"), || format!("pane-identity:{}:{text}", identity.page))
            .flex().items_center().size_full().text_size(crate::scale::tpx(12.)).tooltip(crate::keys::tip(&full, "", light))
            .child(div().min_w_0().truncate().child(text)).into_any_element();
        label.prepaint_as_root(bounds.origin, size(bounds.size.width.into(), bounds.size.height.into()), w, cx);
        label
    }, |_, mut label, w, cx| label.paint(w, cx)).flex_1().min_w_0().h_full()
}

/// A dock title keeps the page word, and includes a separator only if a real
/// part of the source or version fits after it. The tooltip keeps the full path.
fn readable_source(name: &str) -> String {
    let chars: Vec<_> = name.chars().collect();
    if chars.len() <= 13 { return name.to_owned(); }
    let start: String = chars[..6].iter().collect();
    let end: String = chars[chars.len()-6..].iter().collect();
    format!("{start}…{end}")
}

fn tile_title(identity: &Identity, basename: &str, room: f32, measure: impl Fn(&str) -> f32) -> String {
    if basename.is_empty() { return identity.label.clone(); }
    let complete = identity.text(basename);
    if measure(&complete) <= room { return complete; }
    let minimum = identity.text(&readable_source(basename));
    if measure(&minimum) > room { return identity.label.clone(); }
    shorten(identity, basename, room, measure)
}

// Twelve standard button widths, scaled with the toolbar. A tile's Add location
// depends only on tile width, never on its page name, file name or Layers label.
fn tile_add_inline(width: f32, button: f32) -> bool { width >= 12. * button }

// GPUI can measure a control at its intrinsic size, then prepaint that same element
// at the position assigned to its canvas. This keeps nested menus and handlers intact.
fn measured(mut el: AnyElement, w: &mut Window, cx: &mut App) -> (f32, AnyElement) {
    let sz = el.layout_as_root(size(AvailableSpace::MaxContent, AvailableSpace::MaxContent), w, cx);
    let slot = canvas(move |b, w, cx| { el.prepaint_at(b.origin, w, cx); el }, |_, mut el, w, cx| el.paint(w, cx))
        .flex_none().w(sz.width).h(sz.height);
    (sz.width.into(), slot.into_any_element())
}

pub fn pane_header(light: bool, mut identity: Identity, tools: Vec<PaneTool<ToolAction>>, controls: Controls) -> AnyElement {
    let page = identity.page.clone();
    let identity_page = page.clone();
    let tile_focus = identity.tile_focus;
    let corner = identity.corner;
    let refusal = identity.refusal.take();
    let c = move |n: &str| theme::color(light, n);
    let inner = canvas(move |bounds, w, cx| {
        let scope = controls.tools.last().and_then(|tool| tool.id.strip_suffix(":add-beside")).unwrap_or(&page).to_owned();
        let state = w.use_keyed_state(SharedString::from(format!("pane-menu-{scope}")), cx, |_, cx| {
            (None::<usize>, cx.focus_handle().tab_stop(true), std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default())))
        });
        let (selected, focus, trigger_rect) = state.read(cx).clone();
        let gap = if corner { 0. } else if tile_focus.is_some() { 2. } else { 4. };
        let more = f32::from(crate::scale::tpx(super::head::BTN_H));
        let (close_width, close) = measured(controls.close, w, cx);
        let grip = identity.grip.take().map(|el| measured(el, w, cx));
        let grip_room = grip.as_ref().map_or(0., |(width, _)| width + gap);
        let path = if identity.rel.is_empty() { &identity.name } else { &identity.rel };
        let basename = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let full = identity.text(path);
        let short = if path == basename { full.clone() } else { identity.text(&format!("…/{basename}")) };
        let measure = |s: &str| f32::from(w.text_system().shape_line(s.to_owned().into(), crate::scale::tpx(12.), &[w.text_style().to_run(s.len())], None).width).ceil();
        let picker_width = if identity.picker.is_some() { 16. } else { 0. };
        let title_min = if tile_focus.is_some() {
            measure(&identity.text(&readable_source(basename)))
        } else { measure(&short).min(f32::from(crate::scale::tpx(150.))) } + picker_width;
        let room = f32::from(bounds.size.width) - grip_room;
        let address_min = f32::from(crate::scale::tpx(110.));
        let mut widths = vec![];
        let mut items = vec![];
        let tracking = crate::script::header_tracking();
        if tracking && !corner { crate::script::header_present(); }
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
            let wrapper = div().child(inline).when(id.ends_with(":add-beside"), |d| with_rect(d, || id.clone()));
            let wrapped = with_rect(wrapper, || format!("pane-tool:{page}:{id}")).into_any_element();
            let (width, inline) = if id == "br-url" { (address_min, wrapped) } else { measured(wrapped, w, cx) };
            widths.push((priority(&id, i >= page_count), width + gap));
            items.push((id, label, tool.active, inline, action, tool.action.popup));
        }
        // Measure the original title before deciding whether the lowest-priority tool fits.
        let has_add = items.last().is_some_and(|item| item.0.ends_with(":add-beside"));
        let legacy = widths.len() - usize::from(has_add);
        let docked = tile_focus.is_some() && has_add;
        let inline_in_dock = docked && tile_add_inline(bounds.size.width.into(), more);
        let (mut visible, title_room) = if docked {
            let menu_start = items.iter().position(|item| item.0.ends_with(":more:add-beside")).unwrap_or(legacy);
            let reserved_add = if inline_in_dock { widths[legacy].1 } else { 0. };
            let (mut shown, room) = tiled_room(room - reserved_add, close_width, title_min, more, gap, &widths[..menu_start]);
            shown.resize(legacy, false);
            (shown, room)
        } else { header_room(room, close_width, title_min, more, gap, &widths[..legacy]) };
        let address_extra = if items.iter().zip(&visible).any(|(item, show)| item.0 == "br-url" && *show) {
            (title_room - title_min - gap).max(0.)
        } else { 0. };
        let add_room = title_room - address_extra - picker_width;
        let measure = |s: &str| f32::from(w.text_system().shape_line(s.to_owned().into(), crate::scale::tpx(12.), &[w.text_style().to_run(s.len())], None).width).ceil();
        let mut text = if measure(&full) <= title_room { full.clone() } else if measure(&short) <= title_room { short }
            else if tile_focus.is_some() { tile_title(&identity, basename, title_room, measure) }
            else { shorten(&identity, basename, title_room, measure) };
        let mut palette_popup = None;
        if docked {
            if inline_in_dock { visible.push(true); }
            else { palette_popup = items.pop().and_then(|item| item.5); }
        } else if has_add {
            let slot = widths[legacy].1;
            let candidate = (identity.has_source && measure(&text) > add_room - slot)
                .then(|| shorten(&identity, basename, add_room - slot, measure));
            match place_add(&visible, add_room, measure(&text), title_min - picker_width, slot, slot-gap,
                candidate.as_deref().map(measure)) {
                AddPlacement::Inline => visible.push(true),
                AddPlacement::Shorten => { text = candidate.unwrap(); visible.push(true); }
                AddPlacement::More => visible.push(false),
                AddPlacement::Omit => palette_popup = items.pop().and_then(|item| item.5),
            }
        }
        if has_add && visible.len() == widths.len() && visible.last() == Some(&true) {
            items.rotate_right(1);
            visible.rotate_right(1);
        }
        let has_strip = !visible.is_empty();
        let folded = visible.contains(&false);
        let tooltip = [full, identity.detail].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
        let title = with_rect(div().id("pane-identity"), || format!("pane-identity:{page}:{text}"))
            .flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_size(crate::scale::tpx(12.)).when(tile_focus == Some(false), |d| d.text_color(c("dim")))
            .tooltip(crate::keys::tip(&tooltip, "", light)).child(text);
        let title = if let Some((entity, press, popup)) = identity.picker {
            if tracking { crate::script::header_action(&page, "mech-picker", &tooltip, false, Some(press.clone())); }
            div().id("mech-picker").relative().flex().items_center().flex_1().min_w_0().gap_1().cursor_pointer().on_click(listener(&entity, &press))
                .child(title).child(crate::art::icon("chevron-down", 12., c("dim"))).children(popup).into_any_element()
        } else { title.into_any_element() };
        let mut strip = with_rect(div().flex_none().flex().items_center().gap(px(gap)), || format!("pane-tools:{page}"));
        let mut rows = Vec::new();
        let mut actions = Vec::new();
        let mut popups = Vec::new();
        for ((id, label, active, inline, action, popup), show) in items.into_iter().zip(visible) {
            let id = if !show && !id.ends_with(":more:add-beside") {
                id.strip_suffix(":add-beside").map(|tile| format!("{tile}:more:add-beside")).unwrap_or(id)
            } else { id };
            if tracking {
                let press = action.as_ref().map(|(_, p)| if show { p.clone() }
                    else { Press::MoreRow(state.clone(), Box::new(p.clone())) });
                crate::script::header_action(&page, &id, &label, !show, press);
            }
            let inline = if id == "br-url" {
                let width = if show { address_min + address_extra } else { address_min };
                div().w(px(width)).child(inline).into_any_element()
            } else { inline };
            if show {
                strip = strip.child(div().relative().child(inline).children(popup));
                continue;
            }
            if id == "br-url" {
                rows.push(div().id("menu-br-url").child(inline));
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
            if tracking && docked {
                crate::script::header_action(&page, &format!("{scope}:more"), &t("common.more"),
                    false, Some(Press::More(state.clone())));
            }
            let toggle = state.clone();
            let keys = state.clone();
            let rect = trigger_rect.clone();
            let button = with_rect(div().id("pane-more"), || format!("pane-more:{page}"))
                .when(docked, |d| with_rect(with_rect(d, || format!("{scope}:more")), || format!("pane-tool:{page}:{scope}:more")))
                .track_focus(&focus).tab_stop(true).focus(|s| s.opacity(1.).border_1().border_color(c("accent")))
                .w(px(if corner { more.min(room) } else { more })).h(px(more)).flex().justify_center().items_center().cursor_pointer().child("⋯")
                .child(canvas(move |b, _, _| rect.set(b), |_, _, _, _| {}).absolute().inset_0())
                .on_click(move |_, w, cx| toggle_more(&toggle, w, cx))
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
            .children(grip.map(|(_, el)| el)).when(!corner, |r| r.child(title))
            .when(has_strip, |r| r.child(strip)).when(!corner, |r| r.child(close))
            .children(palette_popup).into_any_element();
        row.prepaint_as_root(bounds.origin, size(bounds.size.width.into(), bounds.size.height.into()), w, cx);
        row
    }, |_, mut row, w, cx| row.paint(w, cx));
    if corner { return inner.size_full().into_any_element(); }
    let inner = inner.absolute().top_0().bottom_0()
        .when(tile_focus.is_some(), |d| d.left_1().right_1())
        .when(tile_focus.is_none(), |d| d.left_3().right_3());
    let bar = with_rect(viewer::bar(light).relative(), || format!("pane-header:{}", identity_page)).child(inner);
    if let Some(text) = refusal {
        div().flex_shrink_0().child(bar).child(div().px_2().py_1().text_size(crate::scale::tpx(11.)).text_color(c("warn")).child(text)).into_any_element()
    } else { bar.into_any_element() }
}

#[cfg(test)]
mod tests {
    use super::{header_room, layout, place_add, priority, tile_title, Identity};
    #[test]
    fn embedded_title_only_uses_the_wanted_frames_file() {
        let board = serde_json::json!({"page":"board","source":{"rel":"boards/main.kicad_pcb"}});
        assert_eq!(super::embedded_identity("PCB", &board).rel, "boards/main.kicad_pcb");
        let sheet = serde_json::json!({"page":"schematic","source":{"rel":"main.kicad_sch"}});
        for (frame, facts, key) in [("FOOTPRINT_EDITOR", &board, "viewer.footprinteditor"),
                                    ("SYMBOL_EDITOR", &sheet, "viewer.symboleditor")] {
            let identity = super::embedded_identity(frame, facts);
            assert!(identity.rel.is_empty());
            assert_eq!(identity.text(""), super::t(key));
        }
        for (frame, page) in [("SCH","schematic"), ("SIMULATOR","simulation"), ("PCB_3D","3d"), ("unknown","kicad")] {
            let identity = super::embedded_identity(frame, &board);
            assert_eq!(identity.page, page);
            assert!(identity.rel.is_empty());
        }
    }

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
    fn showing_a_refusal_preserves_the_acted_tiles_identity() {
        let mut identity = Identity::new("files", &json!({"source":{"rel":"docs/guide","name":"guide"}, "version":"v2"}));
        identity.refusal("Too small");
        assert_eq!(identity.text(&identity.rel), "Files · docs/guide · v2");
        assert_eq!(identity.refusal.as_deref(), Some("Too small"));
    }
    #[test]
    fn cramped_tile_title_only_includes_a_separator_with_source_text() {
        let id = Identity::new("board", &json!({"source":{"rel":"directory/longboard.kicad_pcb"}})).tile(false);
        let width = |s: &str| s.chars().count() as f32;
        assert_eq!(tile_title(&id, "longboard.kicad_pcb", width("Board · …"), width), "Board");
        assert_eq!(tile_title(&id, "longboard.kicad_pcb", width("Board · …b"), width), "Board");
        assert_eq!(tile_title(&id, "longboard.kicad_pcb", width("Board · longbo…ad_pcb"), width), "Board · longbo…ad_pcb");
        assert_eq!(tile_title(&id, "a.sch", width("Board · a.sch"), width), "Board · a.sch");
        assert_eq!(tile_title(&id, "a.sch", width("Board · a.sc"), width), "Board");
        assert_eq!(tile_title(&id, "", 15., width), "Board");
    }

    #[test]
    fn add_preserves_legacy_header_and_dock_controls() {
        use super::AddPlacement::*;
        assert_eq!(place_add(&[true, true], 500., 50., 50., 28., 24., None), Inline);
        assert_eq!(place_add(&[true, false], 500., 50., 50., 28., 24., None), Inline);
        assert_eq!(place_add(&[false], 0., 50., 50., 28., 24., None), More);
        assert_eq!(place_add(&[true], 62., 50., 50., 28., 24., None), Omit);
        let (shown, _) = header_room(591., 30., 187., 30., 4.,
            &[(0, 67.), (0, 45.), (1, 112.), (1, 120.)]);
        assert_eq!(shown, [true; 4]);
    }

    #[test]
    fn standard_headers_fit_add_in_the_gap_before_the_first_tool() {
        for (ink_end, first_tool) in [(921., 1040.), (945., 1040.), (891., 1046.),
            (855., 1092.), (891., 1140.)] {
            // Title begins at 765; its allocation ends one gap before the first tool.
            let title = ink_end - 765.;
            let room = first_tool - 4. - 765.;
            assert_eq!(place_add(&[true], room, title, title, 28., 24., None),
                super::AddPlacement::Inline);
        }
    }

    #[test]
    fn tiled_more_is_reserved_once_before_assigning_tools() {
        let tools = [(0, 60.), (1, 40.), (2, 90.)];
        assert_eq!(super::tiled_room(210., 30., 50., 30., 2., &tools),
            (vec![true, false, false], 86.));
        assert_eq!(super::tiled_room(340., 30., 50., 30., 2., &tools),
            (vec![true; 3], 86.));
    }

    #[test]
    fn equal_tile_widths_choose_the_same_add_and_fold_fit_before_layers() {
        let minimum = 130.;
        let button = 24.;
        for (width, inline) in [(287., false), (288., true)] {
            assert_eq!(super::tile_add_inline(width, button), inline);
            for layers in [55., 76.] {
                let tools = [(priority("tile:t2:fit", true), 38.),
                             (priority("tile:t2:layers", false), layers)];
                let (shown, title) = super::tiled_room(270., 26., minimum, button, 2., &tools);
                assert_eq!(shown, [false, true]);
                assert!(title >= minimum);
                assert_eq!(super::tiled_room(230., 26., minimum, button, 2., &tools).0, [false; 2]);
            }
        }
    }

    #[test]
    fn title_only_add_respects_the_source_minimum_and_shortening_budget() {
        use super::AddPlacement::*;
        assert_eq!(place_add(&[], 200., 172., 150., 28., 24., None), Inline);
        assert_eq!(place_add(&[], 200., 190., 150., 28., 24., Some(170.)), Shorten);
        assert_eq!(place_add(&[], 200., 190., 150., 28., 24., Some(165.)), Omit);
        assert_eq!(place_add(&[], 170., 158., 150., 28., 24., Some(142.)), Omit);
        assert_eq!(place_add(&[], 200., 190., 150., 28., 24., None), Omit);
    }

    #[test]
    fn tile_plot_tools_fold_after_lower_priority_actions() {
        let widths = [(priority("tile:t2:layers", false), 70.),
            (priority("tile:t2:fit", true), 40.), (priority("vs-board", true), 90.)];
        assert_eq!(layout(160., 30., &widths), [true, true, false]);
        assert_eq!(layout(35., 30., &widths), [false, false, false]);
    }

    #[test]
    fn split_names_the_available_design() {
        let mut facts = json!({"panes":[{"page":"schematic","source":{"rel":"amp.sch"}},{"page":"board","missing":true}]});
        assert_eq!(Identity::new("split", &facts).rel, "amp.sch");
        facts["panes"][1] = json!({"page":"board","source":{"rel":"layout/amp.pcb"}});
        assert_eq!(Identity::new("split", &facts).rel, "layout/amp.pcb");
    }
}
