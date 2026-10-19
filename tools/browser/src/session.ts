// A browser engine handoff reloads documents, preserving their owned tabs and
// browser storage. Browser profiles stay separate and are never overwritten.
import type { Page } from 'playwright-core';

export type TabState = {
  url: string;
  origin: string;
  local: [string, string][];
  session: [string, string][];
  fields: { selector: string; value: string; checked: boolean; selected: number }[];
  scroll: [number, number];
};
export async function saveTab(p: Page): Promise<TabState> {
  const value = await p.evaluate(() => {
    const store = (s: Storage): [string, string][] => Object.keys(s).map(k => [k, s.getItem(k)!]);
    const path = (e: Element): string => {
      if (e.id) return '#' + CSS.escape(e.id);
      const parts: string[] = [];
      for (let at: Element | null = e; at && at !== document.documentElement; at = at.parentElement) {
        const siblings = at.parentElement ? [...at.parentElement.children].filter(q => q.tagName === at!.tagName) : [at];
        parts.unshift(at.tagName.toLowerCase() + ':nth-of-type(' + (siblings.indexOf(at) + 1) + ')');
      }
      return parts.join(' > ');
    };
    let local: [string, string][] = [], session: [string, string][] = [];
    try { local = store(localStorage); } catch {}
    try { session = store(sessionStorage); } catch {}
    // Do not persist passwords, hidden CSRF fields or file selections.
    const fields = [...document.querySelectorAll<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>('input:not([type=password]):not([type=hidden]):not([type=file]),textarea,select')].map(e => ({ selector: path(e), value: e.value, checked: 'checked' in e && e.checked, selected: 'selectedIndex' in e ? e.selectedIndex : -1 }));
    return { origin: location.origin, local, session, fields, scroll: [scrollX, scrollY] as [number, number] };
  });
  return { url: p.url(), ...value };
}
export async function restoreTab(p: Page, saved: TabState, timeout: number): Promise<void> {
  // Install storage before document scripts, then remove the hook. A permanent
  // Playwright init script would overwrite later user changes on every reload.
  const cdp = await p.context().newCDPSession(p);
  await cdp.send('Page.enable');
  const { identifier } = await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `(() => { const s = ${JSON.stringify(saved)};
      if (location.origin !== s.origin) return;
      for (const [k,v] of s.local) try { localStorage.setItem(k,v); } catch {}
      for (const [k,v] of s.session) try { sessionStorage.setItem(k,v); } catch {}
    })()`,
  });
  try { await p.goto(saved.url, { waitUntil: 'load', timeout }); }
  finally {
    await cdp.send('Page.removeScriptToEvaluateOnNewDocument', { identifier }).catch(() => {});
    await cdp.detach().catch(() => {});
  }
  await p.evaluate(s => {
    if (location.origin !== s.origin) return;
    for (const f of s.fields) {
      const e = document.querySelector<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>(f.selector);
      if (!e) continue;
      e.value = f.value;
      if ('checked' in e) e.checked = f.checked;
      if ('selectedIndex' in e && f.selected >= 0) e.selectedIndex = f.selected;
    }
    scrollTo(...s.scroll);
  }, saved);
}

export function defaultEngine(explicit: string | null, parent: number, display: string, platform = process.platform): 'chrome' | 'cef' {
  if (explicit !== null) {
    if (explicit !== 'chrome' && explicit !== 'cef') throw new Error('engine must be chrome or cef');
    return explicit;
  }
  return platform === 'linux' && Number.isSafeInteger(parent) && parent > 0 && !!display ? 'cef' : 'chrome';
}
