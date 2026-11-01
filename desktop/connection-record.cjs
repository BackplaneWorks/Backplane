'use strict';
// Diagnostic wire: fixed metadata only. Never retain arbitrary reason/error text.
const PREFIX = '[bp-connection] ', LIMIT = 64, MAX_BYTES = 65536, TTL = 86400000;
const events = new Set(['created','open','greeting','ready','closing','closed','error','view-error','pagehide','deploy-reload','renderer-gone','window-close','app-quit','retry-discarded']);
const causes = new Set(['initial','retry','reconnect','replaced','auth-denied','transport','transport-error','heartbeat-expired','wake-expired','frame-limit','decode-error','greeting-timeout','client-setup','view-limit','view-refused','view-timeout','view-error','pagehide','deploy','renderer','app-quit','window-close','stale-retry']);
const reasons = new Set(['','server-shutdown','heartbeat-timeout','backpressure','protocol-error','deploy','redacted']);
const crashes = new Set(['clean-exit','abnormal-exit','killed','crashed','oom','launch-failed','integrity-failure','memory-eviction']);
function sanitize(x) {
  if (!x || typeof x !== 'object' || !events.has(x.event)) return null;
  const r = { event: x.event };
  for (const k of ['at','connection','frameAgeMs','heartbeatAgeMs','ageMs','lastFrameBytes','closeCode','reasonBytes','process','exitCode'])
    if (Number.isSafeInteger(x[k]) && x[k] >= 0 && x[k] <= Number.MAX_SAFE_INTEGER) r[k] = x[k];
  for (const k of ['wasClean','online']) if (typeof x[k] === 'boolean') r[k] = x[k];
  if (causes.has(x.cause)) r.cause = x.cause;
  if (['visible','hidden','prerender','unknown'].includes(x.visibility)) r.visibility = x.visibility;
  if (typeof x.page === 'string' && /^[a-f0-9]{16}$/.test(x.page)) r.page = x.page;
  if (typeof x.bundle === 'string' && /^(?:chunk-[\w-]+\.js)?$/.test(x.bundle) && x.bundle.length < 100) r.bundle = x.bundle;
  for (const k of ['generation','epoch']) if (typeof x[k] === 'string' && /^\d{1,15}$/.test(x[k])) r[k] = x[k];
  if (typeof x.closeReason === 'string') r.closeReason = reasons.has(x.closeReason) ? x.closeReason : 'redacted';
  if (crashes.has(x.rendererReason)) r.rendererReason = x.rendererReason;
  if (['Error','TypeError','RangeError','SyntaxError','DOMException'].includes(x.errorKind)) r.errorKind = x.errorKind;
  if (['reply','view','hb','view.changed','plot','term','shot','info','log','changes','delta','unknown'].includes(x.frameType)) r.frameType = x.frameType;
  if (['view-reply','ordinary-reply','unknown-reply','notification','unknown'].includes(x.frameClass)) r.frameClass = x.frameClass;
  return r;
}
function retained(rows, now) {
  return (Array.isArray(rows) ? rows.slice(-LIMIT) : []).map(sanitize).filter(x => x && Number.isSafeInteger(x.at) && now - x.at >= 0 && now - x.at <= TTL);
}
module.exports = { PREFIX, LIMIT, MAX_BYTES, TTL, sanitize, retained };
