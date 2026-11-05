// interactions.mjs : what a person clicks in a day. Each entry:
//   { id, what, prep(h, S) (unmeasured: bring the app to the starting point),
//     step(h, S, i) -> { pred, go, undo? } (measured: pred = JS expression true when the result is on screen;
//     undo = unmeasured, back to the starting point for the next rep; reps alternate where it toggles) }
// S = the state's fixture: thread / project ids (see STATES at the bottom).
import { sleep } from './driver.mjs';

const q = (s) => JSON.stringify(s);
// thread X is on screen: the main view is X, not busy, and its timeline shows text
export const threadShown = (id) => `(() => { const m = document.querySelector('main.thread-view[data-thread=${q(id)}]'); if (!m || m.getAttribute('aria-busy') === 'true') return false;
  const rows = m.querySelectorAll('#timeline .timeline-virtual-row'); for (const r of rows) if (r.textContent.trim().length > 20) return true; return false; })()`;
const visible = (sel) => `(() => { for (const e of document.querySelectorAll(${q(sel)})) if (e.getClientRects().length) return true; return false; })()`;
const gone = (sel) => `!${visible(sel)}`;
const row = (id) => `.thread[data-value="${id}"]`;

// make sure a sidebar row is visible: open the project's shelves (Settled / Older) if needed
export async function ensureRow(h, id) {
  const p = h.page;
  for (let i = 0; i < 6; i++) {
    if (await p.locator(row(id)).filter({ visible: true }).count()) return true;
    const shelves = p.locator('button.shelf[aria-expanded="false"], button.shelf:not([aria-expanded])').filter({ visible: true });
    const n = await shelves.count();
    if (!n) break;
    await shelves.nth(Math.min(i, n - 1)).click().catch(() => {});
    await sleep(500);
  }
  return !!(await p.locator(row(id)).filter({ visible: true }).count());
}
export async function openThread(h, id) {
  if (await h.page.evaluate(threadShown(id))) return;
  if (await ensureRow(h, id)) await h.page.locator(row(id)).filter({ visible: true }).first().click();
  else await paletteTo(h, id);
  await h.page.waitForFunction(threadShown(id), null, { timeout: 20000 });
  await h.idle(800);
}
async function paletteTo(h, id) {
  const title = h.S.titles[id];
  await h.key('Control+k'); await sleep(400);
  await h.type(title.slice(0, 24)); await sleep(800);
  await h.key('Enter');
}
const escClose = (sel) => async (h) => {
  await h.key('Escape');
  const ok = await h.page.waitForFunction(gone(sel), null, { timeout: 3000 }).then(() => true, () => false);
  if (!ok) { await h.page.mouse.click(700, 450); await h.page.waitForFunction(gone(sel), null, { timeout: 3000 }).catch(() => {}); }
  await h.idle(300);
};

const toggleThread = (id, a, b, via = 'row') => ({
  prep: async (h, S) => { await openThread(h, S[b]); await ensureRow(h, S[a]); await ensureRow(h, S[b]); },
  step: async (h, S, i) => {
    const to = S[i % 2 ? b : a];
    await ensureRow(h, to);
    return { pred: threadShown(to), content: '#timeline', go: () => h.click(row(to)), settleAfter: 600 };
  },
});

export const INTERACTIONS = [
  { id: 'thread-near', what: 'sidebar: switch between two threads of one project (short <-> long)', ...toggleThread('x', 'A', 'B') },
  { id: 'thread-long', what: 'sidebar: open the long thread from a short one (one-way, each rep from A)',
    prep: async (h, S) => { await openThread(h, S.A); },
    step: async (h, S) => { await ensureRow(h, S.B); return { pred: threadShown(S.B), go: () => h.click(row(S.B)), undo: () => openThread(h, S.A) }; } },
  { id: 'thread-project', what: 'sidebar: switch to a thread in another project', ...toggleThread('x', 'B', 'C') },
  { id: 'thread-short', what: 'sidebar: switch between two short threads', ...toggleThread('x', 'A', 'D') },
  { id: 'thread-far', what: 'palette: jump to a far thread (other project, 11k events) by title + Enter',
    prep: async (h, S) => { await openThread(h, S.A); },
    step: async (h, S) => { await h.key('Control+k'); await sleep(400); await h.type(S.titles[S.FAR].slice(0, 20)); await h.idle(900);
      return { pred: threadShown(S.FAR), go: () => h.key('Enter'), undo: () => openThread(h, S.A) }; } },
  { id: 'project-home', what: 'sidebar: open a project home (from a thread)',
    prep: async (h, S) => { await openThread(h, S.A); },
    step: async (h, S) => ({ pred: `(() => { const m = document.querySelector('main.project-home[data-project-home=${q(S.P)}]'); return !!m && m.getAttribute('aria-busy') !== 'true' && m.textContent.length > 80; })()`,
      go: () => h.click(`.project-home-link[data-value=${q(S.P)}]`), undo: () => openThread(h, S.A) }) },
  { id: 'thread-menu', what: 'thread header ▾ menu open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: `!!document.querySelector('details.thread-actions[open]') && ${visible('details.thread-actions[open] .local-menu-body button')}`,
      go: () => h.click('details.thread-actions > summary'), undo: escClose('details.thread-actions[open] .local-menu-body') }) },
  { id: 'model-picker', what: 'composer: model / effort picker open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('.model-menu'), content: '.model-menu', go: () => h.click('button.model-chip'), undo: escClose('.model-menu') }) },
  { id: 'account-picker', what: 'composer: account picker open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('.account-picker'), content: '.account-picker', go: () => h.click('button.account-chip'), undo: escClose('.account-picker') }) },
  { id: 'token-popover', what: 'composer: token stats popover open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('.tok-pop'), content: '.tok-pop', go: () => h.click('button.tok-chip'), undo: escClose('.tok-pop') }) },
  { id: 'project-look', what: 'sidebar: project colour / icon picker open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('.look'), content: '.look', go: () => h.click('.sidebar button.badge >> nth=0'), undo: escClose('.look') }) },
  { id: 'palette-open', what: 'Ctrl+K command palette open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: `${visible('.workspace-palette')} && !!document.querySelector('.workspace-palette input')`,
      content: '.workspace-palette', go: () => h.key('Control+k'), undo: escClose('.workspace-palette') }) },
  { id: 'palette-type', what: 'palette: one keystroke of a query -> the results list changes',
    prep: async (h, S) => { await openThread(h, S.B); await h.key('Control+k'); await sleep(500); },
    step: async (h, S, i) => {
      const before = await h.page.evaluate(() => document.activeElement?.value || '');
      if (before.length >= 6) { await h.key('Control+a'); await h.key('Backspace'); await h.idle(500); }
      const typed = before.length >= 6 ? '' : before;
      const list = await h.page.evaluate(() => [...document.querySelectorAll('.workspace-palette .workspace-command')].map((e) => e.textContent).join('|'));
      const ch = 'neurbo'[typed.length];
      return { pred: `(() => { const a = document.activeElement; if (!a || a.value !== ${q(typed + ch)}) return false;
        return [...document.querySelectorAll('.workspace-palette .workspace-command')].map((e) => e.textContent).join('|') !== ${q(list)}; })()`, go: () => h.type(ch), timeout: 4000 };
    } },
  { id: 'search-open', what: 'top bar: Search conversations open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: `${visible('#thin-result')} && !!document.activeElement?.closest?.('#thin-result')`,
      go: () => h.click('button.find-open'), undo: escClose('#thin-result') }) },
  { id: 'search-type', what: 'Search conversations: last keystroke of a word -> hits for the whole word shown',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S, i) => {
      const w = ['board', 'layer', 'curre', 'route'][i % 4];
      await h.click('button.find-open'); await h.page.waitForFunction(`!!document.activeElement?.closest?.('#thin-result')`, null, { timeout: 5000 });
      await h.type(w.slice(0, 4)); await h.idle(1500);
      return { pred: `[...document.querySelectorAll('#thin-result .find-hit mark')].some((m) => m.textContent.toLowerCase() === ${q(w)})`,
        go: () => h.type(w[4]), undo: escClose('#thin-result'), timeout: 8000 };
    } },
  { id: 'settings-open', what: 'gear: Settings open (from a thread)',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S) => ({ pred: visible('#settings-content'), content: '#settings-content', go: () => h.click('button.gear'), undo: () => openThread(h, S.B) }) },
  { id: 'settings-tab', what: 'Settings: switch between two pages',
    prep: async (h, S) => { await openThread(h, S.B); await h.click('button.gear'); await h.page.waitForSelector('#settings-content'); await h.idle(500); },
    step: async (h, S, i) => {
      const links = await h.page.evaluate(() => [...document.querySelectorAll('#settings-content [data-on-click="settings-page"]')].filter((e) => e.getClientRects().length && e.dataset.value).map((e) => e.dataset.value));
      const page = await h.page.evaluate(() => document.querySelector('#settings-content')?.dataset.settingsPage || '');
      if (page) return { pred: `document.querySelector('#settings-content')?.dataset.settingsPage === ''`, go: () => h.click(`[data-on-click="settings-page"][data-value=""]`) };
      const to = links[i % Math.max(1, links.length)] || 'agents';
      return { pred: `document.querySelector('#settings-content')?.dataset.settingsPage === ${q(to)}`, go: () => h.click(`#settings-content [data-on-click="settings-page"][data-value=${q(to)}]`) };
    } },
  { id: 'bot-page', what: 'sidebar: open a bot',
    prep: async (h, S) => { await openThread(h, S.A); },
    step: async (h, S) => ({ pred: `!!document.querySelector('main') && (document.querySelector('main')?.textContent || '').includes(${q(S.bot)}) && !document.querySelector('main.thread-view[data-thread=${q(S.A)}]')`,
      go: () => h.click(`.sidebar >> text=${S.bot}`), undo: () => openThread(h, S.A) }) },
  ...[['Files', 'fm'], ['Diff', 'diff'], ['Board', 'board'], ['Schematic', 'schematic'], ['3D', '3d'], ['Browser', 'browser']].map(([name, k]) => ({
    id: 'viewer-' + k, what: `workspace rail: open ${name} (from chat), long thread`,
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S) => ({ pred: k === 'term' ? visible('.term-head') : visible('aside#viewer'), content: 'aside#viewer',
      go: () => h.click(`nav.workspace-bar button.tab[aria-label=${q(name)}]`), undo: () => closeViewer(h, name) }) })),
  { id: 'tool-fold', what: 'timeline: expand / collapse a tool-call group',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S, i) => {
      const open = await h.page.evaluate(() => !!document.querySelector('#timeline button.fold[aria-expanded="true"]'));
      if (open) return { pred: `!document.querySelector('#timeline button.fold[aria-expanded="true"]')`, go: () => h.click('#timeline button.fold[aria-expanded="true"]') };
      return { pred: `!!document.querySelector('#timeline button.fold[aria-expanded="true"]')`, go: () => h.click('#timeline button.fold[aria-expanded="false"] >> nth=-1') };
    } },
  { id: 'scroll-up', what: 'timeline: wheel up 3 notches in the long thread -> new rows drawn',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S, i) => {
      const box = await h.box('#timeline');
      const key = await h.page.evaluate(() => [...document.querySelectorAll('#timeline .timeline-virtual-row')].map((r) => r.dataset.virtualKey).join(','));
      return { pred: `[...document.querySelectorAll('#timeline .timeline-virtual-row')].map((r) => r.dataset.virtualKey).join(',') !== ${q(key)}`,
        go: async () => { await h.page.mouse.move(box.x, box.y + 200); await h.page.mouse.wheel(0, -900); },
        undo: async () => { if (i % 5 === 4) { await h.key('End'); await h.idle(600); } } };
    } },
  { id: 'type-key', what: 'composer: keystroke -> echo (long thread)',
    prep: async (h, S) => { await openThread(h, S.B); await h.click('#composer'); await h.idle(300); },
    step: async (h, S, i) => {
      const v = await h.page.evaluate(() => document.querySelector('#composer').value);
      const ch = 'abcdefghij'[i % 10];
      return { pred: `document.querySelector('#composer').value === ${q(v + ch)}`, go: () => h.type(ch), quiet: 200 };
    } },
  { id: 'send', what: 'composer: Enter -> my message bubble shown (short thread, stand-in agent replies)',
    prep: async (h, S) => { await openThread(h, S.D); },
    step: async (h, S, i) => {
      const text = `ping ${Date.now() % 100000} ${i}`;
      await h.click('#composer'); await h.page.keyboard.type(text); await h.idle(300);
      return { pred: `[...document.querySelectorAll('#timeline .timeline-virtual-row')].some((r) => r.textContent.includes(${q(text)}))`, go: () => h.key('Enter'), cap: 4000,
        undo: async () => { await h.idle(2500); } };
    } },
  { id: 'esc-back', what: 'Esc closes Settings back to the thread',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S) => {
      await h.click('button.gear'); await h.page.waitForSelector('#settings-content'); await h.idle(500);
      return { pred: threadShown(S.B), go: () => h.key('Escape'), undo: () => openThread(h, S.B) };
    } },
  { id: 'resize', what: 'window resize 1400 -> 1100 wide (and back)',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S, i) => {
      const w = i % 2 ? 1400 : 1100;
      return { pred: `innerWidth === ${w}`, go: async () => { await h.page.evaluate(() => { window.__c2r.t0 = performance.now(); window.__c2r.inputType = 'resize'; }); await h.page.setViewportSize({ width: w, height: 900 }); } };
    }, webOnly: true },
];

async function closeViewer(h, name) {
  const p = h.page;
  if (name === 'Terminal') { await p.locator('nav.workspace-bar button.tab[aria-label="Terminal"]').first().click(); await p.waitForFunction(gone('.term-head'), null, { timeout: 5000 }).catch(() => {}); await h.idle(400); return; }
  const tab = `nav.workspace-bar button.tab[aria-label=${q(name)}]`;
  // the Chat tab, else toggling the same tab; wait until it is really off (a remote hub answers late), else Escape
  const on = () => p.evaluate((s) => { const e = document.querySelector(s); return e?.getAttribute('aria-pressed') === 'true' || e?.getAttribute('aria-current') === 'true'; }, tab);
  const chat = p.locator('nav.workspace-bar button.tab[aria-label="Chat"]').filter({ visible: true });
  if (await chat.count()) await chat.first().click(); else await p.locator(tab).first().click().catch(() => {});
  const frame = () => p.evaluate(() => { for (const e of document.querySelectorAll('aside#viewer')) if (e.getClientRects().length) return true; return false; });
  for (let i = 0; i < 40 && (await on() || await frame()); i++) { if (i === 20) await h.key('Escape'); await sleep(100); }
  await h.idle(500);
}

// a state's fixture: which threads play A (short), B (long), C (other project), D (short, same project as C),
// FAR (a long thread elsewhere, reached by the palette), P (A's project), bot (a bot's name), titles (for the palette).
// run.mjs --fixture FILE.json, else fixture-<STATE>.json beside this file.
export const STATES = {};
