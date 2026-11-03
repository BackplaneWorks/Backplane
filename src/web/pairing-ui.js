// Connection-boundary UI: available before a client can ask the hub for views.
// Invitation material stays in this closure/DOM, never in Ui, logs or caches.
import jsQR from './vendor/jsqr.cjs';
import {pairingInput, codeLink, pairingErrors} from './pairing-link.js';
export function installPairingUI({document, location, fetch, onPaired, navigator = globalThis.navigator}) {
  let panel, camera, scanGeneration = 0, expiryTimer, pairingShown = false;
  const element = (tag, text, parent, attrs = {}) => {
    const node = document.createElement(tag);if (text) node.textContent = text;
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
    parent?.append(node);return node;
  };
  function close() {
    ++scanGeneration;camera?.getTracks().forEach(t => t.stop());camera = null;
    clearInterval(expiryTimer);panel?.remove();panel = null;
    document.getElementById('root')?.removeAttribute('inert');
    document.body.classList.remove('pairing-pending');pairingShown = false;
  }
  function frame(title, lead, dismissible) {
    close();panel = element('main', '', document.body, {class: 'pairing-screen', 'aria-label': title});
    document.getElementById('root')?.setAttribute('inert', '');
    const card = element('section', '', panel, {class: 'pairing-card'});
    element('p', 'BACKPLANE / CONNECT', card, {class: 'pairing-brand'});
    element('h1', title, card);element('p', lead, card, {class: 'pairing-lead'});
    if (dismissible) button(card, 'Back', close);
    return card;
  }
  function button(parent, text, click) { const b = element('button', text, parent, {type: 'button'});b.addEventListener('click', click);return b; }
  function input(parent, label, placeholder, type = 'text') {
    const l = element('label', label, parent), i = element('input', '', l, {placeholder, type, autocomplete: 'off', autocapitalize: 'none', spellcheck: 'false'});
    return i;
  }
  function error(target, cause) { target.textContent = pairingErrors[cause] || 'Connection could not be completed. Try again.'; }
  async function enroll(secret, name) {
    const abort = new AbortController(), timer = setTimeout(() => abort.abort(), 8000);
    try {
      const r = await fetch('/auth/enroll', {method: 'POST', cache: 'no-store', credentials: 'same-origin', redirect: 'error', signal: abort.signal,
        headers: {Authorization: 'Bearer ' + secret, 'X-Backplane-Name': name.replace(/[^\x20-\x7e]/g, '').slice(0, 80)}});
      const result = await r.json();if (!r.ok || result.error) throw Error(result.error || 'pairing_required');
      close();onPaired();
    } catch (e) { throw Error(pairingErrors[e.message] ? e.message : 'hub_unreachable'); }
    finally { clearTimeout(timer); }
  }
  function show({repair = false, cause = '', pending = null} = {}) {
    const card = frame(repair ? 'Re-pair this device' : 'Connect to your Backplane hub',
      'On your paired computer, open Settings → Devices → Add a device. Choose how to connect below.', false);
    pairingShown = true;
    const name = input(card, 'Device name', 'My laptop');name.value = /Android|iPhone|iPad/.test(navigator?.userAgent || '') ? 'Phone or tablet' : 'Computer';
    const status = element('p', '', card, {role: 'status', 'aria-live': 'polite', class: 'pairing-status'});if (cause) error(status, cause);
    const link = input(card, 'Paste a pairing link', 'https://your-hub/pair#invite=…');
    async function connect(text) {
      status.textContent = 'Connecting…';
      try {
        const p = pairingInput(text, location.origin);
        if (globalThis.window?.backplaneDesktop?.pair) { const result = await window.backplaneDesktop.pair(text, name.value);if (!result?.ok) throw Error(result?.error || 'hub_unreachable');return; }
        if (p.origin !== location.origin) { location.assign(p.origin + '/pair' + p.hash);return; }
        if (p.kind === 'token') { location.assign('/pair' + p.hash);return; }
        await enroll(p.secret, name.value);
      } catch (e) { error(status, e.message); }
    }
    button(card, 'Connect with link', () => connect(link.value));
    link.addEventListener('keydown', e => { if (e.key === 'Enter') void connect(link.value); });
    const scanStatus = element('p', '', card, {class: 'pairing-help'});
    button(card, 'Scan a QR code', async () => {
      if (!navigator?.mediaDevices?.getUserMedia) { scanStatus.textContent = 'Camera scanning is unavailable in this browser. Use your phone’s camera to open the QR link, or paste the link below.';return; }
      let video;const generation = ++scanGeneration;
      try {
        camera?.getTracks().forEach(t => t.stop());camera = await navigator.mediaDevices.getUserMedia({video: {facingMode: 'environment'}});
        if (generation !== scanGeneration) { camera.getTracks().forEach(t => t.stop());return; }
        video = element('video', '', card, {autoplay: '', playsinline: '', class: 'pairing-camera'});video.srcObject = camera;await video.play();
        button(card, 'Stop scanning', () => { ++scanGeneration;camera?.getTracks().forEach(t => t.stop());video.remove(); });
        const detector = globalThis.BarcodeDetector ? new globalThis.BarcodeDetector({formats: ['qr_code']}) : null;
        const canvas = document.createElement('canvas'), ctx = canvas.getContext('2d', {willReadFrequently: true});
        const next = async () => {
          if (generation !== scanGeneration) return;
          try {
            let codes;
            if (detector) codes = await detector.detect(video);
            else {
              canvas.width = Math.min(800, video.videoWidth);canvas.height = Math.round(video.videoHeight * canvas.width / video.videoWidth);
              ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
              const frame = ctx.getImageData(0, 0, canvas.width, canvas.height), result = jsQR(frame.data, frame.width, frame.height);
              codes = result ? [{rawValue: result.data}] : [];
            }
            if (codes.length) { camera.getTracks().forEach(t => t.stop());video.remove();await connect(codes[0].rawValue);return; } }
          catch { scanStatus.textContent = 'Could not read the code. Keep the whole QR code in view.'; }
          if (generation === scanGeneration) setTimeout(next, 150);
        };void next();
      } catch { scanStatus.textContent = 'Camera unavailable. Allow camera access or paste a link.';camera?.getTracks().forEach(t => t.stop());video?.remove(); }
    });
    const qrFile = input(card, 'Or choose a QR image', '', 'file');qrFile.accept = 'image/*';
    qrFile.addEventListener('change', async () => {
      const file = qrFile.files?.[0];if (!file) return;
      if (file.size > 10 * 1024 * 1024) { scanStatus.textContent = 'Choose an image smaller than 10 MB.';return; }
      let bitmap;
      try {
        bitmap = await createImageBitmap(file);
        const scale = Math.min(1, 1600 / Math.max(bitmap.width, bitmap.height));
        const canvas = document.createElement('canvas');canvas.width = Math.round(bitmap.width * scale);canvas.height = Math.round(bitmap.height * scale);
        const ctx = canvas.getContext('2d');ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
        const frame = ctx.getImageData(0, 0, canvas.width, canvas.height), result = jsQR(frame.data, frame.width, frame.height);
        if (!result) { scanStatus.textContent = 'No QR code found. Choose a clear image of the whole code.';return; }
        await connect(result.data);
      } catch { scanStatus.textContent = 'Could not read this QR image. Try another image or paste the link.'; }
      finally { bitmap?.close();qrFile.value = ''; }
    });
    const codes = element('details', '', card);element('summary', 'Type a short code', codes);
    const address = input(codes, 'Hub address', 'http://computer:3787');address.value = location.origin;
    element('p', 'Use the hub address shown beside the code. Both computers must be on the same LAN or tailnet.', codes, {class: 'pairing-help'});
    const code = input(codes, '8-character code', 'ABCD 2345');code.maxLength = 12;
    button(codes, 'Connect with code', () => { try { void connect(codeLink(address.value, code.value)); } catch (e) { error(status, e.message); } });
    if (pending) {
      button(card, 'Connect this device', async () => { status.textContent = 'Connecting…';try { await enroll(pending.secret, name.value); } catch(e) { error(status,e.message); } });
      element('a', 'Open in Backplane', card, {href: 'backplane://pair?url=' + encodeURIComponent(location.origin + '/pair' + pending.hash)});
    }
    link.focus();
  }
  async function devices() {
    const card = frame('Devices', 'Add another computer or phone. Each invitation expires in 10 minutes and works once.', true);
    const status = element('p', '', card, {role: 'status', 'aria-live': 'polite'});
    async function request(path, method = 'GET', headers = {}) {
      const r = await fetch(path, {method, credentials: 'same-origin', cache: 'no-store', redirect: 'error', headers, signal: AbortSignal.timeout(8000)});
      const value = await r.json();if (!r.ok || value.error) throw Error(value.error || 'pairing_required');return value;
    }
    const invitation = element('section', '', card, {class: 'pairing-invitation'});
    button(card, 'Add a device', async () => {
      status.textContent = 'Creating invitation…';invitation.replaceChildren();clearInterval(expiryTimer);
      try {
        const invite = await request('/auth/invitations', 'POST');if (!panel?.contains(card)) return;
        const until = element('p', '', invitation, {role: 'status'});
        const update = () => { const seconds = Math.max(0, Math.ceil((invite.expiresAt - Date.now()) / 1000));until.textContent = seconds ? 'Works once · expires in ' + Math.ceil(seconds / 60) + ' minutes' : 'Expired — create a new invitation';if (!seconds) { clearInterval(expiryTimer);for (const b of invitation.querySelectorAll('button')) b.disabled = true; } };
        expiryTimer = setInterval(update, 1000);update();
        element('p', invite.code, invitation, {class: 'pairing-code'});
        element('p', 'Hub address: ' + invite.routes[0], invitation);
        const link = input(invitation, 'One-time pairing link', '');link.value = invite.link;link.readOnly = true;
        const copy = button(invitation, 'Copy link', async () => { try { await navigator.clipboard.writeText(invite.link);copy.textContent = 'Copied'; } catch { link.select();status.textContent = 'Select and copy the link.'; } });
        button(invitation, 'Copy code', async () => { try { await navigator.clipboard.writeText(invite.code);status.textContent = 'Code copied. Share the hub address with it.'; } catch { status.textContent = 'Select and copy the code shown above.'; } });
        button(invitation, 'Share', async () => { try { if (navigator.share) await navigator.share({title: 'Connect to Backplane', url: invite.link});else await navigator.clipboard.writeText(invite.link); } catch (e) { if (e.name !== 'AbortError') status.textContent = 'Share unavailable. Use Copy link.'; } });
        const rows = String(invite.qr).split('\n').filter(Boolean), size = rows.length + 8;
        const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');svg.setAttribute('viewBox', `0 0 ${size} ${size}`);svg.setAttribute('role', 'img');svg.setAttribute('aria-label', 'Scan to pair this device');svg.setAttribute('class', 'pairing-qr');
        const path = document.createElementNS(svg.namespaceURI, 'path');let d = '';
        rows.forEach((row,y) => [...row].forEach((v,x) => { if (v === '1') d += `M${x+4} ${y+4}h1v1h-1z`; }));path.setAttribute('d', d);svg.append(path);invitation.append(svg);
        status.textContent = invite.localOnly ? 'This address works only on this computer. Enable tailnet access in Settings → Network to connect a remote device.' : 'Invitation ready. Share only with the device you want to authorize.';
      } catch(e) { error(status, e.message); }
    });
    const list = element('section', '', card);element('h2', 'Paired devices', list);
    async function refresh() {
      try {
        const result = await request('/auth/devices');if (!panel?.contains(card)) return;
        list.replaceChildren();element('h2', 'Paired devices', list);
        const active = result.devices.filter(d => !d.revoked);
        if (!active.length) element('p', 'Existing pairings are preserved. Devices added with new invitations appear here.', list);
        for (const d of active) {
          const row = element('div', '', list, {class: 'pairing-device'});element('strong', d.name, row);
          element('p', 'Last seen ' + new Date(d.lastSeen).toLocaleString(), row);
          button(row, 'Revoke', async () => { if (!globalThis.confirm('Revoke access for ' + d.name + '?')) return;try { await request('/auth/revoke', 'POST', {'X-Backplane-Device': d.id});await refresh(); } catch(e) { error(status, e.message); } });
        }
      } catch(e) { error(status, e.message); }
    }
    await refresh();
  }
  return {show, devices, close, accepted: () => { if (pairingShown) close();else document.body.classList.remove("pairing-pending"); }};
}
