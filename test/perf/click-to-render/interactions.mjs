// interactions.mjs : what a person clicks in a day. Each entry:
//   { id, what, prep(h, S) (unmeasured: bring the app to the starting point),
//     step(h, S, i) -> { pred, go, undo? } (measured: pred = JS expression true when the result is on screen;
//     undo = unmeasured, back to the starting point for the next rep; reps alternate where it toggles) }
// S = the state's fixture: thread / project ids (see STATES at the bottom).
import { sleep } from './driver.mjs';

const q = (s) => JSON.stringify(s);
// thread X is on screen (what the person sees: the main view is X and its timeline shows text, a cached picture
// marked aria-busy included); threadShown = the same, confirmed by the hub (not busy)
export const threadSeen = (id) => `(() => { const m = document.querySelector('main.thread-view[data-thread=${q(id)}]'); if (!m) return false;
  const rows = m.querySelectorAll('#timeline .timeline-virtual-row'); for (const r of rows) if (r.textContent.trim().length > 20) return true; return false; })()`;
export const threadShown = (id) => `(() => { const m = document.querySelector('main.thread-view[data-thread=${q(id)}]'); if (!m || m.getAttribute('aria-busy') === 'true') return false;
  const rows = m.querySelectorAll('#timeline .timeline-virtual-row'); for (const r of rows) if (r.textContent.trim().length > 20) return true; return false; })()`;
const visible = (sel) => `(() => { for (const e of document.querySelectorAll(${q(sel)})) if (e.getClientRects().length) return true; return false; })()`;
const gone = (sel) => `!${visible(sel)}`;
const row = (id) => `.thread[data-value="${id}"]`;

// ---- CONTENT predicates (round 2): the content the person came for is on screen (cpred) + a signature of it (csig);
// the recorder takes the last time either changed (content shown AND final). Text via textContent (no forced layout).
const TL = (m) => `(() => { const tl = ${m}?.querySelector('#timeline'); if (!tl) return null; const b = tl.getBoundingClientRect();
  return [...tl.querySelectorAll('.timeline-virtual-row')].filter((r) => { const x = r.getBoundingClientRect(); return x.bottom > b.top + 2 && x.top < b.bottom - 2; }); })()`;
// a text's signature: its length + head + tail (a long message's later body windows change the length / tail)
const TS = (x) => `((t) => t.length + ':' + t.slice(0, 60) + '~' + t.slice(-40))((${x}).trim().replace(/\\s+/g, ' '))`;
const rowsText = (m) => `(${TL(m)} || []).map((r) => ${TS('r.textContent')}).join('|')`;
const MAIN = (id) => `document.querySelector('main.thread-view[data-thread=${q(id)}]')`;
// thread content: the thread's timeline rows in view all drawn (text), a message body among them, the header title
export const threadContent = (id) => `(() => { const m = ${MAIN(id)}; if (!m) return false; const rs = ${TL(MAIN(id))}; if (!rs || !rs.length) return false;
  if (rs.some((r) => !r.textContent.trim())) return false; return rs.some((r) => (r.querySelector('.msg .body')?.textContent.trim().length || 0) > 0); })()`;
export const threadSig = (id) => `(${MAIN(id)}?.querySelector('.title')?.textContent || '') + '#' + ${rowsText(MAIN(id))}`;
const VW = `(() => { for (const e of document.querySelectorAll('aside#viewer')) if (e.getClientRects().length) return e; return null; })()`;
const CLOSE = `(() => { if (innerWidth < 700) return true; /* phone: Back, no Close button */ for (const e of document.querySelectorAll('.viewer-close')) if (e.getClientRects().length) return true; return false; })()`;
const vtext = (sel, n = 60, k = 60) => `(() => { const v = ${VW}; if (!v) return ''; return [...v.querySelectorAll(${q(sel)})].slice(0, ${n}).map((e) => ${TS('e.textContent')}).join('|'); })()`;
const fmNames = vtext('.fm-item', 80, 60);
const VIEWER_CONTENT = {
  fm: { cpred: `(() => { const v = ${VW}; return !!v && v.classList.contains('files') && ${CLOSE} && v.querySelectorAll('.fm-item').length > 0; })()`, csig: `${vtext('.fm-bar .crumb', 8, 40)} + '#' + ${fmNames}` },
  diff: { cpred: `(() => { const v = ${VW}; return !!v && v.classList.contains('diff') && ${CLOSE} && !!v.querySelector('.diff-head .summary')?.textContent.trim() && (v.querySelectorAll('.dfile').length > 0 || /\\b0 files|no changes|nothing/i.test(v.textContent)); })()`,
          csig: `${vtext('.diff-head .summary', 1, 80)} + '#' + ${vtext('.dfile .dhead .name', 25, 60)}` },
  board: { cpred: `(() => { const v = ${VW}; if (!v || !v.classList.contains('board') || !${CLOSE}) return false; const c = window.__c2r.cv('aside#viewer canvas#plot'); return !!c && !c.includes('blank') && !!v.querySelector('.hist .title')?.textContent.trim(); })()`,
           csig: `window.__c2r.cv('aside#viewer canvas#plot') + '#' + ${vtext('.hist .title', 1, 80)}` },
  schematic: { cpred: `(() => { const v = ${VW}; if (!v || !v.classList.contains('schematic') || !${CLOSE}) return false; const c = window.__c2r.cv('aside#viewer canvas#plot'); return !!c && !c.includes('blank') && !!v.querySelector('.hist .title')?.textContent.trim(); })()`,
               csig: `window.__c2r.cv('aside#viewer canvas#plot') + '#' + ${vtext('.hist .title', 1, 80)}` },
  '3d': { cpred: `(() => { const v = ${VW}; return !!v && v.classList.contains('3d') && ${CLOSE} && /triangles/.test(v.querySelector('.viewer-stat')?.textContent || '') && !!v.querySelector('canvas#solid'); })()`,
          csig: `${vtext('.viewer-stat', 1, 80)} + '#' + ${vtext('.solid-model-name', 1, 60)} + '#' + [...(document.querySelector('#solid-workbench')?.classList || [])].filter((c) => /original|preview/.test(c)).join(' ') + '#' + (document.querySelector('.solid-render-status')?.textContent || '').slice(0, 40)` },
  browser: { cpred: `(() => { const v = ${VW}; return !!v && v.classList.contains('browser') && ${CLOSE} && (!!v.querySelector('.bot-note')?.textContent.trim() || !!v.querySelector('img, iframe, canvas, .browser-tab')); })()`,
             csig: `${vtext('.bot-note, .browser-tab', 10, 60)} + '#' + (${VW}?.querySelectorAll('img, iframe, canvas').length || 0)` },
  review: { cpred: `(() => { const v = ${VW}; return !!v && ${CLOSE} && v.textContent.trim().length > 60; })()`, csig: `(${VW}?.textContent || '').replace(/\\s+/g, ' ').slice(0, 1500)` },
};
const popContent = (sel, inner) => ({ cpred: `(() => { for (const e of document.querySelectorAll(${q(sel)})) if (e.getClientRects().length && e.querySelectorAll(${q(inner)}).length) return true; return false; })()`,
  csig: `[...document.querySelectorAll(${q(sel)})].map((e) => e.textContent.replace(/\\s+/g, ' ').slice(0, 1500) + '#' + e.querySelectorAll('.on, [aria-checked="true"], [aria-selected="true"]').length).join('|')` });

// make sure a sidebar row is visible: open the project's shelves (Settled / Older) if needed
export async function ensureRow(h, id) {
  const p = h.page;
  for (let i = 0; i < 80; i++) {
    if (await p.locator(row(id)).filter({ visible: true }).count()) return true;
    // closed shelves (Settled / Older / quiet projects), then closed projects (their disclosure shows chevron-right)
    let shelves = p.locator('button.shelf:not(.open)').filter({ visible: true });
    if (!(await shelves.count())) shelves = p.locator('button.project-disclosure:has(svg[data-icon="chevron-right"])').filter({ visible: true });
    const n = await shelves.count();
    if (!n) break;
    // click it and wait until it really opened (a slow or remote hub redraws late; a second click would shut it)
    const el = await shelves.nth(0).elementHandle().catch(() => null);
    await shelves.nth(0).click().catch(() => {});
    if (el) await p.waitForFunction((e) => !e.isConnected || e.classList.contains('open') || !!e.querySelector('svg[data-icon="chevron-down"]'), el, { timeout: 4000 }).catch(() => {});
    await sleep(150);
  }
  return !!(await p.locator(row(id)).filter({ visible: true }).count());
}
export async function openThread(h, id) {
  if (await h.page.evaluate(threadShown(id))) return;
  if (await ensureRow(h, id)) await h.page.locator(row(id)).filter({ visible: true }).first().click();
  else await paletteTo(h, id);
  await h.page.waitForFunction(threadShown(id), null, { timeout: 40000 });
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
    return { pred: threadSeen(to), confirm: threadShown(to), cpred: threadContent(to), csig: threadSig(to), content: '#timeline', go: () => h.click(row(to)), settleAfter: 600 };
  },
});

async function foldStep(h, S, i) {
      const open = await h.page.evaluate(() => !!document.querySelector('#timeline button.fold[aria-expanded="true"]'));
      const tsig = `(${TL("document.querySelector('main.thread-view')")} || []).map((r) => ${TS('r.textContent')}).join('|')`;
      if (open) return { pred: `!document.querySelector('#timeline button.fold[aria-expanded="true"]')`, cpredSame: true, csig: tsig, go: () => h.click('#timeline button.fold[aria-expanded="true"]') };
      return { pred: `!!document.querySelector('#timeline button.fold[aria-expanded="true"]')`, cpredSame: true, csig: tsig, go: () => h.click('#timeline button.fold[aria-expanded="false"] >> nth=-1') };
    }

export const INTERACTIONS = [
  { id: 'thread-near', what: 'sidebar: switch between two threads of one project (short <-> long)', ...toggleThread('x', 'A', 'B') },
  { id: 'thread-long', what: 'sidebar: open the long thread from a short one (one-way, each rep from A)',
    prep: async (h, S) => { await openThread(h, S.A); },
    step: async (h, S) => { await ensureRow(h, S.B); return { pred: threadSeen(S.B), confirm: threadShown(S.B), cpred: threadContent(S.B), csig: threadSig(S.B), go: () => h.click(row(S.B)), undo: () => openThread(h, S.A) }; } },
  { id: 'thread-project', what: 'sidebar: switch to a thread in another project', ...toggleThread('x', 'B', 'C') },
  { id: 'thread-short', what: 'sidebar: switch between two short threads', ...toggleThread('x', 'A', 'D') },
  { id: 'thread-far', what: 'palette: jump to a far thread (other project, 11k events) by title + Enter',
    prep: async (h, S) => { await openThread(h, S.A); },
    step: async (h, S) => { await h.key('Control+k'); await sleep(400); await h.type(S.titles[S.FAR].slice(0, 20)); await h.idle(900);
      return { pred: threadSeen(S.FAR), confirm: threadShown(S.FAR), cpred: threadContent(S.FAR), csig: threadSig(S.FAR), go: () => h.key('Enter'), undo: () => openThread(h, S.A) }; } },
  { id: 'project-home', what: 'sidebar: open a project home (from a thread)',
    prep: async (h, S) => { await openThread(h, S.A); },
    step: async (h, S) => ({ pred: `(() => { const m = document.querySelector('main.project-home[data-project-home=${q(S.P)}]'); return !!m && m.getAttribute('aria-busy') !== 'true' && m.textContent.length > 80; })()`,
      cpred: `(() => { const m = document.querySelector('main.project-home'); return !!m && !!m.querySelector('.home-card') && (m.querySelector('.home-layout')?.textContent.trim().length || 0) > 40; })()`, csig: `(document.querySelector('main.project-home .home-header')?.textContent || '') + '#' + (document.querySelector('main.project-home .home-layout')?.textContent || '').replace(/\\s+/g, ' ').slice(0, 3000)`,
      go: () => h.click(`.project-home-link[data-value=${q(S.P)}]`), undo: () => openThread(h, S.A) }) },
  { id: 'thread-menu', what: 'thread header ▾ menu open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: `!!document.querySelector('details.thread-actions[open]') && ${visible('details.thread-actions[open] .local-menu-body button')}`,
      ...popContent('details.thread-actions[open] .local-menu-body', 'button'), go: () => h.click('details.thread-actions > summary'), undo: escClose('details.thread-actions[open] .local-menu-body') }) },
  { id: 'model-picker', what: 'composer: model / effort picker open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('.model-menu'), content: '.model-menu', ...popContent('.model-menu', 'button.row'), go: () => h.click('button.model-chip'), undo: escClose('.model-menu') }) },
  { id: 'account-picker', what: 'composer: account picker open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('.account-picker'), content: '.account-picker', ...popContent('.account-picker', 'button'), go: () => h.click('button.account-chip'), undo: escClose('.account-picker') }) },
  { id: 'token-popover', what: 'composer: token stats popover open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('.tok-pop'), content: '.tok-pop', ...popContent('.tok-pop', '.tok-v'), go: () => h.click('button.tok-chip'), undo: escClose('.tok-pop') }) },
  { id: 'project-look', what: 'sidebar: project colour / icon picker open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('.look'), content: '.look', ...popContent('.look', '.swatch'), go: () => h.click('.sidebar button.badge >> nth=0'), undo: escClose('.look') }) },
  { id: 'palette-open', what: 'Ctrl+K command palette open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: `${visible('.workspace-palette')} && !!document.querySelector('.workspace-palette input')`,
      content: '.workspace-palette', ...popContent('.workspace-palette', '.workspace-command'), go: () => h.key('Control+k'), undo: escClose('.workspace-palette') }) },
  { id: 'palette-type', what: 'palette: one keystroke of a query -> the results list changes',
    prep: async (h, S) => { await openThread(h, S.B); await h.key('Control+k'); await sleep(500); },
    step: async (h, S, i) => {
      const before = await h.page.evaluate(() => document.activeElement?.value || '');
      if (before.length >= 6) { await h.key('Control+a'); await h.key('Backspace'); await h.idle(500); }
      const typed = before.length >= 6 ? '' : before;
      const list = await h.page.evaluate(() => [...document.querySelectorAll('.workspace-palette .workspace-command')].map((e) => e.textContent).join('|'));
      const ch = 'neurbo'[typed.length];
      return { pred: `(() => { const a = document.activeElement; if (!a || a.value !== ${q(typed + ch)}) return false;
        return [...document.querySelectorAll('.workspace-palette .workspace-command')].map((e) => e.textContent).join('|') !== ${q(list)}; })()`, csig: `[...document.querySelectorAll('.workspace-palette .workspace-command')].map((e) => e.textContent).join('|')`, cpredSame: true, go: () => h.type(ch), timeout: 4000 };
    } },
  { id: 'search-open', what: 'top bar: Search conversations open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: `${visible('#thin-result')} && !!document.activeElement?.closest?.('#thin-result')`,
      cpredSame: true, csig: `document.querySelector('#thin-result')?.textContent.replace(/\\s+/g, ' ').slice(0, 1500)`, go: () => h.click('button.find-open'), undo: escClose('#thin-result') }) },
  { id: 'search-type', what: 'Search conversations: last keystroke of a word -> hits for the whole word shown',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S, i) => {
      const w = ['board', 'layer', 'curre', 'route'][i % 4];
      await h.click('button.find-open'); await h.page.waitForFunction(`!!document.activeElement?.closest?.('#thin-result')`, null, { timeout: 5000 });
      await h.type(w.slice(0, 4)); await h.idle(1500);
      return { pred: `[...document.querySelectorAll('#thin-result .find-hit mark')].some((m) => m.textContent.toLowerCase() === ${q(w)})`,
        cpredSame: true, csig: `document.querySelector('#find-rows')?.textContent.replace(/\\s+/g, ' ').slice(0, 2500)`, go: () => h.type(w[4]), undo: escClose('#thin-result'), timeout: 8000 };
    } },
  { id: 'settings-open', what: 'gear: Settings open (from a thread)',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S) => ({ pred: visible('#settings-content'), content: '#settings-content', ...popContent('#settings-content', '.settings-category'), go: () => h.click('button.gear'), undo: () => openThread(h, S.B) }) },
  { id: 'settings-tab', what: 'Settings: switch between two pages',
    prep: async (h, S) => { await openThread(h, S.B); await h.click('button.gear'); await h.page.waitForSelector('#settings-content'); await h.idle(500); },
    step: async (h, S, i) => {
      const links = await h.page.evaluate(() => [...document.querySelectorAll('#settings-content [data-on-click="settings-page"]')].filter((e) => e.getClientRects().length && e.dataset.value).map((e) => e.dataset.value));
      const page = await h.page.evaluate(() => document.querySelector('#settings-content')?.dataset.settingsPage || '');
      if (page) return { pred: `document.querySelector('#settings-content')?.dataset.settingsPage === ''`, cpred: `document.querySelector('#settings-content')?.dataset.settingsPage === '' && !!document.querySelector('#settings-content .settings-category')`, csig: `(() => { const c = document.querySelector('#settings-content'); if (!c) return ''; return c.textContent.replace(/\\s+/g, ' ').slice(0, 2500) + '#' + [...c.querySelectorAll('input, select, textarea')].map((e) => e.type === 'checkbox' || e.type === 'radio' ? (e.checked ? 1 : 0) : e.value).join(',') + '#' + c.querySelectorAll('[aria-checked="true"], [aria-pressed="true"], .on').length; })()`, go: () => h.click(`[data-on-click="settings-page"][data-value=""]`) };
      const to = links[i % Math.max(1, links.length)] || 'agents';
      return { pred: `document.querySelector('#settings-content')?.dataset.settingsPage === ${q(to)}`, cpred: `document.querySelector('#settings-content')?.dataset.settingsPage === ${q(to)} && !document.querySelector('#settings-content .settings-category') && document.querySelector('#settings-content').querySelectorAll('input, select, textarea, button').length > 1`, csig: `(() => { const c = document.querySelector('#settings-content'); if (!c) return ''; return c.textContent.replace(/\\s+/g, ' ').slice(0, 2500) + '#' + [...c.querySelectorAll('input, select, textarea')].map((e) => e.type === 'checkbox' || e.type === 'radio' ? (e.checked ? 1 : 0) : e.value).join(',') + '#' + c.querySelectorAll('[aria-checked="true"], [aria-pressed="true"], .on').length; })()`, go: () => h.click(`#settings-content [data-on-click="settings-page"][data-value=${q(to)}]`) };
    } },
  { id: 'bot-page', what: 'sidebar: open a bot',
    prep: async (h, S) => { await openThread(h, S.A); },
    step: async (h, S) => ({ pred: `!!document.querySelector('main') && (document.querySelector('main')?.textContent || '').includes(${q(S.bot)}) && !document.querySelector('main.thread-view[data-thread=${q(S.A)}]')`,
      cpredSame: true, csig: `(document.querySelector('main')?.textContent || '').replace(/\\s+/g, ' ').slice(0, 2500)`, go: () => h.click(`.sidebar >> text=${S.bot}`), undo: () => openThread(h, S.A) }) },
  ...[['Files', 'fm'], ['Diff', 'diff'], ['Board', 'board'], ['Schematic', 'schematic'], ['3D', '3d'], ['Browser', 'browser'], ['Review', 'review']].map(([name, k]) => ({
    id: 'viewer-' + k, what: `workspace rail: open ${name} (from chat), long thread`,
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S) => ({ pred: k === 'term' ? visible('.term-head') : visible('aside#viewer'), content: 'aside#viewer', ...VIEWER_CONTENT[k],
      go: () => h.click(`nav.workspace-bar button.tab[aria-label=${q(name)}]`), undo: () => closeViewer(h, name) }) })),
  // ---- round 2: clicks a person makes daily that were missing
  { id: 'viewer-switch', what: 'workspace rail: Files open -> click Diff (viewer tab switch while a viewer is open)',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S) => { await openViewer(h, 'Files', 'fm');
      return { pred: `!!${VW}?.classList.contains('diff')`, content: 'aside#viewer', ...VIEWER_CONTENT.diff, go: () => h.click(tabSel('Diff')), undo: () => closeViewer(h, 'Diff') }; } },
  { id: 'viewer-board-sch', what: 'workspace rail: Board <-> Schematic while one is open',
    prep: async (h, S) => { await openThread(h, S.B); await openViewer(h, 'Board', 'board'); },
    step: async (h, S, i) => { const [name, k] = i % 2 ? ['Board', 'board'] : ['Schematic', 'schematic'];
      if (await h.page.evaluate(`!${VW}`)) await openViewer(h, i % 2 ? 'Schematic' : 'Board', i % 2 ? 'schematic' : 'board');
      return { pred: `!!${VW}?.classList.contains(${q(k)})`, content: 'aside#viewer', ...VIEWER_CONTENT[k], go: () => h.click(tabSel(name)) }; } },
  { id: 'fm-in', what: 'Files: open a folder (boards) from the project root',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S) => { await openViewer(h, 'Files', 'fm'); await fmRoot(h);
      const root = await h.page.evaluate(fmNames);
      return { pred: `/boards/.test(${vtext('.fm-bar .crumb', 8, 40)})`, content: 'aside#viewer',
        cpred: `(() => { const n = ${fmNames}; return /boards/.test(${vtext('.fm-bar .crumb', 8, 40)}) && n !== '' && n !== ${q(root)} && ${CLOSE}; })()`, csig: VIEWER_CONTENT.fm.csig,
        go: () => h.click(`aside#viewer .fm-row.folder[data-value="boards"]`), undo: () => fmRoot(h) }; } },
  { id: 'fm-up', what: 'Files: Up from a folder (boards) to the project root',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S) => { await openViewer(h, 'Files', 'fm'); await fmRoot(h); const root = await h.page.evaluate(fmNames);
      await h.click(`aside#viewer .fm-row.folder[data-value="boards"]`);
      await h.page.waitForFunction(`(() => { const n = ${fmNames}; return /boards/.test(${vtext('.fm-bar .crumb', 8, 40)}) && n !== '' && n !== ${q(root)}; })()`, null, { timeout: 15000 }).catch(() => {}); await h.idle(800);
      const inner = await h.page.evaluate(fmNames);
      return { pred: `!/boards/.test(${vtext('.fm-bar .crumb', 8, 40)})`, content: 'aside#viewer',
        cpred: `(() => { const n = ${fmNames}; return !/boards/.test(${vtext('.fm-bar .crumb', 8, 40)}) && n !== '' && n !== ${q(inner)} && ${CLOSE}; })()`, csig: VIEWER_CONTENT.fm.csig,
        go: () => h.click(`aside#viewer button[data-on-click="fm-up"]`) }; } },
  ...[['Files', 'fm'], ['Board', 'board'], ['Diff', 'diff']].map(([name, k]) => ({
    id: 'reload-' + k, what: `reopen ${name} after a page reload (seen before the reload), long thread`,
    prep: async (h, S) => { await openThread(h, S.B); await openViewer(h, name, k); await closeViewer(h, name); },
    step: async (h, S) => { await h.page.reload(); await h.ready(); await openThread(h, S.B); await h.idle(1000);
      return { pred: visible('aside#viewer'), content: 'aside#viewer', ...VIEWER_CONTENT[k], go: () => h.click(tabSel(name)), undo: () => closeViewer(h, name) }; } })),
  { id: 'reload-thread', what: 'after a page reload: switch to a thread seen before the reload (long -> short)',
    prep: async (h, S) => { await openThread(h, S.A); await openThread(h, S.B); },
    step: async (h, S) => { await h.page.reload(); await h.ready(); await openThread(h, S.B); await ensureRow(h, S.A); await h.idle(1000);
      return { pred: threadSeen(S.A), confirm: threadShown(S.A), cpred: threadContent(S.A), csig: threadSig(S.A), content: '#timeline', go: () => h.click(row(S.A)), undo: () => openThread(h, S.B) }; } },
  { id: 'shelf-open', what: "sidebar: open a project's Settled shelf (its rows shown)",
    prep: async (h, S) => { await openThread(h, S.C); },
    step: async (h, S) => {
      const key = await h.page.evaluate(() => { for (const b of document.querySelectorAll('.sidebar button.shelf[data-on-click="toggle-settled"]')) if (b.getClientRects().length && !b.classList.contains('open')) return b.dataset.value; return null; });
      if (!key) throw new Error('no closed Settled shelf in view');
      const sh = `.sidebar button.shelf[data-on-click="toggle-settled"][data-value=${q(key)}]`;
      const rowsUnder = `(() => { const b = document.querySelector(${q(sh)}); if (!b) return ''; const out = []; let e = b.nextElementSibling; for (let i = 0; e && i < 40; i++, e = e.nextElementSibling) { if (e.matches('button.shelf, section')) break; if (e.querySelector?.('.thread') || e.matches('.thread, .thread-tree-row')) out.push(e.textContent.trim().replace(/\\s+/g, ' ').slice(0, 60)); } return out.join('|'); })()`;
      return { pred: `!!document.querySelector(${q(sh)})?.classList.contains('open')`, cpred: `!!document.querySelector(${q(sh)})?.classList.contains('open') && ${rowsUnder} !== ''`, csig: rowsUnder,
        go: () => h.click(sh), undo: async () => { await h.click(sh); await h.page.waitForFunction(`!document.querySelector(${q(sh)})?.classList.contains('open')`, null, { timeout: 5000 }).catch(() => {}); await h.idle(400); } };
    } },
  { id: 'tool-fold', what: 'timeline: expand / collapse a tool-call group',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: (h, S, i) => foldStep(h, S, i) },
  { id: 'scroll-up', what: 'timeline: wheel up 3 notches in the long thread -> new rows drawn',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S, i) => {
      const box = await h.box('#timeline');
      const key = await h.page.evaluate(() => [...document.querySelectorAll('#timeline .timeline-virtual-row')].map((r) => r.dataset.virtualKey).join(','));
      return { pred: `[...document.querySelectorAll('#timeline .timeline-virtual-row')].map((r) => r.dataset.virtualKey).join(',') !== ${q(key)}`,
        cpred: `(() => { const rs = ${TL("document.querySelector('main.thread-view')")} || []; return rs.map((r) => r.dataset.virtualKey).join(',') !== ${q(key)} && rs.length > 0 && rs.every((r) => r.textContent.trim().length > 0); })()`,
        csig: `(${TL("document.querySelector('main.thread-view')")} || []).map((r) => ${TS('r.textContent')}).join('|')`,
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
      return { pred: `[...document.querySelectorAll('#timeline .timeline-virtual-row')].some((r) => r.textContent.includes(${q(text)}))`, cpredSame: true, csig: `[...document.querySelectorAll('#timeline .msg.user .body')].filter((b) => b.textContent.includes(${q(text)})).map((b) => b.textContent.trim()).join('|')`, go: () => h.key('Enter'), cap: 4000,
        undo: async () => { await h.idle(2500); } };
    } },
  { id: 'esc-back', what: 'Esc closes Settings back to the thread',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S) => {
      await h.click('button.gear'); await h.page.waitForSelector('#settings-content'); await h.idle(500);
      return { pred: threadSeen(S.B), confirm: threadShown(S.B), cpred: threadContent(S.B), csig: threadSig(S.B), go: () => h.key('Escape'), undo: () => openThread(h, S.B) };
    } },
  { id: 'resize', what: 'window resize 1400 -> 1100 wide (and back)',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h, S, i) => {
      const w = i % 2 ? 1400 : 1100;
      return { pred: `innerWidth === ${w}`, go: async () => { await h.page.evaluate(() => { window.__c2r.t0 = performance.now(); window.__c2r.inputType = 'resize'; }); await h.page.setViewportSize({ width: w, height: 900 }); } };
    }, webOnly: true },
];

// Phone (iPhone 15 web, form=phone): the list and the thread are separate screens; Back is [data-ph-back]
const phBack = async (h) => { const b = h.page.locator('[data-ph-back]').filter({ visible: true }); if (await b.count()) { await h.click('[data-ph-back]'); await sleep(800); } };
const PH_VIEWS = 'summary[aria-label="Views"]';
const phList = `!document.querySelector('main.thread-view') && ${visible('.thread')}`;
export const PHONE = [
  { id: 'ph-thread-open', what: 'phone: tap a thread in the list -> the thread',
    prep: async (h) => { await phBack(h); },
    step: async (h, S, i) => { const to = i % 2 ? S.A : S.B; await ensureRow(h, to); return { pred: threadSeen(to), confirm: threadShown(to), cpred: threadContent(to), csig: threadSig(to), content: '#timeline', go: () => h.click(row(to)), undo: phBack }; } },
  { id: 'ph-back', what: 'phone: Back from a thread to the list',
    prep: async () => {},
    step: async (h, S) => { await openThread(h, S.B); return { pred: phList, go: () => h.click('[data-ph-back]') }; } },
  { id: 'ph-model-picker', what: 'phone: model picker open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('.model-menu'), content: '.model-menu', ...popContent('.model-menu', 'button.row'), go: () => h.click('button.model-chip'), undo: escClose('.model-menu') }) },
  { id: 'ph-views-menu', what: 'phone: the thread header Views menu open',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => ({ pred: visible('details[open] button.tab[data-on-click="view-kind"]'), go: () => h.click(PH_VIEWS), undo: escClose('details[open] button.tab[data-on-click="view-kind"]') }) },
  { id: 'ph-palette', what: 'phone: Views -> Commands (the second tap measured)',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => { await h.click(PH_VIEWS); await sleep(500);
      return { pred: visible('.workspace-palette'), content: '.workspace-palette', go: () => h.click('details[open] button.tab[data-on-click="ws-palette-open"]'), undo: escClose('.workspace-palette') }; } },
  { id: 'ph-tool-fold', what: 'phone: expand / collapse a tool-call group',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: (h, S, i) => foldStep(h, S, i) },
  { id: 'ph-type-key', what: 'phone: composer keystroke -> echo',
    prep: async (h, S) => { await openThread(h, S.B); await h.click('#composer'); await h.idle(300); },
    step: async (h, S, i) => { const v = await h.page.evaluate(() => document.querySelector('#composer').value); const ch = 'abcdefghij'[i % 10];
      return { pred: `document.querySelector('#composer').value === ${q(v + ch)}`, go: () => h.type(ch), quiet: 200 }; } },
  { id: 'ph-send', what: 'phone: send -> my bubble',
    prep: async (h, S) => { await openThread(h, S.D); },
    step: async (h, S, i) => { const text = `ping ${Date.now() % 100000} ${i}`; await h.click('#composer'); await h.page.keyboard.type(text); await h.idle(300);
      return { pred: `[...document.querySelectorAll('#timeline .timeline-virtual-row')].some((r) => r.textContent.includes(${q(text)}))`, go: () => h.click('button.primary[data-on-click="send"]'), cap: 4000, undo: async () => { await h.idle(2500); } }; } },
  ...[['Files', 'fm'], ['Diff', 'diff'], ['Schematic', 'schematic'], ['3D', '3d']].map(([name, k]) => ({ id: 'ph-viewer-' + k, what: `phone: Views -> ${name} (the second tap measured)`,
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => { await h.click(PH_VIEWS); await sleep(500);
      return { pred: visible('aside#viewer'), content: 'aside#viewer', ...VIEWER_CONTENT[k], go: () => h.click(`details[open] button.tab[aria-label=${q(name)}]`),
        undo: async (hh) => { await phBack(hh); await sleep(500); if (await hh.page.evaluate(() => !document.querySelector('main.thread-view'))) await openThread(hh, hh.S.B); } }; } })),
  { id: 'ph-viewer-board', what: 'phone: Views -> Board (the second tap measured)',
    prep: async (h, S) => { await openThread(h, S.B); },
    step: async (h) => { await h.click(PH_VIEWS); await sleep(500);
      return { pred: visible('aside#viewer'), content: 'aside#viewer', ...VIEWER_CONTENT.board, go: () => h.click('details[open] button.tab[data-value="board"]'),
        undo: async (hh) => { await phBack(hh); await sleep(500); if (await hh.page.evaluate(() => !document.querySelector('main.thread-view'))) await openThread(hh, hh.S.B); } }; } },
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

const tabSel = (name) => `nav.workspace-bar button.tab[aria-label=${q(name)}]`;
// open a viewer (unmeasured) and wait for its content
async function openViewer(h, name, k) {
  if (!(await h.page.evaluate(`!!${VW}?.classList.contains(${q(k === 'fm' ? 'files' : k)})`))) await h.click(tabSel(name));
  await h.page.waitForFunction(VIEWER_CONTENT[k].cpred, null, { timeout: 20000 }).catch(() => {});
  await h.idle(600);
}
// Files back to the project root (Up until no folder in the crumbs)
async function fmRoot(h) {
  for (let i = 0; i < 6; i++) {
    const deep = await h.page.evaluate(() => document.querySelectorAll('aside#viewer .fm-bar .crumb').length > 1);
    if (!deep) break;
    await h.click('aside#viewer button[data-on-click="fm-up"]'); await sleep(700);
  }
  await h.page.waitForFunction(`(${VW}?.querySelectorAll('.fm-item').length || 0) > 0`, null, { timeout: 15000 }).catch(() => {});
  await h.idle(500);
}
