import { describe, expect, mock, test } from "bun:test";
import "./md-swarm.test";

mock.module("virtual:i18n", () => ({ CATALOGS: {} }));

const {
  parseSwarmBoard,
  mergeSwarmBoard,
  swarmEnded,
  swarmPostLabels,
  highlightSwarmMentions,
  SwarmView,
} = await import("../src/swarm");

const SAMPLE_BOARD = {
  room: { id: "r1", name: "Boost", kind: "swarm", project: "p1", goal: "12 to 48 V", state: "active", reason: "", by: "t1" },
  members: [
    { member: "thread:t1", thread: "t1", name: "opus", color: "#3b82f6", role: "designer", provider: "claude", model: "opus", state: "working", title: "opus" },
    { member: "thread:t2", thread: "t2", name: "codex", color: "#22c55e", role: "critic", provider: "codex", model: "gpt-5", state: "waiting", title: "codex" },
  ],
  posts: [
    { id: "p1", from: "opus", thread: "t1", color: "#3b82f6", text: "@codex check `foo` and **table**:\n\n| A | B |\n|---|---|\n| 1 | 2 |", tags: ["idea"], reply: "", mentions: ["codex"], files: ["src/a.rs"], images: ["out/plot.png"], guard: false, held: "", at: 1700000000 },
    { id: "p2", from: "hub", thread: "", color: "", text: "Paused for check.", tags: [], reply: "", mentions: [], files: [], images: [], guard: true, held: "", at: 1700000001 },
    { id: "p3", from: "opus", thread: "t1", color: "#3b82f6", text: "Final summary", tags: ["synthesis", "done"], reply: "", mentions: [], files: [], images: [], guard: false, held: "", at: 1700000002 },
  ],
  claims: [{ post: "p1", thread: "t1", files: ["src/a.rs"], state: "waiting" }],
};

describe("Swarm web model and view parity", () => {
  test("parseSwarmBoard normalizes timestamps, images, tags, and claims", () => {
    const b = parseSwarmBoard(SAMPLE_BOARD);
    expect(b.room.name).toBe("Boost");
    expect(b.posts.length).toBe(3);
    expect(b.posts[0].images).toEqual(["out/plot.png"]);
    expect(b.posts[0].at).toBe(1700000000000);
    expect(swarmPostLabels(b.posts[1])).toEqual([]);
    expect(swarmPostLabels(b.posts[2])).toEqual(["done"]);
    expect(swarmEnded("active")).toBe(false);
    expect(swarmEnded("done")).toBe(true);
    expect(swarmEnded("stopped")).toBe(true);
  });

  test("mergeSwarmBoard updates changed posts in place and appends newer posts", () => {
    const base = parseSwarmBoard(SAMPLE_BOARD);
    const delta = parseSwarmBoard({
      ...SAMPLE_BOARD,
      posts: [
        { ...SAMPLE_BOARD.posts[0], held: "rate" },
        { id: "p4", from: "codex", thread: "t2", color: "#22c55e", text: "New finding", tags: ["critique"], files: [], images: [], guard: false, held: "", at: 1700000005 },
      ],
    });
    const merged = mergeSwarmBoard(base, delta, "p3");
    expect(merged.posts.map((p) => p.id)).toEqual(["p1", "p2", "p3", "p4"]);
    expect(merged.posts[0].held).toBe("rate");
  });

  test("highlightSwarmMentions bolds member mentions in text nodes without touching code", () => {
    const root = document.createElement("div");
    const p = document.createElement("p");
    p.appendChild(document.createTextNode("Ask @codex about "));
    const code = document.createElement("code");
    code.textContent = "@codex";
    p.appendChild(code);
    root.appendChild(p);
    highlightSwarmMentions(root, ["thread:2"], new Map([["thread:2", "codex"], ["thread:1", "opus"]]));
    const strongs = root.querySelectorAll("strong");
    expect(strongs.length).toBe(1);
    expect(strongs[0].textContent).toBe("@codex");
    expect(code.querySelectorAll("strong").length).toBe(0);
  });

  test("SwarmView renders posts incrementally and preserves untouched post DOM nodes", async () => {
    const host = document.createElement("div");
    let current = parseSwarmBoard(SAMPLE_BOARD);
    const loadedImgs: string[] = [];
    const view = new SwarmView(host, {
      onLoad: async () => current,
      onPost: async () => {},
      onAct: async () => {},
      onOpenThread: () => {},
      onLoadImage: async (id, path) => {
        loadedImgs.push(`${id}|${path}`);
        return { url: `/blob/${path}` };
      },
    });
    view.open("r1");
    await new Promise((r) => setTimeout(r, 10));
    const p1NodeBefore = host.querySelector('[data-testid="swarm-post-p1"]');
    expect(p1NodeBefore).toBeDefined();
    expect(loadedImgs).toEqual(["p1|out/plot.png"]);
    expect(host.querySelector('[data-testid="swarm-guard"]')).toBeDefined();
    expect(host.querySelector('[data-testid="swarm-synthesis"]')).toBeDefined();

    // Update post p1 in-place (at cursor 0) + append p4 without NotFoundError on subsequent nodes
    current = parseSwarmBoard({
      ...SAMPLE_BOARD,
      posts: [
        { ...SAMPLE_BOARD.posts[0], held: "rate" },
        SAMPLE_BOARD.posts[1],
        SAMPLE_BOARD.posts[2],
        { id: "p4", from: "codex", thread: "t2", color: "#22c55e", text: "Added later", tags: [], files: [], images: [], guard: false, held: "", at: 1700000010 },
      ],
    });
    await view.refresh(false);
    expect(host.querySelector('[data-testid="swarm-post-p1"]')?.textContent).toContain("held: rate");
    expect(host.querySelector('[data-testid="swarm-post-p4"]')).toBeDefined();

    // Subsequent incremental refresh while held clears held without cursor error
    current = parseSwarmBoard(SAMPLE_BOARD);
    await view.refresh(true);
    expect(host.querySelector('[data-testid="swarm-post-p1"]')?.textContent).not.toContain("held: rate");

    const p1Stable = host.querySelector('[data-testid="swarm-post-p1"]');
    current = parseSwarmBoard({
      ...SAMPLE_BOARD,
      posts: [{ id: "p5", from: "codex", thread: "t2", color: "#22c55e", text: "Fifth", tags: [], files: [], images: [], guard: false, held: "", at: 1700000012 }],
    });
    await view.refresh(true);
    expect(host.querySelector('[data-testid="swarm-post-p1"]')).toBe(p1Stable);
    expect(host.querySelector('[data-testid="swarm-post-p5"]')).toBeDefined();
    view.hide();
  });
});
