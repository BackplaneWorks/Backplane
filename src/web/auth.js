import { startingText } from "./reconnect.js";

// Exchange pairing material once. The durable browser credential belongs in an
// HttpOnly cookie; fragments and legacy localStorage never enter socket URLs.
export async function pairBrowser({ location, history, storage, fetch, timeoutMs = 2000, onWait = () => {}, sleep = ms => new Promise(r => setTimeout(r, ms)), random = Math.random, attempts = Infinity, unreachableMs = 150000, now = () => Date.now() }) {
  const fragment = new URLSearchParams(location.hash.slice(1));
  const supplied = fragment.get("token");
  const legacy = storage.getItem("backplane-token");
  const token = supplied ?? legacy;
  // Kept until the hub accepts it: a hub that is starting (503, building its
  // store) or not up yet must not cost the person their pairing link.
  if (supplied !== null && /^[0-9a-f]{32}$/.test(supplied)) { try { storage.setItem("backplane-token", supplied); } catch {} }
  if (supplied !== null) history.replaceState(null, "", location.pathname + location.search);
  if (!token) return true;
  if (!/^[0-9a-f]{32}$/.test(token)) return false;
  // A starting hub (503) is waited for as long as it says; one that does not
  // answer at all only for unreachableMs (a restart's window), then this
  // fails, and the kept token pairs on the next load.
  const since = now();
  for (let tried = 0, wait = 250; tried < attempts; tried += 1) {
    const controller = new AbortController();let timer;
    let response;
    try { response = await Promise.race([fetch("/auth/pair", {
      method: "POST", credentials: "same-origin", cache: "no-store",
      headers: { Authorization: `Bearer ${token}` },
      signal: controller.signal,
    }), new Promise(resolve => { timer = setTimeout(() => { controller.abort();resolve(null); }, timeoutMs); })]); }
    catch { response = null; }
    finally { clearTimeout(timer); }
    if (response?.ok) { storage.removeItem("backplane-token"); return true; }
    // a refusal (wrong or used token) is final; a link's token kept above goes
    if (response && response.status !== 503 && response.status !== 502 && response.status !== 504) {
      if (supplied !== null) storage.removeItem("backplane-token");
      return false;
    }
    // starting (503 + Retry-After) or unreachable: wait and ask again
    let ms, note = "";
    if (response?.status === 503) {
      const after = Number(response.headers?.get("retry-after"));
      ms = Number.isFinite(after) && after > 0 ? Math.min(30000, after * 1000) : 2000;
      try { note = startingText(JSON.parse((await response.text()).slice(0, 4096))); } catch {}
    } else {
      if (now() - since >= unreachableMs) return false;
      ms = Math.round(wait / 2 + random() * wait / 2);wait = Math.min(wait * 2, 2000);
    }
    onWait(note || "Waiting for the hub to pair this browser…", ms);
    await sleep(ms);
  }
  return false;
}
