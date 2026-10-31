import Base
import ./check.bend as C
import ../src/core/checks.bend as K

def erc() -> String:
  "{\"coordinate_units\":\"mm\",\"kicad_version\":\"10.0.6\",\"sheets\":[{\"path\":\"/\",\"uuid_path\":\"/a\",\"violations\":[{\"description\":\"Footprint 'DSUB-25_Female_Horizontal_P2.77x2.84mm_EdgePinOffset9.90mm_Housed_MountingHolesOffset11.32mm' not found in library 'Connector_Dsub'\",\"items\":[{\"description\":\"Symbol P1 [DB25_Female]\",\"pos\":{\"x\":3.8227,\"y\":1.8288},\"uuid\":\"00000000-0000-0000-0000-00003256759c\"}],\"severity\":\"warning\",\"type\":\"footprint_link_issues\"}]},{\"path\":\"/Sub/\",\"uuid_path\":\"/a/b\",\"violations\":[{\"description\":\"Pin not connected\",\"items\":[{\"description\":\"Symbol U1 Pin 3 [IN, Input, Line]\",\"pos\":{\"x\":50.8,\"y\":-25.4},\"uuid\":\"u1\"},{\"description\":\"Symbol R1 Pin 1 [Passive, Line]\",\"pos\":{\"x\":10,\"y\":20},\"uuid\":\"u2\"}],\"severity\":\"error\",\"type\":\"pin_not_connected\"}]},{\"path\":\"/Empty/\",\"uuid_path\":\"/a/c\",\"violations\":[]}]}"

def drc() -> String:
  "{\"coordinate_units\":\"mm\",\"kicad_version\":\"10.0.6\",\"violations\":[{\"description\":\"Thermal relief connection to zone incomplete (layer bottom_copper; zone min spoke count 2; actual 1)\",\"items\":[{\"description\":\"Zone [GND] on bottom_copper, priority 0\",\"pos\":{\"x\":193.675,\"y\":133.35},\"uuid\":\"00000000-0000-0000-0000-00004cdec22b\"},{\"description\":\"PTH pad 16 [GND] of U5\",\"pos\":{\"x\":125.095,\"y\":76.2},\"uuid\":\"363a9f9a-0b01-4ef4-a972-9e6d1eab7c5a\"}],\"severity\":\"error\",\"type\":\"starved_thermal\"},{\"description\":\"Non-Mirrored text on back layer\",\"items\":[{\"description\":\"Reference field of R3\",\"pos\":{\"x\":195.65,\"y\":111.06},\"uuid\":\"4cbe1a4d-c11a-4c56-b91c-c730a4af03f8\"}],\"severity\":\"warning\",\"type\":\"nonmirrored_text_on_back_layer\"}],\"unconnected_items\":[{\"description\":\"Missing connection between items\",\"items\":[{\"description\":\"Pad 1 [GND] of C21 on B.Cu\",\"pos\":{\"x\":31.715,\"y\":81.965},\"uuid\":\"aac29d70-ff4c-4bbb-9457-634dbe637b9f\"},{\"description\":\"Pad 4 [GND] of IC3 on B.Cu\",\"pos\":{\"x\":41.485,\"y\":78.965},\"uuid\":\"227e116e-d64e-4b37-b027-28d3e4e6d99c\"}],\"severity\":\"error\",\"type\":\"unconnected_items\"}],\"schematic_parity\":[{\"description\":\"Footprints:CP_Radial_D10.0mm_P5.00mm doesn't match footprint given by symbol (Capacitor_THT:CP_Radial_D10.0mm_P5.00mm)\",\"items\":[{\"description\":\"Footprint C1\",\"pos\":{\"x\":141.605,\"y\":99.695},\"uuid\":\"00000000-0000-0000-0000-000054a581e5\"}],\"severity\":\"warning\",\"type\":\"footprint_symbol_mismatch\"}]}"

def fshow(f: K.Finding) -> String:
  match f:
    case K.Finding{s, v, k, t, p, x, y, h}:
      s ++ "/" ++ v ++ "/" ++ k ++ "/" ++ p ++ "/" ++ Bool.show(h) ++ "/" ++ F32.show(x) ++ "," ++ F32.show(y) ++ "/" ++ t

def fshows(fs: List<&2, K.Finding>) -> String:
  match fs:
    case Nil{}:
      ""
    case f <> t:
      fshow(f) ++ "\n" ++ fshows(t)

def tshow(ts: List<&2, K.Task>) -> String:
  match ts:
    case Nil{}:
      ""
    case K.Task{x, d, g} <> t:
      g ++ ":" ++ x ++ ":" ++ Bool.show(d) ++ " " ++ tshow(t)

def cshow(cs: List<&2, K.Custom>) -> String:
  match cs:
    case Nil{}:
      ""
    case K.Custom{n, r, on} <> t:
      n ++ ":" ++ r ++ ":" ++ String.join(on, ",") ++ ";" ++ cshow(t)

def gshow(c: K.Config) -> String:
  match c:
    case K.Config{cs, K.Gate{e, d, l}}:
      cshow(cs) ++ " " ++ e ++ " " ++ d ++ " " ++ Bool.show(l)

def lens(fs: List<&2, K.Finding>) -> String:
  Nat.show(List.length(&2, K.Finding, fs))

def chks(c: K.Config) -> List<&2, K.Custom>:
  match c:
    case K.Config{cs, _}:
      cs

def ex() -> String:
  "# Power\n- [ ] a\n* [x] b\nplain\n  - [ ] nested\n# Fab\n- [X] c\n"

def both() -> List<&2, K.Finding>:
  List.append(&2, K.Finding, K.Checks.erc(erc()), K.Checks.drc(drc()))

def flood(+n: Nat) -> List<&2, K.Finding>:
  List.replicate(K.Finding, n, K.Checks.finding("drc", "warning", "silk", "a|b\nc", "F.Cu"))

def vio() -> String:
  "{\"description\":\"Board edge clearance violation\",\"items\":[{\"description\":\"Segment on Edge.Cuts\",\"pos\":{\"x\":1.5,\"y\":2.5},\"uuid\":\"a\"}],\"severity\":\"warning\",\"type\":\"copper_edge_clearance\"}"

def big(+n: Nat) -> String:
  "{\"violations\":[" ++ String.join(List.replicate(String, n, vio()), ",") ++ "]}"

def count.at(+s: String, +p: String) -> String:
  Bool.show(String.contains(s, p))

def main() -> IO(Unit):
  +none = K.Checks.gate.default()
  +runs = {[K.Run{"lint", True{}, "ok\nmore"}, K.Run{"fmt", False{}, "bad | pipe"}] : List<&2, K.Run>}
  +tasks = K.Checklist.parse(ex())
  do IO<Unit>:
    C.Check.str("erc findings", fshows(K.Checks.erc(erc())), "erc/warning/footprint_link_issues///True/3.8227,1.8288/Footprint 'DSUB-25_Female_Horizontal_P2.77x2.84mm_EdgePinOffset9.90mm_Housed_MountingHolesOffset11.32mm' not found in library 'Connector_Dsub': Symbol P1 [DB25_Female]\nerc/error/pin_not_connected//Sub//True/50.8,-25.4/Pin not connected: Symbol U1 Pin 3 [IN, Input, Line]; Symbol R1 Pin 1 [Passive, Line]\n")
    C.Check.str("drc findings", fshows(K.Checks.drc(drc())), "drc/error/starved_thermal/bottom_copper/True/193.675,133.35/Thermal relief connection to zone incomplete (layer bottom_copper; zone min spoke count 2; actual 1): Zone [GND] on bottom_copper, priority 0; PTH pad 16 [GND] of U5\ndrc/warning/nonmirrored_text_on_back_layer//True/195.65,111.06/Non-Mirrored text on back layer: Reference field of R3\ndrc/error/unconnected_items/B.Cu/True/31.715,81.965/Missing connection between items: Pad 1 [GND] of C21 on B.Cu; Pad 4 [GND] of IC3 on B.Cu\ndrc/warning/footprint_symbol_mismatch//True/141.605,99.695/Footprints:CP_Radial_D10.0mm_P5.00mm doesn't match footprint given by symbol (Capacitor_THT:CP_Radial_D10.0mm_P5.00mm): Footprint C1\n")
    C.Check.str("erc is not json", lens(K.Checks.erc("{oops")), "0")
    C.Check.str("drc empty", lens(K.Checks.drc("{}")), "0")
    C.Check.str("parses", Bool.show(K.Checks.parses("{}")) ++ Bool.show(K.Checks.parses("[")), "TrueFalse")
    C.Check.str("line", K.Checks.line(both()), "ERC 1 error, 1 warning; DRC 2 errors, 2 warnings")
    C.Check.str("line, none", K.Checks.line(Nil{}), "No findings")
    C.Check.str("gate: none passes", K.Checks.gate(Nil{}, none), "")
    C.Check.str("gate: both", K.Checks.gate(both(), none), "ERC: 1 error; DRC: 2 errors")
    C.Check.str("gate: warnings block too", K.Checks.gate(both(), K.Gate{"warning", "warning", False{}}), "ERC: 2 errors or warnings; DRC: 4 errors or warnings")
    C.Check.str("gate: off", K.Checks.gate(both(), K.Gate{"off", "off", False{}}), "")
    C.Check.str("gate: erc only", K.Checks.gate(both(), K.Gate{"error", "off", False{}}), "ERC: 1 error")
    C.Check.str("gate all: checklist unticked", K.Checks.gate.all(Nil{}, tasks, runs, K.Gate{"error", "error", True{}}), "Checklist: 2 unticked; Custom: 1 failed")
    C.Check.str("gate all: checklist ignored", K.Checks.gate.all(Nil{}, tasks, Nil{}, none), "")
    C.Check.str("checklist parse", tshow(tasks), "Power:a:False Power:b:True Power:nested:False Fab:c:True ")
    C.Check.str("checklist open", Nat.show(K.Checklist.open(tasks)), "2")
    C.Check.str("toggle 1", K.Checklist.toggle(ex(), 1n), "# Power\n- [ ] a\n* [ ] b\nplain\n  - [ ] nested\n# Fab\n- [X] c\n")
    C.Check.str("toggle 3 (an X)", K.Checklist.toggle(ex(), 3n), "# Power\n- [ ] a\n* [x] b\nplain\n  - [ ] nested\n# Fab\n- [ ] c\n")
    C.Check.str("toggle past the end", K.Checklist.toggle(ex(), 9n), ex())
    C.Check.str("toggle twice", K.Checklist.toggle(K.Checklist.toggle(ex(), 2n), 2n), ex())
    C.Check.str("toggle keeps CRLF", K.Checklist.toggle("- [ ] a\r\n- [ ] b\r\n", 1n), "- [ ] a\r\n- [x] b\r\n")
    C.Check.str("toggle no trailing newline", K.Checklist.toggle("- [x] a", 0n), "- [ ] a")
    C.Check.str("toggle text with brackets", K.Checklist.toggle("- [ ] see [1] and [x]", 0n), "- [x] see [1] and [x]")
    C.Check.str("default checklist", Nat.show(List.length(&2, K.Task, K.Checklist.parse(K.Checklist.default()))) ++ " " ++ Nat.show(K.Checklist.open(K.Checklist.parse(K.Checklist.default()))), "15 15")
    C.Check.str("config", gshow(K.Checks.config("{\"checks\":[{\"name\":\"lint\",\"run\":\"make lint\",\"on\":[\"pr\"]},{\"run\":\"echo hi\"},{\"name\":\"nocmd\"}],\"gate\":{\"erc\":\"warning\",\"drc\":\"off\",\"checklist\":true}}")), "lint:make lint:pr;echo hi:echo hi:pr,release,manual; warning off True")
    C.Check.str("config missing", gshow(K.Checks.config("")), " error error False")
    C.Check.str("config gate only", gshow(K.Checks.config("{\"gate\":{\"drc\":\"warning\"}}")), " error warning False")
    C.Check.str("checks for a trigger", cshow(K.Checks.for(chks(K.Checks.config("{\"checks\":[{\"name\":\"lint\",\"run\":\"make lint\",\"on\":[\"pr\"]},{\"run\":\"echo hi\"}]}")), "release")), "echo hi:echo hi:pr,release,manual;")
    C.Check.str("md", K.Checks.md(both(), tasks, runs), "**Design checks:** ERC 1 error, 1 warning; DRC 2 errors, 2 warnings\n\n## ERC (2)\n\n| Severity | Type | Where | Description |\n|---|---|---|---|\n| warning | footprint_link_issues | / | Footprint 'DSUB-25_Female_Horizontal_P2.77x2.84mm_EdgePinOffset9.90mm_Housed_MountingHolesOffset11.32mm' not found in library 'Connector_Dsub': Symbol P1 [DB25_Female] |\n| error | pin_not_connected | /Sub/ | Pin not connected: Symbol U1 Pin 3 [IN, Input, Line]; Symbol R1 Pin 1 [Passive, Line] |\n\n## DRC (4)\n\n| Severity | Type | Where | Description |\n|---|---|---|---|\n| error | starved_thermal | bottom_copper | Thermal relief connection to zone incomplete (layer bottom_copper; zone min spoke count 2; actual 1): Zone [GND] on bottom_copper, priority 0; PTH pad 16 [GND] of U5 |\n| warning | nonmirrored_text_on_back_layer |  | Non-Mirrored text on back layer: Reference field of R3 |\n| error | unconnected_items | B.Cu | Missing connection between items: Pad 1 [GND] of C21 on B.Cu; Pad 4 [GND] of IC3 on B.Cu |\n| warning | footprint_symbol_mismatch |  | Footprints:CP_Radial_D10.0mm_P5.00mm doesn't match footprint given by symbol (Capacitor_THT:CP_Radial_D10.0mm_P5.00mm): Footprint C1 |\n\n## Checklist (2/4)\n\n**Power**\n\n- [ ] a\n- [x] b\n- [ ] nested\n\n**Fab**\n\n- [x] c\n\n## Custom checks\n\n| Check | Result | Output |\n|---|---|---|\n| lint | pass | ok |\n| fmt | FAIL | bad \\| pipe |\n")
    C.Check.str("md, nothing", K.Checks.md(Nil{}, Nil{}, Nil{}), "**Design checks:** No findings\n")
    C.Check.str("md escapes pipes", K.Checks.md(flood(1n), Nil{}, Nil{}), "**Design checks:** DRC 1 warning\n\n## DRC (1)\n\n| Severity | Type | Where | Description |\n|---|---|---|---|\n| warning | silk | F.Cu | a\\|b c |\n")
    C.Check.str("md caps rows", count.at(K.Checks.md(flood(9n), Nil{}, Nil{}), "\n_5 more not shown_\n") ++ count.at(K.Checks.md(flood(4n), Nil{}, Nil{}), "more not shown"), "TrueFalse")
    C.Check.str("long cells are marked", K.Ct.cell(String.repeat("x", 241n)), String.repeat("x", 240n) ++ "…")
    C.Check.str("summary counts omitted findings", Bool.show(String.contains(K.Checks.md(flood(205n), Nil{}, Nil{}), "DRC 205 warnings")), "True")
    C.Check.str("many findings counted", Nat.show(K.Checks.count(flood(3000n), "drc", "warning")), "3000")
    C.Check.str("many violations parsed", lens(K.Checks.drc(big(300n))), "300")
