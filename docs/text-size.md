# Text size

A setting of this device (Settings → This device → Text size), kept in
`local-settings.json` as `text_pct`: 80% to 160% in steps of 5. It is a slider
(press anywhere on the track or drag the thumb; the value snaps to a step and
applies at once) with Reset beside it. The phone apps follow their operating
system's accessibility text size.

Every text size and line height in the GPUI client is written as its size at
100% and drawn through `scale::tpx` (`native/gpui/src/scale.rs`), so a label and
its line grow together and keep their 100% proportions. A box sized to hold a
line of text (a bar, a chip, a list row, a button, a label column, an input's
box) grows the same way. Width estimates made from character counts (settings
chips, the composer's model chip, the BOM and life-cycle columns, the header's
tools) use the scaled size too. Icons, rules, splitters and the layout's fixed
widths (the rail's 72 px, the sidebar's chosen width) keep their size; the
rail's labels grow until "Schematic" fills the rail and no further. The terminal
keeps its own grid. The command palette's list scrolls when its rows outgrow the
box and keeps the chosen row in view.

The window's root sets the size in force from `Local` at the start of every
frame. At 100% the client draws exactly what it drew before the setting existed.
