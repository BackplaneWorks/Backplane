// Per-press timing verdicts and the friction report (pure; unit-tested in report.test.mjs). Every adapter returns `steps`:
// [{step, ok, ms, ackMs, error, phases}] in the order the client ran them; this file classifies each step, judges it against the
// person-facing budgets in journeys.json (`step_budgets_ms`), and ranks everything a person would trip over by how much it hurts.
export const DEFAULT_BUDGETS={press:[300,2000],navigation:[1000,3000],viewer:[5000,15000],reply:[3000,8000],network:[10000,20000]};
// what kind of wait a step is, from the client's step label (GPUI: select:N, home:N, click:rail:Board, popup:/, type:12, send, net_sleep:45 ...)
export function kindOf(label=''){
  if(/^(select|home):/.test(label))return 'navigation';
  if(/^viewer_drawn:/.test(label))return 'viewer'; // (no client reports a drawn viewer yet: a rail click is a press, its draw is not timed)
  if(/^net_sleep:/.test(label))return 'network';
  if(label==='reply'||/^wait_reply/.test(label))return 'reply';
  return 'press'}
// ok within the first number, slow within the second, fail past it; a step that never showed its result is a fail whatever its time
export function verdict(ms,kind,budgets=DEFAULT_BUDGETS,ok=true){
  if(!ok)return 'fail';const [a,b]=budgets[kind]||budgets.press;if(!Number.isFinite(ms))return 'unmeasured';return ms<=a?'ok':ms<=b?'slow':'fail'}
// net_sleep's latency includes the sleep itself: judge only the time after it (the reconnect)
const judged=s=>{const m=/^net_sleep:([\d.]+)/.exec(s.step||'');return m?s.ms-Number(m[1])*1000:s.ms};
export function judgeSteps(steps,budgets=DEFAULT_BUDGETS){
  return steps.map(s=>{const kind=s.kind||kindOf(s.step);const ms=judged(s);return {...s,kind,judgedMs:ms,budget:budgets[kind]||budgets.press,verdict:verdict(ms,kind,budgets,s.ok!==false)}})}
export function stepSummary(steps){const c={ok:0,slow:0,fail:0,unmeasured:0};for(const s of steps)c[s.verdict]=(c[s.verdict]||0)+1;
  const timed=steps.filter(s=>Number.isFinite(s.judgedMs));const slowest=timed.reduce((a,s)=>!a||s.judgedMs>a.judgedMs?s:a,null);
  return {...c,n:steps.length,slowest:slowest?{step:slowest.step,ms:slowest.judgedMs,kind:slowest.kind,verdict:slowest.verdict}:null}}

// Harm ranking: what costs the person most comes first. A broken journey (a press that never answers, a crash) beats a slow one; a
// slow one beats a cosmetic one. `blocked` is a coverage gap, not harm to the person: listed apart, never ranked among the frictions.
const HARM={crash:100,step_failed:90,over_fail_budget:80,expect_failed:75,layout_jump:60,dead_looks_enabled:50,slow:30,frame_jank:20,click_over_frame:10,disabled_no_handler:5};
export function frictions(results){const out=[];const add=(r,kind,what,evidence,extra=0)=>out.push({harm:HARM[kind]+extra,kind,journey:r.journey,story:r.story||null,what,evidence});
  for(const r of results){if(r.status==='blocked')continue;
    for(const n of r.notes||[])if(/harness error|client did not finish|no input metrics/.test(n))add(r,'crash',n.slice(0,160),r.screenshot||'');
    for(const s of r.steps||[]){
      if(s.ok===false)add(r,'step_failed',`${s.step} never showed its result${s.error?' ('+s.error+')':''}`,`after ${fmt(s.ms)}`);
      else if(s.verdict==='fail')add(r,'over_fail_budget',`${s.step} took ${fmt(s.judgedMs)} (${s.kind}: fail over ${s.budget?.[1]??'?'} ms)`,phases(s),Math.min(9,s.judgedMs/1000));
      else if(s.verdict==='slow')add(r,'slow',`${s.step} took ${fmt(s.judgedMs)} (${s.kind}: ok under ${s.budget?.[0]??'?'} ms)`,phases(s),Math.min(9,s.judgedMs/500))}
    for(const e of r.expects||[])if(e.ok===false)add(r,'expect_failed',`expected ${e.what}, got ${e.got}`,'');
    const m=r.metrics||{};
    if(m.layout&&m.layout.pass===false)add(r,'layout_jump',`layout moved with no input (score ${m.layout.score?.toFixed?.(3)}, ${m.layout.shifts} shifts)`,(m.layout.moved||[]).slice(0,3).map(x=>x.ids.join('+')).join('; '));
    for(const d of m.dead?.dead||[])add(r,'dead_looks_enabled',`control "${d.label||d.id}" does nothing when clicked`,d.id);
    for(const d of m.dead?.deadLooksEnabled||[])add(r,'dead_looks_enabled',`control "${d.label||d.id}" has no handler but looks live`,d.id);
    for(const d of m.dead?.disabledNoHandler||[])add(r,'disabled_no_handler',`control "${d.label||d.id}" has no handler (drawn disabled)`,d.id);
    if(m.frames&&m.frames.pass===false)add(r,'frame_jank',`frames: ${m.frames.jank} janks, p99 ${fmt(m.frames.dtMs?.p99)}${r.gpu?.software?' (software-rendered)':''}`,m.frames.source||'');
    if(m.click&&m.click.pass===false&&m.click.overBudget)add(r,'click_over_frame',`${m.click.overBudget} of ${m.click.n} presses answered later than one frame (p95 ack ${fmt(m.click.ackMs?.p95)})${r.gpu?.software?' (software-rendered)':''}`,m.click.source||'')}
  return out.sort((a,b)=>b.harm-a.harm||String(a.journey).localeCompare(String(b.journey)))}
const fmt=ms=>Number.isFinite(ms)?(ms>=1000?(ms/1000).toFixed(2)+' s':ms.toFixed(0)+' ms'):'-';
const phases=s=>s.phases?Object.entries(s.phases).filter(([,v])=>Number.isFinite(v)).map(([k,v])=>`${k} ${v.toFixed(1)}`).join(', '):'';

// report.md: the totals, the journey table with each one's slowest step, the frictions ranked by harm, what is blocked and on what
export function markdown(summary){const L=[];const rs=summary.results||[];
  L.push(`# e2e ${summary.client} ${summary.when}`,'',`hub ${summary.hubKind||'?'}${summary.copyOf?' (COPY of '+summary.copyOf+', BACKPLANE_COPY=1)':''}, real hardware: ${!!summary.realHardware}, counts ${JSON.stringify(summary.counts||{})}`);
  if(rs.some(r=>r.gpu?.software))L.push('','Software-rendered frames (llvmpipe/lavapipe): frame and one-frame numbers validate the harness, they are not GPU evidence.');
  if(summary.fatal)L.push('','**fatal:** '+summary.fatal.split('\n')[0]);
  L.push('','| journey | story | status | steps ok/slow/fail | slowest step |','|---|---|---|---|---|');
  for(const r of rs){const t=r.stepSummary;L.push(`| ${r.journey} | ${r.story||''} | ${r.status}${r.blocked?.length?' ('+r.blocked.join(', ')+')':''} | ${t?`${t.ok}/${t.slow}/${t.fail}`:'-'} | ${t?.slowest?`${t.slowest.step} ${fmt(t.slowest.ms)} (${t.slowest.verdict})`:'-'} |`)}
  const fr=summary.frictions||[];L.push('',`## Friction, ranked by harm (${fr.length})`,'');
  if(!fr.length)L.push('none found in what ran');
  fr.forEach((f,i)=>L.push(`${i+1}. [${f.kind}, ${f.harm.toFixed(0)}] ${f.journey}${f.story?' ('+f.story+')':''}: ${f.what}${f.evidence?' — '+f.evidence:''}`));
  const bl=rs.filter(r=>r.status==='blocked'||r.blocked?.length);if(bl.length){L.push('','## Blocked (a capability this client lacks; never a pass)','');for(const r of bl)L.push(`- ${r.journey}${r.story?' ('+r.story+')':''}: ${(r.blocked||[]).join(', ')}`)}
  const all=rs.flatMap(r=>(r.steps||[]).map(s=>({...s,journey:r.journey}))).filter(s=>Number.isFinite(s.judgedMs)).sort((a,b)=>b.judgedMs-a.judgedMs).slice(0,25);
  if(all.length){L.push('','## Slowest presses','','| journey | step | kind | ms | ack ms | verdict | where the time went |','|---|---|---|---|---|---|---|');for(const s of all)L.push(`| ${s.journey} | ${s.step} | ${s.kind} | ${s.judgedMs.toFixed(0)} | ${Number.isFinite(s.ackMs)?s.ackMs.toFixed(1):'-'} | ${s.verdict} | ${phases(s)} |`)}
  if(summary.captures?.length)L.push('','Captures: '+summary.captures.join(', '));
  return L.join('\n')+'\n'}
