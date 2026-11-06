export async function cdp(port,match){
 const list=await (await fetch(`http://127.0.0.1:${port}/json`)).json();
 const t=list.find(x=>x.type==='page'&&match(x.url));if(!t)throw Error('no page');
 const ws=new WebSocket(t.webSocketDebuggerUrl);await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});
 let id=0;const pend=new Map();ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id&&pend.has(m.id)){pend.get(m.id)(m);pend.delete(m.id)}};
 const send=(method,params={})=>new Promise(r=>{const i=++id;pend.set(i,r);ws.send(JSON.stringify({id:i,method,params}))});
 const ev=async(expr,aw=true)=>{const r=await send('Runtime.evaluate',{expression:expr,returnByValue:true,awaitPromise:aw});if(r.result?.exceptionDetails)throw Error(JSON.stringify(r.result.exceptionDetails).slice(0,300));return r.result?.result?.value};
 return {send,ev,close:()=>ws.close()};
}
