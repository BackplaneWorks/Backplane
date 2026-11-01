// Capture only this application. Desktop uses the trusted app window's
// capturePage; web accepts only a tab proving this page's capture handle.
export async function captureApp({ bridge = globalThis.backplaneDesktop, media = navigator.mediaDevices, doc = document, win = window, signal } = {}) {
  if (bridge?.captureApp) {
    const png = await bridge.captureApp();
    if (typeof png !== 'string' || !png.startsWith('data:image/png;base64,')) throw Error('The app could not capture this window. Capture again or choose an image.');
    // Local bytes avoid a data: fetch, which the desktop's connect-src blocks.
    const binary = atob(png.slice('data:image/png;base64,'.length));
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return new File([bytes], 'backplane.png', { type: 'image/png' });
  }
  if (!media?.getDisplayMedia || !media.setCaptureHandleConfig) throw Error('Capture is unavailable here. Choose a screenshot from this device.');
  const handle = 'backplane-improvements-' + crypto.randomUUID();
  media.setCaptureHandleConfig({ handle, exposeOrigin: false, permittedOrigins: [win.location.origin] });
  let stream;
  try {
    stream = await media.getDisplayMedia({ video: { displaySurface: 'browser' }, audio: false,
      preferCurrentTab: true, selfBrowserSurface: 'include', monitorTypeSurfaces: 'exclude', surfaceSwitching: 'exclude' });
    if (signal?.aborted) throw Error('Capture canceled.');
    signal?.addEventListener('abort', () => stream.getTracks().forEach(t => t.stop()), { once: true });
    const track = stream.getVideoTracks()[0];
    if (track?.getSettings().displaySurface !== 'browser' || track.getCaptureHandle?.()?.handle !== handle) throw Error('Choose this Backplane tab to capture the app.');
    const video = doc.createElement('video');video.muted = true;video.srcObject = stream;
    await video.play();
    const width = video.videoWidth, height = video.videoHeight;
    if (!width || !height || width * height > 24000000) throw Error('This capture is too large. Choose a smaller image.');
    const canvas = doc.createElement('canvas');canvas.width = width;canvas.height = height;
    canvas.getContext('2d').drawImage(video, 0, 0);
    const blob = await new Promise(resolve => canvas.toBlob(resolve, 'image/png'));
    if (!blob) throw Error('The app could not capture this tab. Capture again.');
    return new File([blob], 'backplane.png', { type: 'image/png' });
  } finally { stream?.getTracks().forEach(t => t.stop()); }
}
