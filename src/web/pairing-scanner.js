// The pairing screen's QR scanner: the rear camera full screen inside the
// page, read with the browser's BarcodeDetector where it has one, else the
// bundled jsQR (no network). The first code that is a Backplane invitation
// closes it and goes to onCode, the same way in as a pasted pairing link;
// another QR code says so and scanning goes on. Close and Esc (and the
// phone's Back, through window.backplaneBack) stop the camera.
//
// The camera needs a secure context: a page over plain http:// from another
// machine (a phone at http://100.x.y.z:57808) has no navigator.mediaDevices
// at all. cameraSupport says why before anything is offered, so the pairing
// screen explains the other ways in instead of showing a dead button.
//
// In a phone app's WebView the page asks the app first, when it offers
// window.backplaneApp.requestCamera() (a Promise of true once the app holds
// the system's camera permission); the WebView must then grant the page's
// own request (Android: WebChromeClient.onPermissionRequest with
// RESOURCE_VIDEO_CAPTURE; iOS: NSCameraUsageDescription and WKUIDelegate's
// media capture permission). See docs/phone-web.md.
import jsQR from './vendor/jsqr.cjs';

export function cameraSupport(env = globalThis) {
  const navigator = env.navigator;
  if (env.isSecureContext === false) return {ok: false, why: 'insecure'};
  if (!navigator?.mediaDevices?.getUserMedia) return {ok: false, why: 'unsupported'};
  return {ok: true, why: ''};
}

export const scannerWords = {
  insecure: 'The camera can’t be used on this page: browsers give the camera only to https:// pages, and this one is http://. Scan the QR code with your phone’s own camera app instead (it opens the pairing link), or use Short code.',
  unsupported: 'This browser can’t use the camera here. Scan the QR code with your phone’s own camera app instead (it opens the pairing link), or use Short code.',
  denied: 'Camera access was refused. Allow the camera for this page in the browser’s site settings (or the app’s settings) and try again, or use Short code.',
  missing: 'No camera was found on this device. Use Short code or Pairing link.',
  busy: 'The camera is in use by another app. Close it and try again.',
  failed: 'The camera could not start. Try again, or use Short code.',
  hint: 'Point the camera at the QR code on your computer’s Devices screen.',
  other: 'That QR code is not a Backplane invitation. Point at the one under Add a device.',
};

export function cameraError(e) {
  const n = e?.name || '';
  if (n === 'NotAllowedError' || n === 'SecurityError' || e?.message === 'app_denied') return 'denied';
  if (n === 'NotFoundError' || n === 'OverconstrainedError' || n === 'DevicesNotFoundError') return 'missing';
  if (n === 'NotReadableError' || n === 'TrackStartError' || n === 'AbortError') return 'busy';
  return 'failed';
}

// accept(text) is true for a code worth leaving the scanner for (a pairing
// link); start() answers '' once the camera runs, else an error's key
export function openScanner({document, navigator = globalThis.navigator, accept, onCode, onClose, detector = globalThis.BarcodeDetector, decode = jsQR, app = globalThis.window?.backplaneApp}) {
  const el = (tag, cls, parent, text) => { const n = document.createElement(tag);if (cls) n.className = cls;if (text) n.textContent = text;parent?.append(n);return n; };
  const view = el('div', 'pairing-scanner', document.body);
  view.setAttribute('role', 'dialog');view.setAttribute('aria-modal', 'true');view.setAttribute('aria-label', 'Scan the pairing QR code');
  const video = el('video', 'pairing-scanner-video', view);
  video.muted = true;video.setAttribute('playsinline', '');video.setAttribute('autoplay', '');video.setAttribute('muted', '');
  el('div', 'pairing-scanner-frame', view).setAttribute('aria-hidden', 'true');
  const bar = el('div', 'pairing-scanner-bar', view);
  const close = el('button', 'pairing-scanner-close', bar, 'Close');close.type = 'button';close.setAttribute('aria-label', 'Close the scanner');
  const torch = el('button', 'pairing-scanner-torch', bar, 'Torch');torch.type = 'button';torch.hidden = true;torch.setAttribute('aria-pressed', 'false');
  const say = el('p', 'pairing-scanner-say', view, scannerWords.hint);say.setAttribute('role', 'status');say.setAttribute('aria-live', 'polite');
  // the page under it takes no taps or keys while it is open
  const under = [...document.body.children].filter(n => n !== view && !n.hasAttribute('inert'));
  under.forEach(n => n.setAttribute('inert', ''));
  let stream = null, live = true, timer = 0, lit = false;
  const canvas = document.createElement('canvas'), ctx = canvas.getContext?.('2d', {willReadFrequently: true});
  const stopCamera = () => { stream?.getTracks().forEach(t => t.stop());stream = null;video.srcObject = null; };
  function shut(why = 'closed') {
    if (!live) return;live = false;clearTimeout(timer);stopCamera();view.remove();under.forEach(n => n.removeAttribute('inert'));
    (document.defaultView || globalThis).removeEventListener?.('keydown', onKey, true);
    if (prevBack) win.backplaneBack = prevBack;else delete win.backplaneBack;
    onClose?.(why);
  }
  const onKey = e => { if (e.key === 'Escape') { e.preventDefault();e.stopImmediatePropagation();shut(); } };
  (document.defaultView || globalThis).addEventListener?.('keydown', onKey, true);
  // the phone's Back (an app's WebView calls backplaneBack) shuts the scanner first
  const win = document.defaultView || globalThis, prevBack = win.backplaneBack;
  win.backplaneBack = () => { shut();return true; };
  close.addEventListener('click', () => shut());
  close.focus({preventScroll: true});
  torch.addEventListener('click', async () => {
    const track = stream?.getVideoTracks()[0];if (!track) return;
    try { await track.applyConstraints({advanced: [{torch: !lit}]});lit = !lit;torch.setAttribute('aria-pressed', String(lit));torch.classList.toggle('on', lit); }
    catch { torch.hidden = true; }
  });
  let reader = null;
  async function read() {
    if (reader) { const codes = await reader.detect(video);return codes.map(c => c.rawValue); }
    const w = video.videoWidth, h = video.videoHeight;if (!w || !h || !ctx) return [];
    canvas.width = Math.min(640, w);canvas.height = Math.round(h * canvas.width / w);
    ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
    const frame = ctx.getImageData(0, 0, canvas.width, canvas.height), found = decode(frame.data, frame.width, frame.height, {inversionAttempts: 'dontInvert'});
    return found ? [found.data] : [];
  }
  async function scan() {
    if (!live) return;
    try {
      for (const text of await read()) {
        if (accept(text)) { const t = text;shut('scanned');await onCode(t);return; }
        say.textContent = scannerWords.other;
      }
    } catch {}
    if (live) timer = setTimeout(scan, 120);
  }
  async function start() {
    try {
      if (app?.requestCamera && !(await app.requestCamera())) throw Error('app_denied');
      if (!live) return 'closed';
      stream = await navigator.mediaDevices.getUserMedia({audio: false, video: {facingMode: {ideal: 'environment'}, width: {ideal: 1280}, height: {ideal: 720}}});
      if (!live) { stopCamera();return 'closed'; }
      video.srcObject = stream;await video.play().catch(() => {});
      const track = stream.getVideoTracks()[0];
      try { torch.hidden = !track?.getCapabilities?.().torch; } catch { torch.hidden = true; }
      try { if (detector && (await detector.getSupportedFormats?.() || ['qr_code']).includes('qr_code')) reader = new detector({formats: ['qr_code']}); } catch { reader = null; }
      void scan();
      return '';
    } catch (e) {
      const why = cameraError(e);shut(why);return why;
    }
  }
  return {start, close: shut, element: view, get reading() { return reader ? 'detector' : 'jsqr'; }};
}
