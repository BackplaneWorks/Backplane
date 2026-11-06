// node compare.mjs A.json B.json : where do two recorded sessions differ? (identity fields are ignored)
import {readFileSync} from 'node:fs';
const [a,b]=process.argv.slice(2).map(f=>JSON.parse(readFileSync(f,'utf8').replace(/127\.0\.0\.1:38\d\d/g,'HUB')));
const ignore=new Set(['generation','epoch','viewId','requestRevision','rid','hub','viewRevision','at','seq','home','listen','origin','cid','revision','machineUsage']);
const diffs=[];
function walk(x,y,path){
 if(path.split('.').some(k=>ignore.has(k)))return;
 if(typeof x!==typeof y||Array.isArray(x)!==Array.isArray(y)||(x===null)!==(y===null)){diffs.push([path,'type',typeof x,typeof y]);return}
 if(x&&typeof x==='object'){
  if(Array.isArray(x)){if(x.length!==y.length)diffs.push([path,'length',x.length,y.length]);for(let i=0;i<Math.min(x.length,y.length);i++)walk(x[i],y[i],path+'.'+i);return}
  for(const k of new Set([...Object.keys(x),...Object.keys(y)])){if(!(k in x))diffs.push([path+'.'+k,'only-B']);else if(!(k in y))diffs.push([path+'.'+k,'only-A']);else walk(x[k],y[k],path+'.'+k)}
  return}
 if(x!==y)diffs.push([path,'value',String(x).slice(0,60),String(y).slice(0,60)]);
}
const replies=s=>s.frames.filter(f=>f.dir==='in'&&f.o.t==='reply'&&f.o.ok&&f.o.result?.viewId!==undefined).map(f=>f.o.result);
const ra=replies(a),rb=replies(b);
console.log(`A ${a.name}: ${ra.length} view replies; B ${b.name}: ${rb.length}`);
const shape=s=>s.frames.filter(f=>f.dir==='in').map(f=>f.o.t+(f.o.result?'['+Object.keys(f.o.result).join(',')+']':'')).join(' | ');
console.log('A frame sequence:',shape(a).slice(0,400));console.log('B frame sequence:',shape(b).slice(0,400));
for(let i=0;i<Math.min(ra.length,rb.length);i++){
 diffs.length=0;walk(ra[i],rb[i],'');
 const byTop={};for(const d of diffs){const k=d[0].split('.').slice(1,3).join('.');byTop[k]=(byTop[k]||0)+1}
 console.log(`reply ${i}: ${diffs.length} differences`,JSON.stringify(Object.entries(byTop).sort((p,q)=>q[1]-p[1]).slice(0,12)));
 for(const d of diffs.slice(0,6))console.log('   ',JSON.stringify(d));
}
if(process.env.SHOW){
 const I=Number(process.env.I||0);diffs.length=0;walk(ra[I],rb[I],'');
 const seen=new Set();
 for(const d of diffs){const k=d[0].replace(/\.\d+/g,'.#');if(seen.has(k))continue;seen.add(k);console.log(JSON.stringify(d))}
}
