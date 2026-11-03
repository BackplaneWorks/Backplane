'use strict';
// Parser errors are intentionally secret-free: an OS protocol URL may carry
// an invitation, and must never be interpolated into diagnostics.
function invitation(text) {
  try {
    if (typeof text !== 'string' || text.length > 8192) return null;
    let raw = text.trim();
    if (raw.startsWith('backplane://')) { const p = new URL(raw);if (p.hostname !== 'pair') return null;raw = p.searchParams.get('url') || ''; }
    const u = new URL(raw), f = new URLSearchParams(u.hash.slice(1));
    if (!['http:', 'https:'].includes(u.protocol) || u.username || u.password) return null;
    const kind = f.has('invite') ? 'invite' : f.has('code') ? 'code' : 'token', secret = f.get(kind);
    if (!(kind === 'code' ? /^[A-Z2-9]{8}$/ : /^[a-f0-9]{32}$/).test(secret || '')) return null;
    return {origin: u.origin, kind, secret};
  } catch { return null; }
}
async function pair({text, name, fetch}) {
  const p = invitation(text);if (!p) return {ok: false, error: 'invalid_link'};
  try {
    const r = await fetch(p.origin + (p.kind === 'token' ? '/auth/pair' : '/auth/enroll'), {
      method: 'POST', credentials: 'include', cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(8000),
      headers: {Authorization: 'Bearer ' + p.secret, 'X-Backplane-Name': String(name || ({linux:'Linux',darwin:'macOS',win32:'Windows'}[process.platform] || 'Device') + ' / Desktop').replace(/[^\x20-\x7e]/g, '').slice(0,80), 'X-Backplane-Client': 'native', 'X-Backplane-Scheme': new URL(p.origin).protocol.slice(0,-1)},
    });
    if (!r.ok) { const j = await r.json().catch(()=>({}));return {ok: false, error: ['wrong_code','expired_code','used_code','revoked_code','rate_limited','storage_unavailable'].includes(j.error) ? j.error : 'pairing_required'}; }
    return {ok: true, origin: p.origin};
  } catch { return {ok: false, error: 'hub_unreachable'}; }
}
// Whether a pairing link that came from outside the app (the OS's
// backplane:// handler, which any web page or message can open) is for one
// of the person's own hubs: this app's own service, the hub it is paired
// with, or one the own service knows as the owner's (its machines and
// verified links: POST /auth/known). That hub then pairs only with a link it
// issued and has not revoked. Any other origin is paired only when the
// person says so (main.cjs's pairExternal).
async function knownHub({text, local, saved, token, fetch}) {
  const p = invitation(text);if (!p || !local) return false;
  if (p.origin === local || p.origin === saved) return true;
  try {
    const r = await fetch(local + '/auth/known', {method: 'POST', cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(5000),
      headers: {Authorization: 'Bearer ' + token, 'X-Backplane-Origin': p.origin, 'X-Backplane-Client': 'native'}});
    if (!r.ok) return false;
    return (await r.json()).known === true;
  } catch { return false; }
}
module.exports = {invitation, pair, knownHub};
