// Failure circuits are private to this process, bounded independently of the
// event log. Permanent APNs token/topic failures also follow DeviceDropped.
// No timer, queued payload, state snapshot, token or JWT is retained here.
const bp_push_circuits = new Map();
const bp_push_leases = new Map();
let bp_push_lease = 0;
let bp_push_capacity_reported = false;
const bp_push_slots = 1024;
const bp_push_window = 1800;

function pushgate_claim(url, topic, kind, registration, now) {
  now = Number(now);
  const key = require('node:crypto').createHash('sha256')
    .update(JSON.stringify([url, topic, kind, registration])).digest('hex');
  let slot = bp_push_circuits.get(key);
  if (!slot) {
    // Only inactive, expired circuits may be discarded. Never evict a live
    // cooldown to admit a new key and thereby bypass the retry bound.
    for (const [saved, value] of bp_push_circuits) {
      if (!value.permanent && !value.active && now >= value.touched + bp_push_window && now >= value.until)
        bp_push_circuits.delete(saved);
    }
    if (bp_push_circuits.size >= bp_push_slots) {
      if (bp_push_capacity_reported) return '';
      bp_push_capacity_reported = true;
      return '-capacity';
    }
    bp_push_capacity_reported = false;
    slot = {active: 0, until: 0, touched: now, started: now,
      failures: 0, completed: 0, permanent: false, reported: ''};
    bp_push_circuits.set(key, slot);
  }
  slot.touched = now;
  const hold = slot.permanent ? 'permanent' : now < slot.until ? 'backoff'
    : slot.active >= (kind === 'activity' ? 1 : 4) ? 'busy' : '';
  if (hold) {
    // Activities describe replaceable current state. Ordinary alerts retain
    // their canonical source record; report the delivery hold explicitly once.
    if (kind === 'activity' || slot.reported === hold) return '';
    slot.reported = hold;
    return '-' + hold;
  }
  slot.reported = '';
  slot.active++;
  const id = String(++bp_push_lease);
  bp_push_leases.set(id, {slot, kind, serial: bp_push_lease});
  return id;
}

function pushgate_finish(lease, code, reason, now) {
  const grant = bp_push_leases.get(lease);
  if (!grant) return { $: "Unit" };
  bp_push_leases.delete(lease);
  const {slot, kind, serial} = grant;
  slot.active--;
  now = Number(now);
  slot.touched = now;
  // A late success must not erase a newer failure/cooldown.
  if (serial < slot.completed) return { $: "Unit" };
  slot.completed = serial;
  if (code === '410' || (code === '400' &&
      ['BadDeviceToken', 'Unregistered', 'DeviceTokenNotForTopic', 'TopicDisallowed'].includes(reason))) {
    slot.permanent = true;
    slot.until = now + bp_push_window;
    return { $: "Unit" };
  }
  if (code === '200') {
    slot.failures = 0;
    slot.started = now;
    slot.until = now + (kind === 'activity' ? 1 : 0);
    return { $: "Unit" };
  }
  if (now >= slot.started + bp_push_window) {
    slot.started = now;
    slot.failures = 0;
  }
  slot.failures++;
  // At most six failed delivery jobs per half hour, each with at most two
  // curl retries. A new registration has a new circuit; no retry timer spins.
  slot.until = slot.failures >= 6 ? Math.max(now + 900, slot.started + bp_push_window)
    : now + Math.min(900, 30 * 2 ** (slot.failures - 1));
  return { $: "Unit" };
}

io_eff(CID(PushGate.claim), pushgate_claim);
io_eff(CID(PushGate.finish), pushgate_finish);
