// A transport reference can restore only the immutable sidebar acknowledged
// by this request on its surviving ThinViews link. Never persisted or global.
const object = x => x !== null && typeof x === 'object' && !Array.isArray(x);
const token = x => typeof x === 'string' && /^[1-9][0-9]{0,9}$/.test(x) && Number(x) <= 2 ** 32 - 1;
const region = (n, tag) => object(n) && n.type === 'element' && n.tag === tag && n.key === 'sidebar';
const fields = (o, names) => object(o) && Object.keys(o).length === names.length && names.every(n => Object.hasOwn(o, n));

export function sidebarOffer(cache) { return { version: 1, token: cache?.token || '', exactContent: true }; }

export function sidebarPicture(result, input, base) {
  if (!Object.hasOwn(result, 'sidebarRegion')) return { result, cache: null };
  const r = result.sidebarRegion;
  if (!fields(r, ['version','token','base']) || r.version !== 1 || !token(r.token) || typeof r.base !== 'string'
      || input.sidebarRegion?.version !== 1 || result.nextTarget || !result.shown || result.remoteReady !== true)
    throw Error('Invalid sidebar region');
  if (r.base && (r.base !== r.token || r.base !== input.sidebarRegion.token || base?.token !== r.base))
    throw Error('Unacknowledged sidebar region');
  if (!r.base && r.token !== String(input.requestRevision)) throw Error('Invalid sidebar seed');
  let sidebars = 0, references = 0, node = null, remaining = 20000;
  const walk = (n, depth = 0) => {
    if (!object(n) || --remaining < 0 || depth >= 64) throw Error('Invalid sidebar picture');
    if (region(n, 'bp-sidebar-ref')) {
      if (++references !== 1 || !r.base || !fields(n, ['type','tag','key','attrs','kids'])
          || !Array.isArray(n.attrs) || n.attrs.length || !Array.isArray(n.kids) || n.kids.length
          || !region(base?.node, 'aside')) throw Error('Invalid sidebar reference');
      return base.node;
    }
    if (region(n, 'aside')) { sidebars++;node = n;return n; }
    if (n.type === 'text') return n;
    if (n.type !== 'element' || !Array.isArray(n.kids)) throw Error('Invalid sidebar tree');
    const kids = n.kids.map(k => walk(k, depth + 1));
    return kids.some((k, i) => k !== n.kids[i]) ? { ...n, kids } : n;
  };
  const shown = walk(result.shown);
  if (r.base ? references !== 1 || sidebars !== 0 : references !== 0 || sidebars !== 1)
    throw Error('Ambiguous sidebar region');
  return { result: shown === result.shown ? result : { ...result, shown }, cache: { token: r.token, node: r.base ? base.node : node } };
}

// Canonical sidebar v1 (server/canonical-sidebar.bend): a view too big for one
// reply used to carry the whole sidebar every time. A hub that says so in its
// greeting sends it once as a seed, then only ops from a sidebar this link
// acknowledged. The history outlives a change of thread (the sidebar is the
// same on every target) and dies with the link.
const MAX_NODES = 20000, MAX_DEPTH = 64;
const string = x => typeof x === 'string';
export const canonicalHistory = () => ({ entries: [] });
export function canonicalCapable(greeting) {
  const c = greeting?.canonicalSidebar;
  return greeting?.renderer === 'web' && object(c) && c.version === 1 && Number.isSafeInteger(c.sidebarBytes) && c.sidebarBytes > 0
    && c.sidebarBytes <= 2097152 && Number.isSafeInteger(c.bases) && c.bases > 0 && c.bases <= 3;
}
// the newest sidebar held, by request revision: a late reply never moves it back
export const canonicalNewest = history => history?.entries[0] || null;
export function canonicalOffer(entry) { return { version: 1, token: entry?.token || '', accountBinding: entry?.account || '' }; }
function bounded(node) {
  let left = MAX_NODES;
  const walk = (n, depth) => {
    if (--left < 0 || depth >= MAX_DEPTH || !object(n)) throw Error('Invalid canonical sidebar');
    if (n.type === 'text') return;
    if (n.type !== 'element' || !Array.isArray(n.kids)) throw Error('Invalid canonical sidebar');
    for (const k of n.kids) walk(k, depth + 1);
  };
  walk(node, 0);
}
// What this reply makes of the sidebar: {token, account, node, bytes}. Throws
// on anything malformed; `rest` is the reply as received without its node or
// ops, measured before anything is put back (the hub measured the same).
export function canonicalPicture(result, input, base, limits, measure, apply) {
  const c = result.canonicalSidebar, seed = object(c) && Object.hasOwn(c, 'node');
  if (!fields(c, ['version','token','base','accountBinding', seed ? 'node' : 'ops']) || c.version !== 1 || !token(c.token)
      || c.token !== String(input.requestRevision) || !string(c.base) || !string(c.accountBinding)
      || input.canonicalSidebar?.version !== 1 || c.accountBinding !== result.accountBinding) throw Error('Invalid canonical sidebar');
  const { node: _n, ops: _o, ...kept } = c;
  if (measure(JSON.stringify({ ...result, canonicalSidebar: kept })) > limits.replyBytes) throw Error('Expanded view limit');
  let node, bytes;
  if (seed) {
    if (c.base !== '' || !region(c.node, 'aside')) throw Error('Invalid canonical seed');
    node = c.node;
  } else {
    if (!c.base || c.base !== input.canonicalSidebar.token || base?.token !== c.base || base.account !== c.accountBinding
        || !Array.isArray(c.ops) || measure(JSON.stringify(c.ops)) > limits.sidebarBytes) throw Error('Unacknowledged canonical sidebar');
    node = apply(base.node, c.ops);
    if (node === base.node) bytes = base.bytes;
    if (!region(node, 'aside')) throw Error('Invalid canonical delta');
  }
  if (bytes === undefined) {
    bytes = measure(JSON.stringify(node));
    if (bytes > limits.sidebarBytes) throw Error('Expanded sidebar limit');
    bounded(node);
  }
  return { token: c.token, account: c.accountBinding, node, bytes };
}
// A validated sidebar, kept whether or not its reply reached the screen. A
// newer binding ends the older one's; an older reply never ends a newer one's.
export function canonicalKeep(history, entry, bases) {
  const newest = history.entries[0], n = Number(entry.token);
  if (newest && newest.account !== entry.account) {
    if (Number(newest.token) > n) return;
    history.entries = [];
  }
  history.entries = [entry, ...history.entries.filter(e => e.token !== entry.token)]
    .sort((a, b) => Number(b.token) - Number(a.token)).slice(0, bases);
}
