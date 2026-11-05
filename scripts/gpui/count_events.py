import re,collections,sys
pat=re.compile(rb'^\{"\$":"([A-Za-z]+)"')
th=re.compile(rb'"thread":"([^"]*)"')
c=collections.Counter(); n=0; pos=[]; with_entries=set()
with open(sys.argv[1],"rb") as f:
    for l in f:
        n+=1
        m=pat.match(l); k=m.group(1).decode() if m else "?"
        c[k]+=1
        if k=="ThreadCreated": pos.append(n)
        elif k in ("MessagePosted","ActivityLogged"):
            t=th.search(l)
            if t: with_entries.add(hash(t.group(1)))
print("events",n,"kinds",len(c))
for k,v in c.most_common(10): print(k,v)
print("ThreadCreated",len(pos))
print("threads created by 25/50/75/100% of events",[sum(1 for p in pos if p<=n*i//4) for i in (1,2,3,4)])
print("threads with entries",len(with_entries))
e=c["MessagePosted"]+c["ActivityLogged"]; print("entries",e,"share",round(e/n,3))
