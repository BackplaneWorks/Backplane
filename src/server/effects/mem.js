// Mem.settle (host.bend): one full collection and a release of free memory,
// at most once per process (after the boot's thin index is warm); the line
// to log with its time and the RSS before and after, "" when nothing was done.
let mem_settled = false;

function mem_settle() {
  if (mem_settled || typeof Bun === "undefined" || typeof Bun.gc !== "function") return "";
  mem_settled = true;
  const r0 = process.memoryUsage().rss;
  const t0 = Date.now();
  Bun.gc(true);
  if (typeof Bun.shrink === "function") Bun.shrink();
  const ms = Date.now() - t0;
  const r1 = process.memoryUsage().rss;
  return "memory settled after the thin index: one full collection in " + ms + " ms, rss " + Math.round(r0 / 1e6) + " MB -> " + Math.round(r1 / 1e6) + " MB";
}

io_eff(CID(Mem.settle), mem_settle);
