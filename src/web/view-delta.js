// A view's keyed delta (the hub's src/server/thin-delta.bend): ops that turn
// a body the client holds into the hub's new one. Applied copy on write:
// every subtree no op reaches is the base's own object, so a renderer may
// key on identity. Throws on anything malformed; the caller then asks for
// the whole view again.
const object = x => x !== null && typeof x === 'object' && !Array.isArray(x);
const MAX_OPS = 20000, MAX_DEPTH = 96;

function step(node, key) {
  if (typeof key === 'string') {
    if (!object(node) || !Object.hasOwn(node, key)) throw Error('Delta path');
    return node[key];
  }
  if (!Array.isArray(node) || !Number.isSafeInteger(key) || key < 0 || key >= node.length) throw Error('Delta path');
  return node[key];
}
function put(node, key, value) {
  if (typeof key === 'string') return { ...node, [key]: value };
  const copy = node.slice();copy[key] = value;return copy;
}
function rebuild(old, specs) {
  if (!Array.isArray(old) || !Array.isArray(specs)) throw Error('Delta array');
  return specs.map(s => {
    if (!Array.isArray(s) || s.length !== 2) throw Error('Delta spec');
    if (s[0] === 1) return s[1];
    if (s[0] !== 0 || !Number.isSafeInteger(s[1]) || s[1] < 0 || s[1] >= old.length) throw Error('Delta spec');
    return old[s[1]];
  });
}
// Each op copies only the nodes on its path that the delta has not copied
// already (fresh), so a delta of n ops over a path of depth d costs O(n·d).
function at(node, path, i, leaf, fresh) {
  if (i === path.length) return leaf(node);
  const child = at(step(node, path[i]), path, i + 1, leaf, fresh);
  if (fresh.has(node)) { node[path[i]] = child;return node; }
  const copy = put(node, path[i], child);fresh.add(copy);return copy;
}
// draft: the selected thread's draft the request carried; ["t", path] puts
// it back (the hub never sends a draft echo's text)
export function applyViewDelta(base, ops, draft = null) {
  if (!Array.isArray(ops) || ops.length > MAX_OPS) throw Error('Delta ops');
  const fresh = new WeakSet();
  let body = base;
  for (const op of ops) {
    if (!Array.isArray(op) || !Array.isArray(op[1]) || op[1].length > MAX_DEPTH) throw Error('Delta op');
    const [kind, path, value] = op;
    if (kind === 't') {
      if (op.length !== 2 || typeof draft !== 'string' || !draft) throw Error('Delta draft');
      body = at(body, path, 0, () => draft, fresh);continue;
    }
    if (op.length !== 3) throw Error('Delta op');
    if (kind === 's') body = at(body, path, 0, () => value, fresh);
    else if (kind === 'a') body = at(body, path, 0, old => { const a = rebuild(old, value);fresh.add(a);return a; }, fresh);
    else throw Error('Delta op');
  }
  return body;
}

// The transport's side (src/server/thin-view-delta.bend). A link keeps the
// last bodies it received by token; a request offers the newest, and a
// reply is either whole or the ops from the body it names.
const FIELDS = ['shown', 'session', 'window', 'overlay'];
export const DELTA_BASES = 8;
const token = x => typeof x === 'string' && /^[1-9][0-9]{0,15}$/.test(x);
export function deltaOffer(bases) {
  return { version: 1, token: bases?.newest || '' };
}
function bodyOf(r) {
  const body = {};
  for (const k of FIELDS) if (Object.hasOwn(r, k)) body[k] = r[k];
  return body;
}
// The full reply from a delta reply, and the body to keep. Throws when the
// reply is malformed or names a body this link does not hold.
export function deltaPicture(r, input, bases) {
  const d = r.viewDelta;
  if (!object(d) || d.version !== 1 || !token(d.token) || typeof d.base !== 'string' || input?.viewDelta?.version !== 1
      || d.token !== String(input.requestRevision)) throw Error('Invalid view delta');
  const rest = { ...r };delete rest.viewDelta;
  if (!Object.hasOwn(d, 'ops')) {
    if (d.base !== '' || !Object.hasOwn(r, 'shown')) throw Error('Invalid whole view');
    return { result: rest, token: d.token, body: bodyOf(rest) };
  }
  if (d.base !== input.viewDelta.token || !bases?.map.has(d.base) || FIELDS.some(k => Object.hasOwn(r, k))) throw Error('Unacknowledged view delta');
  const session = input.session, draft = session && typeof session.sel === 'string' && session.drafts ? session.drafts[session.sel] : null;
  const body = applyViewDelta(bases.map.get(d.base), d.ops, typeof draft === 'string' ? draft : null);
  if (!object(body) || !Object.hasOwn(body, 'shown')) throw Error('Invalid view delta body');
  return { result: { ...rest, ...body }, token: d.token, body };
}
export function deltaKeep(bases, picture) {
  bases.map.set(picture.token, picture.body);bases.newest = picture.token;
  while (bases.map.size > DELTA_BASES) bases.map.delete(bases.map.keys().next().value);
}
export function deltaBases() { return { map: new Map(), newest: '' }; }
