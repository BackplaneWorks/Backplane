// Isolated real-hub/real-MCP pipeline with deterministic provider stand-ins.
// No live home, credentials, remote services or hardware. Run only as part
// of the single combined validation: bun ... APP WIRE_DIR.
import { appendFileSync, chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));

async function agent() {
  const args = process.argv.slice(3), cfg = JSON.parse(args[args.indexOf('--mcp-config') + 1]);
  const url = cfg.mcpServers.backplane.url, session = `triage-${process.pid}`;
  const log = (o: any) => appendFileSync(process.env.TRIAGE_AGENT_LOG!, JSON.stringify({ pid: process.pid, ...o }) + '\n');
  const emit = (o: any) => console.log(JSON.stringify({ session_id: session, uuid: crypto.randomUUID(), ...o }));
  log({ args });
  emit({ type: 'system', subtype: 'init', tools: [], mcp_servers: [], model: 'sonnet', claude_code_version: '2.1.286', capabilities: ['msg_lifecycle_v1'] });
  let call = 0, chain = Promise.resolve();
  const tool = async (name: string, a: any, expectError = false) => {
    const r = await fetch(url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: ++call, method: 'tools/call', params: { name, arguments: a } }) });
    const text = await r.text(); log({ tool: name, a, reply: text });
    const failed = !r.ok || text.includes('"isError":true');
    if (failed !== expectError) throw Error(`${name}: ${text}`);
    return text;
  };
  createInterface({ input: process.stdin }).on('line', line => {
    let o: any; try { o = JSON.parse(line); } catch { return; }
    if (o.type !== 'user') return;
    const text = (Array.isArray(o.message?.content) ? o.message.content : [o.message?.content ?? '']).map((x: any) => typeof x === 'string' ? x : x.text ?? '').join(' ');
    chain = chain.then(async () => {
      log({ prompt: text });
      emit({ type: 'command_lifecycle', command_uuid: o.uuid, state: 'queued' });
      emit({ type: 'command_lifecycle', command_uuid: o.uuid, state: 'started' });
      let final = '';
      if (text.includes('<charter>') || text.startsWith('[mail ')) {
        const input = text.match(/Current actionable input: ([^\s]+)/)?.[1];
        if (!input) throw Error('missing exact input identity');
        // Historical markers belong to earlier notices, not this input.
        const block = text.split(`\n[mail ${input} from `)[1];
        const current = block === undefined ? (text.startsWith('[mail ') ? text.split(`Current actionable input: ${input}\n`)[1] : undefined)
          : block.split(/\n\[mail |<\/mail_history>/)[0];
        if (current === undefined) throw Error(`missing current input body: ${input}`);
        await tool('mail_context', { query: 'hardware' });
        await tool('mail_context', { thread: 'another-owner' }, true);
        if (current.includes('TRIAGE_FAIL')) process.exit(1);
        if (current.includes('TRIAGE_UNKNOWN')) final = 'No durable disposition was recorded; outcome unknown.';
        else if (current.includes('Actual pending ask')) {
          await tool('mail_dispose', { input, mode: 'done', reason: 'attempted mistaken silent classification' }, true);
          await tool('escalate', { to: 'person', input, key: 'actual-source-approval', text: 'A real source approval needs the person. Follow backplane://thread/worker; the approval remains pending.' });
          final = 'Escalated privately; no approval granted.';
        } else if (current.includes('TRIAGE_COMPLETE') || current.includes('TRIAGE_DUPLICATE')) {
          await tool('escalate', { to: 'person', input, key: 'artifact-v1-complete', text: 'Meaningful artifact completion; ask Bub to contextualize it with backplane://thread/worker and backplane://project/p. Hardware remains held.' });
          final = 'Privately queued or already queued, never a raw user message.';
        } else {
          if (current.includes('TRIAGE_RACE_FIRST')) {
            log({ raceReady: input });
            const deadline = Date.now() + 15000;
            while (!(await Bun.file(process.env.TRIAGE_RACE_GATE!).exists())) {
              if (Date.now() > deadline) throw Error('race gate did not open');
              await sleep(20);
            }
          }
          await tool('thread_read', { threadId: 'worker', limit: 2 });
          const denied = await tool('permission_prompt', { tool_name: 'Bash', input: { command: 'status-evidence-private-marker | bounded-output' } });
          if (!denied.includes('deny') || denied.includes('status-evidence-private-marker')) throw Error('triage denial must stay local and redact command');
          await tool('mail_dispose', { input, mode: 'done', reason: 'routine FYI, unchanged known hardware hold' });
          final = 'Notice acknowledged; underlying hardware work remains held.';
        }
      } else if (text.includes('[Private mailroom escalation;')) {
        final = 'Bub: I have a result or decision to review. Hardware testing remains held. [Worker](backplane://thread/worker) · [Project](backplane://project/p).';
      } else if (text.includes('TRIAGE_ASK')) {
        await tool('permission_prompt', { tool_name: 'Bash', input: { command: 'printf isolated-approval-placeholder' }, tool_use_id: 'isolated-approval-1' });
        final = 'Approval returned in isolated fixture.';
      } else if (text.includes('TRIAGE_RACE_')) {
        final = `${text}: routine unchanged hold, no decision.`;
      } else if (text.includes('TRIAGE_DUPLICATE')) {
        await tool('thread_send', { threadId: 'foh', text: 'TRIAGE_COMPLETE: same artifact, direct report route.' });
        final = 'TRIAGE_DUPLICATE: same artifact, launched report route.';
      } else if (text.includes('TRIAGE_COMPLETE')) final = 'TRIAGE_COMPLETE: raw worker artifact result; do not dump me into user chat.';
      else if (text.includes('TRIAGE_UNKNOWN')) final = 'TRIAGE_UNKNOWN: worker reports unknown outcome.';
      else if (text.includes('TRIAGE_FAIL')) final = 'TRIAGE_FAIL: triage provider will fail.';
      else final = 'TRIAGE_QUIET: routine unchanged hold.';
      emit({ type: 'assistant', message: { id: `m-${crypto.randomUUID()}`, role: 'assistant', content: [{ type: 'text', text: final }] }, parent_tool_use_id: null });
      emit({ type: 'result', subtype: 'success', is_error: false, num_turns: 1, result: final, usage: {} });
      emit({ type: 'command_lifecycle', command_uuid: o.uuid, state: 'completed' });
    }).catch(e => { log({ error: String(e) }); process.exit(1); });
  });
}

if (process.argv[2] === '--agent') await agent();
else {
  const bin = resolve(process.argv[2] ?? ''), wire = resolve(process.argv[3] ?? 'build/wire');
  for (const f of readdirSync(wire).filter(f => f.endsWith('.js'))) (0, eval)(readFileSync(join(wire, f), 'utf8'));
  const W = (globalThis as any).Wire;
  const root = mkdtempSync(join(tmpdir(), 'bp-mailtriage-')), home = join(root, 'home'), fake = join(root, 'agents'), project = join(root, 'project'), bot = join(root, 'bot');
  for (const d of [home, fake, project, bot]) mkdirSync(d);
  const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
  writeFileSync(join(fake, 'claude'), `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(resolve(import.meta.path))} --agent "$@"\n`); chmodSync(join(fake, 'claude'), 0o755);
  for (const name of ['codex', 'grok', 'muse', 'agy', 'opencode']) { writeFileSync(join(fake, name), '#!/bin/sh\nexit 1\n'); chmodSync(join(fake, name), 0o755); }
  const ev = ($: string, a: object) => ({ $, ...a, at: 1 });
  const journal = join(home, 'events.jsonl'), agentLog = join(root, 'agents.jsonl'); writeFileSync(agentLog, '');
  writeFileSync(journal, [ev('ProjectCreated', { id: 'p', title: 'Project', root: project }),
    ev('ThreadCreated', { id: 'worker', project: 'p', title: 'Worker', env: 'local', branch: '', worktree: project, provider: 'claude', model: 'sonnet' }),
    ev('ThreadCreated', { id: 'worker-fail', project: 'p', title: 'Storage failure source', env: 'local', branch: '', worktree: project, provider: 'claude', model: 'sonnet' }),
    ev('ThreadLaunched', { id: 'worker-fail', by: 'foh', report: true }),
    ev('ThreadCreated', { id: 'foh', project: '', title: 'Bub original conversation', env: 'local', branch: '', worktree: bot, provider: 'claude', model: 'sonnet' }),
    ev('BotSet', { id: 'b', name: 'Bub', thread: 'foh', look: 0, persona: 'Coordinate; author your own user-facing status.' }),
    ev('SettingSet', { key: 'uber.bot', value: 'b' }), ev('ThreadLaunched', { id: 'worker', by: 'foh', report: true }),
    ev('SettingSet', { key: 'mail.on', value: 'off' }), ev('SettingSet', { key: 'text.provider', value: 'claude' }), ev('SettingSet', { key: 'text.model', value: 'sonnet' }),
    ev('SettingSet', { key: 'restart.continue', value: 'off' }), ev('MessagePosted', { thread: 'foh', id: 'owner-context', role: 'user', text: 'Keep hardware tests on hold. UDP address width is unresolved.' })].map(JSON.stringify).join('\n') + '\n');
  const listener = Bun.listen({ hostname: '127.0.0.1', port: 0, socket: { data() {} } }); const port = listener.port; listener.stop(true);
  const env = { ...process.env, HOME: root, PATH: `${fake}:${process.env.PATH}`, BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_GOOGLE_BAKE: '0', BACKPLANE_TELEMETRY: 'off', BACKPLANE_PEERS: '', BACKPLANE_KEYRING_LOCKED: '0', BACKPLANE_LOCK_DIR: join(root, 'locks'), BACKPLANE_DEPLOY_LOCKS: join(root, 'deploy'), TRIAGE_AGENT_LOG: agentLog, TRIAGE_RACE_GATE: join(root, 'race-gate'), DISPLAY: '', WAYLAND_DISPLAY: '' };
  let proc: ReturnType<typeof Bun.spawn> | undefined, ws: WebSocket | undefined, failures = 0, request = 0;
  const events = () => readFileSync(journal, 'utf8').trim().split('\n').flatMap(l => { try { return [JSON.parse(l)]; } catch { return []; } });
  const logs = () => readFileSync(agentLog, 'utf8').trim().split('\n').flatMap(l => { try { return [JSON.parse(l)]; } catch { return []; } });
  const until = async (f: () => boolean, ms = 20000) => { const deadline = Date.now() + ms; while (Date.now() < deadline) { if (f()) return true; await sleep(40); } return false; };
  const check = (ok: boolean, name: string) => { console.log(`${ok ? 'ok' : 'FAIL'} ${name}`); if (!ok) failures++; };
  const ownerReplies = () => events().filter(e => e.$ === 'MessagePosted' && e.thread === 'foh' && e.role === 'assistant');
  const send = (text: string, thread = 'worker') => ws!.send(W.encode(JSON.stringify({ id: ++request, m: 'turn.start', p: { thread, text, msg: `person-${request}` } })));
  const start = async () => {
    proc = Bun.spawn([bin, '--home', home, '--port', String(port), '--no-tailscale', '--foreground'], { env, stdout: Bun.file(join(root, 'stdout')), stderr: Bun.file(join(root, 'stderr')) });
    let ready = false; for (let i = 0; i < 300; i++) { try { ready = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {} if (ready) break; await sleep(40); }
    if (!ready) throw Error('isolated hub did not start');
    ws = new WebSocket(`ws://127.0.0.1:${port}/ws?since=999999999`);
    await new Promise((r, j) => { ws!.onopen = r; ws!.onerror = j; });
  };
  const stop = async () => { ws?.close(); if (proc?.exitCode === null) proc.kill('SIGTERM'); if (proc) await proc.exited; };
  try {
    await start();
    send('TRIAGE_QUIET');
    check(await until(() => events().some(e => e.$ === 'MailMarked' && e.bot === 'b' && e.status === 'done')), 'quiet launched FYI is assessed and marked done');
    check(ownerReplies().length === 0, 'quiet FYI never wakes Bub');
    check(logs().some(e => e.tool === 'mail_context' && !e.a.thread && e.reply.includes('UDP address width')), 'real history query reads original owner context');
    check(logs().some(e => e.tool === 'mail_context' && e.a.thread && e.reply.includes('"isError":true')), 'history owner spoof fails closed');
    check(logs().some(e => e.tool === 'thread_read' && e.reply.includes('status') && e.reply.includes('turn')), 'triage reads source status through supported tool');
    check(logs().some(e => e.tool === 'permission_prompt' && e.reply.includes('deny') && !e.reply.includes('status-evidence-private-marker')), 'refused internal lookup stays local without raw command');
    const mailBeforeRace = events().filter(e => e.$ === 'MailPosted' && e.conv === 'launch:worker').length;
    send('TRIAGE_RACE_FIRST');
    check(await until(() => logs().some(e => e.raceReady)), 'race first input holds triage before disposition');
    send('TRIAGE_RACE_SECOND');
    check(await until(() => events().filter(e => e.$ === 'MailPosted' && e.conv === 'launch:worker').length === mailBeforeRace + 2), 'newer report arrives while first input is bound');
    const raceIds = events().filter(e => e.$ === 'MailPosted' && e.conv === 'launch:worker').slice(-2).map(e => e.id);
    writeFileSync(join(root, 'race-gate'), 'release');
    check(await until(() => raceIds.every(id => events().some(e => e.$ === 'SettingSet' && e.key.startsWith('run.disposition.') && e.key.includes(id)))), 'both race notices commit exact dispositions');
    const raceRuns = events().filter(e => e.$ === 'RunSet' && raceIds.includes(e.mail));
    check(raceRuns.length === 2 && raceRuns[0].thread !== raceRuns[1].thread && ownerReplies().length === 0 && !events().some(e => e.$ === 'MessagePosted' && e.thread === raceRuns[0].thread && e.msg === raceIds[1]), 'newer notice uses separate run with no injection or user escalation');
    send('TRIAGE_COMPLETE');
    check(await until(() => events().some(e => e.$ === 'SettingSet' && e.key.startsWith('triage.event.') && String(e.key).includes('artifact-v1-complete')) && ownerReplies().length === 1), 'triaged completion reaches original Bub conversation');
    check(ownerReplies()[0]?.text.includes('[Worker](backplane://thread/worker)'), 'Bub authors a fresh contextual linked status');
    send('TRIAGE_DUPLICATE');
    check(await until(() => events().some(e => e.$ === 'SettingSet' && e.key.startsWith('triage.escalated.') && String(e.value).startsWith('duplicate:'))), 'duplicate direct/launch routes share durable exact-event suppression');
    await sleep(300); check(ownerReplies().length === 1, 'same artifact surfaces once');
    send('TRIAGE_UNKNOWN');
    check(await until(() => ownerReplies().length === 2), 'unknown triage result forwards unresolved work');
    send('TRIAGE_FAIL');
    check(await until(() => ownerReplies().length === 3), 'failed triage process cannot silently swallow important input');
    const beforeRestart = ownerReplies().length; await stop(); await start(); await sleep(300);
    check(ownerReplies().length === beforeRestart, 'recovery does not replay acknowledged escalations');
    // A directory at this fresh conversation's journal path deterministically
    // fails append, without chmod assumptions or touching any real mailbox.
    const badJournal = join(home, 'mail', encodeURIComponent('launch:worker-fail') + '.jsonl');
    mkdirSync(badJournal);
    send('TRIAGE_STORAGE_FAILURE', 'worker-fail');
    check(await until(() => ownerReplies().length === 4), 'mail journal failure reaches owner through independent private delivery');
    check(!events().some(e => e.$ === 'MailPosted' && e.conv === 'launch:worker-fail'), 'failed append is never acknowledged as filed');
    rmSync(badJournal, { recursive: true });
    send('TRIAGE_ASK');
    check(await until(() => events().some(e => e.$ === 'AskOpened' && e.thread === 'worker')), 'actual source approval retained');
    check(await until(() => ownerReplies().length === 5), 'worker approval routes through triage and fresh Bub response');
    const ask = events().find(e => e.$ === 'AskOpened' && e.thread === 'worker');
    check(!!ask && !events().some(e => e.$ === 'AskClosed' && e.id === ask.id), 'triage never invents an approval or closes the source ask');
    check(!events().some(e => e.$ === 'MessagePosted' && e.thread === 'foh' && e.id !== 'owner-context' && e.role !== 'assistant'), 'no worker/triage messages dumped into original conversation');
    check(!events().some(e => e.$ === 'SettingSet' && e.key === 'alert.worker' && !String(e.value).startsWith('\t')), 'source worker emits no unsolicited alert');
  } finally {
    await stop();
    for (const pid of new Set(logs().map(e => e.pid).filter(Number.isInteger))) { try { process.kill(pid, 'SIGTERM'); } catch {} }
    if (failures || process.env.KEEP) console.log(`artifacts ${root}`); else rmSync(root, { recursive: true, force: true });
  }
  process.exit(failures ? 1 : 0);
}
