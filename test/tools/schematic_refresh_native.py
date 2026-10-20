#!/usr/bin/env python3
"""Synthetic-only save-to-native-presentation probe; never takes a CAD path."""
import argparse, hashlib, json, os, selectors, shutil, subprocess, time
from pathlib import Path


def schematic(rev=0, wires=1, sheets=0):
    forms = ['(kicad_sch (version 20250114)',
             f'(text "REV{rev:03}" (at 10 5 0) (effects (font (size 2 2))) (uuid "text"))']
    for i in range(wires):
        forms.append(f'(wire (pts (xy 10 {10+i%80}) (xy {20+rev%10} {10+i%80})) (stroke (width 0.4) (type default)) (uuid "w{i}"))')
    for i in range(sheets):
        forms.append(f'(sheet (at {10+i*10} 30) (size 8 8) (uuid "s{i}") (property "Sheetname" "Sheet{i}" (at 10 28 0) (effects (font (size 1 1)))) (property "Sheetfile" "sheet{i}.kicad_sch" (at 10 40 0) (effects (font (size 1 1)))))')
    return '\n'.join(forms)+')\n'


def run(binary, out, trace=False):
    out.mkdir(parents=True, exist_ok=False)
    fixture = out/'fixture'; fixture.mkdir()
    path = fixture/'refresh.kicad_sch'
    path.write_text(schematic())
    # Metadata companions must survive the benchmark unchanged.
    for name in ('refresh.kicad_pro','refresh.kicad_prl','.backplane.json'):
        (fixture/name).write_text('{}\n')
    snap = out/'present.ppm'
    env = dict(os.environ, BACKPLANE_SNAP=str(snap))
    command=[str(binary), '--threads', '2', str(path)]
    if trace:command=['strace','-f','-qq','-ttt','-T','-e','trace=newfstatat,openat','-o',str(out/'native.syscalls')]+command
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, bufsize=0, env=env)
    sel=selectors.DefaultSelector(); sel.register(process.stdout, selectors.EVENT_READ)
    captured_pids={process.pid}
    references={}
    reference=os.environ.get("SCHEMATIC_REFRESH_REFERENCE", "")
    if reference:
        for frame in json.loads(Path(reference).read_text())["frames"]:
            references.setdefault(frame["revision"],frame["x11"])
    buffer=b''
    events=[]; writes={}; frames=[]; revision=None; scheduled=None; pending=[]
    deadline=time.monotonic()+22
    def save(rev, atomic=False, partial=False):
        nonlocal revision
        data=schematic(rev)
        if partial: data=data[:len(data)//2]
        if atomic:
            tmp=path.with_suffix('.tmp'); tmp.write_text(data); os.replace(tmp,path)
        else: path.write_text(data)
        t=time.monotonic_ns(); st=path.stat()
        writes[rev]={'monotonic_ns':t,'wall_ns':time.time_ns(), 'mtime_ns':st.st_mtime_ns,'size':st.st_size,'atomic':atomic,'partial':partial}
        events.append({'save':rev,**writes[rev]}); print('save',rev,flush=True)
    try:
        while time.monotonic()<deadline:
            now=time.monotonic()
            if scheduled and time.time()>=scheduled:
                save(1); scheduled=None
            while pending and now>=pending[0][0]:
                _,rev,atomic,partial=pending.pop(0);save(rev,atomic,partial)
            ready=sel.select(.02)
            if ready:
                chunk=os.read(process.stdout.fileno(),65536)
                buffer+=chunk
                lines=buffer.split(b'\n'); buffer=lines.pop()
                for line in lines:
                    line=line.decode().rstrip();print(line,flush=True)
                    events.append({'line':line,'monotonic_ns':time.monotonic_ns()})
                    if line.startswith('received '):
                        import re
                        m=re.search(r'revision=(\d+)',line)
                        revision=int(m[1]) if m else None
                    if line.startswith('display '):
                        children=Path(f'/proc/{process.pid}/task/{process.pid}/children')
                        if children.exists():captured_pids.update(map(int,children.read_text().split()))
                        t=time.monotonic_ns(); shot=out/f'rev-{revision}-{len(frames)}.ppm'
                        if snap.exists():shutil.copyfile(snap,shot)
                        server=out/f'rev-{revision}-{len(frames)}-x11.png'
                        cap=subprocess.Popen(['import','-window','root',str(server)],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
                        _,err=cap.communicate(timeout=3)
                        assert cap.returncode==0,err.decode()
                        from PIL import Image
                        shown=Image.open(server).convert('RGB'); submitted=Image.open(shot).convert('RGB')
                        assert shown.size==submitted.size and shown.tobytes()==submitted.tobytes(),'X11 pixels differ from submitted native frame'
                        # Read the revision from the actual wire endpoint pixels,
                        # independently of the probe's parser diagnostics.
                        green=[x for x in range(400,601) if shown.getpixel((x,340))==(63,191,79)]
                        assert green,'saved wire is absent from X11 readback'
                        revision=round((max(green)-500)/10)
                        if revision is not None:
                            endpoint=500+10*(revision%10)
                            assert shown.getpixel((endpoint-5,340))==(63,191,79),'expected saved wire endpoint absent'
                            assert shown.getpixel((endpoint+5,340))!=(63,191,79),'stale wire extends past saved endpoint'
                        if references:
                            expected=Image.open(references[revision]).convert('RGB')
                            box=(330,270,465,310)
                            assert shown.crop(box).tobytes()==expected.crop(box).tobytes(),f'expected REV{revision:03} text pixels differ'
                        record={'revision':revision,'monotonic_ns':t,'snapshot':str(shot),'x11':str(server),'x11_matches':True,'capture_pid':cap.pid,'capture_pid_absent':not Path(f'/proc/{cap.pid}').exists()}
                        if revision in writes:record['save_to_present_ms']=(t-writes[revision]['monotonic_ns'])/1e6
                        frames.append(record)
                        if revision==0 and scheduled is None: scheduled=int(time.time())+1.02
                        if revision==1 and 2 not in writes:save(2)
                        if revision==2 and 3 not in writes:
                            pending=[(time.monotonic()+.2,3,True,False),(time.monotonic()+1.7,4,False,True),(time.monotonic()+2.1,5,False,False)]
            if process.poll() is not None:break
        if process.poll() is None:raise TimeoutError('native probe exceeded bound')
    finally:
        if process.poll() is None:process.terminate()
        process.wait(timeout=5)
    report={'trace':trace,'binary':str(binary),'sha256':hashlib.sha256(binary.read_bytes()).hexdigest(), 'returncode':process.returncode,'native_pid':process.pid,'native_pid_absent':not Path(f'/proc/{process.pid}').exists(),'captured_native_pids':sorted(captured_pids),'captured_native_pids_absent':all(not Path(f'/proc/{p}').exists() for p in captured_pids),'writes':writes,'frames':frames,'events':events,'same_second':1 in writes and 2 in writes and writes[1]['mtime_ns']//10**9==writes[2]['mtime_ns']//10**9,'companions_intact':all((fixture/n).read_text()=='{}\n' for n in ('refresh.kicad_pro','refresh.kicad_prl','.backplane.json'))}
    (out/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k not in ('events','writes')},indent=2))
    assert report['returncode']==0
    assert report['captured_native_pids_absent']
    assert writes[1]['size']==writes[2]['size']
    revisions=[f['revision'] for f in frames]
    assert revisions==sorted(revisions) and revisions[-1]==5,'last saved revision did not win'
    assert report['same_second'], 'did not exercise real same-second same-size writes'
    assert {0,1,2,3,5}<={r['revision'] for r in frames}, 'final saved revisions did not display'
    assert 4 not in {r['revision'] for r in frames}, 'partial write was displayed'
    assert report['companions_intact']

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('binary',type=Path);p.add_argument('output',type=Path);p.add_argument('--trace',action='store_true')
    a=p.parse_args();run(a.binary.resolve(),a.output.resolve(),a.trace)
