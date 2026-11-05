// The composer's completion popup (`@bot`, `>thread`, `%project`, `#room`,
// `$skill`, `/command`) drawn by the page itself in the keystroke's frame.
//
// The hub works the rows out (core/refs.bend's Refs.offer, client.bend's
// Skills.shown) and draws them into its composer, but only after the draft
// reached it and it re-rendered the whole view: at 1,000 threads that was
// 0.7 s on a desktop and 1.5 s on a phone per popup. Here the page keeps
// what the hub last drew for each word (PopupRows), narrows an earlier
// word's rows to the one being typed, and, before the hub drew any, offers
// the threads, projects, bots and rooms the sidebar lists (sidebarRefs), with
// the hub's own slug rules. `$` and `/` need no guess: the thread's skills
// come ahead of time from the small skills.list request, and the commands
// from core/slash.bend (skillRows, slashRows). The hub's rows replace these as soon as its composer is
// drawn for the same draft. Picking a row is a plain draft edit
// (completeWord): the token goes in at once, no round trip.
//
// The sidebar lacks what the hub ranks by (activity, running, summaries,
// folded threads), so its rows are a guess at the order: Tab or Enter on them
// waits for the hub's (pickWaits), and only a row the person chose goes at once.

const SEPARATORS = new Set([" ", "\n", "\t", "\r"]);
const SIGILS = "@>%#$/";
export const POPUP_ROWS = 6;

// the draft's last word: after its last space, tab or line break (refs.bend's
// Refs.token)
export function lastWord(text) {
  let i = text.length;
  while (i > 0 && !SEPARATORS.has(text[i - 1])) i--;
  return text.slice(i);
}

// the word a popup is offered for ("" for none)
export function popupWord(text) {
  const w = lastWord(text);
  return w && SIGILS.includes(w[0]) ? w : "";
}

// the draft with its last word replaced by a completion (client.bend's
// Skills.complete)
export function completeWord(text, word) {
  return text.slice(0, text.length - lastWord(text).length) + word + " ";
}

// refs.bend's Slug.of: letters and digits lower-cased, every other run one
// '-', none at either end, at most 40 characters
export function slugOf(s) {
  let out = "", dash = true;
  for (const ch of s) {
    const c = ch.codePointAt(0);
    const alnum = (c >= 97 && c <= 122) || (c >= 65 && c <= 90) || (c >= 48 && c <= 57);
    if (alnum) { out += c >= 65 && c <= 90 ? String.fromCharCode(c + 32) : ch;dash = false; }
    else if (!dash) { out += "-";dash = true; }
  }
  if (out.endsWith("-")) out = out.slice(0, -1);
  out = out.slice(0, 40);
  return out.endsWith("-") ? out.slice(0, -1) : out;
}

// a thread's seq: what its id ends with after the last '-'
const seqOf = id => id.slice(id.lastIndexOf("-") + 1);
// refs.bend's Refs.t.slug: the title's slug, else the id's
const refSlug = (title, id) => slugOf(title) || slugOf(id);

// fuzzy.bend's Fuzzy.score over a folded candidate: every query character in
// order; prefix, runs and word starts score more; 0 is no match
const fold = s => s.normalize("NFKD").replace(/[̀-ͯ]/g, "").toLowerCase();
export function fuzzy(candidate, query) {
  const q = fold(query);
  if (!q) return 1;
  const s = fold(candidate);
  let qi = 0, acc = 0, run = false, prev = " ";
  for (let i = 0; i < s.length && qi < q.length; i++) {
    const c = s[i];
    if (c === q[qi]) {
      const first = i === 0;
      const word = first || " -_./".includes(prev);
      acc += (run ? 5 : 0) + (first ? 15 : 0) + (word ? 8 : 1);
      run = true;qi++;
    } else run = false;
    prev = c;
  }
  return qi === q.length ? acc + 1 : 0;
}

// the rows narrowed to a longer word: those whose token still matches what
// follows its sigil (best first, in the order they had on a tie)
export function narrow(rows, word) {
  const q = word.slice(1);
  return rows.map((r, i) => ({ r, i, s: fuzzy(r.value.slice(1), q) + fuzzy(r.desc || "", q) }))
    .filter(x => fuzzy(x.r.value.slice(1), q) > 0 || fuzzy(x.r.desc || "", q) > 0)
    .sort((a, b) => b.s - a.s || a.i - b.i).map(x => x.r);
}

// whether Tab or Enter waits for the hub's rows: only on rows the hub never
// drew for this word (guessed), with the highlight where it started
export const pickWaits = popup => !!popup?.guessed && !popup.moved;
export const PICK_WAIT_MS = 5000;

// What the hub drew for each word, newest kept: `$` (a thread's own skills)
// per thread, every other word for any thread (the same store answers them).
export class PopupRows {
  constructor(limit = 96) { this.limit = limit;this.rows = new Map(); }
  key(thread, word) { return (word[0] === "$" ? thread : "") + "\u0000" + word; }
  put(thread, word, rows) {
    if (!popupWord(word)) return;
    const k = this.key(thread, word);
    this.rows.delete(k);this.rows.set(k, rows);
    while (this.rows.size > this.limit) this.rows.delete(this.rows.keys().next().value);
  }
  // the rows for a word: its own, else the longest earlier word's narrowed
  // to it (null: nothing known yet)
  get(thread, word) {
    for (let n = word.length; n >= 1; n--) {
      const rows = this.rows.get(this.key(thread, word.slice(0, n)));
      if (rows) return n === word.length ? rows : narrow(rows, word);
    }
    return null;
  }
  clear() { this.rows.clear(); }
}

// The threads and projects a hub-drawn sidebar lists, as the hub's own
// reference words (refs.bend's Refs.t.token, Refs.p.cand): a thread's slug,
// with its seq when another listed thread has the same; a project's slug.
// read(node) gives {threads: [{id, title, project, pinned}], projects:
// [{id, title}]} from the view tree, in the sidebar's order.
export function sidebarRefs(list, selected) {
  const counts = new Map();
  for (const t of list.threads) { const s = refSlug(t.title, t.id);counts.set(s, (counts.get(s) || 0) + 1); }
  const here = list.threads.find(t => t.id === selected)?.project || "";
  const names = new Map(list.projects.map(p => [p.id, p.title]));
  const threads = list.threads.filter(t => t.id !== selected).map((t, i) => {
    const slug = refSlug(t.title, t.id);
    return { value: ">" + (counts.get(slug) > 1 ? slug + "-" + seqOf(t.id) : slug), slug, name: t.title, desc: t.project === here ? t.title : (names.get(t.project) || "") + ": " + t.title,
      bonus: (t.project === here ? 40 : 0) + (t.pinned ? 10 : 0), order: i };
  });
  const projects = list.projects.map((p, i) => {
    const slug = refSlug(p.title, p.id);
    return { value: "%" + slug, slug, name: p.title, desc: p.title, bonus: p.id === here ? 5 : 0, order: i };
  });
  const bots = (list.bots || []).filter(b => b.name && !b.id.includes("~")).map((b, i) => ({ value: "@" + b.name, slug: slugOf(b.name), name: b.name, desc: "", bonus: 0, order: i }));
  const rooms = (list.rooms || []).map((r, i) => {
    const slug = refSlug(r.name, r.id);
    return { value: "#" + slug, slug, name: r.name, desc: "", bonus: 0, order: i };
  });
  return { threads, projects, bots, rooms };
}

// client.bend's Skills.rows: the skills whose name starts with what follows
// the sigil, in the hub's order, at most six (skills: [{name, description}])
export function skillRows(skills, word) {
  const q = word.slice(1);
  return skills.filter(k => k.name.startsWith(q)).slice(0, POPUP_ROWS).map(k => ({ value: "$" + k.name, desc: k.description || "" }));
}

// client.bend's Slash.of: the commands, then the skills, at most eight. Only
// a draft that is one word is a command (slash.bend's Slash.typing).
export function slashRows(commands, skills, draft) {
  if (!draft.startsWith("/") || lastWord(draft) !== draft) return null;
  return [...commands, ...skillRows(skills, draft)].slice(0, 8);
}

// whether the sidebar can answer a word at all: walking it costs a phone a
// frame or more, so a word it cannot answer never asks
export const sidebarWord = word => ">%@#".includes(word[0]);

// the index's rows for a word the sidebar answers (null for any other word)
export function indexRows(index, word) {
  const pool = { ">": index?.threads, "%": index?.projects, "@": index?.bots, "#": index?.rooms }[word[0]];
  if (!pool) return null;
  const q = word.slice(1).toLowerCase();
  const scored = [];
  for (const c of pool) {
    const s = q ? fuzzy(c.slug, q) + fuzzy(c.name, q) : 1;
    if (s > 0) scored.push({ c, s: s + c.bonus });
  }
  scored.sort((a, b) => b.s - a.s || a.c.order - b.c.order);
  return scored.slice(0, POPUP_ROWS).map(({ c }) => ({ value: c.value, desc: c.desc }));
}
