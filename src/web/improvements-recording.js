// User-initiated capture only. Never ask for audio or retain source labels.
export const RECORDING = Object.freeze({ maxMs: 120000, maxBytes: 9 * 1024 * 1024, diskBytes: 32 * 1024 * 1024, ttlMs: 86400000, keep: 2 });
export function recordingSupported(win = window) {
  return !/Android|iPhone|iPad|iPod/i.test(win.navigator.userAgent || '') && !(win.navigator.platform === 'MacIntel' && win.navigator.maxTouchPoints > 1)
    && !!win.navigator.mediaDevices?.getDisplayMedia && !!win.MediaRecorder;
}
export async function startScreenRecording({ media = navigator.mediaDevices, Recorder = MediaRecorder, signal, onTime = () => {},
  now = () => performance.now(), timers = globalThis, limits = RECORDING } = {}) {
  const mimeType = ['video/webm;codecs=vp9', 'video/webm;codecs=vp8', 'video/webm'].find(m => Recorder.isTypeSupported(m));
  if (!mimeType) throw Error('WebM recording is unavailable in this browser.');
  let stream, recorder, clock, deadline, settle, fail, stopped = false, aborted = false, began = 0, bytes = 0, chunks = [];
  const done = new Promise((resolve, reject) => { settle = resolve;fail = reject; });
  // A picker may resolve after cancellation. Stop every late track as well.
  const tracksOff = () => stream?.getTracks().forEach(t => t.stop());
  const clean = () => { timers.clearInterval(clock);timers.clearTimeout(deadline);signal?.removeEventListener('abort', cancel);tracksOff(); };
  function stop() { if (stopped) return;stopped = true;if (recorder?.state !== 'inactive') recorder?.stop();tracksOff(); }
  function cancel() { aborted = true;stop(); }
  signal?.addEventListener('abort', cancel, { once: true });
  try {
    // This invocation stays in the click's activation, before any await.
    stream = await media.getDisplayMedia({ video: { width: { ideal: 1280 }, height: { ideal: 720 }, frameRate: { ideal: 15, max: 30 } }, audio: false,
      surfaceSwitching: 'exclude', selfBrowserSurface: 'include' });
    if (signal?.aborted || aborted) throw Error('Recording canceled.');
    if (!stream.getVideoTracks().length) throw Error('No screen video was selected.');
    recorder = new Recorder(stream, { mimeType, videoBitsPerSecond: 512000 });began = now();
    recorder.ondataavailable = ({ data }) => {
      if (!data?.size || aborted) return;
      if (bytes + data.size > limits.maxBytes) { aborted = true;stop();return; }
      chunks.push(data);bytes += data.size;
      onTime(Math.max(0, now() - began), bytes);
      if (bytes >= limits.maxBytes - Math.min(131072, limits.maxBytes / 8) || now() - began >= limits.maxMs) stop();
    };
    recorder.onerror = () => { aborted = true;stop();clean();chunks = [];fail(Error('Recording failed. Your report text is kept.')); };
    recorder.onstop = () => {
      clean();
      const elapsed = now() - began;
      if (aborted || !bytes || elapsed > limits.maxMs + 2000) { chunks = [];fail(Error(aborted ? 'Recording canceled or exceeded the size limit. Record a shorter clip.' : 'No bounded recording was produced. Try a shorter clip.'));return; }
      const file = new File(chunks, 'backplane-recording.webm', { type: 'video/webm' });chunks = [];
      settle({ file, durationMs: Math.max(0, elapsed) });
    };
    for (const track of stream.getVideoTracks()) track.addEventListener('ended', stop, { once: true });
    recorder.start(500);onTime(0, 0);
    clock = timers.setInterval(() => { onTime(Math.max(0, now() - began), bytes);if (now() - began >= limits.maxMs) stop(); }, 250);
    deadline = timers.setTimeout(stop, limits.maxMs);
    return { stop, cancel, done };
  } catch (e) { clean();throw e; }
}
