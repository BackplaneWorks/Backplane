'use strict';

const { randomUUID, randomBytes, createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

// Main-process component, never a renderer IPC handler. The caller must bind
// authorize to the authenticated Bend owner/Teach state, not renderer claims.
// Descriptors contain no WebContents, session, debugger, or application token.
// Cookies are shared across guest owners (matching the existing Browser profile),
// but never with the privileged app session. Tabs and commands are owner scoped.
// Integration must supply an input shield for native drops/menu actions while
// the agent drives: before-input/mouse-event alone is not a complete drop shield.
// Popup adoption preserves Chromium's supplied WebContents/opener. Pre-document
// recorder readiness is a SEPARATE integration contract; popups default to deny.
// Optional recorder integration (trusted main configuration, never renderer IPC):
// recorder:{preloadPath,preloadSha256,recorderSourceSha256,popupPreloadVerified?,subframePreload?}.
// subframePreload defaults false; only explicit true enables iframe preloads.
// The dedicated guest preload is owned by the shell lead. No app preload reuse,
// eval, runtime source delivery, contextBridge or service credential is involved.
// Session frame preload gets --backplane-guest-recorder=<32 lowercase hex>.
// Channels backplane:guest-recorder:<hex>:init|ready|record|control:
//   init sendSync {v:1,recorderSourceSha256}
//     -> {v:1,documentNonce,enabled,recorderSourceSha256} or {v:1,ok:false}
//   ready sendSync {documentNonce,recorderSourceSha256}
//     -> {v:1,ok:true,enabled} or {v:1,ok:false}
//   record send {documentNonce,event:{step:{...existing helper schema}}}
//   control main -> exact frame {documentNonce,enabled}
// Source masks secrets BEFORE IPC; main rebuilds known keys and drops secret
// values again. Nonces/owner/tab/grants are not exposed to the page main world.
// Every record is checked against current sender/frame/generation and authorize
// ('record-event'). setRecording(target,on) requires authorize('recording').
// The private main/service callback receives actual document identity, never a
// page-supplied one. createId is optional service correlation, NOT a grant.
// Non-null createId also identifies a main-only outstanding creation teardown
// capability. Controller must completeCreation before any user command, or abort
// that exact pending root after validating service cancel phase. IDs must never
// be reused across controller/manager epochs; owner+ID cannot distinguish an
// illegally reissued identical ID. No caller owner field supplies service trust.
// A create rejection after materialization carries frozen error.creationCleanup
// {owner,createId,target,closed,nativeDestroyed,closeUnknown}. This is an actual
// module-owned snapshot, not authority to retry close or proof of later native
// destruction. Normal Error identity/code survives cleanup failure. If the
// thrown value cannot accept the property, a wrapper retains it as exact cause.
// recorder-document-retired is a synchronous private old-document barrier:
// retire queued matching records before forwarding subsequent lifecycle events.
// Init has no document nonce: after retirement/deletion, an init from the same
// frameToken cannot alone distinguish old/new document incarnation. Real preload
// ordering remains an integration gate; frame tokens are not permanently banned.
// popupPreloadVerified defaults false: setting it is an integration capability,
// NOT evidence. Exact first-script/argv/child-init runtime gates are still owed.
//
// Integration contract:
//   create({owner,url}) -> Promise<{owner,tabId,generation}>
//   list(owner) -> frozen tab descriptors; use these to refresh document leases
//   select(target), present(target,{x,y,width,height}), hide(), focus(target)
//   navigate(target,url), close(target), dispose()
//   historyState(target) under list; navigateHistory(target,'back'|'forward') and
//   reload(target) under navigate. These return finite DISPATCH receipts with
//   the ORIGINAL target, not load promises or permission for a new generation.
//   command(target,CDPmethod,params) -> Promise<{target,result}>
//   reconcileInput() after every authoritative Teach/owner permission change
//
// authorize({operation,owner,tabId,generation,method?}) MUST return true
// synchronously. It must distinguish service-owned instrumentation from agent
// mutations, using authenticated call context (never a renderer-supplied role).
// Initialization calls have initialization:true, supplied only by the restricted
// prepareGuest command function or the module's fixed file-guard setup. After
// prepareGuest obtains the exact target's preparation grant, the module awaits
// Page.enable({}) then Page.setInterceptFileChooserDialog({enabled:true,cancel:true})
// before the first requested URL. Both require that same initialization grant.
// Unsupported/rejected setup fails creation closed. Successful experimental CDP
// replies are NOT native/OOPIF coverage proof; file chooser/upload/drop workflows
// still need explicit qualification. Public chooser toggles, Page.disable and
// DOM.setFileInputFiles are not broker capabilities. Detach/reset/crash latches
// file-guard-lost and hides the affected guest; no same-tab automatic reattach.
// create/list have tabId:null. There is no ambient "current owner" lookup.
// prepareGuest({target,command}) is awaited on a blank target before initial
// navigation; it is NOT called early enough for adopted popup documents.
// popupPolicy must deny while Teach requires an unproven early-popup recorder.
// onEvent is a MAIN/SERVICE hook, not something to broadcast wholesale to the
// renderer: CDP binding payloads must pass the existing recorder redaction and
// owner checks. UI consumers need only the scoped tabs/navigation/input events.
// load-state carries current native WC/document observations, with a manager-wide
// serial. Only did-finish-load sets finishObserved. No dispatchId correlation,
// recorder-readiness inference or retroactive command settlement is provided.
// Caller owns window bounds, overlay/drop shield, popup selection, dialogs,
// upload/download UX, service registration and translating the existing NDJSON
// operations into this broker. This is not a Playwright browser-level CDP socket.
// Profiles are not copied/migrated and no headless browser is launched here.

const CDP_METHODS = new Set([
  'DOM.getDocument', 'DOM.describeNode', 'DOM.querySelector', 'DOM.querySelectorAll',
  'DOM.resolveNode', 'DOM.getBoxModel', 'DOM.focus',
  'Accessibility.getFullAXTree', 'Accessibility.getPartialAXTree',
  'Runtime.enable', 'Runtime.disable', 'Runtime.evaluate', 'Runtime.callFunctionOn',
  'Runtime.getProperties', 'Runtime.releaseObject', 'Runtime.releaseObjectGroup',
  'Runtime.addBinding', 'Runtime.removeBinding',
  'Page.enable', 'Page.navigate', 'Page.reload', 'Page.stopLoading',
  'Page.getNavigationHistory', 'Page.navigateToHistoryEntry', 'Page.getLayoutMetrics',
  'Page.addScriptToEvaluateOnNewDocument', 'Page.removeScriptToEvaluateOnNewDocument',
  'Page.handleJavaScriptDialog',
  'Input.dispatchMouseEvent', 'Input.dispatchKeyEvent', 'Input.insertText',
  'Input.imeSetComposition',
]);
const SESSION_OWNERS = new WeakSet();
const RECORDER_SOURCE_SHA256 = '2e4b03765cc4ae6595e32048555ffe27e750994a85c18b89027064f3c0c15350';
const STEP_KEYS = new Set(['click', 'type', 'select', 'press', 'submit']);
const TARGET_KEYS = ['role', 'name', 'label', 'placeholder', 'text', 'testid', 'css', 'nth', 'near', 'tag', 'type'];
const NAVIGATION_METHODS = new Set(['Page.navigate', 'Page.reload', 'Page.navigateToHistoryEntry']);
const PREPARATION_METHODS = new Set(['Runtime.enable', 'Runtime.addBinding', 'Runtime.removeBinding',
  'Page.enable', 'Page.addScriptToEvaluateOnNewDocument', 'Page.removeScriptToEvaluateOnNewDocument']);

function identifier(value, name) {
  if (typeof value !== 'string' || !value.length || value.length > 512 || /[\x00-\x1f]/.test(value)) throw new TypeError(`Invalid ${name}`);
  return value;
}
function jsonObject(value) {
  if (!value || Object.getPrototypeOf(value) !== Object.prototype) throw new TypeError('Expected JSON parameters');
  const text = JSON.stringify(value, (_key, item) => {
    if (typeof item === 'number' && !Number.isFinite(item)) throw new TypeError('Non-finite parameter');
    if (item !== null && typeof item === 'object' && !Array.isArray(item) && Object.getPrototypeOf(item) !== Object.prototype) throw new TypeError('Expected plain JSON');
    if (['function', 'symbol', 'bigint', 'undefined'].includes(typeof item)) throw new TypeError('Expected JSON value');
    return item;
  });
  if (text.length > 1024 * 1024) throw new RangeError('Parameters too large');
  return JSON.parse(text);
}
function shape(value, keys) {
  return value && Object.getPrototypeOf(value) === Object.prototype &&
    Object.keys(value).every(key => keys.includes(key));
}
function recordedStep(value) {
  // Match the helper's known-key reconstruction. Never forward raw binding data.
  if (!value || !STEP_KEYS.has(value.do)) return null;
  const step = { do: value.do, target: {} };
  for (const key of TARGET_KEYS) {
    const item = value.target?.[key];
    step.target[key] = key === 'nth' ? (Number.isFinite(item) ? item : 0) : typeof item === 'string' ? item.slice(0, 200) : '';
  }
  if (value.do === 'press') {
    if (!['Enter', 'Tab', 'Escape'].includes(value.key)) return null;
    step.key = value.key;
  }
  if ((value.secret === true || step.target.type === 'password') && ['type', 'select'].includes(value.do)) {
    step.secret = true; step.label = String(value.label ?? '').slice(0, 80);
    step.target.placeholder = ''; step.target.text = '';
  } else if (value.do === 'type') step.value = String(value.value ?? '').slice(0, 10000);
  else if (value.do === 'select') {
    step.value = String(value.value ?? '').slice(0, 500); step.text = String(value.text ?? '').slice(0, 200);
  }
  return step;
}

function createBrowserGuestManager({
  electron, window, appSession, trustedOrigins, authorize, onEvent = () => {},
  partition = 'persist:backplane-browser-guests',
  prepareGuest = async () => {}, popupPolicy = () => false,
  commandTimeoutMs = 10000,
  recorder = null,
}) {
  if (!electron?.WebContentsView || !electron?.session || !window?.contentView ||
      !appSession || typeof authorize !== 'function' || typeof onEvent !== 'function' ||
      typeof prepareGuest !== 'function' || typeof popupPolicy !== 'function') throw new TypeError('Missing guest integration');
  if (!/^persist:backplane-browser-guests(?:[.-][a-zA-Z0-9_-]+)?$/.test(partition)) throw new TypeError('Invalid guest partition');
  if (!Array.isArray(trustedOrigins) || !trustedOrigins.length) throw new TypeError('App origins required');
  const blocked = new Set(trustedOrigins.map(value => {
    const url = new URL(value);
    if (!['http:', 'https:'].includes(url.protocol) || url.origin !== value) throw new TypeError('Expected exact app origin');
    return value;
  }));
  const guestSession = electron.session.fromPartition(partition);
  if (guestSession === appSession) throw new Error('Guest session must be separate');
  if (SESSION_OWNERS.has(guestSession)) throw new Error('Guest partition already managed');
  if (guestSession.getPreloadScripts().length) throw new Error('Guest session has a preload');
  if (!Number.isSafeInteger(commandTimeoutMs) || commandTimeoutMs < 1 || commandTimeoutMs > 60000) throw new TypeError('Invalid command timeout');
  let recorderConfig = null;
  if (recorder) {
    if (!electron.ipcMain || typeof recorder.preloadPath !== 'string' || !path.isAbsolute(recorder.preloadPath) ||
        !/^[a-f0-9]{64}$/.test(recorder.preloadSha256) || recorder.recorderSourceSha256 !== RECORDER_SOURCE_SHA256 ||
        (recorder.popupPreloadVerified !== undefined && typeof recorder.popupPreloadVerified !== 'boolean') ||
        (recorder.subframePreload !== undefined && typeof recorder.subframePreload !== 'boolean')) throw new TypeError('Invalid guest recorder integration');
    const bytes = fs.readFileSync(recorder.preloadPath);
    if (bytes.length > 256 * 1024 || createHash('sha256').update(bytes).digest('hex') !== recorder.preloadSha256) throw new Error('Guest recorder preload hash mismatch');
    recorderConfig = Object.freeze({ preloadPath: recorder.preloadPath, preloadSha256: recorder.preloadSha256,
      recorderSourceSha256: RECORDER_SOURCE_SHA256, popupPreloadVerified: recorder.popupPreloadVerified === true,
      subframePreload: recorder.subframePreload === true });
  }
  SESSION_OWNERS.add(guestSession);
  const owners = new Map(), creations = new Map();
  const contents = new Map(), recordingOwners = new Set();
  const namespace = recorderConfig ? randomBytes(16).toString('hex') : null;
  const channels = namespace ? Object.fromEntries(['init', 'ready', 'record', 'control'].map(key => [key, `backplane:guest-recorder:${namespace}:${key}`])) : null;
  const recorderArguments = namespace ? [`--backplane-guest-recorder=${namespace}`] : [];
  let registeredPreload = null;
  const registryId = randomUUID();
  let nextId = 0, loadObservationSerial = 0, disposed = false, presented = null;

  function alive() { if (disposed) throw new Error('Guest manager disposed'); }
  function urlAllowed(value, navigation = true) {
    try {
      const url = new URL(value);
      // HTTP and its WebSocket endpoint share the same protected service. Allow
      // ordinary guest-site WebSockets, without opening the app's WS channel.
      const origin = url.origin.replace(/^ws:/, 'http:').replace(/^wss:/, 'https:');
      if (blocked.has(origin) || url.username || url.password) return false;
      return ['http:', 'https:'].includes(url.protocol) || value === 'about:blank' ||
        (!navigation && ['ws:', 'wss:', 'data:', 'blob:'].includes(url.protocol));
    } catch { return false; }
  }
  // A refusal before anything reaches the guest says so (dispatched false):
  // the dispatcher reports it to the caller as refused. Thrown bare, it read
  // as an unknown outcome, which latches the owner and closes the whole
  // browser controller: one disallowed URL (a data: navigation) left every
  // agent's preview_* refused until the app restarted.
  function refused(message, Kind = Error) {
    return Object.assign(new Kind(message), { dispatched: false });
  }
  function validURL(value) {
    if (typeof value !== 'string' || value.length > 32768 || !urlAllowed(value)) throw refused('Disallowed guest URL', TypeError);
    return value;
  }
  function target(tab) { return Object.freeze({ owner: tab.owner, tabId: tab.id, generation: tab.generation }); }
  function allowed(operation, tab, extra = {}) {
    const lineage = tab.opener ? { opener: tab.opener, openerDocument: tab.openerDocument } : {};
    try { return authorize(Object.freeze({ operation, ...target(tab), createId: tab.createId ?? null, ...lineage, ...extra })) === true; }
    catch { return false; }
  }
  function demand(operation, tab, extra) {
    alive();
    if (!allowed(operation, tab, extra)) throw new Error(`Guest ${operation} refused`);
  }
  function emit(type, tab, extra = {}) {
    // Product listeners must not be able to abort Chromium lifecycle cleanup.
    try { onEvent(Object.freeze({ type, ...target(tab), ...extra })); } catch {}
  }
  function get(ref) {
    alive();
    if (!ref || typeof ref !== 'object') throw new TypeError('Expected guest target');
    identifier(ref.owner, 'owner'); identifier(ref.tabId, 'tab id');
    const tab = owners.get(ref.owner)?.tabs.get(ref.tabId);
    if (!tab || tab.closed || !Number.isSafeInteger(ref.generation) || tab.generation !== ref.generation) throw new Error('Stale or foreign guest target');
    return tab;
  }
  function creationError(code, message) {
    const error = new Error(message); error.code = code; return error;
  }
  function reserveCreation(owner, createId) {
    if (createId === null) return null; // Legacy calls get no teardown capability.
    if (creations.has(createId)) throw new Error('Guest create id already used or consumed');
    // Retain terminal tombstones; never evict/reuse an ID to make room. Exhaustion
    // refuses only the next create and never disposes unrelated active guests.
    if (creations.size >= 1024) throw new Error('Guest creation ledger full');
    const entry = { owner, createId, state: 'outstanding', tab: null, prepared: false, reject: null, canceled: null, abortReceipt: null };
    entry.canceled = new Promise((_resolve, reject) => { entry.reject = reject; });
    entry.canceled.catch(() => {}); // Abort may occur reentrantly during created.
    creations.set(createId, entry); return entry;
  }
  function creationEntry(owner, createId) {
    alive(); identifier(owner, 'owner'); identifier(createId, 'create id');
    const entry = creations.get(createId);
    if (!entry || entry.owner !== owner) throw new Error('Unknown or foreign guest creation');
    return entry;
  }
  function cancelCreation(entry, state) {
    if (!entry || entry.state !== 'outstanding') return;
    entry.state = state; entry.tab = null;
    const reject = entry.reject; entry.reject = null; entry.canceled = null;
    reject?.(creationError(state === 'aborted' ? 'GUEST_CREATION_ABORTED' : 'GUEST_CREATION_RETIRED',
      state === 'aborted' ? 'Guest creation aborted' : 'Guest creation retired'));
  }
  function creationOutstanding(entry) {
    if (entry && entry.state !== 'outstanding') throw creationError(
      entry.state === 'aborted' ? 'GUEST_CREATION_ABORTED' : 'GUEST_CREATION_RETIRED', 'Guest creation ' + entry.state);
  }
  function creationReady(tab) {
    if (tab.creation?.state === 'outstanding') throw new Error('Guest creation incomplete');
  }
  function loseFileGuard(tab, reason) {
    if (tab.closed || tab.fileGuard === 'lost') return;
    tab.fileGuard = 'lost';
    // Latch before callbacks or focus changes. No same-tab automatic recovery:
    // a new debugger attachment does not prove the old interception survived.
    if (presented?.tab === tab) detach();
    emit('file-guard-lost', tab, { reason });
  }
  function fileGuardReady(tab) {
    if (tab.fileGuard === 'installed') {
      let attached = false;
      try { attached = tab.debuggerOwned && tab.wc.debugger.isAttached(); } catch {}
      if (!attached) loseFileGuard(tab, 'attachment-missing');
    }
    return tab.fileGuard === 'installed';
  }
  function demandFileGuard(tab, dispatched = false) {
    if (fileGuardReady(tab)) return;
    const error = new Error('Guest file guard unavailable; explicit teardown and fresh creation required');
    error.code = 'GUEST_FILE_GUARD_LOST';
    if (dispatched) { error.dispatched = true; error.unknown = true; }
    throw error;
  }
  function summaries(owner) {
    const state = owners.get(owner);
    return Object.freeze(state ? [...state.tabs.values()].map(tab => Object.freeze({ ...target(tab), url: tab.url, title: tab.title, active: state.active === tab.id })) : []);
  }
  function changed(tab) { emit('tabs', tab, { tabs: summaries(tab.owner) }); }
  function documentReady(tab) {
    if (!recorderConfig) return false;
    try {
      const doc = tab.documents.get(tab.wc.mainFrame?.frameTreeNodeId);
      return !!doc?.ready && !doc.retiring && doc.generation === tab.generation && !doc.frame.detached;
    } catch { return false; }
  }
  function documentIdentity(tab, doc) {
    return Object.freeze({ documentNonce: doc.nonce, frameTreeNodeId: doc.frame.frameTreeNodeId,
      frameToken: doc.frameToken, webContentsId: tab.wc.id });
  }
  function historySnapshot(tab) {
    return { canGoBack: tab.wc.navigationHistory.canGoBack(), canGoForward: tab.wc.navigationHistory.canGoForward(),
      isLoadingMainFrame: tab.wc.isLoadingMainFrame() };
  }
  function navigationReady(tab) {
    demandFileGuard(tab);
    if (!tab.ready || (recorderConfig && !documentReady(tab))) throw new Error('Guest not ready');
    creationReady(tab);
  }
  function dispatchNavigation(ref, action, invoke) {
    const tab = get(ref); navigationReady(tab);
    const original = target(tab);
    demand('navigate', tab, { navigationAction: action });
    // A trusted policy callback must not make an old lease control a replacement.
    get(original); navigationReady(tab);
    try { invoke(tab); }
    catch {
      const error = new Error('Guest native navigation outcome unknown; do not retry automatically');
      error.code = 'GUEST_NATIVE_DISPATCH_UNKNOWN'; error.dispatched = true; error.unknown = true;
      error.target = original; error.action = action; throw error;
    }
    return Object.freeze({ target: original, action, dispatched: true, loadComplete: false });
  }
  function observeLoad(tab, nativeEvent) {
    if (disposed || tab.closed) return;
    try {
      if (tab.wc.isDestroyed()) return;
      const frame = tab.wc.mainFrame;
      if (!frame || frame.detached) return;
      const currentFrame = Object.freeze({ frameTreeNodeId: frame.frameTreeNodeId, frameToken: frame.frameToken, webContentsId: tab.wc.id });
      const doc = tab.documents.get(frame.frameTreeNodeId);
      const ready = documentReady(tab) && doc.frameToken === frame.frameToken;
      const state = historySnapshot(tab);
      emit('load-state', tab, { serial: ++loadObservationSerial, nativeEvent, ...state,
        frame: currentFrame, document: ready ? documentIdentity(tab, doc) : null, mainDocumentReady: ready,
        finishObserved: nativeEvent === 'did-finish-load' });
    } catch { /* A disappearing native frame yields no observation or completion. */ }
  }
  function retireRecorderDocuments(tab, entries, reason) {
    const retiring = entries.filter(([, doc]) => !doc.retiring).map(([key, doc]) => ({ key, doc, document: documentIdentity(tab, doc) }));
    // Mark the whole fallback batch before any trusted callback can reenter IPC.
    for (const { doc } of retiring) { doc.retiring = true; doc.ready = false; doc.enabled = false; }
    for (const { key, doc, document } of retiring) {
      emit('recorder-document-retired', tab, { mainFrame: false, reason, document });
      if (tab.documents.get(key) === doc) tab.documents.delete(key);
    }
  }
  function recordingEnabled(tab, doc = tab.documents.get(tab.wc.mainFrame?.frameTreeNodeId)) {
    return !!doc && !doc.retiring && recordingOwners.has(tab.owner) && allowed('record-event', tab, { document: documentIdentity(tab, doc) });
  }
  function syncRecordingControls() {
    if (!recorderConfig || disposed) return;
    for (const state of owners.values()) for (const tab of state.tabs.values()) for (const doc of tab.documents.values()) {
      const enabled = doc.ready && doc.generation === tab.generation && recordingEnabled(tab, doc);
      try {
        if (doc.enabled !== enabled && !doc.frame.detached) {
          doc.enabled = enabled;
          doc.frame.send(channels.control, { documentNonce: doc.nonce, enabled });
        }
      } catch { doc.ready = false; }
    }
  }
  function currentInput(tab) {
    return !disposed && !tab.closed && fileGuardReady(tab) && tab.ready && (!recorderConfig || documentReady(tab)) &&
      tab.creation?.state !== 'outstanding' &&
      presented?.tab === tab && allowed('direct-input', tab) && tab.fileGuard === 'installed' && presented?.tab === tab;
  }
  function detach() {
    if (!presented) return;
    const old = presented; presented = null;
    // BrowserWindow's closed event can arrive after its native view is gone.
    // A destroyed WC must never receive focus/query calls during cleanup.
    if (!window.isDestroyed()) {
      old.tab.view.setVisible(false);
      window.contentView.removeChildView(old.tab.view);
      if (!old.tab.wc.isDestroyed() && old.tab.wc.isFocused() && !window.webContents.isDestroyed()) window.webContents.focus();
    }
  }
  function bounds(value) {
    const size = window.getContentBounds();
    if (!value || !['x', 'y', 'width', 'height'].every(key => Number.isSafeInteger(value[key])) ||
        value.x < 0 || value.y < 0 || value.width < 1 || value.height < 1 ||
        value.x + value.width > size.width || value.y + value.height > size.height) throw new RangeError('Guest bounds outside content');
    return { x: value.x, y: value.y, width: value.width, height: value.height };
  }
  function display(tab, rect) {
    detach();
    tab.view.setBounds(rect);
    window.contentView.addChildView(tab.view);
    tab.view.setVisible(true);
    presented = { tab, bounds: rect };
    reconcileInput();
  }
  function reconcileInput() {
    syncRecordingControls();
    if (!presented) return;
    const { tab } = presented;
    if (tab.wc.isDestroyed() || window.isDestroyed()) return;
    const enabled = currentInput(tab);
    tab.wc.setIgnoreMenuShortcuts(!enabled);
    if (!enabled && tab.wc.isFocused()) window.webContents.focus();
    emit('input-policy', tab, { enabled });
  }
  function retire(tab, closeContents) {
    if (tab.closed) return;
    cancelCreation(tab.creation, 'retired');
    if (presented?.tab === tab) detach();
    tab.closed = true;
    clearTimeout(tab.popupTimer); tab.documents.clear(); contents.delete(tab.wc);
    const state = owners.get(tab.owner);
    state.tabs.delete(tab.id);
    if (state.active === tab.id) state.active = state.tabs.keys().next().value ?? null;
    if (!state.tabs.size) { owners.delete(tab.owner); recordingOwners.delete(tab.owner); }
    try {
      if (closeContents && !tab.wc.isDestroyed()) tab.wc.close({ waitForBeforeUnload: false });
    } catch (error) { tab.closeUnknown = true; throw error; }
    emit('closed', tab); changed(tab);
  }
  function creationFailure(error, tab) {
    // Retain this actual root before retirement consumes its creation ledger
    // entry. Never use an error's supplied target/cleanup or a newer active tab.
    const original = target(tab);
    let closeUnknown = tab.closeUnknown;
    try { retire(tab, true); } catch { closeUnknown = true; }
    let nativeDestroyed = false;
    try { nativeDestroyed = tab.wc.isDestroyed() === true; } catch { closeUnknown = true; }
    const receipt = Object.freeze({ owner: tab.owner, createId: tab.createId, target: original,
      closed: tab.closed, nativeDestroyed, closeUnknown: closeUnknown || tab.closeUnknown });
    // Frozen/primitive rejection values or non-configurable foreign properties
    // cannot be extended safely. Preserve them as cause rather than losing the
    // actual snapshot or mistaking a caller-supplied receipt for module evidence.
    const descriptor = { value: receipt, writable: false, configurable: false, enumerable: false };
    try { Object.defineProperty(error, 'creationCleanup', descriptor); return error; }
    catch {
      const wrapped = new Error('Guest creation failed', { cause: error });
      Object.defineProperty(wrapped, 'creationCleanup', descriptor); return wrapped;
    }
  }

  // This partition belongs exclusively to this manager. Blocking the trusted
  // origin also covers subframes/fetches, redirects, and programmatic navigation.
  guestSession.webRequest.onBeforeRequest((details, callback) => callback({ cancel: !urlAllowed(details.url, false) }));
  guestSession.setPermissionRequestHandler((_wc, _permission, callback) => callback(false));
  guestSession.setPermissionCheckHandler(() => false);
  const denyDownload = (event, _item, wc) => {
    event.preventDefault();
    const tab = [...owners.values()].flatMap(state => [...state.tabs.values()]).find(item => item.wc === wc);
    if (tab) emit('download-blocked', tab);
  };
  guestSession.on('will-download', denyDownload);

  function recorderFailure(tab, code) {
    if (tab.closed) return;
    emit('recorder-failed', tab, { code }); retire(tab, true);
  }
  function recorderSender(event) {
    const tab = contents.get(event.sender), frame = event.senderFrame;
    if (disposed || !tab || tab.closed || event.sender === window.webContents || event.sender.session !== guestSession ||
        !frame || frame.detached || !Number.isSafeInteger(frame.frameTreeNodeId) ||
        typeof frame.frameToken !== 'string' || !frame.frameToken ||
        (frame !== tab.wc.mainFrame && frame.top !== tab.wc.mainFrame)) return null;
    return { tab, frame };
  }
  function recorderInit(event, payload) {
    // returnValue immediately consumes Electron's synchronous reply channel.
    // Each path must send only its final response, never a provisional denial.
    const sender = recorderSender(event);
    if (!sender) { event.returnValue = { v: 1, ok: false }; return; } // Unknown popup ordering is not inferred from payload.
    const { tab, frame } = sender;
    if (!shape(payload, ['v', 'recorderSourceSha256']) || payload.v !== 1 || payload.recorderSourceSha256 !== RECORDER_SOURCE_SHA256) {
      event.returnValue = { v: 1, ok: false }; recorderFailure(tab, 'INIT_SOURCE_OR_SHAPE'); return;
    }
    const previous = tab.documents.get(frame.frameTreeNodeId);
    if (previous?.retiring) { event.returnValue = { v: 1, ok: false }; return; }
    const doc = previous && previous.generation === tab.generation && previous.frameToken === frame.frameToken ? previous : {
      frame, frameToken: frame.frameToken, generation: tab.generation, nonce: randomUUID(), ready: false, enabled: false,
    };
    tab.documents.set(frame.frameTreeNodeId, doc);
    event.returnValue = { v: 1, documentNonce: doc.nonce, enabled: recordingEnabled(tab, doc), recorderSourceSha256: RECORDER_SOURCE_SHA256 };
  }
  function recorderDocument(event, payload, keys) {
    const sender = recorderSender(event);
    if (!sender || !shape(payload, keys)) return null;
    const doc = sender.tab.documents.get(sender.frame.frameTreeNodeId);
    if (!doc || doc.retiring || doc.generation !== sender.tab.generation || doc.frameToken !== sender.frame.frameToken || doc.nonce !== payload.documentNonce) return null;
    return { ...sender, doc };
  }
  function recorderReady(event, payload) {
    const item = recorderDocument(event, payload, ['documentNonce', 'recorderSourceSha256']);
    if (!item) { event.returnValue = { v: 1, ok: false }; return; }
    const { tab, frame, doc } = item;
    if (payload.recorderSourceSha256 !== RECORDER_SOURCE_SHA256) { event.returnValue = { v: 1, ok: false }; recorderFailure(tab, 'READY_SOURCE'); return; }
    doc.ready = true; doc.enabled = recordingEnabled(tab, doc);
    event.returnValue = { v: 1, ok: true, enabled: doc.enabled };
    emit('recorder-ready', tab, { mainFrame: frame === tab.wc.mainFrame, document: documentIdentity(tab, doc) });
    if (tab.popupPending && frame === tab.wc.mainFrame) {
      clearTimeout(tab.popupTimer); tab.popupPending = false; tab.ready = true;
      emit('popup', tab, { opener: tab.opener, openerDocument: tab.openerDocument, disposition: tab.disposition,
        document: documentIdentity(tab, doc), preDocumentReady: true }); changed(tab);
    }
  }
  function recorderRecord(event, payload) {
    const item = recorderDocument(event, payload, ['documentNonce', 'event']);
    if (!item || !item.doc.ready || !recordingEnabled(item.tab, item.doc) || !shape(payload.event, ['step'])) return;
    const step = recordedStep(payload.event.step);
    if (!step) return;
    step.url = item.tab.wc.mainFrame.url; step.at = Date.now();
    step.frame = item.frame === item.tab.wc.mainFrame ? '' : item.frame.url;
    emit('record', item.tab, { step: Object.freeze(step), document: documentIdentity(item.tab, item.doc) });
  }
  if (recorderConfig) {
    // Handlers exist before the session preload, and before any WC is created.
    electron.ipcMain.on(channels.init, recorderInit);
    electron.ipcMain.on(channels.ready, recorderReady);
    electron.ipcMain.on(channels.record, recorderRecord);
    try { registeredPreload = guestSession.registerPreloadScript({ type: 'frame', filePath: recorderConfig.preloadPath }); }
    catch (error) {
      electron.ipcMain.removeListener(channels.init, recorderInit); electron.ipcMain.removeListener(channels.ready, recorderReady);
      electron.ipcMain.removeListener(channels.record, recorderRecord); SESSION_OWNERS.delete(guestSession); throw error;
    }
  }

  function attachDebugger(tab, initialization) {
    if (tab.debuggerOwned && tab.wc.debugger.isAttached()) return;
    if (!initialization || tab.fileGuard !== 'new') { loseFileGuard(tab, 'attachment-missing'); demandFileGuard(tab); }
    if (tab.wc.debugger.isAttached()) throw new Error('Guest debugger already owned');
    tab.wc.debugger.attach('1.3'); tab.debuggerOwned = true;
  }
  async function send(tab, ref, method, parameters, initialization, fixedGuardSetup) {
    get(ref);
    if (initialization && !tab.preparing) throw refused('Guest preparation has ended');
    if ((!tab.ready || (recorderConfig && !documentReady(tab))) && !initialization) throw refused('Guest not ready');
    if (!initialization) creationReady(tab);
    if (!fixedGuardSetup && initialization && !PREPARATION_METHODS.has(method)) throw refused('Not a guest initialization command');
    if (!fixedGuardSetup && !CDP_METHODS.has(method)) throw refused('CDP method outside guest broker');
    const params = jsonObject(parameters);
    if (fixedGuardSetup) {
      const valid = initialization && ((method === 'Page.enable' && tab.fileGuard === 'new' && Object.keys(params).length === 0) ||
        (method === 'Page.setInterceptFileChooserDialog' && tab.fileGuard === 'page-enabled' &&
          Object.keys(params).length === 2 && params.enabled === true && params.cancel === true));
      if (!valid) { loseFileGuard(tab, 'invalid-setup'); demandFileGuard(tab); }
    } else if (!initialization || tab.fileGuard === 'lost') demandFileGuard(tab);
    if ('sessionId' in params) throw refused('Foreign CDP session refused');
    if (method === 'Page.navigate') {
      validURL(params.url);
      if (params.frameId) throw refused('Frame navigation requires an owner-scoped adapter');
    }
    demand('cdp', tab, { method, initialization });
    get(ref);
    if (tab.fileGuard === 'lost') demandFileGuard(tab);
    attachDebugger(tab, initialization);
    let timer;
    const answer = await Promise.race([
      tab.wc.debugger.sendCommand(method, params),
      new Promise((_resolve, reject) => { timer = setTimeout(() => {
        const error = new Error('Guest CDP timed out; command may have run');
        error.code = 'GUEST_COMMAND_TIMEOUT'; error.dispatched = true; reject(error);
      }, commandTimeoutMs); }),
    ]).finally(() => clearTimeout(timer));
    // A detach/reset while awaiting a response cannot re-establish protection.
    // Public command side effects may already have happened; never retry them.
    if (tab.fileGuard === 'lost' || !tab.debuggerOwned || !tab.wc.debugger.isAttached()) {
      loseFileGuard(tab, 'attachment-missing'); demandFileGuard(tab, true);
    }
    if (fixedGuardSetup) tab.fileGuard = method === 'Page.enable' ? 'page-enabled' : 'installed';
    // Never return an old document's result as a replacement tab's result.
    try { get(NAVIGATION_METHODS.has(method) ? target(tab) : ref); }
    catch {
      const error = new Error('Guest changed after command dispatch; do not retry automatically');
      error.code = 'GUEST_STALE_AFTER_DISPATCH'; error.dispatched = true; throw error;
    }
    return Object.freeze({ target: target(tab), result: answer });
  }
  function enqueue(tab, ref, method, params, initialization = false, fixedGuardSetup = false) {
    // Snapshot caller-owned objects now, then recheck both lease and authorization
    // when this command reaches the guest. A failed command does not stall a lane.
    const lease = Object.freeze({ ...target(get(ref)) });
    if (!initialization) demandFileGuard(tab);
    if ((!tab.ready || (recorderConfig && !documentReady(tab))) && !initialization) throw new Error('Guest not ready');
    if (!initialization) creationReady(tab);
    const parameters = jsonObject(params);
    const result = tab.queue.then(() => send(tab, lease, method, parameters, initialization, fixedGuardSetup));
    tab.queue = result.catch(() => {});
    return result;
  }
  function wire(owner, suppliedContents, context = {}) {
    if (suppliedContents && (contents.has(suppliedContents) || suppliedContents === window.webContents || suppliedContents.session !== guestSession)) throw new Error('Foreign WebContents refused');
    const view = new electron.WebContentsView(suppliedContents ? { webContents: suppliedContents } : {
      webPreferences: {
        session: guestSession, nodeIntegration: false,
        // Electron also gates sandboxed iframe preloads on this setting. Enable
        // it only for an opted-in isolated recorder; Node stays disabled and
        // sandbox/context isolation stay explicit for every guest frame.
        nodeIntegrationInSubFrames: recorderConfig?.subframePreload === true,
        nodeIntegrationInWorker: false, contextIsolation: true, sandbox: true,
        webSecurity: true, webviewTag: false, allowRunningInsecureContent: false,
        navigateOnDragDrop: false, safeDialogs: true,
        additionalArguments: recorderArguments,
      },
    });
    const wc = view.webContents;
    if (wc === window.webContents || wc.session !== guestSession) throw new Error('Foreign WebContents refused');
    const tab = { owner, id: `guest-${registryId}-${++nextId}`, generation: 0, view, wc, url: 'about:blank', title: '', closed: false, ready: false, debuggerOwned: false,
      queue: Promise.resolve(), documents: new Map(), popupPending: !!context.opener, popupTimer: null,
      fileGuard: 'new', preparing: false, closeUnknown: false,
      createId: context.createId ?? null, opener: context.opener ?? null, openerDocument: context.openerDocument ?? null,
      creation: context.creation ?? null,
      disposition: context.disposition ?? null };
    contents.set(wc, tab);
    view.setVisible(false);
    let state = owners.get(owner);
    if (!state) { state = { tabs: new Map(), active: null }; owners.set(owner, state); }
    state.tabs.set(tab.id, tab); state.active ??= tab.id;
    if (tab.creation) tab.creation.tab = tab; // Before any created callback.
    const guardNavigation = (event, value) => {
      if (!urlAllowed(typeof value === 'string' ? value : event.url)) event.preventDefault();
    };
    wc.on('will-navigate', guardNavigation);
    wc.on('will-redirect', guardNavigation);
    wc.on('will-frame-navigate', guardNavigation);
    wc.on('will-attach-webview', event => event.preventDefault());
    wc.on('did-start-navigation', (event, _url, legacyInPlace, legacyMainFrame) => {
      const isInPlace = event.isSameDocument ?? legacyInPlace;
      const isMainFrame = event.isMainFrame ?? legacyMainFrame;
      if (isInPlace || tab.closed) return;
      if (!isMainFrame) {
        const mainKey = tab.wc.mainFrame?.frameTreeNodeId;
        const entries = [...tab.documents.entries()].filter(([key]) => key !== mainKey && (!event.frame || key === event.frame.frameTreeNodeId));
        retireRecorderDocuments(tab, entries, event.frame ? 'subframe-navigation' : 'subframe-navigation-unresolved');
        return;
      }
      tab.documents.clear();
      tab.generation++; emit('generation', tab); reconcileInput();
    });
    for (const name of ['did-navigate', 'did-navigate-in-page']) wc.on(name, (_event, url, ...args) => {
      if (name === 'did-navigate-in-page' && args[0] === false) return;
      if (!tab.closed) { tab.url = url; emit('navigated', tab, { url }); changed(tab); }
    });
    wc.on('page-title-updated', (_event, title) => { if (!tab.closed) { tab.title = title; changed(tab); } });
    for (const name of ['did-start-loading', 'did-stop-loading', 'did-finish-load']) wc.on(name, () => observeLoad(tab, name));
    for (const name of ['before-input-event', 'before-mouse-event', 'context-menu']) wc.on(name, (event, input) => {
      if (!currentInput(tab)) { event.preventDefault(); emit('input-blocked', tab, { inputType: input?.type ?? name }); }
    });
    wc.on('focus', () => { if (!currentInput(tab)) window.webContents.focus(); });
    wc.on('render-process-gone', () => { loseFileGuard(tab, 'render-process-gone'); tab.documents.clear(); tab.ready = false; tab.generation++; emit('crashed', tab); if (presented?.tab === tab) detach(); });
    wc.on('preload-error', () => { if (recorderConfig) recorderFailure(tab, 'PRELOAD_ERROR'); });
    wc.on('dom-ready', () => {
      if (recorderConfig && !documentReady(tab)) recorderFailure(tab, 'DOCUMENT_NOT_READY');
    });
    wc.on('destroyed', () => retire(tab, false));
    wc.debugger.on('detach', () => {
      tab.debuggerOwned = false;
      loseFileGuard(tab, 'debugger-detached');
      // Lost bindings/init scripts require a deliberate recorder re-preparation
      // by the integration. The detach event must not be treated as Teach-ready.
      emit('debugger-detached', tab);
    });
    wc.debugger.on('message', (_event, method, params, sessionId) => {
      if (!sessionId && ['Inspector.detached', 'Inspector.targetCrashed'].includes(method)) loseFileGuard(tab, method);
      // No general Browser/Target bridge and no sessions from another target.
      if (!tab.closed && !sessionId && /^(Runtime|Page|DOM|Accessibility)\./.test(method)) emit('cdp', tab, { method, params });
    });
    wc.setWindowOpenHandler(details => {
      let accepted = false;
      try { accepted = fileGuardReady(tab) && !!registeredPreload && recorderConfig.popupPreloadVerified && documentReady(tab) &&
        tab.ready && urlAllowed(details.url) && allowed('popup', tab) &&
        popupPolicy(Object.freeze({ ...target(tab), url: details.url, disposition: details.disposition })) === true; } catch {}
      if (!accepted) { emit('popup-blocked', tab); return { action: 'deny' }; }
      const opener = target(tab);
      const openerDocument = documentIdentity(tab, tab.documents.get(tab.wc.mainFrame.frameTreeNodeId));
      const popupURL = details.url, disposition = details.disposition, hasPost = !!details.postBody;
      return {
        action: 'allow',
        overrideBrowserWindowOptions: { webPreferences: {
          session: guestSession, preload: '', nodeIntegration: false,
          nodeIntegrationInSubFrames: false, nodeIntegrationInWorker: false,
          contextIsolation: true, sandbox: true, webSecurity: true, webviewTag: false,
          navigateOnDragDrop: false, safeDialogs: true,
          additionalArguments: recorderArguments,
        } },
        createWindow(options) {
          // Recheck after Chromium's deferred background-tab creation.
          get(opener);
          if (!allowed('popup', tab)) throw new Error('Stale popup owner');
          if (options.webContents?.opener && options.webContents.opener !== tab.wc.mainFrame && options.webContents.opener.top !== tab.wc.mainFrame) throw new Error('Foreign popup opener refused');
          if (!options.webContents && (disposition !== 'background-tab' || hasPost)) throw new Error('Unsupported deferred popup');
          const child = wire(tab.owner, options.webContents, { opener, openerDocument, disposition });
          // Do not reconstruct window.open via loadURL: that loses opener/POST.
          // Deferred background tabs have no supplied WebContents, and need it.
          child.popupTimer = setTimeout(() => recorderFailure(child, 'POPUP_NOT_READY'), commandTimeoutMs);
          emit('popup-pending', child, { opener, openerDocument, disposition, preDocumentReady: false });
          changed(child);
          if (!options.webContents) {
            void child.wc.loadURL(popupURL).catch(() => emit('navigation-failed', child));
          }
          return child.wc;
        },
      };
    });
    emit('created', tab, { createId: tab.createId, opener: tab.opener, openerDocument: tab.openerDocument }); changed(tab);
    return tab;
  }

  const resize = () => {
    if (!presented) return;
    try { bounds(presented.bounds); } catch { detach(); }
  };
  window.on('resize', resize);
  window.once('closed', dispose);
  function dispose() {
    if (disposed) return;
    detach(); disposed = true;
    for (const state of [...owners.values()]) for (const tab of [...state.tabs.values()]) retire(tab, true);
    window.removeListener('resize', resize); window.removeListener('closed', dispose);
    guestSession.removeListener('will-download', denyDownload);
    if (registeredPreload) guestSession.unregisterPreloadScript(registeredPreload);
    if (channels) {
      electron.ipcMain.removeListener(channels.init, recorderInit);
      electron.ipcMain.removeListener(channels.ready, recorderReady);
      electron.ipcMain.removeListener(channels.record, recorderRecord);
    }
    SESSION_OWNERS.delete(guestSession);
    // Request/permission guards intentionally remain fail-closed for in-flight
    // requests. Another manager must not share this partition concurrently.
  }

  return Object.freeze({
    async create({ owner, url = 'about:blank', createId = null }) {
      identifier(owner, 'owner'); validURL(url); alive();
      if (createId !== null) identifier(createId, 'create id');
      demand('create', { owner, id: null, generation: 0 }, { createId });
      const creation = reserveCreation(owner, createId), canceled = creation?.canceled;
      const wait = promise => creation ? Promise.race([promise, canceled]) : promise;
      let tab;
      try {
        tab = wire(owner, undefined, { createId, creation });
        creationOutstanding(creation);
        // Materialize the initial target first. Electron documents that debugger
        // commands sent before any navigation can otherwise remain unresolved.
        // about:blank contains no untrusted document scripts.
        await wait(tab.wc.loadURL('about:blank'));
        creationOutstanding(creation);
        get(target(tab));
        // Trusted integration installs binding/init scripts before first load.
        // This callback receives a scoped broker, never the application's WC.
        const preparedTarget = target(tab);
        tab.preparing = true;
        try {
          // The callback offers this actual target to the service and obtains
          // its preparation-only grant. Fixed setup must not borrow a grant
          // before that offer or from a later target/revision.
          await wait(prepareGuest(Object.freeze({ target: preparedTarget, createId: tab.createId, command: (method, params = {}) => enqueue(tab, preparedTarget, method, params, true) })));
          // Fixed module-owned setup, authorized only as initialization. Never
          // expose a chooser toggle or local file paths through public CDP.
          await wait(enqueue(tab, preparedTarget, 'Page.enable', {}, true, true));
          await wait(enqueue(tab, preparedTarget, 'Page.setInterceptFileChooserDialog', { enabled: true, cancel: true }, true, true));
        } finally { tab.preparing = false; }
        creationOutstanding(creation);
        if (tab.closed || disposed) throw new Error('Guest closed during preparation');
        demandFileGuard(tab);
        tab.ready = true;
        await wait(tab.wc.loadURL(url));
        creationOutstanding(creation);
        if (recorderConfig && !documentReady(tab)) throw new Error('Guest recorder not ready');
        get(target(tab));
        demandFileGuard(tab);
        if (creation) creation.prepared = true;
        return target(tab);
      } catch (error) {
        const owned = tab ?? creation?.tab;
        if (owned) throw creationFailure(error, owned);
        cancelCreation(creation, 'retired');
        throw error;
      }
    },
    completeCreation({ owner, createId, target: ref }) {
      const entry = creationEntry(owner, createId);
      if (entry.state !== 'outstanding') throw new Error('Guest creation already consumed or ' + entry.state);
      if (!entry.prepared) throw new Error('Guest creation still pending or incomplete');
      const tab = get(ref);
      if (tab !== entry.tab || tab.owner !== owner || tab.createId !== createId) throw new Error('Foreign creation target');
      demandFileGuard(tab);
      if (!tab.ready || (recorderConfig && !documentReady(tab))) throw new Error('Guest not ready');
      const result = Object.freeze({ owner, createId, target: target(tab), completed: true });
      entry.state = 'completed'; entry.tab = null; entry.reject = null; entry.canceled = null;
      return result;
    },
    abortCreation({ owner, createId }) {
      const entry = creationEntry(owner, createId);
      if (entry.state === 'aborted') return Object.freeze({ ...entry.abortReceipt, alreadyAborted: true });
      if (entry.state !== 'outstanding') throw new Error('Guest creation already consumed or ' + entry.state);
      const tab = entry.tab;
      if (!tab || tab.closed) throw new Error('Guest creation not materialized');
      const original = target(tab);
      // closed is registry retirement; nativeDestroyed is an honest immediate
      // native observation, not a promise that asynchronous teardown completed.
      entry.abortReceipt = Object.freeze({ owner, createId, target: original, closed: true, nativeDestroyed: false, alreadyAborted: false });
      cancelCreation(entry, 'aborted');
      try { retire(tab, true); }
      catch {
        entry.abortReceipt = Object.freeze({ ...entry.abortReceipt, closeUnknown: true });
        const error = creationError('GUEST_CREATION_ABORT_UNKNOWN', 'Guest creation native close outcome unknown');
        error.target = original; error.closed = tab.closed; error.unknown = true; error.dispatched = true; throw error;
      }
      entry.abortReceipt = Object.freeze({ ...entry.abortReceipt, nativeDestroyed: tab.wc.isDestroyed() });
      return entry.abortReceipt;
    },
    list(owner) {
      identifier(owner, 'owner'); demand('list', { owner, id: null, generation: 0 });
      return summaries(owner);
    },
    select(ref) {
      const tab = get(ref); demand('select', tab);
      demandFileGuard(tab);
      if (!tab.ready || (recorderConfig && !documentReady(tab))) throw new Error('Guest not ready');
      creationReady(tab);
      if (presented?.tab.owner === tab.owner) demand('present', tab);
      owners.get(tab.owner).active = tab.id;
      // Changing selection alone never moves a hidden owner's tab on screen.
      if (presented?.tab.owner === tab.owner) display(tab, presented.bounds);
      changed(tab); return target(tab);
    },
    present(ref, rect) {
      const tab = get(ref); demand('present', tab);
      demandFileGuard(tab);
      if (!tab.ready || (recorderConfig && !documentReady(tab))) throw new Error('Guest not ready');
      creationReady(tab);
      const checked = bounds(rect);
      owners.get(tab.owner).active = tab.id; display(tab, checked); changed(tab);
      return Object.freeze({ ...target(tab), bounds: checked, inputAllowed: currentInput(tab) });
    },
    // Local safety teardown must remain possible after service authority expires.
    hide() { alive(); detach(); },
    focus(ref) {
      const tab = get(ref); demand('focus', tab);
      if (!currentInput(tab)) throw new Error('Guest input refused');
      tab.wc.focus();
    },
    reconcileInput,
    historyState(ref) {
      const tab = get(ref); demand('list', tab); get(ref);
      return Object.freeze({ ...target(tab), ...historySnapshot(tab) });
    },
    navigateHistory(ref, direction) {
      if (!['back', 'forward'].includes(direction)) throw new TypeError('Invalid history direction');
      const tab = get(ref); navigationReady(tab);
      const original = target(tab);
      demand('navigate', tab, { navigationAction: direction }); get(original); navigationReady(tab);
      const available = direction === 'back' ? tab.wc.navigationHistory.canGoBack() : tab.wc.navigationHistory.canGoForward();
      if (!available) {
        const error = new Error('Guest history unavailable'); error.code = 'GUEST_HISTORY_UNAVAILABLE'; error.dispatched = false;
        throw error;
      }
      return dispatchNavigation(original, direction, current => {
        if (direction === 'back') current.wc.navigationHistory.goBack(); else current.wc.navigationHistory.goForward();
      });
    },
    reload(ref) { return dispatchNavigation(ref, 'reload', tab => tab.wc.reload()); },
    async navigate(ref, url) {
      const tab = get(ref); demand('navigate', tab); validURL(url);
      demandFileGuard(tab);
      if (!tab.ready || (recorderConfig && !documentReady(tab))) throw new Error('Guest not ready');
      creationReady(tab);
      await tab.wc.loadURL(url);
      demandFileGuard(tab, true);
      if (recorderConfig && !documentReady(tab)) { recorderFailure(tab, 'NAVIGATION_NOT_READY'); throw new Error('Guest recorder not ready'); }
      get(target(tab)); return target(tab);
    },
    setRecording(ref, enabled) {
      const tab = get(ref); demand('recording', tab);
      if (typeof enabled !== 'boolean') throw new TypeError('Expected recording boolean');
      if (!recorderConfig || (enabled && !documentReady(tab))) throw new Error('Guest recorder not ready');
      if (enabled) recordingOwners.add(tab.owner); else recordingOwners.delete(tab.owner);
      syncRecordingControls();
      return Object.freeze({ ...target(tab), requested: enabled, enabled: documentReady(tab) && recordingEnabled(tab) });
    },
    recorderStatus(ref) {
      const tab = get(ref); demand('list', tab);
      return Object.freeze({ ...target(tab), installed: !!registeredPreload, mainDocumentReady: documentReady(tab),
        enabled: documentReady(tab) && recordingEnabled(tab), popupPreloadVerified: recorderConfig?.popupPreloadVerified === true });
    },
    close(ref) { const tab = get(ref); demand('close', tab); retire(tab, true); },
    command(ref, method, params = {}) { const tab = get(ref); return enqueue(tab, ref, method, params); },
    dispose,
  });
}

module.exports = { createBrowserGuestManager };
