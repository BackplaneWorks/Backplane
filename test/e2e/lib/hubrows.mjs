// Shared by the hub-level e2e drivers: stand-ins that log their argv, the data/1 socket, a setting writer, small helpers.
import {mkdirSync,writeFileSync,readdirSync,readFileSync,chmodSync,existsSync,renameSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep,client} from '../../perf/lib.mjs';
export {sleep,startHub,stopHub,client};
/** The claude stand-in, wrapped: every start writes its argv (NUL separated) to $ARGV_DIR, then runs the usual stand-in. */
export function argvStandins(dir){makeStandins(dir);renameSync(dir+'/claude',dir+'/claude.inner');
  writeFileSync(dir+'/claude',`#!/bin/sh\nd="$ARGV_DIR"; [ -n "$d" ] && printf '%s\\0' "$@" > "$d/$(date +%s%N)-$$.argv"\nexec "$(dirname "$0")/claude.inner" "$@"\n`);chmodSync(dir+'/claude',0o755)}
export const argvs=dir=>{try{return readdirSync(dir).filter(f=>f.endsWith('.argv')).sort().map(f=>readFileSync(dir+'/'+f,'utf8').split('\0').slice(0,-1))}catch{return []}};
export const argOf=(a,flag)=>{const i=a.indexOf(flag);return i>=0?a[i+1]:undefined};
export async function sock(hub,port){const ws=new WebSocket(`ws://127.0.0.1:${port}/ws?data=1&enc=json&token=${hub.token}`);const c={ws,id:0,w:new Map(),push:[],hello:null};
  ws.onmessage=e=>{const o=JSON.parse(e.data);if(o.t==='hello')c.hello=o;else if(o.t==='reply'){c.w.get(o.id)?.(o);c.w.delete(o.id)}else c.push.push(o)};
  await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j});
  c.rpc=(m,p={},ms=20000)=>new Promise(r=>{const i=++c.id;const t=setTimeout(()=>{c.w.delete(i);r({ok:false,error:'timeout'})},ms);c.w.set(i,o=>{clearTimeout(t);r(o)});ws.send(JSON.stringify({id:i,m,p}))});
  for(let i=0;i<50&&!c.hello;i++)await sleep(100);return c}
export const idof=x=>x&&typeof x==='object'?x.id:x;
/** setting.set over the hub's own socket (data/1 does not carry it) */
export async function setting(hub,key,value){const k=await client(hub);const r=await k.rpc('setting.set',{key,value},10000);k.ws.close();return r?.o?.ok===true}
export async function waitFor(f,ms=20000,step=150){const end=Date.now()+ms;for(;;){const v=await f();if(v)return v;if(Date.now()>end)return undefined;await sleep(step)}}
