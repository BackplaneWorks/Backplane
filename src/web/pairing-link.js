// Invitation parsing shared by pasted links, QR results and startup fragments.
// Never emit a supplied value in errors or logs.
export function pairingInput(text, base) {
  let raw = String(text).trim();
  if (raw.startsWith('backplane://')) raw = new URL(raw).searchParams.get('url') || '';
  const url = new URL(raw.includes('://') ? raw : 'http://' + raw, base);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || !url.hostname) throw Error('invalid_link');
  const f = new URLSearchParams(url.hash.slice(1));
  const kind = f.has('invite') ? 'invite' : f.has('code') ? 'code' : 'token';
  const secret = f.get(kind);
  if (!(kind === 'code' ? /^[A-Z2-9]{8}$/i : /^[a-f0-9]{32}$/).test(secret || '')) throw Error('invalid_link');
  return {origin: url.origin, kind, secret, hash: url.hash};
}
export function codeLink(address, code) {
  const compact = code.replace(/[ -]/g, '').toUpperCase();
  if (!/^[A-Z2-9]{8}$/.test(compact)) throw Error('wrong_code');
  const url = new URL(address.includes('://') ? address : 'http://' + address);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw Error('invalid_link');
  return url.origin + '/pair#code=' + compact;
}
export const pairingErrors = {
  wrong_code: 'That code is not correct. Check it and try again.',
  invalid_link: 'Paste the complete pairing link from Settings → Devices → Add a device.',
  expired_code: 'This invitation has expired. Create a new one on your paired machine.',
  used_code: 'This invitation has already been used. Create a new one for this device.',
  revoked_code: 'This invitation was revoked. Create a new one on your paired machine.',
  rate_limited: 'Too many attempts. Wait a minute before trying again.',
  hub_unreachable: 'Hub unreachable. Check that it is running and that you are on its LAN or tailnet.',
  pairing_required: 'Access was revoked or pairing is missing. Connect with a new invitation.',
  storage_unavailable: 'The hub could not save this pairing. Try again when its storage is available.',
  lan_unavailable: 'The hub could not listen on that address. Another program may be using its port.',
  lan_unknown: 'This computer no longer has that local network address. Create a new invitation.',
};
