# A TCP relay for a rig: listen on LHOST LPORT, forward to RHOST RPORT. While the file CUT exists, every connection is closed and new ones are refused.
# A control port (LPORT+1): any request to /cut creates the file, /heal removes it.
# usage: tcp-relay.py LHOST LPORT RHOST RPORT CUT
import asyncio, os, sys
lh, lp, rh, rp, cut = sys.argv[1], int(sys.argv[2]), sys.argv[3], int(sys.argv[4]), sys.argv[5]
live = set()
async def pipe(r, w):
    try:
        while data := await r.read(65536):
            if os.path.exists(cut): break
            w.write(data); await w.drain()
    except Exception: pass
    finally: w.close()
async def handle(r, w):
    if os.path.exists(cut): w.close(); return
    try: rr, rw = await asyncio.open_connection(rh, rp)
    except Exception: w.close(); return
    live.update((w, rw))
    await asyncio.gather(pipe(r, rw), pipe(rr, w))
    live.discard(w); live.discard(rw)
async def watch():
    while True:
        await asyncio.sleep(0.2)
        if os.path.exists(cut):
            for w in list(live): w.close()
            live.clear()
async def control(r, w):
    line = (await r.readline()).decode(errors="replace")
    if " /cut" in line: open(cut, "w").close()
    elif " /heal" in line:
        try: os.remove(cut)
        except FileNotFoundError: pass
    w.write(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"); await w.drain(); w.close()
async def main():
    s = await asyncio.start_server(handle, lh, lp)
    c = await asyncio.start_server(control, lh, lp + 1)
    await asyncio.gather(s.serve_forever(), c.serve_forever(), watch())
asyncio.run(main())
