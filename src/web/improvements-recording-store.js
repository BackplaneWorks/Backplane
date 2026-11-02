import { RECORDING } from './improvements-recording.js';
const validId = id => typeof id === 'string' && /^[a-f0-9]{32}$/.test(id);
const expired = file => !file.size || file.size > RECORDING.maxBytes || file.lastModified < Date.now() - RECORDING.ttlMs || file.lastModified > Date.now() + 60000;
// Desktop measures real file allocation. Web uses private origin storage;
// its payload quota is measured, physical allocation belongs to the browser.
export class RecordingStore {
  constructor({ bridge = globalThis.backplaneDesktop, storage = globalThis.navigator?.storage } = {}) { this.bridge = bridge;this.storage = storage;this.queue = Promise.resolve(); }
  serial(fn) { const work = this.queue.catch(() => {}).then(fn);this.queue = work;return work; }
  async directory() { if (!this.storage?.getDirectory) throw Error('Private recording storage is unavailable.');return (await this.storage.getDirectory()).getDirectoryHandle('diagnostics-recordings', { create: true }); }
  async sweep(dir, reserve = 0) {
    const rows = [];
    for await (const [name, handle] of dir.entries()) {
      if (!/^[a-f0-9]{32}\.webm$/.test(name) || handle.kind !== 'file') throw Error('Recording storage needs cleanup.');
      const file = await handle.getFile();if (expired(file)) await dir.removeEntry(name);else rows.push({ name, size: file.size, at: file.lastModified });
    }
    rows.sort((a,b) => a.at - b.at);
    while (rows.length > RECORDING.keep - (reserve ? 1 : 0) || rows.reduce((s,r) => s+r.size,0) + reserve * 2 > RECORDING.diskBytes) { const old = rows.shift();if (!old) throw Error('Recording storage is full.');await dir.removeEntry(old.name); }
    return rows;
  }
  put(file) { return this.serial(async () => {
    if (file.type !== 'video/webm' || !file.size || file.size > RECORDING.maxBytes) throw Error('Recording exceeds the local size limit.');
    const magic = new Uint8Array(await file.slice(0, 4).arrayBuffer());
    if (magic.length !== 4 || !magic.every((v,i) => v === [0x1a,0x45,0xdf,0xa3][i])) throw Error('Expected a WebM recording.');
    const id = Array.from(crypto.getRandomValues(new Uint8Array(16)),x=>x.toString(16).padStart(2,'0')).join('');
    if (this.bridge?.recording) { await this.bridge.recording.put(id, await file.arrayBuffer());return { id, size: file.size, at: Date.now() }; }
    const dir = await this.directory();await this.sweep(dir, file.size);
    try { const handle = await dir.getFileHandle(id+'.webm', { create: true }), writer = await handle.createWritable();try { await writer.write(file);await writer.close(); } catch(e) { await writer.abort().catch(()=>{});throw e; }await this.sweep(dir); }
    catch { await dir.removeEntry(id+'.webm').catch(()=>{});throw Error('Could not save the recording. Free local storage and record again.'); }
    return { id, size: file.size, at: Date.now() };
  }); }
  get(ref) { return this.serial(async () => {
    if (!validId(ref?.id)) throw Error('Recording is unavailable.');
    if (this.bridge?.recording) return new File([await this.bridge.recording.read(ref.id)],'backplane-recording.webm',{type:'video/webm'});
    const dir = await this.directory();await this.sweep(dir);const file = await (await dir.getFileHandle(ref.id+'.webm')).getFile();return new File([file], 'backplane-recording.webm', { type: 'video/webm' });
  }); }
  remove(ref) { return this.serial(async () => { if (!validId(ref?.id)) return;if (this.bridge?.recording) await this.bridge.recording.remove(ref.id);else await (await this.directory()).removeEntry(ref.id+'.webm').catch(()=>{}); }); }
  clear() { return this.serial(async () => { if (this.bridge?.recording) return this.bridge.recording.clear();const dir = await this.directory();for (const r of await this.sweep(dir)) await dir.removeEntry(r.name); }); }
}
