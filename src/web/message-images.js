// Capture phase sees image events even though load/error do not bubble.
export function installMessageImages(doc = document) {
  const update = event => {
    const img = event.target;
    if (!img?.matches?.('img[data-message-image]')) return;
    const failed = event.type === 'error' || !img.naturalWidth;
    img.parentElement.classList.toggle('image-unavailable', failed);
    if (failed) img.parentElement.removeAttribute('data-lightbox');
  };
  doc.addEventListener('error', update, true);
  doc.addEventListener('load', update, true);
  return () => { doc.removeEventListener('error', update, true);doc.removeEventListener('load', update, true); };
}
