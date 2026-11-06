// Pure frame-quality metrics. Inputs are plain arrays so every client adapter (GPUI metrics, Android gfxinfo, iOS hitch logs) feeds the same code.
const q=(xs,p)=>{if(!xs.length)return NaN;const s=[...xs].sort((a,b)=>a-b);return s[Math.min(s.length-1,Math.floor(p/100*s.length))]};
export const budgetMs=hz=>1000/hz;

// Clicks: [{id, t_inject, t_ack, t_result}] in ms on one clock. ack = first frame that differs after the click (pressed/hover/state),
// result = the frame that shows the expected state. "Answers within one frame": ack - inject <= one refresh period (+ slack for frame phase).
export function clickStats(clicks,hz,slackMs=0,resolutionMs=0){
  const b=budgetMs(hz)+slackMs;const ack=clicks.filter(c=>Number.isFinite(c.t_ack)).map(c=>c.t_ack-c.t_inject);
  const res=clicks.filter(c=>Number.isFinite(c.t_result)).map(c=>c.t_result-c.t_inject);
  const noAck=clicks.filter(c=>!Number.isFinite(c.t_ack)).map(c=>c.id);
  return {n:clicks.length,hz,budgetMs:b,ackMs:{p50:q(ack,50),p95:q(ack,95),max:ack.length?Math.max(...ack):NaN},resultMs:{p50:q(res,50),p95:q(res,95),max:res.length?Math.max(...res):NaN},
    overBudget:ack.filter(x=>x>b).length,noAck:noAck.length,noAckIds:noAck.slice(0,20),resolutionMs,pass:resolutionMs>b?null:(noAck.length===0&&ack.length>0&&ack.filter(x=>x>b).length===0),note:resolutionMs>b?`evidence resolution ${resolutionMs.toFixed(0)} ms is coarser than the ${b.toFixed(1)} ms budget: pass is unknown, not true`:undefined}}

// Frame times: ascending presentation timestamps in ms (one per presented frame) during a scroll or a stream.
// jank = a frame interval over 2 refresh periods (a visibly missed frame); long = over 50 ms.
export function frameStats(ts,hz){
  const dts=[];for(let i=1;i<ts.length;i++)dts.push(ts[i]-ts[i-1]);
  const b=budgetMs(hz);const span=ts.length>1?ts[ts.length-1]-ts[0]:0;
  const jank=dts.filter(d=>d>2*b+1).length;
  return {frames:ts.length,seconds:span/1000,fps:span>0?(ts.length-1)/(span/1000):NaN,hz,dtMs:{p50:q(dts,50),p95:q(dts,95),p99:q(dts,99),max:dts.length?Math.max(...dts):NaN},
    jank,jankPct:dts.length?100*jank/dts.length:0,longOver50:dts.filter(d=>d>50).length,droppedEstimate:dts.reduce((a,d)=>a+Math.max(0,Math.round(d/b)-1),0),
    pass:dts.length>=10&&jank/dts.length<=0.01&&q(dts,99)<=2*b+1}}

// Layout shift, after the web CLS: snapshots [{t, inputT (ms of the last user input before it, or null), rects:{id:{x,y,w,h}}}], viewport {w,h}.
// A shift counts when an element present in two consecutive snapshots moved without a user input in the previous `quietMs`.
export function layoutShift(snaps,viewport,quietMs=500){
  let score=0;const moved=[];
  for(let i=1;i<snaps.length;i++){const a=snaps[i-1],b=snaps[i];if(b.inputT!=null&&b.t-b.inputT<=quietMs)continue;
    let impact=0,dist=0,ids=[];
    for(const [id,r1] of Object.entries(a.rects)){const r2=b.rects[id];if(!r2)continue;
      const dx=Math.abs(r2.x-r1.x),dy=Math.abs(r2.y-r1.y);if(dx<1&&dy<1)continue;
      const ux=Math.min(viewport.w,Math.max(r1.x+r1.w,r2.x+r2.w))-Math.max(0,Math.min(r1.x,r2.x)),uy=Math.min(viewport.h,Math.max(r1.y+r1.h,r2.y+r2.h))-Math.max(0,Math.min(r1.y,r2.y));
      impact+=Math.max(0,ux)*Math.max(0,uy)/(viewport.w*viewport.h);dist=Math.max(dist,Math.max(dx/viewport.w,dy/viewport.h));ids.push(id)}
    if(ids.length){const s=Math.min(1,impact)*dist;score+=s;moved.push({t:b.t,score:s,ids:ids.slice(0,8)})}}
  return {score,shifts:moved.length,moved:moved.slice(0,20),pass:score<=0.1}}

// Dead controls: inventory [{id,label,enabled,visible}], results {id:{changed:bool,action:bool}} from clicking each once.
export function deadControls(inventory,results,ignore=[]){
  const live=inventory.filter(c=>c.visible!==false&&c.enabled!==false&&!ignore.includes(c.id));
  const dead=live.filter(c=>{const r=results[c.id];return !r||(!r.changed&&!r.action)});
  return {controls:inventory.length,clickable:live.length,dead:dead.map(c=>({id:c.id,label:c.label})),deadCount:dead.length,notClicked:live.filter(c=>!results[c.id]).length,pass:dead.length===0&&live.length>0}}

// Memory growth over a long session: samples [{t_s,rssMB}] after warm-up; least squares slope MB/h with a naive 95% half width.
export function rssSlope(samples,warmS=300){
  const d=samples.filter(s=>s.t_s>=warmS);if(d.length<6)return {n:d.length,slopeMBh:NaN,pass:false,note:'too few samples'};
  const n=d.length,xs=d.map(s=>s.t_s/3600),ys=d.map(s=>s.rssMB),mx=xs.reduce((a,b)=>a+b)/n,my=ys.reduce((a,b)=>a+b)/n;
  const sxx=xs.reduce((a,x)=>a+(x-mx)**2,0),b=xs.reduce((a,x,i)=>a+(x-mx)*(ys[i]-my),0)/sxx,a0=my-b*mx;
  const res=ys.reduce((s,y,i)=>s+(y-a0-b*xs[i])**2,0),se=Math.sqrt(res/Math.max(1,n-2)/sxx);
  return {n,slopeMBh:b,ci95:1.96*se,pass:b-1.96*se<=0||b<=10}}
