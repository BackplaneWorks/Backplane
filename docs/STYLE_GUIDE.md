# Backplane style guide: posts and graphics

How a Backplane graphic looks and reads: posts, carousels and release images.

> **Typeface: use Berkeley Mono.** Every graphic is set in Berkeley Mono, headlines to footnotes. The font is
> licensed: install it on the machine that renders, never commit its files.

## The rule: images lead

A graphic is a picture first. Each image is mostly real visuals (KiCad renders, screenshots, the agent's own code shown
as code, tick strips, area squares) with at most a headline of a few words and one number. Nothing is written as a
paragraph on an image: no step lists, no captions, no explanations. A tag of two or three words over a picture
("before", "Backplane") and the small footnote are the only other text.

## Formats

| Use | Size | Notes |
|---|---|---|
| X post, one image | 1600×900 (16:9) | Shown whole in the timeline. |
| X post, 2 to 4 images, carousel | 1080×1080 each | X shows two side by side cropped near square: squares lose nothing. |

- Safe margin: 64 px left and right, 56 to 60 px top, 24 to 30 px bottom (the footnote sits in it). Nothing important
  closer to an edge than that.
- One idea per image. A carousel numbers its pages top right (`2/5`).

## Colour

The app's dark theme (`assets/web/style.css`, `:root`). Use these values and no others.

| Role | Hex | Where |
|---|---|---|
| Background | `#0d0f12` | The whole image. Never a gradient, never a photo. |
| Panel | `#13161b` | Cards, columns, stat blocks. |
| Raise | `#1a1e25` | The "before" bar and square fill. |
| Rule | `#353c49` | The "before" bar's and square's 1 px border. |
| Line | `#262b34` | Dividers between rows, card and screenshot borders. |
| Code | `#0a0c0f` | Code boxes. |
| Text | `#d9dde4` | Headlines and numbers. |
| Dim | `#8a93a3` | Tags, labels, the "before" side of the one number. |
| Faint | `#5c6473` | Footnotes, page numbers. |
| Tick | `#6b7383` | The "before" tick strip. |
| Accent | `#5fb3ff` | Backplane: the "after" bar, square and ticks, its numbers, one highlighted word in a headline, its picture's border. |
| OK green | `#58c28a` | A gain beside an "after" number (`−NN%`, `N× faster`), passes (`N/N`). Text only, never a fill. |
| Bad red | `#f06a6a` | A failed run, only when the image is about failures. Use sparingly. |
| Mark | `#e6e9de` | The logo mark (`assets/icon/mark.svg`), never recoloured. |

KiCad's own colours (its green board, red copper, blue back layer, its schematic paper) appear only inside KiCad
screenshots and renders, as KiCad draws them: never recolour a screenshot, never pick a brand colour out of one, never
paint our own shapes in KiCad's board green.

## Type

Berkeley Mono throughout. Weights: 700 for headlines and numbers, 400 for everything else.

| Text | Size (1080 square) |
|---|---|
| Headline | 44 to 54 px, at most two lines, one word or number in accent |
| The one number | 30 px |
| Bar value | 20 to 24 px |
| Tags and labels | 15 to 20 px |
| Code | 15 px |
| Footnote | 13 to 14 px, faint |

## Layout

- **Logo** top left: the mark at 40 to 56 px, then "Backplane" in 700 at 26 to 34 px, optionally "works with KiCad" in
  dim. Nothing else in that row but a carousel's page number at the right.
- **Headline** under the logo row, the one number under it, the pictures below, the footnote pinned to the bottom margin.
- **Grid**: one column, or two equal columns with a 20 px gap (before | after). A picture: 1 px line border, 6 px
  radius, a soft shadow (`0 20px 60px rgba(0,0,0,.55)`); the Backplane picture's border is accent. Its tag sits 8 px
  above it.
- **The one number**: under the headline, 30 px, dim, `before → after` with the after in accent (`N calls → M`).
- **Tick strip** (tool calls): one tick per tool call, 3 px wide with a 2 px gap, 30 to 60 px tall; the "before" strip
  in tick grey, the Backplane strip in accent, one above the other with a two-word label left of each. The number of
  ticks is the measured count, never rounded.
- **Area squares** (tokens, cost): two squares whose areas are the two values, bottoms aligned, before in raise with a
  rule border, after in accent.
- **Before/after bar** (one metric, when a square won't do): label left, two bars stacked right of it. Before: raise
  fill with a rule border; after: solid accent. Bars scale to the larger value at 76 to 80% of the track so the value
  fits after it; the after value in accent, its gain in OK green.
- **Code as an image**: the agent's own code, verbatim from its trace, 5 to 8 lines, code fill, line border, 6 px
  radius, keywords in `#c792ea`, names in accent, numbers in `#f78c6c`. The footnote says whose code it is.

## Imagery

- KiCad is shown clearly: a real screenshot of KiCad inside Backplane (the board editor with its toolbars), or a real
  render of a real run (KiCad's own SVG export of the board or sheet). Large: full width of the content area, or half
  of it for a before/after pair. Crop, don't scale a capture up past its size.
- No KiCad logo. The Linux Foundation owns KiCad's marks and its policy needs written permission to use the logo in
  promotion; the name in a true sentence ("works with KiCad") is fine. Every image showing KiCad carries:
  "KiCad is a trademark of The Linux Foundation; Backplane is not affiliated with or endorsed by the KiCad project."
- Show results as they came out: no retouching a render to hide a flaw.
- No stock photos, no AI-generated pictures, no mockups of screens that do not exist. Only screenshots and renders of
  the real thing.

## Copy

- A headline of a few words and one number per image (see the rule above). No subheads on carousels, no hashtags, no
  emoji, no exclamation marks.
- Only measured numbers, each traceable to its source (a results file, a published benchmark table): the graphic's
  source names it in a comment. Totals as measured (sums of per-task medians); don't quote single-task ratios to three
  digits from two runs.
- A footnote names the model, the task set and the run count, and says what was checked.
- Compare against the right baseline, named plainly ("an agent editing the KiCad files directly", "through KiCad's IPC
  API"). Never relabel one baseline as another; never mix models in one comparison without saying so.
- Code, call counts and pictures shown as the agent's come from its trace and its files, never made for the picture;
  a picture made by replaying the agent's own calls on a fresh copy says so in the source.
- Don't overclaim: one benchmark is one benchmark; a feature that has not landed is not in a post.
- The manufacturer is "the fab", its catalogue "the fab's stock": never a manufacturer's name.

## Files

- Every graphic is code: one HTML (or SVG) source per post, its pages as `#1`, `#2`…, its data in one object at the
  top of its script, and a comment naming where each number comes from. Render it to PNG with a headless browser at the
  format's size.
- The PNGs sit beside their source, with the assets they use (the mark, screenshots, renders).
- Nothing is posted by an agent; the person posts.
