#!/usr/bin/env python3
"""A plain TCP relay: relay.py LISTEN_PORT TARGET_PORT (loopback). Killing it cuts the link; starting it again restores it."""
import asyncio, sys
LP, TP = int(sys.argv[1]), int(sys.argv[2])
async def pipe(r, w):
    try:
        while True:
            b = await r.read(65536)
            if not b: break
            w.write(b); await w.drain()
    except Exception: pass
    finally:
        try: w.close()
        except Exception: pass
async def handle(r, w):
    try: tr, tw = await asyncio.open_connection("127.0.0.1", TP)
    except Exception: w.close(); return
    await asyncio.gather(pipe(r, tw), pipe(tr, w))
async def main():
    s = await asyncio.start_server(handle, "127.0.0.1", LP, reuse_address=True)
    async with s: await s.serve_forever()
asyncio.run(main())
