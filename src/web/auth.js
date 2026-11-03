// Exchange pairing material once. The durable browser credential belongs in an
// HttpOnly cookie; fragments and legacy localStorage never enter socket URLs.
export async function pairBrowser({ location, history, storage, fetch, timeoutMs = 2000, report = () => {} }) {
  const fragment = new URLSearchParams(location.hash.slice(1));
  const invite = fragment.get("invite") ?? fragment.get("code");
  const supplied = invite ?? fragment.get("token");
  const legacy = storage.getItem("backplane-token");
  const token = supplied ?? legacy;
  if (supplied !== null) history.replaceState(null, "", location.pathname + location.search);
  if (!token) return true;
  if (!(invite ? /^(?:[0-9a-f]{32}|[A-Z2-9]{8})$/i : /^[0-9a-f]{32}$/).test(token)) { report("invalid_link");return false; }
  const controller = new AbortController();let timer;
  let response;
  try { response = await Promise.race([fetch(invite ? "/auth/enroll" : "/auth/pair", {
    method: "POST", credentials: "same-origin", cache: "no-store",
    headers: { Authorization: `Bearer ${token}` },
    signal: controller.signal,
  }), new Promise(resolve => { timer = setTimeout(() => { controller.abort();resolve(null); }, timeoutMs); })]); }
  catch { report("hub_unreachable");return false; }
  finally { clearTimeout(timer); }
  if (!response) { report("hub_unreachable");return false; }
  if (!response.ok) { let cause = "pairing_required";try { cause = (await response.json()).error || cause; } catch {} report(cause);return false; }
  storage.removeItem("backplane-token");
  return true;
}
