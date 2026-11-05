import {mkdirSync,rmSync} from 'node:fs';import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';import {decode} from '../wire.mjs';
const R='/home/h/Documents/i2cjak/backplane-gpui',root=R+'/build/gpui/slim-run/hello';rmSync(root,{recursive:true,force:true});mkdirSync(root,{recursive:true});makeStandins(root+'/bin');
const h=await startHub({bin:R+'/build/gpui/backplane-serve',home:root+'/home',port:3826,standins:root+'/bin'});
const r=await fetch(h.url+'/hello');console.log(r.status,[...r.headers]);const b=new Uint8Array(await r.arrayBuffer());console.log(Buffer.from(b).toString('hex').slice(0,200));console.log(JSON.stringify(decode(b)));
const r2=await fetch(h.url+'/ws');console.log('ws no token',r2.status);
stopHub(h);await sleep(300);process.exit(0);
