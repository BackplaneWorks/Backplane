#!/usr/bin/env python3
"""The simulation benchmark's fixtures (docs/kicad-sim-bench.md): mkfix.py RUNTIME OUT [--no-vendor]
  OUT/<fixture>/          a KiCad project as a board's schematic is: connectors, mounting holes, a test point, footprints, no Sim.* field, no source
  OUT/<fixture>.solved/   the same schematic set up for simulation (models assigned, parts that do not simulate left out, sources and loads that are
                          out of the BOM and off the board): what the references are simulated from, and what the checks must pass
  OUT/vendor/             the vendor model the buck task's reference needs, downloaded (VENDOR: its URL and the file's sha256 when the reference was made)
Fixtures: lcfilt (an LC output filter with a Zobel network), preamp (a single-supply non-inverting op-amp stage), ldo (an adjustable regulator),
buck (a TPS54331 step-down converter), buck_mp (a step-down converter around a controller whose maker publishes no SPICE model),
vsense (a bus voltage divider into an ADC), aafilter (a two-stage RC low-pass whose second capacitor is to be chosen)."""
import hashlib, io, os, shutil, sys, urllib.request, zipfile
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
from schgen import Sch

# The buck controller's model: Texas Instruments' own unencrypted PSpice transient model of the TPS54331, a plain download from the part's page
# (https://www.ti.com/product/TPS54331, "TPS54331 Unencrypted PSpice Transient Model Package (Rev. A)"): no sign-in, no licence click
# (checked 2026-11-17). Its library names a diode model `MBR340` that it does not define (PSpice's own library has it).
VENDOR = {"url": "https://www.ti.com/lit/zip/SLVM659", "member": "TPS54331_TRANS.LIB", "subckt": "TPS54331_TRANS",
          "sha256": "6033fcc1d6ab2d8a1b16092c1cb77075e613aa0c8e8ef840ae5fec67df72e505"}
G = 2.54
R0603, C0603, C0805 = "Resistor_SMD:R_0603_1608Metric", "Capacitor_SMD:C_0603_1608Metric", "Capacitor_SMD:C_0805_2012Metric"
HDR2, HDR3 = "Connector_PinHeader_2.54mm:PinHeader_1x02_P2.54mm_Vertical", "Connector_PinHeader_2.54mm:PinHeader_1x03_P2.54mm_Vertical"
SOT235 = "Package_TO_SOT_SMD:SOT-23-5"
SRC = dict(board=False)  # a source or a load that is there for simulation only

def at(i, j): return 30 * G + i * 12 * G, 20 * G + j * 12 * G  # a place on the sheet: column i, row j, on the 2.54 mm grid
def board(s, solved, flags=("GND",)):
    """what every board has and no simulation wants: four mounting holes, and the power flags ERC asks for"""
    for i in range(4): s.part("Mechanical:MountingHole", f"H{i + 1}", "MountingHole", *at(i, 4), fp="MountingHole:MountingHole_3.2mm_M3", exclude=solved)
    for i, n in enumerate(flags): s.part("power:PWR_FLAG", f"#FLG{i + 1:02d}", "PWR_FLAG", *at(5 + i, 4), {"1": n})
def conn(s, solved, ref, value, i, j, nets):
    s.part(f"Connector_Generic:Conn_01x{len(nets):02d}", ref, value, *at(i, j), {str(k + 1): n for k, n in enumerate(nets)}, fp=HDR3 if len(nets) == 3 else HDR2, exclude=solved)
def tp(s, solved, i, j, net): s.part("Connector:TestPoint", "TP1", "TestPoint", *at(i, j), {"1": net}, fp="TestPoint:TestPoint_Pad_D1.5mm", exclude=solved)
def vsrc(s, ref, kind, value, i, j, p, n, params=None):
    dev = "I" if kind.startswith("I") else "V"; ty = {"DC": "DC", "SIN": "SIN", "PULSE": "PULSE"}[kind[1:]]
    s.part(f"Simulation_SPICE:{kind}", ref, value, *at(i, j), {"1": p, "2": n}, sim={"Sim.Device": dev, "Sim.Type": ty, "Sim.Pins": "1=+ 2=-", **({"Sim.Params": params} if params else {})}, **SRC)

def lcfilt(s, v):
    conn(s, v, "J1", "AMP_OUT", 0, 0, ["IN", "GND"]); conn(s, v, "J2", "SPEAKER", 5, 0, ["OUT", "GND"])
    s.part("Device:L", "L1", "22u", *at(1, 0), {"1": "IN", "2": "OUT"}, fp="Inductor_SMD:L_Bourns_SRP7028A_7.3x6.6mm", fields={"MPN": "SRP7028A-220M"})
    s.part("Device:C", "C1", "680n", *at(2, 0), {"1": "OUT", "2": "GND"}, fp="Capacitor_SMD:C_1206_3216Metric")
    s.part("Device:R", "R1", "15", *at(3, 0), {"1": "OUT", "2": "ZOBEL"}, fp="Resistor_SMD:R_1206_3216Metric")
    s.part("Device:C", "C2", "220n", *at(3, 1), {"1": "ZOBEL", "2": "GND"}, fp=C0805)
    tp(s, v, 4, 0, "OUT"); board(s, v)
    if v:
        vsrc(s, "V1", "VSIN", "VSIN", 0, 2, "IN", "GND", "dc=0 ampl=1 f=1k ac=1")
        s.part("Device:R", "RL1", "8", *at(5, 2), {"1": "OUT", "2": "GND"}, **SRC)

def preamp(s, v):
    conn(s, v, "J1", "SENSOR", 0, 0, ["IN", "GND"]); conn(s, v, "J2", "OUT", 6, 0, ["AOUT", "GND"]); conn(s, v, "J3", "5V", 0, 2, ["+5V", "GND"])
    s.part("Device:C", "C1", "1u", *at(1, 0), {"1": "IN", "2": "INP"}, fp=C0603)
    s.part("Device:R", "R1", "100k", *at(2, 0), {"1": "+5V", "2": "INP"}, fp=R0603); s.part("Device:R", "R2", "100k", *at(2, 1), {"1": "INP", "2": "GND"}, fp=R0603)
    model = {"Sim.Device": "SUBCKT", "Sim.Library": "models/BP321.lib", "Sim.Name": "BP321", "Sim.Pins": "1=inp 3=inn 5=vcc 2=vee 4=out"} if v else None
    s.part("Amplifier_Operational:LMV321", "U1", "BP321", *at(3, 0), {"1": "INP", "3": "INN", "5": "+5V", "2": "GND", "4": "AOUT"}, fp=SOT235, fields={"MPN": "BP321"}, sim=model)
    s.part("Device:R", "R3", "12k", *at(4, 0), {"1": "AOUT", "2": "INN"}, fp=R0603); s.part("Device:R", "R4", "10k", *at(4, 1), {"1": "INN", "2": "FBG"}, fp=R0603)
    s.part("Device:C", "C2", "10u", *at(4, 2), {"1": "FBG", "2": "GND"}, fp=C0805); s.part("Device:C", "C3", "100n", *at(3, 2), {"1": "+5V", "2": "GND"}, fp=C0603)
    s.part("Device:C", "C4", "4.7n", *at(5, 0), {"1": "AOUT", "2": "GND"}, fp=C0603)
    tp(s, v, 5, 1, "AOUT"); board(s, v, ("GND", "+5V"))
    if v:
        vsrc(s, "V1", "VSIN", "VSIN", 0, 1, "IN", "GND", "dc=0 ampl=1m f=1k ac=1"); vsrc(s, "V2", "VDC", "5", 1, 2, "+5V", "GND")

def ldo(s, v):
    conn(s, v, "J1", "VIN", 0, 0, ["VIN", "GND"]); conn(s, v, "J2", "3V3", 5, 0, ["VOUT", "GND"])
    s.part("Device:C", "C1", "1u", *at(1, 0), {"1": "VIN", "2": "GND"}, fp=C0603)
    model = {"Sim.Device": "SUBCKT", "Sim.Library": "models/BP758.lib", "Sim.Name": "BP758", "Sim.Pins": "1=in 2=gnd 3=en 4=fb 5=out"} if v else None
    s.part("Regulator_Linear:TLV75801PDBV", "U1", "BP758", *at(2, 0), {"1": "VIN", "2": "GND", "3": "VIN", "4": "FB", "5": "VOUT"}, fp=SOT235, fields={"MPN": "BP758"}, sim=model)
    s.part("Device:R", "R1", "100k", *at(3, 0), {"1": "VOUT", "2": "FB"}, fp=R0603); s.part("Device:R", "R2", "20k", *at(3, 1), {"1": "FB", "2": "GND"}, fp=R0603)
    s.part("Device:C", "C2", "10u", *at(4, 0), {"1": "VOUT", "2": "GND"}, fp=C0805)
    tp(s, v, 4, 1, "VOUT"); board(s, v, ("GND", "VIN"))
    if v:
        vsrc(s, "V1", "VDC", "5", 0, 2, "VIN", "GND")
        vsrc(s, "I1", "IPULSE", "IPULSE", 5, 2, "VOUT", "GND", "y1=10m y2=200m td=2m tr=1u tf=1u tw=2m per=10m")

def buck(s, v):
    conn(s, v, "J1", "12V_IN", 0, 0, ["VIN", "GND"]); conn(s, v, "J2", "3V3_OUT", 6, 0, ["VOUT", "GND"])
    s.part("Device:C", "C1", "10u", *at(0, 1), {"1": "VIN", "2": "GND"}, fp="Capacitor_SMD:C_1210_3225Metric"); s.part("Device:C", "C3", "100n", *at(0, 2), {"1": "VIN", "2": "GND"}, fp=C0603)
    s.part("Device:R", "R5", "332k", *at(1, 0), {"1": "VIN", "2": "EN"}, fp=R0603); s.part("Device:R", "R6", "68.1k", *at(1, 1), {"1": "EN", "2": "GND"}, fp=R0603)
    s.part("Device:C", "C5", "10n", *at(1, 2), {"1": "SS", "2": "GND"}, fp=C0603)
    model = {"Sim.Device": "SUBCKT", "Sim.Library": "sim/models/" + VENDOR["member"], "Sim.Name": VENDOR["subckt"], "Sim.Pins": "1=BOOT 6=COMP 3=EN 7=GND 8=PH 4=SS 2=VIN 5=VSENSE"} if v else None
    s.part("Regulator_Switching:TPS54233", "U1", "TPS54331", *at(2, 0), {"1": "BOOT", "2": "VIN", "3": "EN", "4": "SS", "5": "FB", "6": "COMP", "7": "GND", "8": "SW"},
           fp="Package_SO:SOIC-8_3.9x4.9mm_P1.27mm", fields={"MPN": "TPS54331DR"}, sim=model)
    s.part("Device:C", "C4", "100n", *at(3, 0), {"1": "BOOT", "2": "SW"}, fp=C0603)
    s.part("Device:R", "R3", "29.4k", *at(2, 1), {"1": "COMP", "2": "CZ"}, fp=R0603); s.part("Device:C", "C6", "1n", *at(2, 2), {"1": "CZ", "2": "GND"}, fp=C0603)
    s.part("Device:C", "C7", "47p", *at(3, 2), {"1": "COMP", "2": "GND"}, fp=C0603)
    s.part("Device:D_Schottky", "D1", "MBR340", *at(3, 1), {"1": "SW", "2": "GND"}, fp="Diode_SMD:D_SMC", fields={"MPN": "MBRS340T3G"},
           sim={"Sim.Device": "D", "Sim.Pins": "1=K 2=A", "Sim.Library": "sim/models/parts.lib", "Sim.Name": "MBR340"} if v else None)
    s.part("Device:L", "L1", "6.8u", *at(4, 0), {"1": "SW", "2": "VOUT"}, fp="Inductor_SMD:L_Bourns_SRR1260", fields={"MPN": "SRR1260-6R8Y"})
    s.part("Device:R", "R1", "10.2k", *at(4, 1), {"1": "VOUT", "2": "FB"}, fp=R0603); s.part("Device:R", "R2", "3.24k", *at(4, 2), {"1": "FB", "2": "GND"}, fp=R0603)
    s.part("Device:C_Polarized", "C2", "220u", *at(5, 0), {"1": "VOUT", "2": "GND"}, fp="Capacitor_SMD:CP_Elec_8x10", fields={"MPN": "EEE-FK1A221P"},
           sim={"Sim.Device": "SUBCKT", "Sim.Library": "sim/models/parts.lib", "Sim.Name": "C220U_ESR60M", "Sim.Pins": "1=p 2=n"} if v else None)
    tp(s, v, 5, 1, "VOUT"); board(s, v, ("GND", "VIN"))
    if v:
        vsrc(s, "V1", "VDC", "12", 5, 2, "VIN", "GND"); s.part("Device:R", "RL1", "1.65", *at(6, 2), {"1": "VOUT", "2": "GND"}, **SRC)
PARTS_LIB = """* The buck reference's other parts (test/ksim_bench/mkfix.py): the output capacitor with its series resistance, and a 3 A 40 V Schottky
* diode under the name the controller's library asks for.
.subckt C220U_ESR60M p n
C1 p x 220u
R1 x n 60m
.ends
.model MBR340 D (IS=2u RS=30m N=1.05 CJO=300p VJ=0.4 M=0.45 BV=40 TT=1n)
"""

def buck_mp(s, v):
    """the same kind of converter around an MP1470, whose maker publishes no SPICE model (only one for its own simulator): the agent is to stop and ask"""
    conn(s, v, "J1", "12V_IN", 0, 0, ["VIN", "GND"]); conn(s, v, "J2", "3V3_OUT", 6, 0, ["VOUT", "GND"])
    s.part("Device:C", "C1", "22u", *at(0, 1), {"1": "VIN", "2": "GND"}, fp="Capacitor_SMD:C_1210_3225Metric"); s.part("Device:R", "R5", "100k", *at(1, 0), {"1": "VIN", "2": "EN"}, fp=R0603)
    s.part("Regulator_Switching:MP1470", "U1", "MP1470", *at(2, 0), {"1": "GND", "2": "SW", "3": "VIN", "4": "FB", "5": "EN", "6": "BST"}, fp="Package_TO_SOT_SMD:TSOT-23-6", fields={"MPN": "MP1470GJ-Z"})
    s.part("Device:C", "C4", "100n", *at(3, 1), {"1": "BST", "2": "SW"}, fp=C0603); s.part("Device:L", "L1", "4.7u", *at(4, 0), {"1": "SW", "2": "VOUT"}, fp="Inductor_SMD:L_Bourns_SRP7028A_7.3x6.6mm")
    s.part("Device:R", "R1", "40.2k", *at(4, 1), {"1": "VOUT", "2": "FB"}, fp=R0603); s.part("Device:R", "R2", "13k", *at(4, 2), {"1": "FB", "2": "GND"}, fp=R0603)
    s.part("Device:C", "C2", "47u", *at(5, 0), {"1": "VOUT", "2": "GND"}, fp="Capacitor_SMD:C_1210_3225Metric")
    tp(s, v, 5, 1, "VOUT"); board(s, v, ("GND", "VIN"))

def vsense(s, v):
    conn(s, v, "J1", "VBUS", 0, 0, ["VBUS", "GND"]); conn(s, v, "J2", "MCU", 5, 0, ["ADC", "+3V3", "GND"])
    t = lambda p: {"Tolerance": p}
    s.part("Device:R", "R1", "180k", *at(1, 0), {"1": "VBUS", "2": "SENSE"}, fp=R0603, fields=t("1%")); s.part("Device:R", "R2", "22k", *at(1, 1), {"1": "SENSE", "2": "GND"}, fp=R0603, fields=t("1%"))
    s.part("Device:R", "R3", "220k", *at(2, 0), {"1": "+3V3", "2": "SENSE"}, fp=R0603, fields=t("5%")); s.part("Device:C", "C1", "100n", *at(2, 1), {"1": "SENSE", "2": "GND"}, fp=C0603)
    s.part("Device:R", "R4", "1k", *at(3, 0), {"1": "SENSE", "2": "ADC"}, fp=R0603, fields=t("5%"))
    tp(s, v, 4, 0, "ADC"); board(s, v, ("GND", "+3V3", "VBUS"))
    if v: vsrc(s, "V1", "VDC", "24", 0, 2, "VBUS", "GND"); vsrc(s, "V2", "VDC", "3.3", 5, 2, "+3V3", "GND")

TUNED = "3.9n"  # the aafilter's C2 as the reference chooses it (refs.py derives it and checks this)
def aafilter(s, v):
    conn(s, v, "J1", "SIGNAL", 0, 0, ["IN", "GND"]); conn(s, v, "J2", "ADC", 5, 0, ["OUT", "GND"])
    s.part("Device:R", "R1", "2.2k", *at(1, 0), {"1": "IN", "2": "MID"}, fp=R0603); s.part("Device:C", "C1", "10n", *at(1, 1), {"1": "MID", "2": "GND"}, fp=C0603)
    s.part("Device:R", "R2", "4.7k", *at(2, 0), {"1": "MID", "2": "OUT"}, fp=R0603); s.part("Device:C", "C2", TUNED if v else "1n", *at(2, 1), {"1": "OUT", "2": "GND"}, fp=C0603)
    s.part("Device:R", "R3", "100k", *at(3, 0), {"1": "OUT", "2": "GND"}, fp=R0603)
    tp(s, v, 4, 0, "OUT"); board(s, v)
    if v: vsrc(s, "V1", "VSIN", "VSIN", 0, 2, "IN", "GND", "dc=0 ampl=1 f=1k ac=1")

FIXTURES = {"lcfilt": lcfilt, "preamp": preamp, "ldo": ldo, "buck": buck, "buck_mp": buck_mp, "vsense": vsense, "aafilter": aafilter}
MODELS = {"preamp": ["BP321.lib"], "ldo": ["BP758.lib"]}  # a model that is already in the project's folder (not yet assigned to its part)

def vendor(out):
    """the vendor's model, downloaded: OUT/vendor/<member>, or None when the download fails (the buck reference is then not made here)"""
    p = f"{out}/vendor/{VENDOR['member']}"; os.makedirs(f"{out}/vendor", exist_ok=True)
    try:
        req = urllib.request.Request(VENDOR["url"], headers={"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) backplane-ksim-bench"})
        z = zipfile.ZipFile(io.BytesIO(urllib.request.urlopen(req, timeout=60).read()))
        open(p, "wb").write(z.read(next(n for n in z.namelist() if n.endswith(VENDOR["member"]))))
    except Exception as e: print("vendor model not downloaded:", e); return None
    h = hashlib.sha256(open(p, "rb").read()).hexdigest()
    if h != VENDOR["sha256"]: print(f"vendor model changed since the reference was made: sha256 {h} (was {VENDOR['sha256']}): derive the buck reference again")
    return p

def main():
    rt, out = sys.argv[1:3]; cli = rt + "/bin/kicad-cli"; shutil.rmtree(out, ignore_errors=True); os.makedirs(out)
    lib = None if "--no-vendor" in sys.argv else vendor(out)
    for name, fn in FIXTURES.items():
        for solved in (False, True):
            if solved and (name == "buck_mp" or (name == "buck" and not lib)): continue
            d = f"{out}/{name}" + (".solved" if solved else ""); s = Sch(rt, name); fn(s, solved)
            for m in MODELS.get(name, []): os.makedirs(d + "/models", exist_ok=True); shutil.copy(f"{HERE}/models/{m}", d + "/models/")
            if solved and name == "buck":
                os.makedirs(d + "/sim/models"); shutil.copy(lib, d + "/sim/models/"); open(d + "/sim/models/parts.lib", "w").write(PARTS_LIB)
                open(d + "/sim/models/SOURCES.txt", "w").write(f"{VENDOR['member']}: {VENDOR['url']} (Texas Instruments, from the part's page; (C) Texas Instruments, provided \"as is\")\n")
            s.write(d, cli); print(d, len(s.want), "pins on their nets")
if __name__ == "__main__": main()
