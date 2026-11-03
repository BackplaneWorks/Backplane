function messageImage(img, failed) {
  if (!img?.matches?.('img[data-message-image]')) return;
  img.parentElement.classList.toggle('image-unavailable', failed);
  if (failed) img.parentElement.removeAttribute('data-lightbox');
  else img.parentElement.setAttribute('data-lightbox', img.getAttribute('src'));
}

// A DOM patch can restore the hub's original attributes without another load
// event. Reapply the known result alongside attachment/shot preview state.
export function messageImages(root) {
  for (const img of root.querySelectorAll('img[data-message-image]'))
    if (img.complete) messageImage(img, !img.naturalWidth);
}

// Capture phase sees image events even though load/error do not bubble.
export function installMessageImages(doc = document) {
  const update = event => messageImage(event.target, event.type === 'error' || !event.target.naturalWidth);
  doc.addEventListener('error', update, true);
  doc.addEventListener('load', update, true);
  return () => { doc.removeEventListener('error', update, true);doc.removeEventListener('load', update, true); };
}
