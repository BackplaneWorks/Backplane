const fs = require('node:fs/promises');
const path = require('node:path');
const net = require('node:net');
const { spawn, execFile } = require('node:child_process');
const { promisify } = require('node:util');
const runFile = promisify(execFile);
const { createHash } = require('node:crypto');

// /hello is the existing service's small CBOR map. Do not pull the domain
// model into Electron or attempt to decode its event stream here.
function hello(bytes) {
  let i = 0;
  function read() {
    const b = bytes[i++], major = b >> 5, add = b & 31;
    let n = add;
    if (add === 24) n = bytes[i++];
    else if (add === 25) { n = bytes.readUInt16BE(i); i += 2; }
    else if (add === 26) { n = bytes.readUInt32BE(i); i += 4; }
    else if (add > 26) throw new Error('Unsupported service greeting');
    if (major === 0) return n;
    if (major === 3) { const s = bytes.subarray(i, i + n).toString('utf8'); i += n; return s; }
    if (major === 5) { const o = {}; for (let j = 0; j < n; j++) o[read()] = read(); return o; }
    throw new Error('Invalid service greeting');
  }
  const result = read();
  if (i !== bytes.length || !result.backplane || typeof result.backplane !== 'string') throw new Error('Not a Backplane service');
  return result;
}
function lockLine(text) {
  const match = /^(\d+) (\d+) flock\s*$/.exec(text);
  if (!match || +match[1] < 1 || +match[2] < 1 || +match[2] > 65535) throw Object.assign(new Error('Invalid service lock'), { code: 'ERR_SERVICE_LOCK' });
  return { posixPid: +match[1], port: +match[2] };
}
async function greeting(origin) {
  const response = await fetch(`${origin}/hello`, { signal: AbortSignal.timeout(1500), redirect: 'error' });
  if (!response.ok) throw Object.assign(new Error('Service greeting failed'), { code: response.status === 503 ? 'ERR_SERVICE_BUSY' : 'ERR_SERVICE_GREETING' });
  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.length > 4096) throw new Error('Service greeting too large');
  return hello(bytes);
}
async function freePort(preferred = 0) {
  const server = net.createServer();
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(preferred, '127.0.0.1', () => { const port = server.address().port; server.close(() => resolve(port)); });
  });
}
function preferredPort(home) {
  // Origin is part of the browser cache boundary. An owned service should
  // return to the same loopback origin after an ordinary app-process restart.
  return 30000 + createHash('sha256').update(path.resolve(home)).digest().readUInt32BE(0) % 30000;
}
async function servicePort(home) {
  try { return await freePort(preferredPort(home)); }
  catch (error) { if (error.code !== 'EADDRINUSE') throw error;return freePort(); }
}
// Node owns native Windows paths and child handles; only the Cygwin service's
// argv/environment crosses into POSIX paths. Never use a PATH-resolved helper.
async function launchPaths({ home, binary, web, env, platform = process.platform, run = runFile, files = fs }) {
  if (platform !== 'win32') return { home, web, env };
  const absolute = value => typeof value === 'string' && !/[\0\r\n]/.test(value) &&
    (/^[A-Za-z]:[\\/]/.test(value) || /^\\\\[^\\/]+[\\/][^\\/]+/.test(value));
  if (![home, binary, web].every(absolute)) throw new Error('Windows service paths must be fully qualified native paths');
  // The JS (Bun) service is a native Windows program: it takes native paths
  // as they are; only the Cygwin service needs them mapped. Its own BUILD.json,
  // beside it, says which it is (serviceRuntime "bun").
  let runtime = '';
  try { runtime = JSON.parse(await files.readFile(path.win32.join(path.win32.dirname(binary), 'BUILD.json'), 'utf8')).serviceRuntime || ''; } catch {}
  if (runtime === 'bun') {
    // The bundled Cygwin runtime (sh, bash, coreutils) beside it, as the hub
    // finds it too: first on PATH, bash as SHELL, and the /tmp Cygwin bash
    // checks for (a ZIP keeps no empty folder)
    const bin = path.win32.join(path.win32.dirname(binary), 'runtime', 'bin');
    const childEnv = { ...env, BACKPLANE_HOME: home };
    let hasRuntime = false;
    try { hasRuntime = (await files.lstat(path.win32.join(bin, 'bash.exe'))).isFile?.() === true; } catch {}
    if (hasRuntime) {
      const pathKey = Object.keys(childEnv).find(k => k.toLowerCase() === 'path') || 'PATH';
      const rest = String(childEnv[pathKey] || '').split(';').filter(p => p && p.toLowerCase() !== bin.toLowerCase());
      for (const k of Object.keys(childEnv)) if (k.toLowerCase() === 'path') delete childEnv[k];
      childEnv.PATH = [bin, ...rest].join(';');
      if (!childEnv.SHELL) childEnv.SHELL = path.win32.join(bin, 'bash.exe');
      try { await files.mkdir(path.win32.join(path.win32.dirname(bin), 'tmp'), { recursive: true, mode: 0o700 }); } catch {}
    }
    return { home, web, env: childEnv };
  }
  const runtimeBin = path.win32.join(path.win32.dirname(binary), 'runtime', 'bin');
  const helper = path.win32.join(runtimeBin, 'cygpath.exe');
  const childEnv = { ...env, LC_ALL: 'C.UTF-8' };
  const systemRootKey = Object.keys(env).find(k => k.toLowerCase() === 'systemroot');
  const systemRoot = systemRootKey && env[systemRootKey];
  if (!absolute(systemRoot)) throw new Error('Windows SystemRoot must be native and absolute');
  if (runtimeBin.includes(';') || systemRoot.includes(';')) throw new Error('Windows runtime loader paths cannot contain a PATH separator');
  // Closed loader lookup: no ambient provider/Cygwin PATH or inherited cwd.
  for (const k of Object.keys(childEnv)) if (k.toLowerCase() === 'path') delete childEnv[k];
  childEnv.PATH = [runtimeBin, path.win32.join(systemRoot, 'System32'), systemRoot].join(';');
  // Cygwin bash checks literal /tmp even when TMPDIR is set. ZIP payloads
  // omit empty directories, so recreate this owned portable-runtime scaffold
  // before the first helper can emit a warning into a shell protocol reply.
  const runtimeTmp = path.win32.join(path.win32.dirname(runtimeBin), 'tmp');
  try {
    await files.mkdir(runtimeTmp, { recursive: true, mode: 0o700 });
    const st = await files.lstat(runtimeTmp);
    if (!st.isDirectory() || st.isSymbolicLink()) throw new Error('Not a real directory');
    await files.access(runtimeTmp, fs.constants.W_OK);
  } catch {
    throw Object.assign(new Error('Packaged Windows runtime/tmp must be a writable real directory'), { code: 'ERR_WINDOWS_RUNTIME_TMP' });
  }
  const convert = async value => {
    if (!absolute(value)) throw new Error('Windows service path environment must be native and absolute');
    let result;
    try {
      result = await run(helper, ['-a', '-u', '--', value], {
        env: { ...childEnv, LC_ALL: 'C.UTF-8' }, encoding: 'utf8', timeout: 5000,
        maxBuffer: 65536, windowsHide: true, shell: false, cwd: runtimeBin,
      });
    } catch { throw new Error('Cannot map paths with the packaged Windows runtime'); }
    const mapped = result.stdout.replace(/\r?\n$/, '');
    if (!mapped.startsWith('/') || /[\0\r\n]/.test(mapped)) throw new Error('Invalid packaged Windows path conversion');
    return mapped;
  };
  const posixHome = await convert(home), posixWeb = await convert(web);
  childEnv.BACKPLANE_HOME = posixHome;
  for (const key of ['BACKPLANE_AGENTS', 'BACKPLANE_LOCK_DIR']) if (childEnv[key]) childEnv[key] = await convert(childEnv[key]);
  return { home: posixHome, web: posixWeb, env: childEnv, cwd: runtimeBin };
}
class Service {
  constructor({ home, binary, web, expectedVersion, residentService = false }) {
    if (!path.isAbsolute(home)) throw new Error('BACKPLANE_HOME must be absolute');
    this.home = path.resolve(home); this.binary = binary; this.web = web; this.expectedVersion = expectedVersion;
    if (typeof residentService !== 'boolean') throw new Error('Invalid resident service policy');
    this.residentService = residentService;
    this.child = null;
  }
  async discover({ browserReconnectOrigin } = {}) {
    // Only an explicit Browser reconnect may rebind the version. It stays on
    // the already selected service origin and authenticates this home's token;
    // ordinary startup discovery retains the packaged-version hard gate.
    const reconnect = browserReconnectOrigin !== undefined;
    if (reconnect && (!this.connection || browserReconnectOrigin !== this.connection.origin)) throw new Error('Browser reconnect does not match the selected service');
    let line;
    try { line = await fs.readFile(path.join(this.home, 'hub.lock'), 'utf8'); }
    catch (error) { if (error.code === 'ENOENT') return null; throw error; }
    const lock = lockLine(line);
    const origin = `http://127.0.0.1:${lock.port}`;
    // Check before either greeting or credentials can reach a different port.
    if (reconnect && origin !== browserReconnectOrigin) throw new Error('Selected Browser service origin changed');
    let info;
    try { info = await greeting(origin); }
    catch (error) {
      if (error.cause?.code === 'ECONNREFUSED') {
        // On Linux the resident hub writes its lock before restoring history
        // and opening the listener. Wait for that live process rather than
        // launching another hub, which immediately loses the home lock.
        if (process.platform === 'linux') {
          try { await fs.access(`/proc/${lock.posixPid}`); }
          catch (ownerError) { if (ownerError.code === 'ENOENT') return null; throw ownerError; }
          throw Object.assign(new Error('Service is restoring history'), { code: 'ERR_SERVICE_BUSY' });
        }
        return null;
      }
      throw error;
    }
    if (!reconnect && this.expectedVersion && info.backplane !== this.expectedVersion) throw new Error('Service version differs from the packaged version');
    // Authenticating with this home's credential binds the discovered endpoint
    // to the selected home. The legacy lock PID is POSIX (including Cygwin),
    // never a Node/Win32 process identity and never a target for signalling.
    const paired = await fetch(`${origin}/auth/pair`, { method: 'POST', redirect: 'error', signal: AbortSignal.timeout(1500), headers: { Authorization: `Bearer ${await this.token()}` } });
    if (!paired.ok) throw new Error('Discovered service does not authenticate for this home');
    return { origin, version: info.backplane, lockIdentity: { posixPid: lock.posixPid } };
  }
  async start() {
    await fs.mkdir(this.home, { recursive: true, mode: 0o700 });
    const deadline = Date.now() + 300000;
    // A listening service can still be recovering a large log. Keep the local
    // window visible while waiting; a busy endpoint is not permission to spawn.
    while (true) {
      try {
        const existing = await this.discover();
        if (existing) return this.connection = existing;
        break;
      } catch (error) {
        if ((error.name !== 'TimeoutError' && !['ERR_SERVICE_BUSY', 'TimeoutError'].includes(error.code ?? error.name)) || Date.now() >= deadline) throw error;
        await new Promise(resolve => setTimeout(resolve, 100));
      }
    }
    if (!this.binary || !path.isAbsolute(this.binary) || !this.web || !path.isAbsolute(this.web)) throw new Error('Set absolute BACKPLANE_SERVICE and BACKPLANE_WEB paths');
    const port = await servicePort(this.home);
    const launch = await launchPaths({ home: this.home, binary: this.binary, web: this.web,
      env: { ...process.env, BACKPLANE_HOME: this.home, ...(this.residentService ? { BACKPLANE_NO_UPDATE: '1' } : {}) } });
    // Like `backplane --headless`: the listener stays on 127.0.0.1 and the
    // hub's own tailnet handling decides whether the owner's phone reaches it.
    const args = ['--home', launch.home, '--port', String(port), '--host', '127.0.0.1', '--web', launch.web, '--foreground'];
    this.child = spawn(this.binary, args, { stdio: ['ignore', 'ignore', 'ignore'], env: launch.env, ...(launch.cwd ? { cwd: launch.cwd } : {}), windowsHide: true, detached: true });
    this.child.unref();
    let failure;
    this.child.once('error', () => { failure = new Error('Cannot start the packaged service'); });
    this.child.once('exit', code => { failure = new Error(`Service exited (${code})`); });
    while (Date.now() < deadline) {
      if (failure) throw failure;
      try {
        const connection = await this.discover();
        if (connection) {
          // Retain our captured native child handle even when another launcher
          // won the home lock. A losing child exits on its own. Lock-file PID
          // equality is not an ownership test (Cygwin IDs differ on Windows).
          return this.connection = connection;
        }
      } catch (error) {
        if (error.name !== 'TimeoutError' && !['ECONNREFUSED', 'ENOENT', 'ERR_SERVICE_LOCK', 'ERR_SERVICE_BUSY', 'TimeoutError'].includes(error.cause?.code ?? error.code ?? error.name) && error.message !== 'fetch failed') throw error;
      }
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw new Error('Service readiness timed out');
  }
  async token() {
    const token = (await fs.readFile(path.join(this.home, 'token'), 'utf8')).trim();
    if (!/^[0-9a-f]{32}$/.test(token)) throw new Error('Invalid service credential');
    return token;
  }
  stop() {
    // Only the child captured here is ours. Never signal a discovered service.
    if (this.child && this.child.exitCode === null) {
      if (process.platform === 'win32') throw new Error('Explicit Windows service shutdown needs the approved service protocol');
      this.child.kill('SIGTERM');
    }
    this.child = null;
  }
}
module.exports = { Service, hello, lockLine, preferredPort, servicePort, launchPaths };
