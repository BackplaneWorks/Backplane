"""A TCP proxy that holds every byte the hub sends for DELAY_MS before the client gets it, in order (the client's own bytes pass at once):
a hub slow to answer, as a busy one is, for the tab-loading e2e.  usage: slow_proxy.py LISTEN_PORT HUB_PORT DELAY_MS"""
import asyncio, sys

LISTEN, HUB, DELAY = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3]) / 1000.0

async def pipe(r, w, delay):
    q = asyncio.Queue()
    async def feed():
        while True:
            b = await r.read(65536)
            await q.put((asyncio.get_running_loop().time() + delay, b))
            if not b: return
    async def drain():
        while True:
            at, b = await q.get()
            wait = at - asyncio.get_running_loop().time()
            if wait > 0: await asyncio.sleep(wait)
            if not b:
                try: w.write_eof()
                except Exception: pass
                return
            w.write(b); await w.drain()
    try: await asyncio.gather(feed(), drain())
    except Exception: pass
    finally:
        try: w.close()
        except Exception: pass

async def serve(cr, cw):
    try: hr, hw = await asyncio.open_connection("127.0.0.1", HUB)
    except OSError: cw.close(); return
    await asyncio.gather(pipe(cr, hw, 0), pipe(hr, cw, DELAY))

async def main():
    srv = await asyncio.start_server(serve, "127.0.0.1", LISTEN)
    async with srv: await srv.serve_forever()

asyncio.run(main())
