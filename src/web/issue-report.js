
export const ISSUE_URL = 'https://github.com/BackplaneWorks/Backplane/issues/new';
export const ISSUE_TITLE = 'Backplane improvement';
export const MAX_ISSUE_URL = 8000;
export function osFamily(navigator = {}) {
  const platform = navigator.userAgentData?.platform || navigator.platform || '';
  if (/android/i.test(platform + ' ' + (navigator.userAgent || ''))) return 'Android';
  if (/iPhone|iPad|iPod/i.test(platform) || /Mac/i.test(platform) && navigator.maxTouchPoints > 1) return 'iOS';
  if (/win/i.test(platform)) return 'Windows';
  if (/mac/i.test(platform)) return 'macOS';
  if (/linux/i.test(platform)) return 'Linux';
  return 'unknown';
}
// Reconstruct export records: never serialize an arbitrary logger, raw perf
// table, account/context object, exception, CloseEvent or response body.
export function safeConnections(rows, now = Date.now()) {
  const events = new Set(['created','open','greeting','ready','closing','closed','error','view-error','deploy-reload','renderer-gone','window-close','app-quit','retry-discarded','latency']);
  const result = [];
  for (const x of (Array.isArray(rows) ? rows : []).slice(-64)) {
    if (!x || !events.has(x.event) || !Number.isSafeInteger(x.at) || now - x.at < 0 || now - x.at > 86400000) continue;
    const r = { event: x.event, at: x.at };
    for (const k of ['connection','frameAgeMs','heartbeatAgeMs','ageMs','lastFrameBytes','closeCode','durationMs'])
      if (Number.isSafeInteger(x[k]) && x[k] >= 0 && x[k] <= 1e12) r[k] = x[k];
    if (typeof x.wasClean === 'boolean') r.wasClean = x.wasClean;
    if (['frame.decode','connection.ready','view.reply'].includes(x.metric)) r.metric = x.metric;
    if (['transport','transport-error','initial','retry','reconnect','heartbeat-expired','frame-limit','decode-error','greeting-timeout','view-timeout','view-limit','view-refused','deploy','window-close','app-quit','stale-retry'].includes(x.cause)) r.cause = x.cause;
    if (['Error','TypeError','RangeError','SyntaxError','DOMException'].includes(x.errorKind)) r.errorKind = x.errorKind;
    result.push(r);
  }
  return result.slice(-24);
}
const metrics = new Set(['hub.rpc','hub.mcp','hub.tick','hub.join','hub.mobile_join','hub.join_here','hub.leave','hub.thin-read','hub.agent_line','hub.agent_gone','hub.heart','hub.commit','hub.push','hub.push_bin','hub.step.busy','hub.step.far','hub.step.outs','hub.step.pend','hub.step.commit','boot.store','boot.replay','boot.files','boot.kicad','boot.cfg','boot.thin','boot.mail','boot.origin','boot.helpers','boot.adopt','boot.listen','thin.warm','mail.warm','hub.clients','hub.agents','hub.busy','connection.join','connection.mobile_join','connection.leave','connection.drop','connection.connect','connection.disconnect','connection.reconnect','connection.failed']);
const errors = new Set(['store_write','mail_append','disk_full','io','timeout','protocol','agent_exit','log_error','log_warn']);
export function safeHubDiagnostics(value) {
  // The server exports only closed records; apply the allowlist again at the
  // publication boundary, including data read from a modified local file.
  const rows = Array.isArray(value?.records) ? value.records : Array.isArray(value?.events) ? value.events : [];
  const out = [];
  for (const x of rows.slice(-48)) {
    if (!x || typeof x !== 'object') continue;
    const r = {};
    if (['span','latency','error','connection','sample'].includes(x.event)) r.event = x.event;
    if (metrics.has(x.name)) r.name = x.name;
    if (errors.has(x.code)) r.code = x.code;
    for (const k of ['at','ms','durationMs','count','maxMs','meanMs','p90Ms','value','us'])
      if (typeof x[k] === 'number' && Number.isFinite(x[k]) && x[k] >= 0 && x[k] <= 1e12) r[k] = x[k];
    if (r.event && (r.name || r.code)) out.push(r);
  }
  return out;
}
export function diagnosticReport({ connections = [], hub } = {}) {
  return JSON.stringify({ schema: 1, connection: safeConnections(connections), hub: safeHubDiagnostics(hub) }, null, 2);
}
export function issueLink(body) {
  const url = ISSUE_URL + '?title=' + encodeURIComponent(ISSUE_TITLE) + '&body=' + encodeURIComponent(body).replace(/[!'()*]/g, c => '%' + c.charCodeAt(0).toString(16).toUpperCase());
  return url.length <= MAX_ISSUE_URL ? url : '';
}
// Both public preview and the final URL use the very same immutable body.
// Opening GitHub is a browser handoff; no GitHub API or account lookup exists.
export class IssueHandoff {
  constructor({ save, open }) { this.save = save;this.open = open;this.busy = false; }
  async send(draft, preview) {
    if (this.busy || draft.issueOpened) return false;
    this.busy = true;
    try {
      if (!preview?.body || preview.body !== draft.issueBody) throw Error('Review the updated issue before opening GitHub.');
      // Persist BEFORE opening a new window. If a write fails, nothing leaves.
      draft.issueOpened = true;
      try { await this.save(draft); } catch (error) { draft.issueOpened = false;throw error; }
      try { await this.open(preview.url || ISSUE_URL); }
      catch (error) { draft.issueOpened = false;await this.save(draft);throw error; }
      return true;
    } finally { this.busy = false; }
  }
}
