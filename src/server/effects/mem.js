// Mem.settle (host.bend): one full collection and a release of free memory;
// the line to log with what it was after, its time and the RSS before and
// after, "" where there is no collector to ask.
function mem_settle(after) {
  if (typeof Bun === "undefined" || typeof Bun.gc !== "function") return "";
  const r0 = process.memoryUsage().rss;
  const t0 = Date.now();
  Bun.gc(true);
  if (typeof Bun.shrink === "function") Bun.shrink();
  const ms = Date.now() - t0;
  const r1 = process.memoryUsage().rss;
  return "memory settled after " + after + ": one full collection in " + ms + " ms, rss " + Math.round(r0 / 1e6) + " MB -> " + Math.round(r1 / 1e6) + " MB";
}

io_eff(CID(Mem.settle), mem_settle);
