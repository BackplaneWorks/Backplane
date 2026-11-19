# Mouse and trackpad controls in the built-in viewers

One table decides what every press, drag, scroll and pinch means in the desktop client's viewers: `native/host/src/mouse.rs`. The hub draws
Settings, Mouse and trackpad from it, and the client includes the same file by path (`native/gpui/src/main.rs`), so the page and the viewers cannot
disagree. The viewers (`screens/plotview.rs`, `viewers/solid.rs`) name no button of their own; `mousecfg.rs` has a guard test that greps for one.
"The person panned or zoomed" is decided there and nowhere else: `Scheme::drag` and `Scheme::scroll`, then each viewer's single
`person_moved` / `touched` step (the hook for a follow mode that pauses when the person moves the view).

Two viewer kinds, each with its own scheme: the flat views (board and schematic plots) and the 3D views (a board's 3D model, STEP parts).
The phone apps are touch-only and have no mouse: they are out of scope.

## Settings

`mouse.2d`, `mouse.3d`: a preset name, then what differs from it (`kicad;middle=zoom;wheel=pan`). Empty is the KiCad preset. `mouse.invert`: `on`
reverses the zoom direction of the wheel and the pinch. The hub puts all three in its picture as `mouse` (`present/screen.rs`).
Every input has exactly one action, so two actions never fight for an input; picking an action for an input replaces what it did, and the page
names an action nothing reaches ("Nothing is set to zoom").

Inputs: left, middle, right drag; Shift, Ctrl (Cmd on a Mac) or Alt with the left drag; the wheel; the wheel with Shift, Ctrl or Alt.
Actions for a drag: nothing, pan, zoom (3D adds rotate, roll). For a wheel: nothing, zoom, pan, pan sideways, pan up and down.
Select is a click without moving, with no key held, on the button chosen. Two-finger scroll (a trackpad's pixel deltas) either pans, with Ctrl
(the pinch the system sends as Ctrl+scroll) zooming, or is the wheel like any other. A pinch (macOS: gpui's `PinchEvent`) zooms or is off.

## The KiCad preset, and where it comes from

From KiCad 10's source (Backplane KiCad, `common/settings/common_settings.cpp`, `common/gal/hidpi_gl_3d_canvas.cpp`, `3d-viewer/3d_canvas/eda_3d_canvas.cpp`):

| input | KiCad's default | source |
|---|---|---|
| wheel | zoom | `input.scroll_modifier_zoom` = 0 |
| Ctrl + wheel | pan left and right | `input.scroll_modifier_pan_h` = `WXK_CONTROL` |
| Shift + wheel | pan up and down | `input.scroll_modifier_pan_v` = `WXK_SHIFT` |
| middle drag | pan | `input.mouse_middle` = `PAN` |
| right drag | pan | `input.mouse_right` = `PAN` |
| left button | select (its drag moves a selection or draws a box, which a viewer has no use for: nothing here) | `input.mouse_left` = `DRAG_SELECTED` |
| 3D: left drag | rotate (trackball) | `HIDPI_GL_3D_CANVAS::OnMouseMoveCamera` |
| 3D: middle, right drag | pan | the same, when `mouse_middle`/`mouse_right` are `PAN` |
| 3D: wheel, Ctrl, Shift + wheel | zoom, pan sideways, pan up and down | `OnMouseWheelCamera` with `scroll_modifier_zoom` = 0 |

KiCad has no roll drag in 3D, so the preset has none; Shift, Ctrl and Alt with the left drag do nothing in it. Where a trackpad's two-finger scroll
meets a plot, it pans (what a trackpad user expects, and what the viewers always did), with Ctrl it is the pinch and zooms; in the 3D view it is
the wheel. Backplane's own double-click on the selecting button fits the view, as before.

## The Legacy preset

What the viewers did before the presets, captured from the code at the commit that introduced them:

- Flat: left drag pans; a tap (press and release within 4 px) selects; wheel and Ctrl + wheel zoom; a trackpad's two-finger scroll pans, Ctrl + scroll (pinch) zooms; pinch zooms.
- 3D: left drag rotates; Alt + left drag rolls; Shift or Ctrl + left drag, right drag and middle drag pan; wheel and Ctrl + wheel zoom (Ctrl faster); a trackpad's scroll zooms; pinch zooms; a click that does not move selects.

One difference remains in Legacy: with several of Alt, Shift and Ctrl held at once, Legacy's order was Alt, then Shift or Ctrl, which `Mods::drag` keeps; a wheel
takes KiCad's order, Shift, Ctrl, Alt.

## Existing installs

KiCad is the default for everyone, including those who had the old behaviour; the release note says so, and Legacy is one click away in Settings, Mouse and trackpad.
