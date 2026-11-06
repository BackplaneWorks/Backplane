# Phone-lane latency probe

`bun scripts/lat/lat.ts ws://HOST:PORT TOKEN [N=200] [M=30]` against a hub's phone stream (`/ws?view=1&mobile=1&enc=cbor`), one WebSocket:
RPC round trip (an unknown-method `ping`, so transport plus dispatch), reconnect (socket open, greeting, first RPC reply) and the first view
(`view.open` of the recent target, ids from 2^47) with its bytes and what a raw deflate of the frame would send. It prints p50/p95/max in ms.

Run it from a tailnet peer against a hub started with `--tailnet-only`; the client is only a measurement and sends nothing a phone would not.
`BACKPLANE_WS_DEBUG=1` on the hub prints what a client offers on the `/ws` upgrade (permessage-deflate).
