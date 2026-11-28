//! Settings and the other row lists the hub describes (`settings.rows`: label, note, buttons, fields, QR code).
//! Pure parsing and a generic renderer: every button is the view action the row names, every typed field goes out as
//! `bfield` ("<name>\u{1f}<text>") exactly as on the phones. Nothing here knows what a setting means.
use super::composer::{Composer, ComposerEvent};
use crate::{hub::Command, i18n::{tf, trs}, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Btn { pub label: String, pub action: String, pub value: String, pub on: bool }
/// A field to type in. A `vault` field (a key or a secret's value) is the app's own: what is typed is held here and handed over
/// once, at Save, in one request; it is never a `bfield`, never taken back from the hub, never in a format string (no Debug).
#[derive(Clone, PartialEq)]
pub struct Field { pub name: String, pub hint: String, pub secret: bool, pub value: String, pub vault: bool }
#[derive(Clone, Debug, PartialEq, Default)]
/// `tone`: "warn" or "bad" draws the note in that colour (a failed install, a code that names only this computer); empty is the usual.
/// `kind`: a row the hub draws as more than words and controls (a card, a group label, a notice...: cards.rs); `raw` is the row as the hub sent it.
pub struct Row { pub label: String, pub note: String, pub buttons: Vec<Btn>, pub fields: Vec<Field>, pub qr: Vec<String>, pub qrnote: String, pub local: bool, pub tone: String, pub kind: String, pub raw: Value }
/* i18n-ok: drawn through trs by row_el, nav_title or the title */ impl std::fmt::Debug for Field { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Field({}{})", self.name, if self.vault { ", vault" } else { "" }) } }

/// A request to the hub that carries a secret: its Debug shows nothing.
#[derive(Clone)]
pub struct Secret(pub Value);
impl std::fmt::Debug for Secret { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Secret(..)") } }

/// At most this many rows are drawn at once; the rest are said ("N more"), never hidden.
pub const MAX_ROWS: usize = 300;

pub fn parse(settings: &Value) -> Vec<Row> {
    settings["rows"].as_array().into_iter().flatten().map(|r| Row {
        label: string(r, "label"), note: string(r, "note"),
        buttons: r["buttons"].as_array().into_iter().flatten().map(|b| Btn { label: string(b, "label"), action: string(b, "action"), value: string(b, "value"), on: b["on"].as_bool().unwrap_or(false) }).collect(),
        fields: r["fields"].as_array().into_iter().flatten().map(|f| { let vault = f["vault"].as_bool().unwrap_or(false); Field { name: string(f, "name"), hint: string(f, "hint"), secret: f["secret"].as_bool().unwrap_or(false), value: if vault { String::new() } else { string(f, "value") }, vault } }).collect(),
        qr: r["qr"].as_array().into_iter().flatten().filter_map(|l| l.as_str().map(str::to_string)).collect(),
        qrnote: string(r, "qrnote"), local: r["local"].as_bool().unwrap_or(false), tone: string(r, "tone"),
        kind: string(r, "kind"), raw: if r["kind"].is_string() || r["in"] == true || r["progress"].is_number() || r["sublink"].is_object() || r["adv"] == true || r["changed"] == true || r["intro"] == true
            || r["fields"].as_array().map_or(false, |f| f.iter().any(|f| f["w"].is_u64())) { r.clone() } else { Value::Null },
    }).collect()
}

/// What a typed change sends: a form field's text as `bfield`; a vault field's text never (None).
pub fn change(f: &Field, text: &str) -> Option<Command> { if f.vault { None } else { Some(Command::Action("bfield".into(), format!("{}\u{1f}{}", f.name, text))) } }
pub fn field_command(name: &str, text: &str) -> Command { Command::Action("bfield".into(), format!("{name}\u{1f}{text}")) }
/// What a button press sends: its action and value, except the vault Saves, which are `save_request`.
pub fn press(b: &Btn) -> Command { Command::Action(b.action.clone(), b.value.clone()) }
/// `open-url` is the client's own (it never goes to the hub): only web addresses open, in the system browser.
pub fn openable(url: &str) -> bool { (url.starts_with("https://") || url.starts_with("http://")) && url.len() <= 8000 && !url.chars().any(|c| c.is_whitespace() || c.is_control()) }
pub fn is_vault_save(b: &Btn) -> bool { b.action == "secret-save" }
/// The masked box a sign-in panel's Send reads (`@signin...`), None for the other vault Saves.
pub fn signin_field(packed: &str) -> Option<&'static str> {
    if packed == "@signin-sync" { Some("sicode") } else if packed.starts_with("@signin\u{1f}") || packed.starts_with("@signin-key\u{1f}") { Some("sikeys") } else { None }
}

/// The request a vault Save sends, straight to the hub (never a view action): an API key (`@apikey` + name) is `bots.voice`
/// `api-add`; a secret ("name\u001fpurpose\u001fthread") is `secret.set`. Nothing is sent without a name and a value.
pub fn save_request(packed: &str, typed_name: &str, value: &str) -> Result<(String, Value), &'static str> {
    // a line or a key typed into a sign-in on another machine, or the sync token's code (host present/signin_there.rs): Settings, Machines' own request
    if let Some(field) = signin_field(packed) {
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if value.trim().is_empty() { return Err("Type it first."); }
        return Ok(("machines.page.act".into(), serde_json::json!({"action": "secret-save", "value": packed, "fields": {field: value}})));
    }
    let value = value.trim();
    if packed.starts_with("@apikey\u{1f}") {
        let rest = &packed["@apikey\u{1f}".len()..];
        let name = if typed_name.trim().is_empty() { rest.trim() } else { typed_name.trim() };
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if name.is_empty() { return Err("Name the key first."); }
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if value.is_empty() { return Err("Paste the key first."); }
        return Ok(("bots.voice".into(), serde_json::json!({"op": "api-add", "name": name, "value": value})));
    }
    let mut it = packed.split('\u{1f}');
    let (packed_name, purpose, thread) = (it.next().unwrap_or("").trim(), it.next().unwrap_or(""), it.next().unwrap_or(""));
    let name = if typed_name.trim().is_empty() { packed_name } else { typed_name.trim() };
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if name.is_empty() { return Err("Name the secret first."); }
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if value.is_empty() { return Err("Paste the secret first."); }
    Ok(("secret.set".into(), serde_json::json!({"name": name, "purpose": purpose, "thread": thread, "value": value})))
}

/// Recent texts sent for a field, so the hub's late echo of an older text is not mistaken for a change made elsewhere. Replies come in
/// order: once the hub echoes the newest text sent, everything earlier is settled and any other value is a real change.
const SENT_KEPT: usize = 32;
#[derive(Default, Debug)]
pub struct Echo { sent: String, log: std::collections::VecDeque<String> }
impl Echo {
    pub fn new(v: &str) -> Self { let mut e = Echo::default(); e.remember(v); e }
    pub fn remember(&mut self, t: &str) { self.sent = t.to_string(); self.log.push_back(t.to_string()); while self.log.len() > SENT_KEPT { self.log.pop_front(); } }
    pub fn sent(&self) -> &str { &self.sent }
    /// The hub shows `hub` while `typed` is in the field: Some(true) take it, Some(false) ignore it (an echo or the same text).
    pub fn on_hub(&mut self, hub: &str, typed: &str) -> bool {
        if hub == self.sent { self.log.clear(); self.log.push_back(self.sent.clone()); return false; }   // caught up
        hub != typed && !self.log.iter().any(|s| s == hub)
    }
}

/// The settings dialog's own state (the hub keeps the page; this keeps the list the left nav shows and what is typed in its search).
#[derive(Default)]
pub struct SettingsUi { pub accounts: bool, pub nav: Vec<(String, String)>, /** page -> its group's name (the hub's `group`) */ pub groups: HashMap<String, String>, /** page -> each of its rows' label, note and search words (the hub's `words`, `notes` and `also`): what the search finds besides titles */ pub finds: HashMap<String, Vec<Find>>,
    /** page -> its other names (the hub's `aka`, one a line): found by the search as its title is */ pub akas: HashMap<String, Vec<String>>, pub page: String, pub q: String, pub search: Option<Entity<Composer>>, pub auto: bool, pub open_local: bool,
    /** the Keyboard page of this device */ pub keys: bool, /** the Hubs page of this device */ pub hubs: bool, /** the pages whose Advanced fold is open (while the app runs) */ pub adv: HashSet<String>,
    /** the rows' scroll: a page opens at its top */ pub scroll: ScrollHandle, /** the hub page last shown (its title), to tell when it changes */ pub shown: String,
    /** the search's results highlighted by Up and Down (Enter opens it) */ pub hit: usize, /** a result or a page was opened since the search last changed: the page shows, not the results */ pub landed: bool,
    /** a result pressed: its page, label and which row of that label it is, until the page draws it (`land`) */ pub pending: Option<(String, String, usize)>,
    /** the row a result led to, lit for `LIT_MS` */ pub lit: Option<(String, String, usize)>, lit_gen: u64, /** where the page was scrolled when the results took its place */ page_at: Option<Point<Pixels>>,
    /** the shortcut whose button was pressed, waiting for the keys typed for it (`recorded`) */ pub rec: Option<String>, /** a chord refused for a shortcut: its id and why, its row's note until the next press */ pub rec_note: Option<(String, String)>,
    /** what the hub refused while Settings was open (a Pair, a Save): said beside the page's title until the next press, page or close */ pub refused: Option<String>,
    /** the page Settings was on when it shut (`current`): the gear opens it again while the app runs (`reopen`) */ pub left: String }
/// A setting the search can find on a page: its label, its note and the hub's plain search words for it (`also`: what it does, what it was called).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Find { pub label: String, pub note: String, pub also: String }
/// What the search found on a page: a row (its label, which of the page's rows with that label it is, its note), or the page itself (`row` false:
/// its title or other name matched and none of its rows did; `label` is the page's title).
#[derive(Clone, Debug, PartialEq)]
pub struct Found { pub label: String, pub nth: usize, pub note: String, pub row: bool }
/// How long the row a result led to stays lit.
pub const LIT_MS: u64 = 1500;
/// How the dialog fits a window of `w` x `h` px: the margin around it, the page list's width, and whether a row's controls go under its words
/// (a narrow page has no room beside them). The page list and the rows each scroll on their own, so nothing is out of reach on a short window.
/// `rows` is the width the rows are laid out in (the page less its side margins, `gutter` each); each row decides from it whether its controls
/// fit beside its words (`row_fit`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit { pub pad: f32, pub nav: f32, pub gutter: f32, pub rows: f32 }
pub fn fit(w: f32, h: f32) -> Fit {
    let pad = if w < 900. || h < 640. { 8. } else { 24. };
    let dialog = (w - 2. * pad).min(1040.);
    let nav = if dialog < 640. { 150. } else if dialog < 860. { 180. } else { 220. };
    let gutter = if dialog < 640. { 12. } else { 24. };
    Fit { pad, nav, gutter, rows: (dialog - 2. - nav - 2. * gutter).max(120.) }
}
/// The least width a row's words keep beside its controls; narrower, the controls go under them.
pub const LABEL_MIN: f32 = 220.0;
/// The widest the controls beside the words get.
pub const CONTROLS_MAX: f32 = 430.0;
const GAP: f32 = 16.0;
/// A chip's width at the settings' 12 px (its 12 px padding each side): wide (CJK) characters count as a full em. An estimate on the generous side,
/// so a line of chips is never wider than the room it was packed for.
/// The words grow with the text size (scale.rs); the padding does not.
/// A short box's width (the hub's `w` on a field: about how many characters it holds) in the same maths: 8 px of padding each side.
pub fn short_w(chars: u64) -> f32 { 16. + crate::scale::s() * 7.6 * chars as f32 }
/// The short boxes that end a row (its last fields that each say `w`, the hub's `field_w`), their widths in order; empty when the last field
/// does not say it. They go on one line with the row's buttons (`short_line_w`), after any longer box above them.
pub fn short_tail(raw: &Value, n: usize) -> Vec<f32> {
    let ws: Vec<Option<u64>> = (0..n).map(|k| raw["fields"][k]["w"].as_u64().filter(|w| *w > 0)).collect();
    let k = ws.iter().rposition(|w| w.is_none()).map_or(0, |i| i + 1);
    ws[k..].iter().map(|w| short_w(w.unwrap_or(0))).collect()
}
/// The width of the line of short boxes and buttons: each as wide as it is drawn, 4 px between them.
pub fn short_line_w(boxes: &[f32], buttons: &[f32]) -> f32 { let all = boxes.iter().chain(buttons); all.clone().sum::<f32>() + 4. * all.count().saturating_sub(1) as f32 }
pub fn chip_w(label: &str) -> f32 { 24. + crate::scale::s() * label.chars().map(|c| if (c as u32) >= 0x1100 && !c.is_whitespace() { 12.5 } else { 7.6 }).sum::<f32>() }

/// A row's slider (a local row's `raw.slider`: the `local:<name>` it sets, its range, step and value): dragged or pressed anywhere on its
/// track, it sets the value at that point, snapped to a step.
#[derive(Clone, Debug, PartialEq)]
pub struct Slider { pub name: String, pub min: u32, pub max: u32, pub step: u32, pub value: u32 }
pub const SLIDER_W: f32 = 220.;
const THUMB: f32 = 14.;
pub fn slider_of(raw: &Value) -> Option<Slider> {
    let o = raw["slider"].as_object()?; let n = |k: &str| o.get(k).and_then(Value::as_u64).map(|v| v as u32);
    let (min, max) = (n("min")?, n("max")?); if max <= min { return None }
    Some(Slider { name: o.get("name")?.as_str()?.to_string(), min, max, step: n("step").unwrap_or(1).max(1), value: n("value").unwrap_or(min).clamp(min, max) })
}
/// Where along the track (0 to 1) a value sits.
pub fn slider_frac(sl: &Slider) -> f32 { (sl.value.clamp(sl.min, sl.max) - sl.min) as f32 / (sl.max - sl.min) as f32 }
/// The value at `x` on a track that starts at `left` and is `w` wide: the thumb's centre runs from half a thumb in to half a thumb from the end.
pub fn slider_at(sl: &Slider, x: f32, left: f32, w: f32) -> u32 {
    let run = (w - THUMB).max(1.); let f = ((x - left - THUMB / 2.) / run).clamp(0., 1.);
    let v = sl.min as f32 + f * (sl.max - sl.min) as f32;
    crate::local::snap(v.round() as u32, (sl.min, sl.max, sl.step))
}
struct SliderDrag(Slider);
struct NoGhost;
impl Render for NoGhost { fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div() } }
// the track's left edge and width as last drawn, and the value last set (a drag sets it only when it moves to another stop)
thread_local! { static SLIDER_BOX: std::cell::Cell<(f32, f32)> = const { std::cell::Cell::new((0., 0.)) }; static SLIDER_LAST: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
// the window's text registry while Settings or setup draws (`with_sel`): their words select and copy as a message's do (timeline.rs, "outside")
thread_local! { static SEL: std::cell::RefCell<Option<super::timeline::Reg>> = const { std::cell::RefCell::new(None) }; }
/// Draw Settings or setup (`f`) with their words selectable, as the blocks of `id` (each frame's replace the last; closed, there are none).
pub fn with_sel<R>(reg: &super::timeline::Reg, id: &str, f: impl FnOnce() -> R) -> R {
    SEL.with(|s| *s.borrow_mut() = Some(reg.clone()));
    let out = super::timeline::Registry::scoped(reg, 0, &format!("{}{id}", super::timeline::SET_ID), f);
    SEL.with(|s| *s.borrow_mut() = None); out
}
/// `t` in `d`: selectable text (drag, double/triple click, Copy) in the colour and size `d` gives it; outside `with_sel` plain text.
pub(crate) fn words(cx: &mut Context<Backplane>, light: bool, d: Div, t: String) -> Div {
    let Some(reg) = SEL.with(|s| s.borrow().clone()) else { return d.child(t) };
    d.on_mouse_down(MouseButton::Left, cx.listener(|this, e, w, cx| this.doc_down(e, w, cx))).on_mouse_down(MouseButton::Right, cx.listener(|this, e, w, cx| this.doc_down(e, w, cx)))
        .on_mouse_move(cx.listener(|this, e, _, cx| this.doc_move(e, cx))).child(super::timeline::line_text(&reg, light, t, ""))
}
fn slider_set(this: &mut Backplane, sl: &Slider, x: f32, cx: &mut Context<Backplane>) {
    let (left, w) = SLIDER_BOX.with(|b| b.get()); if w <= 0. { return }
    let v = slider_at(sl, x, left, w);
    if SLIDER_LAST.with(|l| l.replace(v)) != v { this.local_press(&sl.name, &v.to_string(), cx); }
}
fn slider_el(cx: &mut Context<Backplane>, light: bool, sl: &Slider) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let f = slider_frac(sl);
    let down = sl.clone();
    let n = format!("slider:{}", sl.name);
    div().id(SharedString::from(n.clone())).relative().w(px(SLIDER_W)).h(px(24.)).flex_none().cursor_pointer()
        .child(canvas(move |b, w, _| { SLIDER_BOX.with(|x| x.set((f32::from(b.left()), f32::from(b.size.width)))); crate::art::report(&n, b, w.scale_factor()); }, |_, _, _, _| {}).absolute().size_full())
        // the track, filled up to the thumb
        .child(div().absolute().left(px(THUMB / 2.)).right(px(THUMB / 2.)).top(px(10.)).h(px(4.)).bg(c("raise")).child(div().h_full().w(relative(f)).bg(c("accent"))))
        .child(div().absolute().top(px(5.)).left(px(f * (SLIDER_W - THUMB))).size(px(THUMB)).bg(c("accent")).border_2().border_color(c("bg")))
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| { SLIDER_LAST.with(|l| l.set(down.value)); slider_set(this, &down, f32::from(e.position.x), cx) }))
        .on_drag(SliderDrag(sl.clone()), |_, _, _, cx| cx.new(|_| NoGhost))
        .on_drag_move(cx.listener(move |this, e: &DragMoveEvent<SliderDrag>, _, cx| { let sl = e.drag(cx).0.clone(); slider_set(this, &sl, f32::from(e.event.position.x), cx) }))
        .into_any_element()
}
/// How a row lays out in `avail` px: whether its controls go under its words (`stack`), and its chips as lines (how many on each), each line
/// no wider than the room it has (a single chip wider than that has a line of its own). `field`/`qr`: the widths of a text field, a QR code.
#[derive(Clone, Debug, PartialEq)]
pub struct RowFit { pub stack: bool, pub lines: Vec<usize> }
pub fn row_fit(avail: f32, chips: &[f32], field: f32, qr: f32) -> RowFit {
    let side = (avail - LABEL_MIN - GAP).min(CONTROLS_MAX);
    let widest = chips.iter().copied().fold(0.0f32, f32::max).max(field).max(qr);
    // beside the words when the widest single control fits there; the chips then wrap within that room
    let stack = widest > side;
    let room = if stack { avail } else { side };
    let mut lines = Vec::new(); let (mut n, mut w) = (0usize, 0.0f32);
    for &c in chips {
        let next = if n == 0 { c } else { w + 4. + c };
        if n > 0 && (next > room || n == CHIPS_PER_LINE) { lines.push(n); n = 1; w = c; } else { n += 1; w = next; }
    }
    if n > 0 { lines.push(n); }
    RowFit { stack, lines }
}
/// A row's button as wide as it is drawn: a copy press (`copyable::chip`) carries its icon and keeps the room of the wider of its label and
/// "Copied", so it neither grows under the pointer nor ends past the edge the other buttons share.
pub fn btn_w(b: &Btn) -> f32 {
    if b.action != "copy" { return chip_w(&trs(&b.label)) }
    super::copyable::CHIP_ICON + chip_w(&trs(&b.label)).max(chip_w(&trs("Copied")))
}
/// The least a text box keeps beside its row's buttons; narrower, the buttons go under it.
pub const FIELD_MIN: f32 = 140.;
/// The width of a row's last text box when its buttons share its line: the room the box would have alone (`field_w`) less the buttons (their
/// widths as `btn_w` estimates them, on the generous side) and a 4 px gap before each; None when that leaves under `FIELD_MIN` (the buttons
/// then go under the box). The line is drawn `field_w` wide, the box taking what the buttons leave, so it is never narrower than this.
pub fn field_beside(field_w: f32, buttons: &[f32]) -> Option<f32> {
    if buttons.is_empty() { return None }
    let w = field_w - buttons.iter().map(|b| b + 4.).sum::<f32>();
    (w >= FIELD_MIN).then_some(w)
}
/// What this device's settings differ in from a fresh install (`fresh`: `device_rows(&Local::default())`), as the hub marks its own rows
/// (host present/settings.rs `mark_changed`): a row whose lit choice is another than the fresh one's (matched by label, and which of the rows with
/// that label it is) gets `changed`, and `reset`, the fresh lit press, when its buttons press the same in both. A shortcut is changed when its key
/// is not its default one, and its Reset puts that one back (`local:shortcut-reset`; "Shortcuts to defaults" puts them all back). The text size
/// has its own Reset, and lights nothing to compare.
pub fn mark_device(rows: &mut [Row], fresh: &[Row]) {
    let nth = |all: &[Row], i: usize| all[..i].iter().filter(|r| r.label == all[i].label).count();
    let lit = |r: &Row| -> Vec<Btn> { r.buttons.iter().filter(|b| b.on).cloned().collect() };
    let presses = |r: &Row| -> Vec<(String, String)> { r.buttons.iter().map(|b| (b.action.clone(), b.value.clone())).collect() };
    let all: &[Row] = rows;
    let marks: Vec<Option<Option<Btn>>> = (0..all.len()).map(|i| {
        let r = &all[i];
        let d = fresh.iter().filter(|f| f.label == r.label).nth(nth(all, i))?;
        if r.buttons.len() == 1 && r.buttons[0].action == "local:shortcut" { return (d.buttons.first().map(|b| &b.label) != Some(&r.buttons[0].label)).then(|| Some(Btn { label: String::new(), action: "local:shortcut-reset".into(), value: r.buttons[0].value.clone(), on: false })) }
        let (now, was) = (lit(r), lit(d));
        let names = |v: &[Btn]| v.iter().map(|b| b.label.clone()).collect::<Vec<_>>();
        if now.is_empty() || was.is_empty() || names(&now) == names(&was) { return None }
        Some((was.len() == 1 && presses(r) == presses(d)).then(|| was[0].clone()))
    }).collect();
    for (r, m) in rows.iter_mut().zip(marks) {
        let Some(m) = m else { continue };
        r.raw["changed"] = Value::Bool(true);
        if let Some(b) = m { r.raw["reset"] = serde_json::json!({"action": b.action, "value": b.value}); }
    }
}
/// The shortcut waiting for its keys (`rec`): its button reads "Type the keys…", lit; a chord refused for one (`note`: its id and why) is that
/// row's note, in the warn tone. Applied after `mark_device`, which compares the keys the buttons name.
pub fn show_recording(rows: &mut [Row], rec: Option<&str>, note: Option<&(String, String)>) {
    for r in rows.iter_mut() {
        let Some(b) = r.buttons.first_mut().filter(|b| b.action == "local:shortcut") else { continue };
        if let Some((_, why)) = note.filter(|(id, _)| *id == b.value) { r.note = why.clone(); r.tone = "warn".into(); }
        /* i18n-ok: drawn through trs by chip_el */ if rec == Some(b.value.as_str()) { b.label = "Type the keys…".into(); b.on = true; }
    }
}
/// A key typed while a shortcut waits for its keys (`SettingsUi::rec`), taken here before any binding sees it (true; false when nothing waits):
/// it becomes the shortcut and the wait ends, or it is refused and the row says why until the next press, or Esc ends the wait (Settings stays
/// open); a modifier alone waits on (`keys::take`).
pub fn recorded(this: &mut Backplane, k: &Keystroke, cx: &mut Context<Backplane>) -> bool {
    let Some(id) = this.screens.settings_ui.rec.clone() else { return false };
    if !is_open(this) { this.screens.settings_ui.rec = None; return false }
    let took = crate::keys::take(&this.local, &id, k);
    let ui = &mut this.screens.settings_ui;
    match took {
        crate::keys::Take::Wait => return true,
        crate::keys::Take::Cancel => { ui.rec = None; ui.rec_note = None; }
        crate::keys::Take::Refuse(why) => ui.rec_note = Some((id, why)),
        crate::keys::Take::Set(key) => { ui.rec = None; ui.rec_note = None; this.local_press("shortcut", &format!("{id}\u{1f}{key}"), cx); }
    }
    cx.notify(); true
}
/// A press anywhere in the dialog ends a shortcut's wait for its keys (a press on a shortcut's button then starts its own). True when one waited.
pub fn end_recording(ui: &mut SettingsUi) -> bool { ui.rec_note = None; ui.rec.take().is_some() }
/// A group's name in the nav: the hub's English name, drawn through its own message (`gpui.settings.group.<name>`) where there is one, so a
/// group is not read as the verb or another screen's word that says the same in English ("Connect").
pub fn group_name(h: &str) -> String {
    let key = format!("gpui.settings.group.{}", h.to_lowercase());
    if crate::i18n::catalog().has_lang(&key, crate::i18n::Lang::En) { crate::i18n::t(&key) } else { trs(h) }
}
/// A page list entry's icon keeps its size and sits on its title's first line; a title too long for the list wraps onto a second line
/// (never cut, never under the icon).
fn nav_icon_el(icon: AnyElement) -> Div { div().flex_shrink_0().mt(px(((13. * NAV_LINE * crate::scale::s() - 16.) / 2.).max(0.))).child(icon) }
fn nav_title(t: &str) -> Div { div().flex_1().min_w_0().child(trs(t)) }
/// A text line's height over its size (gpui's default line height, the golden ratio): the nav's 16 px icon is centred on its title's first
/// line by a margin above it, so the entry is no taller than its title.
const NAV_LINE: f32 = 1.618;
/// An icon (from the set the client carries) for each page.
pub fn nav_icon(page: &str) -> &'static str {
    match page { "appearance" => "sun", "threads" => "message-square", "git" => "git-branch", "notifications" => "bell", "agents" => "cpu", "mcp" => "plug", "writing" => "square-pen", "cad" => "circuit-board", "ordering" => "shopping-cart",
        "devices" => "link", "network" => "globe", "browser" => "external-link", "voice" => "mic", "keys" => "code", "timezone" => "clock", "rules" => "circle-check", "usage" => "gauge", "about" => "book", "privacy" => "file-text",
        "machines" => "terminal", "mouse" => "zoom-in", "setup" => "wrench", "developer" => "flask-conical", "@device" => "settings", "@keys" => "settings", "@hubs" => "globe", "@accounts" => "user", _ => "settings" }
}
/// The hub shows its page menu (rows that are each one `settings-page` button naming a page) or one page (its first row is the way back).
#[derive(Debug, PartialEq)]
pub enum Shown { Menu(Vec<(String, String)>), Page, Other }
pub fn shown(rows: &[Row]) -> Shown {
    let items: Vec<(String, String)> = rows.iter().filter_map(|r| r.buttons.iter().find(|b| b.action == "settings-page" && !b.value.is_empty()).map(|b| (b.value.clone(), r.label.clone()))).collect();
    if items.len() >= 3 && items.len() * 2 >= rows.len() { return Shown::Menu(items); }
    if rows.first().map_or(false, |r| r.buttons.iter().any(|b| b.action == "settings-page" && b.value.is_empty())) { return Shown::Page; }
    Shown::Other
}
/// Whether a nav title matches what is typed in the search (an empty search matches all).
pub fn nav_matches(title: &str, q: &str) -> bool { let q = q.trim().to_lowercase(); q.is_empty() || title.to_lowercase().contains(&q) }
fn menu_page(r: &Value) -> Option<String> { r["buttons"].as_array()?.iter().find(|b| b["action"] == "settings-page" && b["value"].as_str().map_or(false, |v| !v.is_empty()))?["value"].as_str().map(str::to_string) }
/// The hub's menu rows' `group` by page (older hubs send none: the nav is then one list).
pub fn menu_groups(rows: &Value) -> HashMap<String, String> {
    rows.as_array().into_iter().flatten().filter_map(|r| Some((menu_page(r)?, r["group"].as_str().filter(|g| !g.is_empty())?.to_string()))).collect()
}
/// What each page holds, by page: its rows' labels (the hub's `words`, one a line) each with its note and its search words (the same line of
/// `notes` and of `also`; older hubs send no notes or no also, and none at all: the search then matches titles only).
pub fn menu_finds(rows: &Value) -> HashMap<String, Vec<Find>> {
    rows.as_array().into_iter().flatten().filter_map(|r| { let words = r["words"].as_str()?; let (mut notes, mut also) = (r["notes"].as_str().unwrap_or("").split('\n'), r["also"].as_str().unwrap_or("").split('\n'));
        Some((menu_page(r)?, words.split('\n').map(|w| Find { label: w.to_string(), note: notes.next().unwrap_or("").to_string(), also: also.next().unwrap_or("").to_string() }).filter(|f| !f.label.is_empty()).collect())) }).collect()
}
/// Each page's other names (the hub's `aka`, one a line: what it was called before the pages were regrouped, or what it holds); older hubs send none.
pub fn menu_akas(rows: &Value) -> HashMap<String, Vec<String>> {
    rows.as_array().into_iter().flatten().filter_map(|r| Some((menu_page(r)?, r["aka"].as_str()?.split('\n').filter(|a| !a.trim().is_empty()).map(str::to_string).collect::<Vec<_>>()))).filter(|(_, a)| !a.is_empty()).collect()
}
/// The settings of a page the search finds: the rows whose label or note holds what is typed, as written or as this language shows it, or
/// whose search words (`also`) do, in the page's order; a label the page has twice is found as each of its rows (`nth`). Nothing for an empty search.
pub fn hits(finds: &[Find], q: &str) -> Vec<Found> {
    let q = q.trim().to_lowercase(); if q.is_empty() { return vec![] }
    let has = |t: &str| !t.is_empty() && (t.to_lowercase().contains(&q) || trs(t).to_lowercase().contains(&q));
    finds.iter().enumerate().filter(|(_, f)| has(&f.label) || has(&f.note) || f.also.to_lowercase().contains(&q))
        .map(|(i, f)| Found { label: f.label.clone(), nth: finds[..i].iter().filter(|x| x.label == f.label).count(), note: f.note.clone(), row: true }).collect()
}
/// A line of the nav: a group's name, or a page (`id`: a hub page, or `@keys`, `@hubs`, `@accounts` for the three that are this app's) with
/// what the search found on it (nothing while nothing is typed).
#[derive(Clone, Debug, PartialEq)]
pub enum NavLine { Head(String), Item { id: String, title: String, hits: Vec<Found> } }
/// The nav with no hub: the pages that hold this device's rows (`device_section`) and the Hubs page, under the groups the hub would put them in.
/* i18n-ok: drawn through trs by row_el, nav_title or the title */ pub const LOCAL_PAGES: &[(&str, &str, &str)] = &[("General", "appearance", "Appearance"), ("General", "notifications", "Notifications"), ("General", "@keys", "Keyboard"), ("Connect", "@hubs", "Hubs"), ("System", "privacy", "Privacy")];
/// The nav as drawn: the hub's pages in its order under its group names, this app's placed among them: Accounts before Agents, Keyboard before
/// Mouse and trackpad, Hubs after Machines (a hub without the page they follow, an older one, has them at the end under "This device", with
/// any page of `LOCAL_PAGES` it does not serve). No hub: `LOCAL_PAGES`. A search keeps every page where it is and says what it found on each: the
/// settings that match (the hub's `finds` or this device's rows on that page, `device`), else the page itself when its title or other name
/// (`akas`) does. A hub that names no groups (an older one) lists its pages under "All devices (hub)".
pub fn nav_lines(nav: &[(String, String)], groups: &HashMap<String, String>, finds: &HashMap<String, Vec<Find>>, akas: &HashMap<String, Vec<String>>, device: &HashMap<String, Vec<Find>>, q: &str) -> Vec<NavLine> {
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ const HUB: &str = "All devices (hub)";
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ const HERE: &str = "This device";
    let none: Vec<Find> = Vec::new();
    // this device's rows first, as the page draws them
    let pairs = |id: &str| -> Vec<Find> { device.get(id).unwrap_or(&none).iter().chain(finds.get(id).unwrap_or(&none)).cloned().collect() };
    let mut out: Vec<NavLine> = Vec::new(); let mut last = String::new();
    let mut put = |head: &str, id: &str, title: &str, pairs: &[Find]| {
        let mut found = hits(pairs, q);
        let named = !q.trim().is_empty() && (nav_matches(title, q) || nav_matches(&trs(title), q) || akas.get(id).map_or(&[][..], Vec::as_slice).iter().any(|a| nav_matches(a, q) || nav_matches(&trs(a), q)));
        if found.is_empty() && named { found.push(Found { label: title.to_string(), nth: 0, note: String::new(), row: false }); }
        if last != head { last = head.to_string(); out.push(NavLine::Head(head.to_string())); }
        out.push(NavLine::Item { id: id.to_string(), title: title.to_string(), hits: found });
    };
    if nav.is_empty() { for (head, id, title) in LOCAL_PAGES { put(head, id, title, &pairs(id)); } return out }
    let has = |p: &str| nav.iter().any(|(id, _)| id == p);
    for (page, title) in nav {
        let head = groups.get(page).map_or(HUB, String::as_str);
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if page == "agents" { put(head, "@accounts", "Accounts", &[]); }
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if page == "mouse" { put(head, "@keys", "Keyboard", &pairs("@keys")); }
        put(head, page, title, &pairs(page));
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if page == "machines" { put(head, "@hubs", "Hubs", &[]); }
    }
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if !has("agents") { put(HERE, "@accounts", "Accounts", &[]); }
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if !has("mouse") { put(HERE, "@keys", "Keyboard", &pairs("@keys")); }
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if !has("machines") { put(HERE, "@hubs", "Hubs", &[]); }
    for (_, id, title) in LOCAL_PAGES { if !id.starts_with('@') && !has(id) { put(HERE, id, title, &pairs(id)); } }
    out
}
/// What the search found, in the nav's order: each page's finds with the page (its id and title). The results the page area lists.
pub fn found(lines: &[NavLine]) -> Vec<(String, String, Found)> {
    lines.iter().flat_map(|l| match l { NavLine::Item { id, title, hits } => hits.iter().map(|h| (id.clone(), title.clone(), h.clone())).collect(), _ => vec![] }).collect()
}
/// The result Enter opens: the first one found.
pub fn first_found(lines: &[NavLine]) -> Option<(String, String, Found)> { found(lines).into_iter().next() }
/// Plain words the search also finds this device's rows by, keyed by label (the hub's rows have theirs in `also`).
/* i18n-ok: search words, never drawn */ const DEVICE_ALSO: &[(&str, &str)] = &[("Language", "locale japanese spanish english"), ("Text size", "font size zoom bigger"), ("Font", "typeface mono"), ("Sidebar width", "narrower wider"), ("Opens on", "start launch"), ("Desktop notifications", "alerts popup"), ("Streamer mode", "hide private recording obs stream"), ("Theme", "dark mode light mode")];
/// What the search looks through of this device's rows (`device_rows`), by the page that holds them (`device_section`).
pub fn device_finds(rows: &[Row]) -> HashMap<String, Vec<Find>> {
    let also = |l: &str| DEVICE_ALSO.iter().find(|(k, _)| *k == l).map_or("", |(_, a)| *a).to_string();
    DEVICE_PAGES.iter().map(|(_, p)| *p).chain(["@keys"]).map(|p| (p.to_string(), device_section(p, rows).iter().filter(|r| !r.label.is_empty()).map(|r| Find { label: r.label.clone(), note: r.note.clone(), also: also(&r.label) }).collect())).collect()
}
/// The page that holds each section of `device_rows`, by its heading: Appearance and Layout on Appearance, Notifications on Notifications,
/// Streaming on Privacy, the keyboard shortcuts on this app's Keyboard page (`@keys`).
/* i18n-ok: section headings matched, not drawn */ const DEVICE_PAGES: &[(&str, &str)] = &[("Appearance", "appearance"), ("Layout", "appearance"), ("Notifications", "notifications"), ("Streaming", "privacy"), ("Keyboard shortcuts", "@keys")];
/// This device's rows on `page` (a hub page id, or `@keys`), in their order and without their section's heading. On `@keys` Shortcuts to
/// defaults comes first, and the keys that cannot be changed (no button) are marked rarely needed (`adv`) and go last, so the page lists what
/// can be changed and folds the fixed ones.
pub fn device_section(page: &str, rows: &[Row]) -> Vec<Row> {
    let mut at = ""; let mut out: Vec<Row> = Vec::new();
    for r in rows {
        if r.buttons.is_empty() && r.note.is_empty() { if let Some((_, p)) = DEVICE_PAGES.iter().find(|(h, _)| *h == r.label) { at = p; continue } }
        if at == page { out.push(r.clone()); }
    }
    if page != "@keys" { return out }
    let (mut v, fixed): (Vec<Row>, Vec<Row>) = out.into_iter().partition(|r| !r.buttons.is_empty());
    // Shortcuts to defaults first: a person who broke a key looks for it at the top, not under thirty rows
    v.sort_by_key(|r| !r.buttons.iter().any(|b| b.action == "local:shortcuts-reset"));
    v.extend(fixed.into_iter().map(|mut r| { r.raw["adv"] = Value::Bool(true); r })); v
}
/// Whether a row is marked rarely needed (the hub's `adv`, or this device's fixed keys): drawn under the page's Advanced fold.
pub fn is_adv(r: &Row) -> bool { r.raw["adv"] == true }
/// A section heading (a label and nothing else, or a paragraph that opens a group: the hub's `intro`), not a setting: the Advanced fold counts the settings only.
pub fn is_heading(r: &Row) -> bool { r.raw["intro"] == true || r.kind.is_empty() && r.note.is_empty() && r.buttons.is_empty() && r.fields.is_empty() && r.qr.is_empty() && r.raw["slider"].is_null() }
/// Whether a page's Advanced fold is drawn open: it was opened, or a search is typed that one of its folded rows matches.
pub fn adv_open(rows: &[Row], q: &str, opened: bool) -> bool {
    opened || !hits(&rows.iter().filter(|r| is_adv(r) && !r.label.is_empty()).map(|r| Find { label: r.label.clone(), note: r.note.clone(), also: String::new() }).collect::<Vec<_>>(), q).is_empty()
}

pub struct FieldSlot { pub input: Entity<Composer>, pub vault: bool, pub hub: String, echo: Echo, pending: Option<String>, timer: bool }

impl FieldSlot { fn remember(&mut self, t: &str) { self.echo.remember(t); } }

/// The settings fields that hold a path or a short value: one line tall, beside their row's button (the rest, words to write, wrap).
/// An always-on machine's address and code too, and setup's boxes of that kind (`ob*`: the first project's folder, where KiCad and FreeCAD
/// are, the always-on address and code), so a box keeps its height while a path is typed and the button beside it stays where it was.
pub const ONE_LINE: &[&str] = &["kpath", "fpath", "scpath", "tzname", "whurl", "whsec", "bstart", "qfrom", "qto", "maddr", "mcode", "kname", "aonaddr", "aoncode",
    "obproject", "obkicad", "obfreecad", "obaonaddr", "obaoncode"];
/// One composer per field name. A form field's typing goes out as `bfield`, coalesced (the first change at once, the rest the newest after
/// 30 ms); the hub's value is taken only when it is a real change. A vault field holds its text here and sends nothing until Save.
pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let rows = parse(&this.data["settings"]);
    sync_nav(this, &rows, cx);
    let mut want: Vec<Field> = rows.into_iter().flat_map(|r| r.fields).collect();
    want.extend(super::bots::form_fields(&this.data)); want.extend(super::swarm::form_fields(&this.data)); want.extend(super::panels::form_fields(&this.data)); want.extend(super::tools::form_fields(&this.data)); want.extend(super::onboard::form_fields(&this.data)); want.extend(super::sidebox::form_fields(&this.data));
    this.screens.fields.retain(|n, _| want.iter().any(|f| f.name == *n));
    for f in want {
        if let Some(slot) = this.screens.fields.get_mut(&f.name) {
            if slot.vault || slot.hub == f.value { continue; }
            slot.hub = f.value.clone();
            let typed = slot.input.read(cx).content.to_string();
            if slot.echo.on_hub(&f.value, &typed) { let (input, v) = (slot.input.clone(), f.value.clone()); slot.remember(&v); input.update(cx, |i, cx| i.set(v, cx)); }
            continue;
        }
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        let (hint, secret, v) = (f.hint.clone(), f.secret, f.value.clone());
        // a row's rename box is one row tall: a long title wrapped to three, drawn over the rows above and below it; so is a box for a path or
        // a short value beside its row's button (`ONE_LINE`)
        let one_row = f.name == "title" || ONE_LINE.contains(&f.name.as_str());
        input.update(cx, |i, cx| { i.set_placeholder(&trs(&hint)); i.masked = secret; if one_row { i.max_lines = 1.; } i.set(v, cx); });
        let field = f.clone();
        if !f.vault {
            cx.observe(&input, move |this, input, cx| {
                let text = input.read(cx).content.to_string();
                let Some(slot) = this.screens.fields.get_mut(&field.name) else { return };
                if slot.echo.sent() == text { return; }
                slot.pending = Some(text);
                cx.notify(); // the cat beside a name box follows each letter (screens/catpick.rs)
                send_field(this, &field, cx);
            }).detach();
        }
        let sname = f.name.clone();
        cx.subscribe(&input, move |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { submit(this, &sname, cx); }).detach();
        this.screens.fields.insert(f.name.clone(), FieldSlot { input, vault: f.vault, hub: f.value.clone(), echo: Echo::new(&f.value), pending: None, timer: false });
    }
}

/// Keep the nav's pages from the hub's menu, open the first page when the dialog opens on the menu, and own the search box.
fn sync_nav(this: &mut Backplane, rows: &[Row], cx: &mut Context<Backplane>) {
    // (while this device's pages are open with no hub page, the page chosen stays: Notifications pressed before the hub answered, or no hub)
    if !this.data["settings"].is_object() { let ui = &mut this.screens.settings_ui; if !ui.open_local { ui.page.clear(); } ui.auto = false; ui.shown.clear(); return; }
    if this.screens.settings_ui.search.is_none() {
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        // (popup_open: Up and Down move the results' highlight instead of the caret)
        input.update(cx, |i, _| { i.set_placeholder(&trs("Search")); i.popup_open = true; });
        cx.observe(&input, |this, input, cx| { let q = input.read(cx).content.to_string(); if q != this.screens.settings_ui.q { searched(this, q); cx.notify(); } }).detach();
        // Enter opens the result highlighted (the first one found unless Up or Down moved it); Up and Down move the highlight
        cx.subscribe(&input, |this, _, ev: &ComposerEvent, cx| match ev {
            ComposerEvent::Submit => { let r = results(this); if let Some((page, _, f)) = r.get(this.screens.settings_ui.hit.min(r.len().saturating_sub(1))).cloned() { open_hit(this, &page, &f, cx); } }
            ComposerEvent::PopupMove(d) => {
                let n = results(this).len(); let ui = &mut this.screens.settings_ui;
                if n == 0 || !results_shown(ui) { return }
                ui.hit = (ui.hit as i32 + d).clamp(0, n as i32 - 1) as usize; ui.scroll.scroll_to_item(1 + ui.hit); cx.notify();
            }
            _ => {}
        }).detach();
        // a rig's shot of the search (`BACKPLANE_GPUI_SETTINGS_Q`): the box opens holding it
        if let Some(q) = std::env::var("BACKPLANE_GPUI_SETTINGS_Q").ok().filter(|q| !q.is_empty()) { input.update(cx, |i, cx| i.set(q, cx)); }
        this.screens.settings_ui.search = Some(input);
    }
    if let Shown::Menu(items) = shown(rows) {
        let (groups, finds, akas) = (menu_groups(&this.data["settings"]["rows"]), menu_finds(&this.data["settings"]["rows"]), menu_akas(&this.data["settings"]["rows"]));
        let ui = &mut this.screens.settings_ui;
        ui.nav = items; ui.groups = groups; ui.finds = finds; ui.akas = akas;
        // the hub's menu while the dialog opens: the page already shown (Appearance, or one pressed before the hub answered) is asked for, else the first
        if !ui.auto { ui.auto = true; let p = if ui.nav.iter().any(|(id, _)| *id == ui.page) { Some(ui.page.clone()) } else { ui.nav.first().map(|(p, _)| p.clone()) };
            if let Some(p) = p { ui.page = p.clone(); let _ = this.tx.try_send(Command::Action("settings-page".into(), p)); } }
    }
    if matches!(shown(rows), Shown::Page) {
        let ui = &mut this.screens.settings_ui; let t = rows.first().map(|r| r.label.clone()).unwrap_or_default(); let hp = hub_page(&ui.nav, &t);
        if !ui.auto {
            // the dialog opens on the page chosen here (Appearance from the gear), not on the one the hub kept from last time; a page the hub
            // opened itself (nothing chosen here) is taken. Not knowing the hub's pages yet, its menu is asked for first (and the Menu case above goes on)
            ui.auto = true;
            if ui.page.is_empty() { ui.page = hp.clone().unwrap_or_default(); }
            else if hp.as_deref() != Some(ui.page.as_str()) {
                let p = if ui.nav.iter().any(|(id, _)| *id == ui.page) { ui.page.clone() } else { ui.auto = false; String::new() };
                let _ = this.tx.try_send(Command::Action("settings-page".into(), p));
            }
        } else if ui.shown != t { if let Some(p) = hp { ui.page = p; } } // the hub moved to another page (an answer, a link, the hub's own move)
        // another page: it opens at its top
        let ui = &mut this.screens.settings_ui; if ui.shown != t { ui.shown = t; to_top(ui); }
    }
}

/// Send the newest pending text of a field if there is no window running; start the 30 ms window. `sent` moves only when the send worked.
fn send_field(this: &mut Backplane, f: &Field, cx: &mut Context<Backplane>) {
    let Some(slot) = this.screens.fields.get_mut(&f.name) else { return };
    if slot.timer { return; }
    let Some(text) = slot.pending.take() else { return };
    if text == slot.echo.sent() { return; }
    let Some(cmd) = change(f, &text) else { return };
    match this.tx.try_send(cmd) { Ok(_) => slot.remember(&text), Err(_) => slot.pending = Some(text) }
    slot.timer = true;
    let f = f.clone();
    cx.spawn(async move |weak, cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(30)).await;
        let _ = weak.update(cx, |this, cx| {
            if let Some(s) = this.screens.fields.get_mut(&f.name) { s.timer = false; }
            send_field(this, &f, cx);
        });
    }).detach();
}

/// A nav entry's element id: one per entry (`keys` = API keys, `@keys` = Keyboard).
fn nav_el_id(id: &str) -> String { format!("set-nav-{id}") }
/// Send every field's pending (coalesced) text now: called before any button action, so a fast press never overtakes the last keystrokes.
/// A hub page of the nav, chosen (a click, or the e2e `click` on `set-nav:<page>`): what is typed goes first, then the hub is asked for the page.
pub fn go_page(this: &mut Backplane, p: &str) { flush_fields(this); this.screens.focus_settings = true; let ui = &mut this.screens.settings_ui; ui.refused = None; ui.keys = false; ui.hubs = false; ui.accounts = false; ui.page = p.to_string(); ui.landed = true; ui.lit = None; end_recording(ui); to_top(ui); let _ = this.tx.try_send(Command::Action("settings-page".into(), p.to_string()));
    // Usage opens with each account's limit bars over the totals: the hub's accounts read again (and each signed-in account's usage, at most every 5 minutes)
    if p == "usage" { super::accounts::list(this); super::accounts::refresh_usage(this); } }
/// Where the gear opens Settings: the page it was on when it shut (`left`, this run of the app), else Appearance; as (hub page, Keyboard,
/// Hubs, Accounts). One of this app's pages keeps Appearance as the hub's page behind it.
pub fn reopen(left: &str) -> (String, bool, bool, bool) {
    match left { "" => ("appearance".into(), false, false, false), "@keys" => ("appearance".into(), true, false, false), "@hubs" => ("appearance".into(), false, true, false),
        "@accounts" => ("appearance".into(), false, false, true), p => (p.to_string(), false, false, false) }
}
/// A page of the nav, chosen: one of this app's three, or the hub's (`@device`, the page this device's rows had before they moved to the pages
/// of their topic, is Appearance).
pub fn open_nav(this: &mut Backplane, id: &str, cx: &mut Context<Backplane>) {
    let id = if id == "@device" { "appearance" } else { id };
    if !id.starts_with('@') { go_page(this, id); cx.notify(); return }
    flush_fields(this); this.screens.focus_settings = true;
    let ui = &mut this.screens.settings_ui; ui.refused = None; ui.keys = id == "@keys"; ui.hubs = id == "@hubs"; ui.accounts = id == "@accounts"; ui.landed = true; ui.lit = None; end_recording(ui); to_top(ui);
    if id == "@accounts" { super::accounts::open_page(this, cx); }
    cx.notify();
}
/// The rows scrolled back to their top: a page opens there, never at the offset the page before had.
fn to_top(ui: &SettingsUi) { ui.scroll.set_offset(point(px(0.), px(0.))); }
/// Whether the page area lists the search's results: something is typed and no result or page was opened since.
pub fn results_shown(ui: &SettingsUi) -> bool { !ui.q.trim().is_empty() && !ui.landed }
/// The search changed to `q`: the results show (from their top, the page's place kept) while it holds something, the page again once it is empty.
fn searched(this: &mut Backplane, q: String) {
    let ui = &mut this.screens.settings_ui; let before = results_shown(ui);
    ui.q = q; ui.landed = false; ui.hit = 0;
    let now = results_shown(ui);
    if !before && now { ui.page_at = Some(ui.scroll.offset()); to_top(ui); }
    if before && !now { if let Some(at) = ui.page_at.take() { ui.scroll.set_offset(at); } }
}
/// Esc while the search holds something empties it (the page shows again, the row a result lit no longer lit); a second Esc shuts Settings.
pub fn escape_search(this: &mut Backplane, cx: &mut Context<Backplane>) -> bool {
    if !is_open(this) || this.screens.settings_ui.q.is_empty() { return false }
    let ui = &mut this.screens.settings_ui; ui.lit = None; ui.pending = None;
    match this.screens.settings_ui.search.clone() { Some(input) => input.update(cx, |i, cx| i.set(String::new(), cx)), None => searched(this, String::new()) }
    cx.notify(); true
}
/// What the search finds now, in the nav's order (`found`): the page area's results, and what Enter, Up and Down move through.
pub fn results(this: &Backplane) -> Vec<(String, String, Found)> {
    let ui = &this.screens.settings_ui;
    found(&nav_lines(if this.data["settings"].is_object() { &ui.nav } else { &[] }, &ui.groups, &ui.finds, &ui.akas, &device_finds(&device_rows(&this.local)), &ui.q))
}
/// A result pressed (or Enter): its page opens (the one already open keeps its place), the search text stays; a row is then scrolled into
/// view and lit once the page draws it (`land`), its Advanced fold opened if it is under it.
pub fn open_hit(this: &mut Backplane, page: &str, f: &Found, cx: &mut Context<Backplane>) {
    if current(this) == page { let ui = &mut this.screens.settings_ui; ui.landed = true; if let Some(at) = ui.page_at.take() { ui.scroll.set_offset(at); } } else { open_nav(this, page, cx); }
    let ui = &mut this.screens.settings_ui; ui.page_at = None;
    ui.pending = f.row.then(|| (page.to_string(), f.label.clone(), f.nth));
    cx.notify();
}
/// The page shown now: one of this app's (`@keys`, `@hubs`, `@accounts`) or the hub's (`page_shown`).
pub fn current(this: &Backplane) -> String {
    let ui = &this.screens.settings_ui;
    if ui.accounts { return "@accounts".into() } if ui.hubs { return "@hubs".into() }
    let rows = parse(&this.data["settings"]);
    let on_page = this.data["settings"].is_object() && !ui.keys && matches!(shown(&rows), Shown::Page) && !hub_behind(ui, &rows[0].label);
    page_shown(ui, if on_page { rows[0].label.as_str() } else { "" }, ui.keys)
}
/// Which row of `list` is the `nth` (from 0) with this label.
pub fn nth_row(list: &[Row], label: &str, nth: usize) -> Option<usize> { list.iter().enumerate().filter(|(_, r)| r.label == label).nth(nth).map(|(i, _)| i) }
/// The child of the rows' scroll that draws row `at` of a page's `list` (`page_rows`): the title comes first, then a page's own words (`sub`),
/// then what is drawn over the rows (`top`: Usage's limit bars, one child), the rows above the fold, the fold line and the folded rows.
pub fn row_child(list: &[Row], at: usize, sub: bool, top: usize) -> usize {
    let k = if is_adv(&list[at]) { list.iter().filter(|r| !is_adv(r)).count() + 1 + list[..at].iter().filter(|r| is_adv(r)).count() } else { list[..at].iter().filter(|r| !is_adv(r)).count() };
    1 + sub as usize + top + k
}
/// A pressed result's row, once its page draws it: the fold it is under opened, the row lit for `LIT_MS` (one timer; nothing repaints
/// meanwhile) and, after this frame has laid the page out, scrolled to (only when it is not wholly in view: then with the row above it at the
/// top). Until then it waits: the hub's rows arrive after the page is pressed. A row the page does not have once its rows are there is let go.
pub fn land(this: &mut Backplane, window: &mut Window, cx: &mut Context<Backplane>) {
    let Some((page, label, nth)) = this.screens.settings_ui.pending.clone() else { return };
    if !is_open(this) || results_shown(&this.screens.settings_ui) { this.screens.settings_ui.pending = None; return }
    let pr = page_rows(&this.data["settings"], &this.screens.settings_ui, &this.local, &device_rows(&this.local));
    if pr.page != page { return }
    let Some(at) = nth_row(&pr.list, &label, nth) else { if pr.on_page || !this.data["settings"].is_object() || page.starts_with('@') { this.screens.settings_ui.pending = None; } return };
    let ix = row_child(&pr.list, at, page == "@keys", (page == "usage") as usize);
    let ui = &mut this.screens.settings_ui; ui.pending = None;
    if is_adv(&pr.list[at]) { ui.adv.insert(page.clone()); }
    ui.lit_gen += 1; let mine = ui.lit_gen; ui.lit = Some((page, label, nth));
    cx.on_next_frame(window, move |this, _, cx| {
        let s = &this.screens.settings_ui.scroll;
        let Some(b) = s.bounds_for_item(ix) else { s.scroll_to_item(ix); cx.notify(); return };
        let (vp, y) = (s.bounds(), s.offset().y);
        if b.top() + y >= vp.top() && b.bottom() + y <= vp.bottom() { return }
        s.scroll_to_top_of_item(ix - 1); cx.notify();
    });
    cx.spawn(async move |weak, cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(LIT_MS)).await;
        let _ = weak.update(cx, |this, cx| { let ui = &mut this.screens.settings_ui; if ui.lit_gen == mine { ui.lit = None; cx.notify(); } });
    }).detach();
}
/// A page's Advanced fold pressed: open or shut, kept while the app runs.
pub fn toggle_adv(this: &mut Backplane, page: &str, cx: &mut Context<Backplane>) { let a = &mut this.screens.settings_ui.adv; if !a.remove(page) { a.insert(page.to_string()); } cx.notify(); }
pub fn flush_fields(this: &mut Backplane) {
    let names: Vec<String> = this.screens.fields.iter().filter(|(_, s)| !s.vault && s.pending.is_some()).map(|(n, _)| n.clone()).collect();
    for n in names {
        let Some(slot) = this.screens.fields.get_mut(&n) else { continue };
        let Some(text) = slot.pending.take() else { continue };
        if text == slot.echo.sent() { continue; }
        match this.tx.try_send(field_command(&n, &text)) { Ok(_) => slot.remember(&text), Err(_) => slot.pending = Some(text) }
    }
}

/// Add (or Enter) on the to-do list's new-step box: sent once, and the box is empty again.
pub fn send_todo(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.fields.get("todonew").map(|s| s.input.clone()) else { return };
    if input.read(cx).content.trim().is_empty() { return; }
    flush_fields(this);
    let _ = this.tx.try_send(Command::Action("todo-add".into(), String::new()));
    input.update(cx, |i, cx| i.set(String::new(), cx));
    cx.notify();
}

/// Enter in a typed field: a room's or a remote bot's post box sends, as on the window.
/// What Enter in the masked box saves: the hub's packed Save of the open `thread.secretBox` (None when no box is open).
pub fn secret_enter(data: &Value) -> Option<String> { Some(string(&data["thread"]["secretBox"], "save")).filter(|s| !s.is_empty()) }

pub fn submit(this: &mut Backplane, name: &str, cx: &mut Context<Backplane>) {
    if (name.starts_with("ob") || (name == "sval" && this.data["tour"].is_object())) && super::onboard::submit(this, name, cx) { return }
    if super::bots::form_submit(this, name, cx) { return } // Enter in the New bot / New room form: its Create / Save
    match name { "rpost" => super::bots::send_post(this, "rpost", "room-post", cx), "rtext" => super::bots::send_post(this, "rtext", "remote-send", cx), "title" => { // Enter in the rename box saves, as its Rename button does (nothing for a blank title)
            let id = string(&this.data["renaming"], "id");
            let text = this.screens.fields.get("title").map(|s| s.input.read(cx).content.to_string()).unwrap_or_default();
            if let Some(c) = super::tools::rename_save(&id, &text) { flush_fields(this); let _ = this.tx.try_send(c); cx.notify(); }
        }
        // Enter in a settings field is its row's Save
        "whurl" | "whsec" | "tzname" | "qfrom" | "qto" | "bstart" => { flush_fields(this); let (a, v) = match name { "whurl" => ("hook-url", ""), "whsec" => ("hook-secret", ""), "tzname" => ("tz-set", ""), "bstart" => ("browser-start", "save"), _ => ("notify-quiet", "save") }; let _ = this.tx.try_send(Command::Action(a.into(), v.into())); cx.notify(); }
        // Enter in the masked box (the name or the value) is its Save
        /* i18n-ok: a button the row draws through trs */ "sname" | "sval" => if let Some(packed) = secret_enter(&this.data) { save_vault(this, &Btn { label: "Save".into(), action: "secret-save".into(), value: packed, on: true }, cx); },
        "todonew" => send_todo(this, cx),
        n if n.starts_with("todo.") => { flush_fields(this); let _ = this.tx.try_send(Command::Action("todo-text".into(), n[5..].to_string())); cx.notify(); }
        n => if let Some(a) = n.strip_prefix("sp.") { super::bots::send_space(this, a, cx) } else if let Some(b) = enter_press(&parse(&this.data["settings"]), n) { on_press(this, &b, cx) } }
}
/// What Enter in a Settings box presses: its row's button when the row has just one (KiCad's path and Set, a machine's address and Connect).
pub fn enter_press(rows: &[Row], field: &str) -> Option<Btn> {
    let r = rows.iter().find(|r| r.fields.iter().any(|f| f.name == field))?;
    if r.buttons.len() == 1 { r.buttons.first().cloned() } else { None }
}

/// A vault Save: the field is the button's (`@apikey` is `kval`, else `sval`); the typed value leaves in one request straight to the hub, and
/// the field is emptied (history too) only when the hub accepted it. Nothing sent says why, as the phones do (`key-err`, `secret-err`).
pub fn save_vault(this: &mut Backplane, b: &Btn, cx: &mut Context<Backplane>) {
    let key = b.value.starts_with("@apikey\u{1f}");
    // a sign-in panel's box: sent as it is, emptied once the hub took it; a refusal is the panel's own row (the hub keeps why)
    if let Some(field) = signin_field(&b.value) {
        let Some(input) = this.screens.fields.get(field).map(|s| s.input.clone()) else { return };
        if let Ok((m, p)) = save_request(&b.value, "", &input.read(cx).content.to_string()) { let _ = this.tx.try_send(Command::Rpc(m, Secret(p), if field == "sicode" { "signin-code" } else { "signin" })); }
        cx.notify(); return;
    }
    // an MCP server's key has a box of its own (`msec.<vault name>`, mcpx/page.rs); every other secret is the masked box's `sval`
    let mcp_box = b.value.split('\u{1f}').next().filter(|n| n.starts_with("mcp-")).map(|n| format!("msec.{n}"));
    let (field, err) = if key { ("kval", "key-err") } else { (mcp_box.as_deref().unwrap_or("sval"), "secret-err") };
    let say = |this: &mut Backplane, why: &str| { let _ = this.tx.try_send(Command::Action(err.into(), why.into())); };
    let Some(input) = this.screens.fields.get(field).map(|s| s.input.clone()) else { return };
    let typed_name = this.screens.fields.get(if key { "kname" } else { "sname" }).map(|s| s.input.read(cx).content.to_string()).unwrap_or_default();
    let value = input.read(cx).content.to_string();
    match save_request(&b.value, &typed_name, &value) {
        Err(why) => say(this, why),
        Ok((m, p)) => {
            let tag = if key { "key" } else { "secret" };
            // the field is kept until the hub's answer (rpc_done): accepted empties it and shuts the form, refused says why
            /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if this.tx.try_send(Command::Rpc(m, Secret(p), tag)).is_err() { say(this, "The hub is not reachable. Try again."); }
        }
    }
    cx.notify();
}

/// Why the hub refused a request: its `error` is a plain string (`{"ok":false,"error":"..."}`); an object with `message` or `code` is read too.
pub fn reply_error(x: &Value) -> String {
    let e = &x["error"];
    e.as_str().or_else(|| e["message"].as_str()).or_else(|| e["code"].as_str()).unwrap_or("").to_string()
}

/// The hub answered a direct request: a secret's outcome is shown by the client (`secret-sent` shuts the box, `secret-err` says why).
pub fn rpc_done(this: &mut Backplane, tag: &str, ok: bool, why: &str, cx: &mut Context<Backplane>) {
    if tag == "browser" { if !ok { if let Some((_, page)) = &this.screens.browser { let page = page.clone(); page.update(cx, |p, cx| p.open_failed(why, cx)); } } return; }
    if tag.starts_with("hook") { return super::bots::hook_done(this, tag, ok, why, cx); }
    if tag == "signin" || tag == "signin-code" {
        if ok { if let Some(input) = this.screens.fields.get(if tag == "signin" { "sikeys" } else { "sicode" }).map(|s| s.input.clone()) { input.update(cx, |i, cx| i.set(String::new(), cx)); } }
        return;
    }
    let (name, sent, err) = match tag { "key" => ("kval", "key-sent", "key-err"), "secret" => ("sval", "secret-sent", "secret-err"), _ => return };
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ let (a, v) = if ok { (sent, String::new()) } else { (err, if why.is_empty() { format!("The hub refused the {}", if tag == "key" { "key" } else { "secret" }) } else { why.to_string() }) };
    // the typed value is kept until the hub accepted it, so a refusal can be corrected and sent again
    if ok { if let Some(input) = this.screens.fields.get(name).map(|s| s.input.clone()) { input.update(cx, |i, cx| i.set(String::new(), cx)); } }
    let _ = this.tx.try_send(Command::Action(a.into(), v));
}

/// A QR code's module size, and its drawn width: `n` modules and the four-module quiet zone on both sides (the row is laid out for this width;
/// counting the quiet zone once pushed the right column 16 px past the dialog's edge, the code and the chips under it cut off).
const QR_CELL: f32 = 4.0;
pub fn qr_w(n: usize) -> f32 { (n as f32 + 8.) * QR_CELL }
/// A QR code as scanners read it in either theme: dark modules on white with the four-module quiet zone the standard asks for (a light
/// code on the dark theme's background, as it was drawn, is one many phone scanners do not read at all).
fn qr(_light: bool, lines: &[String]) -> AnyElement {
    let n = lines.len(); let cell = QR_CELL;
    let (dark, bg) = (gpui::rgb(0x000000), gpui::rgb(0xffffff));
    let rows: Vec<Vec<bool>> = lines.iter().map(|l| l.chars().map(|c| c == '1').collect()).collect();
    div().p(px(4.0 * cell)).bg(bg).child(canvas(|_, _, _| (), move |bounds, _, window, _| {
        for (y, row) in rows.iter().enumerate() { for (x, on) in row.iter().enumerate() {
            if *on { window.paint_quad(fill(Bounds::new(point(bounds.left() + px(x as f32 * cell), bounds.top() + px(y as f32 * cell)), size(px(cell), px(cell))), dark)); }
        } }
    }).size(px(n as f32 * cell))).into_any_element()
}

/// The overlay's structure as one JSON text: each row's label, whether it has a note, its buttons (label, action, value, on) and field names. No field values.
pub fn structure(settings: &Value) -> String {
    let rows: Vec<Value> = parse(settings).iter().map(|r| serde_json::json!({"label": r.label, "note": !r.note.is_empty(), "buttons": r.buttons.iter().map(|b| serde_json::json!([b.label, b.action, b.value, b.on])).collect::<Vec<_>>(), "fields": r.fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>(), "qr": !r.qr.is_empty()})).collect();
    serde_json::json!({"keys": settings.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(), "rows": rows}).to_string()
}

/// The most chips on one line of a row's controls.
const CHIPS_PER_LINE: usize = 6;
/// A hub row as a dialog row: label and muted note on the left, the controls on the right, a hairline under it. A row that is only a label is a
/// section heading, and so is a paragraph that opens a group (the hub's `intro`), its words under it; a label and a note with nothing to press or
/// type is a paragraph. The last text box and the row's buttons share a line (`field_beside`). A row the hub (or `mark_device`) says is
/// `changed` has a mark after its label, and Reset when it says how (`reset`).
/// `avail`: the width the row is laid out in (`Fit::rows`); `row_fit` decides from it whether the controls go beside the words or under them.
pub(crate) fn row_el(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, r: &Row, i: usize, avail: f32) -> AnyElement {
    // a row drawn inside the card above it (a server's details): the row as usual, framed as the card's continuation
    if r.raw["in"] == true { let mut inner = r.clone(); inner.raw["in"] = Value::Bool(false); let e = row_el(cx, light, fields, &inner, i, avail - 26.); return super::cards::inside(light, e, r.kind == "actions") }
    if !r.kind.is_empty() { if let Some(e) = super::cards::el(cx, light, fields, r, i, avail) { return e } }
    let c = |n: &str| theme::color(light, n);
    let has_field = r.fields.iter().any(|f| fields.contains_key(&f.name));
    let controls = !r.buttons.is_empty() || has_field || !r.qr.is_empty();
    if !controls && r.note.is_empty() {
        return words(cx, light, div().pt_4().pb_1().text_size(crate::scale::tpx(14.)).font_weight(FontWeight::BOLD), trs(&r.label)).into_any_element();
    }
    // a paragraph that opens a group: the heading, its words under it, no hairline (it is not a setting)
    if r.raw["intro"] == true && !controls {
        return div().pt_4().pb_1().flex().flex_col().gap_1().child(words(cx, light, div().text_size(crate::scale::tpx(14.)).font_weight(FontWeight::BOLD), trs(&r.label)))
            .child(words(cx, light, div().text_size(crate::scale::tpx(12.)).text_color(c("dim")), trs(&r.note))).into_any_element();
    }
    // a text field takes the room the controls have beside the words (a folder path or an address is longer than 260 px, and wrapped in the field)
    let field_w = (avail - LABEL_MIN - GAP).clamp(260., CONTROLS_MAX).min(avail);
    let slider = slider_of(&r.raw);
    let btn_ws: Vec<f32> = r.buttons.iter().map(btn_w).collect();
    // short boxes (the hub's `w`: quiet hours' From and Until, a pairing code) at their own width on one line with the buttons, at the right
    // edge the row's chips share; a longer box before them keeps its own line above (`short_tail`)
    let short = if slider.is_none() { short_tail(&r.raw, r.fields.len()) } else { vec![] };
    let tail_at = r.fields.len() - short.len();
    let short = if r.fields[tail_at..].iter().all(|f| fields.contains_key(&f.name)) { short } else { vec![] };
    let line_w = if short.is_empty() { 0. } else { short_line_w(&short, &btn_ws) };
    // the last box and the buttons on one line, the box less the buttons' width (KiCad's path and Set); too narrow, the buttons go under it
    let beside = if slider.is_none() && short.is_empty() && r.fields.last().map_or(false, |f| fields.contains_key(&f.name)) { field_beside(field_w, &btn_ws) } else { None };
    // a slider and its buttons are one line: the slider then the chips
    let widths: Vec<f32> = if slider.is_some() || beside.is_some() || !short.is_empty() { vec![] } else { btn_ws };
    // the room the boxes take: a long box's, or the short line's alone
    let field_room = if short.is_empty() { field_w } else if tail_at == 0 { line_w } else { field_w.max(line_w) };
    let slider_w = slider.as_ref().map_or(0., |_| SLIDER_W + r.buttons.iter().map(|b| 8. + chip_w(&trs(&b.label))).sum::<f32>());
    let RowFit { stack, lines } = row_fit(avail, &widths, if has_field { field_room } else { slider_w }, if r.qr.is_empty() { 0. } else { qr_w(r.qr.len()) });
    // the words keep their least width beside the controls (never squeezed to a letter a line), or the whole row's under a stack
    let mut left = div().flex().flex_col().gap_1().min_w_0();
    // beside the controls, the words get the row's width less the controls' (their widest line, field or QR code), as a fixed width: a flex
    // share alone let a long note be measured wider than its column in setup's page, and run on under the button
    let right_w = { let mut at = 0; let mut w = 0f32; for &n in &lines { w = w.max(widths[at..at + n].iter().sum::<f32>() + 4. * (n.max(1) - 1) as f32); at += n; }
        w.max(if has_field { field_room } else { slider_w }).max(if r.qr.is_empty() { 0. } else { qr_w(r.qr.len()) }) };
    left = if stack { left.w_full() } else { left.flex_none().w(px((avail - GAP - right_w).max(LABEL_MIN))) };
    if !r.label.is_empty() {
        let label = words(cx, light, div().text_size(crate::scale::tpx(13.)), trs(&r.label));
        // changed from what a fresh install has: a mark after the label on its line, and Reset when the row says what puts it back (the label stays where it is)
        left = if r.raw["changed"] != true { left.child(label) } else {
            let mut line = div().flex().items_center().gap_2().text_size(crate::scale::tpx(13.)).child(label.min_w_0()).child(div().flex_none().size(px(6.)).bg(c("accent")));
            if let Some(rs) = r.raw["reset"].as_object() {
                /* i18n-ok: drawn through trs below */ let b = Btn { label: "Reset".into(), action: rs.get("action").and_then(Value::as_str).unwrap_or("").into(), value: rs.get("value").and_then(Value::as_str).unwrap_or("").into(), on: false };
                let d = div().id(SharedString::from(format!("set-reset-{i}"))).flex_none().text_size(crate::scale::tpx(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.underline().text_color(c("text")));
                let d = if crate::art::rects_on() { let n = format!("btn:reset:{}", b.value); d.relative().child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&n, r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { d };
                line = line.child(d.child(trs(&b.label)).on_click(cx.listener(move |this, _, _, cx| on_press(this, &b, cx))));
            }
            left.child(line)
        };
    }
    if !r.note.is_empty() {
        let note = words(cx, light, div().text_size(crate::scale::tpx(12.)).text_color(c(match r.tone.as_str() { "warn" => "warn", "bad" => "bad", _ => "dim" })), tr_lines(&r.note));
        // a value to copy (the hub's `copy`: a path, an id, a link): its copy button shows while the pointer is on the row
        left = match r.raw["copy"].as_str().filter(|v| !v.is_empty()) { Some(v) => left.child(super::copyable::value(light, format!("set-note-{i}"), note, v.to_string(), false, cx)), None => left.child(note) };
    }
    // a download's share done (setup's and Settings' KiCad install): a thin bar under the words
    if let Some(p) = r.raw["progress"].as_f64() {
        let p = p.clamp(0., 1.) as f32;
        left = left.child(div().id(SharedString::from(format!("row-progress-{i}"))).mt_1().h(px(4.)).w_full().max_w(px(360.)).rounded(px(2.)).bg(c("raise"))
            .child(div().h_full().rounded(px(2.)).bg(c("accent")).w(relative(p))));
    }
    // a quiet way out under the words (not a button: the row's own action, or none, stays the thing to press)
    if let Some(l) = r.raw["sublink"].as_object() {
        let b = Btn { label: l.get("label").and_then(|v| v.as_str()).unwrap_or("").into(), action: l.get("action").and_then(|v| v.as_str()).unwrap_or("").into(), value: l.get("value").and_then(|v| v.as_str()).unwrap_or("").into(), on: false };
        let id = SharedString::from(format!("btn:{}:{}", b.action, b.value)); let label = trs(&b.label); let b2 = b.clone();
        let d = div().id(id).mt_1().text_size(crate::scale::tpx(12.)).text_color(c("dim")).underline().cursor_pointer().hover(|x| x.text_color(c("text")));
        let d = if crate::art::rects_on() { let n = format!("btn:{}:{}", b.action, b.value); d.relative().child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&n, r, w.scale_factor())).absolute().size_full()) } else { d };
        left = left.child(div().flex().child(d.child(label).on_click(cx.listener(move |this, _, _, cx| on_press(this, &b2, cx)))));
    }
    let mut right = if stack { div().flex().flex_col().items_start().gap_2().max_w_full() } else { div().flex().flex_col().items_end().gap_2().flex_shrink_0().max_w(px(CONTROLS_MAX)) };
    let last = r.fields.len().saturating_sub(1);
    for (k, f) in r.fields.iter().enumerate() {
        if !short.is_empty() && k >= tail_at { break }
        let Some(slot) = fields.get(&f.name) else { continue };
        // the line is as wide as a box alone, so the box starts where the boxes above it (and on the rows around) start; the buttons take
        // what they are drawn at and the box the rest (`beside` is the least it gets, from the buttons' estimated widths)
        match beside.filter(|_| k == last) {
            Some(w) => right = right.child(div().w(px(field_w)).max_w_full().flex().items_center().gap_1().child(div().flex_1().min_w(px(w.min(FIELD_MIN))).px_2().py_1().bg(c("raise")).child(slot.input.clone()))
                .children(r.buttons.iter().enumerate().map(|(j, b)| chip_el(cx, light, i, j, b)).collect::<Vec<_>>())),
            None => right = right.child(div().w(px(field_w)).max_w_full().px_2().py_1().bg(c("raise")).child(slot.input.clone())),
        }
    }
    if !short.is_empty() {
        let boxes = r.fields[tail_at..].iter().zip(&short).filter_map(|(f, &w)| fields.get(&f.name).map(|slot| div().flex_none().w(px(w)).px_2().py_1().bg(c("raise")).child(slot.input.clone()).into_any_element())).collect::<Vec<_>>();
        right = right.child(div().flex().items_center().gap_1().children(boxes).children(r.buttons.iter().enumerate().map(|(j, b)| chip_el(cx, light, i, j, b)).collect::<Vec<_>>()));
    }
    // the note under the code (code, address, until when) wraps in the room planned for the controls: one line of it widened the column past the dialog
    if !r.qr.is_empty() && crate::streamer::is_on() { right = right.child(div().w(px(qr_w(r.qr.len()))).h(px(qr_w(r.qr.len()))).flex().items_center().justify_center().p_3().border_1().border_color(c("line")).text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(trs("The code is hidden in streamer mode."))); }
    else if !r.qr.is_empty() { right = right.child(qr(light, &r.qr)); if !r.qrnote.is_empty() { right = right.child(words(cx, light, div().max_w(px(right_w)).text_size(crate::scale::tpx(11.)).text_color(c("dim")), trs(&r.qrnote))); } }
    if let Some(sl) = &slider {
        right = right.child(div().flex().items_center().gap_2().child(slider_el(cx, light, sl)).children(r.buttons.iter().enumerate().map(|(j, b)| {
            let b2 = b.clone();
            let d = div().id(SharedString::from(format!("set-{i}-{j}"))).px_3().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer();
            let d = if b.on { d.bg(c("accent")).text_color(c("on-accent")) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) };
            d.child(trs(&b.label)).on_click(cx.listener(move |this, _, _, cx| on_press(this, &b2, cx))).into_any_element()
        })));
    } else if !r.buttons.is_empty() {
        // (not a wrapping flex row: it does not grow its parent's height in gpui, and the next row would be drawn over it)
        // chips in lines packed to the room they have (row_fit), at most CHIPS_PER_LINE each (none when they are on the box's line)
        let mut start = 0;
        for n in lines {
            let (first, chunk) = (start, &r.buttons[start..start + n]); start += n;
            right = right.child(div().flex().when(!stack, |d| d.justify_end()).gap_1().children(chunk.iter().enumerate().map(|(k, b)| chip_el(cx, light, i, first + k, b)).collect::<Vec<_>>()));
        }
    }
    let line = if stack { div().flex().flex_col().gap_2() } else { div().flex().items_center().justify_between().gap(px(GAP)) };
    line.py_3().border_b_1().border_color(c("line")).child(left).child(right).into_any_element()
}
/// A note through the catalog line by line when its lines are messages of their own (the hub puts a sentence on a line under a status: CAD's
/// "Empty looks for it."; whole, a status template would take the sentence for part of its path), else whole.
pub fn tr_lines(note: &str) -> String {
    if !note.contains('\n') { return trs(note) }
    let lines = note.split('\n').map(trs).collect::<Vec<_>>().join("\n");
    if lines != note { lines } else { trs(note) }
}
/// A row's button `j` of row `i`: lit or not, its press the row's action (a copy press is the copy button).
fn chip_el(cx: &mut Context<Backplane>, light: bool, i: usize, j: usize, b: &Btn) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let b2 = b.clone();
    let d = div().id(SharedString::from(format!("set-{i}-{j}"))).px_3().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer();
    let d = if b.on { d.bg(c("accent")).text_color(c("on-accent")) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) };
    // its rectangle for the e2e drivers (BACKPLANE_GPUI_RECTS=1 only), named by what it does: `btn:<action>:<value>`
    let d = if crate::art::rects_on() { let n = format!("btn:{}:{}", b.action, b.value); d.relative().child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&n, r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { d };
    // a copy press is the copy button (screens/copyable.rs): its check, its keyboard, the exact text
    if b.action == "copy" { return super::copyable::chip(d, light, trs(&b.label), format!("set-copy-{i}-{j}"), b.value.clone(), cx).into_any_element() }
    d.child(trs(&b.label)).on_click(cx.listener(move |this, _, _, cx| on_press(this, &b2, cx))).into_any_element()
}

/// A row's button pressed: the client's own actions here, a vault Save straight to the hub, anything else the view action it names.
pub fn on_press(this: &mut Backplane, b2: &Btn, cx: &mut Context<Backplane>) {
    this.screens.settings_ui.refused = None; // what the hub refused answered the press before: this one may be answered anew
    // a shortcut's button waits for the keys typed for it (`recorded`); any other press ends that wait
    if b2.action == "local:shortcut" { let ui = &mut this.screens.settings_ui; ui.rec_note = None; ui.rec = Some(b2.value.clone()); cx.notify(); return }
    end_recording(&mut this.screens.settings_ui);
    if super::onboard::press(this, b2, cx) { return; } if let Some(a) = super::hubpick::parse(&b2.action) { super::hubpick::act(this, a, cx); return; } if let Some(name) = b2.action.strip_prefix("local:") { this.local_press(name, &b2.value, cx); return; } if b2.action == "update-check" { this.app_check(cx); } if b2.action == "open-url" { if openable(&b2.value) { cx.open_url(&b2.value); } return; } if b2.action == "open-licenses" { if let Some(d) = crate::hub::licenses_dir() { cx.open_with_system(&d); } return; } flush_fields(this); if is_vault_save(b2) { save_vault(this, b2, cx); } else { let _ = this.tx.try_send(press(b2)); } cx.notify();
}

/// The dialog while the hub has settings open; None when it has not: a title bar with a close ×, the pages on the left (search above, an icon and a
/// label each, the open one on a rounded fill, a thin divider between groups) and the page's rows on the right with their own scroll.
/// Settings shut (the ×, Esc, a click outside): this device's page and the accounts page at once, and the hub's pages when it has them open.
pub fn close(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let hub = this.data["settings"].is_object(); let left = current(this);
    let ui = &mut this.screens.settings_ui; ui.left = left; ui.refused = None; ui.open_local = false; ui.keys = false; ui.accounts = false; ui.hubs = false; ui.pending = None; ui.lit = None; end_recording(ui);
    this.screens.acct.menu = false;
    if hub { let _ = this.tx.try_send(Command::Action("flag".into(), "settings".into())); }
    cx.notify();
}
/// Whether any settings page is up (the hub's, or this device's and the accounts page, which the hub need not serve).
pub fn is_open(this: &Backplane) -> bool { this.data["settings"].is_object() || this.screens.settings_ui.open_local }

/// What a page of the dialog draws in its rows (but Hubs' and Accounts'), in order: the page shown (`page_shown`), whether the hub's rows for it
/// are there (`on_page`) and its title then, the rows (this device's under "This device", the hub's, an About page's app row among them) and how
/// many of the hub's are left out (over `MAX_ROWS`). `view` draws it; `land` finds a result's row in it.
pub struct PageRows { pub page: String, pub on_page: bool, pub title_now: String, pub list: Vec<Row>, pub more: usize }
pub fn page_rows(settings: &Value, ui: &SettingsUi, local: &crate::local::Local, dev_rows: &[Row]) -> PageRows {
    let hub_on = settings.is_object();
    let on_acct = ui.accounts;
    let on_hubs = ui.hubs && !on_acct;
    let on_keys = ui.keys && !on_acct && !on_hubs;
    let mut rows = parse(settings);
    let on_page = hub_on && !on_keys && !on_acct && !on_hubs && matches!(shown(&rows), Shown::Page) && !hub_behind(ui, &rows[0].label);
    let title_now = if on_page { rows[0].label.clone() } else { String::new() };
    let page = page_shown(ui, &title_now, on_keys);
    // About and updates (an older hub's Network and updates, and About): this computer's app beside the hub's version; with the hub's update rows
    // on the page (not an older hub's About), the app's own update controls too
    if on_page && (page == "network" || page == "about") {
        let form = if page == "network" || rows.iter().any(|r| r.label == "Update channel") { "network" } else { "about" };
        if let Some(r) = crate::selfupdate::row(local, form) {
            let at = crate::selfupdate::row_at(&rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>());
            rows.insert(at, r);
        }
    }
    let dev = if on_acct || on_hubs { vec![] } else { device_section(&page, dev_rows) };
    let more = if on_page { rows.len().saturating_sub(1 + MAX_ROWS) } else { 0 };
    // a hub page that holds this device's rows too: "This device" over them (drawn while the hub's rows are on their way too, so nothing moves
    // when they come), "Every device" over the hub's, unless the hub's part starts with a heading of its own (Notifications' "Notify me for").
    // A page with one kind has no such heading
    let heads = hub_on && !on_keys && !dev.is_empty() && (on_page || ui.nav.iter().any(|(p, _)| *p == page));
    let mut list: Vec<Row> = Vec::new();
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if heads { list.push(Row { label: "This device".into(), local: true, ..Default::default() }); }
    list.extend(dev);
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ if heads && on_page && rows.len() > 1 && !is_heading(&rows[1]) { list.push(Row { label: "Every device".into(), ..Default::default() }); }
    if on_page { list.extend(rows.iter().skip(1).take(MAX_ROWS).cloned()); }
    PageRows { page, on_page, title_now, list, more }
}

pub fn view(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, settings: &Value, focus: &FocusHandle, ui: &SettingsUi, local: &crate::local::Local, acct: &super::accounts::Accounts, fit: Fit, hub_list: &[crate::hubs::Hub], on_home: Option<&str>) -> Option<AnyElement> {
    let hub_on = settings.is_object();
    if !hub_on && !ui.open_local { return None; }
    let on_acct = ui.accounts;
    let on_hubs = ui.hubs && !on_acct;
    let on_keys = ui.keys && !on_acct && !on_hubs;
    let c = |n: &str| theme::color(light, n);
    let mut dev_rows = device_rows(local);
    mark_device(&mut dev_rows, &device_rows(&crate::local::Local::default()));
    show_recording(&mut dev_rows, ui.rec.as_deref(), ui.rec_note.as_ref());
    let PageRows { page, on_page, title_now, list, more } = page_rows(settings, ui, local, &dev_rows);
    // the row a result led to, lit for a moment
    let lit = ui.lit.as_ref().filter(|(p, _, _)| *p == page && !on_acct && !on_hubs).and_then(|(_, l, n)| nth_row(&list, l, *n));
    let lit_el = |i: usize, e: AnyElement| if lit == Some(i) { div().mx(px(-8.)).px(px(8.)).bg(c("raise")).child(e).into_any_element() } else { e };
    // the rarely needed rows (`adv`) after a fold line, in their order; ids by index over both, so no two rows share one
    let (main, folded): (Vec<usize>, Vec<usize>) = (0..list.len()).partition(|&i| !is_adv(&list[i]));
    let hubs_rows = super::hubpick::rows_on(hub_list, on_home);
    let mut els: Vec<AnyElement> = if on_hubs { hubs_rows.iter().enumerate().map(|(i, r)| row_el(cx, light, fields, r, i, fit.rows)).collect() } else if on_acct { super::accounts::page(cx, light, acct, fit.rows, ui.adv.contains("accounts")) } else { main.iter().enumerate().map(|(k, &i)| { let e = row_el(cx, light, fields, &list[i], k, fit.rows); lit_el(i, e) }).collect() };
    if !on_hubs && !on_acct && !folded.is_empty() {
        let open = adv_open(&list, &ui.q, ui.adv.contains(&page));
        els.push(adv_el(cx, light, &page, folded.iter().filter(|&&i| !is_heading(&list[i])).count(), open));
        if open { els.extend(folded.iter().enumerate().map(|(k, &i)| { let e = row_el(cx, light, fields, &list[i], main.len() + k, fit.rows); lit_el(i, e) })); }
    }
    // Usage: each account's limit windows first, then the hub's lifetime totals
    // (one child, so a row's child index is known before it is drawn: `row_child`)
    if !on_keys && !on_acct && !on_hubs && page == "usage" { els.insert(0, div().flex().flex_col().children(super::accounts::usage_summary(cx, light, acct)).into_any_element()); }
    if more > 0 { els.push(words(cx, light, div().py_2().text_color(c("dim")), tf("gpui.settings.more", &[("n", more.into())])).into_any_element()); }
    // the left nav
    let mut nav = div().id("settings-nav").flex().flex_col().gap(px(2.)).p_3().w(px(fit.nav)).flex_shrink_0().min_h_0().overflow_y_scroll().overflow_x_hidden().border_r_1().border_color(c("line"));
    if let Some(input) = &ui.search {
        let bx = div().flex().items_center().gap_2().px_2().py_1().mb_2().bg(c("raise"));
        // its rectangle for the e2e drivers (BACKPLANE_GPUI_RECTS=1 only): `set-search`, where a pointer press puts the keys. In the control
        // list too (`art::probe_rect`): a `pointer` step aims only at those, and without it the press went nowhere, the keys were typed
        // into nothing and the scenario's Escape shut Settings
        let bx = if crate::art::rects_on() { bx.relative().child(crate::art::probe_rect("set-search".into())).child(canvas(|_, _, _| (), |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report("set-search", r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { bx };
        nav = nav.child(bx.child(crate::art::icon("search", 14., c("dim"))).child(div().flex_1().child(input.clone())));
    }
    let lines = nav_lines(if hub_on { &ui.nav } else { &[] }, &ui.groups, &ui.finds, &ui.akas, &device_finds(&dev_rows), &ui.q);
    // while something is typed every page stays where it is: the ones with nothing found dimmed, the others with how many were found
    let searching = !ui.q.trim().is_empty();
    for (n, line) in lines.iter().enumerate() {
        match line {
            NavLine::Head(h) => nav = nav.child(div().px_3().pt(px(if n == 0 { 2. } else { 8. })).pb(px(2.)).text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(group_name(h).to_uppercase())),
            NavLine::Item { id, title, hits } => {
                let on = match id.as_str() { "@keys" => on_keys, "@hubs" => on_hubs, "@accounts" => on_acct, p => !on_keys && !on_acct && !on_hubs && p == page };
                let none = searching && hits.is_empty();
                // each entry its own element id, this app's pages keeping their `@`: the hub's API keys page is `keys`, and Keyboard as `set-nav-keys` too
                // took its clicks (two elements of one id: the press went to Keyboard's, so the API keys row never opened)
                let el_id = nav_el_id(id);
                let item = div().id(SharedString::from(el_id)).relative().flex().items_start().gap_3().px_3().py(px(3.)).cursor_pointer().text_size(crate::scale::tpx(13.)).border_l_2().border_color(c(if on { "accent" } else { "bg" }));
                let item = if on { item.bg(c("raise")) } else { item.text_color(c(if none { "faint" } else { "text" })).hover(|s| s.bg(c("panel"))) };
                // its rectangle for the e2e drivers (BACKPLANE_GPUI_RECTS=1 only), named as its control is: `set-nav:<page>` (a GPUI_RECT line, and the
                // control list's rect: `art::probe_rect`)
                let item = if crate::art::rects_on() { let rn = format!("set-nav:{id}"); item.child(crate::art::probe_rect(rn.clone())).child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&rn, r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { item };
                // how many were found, on the title's first line in the entry's right margin (over it, so no title wraps or moves while searching)
                let count = (searching && !hits.is_empty()).then(|| div().absolute().right(px(4.)).top(px(3. + (13. - 11.) * NAV_LINE * crate::scale::s() / 2.)).text_size(crate::scale::tpx(11.)).text_color(c("accent")).child(hits.len().to_string()));
                let to = id.clone();
                nav = nav.child(item.child(nav_icon_el(crate::art::icon(nav_icon(id), 16., c(if on { "accent" } else if none { "faint" } else { "dim" })))).child(nav_title(title)).children(count)
                    .on_click(cx.listener(move |this, _, _, cx| open_nav(this, &to, cx))));
            }
        }
    }
    if !hub_on && ui.q.trim().is_empty() { nav = nav.child(div().px_3().pt_2().text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(trs("The hub's other settings open here when it serves them."))); }
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ let title = if on_hubs { "Hubs".to_string() } else if on_acct { "Accounts".to_string() } else if on_keys { "Keyboard".to_string() } else if on_page { title_now.clone() }
        else { ui.nav.iter().find(|(p, _)| *p == page).map(|(_, t)| t.clone()).or_else(|| LOCAL_PAGES.iter().find(|(_, p, _)| *p == page).map(|(_, _, t)| t.to_string())).unwrap_or_default() };
    // this app's own pages say where they are kept; a hub page says nothing (its rows are the hub's, under "Every device" beside this device's)
    // what the search found, in place of the page, while it holds something and nothing was opened from it since
    let (shows, res) = (results_shown(ui), found(&lines));
    let title = if !shows { trs(&title) } else if res.is_empty() { trs("Nothing matches") } else { tf("gpui.settings.found", &[("n", res.len().into())]) };
    if shows { let mut at: HashMap<&str, usize> = HashMap::new(); els = res.iter().enumerate().map(|(k, (p, t, f))| { let n = at.entry(p.as_str()).or_default(); *n += 1; hit_el(cx, light, k, k == ui.hit.min(res.len().saturating_sub(1)), p, *n - 1, t, f, fit.rows) }).collect(); }
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ let sub = if shows { None } else if on_hubs { Some("Kept on this computer only. Each hub has its own credential here.") } else if on_acct { Some("Kept in the hub for this machine; sign-ins never leave it.") } else if on_keys { Some("Kept on this computer only. Never sent to the hub, and other devices keep their own.") } else { None };
    let dialog = div().id("settings-dialog").occlude().flex().flex_col().w(px(1040.)).max_w_full().h(px(840.)).max_h_full().border_1().border_color(c("line")).bg(c("bg")).overflow_hidden().on_mouse_down_out(cx.listener(|this, _, _, cx| close(this, cx)))
        .capture_any_mouse_down(cx.listener(|this, _, _, cx| if end_recording(&mut this.screens.settings_ui) { cx.notify() }))
        .child(div().h(crate::scale::tpx(44.)).flex_shrink_0().px_4().flex().items_center().border_b_1().border_color(c("line"))
            .child(div().flex_1().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(14.)).child(trs("Settings")))
            .child(div().id("settings-close").p_1().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("text")).bg(c("raise"))).child(crate::art::icon("x", 16., c("dim")))
                .on_click(cx.listener(|this, _, _, cx| close(this, cx)))))
        .child(div().flex().flex_1().min_h_0().child(nav)
            .child(div().id("settings-rows").flex_1().min_w_0().min_h_0().overflow_y_scroll().track_scroll(&ui.scroll).px(px(fit.gutter)).py_3().flex().flex_col()
                // the title, and on its line what the hub refused of a press here (said by the hub, through the catalog already): beside the
                // title, not under it, so no row moves away from the pointer that pressed it
                .child(div().pb_1().flex().items_baseline().gap_3().child(div().flex_shrink_0().text_size(crate::scale::tpx(15.)).font_weight(FontWeight::BOLD).child(title))
                    .children(ui.refused.clone().map(|t| div().min_w_0().truncate().text_size(crate::scale::tpx(12.)).text_color(c("bad")).child(t))))
                .children(sub.map(|t| words(cx, light, div().pb_2().text_size(crate::scale::tpx(12.)).text_color(c(if on_acct { "dim" } else { "accent" })), trs(t)))).children(els)));
    // (the dim backdrop lets a press through: a thread clicked while Settings is open shuts Settings and shows that thread, as 0.14 did;
    // the dialog itself takes its own presses)
    Some(div().id("settings").absolute().inset_0().track_focus(focus).bg(gpui::hsla(0., 0., 0., 0.55)).flex().items_center().justify_center().p(px(fit.pad)).child(dialog).into_any_element())
}

/// A result of the search in the page area: the row's label with the page that holds it at the right (a page found by its name: its title alone),
/// under it its note on one line, a hairline under it as under a setting (the words where a setting's are; `avail`: the rows' width, `Fit::rows`).
/// `on`: the one Enter opens. A press opens it (`open_hit`). Its rectangle for the e2e drivers: `set-hit:<page>:<n>`, the page's `n`th result.
fn hit_el(cx: &mut Context<Backplane>, light: bool, k: usize, on: bool, page: &str, n: usize, title: &str, f: &Found, avail: f32) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    // (the note's width is set, not left to the flex: gpui cuts a line with "…" only at a width it knows when it first measures it)
    let words = div().flex().flex_col().gap_1()
        .child(div().flex().items_center().gap(px(GAP)).child(div().flex_1().min_w_0().text_size(crate::scale::tpx(13.)).child(trs(&f.label))).children(f.row.then(|| div().flex_none().text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(trs(title)))))
        .children((!f.note.is_empty()).then(|| div().w(px(avail)).text_size(crate::scale::tpx(12.)).text_color(c("dim")).truncate().child(trs(&f.note).replace('\n', " "))));
    let d = div().id(SharedString::from(format!("set-hit-{k}"))).mx(px(-8.)).px(px(8.)).cursor_pointer();
    let d = if on { d.bg(c("raise")) } else { d.hover(|s| s.bg(c("panel"))) };
    let d = if crate::art::rects_on() { let rn = format!("set-hit:{page}:{n}"); d.relative().child(crate::art::probe_rect(rn.clone())).child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&rn, r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { d };
    let (p, f2) = (page.to_string(), f.clone());
    d.child(div().py_3().border_b_1().border_color(c("line")).child(words))
        .on_click(cx.listener(move |this, _, _, cx| open_hit(this, &p, &f2, cx))).into_any_element()
}

/// The page the dialog shows (but for Hubs and Accounts): `@keys`, the hub's page (known from its title `hub_title`, as well as from ui.page: a
/// page the hub opened itself), else the one chosen, else Appearance.
pub fn page_shown(ui: &SettingsUi, hub_title: &str, keys: bool) -> String {
    let chosen = if ui.page.is_empty() { "appearance".to_string() } else { ui.page.clone() };
    if keys { return "@keys".into() }
    if hub_title.is_empty() { return chosen }
    hub_page(&ui.nav, hub_title).unwrap_or(chosen)
}
/// The page id of a hub page's title (the menu's, else an older hub's titles of About and Network and updates); None while the menu is unknown.
pub fn hub_page(nav: &[(String, String)], title: &str) -> Option<String> {
    /* i18n-ok: page titles from hub */
    nav.iter().find(|(_, t)| *t == title).map(|(p, _)| p.clone())
        .or_else(|| match title { "About and updates" | "About" => Some("about".to_string()), "Network and updates" => Some("network".to_string()), _ => None })
}
/// Whether the hub still shows another page than the one chosen here (a press on the nav, or the gear, not answered yet): the dialog then draws
/// the chosen page's title and this device's rows on it at once, and the hub's rows when they come.
pub fn hub_behind(ui: &SettingsUi, hub_title: &str) -> bool { !ui.page.is_empty() && hub_page(&ui.nav, hub_title).as_deref() != Some(ui.page.as_str()) }
/// The page whose Advanced fold is drawn now, if one is (the e2e drivers' `set-adv:<page>`).
pub fn adv_shown(this: &Backplane) -> Option<String> {
    let ui = &this.screens.settings_ui;
    if !is_open(this) || ui.accounts || ui.hubs { return None }
    let rows = parse(&this.data["settings"]);
    let on_page = this.data["settings"].is_object() && !ui.keys && matches!(shown(&rows), Shown::Page) && !hub_behind(ui, &rows[0].label);
    let page = page_shown(ui, if on_page { rows[0].label.as_str() } else { "" }, ui.keys);
    let adv = device_section(&page, &device_rows(&this.local)).iter().any(is_adv) || (on_page && rows.iter().any(is_adv));
    adv.then_some(page)
}
/// The Advanced fold of a page: a line as tall as a setting row, with its hairline, the chevron the sidebar's folds use, and how many settings are
/// under it. A press opens or shuts it; it stays where it is (the rows appear under it).
fn adv_el(cx: &mut Context<Backplane>, light: bool, page: &str, n: usize, open: bool) -> AnyElement { fold_el(cx, light, page, &trs("Advanced"), n, open) }
/// The same fold line under another name (Settings › Accounts' "Not set up", keyed `accounts`): its open state is `SettingsUi::adv`'s, toggled by `toggle_adv`.
pub(super) fn fold_el(cx: &mut Context<Backplane>, light: bool, page: &str, label: &str, n: usize, open: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let to = page.to_string();
    let d = div().id(SharedString::from(format!("set-adv-{page}"))).flex().items_center().py_3().border_b_1().border_color(c("line")).cursor_pointer().hover(|s| s.bg(c("panel")))
        .child(div().h(px(24.)).flex().items_center().gap_2().child(crate::art::icon(if open { "chevron-down" } else { "chevron-right" }, 14., c("dim")))
            .child(div().text_size(crate::scale::tpx(13.)).child(label.to_string())).child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(n.to_string())));
    // its rectangle for the e2e drivers (BACKPLANE_GPUI_RECTS=1 only): `set-adv:<page>`
    let d = if crate::art::rects_on() { let rn = format!("set-adv:{page}"); d.relative().child(crate::art::probe_rect(rn.clone())).child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(&rn, r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { d };
    d.on_click(cx.listener(move |this, _, _, cx| toggle_adv(this, &to, cx))).into_any_element()
}

/// This device's rows, under the headings `device_section` sorts them by onto the pages of their topic: what only this computer decides.
/// Every button is `local:<name>` with a value; none reaches the hub.

pub fn device_rows(l: &crate::local::Local) -> Vec<Row> {
    use crate::local as lo;
    let b = |label: &str, name: &str, value: &str, on: bool| Btn { label: label.into(), action: format!("local:{name}"), value: value.into(), on };
    let r = |label: &str, note: &str, buttons: Vec<Btn>| Row { label: label.into(), note: note.into(), buttons, fields: vec![], qr: vec![], qrnote: String::new(), local: true, tone: String::new(), ..Default::default() };
    let mut rows = vec![
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Appearance", "", vec![]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Language", "This device. Automatic follows the system language.", crate::i18n::CHOICES.iter().map(|(k, key)| b(&crate::i18n::t(key), "lang", k, l.lang == *k)).collect()),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Theme", "Light or dark on this device, or follow what the hub says.", lo::THEMES.iter().map(|(k, lab)| b(lab, "theme", k, l.theme == *k)).collect()),
        // the text size: a slider (its stops are the setting's steps) and Reset beside it
        Row { raw: serde_json::json!({"slider": {"name": "text", "min": lo::TEXT_PCT.0, "max": lo::TEXT_PCT.1, "step": lo::TEXT_PCT.2, "value": l.text_pct}}),
            /* i18n-ok: drawn through trs by row_el, nav_title or the title */ ..r("Text size", &tf("gpui.settings.textpct", &[("pct", l.text_pct.into())]), vec![b("Reset", "text-reset", "", l.text_pct != 100)]) }, // lit when there is something to reset
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Font", "The typeface of the interface and the thread.", lo::FONTS.iter().map(|(k, lab)| b(lab, "font", k, l.font == *k)).collect()),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("3D viewer background", "Behind Mech parts and the 3D board: Auto follows the theme.", lo::VIEWER_BGS.iter().map(|(k, lab)| b(lab, "viewer-bg", k, l.viewer_bg == *k)).collect()),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Custom background colour", "Pick a colour, then a lighter or darker shade of it.", lo::VIEWER_BG_SWATCHES.iter().map(|(lab, c)| b(lab, "viewer-bg-color", &format!("{c:06x}"), l.viewer_bg == "custom" && l.viewer_bg_rgb == *c))
            /* i18n-ok: drawn through trs by row_el */ .chain([b("Lighter", "viewer-bg-shade", "+", false), b("Darker", "viewer-bg-shade", "-", false)]).collect()),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Layout", "", vec![]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Sidebar width", &tf("gpui.settings.px", &[("px", l.sidebar_px.into())]), vec![b("Narrower", "sidebar-width", "-", false), b("Wider", "sidebar-width", "+", false)]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Sidebar", "Whether it is folded when the app opens.", vec![b("Open", "sidebar-open", "on", l.sidebar_open), b("Folded", "sidebar-open", "off", !l.sidebar_open)]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Opens on", "What this device shows first.", lo::VIEWS.iter().map(|(k, lab)| b(lab, "default-view", k, l.default_view == *k)).collect()),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Notifications", "", vec![]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Desktop notifications", "Kept for this device. Alerts for what needs you and for settled work show as desktop notifications.", vec![b("On", "notifications", "on", l.notifications), b("Off", "notifications", "off", !l.notifications)]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Streaming", "", vec![]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Streamer mode", "Hides emails, account names, home folders, machine names, addresses, codes and keys wherever they are drawn, for screen recordings and streams. Pictures in threads are not hidden.", vec![b("On", "streamer", "on", l.streamer), b("Off", "streamer", "off", !l.streamer)]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Turn on while the screen is shared", crate::streamer::detect::what_auto_sees(), vec![b("On", "streamer-share", "on", l.streamer_share), b("Off", "streamer-share", "off", !l.streamer_share)]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Turn on while a recording app runs", "A guess: OBS, SimpleScreenRecorder, Kazam, vokoscreen, Peek, Kooha, GPU Screen Recorder, wf-recorder, ffmpeg grabbing the screen (and Loom, ScreenFlow, Streamlabs ... on macOS and Windows). It turns on while one runs, recording or not.", vec![b("On", "streamer-apps", "on", l.streamer_apps), b("Off", "streamer-apps", "off", !l.streamer_apps)]),
        /* i18n-ok: drawn through trs by row_el, nav_title or the title */ r("Keyboard shortcuts", "", vec![]),
    ];
    for x in crate::keys::TABLE {
        if x.kind == crate::keys::Kind::Fixed { rows.push(r(&format!("{} · {}", trs(x.label), trs(x.scope)), &tf("gpui.keys.fixed", &[("key", crate::keys::hint(l, x.id).into())]), vec![])); }
        else { let k = crate::keys::hint(l, x.id); rows.push(r(&format!("{} · {}", trs(x.label), trs(x.scope)), &tf("gpui.keys.now", &[("key", k.clone().into())]), vec![b(&k, "shortcut", x.id, false)])); }
    }
    for (k, why) in crate::keys::MISSING { rows.push(r(k, &tf("gpui.keys.notyet", &[("why", trs(why).into())]), vec![])); }
    /* i18n-ok: drawn through trs by row_el, nav_title or the title */ rows.push(r("Shortcuts to defaults", "", vec![b("Reset", "shortcuts-reset", "", false)]));
    rows
}

#[cfg(test)]
mod tests {
    #[test]
    fn settings_nav_ids_differ_for_api_keys_and_keyboard() {
        use super::nav_el_id;
        let ids = ["keys", "@keys", "hubs", "@hubs", "accounts", "@accounts"].map(nav_el_id);
        for (i, a) in ids.iter().enumerate() { for b in &ids[i + 1..] { assert_ne!(a, b); } }
    }
    use super::*;
    #[test] fn enter_in_the_masked_box_is_its_save() {
        assert_eq!(secret_enter(&serde_json::json!({"thread":{"secretBox":{"save":"@secret\u{1f}gh"}}})).as_deref(), Some("@secret\u{1f}gh"));
        assert_eq!(secret_enter(&serde_json::json!({"thread":{}})), None, "no box, nothing to save");
    }
    #[test] fn the_dialog_fits_a_small_window() {
        assert_eq!(fit(1400., 900.), Fit { pad: 24., nav: 220., gutter: 24., rows: 1040. - 2. - 220. - 48. }, "a desktop window");
        let f = fit(1024., 600.); assert_eq!(f.pad, 8., "a short window keeps its height for the rows");
        let f = fit(800., 600.); assert_eq!(f.nav, 180.);
        let f = fit(600., 500.); assert_eq!((f.nav, f.gutter), (150., 12.)); assert!(f.rows > 300., "the rows keep room to read");
        let f = fit(420., 600.); assert!(f.rows >= 200., "{f:?}");
    }
    #[test] fn setup_path_boxes_keep_one_line_while_typing() {
        // a wrapped default path and a shorter typed one moved "Use this folder" up a line under the pointer
        for n in ["obproject", "obkicad", "obfreecad", "obaonaddr", "obaoncode", "aonaddr", "aoncode"] { assert!(ONE_LINE.contains(&n), "{n}"); }
        assert!(!ONE_LINE.contains(&"obfirst"), "the first task is words to write: it wraps");
    }
    #[test] fn a_row_keeps_its_words_readable_and_its_chips_inside_the_room_they_have() {
        let fonts: Vec<f32> = ["Noto Sans Mono", "JetBrains Mono", "Fira Code", "IBM Plex Mono", "Source Code Pro"].iter().map(|l| chip_w(l)).collect();
        // wide: beside the words, the chips in lines no wider than the controls' room
        let wide = row_fit(fit(1400., 900.).rows, &fonts, 0., 0.); assert!(!wide.stack);
        let fits = |avail: f32, f: &RowFit| { let room = if f.stack { avail } else { (avail - LABEL_MIN - GAP).min(CONTROLS_MAX) }; let mut i = 0; for &n in &f.lines { let w: f32 = fonts[i..i + n].iter().sum::<f32>() + 4. * (n as f32 - 1.); assert!(n == 1 || w <= room, "{w} > {room}"); i += n; } assert_eq!(i, fonts.len()); };
        fits(fit(1400., 900.).rows, &wide);
        // every window from phone-thin to wide: the words keep LABEL_MIN beside the controls, or have the whole row with the controls under them
        for w in [420., 500., 600., 700., 800., 900., 1000., 1200.] {
            let avail = fit(w, 700.).rows; let f = row_fit(avail, &fonts, 0., 0.); fits(avail, &f);
            if !f.stack { assert!(avail - LABEL_MIN - GAP >= fonts.iter().copied().fold(0., f32::max), "{w}"); }
        }
        assert!(row_fit(fit(420., 700.).rows, &fonts, 0., 0.).stack, "a thin window stacks");
        assert!(row_fit(fit(420., 700.).rows, &[], 260., 0.).stack, "and a text field goes under its words");
        assert!(!row_fit(fit(1400., 900.).rows, &[], 260., 0.).stack);
        assert_eq!(row_fit(100., &[300.], 0., 0.).lines, vec![1], "a chip wider than the room still has a line");
        assert_eq!(row_fit(2000., &[50.; 9], 0., 0.).lines, vec![6, 3], "at most CHIPS_PER_LINE a line");
        assert!(chip_w("日本語") > chip_w("abc"));
    }
    #[test] fn the_text_size_is_a_slider_with_a_reset_and_a_press_on_its_track_is_a_snapped_value() {
        let rows = device_rows(&crate::local::Local { text_pct: 130, ..Default::default() });
        let r = rows.iter().find(|r| r.label == "Text size").unwrap();
        let sl = slider_of(&r.raw).expect("a slider");
        assert_eq!(sl, Slider { name: "text".into(), min: 80, max: 160, step: 5, value: 130 });
        assert_eq!(r.buttons.iter().map(|b| b.value.as_str()).collect::<Vec<_>>(), [""], "only Reset beside it");
        assert_eq!(slider_frac(&sl), 0.625);
        let (left, w) = (100., SLIDER_W);
        assert_eq!(slider_at(&sl, left, left, w), 80, "the start"); assert_eq!(slider_at(&sl, left + w, left, w), 160, "the end");
        assert_eq!(slider_at(&sl, left - 50., left, w), 80); assert_eq!(slider_at(&sl, left + w + 50., left, w), 160, "past the ends, the ends");
        assert_eq!(slider_at(&sl, left + THUMB / 2. + (w - THUMB) / 2., left, w), 120, "the middle");
        assert!(slider_of(&serde_json::json!({"slider": {"name": "x", "min": 5, "max": 5}})).is_none());
        // a slider row lays out the slider and its chips as one line beside the words, or under them in a thin window
        assert!(!row_fit(fit(1400., 900.).rows, &[], SLIDER_W + 8. + chip_w("Reset"), 0.).stack);
        assert!(row_fit(fit(420., 700.).rows, &[], SLIDER_W + 8. + chip_w("Reset"), 0.).stack);
    }
    use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn the_device_page_lists_the_drawing_pads_keys_and_no_longer_says_they_are_missing() {
        let rows = device_rows(&crate::local::Local::default());
        let note = |label: &str| rows.iter().find(|r| r.label == label).map(|r| r.note.clone());
        assert_eq!(note("Undo the last stroke or change · Drawing pad").as_deref(), Some(format!("{}. Fixed.", crate::keys::show("secondary-z", crate::keys::mac())).as_str()));
        assert_eq!(note("Delete the selection, or clear the pad · Drawing pad").as_deref(), Some("Delete. Fixed."));
        assert_eq!(note("Delete (also) · Drawing pad").as_deref(), Some("Backspace. Fixed."));
        assert!(!rows.iter().any(|r| r.note.contains("Not yet") && r.label.to_lowercase().contains("drawing pad")));
    }
    #[test] fn the_performance_overlay_is_a_shortcut_on_the_device_page_and_no_longer_a_not_yet() {
        let rows = device_rows(&crate::local::Local::default());
        let r = rows.iter().find(|r| r.label.starts_with("Performance overlay")).expect("its row");
        assert!(r.note.contains("Ctrl+Shift+H") || r.note.contains("⌃⇧H"), "{}", r.note);
        assert_eq!((r.buttons[0].action.as_str(), r.buttons[0].value.as_str()), ("local:shortcut", "perf"));
        assert!(!rows.iter().any(|r| r.note.starts_with("Not yet") && r.note.contains("Performance")));
    }
    #[test] fn a_qr_code_is_laid_out_as_wide_as_it_is_drawn() {
        // 33 modules and a quiet zone of 4 each side, 4 px each; the 600 px dialog row (the pairing invitation) keeps it beside the words
        assert_eq!(qr_w(33), 164.);
        let f = row_fit(600., &[chip_w("Copy link"), chip_w("Copy code")], 0., qr_w(33));
        assert!(!f.stack);
    }
    #[test] fn rows_parse_with_buttons_fields_and_qr_and_a_vault_field_keeps_no_hub_value() {
        let r = parse(&j(r#"{"rows":[{"label":"Theme","note":"light or dark","buttons":[{"label":"Dark","action":"theme","value":"dark","on":true},{"label":"Light","action":"theme","value":"light","on":false}]},
            {"label":"Key","note":"","buttons":[],"fields":[{"name":"kname","hint":"name","secret":false,"value":"OPENAI"},{"name":"kval","hint":"value","secret":true,"vault":true,"value":"leaked-from-hub"}]},
            {"label":"Pair","note":"","buttons":[],"qr":["101","010","101"],"qrnote":"scan"}]}"#));
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].buttons[0], Btn { label: "Dark".into(), action: "theme".into(), value: "dark".into(), on: true });
        assert!(!r[1].fields[0].vault && r[1].fields[0].value == "OPENAI"); assert!(r[1].fields[1].vault && r[1].fields[1].value.is_empty(), "a vault value is never taken from the hub");
        assert_eq!(r[2].qr.len(), 3); assert_eq!(r[2].qrnote, "scan"); assert!(parse(&Value::Null).is_empty());
    }
    #[test] fn a_button_press_is_its_action_and_a_form_field_change_is_bfield_name_unit_text() {
        match press(&Btn { label: "x".into(), action: "theme".into(), value: "dark".into(), on: false }) { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("theme", "dark")), c => panic!("{c:?}") }
        match field_command("kname", "OPENAI_API_KEY") { Command::Action(a, v) => { assert_eq!(a, "bfield"); assert_eq!(v, "kname\u{1f}OPENAI_API_KEY"); } c => panic!("{c:?}") }
    }
    #[test] fn a_sign_in_box_goes_as_its_own_request_and_names_its_field() {
        let (m, p) = save_request("@signin-key\u{1f}p1\u{1f}grok", "", "xai-SECRET").unwrap();
        assert_eq!((m.as_str(), p["action"].as_str(), p["value"].as_str(), p["fields"]["sikeys"].as_str()), ("machines.page.act", Some("secret-save"), Some("@signin-key\u{1f}p1\u{1f}grok"), Some("xai-SECRET")));
        assert_eq!(save_request("@signin-sync", "", "CODE#S").unwrap().1["fields"]["sicode"], "CODE#S");
        assert_eq!(signin_field("@apikey\u{1f}X"), None); assert!(save_request("@signin\u{1f}p1\u{1f}codex", "", " ").is_err());
    }
    /// The secret never rides a view action: typing in a vault field sends nothing, and the Save is a direct request.
    #[test] fn only_web_addresses_open() { assert!(openable("https://backplane.works/privacy") && openable("http://127.0.0.1:1/x")); for u in ["file:///etc/passwd", "javascript:alert(1)", "https://a b", "", "ftp://x"] { assert!(!openable(u), "{u}"); } }
    #[test] fn a_vault_value_never_travels_as_a_command_action() {
        let vault = Field { name: "kval".into(), hint: "".into(), secret: true, value: String::new(), vault: true };
        let plain = Field { name: "kname".into(), hint: "".into(), secret: false, value: String::new(), vault: false };
        assert!(change(&vault, "sk-SECRET").is_none()); assert!(change(&plain, "OPENAI").is_some());
        let (m, p) = save_request("@apikey\u{1f}OPENAI", "", "sk-SECRET").unwrap();
        assert_eq!((m.as_str(), p["op"].as_str(), p["name"].as_str(), p["value"].as_str()), ("bots.voice", Some("api-add"), Some("OPENAI"), Some("sk-SECRET")));
        let cmd = Command::Rpc(m, Secret(p), "key");
        assert!(!format!("{cmd:?}").contains("SECRET"), "Debug of a request shows no secret"); assert!(!format!("{vault:?}").contains("SECRET"));
        let (m, p) = save_request("db\u{1f}for the db\u{1f}t1", "", "pw").unwrap();
        assert_eq!((m.as_str(), p["name"].as_str(), p["purpose"].as_str(), p["thread"].as_str()), ("secret.set", Some("db"), Some("for the db"), Some("t1")));
        assert!(save_request("@apikey\u{1f}X", "", "   ").is_err(), "no value, no request"); assert!(save_request("@apikey\u{1f}", "", "v").unwrap_err().contains("Name"), "no name says why");
        assert_eq!(save_request("@apikey\u{1f}old", "typed", "v").unwrap().1["name"], "typed", "the name typed last wins");
    }
    /// Type "abc": the reply for "a" arrives late and must not put "a" back; when the hub really clears the field afterwards, it is cleared.
    #[test] fn the_hubs_reason_is_read_from_a_plain_error_string() {
        assert_eq!(reply_error(&j(r#"{"t":"reply","id":1073741825,"ok":false,"error":"A secret is at least 8 characters"}"#)), "A secret is at least 8 characters");
        assert_eq!(reply_error(&j(r#"{"ok":false,"error":{"message":"nope","code":"bad"}}"#)), "nope"); assert_eq!(reply_error(&j(r#"{"ok":false,"error":{"code":"bad"}}"#)), "bad");
        assert_eq!(reply_error(&j(r#"{"ok":true}"#)), "");
    }
    #[test] fn echoes_are_ignored_and_a_real_clear_after_catching_up_is_taken() {
        let mut e = Echo::new("");
        for t in ["a", "ab", "abc"] { e.remember(t); }
        assert!(!e.on_hub("a", "abc"), "a late echo of an older text"); assert!(!e.on_hub("ab", "abc"));
        assert!(!e.on_hub("abc", "abc"), "caught up: the log is settled");
        assert!(e.on_hub("", "abc"), "the hub cleared the field (a Save or a Set): that is a real change");
        let mut f = Echo::new(""); f.remember("x");
        assert!(f.on_hub("changed on another device", "x"), "a real change from elsewhere");
    }
    fn row(label: &str, page: &str) -> Row { Row { label: label.into(), note: String::new(), buttons: if page.is_empty() { vec![] } else { vec![Btn { label: format!("{label} ›"), action: "settings-page".into(), value: page.into(), on: false }] }, fields: vec![], qr: vec![], qrnote: String::new(), local: false, tone: String::new(), ..Default::default() } }
    #[test] fn the_pages_menu_is_told_from_a_page_and_the_nav_keeps_the_hubs_order() {
        let menu = vec![row("Usage", "usage"), row("Appearance", "appearance"), row("Voice", "voice"), row("Agents", "agents")];
        match shown(&menu) { Shown::Menu(items) => assert_eq!(items.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["usage", "appearance", "voice", "agents"], "the hub's order: the client sorts nothing"), s => panic!("{s:?}") }
        let mut back = row("‹ Settings", ""); back.buttons.push(Btn { label: "‹ Settings".into(), action: "settings-page".into(), value: String::new(), on: false });
        assert_eq!(shown(&[back, row("Theme", "")]), Shown::Page);
        assert_eq!(shown(&[row("Theme", ""), row("Language", "")]), Shown::Other);
        assert_eq!(shown(&[]), Shown::Other);
    }
    fn menu_json() -> Value { serde_json::json!([
        {"label":"Appearance","group":"General","words":"Theme\nThread list\nGroup projects","notes":"The window and the web page.\n\nOn: projects with nothing going on lately are grouped by their git owner.","buttons":[{"action":"settings-page","value":"appearance"}]},
        {"label":"Agents","group":"Agents and tools","words":"Default agent","buttons":[{"action":"settings-page","value":"agents"}]},
        {"label":"Devices","group":"Connect","words":"Add a device\nWeb UI\nBrowser access\nWeb address","notes":"Makes a link.\n\nPair a browser over HTTPS on your tailnet.\n","also":"pair phone qr code link android iphone\n\nweb ui https tailscale\n","buttons":[{"action":"settings-page","value":"devices"}]},
        {"label":"Notifications","group":"General","words":"Notify me for\nQuiet hours","notes":"Phone and desktop alerts.\nNo alerts between these times.","also":"\nnight do not disturb dnd silence mute sleep","buttons":[{"action":"settings-page","value":"notifications"}]},
        {"label":"Privacy","group":"System","words":"Share anonymous usage","notes":"Counts of what is used.","also":"telemetry analytics privacy data","buttons":[{"action":"settings-page","value":"privacy"}]},
        {"label":"Voice","buttons":[{"action":"settings-page","value":"voice"}]}]) }
    fn lines_for(q: &str) -> Vec<NavLine> {
        let m = menu_json(); let nav: Vec<(String, String)> = m.as_array().unwrap().iter().map(|r| (r["buttons"][0]["value"].as_str().unwrap().to_string(), r["label"].as_str().unwrap().to_string())).collect();
        let device: HashMap<String, Vec<Find>> = [("appearance".to_string(), vec![Find { label: "Font".into(), note: "The typeface of the interface and the thread.".into(), also: "typeface mono".into() }])].into_iter().collect();
        nav_lines(&nav, &menu_groups(&m), &menu_finds(&m), &menu_akas(&m), &device, q)
    }
    fn heads(l: &[NavLine]) -> Vec<&str> { l.iter().filter_map(|x| match x { NavLine::Head(h) => Some(h.as_str()), _ => None }).collect() }
    fn ids(l: &[NavLine]) -> Vec<&str> { l.iter().filter_map(|x| match x { NavLine::Item { id, .. } => Some(id.as_str()), _ => None }).collect() }
    /// The pages the search found something on.
    fn hit_ids(l: &[NavLine]) -> Vec<&str> { l.iter().filter_map(|x| match x { NavLine::Item { id, hits, .. } if !hits.is_empty() => Some(id.as_str()), _ => None }).collect() }
    fn labels(h: &[Found]) -> Vec<&str> { h.iter().map(|f| f.label.as_str()).collect() }
    fn f(label: &str, note: &str) -> Find { Find { label: label.into(), note: note.into(), also: String::new() } }
    #[test] fn the_search_finds_a_page_by_a_setting_it_holds_or_what_its_note_says() {
        let m = menu_finds(&menu_json());
        assert_eq!(m.len(), 5, "a page with no words is found by its title only"); assert_eq!(m["appearance"][0], f("Theme", "The window and the web page.")); assert_eq!(m["agents"], [f("Default agent", "")], "an older hub sends no notes nor also");
        let a = &m["appearance"];
        assert_eq!(labels(&hits(a, "group")), ["Group projects"]); assert_eq!(labels(&hits(a, " Group Projects ")), ["Group projects"]); assert_eq!(labels(&hits(a, "thread")), ["Thread list"]);
        assert_eq!(labels(&hits(a, "git owner")), ["Group projects"], "found by its note"); assert_eq!(labels(&hits(a, "web page")), ["Theme"]);
        assert!(hits(a, "voice").is_empty() && hits(a, "").is_empty() && hits(&[], "group").is_empty(), "an empty search is the titles' to match");
        // by what a row does or was called (the hub's `also`), line for line with its label
        assert_eq!(m["devices"][2].also, "web ui https tailscale"); assert_eq!(m["devices"][3].also, "");
        assert_eq!(labels(&hits(&m["notifications"], "night")), ["Quiet hours"]); assert_eq!(labels(&hits(&m["privacy"], "telemetry")), ["Share anonymous usage"]);
        assert_eq!(labels(&hits(&m["devices"], "pair")), ["Add a device", "Browser access"], "Add a device by its words, Browser access by its note");
        assert_eq!(hit_ids(&lines_for("night")), ["notifications"]); assert_eq!(hit_ids(&lines_for("typeface")), ["appearance"], "this device's rows by their words too");
        assert_eq!(labels(&hits(&[f("Theme", ""), Find { also: "dark mode".into(), ..f("Font", "") }], "dark")), ["Font"]);
    }
    #[test] fn the_nav_draws_the_hubs_groups_and_a_search_keeps_every_page_with_what_it_found() {
        let all = lines_for("");
        assert_eq!(heads(&all), ["General", "Agents and tools", "Connect", "General", "System", "All devices (hub)", "This device"], "a page the hub put in no group goes under the hub's own name");
        assert_eq!(ids(&all), ["appearance", "@accounts", "agents", "devices", "notifications", "privacy", "voice", "@keys", "@hubs"],
            "Accounts leads the group that holds Agents; a hub without Mouse or Machines has this app's pages for them at the end");
        assert!(all.iter().all(|l| !matches!(l, NavLine::Item { hits, .. } if !hits.is_empty())), "nothing is found while nothing is typed");
        // a search keeps the whole list, in place, and says how many it found on each page
        for q in ["tailnet", "web", "zzz", "night"] { let l = lines_for(q); assert_eq!(ids(&l), ids(&all), "{q}"); assert_eq!(heads(&l), heads(&all), "{q}"); }
        let fw = lines_for("tailnet");
        assert_eq!(fw.iter().find_map(|l| match l { NavLine::Item { id, hits, .. } if id == "devices" => Some(hits.clone()), _ => None }).unwrap(), [Found { label: "Browser access".into(), nth: 0, note: "Pair a browser over HTTPS on your tailnet.".into(), row: true }]);
        assert_eq!(hit_ids(&fw), ["devices"]); assert_eq!(first_found(&fw).map(|(p, t, f)| (p, t, f.label)), Some(("devices".into(), "Devices".into(), "Browser access".into())), "Enter opens it");
        assert_eq!(hit_ids(&lines_for("web")), ["appearance", "devices"]); assert_eq!(hit_ids(&lines_for("font")), ["appearance"], "this device's own settings are found on the page that holds them");
        assert_eq!(found(&lines_for("web")).iter().map(|(p, _, f)| (p.as_str(), f.label.as_str())).collect::<Vec<_>>(), [("appearance", "Theme"), ("devices", "Web UI"), ("devices", "Browser access"), ("devices", "Web address")], "in the nav's order, each page's in its order");
        // a page found by its title (or another name) and nothing in it: the page itself is the one result
        assert_eq!(found(&lines_for("VOICE")), [("voice".to_string(), "Voice".to_string(), Found { label: "Voice".into(), nth: 0, note: String::new(), row: false })], "a title, in any case");
        assert_eq!(hit_ids(&lines_for("acc")), ["@accounts", "devices"]);
        assert!(hit_ids(&lines_for("zzz")).is_empty() && first_found(&lines_for("zzz")).is_none());
        // a hub that names no groups (an older one): one list under the hub's name, Accounts before Agents, this app's pages at the end
        let nav = vec![("appearance".to_string(), "Appearance".to_string()), ("agents".to_string(), "Agents".to_string())];
        let old = nav_lines(&nav, &HashMap::new(), &HashMap::new(), &HashMap::new(), &HashMap::new(), "");
        assert_eq!(heads(&old), ["All devices (hub)", "This device"]); assert_eq!(ids(&old), ["appearance", "@accounts", "agents", "@keys", "@hubs", "notifications", "privacy"]);
    }
    #[test] fn a_note_with_a_sentence_on_a_line_of_its_own_is_translated_line_by_line() {
        crate::i18n::with_lang("ja", || {
            let n = tr_lines("stock KiCad 10.0.7 · /usr/bin/kicad-cli\nEmpty uses the one on PATH.");
            assert!(n.ends_with(&format!("\n{}", trs("Empty uses the one on PATH."))) && !n.contains("Empty"), "{n}");
            assert_eq!(tr_lines("Empty looks for it."), trs("Empty looks for it."));
        });
        assert_eq!(tr_lines("/x/freecadcmd\nEmpty looks for it."), "/x/freecadcmd\nEmpty looks for it.");
    }
    #[test] fn a_result_lands_on_its_row_the_nth_of_a_label_and_the_child_the_scroll_is_given() {
        // a label a page has twice (Mouse and trackpad's "Left drag", once a kind) is found as each of its rows
        let fs = [f("Board and schematic: each input", ""), f("Left drag", "Pans the board."), f("3D and STEP: each input", ""), f("Left drag", "Orbits the model.")];
        assert_eq!(hits(&fs, "left drag").iter().map(|h| (h.label.as_str(), h.nth, h.note.as_str())).collect::<Vec<_>>(), [("Left drag", 0, "Pans the board."), ("Left drag", 1, "Orbits the model.")]);
        let list = parse(&j(r#"{"rows":[{"label":"Zoom direction","note":"n","buttons":[{"label":"On","action":"x","value":"","on":true}]},{"label":"Board and schematic: each input","adv":true},{"label":"Left drag","note":"a","adv":true,"buttons":[{"label":"Pan","action":"x","value":"","on":true}]},
            {"label":"3D and STEP: each input","adv":true},{"label":"Left drag","note":"b","adv":true,"buttons":[{"label":"Orbit","action":"x","value":"","on":true}]},{"label":"Reset","note":"r","buttons":[{"label":"Reset","action":"x","value":"","on":false}]}]}"#));
        assert_eq!((nth_row(&list, "Left drag", 0), nth_row(&list, "Left drag", 1), nth_row(&list, "Left drag", 2), nth_row(&list, "Zoom direction", 0)), (Some(2), Some(4), None, Some(0)));
        // the title, then the rows above the fold (Zoom direction, Reset), the fold line, the folded rows in their order
        assert_eq!(row_child(&list, 0, false, 0), 1); assert_eq!(row_child(&list, 5, false, 0), 2); assert_eq!(row_child(&list, 1, false, 0), 4);
        assert_eq!(row_child(&list, 4, false, 0), 7); assert_eq!(row_child(&list, 4, true, 1), 9, "a page's own words and Usage's bars come before the rows");
    }
    /// The hub's menu as it is now (present/settings.rs PAGES, GROUPS and some of AKA).
    fn hub_menu() -> Value {
        let pages = [("appearance", "Appearance", "General"), ("notifications", "Notifications", "General"), ("threads", "Threads and git", "General"), ("mouse", "Mouse and trackpad", "General"),
            ("agents", "Agents", "Agents"), ("keys", "API keys", "Agents"), ("mcp", "MCP servers", "Agents"), ("rules", "Rules and task board", "Agents"), ("voice", "Voice", "Agents"), ("usage", "Usage", "Agents"),
            ("cad", "CAD", "Workspace"), ("browser", "Browser", "Workspace"), ("ordering", "Ordering", "Workspace"), ("devices", "Devices", "Connect"), ("machines", "Machines", "Connect"),
            ("setup", "Setup", "System"), ("privacy", "Privacy", "System"), ("about", "About and updates", "System")];
        let aka = |id: &str| match id { "notifications" => "Time zone\nWebhook\nQuiet hours", "about" => "About\nNetwork and updates\nNetwork\nUpdates", "threads" => "Threads\nProjects and git", _ => "" };
        Value::Array(pages.iter().map(|(id, t, g)| serde_json::json!({"label": t, "group": g, "aka": aka(id), "words": "", "buttons": [{"label": format!("{t} ›"), "action": "settings-page", "value": id}]})).collect())
    }
    fn hub_lines(q: &str) -> Vec<NavLine> {
        let m = hub_menu(); let Shown::Menu(nav) = shown(&parse(&serde_json::json!({"rows": m}))) else { panic!("a menu") };
        nav_lines(&nav, &menu_groups(&m), &menu_finds(&m), &menu_akas(&m), &device_finds(&device_rows(&crate::local::Local::default())), q)
    }
    #[test] fn the_nav_places_this_apps_pages_inside_the_hubs_groups() {
        let all = hub_lines("");
        assert_eq!(heads(&all), ["General", "Agents", "Workspace", "Connect", "System"]);
        assert_eq!(ids(&all), ["appearance", "notifications", "threads", "@keys", "mouse", "@accounts", "agents", "keys", "mcp", "rules", "voice", "usage", "cad", "browser", "ordering",
            "devices", "machines", "@hubs", "setup", "privacy", "about"], "Keyboard before Mouse and trackpad, Accounts before Agents, Hubs after Machines");
        assert_eq!(ids(&all).len(), 21);
        // no hub: the pages that hold this device's rows, and Hubs
        let none = nav_lines(&[], &HashMap::new(), &HashMap::new(), &HashMap::new(), &device_finds(&device_rows(&crate::local::Local::default())), "");
        assert_eq!(ids(&none), ["appearance", "notifications", "@keys", "@hubs", "privacy"]); assert_eq!(heads(&none), ["General", "Connect", "System"]);
    }
    #[test] fn the_search_finds_a_page_by_its_other_names_and_this_devices_rows_where_they_are() {
        assert!(hit_ids(&hub_lines("time zone")).contains(&"notifications"), "Time zone was a page of its own");
        assert!(hit_ids(&hub_lines("network")).contains(&"about"), "Network and updates is About and updates now");
        assert!(hit_ids(&hub_lines("shortcut")).contains(&"@keys"), "the shortcuts are on Keyboard");
        assert!(hit_ids(&hub_lines("streamer")).contains(&"privacy") && !hit_ids(&hub_lines("streamer")).contains(&"appearance"), "Streamer mode is on Privacy");
        assert!(hit_ids(&hub_lines("desktop notifications")).contains(&"notifications"));
        assert!(!hit_ids(&hub_lines("time zone")).contains(&"about"), "an other name is not a page's when nothing in it matches");
        assert!(hit_ids(&hub_lines("obs")).contains(&"privacy") && hit_ids(&hub_lines("dark mode")).contains(&"appearance"), "this device's rows by their search words");
        assert_eq!(menu_akas(&hub_menu())["notifications"], ["Time zone", "Webhook", "Quiet hours"]); assert!(!menu_akas(&hub_menu()).contains_key("cad"), "no other names, no entry");
    }
    #[test] fn device_inventory_is_all_still_reachable() {
        let rows = device_rows(&crate::local::Local::default());
        let pages = ["appearance", "notifications", "privacy", "@keys"];
        let on: Vec<Row> = pages.iter().flat_map(|p| device_section(p, &rows)).collect();
        let names: Vec<String> = on.iter().flat_map(|r| r.buttons.iter().filter_map(|b| b.action.strip_prefix("local:").map(str::to_string)).chain(slider_of(&r.raw).map(|s| s.name))).collect();
        for n in ["lang", "theme", "viewer-bg", "viewer-bg-color", "viewer-bg-shade", "text-reset", "font", "sidebar-width", "sidebar-open", "default-view", "notifications", "streamer", "streamer-share", "streamer-apps", "shortcut", "shortcuts-reset", "text"] {
            assert!(names.iter().any(|x| x == n), "local:{n} is on no page");
        }
        let controls = |r: &&Row| !r.buttons.is_empty() || slider_of(&r.raw).is_some();
        assert_eq!(on.iter().filter(controls).count(), rows.iter().filter(controls).count(), "every row with a control is on a page, once");
        assert_eq!(device_section("appearance", &rows).iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Language", "Theme", "Text size", "Font", "3D viewer background", "Custom background colour", "Sidebar width", "Sidebar", "Opens on"],
            "Appearance and Layout, without their headings, in their order");
        assert_eq!(device_section("notifications", &rows).iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Desktop notifications"]);
        assert_eq!(device_section("privacy", &rows).iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Streamer mode", "Turn on while the screen is shared", "Turn on while a recording app runs"]);
        assert!(device_section("cad", &rows).is_empty() && device_section("@device", &rows).is_empty());
        // Keyboard: Shortcuts to defaults first, then what can be changed, the fixed keys last and folded
        let k = device_section("@keys", &rows);
        let first_fixed = k.iter().position(is_adv).expect("fixed keys");
        assert!(k[..first_fixed].iter().all(|r| !r.buttons.is_empty()) && k[first_fixed..].iter().all(|r| is_adv(r) && r.buttons.is_empty()), "the changeable keys, then the fixed ones");
        assert_eq!(k[0].label, "Shortcuts to defaults"); assert!(k[1..first_fixed].iter().all(|r| r.buttons[0].action == "local:shortcut"), "then each key in its order");
        // Text size: its Reset lit only when there is a size to reset
        let reset = |pct: u32| { let l = crate::local::Local { text_pct: pct, ..Default::default() }; device_section("appearance", &device_rows(&l)).into_iter().find(|r| r.label == "Text size").unwrap().buttons[0].on };
        assert!(!reset(100) && reset(110));
        assert_eq!(k.len(), crate::keys::TABLE.len() + crate::keys::MISSING.len() + 1);
    }
    #[test] fn the_gear_reopens_the_page_settings_was_left_on() {
        assert_eq!(reopen(""), ("appearance".to_string(), false, false, false), "the first open of a run");
        assert_eq!(reopen("cad"), ("cad".to_string(), false, false, false));
        assert_eq!(reopen("@keys"), ("appearance".to_string(), true, false, false));
        assert_eq!(reopen("@hubs"), ("appearance".to_string(), false, true, false));
        assert_eq!(reopen("@accounts"), ("appearance".to_string(), false, false, true));
    }
    #[test] fn advanced_rows_fold_and_a_search_opens_the_fold() {
        let rows = parse(&serde_json::json!({"rows": [{"label": "Desktop", "note": "n", "buttons": [{"label": "On", "action": "x", "value": "", "on": true}]},
            {"label": "Webhook", "note": "Posts what Notifications allows.", "adv": true, "fields": [{"name": "whurl", "hint": "", "value": ""}]}, {"label": "Secret file", "adv": true}]}));
        assert!(!is_adv(&rows[0]) && is_adv(&rows[1]) && is_adv(&rows[2]), "a hub row's adv is kept");
        assert!(is_heading(&rows[2]) && !is_heading(&rows[1]), "a label alone is a heading, not counted");
        assert!(!adv_open(&rows, "", false) && adv_open(&rows, "", true), "shut until opened");
        assert!(adv_open(&rows, "webhook", false) && adv_open(&rows, "notifications allows", false), "a folded row the search found opens it");
        assert!(!adv_open(&rows, "desktop", false), "a row above the fold leaves it shut");
    }
    #[test] fn the_page_shown_is_known_from_the_hubs_title() {
        let mut ui = SettingsUi::default(); ui.nav = vec![("about".into(), "About and updates".into()), ("cad".into(), "CAD".into())];
        assert_eq!(page_shown(&ui, "", false), "appearance", "nothing chosen yet: Appearance");
        assert_eq!(page_shown(&ui, "CAD", false), "cad", "a page the hub opened itself");
        ui.page = "cad".into(); assert_eq!(page_shown(&ui, "", false), "cad"); assert_eq!(page_shown(&ui, "CAD", true), "@keys");
        assert!(!hub_behind(&ui, "CAD") && hub_behind(&ui, "About and updates"), "Settings opened (or a nav press) on CAD while the hub still shows another page");
        ui.page.clear(); assert!(!hub_behind(&ui, "About and updates"), "nothing chosen here: the hub's page is the one");
        ui.nav.clear(); assert_eq!(page_shown(&ui, "About and updates", false), "about"); assert_eq!(page_shown(&ui, "Network and updates", false), "network", "an older hub");
        ui.page = "appearance".into(); assert!(hub_behind(&ui, "Notifications"), "the hub's pages not known yet: not taken for the one chosen");
    }
    #[test] fn every_page_has_an_icon_in_the_set_and_the_search_filters_titles_by_substring() {
        let set: Value = serde_json::from_str(include_str!("../../../../assets/icons.json")).unwrap();
        for p in ["appearance", "threads", "notifications", "timezone", "agents", "writing", "keys", "mcp", "voice", "rules", "usage", "git", "cad", "browser", "mouse", "ordering", "devices", "machines", "network", "setup", "privacy", "about", "developer", "@device", "@keys", "@hubs", "@accounts", "zzz"] { assert!(set.get(nav_icon(p)).is_some(), "{p}: {}", nav_icon(p)); }
        assert!(nav_matches("Appearance", "") && nav_matches("Appearance", " ear") && nav_matches("Appearance", "APP") && !nav_matches("Appearance", "zzz"));
    }
    #[test] fn a_box_and_its_buttons_share_a_line_while_the_box_keeps_140_px() {
        let set = chip_w("Set");
        let near = |a: Option<f32>, b: f32| a.map_or(false, |a| (a - b).abs() < 0.01);
        assert!(near(field_beside(430., &[set]), 430. - set - 4.), "the box is the field's room less the button and its gap");
        let two = [chip_w("Save"), chip_w("Use Jev")]; assert!(near(field_beside(430., &two), 430. - two[0] - two[1] - 8.));
        assert_eq!(field_beside(260., &[200.]), None, "under 140 px the button goes under the box");
        assert_eq!(field_beside(144. + 4. + 50., &[50.]), Some(144.)); assert_eq!(field_beside(139. + 4. + 50., &[50.]), None);
        assert_eq!(field_beside(430., &[]), None, "a box alone keeps its width");
        // the line the box and its buttons make is no wider than the room row_fit planned for the box
        for avail in [400., 520., 700., 900.] { let w = (avail - LABEL_MIN - GAP).clamp(260., CONTROLS_MAX).min(avail); if let Some(b) = field_beside(w, &[set]) { assert!(b + 4. + set <= w + 0.01); } }
        // a copy press keeps the room of the wider word, and its icon
        let copy = Btn { label: "Copy".into(), action: "copy".into(), value: "x".into(), on: false };
        assert!(btn_w(&copy) >= chip_w(&trs("Copied")) + super::super::copyable::CHIP_ICON && btn_w(&copy) > chip_w("Copy"));
    }
    #[test] fn a_row_keeps_what_the_hub_says_was_changed_and_how_to_reset_it_and_its_intro() {
        let r = parse(&j(r#"{"rows":[{"label":"Mech tab","note":"","changed":true,"reset":{"action":"setting","value":"viewer.mech=solidcraft"},"buttons":[{"label":"SolidCraft","action":"setting","value":"viewer.mech=solidcraft","on":false}]},
            {"label":"Notify me for","note":"Phone and desktop alerts.","intro":true,"buttons":[]},{"label":"Old","note":"","buttons":[]}]}"#));
        assert_eq!((r[0].raw["changed"].as_bool(), r[0].raw["reset"]["value"].as_str()), (Some(true), Some("viewer.mech=solidcraft")));
        assert!(r[1].raw["intro"] == true && is_heading(&r[1]), "an intro is a heading, not a setting the fold counts");
        assert!(r[2].raw.is_null(), "an older hub's row: nothing more kept");
        // Enter in a box presses its row's one button
        let k = parse(&j(r#"{"rows":[{"label":"KiCad","note":"","fields":[{"name":"kpath","hint":"","value":""}],"buttons":[{"label":"Set","action":"kicad-path","value":"","on":true}]}]}"#));
        assert_eq!(enter_press(&k, "kpath").map(|b| b.action), Some("kicad-path".into())); assert!(enter_press(&k, "fpath").is_none());
    }
    #[test] fn this_devices_changed_settings_are_marked_and_reset_to_a_fresh_install() {
        let fresh = device_rows(&crate::local::Local::default());
        let mut same = fresh.clone(); mark_device(&mut same, &fresh); assert!(same.iter().all(|r| r.raw["changed"].is_null()), "a fresh install has changed nothing");
        let mut l = crate::local::Local::default(); l.font = crate::local::FONTS[1].0.into(); l.text_pct = 150;
        let first = crate::keys::TABLE.iter().find(|x| x.kind != crate::keys::Kind::Fixed).unwrap(); l.shortcuts.insert(first.id.to_string(), "ctrl-alt-f9".into());
        let mut rows = device_rows(&l); mark_device(&mut rows, &fresh);
        let marked: Vec<&Row> = rows.iter().filter(|r| r.raw["changed"] == true).collect();
        assert_eq!(marked.len(), 2, "{:?}", marked.iter().map(|r| &r.label).collect::<Vec<_>>());
        let font = marked.iter().find(|r| r.label == "Font").unwrap(); assert_eq!((font.raw["reset"]["action"].as_str(), font.raw["reset"]["value"].as_str()), (Some("local:font"), Some(crate::local::FONTS[0].0)));
        let key = marked.iter().find(|r| r.buttons.first().map_or(false, |b| b.action == "local:shortcut")).unwrap();
        assert_eq!((key.raw["reset"]["action"].as_str(), key.raw["reset"]["value"].as_str()), (Some("local:shortcut-reset"), Some(first.id)), "a changed shortcut's own Reset");
        assert!(l.set("shortcut-reset", first.id)); let mut back = device_rows(&l); mark_device(&mut back, &fresh); assert_eq!(back.iter().filter(|r| r.raw["changed"] == true).count(), 1, "the reset shortcut is its default again");
        assert!(rows.iter().find(|r| r.label == "Text size").unwrap().raw["changed"].is_null(), "the text size has its own Reset");
    }
    #[test] fn short_boxes_say_their_width_and_share_a_line_with_the_buttons() {
        let quiet = j(r#"{"fields":[{"name":"qfrom","w":6},{"name":"qto","w":6}]}"#);
        assert_eq!(short_tail(&quiet, 2), vec![short_w(6); 2]); assert_eq!(short_w(6), 16. + 7.6 * 6. * crate::scale::s());
        let save = chip_w("Save"); assert_eq!(short_line_w(&short_tail(&quiet, 2), &[save]), 2. * short_w(6) + save + 8., "4 px between each");
        assert!(short_line_w(&short_tail(&quiet, 2), &[save]) < 260., "narrower than a long box");
        let pair = j(r#"{"fields":[{"name":"maddr"},{"name":"mcode","w":10}]}"#); assert_eq!(short_tail(&pair, 2), vec![short_w(10)], "the long box keeps its line above");
        let none = j(r#"{"fields":[{"name":"qfrom","w":6},{"name":"kname"}]}"#); assert!(short_tail(&none, 2).is_empty(), "the last box is long: as before");
        assert!(short_tail(&j(r#"{"fields":[{"name":"kname"}]}"#), 1).is_empty()); assert!(short_tail(&j("{}"), 0).is_empty());
        let rows = parse(&j(r#"{"rows":[{"label":"Quiet from and until","fields":[{"name":"qfrom","w":6},{"name":"qto","w":6}],"buttons":[{"label":"Save","action":"notify-quiet","value":"save"}]}]}"#));
        assert_eq!(short_tail(&rows[0].raw, rows[0].fields.len()).len(), 2, "the row the hub sent is kept for its widths");
    }
    #[test] fn a_shortcut_waiting_for_its_keys_says_so_and_a_refused_chord_says_why_on_its_row() {
        let l = crate::local::Local::default(); let fresh = device_rows(&l);
        let mut rows = fresh.clone(); mark_device(&mut rows, &fresh);
        let why = ("models".to_string(), "Ctrl+K is already used by Commands: jump to a thread or run a command.".to_string());
        show_recording(&mut rows, Some("models"), Some(&why));
        let at = |rows: &[Row], id: &str| rows.iter().position(|r| r.buttons.first().map_or(false, |b| b.action == "local:shortcut" && b.value == id)).unwrap();
        let (m, f) = (&rows[at(&rows, "models")], &rows[at(&rows, "find")]);
        assert_eq!((m.buttons[0].label.as_str(), m.buttons[0].on, m.note.as_str(), m.tone.as_str()), ("Type the keys…", true, why.1.as_str(), "warn"));
        assert_eq!((f.buttons[0].on, f.tone.as_str()), (false, ""), "the other shortcuts as they were"); assert_eq!(f.note, tf("gpui.keys.now", &[("key", crate::keys::hint(&l, "find").into())]));
        assert!(rows[at(&rows, "models")].raw["changed"].is_null(), "waiting is no change");
        assert_eq!(tf("gpui.keys.now", &[("key", "Ctrl+M".into())]), "Now Ctrl+M.");
        let mut ui = SettingsUi { rec: Some("models".into()), rec_note: Some(why), ..Default::default() }; assert!(end_recording(&mut ui)); assert!(ui.rec.is_none() && ui.rec_note.is_none()); assert!(!end_recording(&mut ui));
    }
    #[test] fn a_group_is_named_by_its_own_message_and_not_the_verb() {
        crate::i18n::with_lang("es", || {
            assert_eq!(group_name("Connect"), "Conexión"); assert_eq!(group_name("System"), "Sistema"); assert_eq!(group_name("General"), "General");
            assert_eq!(trs("Connect"), "Conectar", "a Connect button elsewhere keeps its verb");
            assert_eq!(group_name("This device"), trs("This device"));
        });
    }
}
