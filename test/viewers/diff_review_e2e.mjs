// Diff, Review and a board on a hub with NO policy helper, driven as the window drives it (rig.mjs). Temp home, stand-in agents, a temp git repo with real changes.
// usage: HUB=<backplane-hub> RIG=<scratch dir> [PORT=3841] bun test/viewers/diff_review_e2e.mjs
import {startHub, Client, makeProject, changeProject, addRemote, checkpoint0, sleep} from './rig.mjs';
import {readFileSync, existsSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
const BIN = process.env.HUB, RIG = process.env.RIG, PORT = +(process.env.PORT || 3841), P = RIG + '/proj';
let bad = 0; const ok = (c, m) => { console.log((c ? 'PASS ' : 'FAIL ') + m); if (!c) bad++ };
let W = P; const git = (...a) => execFileSync('git', a, {cwd: W}).toString();
makeProject(P); addRemote(P, RIG + '/remote.git');
const hub = await startHub({bin: BIN, rig: RIG, port: PORT});
ok(hub.children === '' && !hub.cmd.includes('--policy-bin'), `hub as the person's: no --policy-bin, no child process (children=${JSON.stringify(hub.children)})`);
ok(!!hub.agentsEnv, 'stand-in agents: ' + hub.agentsEnv);
const c = new Client(PORT, hub.token); await c.connect();
const th = () => c.thread(); const logBad = () => readFileSync(hub.log, 'utf8').split('\n').filter(l => /not ported|PRESENTER_UNAVAILABLE/.test(l));
try {
  await c.view('view.open'); await c.rpc('project.add', {path: P}); await sleep(800); await c.update();
  const id = await c.selectFirstThread(); ok(c.last.ok, 'thread opens ' + id);
  // a thread in a git repository works in a worktree of its own: its folder is in the log
  const ev = readFileSync(RIG + '/home/events.jsonl', 'utf8').split('\n').filter(Boolean).map(l => JSON.parse(l)).filter(e => e.$ === 'WorktreeSet' && e.id === id).pop();
  W = ev?.worktree || P; console.log('  thread folder', W); checkpoint0(W, id); changeProject(W);

  // Diff: the changes since the thread began
  let r = await c.action('diff'); ok(r.ok, 'diff action answered'); await c.update(600); r = await c.update(600);
  let d = th().diff; ok(d && d.files, 'thread.diff is there: ' + JSON.stringify(d)?.slice(0, 160));
  const names = (d?.files || []).map(f => f.name + ':' + f.status).sort();
  ok(JSON.stringify(names) === JSON.stringify(['a.txt:modified', 'gone.txt:deleted', 'new.txt:added']), 'diff lists the modified, deleted and added file: ' + names);
  const a = d.files.find(f => f.name === 'a.txt');
  ok(a.lines.some(l => l.k === 2 && l.t === 'two' && l.o === '2') && a.lines.some(l => l.k === 1 && l.t === '2' && l.n === '2') && a.lines.some(l => l.k === 4 && l.t.startsWith('@@')), 'a.txt lines: removed "two", added "2", the hunk header');
  ok(/^3 files, \+\d+ −\d+$/.test(d.summary), 'summary ' + d.summary);
  ok(d.reverting === false, 'not armed yet');

  // Commit (what the button sends) with a remote to push to
  r = await c.action('git', 'commit_push', 2500); await c.update(500); await c.update(500);
  ok(r.ok, 'Commit answered'); ok(git('log', '--format=%s').trim().split('\n').length === 2, 'the commit landed: ' + git('log', '--oneline').trim().replace(/\n/g, ' | '));
  ok(git('ls-remote', RIG + '/remote.git').includes('refs/heads/backplane/'), 'and was pushed (on the thread branch)');
  ok(!!th().diff, 'Diff stays on show after Commit');
  // an empty commit says it has nothing to do rather than failing the panel
  r = await c.action('git', 'commit', 1500); ok(r.ok, 'Commit again (nothing to commit) answered');

  // Revert: asks first, then puts the files back
  r = await c.action('revert', 'ask', 600); await c.update(300); ok(th().diff?.reverting === true, 'Revert asks first (armed)');
  ok(existsSync(W + '/new.txt'), 'nothing was reverted yet');
  r = await c.action('revert', 'confirm', 2500); await c.update(800); await c.update(800);
  ok(!existsSync(W + '/new.txt') && existsSync(W + '/gone.txt') && readFileSync(W + '/a.txt', 'utf8') === 'one\ntwo\nthree\n', 'Revert put the files back: new.txt gone, gone.txt back, a.txt as it was');
  ok(th().diff?.reverting === false, 'disarmed'); ok(th().diff?.files?.length === 0 && /No changes/.test(th().diff.summary), 'Diff is refreshed: ' + th().diff?.summary);
  // Cancel
  await c.action('revert', 'ask', 400); ok(th().diff?.reverting === true, 'armed again'); await c.action('revert-no', '', 400); ok(th().diff?.reverting === false, 'Cancel disarms');

  // Review: the pages
  r = await c.action('view', 'review', 300); ok(r.ok, 'Review opens'); ok(th().viewer.open === 'review' && !th().diff, 'viewer.open review, Diff put away');
  for (let i = 0; i < 20 && th().viewer.review?.busy; i++) await c.update(500);
  let rv = th().viewer.review; ok(rv && rv.tabs.length === 4 && rv.tabs[0].on, 'tabs: ' + rv?.tabs.map(t => t.label));
  ok(rv.busy === false && rv.blocks.length > 0, 'the BOM report arrived: ' + rv.blocks.length + ' blocks, title ' + rv.title + (rv.blocks.length ? '' : ', note: ' + rv.note));
  console.log('  BOM blocks:', JSON.stringify(rv.blocks).slice(0, 300));
  r = await c.action('rv-page', 'checks', 300); for (let i = 0; i < 40 && th().viewer.review?.busy; i++) await c.update(500);
  rv = th().viewer.review; ok(rv.tabs[1].on && !rv.busy, 'Checks page answered (note=' + JSON.stringify(rv.note) + ', blocks=' + rv.blocks.length + ', tasks=' + rv.checks?.tasks?.length + ')');
  r = await c.action('rv-page', 'comments', 300); ok(th().viewer.review.tabs[2].on, 'Comments page');

  // a board: no refusal, and the reads its look carries are taken
  r = await c.action('view', 'board', 800); ok(r.ok && th().viewer.open === 'board', 'Board opens from Review (viewer.open=' + th().viewer.open + ')');
  for (let i = 0; i < 4; i++) { r = await c.update(500); ok(r.ok, 'update ' + i + ' after the board opened'); }
  r = await c.action('view', 'schematic', 500); ok(r.ok && th().viewer.open === 'schematic', 'Schematic'); r = await c.action('view', '3d', 500); ok(r.ok, '3D');
  r = await c.action('diff', '', 800); await c.update(600); ok(r.ok && !!th().diff, 'Diff from a drawing'); r = await c.action('view', 'board', 500); ok(r.ok && th().viewer.open === 'board' && !th().diff, 'Board from Diff');
  r = await c.action('view', '', 400); ok(r.ok && th().viewer.open === '', 'Close');
  // another thread and back with Review open
  await c.action('view', 'review', 300); for (let i = 0; i < 20 && th().viewer.review?.busy; i++) await c.update(500);
  r = await c.update(300); ok(r.ok && th().viewer.open === 'review', 'Review on show');
  const bad2 = logBad(); ok(bad2.length === 0, 'the hub log has no "not ported" / PRESENTER_UNAVAILABLE: ' + JSON.stringify(bad2));
  ok(!c.frames.some(f => f.t === 'reply' && f.ok === false && f.error?.code === 'PRESENTER_UNAVAILABLE'), 'no reply refused with PRESENTER_UNAVAILABLE');
} catch (e) { console.log('ERR', e.stack); bad++ } finally { c.close(); hub.stop(); }
console.log(bad ? `${bad} FAILED` : 'ALL PASSED'); process.exit(bad ? 1 : 0);
