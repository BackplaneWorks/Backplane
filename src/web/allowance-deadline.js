// Presentation retirement only. The server owns percentages and deadlines;
// this timer cannot fetch, refresh, refill or change account/settings state.
export function allowanceDeadline({clock = () => Date.now() / 1000, schedule = setTimeout, cancel = clearTimeout} = {}) {
  let timer = null, current = null, key = '';
  return (root, ready = true) => {
    const chip = root.querySelector('#allowance-chip');
    const details = root.querySelector('#allowance-details');
    if (timer !== null) { cancel(timer); timer = null; }
    if (!chip) { current = null; key = ''; return; }
    const next = chip.getAttribute('data-as-of') + ':' + chip.getAttribute('data-valid-until');
    const retire = () => {
      chip.setAttribute('data-retired', 'true');
      // Historical rows may retain a last report; the panel's status says so.
      details?.setAttribute('data-retired', 'true');
    };
    const at = Number(chip.getAttribute('data-as-of'));
    const until = Number(chip.getAttribute('data-valid-until'));
    const valid = Number.isSafeInteger(at) && Number.isSafeInteger(until) && at > 0 && until > at;
    if (!ready) { retire(); current = chip; key = next; return; }
    if (current === chip && key === next && chip.getAttribute('data-retired') === 'true') { retire(); return; }
    current = chip; key = next;
    chip.removeAttribute('data-retired');
    details?.removeAttribute('data-retired');
    if (!valid) return; // The server already rendered unknown/stale.
    // A client clock behind the server can never extend the reported lifetime.
    const remaining = Math.min(until - clock(), until - at);
    if (remaining <= 0) retire();
    else timer = schedule(retire, Math.min(remaining * 1000, 2147483647));
  };
}
