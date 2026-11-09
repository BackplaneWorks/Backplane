# SendCutSend

> Online sheet metal fabrication, laser cutting, CNC, and finishing services. Upload a design file, choose a material, add services, and get instant pricing. No minimum orders.

SendCutSend cuts, bends, and finishes parts from 178+ materials including metals (aluminum, steel, stainless, titanium, copper, brass), plastics (acrylic, HDPE, Delrin, polycarbonate, ABS), composites (carbon fiber, G-10, phenolic), and soft materials (rubber, cork, wood, foam). Typical turnaround is 3–5 business days.

This file helps LLM agents order parts on sendcutsend.com. Two companion JSON files provide structured data:

- **`sendcutsend-catalog.json`** — Ordering data: material list with SKUs, thicknesses, stock status, available services, cutting process, and size limits. Plus 151 hardware items and 25 finish options. Use this to select materials and configure orders.
- **`sendcutsend-specs.json`** — Engineering reference: bend parameters (k-factor, bend radius, bend deduction), cutting tolerances, material properties (density, strength, hardness), countersinking/tapping/dimple specs. Use this for design validation and manufacturability checks.

Both files are co-hosted alongside this guide.

## Ordering Flow

The ordering process has four configurator steps, then checkout:

1. **Upload** — Go to https://app.sendcutsend.com. Drag-and-drop or browse to upload a design file. Accepted formats: STEP (.step, .stp) preferred for 3D parts with bends; DXF (.dxf) for 2D flat patterns; DWG (.dwg); SVG (.svg); AI (.ai); EPS (.eps). The system auto-detects geometry, dimensions, and units. After upload, a **"Confirm drawing units"** dialog appears — select INCH or MM and click CONFIRM before proceeding. No login is required to upload and get instant pricing.

2. **Select Material** — The app displays the part outline and detected dimensions. Select a material by drilling through four levels: **Group** (Metals, Composites, Plastics, Wood and MDF, Rubber and Gasket) → **Category** (e.g., Aluminum) → **Alloy/Grade** (e.g., 5052 H32 Aluminum) → **Thickness** (e.g., .063"). Alternatively, use the "LIST VIEW" toggle for a flat searchable list of all materials.

3. **Add Services** — Based on the selected material, the app offers applicable secondary operations: bending, countersinking, tapping, hardware insertion, dimple forming, welding. Not all materials support all services — check the material's `available_services` array in the catalog JSON to know what is offered. Set **quantity** in the right sidebar (volume pricing tiers are shown: e.g., 2, 10, 50, 100, 1000). Engraving and etching appear under Add Finishing (step 4), not here.

4. **Add Finishing** — Select optional finishing: anodizing (Type II or Type III), powder coating, plating, tumbling, deburring, engraving, etching. Availability depends on material — again, check `available_services` in the catalog JSON. Anodizing is aluminum-only. Click **"ADD TO CART"** to add the configured part.

5. **Review & Checkout** — After adding to cart, review line items, adjust quantities, and place order. Login/account required at checkout. No minimum order value. Pricing is instant and per-part.

## File Format Guidelines

- **STEP** (.step, .stp): Best for parts with bends. SCS extracts the flat pattern automatically.
- **DXF** (.dxf): Best for 2D flat parts. Use polylines and arcs, not splines where possible. Set $INSUNITS correctly (1 = inches, 4 = mm). Layer names matter for bend lines (see Bending below).
- **DWG** (.dwg): AutoCAD native format. Same geometry rules as DXF.
- **SVG** (.svg): Supported. Convert text to outlines. Set document units explicitly.
- **AI** (.ai): Adobe Illustrator files. Convert text to outlines.
- **EPS** (.eps): Encapsulated PostScript. Convert text to outlines.

Maximum part size depends on material — check `max_part_size` in the catalog JSON. Minimum part size is typically 0.5" x 0.5" — check `min_part_size`. Exact limits vary per material.
