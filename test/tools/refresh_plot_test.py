#!/usr/bin/env python3
"""Real hub actor/child protocol correctness on synthetic schematic copies."""
import argparse, importlib.util, json, os, selectors, subprocess, time
from pathlib import Path
from schematic_refresh_native import schematic

def decode(data):
    i=0
    def item():
        nonlocal i
        h=data[i];i+=1;major=h>>5;arg=h&31
        if arg>=24:
            n=1<<(arg-24);arg=int.from_bytes(data[i:i+n],'big');i+=n
        if major==0:return arg
        if major==1:return -1-arg
        if major in (2,3):
            s=data[i:i+arg];i+=arg;return s if major==2 else s.decode()
        if major==4:return [item() for _ in range(arg)]
        if major==5:
            out={}
            for _ in range(arg):
                k=item();out[{1:'t',31:'key'}.get(k,k)]=item()
            return out
        raise ValueError(major)
    v=item();assert i==len(data);return v

def numbers(data):
    out=[];v=shift=0
    for b in data:
        v|=(b&127)<<shift
        if b<128:out.append(v);v=shift=0
        else:shift+=7
    assert shift==0
    return out

def wire_revision(chunks):
    c=next(c for c in chunks if c['l']==14)
    ns=numbers(c['d']);assert ns[:2]==[0,2]
    points=[];x=y=0
    for a,b in zip(ns[2::2],ns[3::2]):
        x+=(a>>1)^-(a&1);y+=(b>>1)^-(b&1);points.append((x,y))
    assert points[0]==(10000,10000) and points[1][1]==10000,points
    return (points[1][0]-20000)//1000

def run(binary,out,child=False):
    out.mkdir(parents=True,exist_ok=False);fixture=out/'fixture';fixture.mkdir()
    path=fixture/('sheet0.kicad_sch' if child else 'refresh.kicad_sch');path.write_text(schematic())
    if child:(fixture/'refresh.kicad_sch').write_text(schematic(sheets=1))
    companions={'refresh.kicad_pro':'{}\n','.backplane.json':'{"project":"refresh.kicad_pro"}\n','refresh.kicad_prl':'{}\n'}
    for name,text in companions.items():(fixture/name).write_text(text)
    info=out/'infos';info.mkdir()
    process=subprocess.Popen([str(binary),'--threads','2',str(path),str(info)],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,bufsize=0)
    sel=selectors.DefaultSelector();sel.register(process.stdout,selectors.EVENT_READ)
    buffer=b'';chunks=[];seen=[];events=[];writes={};scheduled=None;pending=[];end=time.monotonic()+20
    def save(rev,atomic=False,partial=False):
        data=schematic(rev);data=data[:len(data)//2] if partial else data
        if atomic:
            tmp=path.with_suffix('.tmp');tmp.write_text(data);os.replace(tmp,path)
        else:path.write_text(data)
        writes[rev]={'time_ns':time.monotonic_ns(),'mtime_ns':path.stat().st_mtime_ns}
    try:
        while time.monotonic()<end:
            now=time.monotonic()
            if scheduled and time.time()>=scheduled:save(1);scheduled=None
            while pending and now>=pending[0][0]:
                _,r,a,p=pending.pop(0);save(r,a,p)
            if sel.select(.02):
                buffer+=os.read(process.stdout.fileno(),65536);lines=buffer.split(b'\n');buffer=lines.pop()
                for line in lines:
                    line=line.decode();events.append(line)
                    if not line.startswith('plothex '):continue
                    msg=decode(bytes.fromhex(line[8:]));assert msg['t']=='plot';assert msg['key']==f'1|{fixture}/|{path}',msg['key']
                    chunks=[chunks[c] if isinstance(c,int) else c for c in msg['cs']]
                    rev=wire_revision(chunks);record={'revision':rev,'time_ns':time.monotonic_ns()}
                    if rev in writes:record['latency_ms']=(record['time_ns']-writes[rev]['time_ns'])/1e6
                    seen.append(record);print(record,flush=True)
                    if rev==0 and scheduled is None:scheduled=int(time.time())+1.02
                    if rev==1 and 2 not in writes:save(2)
                    if rev==2 and 3 not in writes:
                        pending=[(now+.2,3,True,False),(now+1.7,4,False,True),(now+2.1,5,False,False)]
            if process.poll() is not None:break
        assert process.wait(timeout=1)==0
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
    report={'pid':process.pid,'pid_absent':not Path(f'/proc/{process.pid}').exists(),'seen':seen,'writes':writes,'events':events}
    (out/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert writes[1]['mtime_ns']//10**9==writes[2]['mtime_ns']//10**9,'not same-second edits'
    revisions=[s['revision'] for s in seen];assert {0,1,2,3,5}<=set(revisions),revisions;assert 4 not in revisions
    assert revisions==sorted(revisions),'stale plot replaced newer revision'
    assert all((fixture/n).read_text()==s for n,s in companions.items()),'metadata companion changed'
    if child:assert any('sheet0.kicad_sch' in e for e in events if e.startswith('note ')),'canonical root tree missing watched child'
    assert next(s['latency_ms'] for s in seen if s['revision']==2)<500,'same-second edit still waits for fallback'
    print('ok shared web/phone plot actor: rapid edit/atomic/partial/final revision',flush=True)
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('binary',type=Path);p.add_argument('output',type=Path);p.add_argument('--child',action='store_true');a=p.parse_args();run(a.binary.resolve(),a.output.resolve(),a.child)
