// Remaining CEF acceptance, Oct 18: isolated X11 clipboard + five-page load.
// Reuse deployed helper/runtime; never touch the person's display, profile or hub.
// Run through bp-admit --mem 6 --cores 2. Results are partial until real IME passes.
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { check, until, jsonProcess, privateDisplay } from '../../tools/cef/test/harness.ts';
const out = resolve(process.env.CEF_ACCEPT_OUT ?? 'build/cef-remaining');
mkdirSync(out, { recursive: true });
const helperPath = process.env.BACKPLANE_BROWSER!;
const hostPath = process.env.BACKPLANE_CEF!;
if (!helperPath || !hostPath) throw new Error('explicit deployed helper and CEF paths required');
const hz = Number(Bun.spawnSync(['getconf', 'CLK_TCK']).stdout.toString().trim());
const sampleMs = Number(process.env.CEF_SAMPLE_MS ?? 8000);
const rounds = Number(process.env.CEF_ROUNDS ?? 3);
const result: any = { at: new Date().toISOString(), scope: 'five owner pages, helper+owned browser tree; no Bend window, private Xvfb/software GPU', helperPath, hostPath, sampleMs, rounds, clipboard: {}, runs: [], ime: { passed: false, reason: 'Japanese conversion engine absent; real desktop composition unverified' } };
const pageHtml = `<!doctype html><meta charset=utf-8><title>Acceptance</title><body style="margin:20px;font:18px sans-serif"><h1>Project dashboard</h1><textarea id=q style="width:500px;height:90px"></textarea><div id=status>Ready</div><canvas id=c width=800 height=480></canvas><script>
window.mode='static';window.frames=0;window.polls=0;window.generation=0;
const ctx=c.getContext('2d');
function draw(){frames++;ctx.fillStyle='hsl('+(frames%360)+',60%,25%)';ctx.fillRect(0,0,800,480);ctx.fillStyle='white';ctx.font='28px sans-serif';ctx.fillText('Dashboard '+frames,30,100);for(let i=0;i<48;i++)ctx.fillRect(i*16,200+Math.sin((i+frames)/15)*60,8,100);}
window.setMode=m=>{mode=m;const g=++generation;if(m==='anim'){function frame(){if(mode!=='anim'||g!==generation)return;draw();requestAnimationFrame(frame)}requestAnimationFrame(frame)}else{ctx.clearRect(0,0,800,480)}};
setInterval(()=>{if(mode==='poll'){polls++;status.textContent='Task poll '+polls}},1000);
</script></body>`;
const site = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: () => new Response(pageHtml, { headers: { 'content-type': 'text/html; charset=utf-8' } }) });
const d = await privateDisplay();
const env = { ...d.env, BACKPLANE_CEF: hostPath, BACKPLANE_CEF_NO_GPU: '1' };
let helper: ReturnType<typeof jsonProcess> | null = null;
const ownClipboard: ReturnType<typeof Bun.spawn>[] = [];
async function run(cmd: string[]) {
  const p = Bun.spawn(cmd, { env, stdout: 'pipe', stderr: 'inherit' });
  const text = await new Response(p.stdout).text();
  if (await p.exited) throw new Error('failed: '+cmd[0]);
  return text;
}
function start(name: string) {
  return jsonProcess([helperPath, '--profile', join(d.root,name,'profile'), '--jpeg-dir', join(d.root,name,'shots'), '--frames', join(d.root,name,'frames.rgb')], env);
}
async function closeHelper() {
  if (helper && helper.proc.exitCode === null) { helper.proc.stdin.end(); await helper.proc.exited; }
  helper = null;
}
function tree(root: number): number[] {
  const parents = new Map<number,number[]>();
  for (const s of readdirSync('/proc')) if (/^\d+$/.test(s)) try {
    const v=readFileSync(`/proc/${s}/stat`,'utf8');const fields=v.slice(v.lastIndexOf(')')+2).split(' ');const pp=Number(fields[1]);
    parents.set(pp,[...(parents.get(pp)??[]),Number(s)]);
  } catch {}
  const pids=[root];for(let i=0;i<pids.length;i++)pids.push(...(parents.get(pids[i])??[]));return pids;
}
function usage(root: number) {
  const ticks: Record<number,number> = {};let pss=0,rss=0;
  for(const pid of tree(root))try {
    const v=readFileSync(`/proc/${pid}/stat`,'utf8');const f=v.slice(v.lastIndexOf(')')+2).split(' ');ticks[pid]=Number(f[11])+Number(f[12]);
    const m=readFileSync(`/proc/${pid}/smaps_rollup`,'utf8');pss+=Number(/^Pss:\s+(\d+)/m.exec(m)?.[1]??0);rss+=Number(/^Rss:\s+(\d+)/m.exec(m)?.[1]??0);
  } catch {}
  return {ticks,pssMiB:pss/1024,rssSumMiB:rss/1024,processes:Object.keys(ticks).length};
}
try {
  helper=start('clipboard');
  await helper.call('engine',{engine:'cef',parent:d.parent,display:env.DISPLAY});
  await helper.call('open',{page:'clip',url:`http://127.0.0.1:${site.port}/clip`,width:800,height:600});
  await helper.call('show',{page:'clip',x:10,y:10,w:800,h:600});
  await helper.call('focus',{page:'clip'});
  await helper.call('click',{page:'clip',selector:'#q'});
  const fixture=join(d.root,'xclipboard');
  await run(['cc','test/tools/xclipboard.c','-lX11','-o',fixture]);
  const text='Backplane clipboard 日本語 🦊\nsecond line';
  const file=join(d.root,'clipboard.txt');writeFileSync(file,text);
  const fixtureLog=join(d.root,'clipboard.log');
  const owner=Bun.spawn([fixture,'text/plain;charset=utf-8',file,'legacy'],{env,stdin:'ignore',stdout:'ignore',stderr:Bun.file(fixtureLog)});ownClipboard.push(owner);
  await until(()=>{try{return readFileSync(fixtureLog,'utf8').includes('clipboard ready')}catch{return false}},'external selection owner');
  const external=(await d.x('clipboard')).owner;
  await d.x('chord Control_L v');
  await until(()=>helper!.call('evaluate',{page:'clip',expression:`q.value===${JSON.stringify(text)}`}), 'native Unicode paste from another X11 app');
  check('clipboard Unicode paste into native CEF',true);
  await d.x('chord Control_L a');await d.x('chord Control_L c');
  await until(()=>owner.exitCode!==null,'Chromium takes selection ownership');
  const chromium=(await d.x('clipboard')).owner;
  check('copy transfers selection from fixture to Chromium',chromium>0&&chromium!==external);
  const copied=await run(['xclip','-selection','clipboard','-out']);
  check('native CEF copy reaches another X11 app losslessly',copied===text);
  result.clipboard={passed:true,unicode:true,multiline:true,externalOwner:external,chromiumOwner:chromium,fixtureLog:readFileSync(fixtureLog,'utf8')};
  await closeHelper();
  const pages=['visible','bg1','bg2','bg3','bg4'];
  const phases=['static','foreground-animation','background-only','background-with-view','remote-watch'];
  for(let round=0;round<rounds;round++)for(const engine of round%2?['chrome','cef']:['cef','chrome']) {
    helper=start(`r${round}-${engine}`);
    const started=performance.now();
    await helper.call('engine',{engine,...(engine==='cef'?{parent:d.parent,display:env.DISPLAY}:{})});
    for(const page of pages)await helper.call('open',{page,url:`http://127.0.0.1:${site.port}/${page}`,width:800,height:600});
    const meta=await helper.call('status',{page:'visible'});
    const userAgent=await helper.call('evaluate',{page:'visible',expression:'navigator.userAgent'});
    const runResult:any={round,engine,userAgent,openFiveMs:performance.now()-started,phases:[]};result.runs.push(runResult);
    for(const phase of phases) {
      const background=phase.startsWith('background')||phase==='remote-watch';
      for(const page of pages)await helper.call('evaluate',{page,expression:`setMode(${JSON.stringify(page==='visible'?(phase==='foreground-animation'||phase==='remote-watch'?'anim':'static'):(background?(page==='bg1'||page==='bg2'?'anim':'poll'):'static'))})`});
      if(engine==='cef')await helper.call(phase==='background-only'?'hide':'show',{page:'visible',x:10,y:10,w:800,h:600});
      let active=true,busy=false;let actions=0;const errors:string[]=[];
      const lease=async()=>{if(!active)return;try {if(engine==='chrome'&&phase!=='background-only')await helper!.call('cast',{page:'visible',ms:1800});if(phase==='remote-watch')await helper!.call('cast',{page:'bg1',ms:1800});}catch(e){errors.push(String(e))}};
      await lease();const timer=setInterval(()=>{void lease()},900);
      // Agent activity: snapshots of background pages once a second, round robin.
      const agent=setInterval(()=>{if(!active||busy)return;busy=true;void helper!.call('snapshot',{page:pages[1+(actions%4)]}).then(()=>{actions++},e=>errors.push(String(e))).finally(()=>{busy=false})},1000);
      try {
        await Bun.sleep(2500); // settle renderer mode and any prior cast lease
        const beforeFrames=helper.events.filter(e=>e.event==='frame').length;
        const watchedTicks=phase==='remote-watch'?await helper.call('evaluate',{page:'bg1',expression:'frames'}):0;
        const before=usage(helper.proc.pid);const t=performance.now();
        await Bun.sleep(sampleMs);
        const after=usage(helper.proc.pid);const elapsed=(performance.now()-t)/1000;
        const delta=Object.entries(after.ticks).reduce((sum,[pid,n])=>sum+Math.max(0,n-(before.ticks[Number(pid)]??0)),0);
        const ticksByPage=[];for(const page of pages)ticksByPage.push(await helper.call('evaluate',{page,expression:'({mode,frames,polls,hidden:document.hidden})'}));
        const sample={phase,elapsedSeconds:elapsed,cpuPercent:delta/hz/elapsed*100,pssMiB:after.pssMiB,rssSumMiB:after.rssSumMiB,processes:after.processes,frames:helper.events.filter(e=>e.event==='frame').length-beforeFrames,actions,ticksByPage,errors};
        runResult.phases.push(sample);console.log(JSON.stringify({round,engine,...sample}));
        if(errors.length||actions<Math.floor(sampleMs/1000))throw new Error('agent activity/lease failed');
        if(phase==='remote-watch'&&(ticksByPage[1].frames-watchedTicks<sampleMs/1000*20||sample.frames<sampleMs/1000*15))throw new Error('remote animation/cast cadence regressed');
      } finally {active=false;clearInterval(timer);clearInterval(agent);await until(()=>!busy,'agent command drained')}
    }
    await closeHelper();
    writeFileSync(join(out,'results.json'),JSON.stringify(result,null,2));
  }
  result.completed=true;
} catch(e) {result.error=String((e as Error).stack??e);throw e}
finally {
  writeFileSync(join(out,'results.json'),JSON.stringify(result,null,2));
  await d.x('clearclip').catch(()=>{});
  await closeHelper();
  for(const p of ownClipboard)await p.exited;
  site.stop(true);await d.cleanup();
}
