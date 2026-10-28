// Narrow Bend-export substitute for source adapter controls, never an emitter.
export const observed={recv:0,act:0,tick:0,register:0};
export function j(v){
 if(v===null||v===undefined)return{$:'Null'};
 if(typeof v==='string')return{$:'Str',text:v};
 if(typeof v==='boolean')return{$:'Flag',value:v};
 if(typeof v==='number')return{$:'Num',raw:String(v)};
 if(Array.isArray(v)){let tail={$:'End'};for(let i=v.length-1;i>=0;i--)tail={$:'Item',head:j(v[i]),tail};return{$:'Arr',items:tail};}
 let tail={$:'End'};for(const [key,value]of Object.entries(v).reverse())tail={$:'Field',key,value:j(value),tail};return{$:'Obj',fields:tail};
}
export function p(v){if(v.$==='Null')return null;if(v.$==='Str')return v.text;if(v.$==='Num')return Number(v.raw);if(v.$==='Flag')return v.value;if(v.$==='Arr'){const a=[];for(let c=v.items;c.$==='Item';c=c.tail)a.push(p(c.head));return a;}const o={};for(let c=v.fields;c.$==='Field';c=c.tail)Object.defineProperty(o,c.key,{value:p(c.value),enumerable:true,writable:true,configurable:true});return o;}
const base=()=>({sel:'',drafts:{},open:{},adding:false,panel:false,info:{},rid:1,outbox:{}});
export function screen(hub='h',sel='a'){return{hub,sel,online:true,version:'test',error:'',note:'',empty:'',hubs:[],found:[],projects:[],bots:[],rooms:[],thread:sel?{id:sel,draft:'',entries:[{id:'e',kind:'assistant',text:'current',blocks:[]}]}:null};}
const noRoute=s=>{const n={...s};delete n.route;return n;};
const result=hs=>({hubs:hs,cmds:null});
export default {
 json_tags:()=>j([null,false,0,'',{x:null}]),cbor_keys:()=>({$:'Nil'}),cbor_words:()=>({$:'Nil'}),
 native_locale:(choice,os)=>choice==='auto'?(os||'en'):choice,
 init:()=>({keys:[],cur:'',sessions:{}}),hubs:(hs,text)=>{hs={...hs,keys:JSON.parse(text)};hs.cur=hs.keys.includes(hs.cur)?hs.cur:hs.keys[0]||'';return hs;},
 local:(hs,k,v)=>({...hs}),current:hs=>hs.cur,lean:hs=>hs,
 session:(hs,key)=>j(hs.sessions[key]||base()),
 session_target:(hs,cur,a,v)=>hs.keys.find(k=>v.startsWith(k+'|'))||cur,
 session_local:(text,key,value)=>{const s=noRoute(JSON.parse(text));s.drafts={...s.drafts,['@lang'+(key==='lang'?'':'.'+key.replace('lang-',''))]:value};return j(s);},
 session_prepare:(text,input)=>j(noRoute({...JSON.parse(text),drafts:JSON.parse(input).drafts})),
 session_reply:(text)=>j(noRoute(JSON.parse(text))),
 session_adopt:(hs,key,text)=>({...hs,sessions:{...hs.sessions,[key]:JSON.parse(text)}}),session_draft:(text,draft)=>{const s=noRoute(JSON.parse(text));s.drafts={...s.drafts,[s.sel]:draft};return j(s);},
 session_reading:text=>j(JSON.parse(text)),session_issued:()=>j([]),session_failed:()=>j([]),session_mail_action:()=>false,
 thin_target:text=>{const s=JSON.parse(text);return j(s.route?{kind:'bot',id:s.route.bot}:s.sel?{kind:'thread',id:s.sel}:{kind:'recent',id:''});},
 thin_choice:(text,a,v)=>{const s=noRoute(JSON.parse(text));if(a==='select')s.sel=v.includes('|')?v.slice(v.indexOf('|')+1):v;if(a==='draft')s.drafts={...s.drafts,[s.sel]:v};return j(s);},
 json_parse:text=>j(JSON.parse(text)),
 thin_partial:(shown,window)=>{const s=p(shown),w=p(window);if(s.thread)s.thread.entries=s.thread.entries.map(e=>({...e,...(w.partial.find(x=>x.id===e.id)?{partial:w.partial.find(x=>x.id===e.id)}:{})}));return j(s);},
 thin_shown_draft:(overlay,text)=>{const o=p(overlay),s=JSON.parse(text);if(o.j.thread)o.j.thread.draft=s.drafts[s.sel]||'';return j(o);},
 thin_presentation:(hs,overlays,cur)=>{const o=p(overlays).find(o=>o.key===cur);return j({...o.j,hub:cur,sel:o.j.sel?cur+'|'+o.j.sel:'',hubs:hs.keys.map(key=>({key}))});},
 thin_search:(text,query)=>{const s=JSON.parse(text),q=JSON.parse(query);return j({...s,find:{mode:'search',query:q.query,rows:q.rows.map(r=>({label:r.text,action:'select',value:s.hub+'|'+r.thread,kind:'thread'}))}});},
 screen_list_in:hs=>j(screen(hs.cur,'')),screenj:hs=>j(screen(hs.cur,'')),focus:()=>j(null),live_text:()=>'',live_nodes:text=>j([{text}]),
 wire:text=>text,alerts:()=> '[]',restore:hs=>hs,
 key_req:packed=>{const [n,v]=packed.split('\u001f');return JSON.stringify(!n?{error:'Give the key a name'}:!v?{error:'Paste the key itself'}:{rpc:JSON.stringify({id:0,m:'bots.voice',p:{op:'api-add',name:n,value:v}}),name:n});},
 recv:()=>{observed.recv++;throw Error('Canonical recv reached');},act:()=>{observed.act++;throw Error('Canonical act reached');},
 online:()=>{throw Error('Canonical online reached');},online_same:()=>false,tick:()=>{observed.tick++;throw Error('Canonical tick reached');},
 register:()=>{observed.register++;throw Error('Canonical register reached');},register_at:(hs,key,platform,token,kind,thread,env,bundle)=>{observed.register++;const session=hs.sessions[key]||base(),id=session.rid,text=JSON.stringify({id,m:'device.register',p:{platform,token,kind,thread,env,bundle}});return{hubs:{...hs,sessions:{...hs.sessions,[key]:{...session,rid:id+1,outbox:{...session.outbox,[id]:text}}}},cmds:{$:'Con',head:{hub:key,cmd:{$:'Send',text}},tail:{$:'Nil'}}};},
 t:(hs,key)=>key,
};
