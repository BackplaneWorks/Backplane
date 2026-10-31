// A visible stream is checkpointed before broadcast. Atomic replacement
// leaves either the previous complete buffer or the new one after an exit.
function checkpoint_file(home, thread) {
  return require('node:path').join(home, 'live-checkpoints', require('node:crypto').createHash('sha256').update(thread).digest('hex') + '.json');
}
function checkpoint_write(home, text) {
  const fs = require('node:fs'), path = require('node:path');
  try {
    const record = JSON.parse(text), thread = record?.buffer?.thread;
    if (typeof thread !== 'string' || !thread || thread.length > 2048) return 'invalid live checkpoint';
    const file = checkpoint_file(home, thread);
    if (!record.buffer.text) { fs.rmSync(file, {force:true});return ''; }
    fs.mkdirSync(path.dirname(file), {recursive:true, mode:0o700});
    const temp = file + '.new';
    fs.writeFileSync(temp, text, {mode:0o600});fs.renameSync(temp, file);
    return '';
  } catch (error) { return String(error.message || error); }
}
function checkpoint_sync(home, text) {
  const fs = require('node:fs'), path = require('node:path');
  try {
    const records = JSON.parse(text), keep = new Set();
    for (const record of records) {
      const error = checkpoint_write(home, JSON.stringify(record));
      if (error) return error;
      if (record.buffer.text) keep.add(path.basename(checkpoint_file(home, record.buffer.thread)));
    }
    const dir = path.join(home, 'live-checkpoints');
    if (fs.existsSync(dir)) for (const file of fs.readdirSync(dir)) {
      if (file.endsWith('.json') && !keep.has(file)) fs.rmSync(path.join(dir, file), {force:true});
    }
    return '';
  } catch (error) { return String(error.message || error); }
}
function checkpoint_read(home) {
  const fs = require('node:fs'), path = require('node:path');
  const dir = path.join(home, 'live-checkpoints'), records = [];
  if (!fs.existsSync(dir)) return '[]';
  for (const file of fs.readdirSync(dir)) {
    if (!/^[A-Za-z0-9_-]+\.json$/.test(file)) continue;
    try {
      const name = path.join(dir, file), stat = fs.lstatSync(name);
      if (!stat.isFile() || stat.size > 2 * 1024 * 1024) continue;
      records.push(JSON.parse(fs.readFileSync(name, 'utf8')));
    } catch {}
  }
  return JSON.stringify(records);
}
io_eff(CID(Checkpoint.write), checkpoint_write);
io_eff(CID(Checkpoint.sync), checkpoint_sync);
io_eff(CID(Checkpoint.read), checkpoint_read);
