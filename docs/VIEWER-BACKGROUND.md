# 3D viewer background and light

One setting, shared by the Mech viewer (STEP/GLB parts) and the 3D board viewer, kept per device (never sent to the hub), on the desktop and on both phones.

## The setting
`viewer_bg`: `auto` (default) | `light` | `dark` | `custom`; `viewer_bg_custom`: `#rrggbb` (sRGB), used when `custom`.
`auto` follows the app's theme (the phone's light/dark, the desktop's resolved theme) and changes live with it. The viewer redraws when either changes.

## The background: a vertical gradient, top to bottom, sRGB bytes
| resolved | top | bottom |
| --- | --- | --- |
| light | `#f5f6f8` | `#e1e4e9` |
| dark | `#4d535e` | `#353a43` |
| custom c | c | each channel of c × 0.88, rounded |

`auto` resolves to light or dark by the theme. The gradient is mixed in linear light (decode each end with the sRGB transfer, mix by the pixel's y over the view's height, 0 at the top).

## The light (raised a bit; the material model and colour decode are untouched)
In every renderer (desktop CPU `raster.rs`, desktop GPU `solid.wgsl`, iOS `PlotShaders.swift`, Android `PlotView.kt`):
- ambient: `(rough >= 0.8 ? 0.74 : 0.27)` becomes `(rough >= 0.8 ? 0.80 : 0.34)`, and `0.08 * max(dot(N, up), 0)` becomes `0.10 * ...`;
- the fill light's weight `0.65` becomes `0.85` (the direct `lit(... fill ...) * 0.65`; not the studio reflection colours);
- the studio sky's base `0.08` becomes `0.12`.
No contact shadow (the wire carries no ground).
