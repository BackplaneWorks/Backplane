// Evidence guards shared by the harness and its regression tests.
export class FixtureBots {
  threads = new Map<string, string>();
  uber = "";

  apply(event: any) {
    if (event.$ === "UberSet") this.uber = event.bot;
    if (event.$ === "SettingSet" && event.key === "uber.bot") this.uber = event.value;
    if (event.$ === "BotDeleted") this.threads.delete(event.id);
    if (event.$ === "BotSet") this.threads.set(event.id, event.thread || this.threads.get(event.id) || "");
  }
}

export function streamWindow(times: number[], start: number, end: number, maxGapMs = 2000) {
  const inside = times.filter((t) => t >= start && t < end).sort((a, b) => a - b);
  let last = start, longestGapMs = 0;
  for (const t of inside) { longestGapMs = Math.max(longestGapMs, t - last); last = t; }
  longestGapMs = Math.max(longestGapMs, end - last);
  return { count: inside.length, longest_gap_ms: longestGapMs, sustained: inside.length > 0 && longestGapMs <= maxGapMs };
}

export function completeOpens(values: number[], expected: number) {
  return expected > 0 && values.length === expected && values.every(Number.isFinite);
}
