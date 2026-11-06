import {readFileSync} from 'node:fs';
import {cbor, plain} from './wire-codec.js';
const source=readFileSync(new URL('../native/host/src/wire_dict.rs',import.meta.url),'utf8');
const dictionary=name=>JSON.parse(source.match(new RegExp(`pub const ${name.toUpperCase()}: &\\[&str\\] = &(\\[[\\s\\S]*?\\]);`))[1]);
export const decode=bytes=>plain(cbor(new Uint8Array(bytes),dictionary('keys'),dictionary('words')));
export function encode(value){
 const bytes=[]; const argument=(major,n)=>{if(n<24)bytes.push(major*32+n);else if(n<256)bytes.push(major*32+24,n);else if(n<65536)bytes.push(major*32+25,n>>>8,n&255);else bytes.push(major*32+26,(n>>>24)&255,(n>>>16)&255,(n>>>8)&255,n&255);};
 const go=v=>{if(v===null)bytes.push(246);else if(typeof v==='boolean')bytes.push(v?245:244);else if(typeof v==='number'){if(Number.isInteger(v)&&v>=0&&v<=4294967295)argument(0,v);else{bytes.push(199);go(String(v));}}else if(typeof v==='string'){const b=new TextEncoder().encode(v);argument(3,b.length);bytes.push(...b);}else if(Array.isArray(v)){argument(4,v.length);v.forEach(go);}else{argument(5,Object.keys(v).length);for(const[k,x]of Object.entries(v)){go(k);go(x);}}};go(value);return new Uint8Array(bytes);
}
