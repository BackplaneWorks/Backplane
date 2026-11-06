// What the hub holds, read over its data/1 socket (JSON), for a journey's `expect {hub: ...}` checks: the client's picture is not the
// evidence that a thread was pinned, the hub's state is. Counts are compared before and after a journey (the sidebar order the client
// uses is not the creation order, so "thread 3" cannot be named from here).
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
export async function dataSocket(url,token){const u=new URL(url);const ws=new WebSocket(`ws://${u.host}/ws?data=1&enc=json&token=${encodeURIComponent(token)}`);
  const c={ws,id:0,w:new Map()};ws.onmessage=e=>{let o;try{o=JSON.parse(e.data)}catch{return}if(o.t==='reply'){c.w.get(o.id)?.(o);c.w.delete(o.id)}};
  await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});
  c.rpc=(m,p={},ms=15000)=>new Promise(r=>{const i=++c.id;const t=setTimeout(()=>{c.w.delete(i);r({ok:false,error:'timeout'})},ms);c.w.set(i,o=>{clearTimeout(t);r(o)});ws.send(JSON.stringify({id:i,m,p}))});
  await sleep(200);return c}
export async function hubCounts(fx){const s=await dataSocket(fx.hub.url,fx.hub.token);
  try{const live=(await s.rpc('threads.list',{limit:200,archived:false})).result?.threads||[];const arch=(await s.rpc('threads.list',{limit:200,archived:true})).result?.threads||[];
    return {threads:live.length,archived:arch.length,pinned:live.filter(t=>t.pinned).length,settled:live.filter(t=>t.settled).length,working:live.filter(t=>t.state==='working').length,failed:live.filter(t=>t.state==='failed').length,titles:live.map(t=>t.title)}}
  finally{try{s.ws.close()}catch{}}}
// expect {hub:{pinned:+1, archived:+1, threads:-1, title:"x"}}: numbers are changes against `before`, a string must be one of the titles
export function checkHub(want,before,after){const out=[];
  for(const [k,v] of Object.entries(want)){if(typeof v==='number'){const got=(after[k]??NaN)-(before[k]??NaN);out.push({what:`hub ${k} ${v>=0?'+':''}${v}`,got:`${got>=0?'+':''}${got}`,ok:got===v})}
    else out.push({what:`hub has a thread titled "${v}"`,got:after.titles.includes(v)?'yes':'no',ok:after.titles.includes(v)})}
  return out}
