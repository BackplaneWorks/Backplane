// A visible stream is journaled before broadcast, cheaply: a thread's file
// holds one whole record (written atomically: temp file, then rename) and
// then each later delta appended as one JSON string line, so a delta costs
// a write of its own size, never a copy of the reply so far. A line cut by
// an exit is dropped when the file is read again; everything before it is
// whole. The hub writes a whole record again when the stream is new (its
// gen differs from the file's), the file is gone, or the journal has grown
// past CHECKPOINT_JOURNAL_MAX (then it is the buffer, at most Live.cap(), again).
const CHECKPOINT_JOURNAL_MAX = 4 * 1024 * 1024;
// core/live.bend's Live.cap(): what a restored buffer keeps of a longer reply
const CHECKPOINT_LIVE_CAP = 262144;
// file -> {gen, bytes}: what this process knows of each journal it wrote
const checkpoint_journals = new Map();
function checkpoint_dir(home) {
  return require('node:path').join(home, 'live-checkpoints');
}
function checkpoint_name(thread) {
  return require('node:crypto').createHash('sha256').update(thread).digest('hex');
}
function checkpoint_file(home, thread) {
  return require('node:path').join(checkpoint_dir(home), checkpoint_name(thread) + '.jsonl');
}
function checkpoint_thread_ok(thread) {
  return typeof thread === 'string' && thread.length > 0 && thread.length <= 2048;
}
// a whole record: the anchor and the buffer {thread, text, gen, trunc}
function checkpoint_write(home, text) {
  const fs = require('node:fs'), path = require('node:path');
  let file = '';
  try {
    const record = JSON.parse(text), thread = record?.buffer?.thread;
    if (!checkpoint_thread_ok(thread)) return 'invalid live checkpoint';
    file = checkpoint_file(home, thread);
    // (a file the first version of this journal wrote: one record, no lines)
    fs.rmSync(path.join(checkpoint_dir(home), checkpoint_name(thread) + '.json'), {force:true});
    if (!record.buffer.text) { checkpoint_journals.delete(file);fs.rmSync(file, {force:true});return ''; }
    fs.mkdirSync(path.dirname(file), {recursive:true, mode:0o700});
    const line = (text.includes('\n') ? JSON.stringify(record) : text) + '\n', temp = file + '.new';
    fs.writeFileSync(temp, line, {mode:0o600});fs.renameSync(temp, file);
    checkpoint_journals.set(file, {gen: String(record.buffer.gen), bytes: Buffer.byteLength(line)});
    return '';
  } catch (error) { if (file) checkpoint_journals.delete(file);return String(error.message || error); }
}
// what a journal on disk holds (a hub started again goes on with the file
// of a stream it restored); a cut last line is cut off the file
function checkpoint_load(file) {
  const fs = require('node:fs');
  if (!fs.existsSync(file)) return null;
  let body = fs.readFileSync(file, 'utf8');
  const end = body.lastIndexOf('\n') + 1;
  if (end < body.length) { fs.truncateSync(file, Buffer.byteLength(body.slice(0, end)));body = body.slice(0, end); }
  const first = body.slice(0, body.indexOf('\n'));
  const gen = first ? JSON.parse(first)?.buffer?.gen : undefined;
  if (gen === undefined) return null;
  const known = {gen: String(gen), bytes: Buffer.byteLength(body)};
  checkpoint_journals.set(file, known);
  return known;
}
// one delta onto the thread's journal: "" when written, "new" when the hub
// must write the whole record instead (see the top), else the error
function checkpoint_append(home, thread, gen, text) {
  const fs = require('node:fs');
  let file = '';
  try {
    if (!checkpoint_thread_ok(thread)) return 'invalid live checkpoint';
    if (!text) return '';
    file = checkpoint_file(home, thread);
    const known = checkpoint_journals.get(file) || checkpoint_load(file);
    if (!known || known.gen !== gen || known.bytes > CHECKPOINT_JOURNAL_MAX) return 'new';
    const line = JSON.stringify(text) + '\n';
    fs.appendFileSync(file, line);
    known.bytes += Buffer.byteLength(line);
    return '';
  } catch (error) { if (file) checkpoint_journals.delete(file);return String(error.message || error); }
}
// the records of the threads a commit touched: each written whole, or
// removed when its buffer is empty
function checkpoint_sync(home, text) {
  try {
    for (const record of JSON.parse(text)) {
      const error = checkpoint_write(home, JSON.stringify(record));
      if (error) return error;
    }
    return '';
  } catch (error) { return String(error.message || error); }
}
// at start, once the restored streams are known: every other file goes
function checkpoint_keep(home, text) {
  const fs = require('node:fs'), path = require('node:path');
  try {
    const keep = new Set(JSON.parse(text).filter(checkpoint_thread_ok).map(checkpoint_name)), dir = checkpoint_dir(home);
    if (!fs.existsSync(dir)) return '';
    for (const file of fs.readdirSync(dir)) {
      if (keep.has(file.replace(/\.jsonl?$/, ''))) continue;
      checkpoint_journals.delete(path.join(dir, file));
      fs.rmSync(path.join(dir, file), {force:true});
    }
    return '';
  } catch (error) { return String(error.message || error); }
}
// a journal as one record again: its deltas joined onto the record's text,
// the start cut to Live.cap() characters (noted, as the buffer does)
function checkpoint_record(body) {
  const lines = body.split('\n');
  const record = JSON.parse(lines[0]);
  if (!checkpoint_thread_ok(record?.buffer?.thread) || typeof record.buffer.text !== 'string') return null;
  const parts = [record.buffer.text];
  for (const line of lines.slice(1)) {
    if (!line) continue;
    let piece;
    try { piece = JSON.parse(line); } catch { break; }
    if (typeof piece !== 'string') break;
    parts.push(piece);
  }
  let text = parts.join('');
  if (text.length > CHECKPOINT_LIVE_CAP) {
    const chars = Array.from(text);
    if (chars.length > CHECKPOINT_LIVE_CAP) { text = chars.slice(chars.length - CHECKPOINT_LIVE_CAP).join('');record.buffer.trunc = true; }
  }
  record.buffer.text = text;
  return record;
}
function checkpoint_read(home) {
  const fs = require('node:fs'), path = require('node:path');
  const dir = checkpoint_dir(home), records = [];
  if (!fs.existsSync(dir)) return '[]';
  for (const file of fs.readdirSync(dir)) {
    if (!/^[A-Za-z0-9_-]+\.jsonl?$/.test(file)) continue;
    try {
      const name = path.join(dir, file), stat = fs.lstatSync(name);
      if (!stat.isFile() || stat.size > 2 * CHECKPOINT_JOURNAL_MAX) continue;
      const record = checkpoint_record(fs.readFileSync(name, 'utf8'));
      if (record) records.push(record);
    } catch {}
  }
  return JSON.stringify(records);
}
io_eff(CID(Checkpoint.write), checkpoint_write);
io_eff(CID(Checkpoint.append), checkpoint_append);
io_eff(CID(Checkpoint.sync), checkpoint_sync);
io_eff(CID(Checkpoint.keep), checkpoint_keep);
io_eff(CID(Checkpoint.read), checkpoint_read);
