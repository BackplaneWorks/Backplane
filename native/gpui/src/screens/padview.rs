//! The drawing pad on screen, over the whole window: the composer's pen button opens it. The paper is `pad.rs`'s picture as one image, redrawn
//! only when the paper changed. Its key context, DrawPad, has the keyboard while it is open, so Ctrl+Z and Delete are the pad's and never the composer's
//! (keys.rs also mutes every other shortcut there). Attach hands the picture to the same upload as a pasted image; Esc, Cancel or a press outside drops it.
use super::{composer::{Composer, ComposerEvent}, pad::{self, Part, Tool}};
use crate::{keys, script::{Ctl, CtlEntry}, theme, ui::with_rect, Backplane};
use gpui::{actions, prelude::*, *};
use std::{cell::Cell, rc::Rc, sync::Arc};

actions!(pad, [Undo, Delete]);

/// What a pad needs that the model does not: its keyboard, the length box and the image on screen.
pub struct PadView {
    pub pad: pad::Pad,
    focus: FocusHandle,
    length: Entity<Composer>,
    _submit: Subscription,
    paper: Rc<Cell<Bounds<Pixels>>>,
    image: Option<Arc<RenderImage>>,
    retired: Vec<Arc<RenderImage>>,
}

/// The paper's size on screen: as large as the window lets it be, up to one screen pixel per paper pixel.
pub fn paper_size(window_w: f32, window_h: f32) -> (f32, f32) {
    let w = (pad::W as f32).min(window_w - 58.).min((window_h - 190.) * pad::W as f32 / pad::H as f32).max(160.);
    (w, w * pad::H as f32 / pad::W as f32)
}
/// A window position in paper pixels.
fn on_paper(b: Bounds<Pixels>, p: Point<Pixels>) -> (f32, f32) {
    let k = pad::W as f32 / f32::from(b.size.width).max(1.);
    (f32::from(p.x - b.origin.x) * k, f32::from(p.y - b.origin.y) * k)
}
fn cursor_for(tool: Tool, over: Option<Part>) -> CursorStyle {
    match (tool, over) { (Tool::Boxes, Some(Part::L | Part::R)) => CursorStyle::ResizeLeftRight, (Tool::Boxes, Some(Part::T | Part::B)) => CursorStyle::ResizeUpDown, _ => CursorStyle::Crosshair }
}
/// The words under the title, for the tool in hand.
fn hint(tool: Tool, undo: &str, delete: &str) -> String {
    match tool {
        Tool::Pen => format!("Drag to draw · {delete} clears · {undo} undoes"),
        Tool::Boxes => format!("Drag to draw a box · drag its middle to move it, a side to resize · click a side to type its length · {delete} removes · {undo} undoes"),
        Tool::Eraser => format!("Drag over what to erase · {undo} undoes"),
    }
}

impl Backplane {
    /// The composer's pen button. A pad that could not be attached is not opened, so nothing is drawn only to be lost.
    pub(crate) fn pad_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screens.pad.is_some() { return; }
        if let Err(e) = self.screens.attacher.ready(&self.selected) { self.error = e; cx.notify(); return; }
        let length = cx.new(|cx| { let mut c = Composer::with_context(cx, "PadLength"); c.set_placeholder("length, e.g. 40 mm"); c });
        let submit = cx.subscribe_in(&length, window, |this, _, ev: &ComposerEvent, window, cx| if matches!(ev, ComposerEvent::Submit) { this.pad_commit_length(window, cx); });
        let focus = cx.focus_handle();
        focus.focus(window);
        self.screens.pad = Some(PadView { pad: pad::Pad::default(), focus, length, _submit: submit, paper: Rc::new(Cell::new(Bounds::default())), image: None, retired: vec![] });
        cx.notify();
    }
    /// True when a pad was open. The keyboard goes back to the message box.
    pub(crate) fn pad_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(mut p) = self.screens.pad.take() else { return false };
        for i in p.retired.drain(..).chain(p.image.take()) { let _ = window.drop_image(i); }
        self.want_focus = true; cx.notify(); true
    }
    /// Esc: the length box first, then the pad.
    pub(crate) fn pad_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match &mut self.screens.pad {
            Some(p) if p.pad.editing.is_some() => { p.pad.cancel_length(); p.focus.focus(window); cx.notify(); true }
            _ => self.pad_close(window, cx),
        }
    }
    fn pad_commit_length(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = &mut self.screens.pad else { return };
        if p.pad.editing.is_none() { return; }
        let text = p.length.read(cx).content.to_string();
        p.pad.commit_length(&text); p.focus.focus(window); cx.notify();
    }
    fn pad_attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pad_commit_length(window, cx);
        let Some(rgba) = self.screens.pad.as_ref().filter(|p| !p.pad.is_empty()).map(|p| p.pad.paint(false)) else { return };
        self.pad_close(window, cx);
        self.attach_event(&ComposerEvent::Drawing(rgba), cx);
    }
    fn pad_with(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut PadView)) { if let Some(p) = &mut self.screens.pad { f(p); cx.notify(); } }
    fn pad_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = &mut self.screens.pad else { return };
        if p.pad.up() {
            let text = p.pad.length_text();
            p.length.update(cx, |l, cx| { l.set(text, cx); l.select_everything(cx); });
            p.length.focus_handle(cx).focus(window);
        }
        cx.notify();
    }
}

fn tool_button(cx: &mut Context<Backplane>, light: bool, id: &'static str, rect: &'static str, label: &'static str, tool: Tool, on: bool) -> impl IntoElement {
    let c = |n: &str| theme::color(light, n);
    with_rect(div().id(id), || rect.to_string()).h(px(26.)).px_3().flex().items_center().border_1().text_size(px(12.)).cursor_pointer()
        .border_color(if on { c("accent") } else { c("line") }).bg(if on { c("accent") } else { c("panel") }).text_color(if on { c("on-accent") } else { c("text") }).child(label)
        .on_click(cx.listener(move |this, _, window, cx| { this.pad_commit_length(window, cx); this.pad_with(cx, |p| p.pad.set_tool(tool)); }))
}

/// The pad over the window, when one is open. The controls go on the harness's inventory.
pub fn view(cx: &mut Context<Backplane>, window: &mut Window, light: bool, l: &crate::local::Local, slot: &mut Option<PadView>, ctl: &mut Vec<CtlEntry>) -> Option<AnyElement> {
    let p = slot.as_mut()?;
    let c = |n: &str| theme::color(light, n);
    for old in p.retired.drain(..) { let _ = window.drop_image(old); } // a painted RenderImage stays in the atlas until it is dropped
    if p.image.is_none() && crate::art::rects_on() { window.request_animation_frame(); } // one frame more for the harness, which hears a frame's controls on the next; an idle pad draws none of its own
    if p.pad.dirty || p.image.is_none() {
        p.pad.dirty = false;
        let mut px = p.pad.paint(true);
        for q in px.chunks_exact_mut(4) { q.swap(0, 2); } // gpui takes BGRA in this container
        if let Some(i) = crate::viewers::solid::to_image(px, pad::W, pad::H) { p.retired.extend(p.image.replace(i)); }
    }
    let view = window.viewport_size();
    let (pw, ph) = paper_size(f32::from(view.width), f32::from(view.height));
    let (tool, picked_color, empty) = (p.pad.tool, p.pad.next % pad::COLORS.len(), p.pad.is_empty());
    for (id, label) in [("pad:paper", "Paper"), ("pad:pen", "Pen"), ("pad:box", "Box"), ("pad:eraser", "Eraser"), ("pad:cancel", "Cancel"), ("pad:attach", "Attach")] { ctl.push(CtlEntry { id: id.into(), label: label.into(), ctl: Ctl::Pad }); }
    let mut swatches = div().flex().items_center().gap_1();
    for (i, color) in pad::COLORS.iter().enumerate() {
        ctl.push(CtlEntry { id: format!("pad:color:{i}"), label: format!("Colour {}", i + 1), ctl: Ctl::Pad });
        swatches = swatches.child(with_rect(div().id(("pad-color", i)), || format!("pad:color:{i}")).size(px(22.)).border_2().border_color(if i == picked_color { c("text") } else { c("line") }).bg(rgb(*color)).cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| { this.pad_commit_length(window, cx); this.pad_with(cx, |p| p.pad.set_color(i)); })));
    }
    let paper_at = p.paper.clone();
    let on_down = p.paper.clone();
    let on_move = p.paper.clone();
    let mut paper = with_rect(div().id("pad-paper"), || "pad:paper".to_string()).relative().flex_shrink_0().w(px(pw)).h(px(ph)).bg(gpui::white()).cursor(cursor_for(tool, p.pad.over()))
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            this.pad_commit_length(window, cx);
            let (x, y) = on_paper(on_down.get(), e.position);
            this.pad_with(cx, |p| p.pad.down(x, y)); cx.stop_propagation();
        }))
        .child(canvas(move |b, _, _| paper_at.set(b), |_, _, _, _| {}).absolute().size_full())
        .children(p.image.clone().map(|i| img(i).size_full().object_fit(ObjectFit::Fill)));
    if let Some((x, y)) = p.pad.length_at() {
        let k = pw / pad::W as f32;
        ctl.push(CtlEntry { id: "pad:length".into(), label: "Length".into(), ctl: Ctl::Pad });
        paper = paper.child(with_rect(div().id("pad-length"), || "pad:length".to_string()).absolute().left(px((x * k - 70.).clamp(0., pw - 140.))).top(px((y * k - 14.).clamp(0., ph - 28.))).w(px(140.)).px_2().py_1().border_1().border_color(c("accent")).bg(c("view")).text_size(px(13.)).text_color(c("text"))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(p.length.clone()));
    }
    let hint = hint(tool, &keys::hint(l, "pad-undo"), &keys::hint(l, "pad-delete"));
    let foot_button = |id: &'static str, rect: &'static str, label: &'static str| with_rect(div().id(id), || rect.to_string()).h(px(26.)).px_3().flex().items_center().text_size(px(12.)).child(label);
    Some(div().id("pad-back").key_context("DrawPad").track_focus(&p.focus).absolute().inset_0().occlude().flex().items_center().justify_center().p_4().bg(hsla(0., 0., 0., 0.6))
        .on_action(cx.listener(|this, _: &Undo, _, cx| this.pad_with(cx, |p| p.pad.undo())))
        .on_action(cx.listener(|this, _: &Delete, _, cx| this.pad_with(cx, |p| p.pad.delete())))
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { this.pad_close(window, cx); }))
        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, window, cx| {
            let b = on_move.get(); let (x, y) = on_paper(b, e.position);
            let Some(p) = &mut this.screens.pad else { return };
            if p.pad.dragging() && e.pressed_button != Some(MouseButton::Left) { return this.pad_up(window, cx); } // the button came up where no element heard it
            let before = p.pad.dirty;
            if p.pad.dragging() || b.contains(&e.position) { p.pad.moved(x, y); } else { p.pad.left(); }
            if p.pad.dirty && !before { cx.notify(); }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|this, _, window, cx| this.pad_up(window, cx)))
        .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, window, cx| this.pad_up(window, cx)))
        .child(div().id("pad-box").w(px(pw + 26.)).flex_shrink_0().flex().flex_col().gap_2().p_3().bg(c("panel")).border_1().border_color(c("line")).text_color(c("text"))
            // a press anywhere on the box is not a press outside it, and ends a length being typed
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { this.pad_commit_length(window, cx); cx.stop_propagation(); }))
            .child(div().flex().items_baseline().gap_3().child(div().font_weight(FontWeight::BOLD).child("Draw")).child(div().flex_1().min_w_0().text_size(px(12.)).text_color(c("dim")).child(hint)))
            .child(div().flex().items_center().gap_2()
                .child(tool_button(cx, light, "pad-pen", "pad:pen", "Pen", Tool::Pen, tool == Tool::Pen))
                .child(tool_button(cx, light, "pad-box-tool", "pad:box", "Box", Tool::Boxes, tool == Tool::Boxes))
                .child(tool_button(cx, light, "pad-eraser", "pad:eraser", "Eraser", Tool::Eraser, tool == Tool::Eraser))
                .child(div().flex_1()).child(swatches))
            .child(paper)
            .child(div().flex().items_center().justify_end().gap_2()
                .child(foot_button("pad-cancel", "pad:cancel", "Cancel").border_1().border_color(c("line")).cursor_pointer().on_click(cx.listener(|this, _, window, cx| { this.pad_close(window, cx); })))
                .child(foot_button("pad-attach", "pad:attach", "Attach").bg(c("accent")).text_color(c("on-accent")).when(empty, |d| d.opacity(0.45)).when(!empty, |d| d.cursor_pointer()).on_click(cx.listener(|this, _, window, cx| this.pad_attach(window, cx))))))
        .into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test; // gpui::* also exports a `test` macro
    #[test] fn the_paper_fits_the_window_and_keeps_its_shape() {
        assert_eq!(paper_size(1400., 900.), (960., 600.));
        let (w, h) = paper_size(700., 900.); assert_eq!(w, 642.); assert!((w / h - 1.6).abs() < 0.001);
        let (w, h) = paper_size(1400., 500.); assert!(h <= 310. && (w / h - 1.6).abs() < 0.001);
    }
    #[test] fn a_window_position_lands_on_the_paper_at_any_size() {
        let b = Bounds::new(point(px(100.), px(50.)), size(px(480.), px(300.)));
        assert_eq!(on_paper(b, point(px(340.), px(200.))), (480., 300.));
    }
    #[test] fn the_hint_names_the_keys_it_was_given() {
        assert!(hint(Tool::Boxes, "⌘Z", "Delete").contains("⌘Z undoes")); assert!(hint(Tool::Pen, "Ctrl+Z", "Delete").contains("Delete clears"));
    }
}
