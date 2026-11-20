// Standalone browser pairing page (served at / when unpaired).
// Strict CSP: zero inline scripts, zero credentials in URL or client storage.
import { t } from "./i18n";
import { renderQrSvg } from "./qr";

interface PairRequestResponse {
  id?: string;
  code?: string;
  checkCode?: string;
  expiresAt?: number;
  expiresIn?: number;
  pollMs?: number;
  error?: string;
}

interface PairPollResponse {
  state?: string;
  status?: string;
  approved?: boolean;
  error?: string;
}

function defaultDeviceName(): string {
  const ua = navigator.userAgent || "";
  const browser =
    ua.includes("Firefox") ? "Firefox" :
    ua.includes("Edg/") ? "Edge" :
    ua.includes("Chrome") ? "Chrome" :
    ua.includes("Safari") ? "Safari" : "Browser";
  const os =
    ua.includes("iPhone") ? "iPhone" :
    ua.includes("iPad") ? "iPad" :
    ua.includes("Android") ? "Android" :
    ua.includes("Mac OS") ? "macOS" :
    ua.includes("Windows") ? "Windows" :
    ua.includes("Linux") ? "Linux" : "";
  return os ? `${browser} on ${os}` : browser;
}

function formatCode(raw: string): string {
  const digits = raw.replace(/\s+/g, "");
  if (digits.length === 6) return `${digits.slice(0, 3)} ${digits.slice(3)}`;
  return digits;
}

function formatRemaining(ms: number): string {
  const s = Math.max(0, Math.ceil(ms / 1000));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${String(r).padStart(2, "0")}`;
}

function mount(): void {
  const root = document.getElementById("pair-root");
  if (!root) return;

  // Follow saved theme preference (non-sensitive UI pref only)
  try {
    if (localStorage.getItem("bp.theme") === "light") {
      document.documentElement.classList.add("light");
    }
  } catch {
    // ignore storage errors
  }

  const card = document.createElement("main");
  card.className = "bp-pair-card";
  card.setAttribute("data-testid", "pair-card");

  const brand = document.createElement("div");
  brand.className = "bp-pair-brand";
  const logo = document.createElement("img");
  logo.src = "/assets/favicon.svg";
  logo.alt = "Backplane";
  logo.className = "bp-pair-logo";
  const brandName = document.createElement("span");
  brandName.className = "bp-pair-title";
  brandName.textContent = "Backplane";
  brand.append(logo, brandName);

  const subtitle = document.createElement("p");
  subtitle.className = "bp-pair-sub";
  subtitle.textContent = "Pair this browser with your hub. Approve the 6-digit code in Backplane › Settings › Devices.";

  const form = document.createElement("form");
  form.className = "bp-pair-form";

  const label = document.createElement("label");
  label.className = "bp-pair-label";
  label.htmlFor = "bp-device-name";
  label.textContent = "Browser name";

  const input = document.createElement("input");
  input.id = "bp-device-name";
  input.type = "text";
  input.maxLength = 64;
  input.autocomplete = "off";
  input.className = "bp-input";
  input.value = defaultDeviceName();
  input.setAttribute("data-testid", "pair-name");

  const submit = document.createElement("button");
  submit.type = "submit";
  submit.className = "primary bp-pair-btn";
  submit.textContent = "Request pairing";
  submit.setAttribute("data-testid", "pair-request-btn");

  form.append(label, input, submit);

  const pendingBox = document.createElement("div");
  pendingBox.className = "bp-pair-pending bp-hidden";
  pendingBox.setAttribute("data-testid", "pair-pending");

  const codeLabel = document.createElement("div");
  codeLabel.className = "bp-pair-code-label";
  codeLabel.textContent = "Comparison code";

  const codeEl = document.createElement("div");
  codeEl.className = "bp-pair-code";
  codeEl.setAttribute("data-testid", "pair-code");

  const metaEl = document.createElement("div");
  metaEl.className = "bp-pair-meta";

  const actions = document.createElement("div");
  actions.className = "bp-pair-actions";
  const cancelBtn = document.createElement("button");
  cancelBtn.type = "button";
  cancelBtn.textContent = t("common.cancel", "Cancel");
  actions.append(cancelBtn);

  pendingBox.append(codeLabel, codeEl, metaEl, actions);

  const statusEl = document.createElement("div");
  statusEl.className = "bp-pair-status";
  statusEl.setAttribute("role", "status");
  statusEl.setAttribute("data-testid", "pair-status");

  // Full HTTPS address + scannable QR code sourced from hub GET /auth/web/info ({ address }),
  // never guessed from window.location.origin and never carrying credentials.
  const addrBox = document.createElement("div");
  addrBox.className = "bp-pair-addr bp-hidden";

  const renderHubAddress = (rawAddress: string) => {
    const trimmed = rawAddress.trim();
    if (!trimmed) return;
    const fullUrl = trimmed.startsWith("https://") ? trimmed : `https://${trimmed}`;
    if (!fullUrl.startsWith("https://") || fullUrl.length <= "https://".length) return;
    const addrMeta = document.createElement("div");
    addrMeta.className = "bp-pair-addr-meta";
    const addrLbl = document.createElement("div");
    addrLbl.className = "bp-pair-code-label";
    addrLbl.textContent = "Hub address";
    const addrUrl = document.createElement("code");
    addrUrl.className = "bp-pair-addr-url";
    addrUrl.textContent = fullUrl;
    addrUrl.setAttribute("data-testid", "pair-address");
    const copyAddr = document.createElement("button");
    copyAddr.type = "button";
    copyAddr.className = "bp-pair-copy";
    const copyLbl = t("common.copy", "Copy");
    copyAddr.textContent = copyLbl;
    copyAddr.addEventListener("click", () => {
      navigator.clipboard?.writeText(fullUrl).then(() => {
        copyAddr.textContent = "Copied";
        window.setTimeout(() => { copyAddr.textContent = copyLbl; }, 1200);
      }).catch(() => {});
    });
    addrMeta.append(addrLbl, addrUrl, copyAddr);

    const frag = document.createDocumentFragment();
    frag.appendChild(addrMeta);
    const qrSvg = renderQrSvg(fullUrl);
    if (qrSvg) {
      const qrWrap = document.createElement("div");
      qrWrap.className = "bp-pair-qr";
      qrWrap.setAttribute("data-testid", "pair-qr");
      qrWrap.appendChild(qrSvg);
      frag.appendChild(qrWrap);
    }
    addrBox.replaceChildren(frag);
    addrBox.classList.remove("bp-hidden");
  };

  void fetch("/auth/web/info", { credentials: "same-origin" })
    .then(async (res) => {
      if (!res.ok) return;
      const data = (await res.json()) as { address?: string };
      if (typeof data.address === "string") renderHubAddress(data.address);
    })
    .catch(() => {});

  card.append(brand, subtitle, form, pendingBox, statusEl, addrBox);
  root.appendChild(card);

  let pollTimer: number | null = null;
  let tickTimer: number | null = null;
  let pollAbort: AbortController | null = null;
  let attemptSeq = 0;
  let expiresAt = 0;
  let pollIntervalMs = 150;
  let netBackoffMs = 1000;

  function stopTimers(): void {
    attemptSeq++;
    if (pollAbort) {
      pollAbort.abort();
      pollAbort = null;
    }
    if (pollTimer !== null) {
      window.clearTimeout(pollTimer);
      pollTimer = null;
    }
    if (tickTimer !== null) {
      window.clearInterval(tickTimer);
      tickTimer = null;
    }
  }

  function showForm(msg = "", isError = false): void {
    stopTimers();
    form.classList.remove("bp-hidden");
    pendingBox.classList.add("bp-hidden");
    submit.disabled = false;
    statusEl.textContent = msg;
    statusEl.className = isError ? "bp-pair-status bp-bad" : "bp-pair-status";
  }

  function updateCountdown(): void {
    const rem = expiresAt - Date.now();
    if (rem <= 0) {
      showForm("Pairing code expired. Request a new code to try again.", true);
      return;
    }
    metaEl.textContent = `Approve on desktop or paired phone · expires in ${formatRemaining(rem)}`;
  }

  async function pollOnce(seq: number): Promise<void> {
    if (seq !== attemptSeq) return;
    if (expiresAt > 0 && Date.now() >= expiresAt) {
      showForm("Pairing code expired. Request a new code to try again.", true);
      return;
    }
    const ctrl = new AbortController();
    pollAbort = ctrl;
    try {
      const res = await fetch("/auth/web/poll", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: "{}",
        signal: ctrl.signal,
      });
      if (seq !== attemptSeq) return;
      netBackoffMs = 1000;
      if (res.status === 202) {
        pollTimer = window.setTimeout(() => { void pollOnce(seq); }, 150);
        return;
      }
      if (res.status === 200 || res.status === 204) {
        let data: PairPollResponse = {};
        if (res.status === 200) {
          try { data = (await res.json()) as PairPollResponse; } catch { /* empty */ }
        }
        if (seq !== attemptSeq) return;
        const st = (data.state || data.status || "").toLowerCase();
        if (res.status === 204 || data.approved === true || st === "approved" || st === "ok") {
          stopTimers();
          statusEl.textContent = "Approved. Opening Backplane…";
          statusEl.className = "bp-pair-status bp-ok";
          window.location.replace("/");
          return;
        }
        if (st === "denied" || st === "rejected") {
          showForm("Pairing request was denied.", true);
          return;
        }
        if (st === "expired") {
          showForm("Pairing code expired. Request a new code to try again.", true);
          return;
        }
        pollTimer = window.setTimeout(() => { void pollOnce(seq); }, 150);
        return;
      }
      if (res.status === 429) {
        const retryAfter = Number(res.headers.get("Retry-After") || "5");
        const delay = Math.max(2000, (Number.isFinite(retryAfter) ? retryAfter : 5) * 1000);
        pollTimer = window.setTimeout(() => { void pollOnce(seq); }, delay);
        return;
      }
      if (res.status === 401 || res.status === 403 || res.status === 404 || res.status === 410) {
        let st = "";
        try {
          const data = (await res.json()) as PairPollResponse;
          st = (data.state || data.status || data.error || "").toLowerCase();
        } catch { /* ignore */ }
        if (seq !== attemptSeq) return;
        if (st === "denied" || st === "rejected" || res.status === 403) {
          showForm("Pairing request was denied.", true);
        } else {
          showForm("Pairing code expired. Request a new code to try again.", true);
        }
        return;
      }
      const delay = netBackoffMs;
      netBackoffMs = Math.min(10_000, Math.round(netBackoffMs * 1.8));
      pollTimer = window.setTimeout(() => { void pollOnce(seq); }, delay);
    } catch {
      if (seq !== attemptSeq) return;
      const delay = netBackoffMs;
      netBackoffMs = Math.min(10_000, Math.round(netBackoffMs * 1.8));
      pollTimer = window.setTimeout(() => { void pollOnce(seq); }, delay);
    } finally {
      if (pollAbort === ctrl) pollAbort = null;
    }
  }

  cancelBtn.addEventListener("click", () => {
    showForm("Cancelled.");
  });

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const name = input.value.trim() || defaultDeviceName();
    submit.disabled = true;
    statusEl.textContent = "Requesting pairing code…";
    statusEl.className = "bp-pair-status";
    try {
      const res = await fetch("/auth/web/request", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
      });
      if (!res.ok) {
        let errText = `Pairing request failed (${res.status})`;
        try {
          const body = (await res.json()) as PairRequestResponse;
          if (body.error) errText = body.error;
        } catch { /* ignore */ }
        if (res.status === 429) errText = "Too many pairing attempts. Wait a moment and try again.";
        showForm(errText, true);
        return;
      }
      const data = (await res.json()) as PairRequestResponse;
      const code = data.code || data.checkCode || "";
      if (!code) {
        showForm("Hub did not return a comparison code.", true);
        return;
      }
      const now = Date.now();
      expiresAt = data.expiresAt
        ? (data.expiresAt < 10_000_000_000 ? data.expiresAt * 1000 : data.expiresAt)
        : now + (data.expiresIn ? data.expiresIn * 1000 : 120_000);
      stopTimers();
      const seq = attemptSeq;
      netBackoffMs = 1000;
      pollIntervalMs = data.pollMs && data.pollMs >= 100 ? data.pollMs : 150;

      codeEl.textContent = formatCode(code);
      form.classList.add("bp-hidden");
      pendingBox.classList.remove("bp-hidden");
      statusEl.textContent = "";
      updateCountdown();
      tickTimer = window.setInterval(updateCountdown, 1000);
      pollTimer = window.setTimeout(() => { void pollOnce(seq); }, pollIntervalMs);
    } catch {
      showForm("Could not reach the hub. Check your tailnet connection.", true);
    }
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", mount);
} else {
  mount();
}
