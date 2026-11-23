# A realistic big hub for the sidebar rig (SEED=seed43.py rig.sh): 43 hardware and software projects, the uberbot, bots, rooms,
# and threads that need the person, run, stopped, are pinned or settled.
import json, sys, time, os
R = sys.argv[1]; N = int(time.time()); D = 86400; H = 3600
ev = [{"$":"SettingSet","key":k,"value":v,"at":N-90*D} for k, v in [("restart.continue","off"),("onboard.done","1"),("onboard.uber","1")]]
HW = ["STM32N657_SoM", "Orbital Feed Actuator", "OpenUWP Thermostat", "HaLow Baby Monitor", "Boost_12V_48V_480W", "Hubble_DK", "LoRa Gateway Rev C", "BLDC Driver 60V",
      "USB-PD Sink 100W", "Flex Sensor Array", "nRF54 Tag", "PoE Splitter", "E-Ink Badge", "CAN Isolator", "Reflow Controller", "RP2350 Stamp", "GaN Half Bridge",
      "Buck 5V 3A", "Stepper Hat", "Audio DAC I2S", "Solar MPPT 20A", "Thermal Camera Carrier", "Battery Fuel Gauge"]
SW = ["Backplane", "PinPlanner", "i2cjak.com", "kistack", "fabsrv", "pcbparts", "netlist-diff", "bom-lint", "step2glb", "gerber-view", "uwp-firmware", "halow-fw",
      "tag-fw", "orbital-fw", "docs-site", "bench-scripts", "kicad-plugins", "symbol-gen", "label-printer", "ci-runners"]
def proj(pid, title, at, hw):
    root = f"{R}/proj/{pid}"; os.makedirs(root, exist_ok=True)
    if hw:  # a KiCad project: the hub's design detection reads the folder
        for ext in ("kicad_pro", "kicad_pcb", "kicad_sch"): open(f"{root}/{pid}.{ext}", "w").write("")
    ev.append({"$":"ProjectCreated","id":pid,"title":title,"root":root,"at":at})
def th(tid, pid, title, at): ev.append({"$":"ThreadCreated","id":tid,"project":pid,"title":title,"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":at})
def settle(tid, at): ev.append({"$":"ThreadSettled","id":tid,"manual":True,"at":at})
names = []
for i in range(max(len(HW), len(SW))):
    if i < len(HW): names.append((HW[i], True))
    if i < len(SW): names.append((SW[i], False))
for i, (name, hw) in enumerate(names):
    pid = f"p{i:02d}"; age = [0, 0, 1, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13][i % 17] + (i // 17) * 9
    proj(pid, name, N-(age+30)*D, hw)
    th(f"t{i:02d}a", pid, ["Route the power stage", "Fix the build", "BOM review", "Bring-up notes", "Rev B changes"][i % 5], N-(age+2)*D)
    settle(f"t{i:02d}a", N-age*D-H)
    th(f"t{i:02d}b", pid, ["Footprint check", "Refactor the parser", "Order rev A", "DRC cleanup", "Release 0.3"][i % 5], N-(age+1)*D)
    settle(f"t{i:02d}b", N-age*D-2*H)
# what needs the person, runs, stopped, is pinned
th("n1", "p00", "Swap the PMIC for one in stock?", N-2*H); ev.append({"$":"AskOpened","id":"a1","thread":"n1","kind":"question","tool":"AskUserQuestion","detail":"TPS65219 or DA9063?","options":"","at":N-900})
th("n2", "p01", "Clean the build cache", N-3*H); ev.append({"$":"AskOpened","id":"a2","thread":"n2","kind":"approval","tool":"Bash","detail":"rm -rf target","options":"","at":N-300})
th("n3", "p04", "Thermal sim of the inductor", N-5*H); ev.append({"$":"TurnChanged","thread":"n3","state":"failed","at":N-H})
th("r1", "p02", "Length-match the DDR bus", N-40*60); th("r2", "p03", "Port the settings page", N-25*60); th("r3", "p06", "Antenna keep-out pass", N-10*60)
for t in ("r1", "r2", "r3"): ev.append({"$":"TurnChanged","thread":t,"state":"running","at":N-300})
th("k1", "p02", "Rev B errata", N-6*D); ev.append({"$":"ThreadPinned","id":"k1","pinned":True,"at":N-6*D})
# marks that outlast the hub's start (it closes open asks and cuts running turns of a log it did not write): a sign-in that waits, turns the
# restart carries on (running), a usage hold (waiting), a snoozed stop, and the person's visits (the bench's order)
def sset(k, v, at=N-10): ev.append({"$":"SettingSet","key":k,"value":v,"at":at})
sset("auth.wait.n2", "1")
for t in ("r1", "r2"): sset(f"carry.wait.{t}", "1")
sset("provider.resume.r3", "1")
th("z1", "p05", "Lighthouse audit", N-8*H); ev.append({"$":"TurnChanged","thread":"z1","state":"failed","at":N-7*H}); ev.append({"$":"ThreadSnoozed","id":"z1","until":N+20*H,"at":N-6*H})
for i, pid in enumerate(["p00", "p02", "p01", "p06", "p04", "p10", "p03"]): sset(f"sidebar.visit.{pid}", str(N-(i+1)*H-i*D//2))
sset("sidebar.pin." + os.environ.get("PINKEY", "none"), str(N-9*D))
# the uberbot, bots, rooms
def bot(bid, name, look, at): th("t"+bid, "", name, at); ev.append({"$":"BotSet","id":bid,"name":name,"thread":"t"+bid,"look":look,"persona":"","at":at})
bot("b1", "Bub", 306440, N-80*D); ev.append({"$":"UberSet","bot":"b1","uid":"u1","at":N-80*D})
for i, n in enumerate(["Ada", "Scout", "Pebble", "Mailroom", "Fabwatch", "Calendar"]): bot(f"b{i+2}", n, 304951+i*977, N-(60-i)*D)
for i, (n, m) in enumerate([("bring-up", 4), ("fab-orders", 3), ("firmware", 5), ("standup", 7), ("thermostat", 2)]):
    ev.append({"$":"RoomSet","id":f"r{i+1}","name":n,"members":"\n".join(f"bot:b{j+1}" for j in range(m)),"ephemeral":False,"closed":False,"kind":"","project":"","goal":"","by":"","state":"","reason":"","at":N-(20-i)*D})
ev.sort(key=lambda e: e["at"])
open(R + "/home/events.jsonl", "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
