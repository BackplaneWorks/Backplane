// A presentation cue only. Eligibility and all actions stay in Bend.
// Remember offers for this loaded page so a tour or DOM patch cannot replay it.
export function starAttention() {
  const seen = new Set();
  return root => {
    const strip = root.querySelector('.star-nudge[data-star-offer]');
    const token = strip?.getAttribute('data-star-offer');
    if (!token || seen.has(token)) return;
    seen.add(token);
    if (seen.size > 64) seen.delete(seen.values().next().value);
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    strip.querySelector('.star-nudge-icon')?.animate([
      {transform: 'scale(1)'}, {transform: 'scale(1.16)', offset: 0.45}, {transform: 'scale(1)'}
    ], {duration: 900, iterations: 1, easing: 'ease-in-out'});
  };
}
