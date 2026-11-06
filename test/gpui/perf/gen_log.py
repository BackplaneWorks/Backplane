#!/usr/bin/env python3
"""Synthetic events.jsonl of realistic shape (fresh data, no real content).
usage: gen_log.py OUT N_EVENTS [SEED]
Mix (from a stand-in hub log): SettingSet ~40%, MessagePosted ~25%, ActivityLogged ~20%, UsageSet ~10%, TurnChanged ~5%.
One thread holds 3% of all events (a long conversation); the rest follow a heavy-tailed size."""
import json,random,sys
out,n=sys.argv[1],int(sys.argv[2]);rnd=random.Random(int(sys.argv[3]) if len(sys.argv)>3 else 1)
t0=1_785_000_000; span=90*86400
nproj=max(3,n//20000); nth=max(5,n//150)
words="board layout net rail footprint pad via trace schematic symbol resistor capacitor regulator decoupling clearance drill copper pour thermal connector header ground power signal clock reset sensor antenna stackup impedance routing placement review revision gerber bom".split()
def text(k):
    return " ".join(rnd.choice(words) for _ in range(k))
def para():
    return ". ".join(text(rnd.randint(6,22)).capitalize() for _ in range(rnd.randint(1,9)))+"."
w=open(out,"w",buffering=1<<20)
def put(o):w.write(json.dumps(o,separators=(",",":"))+"\n")
projs=[f"p{t0}-{i}" for i in range(nproj)]
for i,p in enumerate(projs):put({"$":"ProjectCreated","id":p,"title":f"synthetic {i}","root":f"/tmp/synthetic/proj{i}","at":t0})
ths=[f"t{t0}-{1000+i}" for i in range(nth)]
for i,t in enumerate(ths):put({"$":"ThreadCreated","id":t,"project":projs[i%nproj],"title":f"Thread {i}","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":t0+i})
# weights: thread 0 is the long one
wts=[0.03*n]+[ (rnd.paretovariate(1.3)) for _ in ths[1:]]
tot=sum(wts[1:]);wts=[wts[0]]+[x/tot*0.97*n for x in wts[1:]]
cum=[];s=0
for x in wts:s+=x;cum.append(s)
import bisect
c=0;used=nproj+nth
mid=0
while used<n:
    r=rnd.random()*cum[-1];ti=bisect.bisect_left(cum,r);th=ths[min(ti,len(ths)-1)]
    at=t0+int(span*used/n);k=rnd.random()
    if k<0.40:
        key=rnd.choice(["liveness","autosettle.quiet","autosettle.output","tok.basis","turn.from","provider.resume"])+"."+th
        put({"$":"SettingSet","key":key,"value":json.dumps({"last":at,"n":c}) if key.startswith("liveness") else str(at),"at":at})
    elif k<0.65:
        mid+=1;role="user" if rnd.random()<0.2 else "assistant"
        put({"$":"MessagePosted","thread":th,"msg":f"m{at}-{mid}","role":role,"text":para() if role=="assistant" else text(rnd.randint(4,30)),"at":at})
    elif k<0.85:
        mid+=1;put({"$":"ActivityLogged","thread":th,"act":f"g{at}-{mid}","tone":rnd.choice(["tool","tool","tool","info","step"]),"kind":rnd.choice(["Bash","Read","Edit","Grep"]),"summary":text(rnd.randint(4,16)),"at":at})
    elif k<0.95:
        put({"$":"UsageSet","thread":th,"used":20000+rnd.randint(0,150000),"window":0,"at":at})
    else:
        put({"$":"TurnChanged","thread":th,"state":rnd.choice(["completed","completed","failed"]),"at":at})
    used+=1;c+=1
put({"$":"SettingSet","key":"restart.continue","value":"off","at":t0+span})
w.close()
