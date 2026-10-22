// Platform HTTP IO only. The shared transport owns protocol validation.
import { freshEndpoint, jsonResponse, boundedText } from './http-cache.js';
export async function discoverViews({ fetch, validate, timeoutMs = 2000 }) {
  const abort = new AbortController();let timer;
  const limit = 16384;
  try {
    return await Promise.race([
      (async () => {
        const response = await fetch(freshEndpoint('/client/capabilities'), { credentials: 'same-origin', cache: 'no-store', signal: abort.signal });
        // A hub that is starting (503) or a proxy error is not a hub without
        // views: the caller tries again rather than settling for read-only.
        if (response.status === 503 || response.status === 502 || response.status === 504) return { supported: false, offline: true };
        if (response.status !== 200 || !jsonResponse(response)) return { supported: false };
        // a body that is malformed, too big or not JSON: a hub without views
        let capability;try { capability = JSON.parse(await boundedText(response, limit)); } catch { return { supported: false }; }
        return { supported: validate(capability) === true };
      })().catch(() => ({ supported: false, offline: true })),
      new Promise(resolve => { timer = setTimeout(() => { abort.abort();resolve({ supported: false, offline: true }); }, timeoutMs); }),
    ]);
  } finally { clearTimeout(timer); }
}
