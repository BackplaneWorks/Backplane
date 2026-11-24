---
name: fab-spec
description: "Use when creating, measuring, or updating a part's .fab.json for fabrication."
---

# fab-spec

Each thing to be ordered has a `<name>.fab.json` beside it (a board's in its folder, a part's beside its STEP in mech/): {"kind": "pcb" | "flex" | "hdi" | "aluminum" | "copper_core" | "rogers" | "ptfe" | "3dp" | "cnc" | "sheetmetal", "name": "Main board", "source": "<its file, from the fab.json's folder>", "qty": 5, "options": {...}, "features": {...}}; add "order": false for one not to be ordered. Features are measured, never guessed, and kept current whenever the design changes, because Backplane shows the person a price from them: a board's `width_mm` and `height_mm` (its Edge.Cuts box) and `min_drill_mm` (its smallest plated hole, usually a via); a solid's `volume_cm3`, `area_cm2`, `size_x_mm`, `size_y_mm`, `size_z_mm` (FreeCAD: shape.Volume/1000, shape.Area/100, the tightest box, largest first); a sheet part's `flat_x_mm`, `flat_y_mm`, `cut_length_mm`, `holes`, `bends`, `countersinks` (its flat pattern; the last four default to 0). Boards come in kinds: pcb is a rigid FR-4 board; flex a flexible polyimide one; hdi one with microvias and build-up layers; aluminum and copper_core single-layer metal-core boards (LEDs, power); rogers (RO4350B) and ptfe two-layer high-frequency boards. Every board kind is measured and exported the same way; only pcb boards come assembled.

## Options

Omitted options take the first value.

- pcb: layers 1|2|4|6|8|10|12|14|16 (default 2), thickness_mm 1.6|0.4|0.6|0.8|1.0|1.2|2.0, color green|black|white|blue|red|yellow|purple, finish hasl|hasl_lead_free|enig|osp, copper_oz 1|2, castellated, gold_fingers

- flex: layers 2|1|4, thickness_mm (the total, which sets the substrate and copper; 1 layer: 0.07|0.11|0.12|0.14, 2 layers: 0.11|0.12|0.19|0.2|0.24, 4 layers: 0.2|0.25|0.3|0.35|0.45; left out, the first for its layers), coverlay yellow|black|white, stiffener none|polyimide_0.1|polyimide_0.15|polyimide_0.2|polyimide_0.225|polyimide_0.25|fr4_0.1|fr4_0.2|fr4_0.4|fr4_0.6|fr4_0.8|fr4_1.0|fr4_1.2|fr4_1.6|steel_0.1|steel_0.2|steel_0.3|tape_3m468|tape_tesa8854|tape_3m9077 (mm), emi_shield none|one_side|both_sides, gold_uin 1|2 (its ENIG gold), gold_fingers

- hdi: layers 4|6|8|10|12|14|16, thickness_mm 1.6 (0.6|0.8|1.0|1.2|1.4|1.6|1.8|2.0|2.5|3.0 as the layers allow), steps 1|2|3 (build-up steps: 1 at 4 layers, up to 2 at 6, up to 3 from 8), laminate np175f|s1000_2m (only s1000_2m from 8 layers), color as pcb, finish hasl|hasl_lead_free|enig|osp (only enig|osp from 6 layers), via_fill plugged|untented|epoxy_filled|copper_filled (only epoxy_filled from 6 layers), castellated, gold_fingers, edge_plating

- aluminum: thickness_mm 1.6|0.8|1.0|1.2, color white|black, finish hasl|hasl_lead_free

- copper_core: thickness_mm 1.6|1.0|1.2, color white|black (OSP)

- rogers: thickness_mm 0.51|0.76|1.52, color green|purple, finish enig|osp, gold_uin 1|2, via_fill tented|untented

- ptfe: thickness_mm 0.76|1.52, laminate dk2.94|dk3.0|dk2.65|dk2.55 (its dielectric constant), finish enig|osp, gold_uin 1|2, via_fill tented|untented

- 3dp: process mjf|sla|sls|fdm|slm, material (mjf: pa12|pa11|pa12_glass; sla: standard_resin|black_resin|tough_resin|high_temp_resin|clear_resin; sls: pa12|pa12_glass; fdm: abs|asa|pla|tpu|pa12_cf|pc_abs|peek; slm: stainless_316l|titanium), color, which the material decides (mjf pa12 and pa11: black|gray, pa12_glass: black|white; sla standard_resin: white, black_resin: black, tough_resin: green, high_temp_resin: yellow, clear_resin: translucent|transparent; sls pa12: black, pa12_glass: white; fdm abs and asa: black|white, pla: black|white|red|blue, tpu: black|white|gray|translucent|green, pa12_cf and pc_abs: black, peek: brown; slm: metal)

- cnc: material aluminum_6061|aluminum_7075|stainless_304|steel_45|steel_q235|brass|copper|abs_black|abs_white|pom_black|pom_white|pmma|nylon|polycarbonate|fr4, finish none|anodize|bead_blast|bead_blast_anodize|hard_anodize|conductive_anodize|brushed|brushed_anodize|laser_mark, tolerance standard|0.1|0.05|0.02

- sheetmetal: material aluminum_5052|stainless_304|stainless_201|steel_cold_rolled|steel_galvanized, thickness_mm (0.5 to 6), finish none|powder_coat|anodize|conductive_anodize|hard_anodize.
