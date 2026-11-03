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
    if (!r.ok) { const j = await r.json().catch(()=>({}));return {ok: false, error: ['wrong_code','expired_code','used_code','rate_limited','storage_unavailable'].includes(j.error) ? j.error : 'pairing_required'}; }
    return {ok: true, origin: p.origin};
  } catch { return {ok: false, error: 'hub_unreachable'}; }
}
module.exports = {invitation, pair};
