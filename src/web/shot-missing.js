// A picture a reply names whose file is gone (moved, renamed, cleaned up,
// a project copied without it) fails to load, and its 4:3 box stood empty:
// a screen of nothing in the middle of the thread. Its link folds to one
// line naming the file instead (style.css .shot.missing). A later frame
// that patches the link's class back is marked again (shotsMissing, after
// each render), as host.js does for attachment previews.
export function shotMissing(img) {
  const shot = img?.closest?.('.msg .shot');
  if (shot) shot.classList.toggle('missing', img.complete && img.naturalWidth === 0 && !!img.getAttribute('src'));
}

export function shotsMissing(root) {
  for (const img of root.querySelectorAll('.msg .shot img')) if (img.complete) shotMissing(img);
}

export function installShotMissing(doc) {
  const on = e => { if (e.target?.matches?.('.msg .shot img')) shotMissing(e.target); };
  doc.addEventListener('error', on, true);
  doc.addEventListener('load', on, true);
}
