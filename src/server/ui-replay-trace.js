// Opt-in capture hook for a hub's decoded RPC boundary. No IO, message text,
// paths, raw identifiers, content hashes, timestamps or network transmission.
// A caller owns consent, persistence and export; this prototype is not
// enabled on installed hubs. Passing an entire request never copies it.
const kinds=new Set(['select','bot','send','draft','reply','scroll','page','reconnect','stream','view-error','newest']);
export const TRACE_LIMITS=Object.freeze({events:256,ids:128,bytes:65536,deltaMs:60000});
export function makeReplayTrace({enabled=false,now=()=>performance.now()}={}){
  let rows=[],symbols=new Map(),last=now(),dropped=0,serial=0;
  function alias(id){
    if(typeof id!=='string'||id.length>1024)return 0;
    if(symbols.has(id))return symbols.get(id);
    if(symbols.size>=TRACE_LIMITS.ids)return 0;
    symbols.set(id,++serial);return serial;
  }
  function record(kind,ids=[]){
    if(!enabled||!kinds.has(kind)||!Array.isArray(ids))return false;
    const at=now(),dt=Math.max(0,Math.min(TRACE_LIMITS.deltaMs,Math.round(at-last)));last=at;
    const row={kind,ids:ids.slice(0,2).map(alias),dt};
    if(rows.length===TRACE_LIMITS.events){rows.shift();dropped++;}
    rows.push(row);return true;
  }
  return {record,
    rpc(request){
      const method=request?.m,p=request?.p;
      if(method==='view.page')return record('page',[p?.target?.id]);
      if(method==='view.open')return record('select',[p?.target?.id]);
      if(method==='turn.start')return record('send',[p?.thread,String(request.id)]);
      if(method==='view.action'&&kinds.has(p?.action)){
        // value can be arbitrary user text. Only select/bot values are IDs;
        // draft/send/reply payloads never enter even the ephemeral id map.
        const target=['select','bot'].includes(p.action)?p.value:p?.session?.sel;
        return record(p.action,[target]);
      }return false;
    },
    clear(){rows=[];symbols.clear();last=now();dropped=0;serial=0;},
    export(){const out={version:1,dropped,events:rows.map(r=>({...r,ids:r.ids.slice()}))};
      if(new TextEncoder().encode(JSON.stringify(out)).length>TRACE_LIMITS.bytes)throw Error('trace bound');return out;}
  };
}

export function validateReplayTrace(value){
  if(!value||Object.keys(value).sort().join(',')!=='dropped,events,version'||value.version!==1||
    !Number.isSafeInteger(value.dropped)||value.dropped<0||!Array.isArray(value.events)||value.events.length>TRACE_LIMITS.events)return false;
  if(new TextEncoder().encode(JSON.stringify(value)).length>TRACE_LIMITS.bytes)return false;
  return value.events.every(r=>r&&Object.keys(r).sort().join(',')==='dt,ids,kind'&&kinds.has(r.kind)&&
    Number.isInteger(r.dt)&&r.dt>=0&&r.dt<=TRACE_LIMITS.deltaMs&&Array.isArray(r.ids)&&r.ids.length<=2&&
    r.ids.every(id=>Number.isInteger(id)&&id>=0&&id<=TRACE_LIMITS.ids));
}
