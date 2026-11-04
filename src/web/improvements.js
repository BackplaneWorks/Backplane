import { captureApp } from './improvements-capture.js';
import { softFocus } from './touch-focus.js';
import { ImprovementsStore } from './improvements-store.js';
import { ImprovementsFlow, improvementThread } from './improvements-flow.js';
import { backplaneProject } from './improvements-context.js';
import { osFamily, diagnosticReport, issueLink, IssueHandoff, ISSUE_URL } from './issue-report.js';
import { ReportWriter } from './report-writer.js';
import { startScreenRecording, recordingSupported, RECORDING } from './improvements-recording.js';
import { RecordingStore } from './improvements-recording-store.js';

export function improvementsShortcut(event, platform = '') {
  if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || event.repeat || event.getModifierState?.('AltGraph') || event.shiftKey) return false;
  const mac = /mac/i.test(platform);
  return (event.code === 'KeyI' || event.key?.toLowerCase() === 'i') && event.ctrlKey && (mac ? event.metaKey && !event.altKey : event.altKey && !event.metaKey);
}
export const improvementsChord = platform => /mac/i.test(platform) ? 'Ctrl+Cmd+I' : 'Ctrl+Alt+I';
export function clampPopup(x, y, width, height, viewport) {
  return { x: Math.max(viewport.x + 8, Math.min(x, viewport.x + viewport.width - width - 8)),
    y: Math.max(viewport.y + 8, Math.min(y, viewport.y + viewport.height - height - 8)) };
}
const timed = (promise, ms, message) => new Promise((resolve, reject) => {
  const timer = setTimeout(() => reject(Error(message)), ms);
  Promise.resolve(promise).then(value => { clearTimeout(timer);resolve(value); }, error => { clearTimeout(timer);reject(error); });
});
export function installImprovements({ rpc, context, identity, ready, select, beforeCapture = async () => {}, overlayChanged = () => {},
  capture = captureApp, store = new ImprovementsStore(), doc = document, win = window,
  record = startScreenRecording, recordingStore = new RecordingStore({ bridge: win.backplaneDesktop, storage: win.navigator.storage }),
  reportBody, reportPrompt, reportTemplate = () => '', previewBody = body => ({ body, url: issueLink(body) }), reportTarget = (saved, legacy) => ['issue', 'internal'].includes(saved) ? saved : legacy ? 'internal' : 'issue',
  appVersion = async () => 'unknown', diagnostics = {}, openIssue = url => {
    const link = doc.createElement('a');link.href = url;link.target = '_blank';link.rel = 'noopener noreferrer';link.click();
  } } = {}) {
  let draft, scope, opening = false, busy = false, capturing = false, previousFocus, previousRange, url, operation = 0, pos = null;
  let saving = Promise.resolve();
  let captureController, openEpoch = 0, projects = [], issuePreview = null, version = 'unknown';
  let recording, recordingAbort, recordingActive = false, recordingPicking = false, recordingFile, recordingUrl, previewRecordingId;
  const canRecord = recordingSupported(win);
  const chord = improvementsChord(win.navigator.platform);
  const el = (tag, text, attrs = {}) => { const n = doc.createElement(tag);if (text) n.textContent = text;for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, v);return n; };
  const popup = el('section', '', { class: 'improvements', role: 'dialog', 'aria-labelledby': 'improvements-title', hidden: '' });
  const header = el('header', '', { tabindex: '0', 'aria-label': 'Move Improvements with arrow keys' });
  header.append(el('strong', 'Improvements', { id: 'improvements-title' }), el('kbd', chord));
  const close = el('button', '×', { type: 'button', 'aria-label': 'Close Improvements' });header.append(close);
  const label = el('label', 'What would you like to improve?', { for: 'improvements-text' });
  const input = el('textarea', '', { id: 'improvements-text', rows: '5', placeholder: 'Describe the change…' });
  const projectLabel = el('label', 'Backplane source project', { for: 'improvements-project' });
  const project = el('select', '', { id: 'improvements-project' });
  const targetLabel = el('label', 'Destination', { for: 'improvements-target' });
  const target = el('select', '', { id: 'improvements-target' });
  target.append(el('option', 'Public GitHub issue', { value: 'issue' }), el('option', 'Internal implementation thread', { value: 'internal' }));
  const writerProjectLabel = el('label', 'Report agent project', { for: 'improvements-writer-project' });
  const writerProject = el('select', '', { id: 'improvements-writer-project' });
  const publicBox = el('div', '', { class: 'improvements-public' });
  const publicNote = el('p', 'Public on BackplaneWorks/Backplane, attributed to your GitHub account. Sign in on GitHub and press Submit there. Review your text and images for private information.');
  const check = (id, text) => { const label = el('label');const input = el('input', '', { type: 'checkbox', id });label.append(input, doc.createTextNode(' ' + text));return { label, input }; };
  const screenshot = check('improvements-screenshot', 'Select a screenshot (manual attachment on GitHub)');
  const logs = check('improvements-logs', 'Include sanitized diagnostics in the public issue');
  const extra = check('improvements-extra', 'Include additional details I enter');
  const details = el('textarea', '', { id: 'improvements-details', rows: '2', 'aria-label': 'Additional public details', placeholder: 'Optional reproduction steps or other details…', hidden: '' });
  const diagPreview = el('pre', '', { id: 'improvements-diagnostics', hidden: '' });
  const review = el('textarea', '', { id: 'improvements-review', rows: '8', 'aria-label': 'Editable public issue body', hidden: '' });
  const issueActions = el('div', '', { class: 'improvements-issue-actions', hidden: '' });
  const copy = el('button', 'Copy issue text', { type: 'button' });
  const download = el('button', 'Save report', { type: 'button' });
  const saveImage = el('button', 'Save selected image', { type: 'button', hidden: '' });
  const saveVideo = el('button', 'Save selected recording', { type: 'button', hidden: '' });
  const retryIssue = el('button', 'Allow another GitHub draft', { type: 'button', hidden: '' });
  const confirm = el('button', 'Open GitHub draft', { type: 'button', id: 'improvements-open-issue' });
  issueActions.append(copy, download, saveImage, saveVideo, retryIssue, confirm);
  const evidence = el('textarea', '', { id: 'improvements-evidence', rows: '3', 'aria-label': 'Behavior evidence', placeholder: 'Describe the behavior in detail. Include what you did and what you saw. An image can supplement this description.' });
  const evidenceOk = check('improvements-evidence-confirm', 'My report has a thorough behavior description, or I will attach the selected image or recording on GitHub.');
  const writer = el('button', 'Write report with agent', { type: 'button', id: 'improvements-write-report' });
  const loadReport = el('button', 'Load agent report', { type: 'button', hidden: '' });
  const openWriter = el('button', 'Open report-writing thread', { type: 'button', hidden: '' });
  const useTemplate = el('button', 'Use report template', { type: 'button' });
  publicBox.append(publicNote, el('p', 'Write report sends the selected description, image and diagnostics to your configured agent in the selected project using its normal permissions. Review issue prepares text locally. Replace private images with a redacted image before sharing.'), evidence, screenshot.label, logs.label, extra.label, details, writerProjectLabel, writerProject, writer, loadReport, openWriter, useTemplate, diagPreview, review, evidenceOk.label, issueActions);
  const recordingBox = el('div', '', { class: 'improvements-recording', hidden: '' });
  const recordButton = el('button', 'Record screen', { type: 'button', id: 'improvements-record-screen' });
  const discardVideo = el('button', 'Discard recording', { type: 'button', hidden: '' });
  const videoPreview = el('video', '', { controls: '', playsinline: '', preload: 'metadata', hidden: '', 'aria-label': 'Recording preview' });
  const attachVideo = check('improvements-recording-attach', 'Attach the recording to the local agent thread after previewing it');
  const videoWarning = el('p', 'A recording may show private messages, credentials or other private information. Review the whole clip before sharing it. Recording does not redact the image.');
  recordingBox.append(recordButton, el('p', 'Video only, no audio. Up to 2 minutes and 9 MiB. Private copies expire after 24 hours.'), videoWarning, videoPreview, attachVideo.label, discardVideo);
  const recordingBar = el('aside', '', { class: 'improvements-recording-bar', hidden: '', 'aria-label': 'Screen recording controls' });
  const recordingStatus = el('span', 'Recording screen', { role: 'status', 'aria-live': 'polite' });
  const stopRecording = el('button', 'Stop recording', { type: 'button' }), cancelRecording = el('button', 'Cancel recording', { type: 'button' });
  recordingBar.append(recordingStatus, stopRecording, cancelRecording);doc.body.append(recordingBar);
  const collection = el('details');collection.append(el('summary', 'Local diagnostics'));
  const collectionNote = el('p', 'Bounded connection and timing metadata only; no conversation text. Local collection is separate from telemetry. Nothing is shared until selected and reviewed.');
  const collect = check('improvements-collect', 'Keep local diagnostics on this device');
  const hubCollect = check('improvements-hub-collect', 'Keep local diagnostics on this hub');
  const clearLogs = el('button', 'Clear local diagnostics', { type: 'button' });
  collection.append(collectionNote, collect.label, hubCollect.label, clearLogs);
  const preview = el('img', '', { alt: 'App screenshot to send with this request', hidden: '' });
  const note = el('p', '', { role: 'status', 'aria-live': 'polite' });
  const help = el('p', '', { id: 'improvements-help' });
  const buttons = el('footer');
  const again = el('button', 'Capture again', { type: 'button' }), choose = el('button', 'Choose image', { type: 'button' });
  const file = el('input', '', { type: 'file', accept: 'image/png,image/jpeg,image/webp', hidden: '' });
  const send = el('button', 'Send', { type: 'button', class: 'primary', 'aria-describedby': 'improvements-help', 'aria-keyshortcuts': 'Control+Enter' });buttons.append(again, choose, send);
  popup.append(header, targetLabel, target, label, input, projectLabel, project, publicBox, preview, recordingBox, collection, help, note, buttons, file);doc.body.append(popup);
  function announce(text) { note.textContent = text; }
  function viewport() { const v = win.visualViewport;return { x: v?.offsetLeft || 0, y: v?.offsetTop || 0, width: v?.width || win.innerWidth, height: v?.height || win.innerHeight }; }
  function place() {
    if (popup.hidden) return;
    const v = viewport();
    popup.style.maxHeight = Math.max(100, v.height - 16) + 'px';
    if (v.width <= 600) { popup.style.left = v.x + 8 + 'px';popup.style.top = v.y + 8 + 'px';popup.style.width = v.width - 16 + 'px';return; }
    popup.style.width = '';
    const r = popup.getBoundingClientRect();pos = clampPopup(pos?.x ?? v.x + (v.width - r.width) / 2, pos?.y ?? v.y + 48, r.width, r.height, v);
    popup.style.left = pos.x + 'px';popup.style.top = pos.y + 'px';
  }
  const save = value => {
    const record = { ...value, updated: Date.now() }, savedScope = scope;
    saving = saving.catch(() => {}).then(() => timed(store.write(savedScope, record), 5000, 'Could not keep this draft on this device. Retry Send.'));
    return saving;
  };
  const isPublic = () => draft?.target === 'issue';
  function invalidateIssue(changed = true) {
    issuePreview = null;review.hidden = issueActions.hidden = true;
    if (draft && changed) { delete draft.issueBody;delete draft.issueOpened; }
  }
  function controls() {
    const internal = !isPublic(), locked = busy || !!draft?.message;
    recordingBox.hidden = !canRecord;recordButton.disabled = locked || !!draft?.key || recordingPicking || recordingActive;
    recordButton.textContent = recordingPicking ? 'Choose a screen or window…' : 'Record screen';
    videoPreview.hidden = !recordingFile;discardVideo.hidden = !draft?.recording;discardVideo.disabled = locked || !!draft?.key || recordingPicking || recordingActive;
    attachVideo.label.hidden = !draft?.recording;attachVideo.input.checked = draft?.includeRecording === true;
    attachVideo.input.disabled = locked || !!draft?.key || !recordingFile || previewRecordingId !== draft?.recording?.id || !draft?.recordingReviewed;
    attachVideo.label.lastChild.textContent = isPublic() ? ' Select this recording for the public report and agent. It may show private information. Attach it manually on GitHub.' : ' Attach this recording to the local agent thread. Play the preview first.';
    saveVideo.hidden = !draft?.includeRecording || !draft?.recording;saveVideo.disabled = busy || !draft?.recordingReviewed;
    input.disabled = !draft;input.readOnly = locked;
    target.value = draft?.target || 'issue';target.disabled = locked || !!draft?.key;
    project.disabled = !draft || locked || !!draft?.key;
    projectLabel.hidden = project.hidden = !internal || !!backplaneProject(projects) && draft?.project === backplaneProject(projects)?.id;
    publicBox.hidden = internal;
    screenshot.input.checked = draft?.includeScreenshot === true;logs.input.checked = draft?.includeDiagnostics === true;extra.input.checked = draft?.includeDetails === true;
    for (const node of [screenshot.input, logs.input, extra.input, details, evidence, writerProject, writer, useTemplate]) node.disabled = locked;
    loadReport.hidden = openWriter.hidden = !draft?.writer?.thread;
    loadReport.disabled = openWriter.disabled = locked;
    writer.disabled = locked || !!draft?.writer?.started;
    details.hidden = !draft?.includeDetails;details.readOnly = locked;
    const hasImage = internal || draft?.includeScreenshot;
    again.hidden = choose.hidden = !hasImage;
    again.disabled = choose.disabled = locked || capturing;
    preview.hidden = !draft?.image || !hasImage;
    saveImage.hidden = !draft?.image || !draft?.includeScreenshot;
    send.disabled = busy || capturing || recordingPicking || recordingActive;
    review.readOnly = busy;evidenceOk.input.disabled = busy;
    retryIssue.hidden = !draft?.issueOpened;retryIssue.disabled = busy;
    confirm.disabled = busy || !!draft?.issueOpened || !evidenceOk.input.checked;
    send.textContent = busy ? 'Working…' : internal ? draft?.accepted ? 'Open thread' : 'Send' : 'Review issue';
    help.textContent = internal ? !draft?.image && !(draft?.includeRecording && draft?.recording) ? 'Choose an image or preview and select a recording before sending.' : !draft?.project ? 'Choose the Backplane source project before sending.' : 'Changes will be made in Backplane. Ctrl+Enter sends this request.' :
      'OS family and app release are included. Images and recordings stay local until you send them to the agent or attach them yourself on GitHub. Ctrl+Enter reviews the issue.';
  }
  function showPreview() {
    if (url) URL.revokeObjectURL(url);url = draft?.image ? URL.createObjectURL(draft.image) : null;
    preview.hidden = !url;if (url) preview.src = url;else preview.removeAttribute('src');controls();place();
  }
  function show() { popup.hidden = false;popup.setAttribute('aria-modal', 'true');overlayChanged();showPreview();softFocus(input, { preventScroll: true }); }
  function hide() {
    openEpoch++;operation++;captureController?.abort();recordingAbort?.abort();recording?.cancel();recordingBar.hidden = true;recordingActive = recordingPicking = false;videoPreview.pause?.();capturing = false;opening = false;
    popup.hidden = true;popup.removeAttribute('aria-modal');overlayChanged();
    if (previousFocus?.isConnected) { previousFocus.focus({ preventScroll: true });if (previousRange && previousFocus.setSelectionRange) previousFocus.setSelectionRange(...previousRange); }
  }
  async function recapture(initial = false) {
    if (capturing || busy || draft?.message) return;
    const stamp = ++operation, focus = doc.activeElement, range = focus === input ? [input.selectionStart, input.selectionEnd, input.selectionDirection] : null;
    capturing = true;controls();
    const captureAbort = new AbortController();captureController = captureAbort;
    popup.hidden = true;popup.removeAttribute('aria-modal');
    try {
      await beforeCapture();
      const image = await timed(capture({ signal: captureAbort.signal }), 20000, 'Capture timed out. Capture again or choose an image.');
      if (stamp !== operation) return;
      if (!image?.size || !/^image\/(png|jpeg|webp)$/.test(image.type)) throw Error('Choose a PNG, JPEG or WebP image.');
      await save({ ...draft, image, path: '' });
      if (stamp !== operation) { await save(draft);return; }
      draft.image = image;draft.path = '';forgetReport();invalidateIssue();announce('Screenshot refreshed.');
    } catch { captureAbort.abort();if (stamp === operation) announce('Capture was not completed. Your draft and previous image are kept. Capture again or choose an image.'); }
    finally {
      if (stamp === operation) {
        capturing = false;
        popup.hidden = false;popup.setAttribute('aria-modal', 'true');overlayChanged();showPreview();
        (initial ? input : focus)?.focus?.({ preventScroll: true });if (range) input.setSelectionRange(...range);
      }
    }
  }
  async function open(requestedTarget = '', seededDescription) {
    if (opening || capturing || !popup.hidden) return;
    if (busy && draft) { show();return; }
    const epoch = ++openEpoch;
    opening = true;previousFocus = doc.activeElement;
    previousRange = typeof previousFocus?.selectionStart === 'number' ? [previousFocus.selectionStart, previousFocus.selectionEnd, previousFocus.selectionDirection] : null;
    try {
      const auth = ready() && identity() || 'offline-public';
      let current = {};
      if (ready() && identity()) { try { current = await context(); } catch { /* Public reports remain usable when the hub fails. */ } }
      if (epoch !== openEpoch) return;
      scope = win.location.origin + ':' + auth;
      draft = await timed(store.read(scope), 5000, 'Could not restore the draft. Try Improvements again.');
      if (epoch !== openEpoch) return;
      if (draft?.accepted) { await select(draft.thread);await store.write(scope, null);draft = null; }
      const { projects: choices, accountBinding: sourceAccountBinding, ...viewContext } = current;
      projects = choices || [];const target = backplaneProject(projects);
      const preference = await timed(store.read(scope + ':destination'), 5000, 'Could not read the destination preference.');
      if (epoch !== openEpoch) return;
      const legacy = !!draft;
      draft ||= { text: '', image: null, project: target?.id || '', context: viewContext, key: '' };
      draft.target = reportTarget(draft.target || (!legacy ? requestedTarget || preference?.target || '' : ''), legacy);
      if (seededDescription !== undefined && !legacy) draft.text = seededDescription;
      version = await timed(Promise.resolve(appVersion()), 5000, 'Version check timed out.').catch(() => 'unknown');
      if (epoch !== openEpoch) return;
      collect.input.checked = diagnostics.enabled?.() !== false;
      hubCollect.input.disabled = !ready() || !diagnostics.controlHub;
      hubCollect.input.checked = false;
      if (diagnostics.hubStatus && ready()) void diagnostics.hubStatus().then(s => { hubCollect.input.checked = s?.enabled === true;collectionNote.textContent = 'Local metadata only. Hub storage: ' + (Number.isSafeInteger(s?.stored_bytes) ? s.stored_bytes : '?') + ' / ' + (Number.isSafeInteger(s?.cap_bytes) ? s.cap_bytes : '?') + ' bytes; retention ' + ((s?.retention_seconds || 604800) / 86400) + ' days. No conversation text. Sharing requires selection and review.'; }).catch(() => {});
      invalidateIssue(false);
      // Upgrade an unsent draft from the old origin-project behavior. A request
      // that has already created a thread keeps its stable identity for retry.
      if (!draft.key) {
        draft.project = target?.id || (draft.context?.implementationProject?.id === draft.project && projects.some(p => p.id === draft.project) ? draft.project : '');
        if (!draft.context?.sourceThread) draft.context = viewContext;
        draft.context.implementationProject = target || projects.find(p => p.id === draft.project) || null;
      }
      const workspaces = projects.filter(p => p.id !== 'scratch' && p.root);
      draft.writerProject ||= workspaces.find(p => p.id === draft.context?.sourceProject?.id)?.id || (workspaces.length === 1 ? workspaces[0].id : '');
      writerProject.replaceChildren(el('option', 'Choose a project for the report agent', { value: '' }));
      for (const p of workspaces) writerProject.append(el('option', p.title, { value: p.id }));
      writerProject.value = draft.writerProject;
      input.value = draft.text;details.value = draft.details || '';evidence.value = draft.evidence || '';evidenceOk.input.checked = false;diagPreview.hidden = true;project.replaceChildren(el('option', 'Choose the Backplane source project', { value: '' }));
      for (const p of projects) project.append(el('option', p.title, { value: p.id }));
      if (draft.project && ![...project.options].some(o => o.value === draft.project)) project.append(el('option', 'Saved project', { value: draft.project }));
      project.value = draft.project;
      projectLabel.hidden = project.hidden = !!target && draft.project === target.id;
      await save(draft);if (epoch !== openEpoch) return;
      announce(draft.message ? 'This request is ready to retry. Send uses the same conversation.' : draft.target === 'internal' ? 'Send starts a new Backplane thread.' : 'Review the issue before opening GitHub.');
      if (isPublic() && draft.issueBody) { issuePreview = Object.freeze(previewBody(draft.issueBody));review.value = draft.issueBody;review.hidden = issueActions.hidden = false; }
      if (draft.issueOpened) announce('This draft was already opened on GitHub. Check GitHub before explicitly allowing another copy.');
      if (seededDescription !== undefined && legacy && seededDescription !== draft.text) announce('Your existing draft is kept. Copy the native description and paste it when ready.');
      if (draft.image || draft.recording || isPublic()) show();else await recapture(true);
      if (canRecord && draft.recording) await showRecordingPreview();
    } catch (error) { if (epoch !== openEpoch) return;announce(error.message);popup.hidden = false;popup.setAttribute('aria-modal', 'true');overlayChanged();place();close.focus(); }
    finally { if (epoch === openEpoch) { opening = false;controls(); } }
  }
  function toggle() { if (recordingActive) { recording?.stop();return; }if (opening || capturing || !popup.hidden) hide();else void open(); }
  input.addEventListener('input', () => { if (!draft || draft.message) return;draft.text = input.value;forgetReport();invalidateIssue();controls();void save(draft).catch(e => announce(e.message)); });
  project.addEventListener('change', () => { if (!draft || draft.key) return;draft.project = project.value;draft.context.implementationProject = projects.find(p => p.id === project.value) || null;void save(draft).catch(e => announce(e.message));controls(); });
  close.addEventListener('click', hide);again.addEventListener('click', () => { void recapture(); });choose.addEventListener('click', () => file.click());
  file.addEventListener('change', async () => {
    const image = file.files?.[0];file.value = '';if (!image || busy || draft?.message) return;
    if (!image.size || !/^image\/(png|jpeg|webp)$/.test(image.type)) { announce('Choose a PNG, JPEG or WebP image.');return; }
    draft.image = image;draft.path = '';forgetReport();invalidateIssue();showPreview();
    try { await save(draft);announce('Image refreshed.'); } catch (e) { announce(e.message); }
  });
  target.addEventListener('change', async () => {
    if (!draft || draft.key || busy) return;
    draft.target = reportTarget(target.value, false);draft.includeRecording = false;forgetReport();invalidateIssue();controls();showPreview();
    try { await store.write(scope + ':destination', { target: draft.target });await save(draft); }
    catch (e) { announce(e.message); }
  });
  writerProject.addEventListener('change', () => {
    if (!draft || busy) return;draft.writerProject = writerProject.value;forgetReport();invalidateIssue();controls();void save(draft).catch(e => announce(e.message));
  });
  for (const [node, field] of [[screenshot.input, 'includeScreenshot'], [logs.input, 'includeDiagnostics'], [extra.input, 'includeDetails']]) node.addEventListener('change', () => {
    if (!draft || busy) return;draft[field] = node.checked;forgetReport();invalidateIssue();diagPreview.hidden = true;controls();showPreview();void save(draft).catch(e => announce(e.message));
  });
  details.addEventListener('input', () => { if (!draft || busy) return;draft.details = details.value;forgetReport();invalidateIssue();void save(draft).catch(e => announce(e.message)); });
  collect.input.addEventListener('change', async () => {
    try { await diagnostics.setEnabled?.(collect.input.checked);announce(collect.input.checked ? 'Local collection enabled.' : 'Local collection disabled. Clear local diagnostics to remove earlier records.'); }
    catch (e) { announce(e.message);collect.input.checked = diagnostics.enabled?.() !== false; }
  });
  hubCollect.input.addEventListener('change', async () => {
    try { await diagnostics.controlHub(hubCollect.input.checked ? 'on' : 'off');announce('Hub diagnostics preference saved.'); }
    catch (e) { announce(e.message); }
  });
  clearLogs.addEventListener('click', async () => {
    try { if (canRecord || draft?.recording) await recordingStore.clear();recordingFile = null;if (draft) { delete draft.recording;draft.includeRecording = false; }await showRecordingPreview();await diagnostics.clear?.();if (ready() && diagnostics.controlHub) await diagnostics.controlHub('clear');if (draft) draft.includeDiagnostics = false;forgetReport();invalidateIssue();if (draft) await save(draft);diagPreview.hidden = true;controls();announce('Local diagnostics cleared.'); }
    catch (e) { announce(e.message); }
  });
  async function prepareIssue() {
    if (!reportBody) { announce('This page needs an update to prepare issues. Your draft is kept.');return; }
    if (!hasEvidence()) { announce('Add a thorough behavior description or select an evidence image or reviewed recording before reviewing.');evidence.focus();return; }
    busy = true;controls();
    try {
      let diag = '';
      if (draft.includeDiagnostics) {
        const value = await diagnostics.snapshot?.();
        if (!value) throw Error('Diagnostics are unavailable. Uncheck diagnostics to continue.');
        diag = diagnosticReport(value);diagPreview.textContent = diag;diagPreview.hidden = false;
      }
      const body = draft.report || reportBody(draft.text + '\n\nEvidence\n' + (draft.evidence || (draft.includeRecording ? 'Selected recording, attach manually on GitHub.' : 'Selected image, attach manually on GitHub.')), osFamily(win.navigator), version, draft.includeDetails ? draft.details || '' : '', diag);
      if (typeof body !== 'string' || !body.trim()) throw Error('Could not prepare the exact issue text. Your draft is kept.');
      if (draft.issueBody !== body) delete draft.issueOpened;
      draft.issueBody = body;
      issuePreview = Object.freeze(previewBody(body));
      await save(draft);
      review.value = body;review.hidden = issueActions.hidden = false;
      announce(issuePreview.url ? 'Review the exact public text below. Open GitHub draft sends this text to GitHub; sign in with your account and press Submit there.' : 'This report is too long for a GitHub link. Copy or save the complete text, then paste it into GitHub. Nothing is truncated.');
      place();
    } catch (e) { announce(e.message); }
    finally { busy = false;controls(); }
  }
  function forgetReport() {
    // Preserve earlier authored/generated reports locally across input changes.
    // They are not reused after a changed sharing choice.
    if (draft?.report) { draft.previousReport = draft.report;delete draft.report; }
    if (draft?.writer) { draft.previousWriter = draft.writer;delete draft.writer; }
    evidenceOk.input.checked = false;
  }
  evidence.addEventListener('input', () => { if (!draft || busy) return;draft.evidence = evidence.value;forgetReport();invalidateIssue();controls();void save(draft).catch(e => announce(e.message)); });
  evidenceOk.input.addEventListener('change', controls);
  review.addEventListener('input', () => {
    if (!draft || busy) return;draft.report = review.value;draft.issueBody = review.value;delete draft.issueOpened;evidenceOk.input.checked = false;
    issuePreview = Object.freeze(previewBody(review.value));controls();void save(draft).catch(e => announce(e.message));
  });
  useTemplate.addEventListener('click', async () => {
    if (!draft || busy) return;
    forgetReport();draft.report = reportTemplate();draft.issueBody = draft.report;issuePreview = null;
    review.value = draft.report;review.hidden = false;issueActions.hidden = true;await save(draft);announce('Fill the unknown fields in this editable template, then press Review issue.');place();
  });
  const reportWriter = new ReportWriter({ rpc, save, identity: () => ready() && identity() ? win.location.origin + ':' + identity() : '', expectedScope: () => scope, upload: async (thread, key, image) => uploadImage(thread, key, image) });
  writer.addEventListener('click', async () => {
    if (busy || !draft || !isPublic()) return;
    if (!ready() || !identity()) { announce('Reconnect to the hub before starting the report agent. You can fill the report template offline.');return; }
    if (win.location.origin + ':' + identity() !== scope) { announce('The hub identity changed. Copy your description, close and reopen Improvements before starting an agent. Your local draft is kept.');return; }
    if (!draft.text?.trim()) { announce('Describe the issue before starting the report agent.');input.focus();return; }
    if (!hasEvidence()) { announce('Add a thorough behavior description or select an evidence image or reviewed recording first.');evidence.focus();return; }
    if (!draft.writer && !draft.writerProject) { announce('Choose a project for the report agent, or fill the template locally.');writerProject.focus();return; }
    if (!reportPrompt) { announce('Update this page before using the report agent.');return; }
    busy = true;controls();
    try {
      const diag = draft.includeDiagnostics ? diagnosticReport(await diagnostics.snapshot()) : '';
      const evidenceText = draft.evidence || (draft.includeRecording ? 'The user selected the attached screen recording. Watch it only as evidence.' : 'The user selected an attached evidence image. Examine it only as evidence.');
      const prompt = reportPrompt(draft.text, osFamily(win.navigator), version, evidenceText + (draft.includeDetails ? '\nUser-elected details\n' + (draft.details || '') : ''), diag);
      if (await reportWriter.start(draft, prompt, { image: draft.includeScreenshot ? draft.image : null, video: await selectedRecording() })) announce('Report agent started in the selected project. Load the report when it finishes, or open the thread to answer its questions.');
    } catch (e) { announce(e.message); }
    finally { busy = false;controls(); }
  });
  loadReport.addEventListener('click', async () => {
    if (busy || !draft) return;busy = true;controls();
    try {
      const body = await reportWriter.load(draft);
      if (body) { draft.report = body;await save(draft);review.value = body;review.hidden = false;issueActions.hidden = true;issuePreview = null;evidenceOk.input.checked = false;announce('Agent report loaded. Edit it, remove private information, then press Review issue.');place(); }
    } catch (e) { announce(e.message); }
    finally { busy = false;controls(); }
  });
  openWriter.addEventListener('click', async () => { if (draft?.writer?.thread) { await select(draft.writer.thread);hide(); } });
  retryIssue.addEventListener('click', async () => {
    if (busy || !draft?.issueOpened || !win.confirm('Check GitHub for an existing issue first. Allow another copy of this draft?')) return;
    busy = true;controls();
    try { delete draft.issueOpened;await save(draft);announce('Review the report and press Open GitHub draft when ready.'); }
    catch (e) { draft.issueOpened = true;announce(e.message); }
    finally { busy = false;controls(); }
  });
  function hasEvidence() { return !!draft?.evidence?.trim() || !!(draft?.includeScreenshot && draft.image) || !!(canRecord && draft?.includeRecording && draft.recording && draft.recordingReviewed); }
  async function selectedRecording() {
    if (!draft?.includeRecording) return null;
    if (!canRecord || !draft.recording || !draft.recordingReviewed) throw Error('Play the recording preview and select it before attaching.');
    const selectedScope = scope, owner = draft, id = draft.recording.id;
    const video = await recordingStore.get(draft.recording);
    if (scope !== selectedScope || draft !== owner || owner.recording?.id !== id || !ready() || win.location.origin + ':' + identity() !== selectedScope) throw Error('The hub identity or recording changed. Reopen Improvements.');
    return video;
  }
  async function showRecordingPreview() {
    const owner = draft, ref = draft?.recording, selectedScope = scope;
    if (recordingUrl) URL.revokeObjectURL(recordingUrl);recordingUrl = null;recordingFile = null;previewRecordingId = null;videoPreview.removeAttribute('src');
    if (!ref || !canRecord) { controls();return; }
    try {
      const file = await recordingStore.get(ref);
      if (owner !== draft || selectedScope !== scope || draft.recording?.id !== ref.id) return;
      recordingFile = file;previewRecordingId = ref.id;recordingUrl = URL.createObjectURL(file);videoPreview.src = recordingUrl;
    } catch { if (owner === draft) { draft.includeRecording = false;announce('The recording expired or could not be read. Record again.'); } }
    controls();place();
  }
  recordButton.addEventListener('click', async () => {
    if (!draft || busy || draft.key || recordingPicking || recordingActive) return;
    const owner = draft, selectedScope = scope, stamp = ++operation;
    recordingAbort = new AbortController();recordingPicking = true;controls();
    try {
      const session = await record({ signal: recordingAbort.signal, onTime: (ms, bytes) => {
        recordingStatus.textContent = '● Recording screen · ' + Math.floor(ms / 1000) + '/120 s · ' + (bytes / 1048576).toFixed(1) + '/9 MiB';
      } });
      if (operation !== stamp || owner !== draft || selectedScope !== scope) { session.cancel();await session.done.catch(() => {});return; }
      recording = session;recordingPicking = false;recordingActive = true;stopRecording.disabled = false;
      popup.hidden = true;popup.removeAttribute('aria-modal');recordingBar.hidden = false;overlayChanged();
      const result = await session.done;recordingActive = false;recordingBar.hidden = true;
      if (operation !== stamp || owner !== draft || selectedScope !== scope || !ready() || win.location.origin + ':' + identity() !== scope) return;
      const ref = await recordingStore.put(result.file);
      if (operation !== stamp || owner !== draft || selectedScope !== scope || win.location.origin + ':' + identity() !== scope) { await recordingStore.remove(ref);return; }
      const previous = owner.recording;owner.recording = { ...ref, durationMs: result.durationMs };owner.includeRecording = false;owner.recordingReviewed = false;
      forgetReport();invalidateIssue();await save(owner);if (previous) await recordingStore.remove(previous);
      await showRecordingPreview();show();announce('Recording saved locally. Play the preview, then select it to attach. Clips expire after 24 hours.');
    } catch (e) { if (stamp === operation) { show();announce(e.message || 'Recording was not completed. Your draft is kept.'); } }
    finally { if (stamp === operation) { recording = null;recordingPicking = recordingActive = false;recordingBar.hidden = true;controls(); } }
  });
  stopRecording.addEventListener('click', () => { stopRecording.disabled = true;recordingStatus.textContent = 'Finishing recording…';recording?.stop(); });
  cancelRecording.addEventListener('click', () => { recordingAbort?.abort();recording?.cancel(); });
  videoPreview.addEventListener('play', () => { if (!draft?.recording || !recordingFile || previewRecordingId !== draft.recording.id) return;draft.recordingReviewed = true;controls();void save(draft).catch(e => announce(e.message)); });
  attachVideo.input.addEventListener('change', () => { if (!draft || busy || draft.key || !draft.recordingReviewed) return;draft.includeRecording = attachVideo.input.checked;forgetReport();invalidateIssue();controls();void save(draft).catch(e => announce(e.message)); });
  discardVideo.addEventListener('click', async () => { if (!draft?.recording || busy || draft.key) return;try { await recordingStore.remove(draft.recording);delete draft.recording;draft.includeRecording = false;draft.recordingReviewed = false;forgetReport();invalidateIssue();await save(draft);await showRecordingPreview();announce('Recording removed from local storage.'); } catch(e) { announce(e.message); } });
  saveVideo.addEventListener('click', async () => { try { const video = await selectedRecording();if (video) downloadBlob(video, 'backplane-recording.webm'); } catch(e) { announce(e.message); } });
  win.addEventListener('pagehide', () => { recordingAbort?.abort();recording?.cancel();if (recordingUrl) URL.revokeObjectURL(recordingUrl); });
  const handoff = new IssueHandoff({ save, open: openIssue });
  confirm.addEventListener('click', async () => {
    if (busy || !issuePreview || draft?.issueOpened) return;
    if (!evidenceOk.input.checked || !hasEvidence()) { announce('Confirm that the report has a thorough description or attachable evidence image.');return; }
    busy = true;controls();
    try { if (draft.includeRecording) await selectedRecording();if (await handoff.send(draft, issuePreview)) announce('GitHub draft opened. Sign in with your GitHub account and press Submit there. Backplane has not submitted an issue.'); }
    catch (e) { announce(e.message); }
    finally { busy = false;controls(); }
  });
  copy.addEventListener('click', async () => { try { await win.navigator.clipboard.writeText(issuePreview.body);announce('Exact issue text copied.'); } catch { review.focus();review.select();announce('Select and copy the issue text below.'); } });
  function downloadBlob(blob, name) {
    const u = URL.createObjectURL(blob), link = el('a', '', { href: u, download: name });link.click();setTimeout(() => URL.revokeObjectURL(u), 1000);
  }
  download.addEventListener('click', () => { if (issuePreview) downloadBlob(new Blob([issuePreview.body], { type: 'text/plain;charset=utf-8' }), 'backplane-issue.txt'); });
  saveImage.addEventListener('click', () => { if (draft?.includeScreenshot && draft.image) downloadBlob(draft.image, 'backplane-screenshot.' + ({ 'image/jpeg': 'jpg', 'image/webp': 'webp' }[draft.image.type] || 'png')); });
  const uploadImage = async (thread, key, image) => {
    if (!ready() || identity() !== scope.slice(win.location.origin.length + 1)) throw Error('Reconnect to the same hub before sending.');
    const query = new URLSearchParams({ thread, key, name: image.type === 'video/webm' ? 'backplane-recording.webm' : 'backplane.' + ({ 'image/jpeg': 'jpg', 'image/webp': 'webp' }[image.type] || 'png') });
    const response = await fetch('/attach?' + query, { method: 'POST', body: image, credentials: 'same-origin', signal: AbortSignal.timeout(60000) });
    const result = await response.json();
    if (!response.ok || result.thread !== thread || typeof result.attached !== 'string') throw Error(result.text || 'The image did not arrive. Retry Send.');
    return result.attached;
  };
  const flow = new ImprovementsFlow({ rpc, save, select, status: announce, upload: uploadImage, identity: () => ready() && identity() ? win.location.origin + ':' + identity() : '', expectedScope: () => scope });
  async function submit() {
    if (busy || capturing || recordingPicking || recordingActive) return;
    if (!draft) { announce('Reconnect, close Improvements, then open it again to restore your draft.');return; }
    if (!draft.text?.trim()) { announce('Describe the change to Backplane before sending.');input.focus();return; }
    if (isPublic()) { await prepareIssue();return; }
    if (!draft.image && !(draft.includeRecording && draft.recording)) { announce('Choose an image or preview and select a recording, then retry Send. Your text is kept.');input.focus();return; }
    if (!draft.project) { announce('Choose the Backplane source project, then retry Send.');project.focus();return; }
    const target = backplaneProject(projects);
    if (target && draft.project !== target.id) { announce('This saved request belongs to another project. Open its thread to finish it before starting a Backplane improvement.');return; }
    busy = true;controls();announce('Sending…');
    try {
      if (!ready() || win.location.origin + ':' + identity() !== scope) throw Error('Reconnect to the same hub, then retry Send.');
      if (draft.accepted) await select(draft.thread);
      else if (!await flow.send(draft, { video: await selectedRecording() })) return;
      announce('Request sent. Opening the new Backplane thread.');
      await store.write(scope, null);if (draft.recording) await recordingStore.remove(draft.recording).catch(() => {});recordingFile = null;hide();draft = null;
    } catch (error) { announce(error.message || 'Could not send. Your draft and image are kept. Retry Send.'); }
    finally { busy = false;controls(); }
  }
  send.addEventListener('click', () => { void submit(); });
  let drag;
  header.addEventListener('pointerdown', e => {
    if (e.target.closest('button') || viewport().width <= 600 || e.button !== 0) return;
    const r = popup.getBoundingClientRect();drag = { x: e.clientX - r.left, y: e.clientY - r.top };header.setPointerCapture(e.pointerId);e.preventDefault();header.focus();
  });
  header.addEventListener('pointermove', e => { if (drag) { pos = { x: e.clientX - drag.x, y: e.clientY - drag.y };place(); } });
  for (const name of ['pointerup', 'pointercancel', 'lostpointercapture']) header.addEventListener(name, () => { drag = null; });
  function moveKey(e) {
    if (e.target !== header || e.isComposing || e.ctrlKey || e.altKey || e.metaKey || !['ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].includes(e.key) || viewport().width <= 600) return;
    e.preventDefault();e.stopImmediatePropagation();const r = popup.getBoundingClientRect(), step = e.shiftKey ? 40 : 10;
    pos = { x: r.left + (e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0), y: r.top + (e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0) };place();
  }
  win.addEventListener('keydown', e => {
    if (e.isComposing || e.keyCode === 229 || e.defaultPrevented) return;
    if (improvementsShortcut(e, win.navigator.platform) && (opening || capturing || !popup.hidden || !doc.querySelector('[aria-modal="true"],.lightbox,.draw-back,.tour'))) {
      e.preventDefault();e.stopImmediatePropagation();toggle();return;
    }
    if (capturing && e.key === 'Escape' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();e.stopImmediatePropagation();operation++;captureController?.abort();capturing=false;
      popup.hidden=false;popup.setAttribute('aria-modal','true');overlayChanged();showPreview();softFocus(input);
      announce('Capture canceled. Your draft and previous image are kept.');return;
    }
    if (!popup.hidden) {
      moveKey(e);
      if (e.key === 'Enter' && e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey && !e.repeat) { e.preventDefault();e.stopImmediatePropagation();void submit();return; }
      if (e.key === 'Escape' && !e.ctrlKey && !e.metaKey && !e.altKey) { e.preventDefault();e.stopImmediatePropagation();hide();return; }
      if (e.key === 'Tab') {
        const stops = [...popup.querySelectorAll('textarea,select,button,input[type="checkbox"],[tabindex="0"]')].filter(n => !n.disabled && !n.hidden && n.getClientRects().length), at = stops.indexOf(doc.activeElement);
        if (at < 0 || !e.shiftKey && at === stops.length - 1 || e.shiftKey && at === 0) { e.preventDefault();(e.shiftKey ? stops.at(-1) : stops[0]).focus(); }
      }
      // All other keys belong to this popup's native controls, including IME.
      e.stopImmediatePropagation();return;
    }
  }, true);
  doc.addEventListener('click', e => { if (e.target.closest?.('[data-improvements]')) { e.preventDefault();void open(); } }, true);
  const decorate = () => { for (const button of doc.querySelectorAll('[data-improvements]')) { button.title = 'Improvements · ' + chord;button.setAttribute('aria-keyshortcuts', /mac/i.test(win.navigator.platform) ? 'Control+Meta+I' : 'Control+Alt+I'); } };
  new win.MutationObserver(decorate).observe(doc.getElementById('root') || doc.body, { childList: true, subtree: true });decorate();
  for (const target of [win, win.visualViewport].filter(Boolean)) target.addEventListener('resize', place);
  win.visualViewport?.addEventListener('scroll', place);
  win.backplaneDesktop?.onImprovements?.(() => { if (opening || capturing || !popup.hidden || !doc.querySelector('[aria-modal="true"],.lightbox,.draw-back,.tour')) toggle(); });
  return { open, close: hide, toggle };
}
