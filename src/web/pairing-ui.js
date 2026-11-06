// Connection-boundary UI: available before a client can ask the hub for views.
// Invitation material stays in this closure/DOM, never in Ui, logs or caches.
import jsQR from './vendor/jsqr.cjs';
import {cameraSupport, openScanner, scannerWords} from './pairing-scanner.js';
import {defaultDeviceName} from './pairing-device.js';
import {pairingInput, codeLink, pairingErrors} from './pairing-link.js';
import {softFocus} from './touch-focus.js';
export function installPairingUI({document, location, fetch, onPaired, navigator = globalThis.navigator}) {
  let panel, scanner = null, expiryTimer, pairingShown = false;
  const element = (tag, text, parent, attrs = {}) => {
    const node = document.createElement(tag);if (text) node.textContent = text;
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
    parent?.append(node);return node;
  };
  function close() {
    scanner?.close();scanner = null;
    clearInterval(expiryTimer);panel?.remove();panel = null;
    document.getElementById('root')?.removeAttribute('inert');
    document.body.classList.remove('pairing-pending');pairingShown = false;
  }
  function frame(title, lead, dismissible) {
    close();panel = element('main', '', document.body, {class: 'pairing-screen', 'aria-label': title});
    document.getElementById('root')?.setAttribute('inert', '');
    const card = element('section', '', panel, {class: 'pairing-card'});
    element('h1', title, card);element('p', lead, card, {class: 'pairing-lead'});
    if (dismissible) { button(card, 'Back', close).classList.add('pairing-back');panel.dataset.dismissible = '1'; }
    return card;
  }
  // Esc leaves a screen that has Back (Devices), before any other key
  // handler (the workspace's Esc would act on the page underneath)
  (document.defaultView || globalThis).addEventListener?.('keydown', event => {
    if (event.key !== 'Escape' || event.defaultPrevented || event.isComposing || panel?.dataset.dismissible !== '1') return;
    event.preventDefault();event.stopImmediatePropagation();close();
  }, true);
  function button(parent, text, click) { const b = element('button', text, parent, {type: 'button'});b.addEventListener('click', click);return b; }
  function input(parent, label, placeholder, type = 'text') {
    const l = element('label', label, parent), i = element('input', '', l, {class: 'field', placeholder, type, autocomplete: 'off', autocapitalize: 'none', spellcheck: 'false'});
    return i;
  }
  function error(target, cause) { target.classList.add('bad');target.textContent = pairingErrors[cause] || 'Connection could not be completed. Try again.'; }
  async function enroll(secret, name) {
    const abort = new AbortController(), timer = setTimeout(() => abort.abort(), 8000);
    try {
      const r = await fetch('/auth/enroll', {method: 'POST', cache: 'no-store', credentials: 'same-origin', redirect: 'error', signal: abort.signal,
        headers: {Authorization: 'Bearer ' + secret, 'X-Backplane-Name': name.replace(/[^\x20-\x7e]/g, '').slice(0, 80)}});
      const result = await r.json();if (!r.ok || result.error) throw Error(result.error || 'pairing_required');
      // which device this browser is, for Devices' "This device" (an id
      // /auth/devices lists anyway; never the credential)
      try { if (typeof result.device === 'string' && result.device) globalThis.localStorage?.setItem('backplane-device', result.device); } catch {}
      close();onPaired();
    } catch (e) { throw Error(pairingErrors[e.message] ? e.message : 'hub_unreachable'); }
    finally { clearTimeout(timer); }
  }
  function show({repair = false, cause = '', pending = null} = {}) {
    const card = frame(repair ? 'Re-pair this device' : 'Connect to your Backplane hub',
      'On your computer, open Backplane, then Settings → Devices → Add a device. A QR code, a short code and a pairing link appear there. Choose how to connect below.', false);
    pairingShown = true;
    const name = input(card, 'Device name', 'Name this device');name.value = defaultDeviceName(navigator, !!globalThis.window?.backplaneDesktop);
    const status = element('p', '', card, {role: 'status', 'aria-live': 'polite', class: 'pairing-status'});if (cause) error(status, cause);
    const choices = element('div', '', card, {class: 'pairing-methods', role: 'tablist', 'aria-label': 'Connection method'});
    const methods = ['Pairing link', 'QR code', 'Short code'].map((label, index) => {
      const tab = button(choices, label, () => selectMethod(index));
      tab.id = 'pairing-tab-' + index;tab.setAttribute('role', 'tab');
      tab.setAttribute('aria-controls', 'pairing-method-' + index);
      const content = element('section', '', card, {class: 'pairing-method', role: 'tabpanel', id: 'pairing-method-' + index, 'aria-labelledby': tab.id});
      tab.addEventListener('keydown', e => {
        const next = e.key === 'ArrowRight' ? (index + 1) % 3 : e.key === 'ArrowLeft' ? (index + 2) % 3 : e.key === 'Home' ? 0 : e.key === 'End' ? 2 : -1;
        if (next < 0) return;e.preventDefault();selectMethod(next);methods[next].tab.focus();
      });
      return {tab, content};
    });
    function stopScanning() { scanner?.close();scanner = null; }
    function selectMethod(index) {
      stopScanning();methods.forEach(({tab, content}, i) => {
        tab.setAttribute('aria-selected', String(i === index));tab.tabIndex = i === index ? 0 : -1;
        tab.classList.toggle('on', i === index);content.hidden = i !== index;
      });
    }
    const [links, qr, codes] = methods.map(m => m.content);
    const link = input(links, 'Paste a pairing link', 'https://your-hub/pair#invite=…');
    async function connect(text) {
      status.classList.remove('bad');status.textContent = 'Connecting…';
      try {
        const p = pairingInput(text, location.origin);
        if (globalThis.window?.backplaneDesktop?.pair) { const result = await window.backplaneDesktop.pair(text, name.value);if (!result?.ok) throw Error(result?.error || 'hub_unreachable');return; }
        if (p.origin !== location.origin) { location.assign(p.origin + '/pair' + p.hash);return; }
        if (p.kind === 'token') { location.assign('/pair' + p.hash);return; }
        await enroll(p.secret, name.value);
      } catch (e) { error(status, e.message); }
    }
    button(links, 'Connect with link', () => connect(link.value)).classList.add('primary');
    link.addEventListener('keydown', e => { if (e.key === 'Enter') void connect(link.value); });
    element('p', 'Scan the QR code shown under Add a device on your computer, or choose a picture of it.', qr, {class: 'pairing-help'});
    // the camera only where the page may have it (https://, localhost);
    // else what to do instead, never a button that cannot work
    const cam = cameraSupport(globalThis.window ? {isSecureContext: globalThis.isSecureContext, navigator} : {navigator});
    const scanStatus = element('p', '', qr, {class: 'pairing-help', role: 'status'});
    const accept = text => { try { pairingInput(text, location.origin);return true; } catch { return false; } };
    if (cam.ok) button(qr, 'Scan with the camera', async () => {
      scanStatus.textContent = '';stopScanning();
      const s = scanner = openScanner({document, navigator, accept, onCode: text => connect(text), onClose: () => { if (scanner === s) scanner = null; }});
      const why = await s.start();
      if (why && why !== 'closed') scanStatus.textContent = scannerWords[why];
    }).classList.add('primary', 'pairing-scan');
    else {
      element('p', scannerWords[cam.why], qr, {class: 'pairing-nocam', role: 'note'});
      button(qr, 'Use Short code', () => { selectMethod(2);methods[2].tab.focus(); }).classList.add('primary');
    }
    const qrFile = element('input', '', qr, {type: 'file', accept: 'image/*', class: 'pairing-file', 'aria-label': 'Or choose a QR image', tabindex: '-1'});
    button(qr, 'Choose QR image', () => qrFile.click());
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
    element('p', 'Enter the code and hub address shown on your paired device.', codes, {class: 'pairing-help'});
    const address = input(codes, 'Hub address', 'http://computer:3787');address.value = location.origin;
    element('p', 'Use the hub address shown beside the code. Both computers must be on the same LAN or tailnet.', codes, {class: 'pairing-help'});
    const code = input(codes, '8-character code', 'ABCD 2345');code.maxLength = 12;
    const connectCode = () => { try { void connect(codeLink(address.value, code.value)); } catch (e) { error(status, e.message); } };
    button(codes, 'Connect with code', connectCode).classList.add('primary');
    code.addEventListener('keydown', e => { if (e.key === 'Enter') connectCode(); });
    // a home-screen app (its own cookies, so it pairs again) cannot be
    // opened by a pairing link: it starts on the short code
    const standalone = !pending && (navigator?.standalone === true || !!globalThis.matchMedia?.('(display-mode: standalone)')?.matches);
    selectMethod(standalone ? 2 : 0);
    if (standalone) element('p', 'Opened from the Home Screen: enter the short code from Settings → Devices → Add a device on your computer.', card, {class: 'pairing-help pairing-standalone'});
    if (pending) {
      const actions = element('div', '', card, {class: 'pairing-pending'});
      element('p', 'Or connect another way:', card, {class: 'pairing-or'});
      button(actions, 'Connect this device', async () => { status.classList.remove('bad');status.textContent = 'Connecting…';try { await enroll(pending.secret, name.value); } catch(e) { error(status,e.message); } }).classList.add('primary');
      element('a', 'Open in Backplane', actions, {href: 'backplane://pair?url=' + encodeURIComponent(location.origin + '/pair' + pending.hash)});
    }
    // (a phone raises its keyboard for it, over a pairing link's Connect:
    // on a touch screen the field waits for a tap, touch-focus.js)
    if (standalone) softFocus(code, {preventScroll: true});
    else if (!pending || !document.documentElement.classList.contains('phone')) softFocus(link);
  }
  function invitationState(x) {
    const thing = x.kind === 'machine' ? 'a machine' : 'a device', its = x.kind === 'machine' ? 'the machine it linked' : 'the device it paired';
    if (x.state === 'used') return 'Used ' + new Date(x.usedAt).toLocaleString() + ' by ' + (x.deviceName || thing) + (x.deviceRevoked ? (x.kind === 'machine' ? ' (unlinked)' : ' (revoked)') : '');
    if (x.state === 'revoked') return 'Revoked' + (x.device ? ' with ' + (x.deviceName || its) : ', never used');
    if (x.state === 'expired') return 'Expired, never used';
    return 'Not used yet · expires ' + new Date(x.expiresAt).toLocaleTimeString();
  }
  async function devices() {
    const card = frame('Devices', 'Add another computer or phone. Each invitation expires in 10 minutes and works once.', true);
    const status = element('p', '', card, {role: 'status', 'aria-live': 'polite', class: 'pairing-status'});
    async function request(path, method = 'GET', headers = {}) {
      const r = await fetch(path, {method, credentials: 'same-origin', cache: 'no-store', redirect: 'error', headers, signal: AbortSignal.timeout(8000)});
      const value = await r.json();if (!r.ok || value.error) throw Error(value.error || 'pairing_required');return value;
    }
    const invitation = element('section', '', card, {class: 'pairing-invitation'});
    // the local network addresses the hub was let answer on (POST /auth/lan;
    // /auth/devices lists them): each can be turned off here
    let lanSaved = [], lanShown = '', shown = null, lanList;
    async function setLan(ip, on) {
      status.classList.remove('bad');status.textContent = on ? 'Letting the hub answer on ' + ip + '…' : 'Stopping…';
      try {
        const r = await request('/auth/lan', 'POST', {'X-Backplane-Lan': ip, 'X-Backplane-Lan-State': on ? 'on' : 'off'});
        lanSaved = on ? [...lanSaved.filter(x => x.ip !== ip), {ip, url: '', listening: true}] : lanSaved.filter(x => x.ip !== ip);
        shown?.listening(ip, !!r.listening);
        if (!on) status.textContent = r.listening ? 'The hub still answers on ' + ip + ': it listens on every interface (--host 0.0.0.0).' : 'The hub no longer answers on ' + ip + '.';
        await refresh(true);
      } catch (e) { error(status, e.message); }
    }
    function lanRows() {
      if (JSON.stringify(lanSaved) === lanShown) return;
      lanShown = JSON.stringify(lanSaved);lanList.replaceChildren();if (!lanSaved.length) return;
      element('h2', 'Local network', lanList);
      for (const x of lanSaved) {
        const row = element('div', '', lanList, {class: 'pairing-device'});
        element('strong', x.url || x.ip, row);
        element('p', (x.listening ? 'The hub also answers here' : 'Kept, but this computer does not have this address now') + ': plain HTTP on that network; every device still pairs.', row);
        button(row, 'Stop answering here', () => setLan(x.ip, false));
      }
    }
    const words = {tailnet: 'Tailscale', lan: 'Local network', local: 'This computer'};
    const add = button(card, 'Add a device', async () => {
      status.classList.remove('bad');status.textContent = 'Creating invitation…';invitation.replaceChildren();clearInterval(expiryTimer);shown = null;
      try {
        const invite = await request('/auth/invitations', 'POST');if (!panel?.contains(card)) return;
        // where the device reaches this hub (the tailnet, each local network
        // address, this computer); a hub too old to say has its one link
        const addresses = Array.isArray(invite.addresses) && invite.addresses.length ? invite.addresses
          : [{kind: invite.localOnly ? 'local' : '', url: invite.routes[0], link: invite.link, qr: invite.qr, listening: true}];
        const kinds = ['tailnet', 'lan', 'local'].filter(k => addresses.some(a => a.kind === k));
        const lans = addresses.filter(a => a.kind === 'lan');
        let kind = kinds[0] || '', lan = Math.max(0, lans.findIndex(a => a.listening));
        const chosen = () => kind === 'lan' ? lans[lan] : addresses.find(a => a.kind === kind) || addresses[0];
        const until = element('p', '', invitation, {role: 'status'});
        // (while it can still be used, the list below looks again every 3 s,
        // so the device it brings in shows up without leaving this page)
        let ticks = 0;
        const update = () => { const seconds = Math.max(0, Math.ceil((invite.expiresAt - Date.now()) / 1000));if (seconds && ++ticks % 3 === 0) void refresh(true);until.textContent = seconds ? 'Works once · expires in ' + Math.ceil(seconds / 60) + ' minutes' : 'Expired — create a new invitation';if (!seconds) { clearInterval(expiryTimer);for (const b of invitation.querySelectorAll('button, select')) b.disabled = true; } };
        expiryTimer = setInterval(update, 1000);update();
        // the choice swaps the address, link, QR and notes in place: every
        // address came with this one invitation
        const where = element('div', '', kinds.length > 1 ? invitation : null, {class: 'pairing-where', role: 'radiogroup', 'aria-label': 'How the device reaches this hub'});
        const picks = kinds.map(k => {
          const b = button(where, words[k], () => { kind = k;render(); });b.setAttribute('role', 'radio');b.dataset.kind = k;
          b.addEventListener('keydown', e => {
            const i = kinds.indexOf(kind), next = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? (i + 1) % kinds.length : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? (i + kinds.length - 1) % kinds.length : -1;
            if (next < 0) return;e.preventDefault();kind = kinds[next];render();picks[next].focus();
          });
          return b;
        });
        const lanLabel = element('label', 'Local network address', lans.length > 1 ? invitation : null, {class: 'pairing-lanpick'});
        const lanPick = element('select', '', lanLabel, {class: 'field'});
        lans.forEach((a, i) => { const o = element('option', a.url, lanPick, {value: String(i)});o.selected = i === lan; });
        lanPick.addEventListener('change', () => { lan = Number(lanPick.value);render(); });
        element('p', invite.code, invitation, {class: 'pairing-code'});
        const hub = element('p', '', invitation, {class: 'pairing-hub'});
        const note = element('p', '', invitation, {class: 'pairing-help pairing-note'});
        const offer = element('div', '', invitation, {class: 'pairing-offer'});
        const open = button(offer, 'Let the hub answer on the local network', () => setLan(new URL(chosen().url).hostname, true));open.classList.add('primary');
        const stop = button(offer, 'Stop answering on the local network', () => setLan(new URL(chosen().url).hostname, false));
        const shares = element('div', '', invitation, {class: 'pairing-shares'});
        const link = input(shares, 'One-time pairing link', '');link.readOnly = true;
        const copy = button(shares, 'Copy link', async () => { try { await navigator.clipboard.writeText(chosen().link);copy.textContent = 'Copied'; } catch { link.select();status.textContent = 'Select and copy the link.'; } });
        button(shares, 'Copy code', async () => { try { await navigator.clipboard.writeText(invite.code);status.textContent = 'Code copied. Share the hub address with it: ' + chosen().url; } catch { status.textContent = 'Select and copy the code shown above.'; } });
        button(shares, 'Share', async () => { try { if (navigator.share) await navigator.share({title: 'Connect to Backplane', url: chosen().link});else await navigator.clipboard.writeText(chosen().link); } catch (e) { if (e.name !== 'AbortError') status.textContent = 'Share unavailable. Use Copy link.'; } });
        const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');svg.setAttribute('role', 'img');svg.setAttribute('aria-label', 'Scan to pair this device');svg.setAttribute('class', 'pairing-qr');
        const path = document.createElementNS(svg.namespaceURI, 'path');svg.append(path);shares.append(svg);
        const extra = ip => lanSaved.some(x => x.ip === ip && x.listening);
        function render() {
          const a = chosen(), ip = a.kind === 'lan' ? new URL(a.url).hostname : '';
          picks.forEach(b => { const on = b.dataset.kind === kind;b.classList.toggle('on', on);b.setAttribute('aria-checked', String(on));b.tabIndex = on ? 0 : -1; });
          lanLabel.hidden = kind !== 'lan';
          hub.textContent = 'Hub address: ' + a.url;
          link.value = a.link;copy.textContent = 'Copy link';
          const rows = String(a.qr || '').split('\n').filter(Boolean), size = rows.length + 8;let d = '';
          rows.forEach((row, y) => [...row].forEach((v, x) => { if (v === '1') d += `M${x+4} ${y+4}h1v1h-1z`; }));
          svg.setAttribute('viewBox', `0 0 ${size} ${size}`);path.setAttribute('d', d);
          // a local network address the hub does not answer on yet: no link
          // to hand out until the person lets it answer there
          shares.hidden = !a.listening;offer.hidden = a.kind !== 'lan' || (a.listening && !extra(ip));
          open.hidden = a.listening;stop.hidden = !a.listening;
          note.textContent = a.kind === 'tailnet' ? 'Over your tailnet: the other device needs Tailscale, signed in to the same tailnet.'
            : a.kind === 'lan' ? (a.listening ? 'Plain HTTP on this network: the traffic is not encrypted, though every device still has to pair. Prefer Tailscale on a network you do not trust.'
              : 'The hub does not answer at ' + a.url + ' yet: it listens only on this computer. Letting it answer there opens plain HTTP on this network (not encrypted); every device still has to pair, and you can turn it off here or under Devices.')
            : '';
          note.hidden = !note.textContent;
          status.classList.remove('bad');
          status.textContent = a.kind === 'local' || invite.localOnly && !a.kind ? 'This address works only on this computer.' + (kinds.length > 1 ? ' Choose ' + kinds.filter(k => k !== 'local').map(k => words[k]).join(' or ') + ' to connect another device.' : ' Enable tailnet access in Settings → Network to connect a remote device.')
            : a.listening ? 'Invitation ready. Share only with the device you want to authorize.' : 'Not reachable from other devices yet.';
        }
        shown = {listening(ip, on) { for (const a of lans) if (new URL(a.url).hostname === ip) a.listening = on;render(); }};
        render();
      } catch(e) { error(status, e.message); }
    });
    add.classList.add('primary');
    lanList = element('section', '', card, {class: 'pairing-lan'});
    const list = element('section', '', card);element('h2', 'Paired devices', list);
    const links = element('section', '', card);
    async function refresh(quiet = false) {
      try {
        const result = await request('/auth/devices');if (!panel?.contains(card)) return;
        lanSaved = Array.isArray(result.lan) ? result.lan : [];lanRows();
        list.replaceChildren();element('h2', 'Paired devices', list);
        // (a phone marks itself, first: every phone is "iPhone / Safari")
        let mine = '';try { mine = document.documentElement.classList.contains('phone') ? globalThis.localStorage?.getItem('backplane-device') || '' : ''; } catch {}
        const active = result.devices.filter(d => !d.revoked).sort((a, b) => (b.id === mine) - (a.id === mine));
        if (!active.length) element('p', 'Existing pairings are preserved. Devices added with new invitations appear here.', list);
        for (const d of active) {
          const row = element('div', '', list, {class: 'pairing-device'});element('strong', d.name, row);
          if (mine && d.id === mine) element('span', 'This device', row, {class: 'pairing-this'});
          element('p', 'Last seen ' + new Date(d.lastSeen).toLocaleString(), row);
          button(row, 'Revoke', async () => { if (!globalThis.confirm('Revoke access for ' + d.name + '?')) return;try { await request('/auth/revoke', 'POST', {'X-Backplane-Device': d.id});await refresh(); } catch(e) { error(status, e.message); } });
        }
        // every pairing link this hub issued: when, used or not and by which
        // device; revoking one this hub has not seen used stops it working,
        // revoking a used one also revokes the device it paired
        links.replaceChildren();element('h2', 'Pairing links', links);
        const issued = result.invitations || [];
        if (!issued.length) element('p', 'Links made with Add a device, and invites made in Bots to link a machine, appear here.', links);
        for (const x of issued) {
          const row = element('div', '', links, {class: 'pairing-device'});
          element('strong', (x.kind === 'machine' ? 'Machine invite · ' : 'Device invitation · ') + new Date(x.createdAt).toLocaleString(), row);
          element('p', invitationState(x), row);
          const live = x.state === 'open' || (x.state === 'used' && x.device && !x.deviceRevoked);
          if (live) button(row, 'Revoke', async () => {
            if (!globalThis.confirm(x.state === 'used' ? (x.kind === 'machine' ? 'Revoke this invite and unlink the machine it linked (' : 'Revoke this link and the device it paired (') + (x.deviceName || 'unnamed') + ')?' : 'Revoke this link? It will no longer pair anything.')) return;
            try { await request('/auth/revoke', 'POST', {'X-Backplane-Invitation': x.id});await refresh(); } catch(e) { error(status, e.message); }
          });
        }
      } catch(e) { if (!quiet) error(status, e.message); }
    }
    await refresh();
  }
  return {show, devices, close, accepted: () => { if (pairingShown) close();else document.body.classList.remove("pairing-pending"); }};
}
