// Synthetic Node process tree. No hub, store, network, provider or credentials.
const fs = require('node:fs'), cp = require('node:child_process'), { join } = require('node:path');
const { Worker, isMainThread, workerData } = require('node:worker_threads');
const root = process.argv[2], mode = process.argv[3];
if (mode === 'leaf') {
  fs.writeFileSync(join(root, 'leaf-' + process.pid), '1'); setInterval(() => {}, 1000);
} else if (!isMainThread) {
  const root = workerData.root;
  const spawn = () => {
    const child = cp.spawn(process.execPath, [__filename, root, 'leaf'], { stdio: 'ignore', env: { PATH: '/usr/bin:/bin', HOME: root } });
    // Deliberately not detached: scope membership, not a process group, owns it.
    child.unref(); return child.pid;
  };
  const first = spawn(); fs.writeFileSync(join(root, 'worker-child'), String(first));
  let switched = false;
  setInterval(() => {
    if (!fs.existsSync(join(root, 'switch')) || switched) return;
    switched = true; const late = spawn(); fs.writeFileSync(join(root, 'late-child'), String(late));
    if (workerData.race) { let extra = 0; setInterval(() => { if (extra++ < 12) spawn(); }, 10); }
    else fs.writeFileSync(join(root, 'exit-ready'), '1');
  }, 5);
} else {
  fs.writeFileSync(join(root, 'leader'), String(process.pid));
  new Worker(__filename, { workerData: { root, race: mode === 'race' } });
  setInterval(() => { if (mode === 'orphan' && fs.existsSync(join(root, 'exit-ready'))) process.exit(0); }, 5);
}
