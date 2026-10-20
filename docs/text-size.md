# Text size

`ui.text_scale` is a persisted hub setting for the desktop native and web
clients, including a mobile browser. The allowed string values are `85`, `100`,
`115`, `130`, and `150`; missing or invalid values render at `100`.
Settings → Appearance owns the control. Native phone apps continue to use their
operating system's accessibility text sizing. No physical-device validation is
claimed by this change.

The desktop loads fonts through FreeType at the requested pixel size. Wrapping,
fitting, caret positioning and selection use those same glyph advances. Layout
and hit rectangles remain in physical window coordinates; the setting never
changes screen DPI, devicePixelRatio or the CAD camera. Text rows in the timeline,
composer and wrapped cards grow with the text. A font change invalidates entry,
live-prefix, row, bot wrapping and raster caches. The five font sets are retained
so repeatedly changing size does not keep opening FreeType faces.

The web applies a bounded CSS font multiplier to the same persisted setting.
Browser layout owns reflow and hit regions; the viewport and CAD canvas keep
their existing pixel dimensions. Native terminal/grid content retains its own
cell geometry; this setting is for interface and conversation text.
