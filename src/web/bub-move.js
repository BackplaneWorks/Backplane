// Per-connection observations are transient; neither cached pictures nor a
// reconnect announce a move. The winning lease comparison lives in Bend.
export class BubMoveObserver {
  constructor(compare) { this.compare = compare;this.hubs = new Map();this.latest = new Map();this.announced = new Set(); }
  disconnect(hub) { this.hubs.delete(hub); }
  observe(hub, snapshot) {
    if (!snapshot?.uid || !snapshot.holder || !snapshot.stamp) { this.disconnect(hub);return null; }
    const previous = this.hubs.get(hub), latest = this.latest.get(snapshot.uid);
    if (previous?.uid === snapshot.uid && previous.holder === snapshot.holder && previous.term === snapshot.term && previous.stamp === snapshot.stamp) {
      this.hubs.set(hub, snapshot);
      if (latest?.holder === snapshot.holder && latest.term === snapshot.term && latest.stamp === snapshot.stamp) this.latest.set(snapshot.uid, snapshot);
      return null;
    }
    const result = previous && this.compare(previous, snapshot);
    // A UID change or first live observation establishes a fresh baseline.
    if (!previous || previous.uid !== snapshot.uid || result?.advance) this.hubs.set(hub, snapshot);
    const newer = latest && this.compare(latest, snapshot);
    if (!latest || newer?.advance) this.latest.set(snapshot.uid, snapshot);
    const current = !latest || newer?.advance || latest.term === snapshot.term && latest.stamp === snapshot.stamp && latest.holder === snapshot.holder;
    if (!result?.changed || !current || this.announced.has(result.id)) return null;
    this.announced.add(result.id);
    if (this.announced.size > 64) this.announced.delete(this.announced.values().next().value);
    return result;
  }
}

const machine = document => {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 32 28");svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(svg.namespaceURI, "path");
  path.setAttribute("d", "M3 3h26v18H3z M11 25h10 M16 21v4 M6 7h20");
  svg.append(path);return svg;
};

// The toast is outside the keyed application tree. It has no focusable
// descendants, pointer handlers, or layout reads, and removes itself once.
export function showBubMove(move, { document = globalThis.document, schedule = globalThis.setTimeout } = {}) {
  document.getElementById("bub-move")?.remove();
  const toast = document.createElement("aside");
  toast.id = "bub-move";toast.className = "bub-move";
  // This body sibling cannot inherit the application's scoped palette.
  toast.classList.toggle("light", !!document.querySelector(".app.light") || document.documentElement.classList.contains("light"));
  toast.setAttribute("role", "status");toast.setAttribute("aria-live", "polite");
  const title = document.createElement("div");title.className = "bub-move-title";title.textContent = move.title || "Bub’s moving";
  const route = document.createElement("div");route.className = "bub-move-route";route.setAttribute("aria-hidden", "true");
  for (const label of [move.from, move.to]) {
    const place = document.createElement("div");place.className = "bub-move-place";
    const name = document.createElement("span");name.textContent = label;
    place.append(machine(document), name);route.append(place);
  }
  const cat = document.createElement("img");
  cat.src = move.cat;cat.alt = "";cat.width = 48;cat.height = 48;cat.className = "bub-move-cat";
  route.append(cat);
  const description = document.createElement("span");description.className = "bub-move-description";
  description.textContent = `${move.from} → ${move.to}`;
  toast.append(title, route, description);document.body.append(toast);
  schedule(() => toast.remove(), move.duration);
}
