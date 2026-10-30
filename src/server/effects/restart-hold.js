let bp_restart_held = '';
function restarthold_offer(payload) {
  if (typeof payload !== 'string' || Buffer.byteLength(payload, 'utf8') > 65536) return false;
  let request;
  try { request = JSON.parse(payload); } catch { return false; }
  if (!request || typeof request.path !== 'string' || !request.path || request.path.includes('\0') ||
      !Array.isArray(request.args) || request.args.length > 128 ||
      request.args.some(arg => typeof arg !== 'string' || arg.includes('\0'))) return false;
  if (bp_restart_held && bp_restart_held !== payload) return false;
  bp_restart_held = payload;
  return true;
}
function restarthold_take() {
  const payload = bp_restart_held;
  bp_restart_held = '';
  return payload;
}
io_eff(CID(RestartHold.offer), restarthold_offer);
io_eff(CID(RestartHold.take), restarthold_take);
